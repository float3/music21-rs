//! Runs the doctests of one music21 module against the facades.
//!
//! The real music21 is imported from the submodule, the docstrings of the
//! module under test are collected with `DocTestFinder`, *then* the names
//! listed in the swaps are replaced on their music21 modules with the facade
//! versions, and every docstring is run by `DocTestRunner` as it is. Collect
//! before swapping: the docstrings live on music21's own classes, and once a
//! module's names point at the facades the finder sees only the module-level
//! functions.
//!
//! What is swapped in is not always the bare facade class. A class music21
//! keeps in a `Stream` has to be a `Music21Object` to be kept at all, so
//! those go in as a Python subclass of the facade and of `Music21Object` —
//! see `class_to_install` in the `python` crate, which is also what
//! `install_into_music21` uses. Only the container machinery is inherited,
//! never music21's own implementation of the class being replaced, so a
//! member the facade has not ported still fails rather than falling through.
//!
//! Each docstring is one unit: it passes when all of its examples pass.
//! `python-parity/doctest/<name>.toml` lists the docstrings that pass. The run
//! fails both ways, and says so by name: when a listed docstring stops
//! passing, and when one passes that is not listed, so the list is always
//! exactly what passes. `UPDATE_DOCTEST_EXPECTATIONS=1` rewrites the list from
//! the current run. An optional `[not_passing]` table maps a docstring to the
//! reason it fails on purpose; the rewrite keeps it, and a failure needs no
//! entry there. The full doctest report of every failing docstring is
//! written to `target/doctest_<name>.log`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::{Deserialize, Serialize};
use utils::{init_py, prepare};

pub use music21_rs_python::aligner::NAMES as ALIGNER_NAMES;
pub use music21_rs_python::analysis::{
    ELEMENTS_NAMES, FLOATING_KEY_NAMES, METRICAL_NAMES, PATEL_NAMES, PITCH_ANALYSIS_NAMES,
    SEGMENT_BY_RESTS_NAMES,
};
/// The names each facade module provides, for swapping into the music21
/// module of the same name. They live with the classes they name; the
/// aliases are here because the tests read them off this module.
pub use music21_rs_python::articulations::NAMES as ARTICULATION_NAMES;
pub use music21_rs_python::audiosearch::NAMES as AUDIO_SEARCH_NAMES;
pub use music21_rs_python::bar::NAMES as BAR_NAMES;
pub use music21_rs_python::braillebasic::NAMES as BRAILLE_BASIC_NAMES;
pub use music21_rs_python::braillelookup::NAMES as BRAILLE_LOOKUP_NAMES;
pub use music21_rs_python::braillenotegrouping::NAMES as BRAILLE_NOTE_GROUPING_NAMES;
pub use music21_rs_python::braillesegment::NAMES as BRAILLE_SEGMENT_NAMES;
pub use music21_rs_python::brailletext::NAMES as BRAILLE_TEXT_NAMES;
pub use music21_rs_python::brailletranslate::NAMES as BRAILLE_TRANSLATE_NAMES;
pub use music21_rs_python::chord::NAMES as CHORD_NAMES;
pub use music21_rs_python::chordtables::NAMES as CHORD_TABLES_NAMES;
pub use music21_rs_python::clef::NAMES as CLEF_NAMES;
pub use music21_rs_python::clercqtemperley::NAMES as CLERCQ_TEMPERLEY_NAMES;
pub use music21_rs_python::correlate::NAMES as CORRELATE_NAMES;
pub use music21_rs_python::duration::NAMES as DURATION_NAMES;
pub use music21_rs_python::dynamics::NAMES as DYNAMICS_NAMES;
pub use music21_rs_python::enharmonics::NAMES as ENHARMONICS_NAMES;
pub use music21_rs_python::expressions::NAMES as EXPRESSION_NAMES;
pub use music21_rs_python::extractors::{
    JSYMBOLIC_NAMES as FEATURES_JSYMBOLIC_NAMES, NATIVE_NAMES as FEATURES_NATIVE_NAMES,
};
pub use music21_rs_python::fbrules::NAMES as RULES_NAMES;
pub use music21_rs_python::features::NAMES as FEATURES_NAMES;
pub use music21_rs_python::figuredbass::NAMES as FIGURED_BASS_NAMES;
pub use music21_rs_python::fixer::NAMES as FIXER_NAMES;
pub use music21_rs_python::harmonicfunction::NAMES as HARMONIC_FUNCTION_NAMES;
pub use music21_rs_python::harmony::NAMES as HARMONY_NAMES;
pub use music21_rs_python::harmparser::NAMES as HARM_PARSER_NAMES;
pub use music21_rs_python::hasher::NAMES as HASHER_NAMES;
pub use music21_rs_python::instrument::NAMES as INSTRUMENT_NAMES;
pub use music21_rs_python::interval::NAMES as INTERVAL_NAMES;
pub use music21_rs_python::key::NAMES as KEY_NAMES;
pub use music21_rs_python::layout::NAMES as LAYOUT_NAMES;
pub use music21_rs_python::makenotation::NAMES as MAKE_NOTATION_NAMES;
pub use music21_rs_python::metadata::NAMES as METADATA_NAMES;
pub use music21_rs_python::meter::{CORE_NAMES as METER_CORE_NAMES, NAMES as METER_NAMES};
pub use music21_rs_python::midipercussion::NAMES as MIDI_PERCUSSION_NAMES;
pub use music21_rs_python::naturallanguage::NAMES as NATURAL_LANGUAGE_NAMES;
pub use music21_rs_python::neoriemannian::NAMES as NEO_RIEMANNIAN_NAMES;
pub use music21_rs_python::notation::{BEAM_NAMES, TIE_NAMES, VOLUME_NAMES};
pub use music21_rs_python::note::NAMES as NOTE_NAMES;
pub use music21_rs_python::omrcorrectors::NAMES as OMR_CORRECTORS_NAMES;
pub use music21_rs_python::percussion::NAMES as PERCUSSION_NAMES;
pub use music21_rs_python::pitch::NAMES as PITCH_NAMES;
pub use music21_rs_python::possibility::NAMES as POSSIBILITY_NAMES;
pub use music21_rs_python::primitives::NAMES as PRIMITIVES_NAMES;
pub use music21_rs_python::realizerscale::NAMES as REALIZER_SCALE_NAMES;
pub use music21_rs_python::reducechords::NAMES as REDUCE_CHORDS_NAMES;
pub use music21_rs_python::reduction::NAMES as REDUCTION_NAMES;
pub use music21_rs_python::repeat::NAMES as REPEAT_NAMES;
pub use music21_rs_python::resolution::NAMES as RESOLUTION_NAMES;
pub use music21_rs_python::roman::NAMES as ROMAN_NAMES;
pub use music21_rs_python::scala::NAMES as SCALA_NAMES;
pub use music21_rs_python::scale::NAMES as SCALE_NAMES;
pub use music21_rs_python::searchbase::NAMES as SEARCH_BASE_NAMES;
pub use music21_rs_python::searchlyrics::NAMES as SEARCH_LYRICS_NAMES;
pub use music21_rs_python::searchsegment::NAMES as SEARCH_SEGMENT_NAMES;
pub use music21_rs_python::searchserial::NAMES as SEARCH_SERIAL_NAMES;
pub use music21_rs_python::segment::NAMES as SEGMENT_NAMES;
pub use music21_rs_python::serial::NAMES as SERIAL_NAMES;
pub use music21_rs_python::sieve::NAMES as SIEVE_NAMES;
pub use music21_rs_python::spanner::CLASS_NAMES as SPANNER_NAMES;
pub use music21_rs_python::tablature::NAMES as TABLATURE_NAMES;
pub use music21_rs_python::tempo::NAMES as TEMPO_NAMES;
pub use music21_rs_python::text::NAMES as TEXT_NAMES;
pub use music21_rs_python::transposition::NAMES as TRANSPOSITION_NAMES;
pub use music21_rs_python::tsvconverter::NAMES as TSV_CONVERTER_NAMES;
pub use music21_rs_python::variant::NAMES as VARIANT_NAMES;
pub use music21_rs_python::voiceleading::NAMES as VOICE_LEADING_NAMES;
pub use music21_rs_python::windowed::NAMES as WINDOWED_NAMES;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Expectations {
    #[serde(default)]
    passing: Vec<String>,
    /// Docstrings that fail on purpose, each with its reason. Only the
    /// deliberate ones are here; any other failure is simply unlisted.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    not_passing: BTreeMap<String, String>,
}

/// Where one run and the listed expectations disagree.
#[derive(Debug, Default, PartialEq)]
struct Mismatch {
    /// Listed as passing, and failed.
    regressions: Vec<String>,
    /// Passed, and not listed as passing.
    unlisted: Vec<String>,
}

impl Mismatch {
    fn is_empty(&self) -> bool {
        self.regressions.is_empty() && self.unlisted.is_empty()
    }

    /// The failure message, naming every docstring on either side.
    fn message(&self, expected: &Expectations, path: &Path, log_path: &Path) -> String {
        let mut text = String::new();

        if !self.regressions.is_empty() {
            text.push_str(&format!(
                "{} docstrings that used to pass against the crate now fail:\n    {}\nsee {}\n",
                self.regressions.len(),
                self.regressions.join("\n    "),
                log_path.display()
            ));
        }

        if !self.unlisted.is_empty() {
            // A docstring kept as a deliberate non-pass says so, since that
            // entry is now stale too.
            let names: Vec<String> = self
                .unlisted
                .iter()
                .map(|name| match expected.not_passing.get(name) {
                    Some(reason) => format!("{name} (listed as not passing: {reason})"),
                    None => name.clone(),
                })
                .collect();
            text.push_str(&format!(
                "{} docstrings pass that {} does not list; rerun with UPDATE_DOCTEST_EXPECTATIONS=1 to record them:\n    {}\n",
                names.len(),
                path.display(),
                names.join("\n    ")
            ));
        }
        text
    }
}

/// Compares the docstrings that passed with the listed ones, both ways.
fn compare(expected: &Expectations, passing: &BTreeSet<&str>) -> Mismatch {
    let regressions = expected
        .passing
        .iter()
        .filter(|name| !passing.contains(name.as_str()))
        .cloned()
        .collect();
    let unlisted = passing
        .iter()
        .filter(|name| !expected.passing.iter().any(|known| known == *name))
        .map(|name| name.to_string())
        .collect();
    Mismatch {
        regressions,
        unlisted,
    }
}

/// The expectations to write after a run: what passed, and the deliberate
/// non-passes of `old` that still fail.
fn rewritten(old: Expectations, passing: &BTreeSet<&str>) -> Expectations {
    let not_passing = old
        .not_passing
        .into_iter()
        .filter(|(name, _)| !passing.contains(name.as_str()))
        .collect();
    Expectations {
        passing: passing.iter().map(|name| name.to_string()).collect(),
        not_passing,
    }
}

/// Reads `python-parity/doctest/<name>.toml`, or nothing listed when absent.
fn read_expectations(path: &Path) -> Expectations {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).expect("doctest expectations parse"),
        Err(_) => Expectations::default(),
    }
}

/// The counts of one module's run, written beside the log as
/// `target/doctest_<name>.toml` so `xtask report` can read them back.
#[derive(Debug, Serialize)]
struct Summary {
    module: String,
    docstrings_passing: usize,
    docstrings: usize,
    examples_passing: usize,
    examples: usize,
}

/// music21's own parallelism cannot run here, and must not run here.
///
/// `common.runParallel` spawns worker interpreters through joblib, and
/// inside an embedded interpreter the executable they are spawned from is
/// the test binary itself, which answers `Unrecognized option: 'c'`. A
/// worker that did start would be worse than one that fails: it would be a
/// fresh interpreter holding music21's own classes, nothing of this package
/// installed over them, so an example run there would measure music21
/// against itself. music21 asks `safeToParallize` before every parallel
/// run -- it already answers no for a pytest worker -- and the answer here
/// is no for the same reason.
const SERIAL_HELPER: &str = r#"
from music21.common import parallel


def unsafe() -> bool:
    """Nothing may be run in a worker process from an embedded interpreter."""
    return False


parallel.safeToParallize = unsafe
"#;

/// music21's `fixDoctests`, which its own runner applies to every docstring
/// before a word of it is compared.
const READING_HELPER: &str = r#"
import platform

from music21.test.testRunner import stripAddresses


def fix(test):
    """music21's own `fixDoctests`, for one docstring rather than a suite.

    Its runner rewrites every expectation before comparing: an address is
    replaced by `0x...`, which `doctest.ELLIPSIS` then matches, and on Windows
    a `PosixPath` is read as a `WindowsPath`. A docstring is written against
    that reading, so this is what it has to be run under -- music21's own
    `<music21.style.Style object at 0x10b0a2080>` passes on music21 and would
    fail against anything, this crate included, without it.
    """
    windows = platform.system() == 'Windows'
    for example in test.examples:
        example.want = stripAddresses(example.want, '0x...')
        if windows:
            example.want = example.want.replace('PosixPath', 'WindowsPath')

"#;

#[derive(Debug)]
struct Outcome {
    name: String,
    attempted: usize,
    failed: usize,
    report: String,
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .to_path_buf()
}

/// Claims the name the wheel installs under, so that one process can only
/// ever hold one copy of these classes.
///
/// The virtualenv [`add_dependency_venv`] puts on `sys.path` is where
/// music21's own dependencies live, and it is also where `pip install
/// music21-rs` puts the wheel. Importing that wheel here would load the
/// extension's copy of every facade class beside the linked crate's, and
/// music21 would then hold one where a method wants the other — which reads
/// as `'Accidental' object is not an instance of 'Accidental'`, from a
/// harness that never asked for the wheel at all. Registering the linked
/// classes under the wheel's name first means an import of it finds these,
/// and no `.pyd` is ever loaded.
fn claim_the_wheels_name(py: Python<'_>) -> PyResult<()> {
    let modules = py.import("sys")?.getattr("modules")?;
    if let Ok(taken) = modules.get_item("music21_rs") {
        let file: String = taken
            .getattr("__file__")
            .and_then(|file| file.extract())
            .unwrap_or_default();
        if !file.is_empty() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "the music21_rs wheel is already imported from {file}; this harness                  tests the linked crate, and two copies of these classes in one                  process do not answer each other's isinstance"
            )));
        }
        return Ok(());
    }
    let ours = PyModule::new(py, "music21_rs")?;
    music21_rs_python::register_all(&ours)?;
    modules.set_item("music21_rs", &ours)?;
    Ok(())
}

/// Puts the dependency virtualenv on `sys.path`, the way `xtask`'s fixture
/// generator does, so the full music21 imports.
pub fn add_dependency_venv(py: Python<'_>, root: &Path) -> PyResult<()> {
    claim_the_wheels_name(py)?;
    let sys = py.import("sys")?;
    let path = sys.getattr("path")?;
    let path = path.cast::<PyList>()?;
    for relative in [".m21venv", "venv", ".venv"] {
        let venv = root.join(relative);
        if !venv.is_dir() {
            continue;
        }
        for site in [
            venv.join("Lib/site-packages"),
            venv.join("lib/site-packages"),
        ] {
            if site.is_dir() {
                path.insert(0, site.to_string_lossy().to_string())?;
            }
        }
        if let Ok(entries) = std::fs::read_dir(venv.join("lib")) {
            for entry in entries.flatten() {
                let site = entry.path().join("site-packages");
                if site.is_dir() {
                    path.insert(0, site.to_string_lossy().to_string())?;
                }
            }
        }
    }
    path.insert(0, root.join("music21").to_string_lossy().to_string())?;
    Ok(())
}

/// Throws away music21's parsed-score cache before a run.
///
/// A cached score is a pickle carrying the classes it was parsed with, and
/// these classes name the module they came from. One written by a run through
/// the installed wheel names `music21_rs`, which a harness process has no
/// import of; one written here names `music21_rs_facade`, which nothing else
/// has. Either way the other run reads a pickle it cannot open, so whichever
/// went first would decide what the second measured. `xtask music21-suite` clears
/// it for the same reason.
fn clear_corpus_cache(py: Python<'_>) -> PyResult<()> {
    let environment = py.import("music21.environment")?;
    let root = environment
        .getattr("Environment")?
        .call0()?
        .call_method0("getRootTempDir")?
        .str()?
        .extract::<String>()?;
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("m21-") && name.contains(".p") {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

fn run_doctests(py: Python<'_>, module: &str, swaps: &[(&str, &[&str])]) -> PyResult<Vec<Outcome>> {
    let facade = py.import("music21_rs_facade")?;
    let music21 = py.import("music21")?;
    clear_corpus_cache(py)?;
    PyModule::from_code(
        py,
        &std::ffi::CString::new(SERIAL_HELPER)?,
        c"music21_rs_doctest_serial.py",
        c"music21_rs_doctest_serial",
    )?;
    let target = py.import(module)?;

    let doctest = py.import("doctest")?;
    let flags = doctest.getattr("ELLIPSIS")?.extract::<i64>()?
        | doctest.getattr("NORMALIZE_WHITESPACE")?.extract::<i64>()?;
    let globs = PyDict::new(py);
    globs.update(music21.getattr("__dict__")?.cast::<PyDict>()?.as_mapping())?;

    let finder = doctest.getattr("DocTestFinder")?.call0()?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("globs", &globs)?;
    let tests = finder.call_method("find", (&target,), Some(&kwargs))?;
    let collect = facade.getattr("collect_output")?;

    // Every docstring is read as music21's own runner reads it, before a word
    // of it is compared.
    let fix = PyModule::from_code(
        py,
        &std::ffi::CString::new(READING_HELPER)?,
        c"music21_rs_doctest_reading.py",
        c"music21_rs_doctest_reading",
    )?
    .getattr("fix")?;
    for test in tests.try_iter()? {
        fix.call1((&test?,))?;
    }

    for (module_name, names) in swaps {
        let module = py.import(*module_name)?;
        for name in *names {
            let original = module.getattr(*name).ok();
            let value =
                music21_rs_python::class_to_install(py, &module, name, &facade.getattr(*name)?)?;
            module.setattr(*name, &value)?;
            // Setting the name on its own module is not enough: music21 is
            // fully imported by now, and a module that wrote `from
            // music21.duration import Duration` at the top still holds the
            // class it had then. Every object music21 builds would carry
            // that one, and a score could not be frozen at all — which is
            // what `corpus.parse` does on the way back out. This is the
            // same rebinding `install_into_music21` does.
            if let Some(original) = original {
                music21_rs_python::rebind(py, &original, &value)?;
            }
        }
    }

    let mut outcomes = Vec::new();
    for test in tests.try_iter()? {
        let test: Bound<'_, PyAny> = test?;
        let example_count: usize = test.getattr("examples")?.len()?;
        if example_count == 0 {
            continue;
        }
        let runner_kwargs = PyDict::new(py);
        runner_kwargs.set_item("optionflags", flags)?;
        runner_kwargs.set_item("verbose", false)?;
        let runner = doctest
            .getattr("DocTestRunner")?
            .call((), Some(&runner_kwargs))?;
        let run_kwargs = PyDict::new(py);
        run_kwargs.set_item("out", &collect)?;
        run_kwargs.set_item("clear_globs", false)?;
        let _ = crate::take_output();
        let results = runner.call_method("run", (&test,), Some(&run_kwargs))?;
        let failed: usize = results.getattr("failed")?.extract()?;
        let attempted: usize = results.getattr("attempted")?.extract()?;
        outcomes.push(Outcome {
            name: test.getattr("name")?.extract()?,
            attempted,
            failed,
            report: crate::take_output(),
        });
    }
    Ok(outcomes)
}

/// Runs the doctests of `module` (`"music21.pitch"`) with the swapped names
/// in place, records the report under `name` (`"pitch"`), and panics when
/// the run and `python-parity/doctest/<name>.toml` disagree either way.
///
/// The caller must have registered the facade module with
/// `pyo3::append_to_inittab!(music21_rs_facade)` before the interpreter
/// starts, which in practice means as the first line of the test.
pub fn run(module: &str, name: &str, swaps: &[(&str, &[&str])]) {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare music21 reference checkout");

    let outcomes = Python::attach(|py| -> PyResult<Vec<Outcome>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        run_doctests(py, module, swaps)
    })
    .unwrap_or_else(|error| panic!("running the {module} doctests: {error}"));

    let passing: BTreeSet<&str> = outcomes
        .iter()
        .filter(|outcome| outcome.failed == 0)
        .map(|outcome| outcome.name.as_str())
        .collect();
    let examples_attempted: usize = outcomes.iter().map(|o| o.attempted).sum();
    let examples_failed: usize = outcomes.iter().map(|o| o.failed).sum();

    let log_path = root.join(format!("target/doctest_{name}.log"));
    let mut log = String::new();
    for outcome in outcomes.iter().filter(|outcome| outcome.failed > 0) {
        log.push_str(&format!(
            "==== {} ({} of {} examples failed)\n{}\n",
            outcome.name, outcome.failed, outcome.attempted, outcome.report
        ));
    }
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&log_path, &log).expect("write the doctest log");

    let summary = Summary {
        module: module.to_string(),
        docstrings_passing: passing.len(),
        docstrings: outcomes.len(),
        examples_passing: examples_attempted - examples_failed,
        examples: examples_attempted,
    };
    std::fs::write(
        root.join(format!("target/doctest_{name}.toml")),
        toml::to_string_pretty(&summary).expect("serialize the doctest summary"),
    )
    .expect("write the doctest summary");

    println!(
        "{module} doctests against music21-rs: {} of {} docstrings pass, {} of {} examples pass; details in {}",
        passing.len(),
        outcomes.len(),
        examples_attempted - examples_failed,
        examples_attempted,
        log_path.display()
    );

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("doctest/{name}.toml"));
    let expected = read_expectations(&path);

    if std::env::var("UPDATE_DOCTEST_EXPECTATIONS").is_ok_and(|v| v == "1") {
        let expectations = rewritten(expected, &passing);
        let text = format!(
            "# Docstrings of {module} whose every example passes against the\n# music21-rs facades. Written by `UPDATE_DOCTEST_EXPECTATIONS=1 cargo test\n# --manifest-path python-parity/Cargo.toml --test doctest_{name}`; the test\n# fails when one of these stops passing or one passes that is not listed.\n# `[not_passing]`, kept by the rewrite, names deliberate failures and why.\n\n{}",
            toml::to_string_pretty(&expectations).expect("serialize expectations")
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the doctest directory");
        }
        std::fs::write(&path, text).expect("write the expectations");
        println!("wrote {}", path.display());
        return;
    }

    let mismatch = compare(&expected, &passing);
    assert!(
        mismatch.is_empty(),
        "{}",
        mismatch.message(&expected, &path, &log_path)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(passing: &[&str], not_passing: &[(&str, &str)]) -> Expectations {
        Expectations {
            passing: passing.iter().map(|name| name.to_string()).collect(),
            not_passing: not_passing
                .iter()
                .map(|(name, reason)| (name.to_string(), reason.to_string()))
                .collect(),
        }
    }

    #[test]
    fn matching_run_passes() {
        let expected = listed(&["m.a", "m.b"], &[("m.c", "by design")]);
        let passing = BTreeSet::from(["m.a", "m.b"]);
        assert!(compare(&expected, &passing).is_empty());
    }

    #[test]
    fn gate_fails_both_ways() {
        let expected = listed(&["m.a", "m.b"], &[]);
        let passing = BTreeSet::from(["m.a", "m.c"]);
        let mismatch = compare(&expected, &passing);
        assert_eq!(
            mismatch,
            Mismatch {
                regressions: vec!["m.b".to_string()],
                unlisted: vec!["m.c".to_string()],
            }
        );
        let message = mismatch.message(&expected, Path::new("m.toml"), Path::new("m.log"));
        assert!(message.contains("now fail:\n    m.b"));
        assert!(message.contains("UPDATE_DOCTEST_EXPECTATIONS=1"));
    }

    #[test]
    fn stale_reason_is_named() {
        let expected = listed(&[], &[("m.c", "needs matplotlib")]);
        let passing = BTreeSet::from(["m.c"]);
        let mismatch = compare(&expected, &passing);
        let message = mismatch.message(&expected, Path::new("m.toml"), Path::new("m.log"));
        assert!(message.contains("m.c (listed as not passing: needs matplotlib)"));
    }

    #[test]
    fn rewrite_keeps_reasons() {
        let old = listed(&["m.a"], &[("m.b", "refused"), ("m.c", "stale")]);
        let passing = BTreeSet::from(["m.a", "m.c"]);
        let new = rewritten(old, &passing);
        assert_eq!(new.passing, ["m.a", "m.c"]);
        assert_eq!(
            new.not_passing,
            BTreeMap::from([("m.b".to_string(), "refused".to_string())])
        );
    }

    #[test]
    fn reasons_round_trip() {
        let expectations = listed(&["m.a"], &[("m.b.C.plot", "needs matplotlib")]);
        let text = toml::to_string_pretty(&expectations).expect("serialize");
        let back: Expectations = toml::from_str(&text).expect("parse");
        assert_eq!(back.not_passing, expectations.not_passing);
        // A file without the table still reads.
        let bare: Expectations = toml::from_str("passing = []").expect("parse");
        assert!(bare.not_passing.is_empty());
    }
}
