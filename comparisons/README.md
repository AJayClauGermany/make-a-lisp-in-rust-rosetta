# Comparison implementations

One crate per `<component>/<crate>` pair, for example `reader/nom`,
`env/im`, `line-editor/rustyline`. Every crate here:

- depends on `mal-traits`, `mal-types` and the crate(s) being compared,
  nothing else (plus `mal-testkit` for tests);
- implements exactly one component trait;
- exports `pub const IMPLEMENTATION: mal_traits::Implementation` and
  implements `mal_traits::Row` for its type, so the type names its own row;
- is listed in the root `Cargo.toml`, as a dependency of `crates/mal`, and
  once in the row lists of `crates/mal/src/stack.rs`. That one entry makes
  it selectable (`mal --env <name>`), registered (`mal --list`, the results
  page), benchmarked, run against the mal test suite, and combinable with
  every other row (see `docs/src/stacks.md`). Evaluator and core rows also
  need a family marker next to `StdEvalRow` / `StdCoreRow` there. Line
  editors are not part of a stack: they are listed in `LINE_EDITORS` in
  `crates/mal/src/registry.rs` and constructed in the `--line-editor` match
  in `crates/mal/src/main.rs`, and are not benchmarked.

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
