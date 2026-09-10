//! The `report` command: test coverage and the ported feature set, as one
//! static page for the GitHub Pages site.
//!
//! Coverage comes from `cargo llvm-cov` over the library's own tests, with the
//! generated tables left out — `chord/tables/generated.rs` and
//! `scala_bundled.rs` are data, and counting their thousands of lines would
//! say nothing about the code.
//!
//! The feature set is read off music21 itself. `data/feature_map.toml` names
//! the music21 classes the crate ports and the Rust files each lives in; this
//! scans the submodule for every public method of those classes, converts the
//! name the way the crate does (`isMajorTriad` → `is_major_triad`, a leading
//! `get` dropped), and looks for a `pub fn` of that name. The map carries the
//! renames the convention does not give and the members deliberately left out,
//! with a reason each — and a rename that points at a function that does not
//! exist, or an exclusion of a member music21 no longer has, is an error, so
//! the map cannot go stale silently. That is the "check" in the command.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

use crate::facade::{self, Facade};
use crate::surface::{self, BeyondModule};

mod features;
mod html;
mod suites;

use features::*;
use html::*;
use suites::*;

pub(crate) use suites::python_command;

/// What the coverage figure leaves out. Generated data rather than code
/// (`chord/tables/generated.rs`, `scala_bundled.rs`), and every crate that is
/// not the library: the suites now run instrumented across the whole
/// workspace and `python-parity` besides, so without this the figure would be
/// diluted by the tooling that does the measuring.
const COVERAGE_IGNORE: &str =
    r"generated\.rs|scala_bundled\.rs|[\\/](xtask|utils|examples|python-parity|python)[\\/]";

#[derive(Debug, Deserialize)]
struct FeatureMap {
    class: Vec<ClassMap>,
    /// What the crate has that music21 does not.
    #[serde(default)]
    beyond: Vec<BeyondMap>,
}

/// One thing the crate does that music21 has no counterpart for. The count,
/// where there is one, is read out of the Rust source rather than written
/// here, so it cannot drift.
#[derive(Debug, Deserialize)]
struct BeyondMap {
    name: String,
    note: String,
    #[serde(default)]
    rust: Option<String>,
    /// A `const NAME: [T; N]` whose declared length is the count.
    #[serde(default)]
    count: Option<String>,
    #[serde(default)]
    unit: Option<String>,
    /// Counted against music21's own `.scl` files instead.
    #[serde(default)]
    scala_archive: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct BeyondReport {
    name: String,
    note: String,
    /// How many of them, where that is a number worth giving.
    count: Option<usize>,
    unit: Option<String>,
    /// What music21 has of the same thing, where it has any.
    music21: Option<usize>,
}

/// How big each thing is to install, in bytes.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Sizes {
    /// The `music21` package as it lands in site-packages, corpus and all.
    music21: Option<u64>,
    /// What `cargo add music21-rs` fetches: the source the crate publishes.
    crate_source: Option<u64>,
    /// The built wheel, which is what `pip install music21-rs` fetches.
    wheel: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ClassMap {
    /// Path of the Python file inside the `music21` submodule.
    python: String,
    /// The class whose methods are read, or absent for the module's own
    /// functions.
    #[serde(default)]
    name: Option<String>,
    /// What is enumerated: the class's methods (the default) or, for a module
    /// whose port is a table of variants, its classes.
    #[serde(default)]
    members: Members,
    /// Only classes whose names end with this are enumerated, for
    /// `members = "classes"`.
    #[serde(default)]
    suffix: Option<String>,
    /// Rust files searched for the port.
    rust: Vec<String>,
    /// A short description for the page.
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    renames: BTreeMap<String, String>,
    #[serde(default)]
    excluded: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Members {
    #[default]
    Methods,
    Classes,
}

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    generated_from: String,
    music21_version: String,
    coverage: Option<Coverage>,
    #[serde(default)]
    sizes: Option<Sizes>,
    suites: Vec<Suite>,
    #[serde(default)]
    benchmarks: Option<Benchmarks>,
    doctests: Vec<ModuleDoctests>,
    features: Vec<ClassReport>,
    /// What music21's own suite said about speed, where it has been run.
    #[serde(default)]
    timings: Option<Timings>,
    /// Every public member of the crate no music21 member accounts for.
    #[serde(default)]
    beyond_members: Vec<BeyondModule>,
    #[serde(default)]
    beyond: Vec<BeyondReport>,
}

/// The crate timed against music21 through the same Python API, by
/// `xtask bench`. Both sides are asked the same question and
/// have to agree on the answer before either is timed, so a speedup here is a
/// speedup at doing the same work.
#[derive(Debug, Serialize, Deserialize)]
struct Benchmarks {
    status: SuiteStatus,
    command: String,
    /// Why the benchmarks were not run, when they were not.
    detail: Option<String>,
    #[serde(default)]
    music21: String,
    #[serde(default)]
    python: String,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    cases: Vec<BenchCase>,
}

/// One benchmark case, as `xtask bench --json` writes it.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct BenchCase {
    case: String,
    group: String,
    #[serde(default)]
    notes: String,
    music21_ns: f64,
    /// The wheel, which is what a caller installs.
    music21_rs_ns: f64,
    /// The crate with no Python in the way, where the case has a counterpart
    /// there. Absent means unmeasured, not nought: the difference between
    /// this and the wheel is what the binding costs.
    #[serde(default)]
    music21_rs_native_ns: Option<f64>,
    speedup: f64,
}

/// What the parity harness writes beside each module's failure log.
#[derive(Debug, Deserialize)]
struct DoctestSummary {
    module: String,
    docstrings_passing: usize,
    docstrings: usize,
    examples_passing: usize,
    examples: usize,
}

/// What the pair of runs of music21's own suite said about time, as
/// `xtask music21-suite` writes it into `target/music21-suite/timings.json`.
///
/// The benchmark above is twenty-one cases written for the purpose; this is
/// every test music21 has, timed on both sides of a run that was happening
/// anyway.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Timings {
    #[serde(default)]
    paired: usize,
    /// The columns, music21 first, then every subject that ran: the crate as
    /// it is compiled, and the wheel that ships.
    #[serde(default)]
    sides: Vec<SideTotal>,
    /// Every paired test. The section shows the two tails; the whole of it is
    /// a page of its own, which is the only place all of these fit.
    #[serde(default)]
    rows: Vec<TestTiming>,
    #[serde(default)]
    fastest: Vec<TestTiming>,
    #[serde(default)]
    slowest: Vec<TestTiming>,
}

impl Timings {
    /// Whether there is anything to draw. An old `report.json` written before
    /// the columns existed deserializes to nothing rather than failing the
    /// whole read, and this is what notices.
    fn is_empty(&self) -> bool {
        self.sides.len() < 2 || self.rows.is_empty()
    }

    /// What each side spent on the paired tests, as one phrase.
    fn totals(&self) -> String {
        self.sides
            .iter()
            .map(|side| format!("{:.0}s on {}", side.seconds, escape(&side.name)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// One column's name, or what that column would have been called had it
    /// run. A skipped wheel leaves the prose with nothing to point at.
    fn side_name(&self, index: usize) -> &str {
        const NAMES: [&str; 3] = ["music21", "music21-rs", "music21-rs-wheel"];
        self.sides.get(index).map_or_else(
            || NAMES.get(index).copied().unwrap_or("music21-rs"),
            |side| side.name.as_str(),
        )
    }

    /// The column a headline speedup should be read off, and its name.
    ///
    /// The wheel where there is one: it is what a caller installs, so it is
    /// what a caller gets. The crate compiled here is the fallback, and is
    /// what the number means on a machine with no wheel built.
    fn headline(&self) -> Option<(&SideTotal, usize)> {
        let wheel = self
            .sides
            .iter()
            .enumerate()
            .rfind(|(_, side)| side.median_speedup.is_some())?;
        Some((wheel.1, wheel.0))
    }
}

/// One column of the comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SideTotal {
    name: String,
    seconds: f64,
    /// The median of this side's per-test speedup against music21, absent for
    /// music21 itself.
    #[serde(default)]
    median_speedup: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestTiming {
    name: String,
    /// One duration per side, in the order `sides` gives them.
    seconds: Vec<f64>,
    speedup: f64,
}

impl TestTiming {
    /// This test's speedup on one column against music21.
    fn speedup_of(&self, side: usize) -> Option<f64> {
        let theirs = *self.seconds.first()?;
        let mine = *self.seconds.get(side)?;
        (side > 0).then(|| theirs / mine.max(f64::MIN_POSITIVE))
    }
}

/// One module's share of music21's own unit tests, as `xtask music21-suite`
/// writes it into `target/music21-suite/music21_rs.json`.
///
/// Declared again here rather than shared with that module: it is only
/// compiled under the `python` feature and the report is not, so what crosses
/// between them is the file, exactly as it is for every other suite here.
#[derive(Debug, Clone, Deserialize)]
struct ModuleTests {
    module: String,
    tests: usize,
    tests_passing: usize,
}

/// `data/doctest_totals.toml`: how much documentation each module in scope
/// has, counted by music21's own `DocTestFinder`.
#[derive(Debug, Default, Deserialize)]
struct DoctestTotals {
    #[serde(default)]
    module: Vec<DoctestTotal>,
}

#[derive(Debug, Deserialize)]
struct DoctestTotal {
    module: String,
    docstrings: usize,
    examples: usize,
    #[serde(default)]
    tests: usize,
}

/// How much of one music21 module's own documentation runs against the crate.
#[derive(Debug, Serialize, Deserialize)]
struct ModuleDoctests {
    name: String,
    module: String,
    docstrings_passing: usize,
    docstrings: usize,
    examples_passing: usize,
    examples: usize,
    tests_passing: usize,
    tests: usize,
    /// False for a module in scope that no parity harness runs yet, which is
    /// counted at nought rather than left out.
    #[serde(default = "yes")]
    harnessed: bool,
}

fn yes() -> bool {
    true
}

/// One runnable test suite, and what came of running it.
#[derive(Debug, Serialize, Deserialize)]
struct Suite {
    name: String,
    command: String,
    status: SuiteStatus,
    passed: usize,
    failed: usize,
    /// Why a suite was skipped, or what a run of it produced.
    detail: Option<String>,
    /// What the suite actually measures, in a sentence.
    ///
    /// The name and the command say what was run; this says what running it
    /// proves, which for the ones that reach outside this repository is not
    /// obvious from either. Optional so an older `report.json` still reads.
    #[serde(default)]
    note: Option<String>,
}

impl Suite {
    /// Attaches the sentence saying what this suite measures.
    fn describing(mut self, note: &str) -> Self {
        self.note = Some(note.to_string());
        self
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum SuiteStatus {
    Passed,
    Failed,
    Skipped,
}

impl SuiteStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Coverage {
    lines: Percent,
    functions: Percent,
    regions: Percent,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Percent {
    count: u64,
    covered: u64,
    percent: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClassReport {
    python: String,
    name: String,
    note: Option<String>,
    ported: usize,
    missing: usize,
    /// The same count for the wheel, which is held to the harder standard:
    /// the crate may leave a member out, but a member the facade lacks is one
    /// an existing program loses when it installs over music21.
    #[serde(default)]
    in_wheel: usize,
    members: Vec<MemberReport>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemberReport {
    name: String,
    status: Status,
    /// The Rust name for a ported member, or why an unported one is unported.
    detail: Option<String>,
    /// Whether the Python wheel carries it, which is asked separately.
    #[serde(default)]
    in_wheel: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Ported,
    Missing,
}

pub(crate) struct Options {
    pub out: PathBuf,
    pub coverage: bool,
    pub suites: bool,
    pub benchmarks: bool,
    pub features: bool,
    /// Re-render the page from a `report.json` an earlier run wrote, measuring
    /// nothing. Working on the page's layout otherwise means waiting for
    /// coverage and every suite to run again.
    pub from_json: Option<PathBuf>,
}

pub(crate) fn parse_options(workspace_root: &Path, args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        out: workspace_root.join("target/reports"),
        coverage: true,
        suites: true,
        benchmarks: true,
        features: true,
        from_json: None,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let path = args.next().ok_or("--out needs a directory")?;
                options.out = workspace_root.join(path);
            }
            "--features-only" => {
                options.coverage = false;
                options.suites = false;
                options.benchmarks = false;
            }
            "--coverage-only" => {
                options.features = false;
                options.suites = false;
                options.benchmarks = false;
            }
            "--suites-only" => {
                options.coverage = false;
                options.features = false;
                options.benchmarks = false;
            }
            "--benchmarks-only" => {
                options.coverage = false;
                options.suites = false;
                options.features = false;
            }
            "--no-suites" => options.suites = false,
            "--no-benchmarks" => options.benchmarks = false,
            "--from-json" => {
                let path = args.next().ok_or("--from-json needs a report.json")?;
                options.from_json = Some(workspace_root.join(path));
            }
            other => return Err(format!("unknown report option {other:?}")),
        }
    }
    Ok(options)
}

/// Writes the report and whatever pages hang off it.
///
/// The timings page is written only where there are timings, and removed
/// where there are not: a stale copy left beside a report that no longer
/// links to it would be a page describing a run nobody made.
fn write_pages(out: &Path, report: &Report) -> Result<(), Box<dyn Error>> {
    fs::write(out.join("index.html"), render_html(report))?;
    let page = out.join(TIMINGS_PAGE);
    match report
        .timings
        .as_ref()
        .filter(|timings| !timings.is_empty())
    {
        Some(timings) => {
            fs::write(&page, render_timings_page(report, timings))?;
            println!("wrote {}", page.display());
        }
        None => {
            let _ = fs::remove_file(&page);
        }
    }
    Ok(())
}

pub(crate) fn report(workspace_root: &Path, options: &Options) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&options.out)?;

    if let Some(path) = &options.from_json {
        let report: Report = serde_json::from_str(&fs::read_to_string(path)?)?;
        let page = options.out.join("index.html");
        write_pages(&options.out, &report)?;
        println!("wrote {} from {}", page.display(), path.display());
        return Ok(());
    }

    // Coverage is not a run of its own: it is what the suites leave behind.
    // Every suite is run under the instrumentation `cargo llvm-cov show-env`
    // describes, so the profiles they all write merge into one figure — the
    // parity suite and music21's own doctests included.
    let coverage_env = if options.coverage {
        Some(start_coverage(workspace_root)?)
    } else {
        None
    };
    let env = coverage_env.as_deref().unwrap_or(&[]);

    // The suites have to run for coverage even when their own section is not
    // wanted, because they are the measurement.
    let measured = if options.suites || options.coverage {
        run_suites(workspace_root, env)
    } else {
        Vec::new()
    };

    let coverage = match &coverage_env {
        Some(env) => Some(collect_coverage(workspace_root, &options.out, env)?),
        None => None,
    };

    let suites = if options.suites { measured } else { Vec::new() };

    let benchmarks = options.benchmarks.then(|| run_benchmarks(workspace_root));

    let doctests = read_doctests(workspace_root);
    let timings = read_timings(workspace_root);

    let (features, beyond, beyond_members) = if options.features {
        let map_path = workspace_root.join("data/feature_map.toml");
        let map: FeatureMap = toml::from_str(&fs::read_to_string(&map_path)?)
            .map_err(|err| format!("{} does not parse: {err}", map_path.display()))?;
        let claims = claimed_names(workspace_root, &map)?;
        (
            scan_features(workspace_root, &map)?,
            scan_beyond(workspace_root, &map.beyond)?,
            surface::beyond_music21(workspace_root, &claims.ported, &claims.known),
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };

    let report = Report {
        generated_from: git_head(workspace_root),
        music21_version: submodule_version(workspace_root)?,
        coverage,
        sizes: Some(measure_sizes(workspace_root)),
        suites,
        benchmarks,
        doctests,
        timings,
        features,
        beyond_members,
        beyond,
    };

    fs::write(
        options.out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    write_pages(&options.out, &report)?;

    println!("wrote {}", options.out.join("index.html").display());
    if let Some(coverage) = &report.coverage {
        println!(
            "  coverage: {:.2}% of lines, {:.2}% of functions, {:.2}% of regions",
            coverage.lines.percent, coverage.functions.percent, coverage.regions.percent
        );
    }
    for suite in &report.suites {
        match suite.status {
            SuiteStatus::Skipped => println!(
                "  {}: skipped ({})",
                suite.name,
                suite.detail.as_deref().unwrap_or("no reason given")
            ),
            status => println!(
                "  {}: {}, {} passed, {} failed",
                suite.name,
                status.label(),
                suite.passed,
                suite.failed
            ),
        }
    }
    for module in &report.doctests {
        println!(
            "  {} doctests: {} of {} docstrings, {} of {} examples",
            module.module,
            module.docstrings_passing,
            module.docstrings,
            module.examples_passing,
            module.examples
        );
    }
    for class in &report.features {
        let counted = class.ported + class.missing;
        println!("  {}: {} of {} ported", class.name, class.ported, counted);
    }
    Ok(())
}

fn git_head(workspace_root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn submodule_version(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    let path = workspace_root.join("music21/music21/_version.py");
    let text = fs::read_to_string(&path).map_err(|err| {
        format!(
            "{} is unreadable ({err}); run `git submodule update --init --recursive`",
            path.display()
        )
    })?;
    text.lines()
        .find_map(|line| {
            let rest = line.trim().strip_prefix("__version__")?;
            let rest = rest.trim().strip_prefix('=')?.trim();
            Some(rest.trim_matches(|c| c == '\'' || c == '"').to_string())
        })
        .ok_or_else(|| format!("no __version__ in {}", path.display()).into())
}

/// What music21's own suite made of each module the crate ports, read out of
/// the run `xtask music21-suite` last wrote.
///
/// The suite is one run over the whole of music21 and reports one comparison;
/// this is the same run attributed back per module, which is what the
/// unit-test column of the doctest table says. Absent — no suite run here —
/// the column falls back to nought against the fixture's totals.
fn read_module_tests(workspace_root: &Path) -> Vec<ModuleTests> {
    let path = workspace_root.join("target/music21-suite/music21_rs.json");
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(report) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    report
        .get("modules")
        .cloned()
        .and_then(|modules| serde_json::from_value(modules).ok())
        .unwrap_or_default()
}

/// What the last run of music21's own suite said about time, if there is one.
fn read_timings(workspace_root: &Path) -> Option<Timings> {
    let path = workspace_root.join("target/music21-suite/timings.json");
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Reads the summaries the parity doctest harness writes beside its logs,
/// one per music21 module it runs. They are written by the parity suite, so
/// they describe the run that just happened when the suites ran, and the last
/// one otherwise.
fn read_doctests(workspace_root: &Path) -> Vec<ModuleDoctests> {
    let Ok(entries) = fs::read_dir(workspace_root.join("target")) else {
        return Vec::new();
    };
    // The doctest totals come from the fixture whether a harness covers the
    // module or not, so a module with no harness is counted at nought rather
    // than left off the page.
    let totals = read_doctest_totals(workspace_root);
    // The unit-test half comes from music21's own suite, which runs all of
    // them; what it made of each module in scope is read back here.
    let ran = read_module_tests(workspace_root);
    let unit_tests = |module: &str| -> (usize, usize) {
        if let Some(row) = ran.iter().find(|row| row.module == module) {
            return (row.tests_passing, row.tests);
        }
        let total = totals
            .iter()
            .find(|total| total.module == module)
            .map(|total| total.tests)
            .unwrap_or(0);
        (0, total)
    };

    let mut modules = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(name) = name
            .strip_prefix("doctest_")
            .and_then(|name| name.strip_suffix(".toml"))
        else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(summary) = toml::from_str::<DoctestSummary>(&text) else {
            continue;
        };
        let module = summary.module;
        let (tests_passing, tests) = unit_tests(&module);
        modules.push(ModuleDoctests {
            name: name.to_string(),
            docstrings_passing: summary.docstrings_passing,
            docstrings: summary.docstrings,
            examples_passing: summary.examples_passing,
            examples: summary.examples,
            tests_passing,
            tests,
            module,
            harnessed: true,
        });
    }
    for total in &totals {
        if modules.iter().any(|m| m.module == total.module) {
            continue;
        }
        // A module whose *docstrings* no parity harness runs yet. Leaving it
        // off the page would flatter the score, so it goes in at nought
        // against the total music21's own DocTestFinder counted. Its unit
        // tests are another matter: music21's own suite runs every module's,
        // harness here or no, so that column is the real figure either way.
        let name = total
            .module
            .rsplit('.')
            .next()
            .unwrap_or(&total.module)
            .to_string();
        let (tests_passing, tests) = unit_tests(&total.module);
        modules.push(ModuleDoctests {
            name,
            module: total.module.clone(),
            docstrings_passing: 0,
            docstrings: total.docstrings,
            examples_passing: 0,
            examples: total.examples,
            tests_passing,
            tests,
            harnessed: false,
        });
    }
    modules.sort_by(|a, b| b.examples.cmp(&a.examples).then(a.name.cmp(&b.name)));
    modules
}

/// The checked-in totals for every music21 module the crate ports, generated
/// from music21 by `xtask regenerate-fixtures`. Absent on a tree that has
/// never generated it, in which case the page simply says less.
fn read_doctest_totals(workspace_root: &Path) -> Vec<DoctestTotal> {
    let path = workspace_root.join("data/doctest_totals.toml");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    toml::from_str::<DoctestTotals>(&text)
        .map(|totals| totals.module)
        .unwrap_or_default()
}

/// Adds up every file under a directory.
fn directory_size(path: &Path) -> Option<u64> {
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    (total > 0).then_some(total)
}

/// How big each of the three things is to install. Nothing here is built for
/// the measurement: the wheel is whatever the wheel job left behind, and a
/// figure with nothing to measure is simply absent.
fn measure_sizes(workspace_root: &Path) -> Sizes {
    // What `cargo package` ships: `src`, and the loose example files. The
    // example *crates* are workspace members of their own and never go in the
    // `.crate`, so counting their build output would overstate this twofold.
    let loose_examples: u64 = fs::read_dir(workspace_root.join("examples"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "rs"))
                .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
                .sum()
        })
        .unwrap_or(0);
    let crate_source = directory_size(&workspace_root.join("src")).unwrap_or(0) + loose_examples;

    let wheel = fs::read_dir(workspace_root.join("target/wheels"))
        .ok()
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "whl")
                })
                .filter_map(|entry| entry.metadata().ok().map(|meta| meta.len()))
                .max()
        })
        .unwrap_or_default();

    Sizes {
        music21: directory_size(&workspace_root.join("music21/music21")),
        crate_source: (crate_source > 0).then_some(crate_source),
        wheel,
    }
}

/// What the feature map accounts for: the ports it matched, file by file, and
/// every name music21 uses anywhere.
struct Claims {
    /// `(rust file, name)` pairs the map matched — a real port.
    ported: BTreeSet<(String, String)>,
    /// Every name music21 has, matched or not.
    known: BTreeSet<String>,
}

#[cfg(test)]
mod tests {
    /// The suite notes are written as sentences with `code` spans in them, and
    /// the page has to escape the prose without eating the markup.
    #[test]
    fn prose_escapes_the_sentence_and_keeps_its_code_spans() {
        assert_eq!(
            super::prose("run `cargo test` first"),
            "run <code>cargo test</code> first"
        );
        assert_eq!(super::prose("no spans here"), "no spans here");
        assert_eq!(
            super::prose("a & b <c>"),
            "a &amp; b &lt;c&gt;",
            "the prose still has to be escaped"
        );
        assert_eq!(
            super::prose("`a` and `b`"),
            "<code>a</code> and <code>b</code>"
        );
        // An unbalanced backtick keeps the words and loses only the markup.
        assert!(super::prose("half `open").contains("half "));
        assert!(!super::prose("").contains("code"));
    }

    use super::*;

    #[test]
    fn snake_case_follows_the_crate_convention() {
        assert_eq!(snake_case("isMajorTriad"), "is_major_triad");
        assert_eq!(snake_case("pitchClasses"), "pitch_classes");
        assert_eq!(snake_case("forteClassTnI"), "forte_class_tn_i");
        assert_eq!(snake_case("hasZRelation"), "has_z_relation");
        assert_eq!(snake_case("areZRelations"), "are_z_relations");
        assert_eq!(
            snake_case("correctRNAlterationForMinor"),
            "correct_rn_alteration_for_minor"
        );
        assert_eq!(snake_case("root"), "root");
        assert_eq!(snake_case("mod7inversion"), "mod7inversion");
        assert_eq!(
            snake_case("getHistoricalRowByName"),
            "get_historical_row_by_name"
        );
    }

    #[test]
    fn a_leading_get_is_optional() {
        let none = BTreeMap::new();
        assert_eq!(
            candidates("getChordStep", &none, Members::Methods),
            ["get_chord_step", "chord_step"]
        );
        let mut renames = BTreeMap::new();
        renames.insert("forteClassTnI".to_string(), "forte_class_tni".to_string());
        assert_eq!(
            candidates("forteClassTnI", &renames, Members::Methods),
            ["forte_class_tni", "forte_class_tn_i"]
        );
    }

    #[test]
    fn methods_are_read_per_class_without_privates_or_duplicates() {
        let python = "class A(Base):\n    def one(self):\n        pass\n    @property\n    def two(self):\n        pass\n    @two.setter\n    def two(self, v):\n        pass\n    def _hidden(self):\n        pass\nclass B:\n    def three(self):\n        pass\ndef top():\n    pass\n";
        assert_eq!(class_methods(python, "A"), ["one", "two"]);
        assert_eq!(class_methods(python, "B"), ["three"]);
        assert_eq!(module_functions(python), ["top"]);
        assert_eq!(module_classes(python, None), ["A", "B"]);
        assert_eq!(module_classes(python, Some("B")), ["B"]);
    }

    #[test]
    fn cargo_test_counts_sum_every_binary() {
        let text = "running 424 tests\ntest result: ok. 424 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.85s\n\nrunning 9 tests\ntest result: FAILED. 7 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n";
        assert_eq!(cargo_test_counts(text), (431, 2));
        assert_eq!(cargo_test_counts("nothing to see"), (0, 0));
    }

    #[test]
    fn pytest_counts_read_the_summary_line() {
        assert_eq!(pytest_counts("...\n19 passed in 0.07s\n"), (19, 0));
        assert_eq!(
            pytest_counts("F..\n2 failed, 17 passed in 0.11s\n"),
            (17, 2)
        );
        assert_eq!(pytest_counts("collected 0 items\n"), (0, 0));
    }

    #[test]
    fn a_missing_module_is_read_off_the_traceback() {
        assert_eq!(
            missing_module("ModuleNotFoundError: No module named 'music21_rs'").as_deref(),
            Some("music21_rs")
        );
        assert_eq!(
            missing_module("No module named pytest").as_deref(),
            Some("pytest")
        );
        assert_eq!(missing_module("19 passed in 0.07s"), None);
    }

    #[test]
    fn the_built_wheel_is_named_from_maturin_output() {
        let stdout =
            "Built wheel for CPython 3.13 to target/wheels/music21_rs-0.3.0-cp313-win_amd64.whl";
        assert_eq!(
            built_wheel_name(stdout).as_deref(),
            Some("music21_rs-0.3.0-cp313-win_amd64.whl")
        );
        assert_eq!(built_wheel_name("nothing was built"), None);
    }

    #[test]
    fn definitions_are_found_by_kind() {
        let rust = "impl X {\n    pub fn is_triad(&self) -> bool {}\n    pub const fn len() -> usize {}\n    fn private(&self) {}\n}\npub enum ScaleType { Major, HarmonicMinor }\n";
        assert!(defines(rust, "is_triad", Members::Methods));
        assert!(defines(rust, "len", Members::Methods));
        assert!(!defines(rust, "private", Members::Methods));
        assert!(!defines(rust, "is_tria", Members::Methods));
        assert!(defines(rust, "HarmonicMinor", Members::Classes));
        assert!(!defines(rust, "Minor", Members::Classes));
    }
}
