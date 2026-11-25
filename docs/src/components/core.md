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
- `read-string`, `pr-str`, `str`, `prn` and `println` use the reader and
  printer rows the interpreter was built with; `eval` is installed by the
  interpreter, since it needs the evaluator.

## Status

Only the `std` row exists: the numeric candidates need a number type other
than the shared `Value::Int(i64)`, and the benchmark only times `install`.
See [Trait notes](../trait-notes.md#core-namespace).

## Candidates

| crate | provides | step reached | version |
|-------|----------|--------------|---------|
| std   | checked `i64`, `String`, `Vec`; all stepA built-ins | stepA_mal | - |
| [num-bigint] / [num-rational] | arbitrary precision and rational arithmetic | | |
| [rug] | GMP-backed numbers | | |
| [unicode-segmentation] | grapheme-aware string functions | | |
| [dashu-int] / [malachite] | pure-Rust big integers (malachite is LGPL) | | |
| [bnum] | fixed-width `i256`, no heap | | |
| [jiff] / [quanta] | `time-ms` from a calendar library / a TSC clock | | |

[num-bigint]: https://github.com/rust-num/num-bigint
[num-rational]: https://github.com/rust-num/num-rational
[rug]: https://gitlab.com/tspiteri/rug
[unicode-segmentation]: https://github.com/unicode-rs/unicode-segmentation
[dashu-int]: https://crates.io/crates/dashu-int
[malachite]: https://crates.io/crates/malachite
[bnum]: https://crates.io/crates/bnum
[jiff]: https://crates.io/crates/jiff
[quanta]: https://crates.io/crates/quanta
