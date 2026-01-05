use std::fmt;

use super::Variable;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Literal(u32);

impl Literal {
    /// Safely transmute a mutable slice of Literals to a mutable slice of u32.
    /// This is safe because Literal is #[repr(transparent)] over u32.
    #[inline]
    pub(crate) fn slice_as_u32_mut(slice: &mut [Literal]) -> &mut [u32] {
        // SAFETY: Literal is #[repr(transparent)] over u32, so this is safe
        unsafe { std::mem::transmute(slice) }
    }
}

pub const MAX_LITERAL: u32 = 1 << 31;

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
            panic!("variable too large - must be < 2^31");
        }
        Literal((var.0 << 1) | (polarity as u32))
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
        for idx in vec![0u32, 10000000, 1000, 1 << 20] {
            let var = Variable(idx);
            let lit = Literal::new(var, true);
            assert_eq!(lit.var(), var);
            assert_eq!(lit.invert().var(), var);
            assert_eq!(lit.polarity(), true);
            assert_eq!(lit.invert().polarity(), false);
        }
    }
}
