//! The `ahash` environment row: std's `HashMap` with the aHash hasher, through
//! the crate's `AHashMap` wrapper: keyed and resistant to hash flooding like
//! `SipHash`, but using AES instructions where the CPU has them.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ahash::AHashMap;
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`AHashEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "ahash",
    component: Component::Env,
    crates: &[CrateRef {
        name: "ahash",
        version: "0.8.12",
        url: "https://github.com/tkaitchuck/aHash",
    }],
    highest_step: Step::Mal,
    notes: "std HashMap with aHash (AES-NI where available, resistant to hash flooding) per scope",
};

/// A shared handle to one scope of bindings.
pub struct AHashEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<AHashMap<String, V>>,
    outer: Option<AHashEnv<V>>,
}

impl<V> AHashEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(AHashMap::new()),
            outer,
        }))
    }
}

impl<V> Default for AHashEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for AHashEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for AHashEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AHashEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for AHashEnv<V> {
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
        mal_testkit::check_env(AHashEnv::<mal_types::Value>::new);
    }
}
