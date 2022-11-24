use std::collections::VecDeque;

use crate::instance::*;

use super::assignment_set::EvaluationResult;
use super::clause_index::ClauseIndex;
use super::dfs_path::DFSPath;
use super::knowledge_graph::KnowledgeGraph;

// c -> clauses
// a -> other junk
pub(crate) struct UnitPropagator<'a, 'c: 'a> {
    clause_index: &'a mut ClauseIndex<'c>,
    dfs_path: &'a mut DFSPath,
    knowledge_graph: &'a mut KnowledgeGraph,
}

impl<'a, 'c> UnitPropagator<'a, 'c> {
    pub(crate) fn new(
        clause_index: &'a mut ClauseIndex<'c>,
        dfs_path: &'a mut DFSPath,
        knowledge_graph: &'a mut KnowledgeGraph,
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
                            literal,
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
        eprintln!("q: {:?}", queue);

        while !queue.is_empty() {
            println!("assignment: {:?}", self.dfs_path.assignment());

            let literal = queue.pop_back().unwrap();
            println!("lit: {:?}", literal);
            println!("ci: {:?}", self.clause_index);
            // Build this list to avoid writing to the clause_index during the loop over borrowed clauses
            let mut inferred_literals = vec![];
            for clause in self.clause_index.find_unit_prop_candidates(literal) {
                println!("clause: {:?}", clause);
                match self.propagate_unit(literal, &clause) {
                    PropagationResult::Conflicted(conflict) => return Some(conflict),
                    PropagationResult::Inferred(inferred) => {
                        self.knowledge_graph.add_inferred(inferred, clause);
                        inferred_literals.push(inferred);
                    }
                    _ => panic!("Failed unit prop??"),
                }
            }
            inferred_literals.sort();
            inferred_literals.dedup();
            for inferred in &inferred_literals {
                self.dfs_path.add_inferred(*inferred);
                self.clause_index.mark_resolved(inferred.var());
            }
            queue.extend(inferred_literals);
        }
        None
    }

    // fn propagate_unit(&self, literal: Literal, clause: &'c Clause) -> PropagationResult<'c> {
    //     let assignment = self.search_path.assignment();
    //     let inferred = match assignment.unit_prop(clause) {
    //         // This should really not happen - we filter before propagation to ensure we have a 100% success rate
    //         None => return PropagationResult::Failed,
    //         Some(lit) => lit,
    //     };

    //     // Check if we have logical conflicts
    //     if assignment.get(inferred.var()) == Some(!inferred.polarity()) {
    //         return PropagationResult::Conflicted(Conflict {
    //             literal: inferred,
    //             conflicting_clause: clause,
    //         });
    //     }

    //     PropagationResult::Inferred(inferred)
    // }

    fn propagate_unit(&self, literal: Literal, clause: &'c Clause) -> PropagationResult<'c> {
        let assignment = self.dfs_path.assignment();

        let mut last_unknown = None;
        for literal in clause.literals() {
            if let Some(ass_sign) = assignment.get(literal.var()) {
                if ass_sign == literal.polarity() {
                    return PropagationResult::Conflicted(Conflict {
                        literal: *literal,
                        conflicting_clause: clause,
                    });
                }
            } else {
                if last_unknown != None {
                    // Implies we have multiple unresolved variables, short circuit
                    return PropagationResult::Failed;
                }
                last_unknown = Some(*literal);
            }
        }
        PropagationResult::Inferred(last_unknown.unwrap())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PropagationResult<'a> {
    Conflicted(Conflict<'a>),
    Inferred(Literal),
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict<'a> {
    pub(crate) literal: Literal,
    pub(crate) conflicting_clause: &'a Clause,
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
                literal: lit_a,
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
        problem_builder::ProblemBuilder,
        solver::{
            assignment_set::{EvaluationResult, LiteralSet},
            clause_index::{self, ClauseIndex},
            dfs_path::DFSPath,
            knowledge_graph::{self, KnowledgeGraph},
            unit_propagator::{self, UnitPropagator},
        },
        *,
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
        let mut knowledge_graph = KnowledgeGraph::new();

        let decision = Literal::new(a, false);
        dfs_path.add_decision(decision);
        clause_index.mark_resolved(a);
        knowledge_graph.add_decision(decision);

        let mut unit_propagator =
            UnitPropagator::new(&mut clause_index, &mut dfs_path, &mut knowledge_graph);

        let result = unit_propagator.propagate_units();

        assert_eq!(result, None);
        assert!(false);
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
        let mut knowledge_graph = KnowledgeGraph::new();

        let decision = Literal::new(a, false);
        dfs_path.add_decision(decision);
        clause_index.mark_resolved(a);
        knowledge_graph.add_decision(decision);

        let mut unit_propagator =
            UnitPropagator::new(&mut clause_index, &mut dfs_path, &mut knowledge_graph);

        let result = unit_propagator.propagate_units();

        assert_eq!(result, None);
        assert!(false);
    }

    // #[test]
    // fn test_unit_prop_single_free() {
    //     let x = Literal::new(Variable(0), true);
    //     let y = Literal::new(Variable(1), true);
    //     let clause = Clause::new(&vec![x.invert(), y]);
    //     let mut ass = LiteralSet::new();
    //     ass.add(x);
    //     assert_eq!(ass.unit_prop(&clause), Some(y));
    // }

    // #[test]
    // fn test_unit_prop_multiple_free() {
    //     let x = Literal::new(Variable(0), true);
    //     let y = Literal::new(Variable(1), true);
    //     let clause = Clause::new(&vec![x.invert(), y]);
    //     let ass = LiteralSet::new();
    //     assert_eq!(ass.unit_prop(&clause), None);
    // }

    // #[test]
    // fn test_unit_prop_true() {
    //     let x = Literal::new(Variable(0), true);
    //     let y = Literal::new(Variable(1), true);
    //     let clause = Clause::new(&vec![x, y]);
    //     let mut ass = LiteralSet::new();
    //     ass.add(x);
    //     assert_eq!(ass.unit_prop(&clause), None);
    // }

    // #[test]
    // fn test_unit_prop_scenario_two_unknown() {
    //     let mut pb = ProblemBuilder::new();
    //     let x = pb.var("x");
    //     let y = pb.var("y");

    //     // With x set, unit propagation should result in y being inferred to be true
    //     pb.require(pb.and(x, y));

    //     let instance = pb.build();
    //     // Build an assignment with x = true
    //     let x_lit = instance.variables.get_by_name("x").unwrap();
    //     let mut ass = LiteralSet::from_assignment_vec(&vec![Literal::new(x_lit, true)]);

    //     // We should now get at least one instance of unit propagation across all the clauses
    //     let mut propagation_count = 0;
    //     for clause in &instance.clauses {
    //         let result = ass.unit_prop(clause);
    //         println!("prop result: {:?}", result);
    //         if result != None {
    //             propagation_count += 1;
    //         }
    //     }
    //     assert_ne!(propagation_count, 0);

    //     // Furthermore, we should be able to solve the whole problem using unit prop only
    //     for _i in 0..100 {
    //         let mut all_pass = true;
    //         for clause in &instance.clauses {
    //             match ass.evaluate(clause) {
    //                 EvaluationResult::True => (),
    //                 EvaluationResult::False => assert!(false, "conflict during solve??"),
    //                 EvaluationResult::Unknown => {
    //                     all_pass = false;
    //                     if let Some(result) = ass.unit_prop(clause) {
    //                         println!("adding {:?} to result", result);
    //                         ass.add(result)
    //                     }
    //                 }
    //             }
    //         }
    //         if all_pass {
    //             // We solved the problem through unit propagation alone!
    //             return;
    //         }
    //     }
    //     assert!(false, "did not solve problem through unit propagation")
    // }
}
