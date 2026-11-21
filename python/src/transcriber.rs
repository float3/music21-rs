//! music21's `audioSearch.transcriber`: a recording of one line turned into
//! a score.
//!
//! Each step is one of `audioSearch`'s functions, reached through that
//! module as music21 reaches them, and each of those is the crate's
//! `audiosearch`. Recording from a microphone is refused, as
//! `audioSearch.recording` refuses it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict};

use crate::subconverters::music21_name;

/// The module's dotted name.
const MODULE: &str = "music21.audioSearch.transcriber";

/// The module music21 calls each step from.
const AUDIO_SEARCH: &str = "music21.audioSearch";

/// The file a recording is read from where `saveFile` is `True`, in the
/// temporary directory.
const DEFAULT_WAVE: &str = "ex.wav";

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let environment = crate::environment::module(py)?
            .getattr("Environment")?
            .call1(("audioSearch.transcriber",))?;
        m.add("environLocal", environment)?;
        m.add_function(wrap_pyfunction!(runTranscribe, &m)?)?;
        m.add_function(wrap_pyfunction!(monophonicStreamFromFile, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// The score of a recording's frequencies, rests at its start left out,
/// and the frequencies music21 plots.
fn transcribed<'py>(
    frequencies: &Bound<'py, PyAny>,
    useScale: Option<&Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = frequencies.py();
    let search = py.import(AUDIO_SEARCH)?;
    let detected = search.call_method1("detectPitchFrequencies", (frequencies, useScale))?;
    let smoothed = search.call_method1("smoothFrequencies", (detected,))?;
    let (pitches, plotted): (Bound<'_, PyAny>, Bound<'_, PyAny>) = search
        .call_method1("pitchFrequenciesToObjects", (smoothed, useScale))?
        .extract()?;
    let (notes, lengths): (Bound<'_, PyAny>, Bound<'_, PyAny>) = search
        .call_method1("joinConsecutiveIdenticalPitches", (pitches,))?
        .extract()?;
    let keywords = PyDict::new(py);
    keywords.set_item("removeRestsAtBeginning", true)?;
    let (score, _length): (Bound<'_, PyAny>, Bound<'_, PyAny>) = search
        .call_method(
            "notesAndDurationsToStream",
            (notes, lengths),
            Some(&keywords),
        )?
        .extract()?;
    Ok((score, plotted))
}

/// The notes of a recording read from `saveFile`, as a score: music21's
/// `runTranscribe`. Recording from a microphone (`useMic`) is refused.
#[pyfunction]
#[pyo3(signature = (show = true, plot = true, useMic = true, seconds = 20.0, useScale = None, saveFile = None))]
fn runTranscribe<'py>(
    py: Python<'py>,
    #[pyo3(from_py_with = crate::truthy)] show: bool,
    #[pyo3(from_py_with = crate::truthy)] plot: bool,
    #[pyo3(from_py_with = crate::truthy)] useMic: bool,
    seconds: f64,
    useScale: Option<Bound<'py, PyAny>>,
    saveFile: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = seconds;
    if useMic {
        return Err(crate::recording::refused());
    }
    let scale = match useScale.filter(|given| !given.is_none()) {
        Some(given) => given,
        None => music21_name(py, "music21.scale", "ChromaticScale")?.call1(("C4",))?,
    };

    // `True`, or nothing, is the temporary directory's file; another
    // value names one.
    let named = saveFile.filter(|given| !given.is_none() && !given.is_instance_of::<PyBool>());
    let file = match named {
        Some(given) => given,
        None => module(py)?
            .getattr("environLocal")?
            .call_method0("getRootTempDir")?
            .call_method1("__truediv__", (DEFAULT_WAVE,))?,
    };
    let keywords = PyDict::new(py);
    keywords.set_item("waveFilename", file.str()?)?;
    let frequencies =
        py.import(AUDIO_SEARCH)?
            .call_method("getFrequenciesFromAudioFile", (), Some(&keywords))?;
    let (score, plotted) = transcribed(&frequencies, Some(&scale))?;

    if show {
        score.call_method0("show")?;
    }
    if plot {
        let pyplot = py.import("matplotlib.pyplot")?;
        pyplot.call_method1("plot", (plotted,))?;
        pyplot.call_method0("show")?;
    }
    Ok(score)
}

/// The notes of a WAV file of one line, as a part: music21's
/// `monophonicStreamFromFile`.
#[pyfunction]
#[pyo3(signature = (fileName, useScale = None))]
fn monophonicStreamFromFile<'py>(
    fileName: &Bound<'py, PyAny>,
    useScale: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = fileName.py();
    let keywords = PyDict::new(py);
    keywords.set_item("waveFilename", fileName)?;
    let frequencies =
        py.import(AUDIO_SEARCH)?
            .call_method("getFrequenciesFromAudioFile", (), Some(&keywords))?;
    let scale = useScale.filter(|given| !given.is_none());
    let (score, _plotted) = transcribed(&frequencies, scale.as_ref())?;
    score.getattr("parts")?.call_method0("first")
}
