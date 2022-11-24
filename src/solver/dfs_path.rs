use core::fmt;

use crate::instance::*;

use super::assignment_set::LiteralSet;

/// Stores the traversal path of the DFS. Should be the source of truth for what needs to be reverted upon backtrack.
/// Note: we do not have a root node. An untraversed path has an empty 

#[derive(Clone)]
pub(crate) struct DFSPath {
    path: Vec<DFSPathEntry>,
    assignment: LiteralSet,
}

impl DFSPath {
    // Takes an initial assignment that cannot be backtracked
    pub(crate) fn new(initial_assignment: LiteralSet) -> DFSPath {
        DFSPath {
            path: vec![],
            assignment: initial_assignment
        }
    }

    /// The number of decisions taken, minus decisions backtracked
    pub(crate) fn depth(&self) -> usize {
        self.path.len()
    }

    pub(crate) fn assignment(&self) -> &LiteralSet  {
        &self.assignment
    }

    pub(crate) fn assignments_since_last_decision(&self) -> &LiteralSet {
        &self.path.last().unwrap().all
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
        let last_step = self.path.last_mut().unwrap();
        last_step.inferred.push(literal);
        last_step.all.add(literal);
    }

    /// Finds the last point to backtrack to according to the strategy (see find_backtrack_point_dfs)
    /// and drops the paths. Builds a list of the assignments (to allow state rollbacks), and the
    /// last decision after the backtrack point (to allow pivots).
    /// 
    /// The DFSPatth state (such as the assignment) will be rolled back as part of this.
    pub(crate) fn backtrack(&mut self) -> BacktrackResult {
        let backtrack_point = self.find_backtrack_point_dfs();

        let result = match backtrack_point {
            None => return BacktrackResult {assignments: vec![], last_decision: None},
            Some(ix) => self.execute_backtrack(ix)
        };

        for &literal in result.assignments.iter() {
            self.assignment.remove(literal);
        }

        result
    }

    fn execute_backtrack(&mut self, point: usize) -> BacktrackResult {
        let dropped = self.path.drain(point..).collect::<Vec<_>>();
        let last_decision = dropped.last().map(|e| e.chosen);
        let mut assignments = vec![];
        for entry in dropped.into_iter() {
            assignments.extend(entry.inferred);
            assignments.push(entry.chosen);
        }

        BacktrackResult { assignments, last_decision }
    }

    // A dumb stupid dfs style backtrack strategy - look for the last path where we didn't go "left" - i.e. try the false path
    fn find_backtrack_point_dfs(&self) -> Option<usize> {
        for (ix, entry) in self.path.iter().enumerate().rev() {
            // If this was a left hand path (X=true), go down the right hand path this time.
            // Else, continue
            if entry.chosen.polarity() {
                return Some(ix)
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

#[derive(Clone)]
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
    use crate::{instance::*, solver::dfs_path::*};

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
}