//! Environments: scoped symbol bindings.

/// A chain of symbol tables with lexical scoping.
///
/// This is the `env.qx` module of the mal guide. Environments are introduced
/// at [`Step::Env`](crate::Step::Env); the binding of `fn*` parameters at
/// [`Step::IfFnDo`](crate::Step::IfFnDo) uses [`Env::child`] followed by
/// [`Env::set`] for each parameter.
///
/// Environments are shared by closures, so implementations are expected to
/// be cheap to clone (a handle to shared storage rather than a deep copy).
/// Compared on lookup speed, allocation count and how naturally the crate
/// models the outer-scope chain.
pub trait Env: Clone {
    /// The value representation stored in this environment.
    type Value;

    /// Create a new, empty environment whose lookups fall back to `self`.
    #[must_use]
    fn child(&self) -> Self;

    /// Bind `symbol` to `value` in this environment (not an outer one),
    /// replacing any existing binding.
    fn set(&self, symbol: &str, value: Self::Value);

    /// Look `symbol` up in this environment, then each outer environment in
    /// turn. Equivalent to the guide's `find` + `get` without the error.
    fn get(&self, symbol: &str) -> Option<Self::Value>;

    /// Whether `symbol` is bound in this environment or any outer one.
    fn contains(&self, symbol: &str) -> bool {
        self.get(symbol).is_some()
    }
}
