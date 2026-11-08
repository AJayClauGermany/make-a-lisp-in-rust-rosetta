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
| std   | `fmt::Write` | - | - |
| [itoa] | integer formatting | | |
| [ryu] | float formatting | | |
| [compact_str] / [smol_str] | small-string output buffers | | |

[itoa]: https://github.com/dtolnay/itoa
[ryu]: https://github.com/dtolnay/ryu
[compact_str]: https://github.com/ParkMyCar/compact_str
[smol_str]: https://github.com/rust-analyzer/smol_str
