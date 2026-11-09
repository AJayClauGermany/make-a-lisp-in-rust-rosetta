//! The `std` core namespace row: every built-in of the mal guide, written
//! against `i64`, `String`, `Vec` and `std::fs`/`std::io`/`std::time`.
//!
//! Most built-ins are plain functions in a static table. The few that need
//! another component are closures: `pr-str`, `str`, `prn` and `println`
//! capture the printer row, and `read-string` constructs the reader row, so
//! swapping either component in the REPL also swaps it here.

use std::fs;
use std::io::{self, BufRead, Write};
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use mal_traits::{
    Component, CoreNamespace, Env, Implementation, Printer, ReadOutcome, Reader, Step,
};
use mal_types::{MalError, Map, MapKey, Result, Value};

/// Table entry for [`StdCore`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "std",
    component: Component::Core,
    crates: &[],
    highest_step: Step::Mal,
    notes: "checked i64 arithmetic; Vec-backed sequences; static fn table plus printer/reader closures",
};

/// The core namespace, generic over the environment it installs into, the
/// reader `read-string` uses and the printer the printing functions use.
pub struct StdCore<E, R, P> {
    printer: P,
    // `fn() -> _` keeps the marker `Send`/`Sync`-neutral and covariant.
    marker: PhantomData<fn() -> (E, R)>,
}

impl<E, R, P> StdCore<E, R, P> {
    /// A namespace whose printing functions use `printer`.
    pub fn new(printer: P) -> Self {
        Self {
            printer,
            marker: PhantomData,
        }
    }
}

impl<E, R, P: Default> Default for StdCore<E, R, P> {
    fn default() -> Self {
        Self::new(P::default())
    }
}

type NativeFn = fn(&[Value]) -> Result<Value>;

/// Built-ins that need nothing but their arguments.
const PURE: &[(&str, NativeFn)] = &[
    ("+", add),
    ("-", sub),
    ("*", mul),
    ("/", div),
    ("<", |a| compare(a, |x, y| x < y)),
    ("<=", |a| compare(a, |x, y| x <= y)),
    (">", |a| compare(a, |x, y| x > y)),
    (">=", |a| compare(a, |x, y| x >= y)),
    ("=", |a| {
        let [x, y] = arity(a)?;
        Ok(Value::Bool(x == y))
    }),
    ("throw", |a| {
        let [x] = arity(a)?;
        Err(MalError::Thrown(x.clone()))
    }),
    ("nil?", |a| is(a, |v| matches!(v, Value::Nil))),
    ("true?", |a| is(a, |v| matches!(v, Value::Bool(true)))),
    ("false?", |a| is(a, |v| matches!(v, Value::Bool(false)))),
    ("string?", |a| is(a, |v| matches!(v, Value::Str(_)))),
    ("symbol?", |a| is(a, |v| matches!(v, Value::Symbol(_)))),
    ("keyword?", |a| is(a, |v| matches!(v, Value::Keyword(_)))),
    ("number?", |a| is(a, |v| matches!(v, Value::Int(_)))),
    ("fn?", |a| is(a, Value::is_fn)),
    ("macro?", |a| {
        is(a, |v| matches!(v, Value::Closure(c) if c.is_macro))
    }),
    ("list?", |a| is(a, |v| matches!(v, Value::List(..)))),
    ("vector?", |a| is(a, |v| matches!(v, Value::Vector(..)))),
    ("map?", |a| is(a, |v| matches!(v, Value::Map(..)))),
    ("sequential?", |a| is(a, |v| v.as_seq().is_some())),
    ("atom?", |a| is(a, |v| matches!(v, Value::Atom(_)))),
    ("symbol", |a| match arity(a)? {
        [Value::Str(s)] => Ok(Value::Symbol(Rc::clone(s))),
        [other] => Err(type_error("symbol", "a string", other)),
    }),
    ("keyword", |a| match arity(a)? {
        [Value::Str(s) | Value::Keyword(s)] => Ok(Value::Keyword(Rc::clone(s))),
        [other] => Err(type_error("keyword", "a string", other)),
    }),
    ("list", |a| Ok(Value::list(a.to_vec()))),
    ("vector", |a| Ok(Value::vector(a.to_vec()))),
    ("hash-map", |a| Value::map_from_pairs(a.iter().cloned())),
    ("assoc", assoc),
    ("dissoc", dissoc),
    ("get", get),
    ("contains?", |a| {
        let [map, key] = arity(a)?;
        Ok(Value::Bool(
            as_map("contains?", map)?.contains_key(&MapKey::try_from(key)?),
        ))
    }),
    ("keys", |a| {
        let [map] = arity(a)?;
        Ok(Value::list(
            as_map("keys", map)?.keys().map(MapKey::to_value).collect(),
        ))
    }),
    ("vals", |a| {
        let [map] = arity(a)?;
        Ok(Value::list(
            as_map("vals", map)?.values().cloned().collect(),
        ))
    }),
    ("cons", |a| {
        let [head, tail] = arity(a)?;
        let tail = as_seq("cons", tail)?;
        let mut items = Vec::with_capacity(tail.len() + 1);
        items.push(head.clone());
        items.extend_from_slice(tail);
        Ok(Value::list(items))
    }),
    ("concat", |a| {
        let mut items = Vec::new();
        for seq in a {
            items.extend_from_slice(as_seq("concat", seq)?);
        }
        Ok(Value::list(items))
    }),
    ("vec", |a| match arity(a)? {
        [v @ Value::Vector(..)] => Ok(v.clone()),
        [Value::List(items, _)] => Ok(Value::Vector(Rc::clone(items), None)),
        [other] => Err(type_error("vec", "a list or vector", other)),
    }),
    ("nth", |a| {
        let [seq, index] = arity(a)?;
        let items = as_seq("nth", seq)?;
        let i = as_int("nth", index)?;
        usize::try_from(i)
            .ok()
            .and_then(|i| items.get(i))
            .cloned()
            .ok_or_else(|| MalError::msg(format!("nth: index {i} out of range")))
    }),
    ("first", |a| match arity(a)? {
        [Value::Nil] => Ok(Value::Nil),
        [seq] => Ok(as_seq("first", seq)?.first().cloned().unwrap_or_default()),
    }),
    ("rest", |a| match arity(a)? {
        [Value::Nil] => Ok(Value::list(Vec::new())),
        [seq] => Ok(Value::list(
            as_seq("rest", seq)?.get(1..).unwrap_or_default().to_vec(),
        )),
    }),
    ("empty?", |a| match arity(a)? {
        [Value::Nil] => Ok(Value::Bool(true)),
        [seq] => Ok(Value::Bool(as_seq("empty?", seq)?.is_empty())),
    }),
    ("count", |a| {
        let len = match arity(a)? {
            [Value::Nil] => 0,
            [Value::Map(map, _)] => map.len(),
            [Value::Str(s)] => s.chars().count(),
            [seq] => as_seq("count", seq)?.len(),
        };
        i64::try_from(len)
            .map(Value::Int)
            .map_err(|_| MalError::msg("count: too large"))
    }),
    ("apply", apply),
    ("map", |a| {
        let [f, seq] = arity(a)?;
        let items = as_seq("map", seq)?
            .iter()
            .map(|item| f.apply(std::slice::from_ref(item)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Value::list(items))
    }),
    ("conj", conj),
    ("seq", seq),
    ("meta", |a| {
        let [v] = arity(a)?;
        Ok(v.meta())
    }),
    ("with-meta", |a| {
        let [v, meta] = arity(a)?;
        v.with_meta(meta.clone())
    }),
    ("atom", |a| {
        let [v] = arity(a)?;
        Ok(Value::atom(v.clone()))
    }),
    ("deref", |a| match arity(a)? {
        [Value::Atom(cell)] => Ok(cell.borrow().clone()),
        [other] => Err(type_error("deref", "an atom", other)),
    }),
    ("reset!", |a| match arity(a)? {
        [Value::Atom(cell), v] => {
            cell.replace(v.clone());
            Ok(v.clone())
        }
        [other, _] => Err(type_error("reset!", "an atom", other)),
    }),
    ("swap!", swap),
    ("slurp", |a| match arity(a)? {
        [Value::Str(path)] => fs::read_to_string(&**path)
            .map(|s| Value::string(&s))
            .map_err(|e| MalError::msg(format!("slurp: {path}: {e}"))),
        [other] => Err(type_error("slurp", "a file name", other)),
    }),
    ("readline", |a| match arity(a)? {
        [Value::Str(prompt)] => readline(prompt),
        [other] => Err(type_error("readline", "a prompt string", other)),
    }),
    ("time-ms", |_| {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| MalError::msg(e.to_string()))?
            .as_millis();
        i64::try_from(ms)
            .map(Value::Int)
            .map_err(|_| MalError::msg("time-ms: clock out of range"))
    }),
];

/// Built-ins that need the printer row.
const PRINTING: &[&str] = &["pr-str", "str", "prn", "println"];
/// Built-ins that need the reader row.
const READING: &[&str] = &["read-string"];

impl<E, R, P> CoreNamespace for StdCore<E, R, P>
where
    E: Env<Value = Value>,
    R: Reader<Value = Value> + Default + 'static,
    P: Printer<Value = Value> + Clone + 'static,
{
    type Env = E;

    fn install(&self, env: &E) {
        for &(name, func) in PURE {
            env.set(name, Value::builtin(name, func));
        }
        let printer = &self.printer;
        env.set(
            "pr-str",
            print_fn("pr-str", printer.clone(), true, " ", false),
        );
        env.set("str", print_fn("str", printer.clone(), false, "", false));
        env.set("prn", print_fn("prn", printer.clone(), true, " ", true));
        env.set(
            "println",
            print_fn("println", printer.clone(), false, " ", true),
        );
        env.set(
            "read-string",
            Value::builtin("read-string", |a| match arity(a)? {
                [Value::Str(source)] => match R::default().read_str(source) {
                    Ok(ReadOutcome::Form(v)) => Ok(v),
                    Ok(ReadOutcome::Empty) => Ok(Value::Nil),
                    Err(e) => Err(MalError::msg(e.to_string())),
                },
                [other] => Err(type_error("read-string", "a string", other)),
            }),
        );
    }

    fn symbols(&self) -> Vec<&'static str> {
        PURE.iter()
            .map(|&(name, _)| name)
            .chain(PRINTING.iter().copied())
            .chain(READING.iter().copied())
            .collect()
    }
}

/// `pr-str`, `str`, `prn` and `println` differ only in these settings.
fn print_fn<P>(
    name: &'static str,
    printer: P,
    readably: bool,
    sep: &'static str,
    to_stdout: bool,
) -> Value
where
    P: Printer<Value = Value> + 'static,
{
    Value::builtin(name, move |args| {
        let text = printer.pr_seq(args, readably, sep);
        if to_stdout {
            println!("{text}");
            Ok(Value::Nil)
        } else {
            Ok(Value::string(&text))
        }
    })
}

/// Destructure `args` into exactly `N` arguments.
fn arity<const N: usize>(args: &[Value]) -> Result<&[Value; N]> {
    args.try_into().map_err(|_| {
        MalError::msg(format!(
            "wrong number of arguments: expected {N}, got {}",
            args.len()
        ))
    })
}

fn type_error(name: &str, expected: &str, got: &Value) -> MalError {
    MalError::msg(format!(
        "{name}: expected {expected}, got {}",
        got.type_name()
    ))
}

fn as_int(name: &str, v: &Value) -> Result<i64> {
    match v {
        Value::Int(i) => Ok(*i),
        other => Err(type_error(name, "an integer", other)),
    }
}

fn as_seq<'a>(name: &str, v: &'a Value) -> Result<&'a [Value]> {
    v.as_seq()
        .ok_or_else(|| type_error(name, "a list or vector", v))
}

fn as_map<'a>(name: &str, v: &'a Value) -> Result<&'a Map> {
    match v {
        Value::Map(map, _) => Ok(map),
        other => Err(type_error(name, "a hash-map", other)),
    }
}

fn is(args: &[Value], pred: impl Fn(&Value) -> bool) -> Result<Value> {
    let [v] = arity(args)?;
    Ok(Value::Bool(pred(v)))
}

fn ints<'a>(name: &'a str, args: &'a [Value]) -> impl Iterator<Item = Result<i64>> + 'a {
    args.iter().map(move |v| as_int(name, v))
}

fn overflow(name: &str) -> MalError {
    MalError::msg(format!("{name}: integer overflow"))
}

fn add(args: &[Value]) -> Result<Value> {
    ints("+", args)
        .try_fold(0i64, |acc, x| {
            acc.checked_add(x?).ok_or_else(|| overflow("+"))
        })
        .map(Value::Int)
}

fn mul(args: &[Value]) -> Result<Value> {
    ints("*", args)
        .try_fold(1i64, |acc, x| {
            acc.checked_mul(x?).ok_or_else(|| overflow("*"))
        })
        .map(Value::Int)
}

/// `(- x)` negates; `(- x y z)` subtracts left to right.
fn sub(args: &[Value]) -> Result<Value> {
    let mut nums = ints("-", args);
    let first = nums
        .next()
        .ok_or_else(|| MalError::msg("-: expected at least 1 argument"))??;
    if args.len() == 1 {
        return first
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| overflow("-"));
    }
    nums.try_fold(first, |acc, x| {
        acc.checked_sub(x?).ok_or_else(|| overflow("-"))
    })
    .map(Value::Int)
}

/// Integer division, truncating, left to right.
fn div(args: &[Value]) -> Result<Value> {
    let mut nums = ints("/", args);
    let first = nums
        .next()
        .ok_or_else(|| MalError::msg("/: expected at least 2 arguments"))??;
    nums.try_fold(first, |acc, x| {
        acc.checked_div(x?)
            .ok_or_else(|| MalError::msg("/: division by zero or overflow"))
    })
    .map(Value::Int)
}

fn compare(args: &[Value], op: fn(i64, i64) -> bool) -> Result<Value> {
    let [x, y] = arity(args)?;
    Ok(Value::Bool(op(
        as_int("comparison", x)?,
        as_int("comparison", y)?,
    )))
}

fn assoc(args: &[Value]) -> Result<Value> {
    let (map, pairs) = args
        .split_first()
        .ok_or_else(|| MalError::msg("assoc: expected a hash-map"))?;
    let mut map = as_map("assoc", map)?.clone();
    for pair in pairs.chunks(2) {
        let [key, value] = pair else {
            return Err(MalError::msg("assoc: odd number of arguments"));
        };
        map.insert(MapKey::try_from(key)?, value.clone());
    }
    Ok(Value::map(map))
}

fn dissoc(args: &[Value]) -> Result<Value> {
    let (map, keys) = args
        .split_first()
        .ok_or_else(|| MalError::msg("dissoc: expected a hash-map"))?;
    let mut map = as_map("dissoc", map)?.clone();
    for key in keys {
        map.remove(&MapKey::try_from(key)?);
    }
    Ok(Value::map(map))
}

fn get(args: &[Value]) -> Result<Value> {
    match arity(args)? {
        [Value::Nil, _] => Ok(Value::Nil),
        [map, key] => Ok(as_map("get", map)?
            .get(&MapKey::try_from(key)?)
            .cloned()
            .unwrap_or_default()),
    }
}

/// `(apply f a b [c d])` calls `(f a b c d)`.
fn apply(args: &[Value]) -> Result<Value> {
    let [f, middle @ .., last] = args else {
        return Err(MalError::msg("apply: expected a function and a list"));
    };
    let mut call_args = middle.to_vec();
    call_args.extend_from_slice(as_seq("apply", last)?);
    f.apply(&call_args)
}

/// Lists grow at the front, vectors at the back.
fn conj(args: &[Value]) -> Result<Value> {
    let (coll, items) = args
        .split_first()
        .ok_or_else(|| MalError::msg("conj: expected a collection"))?;
    match coll {
        Value::List(existing, _) => {
            let mut out: Vec<Value> = items.iter().rev().cloned().collect();
            out.extend_from_slice(existing);
            Ok(Value::list(out))
        }
        Value::Vector(existing, _) => {
            let mut out = existing.to_vec();
            out.extend_from_slice(items);
            Ok(Value::vector(out))
        }
        other => Err(type_error("conj", "a list or vector", other)),
    }
}

/// Lists, vectors and strings as a list, or `nil` when empty.
fn seq(args: &[Value]) -> Result<Value> {
    Ok(match arity(args)? {
        [Value::List(items, _) | Value::Vector(items, _)] if items.is_empty() => Value::Nil,
        [Value::List(items, _) | Value::Vector(items, _)] => Value::List(Rc::clone(items), None),
        [Value::Str(s)] if s.is_empty() => Value::Nil,
        [Value::Str(s)] => Value::list(
            s.chars()
                .map(|c| Value::string(c.encode_utf8(&mut [0; 4])))
                .collect(),
        ),
        [Value::Nil] => Value::Nil,
        [other] => return Err(type_error("seq", "a sequence or string", other)),
    })
}

/// `(swap! a f x y)` sets `a` to `(f @a x y)` and returns it.
fn swap(args: &[Value]) -> Result<Value> {
    let [Value::Atom(cell), f, extra @ ..] = args else {
        return Err(MalError::msg("swap!: expected an atom and a function"));
    };
    let mut call_args = Vec::with_capacity(extra.len() + 1);
    // Clone out of the cell first: `f` may deref the same atom.
    call_args.push(cell.borrow().clone());
    call_args.extend_from_slice(extra);
    let new = f.apply(&call_args)?;
    cell.replace(new.clone());
    Ok(new)
}

fn readline(prompt: &str) -> Result<Value> {
    let io_error = |e: io::Error| MalError::msg(format!("readline: {e}"));
    let mut stdout = io::stdout().lock();
    stdout.write_all(prompt.as_bytes()).map_err(io_error)?;
    stdout.flush().map_err(io_error)?;
    let mut line = String::new();
    if io::stdin().lock().read_line(&mut line).map_err(io_error)? == 0 {
        return Ok(Value::Nil);
    }
    Ok(Value::string(line.trim_end_matches(['\n', '\r'])))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// Enough of an environment to install into, without depending on an
    /// environment row.
    #[derive(Clone, Default)]
    struct TestEnv(Rc<RefCell<HashMap<String, Value>>>);

    impl Env for TestEnv {
        type Value = Value;
        fn child(&self) -> Self {
            self.clone()
        }
        fn set(&self, symbol: &str, value: Value) {
            self.0.borrow_mut().insert(symbol.to_owned(), value);
        }
        fn get(&self, symbol: &str) -> Option<Value> {
            self.0.borrow().get(symbol).cloned()
        }
    }

    /// Reads integers only; enough to exercise `read-string`.
    #[derive(Default)]
    struct IntReader;

    impl Reader for IntReader {
        type Value = Value;
        type Error = std::num::ParseIntError;
        fn read_str(
            &mut self,
            input: &str,
        ) -> std::result::Result<ReadOutcome<Value>, Self::Error> {
            if input.trim().is_empty() {
                return Ok(ReadOutcome::Empty);
            }
            input
                .trim()
                .parse()
                .map(|i| ReadOutcome::Form(Value::Int(i)))
        }
    }

    /// Prints integers and strings; enough to exercise the printing fns.
    #[derive(Clone, Default)]
    struct TinyPrinter;

    impl Printer for TinyPrinter {
        type Value = Value;
        fn pr_str_into(&self, value: &Value, readably: bool, out: &mut String) {
            match value {
                Value::Int(i) => out.push_str(&i.to_string()),
                Value::Str(s) if readably => {
                    out.push('"');
                    out.push_str(s);
                    out.push('"');
                }
                Value::Str(s) => out.push_str(s),
                _ => out.push('?'),
            }
        }
    }

    fn core() -> (StdCore<TestEnv, IntReader, TinyPrinter>, TestEnv) {
        let env = TestEnv::default();
        let core = StdCore::default();
        core.install(&env);
        (core, env)
    }

    fn call(env: &TestEnv, name: &str, args: &[Value]) -> Result<Value> {
        env.get(name).expect(name).apply(args)
    }

    #[test]
    fn symbols_match_what_is_installed() {
        let (core, env) = core();
        let symbols = core.symbols();
        assert_eq!(symbols.len(), env.0.borrow().len());
        for name in symbols {
            assert!(env.get(name).is_some(), "{name} not installed");
        }
    }

    #[test]
    fn arithmetic_is_checked() {
        let (_, env) = core();
        let n = |i: i64| Value::Int(i);
        assert_eq!(call(&env, "+", &[n(1), n(2), n(3)]).unwrap(), n(6));
        assert_eq!(call(&env, "-", &[n(10), n(2), n(3)]).unwrap(), n(5));
        assert_eq!(call(&env, "-", &[n(4)]).unwrap(), n(-4));
        assert_eq!(call(&env, "/", &[n(7), n(2)]).unwrap(), n(3));
        assert!(call(&env, "/", &[n(1), n(0)]).is_err());
        assert!(call(&env, "+", &[n(i64::MAX), n(1)]).is_err());
        assert_eq!(call(&env, "<=", &[n(1), n(1)]).unwrap(), Value::Bool(true));
    }

    #[test]
    fn sequences_and_maps() {
        let (_, env) = core();
        let list = Value::list(vec![1.into(), 2.into()]);
        let vector = Value::vector(vec![1.into(), 2.into()]);
        assert_eq!(
            call(&env, "conj", &[list, 3.into(), 4.into()]).unwrap(),
            Value::list(vec![4.into(), 3.into(), 1.into(), 2.into()])
        );
        let conj_vec = call(&env, "conj", &[vector.clone(), 3.into()]).unwrap();
        assert!(matches!(conj_vec, Value::Vector(..)));
        let err = call(&env, "nth", &[vector, 5.into()]).unwrap_err();
        assert!(err.to_string().contains("range"));
        let map = call(&env, "hash-map", &[Value::keyword("a"), 1.into()]).unwrap();
        let map = call(&env, "assoc", &[map, Value::string("b"), 2.into()]).unwrap();
        assert_eq!(
            call(&env, "get", &[map.clone(), Value::keyword("a")]).unwrap(),
            1.into()
        );
        let map = call(&env, "dissoc", &[map, Value::keyword("a")]).unwrap();
        assert_eq!(call(&env, "count", &[map]).unwrap(), 1.into());
        assert_eq!(
            call(&env, "seq", &[Value::string("ab")]).unwrap(),
            Value::list(vec![Value::string("a"), Value::string("b")])
        );
    }

    #[test]
    fn atoms_apply_and_components() {
        let (_, env) = core();
        let atom = call(&env, "atom", &[1.into()]).unwrap();
        let plus = env.get("+").unwrap();
        assert_eq!(
            call(&env, "swap!", &[atom.clone(), plus.clone(), 10.into()]).unwrap(),
            11.into()
        );
        assert_eq!(
            call(&env, "reset!", &[atom.clone(), 5.into()]).unwrap(),
            5.into()
        );
        assert_eq!(call(&env, "deref", &[atom]).unwrap(), 5.into());
        let args = Value::list(vec![2.into(), 3.into()]);
        assert_eq!(
            call(&env, "apply", &[plus, 1.into(), args]).unwrap(),
            6.into()
        );
        assert_eq!(
            call(&env, "read-string", &[Value::string(" 42 ")]).unwrap(),
            42.into()
        );
        assert!(call(&env, "read-string", &[Value::string("x")]).is_err());
        assert_eq!(
            call(&env, "pr-str", &[Value::string("a"), 1.into()]).unwrap(),
            Value::string("\"a\" 1")
        );
        assert_eq!(
            call(&env, "str", &[Value::string("a"), 1.into()]).unwrap(),
            Value::string("a1")
        );
        let thrown = call(&env, "throw", &[1.into()]).unwrap_err();
        assert_eq!(thrown.into_value(), 1.into());
    }
}
