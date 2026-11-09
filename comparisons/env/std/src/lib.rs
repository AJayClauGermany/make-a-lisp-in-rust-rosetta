//! The `std` environment row: a `HashMap` per scope behind `Rc<RefCell<_>>`.
//!
//! The textbook shape from the mal guide: each scope owns a map and a link
//! to its outer scope, and lookups walk outward. Sharing comes from `Rc`,
//! and `RefCell` provides the mutation `def!` needs through a shared handle.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use mal_traits::{Component, Env, Implementation, Step};

/// Table entry for [`StdEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "std",
    component: Component::Env,
    crates: &[],
    highest_step: Step::Mal,
    notes: "HashMap<String, V> (SipHash) per scope, Rc<RefCell<_>> handles, outer-scope chain",
};

/// A shared handle to one scope of bindings.
pub struct StdEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<HashMap<String, V>>,
    outer: Option<StdEnv<V>>,
}

impl<V> StdEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(HashMap::new()),
            outer: None,
        }))
    }
}

impl<V> Default for StdEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for StdEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for StdEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for StdEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(HashMap::new()),
            outer: Some(self.clone()),
        }))
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
        mal_testkit::check_env(StdEnv::<mal_types::Value>::new);
    }
}
