use fnv::FnvHashSet;
use itertools::Itertools;
use log::trace;

use crate::{
    instance::{Literal, Variable},
    solver::sorted_vec::sort_and_dedupe,
};

use super::{
    clause_store::{ClauseRef, ClauseRefResolver, ClauseStore},
    knowledge_graph::KnowledgeGraph,
    trail::{Trail, TrailEntry},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub(crate) conflicting_decision: Option<Literal>,
    pub(crate) conflicting_literal: Literal,
    pub(crate) conflicting_clause: ClauseRef,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct AnalyzedConflict {
    pub(crate) unique_implication_point: Variable,
    pub(crate) learnt_clause: Vec<Literal>,
    pub(crate) second_highest_decision_level: usize,
}

#[derive(Default)]
pub(crate) struct ConflictAnalyzer {}

impl ConflictAnalyzer {
    pub(crate) fn analyse_conflict(
        &self,
        clause_store: &ClauseStore,
        trail: &Trail,
        knowledge_graph: &KnowledgeGraph,
        conflict: &Conflict,
    ) -> Option<AnalyzedConflict> {
        // The first UIP between the conflict and the current decision
        let (uip, edge) =
            self.find_unique_implication_point(clause_store, trail, knowledge_graph, conflict);
        trace!("uip: {:?}", uip);
        trace!("edge: {:?}", edge);

        // Find the 2nd last decision level (where we will backtrack to)
        let current_decision_level = trail.current_decision_level();
        trace!("current level: {:?}", current_decision_level);
        let decision_levels = edge
            .iter()
            .map(|l| {
                let decision = knowledge_graph.vertex(l.var()).decision.unwrap_or(l.var());
                trace!("decision: {decision:?}");
                // If we cannot find an explicit decision level for this, it was set in initial
                // prop (i.e. level 0)
                trail.find_decision_level(decision).unwrap_or(0)
            })
            .collect_vec();
        trace!("levels: {:?}", decision_levels);
        let second_highest_decision_level = edge
            .iter()
            .map(|l| {
                let decision = knowledge_graph.vertex(l.var()).decision.unwrap_or(l.var());
                // If we cannot find an explicit decision level for this, it was set in initial
                // prop (i.e. level 0)
                trail.find_decision_level(decision).unwrap_or(0)
            })
            .filter(|&l| l != current_decision_level)
            .min()
            // If we had no other levels, we need to backtrack to root
            .unwrap_or(0);
        trace!(
            "second highest decision level: {:?}",
            second_highest_decision_level
        );

        // The clause is the inversion of the edge
        let clause = edge.iter().map(|l| l.invert()).collect_vec();

        Some(AnalyzedConflict {
            unique_implication_point: uip,
            learnt_clause: clause,
            second_highest_decision_level,
        })
    }

    /// Returns both the unique implication point AND the cut edge
    fn find_unique_implication_point(
        &self,
        clause_store: &ClauseStore,
        trail: &Trail,
        knowledge_graph: &KnowledgeGraph,
        conflict: &Conflict,
    ) -> (Variable, Vec<Literal>) {
        let assignment = trail.assignment();
        let current_level_assignments = trail.assignments_since_last_decision();
        // TODO(lcm): consider if this is the right datastructure
        let mut cut_edge = FnvHashSet::default();
        // We start by considering the steps that led us to the conflict. These are the literals
        // in the conflict clause
        cut_edge.extend(
            clause_store
                .clause_literals(conflict.conflicting_clause)
                .map(|l| l.var()),
        );
        trace!("initial cut: {:?}", cut_edge.iter().collect_vec());

        let current_decision_level_trail = trail.iter().last().unwrap();
        for current_literal in current_decision_level_trail.iter_literals().rev() {
            let current_vertex = knowledge_graph.vertex(current_literal.var());
            // Remove ourselves from the cut edge, and add the vertices that got us here to
            // the cut edge.
            if let Some(clause_ref) = current_vertex.clause {
                cut_edge.extend(clause_store.clause_literals(clause_ref).map(|l| l.var()));
                cut_edge.remove(&current_literal.var());
            } else {
                // If there's no clause, then we arrived at a decision, and should not replace
                // ourselves - we are at the UIP
            }

            let mut current_level_iter = cut_edge
                .iter()
                .filter(|&&v| current_level_assignments.contains_var(v));
            // There should always be at least one entry in the edge for the current decision level
            let uip = current_level_iter.next().unwrap();
            if current_level_iter.next().is_none() {
                let mut edge = cut_edge
                    .iter()
                    .map(|&v| assignment.get(v).unwrap())
                    .collect_vec();
                sort_and_dedupe(&mut edge);
                return (*uip, edge);
            }
        }
        panic!("could not find UIP: {:?}", cut_edge);
    }
}

pub(crate) trait BacktrackStrategy {
    /// Calculates how far we should roll back the search tree
    fn find_backtrack_point(
        &self,
        path: &[TrailEntry],
        conflict: &Conflict,
        analyzed_conflict: &AnalyzedConflict,
    ) -> Option<usize>;
}

/// A naive dfs backtrack strategy - look for the last path where we didn't go "left" - i.e. try the false path
#[allow(dead_code)]
pub(crate) struct DumbBacktrackStrategy {}

impl BacktrackStrategy for DumbBacktrackStrategy {
    fn find_backtrack_point(
        &self,
        path: &[TrailEntry],
        _conflict: &Conflict,
        _analyzed_conflict: &AnalyzedConflict,
    ) -> Option<usize> {
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
}

pub(crate) struct BackjumpStrategy {}

impl BacktrackStrategy for BackjumpStrategy {
    fn find_backtrack_point(
        &self,
        _path: &[TrailEntry],
        _conflict: &Conflict,
        analyzed_conflict: &AnalyzedConflict,
    ) -> Option<usize> {
        Some(analyzed_conflict.second_highest_decision_level)
    }
}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use crate::{
        solver::{clause_store, knowledge_graph, trail},
        *,
    };

    use super::{AnalyzedConflict, Conflict, ConflictAnalyzer};

    // Run through the example found here: https://users.aalto.fi/~tjunttil/2020-DP-AUT/notes-sat/cdcl.html#implication-graphs-learned-clauses-and-backjumping
    #[test]
    fn test_2020_dp_aut_uip() {
        env_logger::init();
        let xs = (0..13)
            .map(|i| Literal::new(Variable(i), true))
            .collect_vec();

        let clauses = vec![
            vec![xs[1].invert(), xs[2].invert()],
            vec![xs[1].invert(), xs[3]],
            vec![xs[3].invert(), xs[4].invert()],
            vec![xs[2], xs[4], xs[5]],
            vec![xs[5].invert(), xs[6], xs[7].invert()],
            vec![xs[2], xs[7], xs[8]],
            vec![xs[8].invert(), xs[9].invert()],
            vec![xs[8].invert(), xs[10]],
            vec![xs[9], xs[10].invert(), xs[11]],
            vec![xs[10].invert(), xs[12].invert()],
            vec![xs[11].invert(), xs[12]],
        ]
        .iter()
        .enumerate()
        .map(|(id, literals)| Clause::new_with_id(id, literals))
        .collect_vec();

        let store = clause_store::ClauseStore::new(clauses);
        let mut trail = trail::Trail::new();
        let mut kg = knowledge_graph::KnowledgeGraph::new(13);

        // Set up the first decision level
        trail.add_decision(xs[1]);
        kg.add_decision(xs[1]);
        // unit prop with clause 0
        trail.add_inferred(xs[2].invert());
        kg.add_inferred(xs[2].invert(), xs[1], Some(xs[1]), store.get(0).unwrap());
        // unit prop with clause 1
        trail.add_inferred(xs[3]);
        kg.add_inferred(xs[3], xs[1], Some(xs[1]), store.get(1).unwrap());
        // unit prop with clause 2
        trail.add_inferred(xs[4].invert());
        kg.add_inferred(xs[4].invert(), xs[3], Some(xs[1]), store.get(2).unwrap());
        // unit prop with clause 3
        trail.add_inferred(xs[5]);
        kg.add_inferred(xs[5], xs[4].invert(), Some(xs[1]), store.get(3).unwrap());

        // Now the second decision level (boy this is wordy...)
        trail.add_decision(xs[6].invert());
        kg.add_decision(xs[6].invert());
        // Unit prop with clause 4
        trail.add_inferred(xs[7].invert());
        kg.add_inferred(
            xs[7].invert(),
            xs[6].invert(),
            Some(xs[6].invert()),
            store.get(4).unwrap(),
        );
        // Unit prop with clause 5
        trail.add_inferred(xs[8]);
        kg.add_inferred(
            xs[8],
            xs[7].invert(),
            Some(xs[6].invert()),
            store.get(5).unwrap(),
        );
        // Unit prop with clause 6
        trail.add_inferred(xs[9].invert());
        kg.add_inferred(
            xs[9].invert(),
            xs[8],
            Some(xs[6].invert()),
            store.get(6).unwrap(),
        );
        // Unit prop with clause 7
        trail.add_inferred(xs[10]);
        kg.add_inferred(xs[10], xs[8], Some(xs[6].invert()), store.get(7).unwrap());
        // Unit prop with clause 8
        trail.add_inferred(xs[11]);
        kg.add_inferred(xs[11], xs[10], Some(xs[6].invert()), store.get(8).unwrap());
        // Unit prop with clause 9
        trail.add_inferred(xs[12].invert());
        kg.add_inferred(
            xs[12].invert(),
            xs[11],
            Some(xs[6].invert()),
            store.get(9).unwrap(),
        );

        // Now we have a conflict on clause 10
        let conflict = Conflict {
            conflicting_clause: store.get(10).unwrap(),
            conflicting_decision: Some(xs[6].invert()),
            conflicting_literal: xs[12],
        };

        let result = ConflictAnalyzer::default().analyse_conflict(&store, &trail, &kg, &conflict);
        assert_eq!(
            result,
            Some(AnalyzedConflict {
                learnt_clause: vec![xs[8].invert()],
                unique_implication_point: xs[8].var(),
                second_highest_decision_level: 0,
            })
        );
    }
}
