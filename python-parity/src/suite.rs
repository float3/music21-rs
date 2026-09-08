//! music21's own unit tests, run against the crate that is *linked in*.
//!
//! `xtask music21-suite` runs the same tests against the installed wheel, and
//! that is the right subject for a gate: the wheel is the artifact that ships,
//! and a comparison against a plain music21 run says whether installing over
//! it changed anything. What that cannot do is contribute to coverage. The
//! wheel's Rust half is a `.pyd` inside site-packages, with no object under
//! the target directory for `llvm-cov` to map its profiles onto.
//!
//! Here the crate is linked into the test binary, the way the doctest harness
//! beside this one links it, so the code music21's tests drive is instrumented
//! like everything else in `python-parity`. `install_into_music21` never
//! needed the wheel to begin with — it builds its module in process out of
//! `register_all` — so pointing it at the linked crate is the whole trick.
//!
//! This is deliberately *not* a second gate. It runs the modules the crate
//! actually replaces, and asserts only that music21's tests still get through
//! them; `xtask music21-suite` remains the thing that says whether the crate
//! changed music21's behaviour, because only it has a baseline to compare
//! against.

use std::collections::BTreeSet;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use utils::{init_py, prepare};

use crate::doctest::{add_dependency_venv, repo_root};

/// The music21 modules whose tests drive code this crate carries.
///
/// Running the whole of music21's suite here would spend minutes in the
/// MusicXML importer and the corpus for the sake of a few more covered lines;
/// `xtask music21-suite` does that, against the wheel, where it belongs. These
/// are the modules whose classes `install_into_music21` actually replaces.
pub const COVERED_MODULES: [&str; 14] = [
    "music21.pitch",
    "music21.interval",
    "music21.note",
    "music21.duration",
    "music21.chord",
    "music21.key",
    "music21.scale",
    "music21.roman",
    "music21.serial",
    "music21.tempo",
    "music21.beam",
    "music21.volume",
    "music21.tie",
    "music21.harmony",
];

/// What one run of the suite came to.
#[derive(Debug, Default)]
pub struct Outcome {
    /// How many tests ran.
    pub run: usize,
    /// The tests that failed or errored, by name.
    pub bad: BTreeSet<String>,
    /// How many classes `install_into_music21` replaced.
    pub installed: usize,
}

/// Runs music21's own tests for `modules` against the linked crate.
///
/// Panics rather than returning an error: it is called from a test, and the
/// failure of the interpreter to start is not something a caller can act on.
#[must_use]
pub fn run(modules: &[&str]) -> Outcome {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    Python::attach(|py| -> PyResult<Outcome> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;

        // music21's tests print names that the console's own encoding cannot
        // always spell.
        let sys = py.import("sys")?;
        for stream in ["stdout", "stderr"] {
            let kwargs = PyDict::new(py);
            kwargs.set_item("encoding", "utf-8")?;
            kwargs.set_item("errors", "replace")?;
            let _ = sys
                .getattr(stream)?
                .call_method("reconfigure", (), Some(&kwargs));
        }

        // music21 has to be fully imported before anything is installed over
        // it: `install_into_music21` rebinds the classes wherever they were
        // already bound, and there is nothing to rebind until then.
        let _ = py.import("music21")?;
        let installed = music21_rs_python::install_into_music21(py)?;

        let unittest = py.import("unittest")?;
        let common_test = py.import("music21.test.commonTest")?;
        let test_runner = py.import("music21.test.testRunner")?;
        let suite = unittest.getattr("TestSuite")?.call0()?;
        let loader = unittest.getattr("defaultTestLoader")?;

        for name in modules {
            let module = py.import(*name)?;
            if module.hasattr("Test")? {
                let tests =
                    loader.call_method1("loadTestsFromTestCase", (module.getattr("Test")?,))?;
                suite.call_method1("addTests", (tests,))?;
            }
            // A module `defaultDoctestSuite` refuses has no docstrings to run,
            // which upstream skips rather than reporting.
            if let Ok(tests) = common_test.call_method1("defaultDoctestSuite", (&module,)) {
                suite.call_method1("addTests", (tests,))?;
            }
        }
        test_runner.call_method1("fixDoctests", (&suite,))?;

        py.import("warnings")?
            .call_method1("simplefilter", ("ignore",))?;

        let kwargs = PyDict::new(py);
        kwargs.set_item("verbosity", 0)?;
        kwargs.set_item("stream", py.import("io")?.getattr("StringIO")?.call0()?)?;
        let result = unittest
            .getattr("TextTestRunner")?
            .call((), Some(&kwargs))?
            .call_method1("run", (&suite,))?;

        let mut bad = BTreeSet::new();
        for key in ["failures", "errors"] {
            for pair in result.getattr(key)?.cast_into::<PyList>()?.try_iter()? {
                let case = pair?.get_item(0)?;
                bad.insert(case.str()?.extract::<String>()?);
            }
        }
        Ok(Outcome {
            run: result.getattr("testsRun")?.extract()?,
            bad,
            installed,
        })
    })
    .unwrap_or_else(|error| panic!("running music21's suite against the crate: {error}"))
}
