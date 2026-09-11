//! Run music21's own test suite against `music21_rs`.
//!
//! [`crate::downstream`] measures the crate against somebody else's library.
//! This measures it against music21 itself: every `Test` class and every
//! docstring in every music21 module, run three times — once on music21, once
//! with the crate linked into this binary installed over it, and once with the
//! wheel out of site-packages installed over it — with each of the two sets of
//! failures compared against music21's own.
//!
//! Two subjects rather than one because they are two artifacts. The linked
//! crate is the working tree; the wheel is what ships and what a user runs. A
//! difference between them is a stale wheel or a packaging fault, and it is
//! worth seeing. The wheel is also the one side that may be absent, on a
//! machine that has not built one — it is then skipped, with the reason, and
//! the comparison has one fewer column.
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
//! The runs are separate processes, because installing over music21
//! cannot be undone inside one: `install_into_music21` rebinds music21's own
//! classes wherever they were already imported, and there is no putting that
//! back. So this command re-runs *itself* with `--run`, once per side. Needs
//! the `music21` submodule checked out, the `music21_rs` wheel installed for
//! the third side to run at all, and music21's own test dependencies — `scipy` and `python-Levenshtein` on top
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
///
/// Three, not two, because the crate reaches music21 by two different roads
/// and they are not the same artifact. [`Which::Music21Rs`] is the crate
/// linked into this very binary — the code in the working tree, whatever it
/// says today. [`Which::Music21RsWheel`] is the wheel `pip install` put in
/// site-packages, which is what ships and what a user actually runs. Timing
/// both against music21 says what the crate costs and what the packaging
/// costs, and a difference between the two is worth seeing rather than
/// averaging away.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Which {
    Music21,
    Music21Rs,
    Music21RsWheel,
}

/// Every side, in the order they are run and reported in.
pub const SIDES: [Which; 3] = [Which::Music21, Which::Music21Rs, Which::Music21RsWheel];

impl Which {
    pub fn parse(text: &str) -> Option<Self> {
        SIDES.into_iter().find(|side| side.name() == text)
    }

    fn name(self) -> &'static str {
        match self {
            Which::Music21 => "music21",
            Which::Music21Rs => "music21_rs",
            Which::Music21RsWheel => "music21_rs_wheel",
        }
    }

    /// How the page and the console name it.
    pub fn label(self) -> &'static str {
        match self {
            Which::Music21 => "music21",
            Which::Music21Rs => "music21-rs",
            Which::Music21RsWheel => "music21-rs-wheel",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Which::Music21 => "music21.json",
            Which::Music21Rs => "music21_rs.json",
            Which::Music21RsWheel => "music21_rs_wheel.json",
        }
    }
}

/// What a `--run` answers when the side it was asked for is not there to run.
///
/// Only the wheel can be missing: the other two are the submodule and this
/// binary. It is a skip and not a failure, so that the command still works on
/// a machine with no wheel built — the comparison then has one fewer column
/// and says so.
const EXIT_SIDE_ABSENT: i32 = 2;

/// The tests that fail under `music21_rs` on purpose, and why.
///
/// A documented divergence, not a gap: making it pass costs more than it
/// buys, and that was measured. Anything *not* on this list that fails only
/// under `music21_rs` is a real regression and fails the run — the same shape
/// as `python-parity/doctest/*.toml`, where what passes is listed and a newly
/// broken one is named.
const EXPECTED_DIVERGENCES: &[(&str, &str)] = &[(
    "classSet (music21.prebase.ProtoM21Object)",
    "an installed class lists both itself and the class it replaced in \
         classSet, so this doctest counts one more than music21 has. Dropping \
         the replaced class to make it pass costs 250 of music21's own tests. \
         Two ways of hiding it instead, one closed and one open. A frozenset \
         subclass overriding __contains__ is closed: music21 asks this as \
         `classFilterSet.intersection(e.classSet)` (stream/base.py:1497, \
         1512, 4340, 4957, 4962), and set intersection runs in C, which never \
         consults a container's Python-level __contains__. Putting the \
         equality on the *elements* is not closed, because that same C code \
         does consult it — a metaclass whose __eq__ and __hash__ stand in for \
         the replaced class makes frozenset collapse the pair to one entry, \
         keeps the first inserted (music21's own, so the doctest's repr \
         matches), and still answers both directions of intersection and \
         isdisjoint. The cost to weigh before trying it: two classes that \
         compare equal also collide as dict keys, so every map keyed by class \
         — music21's own _classSetCacheDict, and `installed`, `replaced`, \
         `_facades` and `_lineages` here — has to be re-keyed by identity \
         first. Measure the 250 again on the way; it predates `rebind`.",
)];

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
    let mut ran: Vec<Which> = Vec::new();
    for which in SIDES {
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
        match status.code() {
            Some(0) => ran.push(which),
            // The wheel is the one side that may simply not be there. The
            // run says so itself and answers this code; nothing else does.
            Some(code) if code == EXIT_SIDE_ABSENT => {
                println!("  skipped: {} is not installed", which.name());
                let _ = fs::remove_file(out.join(which.file()));
            }
            _ => {
                eprintln!("the {} run did not finish", which.name());
                return Ok(1);
            }
        }
    }

    let plain = read(&out.join(Which::Music21.file()))?;
    let mut sides: Vec<(Which, Report)> = Vec::new();
    for which in ran.into_iter().filter(|side| *side != Which::Music21) {
        sides.push((which, read(&out.join(which.file()))?));
    }

    if let Some(timings) = compare_timings(&plain, &sides) {
        report_timings(&timings);
        let path = out.join("timings.json");
        fs::write(&path, serde_json::to_string_pretty(&timings)?)?;
    }

    // Every side that ran is held to music21's own result. The crate reaching
    // music21 by two roads is two chances to diverge, and averaging them would
    // hide whichever one broke.
    let mut code = 0;
    for (which, report) in &sides {
        println!();
        println!("== {} against music21 ==", which.label());
        code = code.max(compare(&plain, report, only.is_none())?);
    }
    if sides.is_empty() {
        eprintln!("nothing to compare music21 against: no side but music21 ran");
        return Ok(1);
    }
    Ok(code)
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
        match which {
            Which::Music21 => {}
            // The crate compiled into this binary. `install_into_music21`
            // never needed the wheel — it builds its module in process out of
            // `register_all` — so calling it here installs the working tree.
            Which::Music21Rs => {
                let names = music21_rs_python::install_into_music21(py)?;
                println!("music21_rs (linked in) over {names} names");
            }
            // The wheel `pip install` put in site-packages, which is the
            // artifact that ships. Absent is a skip, not a failure.
            Which::Music21RsWheel => match py.import("music21_rs") {
                Ok(ours) => {
                    let names: usize = ours.call_method0("install_into_music21")?.extract()?;
                    let file: String = ours
                        .getattr("__file__")
                        .and_then(|file| file.extract())
                        .unwrap_or_else(|_| "an unnamed place".to_string());
                    println!("music21_rs {file} over {names} names");
                }
                Err(error) if error.is_instance_of::<pyo3::exceptions::PyImportError>(py) => {
                    println!("the music21_rs wheel is not installed: {error}");
                    return Ok(EXIT_SIDE_ABSENT);
                }
                Err(error) => return Err(error),
            },
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

/// How the runs compare in time, test by test.
///
/// The suite is already run on every side; this is what that says about
/// speed. Every case is the same test doing the same work, so unlike a
/// benchmark written for the purpose there is nothing to argue about in the
/// comparison — but most of what a music21 test does is music21's own code
/// either way, so the middle of the distribution sits near parity by
/// construction. The tails are the part worth reading.
///
/// A test counts only where every side ran it, passed it, and took long
/// enough to time. Three columns of which one is blank say less than two
/// columns that are both real.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timings {
    /// Tests timed on every side, passing on every side, and slow enough to
    /// time.
    #[serde(default)]
    pub paired: usize,
    /// The sides, in column order, music21 first.
    #[serde(default)]
    pub sides: Vec<SideTotal>,
    /// Every paired test, worst first by the headline speedup. The report
    /// shows the two tails and puts the whole of it on a page of its own.
    #[serde(default)]
    pub rows: Vec<TestTiming>,
    /// The tests where the crate is furthest ahead, and furthest behind.
    #[serde(default)]
    pub fastest: Vec<TestTiming>,
    #[serde(default)]
    pub slowest: Vec<TestTiming>,
}

/// One column: a side, what it spent, and how that compares to music21.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideTotal {
    /// `music21`, `music21-rs`, `music21-rs-wheel`.
    pub name: String,
    /// Total seconds the paired tests took on this side.
    pub seconds: f64,
    /// The median of this side's per-test speedup against music21. Absent for
    /// music21 itself, which is the thing being compared against.
    pub median_speedup: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTiming {
    pub name: String,
    /// One duration per side, in the order [`Timings::sides`] gives them.
    pub seconds: Vec<f64>,
    /// music21's time over the first side that is not music21 — the crate as
    /// it is compiled here, where it ran. What the tails are ordered by.
    pub speedup: f64,
}

impl TestTiming {
    /// This test's speedup on one column against music21.
    #[must_use]
    pub fn speedup_of(&self, side: usize) -> Option<f64> {
        let theirs = *self.seconds.first()?;
        let mine = *self.seconds.get(side)?;
        (side > 0).then(|| theirs / mine.max(f64::MIN_POSITIVE))
    }
}

/// Below this a duration is mostly the timer, not the test. Every side has
/// to clear it: a ratio taken against a denominator of a few microseconds
/// says more about the clock than about either implementation, and letting
/// those through put a 277x at the head of the list off a test that took
/// nineteen milliseconds one way and none the other.
const TOO_QUICK_TO_TIME: f64 = 0.001;

/// How many of each tail to keep.
const TAIL: usize = 12;

/// The median of a list that need not be sorted.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values.get(values.len() / 2).copied().unwrap_or(1.0)
}

fn compare_timings(plain: &Report, others: &[(Which, Report)]) -> Option<Timings> {
    if others.is_empty() {
        return None;
    }
    let bad = |report: &Report| {
        report
            .bad()
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>()
    };
    let theirs_bad = bad(plain);
    let others_bad: Vec<BTreeSet<String>> = others.iter().map(|(_, report)| bad(report)).collect();

    let mut rows: Vec<TestTiming> = Vec::new();
    let mut totals = vec![0.0; others.len() + 1];
    for (name, theirs) in &plain.durations {
        if theirs_bad.contains(name) || *theirs < TOO_QUICK_TO_TIME {
            continue;
        }
        // Every side or none: a row with a hole in it cannot be compared
        // across, and the tests missing from one side are the ones it failed.
        let mut seconds = vec![*theirs];
        let complete = others.iter().zip(&others_bad).all(|((_, report), bad)| {
            match report.durations.get(name) {
                Some(mine) if *mine >= TOO_QUICK_TO_TIME && !bad.contains(name) => {
                    seconds.push(*mine);
                    true
                }
                _ => false,
            }
        });
        if !complete {
            continue;
        }
        for (total, taken) in totals.iter_mut().zip(&seconds) {
            *total += taken;
        }
        let speedup = theirs / seconds[1].max(f64::MIN_POSITIVE);
        rows.push(TestTiming {
            name: name.clone(),
            seconds,
            speedup,
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

    let mut sides = vec![SideTotal {
        name: Which::Music21.label().to_string(),
        seconds: totals[0],
        median_speedup: None,
    }];
    for (index, (which, _)) in others.iter().enumerate() {
        let mut ratios: Vec<f64> = rows
            .iter()
            .filter_map(|row| row.speedup_of(index + 1))
            .collect();
        sides.push(SideTotal {
            name: which.label().to_string(),
            seconds: totals[index + 1],
            median_speedup: Some(median(&mut ratios)),
        });
    }

    let slowest = rows.iter().take(TAIL).cloned().collect();
    let fastest = rows.iter().rev().take(TAIL).cloned().collect();
    Some(Timings {
        paired: rows.len(),
        sides,
        rows,
        fastest,
        slowest,
    })
}

/// Prints what the runs said about time.
fn report_timings(timings: &Timings) {
    println!();
    let totals = timings
        .sides
        .iter()
        .map(|side| format!("{:.1}s on {}", side.seconds, side.name))
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "  {} of music21's own tests timed on every side: {totals}",
        timings.paired
    );
    for side in timings.sides.iter().skip(1) {
        println!(
            "    {} median {:.2}x",
            side.name,
            side.median_speedup.unwrap_or(1.0)
        );
    }
    for (label, rows) in [
        ("furthest ahead", &timings.fastest),
        ("furthest behind", &timings.slowest),
    ] {
        println!();
        println!("  {label}:");
        for row in rows {
            let times = row
                .seconds
                .iter()
                .map(|taken| format!("{taken:>8.3}s"))
                .collect::<Vec<_>>()
                .join(" ->");
            println!("    {:>6.2}x  {times}  {}", row.speedup, row.name);
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
        assert!(matches!(
            Which::parse("music21_rs_wheel"),
            Some(Which::Music21RsWheel)
        ));
        assert!(Which::parse("neither").is_none());
        assert_eq!(Which::Music21Rs.file(), "music21_rs.json");
        // Every side has a name, a label and a file of its own, or one run
        // would overwrite another's results.
        let files: BTreeSet<&str> = SIDES.iter().map(|side| side.file()).collect();
        assert_eq!(files.len(), SIDES.len());
    }

    fn timed(run: usize, durations: &[(&str, f64)]) -> Report {
        let mut report = report(run, &[], &[]);
        report.durations = durations
            .iter()
            .map(|(name, seconds)| ((*name).to_string(), *seconds))
            .collect();
        report
    }

    #[test]
    fn a_test_is_timed_only_where_every_side_ran_it() {
        let plain = timed(3, &[("slow", 1.0), ("only_here", 1.0), ("blink", 0.0001)]);
        let linked = timed(3, &[("slow", 0.5), ("blink", 0.0001)]);
        let wheel = timed(3, &[("slow", 0.25), ("only_here", 1.0)]);
        let timings = compare_timings(
            &plain,
            &[(Which::Music21Rs, linked), (Which::Music21RsWheel, wheel)],
        )
        .expect("a comparison");

        // `only_here` is missing from one side and `blink` is under the
        // timer's floor, so one test is left.
        assert_eq!(timings.paired, 1);
        assert_eq!(timings.rows[0].name, "slow");
        assert_eq!(timings.rows[0].seconds, vec![1.0, 0.5, 0.25]);

        // Three columns, music21 first, each with its own total.
        let names: Vec<&str> = timings
            .sides
            .iter()
            .map(|side| side.name.as_str())
            .collect();
        assert_eq!(names, vec!["music21", "music21-rs", "music21-rs-wheel"]);
        assert_eq!(timings.sides[0].median_speedup, None);
        assert_eq!(timings.sides[1].median_speedup, Some(2.0));
        assert_eq!(timings.sides[2].median_speedup, Some(4.0));
        assert_eq!(timings.sides[2].seconds, 0.25);

        // The headline is the linked crate, and each column reads its own.
        assert!((timings.rows[0].speedup - 2.0).abs() < 1e-9);
        assert_eq!(timings.rows[0].speedup_of(0), None);
        assert_eq!(timings.rows[0].speedup_of(2), Some(4.0));
    }

    #[test]
    fn a_test_that_failed_somewhere_is_not_timed() {
        let plain = timed(2, &[("a", 1.0), ("b", 1.0)]);
        let mut ours = timed(2, &[("a", 0.5), ("b", 0.5)]);
        ours.failures.push("b".to_string());
        let timings = compare_timings(&plain, &[(Which::Music21Rs, ours)]).expect("a comparison");
        assert_eq!(timings.paired, 1);
        assert_eq!(timings.rows[0].name, "a");
    }

    #[test]
    fn nothing_to_compare_against_is_no_comparison() {
        let plain = timed(1, &[("a", 1.0)]);
        assert!(compare_timings(&plain, &[]).is_none());
        // Nothing paired is the same answer: a table with no rows says less
        // than no table at all.
        let ours = timed(1, &[("b", 1.0)]);
        assert!(compare_timings(&plain, &[(Which::Music21Rs, ours)]).is_none());
    }
}
