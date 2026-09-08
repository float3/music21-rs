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

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    beyond: Vec<BeyondReport>,
}

/// The crate timed against music21 through the same Python API, by
/// `python/benchmarks/bench.py`. Both sides are asked the same question and
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

/// One benchmark case, as `bench.py --json` writes it.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct BenchCase {
    case: String,
    group: String,
    #[serde(default)]
    notes: String,
    music21_ns: f64,
    music21_rs_ns: f64,
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
    /// Written once something runs music21's unit tests against the crate.
    /// Until then the report shows nought against the total in the fixture.
    #[serde(default)]
    tests_passing: usize,
    #[serde(default)]
    tests: Option<usize>,
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
    members: Vec<MemberReport>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemberReport {
    name: String,
    status: Status,
    /// The Rust name for a ported member, or why an unported one is unported.
    detail: Option<String>,
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

pub(crate) fn report(workspace_root: &Path, options: &Options) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(&options.out)?;

    if let Some(path) = &options.from_json {
        let report: Report = serde_json::from_str(&fs::read_to_string(path)?)?;
        let page = options.out.join("index.html");
        fs::write(&page, render_html(&report))?;
        println!("wrote {} from {}", page.display(), path.display());
        return Ok(());
    }

    // Coverage is not a run of its own: it is what the suites leave behind.
    // Every suite is run under the instrumentation `cargo llvm-cov show-env`
    // describes, so the profiles they all write merge into one figure — the
    // parity suite and music21's own doctests included, which used to drive
    // thousands of lines that the report then called uncovered.
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

    let (features, beyond) = if options.features {
        let map_path = workspace_root.join("data/feature_map.toml");
        let map: FeatureMap = toml::from_str(&fs::read_to_string(&map_path)?)
            .map_err(|err| format!("{} does not parse: {err}", map_path.display()))?;
        (
            scan_features(workspace_root, &map)?,
            scan_beyond(workspace_root, &map.beyond)?,
        )
    } else {
        (Vec::new(), Vec::new())
    };

    let report = Report {
        generated_from: git_head(workspace_root),
        music21_version: submodule_version(workspace_root)?,
        coverage,
        sizes: Some(measure_sizes(workspace_root)),
        suites,
        benchmarks,
        doctests,
        features,
        beyond,
    };

    fs::write(
        options.out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    fs::write(options.out.join("index.html"), render_html(&report))?;

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

/// Wipes any previous profile data and returns the environment
/// `cargo llvm-cov show-env` describes, so that every suite run with it is
/// instrumented and writes its profile into one place.
///
/// `CARGO_TARGET_DIR` is added to it: `python-parity` is outside the
/// workspace and would otherwise build into its own target directory, where
/// the report step could not find its binaries to map the profiles onto.
fn start_coverage(workspace_root: &Path) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let cleaned = Command::new("cargo")
        .args(["llvm-cov", "clean", "--workspace"])
        .current_dir(workspace_root)
        .status()
        .map_err(|err| {
            format!("could not run cargo llvm-cov ({err}); is cargo-llvm-cov installed?")
        })?;
    if !cleaned.success() {
        return Err("cargo llvm-cov clean failed".into());
    }

    let output = Command::new("cargo")
        .args(["llvm-cov", "show-env"])
        .current_dir(workspace_root)
        .output()
        .map_err(|err| {
            format!("could not run cargo llvm-cov ({err}); is cargo-llvm-cov installed?")
        })?;
    if !output.status.success() {
        return Err("cargo llvm-cov show-env failed".into());
    }

    let mut env: Vec<(String, String)> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            (
                key.trim().to_string(),
                value.trim().trim_matches('\'').to_string(),
            )
        })
        .collect();
    if env.is_empty() {
        return Err("cargo llvm-cov show-env printed no environment".into());
    }
    let build_dir = env
        .iter()
        .find(|(key, _)| key == "CARGO_LLVM_COV_BUILD_DIR" || key == "CARGO_LLVM_COV_TARGET_DIR")
        .map(|(_, value)| value.clone());
    if let Some(dir) = build_dir {
        env.push(("CARGO_TARGET_DIR".to_string(), dir));
    }
    Ok(env)
}

/// Merges what the suites left behind into one figure, and writes the
/// file-by-file HTML beside the page.
fn collect_coverage(
    workspace_root: &Path,
    out: &Path,
    env: &[(String, String)],
) -> Result<Coverage, Box<dyn Error>> {
    let html_dir = out.join("coverage");
    let common = [
        "llvm-cov",
        "report",
        "--ignore-filename-regex",
        COVERAGE_IGNORE,
    ];

    let status = Command::new("cargo")
        .args(common)
        .arg("--html")
        .arg("--output-dir")
        .arg(&html_dir)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .current_dir(workspace_root)
        .status()
        .map_err(|err| format!("could not run cargo llvm-cov ({err})"))?;
    if !status.success() {
        return Err("cargo llvm-cov report --html failed".into());
    }

    let output = Command::new("cargo")
        .args(common)
        .args(["--json", "--summary-only"])
        .envs(env.iter().map(|(key, value)| (key, value)))
        .current_dir(workspace_root)
        .output()?;
    if !output.status.success() {
        return Err("cargo llvm-cov report failed".into());
    }
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let totals = summary
        .pointer("/data/0/totals")
        .ok_or("cargo llvm-cov report wrote no totals")?;
    let percent = |key: &str| -> Result<Percent, Box<dyn Error>> {
        let entry = totals
            .get(key)
            .ok_or_else(|| format!("coverage totals carry no {key}"))?;
        let field = |name: &str| entry.get(name).and_then(serde_json::Value::as_f64);
        Ok(Percent {
            count: field("count").unwrap_or(0.0) as u64,
            covered: field("covered").unwrap_or(0.0) as u64,
            percent: field("percent").unwrap_or(0.0),
        })
    };
    Ok(Coverage {
        lines: percent("lines")?,
        functions: percent("functions")?,
        regions: percent("regions")?,
    })
}

/// Runs every suite the repository has and records what each one answered.
/// A suite whose tooling is missing is skipped with the reason rather than
/// failing the report, so this runs anywhere and says what it could not reach.
fn run_suites(workspace_root: &Path, env: &[(String, String)]) -> Vec<Suite> {
    let submodule = workspace_root.join("music21/music21/__init__.py");
    let mut suites = vec![
        cargo_suite(
            workspace_root,
            "Workspace",
            &["test", "--workspace", "--all-targets"],
            None,
            env,
        )
        .describing(
            "The crate's own unit tests, run against nothing but themselves.              Instrumented, so this is most of the coverage figure above.",
        ),
        // `--all-targets` does not include doctests, so without this the
        // crate's own rustdoc examples are never run here at all. They do
        // not reach the *coverage* figure even so: rustdoc compiles a
        // doctest itself and never sees `RUSTC_WRAPPER`, and folding them in
        // properly needs cargo-llvm-cov's `--doctests`, which is unstable and
        // nightly-only while this repository is pinned to stable.
        cargo_suite(
            workspace_root,
            "Workspace doctests",
            &["test", "--workspace", "--doc"],
            None,
            env,
        )
        .describing(
            "Every example in the crate's own documentation, run as a test.              `--all-targets` above does not include these, so without this              command nothing runs them. They gate, but they do not reach the              coverage figure: rustdoc compiles a doctest itself and never sees              the instrumentation.",
        ),
        cargo_suite(
            workspace_root,
            "Python parity and music21's doctests",
            &[
                "test",
                "--manifest-path",
                "python-parity/Cargo.toml",
                "--",
                "--test-threads=1",
            ],
            (!submodule.exists()).then_some("the music21 submodule is not checked out"),
            env,
        )
        .describing(
            "music21's own docstrings, collected from the submodule and run              against the crate. The classes in music21's modules are replaced              with the music21-shaped facades in `python-parity`, so each              example runs music21's text against our values. Embedded and              linked against the crate, so it is instrumented and counts              towards coverage.",
        ),
    ];
    // The wheel is deliberately built uninstrumented. Its Rust half lives in
    // a `.pyd` inside the installed package rather than under the target
    // directory, so the report step could not find the object to map its
    // profiles onto; and an instrumented wheel is not the artifact CI ships.
    // What it tests of the crate, the parity suite covers far more of anyway.
    let (build, tests) = wheel_suites(workspace_root);
    suites.push(build.describing(
        "Builds the Python package a caller installs to get the crate without          a Rust toolchain. Deliberately uninstrumented: an instrumented wheel          is not the artifact CI ships.",
    ));
    suites.push(tests.describing(
        "The wheel's own pytest suite, run against the installed package          rather than the source tree, so what is tested is what a user gets.",
    ));
    suites.push(music21_suite(workspace_root, &submodule).describing(
        "music21's entire test suite — every `Test` class and every docstring          in every module — run against the crate. Not the suite translated to          Rust: it is music21's own Python suite running against classes whose          insides are Rust. `install_into_music21()` swaps the pyo3 facade          classes, backed by music21-rs values, over music21's own, and the          suite is then run twice — once on music21, once on ours — and the two          sets of failures diffed. music21's suite has failures of its own in          any environment, so the result is that comparison and not a pass          mark: only a test that fails under music21_rs and not under music21          is a gap here. It is the harshest measure the repository has,          reaching the MusicXML importer, the stream machinery, `freezeThaw`          and the corpus. It runs through the installed wheel, so like the          wheel's own tests it adds nothing to the coverage figure.",
    ));
    suites
}

/// Runs music21's own test suite twice — once on music21, once with the
/// crate installed over it — and records the difference.
///
/// Like the wheel's own tests, this runs against the *installed* wheel, whose
/// Rust half is a `.pyd` in site-packages rather than an object under the
/// target directory; so it is not instrumented and adds nothing to the
/// coverage figure. What it adds is the measure: it drives the MusicXML
/// importer, the stream machinery, `freezeThaw` and the corpus, none of which
/// any other suite here reaches.
///
/// music21's suite has failures of its own in any environment, so what is
/// reported is the comparison. `failed` counts everything red under the
/// crate; the status is green only when nothing is red under the crate that
/// was not already red under music21.
fn music21_suite(workspace_root: &Path, submodule: &Path) -> Suite {
    const NAME: &str = "music21's own test suite";
    let python = python_command();
    let command = format!("{python} python/downstream/music21_suite.py");

    let skipped = |detail: String| Suite {
        name: NAME.to_string(),
        command: command.clone(),
        status: SuiteStatus::Skipped,
        passed: 0,
        failed: 0,
        detail: Some(detail),
        note: None,
    };

    if !submodule.exists() {
        return skipped("the music21 submodule is not checked out".to_string());
    }

    let out = workspace_root.join("target/music21-suite");
    let output = Command::new(&python)
        .arg("python/downstream/music21_suite.py")
        .arg("--out")
        .arg(&out)
        .current_dir(workspace_root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => return skipped(format!("could not run {python} ({err})")),
    };

    // The two reports are what the run means; a non-zero exit only says the
    // comparison found something, which is read off them too.
    let read = |name: &str| -> Option<serde_json::Value> {
        serde_json::from_str(&fs::read_to_string(out.join(name)).ok()?).ok()
    };
    let (Some(plain), Some(ours)) = (read("music21.json"), read("music21_rs.json")) else {
        let text = merged(&output);
        return match missing_module(&text) {
            Some(module) => skipped(format!("{module} is not installed in this interpreter")),
            None => Suite {
                name: NAME.to_string(),
                command,
                status: SuiteStatus::Failed,
                passed: 0,
                failed: 0,
                detail: last_line(&text),
                note: None,
            },
        };
    };

    let bad = |report: &serde_json::Value| -> Vec<String> {
        ["failures", "errors"]
            .iter()
            .filter_map(|key| report.get(*key)?.as_array())
            .flatten()
            .filter_map(|case| case.as_str().map(str::to_string))
            .collect()
    };
    let theirs = bad(&plain);
    let mine = bad(&ours);
    let regressions = mine.iter().filter(|case| !theirs.contains(case)).count();
    let run = ours
        .get("run")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default() as usize;

    Suite {
        name: NAME.to_string(),
        command,
        status: if regressions == 0 {
            SuiteStatus::Passed
        } else {
            SuiteStatus::Failed
        },
        passed: run.saturating_sub(mine.len()),
        failed: mine.len(),
        detail: Some(format!(
            "{} of these fail on music21 itself; {regressions} fail only under music21_rs",
            theirs.len()
        )),
        note: None,
    }
}

/// The interpreter the wheel suite should use: whatever `PYO3_PYTHON` names,
/// since that is what the pyo3 crates here link against.
fn python_command() -> String {
    env::var("PYO3_PYTHON").unwrap_or_else(|_| "python".to_string())
}

/// Times the crate against music21 through the same Python API. Both need to
/// be importable — the wheel installed, music21 with its dependencies — so
/// this is skipped with the reason wherever they are not, like the suites.
fn run_benchmarks(workspace_root: &Path) -> Benchmarks {
    let python = python_command();
    let json = workspace_root.join("target/benchmarks.json");
    let command = format!("{python} python/benchmarks/bench.py --json target/benchmarks.json");

    let skipped = |detail: String| Benchmarks {
        status: SuiteStatus::Skipped,
        command: command.clone(),
        detail: Some(detail),
        music21: String::new(),
        python: String::new(),
        platform: String::new(),
        cases: Vec::new(),
    };

    let output = Command::new(&python)
        .arg("python/benchmarks/bench.py")
        .arg("--json")
        .arg(&json)
        .current_dir(workspace_root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => return skipped(format!("could not run {python} ({err})")),
    };
    if !output.status.success() {
        let text = merged(&output);
        return match missing_module(&text) {
            Some(module) => skipped(format!("{module} is not installed in this interpreter")),
            None => Benchmarks {
                status: SuiteStatus::Failed,
                command,
                detail: last_line(&text),
                music21: String::new(),
                python: String::new(),
                platform: String::new(),
                cases: Vec::new(),
            },
        };
    }

    let Ok(text) = fs::read_to_string(&json) else {
        return skipped(format!("{} was not written", json.display()));
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) else {
        return skipped(format!("{} does not parse", json.display()));
    };
    let string = |key: &str| {
        parsed
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let cases: Vec<BenchCase> = parsed
        .get("results")
        .cloned()
        .and_then(|results| serde_json::from_value(results).ok())
        .unwrap_or_default();

    Benchmarks {
        status: if cases.is_empty() {
            SuiteStatus::Skipped
        } else {
            SuiteStatus::Passed
        },
        command,
        detail: cases.is_empty().then(|| "no case was timed".to_string()),
        music21: string("music21"),
        python: string("python"),
        platform: string("platform"),
        cases,
    }
}

/// The median speedup, which is what the headline figure quotes: one very
/// fast case should not speak for the rest.
fn median_speedup(cases: &[BenchCase]) -> f64 {
    if cases.is_empty() {
        return 0.0;
    }
    let mut speedups: Vec<f64> = cases.iter().map(|case| case.speedup).collect();
    speedups.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    speedups[speedups.len() / 2]
}

/// `bench.py`'s own rendering of a duration, so the page and the terminal
/// agree.
fn humanise(nanoseconds: f64) -> String {
    if nanoseconds < 1_000.0 {
        format!("{nanoseconds:.0} ns")
    } else if nanoseconds < 1_000_000.0 {
        format!("{:.1} \u{b5}s", nanoseconds / 1_000.0)
    } else {
        format!("{:.1} ms", nanoseconds / 1_000_000.0)
    }
}

fn cargo_suite(
    workspace_root: &Path,
    name: &str,
    args: &[&str],
    skip: Option<&str>,
    env: &[(String, String)],
) -> Suite {
    let command = format!("cargo {}", args.join(" "));
    if let Some(reason) = skip {
        return Suite {
            name: name.to_string(),
            command,
            status: SuiteStatus::Skipped,
            passed: 0,
            failed: 0,
            detail: Some(reason.to_string()),
            note: None,
        };
    }
    let output = Command::new("cargo")
        .args(args)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .current_dir(workspace_root)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(err) => {
            return Suite {
                name: name.to_string(),
                command,
                status: SuiteStatus::Skipped,
                passed: 0,
                failed: 0,
                detail: Some(format!("could not run cargo ({err})")),
                note: None,
            };
        }
    };
    let (passed, failed) = cargo_test_counts(&String::from_utf8_lossy(&output.stdout));
    Suite {
        name: name.to_string(),
        command,
        status: if output.status.success() {
            SuiteStatus::Passed
        } else {
            SuiteStatus::Failed
        },
        passed,
        failed,
        detail: None,
        note: None,
    }
}

/// Builds the wheel the way CI does, then runs its own suite against it. Those
/// tests import `music21_rs`, so they only run where the wheel is installed;
/// this never installs it.
fn wheel_suites(workspace_root: &Path) -> (Suite, Suite) {
    const BUILD: &str = "Python wheel";
    const TESTS: &str = "Python wheel tests";

    let wheels = workspace_root.join("target/wheels");
    let command = "maturin build --release --manifest-path python/Cargo.toml".to_string();
    let built = Command::new("maturin")
        .args(["build", "--release", "--manifest-path", "python/Cargo.toml"])
        .arg("--out")
        .arg(&wheels)
        .current_dir(workspace_root)
        .output();
    let build = match built {
        Err(err) => Suite {
            name: BUILD.to_string(),
            command,
            status: SuiteStatus::Skipped,
            passed: 0,
            failed: 0,
            detail: Some(format!("maturin is not installed ({err})")),
            note: None,
        },
        Ok(output) if output.status.success() => Suite {
            name: BUILD.to_string(),
            command,
            status: SuiteStatus::Passed,
            passed: 0,
            failed: 0,
            detail: built_wheel_name(&merged(&output)),
            note: None,
        },
        Ok(output) => Suite {
            name: BUILD.to_string(),
            command,
            status: SuiteStatus::Failed,
            passed: 0,
            failed: 0,
            detail: last_line(&merged(&output)),
            note: None,
        },
    };

    let python = python_command();
    let command = "python -m pytest python/tests".to_string();
    let ran = Command::new(&python)
        .args(["-m", "pytest", "python/tests", "-q"])
        .current_dir(workspace_root)
        .output();
    let tests = match ran {
        Err(err) => Suite {
            name: TESTS.to_string(),
            command,
            status: SuiteStatus::Skipped,
            passed: 0,
            failed: 0,
            detail: Some(format!("could not run {python} ({err})")),
            note: None,
        },
        Ok(output) => {
            let text = merged(&output);
            match missing_module(&text) {
                Some(missing) => Suite {
                    name: TESTS.to_string(),
                    command,
                    status: SuiteStatus::Skipped,
                    passed: 0,
                    failed: 0,
                    detail: Some(format!("{python} has no {missing}")),
                    note: None,
                },
                None => {
                    let (passed, failed) = pytest_counts(&text);
                    Suite {
                        name: TESTS.to_string(),
                        command,
                        status: if output.status.success() {
                            SuiteStatus::Passed
                        } else {
                            SuiteStatus::Failed
                        },
                        passed,
                        failed,
                        detail: None,
                        note: None,
                    }
                }
            }
        }
    };
    (build, tests)
}

/// Everything a command wrote, on either stream: maturin reports the wheel it
/// built on stderr, and a Python import error lands there too.
fn merged(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Sums the `test result:` lines libtest writes, one per test binary.
fn cargo_test_counts(text: &str) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("test result:") else {
            continue;
        };
        let words: Vec<&str> = rest.split_whitespace().collect();
        for pair in words.windows(2) {
            match (pair[0].parse::<usize>(), pair[1].trim_end_matches(';')) {
                (Ok(count), "passed") => passed += count,
                (Ok(count), "failed") => failed += count,
                _ => {}
            }
        }
    }
    (passed, failed)
}

/// Reads pytest's one-line summary, `19 passed in 0.07s` or
/// `2 failed, 17 passed in 0.11s`.
fn pytest_counts(text: &str) -> (usize, usize) {
    for line in text.lines().rev() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut passed = 0;
        let mut failed = 0;
        let mut seen = false;
        for pair in words.windows(2) {
            match (pair[0].parse::<usize>(), pair[1].trim_end_matches(',')) {
                (Ok(count), "passed") => {
                    passed += count;
                    seen = true;
                }
                (Ok(count), "failed") => {
                    failed += count;
                    seen = true;
                }
                _ => {}
            }
        }
        if seen {
            return (passed, failed);
        }
    }
    (0, 0)
}

/// The module a Python run could not import, when that is why it failed.
fn missing_module(text: &str) -> Option<String> {
    let marker = "No module named ";
    let start = text.find(marker)? + marker.len();
    let name: String = text[start..]
        .trim_start_matches(['\'', '"'])
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn built_wheel_name(stdout: &str) -> Option<String> {
    let line = stdout.lines().find(|line| line.contains("Built wheel"))?;
    let (_, path) = line.rsplit_once(' ')?;
    Path::new(path.trim())
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn last_line(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// Reads the summaries the parity doctest harness writes beside its logs,
/// one per music21 module it runs. They are written by the parity suite, so
/// they describe the run that just happened when the suites ran, and the last
/// one otherwise.
fn read_doctests(workspace_root: &Path) -> Vec<ModuleDoctests> {
    let Ok(entries) = fs::read_dir(workspace_root.join("target")) else {
        return Vec::new();
    };
    // The unit-test totals come from the fixture whether a harness covers the
    // module or not, since nothing runs music21's unit tests against the crate
    // yet — that column is nought all the way down, and is the to-do list.
    let totals = read_doctest_totals(workspace_root);
    let total_tests = |module: &str| {
        totals
            .iter()
            .find(|total| total.module == module)
            .map(|total| total.tests)
            .unwrap_or(0)
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
        modules.push(ModuleDoctests {
            name: name.to_string(),
            docstrings_passing: summary.docstrings_passing,
            docstrings: summary.docstrings,
            examples_passing: summary.examples_passing,
            examples: summary.examples,
            tests_passing: summary.tests_passing,
            tests: summary.tests.unwrap_or_else(|| total_tests(&module)),
            module,
            harnessed: true,
        });
    }
    for total in &totals {
        if modules.iter().any(|m| m.module == total.module) {
            continue;
        }
        // A module in scope that no harness runs yet. Leaving it off the page
        // would flatter the score, so it goes in at nought against the total
        // music21's own DocTestFinder counted.
        let name = total
            .module
            .rsplit('.')
            .next()
            .unwrap_or(&total.module)
            .to_string();
        modules.push(ModuleDoctests {
            name,
            module: total.module.clone(),
            docstrings_passing: 0,
            docstrings: total.docstrings,
            examples_passing: 0,
            examples: total.examples,
            tests_passing: 0,
            tests: total.tests,
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

/// The declared length of a `const NAME: [T; N]`, which is the one place a
/// table's size is written down and cannot be wrong.
fn declared_length(rust: &str, name: &str) -> Option<usize> {
    let start = rust.find(&format!("{name}:"))? + name.len() + 1;
    let rest = &rust[start..];
    let close = rest.find(']')?;
    let (_, count) = rest[..close].rsplit_once(';')?;
    count.trim().parse().ok()
}

/// Counts what the crate has beyond music21, reading each number out of the
/// source so none of them can go stale silently.
fn scan_beyond(
    workspace_root: &Path,
    map: &[BeyondMap],
) -> Result<Vec<BeyondReport>, Box<dyn Error>> {
    let mut reports = Vec::new();
    for entry in map {
        let mut count = None;
        if let Some(constant) = &entry.count {
            let relative = entry
                .rust
                .as_deref()
                .ok_or_else(|| format!("{}: a count needs a rust file", entry.name))?;
            let path = workspace_root.join(relative);
            let rust = fs::read_to_string(&path)
                .map_err(|err| format!("{} is unreadable: {err}", path.display()))?;
            count = Some(declared_length(&rust, constant).ok_or_else(|| {
                format!(
                    "{}: {relative} declares no `const {constant}: [_; N]`",
                    entry.name
                )
            })?);
        }
        let mut music21 = None;
        if entry.scala_archive {
            let archive = workspace_root.join("data/scala_archive.toml");
            let text = fs::read_to_string(&archive)
                .map_err(|err| format!("{} is unreadable: {err}", archive.display()))?;
            count = Some(
                text.matches(
                    "
[[scale]]",
                )
                .count()
                    + text.starts_with("[[scale]]") as usize,
            );
            music21 = count_scl_files(&workspace_root.join("music21/music21"));
        }
        reports.push(BeyondReport {
            name: entry.name.clone(),
            note: entry.note.clone(),
            count,
            unit: entry.unit.clone(),
            music21,
        });
    }
    Ok(reports)
}

/// music21's own Scala archive, counted where it ships.
fn count_scl_files(root: &Path) -> Option<usize> {
    let mut found = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "scl") {
                found += 1;
            }
        }
    }
    (found > 0).then_some(found)
}

fn scan_features(
    workspace_root: &Path,
    map: &FeatureMap,
) -> Result<Vec<ClassReport>, Box<dyn Error>> {
    let mut classes = Vec::new();
    let mut problems = Vec::new();

    for class in &map.class {
        let python_path = workspace_root.join("music21/music21").join(&class.python);
        let python = fs::read_to_string(&python_path)
            .map_err(|err| format!("{} is unreadable: {err}", python_path.display()))?;
        let label = match (&class.name, class.members) {
            (Some(name), _) => name.clone(),
            (None, Members::Methods) => format!("{} functions", class.python),
            (None, Members::Classes) => format!("{} classes", class.python),
        };

        let members = match class.members {
            Members::Methods => match &class.name {
                Some(name) => class_methods(&python, name),
                None => module_functions(&python),
            },
            Members::Classes => module_classes(&python, class.suffix.as_deref()),
        };
        if members.is_empty() {
            problems.push(format!("{label}: nothing found in {}", class.python));
        }

        let mut rust = String::new();
        for file in &class.rust {
            let path = workspace_root.join(file);
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("{} is unreadable: {err}", path.display()))?;
            rust.push_str(&text);
            rust.push('\n');
        }

        for excluded in class.excluded.keys() {
            if !members.contains(excluded) {
                problems.push(format!(
                    "{label}: `{excluded}` is excluded but music21 has no such member"
                ));
            }
        }
        for (from, to) in &class.renames {
            if !members.contains(from) {
                problems.push(format!(
                    "{label}: `{from}` is renamed but music21 has no such member"
                ));
            }
            if !defines(&rust, to, class.members) {
                problems.push(format!(
                    "{label}: `{from}` is renamed to `{to}`, which none of {} defines",
                    class.rust.join(", ")
                ));
            }
        }

        let mut reports = Vec::new();
        for member in &members {
            let report = if let Some(reason) = class.excluded.get(member) {
                // Left out on purpose is still left out. The reason is worth
                // showing; carving it out of the total is not.
                MemberReport {
                    name: member.clone(),
                    status: Status::Missing,
                    detail: Some(reason.clone()),
                }
            } else if let Some(found) = candidates(member, &class.renames, class.members)
                .into_iter()
                .find(|candidate| defines(&rust, candidate, class.members))
            {
                MemberReport {
                    name: member.clone(),
                    status: Status::Ported,
                    detail: Some(found),
                }
            } else {
                MemberReport {
                    name: member.clone(),
                    status: Status::Missing,
                    detail: None,
                }
            };
            reports.push(report);
        }

        let count = |status: Status| reports.iter().filter(|r| r.status == status).count();
        classes.push(ClassReport {
            python: class.python.clone(),
            name: label,
            note: class.note.clone(),
            ported: count(Status::Ported),
            missing: count(Status::Missing),
            members: reports,
        });
    }

    if !problems.is_empty() {
        return Err(format!(
            "data/feature_map.toml is out of date:\n  {}",
            problems.join("\n  ")
        )
        .into());
    }
    Ok(classes)
}

/// The public methods and properties of one class, in source order, each once.
fn class_methods(python: &str, class: &str) -> Vec<String> {
    let header = format!("class {class}(");
    let bare = format!("class {class}:");
    let mut inside = false;
    let mut names = Vec::new();
    for line in python.lines() {
        if line.starts_with("class ") {
            inside = line.starts_with(&header) || line.starts_with(&bare);
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(name) = method_name(line, "    def ") {
            push_unique(&mut names, name);
        }
    }
    names
}

/// The public functions defined at the top of a module.
fn module_functions(python: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in python.lines() {
        if let Some(name) = method_name(line, "def ") {
            push_unique(&mut names, name);
        }
    }
    names
}

/// The public classes of a module, optionally only those ending in `suffix`,
/// and never the test cases or the exceptions.
fn module_classes(python: &str, suffix: Option<&str>) -> Vec<String> {
    let mut names = Vec::new();
    for line in python.lines() {
        let Some(rest) = line.strip_prefix("class ") else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty()
            || name.starts_with('_')
            || name.starts_with("Test")
            || name.ends_with("Exception")
            || suffix.is_some_and(|suffix| !name.ends_with(suffix))
        {
            continue;
        }
        push_unique(&mut names, name);
    }
    names
}

fn method_name(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() || name.starts_with('_') {
        return None;
    }
    Some(name)
}

fn push_unique(names: &mut Vec<String>, name: String) {
    if !names.contains(&name) {
        names.push(name);
    }
}

/// The Rust names a music21 member might have been given, most specific
/// first.
fn candidates(member: &str, renames: &BTreeMap<String, String>, members: Members) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(renamed) = renames.get(member) {
        out.push(renamed.clone());
    }
    match members {
        Members::Methods => {
            let snake = snake_case(member);
            out.push(snake.clone());
            if let Some(rest) = snake.strip_prefix("get_") {
                out.push(rest.to_string());
            }
        }
        Members::Classes => out.push(member.to_string()),
    }
    out
}

/// `isMajorTriad` → `is_major_triad`, `forteClassTnI` → `forte_class_tn_i`.
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (index, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let previous_lower = index > 0 && chars[index - 1].is_ascii_lowercase();
            let previous_digit = index > 0 && chars[index - 1].is_ascii_digit();
            let acronym_ends = index > 0
                && chars[index - 1].is_ascii_uppercase()
                && chars.get(index + 1).is_some_and(char::is_ascii_lowercase);
            if previous_lower || previous_digit || acronym_ends {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether the Rust source defines the name: a `pub fn` for a method, or the
/// bare identifier — an enum variant or a type — for a class.
fn defines(rust: &str, name: &str, members: Members) -> bool {
    match members {
        Members::Methods => {
            let plain = format!("pub fn {name}(");
            let plain_generic = format!("pub fn {name}<");
            let constant = format!("pub const fn {name}(");
            rust.contains(&plain) || rust.contains(&plain_generic) || rust.contains(&constant)
        }
        Members::Classes => rust
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .any(|word| word == name),
    }
}

/// Escapes prose for the page, and lets a `backtick` span through as `<code>`.
///
/// The suite notes are written as sentences with the odd command or symbol in
/// them; writing the tags by hand in a Rust string literal would make them
/// unreadable at the point they are edited.
fn prose(text: &str) -> String {
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

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The page's own layout, inlined so the report is one self-contained file.
/// Colours come from the site theme it links to.
const STYLE: &str = include_str!("report.css");

/// Filtering the ported list and highlighting the section in view. The page
/// is complete without it.
const SCRIPT: &str = include_str!("report.js");

/// `1 suite`, `3 suites` — the report says these counts out loud often enough
/// to be worth getting right, and one of the nouns is `class`.
fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("{count} {one}")
    } else {
        format!("{count} {many}")
    }
}

fn share(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

/// A progress bar. `warn` paints it in the warning colour, for a figure that
/// is a shortfall rather than an achievement.
fn meter(percent: f64, warn: bool, extra_class: &str) -> String {
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
fn stacked_meter(ported: usize, missing: usize) -> String {
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
fn section_head(anchor: &str, title: &str, note: &str) -> String {
    format!(
        r#"            <section id="{anchor}">
                <div class="section-head">
                    <h2>{title}</h2>
                    <p class="head-note">{note}</p>
                </div>
"#
    )
}

fn render_coverage(coverage: &Coverage) -> String {
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
        "                <p class=\"section-foot\">Measured by <code>cargo llvm-cov</code> across <em>every</em> suite above, not the library's own unit tests alone: each is run instrumented and their profiles merge, so the crate code that music21's own doctests drive through the parity facades counts here too. The figure is still the library's &mdash; the tooling crates are excluded, as are the generated chord and Scala tables, which are data and whose thousands of lines would say nothing about the code. The wheel's own tests are the one suite outside it: its Rust half ships in the installed package, where the report step cannot reach the object file. <a href=\"./coverage/html/index.html\">Read it file by file</a>.</p>\n            </section>\n",
    );
    html
}

fn render_suites(suites: &[Suite]) -> String {
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
        "                <p class=\"section-foot\">Every suite the repository has, run when this report was generated. A suite whose tooling is not present is skipped rather than failed, and says why. The wheel's own tests import <code>music21_rs</code>, so they need the built wheel installed in the interpreter that runs them. Two of these are <em>comparisons</em> rather than pass marks: music21's own suite, and the downstream library run in CI, are each run twice &mdash; once on music21, once with <code>install_into_music21()</code> in front of it &mdash; and what counts is that the two sets of failures match. Both subjects fail a number of their own tests in any environment, so a green result there means the crate changed nothing, not that nothing was red.</p>\n            </section>\n",
    );
    html
}

/// Bytes, as a person reads them.
fn human_bytes(bytes: u64) -> String {
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
fn render_sizes(sizes: &Sizes) -> String {
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
fn render_beyond(beyond: &[BeyondReport]) -> String {
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
    html.push_str(
        "                <p class=\"section-foot\">The counts are read out of the crate's own tables rather than written down here, so one that grows says so by itself and the report fails when a table it names has gone. The rest of the page measures music21-rs against music21; this is the part with nothing to measure against.</p>
            </section>
",
    );
    html
}

fn render_benchmarks(benchmarks: &Benchmarks, sizes: Option<&Sizes>) -> String {
    if benchmarks.cases.is_empty() {
        let mut html = section_head(
            "speedups",
            "Speedups over music21",
            &escape(benchmarks.detail.as_deref().unwrap_or("not run")),
        );
        let _ = write!(
            html,
            "                <p class=\"section-foot\">The benchmarks time the crate against music21 through the same Python API, and need both installed in the interpreter that runs them: <code>{}</code>.</p>\n            </section>\n",
            escape(&benchmarks.command),
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
                        <thead><tr><th>Case</th><th class="num">music21</th><th class="num">music21-rs</th><th class="num">Speedup</th></tr></thead>
                        <tbody>
"#,
    );
    let mut group = "";
    for case in &benchmarks.cases {
        if case.group != group {
            group = &case.group;
            let _ = writeln!(
                html,
                "                            <tr class=\"group-row\"><td colspan=\"4\">{}</td></tr>",
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
                                <td class="num">{fast}</td>
                                <td class="num"><span class="pill {pill}">{speedup:.1}&#215;</span></td>
                            </tr>
"#,
            name = escape(&case.case),
            slow = humanise(case.music21_ns),
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
    let _ = write!(
        html,
        "                <p class=\"section-foot\">The crate and music21 asked the same question through the same Python API by <code>python/benchmarks/bench.py</code>, on music21 {music21} and Python {python} ({platform}). Both sides have to agree on the answer before either is timed, so a speedup here is a speedup at doing the same work. Each case builds a fresh object, because music21 memoizes its analysis on the object that was asked.</p>\n            </section>\n",
        music21 = escape(&benchmarks.music21),
        python = escape(&benchmarks.python),
        platform = escape(&benchmarks.platform),
    );
    html
}

fn render_doctests(doctests: &[ModuleDoctests]) -> String {
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
        "                <p class=\"section-foot\">What music21-rs passes of music21's own documentation and test suite. The docstrings are collected from the submodule and run against the crate through the music21-shaped facades in <code>python-parity</code>. A docstring counts as passing only when every one of its examples does, which is why that share is always the harsher of the two. The failures of each module are written to <code>target/doctest_&lt;module&gt;.log</code>. A module the crate ports but no harness runs yet counts at nought against the total music21's own <code>DocTestFinder</code> gives it, recorded in <code>data/doctest_totals.toml</code>. The unit-test column is music21's own <code>unittest</code> suites, wherever it keeps them &mdash; beside the module, inside its package, or in <code>music21/test/</code>. No <em>harness here</em> runs those module by module, so that column is nought all the way down: it is the to-do list for this table, not a score. They are not unrun, though &mdash; music21's own suite above runs all of them against the crate at once, and reports the difference rather than a per-module share.</p>\n            </section>\n",
    );
    html
}

fn render_features(features: &[ClassReport]) -> String {
    let ported: usize = features.iter().map(|c| c.ported).sum();
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
                            <span class="of">{ported} of {counted}</span>
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
                                <thead><tr><th>music21</th><th>Status</th><th>music21-rs</th></tr></thead>
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
            let _ = writeln!(
                html,
                "                                    <tr><td class=\"name\"><code>{name}</code></td><td><span class=\"{pill}\">{label}</span></td><td>{detail}</td></tr>",
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
                <p class="section-foot">Every public method of the music21 classes the crate ports, read from the submodule, against the <code>pub fn</code>s of the Rust files that port them. A member music21 has is ported or it is not; where the crate has a reason for not porting one, the reason is shown beside it, but it still counts against the total. The mapping lives in <code>data/feature_map.toml</code>, and the report fails when it goes stale.</p>
            </section>
"#
    );
    html
}

fn render_html(report: &Report) -> String {
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
    if let Some(benchmarks) = &report.benchmarks
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
    if let Some(benchmarks) = &report.benchmarks {
        html.push_str(&render_benchmarks(benchmarks, report.sizes.as_ref()));
    }
    if !report.beyond.is_empty() {
        html.push_str(&render_beyond(&report.beyond));
    }
    if !report.doctests.is_empty() {
        html.push_str(&render_doctests(&report.doctests));
    }
    if !report.features.is_empty() {
        html.push_str(&render_features(&report.features));
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
