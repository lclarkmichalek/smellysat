use std::fmt;
use std::rc::Rc;

use crate::instance::*;
use crate::solver::dfs_path::DFSPath;
use crate::solver::knowledge_graph::{self, KnowledgeGraph};
use crate::solver::unit_propagator::{find_inital_assignment, Conflict, InitialAssignmentResult};
use crate::variable_registry::VariableRegister;

use super::assignment_set::LiteralSet;
use super::clause_index::ClauseIndex;
use super::unit_propagator::UnitPropagator;

#[derive(Debug, Clone)]
struct TraversalPath {
    variables: Rc<VariableRegister>,
}

impl TraversalPath {
    fn next(&self, path: &DFSPath) -> Option<&Variable> {
        self.variables
            .iter()
            .filter(|&&l| path.assignment().get(l).is_none())
            .next()
    }
}

enum IterationResult {
    Ok,
    Backtracked(Option<()>),
}

#[derive(Clone)]
pub struct Instance {
    pub(crate) variables: Rc<VariableRegister>,
    pub(crate) clauses: Vec<Clause>,
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
        }
    }

    pub fn solve(&mut self) -> Solution {
        let mut stats = EvaluationStats {
            step_count: 0,
            unit_prop_count: 0,
            backtrack_from_violation_count: 0,
            backtrack_from_conflict_count: 0,
        };
        let traversal_plan = TraversalPath {
            variables: self.variables.clone(),
        };

        let mut clause_index = ClauseIndex::new(&self.clauses);
        let mut knowledge_graph = KnowledgeGraph::new();

        let mut dfs_path = match find_inital_assignment(&mut clause_index, &mut knowledge_graph) {
            InitialAssignmentResult::Conflict(conflict) => {
                return Solution {
                    literals: self.variables.clone(),
                    solution: None,
                    stats,
                }
            }
            InitialAssignmentResult::Assignment(vars) => {
                DFSPath::new(LiteralSet::from_assignment_vec(&vars))
            }
        };

        println!(
            "inferred {} units pre traversal",
            dfs_path.assignment().size()
        );

        if clause_index.all_clauses_resolved() {
            return Solution {
                literals: self.variables.clone(),
                solution: Some(dfs_path.assignment().clone()),
                stats: stats,
            };
        }

        loop {
            // println!("evaluating with {:?} vars", path.current_assignments.size());
            let mut unit_prop =
                UnitPropagator::new(&mut clause_index, &mut dfs_path, &mut knowledge_graph);

            let prop_eval_result = unit_prop.propagate_units().or_else(|| unit_prop.evaluate());
            if let Some(conflict) = prop_eval_result {
                eprintln!("conflict: {:?}", conflict);
                match self.backtrack_and_pivot(
                    conflict,
                    &mut dfs_path,
                    &mut clause_index,
                    &mut knowledge_graph,
                ) {
                    None => {
                        return Solution {
                            literals: self.variables.clone(),
                            solution: None,
                            stats,
                        }
                    }
                    Some(_) => continue,
                }
            }

            if clause_index.all_clauses_resolved() {
                return Solution {
                    literals: self.variables.clone(),
                    solution: Some(dfs_path.assignment().clone()),
                    stats: stats,
                };
            }

            // Now, keep stepping into the problem
            if let Some(&var) = traversal_plan.next(&dfs_path) {
                let lit = Literal::new(var, true);
                stats.step_count += 1;
                dfs_path.add_decision(lit);
                knowledge_graph.add_decision(lit);
                clause_index.mark_resolved(var)
            } else {
                // If we can't keep going, we're done, i guess. Iterate one more time
                continue;
            }
        }
    }

    fn backtrack_and_pivot(
        &self,
        conflict: Conflict,
        path: &mut DFSPath,
        clause_index: &mut ClauseIndex,
        knowledge_graph: &mut KnowledgeGraph,
    ) -> Option<()> {
        let backtracked = path.backtrack();
        // if there was no decision before we backtracked.. we ran out. EOF. over.
        let last_decision = match backtracked.last_decision {
            None => return None,
            Some(decision) => decision,
        };

        // Rollback the assignments
        for lit in backtracked.assignments.iter() {
            clause_index.mark_unresolved(lit.var());
        }

        // Find the next place to go
        let pivot = last_decision.invert();

        path.add_decision(pivot);
        clause_index.mark_resolved(pivot.var());
        knowledge_graph.add_decision(pivot);

        Some(())
    }
}

#[derive(Clone, Debug)]
pub struct EvaluationStats {
    step_count: usize,
    unit_prop_count: usize,
    backtrack_from_violation_count: usize,
    backtrack_from_conflict_count: usize,
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
}
