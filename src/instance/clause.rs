use core::fmt;
use std::hash::Hasher;

use super::Literal;

#[derive(Clone, Eq)]
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
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Clause {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl Clause {
    pub(crate) fn new(lits: &[Literal]) -> Clause {
        Self::new_with_id(0, lits)
    }

    pub(crate) fn new_with_id(ix: usize, lits: &[Literal]) -> Clause {
        let mut clause = Clause {
            id: ix,
            literals: lits.to_vec(),
        };
        clause.literals.sort_by_key(|l| l.var());
        for window in clause.literals.windows(2) {
            if window.len() != 2 {
                continue;
            }
            if window[0].var() == window[1].var() && window[0] != window[1] {
                panic!("a && !a in same clause")
            }
        }
        clause.literals.dedup();
        clause
    }

    pub(crate) fn len(&self) -> usize {
        self.literals.len()
    }

    #[allow(dead_code)]
    pub(crate) fn is_unit(&self) -> bool {
        self.len() == 1
    }

    #[allow(dead_code)]
    pub(crate) fn literals(&self) -> &Vec<Literal> {
        &self.literals
    }

    pub(crate) fn into_literals(self) -> Vec<Literal> {
        self.literals
    }
}

impl fmt::Debug for Clause {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut fst = true;
        write!(f, "[")?;
        for &lit in &self.literals {
            if !fst {
                write!(f, ", ")?;
            }
            fst = false;
            write!(f, "{:?}", lit)?;
        }
        write!(f, "]")?;
        Ok(())
    }
}
