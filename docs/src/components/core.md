# Core namespace

**Trait:** `mal_traits::CoreNamespace`. **Directory:** `comparisons/core/`.

The core namespace installs the built-in functions: arithmetic and
comparison, list and vector operations, strings, hash-maps, atoms, `slurp`,
`time-ms` and the printing functions. It is where supporting crates for
numbers, strings, hashing and time enter the interpreter.

## What the job demands

- `symbols()` must list exactly what `install()` binds; the test suite's
  coverage is checked against it.
- Arithmetic in the test suite is integer only, but the guide allows richer
  numeric towers, which is what the numeric candidates explore.

## Candidates

| crate | provides | step reached | version |
|-------|----------|--------------|---------|
| std   | `i64`, `String`, `Vec` | - | - |
| [num-bigint] / [num-rational] | arbitrary precision and rational arithmetic | | |
| [rug] | GMP-backed numbers | | |
| [unicode-segmentation] | grapheme-aware string functions | | |

[num-bigint]: https://github.com/rust-num/num-bigint
[num-rational]: https://github.com/rust-num/num-rational
[rug]: https://gitlab.com/tspiteri/rug
[unicode-segmentation]: https://github.com/unicode-rs/unicode-segmentation
