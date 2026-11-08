//! Metadata describing an implementation, for the comparison tables.

use crate::{Component, Step};

/// A third-party crate an implementation is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CrateRef {
    /// The crates.io package name.
    pub name: &'static str,
    /// The exact version compiled against (the `Version` column).
    pub version: &'static str,
    /// Project homepage or repository, linked from the comparison table.
    pub url: &'static str,
}

/// Describes one entry in a comparison table.
///
/// Every crate under `comparisons/` exports exactly one `Implementation`
/// constant, which the `mal` binary and the report generator read so the
/// tables never drift from what is actually built. Measured columns such as
/// build time or binary size are produced by the benchmark harness and are
/// deliberately not part of this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Implementation {
    /// Short unique name, usually the headline crate (`nom`, `rustyline`).
    pub name: &'static str,
    /// Which part of the interpreter this implements.
    pub component: Component,
    /// The crates being compared. Empty for a dependency-free baseline.
    pub crates: &'static [CrateRef],
    /// The highest mal step this implementation passes the test suite for.
    pub highest_step: Step,
    /// One line of qualitative notes for the table (parser style, GC
    /// strategy, notable limitations).
    pub notes: &'static str,
}

impl Implementation {
    /// Whether this implementation reaches at least `step`.
    pub fn supports(&self, step: Step) -> bool {
        self.highest_step >= step
    }

    /// Whether this implementation uses no third-party crates.
    ///
    /// Baselines are the `null` row of a rosetta table: the number every
    /// other row is measured against.
    pub fn is_baseline(&self) -> bool {
        self.crates.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASELINE: Implementation = Implementation {
        name: "std",
        component: Component::Reader,
        crates: &[],
        highest_step: Step::ReadPrint,
        notes: "hand-written",
    };

    #[test]
    fn supports_is_cumulative() {
        assert!(BASELINE.supports(Step::Repl));
        assert!(BASELINE.supports(Step::ReadPrint));
        assert!(!BASELINE.supports(Step::Eval));
    }

    #[test]
    fn empty_crate_list_is_a_baseline() {
        assert!(BASELINE.is_baseline());
        let with_crate = Implementation {
            crates: &[CrateRef {
                name: "nom",
                version: "8.0.0",
                url: "https://github.com/rust-bakery/nom",
            }],
            ..BASELINE
        };
        assert!(!with_crate.is_baseline());
    }
}
