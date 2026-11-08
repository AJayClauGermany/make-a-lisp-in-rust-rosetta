//! Every implementation the binary knows about, and a plain-text table of
//! them for `mal --list`.
//!
//! Comparison crates register themselves here by adding their
//! `Implementation` constant to [`all`]. The Markdown tables in the docs are
//! generated from the same data by the benchmark harness.

use std::fmt::Write;

use mal_traits::{Component, Implementation};

use crate::baseline;

/// All known implementations, baselines first.
pub fn all() -> Vec<&'static Implementation> {
    baseline::IMPLEMENTATIONS.iter().collect()
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

    #[test]
    fn table_lists_every_implementation() {
        let table = render_table();
        for imp in all() {
            assert!(table.contains(imp.name), "{} missing from table", imp.name);
            assert!(table.contains(imp.component.slug()));
        }
    }
}
