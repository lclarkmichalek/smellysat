// The `instance` module contains the data model for the instance. These types are immutable.
mod variable;
pub use crate::instance::variable::Variable;

mod literal;
pub use crate::instance::literal::Literal;

mod clause;
pub use crate::instance::clause::Clause;
