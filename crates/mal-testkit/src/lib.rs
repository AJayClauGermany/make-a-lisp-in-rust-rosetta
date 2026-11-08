//! Shared unit-test cases for comparison rows.
//!
//! Every row runs the checks for its component from its own `#[cfg(test)]`
//! module, so a row is held to the same expectations as every other row of
//! that component before the slower end-to-end mal test suite runs:
//!
//! ```ignore
//! #[test]
//! fn conforms() {
//!     mal_testkit::check_reader(NomReader::default);
//! }
//! ```
//!
//! The cases follow `step1_read_print.mal` of the mal test suite, expressed
//! as values rather than printed text so a reader can be checked without a
//! printer and the other way round. This is a dev-dependency only.

use mal_traits::{Env, Printer, ReadOutcome, Reader};
use mal_types::Value;

fn sym(name: &str) -> Value {
    Value::symbol(name)
}

fn int(i: i64) -> Value {
    Value::Int(i)
}

fn list(items: impl Into<Vec<Value>>) -> Value {
    Value::list(items.into())
}

fn vector(items: impl Into<Vec<Value>>) -> Value {
    Value::vector(items.into())
}

fn map(items: impl Into<Vec<Value>>) -> Value {
    Value::map_from_pairs(items.into()).expect("test map is well formed")
}

/// Inputs and the value each must read as.
#[allow(clippy::too_many_lines)]
fn reader_cases() -> Vec<(&'static str, Value)> {
    vec![
        // Numbers.
        ("1", int(1)),
        ("7", int(7)),
        ("  7   ", int(7)),
        ("-123", int(-123)),
        ("9223372036854775807", int(i64::MAX)),
        // Symbols, including ones that start like numbers.
        ("+", sym("+")),
        ("abc", sym("abc")),
        ("   abc   ", sym("abc")),
        ("abc5", sym("abc5")),
        ("abc-def", sym("abc-def")),
        ("-", sym("-")),
        ("-abc", sym("-abc")),
        ("->>", sym("->>")),
        ("abc:def", sym("abc:def")),
        ("swap-and-reset!", sym("swap-and-reset!")),
        // Lists.
        ("(+ 1 2)", list([sym("+"), int(1), int(2)])),
        ("()", list([])),
        ("( )", list([])),
        ("(nil)", list([Value::Nil])),
        ("((3 4))", list([list([int(3), int(4)])])),
        (
            "  ( +   1   (+   2 3   )   )  ",
            list([sym("+"), int(1), list([sym("+"), int(2), int(3)])]),
        ),
        ("(()())", list([list([]), list([])])),
        ("(1 2, 3,,,,),,", list([int(1), int(2), int(3)])),
        ("(** 1 2)", list([sym("**"), int(1), int(2)])),
        ("(* -3 6)", list([sym("*"), int(-3), int(6)])),
        // Literals.
        ("nil", Value::Nil),
        ("true", Value::Bool(true)),
        ("false", Value::Bool(false)),
        ("nilly", sym("nilly")),
        // Strings and escapes.
        (r#""abc""#, Value::string("abc")),
        (r#"   "abc"   "#, Value::string("abc")),
        (r#""abc (with parens)""#, Value::string("abc (with parens)")),
        (r#""abc\"def""#, Value::string("abc\"def")),
        (r#""""#, Value::string("")),
        (r#""\\""#, Value::string("\\")),
        (r#""\n""#, Value::string("\n")),
        (r#""abc\\def""#, Value::string("abc\\def")),
        (r#""&""#, Value::string("&")),
        (r#""'""#, Value::string("'")),
        (r#""(""#, Value::string("(")),
        (r#"";""#, Value::string(";")),
        (r#""ä→😀""#, Value::string("ä→😀")),
        // Reader macros.
        ("'1", Value::wrap("quote", int(1))),
        (
            "'(1 2 3)",
            Value::wrap("quote", list([int(1), int(2), int(3)])),
        ),
        ("`1", Value::wrap("quasiquote", int(1))),
        ("~1", Value::wrap("unquote", int(1))),
        (
            "`(1 ~a 3)",
            Value::wrap(
                "quasiquote",
                list([int(1), Value::wrap("unquote", sym("a")), int(3)]),
            ),
        ),
        (
            "~@(1 2 3)",
            Value::wrap("splice-unquote", list([int(1), int(2), int(3)])),
        ),
        ("@a", Value::wrap("deref", sym("a"))),
        (
            r#"^{"a" 1} [1 2 3]"#,
            list([
                sym("with-meta"),
                vector([int(1), int(2), int(3)]),
                map([Value::string("a"), int(1)]),
            ]),
        ),
        // Keywords.
        (":kw", Value::keyword("kw")),
        (
            "(:kw1 :kw2 :kw3)",
            list([
                Value::keyword("kw1"),
                Value::keyword("kw2"),
                Value::keyword("kw3"),
            ]),
        ),
        // Vectors.
        ("[+ 1 2]", vector([sym("+"), int(1), int(2)])),
        ("[]", vector([])),
        ("[ ]", vector([])),
        ("[[3 4]]", vector([vector([int(3), int(4)])])),
        ("([])", list([vector([])])),
        // Hash-maps.
        ("{}", map([])),
        ("{ }", map([])),
        (r#"{"abc" 1}"#, map([Value::string("abc"), int(1)])),
        (
            r#"{"a" {"b" 2}}"#,
            map([Value::string("a"), map([Value::string("b"), int(2)])]),
        ),
        (
            "{:a {:b {:cde 3}}}",
            map([
                Value::keyword("a"),
                map([Value::keyword("b"), map([Value::keyword("cde"), int(3)])]),
            ]),
        ),
        (
            r#"{"a1" 1 :a2 2 "a3" 3}"#,
            map([
                Value::string("a1"),
                int(1),
                Value::keyword("a2"),
                int(2),
                Value::string("a3"),
                int(3),
            ]),
        ),
        // Comments and trailing input.
        ("1 ; comment after expression", int(1)),
        ("1; comment after expression", int(1)),
        ("; leading comment\n2", int(2)),
        ("(1 ; inner comment\n 2)", list([int(1), int(2)])),
        ("1 2 3", int(1)),
        ("abc)", sym("abc")),
    ]
}

/// Inputs that contain no form.
const EMPTY_INPUTS: &[&str] = &[
    "",
    "   ",
    ",,,",
    "; just a comment",
    "  ;; comment\n  ; another",
];

/// Inputs that must be reported as errors mentioning EOF or unbalanced
/// input, as `step1_read_print.mal` checks with
/// `.*(EOF|end of input|unbalanced).*`.
const UNBALANCED_INPUTS: &[&str] = &[
    "(1 2",
    "[1 2",
    r#"{"a" 2"#,
    r#""abc"#,
    r#"""#,
    r#""\""#,
    r#""\\\\\\\\\\\\\\\\\\\""#,
    r#"(1 "abc"#,
    r#"(1 "abc""#,
    "(",
    "'",
    "(1 (2 3)",
];

/// Check a [`Reader`] against the shared cases. `new` is called for every
/// case so a reader that keeps state between calls is also exercised fresh.
///
/// # Panics
///
/// On the first case the reader gets wrong, naming the input.
pub fn check_reader<R, F>(mut new: F)
where
    R: Reader<Value = Value>,
    F: FnMut() -> R,
{
    let mut reused = new();
    for (input, expected) in reader_cases() {
        for reader in [&mut new(), &mut reused] {
            match reader.read_str(input) {
                Ok(ReadOutcome::Form(value)) => {
                    assert!(
                        value == expected && same_shape(&value, &expected),
                        "reading {input:?}: got {value:?}, expected {expected:?}"
                    );
                }
                Ok(ReadOutcome::Empty) => panic!("reading {input:?}: got Empty"),
                Err(e) => panic!("reading {input:?}: unexpected error {e}"),
            }
        }
    }
    for input in EMPTY_INPUTS {
        match new().read_str(input) {
            Ok(ReadOutcome::Empty) => {}
            Ok(ReadOutcome::Form(v)) => panic!("reading {input:?}: expected Empty, got {v:?}"),
            Err(e) => panic!("reading {input:?}: expected Empty, got error {e}"),
        }
    }
    for input in UNBALANCED_INPUTS {
        match new().read_str(input) {
            Err(e) => {
                let message = e.to_string();
                assert!(
                    ["EOF", "end of input", "unbalanced"]
                        .iter()
                        .any(|m| message.contains(m)),
                    "reading {input:?}: error {message:?} mentions neither EOF nor unbalanced"
                );
            }
            Ok(outcome) => panic!("reading {input:?}: expected an error, got {outcome:?}"),
        }
    }
}

/// Mal equality treats lists and vectors as equal; the reader must not.
fn same_shape(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::List(x, _), Value::List(y, _)) | (Value::Vector(x, _), Value::Vector(y, _)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| same_shape(a, b))
        }
        (Value::List(..), Value::Vector(..)) | (Value::Vector(..), Value::List(..)) => false,
        (Value::Map(x, _), Value::Map(y, _)) => {
            x.values().zip(y.values()).all(|(a, b)| same_shape(a, b))
        }
        _ => true,
    }
}

/// Values, how to print them, and the expected text.
fn printer_cases() -> Vec<(Value, bool, &'static str)> {
    let quoted = Value::string("a \"b\"\\c\nd");
    vec![
        (Value::Nil, true, "nil"),
        (Value::Bool(true), true, "true"),
        (Value::Bool(false), true, "false"),
        (int(0), true, "0"),
        (int(-123), true, "-123"),
        (int(i64::MIN), true, "-9223372036854775808"),
        (int(i64::MAX), true, "9223372036854775807"),
        (sym("abc"), true, "abc"),
        (Value::keyword("kw"), true, ":kw"),
        (Value::string("abc"), true, r#""abc""#),
        (quoted.clone(), true, r#""a \"b\"\\c\nd""#),
        (quoted, false, "a \"b\"\\c\nd"),
        (Value::string(""), true, r#""""#),
        (list([]), true, "()"),
        (vector([]), true, "[]"),
        (map([]), true, "{}"),
        (
            list([sym("+"), int(1), list([sym("*"), int(2), int(3)])]),
            true,
            "(+ 1 (* 2 3))",
        ),
        (
            vector([Value::string("a"), Value::Nil, Value::keyword("b")]),
            true,
            r#"["a" nil :b]"#,
        ),
        (
            vector([Value::string("a"), Value::Nil, Value::keyword("b")]),
            false,
            "[a nil :b]",
        ),
        (
            map([
                Value::keyword("b"),
                int(2),
                Value::string("a"),
                list([int(1)]),
            ]),
            true,
            r#"{"a" (1) :b 2}"#,
        ),
        (Value::atom(int(7)), true, "(atom 7)"),
        (Value::atom(Value::string("s")), true, r#"(atom "s")"#),
        (Value::builtin("f", |_| Ok(Value::Nil)), true, "#<function>"),
        (
            list([Value::string("x\ny")]).with_meta(int(1)).unwrap(),
            true,
            r#"("x\ny")"#,
        ),
    ]
}

/// Check a [`Printer`] against the shared cases, both through
/// [`Printer::pr_str`] and appending to a non-empty buffer.
///
/// # Panics
///
/// On the first case the printer gets wrong.
pub fn check_printer<P: Printer<Value = Value>>(printer: &P) {
    for (value, readably, expected) in printer_cases() {
        assert_eq!(
            printer.pr_str(&value, readably),
            expected,
            "printing {value:?} (readably: {readably})"
        );
        let mut out = String::from(">");
        printer.pr_str_into(&value, readably, &mut out);
        assert_eq!(out, format!(">{expected}"), "appending {value:?}");
    }
    let values = [int(1), Value::string("two"), list([int(3)])];
    assert_eq!(printer.pr_seq(&values, true, " "), r#"1 "two" (3)"#);
    assert_eq!(printer.pr_seq(&values, false, ""), "1two(3)");
}

/// Check an [`Env`]: binding, shadowing, lookup through outer scopes, and
/// that clones share storage. `new` builds an empty top-level environment.
///
/// # Panics
///
/// On the first expectation the environment breaks.
pub fn check_env<E, F>(mut new: F)
where
    E: Env<Value = Value>,
    F: FnMut() -> E,
{
    let root = new();
    assert_eq!(root.get("a"), None);
    assert!(!root.contains("a"));
    root.set("a", int(1));
    root.set("b", int(2));
    assert_eq!(root.get("a"), Some(int(1)));
    assert!(root.contains("b"));

    // Rebinding replaces.
    root.set("a", int(10));
    assert_eq!(root.get("a"), Some(int(10)));

    // Children see outer bindings, shadow without touching the parent.
    let child = root.child();
    assert_eq!(child.get("a"), Some(int(10)));
    child.set("a", int(100));
    child.set("c", int(3));
    assert_eq!(child.get("a"), Some(int(100)));
    assert_eq!(root.get("a"), Some(int(10)));
    assert_eq!(root.get("c"), None);

    // Lookups walk every outer scope.
    let mut deep = child.clone();
    for _ in 0..16 {
        deep = deep.child();
    }
    assert_eq!(deep.get("b"), Some(int(2)));
    assert_eq!(deep.get("c"), Some(int(3)));
    assert_eq!(deep.get("missing"), None);

    // Clones are handles: a binding made through one is seen through the
    // other and by children created before it. Closures rely on this for
    // recursion (`def!` after `fn*` captured the environment).
    let handle = root.clone();
    handle.set("late", Value::string("x"));
    assert_eq!(root.get("late"), Some(Value::string("x")));
    assert_eq!(deep.get("late"), Some(Value::string("x")));

    // Fresh environments are independent.
    assert_eq!(new().get("a"), None);
}
