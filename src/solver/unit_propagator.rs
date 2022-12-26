use std::collections::VecDeque;

use log::trace;

use crate::instance::*;

use super::assignment_set::EvaluationResult;
use super::backtrack::Conflict;
use super::clause_store::{ClauseRef, ClauseStore};
use super::knowledge_graph::KnowledgeGraph;
use super::trail::Trail;
use itertools::Itertools;

pub(crate) struct UnitPropagator<'a> {
    clause_store: &'a mut ClauseStore,
    trail: &'a mut Trail,
    knowledge_graph: &'a mut KnowledgeGraph,
}

impl<'a> UnitPropagator<'a> {
    pub(crate) fn new(
        clause_store: &'a mut ClauseStore,
        trail: &'a mut Trail,
        knowledge_graph: &'a mut KnowledgeGraph,
    ) -> UnitPropagator<'a> {
        UnitPropagator {
            clause_store,
            trail,
            knowledge_graph,
        }
    }

    pub(crate) fn evaluate(&'a self) -> Option<Conflict> {
        // So we run through the untested literals, and check all of the relevant candidate clauses.
        // If any of them are invalid, return the conflict
        let untested_literals = self
            .trail
            .assignments_since_last_decision()
            .as_assignment_vec();
        for &literal in untested_literals.iter() {
            for clause in self.clause_store.idx().find_evaluatable_candidates(literal) {
                match self
                    .trail
                    .assignment()
                    .evaluate(clause.literals(&self.clause_store))
                {
                    EvaluationResult::False => {
                        return Some(Conflict {
                            conflicting_decision: self.trail.last_decision(),
                            conflicting_literal: literal,
                            conflicting_clause: clause,
                        });
                    }
                    _ => {}
                }
            }
        }

        None
    }

    pub(crate) fn propagate_units(&mut self) -> Option<Conflict> {
        let mut queue = VecDeque::new();
        queue.extend(
            self.trail
                .assignments_since_last_decision()
                .as_assignment_vec(),
        );
        trace!("q: {:?}", queue);

        while !queue.is_empty() {
            trace!("assignment: {:?}", self.trail.assignment());

            let literal = queue.pop_back().unwrap();
            trace!("lit: {:?}", literal);
            // Build this list to avoid writing to the clause_index during the loop over borrowed clauses
            let mut inferred_literals = vec![];
            for clause in self.clause_store.idx().find_unit_prop_candidates(literal) {
                trace!("clause: {:?}", clause);
                match self.propagate_unit(literal, clause) {
                    PropagationResult::Conflicted(conflict) => return Some(conflict),
                    PropagationResult::Inferred(inferred) => {
                        // Important: propagate_unit takes its assignment from trail. Deferring
                        // adding to the dfs path causes issues
                        self.trail.add_inferred(inferred);
                        self.knowledge_graph.add_inferred(
                            inferred,
                            literal,
                            self.trail.last_decision(),
                            clause,
                        );
                        self.clause_store.mark_resolved(inferred.var());
                        inferred_literals.push(inferred);
                    }
                    PropagationResult::Failed => (),
                }
            }
            queue.extend(inferred_literals);
        }
        None
    }

    fn propagate_unit(&self, literal: Literal, clause: ClauseRef) -> PropagationResult {
        let assignment = self.trail.assignment();

        let mut last_free = None;
        for literal in clause.literals(&self.clause_store) {
            if let Some(ass) = assignment.get(literal.var()) {
                if ass == literal {
                    // If there is a matching literal, we can't say anything about the free variable
                    return PropagationResult::Failed;
                }
            } else {
                if last_free != None {
                    // Implies we have multiple unresolved variables, short circuit
                    return PropagationResult::Failed;
                }
                last_free = Some(literal);
            }
        }
        // Having no free variables, but being unable to propagate implies a conflict
        match last_free {
            Some(lit) => PropagationResult::Inferred(lit),
            None => PropagationResult::Conflicted(Conflict {
                conflicting_decision: self.trail.last_decision(),
                conflicting_literal: literal,
                conflicting_clause: clause,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PropagationResult {
    Conflicted(Conflict),
    Inferred(Literal),
    Failed,
}

pub(crate) fn find_inital_assignment(clause_store: &ClauseStore) -> InitialAssignmentResult {
    let mut unit_clauses: Vec<ClauseRef> = clause_store.iter().filter(|cl| cl.is_unit()).collect();

    // Check if we have any duplicates
    unit_clauses.sort_by_key(|cl| cl.unit());
    unit_clauses.dedup();
    for (&cl_a, &cl_b) in unit_clauses.iter().tuple_windows() {
        if cl_a.unit().var() == cl_b.unit().var() {
            return InitialAssignmentResult::Conflict(Conflict {
                conflicting_decision: Some(cl_a.unit()),
                conflicting_literal: cl_a.unit(),
                conflicting_clause: cl_b,
            });
        }
    }

    let literals = unit_clauses.iter().map(|cl| cl.unit()).collect();
    InitialAssignmentResult::Assignment(literals)
}

pub(crate) fn record_initial_assignment(
    clause_store: &mut ClauseStore,
    knowledge_graph: &mut KnowledgeGraph,
    assignment: &Vec<Literal>,
) {
    for &literal in assignment {
        clause_store.mark_resolved(literal.var());
        knowledge_graph.add_initial(literal);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InitialAssignmentResult {
    Conflict(Conflict),
    Assignment(Vec<Literal>),
}

#[cfg(test)]
mod test {
    use log::trace;

    use crate::{
        instance::*,
        solver::{
            assignment_set::LiteralSet, clause_store::ClauseStore, knowledge_graph::KnowledgeGraph,
            trail::Trail, unit_propagator::UnitPropagator,
        },
    };

    /// Set up a instance of `A && !B`, and an assignment of !A.
    /// This should cause us to infer !B through unit prop.
    #[test]
    fn test_unit_prop_single_unit_simple() {
        let va = Variable(0);
        let vb = Variable(1);
        let a = Literal::new(va, true);
        let b = Literal::new(vb, true);

        // a & !b
        let clause = Clause::new(&vec![a, b.invert()]);

        let mut clause_store = ClauseStore::new(vec![clause]);
        trace!("store: {:?}", clause_store);
        let mut trail = Trail::new();
        let mut knowledge_graph = KnowledgeGraph::new(2);

        let decision = a.invert();
        trail.add_decision(decision);
        clause_store.mark_resolved(decision.var());
        knowledge_graph.add_decision(decision);

        let mut unit_propagator =
            UnitPropagator::new(&mut clause_store, &mut trail, &mut knowledge_graph);

        let result = unit_propagator.propagate_units();

        assert_eq!(result, None);
        assert_eq!(
            trail.assignment(),
            &LiteralSet::from_assignment_vec(&vec![a.invert(), b.invert()])
        );
    }

    #[test]
    fn test_unit_prop_single_unit_conflict() {
        let a = Variable(0);
        let b = Variable(1);

        // These two clauses will conflict when we try to propogate a=false
        // a | !b
        let clause_one = Clause::new(&vec![Literal::new(a, true), Literal::new(b, false)]);
        // a | b
        let clause_two = Clause::new(&vec![Literal::new(a, true), Literal::new(b, false)]);
        let clauses = vec![clause_one, clause_two];

        let mut clause_store = ClauseStore::new(clauses);
        let mut trail = Trail::new();
        let mut knowledge_graph = KnowledgeGraph::new(2);

        let decision = Literal::new(a, false);
        trail.add_decision(decision);
        clause_store.mark_resolved(a);
        knowledge_graph.add_decision(decision);

        let mut unit_propagator =
            UnitPropagator::new(&mut clause_store, &mut trail, &mut knowledge_graph);

        let result = unit_propagator.propagate_units();

        assert_eq!(result, None);
    }
}
