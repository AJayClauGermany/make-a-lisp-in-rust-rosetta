//! The `smallvec` environment row: each scope is a short list of
//! `(symbol, value)` pairs searched front to back, stored inline in a
//! `SmallVec` until it outgrows eight bindings.
//!
//! Most scopes in a Lisp are function calls and `let*` forms with a handful
//! of bindings, where a linear scan over a few inline pairs beats hashing.
//! The top-level scope, which holds the whole core namespace, is the
//! exception: it spills to the heap and every lookup that reaches it scans
//! about a hundred entries. That trade-off is what this row measures.
//!
//! Otherwise the shape is the `std` row's: a link to the outer scope, `Rc`
//! handles and a `RefCell` for `def!`.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use mal_traits::{Component, CrateRef, Env, Implementation, Step};
use smallvec::SmallVec;

/// Table entry for [`SmallVecEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "smallvec",
    component: Component::Env,
    crates: &[CrateRef {
        name: "smallvec",
        version: "1.16.5",
        url: "https://github.com/servo/rust-smallvec",
    }],
    highest_step: Step::Mal,
    notes: "linear scan over up to 8 inline (symbol, value) pairs per scope, no hashing",
};

/// Bindings stored inline before a scope spills to the heap.
const INLINE: usize = 8;

/// One scope's bindings, in binding order.
type Bindings<V> = SmallVec<[(Box<str>, V); INLINE]>;

/// A shared handle to one scope of bindings.
pub struct SmallVecEnv<V>(Rc<Scope<V>>);

struct Scope<V> {
    bindings: RefCell<Bindings<V>>,
    outer: Option<SmallVecEnv<V>>,
}

impl<V> SmallVecEnv<V> {
    /// An empty top-level environment.
    pub fn new() -> Self {
        Self::with_outer(None)
    }

    fn with_outer(outer: Option<Self>) -> Self {
        Self(Rc::new(Scope {
            bindings: RefCell::new(SmallVec::new()),
            outer,
        }))
    }
}

impl<V> Default for SmallVecEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

// Derived `Clone` would require `V: Clone`; cloning the handle never
// clones values.
impl<V> Clone for SmallVecEnv<V> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<V> fmt::Debug for SmallVecEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bindings = self.0.bindings.borrow();
        f.debug_struct("SmallVecEnv")
            .field("bindings", &bindings.len())
            .field("spilled", &bindings.spilled())
            .field("has_outer", &self.0.outer.is_some())
            .finish()
    }
}

impl<V: Clone> Env for SmallVecEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        Self::with_outer(Some(self.clone()))
    }

    fn set(&self, symbol: &str, value: V) {
        let mut bindings = self.0.bindings.borrow_mut();
        match bindings.iter_mut().find(|(name, _)| &**name == symbol) {
            Some((_, slot)) => *slot = value,
            None => bindings.push((symbol.into(), value)),
        }
    }

    fn get(&self, symbol: &str) -> Option<V> {
        let mut scope = &self.0;
        loop {
            if let Some((_, value)) = scope
                .bindings
                .borrow()
                .iter()
                .find(|(name, _)| &**name == symbol)
            {
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
        mal_testkit::check_env(SmallVecEnv::<mal_types::Value>::new);
    }

    #[test]
    fn small_scopes_stay_inline() {
        let env = SmallVecEnv::<i32>::new();
        for (i, name) in ["a", "b", "c"].into_iter().enumerate() {
            env.set(name, i32::try_from(i).unwrap());
        }
        assert!(!env.0.bindings.borrow().spilled());
        assert_eq!(env.get("c"), Some(2));
    }
}
