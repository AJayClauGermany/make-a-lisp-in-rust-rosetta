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

> **Status:** scaffolding. The traits, the runner and the documentation site
> are in place; the first comparison implementations are the next step.

## Layout

| Path | What it is |
|------|------------|
| [`crates/mal-traits`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal-traits) | The component traits every implementation is written against. No dependencies. |
| [`crates/mal`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/crates/mal) | The `mal` binary: a REPL over the wired-in components, and `mal --list`. |
| [`comparisons/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/comparisons) | One crate per `<component>/<crate>` pair. Each exports an `Implementation` record and depends only on `mal-traits`. |
| [`docs/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/docs) | The [mdBook] behind the documentation site: methodology, per-component pages, results. |
| [`oranda.json`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/blob/main/oranda.json) | [oranda] configuration for the site, built and published from `main` by `.github/workflows/web.yml`. |
| [`runs/`](https://github.com/AJayClauGermany/make-a-lisp-in-rust-rosetta/tree/main/runs) | Raw measurement output, committed so results are reproducible. |

## Components

Each table lists the crates compared for one component. Measured columns are
filled in by the benchmark harness; the `std` row is the dependency-free
baseline everything else is compared against.

### Reader (`comparisons/reader/`)

| crate | parser type | action code | integration | step reached | version |
|-------|-------------|-------------|-------------|--------------|---------|
| std   | hand-written | in source  | library     | -            | -       |

### Printer (`comparisons/printer/`)

| crate | approach | step reached | version |
|-------|----------|--------------|---------|
| std   | `fmt::Write` | -        | -       |

### Env (`comparisons/env/`)

| crate | structure | sharing | step reached | version |
|-------|-----------|---------|--------------|---------|
| std   | `HashMap` | `Rc<RefCell<_>>` | - | -       |

### Evaluator (`comparisons/eval/`)

| crate | value representation | memory management | step reached | version |
|-------|----------------------|-------------------|--------------|---------|
| echo  | none                 | none              | step0_repl   | -       |

### Core (`comparisons/core/`)

| crate | provides | step reached | version |
|-------|----------|--------------|---------|
| std   | integers, strings, vectors | - | -  |

### Line editor (`comparisons/line-editor/`)

| crate | editing | history | step reached | version |
|-------|---------|---------|--------------|---------|
| std   | none    | none    | stepA_mal    | -       |

## Results

Results tables (binary-size overhead, debug build time, run time against the
mal test suite, downloads, version) are generated per component by the
benchmark harness and published on the [documentation site][site]. No
measurements have been taken yet.

## Running

```bash
cargo run -p mal            # start the REPL (currently step0: echo)
cargo run -p mal -- --list  # list the implementations available to compare
cargo test --workspace
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
[site]: https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/
