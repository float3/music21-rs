//! Run music21's own test suite against `music21_rs`.
//!
//! [`crate::downstream`] measures the crate against somebody else's library.
//! This measures it against music21 itself: every `Test` class and every
//! docstring in every music21 module, run twice — once on music21, once with
//! `music21_rs.install_into_music21()` in front of it — with the two sets of
//! failures compared.
//!
//! music21's suite has failures of its own in any given environment (a missing
//! optional package, a platform difference), so the test is a comparison and
//! not a pass mark, exactly as the downstream run is: anything that fails only
//! under `music21_rs` is a gap in the crate.
//!
//! ```text
//! cargo run --release -p xtask --features python -- music21-suite
//! ```
//!
//! The two runs are separate processes, because installing over music21
//! cannot be undone inside one: `install_into_music21` rebinds music21's own
//! classes wherever they were already imported, and there is no putting that
//! back. So this command re-runs *itself* with `--run`, once per side. Needs
//! the `music21` submodule checked out, the `music21_rs` wheel installed, and
//! music21's own test dependencies — `scipy` and `python-Levenshtein` on top
//! of what music21 itself needs.
//!
//! music21's own `testSingleCoreAll.main` is not used: it insists on lilypond
//! being installed before it will run anything. Everything after that check is
//! what happens here.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::{Deserialize, Serialize};

use crate::bench::add_dependency_venv;
use crate::fixtures::{scoped_modules, test_modules_for};

/// Which side a `--run` is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Music21,
    Music21Rs,
}

impl Which {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "music21" => Some(Which::Music21),
            "music21_rs" => Some(Which::Music21Rs),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Which::Music21 => "music21",
            Which::Music21Rs => "music21_rs",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Which::Music21 => "music21.json",
            Which::Music21Rs => "music21_rs.json",
        }
    }
}

/// The tests that fail under `music21_rs` on purpose, and why.
///
/// Both are documented divergences, not gaps: making either one pass costs
/// more than it buys, and both were measured. Anything *not* on this list that
/// fails only under `music21_rs` is a real regression and fails the run — the
/// same shape as `python-parity/doctest/*.toml`, where what passes is listed
/// and a newly broken one is named.
const EXPECTED_DIVERGENCES: &[(&str, &str)] = &[
    (
        "classSet (music21.prebase.ProtoM21Object)",
        "an installed class lists both itself and the class it replaced in \
         classSet, so this doctest counts one more than music21 has. Dropping \
         the replaced class to make it pass costs 250 of music21's own tests.",
    ),
    (
        "testRagAsawari (music21.scale.test_scale_main.Test.testRagAsawari)",
        "music21 reads a degree out of its interval network's realization \
         cache, so the answer depends on what was last asked. Reproducing that \
         would mean modelling IntervalNetwork's node cache.",
    ),
];

/// How one module in scope fared, so that the report can say so per module
/// rather than only in one total.
///
/// The suite is one run over the whole of music21; this is that run's result
/// attributed back to the modules the crate ports, by the same
/// `test_modules_for` walk that counted the denominators — music21 keeps a
/// module's tests three different places, and guessing at the name gets it
/// wrong.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleTests {
    pub module: String,
    pub tests: usize,
    pub tests_passing: usize,
}

/// What one run writes, and what the comparison — and `xtask report` — read.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Report {
    run: usize,
    failures: Vec<String>,
    errors: Vec<String>,
    detail: std::collections::BTreeMap<String, String>,
    /// How long each test took, in seconds, by the same name the failures
    /// are listed under. `unittest` collects these itself when the runner is
    /// asked for durations; nothing here times anything by hand.
    #[serde(default)]
    durations: BTreeMap<String, f64>,
    /// Empty on the music21 side, which nothing reads per module.
    #[serde(default)]
    modules: Vec<ModuleTests>,
}

impl Report {
    /// Everything red, however it went red.
    fn bad(&self) -> std::collections::BTreeSet<&str> {
        self.failures
            .iter()
            .chain(&self.errors)
            .map(String::as_str)
            .collect()
    }
}

/// Runs both halves and compares them, answering the process exit code.
pub fn run(
    workspace_root: &Path,
    only: Option<String>,
    out: Option<PathBuf>,
    one: Option<Which>,
) -> Result<i32, Box<dyn Error>> {
    let out = out.unwrap_or_else(|| workspace_root.join("target/music21-suite"));
    fs::create_dir_all(&out)?;

    if let Some(which) = one {
        return run_one(
            workspace_root,
            &out.join(which.file()),
            which,
            only.as_deref(),
        )
        .map_err(Into::into);
    }

    if !workspace_root.join("music21").exists() {
        eprintln!("the music21 submodule is not checked out");
        return Ok(1);
    }

    let exe = std::env::current_exe()?;
    for which in [Which::Music21, Which::Music21Rs] {
        println!("$ running music21's suite on {}", which.name());
        let mut command = Command::new(&exe);
        command
            .arg("music21-suite")
            .arg("--run")
            .arg(which.name())
            .arg("--out")
            .arg(&out)
            .current_dir(workspace_root);
        if let Some(only) = &only {
            command.arg("--only").arg(only);
        }
        let status = command.status()?;
        if !status.success() {
            eprintln!("the {} run did not finish", which.name());
            return Ok(1);
        }
    }

    let plain = read(&out.join(Which::Music21.file()))?;
    let ours = read(&out.join(Which::Music21Rs.file()))?;
    if let Some(timings) = compare_timings(&plain, &ours) {
        report_timings(&timings);
        let path = out.join("timings.json");
        fs::write(&path, serde_json::to_string_pretty(&timings)?)?;
    }
    compare(&plain, &ours, only.is_none())
}

fn read(path: &Path) -> Result<Report, Box<dyn Error>> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("{} is unreadable ({err})", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

/// One run of the suite, writing what failed to `out`.
fn run_one(workspace_root: &Path, out: &Path, which: Which, only: Option<&str>) -> PyResult<i32> {
    Python::attach(|py| {
        // music21's suite prints test names, and some of them are not
        // spellable in the console's own encoding.
        let sys = py.import("sys")?;
        for stream in ["stdout", "stderr"] {
            let kwargs = PyDict::new(py);
            kwargs.set_item("encoding", "utf-8")?;
            kwargs.set_item("errors", "replace")?;
            let _ = sys
                .getattr(stream)?
                .call_method("reconfigure", (), Some(&kwargs));
        }
        add_dependency_venv(py, workspace_root)?;
        sys.getattr("path")?.cast_into::<PyList>()?.insert(
            0,
            workspace_root.join("music21").to_string_lossy().to_string(),
        )?;

        let music21 = py.import("music21")?;
        println!(
            "music21 {} from {}",
            music21.getattr("__version__")?.extract::<String>()?,
            music21.getattr("__file__")?.extract::<String>()?
        );
        clear_corpus_cache(py)?;
        if which == Which::Music21Rs {
            let ours = py.import("music21_rs")?;
            let names: usize = ours.call_method0("install_into_music21")?.extract()?;
            println!("music21_rs over {names} names");
        }

        // A one-shot process, so the filter is simply set rather than set and
        // put back the way music21's own `catch_warnings` block does it.
        py.import("warnings")?
            .call_method1("simplefilter", ("ignore",))?;

        let unittest = py.import("unittest")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("verbosity", 0)?;
        kwargs.set_item("stream", sys.getattr("stdout")?)?;
        // Asking for durations is what makes `unittest` time each test and
        // keep the list; nought is how many of the slowest to print, which is
        // none. Python 3.12 and up.
        kwargs.set_item("durations", 0)?;
        let result = unittest
            .getattr("TextTestRunner")?
            .call((), Some(&kwargs))?
            .call_method1("run", (build_suite(py, &music21, only)?,))?;

        let mut report = Report {
            run: result.getattr("testsRun")?.extract()?,
            ..Report::default()
        };
        // A test is named two ways: `str(case)` reads well and is what the
        // comparison is written in, and `case.id()` is the dotted path that
        // says which module the test belongs to.
        let mut failed_ids: BTreeSet<String> = BTreeSet::new();
        for (kind, into) in [("failures", true), ("errors", false)] {
            let mut names = Vec::new();
            for pair in result.getattr(kind)?.try_iter()? {
                let pair = pair?;
                let case = pair.get_item(0)?;
                let name: String = case.str()?.extract()?;
                if let Ok(id) = case.call_method0("id")
                    && let Ok(id) = id.extract::<String>()
                {
                    failed_ids.insert(id);
                }
                let text: String = pair.get_item(1)?.extract()?;
                report.detail.insert(name.clone(), text);
                names.push(name);
            }
            names.sort();
            if into {
                report.failures = names;
            } else {
                report.errors = names;
            }
        }
        report.modules = module_tally(py, workspace_root, &failed_ids)?;
        if let Ok(collected) = result.getattr("collectedDurations") {
            for pair in collected.try_iter()? {
                let pair = pair?;
                let name: String = pair.get_item(0)?.str()?.extract()?;
                let seconds: f64 = pair.get_item(1)?.extract()?;
                report.durations.insert(name, seconds);
            }
        }

        let text = serde_json::to_string_pretty(&report)
            .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
        fs::write(out, text)?;
        println!(
            "ran {}, {} failures, {} errors",
            report.run,
            report.failures.len(),
            report.errors.len()
        );
        Ok(0)
    })
}

/// How each module the crate ports fared in the run that just happened.
///
/// The suite runs every module music21 has; this attributes its results back
/// to the ones in scope. Where a module's tests live is found by looking, not
/// by guessing at a name — `music21.meter.base` would otherwise take
/// `music21/test/test_base.py`, which tests `music21.base` and has nothing to
/// do with meter.
///
/// A test music21 itself fails counts here as not passing, because the
/// question this answers is what the crate passes of music21's suite. Whether
/// a failure is the crate's doing is the comparison the two runs make, and
/// that is reported on its own.
fn module_tally(
    py: Python<'_>,
    workspace_root: &Path,
    failed: &BTreeSet<String>,
) -> PyResult<Vec<ModuleTests>> {
    // `test_modules_for` answers where a module's tests *could* be, and two
    // modules of one package get the same answer: `music21/test/test_chord.py`
    // is found for `music21.chord` and for `music21.chord.tables` alike. Only
    // one of them can own it or the column counts those tests twice, so a
    // shared holder goes to the more general module — the shorter name, which
    // is the package's own. A module's own `Test` class is always its own.
    let mut modules = scoped_modules(workspace_root)?;
    modules.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for module in modules {
        let mut holders = vec![module.clone()];
        for holder in test_modules_for(workspace_root, &module) {
            if claimed.insert(holder.clone()) {
                holders.push(holder);
            }
        }
        let mut ids = BTreeSet::new();
        for holder in holders {
            let Ok(imported) = py.import(holder.as_str()) else {
                continue;
            };
            collect_test_ids(py, &imported, &mut ids)?;
        }
        let passing = ids.iter().filter(|id| !failed.contains(*id)).count();
        out.push(ModuleTests {
            module,
            tests: ids.len(),
            tests_passing: passing,
        });
    }
    out.sort_by(|a, b| a.module.cmp(&b.module));
    Ok(out)
}

/// The ids of one module's `Test` class, if it has one.
///
/// `TestExternal` needs a score viewer and music21's own runner leaves it out,
/// so it is left out here too — the same rule the denominators were counted
/// with, or the two would not be comparable.
fn collect_test_ids(
    py: Python<'_>,
    module: &Bound<'_, PyModule>,
    into: &mut BTreeSet<String>,
) -> PyResult<()> {
    let unittest = py.import("unittest")?;
    let Ok(case) = module.getattr("Test") else {
        return Ok(());
    };
    let Ok(case) = case.cast_into::<pyo3::types::PyType>() else {
        return Ok(());
    };
    if !case.is_subclass(&unittest.getattr("TestCase")?)? {
        return Ok(());
    }
    let loaded = unittest
        .getattr("TestLoader")?
        .call0()?
        .call_method1("loadTestsFromTestCase", (case,))?;
    flatten(&loaded, into)
}

/// Every leaf test under a suite, by id. A `TestSuite` holds suites as
/// readily as tests, so this walks rather than iterating once.
fn flatten(suite: &Bound<'_, PyAny>, into: &mut BTreeSet<String>) -> PyResult<()> {
    let Ok(children) = suite.try_iter() else {
        return Ok(());
    };
    for child in children {
        let child = child?;
        if child.try_iter().is_ok() {
            flatten(&child, into)?;
        } else if let Ok(id) = child.call_method0("id")
            && let Ok(id) = id.extract::<String>()
        {
            into.insert(id);
        }
    }
    Ok(())
}

/// Every `Test` class and every docstring music21 has.
fn build_suite<'py>(
    py: Python<'py>,
    music21: &Bound<'py, PyModule>,
    only: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let unittest = py.import("unittest")?;
    let doctest = py.import("doctest")?;
    let options = doctest.getattr("ELLIPSIS")?.extract::<i64>()?
        | doctest.getattr("NORMALIZE_WHITESPACE")?.extract::<i64>()?;

    let common = py.import("music21.common")?;
    let common_test = py.import("music21.test.commonTest")?;
    let test_runner = py.import("music21.test.testRunner")?;

    let suite = unittest.getattr("TestSuite")?.call0()?;
    let loader = unittest.getattr("defaultTestLoader")?;
    let gathered = common_test
        .getattr("ModuleGather")?
        .call0()?
        .call_method1("load", (false,))?;
    let modules = common
        .getattr("misc")?
        .call_method1("sortModules", (gathered,))?;

    for module in modules.try_iter()? {
        let module = module?;
        let name: String = module.getattr("__name__")?.extract()?;
        if only.is_some_and(|only| !name.contains(only)) {
            continue;
        }
        if module.hasattr("Test")? {
            let tests = loader.call_method1("loadTestsFromTestCase", (module.getattr("Test")?,))?;
            suite.call_method1("addTests", (tests,))?;
        }
        // A module `defaultDoctestSuite` refuses has no docstrings to run, and
        // upstream skips the rest of the loop for it rather than reporting it.
        match common_test.call_method1("defaultDoctestSuite", (&module,)) {
            Ok(tests) => {
                suite.call_method1("addTests", (tests,))?;
            }
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => continue,
            Err(error) => return Err(error),
        }

        let members = PyList::empty(py);
        for member in module.dir()? {
            members.append(module.getattr(member.cast::<pyo3::types::PyString>()?)?)?;
        }
        let kwargs = PyDict::new(py);
        kwargs.set_item("outerFilename", module.getattr("__file__")?)?;
        kwargs.set_item("globs", music21.getattr("__dict__")?.call_method0("copy")?)?;
        kwargs.set_item("optionflags", options)?;
        test_runner
            .getattr("addDocAttrTestsToSuite")?
            .call((&suite, members), Some(&kwargs))?;
    }
    test_runner.call_method1("fixDoctests", (&suite,))?;
    Ok(suite)
}

/// Throws away music21's parsed-score cache before a run.
///
/// A cached score is a pickle carrying the classes it was parsed with, so a
/// cache written by one of the two runs would be read by the other and the
/// comparison would be measuring the wrong thing.
fn clear_corpus_cache(py: Python<'_>) -> PyResult<()> {
    let root: String = py
        .import("music21.environment")?
        .getattr("Environment")?
        .call0()?
        .call_method0("getRootTempDir")?
        .str()?
        .extract()?;
    let Ok(entries) = fs::read_dir(&root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().contains(".p") {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

/// How the two runs compare in time, test by test.
///
/// The suite is already run twice; this is what that pair of runs says about
/// speed. Every case is the same test doing the same work, so unlike a
/// benchmark written for the purpose there is nothing to argue about in the
/// comparison — but most of what a music21 test does is music21's own code
/// either way, so the middle of the distribution sits near parity by
/// construction. The tails are the part worth reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timings {
    /// Tests timed on both sides, passing on both, and slow enough to time.
    pub paired: usize,
    /// Total seconds those tests took, each way.
    pub music21_seconds: f64,
    pub music21_rs_seconds: f64,
    pub median_speedup: f64,
    /// The tests where the crate is furthest ahead, and furthest behind.
    pub fastest: Vec<TestTiming>,
    pub slowest: Vec<TestTiming>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTiming {
    pub name: String,
    pub music21_seconds: f64,
    pub music21_rs_seconds: f64,
    pub speedup: f64,
}

/// Below this a duration is mostly the timer, not the test. Both sides have
/// to clear it: a ratio taken against a denominator of a few microseconds
/// says more about the clock than about either implementation, and letting
/// those through put a 277x at the head of the list off a test that took
/// nineteen milliseconds one way and none the other.
const TOO_QUICK_TO_TIME: f64 = 0.001;

/// How many of each tail to keep.
const TAIL: usize = 12;

fn compare_timings(plain: &Report, ours: &Report) -> Option<Timings> {
    let bad = |report: &Report| {
        report
            .bad()
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>()
    };
    let (theirs_bad, ours_bad) = (bad(plain), bad(ours));
    let mut rows: Vec<TestTiming> = Vec::new();
    let mut music21_seconds = 0.0;
    let mut music21_rs_seconds = 0.0;
    for (name, theirs) in &plain.durations {
        let Some(mine) = ours.durations.get(name) else {
            continue;
        };
        // A test that failed did not do the work the other side did.
        if theirs_bad.contains(name) || ours_bad.contains(name) {
            continue;
        }
        if *theirs < TOO_QUICK_TO_TIME || *mine < TOO_QUICK_TO_TIME {
            continue;
        }
        music21_seconds += theirs;
        music21_rs_seconds += mine;
        rows.push(TestTiming {
            name: name.clone(),
            music21_seconds: *theirs,
            music21_rs_seconds: *mine,
            speedup: theirs / mine.max(f64::MIN_POSITIVE),
        });
    }
    if rows.is_empty() {
        return None;
    }
    rows.sort_by(|a, b| {
        a.speedup
            .partial_cmp(&b.speedup)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let median = rows[rows.len() / 2].speedup;
    let slowest = rows.iter().take(TAIL).cloned().collect();
    let fastest = rows.iter().rev().take(TAIL).cloned().collect();
    Some(Timings {
        paired: rows.len(),
        music21_seconds,
        music21_rs_seconds,
        median_speedup: median,
        fastest,
        slowest,
    })
}

/// Prints what the pair of runs said about time.
fn report_timings(timings: &Timings) {
    println!();
    println!(
        "  {} of music21's own tests timed on both sides: {:.1}s on music21, {:.1}s on music21_rs, median {:.2}x",
        timings.paired, timings.music21_seconds, timings.music21_rs_seconds, timings.median_speedup
    );
    for (label, rows) in [
        ("furthest ahead", &timings.fastest),
        ("furthest behind", &timings.slowest),
    ] {
        println!();
        println!("  {label}:");
        for row in rows {
            println!(
                "    {:>6.2}x  {:>8.3}s -> {:>8.3}s  {}",
                row.speedup, row.music21_seconds, row.music21_rs_seconds, row.name
            );
        }
    }
}

/// What fails under `music21_rs` and not under music21.
fn compare(plain: &Report, ours: &Report, whole: bool) -> Result<i32, Box<dyn Error>> {
    let theirs = plain.bad();
    let mine = ours.bad();
    let divergent: Vec<&str> = mine.difference(&theirs).copied().collect();
    let excused = |name: &str| {
        EXPECTED_DIVERGENCES
            .iter()
            .find(|(listed, _)| *listed == name)
    };
    let new: Vec<&str> = divergent
        .iter()
        .copied()
        .filter(|name| excused(name).is_none())
        .collect();
    let expected: Vec<&str> = divergent
        .iter()
        .copied()
        .filter(|name| excused(name).is_some())
        .collect();

    println!();
    println!(
        "  music21   : {} of its own failures, {} tests",
        theirs.len(),
        plain.run
    );
    println!("  music21_rs: {} failures, {} tests", mine.len(), ours.run);

    if !expected.is_empty() {
        println!();
        println!("{} known divergence(s), allowed:", expected.len());
        for name in &expected {
            println!("  {name}");
            if let Some((_, why)) = excused(name) {
                println!("      {why}");
            }
        }
    }

    // A divergence that has been fixed should stop being excused, or the list
    // quietly grows stale and starts hiding real regressions. Only a whole run
    // can say that: a run filtered by `--only` never reaches most of these, so
    // asking it would report every divergence outside the filter as fixed.
    let stale: Vec<&str> = EXPECTED_DIVERGENCES
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !mine.contains(name))
        .collect();
    if whole && !stale.is_empty() {
        println!();
        println!(
            "{} listed divergence(s) no longer fail; drop them from",
            stale.len()
        );
        println!("EXPECTED_DIVERGENCES in xtask/src/music21_suite.rs:");
        for name in &stale {
            println!("  {name}");
        }
        return Ok(1);
    }

    if new.is_empty() {
        println!();
        println!("music21's own suite behaves the same on music21_rs, bar the known divergences.");
        return Ok(0);
    }
    println!();
    println!("{} tests fail only under music21_rs:", new.len());
    for name in new.iter().take(40) {
        println!("  {name}");
    }
    if new.len() > 40 {
        println!("  ... and {} more", new.len() - 40);
    }
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(run: usize, failures: &[&str], errors: &[&str]) -> Report {
        Report {
            run,
            failures: failures.iter().map(|name| name.to_string()).collect(),
            errors: errors.iter().map(|name| name.to_string()).collect(),
            detail: Default::default(),
            modules: Vec::new(),
            durations: BTreeMap::new(),
        }
    }

    #[test]
    fn a_side_that_matches_music21_passes() {
        let plain = report(10, &["a"], &["b"]);
        let ours = report(10, &["a"], &["b"]);
        // Nothing new is red, but both listed divergences have stopped
        // failing, which is itself reported.
        assert_eq!(compare(&plain, &ours, true).expect("compared"), 1);
        // A filtered run cannot see the listed divergences, so it is not
        // asked whether they have gone.
        assert_eq!(compare(&plain, &ours, false).expect("compared"), 0);
    }

    #[test]
    fn a_new_failure_is_a_regression() {
        let mut ours = report(10, &["a", "brand new"], &[]);
        // The listed divergences are present, so only the new one counts.
        for (name, _) in EXPECTED_DIVERGENCES {
            ours.failures.push((*name).to_string());
        }
        let plain = report(10, &["a"], &[]);
        assert_eq!(compare(&plain, &ours, true).expect("compared"), 1);
    }

    #[test]
    fn the_known_divergences_are_excused() {
        let mut ours = report(10, &["a"], &[]);
        for (name, _) in EXPECTED_DIVERGENCES {
            ours.failures.push((*name).to_string());
        }
        let plain = report(10, &["a"], &[]);
        assert_eq!(compare(&plain, &ours, true).expect("compared"), 0);
    }

    #[test]
    fn a_side_is_named_by_the_flag_it_is_asked_for() {
        assert!(matches!(Which::parse("music21"), Some(Which::Music21)));
        assert!(matches!(Which::parse("music21_rs"), Some(Which::Music21Rs)));
        assert!(Which::parse("neither").is_none());
        assert_eq!(Which::Music21Rs.file(), "music21_rs.json");
    }
}
