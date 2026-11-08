//! The `std` printer row: `std::fmt::Write` into the caller's `String`.
//!
//! The baseline for the printer comparison. Integers go through `Display`,
//! strings are escaped one character at a time, and collections recurse.

use std::fmt::Write;

use mal_traits::{Component, Implementation, Printer, Step};
use mal_types::Value;

/// Table entry for [`StdPrinter`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "std",
    component: Component::Printer,
    crates: &[],
    highest_step: Step::Mal,
    notes: "write! for integers, char-by-char escaping, recursion for collections",
};

/// Prints values with `std::fmt` only.
#[derive(Debug, Default, Clone)]
pub struct StdPrinter;

impl Printer for StdPrinter {
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

impl StdPrinter {
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
        mal_testkit::check_printer(&StdPrinter);
    }
}
