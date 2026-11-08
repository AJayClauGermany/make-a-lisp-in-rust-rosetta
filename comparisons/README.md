# Comparison implementations

One crate per `<component>/<crate>` pair, for example `reader/nom`,
`env/im`, `line-editor/rustyline`. Every crate here:

- depends on `mal-traits`, `mal-types` and the crate(s) being compared,
  nothing else (plus `mal-testkit` for tests);
- implements exactly one component trait;
- exports `pub const IMPLEMENTATION: mal_traits::Implementation`;
- is listed in the root `Cargo.toml`, in `crates/mal/src/registry.rs` and in
  `crates/mal/src/select.rs`, so the conformance tests run the mal test
  suite against it;
- has one line in `crates/mal/benches/components.rs` so it gets measured.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the step-by-step version.

The directory names are the `Component::slug()` values:

| directory      | trait                      |
|----------------|----------------------------|
| `reader/`      | `mal_traits::Reader`       |
| `printer/`     | `mal_traits::Printer`      |
| `env/`         | `mal_traits::Env`          |
| `eval/`        | `mal_traits::Evaluator`    |
| `core/`        | `mal_traits::CoreNamespace`|
| `line-editor/` | `mal_traits::LineEditor`   |
