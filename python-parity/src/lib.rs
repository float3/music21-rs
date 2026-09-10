//! The harness that runs music21's own doctests against the `music21_rs`
//! Python classes.
//!
//! The classes themselves live in the `music21-rs-python` crate, which is
//! also what maturin builds into a wheel; this crate wraps them in a module
//! of its own so the doctest runner has somewhere to send its output, imports
//! the real music21 from the submodule, replaces the classes and functions of
//! one music21 module with ours, and runs that module's docstrings as they
//! are. What passes is what the crate reproduces to the letter; what fails is
//! either a fidelity gap or a feature the crate does not have.

use pyo3::prelude::*;

pub mod doctest;
pub mod suite;

/// One test that runs a music21 module's doctests against the crate: the
/// module's dotted name, the name its expectation file and log carry, and
/// the music21 modules to swap for the run, each with the names replaced.
/// Every `tests/doctest_*.rs` is one of these; each is its own binary, since
/// a swap lasts for the life of the interpreter.
#[macro_export]
macro_rules! doctest_suite {
    ($test:ident, $module:literal, $name:literal, [$(($python:literal, $names:expr)),* $(,)?]) => {
        #[test]
        fn $test() {
            use $crate::music21_rs_facade;
            ::pyo3::append_to_inittab!(music21_rs_facade);
            $crate::doctest::run($module, $name, &[$(($python, $names)),*]);
        }
    };
}

/// Buffer for doctest output, so a runner's report can be read back from Rust
/// instead of going to stdout.
static OUTPUT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// A `doctest` `out` callback that collects into [`take_output`].
#[pyfunction]
fn collect_output(text: &str) {
    OUTPUT.lock().expect("doctest output buffer").push_str(text);
}

/// Everything written through [`collect_output`] since the last call.
pub fn take_output() -> String {
    std::mem::take(&mut *OUTPUT.lock().expect("doctest output buffer"))
}

/// The module the harness imports: every `music21_rs` class plus the output
/// callback the doctest runner writes through.
#[pymodule]
pub fn music21_rs_facade(m: &Bound<'_, PyModule>) -> PyResult<()> {
    music21_rs_python::register_all(m)?;
    m.add_function(wrap_pyfunction!(collect_output, m)?)?;
    Ok(())
}
