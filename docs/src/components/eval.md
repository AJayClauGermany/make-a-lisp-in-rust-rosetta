# Evaluator

**Trait:** `mal_traits::Evaluator`. **Directory:** `comparisons/eval/`.

The evaluator is the guide's `EVAL`: special forms, function application,
tail-call optimisation, quasiquote expansion, macro expansion and
`try*`/`catch*`. It fixes the value representation, so this is the component
that compares memory-management strategies rather than any one crate
category: reference counting, arenas and tracing garbage collectors.

## What the job demands

- `step5_tco` requires the evaluator to loop rather than recurse for `if`,
  `do`, `let*` and function application in tail position.
- Closures capture an environment; atoms are mutable; both force some form
  of shared ownership.
- Deep recursion in the test suite (`sum-to`, `fib`) is where value
  representation shows up in run time.

## Status

The `std` row is generic over the environment row, so it also drives the
environment comparison. The other candidates change the value
representation, which every reader and printer row is written against
(`mal-types`), so each would need its own reader and printer until readers
can build any representation; see
[Trait notes](../trait-notes.md#shared-value-type).

## Candidates

| crate | value representation | memory management | step reached | version |
|-------|----------------------|-------------------|--------------|---------|
| echo  | none | none | step0_repl | - |
| std   | `Rc` value enum (`mal-types`) | reference counting; closure cycles leak | stepA_mal | - |
| std   | `Arc<Value>` enum | atomic reference counting | | |
| [bumpalo] | arena references | bump allocation per REPL line | | |
| [gc] | `Gc<Value>` | tracing mark-and-sweep | | |
| [rust-gc] alternatives, e.g. [shredder] | `Gc<Value>` | concurrent tracing | | |

[bumpalo]: https://github.com/fitzgen/bumpalo
[gc]: https://github.com/Manishearth/rust-gc
[rust-gc]: https://github.com/Manishearth/rust-gc
[shredder]: https://github.com/Others/shredder
