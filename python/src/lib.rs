//! `music21_rs`: music21-shaped Python classes over the `music21-rs` crate.
//!
//! Each class here carries music21's name, module string, constructor
//! keywords, property names and `repr`, and holds a `music21-rs` value
//! underneath. That makes the module two things at once: a Python package for
//! callers who want the crate's analysis without a Rust toolchain, and the
//! subject of `python-parity`, which imports the real music21 and runs its
//! own doctests against these classes.
//!
//! The classes are deliberately thin. Anything music21 does that the crate
//! does not is left to fail rather than reimplemented here in Python-shaped
//! Rust — the point is to expose the crate, not to build a second music21.

use pyo3::prelude::*;

pub mod chord;
pub mod interval;
pub mod key;
pub mod notation;
pub mod note;
pub mod pitch;
pub mod serial;

pub use pitch::{Accidental, Microtone, Pitch};

/// Adds every class and function to a module. `python-parity` builds its own
/// module around this one, so the registration is separate from the
/// `#[pymodule]` below.
pub fn register_all(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pitch::register(m)?;
    serial::register(m)?;
    key::register(m)?;
    interval::register(m)?;
    notation::register(m)?;
    note::register(m)?;
    chord::register(m)?;
    Ok(())
}

/// The Python module: `import music21_rs`. One flat namespace holding every
/// class, since music21's own module layout is what the classes carry in
/// their `__module__` strings rather than where they are imported from.
#[pymodule]
pub fn music21_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_all(m)?;
    // maturin's generated package does `from .music21_rs import *`, which
    // without this would pull the extension module in under its own name.
    let mut names: Vec<String> = m
        .dict()
        .keys()
        .iter()
        .filter_map(|key| key.extract::<String>().ok())
        .filter(|name| !name.starts_with('_'))
        .collect();
    names.sort();
    m.add("__all__", names)?;
    Ok(())
}
