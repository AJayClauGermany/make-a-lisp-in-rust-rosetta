//! Every implementation the binary knows about, and a plain-text table of
//! them for `mal --list`.
//!
//! Comparison crates register themselves here by adding their
//! `Implementation` constant to [`all`]. The Markdown tables in the docs are
//! generated from the same data by the benchmark harness.

use std::fmt::Write;

use mal_traits::{Component, Implementation};

use crate::baseline;

/// Every comparison row, grouped by component with the `std` baseline
/// first. The binary's `--<component> <name>` flags select from these (see
/// [`crate::select`]).
const ROWS: &[&Implementation] = &[
    &reader_std::IMPLEMENTATION,
    &reader_nom::IMPLEMENTATION,
    &reader_winnow::IMPLEMENTATION,
    &reader_chumsky::IMPLEMENTATION,
    &reader_pest::IMPLEMENTATION,
    &reader_peg::IMPLEMENTATION,
    &reader_lalrpop::IMPLEMENTATION,
    &reader_logos::IMPLEMENTATION,
    &reader_regex::IMPLEMENTATION,
    &printer_std::IMPLEMENTATION,
    &printer_itoa::IMPLEMENTATION,
    &printer_memchr::IMPLEMENTATION,
    &printer_ufmt::IMPLEMENTATION,
    &env_std::IMPLEMENTATION,
    &env_hashbrown::IMPLEMENTATION,
    &env_indexmap::IMPLEMENTATION,
    &env_rustc_hash::IMPLEMENTATION,
    &env_ahash::IMPLEMENTATION,
    &env_foldhash::IMPLEMENTATION,
    &env_imbl::IMPLEMENTATION,
    &env_rpds::IMPLEMENTATION,
    &eval_std::IMPLEMENTATION,
    &core_std::IMPLEMENTATION,
    &line_editor_rustyline::IMPLEMENTATION,
    &line_editor_reedline::IMPLEMENTATION,
];

/// All known implementations, the binary's own baselines first.
pub fn all() -> Vec<&'static Implementation> {
    baseline::IMPLEMENTATIONS
        .iter()
        .chain(ROWS.iter().copied())
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
