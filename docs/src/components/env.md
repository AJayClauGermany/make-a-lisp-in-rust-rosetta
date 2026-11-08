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

## Candidates

| crate | structure | sharing | step reached | version |
|-------|-----------|---------|--------------|---------|
| std   | `HashMap` | `Rc<RefCell<_>>` | - | - |
| [hashbrown] | swiss-table `HashMap` | `Rc<RefCell<_>>` | | |
| [indexmap] | insertion-ordered map | `Rc<RefCell<_>>` | | |
| [im] | persistent `HashMap` | structural sharing | | |
| [rpds] | persistent `HashTrieMap` | structural sharing | | |
| [ahash] / [fxhash] | faster hashers over `std::HashMap` | `Rc<RefCell<_>>` | | |

[hashbrown]: https://github.com/rust-lang/hashbrown
[indexmap]: https://github.com/indexmap-rs/indexmap
[im]: https://github.com/bodil/im-rs
[rpds]: https://github.com/orium/rpds
[ahash]: https://github.com/tkaitchuck/aHash
[fxhash]: https://github.com/cbreeden/fxhash
