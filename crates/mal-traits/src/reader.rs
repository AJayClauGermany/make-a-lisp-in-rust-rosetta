//! The reader: text in, values out.

/// The result of reading one top-level form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome<V> {
    /// A complete form was read.
    Form(V),
    /// The input contained only whitespace and comments.
    ///
    /// The mal test suite expects the REPL to print nothing in this case
    /// rather than treating it as an error.
    Empty,
}

/// Parses mal source text into values.
///
/// This is the `reader.qx` module of the mal guide: tokenising on the mal
/// token grammar and building lists, vectors, hash-maps and atoms, including
/// the reader macros (`'`, `` ` ``, `~`, `~@`, `@`, `^`).
///
/// Implementations are compared on parse speed, build time, binary-size
/// overhead and how much of the grammar they express directly.
pub trait Reader {
    /// The value representation this reader produces.
    type Value;
    /// Reported for unbalanced delimiters, bad escapes and similar.
    type Error: std::error::Error;

    /// Read a single top-level form from `input`.
    ///
    /// Equivalent to the guide's `read_str`. Trailing input after the first
    /// form is ignored, matching the reference implementations.
    fn read_str(&mut self, input: &str) -> Result<ReadOutcome<Self::Value>, Self::Error>;
}
