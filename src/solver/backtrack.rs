use log::trace;

use crate::instance::{Clause, Literal};

use super::{
    assignment_set::LiteralSet,
    dfs_path::{BacktrackResult, DFSPathEntry},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict<'c> {
    pub(crate) conflicting_decision: Option<Literal>,
    pub(crate) conflicting_literal: Literal,
    pub(crate) conflicting_clause: &'c Clause,
}

pub(crate) trait BacktrackStrategy {
    /// Calculates how far we should roll back the search tree
    fn find_backtrack_point(&self, path: &Vec<DFSPathEntry>, conflict: &Conflict) -> Option<usize>;
    // Optionally specifies the next step that should be taken after the rollback
    fn next_decision(
        &self,
        path: &Vec<DFSPathEntry>,
        conflict: &Conflict,
        result: &BacktrackResult,
    ) -> Option<Literal>;
}

/// A naive dfs backtrack strategy - look for the last path where we didn't go "left" - i.e. try the false path
pub(crate) struct DumbBacktrackStrategy {}

impl BacktrackStrategy for DumbBacktrackStrategy {
    fn find_backtrack_point(
        &self,
        path: &Vec<DFSPathEntry>,
        _conflict: &Conflict,
    ) -> Option<usize> {
        for (ix, entry) in path.iter().enumerate().rev() {
            // If this was a left hand path (X=true), go down the right hand path this time.
            // Else, continue
            if entry.chosen.polarity() {
                return Some(ix);
            }
        }
        None
    }

    // Go down the other path
    fn next_decision(
        &self,
        _path: &Vec<DFSPathEntry>,
        _conflict: &Conflict,
        result: &BacktrackResult,
    ) -> Option<Literal> {
        result.last_decision.map(|decision| decision.invert())
    }
}
