//! The printer: values in, text out.

use std::fmt::Write;

/// Renders values as mal source text.
///
/// This is the `printer.qx` module of the mal guide.
pub trait Printer {
    /// The value representation this printer understands.
    type Value;

    /// Append the textual form of `value` to `out`.
    ///
    /// When `print_readably` is true, strings are quoted and escaped so the
    /// output can be read back by a [`Reader`](crate::Reader); when false
    /// they are emitted raw (used by `str` and `println`).
    fn pr_str_into(&self, value: &Self::Value, print_readably: bool, out: &mut String);

    /// Render `value` to a fresh `String`. Equivalent to the guide's `pr_str`.
    fn pr_str(&self, value: &Self::Value, print_readably: bool) -> String {
        let mut out = String::new();
        self.pr_str_into(value, print_readably, &mut out);
        out
    }

    /// Render a sequence of values separated by `separator`, as `pr-str`,
    /// `str`, `prn` and `println` require.
    fn pr_seq<'a, I>(&self, values: I, print_readably: bool, separator: &str) -> String
    where
        Self::Value: 'a,
        I: IntoIterator<Item = &'a Self::Value>,
    {
        let mut out = String::new();
        for (i, value) in values.into_iter().enumerate() {
            if i > 0 {
                // Writing to a String cannot fail.
                let _ = out.write_str(separator);
            }
            self.pr_str_into(value, print_readably, &mut out);
        }
        out
    }
}
