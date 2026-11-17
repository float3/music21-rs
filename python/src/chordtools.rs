//! music21's `chord.tools`: the chord helpers too rare for the `Chord`
//! itself, over the crate's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::chord::chord_from_any;
use crate::pitch::pitch_from_any;

/// Every chord step the chord has, each the first of its pitches on that
/// step, keyed in the order the pitches come. The chord's `root()` decides
/// the steps, so a chord with no root raises what that raises.
#[pyfunction]
fn allChordSteps<'py>(c: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py = c.py();
    let root = pitch_from_any(&c.call_method0("root")?)?;
    let pitches = c.getattr("pitches")?;
    let chord = chord_from_any(Some(&pitches))?;
    let steps = PyDict::new(py);
    for (step, index) in chord.all_chord_steps(&root) {
        steps.set_item(step, pitches.get_item(index)?)?;
    }
    Ok(steps)
}

static BUILT: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// music21's `chord.tools`, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.chord.tools")?;
        m.add_function(wrap_pyfunction!(allChordSteps, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
