//! The `foldhash` environment row: std's `HashMap` with foldhash, the hasher
//! hashbrown now defaults to, so together with the `hashbrown` row this
//! separates the hasher from the table.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use foldhash::{HashMap, HashMapExt};
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`FoldHashEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "foldhash",
    component: Component::Env,
    crates: &[CrateRef {
        name: "foldhash",
        version: "0.2.0",
        url: "https://github.com/orlp/foldhash",
    }],
    highest_step: Step::Mal,
    notes: "std HashMap with foldhash (fast, minimally resistant to hash flooding) per scope",
};

/// A shared handle to one scope of bindings.
pub struct FoldHashEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<HashMap<String, V>>,
    outer: Option<FoldHashEnv<V>>,
}

impl<V> FoldHashEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(HashMap::new()),
            outer,
        }))
    }
}

impl<V> Default for FoldHashEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for FoldHashEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for FoldHashEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FoldHashEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for FoldHashEnv<V> {
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
        mal_testkit::check_env(FoldHashEnv::<mal_types::Value>::new);
    }
}
