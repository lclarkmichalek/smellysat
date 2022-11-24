use core::fmt;
use std::hash::Hasher;

use super::Literal;

#[derive(Clone, Eq, Ord)]
pub struct Clause {
    id: usize,
    literals: Vec<Literal>,
}

impl std::hash::Hash for Clause {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl std::cmp::PartialEq for Clause {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::cmp::PartialOrd for Clause {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Clause {
    pub(crate) fn new(lits: &Vec<Literal>) -> Clause {
        Self::new_with_id(0, lits)
    }

    pub(crate) fn new_with_id(ix: usize, lits: &Vec<Literal>) -> Clause {
        let mut clause = Clause {
            id: ix,
            literals: lits.clone(),
        };
        clause.literals.sort_by_key(|l| l.var());
        clause
    }

    pub(crate) fn len(&self) -> usize {
        self.literals.len()
    }

    pub(crate) fn is_unit(&self) -> bool {
        self.len() == 1
    }

    pub(crate) fn literals(&self) -> &Vec<Literal> {
        &self.literals
    }
}

impl fmt::Debug for Clause {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut fst = true;
        for &lit in &self.literals {
            if !fst {
                write!(f, ", ")?;
            }
            fst = false;
            write!(f, "{:?}", lit)?;
        }
        Ok(())
    }
}
