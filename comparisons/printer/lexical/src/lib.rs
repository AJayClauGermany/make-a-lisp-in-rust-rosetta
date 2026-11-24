//! The `lexical` printer row: the `std` printer with integers formatted by
//! `lexical-write-integer`, the integer half of `lexical-core`.
//!
//! Like `itoa`, it skips `core::fmt` and formats into a stack buffer, but it
//! writes bytes (`&mut [u8]`) rather than a `&str`, so the digits are checked
//! as UTF-8 before they can be appended to the `String` the `Printer` trait
//! writes into. The two rows together show what that check costs; everything
//! else is identical to the `std` row.

use lexical_write_integer::{FormattedSize, ToLexical};
use mal_traits::{Component, CrateRef, Implementation, Printer, Step};
use mal_types::Value;

/// Table entry for [`LexicalPrinter`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "lexical",
    component: Component::Printer,
    crates: &[CrateRef {
        name: "lexical-write-integer",
        version: "1.0.6",
        url: "https://github.com/Alexhuszagh/rust-lexical",
    }],
    highest_step: Step::Mal,
    notes: "std printer with integers through lexical's ToLexical (bytes, UTF-8 checked)",
};

/// Prints values; see the crate documentation for what differs from `std`.
#[derive(Debug, Default, Clone)]
pub struct LexicalPrinter;

impl Printer for LexicalPrinter {
    type Value = Value;

    fn pr_str_into(&self, value: &Value, print_readably: bool, out: &mut String) {
        match value {
            Value::Nil => out.push_str("nil"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Int(i) => {
                let mut buffer = [0u8; i64::FORMATTED_SIZE_DECIMAL];
                let digits = i.to_lexical(&mut buffer);
                // Decimal digits and a sign are always ASCII.
                out.push_str(std::str::from_utf8(digits).expect("ASCII digits"));
            }
            Value::Str(s) if print_readably => {
                out.push('"');
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Value::Str(s) | Value::Symbol(s) => out.push_str(s),
            Value::Keyword(k) => {
                out.push(':');
                out.push_str(k);
            }
            Value::List(items, _) => self.seq(items, print_readably, '(', ')', out),
            Value::Vector(items, _) => self.seq(items, print_readably, '[', ']', out),
            Value::Map(map, _) => {
                out.push('{');
                for (i, (key, value)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.pr_str_into(&key.to_value(), print_readably, out);
                    out.push(' ');
                    self.pr_str_into(value, print_readably, out);
                }
                out.push('}');
            }
            Value::Builtin(_) | Value::Closure(_) => out.push_str("#<function>"),
            Value::Atom(cell) => {
                out.push_str("(atom ");
                self.pr_str_into(&cell.borrow(), print_readably, out);
                out.push(')');
            }
        }
    }
}

impl LexicalPrinter {
    fn seq(
        &self,
        items: &[Value],
        print_readably: bool,
        open: char,
        close: char,
        out: &mut String,
    ) {
        out.push(open);
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            self.pr_str_into(item, print_readably, out);
        }
        out.push(close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_printer(&LexicalPrinter);
    }
}
