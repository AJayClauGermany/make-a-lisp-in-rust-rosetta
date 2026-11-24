//! The `pretty` printer row: values become a Wadler-style document
//! (`pretty::RcDoc`) that is laid out for a line width and then rendered.
//!
//! This is how a Lisp printer that wraps long forms is usually written:
//! sequences are groups whose separators are soft line breaks, nested
//! groups are indented, and the renderer chooses where to break. The mal
//! test suite expects every value on one line, and the `Printer` trait has
//! no notion of a line width, so the row is constructed with an unbounded
//! width ([`PrettyPrinter::default`]) for the interpreter and the tests, and
//! with a real width ([`PrettyPrinter::with_width`]) only by callers that
//! know they want wrapping. Either way every value is built as a document
//! first, which is the overhead this row measures against the `std` row's
//! direct writes.

use mal_traits::{Component, CrateRef, Implementation, Printer, Step};
use mal_types::Value;
use pretty::RcDoc;

/// Table entry for [`PrettyPrinter`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "pretty",
    component: Component::Printer,
    crates: &[CrateRef {
        name: "pretty",
        version: "0.12.5",
        url: "https://github.com/Marwes/pretty.rs",
    }],
    highest_step: Step::Mal,
    notes: "Wadler-style RcDoc with soft line breaks, rendered at an unbounded width (the trait has none)",
};

/// The width used when nothing asks for wrapping: wide enough that no group
/// ever breaks, small enough that the renderer's column arithmetic cannot
/// overflow.
const UNBOUNDED: usize = u32::MAX as usize;

/// Prints values through a `pretty` document.
#[derive(Debug, Clone)]
pub struct PrettyPrinter {
    width: usize,
}

impl Default for PrettyPrinter {
    /// Never wraps, as the mal test suite expects.
    fn default() -> Self {
        Self { width: UNBOUNDED }
    }
}

impl PrettyPrinter {
    /// A printer that breaks sequences that do not fit in `width` columns.
    pub fn with_width(width: usize) -> Self {
        Self { width }
    }
}

impl Printer for PrettyPrinter {
    type Value = Value;

    fn pr_str_into(&self, value: &Value, print_readably: bool, out: &mut String) {
        doc(value, print_readably)
            .render_fmt(self.width, out)
            .expect("writing to a String cannot fail");
    }
}

fn doc(value: &Value, print_readably: bool) -> RcDoc<'_> {
    match value {
        Value::Nil => RcDoc::text("nil"),
        Value::Bool(b) => RcDoc::text(if *b { "true" } else { "false" }),
        Value::Int(i) => RcDoc::as_string(i),
        Value::Str(s) if print_readably => RcDoc::text(escape(s)),
        Value::Str(s) | Value::Symbol(s) => RcDoc::text(&**s),
        Value::Keyword(k) => RcDoc::text(":").append(RcDoc::text(&**k)),
        Value::List(items, _) => seq("(", ")", items.iter().map(|v| doc(v, print_readably))),
        Value::Vector(items, _) => seq("[", "]", items.iter().map(|v| doc(v, print_readably))),
        Value::Map(map, _) => seq(
            "{",
            "}",
            map.iter().map(|(key, value)| {
                doc_owned(&key.to_value(), print_readably)
                    .append(RcDoc::line())
                    .append(doc(value, print_readably))
            }),
        ),
        Value::Builtin(_) | Value::Closure(_) => RcDoc::text("#<function>"),
        Value::Atom(cell) => RcDoc::text("(atom ")
            .append(doc_owned(&cell.borrow(), print_readably))
            .append(RcDoc::text(")")),
    }
}

/// A document for a value the caller only holds temporarily (a map key
/// turned back into a value, an atom's contents behind a `RefCell` guard):
/// rendered to text first, since the document cannot borrow it.
fn doc_owned<'a>(value: &Value, print_readably: bool) -> RcDoc<'a> {
    let mut text = String::new();
    doc(value, print_readably)
        .render_fmt(UNBOUNDED, &mut text)
        .expect("writing to a String cannot fail");
    RcDoc::text(text)
}

fn seq<'a>(
    open: &'static str,
    close: &'static str,
    items: impl Iterator<Item = RcDoc<'a>>,
) -> RcDoc<'a> {
    RcDoc::text(open)
        .append(RcDoc::intersperse(items, RcDoc::line()).nest(1).group())
        .append(RcDoc::text(close))
}

fn escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 2);
    escaped.push('"');
    for c in s.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conforms() {
        mal_testkit::check_printer(&PrettyPrinter::default());
    }

    #[test]
    fn a_real_width_wraps() {
        let form = Value::list(
            ["define", "a-rather-long-name", "another-long-name"]
                .into_iter()
                .map(Value::symbol)
                .collect(),
        );
        assert_eq!(
            PrettyPrinter::with_width(20).pr_str(&form, true),
            "(define\n a-rather-long-name\n another-long-name)"
        );
        assert_eq!(
            PrettyPrinter::default().pr_str(&form, true),
            "(define a-rather-long-name another-long-name)"
        );
    }
}
