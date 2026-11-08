//! The vocabulary of the comparison: which part of the interpreter a crate
//! is standing in for, and how far up the mal step ladder it gets.

use std::fmt;

/// A part of the interpreter that can be implemented independently and
/// therefore compared across crates.
///
/// The variants follow the file layout of the mal process guide
/// (`reader`, `printer`, `env`, `step*`, `core`) plus the host-facing REPL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Component {
    /// Tokenising and parsing source text into values. Compares parser and
    /// lexer crates.
    Reader,
    /// Turning values back into text. Compares formatting crates.
    Printer,
    /// Symbol tables with lexical scoping. Compares map and persistent
    /// data-structure crates.
    Env,
    /// Evaluating forms, including special forms, tail calls and macro
    /// expansion. Compares value representation and memory-management
    /// strategies.
    Evaluator,
    /// The built-in function namespace. Compares numeric, string and
    /// collection crates.
    Core,
    /// Line input for the interactive loop. Compares readline-style crates.
    LineEditor,
}

impl Component {
    /// Every component, in the order the mal guide introduces them.
    pub const ALL: [Component; 6] = [
        Component::Reader,
        Component::Printer,
        Component::Env,
        Component::Evaluator,
        Component::Core,
        Component::LineEditor,
    ];

    /// The stable, URL- and directory-safe identifier for this component.
    ///
    /// This is the directory name used under `comparisons/`.
    pub const fn slug(self) -> &'static str {
        match self {
            Component::Reader => "reader",
            Component::Printer => "printer",
            Component::Env => "env",
            Component::Evaluator => "eval",
            Component::Core => "core",
            Component::LineEditor => "line-editor",
        }
    }

    /// A one-line description of what implementations of this component do.
    pub const fn description(self) -> &'static str {
        match self {
            Component::Reader => "parse source text into values",
            Component::Printer => "render values back to text",
            Component::Env => "scoped symbol-to-value bindings",
            Component::Evaluator => "evaluate forms and apply functions",
            Component::Core => "the built-in function namespace",
            Component::LineEditor => "read lines of input for the REPL",
        }
    }
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

/// A step of the mal process guide.
///
/// Steps are cumulative: an implementation that reaches [`Step::Macros`]
/// also satisfies everything before it. The ordering of the enum reflects
/// that, so `Step::Macros > Step::Quoting` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Step {
    /// `step0_repl`: prompt, read a line, print it back.
    Repl,
    /// `step1_read_print`: parse input into values and print them.
    ReadPrint,
    /// `step2_eval`: evaluate symbols and function calls.
    Eval,
    /// `step3_env`: `def!`, `let*` and nested environments.
    Env,
    /// `step4_if_fn_do`: `if`, `do`, `fn*` and the core namespace.
    IfFnDo,
    /// `step5_tco`: tail-call optimisation.
    Tco,
    /// `step6_file`: `read-string`, `slurp`, `eval`, atoms and `load-file`.
    File,
    /// `step7_quote`: `quote` and `quasiquote`.
    Quoting,
    /// `step8_macros`: `defmacro!` and macro expansion.
    Macros,
    /// `step9_try`: `try*`/`catch*`, `throw`, `apply`, `map`.
    Try,
    /// `stepA_mal`: metadata, self-hosting and interop.
    Mal,
}

impl Step {
    /// Every step in order.
    pub const ALL: [Step; 11] = [
        Step::Repl,
        Step::ReadPrint,
        Step::Eval,
        Step::Env,
        Step::IfFnDo,
        Step::Tco,
        Step::File,
        Step::Quoting,
        Step::Macros,
        Step::Try,
        Step::Mal,
    ];

    /// The step name as used by the mal guide and test suite, e.g. `step4_if_fn_do`.
    pub const fn name(self) -> &'static str {
        match self {
            Step::Repl => "step0_repl",
            Step::ReadPrint => "step1_read_print",
            Step::Eval => "step2_eval",
            Step::Env => "step3_env",
            Step::IfFnDo => "step4_if_fn_do",
            Step::Tco => "step5_tco",
            Step::File => "step6_file",
            Step::Quoting => "step7_quote",
            Step::Macros => "step8_macros",
            Step::Try => "step9_try",
            Step::Mal => "stepA_mal",
        }
    }

    /// The single-character step label used in the mal guide (`0`..`9`, `A`).
    pub const fn label(self) -> char {
        match self {
            Step::Repl => '0',
            Step::ReadPrint => '1',
            Step::Eval => '2',
            Step::Env => '3',
            Step::IfFnDo => '4',
            Step::Tco => '5',
            Step::File => '6',
            Step::Quoting => '7',
            Step::Macros => '8',
            Step::Try => '9',
            Step::Mal => 'A',
        }
    }

    /// The step after this one, or `None` for the last step.
    pub fn next(self) -> Option<Step> {
        let index = Step::ALL.iter().position(|s| *s == self)?;
        Step::ALL.get(index + 1).copied()
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_ordered_cumulatively() {
        assert!(Step::Repl < Step::ReadPrint);
        assert!(Step::Macros > Step::Quoting);
        assert_eq!(Step::ALL.first(), Some(&Step::Repl));
        assert_eq!(Step::ALL.last(), Some(&Step::Mal));
    }

    #[test]
    fn step_chain_covers_all_steps() {
        let mut walked = vec![Step::Repl];
        while let Some(next) = walked.last().copied().and_then(Step::next) {
            walked.push(next);
        }
        assert_eq!(walked, Step::ALL);
    }

    #[test]
    fn names_match_mal_guide() {
        assert_eq!(Step::IfFnDo.to_string(), "step4_if_fn_do");
        assert_eq!(Step::Mal.label(), 'A');
        assert_eq!(Component::LineEditor.to_string(), "line-editor");
    }

    #[test]
    fn component_slugs_are_unique_and_path_safe() {
        let slugs: Vec<&str> = Component::ALL.iter().map(|c| c.slug()).collect();
        for (i, slug) in slugs.iter().enumerate() {
            assert!(!slugs[..i].contains(slug), "duplicate slug {slug}");
            assert!(
                slug.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "slug {slug} is not path safe"
            );
        }
    }
}
