use crate::instance::*;
use core::fmt;

use fnv::FnvHashMap;

#[derive(Clone)]
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

    pub(crate) fn get(&self, var: Variable) -> Option<bool> {
        self.values.get(&var).map(|x| *x)
    }

    pub(crate) fn contains(&self, lit: Literal) -> bool {
        self.get(lit.var()) == Some(lit.polarity())
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
        for literal in clause.literals() {
            if let Some(ass_sign) = self.get(literal.var()) {
                if ass_sign == literal.polarity() {
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
        *,
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

    // #[test]
    // fn test_unit_prop_unit() {
    //     let lit = Literal::new(Variable(0), true);
    //     let clause = Clause::new(&vec![lit]);
    //     assert_eq!(LiteralSet::new().unit_prop(&clause), Some(lit));
    // }

    // #[test]
    // fn test_unit_prop_single_free() {
    //     let x = Literal::new(Variable(0), true);
    //     let y = Literal::new(Variable(1), true);
    //     let clause = Clause::new(&vec![x.invert(), y]);
    //     let mut ass = LiteralSet::new();
    //     ass.add(x);
    //     assert_eq!(ass.unit_prop(&clause), Some(y));
    // }

    // #[test]
    // fn test_unit_prop_multiple_free() {
    //     let x = Literal::new(Variable(0), true);
    //     let y = Literal::new(Variable(1), true);
    //     let clause = Clause::new(&vec![x.invert(), y]);
    //     let ass = LiteralSet::new();
    //     assert_eq!(ass.unit_prop(&clause), None);
    // }

    // #[test]
    // fn test_unit_prop_true() {
    //     let x = Literal::new(Variable(0), true);
    //     let y = Literal::new(Variable(1), true);
    //     let clause = Clause::new(&vec![x, y]);
    //     let mut ass = LiteralSet::new();
    //     ass.add(x);
    //     assert_eq!(ass.unit_prop(&clause), None);
    // }

    // #[test]
    // fn test_unit_prop_scenario_two_unknown() {
    //     let mut pb = ProblemBuilder::new();
    //     let x = pb.var("x");
    //     let y = pb.var("y");

    //     // With x set, unit propagation should result in y being inferred to be true
    //     pb.require(pb.and(x, y));

    //     let instance = pb.build();
    //     // Build an assignment with x = true
    //     let x_lit = instance.variables.get_by_name("x").unwrap();
    //     let mut ass = LiteralSet::from_assignment_vec(&vec![Literal::new(x_lit, true)]);

    //     // We should now get at least one instance of unit propagation across all the clauses
    //     let mut propagation_count = 0;
    //     for clause in &instance.clauses {
    //         let result = ass.unit_prop(clause);
    //         println!("prop result: {:?}", result);
    //         if result != None {
    //             propagation_count += 1;
    //         }
    //     }
    //     assert_ne!(propagation_count, 0);

    //     // Furthermore, we should be able to solve the whole problem using unit prop only
    //     for _i in 0..100 {
    //         let mut all_pass = true;
    //         for clause in &instance.clauses {
    //             match ass.evaluate(clause) {
    //                 EvaluationResult::True => (),
    //                 EvaluationResult::False => assert!(false, "conflict during solve??"),
    //                 EvaluationResult::Unknown => {
    //                     all_pass = false;
    //                     if let Some(result) = ass.unit_prop(clause) {
    //                         println!("adding {:?} to result", result);
    //                         ass.add(result)
    //                     }
    //                 }
    //             }
    //         }
    //         if all_pass {
    //             // We solved the problem through unit propagation alone!
    //             return;
    //         }
    //     }
    //     assert!(false, "did not solve problem through unit propagation")
    // }
}