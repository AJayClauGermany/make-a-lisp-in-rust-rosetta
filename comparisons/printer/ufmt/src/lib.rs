//! The `ufmt` printer row: the `std` printer with integers written by
//! `ufmt::uwrite!` instead of `core::fmt`'s `write!`.
//!
//! `ufmt` is a smaller, panic-free formatting framework aimed at embedded
//! targets; with its `std` feature `String` implements `uWrite`. As with
//! `itoa`, integers are the only values where the formatting machinery
//! matters, so this row isolates the cost of `core::fmt` against an
//! alternative framework rather than a single-purpose formatter.

use mal_traits::{Component, CrateRef, Implementation, Printer, Step};
use mal_types::Value;

/// Table entry for [`UfmtPrinter`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "ufmt",
    component: Component::Printer,
    crates: &[CrateRef {
        name: "ufmt",
        version: "0.2.0",
        url: "https://github.com/japaric/ufmt",
    }],
    highest_step: Step::Mal,
    notes: "std printer with integers through ufmt::uwrite! (no core::fmt)",
};

/// Prints values; see the crate documentation for what differs from `std`.
#[derive(Debug, Default, Clone)]
pub struct UfmtPrinter;

impl Printer for UfmtPrinter {
    type Value = Value;

    fn pr_str_into(&self, value: &Value, print_readably: bool, out: &mut String) {
        match value {
            Value::Nil => out.push_str("nil"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            // `String`'s `uWrite` error type is `Infallible`.
            Value::Int(i) => match ufmt::uwrite!(out, "{}", *i) {
                Ok(()) => {}
            },
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

impl UfmtPrinter {
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
        mal_testkit::check_printer(&UfmtPrinter);
    }
}
