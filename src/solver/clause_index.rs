use core::fmt;

use fnv::{FnvHashMap, FnvHashSet};

use crate::instance::*;

#[derive(Clone)]
pub(crate) struct ClauseIndex<'a> {
    clause_states: Vec<ClauseState<'a>>,
    // mapping from variable to the indexes of clauses containing the variable
    by_var: FnvHashMap<Variable, Vec<usize>>,
    // no free var is used for evaluation. one free for unit prop.
    no_free_var_clauses: FnvHashSet<usize>,
    one_free_var_clauses: FnvHashSet<usize>,
    two_free_var_clause_count: usize,
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
            no_free_var_clauses: FnvHashSet::default(),
            one_free_var_clauses: FnvHashSet::default(),
            two_free_var_clause_count: 0,
        };

        for (i, clause) in clauses.iter().enumerate() {
            for lit in clause.literals() {
                idx.by_var.entry(lit.var()).or_insert(vec![]).push(i)
            }
            match clause.literals().len() {
                0 => {
                    panic!("empty clause")
                }
                1 => {
                    idx.one_free_var_clauses.insert(i);
                }
                _ => {
                    idx.two_free_var_clause_count += 1;
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
                    self.no_free_var_clauses.insert(ix);
                }
                1 => {
                    self.one_free_var_clauses.insert(ix);
                    self.two_free_var_clause_count -= 1;
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
                    0 => {
                        self.no_free_var_clauses.insert(ix);
                    }
                    1 => {
                        self.no_free_var_clauses.remove(&ix);
                        self.one_free_var_clauses.insert(ix);
                    }
                    2 => {
                        self.one_free_var_clauses.remove(&ix);
                        self.two_free_var_clause_count += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn all_clauses_resolved(&self) -> bool {
        self.no_free_var_clauses.is_empty()
            && self.one_free_var_clauses.is_empty()
            && self.two_free_var_clause_count == 0
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
                .filter(|ix| self.no_free_var_clauses.contains(ix))
                .map(|&ix| self.clause_states[ix].clause)
                .collect(),
        }
    }

    pub(crate) fn find_unit_clauses(&self) -> Vec<&'a Clause> {
        self.one_free_var_clauses
            .iter()
            .map(|&ix| self.clause_states[ix].clause)
            .filter(|cl| cl.is_unit())
            .collect()
    }

    pub(crate) fn find_unit_clauses_containing_var(&self, var: Variable) -> Vec<&'a Clause> {
        match self.by_var.get(&var) {
            None => vec![],
            Some(clause_ixes) => clause_ixes
                .iter()
                .filter(|&&ix| self.clause_states[ix].clause.is_unit())
                .map(|&ix| self.clause_states[ix].clause)
                .collect(),
        }
    }
}

impl<'a> fmt::Debug for ClauseIndex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClauseIndex {{ clauses: {:?}, no_free: {:?}, one_free: {:?}, more_free: {:?} }}",
            self.clause_states.len(),
            self.no_free_var_clauses.len(),
            self.one_free_var_clauses.len(),
            self.two_free_var_clause_count
        )
    }
}

#[derive(Clone)]
struct ClauseState<'a> {
    clause: &'a Clause,
    free_variable_count: usize,
}

#[cfg(test)]
mod test {
    use crate::instance::*;

    use super::ClauseIndex;


    #[test]
    fn test_clause_index() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);
        let clauses = vec![
            // a || c
            Clause::new(&vec![Literal::new(a,true), Literal::new(c, true)]),
            // b || c
            Clause::new(&vec![Literal::new(b, true), Literal::new(c, true)]),
            // c || c
            Clause::new(&vec![Literal::new(c, true), Literal::new(c, true)]),
            // b
            Clause::new(&vec![Literal::new(b, true)]),
        ];

        let mut ci = ClauseIndex::new(&clauses);

        assert!(!ci.all_clauses_resolved());

        // the 3rd and 4th clauses are unit
        assert_eq!(ci.find_unit_clauses().len(), 2);
        // With a=false, the first clause is a candidate for unit prop
        let nota = Literal::new(a, false);
        ci.mark_resolved(nota.var());
        assert_eq!(ci.find_unit_prop_candidates(nota).len(), 1);
        ci.mark_unresolved(nota.var());
        // With b=false, the second clause is a candidate for unit prop
        let notb = Literal::new(b, false);
        ci.mark_resolved(notb.var());
        assert_eq!(ci.find_unit_prop_candidates(notb).len(), 1);
        ci.mark_unresolved(notb.var());
        // With c=false, the 3rd clause is evaluatable
        let notc = Literal::new(c, false);
        ci.mark_resolved(notc.var());
        println!("ec: {:?}", ci.find_evaluatable_candidates(notc));
        println!("ecl: {:?}", ci.find_evaluatable_candidates(notc).len());
        assert_eq!(ci.find_evaluatable_candidates(notc).len(), 1);
    }
}