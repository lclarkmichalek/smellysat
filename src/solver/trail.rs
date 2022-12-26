use lazy_static::lazy_static;

use crate::instance::*;
use core::fmt;

use super::assignment_set::LiteralSet;

lazy_static! {
    static ref EMPTY_LIT_SET: LiteralSet = LiteralSet::new();
}

/// Stores the traversal path of the DFS. Should be the source of truth for what needs to be reverted upon backtrack.
/// Note: we do not have a root node. An untraversed path has no trail

#[derive(Clone)]
pub(crate) struct Trail {
    // Trail will never be empty - the first element stores decision level 0
    trail: Vec<TrailEntry>,
    cumulative_assignment: LiteralSet,
}

impl Trail {
    pub(crate) fn new() -> Trail {
        Trail {
            trail: vec![TrailEntry::new(None)],
            cumulative_assignment: LiteralSet::new(),
        }
    }

    /// The number of decisions in the current assignment
    pub(crate) fn current_decision_level(&self) -> usize {
        self.trail.len() - 1
    }

    pub(crate) fn assignment(&self) -> &LiteralSet {
        &self.cumulative_assignment
    }

    /// In the case of no decision being made prior to this function being called, we return the initial assignment set
    pub(crate) fn assignments_since_last_decision(&self) -> &LiteralSet {
        match self.trail.last() {
            Some(entry) => &entry.all,
            None => &EMPTY_LIT_SET,
        }
    }

    pub(crate) fn last_decision(&self) -> Option<Literal> {
        self.trail.last().unwrap().decision
    }

    // Records a step in the DFS search
    pub(crate) fn add_decision(&mut self, literal: Literal) {
        self.require_unset(literal);

        self.cumulative_assignment.add(literal);
        self.trail.push(TrailEntry::new(Some(literal)));
    }

    // Records an inferred assignment
    pub(crate) fn add_inferred(&mut self, literal: Literal) {
        self.require_unset(literal);

        self.cumulative_assignment.add(literal);
        match self.trail.last_mut() {
            Some(last_step) => {
                last_step.inferred.push(literal);
                last_step.all.add(literal);
            }
            None => {}
        }
    }

    /// Finds the last point to backtrack to according to the strategy (see find_backtrack_point_dfs)
    /// and drops the paths. Builds a list of the assignments (to allow state rollbacks), and the
    /// last decision after the backtrack point (to allow pivots).
    ///
    /// The DFSPatt state (such as the assignment) will be rolled back as part of this.
    pub(crate) fn backtrack(&mut self, pivot: usize) -> BacktrackResult {
        let result = self.execute_backtrack(pivot);

        for &literal in result.assignments.iter() {
            self.cumulative_assignment.remove(literal);
        }

        result
    }

    fn execute_backtrack(&mut self, point: usize) -> BacktrackResult {
        let dropped = self.trail.drain(point..).collect::<Vec<_>>();
        let last_decision = dropped.first().map(|e| e.decision).flatten();
        let mut assignments = vec![];
        for entry in dropped.into_iter() {
            assignments.extend(entry.inferred);
            if let Some(chosen) = entry.decision {
                assignments.push(chosen);
            }
        }

        BacktrackResult {
            assignments,
            last_decision,
        }
    }

    #[cfg(debug_assertions)]
    fn require_unset(&self, literal: Literal) {
        if self.cumulative_assignment.contains(literal) {
            panic!("{:?} already in assignment", literal);
        }
        if self.cumulative_assignment.contains(literal.invert()) {
            panic!("inverse {:?} already in assignment", literal.invert())
        }
    }

    #[cfg(not(debug_assertions))]
    fn require_unset(&self, _literal: Literal) {}

    pub(crate) fn search_path(&self) -> &Vec<TrailEntry> {
        &self.trail
    }
}

impl fmt::Debug for Trail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Trail {{ depth={:?}, assignment=[{:?}] }}",
            self.current_decision_level(),
            self.assignment()
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TrailEntry {
    pub(crate) decision: Option<Literal>,
    pub(crate) inferred: Vec<Literal>,
    pub(crate) all: LiteralSet,
}

impl TrailEntry {
    fn new(literal: Option<Literal>) -> TrailEntry {
        let mut ls = LiteralSet::new();
        if let Some(lit) = literal {
            ls.add(lit);
        }
        TrailEntry {
            decision: literal,
            inferred: vec![],
            all: ls,
        }
    }
}

#[derive(Clone)]
pub(crate) struct BacktrackResult {
    pub(crate) assignments: Vec<Literal>,
    // The last decision taken before the backtrack. None if the backtrack did not actually reverse any steps
    pub(crate) last_decision: Option<Literal>,
}

#[cfg(test)]
mod test {
    use crate::solver::{
        backtrack::{BacktrackStrategy, Conflict, DumbBacktrackStrategy},
        clause_store::ClauseRef,
        trail::*,
    };

    #[test]
    fn test_bookkeeping() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);

        let mut sp = Trail::new();

        sp.add_decision(Literal::new(a, true));
        assert_eq!(sp.current_decision_level(), 1);
        assert_eq!(sp.assignment().size(), 1);

        sp.add_inferred(Literal::new(b, true));
        assert_eq!(sp.current_decision_level(), 1);
        assert_eq!(sp.assignment().size(), 2);

        sp.add_inferred(Literal::new(c, true));
        assert_eq!(sp.current_decision_level(), 1);
        assert_eq!(sp.assignment().size(), 3);
    }

    // Primarily tests that we are cleaning up the DFSPath assignments etc when we rollback
    #[test]
    fn test_backtrack_rollback() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);
        let notc = Literal::new(c, false);

        let mut path = Trail::new();
        let strategy = DumbBacktrackStrategy {};

        path.add_decision(Literal::new(a, true));
        path.add_inferred(notc);
        let conflict = Conflict {
            conflicting_decision: None,
            conflicting_literal: notc,
            conflicting_clause: ClauseRef::Unit(notc),
        };

        let backtrack_res = path.backtrack(
            strategy
                .find_backtrack_point(path.search_path(), &conflict)
                .unwrap(),
        );
        assert_eq!(path.current_decision_level(), 0);
        assert_eq!(backtrack_res.assignments, vec![notc, Literal::new(a, true)]);
        assert_eq!(backtrack_res.last_decision, Some(Literal::new(a, true)));

        // Here we will backtrack up to A, as we've explored B's true path earlier
        path.add_decision(Literal::new(a, true));
        path.add_decision(Literal::new(b, false));
        path.add_inferred(notc);
        let backtrack_res = path.backtrack(
            strategy
                .find_backtrack_point(path.search_path(), &conflict)
                .unwrap(),
        );
        assert_eq!(path.current_decision_level(), 0);
        assert_eq!(
            backtrack_res.assignments,
            vec![Literal::new(a, true), notc, Literal::new(b, false)]
        );
        assert_eq!(backtrack_res.last_decision, Some(Literal::new(a, true)));
    }
}
