//! The fixed inputs every implementation is measured on.
//!
//! The corpus is deliberately small and stable: changing it invalidates
//! comparisons with earlier runs under `runs/`, so additions should be new
//! entries rather than edits to existing ones.

/// One top-level mal form with a short, stable name used in benchmark ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Form {
    /// Benchmark id suffix, e.g. `nested`. Lowercase, hyphens only.
    pub name: &'static str,
    /// The source text, exactly one top-level form.
    pub source: &'static str,
}

/// Forms that cover the reader grammar and the shapes the evaluator sees
/// most: atoms, flat and nested lists, strings with escapes, the collection
/// literals, the reader macros and one program-sized `do` block.
pub const FORMS: &[Form] = &[
    Form {
        name: "atom",
        source: "42",
    },
    Form {
        name: "symbol",
        source: "swap-and-reset!",
    },
    Form {
        name: "list",
        source: "(+ 1 2 3 4 5 6 7 8 9 10)",
    },
    Form {
        name: "nested",
        source: "(def! fib (fn* [n] (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))))",
    },
    Form {
        name: "string",
        source: r#""hello \"quoted\" world\nwith a newline and a \\ backslash""#,
    },
    Form {
        name: "collections",
        source: r#"{"a" [1 2 3] :b (list 4 5 6) :c {:d nil :e true :f false}}"#,
    },
    Form {
        name: "quoting",
        source: "`(1 ~a ~@(list 2 3) '(4 5) @counter ^{\"meta\" 1} [6 7])",
    },
    Form {
        name: "program",
        source: PROGRAM,
    },
];

/// Programs that spend their time evaluating rather than parsing: deep
/// non-tail recursion, a tail-call loop, and closures passed to built-ins.
/// Each is one `do` form that defines what it needs, so it can be
/// evaluated repeatedly in the same environment. They need the core
/// namespace.
///
/// These are what the evaluator rows and, through the `std` evaluator, the
/// environment rows are timed on: symbol lookup through nested scopes and
/// binding parameters on every call is where an environment shows up.
pub const PROGRAMS: &[Form] = &[
    Form {
        name: "fib",
        source: "(do (def! fib (fn* (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))) (fib 12))",
    },
    Form {
        name: "sum-to",
        source: "(do (def! sum-to (fn* (n acc) (if (= n 0) acc (sum-to (- n 1) (+ n acc))))) (sum-to 1000 0))",
    },
    Form {
        name: "closures",
        source: "(do (def! adder (fn* (k) (fn* (x) (+ x k)))) (let* (xs (list 1 2 3 4 5 6 7 8 9 10) a (adder 5) b (adder 7)) (count (map a (map b (concat xs xs xs xs))))))",
    },
];

/// A program-sized form: roughly what one `load-file` of the mal test
/// suite's helper definitions looks like.
const PROGRAM: &str = r#"(do
  (def! not (fn* [a] (if a false true)))
  (def! load-file (fn* [f] (eval (read-string (str "(do " (slurp f) "\nnil)")))))
  (defmacro! cond (fn* [& xs]
    (if (> (count xs) 0)
      (list 'if (first xs)
        (if (> (count xs) 1) (nth xs 1) (throw "odd number of forms to cond"))
        (cons 'cond (rest (rest xs)))))))
  (def! inc (fn* [a] (+ a 1)))
  (def! dec (fn* [a] (- a 1)))
  (def! zero? (fn* [n] (= 0 n)))
  (def! reduce (fn* [f init xs]
    (if (empty? xs) init (reduce f (f init (first xs)) (rest xs)))))
  (def! identity (fn* [x] x))
  (def! every? (fn* [pred xs]
    (cond (empty? xs) true
          (pred (first xs)) (every? pred (rest xs))
          true false)))
  (def! some (fn* [pred xs]
    (cond (empty? xs) nil
          (pred (first xs)) (first xs)
          true (some pred (rest xs)))))
  (def! sum-to (fn* [n acc] (if (= n 0) acc (sum-to (- n 1) (+ n acc)))))
  (def! gensym-counter (atom 0))
  (def! gensym (fn* []
    (symbol (str "G__" (swap! gensym-counter (fn* [x] (+ 1 x)))))))
  (defmacro! or (fn* [& xs]
    (if (empty? xs) nil
      (if (= 1 (count xs)) (first xs)
        (let* [condvar (gensym)]
          `(let* [~condvar ~(first xs)] (if ~condvar ~condvar (or ~@(rest xs)))))))))
  (sum-to 10 0))"#;

/// The lines of one interactive session: every form in [`FORMS`], a blank
/// line and a comment-only line, in the order a user might type them.
pub fn session() -> Vec<&'static str> {
    let mut lines: Vec<&'static str> = FORMS.iter().map(|form| form.source).collect();
    lines.push("");
    lines.push("; a comment-only line prints nothing");
    lines
}

/// The total number of input bytes in [`session`], for throughput figures.
pub fn session_bytes() -> u64 {
    session().iter().map(|line| line.len() as u64 + 1).sum()
}

/// Symbols for the environment benchmarks: `sym-0` .. `sym-{n-1}`.
pub fn symbols(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("sym-{i}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_names_are_unique_and_id_safe() {
        let all: Vec<&Form> = FORMS.iter().chain(PROGRAMS).collect();
        for (i, form) in all.iter().enumerate() {
            assert!(
                all[..i].iter().all(|other| other.name != form.name),
                "duplicate form name {}",
                form.name
            );
            assert!(
                form.name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "form name {} is not id safe",
                form.name
            );
            assert_ne!(form.source, "", "{} has no source", form.name);
        }
    }

    #[test]
    fn forms_are_balanced() {
        // A cheap sanity check that each form is one complete expression:
        // delimiters outside strings balance.
        for form in FORMS.iter().chain(PROGRAMS) {
            let mut depth = 0i32;
            let mut in_string = false;
            let mut escaped = false;
            for c in form.source.chars() {
                if in_string {
                    match (escaped, c) {
                        (true, _) => escaped = false,
                        (false, '\\') => escaped = true,
                        (false, '"') => in_string = false,
                        _ => {}
                    }
                    continue;
                }
                match c {
                    '"' => in_string = true,
                    '(' | '[' | '{' => depth += 1,
                    ')' | ']' | '}' => depth -= 1,
                    _ => {}
                }
                assert!(depth >= 0, "{} closes more than it opens", form.name);
            }
            assert_eq!(depth, 0, "{} is unbalanced", form.name);
            assert!(!in_string, "{} has an unterminated string", form.name);
        }
    }

    #[test]
    fn session_covers_every_form_and_counts_newlines() {
        let lines = session();
        assert_eq!(lines.len(), FORMS.len() + 2);
        let expected: u64 = lines.iter().map(|l| l.len() as u64 + 1).sum();
        assert_eq!(session_bytes(), expected);
        assert_eq!(symbols(3), ["sym-0", "sym-1", "sym-2"]);
    }
}
