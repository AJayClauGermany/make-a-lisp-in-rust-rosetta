//! The core namespace of built-in functions.

use crate::Env;

/// Provides the built-in functions that populate the top-level environment.
///
/// This is the `core.qx` module of the mal guide, introduced at
/// [`Step::IfFnDo`](crate::Step::IfFnDo) and extended at nearly every step
/// after. Implementations install arithmetic, comparison, sequence,
/// string, atom and I/O functions, and are compared on which supporting
/// crates they use for numbers, strings, hashing and time.
pub trait CoreNamespace {
    /// The environment type the namespace is installed into.
    type Env: Env;

    /// Install every built-in this namespace provides into `env`.
    fn install(&self, env: &Self::Env);

    /// The symbols [`install`](Self::install) will bind, for reporting and
    /// coverage checks against the mal test suite.
    fn symbols(&self) -> Vec<&'static str>;
}
