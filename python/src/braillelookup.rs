//! music21's `braille.lookup`: the signs braille is written with.
//!
//! The tables themselves stay music21's -- its own code looks them up by
//! name, and the crate's `braille::lookup` answers the same questions as
//! functions rather than as dictionaries, which is what every transcription
//! here asks. What is replaced is the two functions that *make* a sign out
//! of its dots, which is arithmetic over the braille block and is the
//! crate's: `lookup::cell` and `lookup::dots_add`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::braille::lookup;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["makeBrailleDictionary", "dotsAdd"];

/// The dots a cell holds, written as music21 writes them: each dot in
/// turn, so dots one, two and four are 124, and nought for the blank cell.
fn dots_of(bits: u32) -> u32 {
    (1..=6).fold(0, |dots, dot| {
        if bits & (1 << (dot - 1)) == 0 {
            dots
        } else {
            dots * 10 + dot
        }
    })
}

/// music21's `makeBrailleDictionary`: every one of the sixty-four cells,
/// each against the dots it holds.
#[pyfunction]
fn makeBrailleDictionary<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for bits in 0u32..64 {
        let dots = dots_of(bits);
        out.set_item(dots, lookup::cell(dots).to_string())?;
    }
    Ok(out)
}

/// music21's `dotsAdd`: the cell holding every dot of these numbers,
/// whatever order they are given in.
#[pyfunction]
fn dotsAdd(dotIter: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut dots = Vec::new();
    for number in dotIter.walk()? {
        dots.push(number?.extract::<u32>()?);
    }
    Ok(lookup::dots_add(&dots).to_string())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(makeBrailleDictionary, m)?)?;
    m.add_function(wrap_pyfunction!(dotsAdd, m)?)?;
    Ok(())
}
