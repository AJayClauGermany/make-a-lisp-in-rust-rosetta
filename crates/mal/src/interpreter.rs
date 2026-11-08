//! The guide's `rep`: one [`Reader`], one [`Evaluator`] and one [`Printer`]
//! row wired together around a top-level environment with a core namespace
//! installed.
//!
//! This is the only place the components meet, so it is also where the
//! glue the traits do not cover lives: the `eval` built-in (which needs the
//! evaluator and the top-level environment), `*ARGV*`, `*host-language*`,
//! the mal-defined prelude (`not`, `load-file`, `cond`), and rendering
//! thrown values with the printer row for the REPL's `Error: ...` line.

use std::fmt;

use mal_traits::{CoreNamespace, Env, Evaluator, Interpreter, Printer, ReadOutcome, Reader};
use mal_types::{MalError, Value};

/// Definitions every step from 6 on expects, written in mal as the guide
/// does.
const PRELUDE: &[&str] = &[
    "(def! not (fn* (a) (if a false true)))",
    r#"(def! load-file (fn* (f) (eval (read-string (str "(do " (slurp f) "\nnil)")))))"#,
    r#"(defmacro! cond (fn* (& xs) (if (> (count xs) 0) (list 'if (first xs) (if (> (count xs) 1) (nth xs 1) (throw "odd number of forms to cond")) (cons 'cond (rest (rest xs)))))))"#,
];

/// An error from any phase of [`Mal::rep`], already rendered: thrown values
/// are printed with the interpreter's printer row, which `Display` on the
/// error alone could not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplError(pub String);

impl fmt::Display for ReplError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ReplError {}

/// A complete interpreter over the shared [`Value`] type.
pub struct Mal<R, P, Ev: Evaluator> {
    reader: R,
    printer: P,
    evaluator: Ev,
    env: Ev::Env,
}

impl<R, P, Ev> Mal<R, P, Ev>
where
    R: Reader<Value = Value>,
    P: Printer<Value = Value>,
    Ev: Evaluator<Value = Value, Error = MalError> + Clone + 'static,
    Ev::Env: 'static,
{
    /// Build an interpreter: install `core` into `env`, add the REPL
    /// built-ins and `argv` as `*ARGV*`, and evaluate the prelude.
    ///
    /// # Panics
    ///
    /// If the prelude fails to evaluate, which means one of the rows is
    /// broken badly enough that nothing else would work either.
    pub fn new<C>(
        reader: R,
        printer: P,
        evaluator: Ev,
        env: Ev::Env,
        core: &C,
        argv: &[String],
    ) -> Self
    where
        C: CoreNamespace<Env = Ev::Env>,
    {
        core.install(&env);
        let (eval_with, root) = (evaluator.clone(), env.clone());
        env.set(
            "eval",
            Value::builtin("eval", move |args| match args {
                [ast] => eval_with.clone().eval(ast.clone(), &root),
                _ => Err(MalError::msg("eval: expected 1 argument")),
            }),
        );
        env.set("*host-language*", Value::string("rust"));
        env.set(
            "*ARGV*",
            Value::list(argv.iter().map(|a| Value::string(a)).collect()),
        );
        let mut mal = Self {
            reader,
            printer,
            evaluator,
            env,
        };
        for line in PRELUDE {
            if let Err(e) = mal.rep(line) {
                panic!("prelude failed: {line}: {e}");
            }
        }
        mal
    }

    /// The top-level environment.
    pub fn env(&self) -> &Ev::Env {
        &self.env
    }

    /// Run `path` as a script: `(load-file path)`.
    pub fn load_file(&mut self, path: &str) -> Result<(), ReplError> {
        let form = Value::list(vec![Value::symbol("load-file"), Value::string(path)]);
        self.eval(form).map(drop)
    }

    fn eval(&mut self, ast: Value) -> Result<Value, ReplError> {
        self.evaluator.eval(ast, &self.env).map_err(|e| match e {
            MalError::Thrown(value) => ReplError(self.printer.pr_str(&value, true)),
            MalError::Message(message) => ReplError(message),
        })
    }
}

impl<R, P, Ev> Interpreter for Mal<R, P, Ev>
where
    R: Reader<Value = Value>,
    P: Printer<Value = Value>,
    Ev: Evaluator<Value = Value, Error = MalError> + Clone + 'static,
    Ev::Env: 'static,
{
    type Error = ReplError;

    fn rep(&mut self, line: &str) -> Result<Option<String>, ReplError> {
        let ast = match self.reader.read_str(line) {
            Ok(ReadOutcome::Form(ast)) => ast,
            Ok(ReadOutcome::Empty) => return Ok(None),
            Err(e) => return Err(ReplError(e.to_string())),
        };
        let value = self.eval(ast)?;
        Ok(Some(self.printer.pr_str(&value, true)))
    }
}

/// `step1_read_print`: read and print with no evaluation, so the reader and
/// printer rows can be checked against the step1 tests on their own.
pub struct ReadPrint<R, P> {
    reader: R,
    printer: P,
}

impl<R, P> ReadPrint<R, P> {
    /// Read with `reader`, print with `printer`.
    pub fn new(reader: R, printer: P) -> Self {
        Self { reader, printer }
    }
}

impl<R, P> Interpreter for ReadPrint<R, P>
where
    R: Reader<Value = Value>,
    P: Printer<Value = Value>,
{
    type Error = ReplError;

    fn rep(&mut self, line: &str) -> Result<Option<String>, ReplError> {
        match self.reader.read_str(line) {
            Ok(ReadOutcome::Form(ast)) => Ok(Some(self.printer.pr_str(&ast, true))),
            Ok(ReadOutcome::Empty) => Ok(None),
            Err(e) => Err(ReplError(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::select::{Selection, build};

    fn rep_all(lines: &[&str]) -> Vec<Result<Option<String>, ReplError>> {
        let mut mal = build(&Selection::default(), &[]).unwrap();
        lines.iter().map(|line| mal.rep(line)).collect()
    }

    #[test]
    fn reads_evaluates_and_prints() {
        let results = rep_all(&[
            "(+ 1 2)",
            "",
            "; comment",
            "(def! fib (fn* [n] (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))))",
            "(fib 15)",
            "(cond false 1 true 2)",
            "(not nil)",
            r#"(eval (read-string "(* 6 7)"))"#,
        ]);
        let printed: Vec<_> = results.into_iter().map(Result::unwrap).collect();
        assert_eq!(
            printed,
            [
                Some("3".into()),
                None,
                None,
                Some("#<function>".into()),
                Some("610".into()),
                Some("2".into()),
                Some("true".into()),
                Some("42".into()),
            ]
        );
    }

    #[test]
    fn errors_are_rendered_with_the_printer() {
        let results = rep_all(&["(throw {:msg \"oops\"})", "undefined", "(1 2"]);
        assert_eq!(results[0], Err(ReplError(r#"{:msg "oops"}"#.into())));
        assert_eq!(results[1], Err(ReplError("'undefined' not found".into())));
        assert!(results[2].as_ref().unwrap_err().0.contains("EOF"));
    }
}
