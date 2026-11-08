//! The `imbl` environment row: a persistent hash array mapped trie from `imbl`,
//! the maintained fork of `im`.
//!
//! The idiomatic use of a persistent map for environments is to give each
//! child scope a structurally shared *snapshot* of its parent, so lookups
//! hit one map instead of walking a chain. The `Env` trait rules that out:
//! a binding made with `set` through a parent handle must be visible to
//! children created before it (recursive `def!` after `fn*` captured the
//! scope relies on it), and snapshots would not see it. So this row keeps
//! the scope chain and a `RefCell`, and the persistent map is only a map.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use imbl::HashMap;
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`ImblEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "imbl",
    component: Component::Env,
    crates: &[CrateRef {
        name: "imbl",
        version: "7.0.2",
        url: "https://github.com/jneem/imbl",
    }],
    highest_step: Step::Mal,
    notes: "imbl::HashMap (persistent HAMT, maintained fork of im) per scope; RefCell forced by Env::set(&self)",
};

/// A shared handle to one scope of bindings.
pub struct ImblEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<HashMap<String, V>>,
    outer: Option<ImblEnv<V>>,
}

impl<V> ImblEnv<V> {
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

impl<V> Default for ImblEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for ImblEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for ImblEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImblEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for ImblEnv<V> {
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
        mal_testkit::check_env(ImblEnv::<mal_types::Value>::new);
    }
}
