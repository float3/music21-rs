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
