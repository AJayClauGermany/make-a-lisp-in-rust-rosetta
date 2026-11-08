# Components

The interpreter is split along the lines of the mal guide. Each component is a
trait in `mal-traits`, a directory under `comparisons/`, and a page here with
its candidate crates and qualitative table.

| component | trait | directory | compares |
|-----------|-------|-----------|----------|
| [Reader](reader.md) | `Reader` | `reader/` | parser and lexer crates |
| [Printer](printer.md) | `Printer` | `printer/` | formatting crates |
| [Environment](env.md) | `Env` | `env/` | map and persistent data-structure crates |
| [Evaluator](eval.md) | `Evaluator` | `eval/` | value representation and memory management |
| [Core namespace](core.md) | `CoreNamespace` | `core/` | numeric, string and collection crates |
| [Line editor](line-editor.md) | `LineEditor` | `line-editor/` | readline-style crates |

Candidate lists on the component pages are intentions, not commitments. A
crate is only in the comparison once its row exists under `comparisons/`.
