//! The `rpds` environment row: the persistent `HashTrieMap` from `rpds`,
//! updated in place with `insert_mut` since nothing else holds the old version.
//!
//! As with the `imbl` row, the persistent structure cannot be used the way
//! it is meant to be (structurally shared snapshots per child scope)
//! because `Env::set` through a parent must be visible to existing
//! children; the scope chain and `RefCell` stay.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use mal_traits::{Component, CrateRef, Env, Implementation, Step};
use rpds::HashTrieMap;

/// Table entry for [`RpdsEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "rpds",
    component: Component::Env,
    crates: &[CrateRef {
        name: "rpds",
        version: "1.2.1",
        url: "https://github.com/orium/rpds",
    }],
    highest_step: Step::Mal,
    notes: "rpds::HashTrieMap (persistent HAMT, Rc-shared) per scope; RefCell forced by Env::set(&self)",
};

/// A shared handle to one scope of bindings.
pub struct RpdsEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<HashTrieMap<String, V>>,
    outer: Option<RpdsEnv<V>>,
}

impl<V> RpdsEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(HashTrieMap::new()),
            outer,
        }))
    }
}

impl<V> Default for RpdsEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for RpdsEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for RpdsEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RpdsEnv")
            .field("bindings", &self.0.bindings.borrow().size())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for RpdsEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        Self::with_outer(Some(self.clone()))
    }

    fn set(&self, symbol: &str, value: V) {
        self.0
            .bindings
            .borrow_mut()
            .insert_mut(symbol.to_owned(), value);
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
        mal_testkit::check_env(RpdsEnv::<mal_types::Value>::new);
    }
}
