//! Rendering the report as one self-contained page.
//!
//! The layout lives in `report.css` and `report.js` beside this
//! module and is inlined here, so the page is one file; the colours
//! come from the site's shared theme, which it links.

use super::*;

/// Escapes prose for the page, and lets a `backtick` span through as `<code>`.
///
/// The suite notes are written as sentences with the odd command or symbol in
/// them; writing the tags by hand in a Rust string literal would make them
/// unreadable at the point they are edited.
pub(super) fn prose(text: &str) -> String {
    let escaped = escape(text);
    let mut html = String::with_capacity(escaped.len());
    let mut open = false;
    for piece in escaped.split('`') {
        html.push_str(piece);
        html.push_str(if open { "</code>" } else { "<code>" });
        open = !open;
    }
    // The split leaves one tag too many; a note with unbalanced backticks
    // keeps its text and loses only the markup.
    let extra = if open { "<code>" } else { "</code>" };
    html.truncate(html.len() - extra.len());
    html
}

pub(super) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The page's own layout, inlined so the report is one self-contained file.
/// Colours come from the site theme it links to.
pub(super) const STYLE: &str = include_str!("../report.css");

/// Filtering the ported list and highlighting the section in view. The page
/// is complete without it.
pub(super) const SCRIPT: &str = include_str!("../report.js");

/// `1 suite`, `3 suites` — the report says these counts out loud often enough
/// to be worth getting right, and one of the nouns is `class`.
pub(super) fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("{count} {one}")
    } else {
        format!("{count} {many}")
    }
}

pub(super) fn share(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

/// A progress bar. `warn` paints it in the warning colour, for a figure that
/// is a shortfall rather than an achievement.
pub(super) fn meter(percent: f64, warn: bool, extra_class: &str) -> String {
    let warn = if warn { " is-warn" } else { "" };
    format!(
        "<div class=\"meter{warn}{extra}\"><span style=\"width: {percent:.1}%\"></span></div>",
        extra = if extra_class.is_empty() {
            String::new()
        } else {
            format!(" {extra_class}")
        },
    )
}

/// The bar in two parts: what is ported and what is not. Every member
/// music21 has counts against the whole.
pub(super) fn stacked_meter(ported: usize, missing: usize) -> String {
    let total = ported + missing;
    let mut html = String::from("<div class=\"meter is-stacked\">");
    for (class, part) in [("is-ported", ported), ("is-missing", missing)] {
        if part > 0 {
            let _ = write!(
                html,
                "<span class=\"{class}\" style=\"width: {:.2}%\"></span>",
                share(part, total)
            );
        }
    }
    html.push_str("</div>");
    html
}

/// One headline figure at the top of the page, linking to the section it
/// summarises.
struct Score {
    anchor: &'static str,
    label: &'static str,
    value: String,
    sub: String,
    /// The bar under the figure, already rendered; empty for a figure that is
    /// not a proportion.
    bar: String,
    warn: bool,
}

impl Score {
    fn render(&self) -> String {
        format!(
            r##"                <a class="score{warn}" href="#{anchor}">
                    <span class="label">{label}</span>
                    <span class="value">{value}</span>
                    <span class="sub">{sub}</span>
                    {bar}
                </a>
"##,
            warn = if self.warn { " is-warn" } else { "" },
            anchor = self.anchor,
            label = self.label,
            value = self.value,
            sub = self.sub,
            bar = self.bar,
        )
    }
}

/// The head of a `<section>`: its title, and the one line that says what the
/// numbers in it are counting.
pub(super) fn section_head(anchor: &str, title: &str, note: &str) -> String {
    format!(
        r#"            <section id="{anchor}">
                <div class="section-head">
                    <h2>{title}</h2>
                    <p class="head-note">{note}</p>
                </div>
"#
    )
}

pub(super) fn render_coverage(coverage: &Coverage) -> String {
    let mut html = section_head(
        "coverage",
        "Test coverage",
        "of music21-rs, across every suite below",
    );
    html.push_str("                <div class=\"section-body\">\n                    <div class=\"coverage-grid\">\n");
    for (label, percent) in [
        ("Lines", coverage.lines),
        ("Functions", coverage.functions),
        ("Regions", coverage.regions),
    ] {
        let _ = write!(
            html,
            r#"                        <div class="coverage-row">
                            <span class="label">{label}</span>
                            {bar}
                            <span class="figure">{value:.1}%<small>{covered} of {count}</small></span>
                        </div>
"#,
            bar = meter(percent.percent, false, ""),
            value = percent.percent,
            covered = percent.covered,
            count = percent.count,
        );
    }
    html.push_str("                    </div>\n                </div>\n");
    html.push_str(
        "                <p class=\"section-foot\">Every suite above is run instrumented and the profiles merged, except those going through the installed wheel. Generated tables and the tooling crates are excluded. <a href=\"./coverage/html/index.html\">Read it file by file</a>.</p>\n            </section>\n",
    );
    html
}

pub(super) fn render_suites(suites: &[Suite]) -> String {
    let passed: usize = suites.iter().map(|s| s.passed).sum();
    let failed: usize = suites.iter().map(|s| s.failed).sum();
    let skipped = suites
        .iter()
        .filter(|s| s.status == SuiteStatus::Skipped)
        .count();
    let note = if failed > 0 {
        format!("{failed} failing, {passed} passing")
    } else if skipped > 0 {
        format!(
            "{passed} passing, {} not run here",
            plural(skipped, "suite", "suites")
        )
    } else {
        format!("{passed} passing, all green")
    };
    let mut html = section_head("suites", "music21-rs's own tests", &escape(&note));
    html.push_str(
        r#"                <div class="table-wrap">
                    <table>
                        <thead><tr><th>Suite</th><th>Result</th><th class="num">Passed</th><th class="num">Failed</th><th>Command</th></tr></thead>
                        <tbody>
"#,
    );
    for suite in suites {
        let (pill, label) = match suite.status {
            SuiteStatus::Passed => ("pill good", "passed"),
            SuiteStatus::Failed => ("pill bad", "failed"),
            SuiteStatus::Skipped => ("pill", "skipped"),
        };
        let detail = match (&suite.detail, suite.status) {
            (Some(detail), SuiteStatus::Skipped) => {
                format!("<span class=\"detail\">{}</span>", escape(detail))
            }
            (Some(detail), _) => format!(
                "<span class=\"detail\"><code>{}</code></span>",
                escape(detail)
            ),
            (None, _) => String::new(),
        };
        let count = |n: usize| {
            if suite.status == SuiteStatus::Skipped || suite.passed + suite.failed == 0 {
                "—".to_string()
            } else {
                n.to_string()
            }
        };
        let note = match &suite.note {
            Some(note) => format!("<span class=\"detail\">{}</span>", prose(note)),
            None => String::new(),
        };
        let _ = write!(
            html,
            r#"                            <tr>
                                <td class="name">{name}{note}{detail}</td>
                                <td><span class="{pill}">{label}</span></td>
                                <td class="num">{passed}</td>
                                <td class="num">{failed}</td>
                                <td><code>{command}</code></td>
                            </tr>
"#,
            name = escape(&suite.name),
            passed = count(suite.passed),
            failed = count(suite.failed),
            command = escape(&suite.command),
        );
    }
    html.push_str(
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    html.push_str(
        "                <p class=\"section-foot\">A suite whose tooling is not installed is skipped, with the reason, rather than failed.</p>\n            </section>\n",
    );
    html
}

/// Bytes, as a person reads them.
pub(super) fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// What each of the three costs to install, biggest first, as bars against
/// the largest.
pub(super) fn render_sizes(sizes: &Sizes) -> String {
    let rows = [
        (
            "music21",
            "installed package, corpus and all",
            sizes.music21,
        ),
        (
            "music21-rs",
            "the source cargo publishes",
            sizes.crate_source,
        ),
        ("music21-rs wheel", "what pip installs", sizes.wheel),
    ];
    let largest = rows.iter().filter_map(|(_, _, size)| *size).max();
    let Some(largest) = largest.filter(|largest| *largest > 0) else {
        return String::new();
    };

    let mut html = String::from(
        "                <div class=\"section-body\">
                    <div class=\"coverage-grid\">
",
    );
    for (name, note, size) in rows {
        let Some(size) = size else { continue };
        let percent = 100.0 * size as f64 / largest as f64;
        let _ = write!(
            html,
            r#"                        <div class="coverage-row">
                            <span class="label">{name}</span>
                            {bar}
                            <span class="figure">{value}<small>{note}</small></span>
                        </div>
"#,
            name = escape(name),
            bar = meter(percent, false, ""),
            value = human_bytes(size),
            note = escape(note),
        );
    }
    html.push_str(
        "                    </div>
                </div>
",
    );
    html
}

/// What the crate does that music21 has no counterpart for.
/// The derived half of the section: every public member of the crate that no
/// music21 member accounts for, one collapsed block per module.
///
/// Collapsed because there are several hundred of them; the summary line
/// carries the count, so the section reads as a set of totals until a module
/// is opened.
pub(super) fn render_beyond_members(modules: &[BeyondModule]) -> String {
    if modules.is_empty() {
        return String::new();
    }
    let (total, by_kind) = surface::totals(modules);
    let mut html = String::new();
    let kinds = by_kind
        .iter()
        .map(|(kind, count)| format!("{count} {}", kind.label()))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = write!(
        html,
        "                <h3 class=\"beyond-head\">{total} public members the map matches to nothing in music21<span class=\"detail\">{kinds}, across {modules} modules</span></h3>
                <div class=\"beyond-list\">
",
        modules = modules.len(),
        kinds = escape(&kinds),
    );
    for module in modules {
        let _ = write!(
            html,
            "                    <details class=\"beyond-item\">
                        <summary><span class=\"feature-name\">{name}</span><span class=\"of\">{count}</span></summary>
                        <ul class=\"member-list\">
",
            name = escape(&module.module),
            count = plural(module.members.len(), "member", "members"),
        );
        for member in &module.members {
            let _ = writeln!(
                html,
                "                            <li><code>{name}</code> <span class=\"of\">{kind}</span></li>",
                name = escape(&member.name),
                kind = member.kind.label(),
            );
        }
        html.push_str(
            "                        </ul>
                    </details>
",
        );
    }
    html.push_str(
        "                </div>
",
    );
    html
}

pub(super) fn render_beyond(beyond: &[BeyondReport], modules: &[BeyondModule]) -> String {
    let mut html = section_head(
        "beyond",
        "Beyond music21",
        &escape(&plural(beyond.len(), "capability", "capabilities")),
    );
    html.push_str(
        r#"                <div class="table-wrap">
                    <table>
                        <thead><tr><th>Capability</th><th>music21-rs</th><th>music21</th></tr></thead>
                        <tbody>
"#,
    );
    for entry in beyond {
        let unit = entry.unit.as_deref().unwrap_or("");
        let ours = match entry.count {
            Some(count) => format!("<b>{count}</b> {}", escape(unit)),
            None => "<span class=\"pill good\">present</span>".to_string(),
        };
        let theirs = match entry.music21 {
            Some(count) => format!("{count} {}", escape(unit)),
            None => "<span class=\"of\">nothing of the kind</span>".to_string(),
        };
        let _ = write!(
            html,
            r#"                            <tr>
                                <td class="name">{name}<span class="detail">{note}</span></td>
                                <td class="num">{ours}</td>
                                <td class="num">{theirs}</td>
                            </tr>
"#,
            name = escape(&entry.name),
            note = escape(&entry.note),
        );
    }
    html.push_str(
        "                        </tbody>
                    </table>
                </div>
",
    );
    html.push_str(&render_beyond_members(modules));
    html.push_str(
        "                <p class=\"section-foot\">Counts are read out of the crate's own tables, so the report fails when one it names has gone. The list below is derived: every public member of the crate, less every name music21 uses. It is bounded by what the map enumerates, so a member of a music21 class the map does not carry can appear here.</p>
            </section>
",
    );
    html
}

/// A second-per-test comparison drawn from music21's own suite.
///
/// Every case is the same test doing the same work, so there is nothing to
/// argue about in the pairing — but most of what a music21 test does is
/// music21's own code either way, which is why the middle sits near parity
/// and the tails are the part worth reading.
///
/// Three columns where all three ran: music21, the crate compiled from the
/// working tree, and the wheel out of site-packages. Only the tails are here;
/// [`TIMINGS_PAGE`] carries every paired test.
pub(super) fn render_timings(timings: &Timings) -> String {
    if timings.is_empty() {
        return String::new();
    }
    let mut html = String::new();
    let _ = writeln!(
        html,
        "                <h3 class=\"beyond-head\">{paired} of music21's own tests, timed on every side<span class=\"detail\">{totals}; {medians}</span></h3>",
        paired = timings.paired,
        totals = timings.totals(),
        medians = timings
            .sides
            .iter()
            .filter_map(|side| side
                .median_speedup
                .map(|median| format!("{} median {median:.2}&#215;", escape(&side.name))))
            .collect::<Vec<_>>()
            .join(", "),
    );
    html.push_str(&timing_table(
        timings,
        &[
            ("furthest ahead", &timings.fastest),
            ("furthest behind", &timings.slowest),
        ],
    ));
    let _ = writeln!(
        html,
        "                <p class=\"section-foot\">A test counts only where every side ran it, passed it, and took longer than a millisecond, and what the garbage collector took inside a test is taken off before the sides are compared &mdash; a full collection lands on whichever test is running, and each test is timed once. <em>{ours}</em> is the crate linked into the binary that ran the suite &mdash; the working tree &mdash; and <em>{wheel}</em> is the wheel a caller installs, so what lies between those two columns is the packaging rather than the code. <a href=\"./{page}\">All {paired} tests, side by side &rarr;</a></p>",
        ours = escape(timings.side_name(1)),
        wheel = escape(timings.side_name(2)),
        page = TIMINGS_PAGE,
        paired = timings.paired,
    );
    html
}

/// One table of timings, however many rows and however many columns.
///
/// Shared by the section and the page it links to, so the two cannot drift
/// apart on what a column means. A group with no label is drawn without a
/// heading row, which is how the whole list is written.
pub(super) fn timing_table(timings: &Timings, groups: &[(&str, &Vec<TestTiming>)]) -> String {
    let mut html = String::from(
        "                <div class=\"table-wrap\">
                    <table>
                        <thead><tr><th>Test</th>",
    );
    for side in &timings.sides {
        let _ = write!(html, "<th class=\"num\">{}</th>", escape(&side.name));
    }
    // One speedup column per subject, which is every column but music21's.
    for side in timings.sides.iter().skip(1) {
        let _ = write!(html, "<th class=\"num\">{} &#215;</th>", escape(&side.name));
    }
    html.push_str(
        "</tr></thead>
                        <tbody>
",
    );
    let columns = timings.sides.len() * 2;
    for (label, rows) in groups {
        if !label.is_empty() {
            let _ = writeln!(
                html,
                "                            <tr class=\"group-row\"><td colspan=\"{columns}\">{}</td></tr>",
                escape(label)
            );
        }
        for row in rows.iter() {
            let _ = write!(
                html,
                "                            <tr><td class=\"name\"><code>{name}</code></td>",
                name = escape(&row.name),
            );
            for taken in &row.seconds {
                let _ = write!(html, "<td class=\"num\">{}</td>", seconds(*taken));
            }
            for column in 1..timings.sides.len() {
                let cell = match row.speedup_of(column) {
                    Some(speedup) => {
                        let pill = if speedup >= 1.0 { "good" } else { "bad" };
                        format!("<span class=\"pill {pill}\">{speedup:.2}&#215;</span>")
                    }
                    None => "<span class=\"of\">&mdash;</span>".to_string(),
                };
                let _ = write!(html, "<td class=\"num\">{cell}</td>");
            }
            html.push_str("</tr>\n");
        }
    }
    html.push_str(
        "                        </tbody>
                    </table>
                </div>
",
    );
    html
}

/// The file the whole comparison is written to, beside the report.
pub(super) const TIMINGS_PAGE: &str = "timings.html";

/// Every paired test, on a page of its own.
///
/// Thousands of rows do not belong in a section read for its totals, but they
/// are the measurement: a reader who wants to know what one particular test
/// cost should be able to find out rather than take the tails on trust.
/// Ordered as the tails are, furthest behind first.
pub(super) fn render_timings_page(report: &Report, timings: &Timings) -> String {
    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en" class="no-js">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>music21-rs Test Timings</title>
        <link rel="stylesheet" href="../theme.css" />
        <style>
{style}        </style>
    </head>
    <body class="page-reports">
        <main class="shell">
            <header>
                <div class="title-row">
                    <a class="home-link" href="./">Reports</a>
                    <h1>Test timings</h1>
                </div>
                <div class="top-links">
                    <a href="../docs/music21_rs/index.html">Rust docs</a>
                    <a href="../python/">Python docs</a>
                </div>
            </header>
            <p class="report-meta">
                <span>commit <code>{head}</code></span>
                <span class="sep">/</span>
                <span>music21 <code>{version}</code></span>
            </p>
"#,
        style = STYLE,
        head = escape(&report.generated_from),
        version = escape(&report.music21_version),
    );
    html.push_str(&section_head(
        "timings",
        "Every test, timed on every side",
        &escape(&format!("{} paired", timings.paired)),
    ));
    let _ = writeln!(
        html,
        "                <p class=\"section-foot\">{totals}. A test is here only where every side ran it, passed it, and took longer than a millisecond, so this is fewer tests than the suite runs. What the garbage collector took inside a test is taken off first: a full collection lands on whichever test is running, and each test is timed once. Ordered by what the crate made of it, furthest behind first.</p>",
        totals = timings.totals(),
    );
    html.push_str(&timing_table(timings, &[("", &timings.rows)]));
    html.push_str(
        "            </section>
        </main>
        <script type=\"module\" src=\"../theme.js\"></script>
    </body>
</html>
",
    );
    html
}

/// A duration in the units the eye wants.
pub(super) fn seconds(value: f64) -> String {
    if value < 1.0 {
        format!("{:.0} ms", value * 1000.0)
    } else {
        format!("{value:.2} s")
    }
}

pub(super) fn render_benchmarks(
    benchmarks: Option<&Benchmarks>,
    sizes: Option<&Sizes>,
    timings: Option<&Timings>,
) -> String {
    // The two halves are measured by different commands. music21's own suite
    // may have been timed where the benchmark was never run, and the section
    // is worth having for either alone.
    let Some(benchmarks) = benchmarks else {
        let mut html = section_head(
            "speedups",
            "Speedups over music21",
            &escape("from music21's own test suite"),
        );
        if let Some(timings) = timings {
            html.push_str(&render_timings(timings));
        }
        html.push_str(
            "            </section>
",
        );
        return html;
    };
    if benchmarks.cases.is_empty() {
        let mut html = section_head(
            "speedups",
            "Speedups over music21",
            &escape(benchmarks.detail.as_deref().unwrap_or("not run")),
        );
        let _ = writeln!(
            html,
            "                <p class=\"section-foot\">The benchmarks time the crate against music21 through the same Python API, and need both installed in the interpreter that runs them: <code>{}</code>.</p>",
            escape(&benchmarks.command),
        );
        if let Some(timings) = timings {
            html.push_str(&render_timings(timings));
        }
        html.push_str(
            "            </section>
",
        );
        return html;
    }

    let median = median_speedup(&benchmarks.cases);
    let mut html = section_head(
        "speedups",
        "Speedups over music21",
        &escape(&format!(
            "{median:.1}\u{d7} median across {}",
            plural(benchmarks.cases.len(), "case", "cases")
        )),
    );
    html.push_str(
        r#"                <div class="table-wrap">
                    <table>
                        <thead><tr><th>Case</th><th class="num">music21</th><th class="num">crate</th><th class="num">wheel</th><th class="num">Speedup</th></tr></thead>
                        <tbody>
"#,
    );
    let mut group = "";
    for case in &benchmarks.cases {
        if case.group != group {
            group = &case.group;
            let _ = writeln!(
                html,
                "                            <tr class=\"group-row\"><td colspan=\"5\">{}</td></tr>",
                escape(group)
            );
        }
        let note = if case.notes.is_empty() {
            String::new()
        } else {
            format!("<span class=\"detail\">{}</span>", escape(&case.notes))
        };
        // A case where music21 is quicker is worth seeing as such.
        let pill = if case.speedup >= 1.0 { "good" } else { "bad" };
        let _ = write!(
            html,
            r#"                            <tr>
                                <td class="name">{name}{note}</td>
                                <td class="num">{slow}</td>
                                <td class="num">{native}</td>
                                <td class="num">{fast}</td>
                                <td class="num"><span class="pill {pill}">{speedup:.1}&#215;</span></td>
                            </tr>
"#,
            name = escape(&case.case),
            slow = humanise(case.music21_ns),
            native = case
                .music21_rs_native_ns
                .map_or_else(|| "<span class=\"of\">&mdash;</span>".to_string(), humanise),
            fast = humanise(case.music21_rs_ns),
            speedup = case.speedup,
        );
    }
    html.push_str(
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    // What each costs to install is the other half of what each costs to run.
    if let Some(sizes) = sizes {
        let block = render_sizes(sizes);
        if !block.is_empty() {
            html.push_str(
                "                <div class=\"sub-head\">What each costs to install</div>\n",
            );
            html.push_str(&block);
        }
    }
    let _ = writeln!(
        html,
        "                <p class=\"section-foot\">music21 {music21}, Python {python} ({platform}). Every side must agree on the answer before any of them is timed. <em>wheel</em> is what a caller installs and what the speedup is taken from; <em>crate</em> is the same operation in Rust with no Python in the way, so the gap between them is what the binding costs. Each case builds a fresh object, except those marked cached.</p>",
        music21 = escape(&benchmarks.music21),
        python = escape(&benchmarks.python),
        platform = escape(&benchmarks.platform),
    );
    if let Some(timings) = timings {
        html.push_str(&render_timings(timings));
    }
    html.push_str(
        "            </section>
",
    );
    html
}

pub(super) fn render_doctests(doctests: &[ModuleDoctests]) -> String {
    let docstrings_passing: usize = doctests.iter().map(|m| m.docstrings_passing).sum();
    let docstrings: usize = doctests.iter().map(|m| m.docstrings).sum();
    let examples_passing: usize = doctests.iter().map(|m| m.examples_passing).sum();
    let examples: usize = doctests.iter().map(|m| m.examples).sum();
    let tests_passing: usize = doctests.iter().map(|m| m.tests_passing).sum();
    let tests: usize = doctests.iter().map(|m| m.tests).sum();

    let unharnessed = doctests.iter().filter(|m| !m.harnessed).count();
    let mut note = format!(
        "music21-rs passes {examples_passing} of {examples} examples and {tests_passing} of {tests} unit tests, across {modules}",
        modules = plural(doctests.len(), "module", "modules")
    );
    if unharnessed > 0 {
        let _ = write!(note, ", {unharnessed} with no harness yet");
    }
    let mut html = section_head(
        "doctests",
        "music21-rs against music21's tests",
        &escape(&note),
    );
    html.push_str(
        r#"                <div class="table-wrap">
                    <table>
                        <thead><tr><th>Module</th><th>Docstrings</th><th>Examples</th><th>Unit tests</th></tr></thead>
                        <tbody>
"#,
    );
    let cell = |passing: usize, total: usize| {
        let percent = share(passing, total);
        format!(
            "<div class=\"progress-cell\"><span><b>{passing}</b> <span class=\"of\">of {total}</span></span>{bar}</div>",
            bar = meter(percent, passing < total, "is-slim"),
        )
    };
    for module in doctests {
        let _ = write!(
            html,
            r#"                            <tr>
                                <td class="name"><code>{module}</code>{note}</td>
                                <td>{docstrings}</td>
                                <td>{examples}</td>
                                <td>{tests}</td>
                            </tr>
"#,
            module = escape(&module.module),
            note = if module.harnessed {
                ""
            } else {
                "<span class=\"detail\">no harness yet</span>"
            },
            docstrings = cell(module.docstrings_passing, module.docstrings),
            examples = cell(module.examples_passing, module.examples),
            tests = cell(module.tests_passing, module.tests),
        );
    }
    let _ = write!(
        html,
        r#"                        </tbody>
                        <tfoot>
                            <tr>
                                <td class="name">every module</td>
                                <td>{docstrings}</td>
                                <td>{examples}</td>
                                <td>{tests}</td>
                            </tr>
                        </tfoot>
                    </table>
                </div>
"#,
        docstrings = cell(docstrings_passing, docstrings),
        examples = cell(examples_passing, examples),
        tests = cell(tests_passing, tests),
    );
    html.push_str(
        "                <p class=\"section-foot\">music21's own docstrings and unit tests, run against the crate. A docstring passes only when every one of its examples does. The unit-test column is the <em>music21's own test suite</em> row above, attributed per module; a test music21 itself fails counts here as not passing.</p>\n            </section>\n",
    );
    html
}

pub(super) fn render_features(features: &[ClassReport], wheel: bool) -> String {
    let ported: usize = features.iter().map(|c| c.ported).sum();
    let in_wheel: usize = features.iter().map(|c| c.in_wheel).sum();
    let missing: usize = features.iter().map(|c| c.missing).sum();
    let total = ported + missing;

    // The legend doubles as the summary: it names each part of the bar and
    // gives its count.
    let mut note = format!(
        "<span class=\"legend\">{members}: <span class=\"ported\">{ported} ported</span>",
        members = plural(total, "member", "members"),
    );
    if missing > 0 {
        let _ = write!(
            note,
            "<span class=\"missing\">{missing} still to port</span>"
        );
    }
    if wheel {
        let _ = write!(note, "<span class=\"wheel\">{in_wheel} in the wheel</span>");
        // The two surfaces are not one: a member the wheel answers and the
        // crate has no function for is why "the crate is a superset of the
        // wheel" is not a thing anybody should assume from this page.
        let wheel_only = features
            .iter()
            .flat_map(|class| &class.members)
            .filter(|member| member.status != Status::Ported && member.in_wheel)
            .count();
        if wheel_only > 0 {
            let _ = write!(
                note,
                "<span class=\"wheel\">{wheel_only} in the wheel only</span>"
            );
        }
    }
    note.push_str("</span>");
    let mut html = section_head("ported", "Ported from music21", &note);

    let _ = write!(
        html,
        r#"                <div class="toolbar">
                    <input type="search" data-filter-search placeholder="Filter by class, file or member name" aria-label="Filter the ported list" />
                    <label class="check"><input type="checkbox" data-filter-incomplete /> Only incomplete</label>
                    <button type="button" class="button-link" data-expand-all>Expand all</button>
                    <button type="button" class="button-link" data-collapse-all>Collapse all</button>
                    <span class="tally" data-filter-tally>{count}</span>
                </div>
                <div data-class-list>
"#,
        count = plural(features.len(), "class", "classes"),
    );

    for class in features {
        let counted = class.ported + class.missing;
        let mut haystack = format!("{} {}", class.name, class.python).to_lowercase();
        for member in &class.members {
            haystack.push(' ');
            haystack.push_str(&member.name.to_lowercase());
        }
        // A complete class says so with a full bar and its own count; only a
        // shortfall is worth a pill of its own.
        let mut pills = String::new();
        if class.missing > 0 {
            let _ = write!(
                pills,
                "<span class=\"pill bad\">{} missing</span>",
                class.missing
            );
        }
        let _ = write!(
            html,
            r#"                    <details class="class-item" data-missing="{missing}" data-search="{haystack}">
                        <summary>
                            <span class="class-name"><span class="caret">&#9654;</span><b>{name}</b><span class="path"><code>{python}</code></span></span>
                            <span class="of">{ported} of {counted}{wheel}</span>
                            {bar}
                            <span class="class-counts">{pills}</span>
                        </summary>
                        <div class="class-body">
"#,
            missing = class.missing,
            haystack = escape(&haystack),
            name = escape(&class.name),
            python = escape(&class.python),
            ported = class.ported,
            wheel = if wheel {
                format!(
                    "<span class=\"wheel-of\">{} in the wheel</span>",
                    class.in_wheel
                )
            } else {
                String::new()
            },
            bar = stacked_meter(class.ported, class.missing),
        );
        if let Some(note) = &class.note {
            let _ = writeln!(
                html,
                "                            <p class=\"note\">{}</p>",
                escape(note)
            );
        }
        html.push_str(
            r#"                            <table>
                                <thead><tr><th>music21</th><th>Status</th><th>music21-rs</th><th>Wheel</th></tr></thead>
                                <tbody>
"#,
        );
        // Missing first: that list is the to-do list for porting.
        let mut ordered: Vec<&MemberReport> = class.members.iter().collect();
        ordered.sort_by_key(|member| match member.status {
            Status::Missing => 0,
            Status::Ported => 1,
        });
        for member in ordered {
            let (pill, label, detail) = match member.status {
                Status::Ported => (
                    "pill good",
                    "ported",
                    format!(
                        "<code>{}</code>",
                        escape(member.detail.as_deref().unwrap_or(""))
                    ),
                ),
                Status::Missing => (
                    "pill bad",
                    "missing",
                    escape(member.detail.as_deref().unwrap_or("")),
                ),
            };
            // The wheel is asked separately: the crate is allowed to leave a
            // member out, but one the facade lacks is a member an existing
            // program loses when it installs over music21.
            let carried = if !wheel {
                String::new()
            } else if member.in_wheel {
                "<span class=\"pill good\">yes</span>".to_string()
            } else {
                "<span class=\"pill bad\">no</span>".to_string()
            };
            let _ = writeln!(
                html,
                "                                    <tr><td class=\"name\"><code>{name}</code></td><td><span class=\"{pill}\">{label}</span></td><td>{detail}</td><td>{carried}</td></tr>",
                name = escape(&member.name),
            );
        }
        html.push_str(
            "                                </tbody>\n                            </table>\n                        </div>\n                    </details>\n",
        );
    }

    let _ = write!(
        html,
        r#"                    <p class="empty-note" data-filter-empty hidden>Nothing matches that filter.</p>
                </div>
                <p class="section-foot">Every public member of the music21 classes the crate ports, read from the submodule. A reason for not porting one is shown beside it but still counts against the total. The wheel is asked separately and under music21's own names: the crate may leave a member out on purpose, but one the wheel lacks is a member a program loses when it calls <code>install_into_music21()</code>. It runs the other way too &mdash; a member counted <em>in the wheel only</em> is one the facade answers and the crate has no function for, because what it answers is Python's rather than music theory: a cached value, a callback from the object that holds it, an instrument, or a walk through the stream a note sits in.</p>
            </section>
"#
    );
    html
}

pub(super) fn render_html(report: &Report) -> String {
    let mut scores: Vec<Score> = Vec::new();
    if let Some(coverage) = &report.coverage {
        scores.push(Score {
            anchor: "coverage",
            label: "Line coverage",
            value: format!("{:.1}%", coverage.lines.percent),
            sub: format!(
                "{} of {} lines",
                coverage.lines.covered, coverage.lines.count
            ),
            bar: meter(coverage.lines.percent, false, ""),
            warn: false,
        });
    }
    if !report.suites.is_empty() {
        let passed: usize = report.suites.iter().map(|s| s.passed).sum();
        let failed: usize = report.suites.iter().map(|s| s.failed).sum();
        let skipped = report
            .suites
            .iter()
            .filter(|s| s.status == SuiteStatus::Skipped)
            .count();
        let sub = if failed > 0 {
            format!("passing, {failed} failing")
        } else if skipped > 0 {
            format!("passing, {} not run", plural(skipped, "suite", "suites"))
        } else {
            format!(
                "passing, across {}",
                plural(report.suites.len(), "suite", "suites")
            )
        };
        scores.push(Score {
            anchor: "suites",
            label: "music21-rs's own tests",
            value: passed.to_string(),
            sub,
            bar: meter(share(passed, passed + failed), failed > 0, ""),
            warn: failed > 0,
        });
    }
    // The headline speedup is read off music21's own suite wherever that has
    // been run: thousands of tests doing the same work on both sides beats
    // twenty-one cases written for the purpose, even though it is much the
    // smaller number — most of what a music21 test does is music21's own code
    // whichever side it runs on. The benchmark's own figure keeps its place in
    // the section below, where what it measures is written down beside it.
    let suite_median = report
        .timings
        .as_ref()
        .filter(|timings| !timings.is_empty())
        .and_then(|timings| timings.headline().map(|(side, _)| (timings, side)));
    if let Some((timings, side)) = suite_median {
        let median = side.median_speedup.unwrap_or(1.0);
        scores.push(Score {
            anchor: "speedups",
            label: "Median speedup",
            value: format!("{median:.2}\u{d7}"),
            sub: format!(
                "{} over music21, across {} of its own tests",
                side.name, timings.paired
            ),
            bar: String::new(),
            warn: median < 1.0,
        });
    } else if let Some(benchmarks) = &report.benchmarks
        && !benchmarks.cases.is_empty()
    {
        let median = median_speedup(&benchmarks.cases);
        scores.push(Score {
            anchor: "speedups",
            label: "Median speedup",
            value: format!("{median:.1}\u{d7}"),
            sub: format!(
                "over music21, across {}",
                plural(benchmarks.cases.len(), "case", "cases")
            ),
            bar: String::new(),
            warn: median < 1.0,
        });
    }
    if !report.doctests.is_empty() {
        let passing: usize = report.doctests.iter().map(|m| m.examples_passing).sum();
        let total: usize = report.doctests.iter().map(|m| m.examples).sum();
        scores.push(Score {
            anchor: "doctests",
            label: "music21's tests passed",
            value: format!("{:.1}%", share(passing, total)),
            sub: format!("{passing} of {total} examples"),
            bar: meter(share(passing, total), passing < total, ""),
            warn: passing < total,
        });
    }
    if !report.features.is_empty() {
        let ported: usize = report.features.iter().map(|c| c.ported).sum();
        let missing: usize = report.features.iter().map(|c| c.missing).sum();
        let total = ported + missing;
        scores.push(Score {
            anchor: "ported",
            label: "music21 API ported",
            value: format!("{:.1}%", share(ported, total)),
            sub: format!("{ported} of {total} members"),
            bar: stacked_meter(ported, missing),
            warn: missing > 0,
        });
    }

    let nav: Vec<(&str, &str)> = [
        ("coverage", "Coverage", report.coverage.is_some()),
        ("suites", "Own tests", !report.suites.is_empty()),
        ("speedups", "Speedups", report.benchmarks.is_some()),
        ("doctests", "Against music21", !report.doctests.is_empty()),
        ("ported", "Ported", !report.features.is_empty()),
        ("beyond", "Beyond music21", !report.beyond.is_empty()),
    ]
    .into_iter()
    .filter(|(_, _, present)| *present)
    .map(|(anchor, label, _)| (anchor, label))
    .collect();

    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en" class="no-js">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>music21-rs Reports</title>
        <link rel="stylesheet" href="../theme.css" />
        <style>
{style}        </style>
    </head>
    <body class="page-reports">
        <main class="shell">
            <header>
                <div class="title-row">
                    <a class="home-link" href="../">music21-rs</a>
                    <h1>Reports</h1>
                </div>
                <div class="top-links">
                    <a href="../docs/music21_rs/index.html">Rust docs</a>
                    <a href="../python/">Python docs</a>
                </div>
            </header>
            <p class="report-meta">
                <span>commit <code>{head}</code></span>
                <span class="sep">/</span>
                <span>music21 <code>{version}</code></span>
            </p>
"#,
        style = STYLE,
        head = escape(&report.generated_from),
        version = escape(&report.music21_version),
    );

    if !scores.is_empty() {
        html.push_str("            <div class=\"scoreboard\">\n");
        for score in &scores {
            html.push_str(&score.render());
        }
        html.push_str("            </div>\n");
    }

    if nav.len() > 1 {
        html.push_str("            <nav class=\"section-nav\" aria-label=\"Report sections\">\n");
        for (anchor, label) in &nav {
            let _ = writeln!(html, "                <a href=\"#{anchor}\">{label}</a>");
        }
        html.push_str("            </nav>\n");
    }

    if let Some(coverage) = &report.coverage {
        html.push_str(&render_coverage(coverage));
    }
    if !report.suites.is_empty() {
        html.push_str(&render_suites(&report.suites));
    }
    if report.benchmarks.is_some() || report.timings.is_some() {
        html.push_str(&render_benchmarks(
            report.benchmarks.as_ref(),
            report.sizes.as_ref(),
            report.timings.as_ref(),
        ));
    }
    if !report.beyond.is_empty() {
        html.push_str(&render_beyond(&report.beyond, &report.beyond_members));
    }
    if !report.doctests.is_empty() {
        html.push_str(&render_doctests(&report.doctests));
    }
    if !report.features.is_empty() {
        // A checkout with no `python/src` can say nothing about the wheel, so
        // the column is left off rather than shown as nought everywhere.
        let wheel_known = report.features.iter().any(|class| class.in_wheel > 0);
        html.push_str(&render_features(&report.features, wheel_known));
    }

    let _ = write!(
        html,
        r#"        </main>
        <script type="module" src="../theme.js"></script>
        <script>
{script}        </script>
    </body>
</html>
"#,
        script = SCRIPT,
    );
    html
}
