# Contributing

The point of this repository is the comparison tables, so the most useful
contribution is a new row: another crate doing one of the component jobs.

## Adding a crate to a comparison

1. **Pick the component** the crate is a candidate for. The six components
   and their directory names are in `mal_traits::Component`:
   `reader`, `printer`, `env`, `eval`, `core`, `line-editor`.

2. **Create the crate** at `comparisons/<component>/<crate>/`, named
   `<component>-<crate>` (for example `comparisons/reader/nom/` with package
   name `reader-nom`). Depend on `mal-traits` through the workspace:

   ```toml
   [package]
   name = "reader-nom"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   publish.workspace = true

   [dependencies]
   mal-traits.workspace = true
   nom = "8"

   [lints]
   workspace = true
   ```

3. **Add it to the workspace** in the root `Cargo.toml` `members` list. Cargo
   rejects globs that match nothing, so members are listed explicitly.

4. **Implement the trait** for that component (`mal_traits::Reader`,
   `Printer`, `Env`, `Evaluator`, `CoreNamespace` or `LineEditor`) and
   export one `pub const IMPLEMENTATION: mal_traits::Implementation`
   describing it: the crates used with exact versions, the highest mal step
   it passes, and one line of notes for the table.

5. **Register it** in `crates/mal/src/registry.rs` so `mal --list` and the
   generated tables see it.

6. **Add a page** at `docs/src/components/<component>.md` if the component
   does not have one yet, and a row in that component's table.

Keep implementations idiomatic for the crate being compared rather than
optimal for the benchmark. The comparison is about what it is like to use the
crate for this job; a row nobody would write in practice is not informative.

## What gets measured

See [the methodology page](docs/src/methodology.md). In short, per
implementation: binary-size overhead against the `std` baseline, debug build
time, run time on the mal test suite for the highest step reached, and the
crate's version and download count at the time of the run.

## Checks

CI runs the following; please run them before pushing.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
```

## Documentation site

The site is built with [oranda](https://github.com/axodotdev/oranda): the
landing page is `README.md`, and the mdBook in `docs/` becomes the `book/`
section (this file is included as its Contributing chapter). `oranda dev`
serves a live preview. The `Web` workflow publishes the site to GitHub Pages
on every push to `main`.
