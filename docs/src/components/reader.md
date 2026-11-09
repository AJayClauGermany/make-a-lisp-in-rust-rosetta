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

## Notes from the rows

Every grammar crate had to be bent to read one form and ignore the rest,
and only lalrpop and chumsky report running out of input in words the test
suite accepts on their own; the others decide it from the error position.
chumsky's parser type borrows the input, so it is rebuilt on every
`read_str` call and its timings include that. See
[Trait notes](../trait-notes.md#reader) for the details and proposed
refinements.

## Candidates

| crate | parser type | action code | integration | step reached | version |
|-------|-------------|-------------|-------------|--------------|---------|
| std   | hand-written recursive descent | in source | library | stepA_mal | - |
| [nom] | combinators | in source | library | stepA_mal | 8.0.0 |
| [winnow] | combinators | in source | library | stepA_mal | 1.0.5 |
| [chumsky] | combinators | in source | library | stepA_mal | 0.13.0 |
| [pest] | PEG | external grammar | proc macro (file) | stepA_mal | 2.9.5 |
| [peg] | PEG | in grammar | proc macro (block) | stepA_mal | 0.8.6 |
| [lalrpop] | LR(1) | in grammar | build script | stepA_mal | 0.22.2 |
| [logos] | lexer + hand-written descent | in source | proc macro | stepA_mal | 0.16.1 |
| [regex] | tokeniser only (the guide's approach) | in source | library | stepA_mal | 1.13.1 |

[nom]: https://github.com/rust-bakery/nom
[winnow]: https://github.com/winnow-rs/winnow
[chumsky]: https://github.com/zesterer/chumsky
[pest]: https://github.com/pest-parser/pest
[peg]: https://github.com/kevinmehall/rust-peg
[logos]: https://github.com/maciejhirsz/logos
[lalrpop]: https://github.com/lalrpop/lalrpop
[regex]: https://github.com/rust-lang/regex
