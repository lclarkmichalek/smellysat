use std::fmt;

use super::Variable;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Literal(u64);

pub const MAX_LITERAL: u64 = 1 << 63;

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.polarity() {
            write!(f, "{:?}", self.var())
        } else {
            write!(f, "!{:?}", self.var())
        }
    }
}

impl Literal {
    pub fn new(var: Variable, polarity: bool) -> Literal {
        if var.0 > MAX_LITERAL {
            panic!("variable too large - must be < 2^63");
        }
        Literal((var.0 << 1) | (polarity as u64))
    }

    pub fn var(&self) -> Variable {
        Variable(self.0 >> 1)
    }
    pub fn polarity(&self) -> bool {
        (self.0 & 1) != 0
    }

    pub fn invert(&self) -> Literal {
        Literal(self.0 ^ 1)
    }
}

#[cfg(test)]
mod test {
    use crate::instance::*;

    #[test]
    fn test_literal_bookkeeping() {
        for idx in vec![0, 10000000, 1000, 1 << 46] {
            let var = Variable(idx);
            let lit = Literal::new(var, true);
            assert_eq!(lit.var(), var);
            assert_eq!(lit.invert().var(), var);
            assert_eq!(lit.polarity(), true);
            assert_eq!(lit.invert().polarity(), false);
        }
    }
}
