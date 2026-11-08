# Reader

**Trait:** `mal_traits::Reader`. **Directory:** `comparisons/reader/`.

The reader tokenises source text on the mal token grammar and builds lists,
vectors, hash-maps, strings, keywords, numbers and symbols, including the
reader macros `'`, `` ` ``, `~`, `~@`, `@` and `^`. It is the component with
the richest field of candidate crates, and the one most directly comparable
to [parse-rosetta-rs](https://github.com/rosetta-rs/parse-rosetta-rs).

## What the job demands

- Unbalanced delimiters and bad string escapes must be reported as errors,
  not panics; the test suite checks the messages loosely.
- Blank and comment-only input is `ReadOutcome::Empty`, not an error.
- Only the first top-level form is read; trailing input is ignored.

## Candidates

| crate | parser type | action code | integration | step reached | version |
|-------|-------------|-------------|-------------|--------------|---------|
| std   | hand-written | in source  | library     | -            | -       |
| [nom] | combinators | in source | library | | |
| [winnow] | combinators | in source | library | | |
| [chumsky] | combinators | in source | library | | |
| [pest] | PEG | external grammar | proc macro (file) | | |
| [peg] | PEG | in grammar | proc macro (block) | | |
| [logos] | lexer | in source | proc macro | | |
| [lalrpop] | LR(1) | in grammar | build script | | |
| [regex] | tokeniser only (the guide's approach) | in source | library | | |

[nom]: https://github.com/rust-bakery/nom
[winnow]: https://github.com/winnow-rs/winnow
[chumsky]: https://github.com/zesterer/chumsky
[pest]: https://github.com/pest-parser/pest
[peg]: https://github.com/kevinmehall/rust-peg
[logos]: https://github.com/maciejhirsz/logos
[lalrpop]: https://github.com/lalrpop/lalrpop
[regex]: https://github.com/rust-lang/regex
