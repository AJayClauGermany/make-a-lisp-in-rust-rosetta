//! The `hashbrown` environment row: the Swiss-table `HashMap` that std's is
//! built on, used directly: its default hasher is foldhash rather than `SipHash`,
//! and its `entry_ref` API can bind a `&str` key without allocating when the
//! symbol is already bound.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use hashbrown::HashMap;
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`HashbrownEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "hashbrown",
    component: Component::Env,
    crates: &[CrateRef {
        name: "hashbrown",
        version: "0.17.1",
        url: "https://github.com/rust-lang/hashbrown",
    }],
    highest_step: Step::Mal,
    notes: "hashbrown::HashMap (SwissTable, foldhash) per scope; entry_ref avoids key allocation on rebind",
};

/// A shared handle to one scope of bindings.
pub struct HashbrownEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<HashMap<String, V>>,
    outer: Option<HashbrownEnv<V>>,
}

impl<V> HashbrownEnv<V> {
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

impl<V> Default for HashbrownEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for HashbrownEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for HashbrownEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HashbrownEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for HashbrownEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        Self::with_outer(Some(self.clone()))
    }

    fn set(&self, symbol: &str, value: V) {
        // `entry_ref` only allocates the owned key when the symbol is new,
        // so rebinding (`def!` in a loop, `swap!`-style updates) is free.
        self.0.bindings.borrow_mut().entry_ref(symbol).insert(value);
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
        mal_testkit::check_env(HashbrownEnv::<mal_types::Value>::new);
    }
}
