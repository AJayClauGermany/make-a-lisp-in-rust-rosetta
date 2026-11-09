//! The `indexmap` environment row: an insertion-ordered map: a hash table of
//! indices into a dense entry vector. Nothing in mal needs the order, so this
//! row measures what the ordering costs; it would pay off for a REPL that lists
//! bindings in definition order.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use indexmap::IndexMap;
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`IndexMapEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "indexmap",
    component: Component::Env,
    crates: &[CrateRef {
        name: "indexmap",
        version: "2.14.2",
        url: "https://github.com/indexmap-rs/indexmap",
    }],
    highest_step: Step::Mal,
    notes: "indexmap::IndexMap (insertion-ordered, SipHash) per scope",
};

/// A shared handle to one scope of bindings.
pub struct IndexMapEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<IndexMap<String, V>>,
    outer: Option<IndexMapEnv<V>>,
}

impl<V> IndexMapEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(IndexMap::new()),
            outer,
        }))
    }
}

impl<V> Default for IndexMapEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for IndexMapEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for IndexMapEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexMapEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for IndexMapEnv<V> {
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
        mal_testkit::check_env(IndexMapEnv::<mal_types::Value>::new);
    }
}
