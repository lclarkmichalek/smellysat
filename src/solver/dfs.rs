use std::fmt;
use std::rc::Rc;

use crate::instance::*;
use crate::solver::knowledge_graph::KnowledgeGraph;
use crate::variable_registry::VariableRegister;

use super::assignment_set::LiteralSet;
use super::clause_index::ClauseIndex;
use super::unit_propagator::UnitPropagator;

#[derive(Clone)]
pub(crate) struct SearchPath {
    initial_inferred_literals: Vec<Literal>,
    path: Vec<SearchPathEntry>,
    current_assignments: LiteralSet,

    assignments_since_last_step: Vec<Literal>,
}

#[derive(Clone)]
struct SearchPathEntry {
    chosen: Vec<Literal>,
    inferred: Vec<Literal>,
}

impl SearchPath {
    pub(crate) fn new() -> SearchPath {
        SearchPath {
            initial_inferred_literals: vec![],
            path: vec![SearchPathEntry {
                chosen: vec![],
                inferred: vec![],
            }],
            current_assignments: LiteralSet::new(),

            assignments_since_last_step: vec![],
        }
    }

    pub(crate) fn assignments_since_last_step(&self) -> &Vec<Literal> {
        &self.assignments_since_last_step
    }

    pub(crate) fn assignment(&self) -> &LiteralSet {
        &self.current_assignments
    }

    pub(crate) fn step(&mut self, clause_index: &mut ClauseIndex, lit: Literal) {
        self.current_assignments.add(lit);
        clause_index.mark_resolved(lit.var());

        self.assignments_since_last_step.clear();
        self.assignments_since_last_step.push(lit);

        self.path.push(SearchPathEntry {
            chosen: vec![lit],
            inferred: vec![],
        });
    }

    pub(crate) fn add_initial_unit(&mut self, literal: Literal) {
        self.current_assignments.add(literal);
        self.assignments_since_last_step.push(literal);
        self.path.last_mut().unwrap().chosen.push(literal);
    }

    pub(crate) fn add_inferred(&mut self, literal: Literal) {
        self.current_assignments.add(literal);
        self.assignments_since_last_step.push(literal);
        self.path.last_mut().unwrap().inferred.push(literal);
    }

    fn backtrack(&mut self, clause_index: &mut ClauseIndex) -> Option<Vec<SearchPathEntry>> {
        // Find the last time we took a 'true' path (i.e. left hand path)
        let last_lhs = self.path.iter().rposition(|step| step.chosen.polarity());
        if last_lhs == None {
            return None;
        }
        // Drop everything after and including that
        let backtrack_ix = last_lhs.unwrap();
        // println!("backtracking {:}", self.path.len() - backtrack_ix);
        let backtracked: Vec<SearchPathEntry> = self.path.drain(backtrack_ix..).collect();
        for step in &backtracked {
            self.current_assignments.remove(step.chosen);
            clause_index.mark_unresolved(step.chosen.var());
            for &ass in &step.inferred {
                self.current_assignments.remove(ass);
                clause_index.mark_unresolved(ass.var());
            }
        }
        // Reset some states
        self.assignments_since_last_step.clear();
        Some(backtracked)
    }

    fn backtrack_and_pivot(&mut self, clause_index: &mut ClauseIndex) -> bool {
        if let Some(backtracked) = self.backtrack(clause_index) {
            // And then put in a traversal on the RHS
            let pivot = backtracked[0].chosen;
            // println!("pivot: {:?}", pivot);
            self.step(clause_index, pivot.invert());
            true
        } else {
            // If we can't backtrack further, then we finished the tree without a solution
            false
        }
    }

    fn depth(&self) -> usize {
        self.path.len()
    }

    fn size(&self) -> usize {
        self.current_assignments.size()
    }
}

impl fmt::Debug for SearchPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "SearchPath {{ depth={:?}, assignment=[{:?}] }}",
            self.depth(),
            self.current_assignments
        )
    }
}

#[derive(Debug, Clone)]
struct TraversalPath {
    variables: Rc<VariableRegister>,
}

impl TraversalPath {
    fn next(&self, path: &SearchPath) -> Option<&Variable> {
        self.variables
            .iter()
            .filter(|&&l| path.current_assignments.get(l).is_none())
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

    fn do_clause_evaluation<'a>(
        &'a self,
        clause_index: &mut ClauseIndex,
        path: &mut SearchPath,
        kg: &mut KnowledgeGraph,
    ) -> IterationResult {
        let mut prop = UnitPropagator::new(clause_index, path, kg);
        match prop.evaluate() {
            Some(conflict) => {
                // println!(
                //     "backtracking due to violated clauses: {:?}",
                //     violated_clauses
                // );
                if !path.backtrack_and_pivot(clause_index) {
                    return IterationResult::Backtracked(None);
                }
                return IterationResult::Backtracked(Some(()));
            }
            _ => IterationResult::Ok,
        }
    }

    fn do_unit_propagation<'a>(
        &'a self,
        clause_index: &mut ClauseIndex,
        path: &mut SearchPath,
        kg: &mut KnowledgeGraph,
    ) -> IterationResult {
        let mut prop = UnitPropagator::new(clause_index, path, kg);
        match prop.propagate_units() {
            Some(conflict) => {
                // println!(
                //     "backtracking due to violated clauses: {:?}",
                //     violated_clauses
                // );
                if !path.backtrack_and_pivot(clause_index) {
                    return IterationResult::Backtracked(None);
                }
                return IterationResult::Backtracked(Some(()));
            }
            _ => IterationResult::Ok,
        }
    }

    pub fn solve(&mut self) -> Solution {
        let mut stats = EvaluationStats {
            step_count: 0,
            unit_prop_count: 0,
            backtrack_from_violation_count: 0,
            backtrack_from_conflict_count: 0,
        };
        let mut path = SearchPath::new();
        let traversal_plan = TraversalPath {
            variables: self.variables.clone(),
        };

        let mut clause_index = ClauseIndex::new(&self.clauses);
        let mut kg = KnowledgeGraph::new();

        // for clause in &self.clauses {
        //     println!("{:?}", clause);
        // }

        {
            let mut prop = UnitPropagator::new(&mut clause_index, &mut path, &mut kg);
            if let Some(conflict) = prop.initial_unit_propagation() {
                println!("conflict during initial unit propagation: {:?}", conflict);
                return Solution {
                    literals: self.variables.clone(),
                    solution: None,
                    stats,
                };
            }
        }

        println!("inferred {} units pre traversal", path.size());

        if clause_index.all_clauses_resolved() {
            return Solution {
                literals: self.variables.clone(),
                solution: Some(path.current_assignments),
                stats: stats,
            };
        }

        if let Some(var) = traversal_plan.next(&path) {
            let lit = Literal::new(*var, true);
            path.step(&mut clause_index, lit);
            kg.add_decision(lit);
        } else {
            panic!("Empty problem?");
        }

        loop {
            // println!("evaluating with {:?} vars", path.current_assignments.size());

            match self.do_clause_evaluation(&mut clause_index, &mut path, &mut kg) {
                IterationResult::Backtracked(None) => {
                    return Solution {
                        literals: self.variables.clone(),
                        solution: None,
                        stats,
                    };
                }
                IterationResult::Backtracked(Some(_)) => continue,
                IterationResult::Ok => (),
            };

            if clause_index.all_clauses_resolved() {
                return Solution {
                    literals: self.variables.clone(),
                    solution: Some(path.current_assignments),
                    stats: stats,
                };
            }

            // If we didn't find a solution, but we didn't hit any violated constraints, we can try unit propagation
            // println!("dfs step evaluated. free clauses: {}", free_clauses.len(),);
            match self.do_unit_propagation(&mut clause_index, &mut path, &mut kg) {
                IterationResult::Backtracked(None) => {
                    return Solution {
                        literals: self.variables.clone(),
                        solution: None,
                        stats,
                    };
                }
                IterationResult::Backtracked(Some(_)) => continue,
                IterationResult::Ok => (),
            }

            if clause_index.all_clauses_resolved() {
                return Solution {
                    literals: self.variables.clone(),
                    solution: Some(path.current_assignments),
                    stats: stats,
                };
            }

            // Now, keep stepping into the problem
            if let Some(var) = traversal_plan.next(&path) {
                let lit = Literal::new(*var, true);
                stats.step_count += 1;
                path.step(&mut clause_index, lit);
                kg.add_decision(lit);
            } else {
                // If we can't keep going, we're done, i guess. Iterate one more time
                continue;
            }
        }
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
    pub(crate) literals: Rc<VariableRegister>,
    pub(crate) solution: Option<LiteralSet>,
    pub stats: EvaluationStats,
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
        solver::{clause_index::ClauseIndex, dfs::SearchPath},
        *,
    };

    #[test]
    fn test_search_path_bookkeeping() {
        let mut sp = SearchPath::new();
        let clauses = vec![];
        let mut ci = ClauseIndex::new(&clauses);
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);

        sp.step(&mut ci, Literal::new(a, true));
        assert_eq!(sp.depth(), 1);
        assert_eq!(sp.size(), 1);

        sp.step(&mut ci, Literal::new(b, true));
        sp.add_inferred(Literal::new(c, false));
        assert_eq!(sp.depth(), 2);
        assert_eq!(sp.size(), 3);

        sp.backtrack(&mut ci);
        assert_eq!(sp.depth(), 1);
        assert_eq!(sp.size(), 1);

        sp.step(&mut ci, Literal::new(b, true));
        assert_eq!(sp.depth(), 2);
        assert_eq!(sp.size(), 2);

        sp.backtrack_and_pivot(&mut ci);
        assert_eq!(sp.depth(), 2);
        assert_eq!(sp.size(), 2);
    }

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
        let mut pb = ProblemBuilder::new();

        let a = pb.var("a");
        let b = pb.var("b");
        let c = pb.var("c");
        pb.require(a);
        pb.require(pb.and(pb.not(a), b));
        pb.require(pb.and(pb.not(c), c));

        let mut instance = pb.build();
        let solution = instance.solve();
        assert!(solution.solution.is_some());
        println!("{:?}", solution);
    }
}
