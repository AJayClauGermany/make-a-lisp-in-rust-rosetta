//! Inline SVG charts for the results page.
//!
//! The tables on the results page are the source of truth; these charts are
//! drawn from exactly the same [`Benchmark`] rows so a reader can see at a
//! glance how the implementations of a component relate. Every value is
//! expressed relative to the component's baseline (the dependency-free
//! [`BASELINE`] row), which is the comparison the project is about.
//!
//! Two charts are drawn per component that has a measured baseline and at
//! least one other measured implementation:
//!
//! * a summary bar chart: the geometric mean, over the workloads both rows
//!   measured, of each implementation's time divided by the baseline's,
//!   on a log2 axis centred on 1x;
//! * a heatmap: one row per workload, one column per implementation, each
//!   cell coloured by the same ratio and labelled with it (the baseline
//!   column shows its absolute time instead).
//!
//! The output is plain SVG inside a `<figure>`, so the page needs no
//! JavaScript; hover tooltips are SVG `<title>` elements. Colours are CSS
//! classes defined once per page by [`STYLE`]: text uses `currentColor` and
//! the colour ramps are translucent over the page background, so the charts
//! follow whatever light or dark theme the site renders with.
//! The output is deterministic, which `cargo xtask report --check` relies on.

use std::fmt::Write as _;

use crate::report::duration;
use crate::run::Benchmark;

/// The implementation every other row of a component is compared against.
pub const BASELINE: &str = "std";

/// Page-level styles for the charts, emitted once before the first chart.
///
/// Faster than the baseline is blue, slower is red, and "about the same"
/// (within 25%) is a neutral tint of the page's own text colour. The ramps
/// are steps of opacity over the page background, so the weakest step
/// recedes toward the surface in both light and dark themes.
pub const STYLE: &str = "<style>
.malc{margin:1.5em 0;overflow-x:auto}
.malc svg{display:block;width:100%;height:auto;font-size:12px;font-variant-numeric:tabular-nums}
.malc figcaption{font-size:.85em;opacity:.8;margin-top:.4em}
.malc .t{fill:currentColor}
.malc .m{fill:currentColor;fill-opacity:.62}
.malc .w{fill:#fff}
.malc .grid{stroke:currentColor;stroke-opacity:.14;stroke-width:1}
.malc .axis{stroke:currentColor;stroke-opacity:.45;stroke-width:1}
.malc .none{fill:none;stroke:currentColor;stroke-opacity:.18;stroke-width:1}
.malc .c0{fill:currentColor;fill-opacity:.09}
.malc .f1{fill:#1c5cab;fill-opacity:.28}
.malc .f2{fill:#1c5cab;fill-opacity:.52}
.malc .f3{fill:#1c5cab;fill-opacity:.8}
.malc .f4{fill:#1c5cab}
.malc .s1{fill:#c0302f;fill-opacity:.28}
.malc .s2{fill:#c0302f;fill-opacity:.52}
.malc .s3{fill:#c0302f;fill-opacity:.8}
.malc .s4{fill:#c0302f}
.malc .bf{fill:#2a78d6}
.malc .bs{fill:#e34948}
.malc .dot{fill:currentColor}
</style>";

/// Ratio buckets, slowest last: (upper bound exclusive, class, legend label).
/// The neutral bucket is symmetric in log space: 0.8 = 1 / 1.25.
const BUCKETS: [(f64, &str, &str); 9] = [
    (0.125, "f4", "<1/8×"),
    (0.25, "f3", "1/8–1/4×"),
    (0.5, "f2", "1/4–1/2×"),
    (0.8, "f1", "1/2–0.8×"),
    (1.25, "c0", "0.8–1.25×"),
    (2.0, "s1", "1.25–2×"),
    (4.0, "s2", "2–4×"),
    (8.0, "s3", "4–8×"),
    (f64::INFINITY, "s4", ">8×"),
];

/// Approximate advance width of one character at the chart font size; used
/// only to size label columns, so it errs wide.
const CHAR_W: f64 = 7.0;
/// The same for the monospace workload labels.
const MONO_W: f64 = 8.0;

/// Both charts for one component, or `None` when there is nothing to
/// compare (no measured baseline, or the baseline is the only row).
///
/// `order` is the implementations in the order the table lists them.
pub fn render(title: &str, order: &[&str], measured: &[&Benchmark]) -> Option<String> {
    let baseline: Vec<&Benchmark> = measured
        .iter()
        .copied()
        .filter(|b| b.implementation == BASELINE)
        .collect();
    let others: Vec<&str> = order
        .iter()
        .copied()
        .filter(|name| *name != BASELINE && measured.iter().any(|b| b.implementation == *name))
        .collect();
    if baseline.is_empty() || others.is_empty() {
        return None;
    }
    let mut out = String::new();
    summary(&mut out, title, &others, measured, &baseline);
    out.push('\n');
    heatmap(&mut out, title, order, measured, &baseline);
    Some(out)
}

/// Time of `b` relative to the baseline's time on the same workload.
fn ratio(b: &Benchmark, baseline: &[&Benchmark]) -> Option<f64> {
    let base = baseline.iter().find(|x| x.workload == b.workload)?;
    (base.mean_ns > 0.0 && b.mean_ns > 0.0).then(|| b.mean_ns / base.mean_ns)
}

fn bucket(r: f64) -> usize {
    BUCKETS
        .iter()
        .position(|(upper, ..)| r < *upper)
        .unwrap_or(BUCKETS.len() - 1)
}

/// A ratio as printed on the charts: two significant figures below 10.
fn times(r: f64) -> String {
    if r >= 9.95 {
        format!("{r:.0}×")
    } else if r >= 0.995 {
        format!("{r:.1}×")
    } else if r >= 0.0995 {
        format!("{r:.2}×")
    } else {
        format!("{r:.3}×")
    }
}

/// A power of two as an axis tick: `1/4×`, `1×`, `8×`.
fn pow2_label(k: i32) -> String {
    if k < 0 {
        format!("1/{}×", 1_u64 << k.unsigned_abs())
    } else {
        format!("{}×", 1_u64 << k.unsigned_abs())
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// Label lengths are a few dozen characters; the casts are exact.
#[allow(clippy::cast_precision_loss)]
fn text_width(s: &str) -> f64 {
    s.chars().count() as f64 * CHAR_W
}

#[allow(clippy::cast_precision_loss)]
fn mono_width(s: &str) -> f64 {
    s.chars().count() as f64 * MONO_W
}

/// Opening `<figure>` and `<svg>` for a chart `w` by `h` user units wide.
fn open(out: &mut String, w: f64, h: f64, label: &str) {
    let _ = writeln!(
        out,
        "<figure class=\"malc\"><svg viewBox=\"0 0 {w:.0} {h:.0}\" style=\"max-width:{w:.0}px;min-width:{:.0}px\" role=\"img\" xmlns=\"http://www.w3.org/2000/svg\"><title>{}</title>",
        w * 0.8,
        esc(label)
    );
}

/// One summary bar: an implementation's geometric-mean ratio to the baseline.
struct Bar<'a> {
    name: &'a str,
    ratio: f64,
    workloads: usize,
}

/// The geometric-mean ratio of every implementation that shares at least one
/// workload with the baseline, fastest first.
#[allow(clippy::cast_precision_loss)]
fn bars<'a>(others: &[&'a str], measured: &[&Benchmark], baseline: &[&Benchmark]) -> Vec<Bar<'a>> {
    let mut rows: Vec<Bar<'a>> = others
        .iter()
        .filter_map(|name| {
            let logs: Vec<f64> = measured
                .iter()
                .filter(|b| b.implementation == *name)
                .filter_map(|b| ratio(b, baseline))
                .map(f64::log2)
                .collect();
            (!logs.is_empty()).then(|| Bar {
                name,
                ratio: (logs.iter().sum::<f64>() / logs.len() as f64).exp2(),
                workloads: logs.len(),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.ratio.total_cmp(&b.ratio).then(a.name.cmp(b.name)));
    rows
}

/// SVG path for a bar from `base` to `end` (x) between `top` and `bottom`
/// (y): square at the baseline, 4px rounded at the data end.
fn bar_path(base: f64, end: f64, top: f64, bottom: f64) -> String {
    let rad = 4.0_f64.min((end - base).abs());
    let dir = if end >= base { 1.0 } else { -1.0 };
    let inner = end - dir * rad;
    format!(
        "M{base:.1},{top:.1}H{inner:.1}Q{end:.1},{top:.1} {end:.1},{:.1}V{:.1}Q{end:.1},{bottom:.1} {inner:.1},{bottom:.1}H{base:.1}Z",
        top + rad,
        bottom - rad
    )
}

/// Gridlines from `top` to `bottom` with tick labels at powers of two from
/// `2^lo` to `2^hi`; the 1x line is drawn as the axis.
fn log2_axis(out: &mut String, lo: i32, hi: i32, to_x: &dyn Fn(f64) -> f64, top: f64, bottom: f64) {
    let step = if hi - lo > 8 { 2 } else { 1 };
    let mut k = lo;
    while k <= hi {
        let gx = to_x(f64::from(k));
        let class = if k == 0 { "axis" } else { "grid" };
        let _ = write!(
            out,
            "<line class=\"{class}\" x1=\"{gx:.1}\" y1=\"{top:.1}\" x2=\"{gx:.1}\" y2=\"{bottom:.1}\"/>\
             <text class=\"m\" x=\"{gx:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            bottom + 16.0,
            pow2_label(k)
        );
        k += step;
    }
    out.push('\n');
}

/// Horizontal bars of the geometric-mean ratio per implementation, sorted
/// fastest first, diverging from the baseline at 1x on a log2 axis.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn summary(
    out: &mut String,
    title: &str,
    others: &[&str],
    measured: &[&Benchmark],
    baseline: &[&Benchmark],
) {
    let rows = bars(others, measured, baseline);
    if rows.is_empty() {
        return;
    }
    let lo = rows
        .iter()
        .map(|r| r.ratio.log2().floor() as i32)
        .fold(-1, i32::min);
    let hi = rows
        .iter()
        .map(|r| r.ratio.log2().ceil() as i32)
        .fold(1, i32::max);

    let label_w = rows
        .iter()
        .map(|r| text_width(r.name))
        .fold(text_width("std (baseline)"), f64::max)
        + 16.0;
    // Room for a value label beyond either end of the axis.
    let value_w = 52.0;
    let width = 680.0;
    let plot_left = label_w + value_w;
    let plot_w = width - plot_left - value_w;
    let row_h = 26.0;
    let top = 8.0;
    let axis_y = top + row_h * (rows.len() + 1) as f64 + 4.0;
    let height = axis_y + 24.0;
    let to_x = |log2: f64| plot_left + (log2 - f64::from(lo)) / f64::from(hi - lo) * plot_w;
    let one = to_x(0.0);
    let mid = |i: usize| top + row_h * i as f64 + row_h / 2.0;

    open(
        out,
        width,
        height,
        &format!("{title}: geometric mean time relative to {BASELINE}, one bar per implementation"),
    );
    log2_axis(out, lo, hi, &to_x, top, axis_y);

    // The baseline itself, as a dot on the 1x line.
    let _ = writeln!(
        out,
        "<g><title>{BASELINE}: the baseline, 1× by definition</title>\
         <text class=\"t\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{BASELINE} (baseline)</text>\
         <circle class=\"dot\" cx=\"{one:.1}\" cy=\"{:.1}\" r=\"5\"/></g>",
        label_w - 10.0,
        mid(0) + 4.0,
        mid(0)
    );
    let bar_h = 16.0;
    for (i, bar) in rows.iter().enumerate() {
        let row_y = mid(i + 1);
        let end = to_x(bar.ratio.log2());
        let faster = bar.ratio < 1.0;
        let (class, word, factor) = if faster {
            ("bf", "faster", 1.0 / bar.ratio)
        } else {
            ("bs", "slower", bar.ratio)
        };
        let tip = format!(
            "{}: {} the time of {BASELINE} ({} {word}), geometric mean over {} workload{}",
            bar.name,
            times(bar.ratio),
            times(factor),
            bar.workloads,
            if bar.workloads == 1 { "" } else { "s" }
        );
        let _ = write!(
            out,
            "<g><title>{}</title><text class=\"t\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
            esc(&tip),
            label_w - 10.0,
            row_y + 4.0,
            esc(bar.name)
        );
        let (bar_top, bar_bottom) = (row_y - bar_h / 2.0, row_y + bar_h / 2.0);
        let _ = write!(
            out,
            "<path class=\"{class}\" d=\"{}\"/>",
            bar_path(one, end, bar_top, bar_bottom)
        );
        // Value at the bar tip, outside the bar.
        let (tx, anchor) = if faster {
            (end - 6.0, "end")
        } else {
            (end + 6.0, "start")
        };
        let _ = writeln!(
            out,
            "<text class=\"t\" x=\"{tx:.1}\" y=\"{:.1}\" text-anchor=\"{anchor}\">{}</text></g>",
            row_y + 4.0,
            times(bar.ratio)
        );
    }
    let _ = writeln!(
        out,
        "</svg><figcaption>Mean time relative to <code>{BASELINE}</code>, as the geometric mean \
         over the workloads both rows measured (log scale). Left of 1× is faster, right is slower. \
         Hover a bar for the details.</figcaption></figure>"
    );
}

/// Geometry of one heatmap cell.
struct Cell {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// One heatmap cell: `b` is the measurement, if there is one.
fn heat_cell(
    out: &mut String,
    cell: &Cell,
    name: &str,
    workload: &str,
    b: Option<&Benchmark>,
    baseline: &[&Benchmark],
) {
    let gap = 2.0;
    let rect = format!(
        "x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"3\"",
        cell.x + gap / 2.0,
        cell.y + gap / 2.0,
        cell.w - gap,
        cell.h - gap
    );
    let at = format!(
        "x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\"",
        cell.x + cell.w / 2.0,
        cell.y + cell.h / 2.0 + 4.0
    );
    let Some(b) = b else {
        let _ = write!(
            out,
            "<g><title>{}: {} not measured</title><rect class=\"none\" {rect}/><text class=\"m\" {at}>–</text></g>",
            esc(name),
            esc(workload)
        );
        return;
    };
    let time = format!("{} ± {}", duration(b.mean_ns), duration(b.std_dev_ns));
    let is_baseline = name == BASELINE;
    let r = if is_baseline {
        None
    } else {
        ratio(b, baseline)
    };
    let (class, label, tip) = match r {
        Some(r) => (
            BUCKETS[bucket(r)].1,
            times(r),
            format!(
                "{name} · {workload}: {time}, {} the time of {BASELINE}",
                times(r)
            ),
        ),
        None if is_baseline => (
            "c0",
            duration(b.mean_ns),
            format!("{name} · {workload}: {time} (the baseline)"),
        ),
        None => (
            "c0",
            duration(b.mean_ns),
            format!("{name} · {workload}: {time}; {BASELINE} has no measurement to compare with"),
        ),
    };
    let ink = if matches!(class, "f3" | "f4" | "s3" | "s4") {
        "w"
    } else {
        "t"
    };
    let style = if is_baseline {
        " font-style=\"italic\""
    } else {
        ""
    };
    let _ = write!(
        out,
        "<g><title>{}</title><rect class=\"{class}\" {rect}/><text class=\"{ink}\" {at}{style}>{}</text></g>",
        esc(&tip),
        esc(&label)
    );
}

/// One row per workload, one column per measured implementation; each cell
/// is coloured by its ratio to the baseline on the same workload.
#[allow(clippy::cast_precision_loss)]
fn heatmap(
    out: &mut String,
    title: &str,
    order: &[&str],
    measured: &[&Benchmark],
    baseline: &[&Benchmark],
) {
    // Baseline first, then the table's order.
    let mut cols: Vec<&str> = vec![BASELINE];
    cols.extend(
        order
            .iter()
            .copied()
            .filter(|n| *n != BASELINE && measured.iter().any(|b| b.implementation == *n)),
    );
    let mut workloads: Vec<&str> = Vec::new();
    for b in measured {
        if !workloads.contains(&b.workload.as_str()) {
            workloads.push(&b.workload);
        }
    }
    workloads.sort_unstable();

    let label_w = workloads.iter().map(|w| mono_width(w)).fold(0.0, f64::max) + 16.0;
    let col_w = cols
        .iter()
        .map(|c| text_width(c) + 10.0)
        .fold(text_width("1.58 µs") + 14.0, f64::max);
    let cell_h = 24.0;
    let head_h = 22.0;
    let legend_top = head_h + cell_h * workloads.len() as f64 + 14.0;
    let width = (label_w + col_w * cols.len() as f64).max(BUCKETS.len() as f64 * 66.0 + 8.0);
    let height = legend_top + 40.0;

    open(
        out,
        width,
        height,
        &format!("{title}: time of each implementation relative to {BASELINE}, per workload"),
    );
    for (j, name) in cols.iter().enumerate() {
        let _ = write!(
            out,
            "<text class=\"t\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-weight=\"600\">{}</text>",
            label_w + col_w * j as f64 + col_w / 2.0,
            head_h - 8.0,
            esc(name)
        );
    }
    out.push('\n');
    for (i, workload) in workloads.iter().enumerate() {
        let row_y = head_h + cell_h * i as f64;
        let _ = write!(
            out,
            "<text class=\"t\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" font-family=\"monospace\">{}</text>",
            label_w - 10.0,
            row_y + cell_h / 2.0 + 4.0,
            esc(workload)
        );
        for (j, name) in cols.iter().enumerate() {
            let cell = Cell {
                x: label_w + col_w * j as f64,
                y: row_y,
                w: col_w,
                h: cell_h,
            };
            let b = measured
                .iter()
                .copied()
                .find(|b| b.implementation == *name && b.workload == *workload);
            heat_cell(out, &cell, name, workload, b, baseline);
        }
        out.push('\n');
    }
    legend(out, width, legend_top);
    let _ = writeln!(
        out,
        "\n</svg><figcaption>Each cell is that implementation's mean time divided by \
         <code>{BASELINE}</code>'s on the same workload; the <code>{BASELINE}</code> column shows \
         its own time. Blue is faster, red slower, grey within 25%. Hover a cell for the time and \
         standard deviation; the table below has every value.</figcaption></figure>"
    );
}

/// The heatmap's colour key: the nine buckets, faster to slower.
#[allow(clippy::cast_precision_loss)]
fn legend(out: &mut String, width: f64, top: f64) {
    let sw = (width - 8.0) / BUCKETS.len() as f64;
    for (k, (_, class, label)) in BUCKETS.iter().enumerate() {
        let lx = 4.0 + sw * k as f64;
        let _ = write!(
            out,
            "<rect class=\"{class}\" x=\"{:.1}\" y=\"{top:.1}\" width=\"{:.1}\" height=\"10\" rx=\"2\"/>\
             <text class=\"m\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" font-size=\"11\">{}</text>",
            lx + 1.0,
            sw - 2.0,
            lx + sw / 2.0,
            top + 25.0,
            esc(label)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench(implementation: &str, workload: &str, mean: f64) -> Benchmark {
        Benchmark {
            id: format!("reader/{implementation}/{workload}"),
            component: "reader".into(),
            implementation: implementation.into(),
            workload: workload.into(),
            mean_ns: mean,
            median_ns: mean,
            std_dev_ns: 1.0,
            throughput: None,
        }
    }

    #[test]
    fn formats_ratios_and_ticks() {
        assert_eq!(times(0.0123), "0.012×");
        assert_eq!(times(0.52), "0.52×");
        assert_eq!(times(1.0), "1.0×");
        assert_eq!(times(5.75), "5.8×");
        assert_eq!(times(14.2), "14×");
        assert_eq!(pow2_label(-3), "1/8×");
        assert_eq!(pow2_label(0), "1×");
        assert_eq!(pow2_label(4), "16×");
        assert_eq!(BUCKETS[bucket(1.0)].1, "c0");
        assert_eq!(BUCKETS[bucket(0.8)].1, "c0");
        assert_eq!(BUCKETS[bucket(0.79)].1, "f1");
        assert_eq!(BUCKETS[bucket(3.0)].1, "s2");
        assert_eq!(BUCKETS[bucket(100.0)].1, "s4");
        assert_eq!(BUCKETS[bucket(0.01)].1, "f4");
    }

    #[test]
    fn needs_a_baseline_and_a_second_row() {
        let std = bench("std", "atom", 10.0);
        let nom = bench("nom", "atom", 40.0);
        assert!(render("Reader", &["std", "nom"], &[&std]).is_none());
        assert!(render("Reader", &["std", "nom"], &[&nom]).is_none());
        assert!(render("Reader", &["std", "nom"], &[&std, &nom]).is_some());
    }

    #[test]
    fn draws_ratios_against_the_baseline() {
        let rows = [
            bench("std", "atom", 10.0),
            bench("std", "list", 100.0),
            bench("nom", "atom", 40.0),
            bench("nom", "list", 100.0),
            bench("peg", "atom", 5.0),
        ];
        let refs: Vec<&Benchmark> = rows.iter().collect();
        let svg = render("Reader", &["std", "nom", "peg"], &refs).unwrap();
        // Geometric mean of 4x and 1x is 2x.
        assert!(svg.contains(">2.0×</text>"));
        assert!(svg.contains("geometric mean over 2 workloads"));
        assert!(svg.contains("geometric mean over 1 workload<"));
        // Heatmap cells: ratio class and label, baseline shows its time.
        assert!(svg.contains("class=\"s2\""));
        assert!(svg.contains(">4.0×</text>"));
        assert!(svg.contains(">0.50×</text>"));
        assert!(svg.contains(">10.0 ns</text>"));
        // peg has no list measurement.
        assert!(svg.contains("peg: list not measured"));
        // Each figure is one HTML block: no blank lines inside one.
        assert_eq!(svg.matches("<figure").count(), 2);
        for figure in svg.split("</figure>") {
            assert!(!figure.trim().contains("\n\n"));
        }
    }
}
