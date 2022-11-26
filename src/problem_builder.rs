use std::cell::RefCell;

use crate::instance::Variable;
use crate::solver::Instance;
use crate::variable_registry::VariableRegister;
use crate::Literal;

// Do an implace tseitin transform, to avoid implementing annoyign things like DFS
#[derive(Clone, Debug)]
pub struct ProblemBuilder {
    variables: RefCell<VariableRegister>,
    expressions: RefCell<Vec<Vec<Literal>>>,
}

impl ProblemBuilder {
    pub fn new() -> ProblemBuilder {
        ProblemBuilder {
            variables: RefCell::new(VariableRegister::new()),
            expressions: RefCell::new(vec![]),
        }
    }

    pub fn var(&mut self, name: &str) -> BoolExpr {
        let lit = self.variables.borrow_mut().create_original(name);

        BoolExpr::Variable(lit)
    }

    pub fn require(&mut self, expr: BoolExpr) {
        self.expressions.borrow_mut().push(vec![expr.as_literal()])
    }

    pub fn build(self) -> Instance {
        Instance::new(self.expressions.into_inner(), self.variables.into_inner())
    }

    pub fn not(&self, expr: BoolExpr) -> BoolExpr {
        match expr {
            BoolExpr::Not(lit) => BoolExpr::Variable(lit),
            BoolExpr::Variable(lit) => BoolExpr::Not(lit),
        }
    }

    pub fn or(&self, a: BoolExpr, b: BoolExpr) -> BoolExpr {
        let expr_label = self.variables.borrow_mut().create_tseitin();

        // Do the tseitin shuffle
        let cnf_terms = vec![
            vec![
                Literal::new(expr_label, false),
                a.as_literal(),
                b.as_literal(),
            ],
            vec![Literal::new(expr_label, true), a.as_literal().invert()],
            vec![Literal::new(expr_label, true), b.as_literal().invert()],
        ];

        self.expressions.borrow_mut().extend(cnf_terms);
        BoolExpr::Variable(expr_label)
    }

    pub fn and(&self, a: BoolExpr, b: BoolExpr) -> BoolExpr {
        let expr_label = self.variables.borrow_mut().create_tseitin();

        // Do the tseitin shuffle AGAIN
        let cnf_terms = vec![
            vec![
                Literal::new(expr_label, true),
                a.as_literal().invert(),
                b.as_literal().invert(),
            ],
            vec![Literal::new(expr_label, false), a.as_literal()],
            vec![Literal::new(expr_label, false), b.as_literal()],
        ];

        self.expressions.borrow_mut().extend(cnf_terms);
        BoolExpr::Variable(expr_label)
    }
}

// lol
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolExpr {
    Not(Variable),
    Variable(Variable),
}
impl BoolExpr {
    fn as_literal(&self) -> Literal {
        match self {
            &BoolExpr::Variable(lit) => Literal::new(lit, true),
            &BoolExpr::Not(lit) => Literal::new(lit, false),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_build_unary_problem() {
        let mut pb = ProblemBuilder::new();

        let x = pb.var("x");
        let y = pb.var("y");

        assert_ne!(x, y);
        assert_ne!(pb.not(x), x);
    }

    #[test]
    fn test_build_binary_problem() {
        let mut pb = ProblemBuilder::new();

        let x = pb.var("x");
        let y = pb.var("y");

        // assert_eq!(pb.literals.borrow().var_count, 2);
        assert_ne!(pb.and(x, pb.or(y, y)), x);
        // assert_eq!(pb.literals.borrow().var_count, 4);
        assert_eq!(pb.expressions.borrow().len(), 6);
    }

    #[test]
    fn test_build_and_run_simple_assignment() {
        let mut pb = ProblemBuilder::new();

        let children = vec!["laurie", "lucy", "eric", "rita"];
        let seats = vec!["a", "b", "c", "d"];

        let mut by_child = HashMap::new();
        let mut by_seat = HashMap::new();

        for child in children.iter() {
            for seat in seats.iter() {
                let var = pb.var(&format!("{}x{}", &child, &seat));
                by_child
                    .entry(child)
                    .or_insert(HashMap::new())
                    .insert(seat, var);
                by_seat
                    .entry(seat)
                    .or_insert(HashMap::new())
                    .insert(child, var);
            }
        }
        // everyone needs a seat
        for child in &children {
            pb.require(or_list(
                &pb,
                &by_child[child]
                    .values()
                    .map(|x| *x)
                    .collect::<Vec<BoolExpr>>(),
            ))
        }

        let mut instance = pb.build();
        let sol = instance.solve();
        assert!(sol.solution.is_some());
    }

    fn or_list(pb: &ProblemBuilder, xs: &Vec<BoolExpr>) -> BoolExpr {
        match xs.len() {
            0 => panic!("Cannot or empty list"),
            1 => xs[0],
            _ => {
                let mut acc = xs[0];
                for i in 1..xs.len() {
                    acc = pb.or(acc, xs[i])
                }
                acc
            }
        }
    }
}
