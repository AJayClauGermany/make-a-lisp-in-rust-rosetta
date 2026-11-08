//! The evaluator.

use crate::Env;

/// Evaluates a form in an environment.
///
/// This is the `EVAL` function of the mal guide and the heart of each
/// `stepN_*.qx` file. Special forms, function application, tail-call
/// optimisation, macro expansion and `try*`/`catch*` all live here.
///
/// The evaluator fixes the value representation, which is why it is the
/// component that compares memory-management strategies (`Rc`, `Arc`, arenas,
/// tracing garbage collectors) rather than any one crate category.
pub trait Evaluator {
    /// The value representation this evaluator operates on.
    type Value;
    /// The environment type this evaluator binds symbols in.
    type Env: Env<Value = Self::Value>;
    /// Raised for unbound symbols, bad arity, and `throw`.
    type Error: std::error::Error;

    /// Evaluate `ast` in `env`.
    fn eval(&mut self, ast: Self::Value, env: &Self::Env) -> Result<Self::Value, Self::Error>;
}
