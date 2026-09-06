//! music21-shaped Python facades over `music21-rs`, for running music21's own
//! doctests against the Rust implementation.
//!
//! Each class here has music21's name, module string, constructor keywords,
//! property names and `repr`, and holds a `music21-rs` value underneath. The
//! harness in [`doctest`] imports the real music21, replaces the classes and
//! functions of one music21 module with these, and runs that module's
//! docstrings as they are. What passes is what the crate reproduces to the
//! letter; what fails is either a fidelity gap or a feature the crate does not
//! have.
//!
//! The facades are deliberately thin. Anything music21 does that the crate
//! does not is left to fail rather than reimplemented here in Python-shaped
//! Rust — the point is to measure the crate, not to build a second one.

use pyo3::prelude::*;

pub mod chord;
pub mod doctest;
pub mod interval;
pub mod key;
pub mod note;
pub mod pitch;
pub mod serial;

pub use pitch::{Accidental, Microtone, Pitch};

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

/// The Python module: `import music21_rs_facade`. One flat namespace holding
/// every facade; the harness copies the names each music21 module needs.
#[pymodule]
pub fn music21_rs_facade(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pitch::register(m)?;
    serial::register(m)?;
    key::register(m)?;
    interval::register(m)?;
    note::register(m)?;
    chord::register(m)?;
    m.add_function(wrap_pyfunction!(collect_output, m)?)?;
    Ok(())
}
