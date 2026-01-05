#[cfg(debug_assertions)]
#[allow(unused_imports)]
use is_sorted::IsSorted;
use log::info;
use std::hash::Hasher;

use crate::instance::{Clause, Literal, Variable};

use super::clause_index::{ClauseIndex, ClauseIndexView};

#[derive(Debug)]
pub(crate) struct ClauseStore {
    clauses: ClauseList,
    index: ClauseIndex,
}

impl ClauseStore {
    pub(crate) fn new(clauses: Vec<Clause>) -> ClauseStore {
        let list = ClauseList::new(clauses);
        let refs: Vec<ClauseRef> = list.iter().collect();
        let idx = ClauseIndex::new(&list, &refs);
        ClauseStore {
            clauses: list,
            index: idx,
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = ClauseRef> + Captures<'_> {
        self.clauses.iter()
    }

    pub(crate) fn idx(&self) -> ClauseIndexView<'_> {
        ClauseIndexView::new(self, &self.index)
    }

    pub(crate) fn get(&self, ix: usize) -> Option<ClauseRef> {
        self.clauses.get(ix)
    }

    #[allow(dead_code)]
    pub(crate) fn contains(&self, clause: &[Literal]) -> bool {
        self.clauses.contains(clause)
    }

    pub(crate) fn mark_resolved(&mut self, var: Variable) {
        self.index.mark_resolved(var)
    }

    pub(crate) fn mark_unresolved(&mut self, var: Variable) {
        self.index.mark_unresolved(var)
    }

    pub(crate) fn add_clause(&mut self, clause_literals: Vec<Literal>) -> Option<ClauseRef> {
        let clause = self.clauses.add_clause(clause_literals.clone())?;
        self.index.add_clause(clause, &clause_literals);
        info!("added clause: {:?}", clause_literals);
        Some(clause)
    }
}

/// A dense store of clauses.
#[derive(Debug)]
struct ClauseList {
    // We store all the literals in the clauses contiguously
    literals: Vec<Literal>,
    // And then store the offsets for a particular clause
    offsets: Vec<usize>,
}

impl ClauseList {
    fn new(clauses: Vec<Clause>) -> ClauseList {
        let mut offsets = Vec::with_capacity(clauses.len());
        let mut literals = Vec::with_capacity(clauses.iter().map(|cl| cl.len()).sum());
        for clause in clauses.into_iter() {
            offsets.push(literals.len());
            let mut clause_literals = clause.into_literals();
            clause_literals.sort();
            literals.extend(clause_literals);
        }

        ClauseList { literals, offsets }
    }

    fn contains(&self, clause: &[Literal]) -> bool {
        ensure_sorted(clause);
        // Check we haven't already seen this clause!
        for cl in self.iter() {
            if cl.len() != clause.len() {
                continue;
            }
            if cl.literals_from_list(self).eq(clause.iter().copied()) {
                // This clause is already in the dataset
                return true;
            }
        }
        false
    }

    fn add_clause(&mut self, clause: Vec<Literal>) -> Option<ClauseRef> {
        if self.contains(&clause) {
            return None;
        }

        // otherwise, add
        let offset = self.literals.len();
        self.offsets.push(offset);
        let clause_len = clause.len();
        self.literals.extend(clause);
        Some(self.mk_ref(offset, clause_len))
    }

    fn iter(&self) -> impl Iterator<Item = ClauseRef> + Captures<'_> {
        (0..self.offsets.len()).map(|ix| self.get(ix).unwrap())
    }

    fn get(&self, ix: usize) -> Option<ClauseRef> {
        let &offset = self.offsets.get(ix)?;
        let &next_offset = self.offsets.get(ix + 1).unwrap_or(&self.literals.len());
        Some(self.mk_ref(offset, next_offset - offset))
    }

    fn mk_ref(&self, offset: usize, length: usize) -> ClauseRef {
        match length {
            0 => panic!("zero length clause"),
            1 => ClauseRef::Unit(self.literals[offset]),
            2 => ClauseRef::Pair(self.literals[offset], self.literals[offset + 1]),
            _ => ClauseRef::Long { offset, length },
        }
    }
}

#[derive(Debug, Clone, Copy, Eq)]
pub(crate) enum ClauseRef {
    Unit(Literal),
    Pair(Literal, Literal),
    /// A reference to a clause in a ClauseList
    Long {
        offset: usize,
        length: usize,
    },
}

impl ClauseRef {
    /// A slice of the literals constituting this clause
    pub(crate) fn literals<'c>(
        &self,
        store: &'c ClauseStore,
    ) -> impl Iterator<Item = Literal> + 'c {
        self.literals_from_list(&store.clauses)
    }

    fn literals_from_list<'c>(&self, list: &'c ClauseList) -> impl Iterator<Item = Literal> + 'c {
        match *self {
            ClauseRef::Unit(l) => ClauseLiteralsIterator {
                fst: Some(l),
                snd: None,
                rst: None,
            },
            ClauseRef::Pair(a, b) => ClauseLiteralsIterator {
                fst: Some(a),
                snd: Some(b),
                rst: None,
            },
            ClauseRef::Long { offset, length } => ClauseLiteralsIterator {
                fst: None,
                snd: None,
                rst: Some(&list.literals[offset..(offset + length)]),
            },
        }
    }

    /// The literal in a unit clause. Panics if the clause is not a unit
    pub(crate) fn unit(&self) -> Literal {
        match self {
            &ClauseRef::Unit(l) => l,
            _ => panic!("not a unit clause"),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match *self {
            ClauseRef::Unit(_) => 1,
            ClauseRef::Pair(_, _) => 2,
            ClauseRef::Long { offset: _, length } => length,
        }
    }

    pub(crate) fn is_unit(&self) -> bool {
        matches!(self, ClauseRef::Unit(_))
    }
}

impl std::hash::Hash for ClauseRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            ClauseRef::Unit(l) => l.hash(state),
            ClauseRef::Pair(a, b) => {
                a.hash(state);
                b.hash(state);
            }
            ClauseRef::Long { offset, length: _ } => offset.hash(state),
        }
    }
}

impl std::cmp::PartialEq for ClauseRef {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&ClauseRef::Unit(a), &ClauseRef::Unit(b)) => a == b,
            (&ClauseRef::Pair(a, b), &ClauseRef::Pair(c, d)) => a == c && b == d,
            (
                &ClauseRef::Long {
                    offset: a,
                    length: _,
                },
                &ClauseRef::Long {
                    offset: b,
                    length: _,
                },
            ) => a == b,
            _ => false,
        }
    }
}

impl std::cmp::PartialOrd for ClauseRef {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for ClauseRef {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.len().cmp(&other.len()) != std::cmp::Ordering::Equal {
            return self.len().cmp(&other.len());
        }

        match (&self, &other) {
            (ClauseRef::Unit(a), ClauseRef::Unit(b)) => a.cmp(b),
            (ClauseRef::Pair(a, b), ClauseRef::Pair(c, d)) => (a, b).cmp(&(c, d)),
            (
                ClauseRef::Long {
                    offset: a,
                    length: _,
                },
                ClauseRef::Long {
                    offset: b,
                    length: _,
                },
            ) => a.cmp(b),
            _ => unreachable!(),
        }
    }
}

/// An interface for structures that can resolve a ClauseRef into an iterator of literals
pub(crate) trait ClauseRefResolver<'a>: Copy {
    fn clause_literals(self, clause: ClauseRef) -> ClauseLiteralsIterator<'a>;
}

impl<'a, 'b: 'a> ClauseRefResolver<'a> for &'b ClauseList {
    fn clause_literals(self, clause: ClauseRef) -> ClauseLiteralsIterator<'a> {
        match clause {
            ClauseRef::Unit(l) => ClauseLiteralsIterator {
                fst: Some(l),
                snd: None,
                rst: None,
            },
            ClauseRef::Pair(a, b) => ClauseLiteralsIterator {
                fst: Some(a),
                snd: Some(b),
                rst: None,
            },
            ClauseRef::Long { offset, length } => ClauseLiteralsIterator {
                fst: None,
                snd: None,
                rst: Some(&self.literals[offset..(offset + length)]),
            },
        }
    }
}

impl<'a, 'b: 'a> ClauseRefResolver<'a> for &'b ClauseStore {
    fn clause_literals(self, clause: ClauseRef) -> ClauseLiteralsIterator<'a> {
        self.clauses.clause_literals(clause)
    }
}

#[derive(Debug)]
pub(crate) struct ClauseLiteralsIterator<'a> {
    fst: Option<Literal>,
    snd: Option<Literal>,
    rst: Option<&'a [Literal]>,
}

impl<'a> Iterator for ClauseLiteralsIterator<'a> {
    type Item = Literal;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.snd, self.fst) {
            (Some(snd), Some(fst)) => {
                self.fst = Some(snd);
                self.snd = None;
                return Some(fst);
            }
            (None, Some(fst)) => {
                self.fst = None;
                return Some(fst);
            }
            _ => {}
        }

        let literals = self.rst.unwrap_or(&[]);
        let &lit = literals.first()?;
        self.rst = Some(&literals[1..]);
        Some(lit)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = if self.snd.is_some() {
            2
        } else if self.fst.is_some() {
            1
        } else if self.rst.is_some() {
            self.rst.unwrap().len()
        } else {
            0
        };
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for ClauseLiteralsIterator<'a> {
    fn len(&self) -> usize {
        self.size_hint().0
    }
}

#[cfg(not(debug_assertions))]
fn ensure_sorted(_: &[Literal]) {}

#[cfg(debug_assertions)]
fn ensure_sorted(lits: &[Literal]) {
    if !lits.iter().is_sorted() {
        panic!("must be sorted");
    }
}

// https://github.com/rust-lang/rust/issues/34511#issuecomment-373423999
pub trait Captures<'a> {}
impl<'a, T: ?Sized> Captures<'a> for T {}

#[cfg(test)]
mod test {
    use itertools::Itertools;

    use crate::instance::{Clause, Literal, Variable};

    use super::ClauseStore;

    #[test]
    fn test_iter_clause_store() {
        let a = Literal::new(Variable(0), true);
        let b = Literal::new(Variable(1), true);
        let c = Literal::new(Variable(2), true);

        // Ensure we get coverage of long, pair, and unit clauses
        let clauses = vec![
            Clause::new(&vec![a, b, c]),
            Clause::new(&vec![b, c]),
            Clause::new(&vec![c]),
        ];

        let cs = ClauseStore::new(clauses);

        let clauses = cs.iter().collect_vec();

        assert_eq!(clauses.len(), 3);
        assert_eq!(clauses[0].literals(&cs).collect_vec(), vec![a, b, c]);
        assert_eq!(clauses[1].literals(&cs).collect_vec(), vec![b, c]);
        assert_eq!(clauses[2].literals(&cs).collect_vec(), vec![c]);
    }
}
