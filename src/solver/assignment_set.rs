use crate::instance::*;
use core::fmt;

use fnv::FnvHashMap;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LiteralSet {
    values: FnvHashMap<Variable, bool>,
}

impl LiteralSet {
    pub(crate) fn new() -> LiteralSet {
        return LiteralSet {
            values: FnvHashMap::default(),
        };
    }

    pub(crate) fn add(&mut self, lit: Literal) {
        self.values.insert(lit.var(), lit.polarity());
    }

    pub(crate) fn get(&self, var: Variable) -> Option<Literal> {
        self.values.get(&var).map(|&x| Literal::new(var, x))
    }

    pub(crate) fn contains(&self, lit: Literal) -> bool {
        self.get(lit.var()) == Some(lit)
    }

    pub(crate) fn contains_var(&self, var: Variable) -> bool {
        self.values.contains_key(&var)
    }

    pub(crate) fn remove(&mut self, lit: Literal) {
        let removed = self.values.remove(&lit.var());
        if removed != Some(lit.polarity()) {
            panic!("removed different value from entry set: {:?}", lit)
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.values.len()
    }

    pub(crate) fn from_assignment_vec(asses: &Vec<Literal>) -> LiteralSet {
        let mut set = LiteralSet::new();
        for ass in asses {
            set.add(*ass);
        }
        set
    }

    pub(crate) fn as_assignment_vec(&self) -> Vec<Literal> {
        self.values
            .iter()
            .map(|(k, v)| Literal::new(*k, *v))
            .collect()
    }

    pub(crate) fn evaluate(&self, clause: &Clause) -> EvaluationResult {
        for &literal in clause.literals() {
            if let Some(ass) = self.get(literal.var()) {
                if ass == literal {
                    return EvaluationResult::True;
                }
            } else {
                return EvaluationResult::Unknown;
            }
        }
        EvaluationResult::False
    }
}

impl fmt::Debug for LiteralSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut keys = self.values.keys().collect::<Vec<_>>();
        keys.sort();
        for key in &keys {
            write!(f, "{:?}={:?}", key, self.values[key])?;
            if key != keys.last().unwrap() {
                write!(f, ", ")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EvaluationResult {
    True,
    False,
    Unknown,
}

#[cfg(test)]
mod test {
    use crate::{
        instance::*,
        solver::assignment_set::{EvaluationResult, LiteralSet},
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
}
