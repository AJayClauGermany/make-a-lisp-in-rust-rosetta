//! The `slotmap` environment row: every scope of an environment tree lives
//! in one `SlotMap` arena, and an environment handle is the arena plus a
//! generational key.
//!
//! An arena is the usual alternative to `Rc` for a graph of scopes: one
//! allocation pool, keys instead of pointers, outer links that are plain
//! `Copy` keys. The `Env` trait makes the handle `Clone` and says nothing
//! about when a scope may be freed, so the arena cannot know when a
//! function call's scope is garbage. A row that never removed scopes would
//! grow without bound under the benchmark loops, so this one counts handles
//! per scope and removes a scope when its count, and the counts of the
//! scopes it kept alive, drop to zero. That is reference counting again,
//! done by hand: the arena saves the per-scope `Rc` allocation but not the
//! bookkeeping, and a tracing collector over the arena would need a hook
//! the trait does not have.
//!
//! Dropping a scope drops its values, and a value may hold another handle
//! into the same arena (a closure captures its environment), so values are
//! always dropped after the arena's `RefCell` borrow is released.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use mal_traits::{Component, CrateRef, Env, Implementation, Step};
use slotmap::{DefaultKey, SlotMap};

/// Table entry for [`SlotMapEnv`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "slotmap",
    component: Component::Env,
    crates: &[CrateRef {
        name: "slotmap",
        version: "1.1.1",
        url: "https://github.com/orlp/slotmap",
    }],
    highest_step: Step::Mal,
    notes: "all scopes in one SlotMap arena, handles are keys; Clone handles force hand-written refcounts",
};

type Arena<V> = Rc<RefCell<SlotMap<DefaultKey, Scope<V>>>>;

/// A handle to one scope in a shared arena.
pub struct SlotMapEnv<V> {
    arena: Arena<V>,
    key: DefaultKey,
}

struct Scope<V> {
    bindings: HashMap<String, V>,
    outer: Option<DefaultKey>,
    /// Handles to this scope plus child scopes that name it as `outer`.
    refs: usize,
}

impl<V> SlotMapEnv<V> {
    /// An empty top-level environment in a fresh arena.
    pub fn new() -> Self {
        let mut arena = SlotMap::new();
        let key = arena.insert(Scope {
            bindings: HashMap::new(),
            outer: None,
            refs: 1,
        });
        Self {
            arena: Rc::new(RefCell::new(arena)),
            key,
        }
    }

    /// How many scopes are alive in this environment's arena.
    pub fn live_scopes(&self) -> usize {
        self.arena.borrow().len()
    }
}

impl<V> Default for SlotMapEnv<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V> Clone for SlotMapEnv<V> {
    fn clone(&self) -> Self {
        self.arena.borrow_mut()[self.key].refs += 1;
        Self {
            arena: Rc::clone(&self.arena),
            key: self.key,
        }
    }
}

impl<V> Drop for SlotMapEnv<V> {
    fn drop(&mut self) {
        // Collect the scopes to free, then drop them (and the values in
        // them) once the arena is no longer borrowed.
        let mut freed = Vec::new();
        {
            let mut arena = self.arena.borrow_mut();
            let mut key = Some(self.key);
            while let Some(k) = key {
                let scope = &mut arena[k];
                scope.refs -= 1;
                if scope.refs > 0 {
                    break;
                }
                let scope = arena.remove(k).expect("live scope");
                key = scope.outer;
                freed.push(scope);
            }
        }
        drop(freed);
    }
}

impl<V> fmt::Debug for SlotMapEnv<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let arena = self.arena.borrow();
        let scope = &arena[self.key];
        f.debug_struct("SlotMapEnv")
            .field("bindings", &scope.bindings.len())
            .field("has_outer", &scope.outer.is_some())
            .field("live_scopes", &arena.len())
            .finish()
    }
}

impl<V: Clone> Env for SlotMapEnv<V> {
    type Value = V;

    fn child(&self) -> Self {
        let mut arena = self.arena.borrow_mut();
        // The child keeps its outer scope alive.
        arena[self.key].refs += 1;
        let key = arena.insert(Scope {
            bindings: HashMap::new(),
            outer: Some(self.key),
            refs: 1,
        });
        Self {
            arena: Rc::clone(&self.arena),
            key,
        }
    }

    fn set(&self, symbol: &str, value: V) {
        let replaced = self.arena.borrow_mut()[self.key]
            .bindings
            .insert(symbol.to_owned(), value);
        // Dropped here, after the borrow, in case it held a handle.
        drop(replaced);
    }

    fn get(&self, symbol: &str) -> Option<V> {
        let arena = self.arena.borrow();
        let mut key = self.key;
        loop {
            let scope = &arena[key];
            if let Some(value) = scope.bindings.get(symbol) {
                return Some(value.clone());
            }
            key = scope.outer?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_env(SlotMapEnv::<mal_types::Value>::new);
    }

    #[test]
    fn scopes_are_freed_with_their_last_handle() {
        let root = SlotMapEnv::<i32>::new();
        let child = root.child();
        let grandchild = child.child();
        let kept = grandchild.clone();
        assert_eq!(root.live_scopes(), 3);
        drop(child);
        drop(grandchild);
        // `kept` still holds the grandchild, which holds the child.
        assert_eq!(root.live_scopes(), 3);
        assert_eq!(kept.get("missing"), None);
        drop(kept);
        assert_eq!(root.live_scopes(), 1);
    }

    /// A value that can hold a handle into its own arena, as a closure
    /// does.
    #[derive(Clone)]
    enum Val {
        // Only ever held so that dropping it releases the handle.
        #[allow(dead_code)]
        Env(SlotMapEnv<Val>),
    }

    #[test]
    fn values_holding_handles_are_dropped_outside_the_borrow() {
        let root = SlotMapEnv::<Val>::new();
        let scope = root.child();
        // The scope holds a handle to a child of itself.
        scope.set("f", Val::Env(scope.child()));
        assert_eq!(root.live_scopes(), 3);
        // Replacing the binding drops that handle, which frees its scope.
        scope.set("f", Val::Env(root.clone()));
        assert_eq!(root.live_scopes(), 2);
        // Freeing the scope drops a value that holds a root handle.
        drop(scope);
        assert_eq!(root.live_scopes(), 1);
        assert!(root.get("f").is_none());
    }
}
