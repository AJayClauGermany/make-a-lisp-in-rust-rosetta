//! The `lasso` environment row: symbols are interned once in a `lasso::Rodeo`
//! shared by the whole environment tree, and each scope maps the resulting
//! 32-bit `Spur` keys to values.
//!
//! Interning is how most Lisp implementations avoid string keys altogether:
//! the reader turns a symbol into an integer once and every later lookup
//! compares integers. Here it can only go half way. `Env::set` and
//! `Env::get` take `&str`, and the shared value type stores symbols as
//! `Rc<str>`, so every lookup still hashes the symbol's text, this time in
//! the interner, before the scope chain is walked with the integer key.
//! What the row does gain is that a symbol that was never bound anywhere is
//! rejected after that one interner lookup, without walking the chain, and
//! that scopes store `u32` keys instead of owned strings, so binding a
//! parameter on every function call no longer allocates.
//!
//! Otherwise the shape is the `std` row's: one map per scope, a link to the
//! outer scope, `Rc` handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use lasso::{Rodeo, Spur};
use mal_traits::{Component, CrateRef, Env, Implementation, Step};

/// Table entry for [`LassoEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "lasso",
    component: Component::Env,
    crates: &[CrateRef {
        name: "lasso",
        version: "0.7.3",
        url: "https://github.com/Kixiron/lasso",
    }],
    highest_step: Step::Mal,
    notes: "symbols interned in one shared Rodeo; scopes keyed by u32 Spur, but &str lookups still hash the text",
};

impl<V> mal_traits::Row for LassoEnv<V> {
    const IMPLEMENTATION: &'static Implementation = &IMPLEMENTATION;
}

/// A shared handle to one scope of bindings.
pub struct LassoEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    /// The interner, shared by every scope descended from the same root.
    symbols: Rc<RefCell<Rodeo>>,
    bindings: RefCell<HashMap<Spur, V>>,
    outer: Option<LassoEnv<V>>,
}

impl<V> LassoEnv<V> {
    /// An empty top-level environment with a fresh interner.
    pub fn new() -> Self {
        Self(Rc::new(Scope {
            symbols: Rc::new(RefCell::new(Rodeo::new())),
            bindings: RefCell::new(HashMap::new()),
            outer: None,
        }))
    }

    /// How many distinct symbols have been bound anywhere in this tree.
    pub fn interned(&self) -> usize {
        self.0.symbols.borrow().len()
    }
}

impl<V> Default for LassoEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for LassoEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for LassoEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LassoEnv")
            .field("bindings", &self.0.bindings.borrow().len())
            .field("interned", &self.interned())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for LassoEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        Self(Rc::new(Scope {
            symbols: Rc::clone(&self.0.symbols),
            bindings: RefCell::new(HashMap::new()),
            outer: Some(self.clone()),
        }))
    }

    fn set(&self, symbol: &str, value: V) {
        let key = self.0.symbols.borrow_mut().get_or_intern(symbol);
        self.0.bindings.borrow_mut().insert(key, value);
    }

    fn get(&self, symbol: &str) -> Option<V> {
        // Never interned means never bound, in any scope.
        let key = self.0.symbols.borrow().get(symbol)?;
        let mut scope = &self.0;
        loop {
            if let Some(value) = scope.bindings.borrow().get(&key) {
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
        mal_testkit::check_env(LassoEnv::<mal_types::Value>::new);
    }

    #[test]
    fn scopes_share_one_interner() {
        let root = LassoEnv::<i32>::new();
        root.set("a", 1);
        let child = root.child();
        child.set("a", 2);
        child.set("b", 3);
        assert_eq!(root.interned(), 2);
        // Looking up an unknown symbol does not intern it.
        assert_eq!(child.get("zzz"), None);
        assert_eq!(root.interned(), 2);
    }
}
