//! music21's `humdrum.harmParser`: a `**harm` token read as the roman
//! numeral it annotates.
//!
//! `humdrum::harm::Harm::parse` is the reading and
//! `humdrum::harm::convert_harm_to_roman` the numeral it writes, both held
//! to music21 token for token by `humdrum_parity`. music21 reads a token
//! with a regular expression and answers the groups as a dictionary; the
//! crate reads it by hand, in the same order, and the facade writes the
//! same dictionary out of what it read.
//!
//! `HarmDefs` stays music21's: it is the regular expressions themselves,
//! which the crate has none of.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::humdrum::harm::{Harm, convert_harm_to_roman};

/// The names this module hands out.
pub const NAMES: &[&str] = &["convertHarmToRoman", "HarmParser"];

/// music21's `convertHarmToRoman`: the figure a `RomanNumeral` is built
/// from, or nothing where the token says no chord.
#[pyfunction]
fn convertHarmToRoman(harmStr: &str) -> Option<String> {
    convert_harm_to_roman(harmStr)
}

/// One token as music21's dictionary of its parts, which is what its own
/// regular expression's groups come to.
fn read<'py>(py: Python<'py>, harm: &Harm) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    let said = |value: &str| -> Option<String> { (!value.is_empty()).then(|| value.to_string()) };
    out.set_item("accidental", said(harm.accidental()))?;
    out.set_item("root", harm.root())?;
    out.set_item("attribute", harm.attribute().map(String::from))?;
    out.set_item("intervals", harm.intervals().map(String::from))?;
    out.set_item("inversion", harm.inversion().map(String::from))?;
    out.set_item(
        "alternative",
        match harm.alternative() {
            Some(alternative) => read(py, alternative)?.into_any(),
            None => py.None().into_bound(py),
        },
    )?;
    out.set_item(
        "secondary",
        match harm.secondary() {
            Some(secondary) => read(py, secondary)?.into_any(),
            None => py.None().into_bound(py),
        },
    )?;
    out.set_item("implied", harm.implied())?;
    Ok(out)
}

/// music21's `humdrum.harmParser.HarmParser`: a reader of `**harm`
/// tokens, which keeps nothing between readings.
#[pyclass(
    name = "HarmParser",
    module = "music21.humdrum.harmParser",
    subclass,
    skip_from_py_object
)]
pub struct HarmParser;

#[pymethods]
impl HarmParser {
    #[new]
    fn new() -> Self {
        Self
    }

    /// The parts of a token: music21's `parse`, which answers an empty
    /// dictionary for a token it cannot read.
    fn parse<'py>(&self, py: Python<'py>, harmExpression: &str) -> PyResult<Bound<'py, PyDict>> {
        match Harm::parse(harmExpression) {
            Some(harm) => read(py, &harm),
            None => Ok(PyDict::new(py)),
        }
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HarmParser>()?;
    m.add_function(wrap_pyfunction!(convertHarmToRoman, m)?)?;
    Ok(())
}
