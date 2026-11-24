//! The `fnv` environment row: std's `HashMap` with the Fowler–Noll–Vo hash,
//! the long-standing "fast hasher for short keys" that predates `FxHash`,
//! aHash and foldhash. It hashes one byte at a time, so it is a useful
//! reference point for the newer word-at-a-time hashers.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use fnv::FnvHashMap;
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`FnvEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "fnv",
    component: Component::Env,
    crates: &[CrateRef {
        name: "fnv",
        version: "1.0.7",
        url: "https://github.com/servo/rust-fnv",
    }],
    highest_step: Step::Mal,
    notes: "std HashMap with the byte-at-a-time FNV-1a hasher per scope",
};

/// A shared handle to one scope of bindings.
pub struct FnvEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<FnvHashMap<String, V>>,
    outer: Option<FnvEnv<V>>,
}

impl<V> FnvEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(FnvHashMap::default()),
            outer,
        }))
    }
}

impl<V> Default for FnvEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for FnvEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for FnvEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FnvEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for FnvEnv<V> {
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
        mal_testkit::check_env(FnvEnv::<mal_types::Value>::new);
    }
}
