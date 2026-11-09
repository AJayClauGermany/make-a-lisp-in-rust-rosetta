//! The `rustc-hash` environment row: std's `HashMap` with the Fx hasher rustc
//! uses: a multiply-rotate hash that is very fast on short keys like symbol
//! names and makes no attempt at resistance to hash flooding.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use mal_traits::{Component, CrateRef, Env, Implementation, Step};
use rustc_hash::FxHashMap;

/// Table entry for [`FxEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "rustc-hash",
    component: Component::Env,
    crates: &[CrateRef {
        name: "rustc-hash",
        version: "2.1.3",
        url: "https://github.com/rust-lang/rustc-hash",
    }],
    highest_step: Step::Mal,
    notes: "std HashMap with rustc's FxHasher (not resistant to hash flooding) per scope",
};

/// A shared handle to one scope of bindings.
pub struct FxEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<FxHashMap<String, V>>,
    outer: Option<FxEnv<V>>,
}

impl<V> FxEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(FxHashMap::default()),
            outer,
        }))
    }
}

impl<V> Default for FxEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for FxEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for FxEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FxEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for FxEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        Self::with_outer(Some(self.clone()))
    }

    fn set(&self, symbol: &str, value: V) {
        self.0
            .bindings
            .borrow_mut()
            .insert(symbol.to_owned(), value);
    }

    fn get(&self, symbol: &str) -> Option<V> {
        let mut scope = &self.0;
        loop {
            if let Some(value) = scope.bindings.borrow().get(symbol) {
                return Some(value.clone());
            }
            scope = &scope.outer.as_ref()?.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_env(FxEnv::<mal_types::Value>::new);
    }
}
