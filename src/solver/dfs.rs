use std::fmt;
use std::rc::Rc;

use log::{info, trace};

use crate::instance::*;
use crate::solver::backtrack::{BacktrackStrategy, ConflictAnalyzer, DumbBacktrackStrategy};
use crate::solver::knowledge_graph::KnowledgeGraph;
use crate::solver::sorted_vec::sort_and_dedupe;
use crate::solver::trail::Trail;
use crate::solver::unit_propagator::{find_inital_assignment, InitialAssignmentResult};
use crate::variable_registry::VariableRegister;

use super::assignment_set::LiteralSet;
use super::backtrack::{AnalyzedConflict, BackjumpStrategy, Conflict};
use super::clause_store::ClauseStore;
use super::unit_propagator::{record_initial_assignment, UnitPropagator};

#[derive(Debug, Clone)]
struct TraversalPath {
    variables: Rc<VariableRegister>,
}

impl TraversalPath {
    fn next(&self, path: &Trail) -> Option<&Variable> {
        self.variables
            .iter()
            .filter(|&&l| path.assignment().get(l).is_none())
            .next()
    }
}

#[derive(Clone)]
pub struct Instance {
    pub(crate) variables: Rc<VariableRegister>,
    pub(crate) clauses: Vec<Clause>,
    backtrack_strategy: Rc<dyn BacktrackStrategy>,
}

impl Instance {
    pub(crate) fn new(cnf: Vec<Vec<Literal>>, literals: VariableRegister) -> Instance {
        let clauses = cnf
            .iter()
            .enumerate()
            .map(|(ix, cl)| Clause::new_with_id(ix, cl))
            .collect();
        Self::new_from_clauses(clauses, literals)
    }

    pub(crate) fn new_from_clauses(clauses: Vec<Clause>, literals: VariableRegister) -> Instance {
        Instance {
            variables: Rc::new(literals),
            clauses,
            backtrack_strategy: Self::backtrack_strategy(),
        }
    }

    fn backtrack_strategy() -> Rc<dyn BacktrackStrategy> {
        Rc::new(BackjumpStrategy {})
    }

    pub fn solve(&mut self) -> Solution {
        let mut stats = EvaluationStats {
            step_count: 0,
            initial_unit_count: 0,
            unit_prop_count: 0,
            backtrack_count: 0,
            learnt_clause_count: 0,
        };
        let traversal_plan = TraversalPath {
            variables: self.variables.clone(),
        };

        let mut clause_store = ClauseStore::new(self.clauses.clone());
        let mut knowledge_graph = KnowledgeGraph::new(self.variables.count());

        let initial_assignment = match find_inital_assignment(&mut clause_store) {
            InitialAssignmentResult::Conflict(_conflict) => {
                return Solution {
                    literals: self.variables.clone(),
                    solution: None,
                    stats,
                }
            }
            InitialAssignmentResult::Assignment(vars) => vars,
        };

        record_initial_assignment(&mut clause_store, &mut knowledge_graph, &initial_assignment);
        let mut trail = Trail::new();
        for lit in initial_assignment {
            trail.add_inferred(lit)
        }

        stats.initial_unit_count = trail.assignment().size();

        if clause_store.idx().all_clauses_resolved() {
            info!("solved through initial unit assignment");
            return Solution {
                literals: self.variables.clone(),
                solution: Some(trail.assignment().clone()),
                stats,
            };
        }

        loop {
            trace!("========");
            trace!(
                "iteration starting. level: {}",
                trail.current_decision_level()
            );
            let mut ass = trail.assignment().as_assignment_vec();
            sort_and_dedupe(&mut ass);
            trace!("assignment: {:?}", ass,);
            trace!("========");

            let deduced = trail.assignments_since_last_decision().size();
            let mut unit_prop =
                UnitPropagator::new(&mut clause_store, &mut trail, &mut knowledge_graph);
            let prop_eval_result = unit_prop.propagate_units().or_else(|| unit_prop.evaluate());
            stats.unit_prop_count += trail.assignments_since_last_decision().size() - deduced;

            if let Some(conflict) = prop_eval_result {
                if trail.current_decision_level() == 0 {
                    info!("conflict in decision level 0: {:?}", conflict);
                    return self.infeasible(stats);
                }

                trace!("conflict: {:?}", conflict);
                let analyzer = ConflictAnalyzer::default();
                let analyzed_conflict = analyzer
                    .analyse_conflict(&clause_store, &trail, &knowledge_graph, &conflict)
                    .unwrap();
                trace!("analyzed_conflict: {:?}", analyzed_conflict);

                self.backtrack(
                    &conflict,
                    &analyzed_conflict,
                    &mut trail,
                    &mut clause_store,
                    &mut knowledge_graph,
                )
                .unwrap();
                stats.backtrack_count += 1;

                if let Some(clause) =
                    clause_store.add_clause(analyzed_conflict.learnt_clause.clone())
                {
                    stats.learnt_clause_count += 1;
                    if clause.is_unit() {
                        let lit = clause.unit();
                        if trail.assignment().get(lit.var()) == Some(lit.invert()) {
                            info!("infeasible due to conflicting learnt unit clause");
                            return self.infeasible(stats);
                        }
                        trail.add_inferred(lit);
                        knowledge_graph.add_initial(lit);
                        clause_store.mark_resolved(lit.var());
                    }
                }
                continue;
            }

            if clause_store.idx().all_clauses_resolved() {
                return Solution {
                    literals: self.variables.clone(),
                    solution: Some(trail.assignment().clone()),
                    stats,
                };
            }

            // Now, keep stepping into the problem
            if let Some(&var) = traversal_plan.next(&trail) {
                let lit = Literal::new(var, true);
                stats.step_count += 1;
                trail.add_decision(lit);
                knowledge_graph.add_decision(lit);
                clause_store.mark_resolved(var)
            } else {
                // If we can't keep going, we're done, i guess. Iterate one more time
                continue;
            }
        }
    }

    fn backtrack(
        &self,
        conflict: &Conflict,
        analyzed_conflict: &AnalyzedConflict,
        path: &mut Trail,
        clause_store: &mut ClauseStore,
        knowledge_graph: &mut KnowledgeGraph,
    ) -> Option<()> {
        // Attempt to find the position that should be pivoted on. if we cannot find such a point, we have failed to backtrack
        let pivot = match self.backtrack_strategy.find_backtrack_point(
            path.search_path(),
            conflict,
            analyzed_conflict,
        ) {
            None => panic!("backtrack failed"),
            Some(pivot) => pivot,
        };
        let backtracked = path.backtrack(pivot);

        // Rollback the assignments
        for lit in backtracked.assignments.iter() {
            clause_store.mark_unresolved(lit.var());
        }
        knowledge_graph.remove(&backtracked.assignments);

        Some(())
    }

    fn infeasible(&self, stats: EvaluationStats) -> Solution {
        Solution {
            literals: self.variables.clone(),
            solution: None,
            stats,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EvaluationStats {
    step_count: usize,
    initial_unit_count: usize,
    unit_prop_count: usize,
    backtrack_count: usize,
    learnt_clause_count: usize,
}

#[derive(Clone)]
pub struct Solution {
    pub literals: Rc<VariableRegister>,
    pub(crate) solution: Option<LiteralSet>,
    pub stats: EvaluationStats,
}

impl Solution {
    pub fn assignments(&self) -> Option<Vec<Literal>> {
        self.solution.clone().map(|ls| ls.as_assignment_vec())
    }
}

impl fmt::Debug for Solution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(solution) = &self.solution {
            let mut first = true;
            for lit in self.literals.iter_original() {
                if !first {
                    write!(f, ", ")?;
                }
                first = false;

                let name = self.literals.get(lit);
                let formatted_val = match solution.get(lit) {
                    Some(val) => format!("{:?}", val),
                    None => "undef".to_string(),
                };
                write!(f, "{}={}", name, formatted_val)?;
            }
        } else {
            write!(f, "no solution found")?;
        }
        write!(f, "; stats={:?}", self.stats)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::{
        problem_builder::ProblemBuilder,
        solver::{assignment_set::LiteralSet, Instance},
        variable_registry::VariableRegister,
        *,
    };

    // This test starts with a satisfiable formula (A OR B), and then goes into an unsatisfiable formula.
    #[test]
    fn test_build_and_solve_infeasible() {
        // env_logger::init();
        let mut pb = ProblemBuilder::new();

        let a = pb.var("a");
        let b = pb.var("b");

        let x = pb.var("x");
        let y = pb.var("y");
        let z = pb.var("z");
        let p = pb.var("p");
        let q = pb.var("q");
        let r = pb.var("r");

        pb.require(pb.or(a, b));

        pb.require(pb.or(x, pb.and(y, z)));
        pb.require(pb.or(y, pb.and(z, p)));
        pb.require(pb.or(z, pb.and(p, q)));
        pb.require(pb.or(p, pb.and(q, r)));
        pb.require(pb.or(q, pb.and(r, x)));

        pb.require(pb.or(pb.not(x), pb.not(y)));
        pb.require(pb.or(pb.not(y), pb.not(z)));
        pb.require(pb.or(pb.not(z), pb.not(p)));
        pb.require(pb.or(pb.not(p), pb.not(q)));
        pb.require(pb.or(pb.not(q), pb.not(r)));

        let mut instance = pb.build();
        let solution = instance.solve();
        assert!(solution.solution.is_none());
        println!("{:?}", solution);
        // assert!(false);
    }

    #[test]
    fn test_build_and_solve_feasible() {
        let mut pb = ProblemBuilder::new();

        let a = pb.var("a");
        let b = pb.var("b");
        let c = pb.var("c");

        let x = pb.var("x");
        let y = pb.var("y");
        let z = pb.var("z");
        let p = pb.var("p");
        let q = pb.var("q");
        let r = pb.var("r");

        pb.require(pb.or(pb.not(a), pb.or(pb.not(b), pb.not(c))));

        pb.require(pb.or(x, pb.and(y, z)));
        pb.require(pb.or(z, pb.and(p, q)));
        pb.require(pb.or(q, pb.and(r, x)));

        pb.require(pb.or(pb.not(x), pb.not(y)));
        pb.require(pb.or(pb.not(y), pb.not(z)));
        pb.require(pb.or(pb.not(z), pb.not(p)));
        pb.require(pb.or(pb.not(p), pb.not(q)));
        pb.require(pb.or(pb.not(q), pb.not(r)));

        let mut instance = pb.build();
        let solution = instance.solve();
        assert!(solution.solution.is_some());
        println!("{:?}", solution);
    }

    #[test]
    fn test_build_and_solve_feasible_from_initial() {
        let mut vr = VariableRegister::new();
        let a = vr.create_original("a");
        let b = vr.create_original("b");
        let c = vr.create_original("c");
        let clauses = vec![
            Clause::new(&vec![Literal::new(a, true)]),
            Clause::new(&vec![Literal::new(a, false), Literal::new(b, true)]),
            Clause::new(&vec![Literal::new(b, false), Literal::new(c, true)]),
        ];

        let mut instance = Instance::new_from_clauses(clauses, vr);
        let solution = instance.solve();

        let mut expected = LiteralSet::new();
        expected.add(Literal::new(a, true));
        expected.add(Literal::new(b, true));
        expected.add(Literal::new(c, true));
        assert_eq!(solution.solution, Some(expected));
    }

    // This test requires the solver to step into a=true, and then use unit prop to resolve the other variables
    #[test]
    fn test_build_and_solve_feasible_one_step_and_prop() {
        let mut vr = VariableRegister::new();
        let a = vr.create_original("a");
        let b = vr.create_original("b");
        let c = vr.create_original("c");
        let clauses = vec![
            Clause::new(&vec![Literal::new(a, true), Literal::new(b, true)]),
            Clause::new(&vec![Literal::new(a, false), Literal::new(b, true)]),
            Clause::new(&vec![Literal::new(b, false), Literal::new(c, true)]),
        ];

        let mut instance = Instance::new_from_clauses(clauses, vr);
        let solution = instance.solve();

        let mut expected = LiteralSet::new();
        expected.add(Literal::new(a, true));
        expected.add(Literal::new(b, true));
        expected.add(Literal::new(c, true));
        assert_eq!(solution.solution, Some(expected));
    }

    // This test requires the solver to step into a=true, hit conflicts, backtrack, and then try a=false
    #[test]
    fn test_build_and_solve_feasible_backtrack() {
        // env_logger::init();

        let mut vr = VariableRegister::new();
        let va = vr.create_original("a");
        let vb = vr.create_original("b");
        let vc = vr.create_original("c");

        let a = Literal::new(va, true);
        let b = Literal::new(vb, true);
        let c = Literal::new(vc, true);
        let clauses = vec![
            Clause::new(&vec![a.invert(), b.invert()]),
            Clause::new(&vec![a.invert(), c.invert()]),
            Clause::new(&vec![b, c]),
        ];

        let mut instance = Instance::new_from_clauses(clauses, vr);
        let solution = instance.solve();

        let mut expected = LiteralSet::new();
        expected.add(a.invert());
        expected.add(b);
        expected.add(c);
        assert_eq!(solution.solution, Some(expected));
    }
}
