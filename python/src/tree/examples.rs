//! music21's `tree.examples`: the score its tree documentation is built
//! on.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyModule;

/// A part of a line of tiny notation put into measures, its instrument
/// named `id`.
fn part_of<'py>(py: Python<'py>, notation: &str, id: &str) -> PyResult<Bound<'py, PyAny>> {
    let converter = py.import("music21.converter")?;
    // music21 makes the measures in place; this package's `makeMeasures`
    // answers a new stream.
    let line = converter
        .call_method1("parse", (notation,))?
        .call_method0("makeMeasures")?;
    let part = py.import("music21.stream")?.getattr("Part")?.call0()?;
    for element in line.try_iter()? {
        part.call_method1("append", (element?,))?;
    }
    let instrument = part.call_method0("getInstrument")?;
    instrument.setattr("partId", id)?;
    part.call_method1("insert", (0, instrument))?;
    Ok(part)
}

/// A score of two parts, four measures of 2/4 each, for showing how a
/// stream becomes a tree.
#[pyfunction]
fn makeExampleScore(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let upper = part_of(py, "tinynotation: 2/4 C4 D E F G A B C", "PartA")?;
    let lower = part_of(py, "tinynotation: 2/4 C#2  G#  E#  D#", "PartB")?;
    let keywords = pyo3::types::PyDict::new(py);
    keywords.set_item("id", "exampleScore")?;
    let score = py
        .import("music21.stream")?
        .getattr("Score")?
        .call((), Some(&keywords))?;
    score.call_method1("insert", (0, upper))?;
    score.call_method1("insert", (0, lower))?;
    Ok(score)
}

/// music21's `tree.examples`, built whole.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "examples")?;
    module.add_function(wrap_pyfunction!(makeExampleScore, &module)?)?;
    module
        .getattr("makeExampleScore")?
        .setattr("__module__", "music21.tree.examples")?;
    Ok(module)
}
