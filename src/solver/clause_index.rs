use core::fmt;

use fnv::{FnvHashMap, FnvHashSet};

use crate::instance::*;

#[derive(Clone)]
pub(crate) struct ClauseIndex<'a> {
    clause_states: Vec<ClauseState<'a>>,
    // mapping from variable to the indexes of clauses containing the variable
    by_var: FnvHashMap<Variable, Vec<usize>>,
    // one free var is used for evaluation. two free for unit prop.
    one_free_var_clauses: FnvHashSet<usize>,
    two_free_var_clauses: FnvHashSet<usize>,
    three_free_var_clause_count: usize,
}

impl<'a> ClauseIndex<'a> {
    // We assume at this point that all literals are free.
    pub(crate) fn new(clauses: &'a Vec<Clause>) -> ClauseIndex<'a> {
        let clause_states = clauses
            .iter()
            .map(|c| ClauseState {
                clause: c,
                free_variable_count: c.len(),
            })
            .collect();

        let mut idx = ClauseIndex {
            clause_states: clause_states,
            by_var: FnvHashMap::default(),
            one_free_var_clauses: FnvHashSet::default(),
            two_free_var_clauses: FnvHashSet::default(),
            three_free_var_clause_count: 0,
        };

        for (i, clause) in clauses.iter().enumerate() {
            for lit in clause.literals() {
                idx.by_var.entry(lit.var()).or_insert(vec![]).push(i)
            }
            match clause.literals().len() {
                1 => {
                    idx.one_free_var_clauses.insert(i);
                }
                2 => {
                    idx.two_free_var_clauses.insert(i);
                }
                0 => {}
                _ => {
                    idx.three_free_var_clause_count += 1;
                }
            };
        }

        idx
    }

    pub(crate) fn mark_resolved(&mut self, var: Variable) {
        let entry = self.by_var.get(&var);
        if entry.is_none() {
            return;
        }
        println!("resolving {:?}", var);
        for &ix in entry.unwrap() {
            self.clause_states[ix].free_variable_count -= 1;
            match self.clause_states[ix].free_variable_count {
                0 => {
                    self.one_free_var_clauses.remove(&ix);
                }
                1 => {
                    self.one_free_var_clauses.insert(ix);
                    self.two_free_var_clauses.remove(&ix);
                }
                2 => {
                    self.two_free_var_clauses.insert(ix);
                    self.three_free_var_clause_count -= 1;
                }
                _ => {}
            };
        }
    }

    pub(crate) fn mark_unresolved(&mut self, var: Variable) {
        println!("unresolving {:?}", var);
        if let Some(ixes) = self.by_var.get(&var) {
            for &ix in ixes {
                self.clause_states[ix].free_variable_count += 1;
                match self.clause_states[ix].free_variable_count {
                    1 => {
                        self.one_free_var_clauses.insert(ix);
                    }
                    2 => {
                        self.one_free_var_clauses.remove(&ix);
                        self.two_free_var_clauses.insert(ix);
                    }
                    3 => {
                        self.two_free_var_clauses.remove(&ix);
                        self.three_free_var_clause_count += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn all_clauses_resolved(&self) -> bool {
        self.one_free_var_clauses.is_empty()
            && self.two_free_var_clauses.is_empty()
            && self.three_free_var_clause_count == 0
    }

    pub(crate) fn find_unit_prop_candidates(&self, literal: Literal) -> Vec<&'a Clause> {
        match self.by_var.get(&literal.var()) {
            None => vec![],
            Some(clause_ixes) => clause_ixes
                .iter()
                .filter(|ix| self.one_free_var_clauses.contains(ix))
                .map(|&ix| self.clause_states[ix].clause)
                .collect(),
        }
    }

    pub(crate) fn find_evaluatable_candidates(&self, literal: Literal) -> Vec<&'a Clause> {
        match self.by_var.get(&literal.var()) {
            None => vec![],
            Some(clause_ixes) => clause_ixes
                .iter()
                .filter(|ix| self.one_free_var_clauses.contains(ix))
                .map(|&ix| self.clause_states[ix].clause)
                .collect(),
        }
    }

    pub(crate) fn find_unit_clauses(&self) -> Vec<&'a Clause> {
        self.one_free_var_clauses
            .iter()
            .map(|&ix| self.clause_states[ix].clause)
            .filter(|cl| cl.len() == 1)
            .collect()
    }
}

impl<'a> fmt::Debug for ClauseIndex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClauseIndex {{ clauses: {:?}, one_free: {:?}, two_free: {:?}, more_free: {:?} }}",
            self.clause_states.len(),
            self.one_free_var_clauses.len(),
            self.two_free_var_clauses.len(),
            self.three_free_var_clause_count
        )
    }
}

#[derive(Clone)]
struct ClauseState<'a> {
    clause: &'a Clause,
    free_variable_count: usize,
}
