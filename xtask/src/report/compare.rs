//! `xtask pr-numbers`: the report's numbers on a pull request beside its
//! base's, as the comment CI keeps on the pull request.
//!
//! ```text
//!  base report.json ─┐
//!                    ├─ metrics() ─ rows ─ markdown ─ comment on the PR
//!  head report.json ─┘
//! ```
//!
//! Every number is a share (part of whole) or a factor (a median speedup).
//! A row shows the head's value, its absolute change and its change in
//! percentage points; a number the base lacks shows only the head's.

use std::error::Error;
use std::fs;
use std::path::Path;

use super::html::share;
use super::suites::median_speedup;
use super::{Report, SuiteStatus};

/// The first line of the comment, which finds it again to edit in place.
pub const MARKER: &str = "<!-- music21-rs-numbers -->";

/// A change this small in percentage points reads as none.
const STEADY: f64 = 0.005;

/// One number the report carries.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    /// `part` of `whole`, as a percentage.
    Share { part: u64, whole: u64 },
    /// A ratio, such as a median speedup.
    Factor(f64),
}

/// A named number, in the order the comment lists them.
#[derive(Debug, Clone, PartialEq)]
struct Metric {
    name: String,
    value: Value,
}

fn share_of(name: &str, part: usize, whole: usize) -> Metric {
    Metric {
        name: name.to_string(),
        value: Value::Share {
            part: part as u64,
            whole: whole as u64,
        },
    }
}

/// Every number worth watching on a pull request, from one report.
fn metrics(report: &Report) -> Vec<Metric> {
    let mut found = Vec::new();

    // Coverage: only a report built with instrumentation has it.
    if let Some(coverage) = &report.coverage {
        for (name, percent) in [
            ("Coverage: lines", coverage.lines),
            ("Coverage: functions", coverage.functions),
            ("Coverage: regions", coverage.regions),
        ] {
            found.push(Metric {
                name: name.to_string(),
                value: Value::Share {
                    part: percent.covered,
                    whole: percent.count,
                },
            });
        }
    }

    // How much of music21's surface is ported.
    let members: usize = report.features.iter().map(|c| c.members.len()).sum();
    let ported: usize = report.features.iter().map(|c| c.ported).sum();
    let in_wheel: usize = report.features.iter().map(|c| c.in_wheel).sum();
    found.push(share_of("music21 members in the crate", ported, members));
    found.push(share_of("music21 members in the wheel", in_wheel, members));

    // music21's own docstrings, run against the port.
    let docstrings: usize = report.doctests.iter().map(|m| m.docstrings).sum();
    let docstrings_passing: usize = report.doctests.iter().map(|m| m.docstrings_passing).sum();
    let examples: usize = report.doctests.iter().map(|m| m.examples).sum();
    let examples_passing: usize = report.doctests.iter().map(|m| m.examples_passing).sum();
    found.push(share_of(
        "music21 docstrings passing",
        docstrings_passing,
        docstrings,
    ));
    found.push(share_of(
        "music21 doctest examples passing",
        examples_passing,
        examples,
    ));

    // music21's suite with only music21-rs installed: the finish line.
    if let Some(provided) = &report.provided {
        found.push(share_of(
            "music21's suite on music21-rs alone",
            provided.passing,
            provided.music21_passing,
        ));
        found.push(share_of(
            "music21 modules importing on music21-rs alone",
            provided.modules_imported,
            provided.modules,
        ));
    }

    // Each test suite that ran, then all of them together.
    let mut passed = 0;
    let mut total = 0;
    for suite in &report.suites {
        if matches!(suite.status, SuiteStatus::Skipped) {
            continue;
        }
        let ran = suite.passed + suite.failed;
        if ran == 0 {
            continue;
        }
        passed += suite.passed;
        total += ran;
        found.push(share_of(
            &format!("Suite: {}", suite.name),
            suite.passed,
            ran,
        ));
    }
    found.push(share_of("Tests passing, all suites", passed, total));

    // Speed against music21 through the same Python calls.
    if let Some(benchmarks) = &report.benchmarks
        && !benchmarks.cases.is_empty()
    {
        found.push(Metric {
            name: "Median speedup over music21".to_string(),
            value: Value::Factor(median_speedup(&benchmarks.cases)),
        });
    }
    found
}

/// A percentage of a share, nought for an empty whole.
fn percent(part: u64, whole: u64) -> f64 {
    share(part as usize, whole as usize)
}

/// A signed change with an arrow: up, down or steady.
fn signed(change: f64, digits: usize, suffix: &str) -> String {
    if change.abs() < STEADY {
        return format!("±0{suffix}");
    }
    let arrow = if change > 0.0 { "▲" } else { "▼" };
    format!("{arrow} {change:+.digits$}{suffix}")
}

/// One table row: the head's value, then its change from the base.
fn row(head: &Metric, base: Option<&Metric>) -> String {
    match (&head.value, base.map(|b| &b.value)) {
        (
            Value::Share { part, whole },
            Some(Value::Share {
                part: was,
                whole: of,
            }),
        ) => {
            let now = percent(*part, *whole);
            let before = percent(*was, *of);
            let absolute = *part as i64 - *was as i64;
            format!(
                "| {} | {now:.2}% ({part} of {whole}) | {before:.2}% ({was} of {of}) | {} | {} |",
                head.name,
                signed(absolute as f64, 0, ""),
                signed(now - before, 2, " pp"),
            )
        }
        (Value::Share { part, whole }, _) => format!(
            "| {} | {:.2}% ({part} of {whole}) | — | new | |",
            head.name,
            percent(*part, *whole),
        ),
        (Value::Factor(now), Some(Value::Factor(before))) => format!(
            "| {} | {now:.2}× | {before:.2}× | {} | |",
            head.name,
            signed(now - before, 2, "×"),
        ),
        (Value::Factor(now), _) => {
            format!("| {} | {now:.2}× | — | new | |", head.name)
        }
    }
}

/// The whole comment: a headline, then a table of every number.
fn comment(head: &Report, base: Option<&Report>) -> String {
    let head_metrics = metrics(head);
    let base_metrics = base.map(metrics).unwrap_or_default();
    let find = |name: &str| base_metrics.iter().find(|m| m.name == name);

    let mut out = vec![MARKER.to_string(), "## music21-rs numbers".to_string()];

    // The headline, as Coveralls puts it: the finish line and coverage.
    for name in ["music21's suite on music21-rs alone", "Coverage: lines"] {
        let Some(metric) = head_metrics.iter().find(|m| m.name == name) else {
            continue;
        };
        let Value::Share { part, whole } = metric.value else {
            continue;
        };
        let now = percent(part, whole);
        let line = match find(name).map(|m| &m.value) {
            Some(Value::Share {
                part: was,
                whole: of,
            }) => format!(
                "**{name}: {now:.2}%** ({} pp from {:.2}%)",
                signed(now - percent(*was, *of), 2, ""),
                percent(*was, *of)
            ),
            _ => format!("**{name}: {now:.2}%**"),
        };
        out.push(line);
        out.push(String::new());
    }

    // Which commits are compared.
    match base {
        Some(base) => out.push(format!(
            "`{}` against base `{}`.",
            head.generated_from, base.generated_from
        )),
        None => out.push(format!(
            "`{}`; no base report was found to compare with.",
            head.generated_from
        )),
    }
    out.push(String::new());

    out.push("| | head | base | change | change (pp) |".to_string());
    out.push("|---|---|---|---|---|".to_string());
    for metric in &head_metrics {
        out.push(row(metric, find(&metric.name)));
    }

    // A number the base had and the head lost is a change too.
    for gone in base_metrics
        .iter()
        .filter(|m| !head_metrics.iter().any(|h| h.name == m.name))
    {
        out.push(format!("| {} | — | | gone | |", gone.name));
    }
    out.push(String::new());
    out.join("\n")
}

/// Writes the comment comparing `head` with `base`, if there is a base.
pub(crate) fn run(base: Option<&Path>, head: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
    let head: Report = serde_json::from_str(&fs::read_to_string(head)?)?;

    // A missing or unreadable base still yields the head's numbers.
    let base: Option<Report> = base
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok());

    fs::write(out, comment(&head, base.as_ref()))?;
    println!("wrote {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report with only the gate and one suite in it.
    fn report(commit: &str, passing: usize, suite_passed: usize) -> Report {
        serde_json::from_value(serde_json::json!({
            "generated_from": commit,
            "music21_version": "11",
            "suites": [{
                "name": "parity", "command": "", "status": "passed",
                "passed": suite_passed, "failed": 10 - suite_passed,
            }],
            "doctests": [],
            "features": [],
            "provided": {
                "music21_passing": 200, "passing": passing,
                "modules": 10, "modules_imported": 5,
            },
        }))
        .unwrap()
    }

    #[test]
    fn a_rise_shows_its_count_and_points() {
        let text = comment(&report("head", 150, 9), Some(&report("base", 100, 9)));
        assert!(text.starts_with(MARKER));
        assert!(
            text.contains(
                "| music21's suite on music21-rs alone | 75.00% (150 of 200) | \
                 50.00% (100 of 200) | ▲ +50 | ▲ +25.00 pp |"
            ),
            "{text}"
        );
        assert!(
            text.contains("| Suite: parity | 90.00% (9 of 10) | 90.00% (9 of 10) | ±0 | ±0 pp |")
        );
    }

    /// A suite that ran no tests says nothing, and gets no row.
    #[test]
    fn a_suite_that_ran_nothing_is_left_out() {
        let mut head = report("head", 100, 9);
        head.suites[0].passed = 0;
        head.suites[0].failed = 0;
        let text = comment(&head, None);
        assert!(!text.contains("Suite: parity"), "{text}");
    }

    #[test]
    fn a_fall_points_down() {
        let text = comment(&report("head", 100, 8), Some(&report("base", 100, 9)));
        assert!(text.contains("| ▼ -1 | ▼ -10.00 pp |"), "{text}");
    }

    #[test]
    fn without_a_base_only_the_head_shows() {
        let text = comment(&report("head", 100, 9), None);
        assert!(text.contains("no base report was found"));
        assert!(text.contains("| Suite: parity | 90.00% (9 of 10) | — | new | |"));
    }

    #[test]
    fn a_number_the_head_lost_is_listed() {
        let mut head = report("head", 100, 9);
        head.provided = None;
        let text = comment(&head, Some(&report("base", 100, 9)));
        assert!(text.contains("| music21's suite on music21-rs alone | — | | gone | |"));
    }
}
