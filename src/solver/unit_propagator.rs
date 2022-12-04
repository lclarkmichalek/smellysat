use std::collections::VecDeque;

use log::trace;

use crate::instance::*;

use super::assignment_set::EvaluationResult;
use super::backtrack::Conflict;
use super::clause_index::ClauseIndex;
use super::dfs_path::DFSPath;
use super::knowledge_graph::KnowledgeGraph;

// c -> clauses
// a -> other junk
pub(crate) struct UnitPropagator<'a, 'c: 'a> {
    clause_index: &'a mut ClauseIndex<'c>,
    dfs_path: &'a mut DFSPath,
    knowledge_graph: &'a mut KnowledgeGraph<'c>,
}

impl<'a, 'c> UnitPropagator<'a, 'c> {
    pub(crate) fn new(
        clause_index: &'a mut ClauseIndex<'c>,
        dfs_path: &'a mut DFSPath,
        knowledge_graph: &'a mut KnowledgeGraph<'c>,
    ) -> UnitPropagator<'a, 'c> {
        UnitPropagator {
            clause_index,
            dfs_path,
            knowledge_graph,
        }
    }

    pub(crate) fn evaluate(&'a self) -> Option<Conflict<'c>> {
        // So we run through the untested literals, and check all of the relevant candidate clauses.
        // If any of them are invalid, return the conflict
        let untested_literals = self
            .dfs_path
            .assignments_since_last_decision()
            .as_assignment_vec();
        for &literal in untested_literals.iter() {
            for clause in self.clause_index.find_evaluatable_candidates(literal) {
                match self.dfs_path.assignment().evaluate(clause) {
                    EvaluationResult::False => {
                        return Some(Conflict {
                            conflicting_decision: self.dfs_path.last_decision(),
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

    pub(crate) fn propagate_units(&mut self) -> Option<Conflict<'c>> {
        let mut queue = VecDeque::new();
        queue.extend(
            self.dfs_path
                .assignments_since_last_decision()
                .as_assignment_vec(),
        );
        trace!("q: {:?}", queue);

        while !queue.is_empty() {
            trace!("assignment: {:?}", self.dfs_path.assignment());

            let literal = queue.pop_back().unwrap();
            trace!("lit: {:?}", literal);
            trace!("ci: {:?}", self.clause_index);
            // Build this list to avoid writing to the clause_index during the loop over borrowed clauses
            let mut inferred_literals = vec![];
            for clause in self.clause_index.find_unit_prop_candidates(literal) {
                trace!("clause: {:?}", clause);
                match self.propagate_unit(literal, &clause) {
                    PropagationResult::Conflicted(conflict) => return Some(conflict),
                    PropagationResult::Inferred(inferred) => {
                        // Important: propagate_unit takes its assignment from dfs_path. Deferring
                        // adding to the dfs path causes issues
                        self.dfs_path.add_inferred(inferred);
                        self.knowledge_graph.add_inferred(
                            inferred,
                            literal,
                            self.dfs_path.last_decision(),
                            clause,
                        );
                        self.clause_index.mark_resolved(inferred.var());
                        inferred_literals.push(inferred);
                    }
                    PropagationResult::Failed => (),
                }
            }
            queue.extend(inferred_literals);
        }
        None
    }

    fn propagate_unit(&self, literal: Literal, clause: &'c Clause) -> PropagationResult<'c> {
        let assignment = self.dfs_path.assignment();

        let mut last_free = None;
        for &literal in clause.literals() {
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
                conflicting_decision: self.dfs_path.last_decision(),
                conflicting_literal: literal,
                conflicting_clause: clause,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PropagationResult<'a> {
    Conflicted(Conflict<'a>),
    Inferred(Literal),
    Failed,
}

pub(crate) fn find_inital_assignment<'a, 'c>(
    clause_index: &'a mut ClauseIndex<'c>,
    knowledge_graph: &'a mut KnowledgeGraph,
) -> InitialAssignmentResult<'c> {
    let mut literals: Vec<Literal> = clause_index
        .find_unit_clauses()
        .iter()
        .map(|cl| cl.literals()[0])
        .collect();

    // Check if we have any duplicates
    literals.sort();
    literals.dedup();
    for window in literals.windows(2) {
        let lit_a = window[0];
        let lit_b = window[1];
        if lit_a.var() == lit_b.var() {
            let relevant_clauses = clause_index.find_unit_clauses_containing_var(lit_a.var());
            return InitialAssignmentResult::Conflict(Conflict {
                conflicting_decision: Some(lit_a),
                conflicting_literal: lit_a,
                conflicting_clause: relevant_clauses[0],
            });
        }
    }

    for &literal in literals.iter() {
        clause_index.mark_resolved(literal.var());
        // Questionable semantics, but whatever.
        knowledge_graph.add_decision(literal);
    }
    InitialAssignmentResult::Assignment(literals)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InitialAssignmentResult<'c> {
    Conflict(Conflict<'c>),
    Assignment(Vec<Literal>),
}

#[cfg(test)]
mod test {
    use crate::{
        instance::*,
        solver::{
            assignment_set::{EvaluationResult, LiteralSet},
            clause_index::ClauseIndex,
            dfs_path::DFSPath,
            knowledge_graph::KnowledgeGraph,
            unit_propagator::UnitPropagator,
        },
    };

    #[test]
    fn test_evaluate_clause_true() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);
        // A OR !C
        let clause = Clause::new(&vec![Literal::new(a, true), Literal::new(c, false)]);
        // a = true, b = true, c = false
        assert_eq!(
            LiteralSet::from_assignment_vec(&vec![
                Literal::new(a, true),
                Literal::new(b, true),
                Literal::new(c, false),
            ])
            .evaluate(&clause),
            EvaluationResult::True
        );
        // a = false, b = false, c = false
        assert_eq!(
            LiteralSet::from_assignment_vec(&vec![
                Literal::new(a, false),
                Literal::new(b, false),
                Literal::new(c, false),
            ])
            .evaluate(&clause),
            EvaluationResult::True
        );
        // a = false, b = false, c = true
        assert_eq!(
            LiteralSet::from_assignment_vec(&vec![
                Literal::new(a, false),
                Literal::new(b, false),
                Literal::new(c, true),
            ])
            .evaluate(&clause),
            EvaluationResult::False
        )
    }

    #[test]
    fn test_evaluate_clause_missing() {
        let c = Clause::new(&vec![Literal::new(Variable(0), true)]);
        assert_eq!(LiteralSet::new().evaluate(&c), EvaluationResult::Unknown)
    }

    #[test]
    fn test_unit_prop_single_unit_simple() {
        let a = Variable(0);
        let b = Variable(1);

        // a & !b
        let clause = Clause::new(&vec![Literal::new(a, true), Literal::new(b, false)]);
        let clauses = vec![clause];

        let mut clause_index = ClauseIndex::new(&clauses);
        let mut dfs_path = DFSPath::new(LiteralSet::new());
        let mut knowledge_graph = KnowledgeGraph::new(2);

        let decision = Literal::new(a, false);
        dfs_path.add_decision(decision);
        clause_index.mark_resolved(a);
        knowledge_graph.add_decision(decision);

        let mut unit_propagator =
            UnitPropagator::new(&mut clause_index, &mut dfs_path, &mut knowledge_graph);

        let result = unit_propagator.propagate_units();

        assert_eq!(result, None);
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

        let mut clause_index = ClauseIndex::new(&clauses);
        let mut dfs_path = DFSPath::new(LiteralSet::new());
        let mut knowledge_graph = KnowledgeGraph::new(2);

        let decision = Literal::new(a, false);
        dfs_path.add_decision(decision);
        clause_index.mark_resolved(a);
        knowledge_graph.add_decision(decision);

        let mut unit_propagator =
            UnitPropagator::new(&mut clause_index, &mut dfs_path, &mut knowledge_graph);

        let result = unit_propagator.propagate_units();

        assert_eq!(result, None);
    }
}
