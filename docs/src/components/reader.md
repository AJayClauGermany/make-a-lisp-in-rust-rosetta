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
`read_str` call and its timings include that.

The second batch of rows tested the trait from other directions. combine's
commit-on-consume semantics give "a missing `)` is an error at EOF" without
any `cut`. `stacker` keeps the `std` parser and removes its stack-overflow
crash on deep nesting. `lexpr` is a ready-made S-expression parser for
Scheme and Emacs Lisp rather than a parser toolkit: it fits the trait with
no friction at all (it reads one datum and stops, reports blank input as
`None`, and its error knows whether it hit end of input), but it cannot read
mal's `{}`, `~`, `@` and `^`, so it stops at step 0 although it passes 85%
of the suite's cases. It is not benchmarked, since the corpus is mal syntax.
See
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
| [stacker] | the `std` parser on a growable stack | in source | library | stepA_mal | 0.1.25 |
| [combine] | LL(1) combinators | in source | library (`parser!` macro for recursion) | stepA_mal | 4.6.9 |
| [lexpr] | ready-made S-expression parser + conversion | none (options) | library | step0_repl (85% of cases) | 0.2.7 |
| [pom] | PEG combinators | in source | library | | |
| [grmtools] (`lrpar`) | LR(1), Yacc grammar | in grammar | build script | | |
| [tree-sitter] | GLR, generated C parser | in source | build script | | |

[nom]: https://github.com/rust-bakery/nom
[winnow]: https://github.com/winnow-rs/winnow
[chumsky]: https://github.com/zesterer/chumsky
[pest]: https://github.com/pest-parser/pest
[peg]: https://github.com/kevinmehall/rust-peg
[logos]: https://github.com/maciejhirsz/logos
[lalrpop]: https://github.com/lalrpop/lalrpop
[regex]: https://github.com/rust-lang/regex
[stacker]: https://github.com/rust-lang/stacker
[combine]: https://github.com/Marwes/combine
[lexpr]: https://github.com/rotty/lexpr-rs
[pom]: https://github.com/J-F-Liu/pom
[grmtools]: https://github.com/softdevteam/grmtools
[tree-sitter]: https://github.com/tree-sitter/tree-sitter
