# Make-a-Lisp in Rust: a rosetta comparison

Compare Rust crates by making each of them do the same job inside a Lisp
interpreter.

[Make-a-Lisp][mal] (mal) describes a small Lisp as a ladder of eleven steps
and a handful of components: a **reader**, a **printer**, an **environment**,
an **evaluator**, a **core** namespace and a **REPL**. Each component is a
job that several Rust crates can do, so each one becomes a comparison in the
spirit of [rosetta-rs]: the same task, implemented once per crate, measured
the same way, with the results in a table.

This repository holds the shared traits, the runner, the comparison
implementations and the published results.

> **Status:** first comparison rows in place: 9 readers, 4 printers, 8
> environments, the `std` evaluator and core namespace, and 3 line editors,
> each checked against the upstream mal test suite and benchmarked. Where
> the traits forced crates away from their idioms is recorded in
> [Trait notes][trait-notes], ahead of refining the traits.

## Layout

| Path | What it is |
|------|------------|
| [`crates/mal-traits`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal-traits) | The component traits every implementation is written against. No dependencies. |
| [`crates/mal-types`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal-types) | The shared `Rc`-based `Value` and `MalError` that rows of different components agree on. No dependencies. |
| [`crates/mal-testkit`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal-testkit) | Unit checks every reader, printer and environment row runs (dev-dependency only). |
| [`crates/mal`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal) | The `mal` binary: a stepA interpreter assembled from rows, `--<component> <name>` to swap one in, and `mal --list`. Its `benches/components.rs` is the list of what gets benchmarked, and `tests/conformance.rs` runs the mal test suite against every row. |
| [`crates/mal-bench`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal-bench) | The benchmark harness: fixed workloads and one generic [Criterion] benchmark per component trait. |
| [`xtask/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/xtask) | `cargo xtask bench` records a run under `runs/`; `cargo xtask report` renders the newest run into the results page. |
| [`comparisons/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/comparisons) | One crate per `<component>/<crate>` pair. Each exports an `Implementation` record and depends only on `mal-traits`, `mal-types` and the crate it compares. |
| [`docs/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/docs) | The [mdBook] behind the documentation site: methodology, per-component pages, results. |
| [`oranda.json`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/blob/main/oranda.json) | [oranda] configuration for the site, built and published from `main` by `.github/workflows/web.yml`. |
| [`runs/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/runs) | One `results.json` per recorded benchmark run (host, toolchain, commit, statistics), committed so results are reproducible. |

## Components

Each table lists the crates compared for one component. The `std` row is the
dependency-free baseline everything else is compared against; *step reached*
is checked by `cargo test` against the upstream mal test suite. Timings are
on the [results page][results]; the component pages on the documentation
site have the candidates not yet implemented.

### Reader (`comparisons/reader/`)

| crate | parser type | action code | integration | step reached | version |
|-------|-------------|-------------|-------------|--------------|---------|
| std     | hand-written recursive descent | in source | library | stepA_mal | - |
| nom     | combinators | in source | library | stepA_mal | 8.0.0 |
| winnow  | combinators | in source | library | stepA_mal | 1.0.5 |
| chumsky | combinators | in source | library | stepA_mal | 0.13.0 |
| pest    | PEG | external grammar | proc macro (file) | stepA_mal | 2.9.5 |
| peg     | PEG | in grammar | proc macro (block) | stepA_mal | 0.8.6 |
| lalrpop | LR(1) | in grammar | build script | stepA_mal | 0.22.2 |
| logos   | lexer + hand-written descent | in source | proc macro | stepA_mal | 0.16.1 |
| regex   | tokeniser (the guide's approach) | in source | library | stepA_mal | 1.13.1 |

### Printer (`comparisons/printer/`)

| crate | approach | step reached | version |
|-------|----------|--------------|---------|
| std    | `fmt::Write` | stepA_mal | - |
| itoa   | integers through `itoa::Buffer` | stepA_mal | 1.0.18 |
| memchr | escaping by runs between `memchr3` hits | stepA_mal | 2.8.3 |
| ufmt   | integers through `ufmt` instead of `core::fmt` | stepA_mal | 0.2.0 |

### Env (`comparisons/env/`)

| crate | structure | sharing | step reached | version |
|-------|-----------|---------|--------------|---------|
| std        | `HashMap` (SipHash) | `Rc<RefCell<_>>` | stepA_mal | - |
| hashbrown  | SwissTable `HashMap` (foldhash) | `Rc<RefCell<_>>` | stepA_mal | 0.17.1 |
| indexmap   | insertion-ordered map | `Rc<RefCell<_>>` | stepA_mal | 2.14.2 |
| rustc-hash | `HashMap` with FxHasher | `Rc<RefCell<_>>` | stepA_mal | 2.1.3 |
| ahash      | `HashMap` with aHash | `Rc<RefCell<_>>` | stepA_mal | 0.8.12 |
| foldhash   | `HashMap` with foldhash | `Rc<RefCell<_>>` | stepA_mal | 0.2.0 |
| imbl       | persistent HAMT | `Rc<RefCell<_>>` | stepA_mal | 7.0.2 |
| rpds       | persistent `HashTrieMap` | `Rc<RefCell<_>>` | stepA_mal | 1.2.1 |

### Evaluator (`comparisons/eval/`)

| crate | value representation | memory management | step reached | version |
|-------|----------------------|-------------------|--------------|---------|
| echo  | none                 | none              | step0_repl   | -       |
| std   | `Rc` value enum      | reference counting | stepA_mal   | -       |

### Core (`comparisons/core/`)

| crate | provides | step reached | version |
|-------|----------|--------------|---------|
| std   | checked `i64`, strings, vectors, all stepA built-ins | stepA_mal | - |

### Line editor (`comparisons/line-editor/`)

| crate | editing | history | step reached | version |
|-------|---------|---------|--------------|---------|
| std       | none    | none    | stepA_mal    | -       |
| rustyline | Emacs/vi keymaps | in-memory | stepA_mal | 17.0.2 |
| reedline  | Emacs keymap | built-in | stepA_mal (piped: `std::io` fallback) | 0.49.0 |

## Results

Every implementation is timed with Criterion on the same fixed workloads.
`cargo xtask bench` records a run as `runs/<id>/results.json`, and the
[results page][results] on the documentation site is generated from the
newest run together with the implementation registry, so it always shows
every registered row, measured or not. CI fails if the page is stale. The
binary-size, build-time and test-suite columns of the methodology are not
measured yet; see the [benchmarking page][benchmarking].

## Running

```bash
cargo run -p mal                      # start the REPL (all std rows)
cargo run -p mal -- --reader nom      # the same with one row swapped in
cargo run -p mal -- --list            # list the implementations available to compare
cargo test --workspace                # includes the mal test suite for every row
cargo xtask bench                     # benchmark, record under runs/, regenerate docs/src/results.md
cargo xtask report --check            # verify the results page matches the newest run
```

To preview the documentation site locally, install [oranda] and run
`oranda dev` in the repository root.

## Contributing

See [CONTRIBUTING.md](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/blob/main/CONTRIBUTING.md) for how to add a crate to a
comparison.

## License

Licensed under the [Mozilla Public License 2.0](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/blob/main/LICENSE).

[mal]: https://github.com/kanaka/mal
[rosetta-rs]: https://github.com/rosetta-rs
[mdBook]: https://rust-lang.github.io/mdBook/
[oranda]: https://github.com/axodotdev/oranda
[Criterion]: https://github.com/bheisler/criterion.rs
[site]: https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/
[results]: https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/book/results.html
[benchmarking]: https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/book/benchmarking.html
[trait-notes]: https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/book/trait-notes.html
