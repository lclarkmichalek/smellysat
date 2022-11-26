use crate::instance::*;
use core::fmt;

use super::assignment_set::LiteralSet;

/// Stores the traversal path of the DFS. Should be the source of truth for what needs to be reverted upon backtrack.
/// Note: we do not have a root node. An untraversed path has an empty

#[derive(Clone)]
pub(crate) struct DFSPath {
    path: Vec<DFSPathEntry>,
    initial_assignment: LiteralSet,
    assignment: LiteralSet,
}

impl DFSPath {
    // Takes an initial assignment that cannot be backtracked
    pub(crate) fn new(initial_assignment: LiteralSet) -> DFSPath {
        DFSPath {
            path: vec![],
            assignment: initial_assignment.clone(),
            initial_assignment,
        }
    }

    /// The number of decisions taken, minus decisions backtracked
    pub(crate) fn depth(&self) -> usize {
        self.path.len()
    }

    pub(crate) fn assignment(&self) -> &LiteralSet {
        &self.assignment
    }

    /// In the case of no decision being made prior to this function being called, we return the initial assignment set
    pub(crate) fn assignments_since_last_decision(&self) -> &LiteralSet {
        match self.path.last() {
            Some(entry) => &entry.all,
            None => &self.initial_assignment,
        }
    }

    // Records a step in the DFS search
    pub(crate) fn add_decision(&mut self, literal: Literal) {
        self.require_unset(literal);

        self.assignment.add(literal);
        self.path.push(DFSPathEntry::new(literal));
    }

    // Records an inferred assignment
    pub(crate) fn add_inferred(&mut self, literal: Literal) {
        self.require_unset(literal);

        self.assignment.add(literal);
        match self.path.last_mut() {
            Some(last_step) => {
                last_step.inferred.push(literal);
                last_step.all.add(literal);
            }
            None => {
                // Ok, now shits getting fucky. If we've inferred something without taking a step,
                // then presumably we're in the first round of unit prop. Cosequentially, it's
                // really part of the initial assignment.
                self.initial_assignment.add(literal);
            }
        }
    }

    /// Finds the last point to backtrack to according to the strategy (see find_backtrack_point_dfs)
    /// and drops the paths. Builds a list of the assignments (to allow state rollbacks), and the
    /// last decision after the backtrack point (to allow pivots).
    ///
    /// The DFSPatth state (such as the assignment) will be rolled back as part of this.
    pub(crate) fn backtrack(&mut self) -> BacktrackResult {
        let backtrack_point = self.find_backtrack_point_dfs();

        let result = match backtrack_point {
            None => {
                return BacktrackResult {
                    assignments: vec![],
                    last_decision: None,
                }
            }
            Some(ix) => self.execute_backtrack(ix),
        };

        for &literal in result.assignments.iter() {
            self.assignment.remove(literal);
        }

        result
    }

    fn execute_backtrack(&mut self, point: usize) -> BacktrackResult {
        let dropped = self.path.drain(point..).collect::<Vec<_>>();
        let last_decision = dropped.first().map(|e| e.chosen);
        let mut assignments = vec![];
        for entry in dropped.into_iter() {
            assignments.extend(entry.inferred);
            assignments.push(entry.chosen);
        }

        BacktrackResult {
            assignments,
            last_decision,
        }
    }

    // A dumb stupid dfs style backtrack strategy - look for the last path where we didn't go "left" - i.e. try the false path
    fn find_backtrack_point_dfs(&self) -> Option<usize> {
        for (ix, entry) in self.path.iter().enumerate().rev() {
            // If this was a left hand path (X=true), go down the right hand path this time.
            // Else, continue
            if entry.chosen.polarity() {
                return Some(ix);
            }
        }
        None
    }

    #[cfg(debug_assertions)]
    fn require_unset(&self, literal: Literal) {
        if self.assignment.contains(literal) {
            panic!("{:?} already in assignment", literal);
        }
        if self.assignment.contains(literal.invert()) {
            panic!("inverse {:?} already in assignment", literal.invert())
        }
    }

    #[cfg(not(debug_assertions))]
    fn require_unset(&self, literal: Literal) {}
}

impl fmt::Debug for DFSPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DFSPath {{ depth={:?}, assignment=[{:?}] }}",
            self.depth(),
            self.assignment()
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DFSPathEntry {
    pub(crate) chosen: Literal,
    pub(crate) inferred: Vec<Literal>,
    pub(crate) all: LiteralSet,
}

impl DFSPathEntry {
    fn new(literal: Literal) -> DFSPathEntry {
        let mut ls = LiteralSet::new();
        ls.add(literal);
        DFSPathEntry {
            chosen: literal,
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
    use crate::{solver::dfs_path::*};

    #[test]
    fn test_bookkeeping() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);

        let mut sp = DFSPath::new(LiteralSet::new());

        sp.add_decision(Literal::new(a, true));
        assert_eq!(sp.depth(), 1);
        assert_eq!(sp.assignment().size(), 1);

        sp.add_inferred(Literal::new(b, true));
        assert_eq!(sp.depth(), 1);
        assert_eq!(sp.assignment().size(), 2);

        sp.add_inferred(Literal::new(c, true));
        assert_eq!(sp.depth(), 1);
        assert_eq!(sp.assignment().size(), 3);
    }

    #[test]
    fn test_backtrack() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);
        let notc = Literal::new(c, false);

        let mut path = DFSPath::new(LiteralSet::new());

        path.add_decision(Literal::new(a, true));
        path.add_inferred(notc);
        let backtrack_res = path.backtrack();
        assert_eq!(path.depth(), 0);
        assert_eq!(backtrack_res.assignments, vec![notc, Literal::new(a, true)]);
        assert_eq!(backtrack_res.last_decision, Some(Literal::new(a, true)));

        // Here we will backtrack up to A, as we've explored B's true path earlier
        path.add_decision(Literal::new(a, true));
        path.add_decision(Literal::new(b, false));
        path.add_inferred(notc);
        let backtrack_res = path.backtrack();
        assert_eq!(path.depth(), 0);
        assert_eq!(
            backtrack_res.assignments,
            vec![Literal::new(a, true), notc, Literal::new(b, false)]
        );
        assert_eq!(backtrack_res.last_decision, Some(Literal::new(a, true)));

        // In this case, the tree is fully explored, and backtracking fails
        path.add_decision(Literal::new(a, false));
        path.add_decision(Literal::new(b, false));
        let backtrack_res = path.backtrack();
        assert_eq!(backtrack_res.last_decision, None);
    }
}
