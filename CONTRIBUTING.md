# Contributing

The point of this repository is the comparison tables, so the most useful
contribution is a new row: another crate doing one of the component jobs.

## Adding a crate to a comparison

1. **Pick the component** the crate is a candidate for. The six components
   and their directory names are in `mal_traits::Component`:
   `reader`, `printer`, `env`, `eval`, `core`, `line-editor`.

2. **Create the crate** at `comparisons/<component>/<crate>/`, named
   `<component>-<crate>` (for example `comparisons/reader/nom/` with package
   name `reader-nom`). Depend on `mal-traits` by path:

   ```toml
   [package]
   name = "reader-nom"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   publish.workspace = true

   [dependencies]
   mal-traits = { path = "../../../crates/mal-traits" }
   mal-types = { path = "../../../crates/mal-types" }
   nom = "8"

   [dev-dependencies]
   mal-testkit = { path = "../../../crates/mal-testkit" }

   [lints]
   workspace = true
   ```

3. **Add it to the workspace** in the root `Cargo.toml` `members` list. Cargo
   rejects globs that match nothing, so members are listed explicitly.

4. **Implement the trait** for that component (`mal_traits::Reader`,
   `Printer`, `Env`, `Evaluator`, `CoreNamespace` or `LineEditor`) over
   `mal_types::Value`, and export one
   `pub const IMPLEMENTATION: mal_traits::Implementation` describing it: the
   crates used with exact versions (a test checks them against
   `Cargo.lock`), the highest mal step it passes, and one line of notes for
   the table. Readers, printers and environments call the shared checks
   from a test: `mal_testkit::check_reader(NomReader::default)`.

5. **Register it** in `crates/mal/src/registry.rs` so `mal --list` and the
   generated tables see it, add it as a dependency of `crates/mal`, and add
   a line to `build` in `crates/mal/src/select.rs` so `mal --reader nom`
   runs it. `cargo test -p mal --test conformance` then runs the mal test
   suite against it and fails unless it reaches exactly the step it claims;
   `MAL_CONFORMANCE=reader/nom` runs just that row.

   If the trait made you write something a user of the crate would not,
   keep it and add a note to `docs/src/trait-notes.md`.

6. **Add a page** at `docs/src/components/<component>.md` if the component
   does not have one yet, and a row in that component's table.

7. **Benchmark it** with one line in `crates/mal/benches/components.rs`, in
   the function for its component:

   ```rust
   mal_bench::reader(c, &reader_nom::IMPLEMENTATION, reader_nom::NomReader::default);
   ```

   Then `cargo xtask bench` records a run under `runs/` and regenerates
   `docs/src/results.md`; commit both. See the
   [benchmarking page](https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/book/benchmarking.html)
   for the components that also take fixtures, and for recording runs.

Keep implementations idiomatic for the crate being compared rather than
optimal for the benchmark. The comparison is about what it is like to use the
crate for this job; a row nobody would write in practice is not informative.

## What gets measured

See [the methodology page](https://ajayclaugermany.github.io/make-a-lisp-in-rust-rosetta/book/methodology.html). In short, per
implementation: Criterion timings of its trait methods on fixed workloads
(recorded today), and, planned, binary-size overhead against the `std`
baseline, debug build time, run time on the mal test suite for the highest
step reached, and the crate's download count at the time of the run.

## Checks

CI runs the following; please run them before pushing.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p mal --benches        # runs every benchmark once, as a smoke test
cargo xtask report --check         # docs/src/results.md matches the newest run
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
```

## Documentation site

The site is built with [oranda](https://github.com/axodotdev/oranda): the
landing page is `README.md`, and the mdBook in `docs/` becomes the `book/`
section (this file is included as its Contributing chapter). `oranda dev`
serves a live preview. The `Web` workflow publishes the site to GitHub Pages
on every push to `main`.
