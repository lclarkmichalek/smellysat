use log::trace;

use crate::instance::Literal;

use super::{
    clause_store::ClauseRef,
    trail::{BacktrackResult, TrailEntry},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub(crate) conflicting_decision: Option<Literal>,
    pub(crate) conflicting_literal: Literal,
    pub(crate) conflicting_clause: ClauseRef,
}

pub(crate) trait BacktrackStrategy {
    /// Calculates how far we should roll back the search tree
    fn find_backtrack_point(&self, path: &Vec<TrailEntry>, conflict: &Conflict) -> Option<usize>;
    // Optionally specifies the next step that should be taken after the rollback
    fn next_decision(
        &self,
        path: &Vec<TrailEntry>,
        conflict: &Conflict,
        result: &BacktrackResult,
    ) -> Option<Literal>;
}

/// A naive dfs backtrack strategy - look for the last path where we didn't go "left" - i.e. try the false path
pub(crate) struct DumbBacktrackStrategy {}

impl BacktrackStrategy for DumbBacktrackStrategy {
    fn find_backtrack_point(&self, path: &Vec<TrailEntry>, _conflict: &Conflict) -> Option<usize> {
        for (ix, entry) in path.iter().enumerate().rev() {
            match entry.decision.map(|c| c.polarity()) {
                // If there was no decision at this decision level, we are at the root - abort
                None => return None,
                // If this was a left hand path (X=true), go down the right hand path this time.
                Some(true) => return Some(ix),
                // Else, continue looking for a decision to revert
                Some(false) => {}
            }
        }
        None
    }

    // Go down the other path
    fn next_decision(
        &self,
        _path: &Vec<TrailEntry>,
        _conflict: &Conflict,
        result: &BacktrackResult,
    ) -> Option<Literal> {
        result.last_decision.map(|decision| decision.invert())
    }
}
