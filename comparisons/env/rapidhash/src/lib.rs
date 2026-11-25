//! The `rapidhash` environment row: std's `HashMap` with rapidhash, a newer
//! word-at-a-time hasher, next to `foldhash`, `ahash`, `rustc-hash` and
//! `fnv` on the same table.
//!
//! rapidhash's documentation recommends `GlobalState` (one seed per process)
//! over the per-map `RandomState` "when creating many maps in a tight loop",
//! which is what an interpreter does: every function call makes a scope. So
//! the row follows that advice rather than using the `RapidHashMap` alias.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use std::collections::HashMap;

use mal_traits::{Component, CrateRef, Env, Implementation, Step};
use rapidhash::fast::GlobalState;

/// Table entry for [`RapidHashEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "rapidhash",
    component: Component::Env,
    crates: &[CrateRef {
        name: "rapidhash",
        version: "4.5.1",
        url: "https://github.com/hoxxep/rapidhash",
    }],
    highest_step: Step::Mal,
    notes: "std HashMap with rapidhash's GlobalState (one seed per process) per scope",
};

impl<V> mal_traits::Row for RapidHashEnv<V> {
    const IMPLEMENTATION: &'static Implementation = &IMPLEMENTATION;
}

/// A shared handle to one scope of bindings.
pub struct RapidHashEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<HashMap<String, V, GlobalState>>,
    outer: Option<RapidHashEnv<V>>,
}

impl<V> RapidHashEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(HashMap::default()),
            outer,
        }))
    }
}

impl<V> Default for RapidHashEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for RapidHashEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for RapidHashEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RapidHashEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for RapidHashEnv<V> {
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
        mal_testkit::check_env(RapidHashEnv::<mal_types::Value>::new);
    }
}
