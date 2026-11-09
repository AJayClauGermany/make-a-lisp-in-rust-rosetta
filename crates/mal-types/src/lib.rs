//! The shared value representation for the Make-a-Lisp rosetta comparison.
//!
//! The component traits in `mal-traits` are generic over the value type, so
//! that value representation is itself something a row can vary. In
//! practice a reader, a printer, an environment and a core namespace can
//! only be swapped for one another if they agree on a value type, and the
//! fixtures every row of a component is benchmarked on have to be the same
//! values. This crate is that agreement: one reference-counted
//! [`Value`] enum, the [`MalError`] the evaluator and the built-ins raise,
//! and the handful of operations on values (equality, truthiness, function
//! application) that every component needs and none of them owns.
//!
//! Like `mal-traits` it has **no dependencies**, so depending on it does not
//! change the size or build-time columns of a row. Evaluators that compare a
//! different memory-management strategy (`Arc`, arenas, tracing GC) cannot
//! use it and have to bring their own value type; see the trait notes in the
//! documentation.

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// Optional metadata attached with `with-meta` (step A).
pub type Meta = Option<Rc<Value>>;

/// The keys a mal hash-map can have: strings and keywords.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MapKey {
    /// A string key, `"a"`.
    Str(Rc<str>),
    /// A keyword key, `:a`, stored without the colon.
    Keyword(Rc<str>),
}

impl MapKey {
    /// The key as a value again, for `keys` and printing.
    pub fn to_value(&self) -> Value {
        match self {
            MapKey::Str(s) => Value::Str(Rc::clone(s)),
            MapKey::Keyword(k) => Value::Keyword(Rc::clone(k)),
        }
    }
}

impl TryFrom<&Value> for MapKey {
    type Error = MalError;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Str(s) => Ok(MapKey::Str(Rc::clone(s))),
            Value::Keyword(k) => Ok(MapKey::Keyword(Rc::clone(k))),
            other => Err(MalError::msg(format!(
                "hash-map keys must be strings or keywords, not {}",
                other.type_name()
            ))),
        }
    }
}

/// The map type behind [`Value::Map`]. Ordered, so printing is
/// deterministic and printer rows can be compared on identical output.
pub type Map = BTreeMap<MapKey, Value>;

/// The signature of a built-in function.
pub type NativeFn = dyn Fn(&[Value]) -> Result<Value>;

/// A function implemented in Rust: the core namespace and `eval`.
pub struct Builtin {
    /// The name it was installed under, for error messages.
    pub name: &'static str,
    /// The implementation. Shared, so `with-meta` can copy the builtin.
    pub func: Rc<NativeFn>,
    /// Metadata attached with `with-meta`.
    pub meta: Meta,
}

impl Builtin {
    /// Wrap `func` as a builtin called `name`.
    pub fn new(name: &'static str, func: impl Fn(&[Value]) -> Result<Value> + 'static) -> Self {
        Self {
            name,
            func: Rc::new(func),
            meta: None,
        }
    }
}

impl fmt::Debug for Builtin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#<builtin {}>", self.name)
    }
}

/// How an evaluator applies one of its closures from outside the
/// evaluator, for `apply`, `map` and `swap!` in the core namespace.
pub type ApplyFn = fn(&Closure, &[Value]) -> Result<Value>;

/// A function or macro defined in mal with `fn*` or `defmacro!`.
///
/// The environment a closure captures belongs to the evaluator, whose
/// environment type is a generic parameter, while this value type is shared
/// by every row. The environment is therefore stored type-erased and the
/// evaluator that created the closure downcasts it again; [`Closure::apply`]
/// is how code that does not know the environment type (the core namespace)
/// calls it.
#[derive(Clone)]
pub struct Closure {
    /// Positional parameter names.
    pub params: Rc<[Rc<str>]>,
    /// The parameter after `&`, bound to the remaining arguments.
    pub rest: Option<Rc<str>>,
    /// The body, evaluated in a child of [`env`](Self::env).
    pub body: Value,
    /// The captured environment, type-erased (see the type docs).
    pub env: Rc<dyn Any>,
    /// Applies this closure; set by the evaluator that created it.
    pub apply: ApplyFn,
    /// Whether `defmacro!` turned this into a macro.
    pub is_macro: bool,
    /// Metadata attached with `with-meta`.
    pub meta: Meta,
}

impl Closure {
    /// Call the closure with `args`, through its evaluator.
    pub fn call(&self, args: &[Value]) -> Result<Value> {
        (self.apply)(self, args)
    }
}

impl fmt::Debug for Closure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closure")
            .field("params", &self.params)
            .field("rest", &self.rest)
            .field("body", &self.body)
            .field("is_macro", &self.is_macro)
            .finish_non_exhaustive()
    }
}

/// A mal value.
///
/// Every heap-allocated variant is behind an [`Rc`], so cloning a value is
/// cheap and values can be shared freely between environments, closures and
/// collections. Collections are immutable; the core functions that "change"
/// them build new ones.
#[derive(Debug, Clone, Default)]
pub enum Value {
    /// `nil`.
    #[default]
    Nil,
    /// `true` or `false`.
    Bool(bool),
    /// An integer. The mal test suite only needs 64-bit integers.
    Int(i64),
    /// A string.
    Str(Rc<str>),
    /// A symbol.
    Symbol(Rc<str>),
    /// A keyword, stored without its leading colon.
    Keyword(Rc<str>),
    /// A list, `(...)`.
    List(Rc<Vec<Value>>, Meta),
    /// A vector, `[...]`.
    Vector(Rc<Vec<Value>>, Meta),
    /// A hash-map, `{...}`.
    Map(Rc<Map>, Meta),
    /// A built-in function.
    Builtin(Rc<Builtin>),
    /// A function or macro defined in mal.
    Closure(Rc<Closure>),
    /// A mutable reference cell, `(atom ...)`.
    Atom(Rc<RefCell<Value>>),
}

impl Value {
    /// A symbol.
    pub fn symbol(name: &str) -> Self {
        Value::Symbol(name.into())
    }

    /// A keyword; `name` is given without the colon.
    pub fn keyword(name: &str) -> Self {
        Value::Keyword(name.into())
    }

    /// A string.
    pub fn string(s: &str) -> Self {
        Value::Str(s.into())
    }

    /// A list without metadata.
    pub fn list(items: Vec<Value>) -> Self {
        Value::List(Rc::new(items), None)
    }

    /// A vector without metadata.
    pub fn vector(items: Vec<Value>) -> Self {
        Value::Vector(Rc::new(items), None)
    }

    /// A hash-map without metadata.
    pub fn map(map: Map) -> Self {
        Value::Map(Rc::new(map), None)
    }

    /// A hash-map from alternating keys and values, as the reader and
    /// `hash-map` produce. Later keys win, as in the reference
    /// implementations.
    pub fn map_from_pairs(items: impl IntoIterator<Item = Value>) -> Result<Self> {
        let mut map = Map::new();
        let mut items = items.into_iter();
        while let Some(key) = items.next() {
            let value = items
                .next()
                .ok_or_else(|| MalError::msg("odd number of forms in hash-map"))?;
            map.insert(MapKey::try_from(&key)?, value);
        }
        Ok(Value::map(map))
    }

    /// The two-element list `(name form)`, which is what the reader macros
    /// expand to: `'x` is `(quote x)`, `@a` is `(deref a)`.
    pub fn wrap(name: &str, form: Value) -> Self {
        Value::list(vec![Value::symbol(name), form])
    }

    /// A builtin function value.
    pub fn builtin(name: &'static str, func: impl Fn(&[Value]) -> Result<Value> + 'static) -> Self {
        Value::Builtin(Rc::new(Builtin::new(name, func)))
    }

    /// An atom holding `value`.
    pub fn atom(value: Value) -> Self {
        Value::Atom(Rc::new(RefCell::new(value)))
    }

    /// Everything except `nil` and `false` is true.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    /// The elements of a list or vector.
    pub fn as_seq(&self) -> Option<&[Value]> {
        match self {
            Value::List(items, _) | Value::Vector(items, _) => Some(items),
            _ => None,
        }
    }

    /// Whether this is a function a program can call: a builtin or a
    /// closure that is not a macro.
    pub fn is_fn(&self) -> bool {
        match self {
            Value::Builtin(_) => true,
            Value::Closure(c) => !c.is_macro,
            _ => false,
        }
    }

    /// Call this value as a function.
    pub fn apply(&self, args: &[Value]) -> Result<Value> {
        match self {
            Value::Builtin(b) => (b.func)(args),
            Value::Closure(c) => c.call(args),
            other => Err(MalError::msg(format!(
                "cannot call {}: not a function",
                other.type_name()
            ))),
        }
    }

    /// The metadata attached to this value, or `nil`.
    #[must_use]
    pub fn meta(&self) -> Value {
        let meta = match self {
            Value::List(_, m) | Value::Vector(_, m) | Value::Map(_, m) => m,
            Value::Builtin(b) => &b.meta,
            Value::Closure(c) => &c.meta,
            _ => return Value::Nil,
        };
        meta.as_deref().cloned().unwrap_or_default()
    }

    /// A copy of this value with `meta` attached.
    pub fn with_meta(&self, meta: Value) -> Result<Value> {
        let meta = Some(Rc::new(meta));
        Ok(match self {
            Value::List(items, _) => Value::List(Rc::clone(items), meta),
            Value::Vector(items, _) => Value::Vector(Rc::clone(items), meta),
            Value::Map(map, _) => Value::Map(Rc::clone(map), meta),
            Value::Builtin(b) => Value::Builtin(Rc::new(Builtin {
                name: b.name,
                func: Rc::clone(&b.func),
                meta,
            })),
            Value::Closure(c) => Value::Closure(Rc::new(Closure {
                meta,
                ..Closure::clone(c)
            })),
            other => {
                return Err(MalError::msg(format!(
                    "cannot attach metadata to {}",
                    other.type_name()
                )));
            }
        })
    }

    /// The name of this value's type, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "boolean",
            Value::Int(_) => "integer",
            Value::Str(_) => "string",
            Value::Symbol(_) => "symbol",
            Value::Keyword(_) => "keyword",
            Value::List(..) => "list",
            Value::Vector(..) => "vector",
            Value::Map(..) => "hash-map",
            Value::Builtin(_) => "builtin function",
            Value::Closure(c) if c.is_macro => "macro",
            Value::Closure(_) => "function",
            Value::Atom(_) => "atom",
        }
    }
}

/// Mal equality, as `=` defines it: lists and vectors with equal elements
/// are equal, metadata is ignored, and functions and atoms are equal only
/// to themselves.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Str(a), Value::Str(b))
            | (Value::Symbol(a), Value::Symbol(b))
            | (Value::Keyword(a), Value::Keyword(b)) => a == b,
            (Value::List(a, _) | Value::Vector(a, _), Value::List(b, _) | Value::Vector(b, _)) => {
                a == b
            }
            (Value::Map(a, _), Value::Map(b, _)) => a == b,
            (Value::Builtin(a), Value::Builtin(b)) => Rc::ptr_eq(&a.func, &b.func),
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            (Value::Atom(a), Value::Atom(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

/// An error raised while evaluating: a message from the interpreter, or a
/// value thrown by `throw`.
#[derive(Debug, Clone)]
pub enum MalError {
    /// Raised by the interpreter itself: unbound symbol, bad arity, wrong
    /// type. `catch*` sees it as a string.
    Message(String),
    /// Raised by `(throw value)`. `catch*` sees the value.
    Thrown(Value),
}

impl MalError {
    /// An interpreter error with `message`.
    pub fn msg(message: impl Into<String>) -> Self {
        MalError::Message(message.into())
    }

    /// The value `catch*` binds: the thrown value, or the message as a
    /// string.
    pub fn into_value(self) -> Value {
        match self {
            MalError::Message(m) => Value::Str(m.into()),
            MalError::Thrown(v) => v,
        }
    }
}

/// Without a printer this can only show the thrown value structurally;
/// the REPL renders thrown values with its printer row instead.
impl fmt::Display for MalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MalError::Message(m) => f.write_str(m),
            MalError::Thrown(Value::Str(s)) => f.write_str(s),
            MalError::Thrown(v) => write!(f, "uncaught exception: {v:?}"),
        }
    }
}

impl std::error::Error for MalError {}

/// Result alias for evaluation and built-ins.
pub type Result<T, E = MalError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_stays_small() {
        // Three words: a tag plus the widest payload (an `Rc<str>`, or an
        // `Rc` and its metadata).
        assert!(std::mem::size_of::<Value>() <= 24);
    }

    #[test]
    fn lists_equal_vectors_and_meta_is_ignored() {
        let list = Value::list(vec![1.into(), 2.into()]);
        let vector = Value::vector(vec![1.into(), 2.into()]);
        assert_eq!(list, vector);
        let tagged = list.with_meta(Value::keyword("m")).unwrap();
        assert_eq!(tagged, vector);
        assert_eq!(tagged.meta(), Value::keyword("m"));
        assert_ne!(Value::string("a"), Value::keyword("a"));
    }

    #[test]
    fn maps_from_pairs_check_keys_and_arity() {
        let map = Value::map_from_pairs([Value::string("a"), 1.into()]).unwrap();
        let Value::Map(m, _) = &map else {
            panic!("not a map")
        };
        assert_eq!(m.get(&MapKey::Str("a".into())), Some(&Value::Int(1)));
        assert!(Value::map_from_pairs([Value::string("a")]).is_err());
        assert!(Value::map_from_pairs([Value::Int(1), Value::Nil]).is_err());
    }

    #[test]
    fn builtins_apply_and_errors_become_values() {
        let add = Value::builtin("+", |args| {
            args.iter()
                .try_fold(Value::Int(0), |acc, v| match (acc, v) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                    _ => Err(MalError::msg("not a number")),
                })
        });
        assert!(add.is_fn());
        assert_eq!(add.apply(&[1.into(), 2.into()]).unwrap(), Value::Int(3));
        let err = add.apply(&[Value::Nil]).unwrap_err();
        assert_eq!(err.to_string(), "not a number");
        assert_eq!(err.into_value(), Value::string("not a number"));
        assert!(Value::Int(1).apply(&[]).is_err());
        assert!(!Value::Nil.is_truthy() && !Value::Bool(false).is_truthy());
        assert!(Value::Int(0).is_truthy());
    }
}
