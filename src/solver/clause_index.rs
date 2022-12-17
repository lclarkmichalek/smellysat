use core::fmt;

use fnv::{FnvHashMap, FnvHashSet};

use crate::instance::*;

use super::clause_store::{ClauseRef, ClauseRefResolver, ClauseStore};

#[derive(Clone)]
pub(crate) struct ClauseIndex {
    // Mapping from variable to the indexes of clauses containing the variable
    by_var: FnvHashMap<Variable, Vec<usize>>,
    // Variables that have been marked resolved
    resolved_vars: FnvHashSet<Variable>,
    // Mapping from the reference of a clause to its index in the following lists
    clause_ref_indexes: FnvHashMap<ClauseRef, usize>,
    // The number of free variables in the clause at the given index
    free_var_count: Vec<usize>,
    // no free var is used for evaluation. one free for unit prop.
    no_free_var_clauses: FnvHashSet<usize>,
    one_free_var_clauses: FnvHashSet<usize>,
    two_free_var_clause_count: usize,
}

impl ClauseIndex {
    // We assume at this point that all literals are free.
    pub(crate) fn new<'a, R>(resolver: R, clauses: &Vec<ClauseRef>) -> ClauseIndex
    where
        R: ClauseRefResolver<'a>,
    {
        let free_var_count = clauses.iter().map(|c| c.len()).collect();

        let mut idx = ClauseIndex {
            by_var: FnvHashMap::default(),
            resolved_vars: FnvHashSet::default(),
            clause_ref_indexes: FnvHashMap::default(),
            free_var_count,
            no_free_var_clauses: FnvHashSet::default(),
            one_free_var_clauses: FnvHashSet::default(),
            two_free_var_clause_count: 0,
        };

        for (i, &clause) in clauses.iter().enumerate() {
            idx.clause_ref_indexes.insert(clause, i);
        }

        for (i, &clause) in clauses.iter().enumerate() {
            for lit in resolver.clause_literals(clause) {
                idx.by_var.entry(lit.var()).or_insert(vec![]).push(i);
            }
        }

        for (i, &clause) in clauses.iter().enumerate() {
            match clause.len() {
                0 => {
                    panic!("empty clause: {:?}", i)
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
        self.resolved_vars.insert(var);

        let entry = self.by_var.get(&var);
        if entry.is_none() {
            return;
        }
        for &ix in entry.unwrap() {
            self.free_var_count[ix] -= 1;
            match self.free_var_count[ix] {
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
        self.resolved_vars.remove(&var);

        if let Some(ixes) = self.by_var.get(&var) {
            for &ix in ixes {
                self.free_var_count[ix] += 1;
                match self.free_var_count[ix] {
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

    pub(crate) fn add_clause(&mut self, clause: ClauseRef, literals: &Vec<Literal>) {
        let ix = self.free_var_count.len();
        self.clause_ref_indexes.insert(clause, ix);

        let free_count = literals
            .iter()
            .filter(|lit| !self.resolved_vars.contains(&lit.var()))
            .count();
        self.free_var_count.push(free_count);

        match free_count {
            0 => {
                self.no_free_var_clauses.insert(ix);
            }
            1 => {
                self.one_free_var_clauses.insert(ix);
            }
            _ => {
                self.two_free_var_clause_count += 1;
            }
        }
    }
}

pub(crate) struct ClauseIndexView<'a> {
    store: &'a ClauseStore,
    idx: &'a ClauseIndex,
}

impl<'a> ClauseIndexView<'a> {
    pub(crate) fn new(store: &'a ClauseStore, index: &'a ClauseIndex) -> ClauseIndexView<'a> {
        ClauseIndexView { store, idx: index }
    }

    pub(crate) fn find_unit_prop_candidates(&self, literal: Literal) -> Vec<ClauseRef> {
        match self.idx.by_var.get(&literal.var()) {
            None => vec![],
            Some(clause_ixes) => clause_ixes
                .iter()
                .filter(|ix| self.idx.one_free_var_clauses.contains(ix))
                .filter_map(|&ix| self.store.get(ix))
                .collect(),
        }
    }

    pub(crate) fn find_evaluatable_candidates(&self, literal: Literal) -> Vec<ClauseRef> {
        match self.idx.by_var.get(&literal.var()) {
            None => vec![],
            Some(clause_ixes) => clause_ixes
                .iter()
                .filter(|ix| self.idx.no_free_var_clauses.contains(ix))
                .filter_map(|&ix| self.store.get(ix))
                .collect(),
        }
    }

    pub(crate) fn all_clauses_resolved(&self) -> bool {
        self.idx.no_free_var_clauses.len() == self.idx.free_var_count.len()
    }
}

impl<'a> fmt::Debug for ClauseIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ClauseIndex {{ clauses: {:?}, no_free: {:?}, one_free: {:?}, more_free: {:?} }}",
            self.free_var_count.len(),
            self.no_free_var_clauses.len(),
            self.one_free_var_clauses.len(),
            self.two_free_var_clause_count
        )
    }
}

#[cfg(test)]
mod test {
    use crate::{instance::*, solver::clause_store::ClauseStore};

    #[test]
    fn test_clause_index() {
        let a = Variable(0);
        let b = Variable(1);
        let c = Variable(2);
        let clauses = vec![
            // a || c
            Clause::new(&vec![Literal::new(a, true), Literal::new(c, true)]),
            // b || c
            Clause::new(&vec![Literal::new(b, true), Literal::new(c, true)]),
            // c || c
            Clause::new(&vec![Literal::new(c, true), Literal::new(c, true)]),
            // b
            Clause::new(&vec![Literal::new(b, true)]),
        ];

        let mut store = ClauseStore::new(clauses);
        let idx = store.idx();

        assert!(!idx.all_clauses_resolved());

        // With a=false, the first clause is a candidate for unit prop
        let nota = Literal::new(a, false);
        store.mark_resolved(nota.var());
        assert_eq!(store.idx().find_unit_prop_candidates(nota).len(), 1);
        store.mark_unresolved(nota.var());
        // With b=false, the second clause is a candidate for unit prop
        let notb = Literal::new(b, false);
        store.mark_resolved(notb.var());
        assert_eq!(store.idx().find_unit_prop_candidates(notb).len(), 1);
        store.mark_unresolved(notb.var());
        // With c=false, the 3rd clause is evaluatable
        let notc = Literal::new(c, false);
        store.mark_resolved(notc.var());
        assert_eq!(store.idx().find_evaluatable_candidates(notc).len(), 1);
        store.mark_unresolved(notc.var());

        // Now let's resolve everything
        store.mark_resolved(a);
        store.mark_resolved(b);
        store.mark_resolved(c);
        assert!(store.idx().all_clauses_resolved());
    }
}
