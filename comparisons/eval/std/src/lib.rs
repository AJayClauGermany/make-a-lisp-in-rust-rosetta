//! The `std` evaluator row: a tree-walking `EVAL` over [`Rc`]-shared values.
//!
//! This is the guide's stepA evaluator: special forms, closures, a loop for
//! tail calls in `if`, `do`, `let*`, `try*`/`catch*` handlers and closure
//! application, quasiquote expansion and macros. Memory is managed by
//! reference counting alone, so closures that capture the environment they
//! are defined in (every recursive `def!`) form cycles that are never freed;
//! that is the cost this row stands for, and what the tracing-GC rows would
//! be compared against.
//!
//! The evaluator is generic over the environment row, so the same evaluator
//! runs on every environment implementation.

use std::any::Any;
use std::marker::PhantomData;
use std::rc::Rc;

use mal_traits::{Component, Env, Evaluator, Implementation, Step};
use mal_types::{Closure, MalError, Result, Value};

/// Table entry for [`StdEvaluator`].
pub const IMPLEMENTATION: Implementation = Implementation {
    name: "std",
    component: Component::Evaluator,
    crates: &[],
    highest_step: Step::Mal,
    notes: "Rc values, loop-based TCO, closures capture a type-erased env; Rc cycles leak",
};

/// The evaluator. It has no state of its own: everything lives in the
/// environment, which is what lets closures call back into it from the core
/// namespace.
pub struct StdEvaluator<E>(PhantomData<fn() -> E>);

impl<E> StdEvaluator<E> {
    /// An evaluator for environments of type `E`.
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<E> Default for StdEvaluator<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Clone for StdEvaluator<E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E> Copy for StdEvaluator<E> {}

impl<E> std::fmt::Debug for StdEvaluator<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StdEvaluator")
    }
}

impl<E: Env<Value = Value> + 'static> Evaluator for StdEvaluator<E> {
    type Value = Value;
    type Env = E;
    type Error = MalError;

    fn eval(&mut self, ast: Value, env: &E) -> Result<Value> {
        eval(ast, env)
    }
}

/// Evaluate `ast` in `env`.
#[allow(clippy::too_many_lines)] // One match over the special forms reads best whole.
pub fn eval<E: Env<Value = Value> + 'static>(mut ast: Value, env: &E) -> Result<Value> {
    let mut env = env.clone();
    loop {
        let items = match &ast {
            Value::List(items, _) if !items.is_empty() => Rc::clone(items),
            Value::Symbol(name) => {
                return env
                    .get(name)
                    .ok_or_else(|| MalError::msg(format!("'{name}' not found")));
            }
            Value::Vector(items, _) => {
                let items = items
                    .iter()
                    .map(|item| eval(item.clone(), &env))
                    .collect::<Result<_>>()?;
                return Ok(Value::vector(items));
            }
            Value::Map(map, _) => {
                let mut evaluated = map.as_ref().clone();
                for value in evaluated.values_mut() {
                    *value = eval(value.clone(), &env)?;
                }
                return Ok(Value::map(evaluated));
            }
            _ => return Ok(ast),
        };

        let special = match &items[0] {
            Value::Symbol(name) => Some(name.as_ref()),
            _ => None,
        };
        match (special, &items[1..]) {
            (Some("def!"), [Value::Symbol(name), value]) => {
                let value = eval(value.clone(), &env)?;
                env.set(name, value.clone());
                return Ok(value);
            }
            (Some("defmacro!"), [Value::Symbol(name), value]) => {
                let Value::Closure(closure) = eval(value.clone(), &env)? else {
                    return Err(MalError::msg("defmacro!: expected a function"));
                };
                let mac = Value::Closure(Rc::new(Closure {
                    is_macro: true,
                    ..Closure::clone(&closure)
                }));
                env.set(name, mac.clone());
                return Ok(mac);
            }
            (Some("let*"), [bindings, body]) => {
                let bindings = bindings
                    .as_seq()
                    .ok_or_else(|| MalError::msg("let*: bindings must be a list or vector"))?;
                let inner = env.child();
                for pair in bindings.chunks(2) {
                    let [Value::Symbol(name), value] = pair else {
                        return Err(MalError::msg("let*: expected symbol/value pairs"));
                    };
                    let value = eval(value.clone(), &inner)?;
                    inner.set(name, value);
                }
                env = inner;
                ast = body.clone();
            }
            (Some("do"), forms) => {
                let Some((last, init)) = forms.split_last() else {
                    return Ok(Value::Nil);
                };
                for form in init {
                    eval(form.clone(), &env)?;
                }
                ast = last.clone();
            }
            (Some("if"), [cond, then, otherwise @ ..]) if otherwise.len() <= 1 => {
                ast = if eval(cond.clone(), &env)?.is_truthy() {
                    then.clone()
                } else {
                    otherwise.first().cloned().unwrap_or_default()
                };
            }
            (Some("fn*"), [params, body]) => return closure(params, body, &env),
            (Some("quote"), [form]) => return Ok(form.clone()),
            (Some("quasiquote"), [form]) => ast = quasiquote(form),
            (Some("try*"), [body]) => ast = body.clone(),
            (Some("try*"), [body, handler]) => {
                let Some([Value::Symbol(catch), Value::Symbol(name), handler]) =
                    handler.as_seq().map(<[Value]>::as_ref)
                else {
                    return Err(MalError::msg("try*: expected (catch* symbol form)"));
                };
                if catch.as_ref() != "catch*" {
                    return Err(MalError::msg("try*: expected (catch* symbol form)"));
                }
                match eval(body.clone(), &env) {
                    Ok(value) => return Ok(value),
                    Err(error) => {
                        let inner = env.child();
                        inner.set(name, error.into_value());
                        env = inner;
                        ast = handler.clone();
                    }
                }
            }
            (
                Some(
                    name @ ("def!" | "defmacro!" | "let*" | "if" | "fn*" | "quote" | "quasiquote"
                    | "try*"),
                ),
                _,
            ) => return Err(MalError::msg(format!("{name}: malformed special form"))),
            _ => {
                let f = eval(items[0].clone(), &env)?;
                if let Value::Closure(c) = &f {
                    if c.is_macro {
                        ast = c.call(&items[1..])?;
                        continue;
                    }
                }
                let args = items[1..]
                    .iter()
                    .map(|arg| eval(arg.clone(), &env))
                    .collect::<Result<Vec<_>>>()?;
                match f {
                    // Tail call: evaluate the body here instead of recursing.
                    Value::Closure(c) => {
                        env = bind::<E>(&c, &args)?;
                        ast = c.body.clone();
                    }
                    other => return other.apply(&args),
                }
            }
        }
    }
}

/// `(fn* [a b & rest] body)`.
fn closure<E: Env<Value = Value> + 'static>(
    params: &Value,
    body: &Value,
    env: &E,
) -> Result<Value> {
    let names = params
        .as_seq()
        .ok_or_else(|| MalError::msg("fn*: parameters must be a list or vector"))?
        .iter()
        .map(|p| match p {
            Value::Symbol(name) => Ok(Rc::clone(name)),
            other => Err(MalError::msg(format!(
                "fn*: parameter must be a symbol, not {}",
                other.type_name()
            ))),
        })
        .collect::<Result<Vec<_>>>()?;
    let (params, rest) = match names.iter().position(|n| n.as_ref() == "&") {
        Some(i) if i + 2 == names.len() => (names[..i].into(), Some(Rc::clone(&names[i + 1]))),
        Some(_) => return Err(MalError::msg("fn*: expected exactly one parameter after &")),
        None => (names.into(), None),
    };
    Ok(Value::Closure(Rc::new(Closure {
        params,
        rest,
        body: body.clone(),
        env: Rc::new(env.clone()) as Rc<dyn Any>,
        apply: apply::<E>,
        is_macro: false,
        meta: None,
    })))
}

/// The environment a call to `closure` with `args` runs its body in.
fn bind<E: Env<Value = Value> + 'static>(closure: &Closure, args: &[Value]) -> Result<E> {
    let captured = closure.env.downcast_ref::<E>().ok_or_else(|| {
        MalError::msg("closure was created by an evaluator with another environment type")
    })?;
    let arity_ok = match closure.rest {
        Some(_) => args.len() >= closure.params.len(),
        None => args.len() == closure.params.len(),
    };
    if !arity_ok {
        return Err(MalError::msg(format!(
            "wrong number of arguments: expected {}{}, got {}",
            closure.params.len(),
            if closure.rest.is_some() {
                " or more"
            } else {
                ""
            },
            args.len()
        )));
    }
    let env = captured.child();
    for (name, value) in closure.params.iter().zip(args) {
        env.set(name, value.clone());
    }
    if let Some(rest) = &closure.rest {
        env.set(rest, Value::list(args[closure.params.len()..].to_vec()));
    }
    Ok(env)
}

/// How the core namespace calls a closure (`apply`, `map`, `swap!`) and
/// how macros are expanded.
fn apply<E: Env<Value = Value> + 'static>(closure: &Closure, args: &[Value]) -> Result<Value> {
    let env = bind::<E>(closure, args)?;
    eval(closure.body.clone(), &env)
}

/// Expand a quasiquoted form into the `cons`/`concat`/`vec`/`quote` calls
/// that build it.
fn quasiquote(ast: &Value) -> Value {
    match ast {
        Value::List(items, _) => match items.as_slice() {
            [Value::Symbol(s), form] if s.as_ref() == "unquote" => form.clone(),
            _ => quasiquote_seq(items),
        },
        Value::Vector(items, _) => Value::list(vec![Value::symbol("vec"), quasiquote_seq(items)]),
        Value::Symbol(_) | Value::Map(..) => Value::wrap("quote", ast.clone()),
        _ => ast.clone(),
    }
}

fn quasiquote_seq(items: &[Value]) -> Value {
    items
        .iter()
        .rev()
        .fold(Value::list(Vec::new()), |acc, item| {
            if let Value::List(inner, _) = item {
                if let [Value::Symbol(s), form] = inner.as_slice() {
                    if s.as_ref() == "splice-unquote" {
                        return Value::list(vec![Value::symbol("concat"), form.clone(), acc]);
                    }
                }
            }
            Value::list(vec![Value::symbol("cons"), quasiquote(item), acc])
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    type Scope = (RefCell<HashMap<String, Value>>, Option<TestEnv>);

    /// A minimal environment so the evaluator is tested on its own.
    #[derive(Clone, Default)]
    struct TestEnv(Rc<Scope>);

    impl Env for TestEnv {
        type Value = Value;
        fn child(&self) -> Self {
            Self(Rc::new((RefCell::default(), Some(self.clone()))))
        }
        fn set(&self, symbol: &str, value: Value) {
            self.0.0.borrow_mut().insert(symbol.to_owned(), value);
        }
        fn get(&self, symbol: &str) -> Option<Value> {
            match self.0.0.borrow().get(symbol) {
                Some(v) => Some(v.clone()),
                None => self.0.1.as_ref()?.get(symbol),
            }
        }
    }

    fn sym(s: &str) -> Value {
        Value::symbol(s)
    }

    fn list(items: impl Into<Vec<Value>>) -> Value {
        Value::list(items.into())
    }

    fn env() -> TestEnv {
        let env = TestEnv::default();
        env.set(
            "+",
            Value::builtin("+", |a| match a {
                [Value::Int(x), Value::Int(y)] => Ok(Value::Int(x + y)),
                _ => Err(MalError::msg("+")),
            }),
        );
        env.set(
            "-",
            Value::builtin("-", |a| match a {
                [Value::Int(x), Value::Int(y)] => Ok(Value::Int(x - y)),
                _ => Err(MalError::msg("-")),
            }),
        );
        env.set(
            "=",
            Value::builtin("=", |a| match a {
                [x, y] => Ok(Value::Bool(x == y)),
                _ => Err(MalError::msg("=")),
            }),
        );
        env
    }

    #[test]
    fn evaluates_through_the_trait() {
        let env = env();
        let mut ev = StdEvaluator::<TestEnv>::new();
        let ast = list([sym("+"), 1.into(), 2.into()]);
        assert_eq!(ev.eval(ast, &env).unwrap(), Value::Int(3));
        let err = ev.eval(sym("nope"), &env).unwrap_err();
        assert_eq!(err.to_string(), "'nope' not found");
    }

    #[test]
    fn tail_calls_do_not_grow_the_stack() {
        // (def! count-down (fn* (n) (if (= n 0) 0 (count-down (- n 1)))))
        let env = env();
        let body = list([
            sym("if"),
            list([sym("="), sym("n"), 0.into()]),
            0.into(),
            list([sym("count-down"), list([sym("-"), sym("n"), 1.into()])]),
        ]);
        let def = list([
            sym("def!"),
            sym("count-down"),
            list([sym("fn*"), list([sym("n")]), body]),
        ]);
        eval(def, &env).unwrap();
        let call = list([sym("count-down"), 100_000.into()]);
        assert_eq!(eval(call, &env).unwrap(), Value::Int(0));
    }

    #[test]
    fn closures_apply_from_outside_and_catch_errors() {
        let env = env();
        let f = eval(
            list([
                sym("fn*"),
                Value::vector(vec![sym("a"), sym("&"), sym("more")]),
                sym("more"),
            ]),
            &env,
        )
        .unwrap();
        assert_eq!(
            f.apply(&[1.into(), 2.into(), 3.into()]).unwrap(),
            list([2.into(), 3.into()])
        );
        let caught = list([
            sym("try*"),
            sym("undefined"),
            list([sym("catch*"), sym("e"), sym("e")]),
        ]);
        assert_eq!(
            eval(caught, &env).unwrap(),
            Value::string("'undefined' not found")
        );
    }

    #[test]
    fn quasiquote_expands_to_constructors() {
        // `(1 ~a ~@b) with a = 2, b = (3 4)
        let env = env();
        env.set("a", 2.into());
        env.set("b", list([3.into(), 4.into()]));
        env.set(
            "cons",
            Value::builtin("cons", |a| match a {
                [x, Value::List(xs, _)] => {
                    let mut v = vec![x.clone()];
                    v.extend_from_slice(xs);
                    Ok(Value::list(v))
                }
                _ => Err(MalError::msg("cons")),
            }),
        );
        env.set(
            "concat",
            Value::builtin("concat", |a| {
                Ok(Value::list(
                    a.iter()
                        .flat_map(|x| x.as_seq().unwrap_or_default().to_vec())
                        .collect(),
                ))
            }),
        );
        let form = list([
            1.into(),
            Value::wrap("unquote", sym("a")),
            Value::wrap("splice-unquote", sym("b")),
        ]);
        let result = eval(Value::wrap("quasiquote", form), &env).unwrap();
        assert_eq!(result, list([1.into(), 2.into(), 3.into(), 4.into()]));
    }
}
