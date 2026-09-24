//! Rendering the report as three self-contained pages.
//!
//! The overview sets the crate beside the wheel; `crate/` and `wheel/` each
//! say everything the report knows about one of them alone. All three are
//! drawn by the same section renderers, told through a [`View`] which of the
//! two subjects to show, so a number cannot mean one thing on one page and
//! another on the next.
//!
//! The layout lives in `report.css` and `report.js` beside this module and is
//! inlined here, so each page is one file; the colours come from the site's
//! shared theme, which it links.

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

/// The pages' own layout, inlined so each is one self-contained file.
/// Colours come from the site theme it links to.
pub(super) const STYLE: &str = include_str!("../report.css");

/// Filtering the member list and highlighting the section in view. The pages
/// are complete without it.
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

fn percent(part: usize, whole: usize) -> String {
    format!("{:.1}%", share(part, whole))
}

/// The middle of a list of numbers, or nothing for an empty one.
fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(values[values.len() / 2])
}

/// A progress bar. `warn` paints it in the warning colour, for a figure that
/// is a shortfall rather than an achievement; `extra_class` is where a bar
/// says whose it is (`is-crate`, `is-wheel`).
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

/// Which of the three pages is being drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    /// The crate and the wheel, side by side.
    Overview,
    /// The Rust library alone.
    Crate,
    /// The Python package alone.
    Wheel,
}

impl View {
    /// Where the page is written, under the report directory.
    pub(super) fn file(self) -> &'static str {
        match self {
            Self::Overview => "index.html",
            Self::Crate => "crate/index.html",
            Self::Wheel => "wheel/index.html",
        }
    }

    /// The way from this page back to the report directory.
    fn root(self) -> &'static str {
        match self {
            Self::Overview => "./",
            Self::Crate | Self::Wheel => "../",
        }
    }

    fn shows_crate(self) -> bool {
        self != Self::Wheel
    }

    fn shows_wheel(self) -> bool {
        self != Self::Crate
    }

    fn covers(self, subject: Subject) -> bool {
        match self {
            Self::Overview => true,
            Self::Crate => subject.covers_crate(),
            Self::Wheel => subject.covers_wheel(),
        }
    }

    /// The class a bar or a swatch wears to say whose it is; nothing on the
    /// overview, where a figure may be either's.
    fn series(self) -> &'static str {
        match self {
            Self::Overview => "",
            Self::Crate => "is-crate",
            Self::Wheel => "is-wheel",
        }
    }
}

/// The tabs every page carries: label, what it is, and the page it opens.
const TABS: [(View, &str, &str); 3] = [
    (View::Overview, "Overview", "the crate beside the wheel"),
    (View::Crate, "Crate", "music21-rs, the Rust library"),
    (View::Wheel, "Wheel", "music21_rs, the Python package"),
];

/// Everything above the first section: the document head, the masthead, the
/// tabs and the line saying what the numbers were read from.
///
/// `root` is the way back to the report directory and `current` the tab that
/// is this page, if any is — the timings page hangs off the report without
/// being one of the three.
fn page_open(report: &Report, root: &str, current: Option<View>, title: &str) -> String {
    let site = site_root(root);
    let mut html = String::new();
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en" class="no-js">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>music21-rs {title}</title>
        <link rel="stylesheet" href="{site}theme.css" />
        <style>
{style}        </style>
    </head>
    <body class="page-reports">
        <main class="shell">
            <header>
                <div class="title-row">
                    <a class="home-link" href="{site}">music21-rs</a>
                    <h1>{title}</h1>
                </div>
                <div class="top-links">
                    <a href="{site}docs/music21_rs/index.html">Rust docs</a>
                    <a href="{site}python/">Python docs</a>
                </div>
            </header>
            <nav class="view-tabs" aria-label="Which report">
"#,
        style = STYLE,
    );
    for (view, label, what) in TABS {
        let here = if current == Some(view) {
            " class=\"is-current\" aria-current=\"page\""
        } else {
            ""
        };
        let swatch = match view {
            View::Overview => String::new(),
            view => format!("<i class=\"swatch {}\"></i>", view.series()),
        };
        let _ = writeln!(
            html,
            "                <a href=\"{root}{file}\"{here}><b>{swatch}{label}</b><span>{what}</span></a>",
            file = view.file(),
        );
    }
    html.push_str("            </nav>\n            <p class=\"report-meta\">\n");
    let _ = write!(
        html,
        "                <span>commit <code>{head}</code></span>
                <span class=\"sep\">/</span>
                <span>music21 <code>{version}</code></span>
",
        head = escape(&report.generated_from),
        version = escape(&report.music21_version),
    );
    if let Some(benchmarks) = report
        .benchmarks
        .as_ref()
        .filter(|benchmarks| !benchmarks.python.is_empty())
    {
        let _ = write!(
            html,
            "                <span class=\"sep\">/</span>
                <span>Python <code>{python}</code></span>
                <span class=\"sep\">/</span>
                <span><code>{platform}</code></span>
",
            python = escape(&benchmarks.python),
            platform = escape(&benchmarks.platform),
        );
    }
    html.push_str("            </p>\n");
    html
}

/// The way from a page to the site the report is published under, given its
/// way to the report directory.
fn site_root(root: &str) -> String {
    if root == "./" {
        "../".to_string()
    } else {
        format!("{root}../")
    }
}

fn page_close(root: &str) -> String {
    let site = site_root(root);
    format!(
        r#"        </main>
        <script type="module" src="{site}theme.js"></script>
        <script>
{SCRIPT}        </script>
    </body>
</html>
"#
    )
}

/// One headline figure, linking to the section it summarises.
struct Fact {
    anchor: &'static str,
    label: &'static str,
    value: String,
    sub: String,
    /// The bar under the figure, already rendered; empty for a figure that is
    /// not a proportion.
    bar: String,
    warn: bool,
}

impl Fact {
    fn render(&self) -> String {
        format!(
            r##"                <a class="fact{warn}" href="#{anchor}">
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

/// The counts the member list is summarised by, taken once for every place
/// that quotes them.
struct MemberCounts {
    total: usize,
    ported: usize,
    in_wheel: usize,
    /// Members the wheel answers and the crate has no function for.
    wheel_only: usize,
    /// Members missing from the crate that the map gives no reason for.
    undecided: usize,
    /// False for a checkout with no `python/src`, which can say nothing about
    /// the wheel; the column is then left off rather than shown as nought.
    wheel_known: bool,
}

impl MemberCounts {
    fn of(features: &[ClassReport]) -> Self {
        let members = || features.iter().flat_map(|class| &class.members);
        let in_wheel = members().filter(|member| member.in_wheel).count();
        Self {
            total: members().count(),
            ported: members()
                .filter(|member| member.status == Status::Ported)
                .count(),
            in_wheel,
            wheel_only: members()
                .filter(|member| member.status != Status::Ported && member.in_wheel)
                .count(),
            undecided: members()
                .filter(|member| member.status != Status::Ported && member.detail.is_none())
                .count(),
            wheel_known: in_wheel > 0,
        }
    }
}

/// Passing, failing and skipped over the suites a page shows, the comparison
/// against music21 left out: what it counts is music21's tests, not ours.
struct SuiteCounts {
    suites: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl SuiteCounts {
    fn of(suites: &[Suite], view: View) -> Self {
        let own: Vec<&Suite> = suites
            .iter()
            .filter(|suite| view.covers(suite.subject) && !suite.comparison)
            .collect();
        Self {
            suites: own.len(),
            passed: own.iter().map(|suite| suite.passed).sum(),
            failed: own.iter().map(|suite| suite.failed).sum(),
            skipped: own
                .iter()
                .filter(|suite| suite.status == SuiteStatus::Skipped)
                .count(),
        }
    }

    fn fact(&self) -> Fact {
        let sub = if self.failed > 0 {
            format!("passing, {} failing", self.failed)
        } else if self.skipped > 0 {
            format!(
                "passing, {} not run here",
                plural(self.skipped, "suite", "suites")
            )
        } else {
            format!("passing, across {}", plural(self.suites, "suite", "suites"))
        };
        Fact {
            anchor: "suites",
            label: "Own tests",
            value: self.passed.to_string(),
            sub,
            bar: String::new(),
            warn: self.failed > 0,
        }
    }
}

/// The two medians the benchmark gives: the crate with no Python in the way,
/// and the wheel a caller installs, each against music21.
struct Speed {
    cases: usize,
    native: Option<f64>,
    wheel: f64,
    /// What a call through the binding costs over the same call in Rust.
    binding_ns: Option<f64>,
}

impl Speed {
    fn of(benchmarks: Option<&Benchmarks>) -> Option<Self> {
        let cases = &benchmarks.filter(|b| !b.cases.is_empty())?.cases;
        Some(Self {
            cases: cases.len(),
            native: median(cases.iter().filter_map(native_speedup).collect()),
            wheel: median_speedup(cases),
            binding_ns: median(cases.iter().filter_map(binding_cost).collect()),
        })
    }
}

fn native_speedup(case: &BenchCase) -> Option<f64> {
    case.music21_rs_native_ns
        .map(|native| case.music21_ns / native.max(f64::MIN_POSITIVE))
}

fn binding_cost(case: &BenchCase) -> Option<f64> {
    case.music21_rs_native_ns
        .map(|native| (case.music21_rs_ns - native).max(0.0))
}

/// How many times over, written to the precision the size of it deserves.
fn times(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0}\u{d7}")
    } else if value >= 3.0 {
        format!("{value:.1}\u{d7}")
    } else {
        format!("{value:.2}\u{d7}")
    }
}

fn crate_facts(report: &Report) -> Vec<Fact> {
    let mut facts = Vec::new();
    let members = MemberCounts::of(&report.features);
    if members.total > 0 {
        let missing = members.total - members.ported;
        facts.push(Fact {
            anchor: "ported",
            label: "music21 API ported",
            value: percent(members.ported, members.total),
            sub: if missing == 0 {
                format!("all {} members", members.total)
            } else {
                format!(
                    "{} of {} members; {missing} to port, {} undecided",
                    members.ported, members.total, members.undecided
                )
            },
            bar: meter(share(members.ported, members.total), false, "is-crate"),
            warn: false,
        });
    }
    if let Some(coverage) = &report.coverage {
        facts.push(Fact {
            anchor: "coverage",
            label: "Line coverage",
            value: format!("{:.1}%", coverage.lines.percent),
            sub: format!(
                "{} of {} lines",
                coverage.lines.covered, coverage.lines.count
            ),
            bar: meter(coverage.lines.percent, false, "is-crate"),
            warn: false,
        });
    }
    if !report.suites.is_empty() {
        facts.push(SuiteCounts::of(&report.suites, View::Crate).fact());
    }
    if let Some(speed) = Speed::of(report.benchmarks.as_ref())
        && let Some(native) = speed.native
    {
        facts.push(Fact {
            anchor: "speedups",
            label: "Median speedup",
            value: times(native),
            sub: format!(
                "over music21 with no Python in the way, across {}",
                plural(speed.cases, "operation", "operations")
            ),
            bar: String::new(),
            warn: native < 1.0,
        });
    }
    if !report.beyond_members.is_empty() {
        let (total, _) = surface::totals(&report.beyond_members);
        facts.push(Fact {
            anchor: "beyond",
            label: "Beyond music21",
            value: total.to_string(),
            sub: format!(
                "public members music21 has nothing for, in {}",
                plural(report.beyond_members.len(), "module", "modules")
            ),
            bar: String::new(),
            warn: false,
        });
    }
    if let Some(size) = report.sizes.and_then(|sizes| sizes.crate_source) {
        facts.push(Fact {
            anchor: "speedups",
            label: "Source size",
            value: human_bytes(size),
            sub: "what cargo publishes".to_string(),
            bar: String::new(),
            warn: false,
        });
    }
    facts
}

fn wheel_facts(report: &Report) -> Vec<Fact> {
    let mut facts = Vec::new();
    let members = MemberCounts::of(&report.features);
    if members.wheel_known {
        let lacking = members.total - members.in_wheel;
        facts.push(Fact {
            anchor: "ported",
            label: "music21 API in the wheel",
            value: percent(members.in_wheel, members.total),
            sub: format!(
                "{} of {} members; {lacking} lacking, {} the crate has no function for",
                members.in_wheel, members.total, members.wheel_only
            ),
            bar: meter(share(members.in_wheel, members.total), false, "is-wheel"),
            warn: false,
        });
    }
    if !report.suites.is_empty() {
        facts.push(SuiteCounts::of(&report.suites, View::Wheel).fact());
    }
    let speed = Speed::of(report.benchmarks.as_ref());
    if let Some(speed) = &speed {
        facts.push(Fact {
            anchor: "speedups",
            label: "Median speedup",
            value: times(speed.wheel),
            sub: format!(
                "over music21 through the same Python API, across {}",
                plural(speed.cases, "operation", "operations")
            ),
            bar: String::new(),
            warn: speed.wheel < 1.0,
        });
    }
    if let Some(binding) = speed.and_then(|speed| speed.binding_ns) {
        facts.push(Fact {
            anchor: "speedups",
            label: "Cost of the binding",
            value: humanise(binding),
            sub: "a call, median, over the same call in Rust".to_string(),
            bar: String::new(),
            warn: false,
        });
    }
    if let Some(timings) = report.timings.as_ref().filter(|t| !t.is_empty())
        && let Some(median) = timings.sides.get(2).and_then(|side| side.median_speedup)
    {
        facts.push(Fact {
            anchor: "speedups",
            label: "A whole program",
            value: times(median),
            sub: format!(
                "median over {} of music21's own tests, installed over it",
                timings.paired
            ),
            bar: String::new(),
            warn: median < 1.0,
        });
    }
    if let Some(sizes) = report.sizes
        && let Some(wheel) = sizes.wheel
    {
        facts.push(Fact {
            anchor: "speedups",
            label: "Install size",
            value: human_bytes(wheel),
            sub: match sizes.music21 {
                Some(music21) if wheel > 0 => format!(
                    "what pip installs; music21 is {}, {:.0}\u{d7} that",
                    human_bytes(music21),
                    music21 as f64 / wheel as f64
                ),
                _ => "what pip installs".to_string(),
            },
            bar: String::new(),
            warn: false,
        });
    }
    facts
}

/// What is true of both: music21's own documentation and tests, run over the
/// wheel's classes answering out of the crate.
fn shared_facts(report: &Report) -> Vec<Fact> {
    let mut facts = Vec::new();
    if !report.doctests.is_empty() {
        let sum = |field: fn(&ModuleDoctests) -> usize| -> usize {
            report.doctests.iter().map(field).sum()
        };
        for (label, passing, total, noun) in [
            (
                "music21's examples",
                sum(|m| m.examples_passing),
                sum(|m| m.examples),
                "examples",
            ),
            (
                "music21's docstrings",
                sum(|m| m.docstrings_passing),
                sum(|m| m.docstrings),
                "docstrings",
            ),
            (
                "music21's unit tests",
                sum(|m| m.tests_passing),
                sum(|m| m.tests),
                "tests",
            ),
        ] {
            if total == 0 {
                continue;
            }
            facts.push(Fact {
                anchor: "doctests",
                label,
                value: percent(passing, total),
                sub: format!(
                    "{passing} of {total} {noun}, across {}",
                    plural(report.doctests.len(), "module", "modules")
                ),
                bar: meter(share(passing, total), passing < total, ""),
                warn: passing < total,
            });
        }
    }
    if let Some(suite) = report.suites.iter().find(|suite| suite.comparison)
        && suite.status != SuiteStatus::Skipped
    {
        let regressions = suite.regressions();
        facts.push(Fact {
            anchor: "suites",
            label: "music21's own suite",
            value: plural(regressions, "regression", "regressions"),
            sub: format!(
                "{} passing; the {} failing are music21's own or listed",
                suite.passed, suite.expected_failures
            ),
            bar: String::new(),
            warn: regressions > 0,
        });
    }
    facts
}

/// The overview's opening: a card for the crate, a card for the wheel, and
/// under them what holds for both.
fn render_subjects(report: &Report) -> String {
    let mut html = String::from("            <div class=\"subjects\">\n");
    for (view, name, package, what, facts) in [
        (
            View::Crate,
            "The crate",
            "music21-rs",
            "The Rust library: what is ported, how well it is tested, and how fast it is with no Python in the way.",
            crate_facts(report),
        ),
        (
            View::Wheel,
            "The wheel",
            "music21_rs",
            "The Python package: what of music21 it answers, how it fares installed over music21, and what the binding costs.",
            wheel_facts(report),
        ),
    ] {
        if facts.is_empty() {
            continue;
        }
        let _ = write!(
            html,
            r#"            <article class="subject {series}">
                <div class="subject-head">
                    <h2><i class="swatch {series}"></i>{name} <code>{package}</code></h2>
                    <a class="more" href="{file}">Full report &rarr;</a>
                </div>
                <p class="subject-note">{what}</p>
                <div class="facts">
"#,
            series = view.series(),
            file = view.file(),
        );
        for fact in &facts {
            html.push_str(&fact.render());
        }
        html.push_str("                </div>\n            </article>\n");
    }
    html.push_str("            </div>\n");

    let shared = shared_facts(report);
    if !shared.is_empty() {
        html.push_str(
            "            <div class=\"shared\">
                <p class=\"shared-note\"><b>Both at once.</b> music21's own documentation and tests, run over the wheel's classes answering out of the crate.</p>
                <div class=\"facts\">
",
        );
        for fact in &shared {
            html.push_str(&fact.render());
        }
        html.push_str("                </div>\n            </div>\n");
    }
    html
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

/// A rule inside a section, where one section carries two things.
fn sub_head(title: &str, detail: &str) -> String {
    format!(
        "                <h3 class=\"sub-head\">{title}<span class=\"detail\">{detail}</span></h3>\n"
    )
}

pub(super) fn render_coverage(coverage: &Coverage, view: View) -> String {
    let mut html = section_head(
        "coverage",
        "Test coverage",
        "of music21-rs, across every instrumented suite",
    );
    html.push_str("                <div class=\"section-body\">\n                    <div class=\"bar-grid\">\n");
    for (label, percent) in [
        ("Lines", coverage.lines),
        ("Functions", coverage.functions),
        ("Regions", coverage.regions),
    ] {
        let _ = write!(
            html,
            r#"                        <div class="bar-row">
                            <span class="label">{label}</span>
                            {bar}
                            <span class="figure">{value:.1}%<small>{covered} of {count}, {left} not reached</small></span>
                        </div>
"#,
            bar = meter(percent.percent, false, "is-crate"),
            value = percent.percent,
            covered = percent.covered,
            count = percent.count,
            left = percent.count.saturating_sub(percent.covered),
        );
    }
    html.push_str("                    </div>\n                </div>\n");
    let _ = writeln!(
        html,
        "                <p class=\"section-foot\">Coverage is not a run of its own: the suites are run instrumented and the profiles merged. The workspace tests, their rustdoc examples and the parity suite reach it, music21's own suite among them, run against the crate linked into the parity suite; whatever goes through the installed wheel does not. Generated tables and the tooling crates are excluded. <a href=\"{root}coverage/html/index.html\">Read it file by file &rarr;</a></p>\n            </section>",
        root = view.root(),
    );
    html
}

fn subject_tags(subject: Subject) -> String {
    let mut html = String::new();
    if subject.covers_crate() {
        html.push_str("<span class=\"tag is-crate\">crate</span>");
    }
    if subject.covers_wheel() {
        html.push_str("<span class=\"tag is-wheel\">wheel</span>");
    }
    html
}

pub(super) fn render_suites(suites: &[Suite], view: View) -> String {
    let shown: Vec<&Suite> = suites
        .iter()
        .filter(|suite| view.covers(suite.subject))
        .collect();
    if shown.is_empty() {
        return String::new();
    }
    let passed: usize = shown.iter().map(|s| s.passed).sum();
    let regressions: usize = shown.iter().map(|s| s.regressions()).sum();
    let expected: usize = shown.iter().map(|s| s.failed).sum::<usize>() - regressions;
    let skipped = shown
        .iter()
        .filter(|s| s.status == SuiteStatus::Skipped)
        .count();
    let note = if regressions > 0 {
        format!("{regressions} failing, {passed} passing")
    } else if expected > 0 {
        format!("{passed} passing, none failing that music21 passes")
    } else if skipped > 0 {
        format!(
            "{passed} passing, {} not run here",
            plural(skipped, "suite", "suites")
        )
    } else {
        format!("{passed} passing, all green")
    };
    let title = match view {
        View::Overview => "Test suites",
        View::Crate => "The suites that test the crate",
        View::Wheel => "The suites that test the wheel",
    };
    let mut html = section_head("suites", title, &escape(&note));
    html.push_str(
        r#"                <div class="table-wrap">
                    <table>
                        <thead><tr><th>Suite</th><th>Tests</th><th>Result</th><th class="num">Passed</th><th class="num">Failed</th><th>Command</th></tr></thead>
                        <tbody>
"#,
    );
    for suite in shown {
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
        let ran = suite.status != SuiteStatus::Skipped && suite.passed + suite.failed > 0;
        let count = |n: usize| {
            if ran {
                n.to_string()
            } else {
                "&mdash;".to_string()
            }
        };
        // Failures that are music21's own are not this suite's, and the cell
        // says how many of its count they are.
        let failed = if ran && suite.expected_failures > 0 {
            format!(
                "{}<span class=\"detail\">{} music21's own or listed</span>",
                suite.failed, suite.expected_failures
            )
        } else {
            count(suite.failed)
        };
        let note = match &suite.note {
            Some(note) => format!("<span class=\"detail\">{}</span>", prose(note)),
            None => String::new(),
        };
        let _ = write!(
            html,
            r#"                            <tr>
                                <td class="name">{name}{note}{detail}</td>
                                <td><span class="tags">{tags}</span></td>
                                <td><span class="{pill}">{label}</span></td>
                                <td class="num">{passed}</td>
                                <td class="num">{failed}</td>
                                <td class="command"><code>{command}</code></td>
                            </tr>
"#,
            name = escape(&suite.name),
            tags = subject_tags(suite.subject),
            passed = count(suite.passed),
            command = escape(&suite.command),
        );
    }
    html.push_str(
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    html.push_str(
        "                <p class=\"section-foot\">A suite whose tooling is not installed is skipped, with the reason, rather than failed. One marked for both runs music21's code over the wheel's classes, which answer out of the crate, so a failure there may be either's.</p>\n            </section>\n",
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

/// What each costs to install, as bars against the largest.
pub(super) fn render_sizes(sizes: &Sizes, view: View) -> String {
    let rows = [
        (
            "music21",
            "installed package, corpus and all",
            sizes.music21,
            "is-reference",
            true,
        ),
        (
            "music21-rs",
            "the source cargo publishes",
            sizes.crate_source,
            "is-crate",
            view.shows_crate(),
        ),
        (
            "music21_rs wheel",
            "what pip installs",
            sizes.wheel,
            "is-wheel",
            view.shows_wheel(),
        ),
    ];
    let largest = rows.iter().filter_map(|row| row.2).max();
    let Some(largest) = largest.filter(|largest| *largest > 0) else {
        return String::new();
    };

    let mut html = sub_head("What each costs to install", "");
    html.push_str("                <div class=\"section-body\">\n                    <div class=\"bar-grid\">\n");
    for (name, note, size, series, shown) in rows {
        let Some(size) = size.filter(|_| shown) else {
            continue;
        };
        let _ = write!(
            html,
            r#"                        <div class="bar-row">
                            <span class="label">{name}</span>
                            {bar}
                            <span class="figure">{value}<small>{note}</small></span>
                        </div>
"#,
            name = escape(name),
            bar = meter(100.0 * size as f64 / largest as f64, false, series),
            value = human_bytes(size),
            note = escape(note),
        );
    }
    html.push_str("                    </div>\n                </div>\n");
    html
}

/// The derived half of "Beyond music21": every public member of the crate
/// that no music21 member accounts for, one collapsed block per module.
///
/// Collapsed because there are several hundred of them; the summary line
/// carries the count, so the section reads as a set of totals until a module
/// is opened.
pub(super) fn render_beyond_members(modules: &[BeyondModule]) -> String {
    if modules.is_empty() {
        return String::new();
    }
    let (total, by_kind) = surface::totals(modules);
    let kinds = by_kind
        .iter()
        .map(|(kind, count)| format!("{count} {}", kind.label()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut html = sub_head(
        &format!("{total} public members the map matches to nothing in music21"),
        &escape(&format!("{kinds}, across {} modules", modules.len())),
    );
    html.push_str("                <div class=\"beyond-list\">\n");
    let largest = modules
        .iter()
        .map(|module| module.members.len())
        .max()
        .unwrap_or(1);
    for module in modules {
        let _ = write!(
            html,
            "                    <details class=\"beyond-item\">
                        <summary><span class=\"feature-name\"><span class=\"caret\">&#9654;</span>{name}</span>{bar}<span class=\"of\">{count}</span></summary>
                        <ul class=\"member-list\">
",
            name = escape(&module.module),
            bar = meter(
                share(module.members.len(), largest),
                false,
                "is-slim is-crate"
            ),
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
        html.push_str("                        </ul>\n                    </details>\n");
    }
    html.push_str("                </div>\n");
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
                        <thead><tr><th>Capability</th><th class="num">music21-rs</th><th class="num">music21</th></tr></thead>
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
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    html.push_str(&render_beyond_members(modules));
    html.push_str(
        "                <p class=\"section-foot\">Counts are read out of the crate's own tables, so the report fails when one it names has gone. The list of members is derived: every public member of the crate, less every name music21 uses. It is bounded by what the map enumerates, so a member of a music21 class the map does not carry can appear here.</p>
            </section>
",
    );
    html
}

/// The columns of music21's suite a page compares: music21 first, then the
/// sides that are this page's subject.
fn timing_columns(timings: &Timings, view: View) -> Vec<usize> {
    (0..timings.sides.len())
        .filter(|side| {
            matches!(
                (side, view),
                (0, _) | (_, View::Overview) | (1, View::Crate) | (2, View::Wheel)
            )
        })
        .collect()
}

/// A per-test comparison drawn from music21's own suite.
///
/// Every case is the same test doing the same work, so there is nothing to
/// argue about in the pairing — but most of what a music21 test does is
/// music21's own code either way, which is why the middle sits near parity
/// and the tails are the part worth reading. Only the tails are here;
/// [`TIMINGS_PAGE`] carries every paired test.
pub(super) fn render_timings(timings: &Timings, view: View) -> String {
    let columns = timing_columns(timings, view);
    if timings.is_empty() || columns.len() < 2 {
        return String::new();
    }
    // The tails music21-suite wrote are the crate's; a page about the wheel
    // alone reads its own off the full list.
    let subject = columns[1];
    let (ahead, behind) = if subject == 1 {
        (timings.fastest.clone(), timings.slowest.clone())
    } else {
        let mut rows: Vec<&TestTiming> = timings.rows.iter().collect();
        rows.sort_by(|a, b| {
            let (a, b) = (a.speedup_of(subject), b.speedup_of(subject));
            b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
        });
        let tail = timings.fastest.len().max(1).min(rows.len());
        (
            rows[..tail].iter().map(|row| (*row).clone()).collect(),
            rows[rows.len() - tail..]
                .iter()
                .rev()
                .map(|row| (*row).clone())
                .collect(),
        )
    };

    let medians = columns
        .iter()
        .filter_map(|column| {
            let side = &timings.sides[*column];
            side.median_speedup
                .map(|median| format!("{} median {median:.2}&#215;", escape(&side.name)))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut html = sub_head(
        &format!(
            "{} of music21's own tests, timed on every side",
            timings.paired
        ),
        &format!("{}; {medians}", timings.totals()),
    );
    html.push_str(&timing_table(
        timings,
        &columns,
        &[("furthest ahead", &ahead), ("furthest behind", &behind)],
    ));
    let _ = writeln!(
        html,
        "                <p class=\"section-foot\">A test counts only where every side ran it, passed it, and took longer than a millisecond, and what the garbage collector took inside a test is taken off before the sides are compared &mdash; a full collection lands on whichever test is running, and each test is timed once. <em>{ours}</em> is the crate linked into the binary that ran the suite &mdash; the working tree &mdash; and <em>{wheel}</em> is the wheel a caller installs, so what lies between those two is the packaging rather than the code. <a href=\"{root}{page}\">All {paired} tests, side by side &rarr;</a></p>",
        ours = escape(timings.side_name(1)),
        wheel = escape(timings.side_name(2)),
        root = view.root(),
        page = TIMINGS_PAGE,
        paired = timings.paired,
    );
    html
}

/// One table of timings, however many rows and whichever columns.
///
/// Shared by the section and the page it links to, so the two cannot drift
/// apart on what a column means. A group with no label is drawn without a
/// heading row, which is how the whole list is written.
pub(super) fn timing_table(
    timings: &Timings,
    columns: &[usize],
    groups: &[(&str, &Vec<TestTiming>)],
) -> String {
    let mut html = String::from(
        "                <div class=\"table-wrap\">
                    <table>
                        <thead><tr><th>Test</th>",
    );
    for column in columns {
        let _ = write!(
            html,
            "<th class=\"num\">{}</th>",
            escape(&timings.sides[*column].name)
        );
    }
    // One speedup column per subject, which is every column but music21's.
    for column in columns.iter().skip(1) {
        let _ = write!(
            html,
            "<th class=\"num\">{} &#215;</th>",
            escape(&timings.sides[*column].name)
        );
    }
    html.push_str("</tr></thead>\n                        <tbody>\n");
    let width = columns.len() * 2;
    for (label, rows) in groups {
        if !label.is_empty() {
            let _ = writeln!(
                html,
                "                            <tr class=\"group-row\"><td colspan=\"{width}\">{}</td></tr>",
                escape(label)
            );
        }
        for row in rows.iter() {
            let _ = write!(
                html,
                "                            <tr><td class=\"name test-name\"><code>{name}</code></td>",
                name = escape(&row.name),
            );
            for column in columns {
                let cell = row
                    .seconds
                    .get(*column)
                    .map_or_else(|| "&mdash;".to_string(), |taken| seconds(*taken));
                let _ = write!(html, "<td class=\"num\">{cell}</td>");
            }
            for column in columns.iter().skip(1) {
                let _ = write!(
                    html,
                    "<td class=\"num\">{}</td>",
                    speedup_pill(row.speedup_of(*column), 2)
                );
            }
            html.push_str("</tr>\n");
        }
    }
    html.push_str(
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    html
}

/// A speedup as a pill, green at parity and above. A case where music21 is
/// quicker is worth seeing as such.
fn speedup_pill(speedup: Option<f64>, places: usize) -> String {
    match speedup {
        Some(speedup) => {
            let pill = if speedup >= 1.0 { "good" } else { "bad" };
            format!("<span class=\"pill {pill}\">{speedup:.places$}&#215;</span>")
        }
        None => "<span class=\"of\">&mdash;</span>".to_string(),
    }
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
    let mut html = page_open(report, "./", None, "Test timings");
    html.push_str(&section_head(
        "timings",
        "Every test, timed on every side",
        &escape(&format!("{} paired", timings.paired)),
    ));
    let _ = writeln!(
        html,
        "                <p class=\"section-foot is-lead\">{totals}. A test is here only where every side ran it, passed it, and took longer than a millisecond, so this is fewer tests than the suite runs. What the garbage collector took inside a test is taken off first: a full collection lands on whichever test is running, and each test is timed once. Ordered by what the crate made of it, furthest behind first.</p>",
        totals = timings.totals(),
    );
    let columns: Vec<usize> = (0..timings.sides.len()).collect();
    html.push_str(&timing_table(timings, &columns, &[("", &timings.rows)]));
    html.push_str("            </section>\n");
    html.push_str(&page_close("./"));
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

/// The benchmark's cases as a table: music21, then the sides this page is
/// about, then what each makes of music21's time.
fn bench_table(benchmarks: &Benchmarks, view: View) -> String {
    let crate_shown = view.shows_crate();
    let wheel_shown = view.shows_wheel();
    let mut html = String::from(
        "                <div class=\"table-wrap\">\n                    <table>\n                        <thead><tr><th>Case</th><th class=\"num\">music21</th>",
    );
    let mut width = 2;
    let mut column = |html: &mut String, shown: bool, label: &str| {
        if shown {
            let _ = write!(html, "<th class=\"num\">{label}</th>");
            width += 1;
        }
    };
    column(&mut html, crate_shown, "crate");
    column(&mut html, wheel_shown, "wheel");
    column(&mut html, crate_shown, "crate &#215;");
    column(&mut html, wheel_shown, "wheel &#215;");
    column(&mut html, wheel_shown, "binding");
    html.push_str("</tr></thead>\n                        <tbody>\n");

    let dash = || "<span class=\"of\">&mdash;</span>".to_string();
    let mut group = "";
    for case in &benchmarks.cases {
        if case.group != group {
            group = &case.group;
            let _ = writeln!(
                html,
                "                            <tr class=\"group-row\"><td colspan=\"{width}\">{}</td></tr>",
                escape(group)
            );
        }
        let note = if case.notes.is_empty() {
            String::new()
        } else {
            format!("<span class=\"detail\">{}</span>", escape(&case.notes))
        };
        let _ = write!(
            html,
            "                            <tr><td class=\"name\"><code>{name}</code>{note}</td><td class=\"num\">{slow}</td>",
            name = escape(&case.case),
            slow = humanise(case.music21_ns),
        );
        let mut cell = |shown: bool, value: String| {
            if shown {
                let _ = write!(html, "<td class=\"num\">{value}</td>");
            }
        };
        cell(
            crate_shown,
            case.music21_rs_native_ns.map_or_else(dash, humanise),
        );
        cell(wheel_shown, humanise(case.music21_rs_ns));
        cell(crate_shown, speedup_pill(native_speedup(case), 1));
        cell(wheel_shown, speedup_pill(Some(case.speedup), 1));
        cell(
            wheel_shown,
            binding_cost(case).map_or_else(dash, |cost| format!("+{}", humanise(cost))),
        );
        html.push_str("</tr>\n");
    }
    html.push_str(
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    html
}

pub(super) fn render_benchmarks(
    benchmarks: Option<&Benchmarks>,
    sizes: Option<&Sizes>,
    timings: Option<&Timings>,
    view: View,
) -> String {
    // The halves are measured by different commands. music21's own suite may
    // have been timed where the benchmark was never run, and the section is
    // worth having for either alone.
    let speed = Speed::of(benchmarks);
    let note = match (&speed, benchmarks) {
        (Some(speed), _) => {
            let mut parts = Vec::new();
            if let Some(native) = speed.native.filter(|_| view.shows_crate()) {
                parts.push(format!("crate {} median", times(native)));
            }
            if view.shows_wheel() {
                parts.push(format!("wheel {} median", times(speed.wheel)));
            }
            format!(
                "{}, across {}",
                parts.join(", "),
                plural(speed.cases, "case", "cases")
            )
        }
        (None, Some(benchmarks)) => benchmarks
            .detail
            .clone()
            .unwrap_or_else(|| "not run".to_string()),
        (None, None) => "from music21's own test suite".to_string(),
    };
    let mut html = section_head("speedups", "Speed and size against music21", &escape(&note));

    match benchmarks {
        Some(benchmarks) if speed.is_some() => {
            html.push_str(&bench_table(benchmarks, view));
            let _ = writeln!(
                html,
                "                <p class=\"section-foot\">music21 {music21}, Python {python} ({platform}). Every side must agree on the answer before any of them is timed. <em>wheel</em> is what a caller installs, asked through the same Python API as music21; <em>crate</em> is the same operation in Rust with no Python in the way, and <em>binding</em> is what lies between the two. Each case builds a fresh object, except those marked cached.</p>",
                music21 = escape(&benchmarks.music21),
                python = escape(&benchmarks.python),
                platform = escape(&benchmarks.platform),
            );
        }
        Some(benchmarks) => {
            let _ = writeln!(
                html,
                "                <p class=\"section-foot is-lead\">The benchmarks time the crate against music21 through the same Python API, and need both installed in the interpreter that runs them: <code>{}</code>.</p>",
                escape(&benchmarks.command),
            );
        }
        None => {}
    }
    if let Some(timings) = timings {
        html.push_str(&render_timings(timings, view));
    }
    // What each costs to install is the other half of what each costs to run.
    if let Some(sizes) = sizes {
        html.push_str(&render_sizes(sizes, view));
    }
    html.push_str("            </section>\n");
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
        "{examples_passing} of {examples} examples and {tests_passing} of {tests} unit tests, across {modules}",
        modules = plural(doctests.len(), "module", "modules")
    );
    if unharnessed > 0 {
        let _ = write!(note, ", {unharnessed} with no harness yet");
    }
    let mut html = section_head("doctests", "Against music21's own tests", &escape(&note));
    html.push_str(
        r#"                <div class="table-wrap">
                    <table>
                        <thead><tr><th>Module</th><th>Docstrings</th><th>Examples</th><th>Unit tests</th></tr></thead>
                        <tbody>
"#,
    );
    let cell = |passing: usize, total: usize| {
        if total == 0 {
            return "<span class=\"of\">none</span>".to_string();
        }
        format!(
            "<div class=\"progress-cell\"><span><b>{passing}</b> <span class=\"of\">of {total}</span></span>{bar}</div>",
            bar = meter(share(passing, total), passing < total, "is-slim"),
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
        "                <p class=\"section-foot\">music21's own docstrings and unit tests, run over the wheel's classes answering out of the crate, so a row here is a row for both. A docstring passes only when every one of its examples does. Every docstring is read as music21's own runner reads it, which rewrites an object's address to <code>0x...</code> before comparing; the unit-test column is music21's own test suite, attributed per module, where a test music21 itself fails counts as not passing.</p>\n            </section>\n",
    );
    html
}

/// One of the two counts a class is summarised by: so many of so many, and
/// the bar that says the same.
fn measure(part: usize, whole: usize, series: &str, label: &str) -> String {
    format!(
        "<span class=\"measure\"><span class=\"count\"><span class=\"measure-label\">{label} </span><b>{part}</b><span class=\"of\"> of {whole}</span></span>{bar}</span>",
        bar = meter(share(part, whole), false, &format!("is-slim {series}")),
    )
}

fn wheel_pill(in_wheel: bool) -> &'static str {
    if in_wheel {
        "<span class=\"pill good\">in the wheel</span>"
    } else {
        "<span class=\"pill bad\">not in the wheel</span>"
    }
}

/// What the crate makes of a member: the function that ports it, or why there
/// is none, or that nobody has said.
fn crate_cell(member: &MemberReport) -> String {
    match (member.status, member.detail.as_deref()) {
        (Status::Ported, detail) => format!("<code>{}</code>", escape(detail.unwrap_or(""))),
        (Status::Missing, Some(reason)) => escape(reason),
        (Status::Missing, None) => "<span class=\"of\">no reason given</span>".to_string(),
    }
}

pub(super) fn render_features(features: &[ClassReport], view: View) -> String {
    let counts = MemberCounts::of(features);
    let crate_shown = view.shows_crate();
    let wheel_shown = view.shows_wheel() && counts.wheel_known;
    if !crate_shown && !wheel_shown {
        return String::new();
    }

    // The legend doubles as the summary: it names each bar and gives its
    // count.
    let mut note = format!(
        "<span class=\"legend\">{}:",
        plural(counts.total, "member", "members")
    );
    if crate_shown {
        let _ = write!(
            note,
            "<span><i class=\"swatch is-crate\"></i>{} in the crate</span>",
            counts.ported
        );
    }
    if wheel_shown {
        let _ = write!(
            note,
            "<span><i class=\"swatch is-wheel\"></i>{} in the wheel</span>",
            counts.in_wheel
        );
    }
    note.push_str("</span>");
    let title = match view {
        View::Overview => "music21's API, member by member",
        View::Crate => "Ported from music21",
        View::Wheel => "music21's API in the wheel",
    };
    let mut html = section_head("ported", title, &note);

    let layout = match (crate_shown, wheel_shown) {
        (true, true) => "is-both",
        _ => "is-one",
    };
    let _ = write!(
        html,
        r#"                <div class="toolbar">
                    <input type="search" data-filter-search placeholder="Filter by class, file or member name" aria-label="Filter the member list" />
                    <label class="check"><input type="checkbox" data-filter-incomplete /> Only incomplete</label>
                    <button type="button" class="button-link" data-expand-all>Expand all</button>
                    <button type="button" class="button-link" data-collapse-all>Collapse all</button>
                    <span class="tally" data-filter-tally>{count}</span>
                </div>
                <div class="class-list {layout}" data-class-list>
                    <div class="class-cols" aria-hidden="true">
                        <span>music21 class</span>
"#,
        count = plural(features.len(), "class", "classes"),
    );
    if crate_shown {
        html.push_str(
            "                        <span><i class=\"swatch is-crate\"></i>Crate</span>\n",
        );
    }
    if wheel_shown {
        html.push_str(
            "                        <span><i class=\"swatch is-wheel\"></i>Wheel</span>\n",
        );
    }
    html.push_str("                        <span></span>\n                    </div>\n");

    for class in features {
        let counted = class.members.len();
        let lacking = counted - class.in_wheel;
        let mut haystack = format!("{} {}", class.name, class.python).to_lowercase();
        let mut members = String::new();
        for member in &class.members {
            members.push(' ');
            members.push_str(&member.name.to_lowercase());
        }
        haystack.push_str(&members);

        // A complete class says so with a full bar and its own count; only a
        // shortfall is worth a pill of its own.
        let mut pills = String::new();
        let mut incomplete = 0;
        if crate_shown && class.missing > 0 {
            incomplete += class.missing;
            let _ = write!(
                pills,
                "<span class=\"pill bad\">{} to port</span>",
                class.missing
            );
        }
        if wheel_shown && lacking > 0 {
            incomplete += lacking;
            let _ = write!(
                pills,
                "<span class=\"pill bad\">{lacking} not in wheel</span>"
            );
        }
        let _ = write!(
            html,
            r#"                    <details class="class-item" data-missing="{incomplete}" data-search="{haystack}">
                        <summary>
                            <span class="class-name"><span class="caret">&#9654;</span><b>{name}</b><span class="path"><code>{python}</code></span></span>
"#,
            haystack = escape(&haystack),
            name = escape(&class.name),
            python = escape(&class.python),
        );
        if crate_shown {
            let _ = writeln!(
                html,
                "                            {}",
                measure(class.ported, counted, "is-crate", "crate")
            );
        }
        if wheel_shown {
            let _ = writeln!(
                html,
                "                            {}",
                measure(class.in_wheel, counted, "is-wheel", "wheel")
            );
        }
        let _ = write!(
            html,
            r#"                            <span class="class-counts">{pills}</span>
                        </summary>
                        <div class="class-body">
"#
        );
        if let Some(note) = &class.note {
            let _ = writeln!(
                html,
                "                            <p class=\"note\">{}</p>",
                escape(note)
            );
        }
        html.push_str("                            <table>\n                                <thead><tr><th>music21</th>");
        if crate_shown {
            html.push_str("<th>Crate</th><th>music21-rs</th>");
        }
        if wheel_shown {
            html.push_str("<th>Wheel</th>");
        }
        if !crate_shown {
            html.push_str("<th>In the crate</th>");
        }
        html.push_str("</tr></thead>\n                                <tbody>\n");

        // What is incomplete first: that list is the to-do list.
        let mut ordered: Vec<&MemberReport> = class.members.iter().collect();
        ordered.sort_by_key(|member| {
            let done = if crate_shown {
                member.status == Status::Ported
            } else {
                member.in_wheel
            };
            (done, wheel_shown && member.in_wheel)
        });
        for member in ordered {
            let _ = write!(
                html,
                "                                    <tr data-member=\"{lower}\"><td class=\"name\"><code>{name}</code></td>",
                lower = escape(&member.name.to_lowercase()),
                name = escape(&member.name),
            );
            if crate_shown {
                let pill = match member.status {
                    Status::Ported => "<span class=\"pill good\">ported</span>",
                    Status::Missing => "<span class=\"pill bad\">missing</span>",
                };
                let _ = write!(html, "<td>{pill}</td><td>{}</td>", crate_cell(member));
            }
            if wheel_shown {
                let _ = write!(html, "<td>{}</td>", wheel_pill(member.in_wheel));
            }
            if !crate_shown {
                let _ = write!(html, "<td>{}</td>", crate_cell(member));
            }
            html.push_str("</tr>\n");
        }
        html.push_str(
            "                                </tbody>\n                            </table>\n                        </div>\n                    </details>\n",
        );
    }

    html.push_str(
        "                    <p class=\"empty-note\" data-filter-empty hidden>Nothing matches that filter.</p>\n                </div>\n",
    );
    let foot = match view {
        View::Overview => {
            "Every public member of the music21 classes the crate ports, read from the submodule, and asked of each side separately. The crate may leave a member out on purpose &mdash; a reason is shown beside it and it still counts against the total &mdash; but one the wheel lacks is a member a program loses when it calls <code>install_into_music21()</code>. It runs the other way too: a member the wheel answers and the crate has no function for is one whose answer is Python's rather than music theory's &mdash; a cached value, a callback from the object that holds it, an instrument, or a walk through the stream a note sits in."
        }
        View::Crate => {
            "Every public member of the music21 classes the crate ports, read from the submodule and matched to a <code>pub fn</code> by the crate's naming convention, with <code>data/feature_map.toml</code> holding the renames the convention does not give. A reason for not porting a member is shown beside it but still counts against the total."
        }
        View::Wheel => {
            "Every public member of the music21 classes the wheel stands in for, asked of the wheel under music21's own names. A member it lacks is one a program loses when it calls <code>install_into_music21()</code>, where it raises rather than quietly answering out of music21's implementation. The last column says what the crate makes of the same member."
        }
    };
    let _ = writeln!(
        html,
        "                <p class=\"section-foot\">{foot}</p>\n            </section>"
    );
    html
}

/// One row of a flat member list: the member under its class, and whatever
/// cells the list is about.
fn todo_row(class: &ClassReport, member: &MemberReport, cells: &[String]) -> String {
    // A module's own functions and classes are listed under the file's name,
    // which the line below already gives; only a class qualifies its member.
    let name = if class.name.contains(".py ") {
        escape(&member.name)
    } else {
        format!("{}.{}", escape(&class.name), escape(&member.name))
    };
    let mut html = format!(
        "                            <tr><td class=\"name\"><code>{name}</code><span class=\"detail\"><code>{python}</code></span></td>",
        python = escape(&class.python),
    );
    for cell in cells {
        let _ = write!(html, "<td>{cell}</td>");
    }
    html.push_str("</tr>\n");
    html
}

/// A flat list in groups, or a line saying the list is empty.
fn todo_table(headings: &[&str], groups: &[(String, Vec<String>)], empty: &str) -> String {
    if groups.iter().all(|(_, rows)| rows.is_empty()) {
        return format!("                <p class=\"empty-note\">{empty}</p>\n");
    }
    let mut html = String::from(
        "                <div class=\"table-wrap\">\n                    <table class=\"todo\">\n                        <thead><tr>",
    );
    for heading in headings {
        let _ = write!(html, "<th>{heading}</th>");
    }
    html.push_str("</tr></thead>\n                        <tbody>\n");
    for (label, rows) in groups {
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(
            html,
            "                            <tr class=\"group-row\"><td colspan=\"{}\">{label}</td></tr>",
            headings.len()
        );
        for row in rows {
            html.push_str(row);
        }
    }
    html.push_str(
        "                        </tbody>\n                    </table>\n                </div>\n",
    );
    html
}

/// The crate's to-do list: every member it has no function for, the ones
/// nobody has decided about first.
fn render_crate_todo(features: &[ClassReport], wheel_known: bool) -> String {
    let mut undecided = Vec::new();
    let mut reasoned = Vec::new();
    for class in features {
        for member in &class.members {
            if member.status == Status::Ported {
                continue;
            }
            let mut cells = vec![crate_cell(member)];
            if wheel_known {
                cells.push(wheel_pill(member.in_wheel).to_string());
            }
            let row = todo_row(class, member, &cells);
            match member.detail {
                None => undecided.push(row),
                Some(_) => reasoned.push(row),
            }
        }
    }
    let total = undecided.len() + reasoned.len();
    let mut html = section_head(
        "todo",
        "Still to port",
        &escape(&format!(
            "{}, {} with no reason given",
            plural(total, "member", "members"),
            undecided.len()
        )),
    );
    let mut headings = vec!["music21 member", "Why not"];
    if wheel_known {
        headings.push("Wheel");
    }
    html.push_str(&todo_table(
        &headings,
        &[
            (format!("undecided &mdash; {}", undecided.len()), undecided),
            (
                format!("left out, with a reason &mdash; {}", reasoned.len()),
                reasoned,
            ),
        ],
        "Nothing: every member music21 has is ported.",
    ));
    html.push_str(
        "                <p class=\"section-foot\">The same members the list above marks missing, in one place. The ones to watch are those with no reason at all: they are what nobody has decided about. A reason is an exclusion in <code>data/feature_map.toml</code>, and an exclusion is checked before the naming convention, so one left behind after its member is ported goes on hiding the port.</p>\n            </section>\n",
    );
    html
}

/// The wheel's to-do list, and the members it answers that the crate does
/// not.
fn render_wheel_todo(features: &[ClassReport]) -> String {
    let mut lacking = Vec::new();
    let mut wheel_only = Vec::new();
    for class in features {
        for member in &class.members {
            if !member.in_wheel {
                let status = match member.status {
                    Status::Ported => "<span class=\"pill good\">ported</span>",
                    Status::Missing => "<span class=\"pill bad\">missing</span>",
                };
                lacking.push(todo_row(
                    class,
                    member,
                    &[status.to_string(), crate_cell(member)],
                ));
            } else if member.status != Status::Ported {
                wheel_only.push(todo_row(class, member, &[crate_cell(member)]));
            }
        }
    }
    let mut html = section_head(
        "todo",
        "Not in the wheel",
        &escape(&plural(lacking.len(), "member", "members")),
    );
    let count = lacking.len();
    html.push_str(&todo_table(
        &["music21 member", "Crate", "music21-rs"],
        &[(
            format!("raises under <code>install_into_music21()</code> &mdash; {count}"),
            lacking,
        )],
        "Nothing: the wheel answers every member music21 has.",
    ));
    html.push_str(
        "                <p class=\"section-foot\">A member listed as ported is one the crate has a function for and the facade has not reached yet, which makes it the cheapest kind to close.</p>\n            </section>\n",
    );

    html.push_str(&section_head(
        "wheel-only",
        "In the wheel only",
        &escape(&plural(wheel_only.len(), "member", "members")),
    ));
    let count = wheel_only.len();
    html.push_str(&todo_table(
        &["music21 member", "Why the crate has no function for it"],
        &[(
            format!("answered by the facade alone &mdash; {count}"),
            wheel_only,
        )],
        "Nothing: every member the wheel answers, the crate has a function for.",
    ));
    html.push_str(
        "                <p class=\"section-foot\">What these answer is Python's rather than music theory's: a cached value, a callback from the object that holds it, an instrument, or a walk through the stream a note sits in. They are why the crate is not a superset of the wheel.</p>\n            </section>\n",
    );
    html
}

/// One of the three pages.
pub(super) fn render_html(report: &Report, view: View) -> String {
    let counts = MemberCounts::of(&report.features);
    let suites = render_suites(&report.suites, view);
    let speed = if report.benchmarks.is_some() || report.timings.is_some() {
        render_benchmarks(
            report.benchmarks.as_ref(),
            report.sizes.as_ref(),
            report.timings.as_ref(),
            view,
        )
    } else {
        String::new()
    };
    let features = if report.features.is_empty() {
        String::new()
    } else {
        render_features(&report.features, view)
    };
    let todo = match view {
        _ if report.features.is_empty() => String::new(),
        View::Overview => String::new(),
        View::Crate => render_crate_todo(&report.features, counts.wheel_known),
        View::Wheel if counts.wheel_known => render_wheel_todo(&report.features),
        View::Wheel => String::new(),
    };

    // Each section with the label the navigation gives it, in page order; one
    // with nothing to show is left out of both.
    let sections: Vec<(&str, &str, String)> = vec![
        ("suites", "Suites", suites),
        (
            "coverage",
            "Coverage",
            report
                .coverage
                .as_ref()
                .filter(|_| view.shows_crate())
                .map_or_else(String::new, |coverage| render_coverage(coverage, view)),
        ),
        ("speedups", "Speed and size", speed),
        (
            "doctests",
            "Against music21",
            if report.doctests.is_empty() {
                String::new()
            } else {
                render_doctests(&report.doctests)
            },
        ),
        (
            "ported",
            match view {
                View::Wheel => "In the wheel",
                _ => "Ported",
            },
            features,
        ),
        (
            "todo",
            match view {
                View::Wheel => "Not in the wheel",
                _ => "Still to port",
            },
            todo,
        ),
        (
            "beyond",
            "Beyond music21",
            if report.beyond.is_empty() || !view.shows_crate() {
                String::new()
            } else {
                render_beyond(&report.beyond, &report.beyond_members)
            },
        ),
    ];

    let title = match view {
        View::Overview => "Reports",
        View::Crate => "Crate report",
        View::Wheel => "Wheel report",
    };
    let mut html = page_open(report, view.root(), Some(view), title);

    match view {
        View::Overview => html.push_str(&render_subjects(report)),
        View::Crate | View::Wheel => {
            let mut facts = if view == View::Crate {
                crate_facts(report)
            } else {
                wheel_facts(report)
            };
            facts.extend(shared_facts(report));
            if !facts.is_empty() {
                let _ = writeln!(
                    html,
                    "            <div class=\"facts is-board {}\">",
                    view.series()
                );
                for fact in &facts {
                    html.push_str(&fact.render());
                }
                html.push_str("            </div>\n");
            }
        }
    }

    let present: Vec<&(&str, &str, String)> = sections
        .iter()
        .filter(|(_, _, body)| !body.is_empty())
        .collect();
    if present.len() > 1 {
        html.push_str("            <nav class=\"section-nav\" aria-label=\"Report sections\">\n");
        for (anchor, label, _) in &present {
            let _ = writeln!(html, "                <a href=\"#{anchor}\">{label}</a>");
        }
        html.push_str("            </nav>\n");
    }
    for (_, _, body) in present {
        html.push_str(body);
    }

    html.push_str(&page_close(view.root()));
    html
}
