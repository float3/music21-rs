//! music21's `musicxml.xmlSoundParser`: a `<sound>` read onto the
//! `MeasureParser` reading the measure it stands in.
//!
//! What a `<sound>` says is read by the crate's MusicXML reader; these
//! functions build music21's objects from it and hand them to the parser
//! as music21's do, through the parser's own members, so they serve
//! whichever `MeasureParser` they are given.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyUserWarning, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule};

/// The module music21 keeps these in.
const MODULE: &str = "music21.musicxml.xmlSoundParser";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// music21's `xmlSound`: a `<sound>` standing alone in a measure, read at
/// where the parser has got to, moved by its own `<offset>`.
#[pyfunction]
fn xmlSound(mp: &Bound<'_, PyAny>, mxSound: &Bound<'_, PyAny>) -> PyResult<()> {
    let moved: f64 = mp.call_method1("xmlToOffset", (mxSound,))?.extract()?;
    let at: f64 = mp.getattr("offsetMeasureNote")?.extract()?;
    setSound(mp, mxSound, None, moved + at)
}

/// music21's `setSound`: what a `<sound>` sets, on the staff its
/// `<direction>` names where it stands in one. Only a tempo is read yet,
/// as in music21.
#[pyfunction]
#[pyo3(signature = (mp, mxSound, *, mxDir, totalOffset))]
fn setSound(
    mp: &Bound<'_, PyAny>,
    mxSound: &Bound<'_, PyAny>,
    mxDir: Option<&Bound<'_, PyAny>>,
    totalOffset: f64,
) -> PyResult<()> {
    let mxDir = mxDir.filter(|direction| !direction.is_none());
    let staffKey = mp.call_method1("getStaffNumber", (mxDir.unwrap_or(mxSound),))?;
    if mxSound.getattr("attrib")?.contains("tempo")? {
        setSoundTempo(mp, mxSound, mxDir, &staffKey, totalOffset)?;
    }
    Ok(())
}

/// music21's `setSoundTempo`: a `<sound>`'s tempo as a metronome mark
/// heard but not shown, a quarter its beat. A tempo of nought warns and
/// sets nothing.
#[pyfunction]
#[pyo3(signature = (mp, mxSound, *, mxDir, staffKey, totalOffset))]
fn setSoundTempo(
    mp: &Bound<'_, PyAny>,
    mxSound: &Bound<'_, PyAny>,
    mxDir: Option<&Bound<'_, PyAny>>,
    staffKey: &Bound<'_, PyAny>,
    totalOffset: f64,
) -> PyResult<()> {
    let py = mp.py();
    let text: String = mxSound
        .call_method1("get", ("tempo", "0"))?
        .str()?
        .to_string();
    let Some(qpm) = music21::musicxml::sound_tempo(&text) else {
        if text.trim().parse::<f64>().is_err() {
            return Err(PyValueError::new_err(format!(
                "could not convert string to float: {text:?}"
            )));
        }
        PyErr::warn(
            py,
            &py.get_type::<PyUserWarning>(),
            c"0 qpm tempo tag found, skipping.",
            2,
        )?;
        return Ok(());
    };

    // music21's `numToIntOrFloat`: a whole tempo is an `int`.
    let sounding = if qpm.fract() == 0.0 {
        (qpm as i64).into_pyobject(py)?.into_any()
    } else {
        qpm.into_pyobject(py)?.into_any()
    };
    let keywords = PyDict::new(py);
    keywords.set_item("type", "quarter")?;
    let referent =
        crate::class_of(py, "music21.duration", "Duration")?.call((), Some(&keywords))?;
    let keywords = PyDict::new(py);
    keywords.set_item("referent", referent)?;
    keywords.set_item("number", py.None())?;
    keywords.set_item("numberSounding", sounding)?;
    let mark = crate::class_of(py, "music21.tempo", "MetronomeMark")?.call((), Some(&keywords))?;

    crate::musicxml::synchronizeIdsToM21(mxSound, &mark)?;
    mp.call_method1("setPrintObject", (mxSound, &mark))?;
    mp.call_method1("setPosition", (mxSound, &mark))?;
    if let Some(direction) = mxDir.filter(|direction| !direction.is_none()) {
        crate::musicxml::setM21AttributeFromAttribute(
            &mark,
            direction,
            "placement",
            Some("placement".to_string()),
            None,
        )?;
        mp.call_method1("setEditorial", (direction, &mark))?;
    }
    mp.call_method1("insertCoreAndRef", (totalOffset, staffKey, mark))?;
    Ok(())
}

/// `music21.musicxml.xmlSoundParser`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_function(wrap_pyfunction!(xmlSound, &m)?)?;
        m.add_function(wrap_pyfunction!(setSound, &m)?)?;
        m.add_function(wrap_pyfunction!(setSoundTempo, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
