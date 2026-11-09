# Printer

**Trait:** `mal_traits::Printer`. **Directory:** `comparisons/printer/`.

The printer renders values back to mal source text, either *readably* (strings
quoted and escaped so the reader can read them back) or raw, for `str` and
`println`. It is a small job, which makes it a good place to compare
formatting and number-to-string crates on overhead alone.

## What the job demands

- Exact escaping of `"`, `\` and newline in readable mode.
- Sequences joined by a single space; hash-maps printed as alternating keys
  and values.
- Function values print as `#<function>`.

## Candidates

| crate | approach | step reached | version |
|-------|----------|--------------|---------|
| std   | `fmt::Write`, char-by-char escaping | stepA_mal | - |
| [itoa] | integers through `itoa::Buffer` | stepA_mal | 1.0.18 |
| [memchr] | escaping by copying runs between `memchr3` hits | stepA_mal | 2.8.3 |
| [ufmt] | integers through `ufmt::uwrite!` instead of `core::fmt` | stepA_mal | 0.2.0 |

Not yet rows: [ryu] has nothing to format because the shared value type
has no floats, and [compact_str] / [smol_str] cannot be the output buffer
while `Printer::pr_str_into` appends to a `String`; see
[Trait notes](../trait-notes.md#printer).

[itoa]: https://github.com/dtolnay/itoa
[memchr]: https://github.com/BurntSushi/memchr
[ufmt]: https://github.com/japaric/ufmt
[ryu]: https://github.com/dtolnay/ryu
[compact_str]: https://github.com/ParkMyCar/compact_str
[smol_str]: https://github.com/rust-analyzer/smol_str
