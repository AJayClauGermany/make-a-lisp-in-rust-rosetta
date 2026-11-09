//! The `memchr` printer row: the `std` printer with readable strings
//! escaped by searching for the next special byte with `memchr3` and copying
//! the run before it in one `push_str`, instead of pushing one `char` at a
//! time.
//!
//! The three bytes that need escaping (`"`, `\` and newline) are ASCII, so
//! searching bytes is safe on UTF-8 and every run boundary is a character
//! boundary. Long strings with few escapes are where this should win.

use std::fmt::Write;

use mal_traits::{Component, CrateRef, Implementation, Printer, Step};
use mal_types::Value;

/// Table entry for [`MemchrPrinter`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "memchr",
    component: Component::Printer,
    crates: &[CrateRef {
        name: "memchr",
        version: "2.8.3",
        url: "https://github.com/BurntSushi/memchr",
    }],
    highest_step: Step::Mal,
    notes: "std printer; readable strings escaped by copying runs between memchr3 hits",
};

/// Prints values; see the crate documentation for what differs from `std`.
#[derive(Debug, Default, Clone)]
pub struct MemchrPrinter;

impl Printer for MemchrPrinter {
    type Value = Value;

    fn pr_str_into(&self, value: &Value, print_readably: bool, out: &mut String) {
        match value {
            Value::Nil => out.push_str("nil"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            // Writing to a String cannot fail.
            Value::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Str(s) if print_readably => {
                out.push('"');
                escape_into(s, out);
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

impl MemchrPrinter {
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

/// Append `s` with `"`, `\\` and newline escaped.
fn escape_into(s: &str, out: &mut String) {
    let bytes = s.as_bytes();
    let mut start = 0;
    for i in memchr::memchr3_iter(b'"', b'\\', b'\n', bytes) {
        out.push_str(&s[start..i]);
        out.push_str(match bytes[i] {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            _ => "\\n",
        });
        start = i + 1;
    }
    out.push_str(&s[start..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_printer(&MemchrPrinter);
    }
}
