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
//! Each docstring is one unit: it passes when all of its examples pass.
//! `python-parity/doctest/<name>.toml` lists the docstrings that pass. The run
//! fails when one of those stops passing, and says so by name; a docstring
//! that starts passing is reported but not required, so the list only ever
//! needs to grow. `UPDATE_DOCTEST_EXPECTATIONS=1` rewrites the list from the
//! current run. The full doctest report of every failing docstring is written
//! to `target/doctest_<name>.log`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::{Deserialize, Serialize};
use utils::{init_py, prepare};

/// The names the pitch facade provides, for swapping into `music21.pitch`.
pub const PITCH_NAMES: &[&str] = &[
    "Pitch",
    "Accidental",
    "Microtone",
    "PitchException",
    "AccidentalException",
    "MicrotoneException",
    "simplifyMultipleEnharmonics",
    "convertPitchClassToStr",
    "isValidAccidentalName",
    "standardizeAccidentalName",
];

/// The names the serial facade provides, for swapping into `music21.serial`.
pub const SERIAL_NAMES: &[&str] = &[
    "ToneRow",
    "TwelveToneRow",
    "HistoricalTwelveToneRow",
    "TwelveToneMatrix",
    "SerialException",
    "pcToToneRow",
    "rowToMatrix",
    "getHistoricalRowByName",
    "historicalDict",
];

/// The names the key facade provides, for swapping into `music21.key`.
/// The names the `chord` facade replaces in `music21.chord`.
pub const CHORD_NAMES: &[&str] = &["Chord", "ChordException"];

/// The names the `note` facade replaces in `music21.note`.
pub const NOTE_NAMES: &[&str] = &[
    "Note",
    "NoteException",
    "NotRestException",
    "Lyric",
    "LyricException",
];

/// The names the `note` facade replaces in `music21.duration`.
pub const DURATION_NAMES: &[&str] = &["Duration"];

/// The names the `notation` facade replaces in `music21.tie`.
pub const TIE_NAMES: &[&str] = &["Tie", "TieException"];

/// The names the `notation` facade replaces in `music21.volume`.
pub const VOLUME_NAMES: &[&str] = &["Volume", "VolumeException"];

/// The names the `notation` facade replaces in `music21.style`.
pub const STYLE_NAMES: &[&str] = &["Style"];

/// The names the `interval` facade replaces in `music21.interval`.
pub const INTERVAL_NAMES: &[&str] = &[
    "Direction",
    "Specifier",
    "GenericInterval",
    "DiatonicInterval",
    "ChromaticInterval",
    "Interval",
    "IntervalException",
    "convertStaffDistanceToInterval",
    "convertDiatonicNumberToStep",
    "parseSpecifier",
    "convertGeneric",
    "convertSemitoneToSpecifierGenericMicrotone",
    "convertSemitoneToSpecifierGeneric",
    "intervalToPythagoreanRatio",
    "notesToGeneric",
    "notesToChromatic",
    "intervalsToDiatonic",
    "intervalFromGenericAndChromatic",
    "getWrittenHigherNote",
    "getWrittenLowerNote",
    "getAbsoluteHigherNote",
    "getAbsoluteLowerNote",
    "transposePitch",
    "transposeNote",
    "notesToInterval",
    "add",
    "subtract",
];

pub const KEY_NAMES: &[&str] = &[
    "KeySignature",
    "Key",
    "KeySignatureException",
    "KeyException",
    "sharpsToPitch",
    "pitchToSharps",
    "convertKeyStringToMusic21KeyString",
];

#[derive(Debug, Default, Serialize, Deserialize)]
struct Expectations {
    #[serde(default)]
    passing: Vec<String>,
}

#[derive(Debug)]
struct Outcome {
    name: String,
    attempted: usize,
    failed: usize,
    report: String,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("python-parity has a parent directory")
        .to_path_buf()
}

/// Puts the dependency virtualenv on `sys.path`, the way `xtask`'s fixture
/// generator does, so the full music21 imports.
fn add_dependency_venv(py: Python<'_>, root: &Path) -> PyResult<()> {
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

fn run_doctests(py: Python<'_>, module: &str, swaps: &[(&str, &[&str])]) -> PyResult<Vec<Outcome>> {
    let facade = py.import("music21_rs_facade")?;
    let music21 = py.import("music21")?;
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

    for (module_name, names) in swaps {
        let module = py.import(*module_name)?;
        for name in *names {
            module.setattr(*name, facade.getattr(*name)?)?;
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
/// in place, records the report under `name` (`"pitch"`), and panics on a
/// regression against `python-parity/doctest/<name>.toml`.
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

    println!(
        "{module} doctests against music21-rs: {} of {} docstrings pass, {} of {} examples pass; details in {}",
        passing.len(),
        outcomes.len(),
        examples_attempted - examples_failed,
        examples_attempted,
        log_path.display()
    );

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("doctest/{name}.toml"));
    if std::env::var("UPDATE_DOCTEST_EXPECTATIONS").is_ok_and(|v| v == "1") {
        let expectations = Expectations {
            passing: passing.iter().map(|name| name.to_string()).collect(),
        };
        let text = format!(
            "# Docstrings of {module} whose every example passes against the\n# music21-rs facades. Written by `UPDATE_DOCTEST_EXPECTATIONS=1 cargo test\n# --manifest-path python-parity/Cargo.toml --test doctest_{name}`; the test\n# fails when one of these stops passing.\n\n{}",
            toml::to_string_pretty(&expectations).expect("serialize expectations")
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create the doctest directory");
        }
        std::fs::write(&path, text).expect("write the expectations");
        println!("wrote {}", path.display());
        return;
    }

    let expected: Expectations = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).expect("doctest expectations parse"),
        Err(_) => Expectations::default(),
    };
    let regressions: Vec<&String> = expected
        .passing
        .iter()
        .filter(|name| !passing.contains(name.as_str()))
        .collect();
    let newly_passing: Vec<&&str> = passing
        .iter()
        .filter(|name| !expected.passing.iter().any(|known| known == *name))
        .collect();
    if !newly_passing.is_empty() {
        println!(
            "{} docstrings pass that {} does not list yet; rerun with UPDATE_DOCTEST_EXPECTATIONS=1 to record them:\n    {}",
            newly_passing.len(),
            path.display(),
            newly_passing
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>()
                .join("\n    ")
        );
    }
    assert!(
        regressions.is_empty(),
        "{} docstrings that used to pass against the crate now fail:\n    {}\nsee {}",
        regressions.len(),
        regressions
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
            .join("\n    "),
        log_path.display()
    );
}
