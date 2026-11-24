# Environment

**Trait:** `mal_traits::Env`. **Directory:** `comparisons/env/`.

An environment maps symbols to values and chains to an outer environment for
lexical scoping. Closures capture environments, so environments are shared
and cheap to clone. This component compares map crates and persistent
data-structure crates, and is sensitive to allocation count and lookup speed
on short string keys.

## What the job demands

- `child()` creates an empty scope that falls back to its parent.
- `set()` binds in the innermost scope only.
- `get()` walks outward; the evaluator turns `None` into a `'x' not found`
  error.

## How the rows are measured

Each row is timed twice: the trait methods on their own (`set`, local and
outer lookups, a miss, `child`), and the `std` evaluator running the
`PROGRAMS` workloads on that environment, which is where binding parameters
and walking scopes show up in practice.

The persistent maps (`imbl`, the maintained fork of `im`, and `rpds`) cannot
be used the way they are meant to be: `Env::set` through a parent has to be
visible to children created before it, which rules out giving each child a
structurally shared snapshot. Both rows keep the scope chain and a `RefCell`
like the others.

The second batch varied the *shape* of a scope rather than the hasher.
`smallvec` drops hashing for a linear scan over a few inline pairs. `lasso`
interns symbols, but because `Env` takes `&str` every lookup still hashes the
text in the interner first. `slotmap` puts every scope in one arena, and
because environment handles are `Clone` with no notion of when a scope is
dead, the row has to count handles per scope by hand: an arena of scopes
behind this trait is reference counting again. See
[Trait notes](../trait-notes.md#environment).

## Candidates

| crate | structure | sharing | step reached | version |
|-------|-----------|---------|--------------|---------|
| std   | `HashMap` (SipHash) | `Rc<RefCell<_>>` | stepA_mal | - |
| [hashbrown] | SwissTable `HashMap` (foldhash), `entry_ref` | `Rc<RefCell<_>>` | stepA_mal | 0.17.1 |
| [indexmap] | insertion-ordered map | `Rc<RefCell<_>>` | stepA_mal | 2.14.2 |
| [rustc-hash] | std `HashMap` with FxHasher | `Rc<RefCell<_>>` | stepA_mal | 2.1.3 |
| [ahash] | std `HashMap` with aHash | `Rc<RefCell<_>>` | stepA_mal | 0.8.12 |
| [foldhash] | std `HashMap` with foldhash | `Rc<RefCell<_>>` | stepA_mal | 0.2.0 |
| [imbl] | persistent `HashMap` (HAMT) | `Rc<RefCell<_>>`, see below | stepA_mal | 7.0.2 |
| [rpds] | persistent `HashTrieMap` | `Rc<RefCell<_>>`, see below | stepA_mal | 1.2.1 |
| [fnv] | std `HashMap` with FNV-1a | `Rc<RefCell<_>>` | stepA_mal | 1.0.7 |
| [smallvec] | linear scan, 8 pairs inline | `Rc<RefCell<_>>` | stepA_mal | 1.16.5 |
| [lasso] | `HashMap<Spur, V>`, symbols interned in a shared `Rodeo` | `Rc<RefCell<_>>` | stepA_mal | 0.7.3 |
| [slotmap] | every scope in one `SlotMap` arena | arena key + hand-written refcount | stepA_mal | 1.1.1 |
| [rapidhash] | std `HashMap` with rapidhash | `Rc<RefCell<_>>` | | |
| [elsa] | append-only `FrozenMap` | `Rc` without `RefCell` | | |

[hashbrown]: https://github.com/rust-lang/hashbrown
[indexmap]: https://github.com/indexmap-rs/indexmap
[imbl]: https://github.com/jneem/imbl
[rpds]: https://github.com/orium/rpds
[rustc-hash]: https://github.com/rust-lang/rustc-hash
[ahash]: https://github.com/tkaitchuck/aHash
[foldhash]: https://github.com/orlp/foldhash
[fnv]: https://github.com/servo/rust-fnv
[smallvec]: https://github.com/servo/rust-smallvec
[lasso]: https://github.com/Kixiron/lasso
[slotmap]: https://github.com/orlp/slotmap
[rapidhash]: https://github.com/hoxxep/rapidhash
[elsa]: https://github.com/Manishearth/elsa
