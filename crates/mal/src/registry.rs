//! Every implementation the binary knows about, and a plain-text table of
//! them for `mal --list`.
//!
//! Rows are registered by listing their type in [`crate::stack`] (or, for
//! line editors, in `LINE_EDITORS`); the table entry comes from the type's
//! [`Row`] implementation. The Markdown tables in the docs are
//! generated from the same data by the benchmark harness.

use std::fmt::Write;

use mal_traits::{Component, Implementation, Row};

use crate::{baseline, stack};

/// The line-editor rows. The rows of every other component are listed in
/// [`crate::stack`], since those are what an interpreter is built from.
const LINE_EDITORS: &[&Implementation] = &[
    <line_editor_rustyline::RustylineEditor as Row>::IMPLEMENTATION,
    <line_editor_reedline::ReedlineEditor as Row>::IMPLEMENTATION,
    <line_editor_dialoguer::DialoguerEditor as Row>::IMPLEMENTATION,
];

/// All known implementations, grouped by component with the `std` baseline
/// first: the binary's own baselines, then the rows of [`crate::stack`],
/// then the line editors. The binary's `--<component> <name>` flags select
/// from these (see [`crate::select`]).
pub fn all() -> Vec<&'static Implementation> {
    baseline::IMPLEMENTATIONS
        .iter()
        .chain(
            stack::READERS
                .iter()
                .chain(stack::PRINTERS)
                .chain(stack::ENVS)
                .chain(stack::EVALS)
                .chain(stack::CORES)
                .chain(LINE_EDITORS)
                .copied(),
        )
        .collect()
}

/// Render the implementation list grouped by component.
pub fn render_table() -> String {
    let implementations = all();
    let mut out = String::new();
    for component in Component::ALL {
        let rows: Vec<_> = implementations
            .iter()
            .filter(|imp| imp.component == component)
            .collect();
        if rows.is_empty() {
            continue;
        }
        // Writing to a String cannot fail.
        let _ = writeln!(out, "{component} ({})", component.description());
        for imp in rows {
            let crates = if imp.is_baseline() {
                String::from("(baseline)")
            } else {
                imp.crates
                    .iter()
                    .map(|c| format!("{}@{}", c.name, c.version))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = writeln!(
                out,
                "  {:<12} {:<18} {:<28} {}",
                imp.name, imp.highest_step, crates, imp.notes
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_per_component() {
        let imps = all();
        for (i, a) in imps.iter().enumerate() {
            for b in &imps[..i] {
                assert!(
                    !(a.name == b.name && a.component == b.component),
                    "duplicate implementation {} for {}",
                    a.name,
                    a.component
                );
            }
        }
    }

    /// The `Version` column comes from the `CrateRef`s, so they must name
    /// the exact versions the workspace is locked to.
    #[test]
    fn crate_versions_match_the_lockfile() {
        let lock = include_str!("../../../Cargo.lock");
        for imp in all() {
            for krate in imp.crates {
                let entry = format!(
                    "name = \"{}\"\nversion = \"{}\"\n",
                    krate.name, krate.version
                );
                assert!(
                    lock.contains(&entry),
                    "{}/{}: {} {} is not in Cargo.lock",
                    imp.component,
                    imp.name,
                    krate.name,
                    krate.version
                );
            }
        }
    }

    #[test]
    fn table_lists_every_implementation() {
        let table = render_table();
        for imp in all() {
            assert!(table.contains(imp.name), "{} missing from table", imp.name);
            assert!(table.contains(imp.component.slug()));
        }
    }
}
