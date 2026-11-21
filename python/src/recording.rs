//! music21's `audioSearch.recording`: recording from a microphone.
//!
//! The package records nothing. The module is here so that what imports it
//! imports, with music21's defaults and its exception; recording refuses
//! when it is asked for, with the `ModuleNotFoundError` music21 raises
//! where pyaudio, which it records with, is not installed.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyModuleNotFoundError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};

use music21::audiosearch::{AUDIO_CHUNK_LENGTH, RECORD_CHANNELS, RECORD_SAMPLE_RATE};

/// The module's dotted name.
const MODULE: &str = "music21.audioSearch.recording";

/// The package music21 records with.
const RECORDER: &str = "pyaudio";

pyo3::create_exception!(
    music21_rs_facade,
    RecordingException,
    crate::Music21Exception
);

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let environment = crate::environment::module(py)?
            .getattr("Environment")?
            .call1(("audioSearch.recording",))?;
        m.add("environLocal", environment)?;
        m.add("default_recordChannels", RECORD_CHANNELS)?;
        // music21 writes the rate as a whole number.
        m.add("default_recordSampleRate", RECORD_SAMPLE_RATE as u32)?;
        m.add("default_recordChunkLength", AUDIO_CHUNK_LENGTH)?;
        let exception = py.get_type::<RecordingException>();
        exception.setattr("__module__", MODULE)?;
        m.add("RecordingException", exception)?;
        m.add_function(wrap_pyfunction!(samplesFromRecording, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// The refusal a request to record meets.
pub(crate) fn refused() -> PyErr {
    let error = PyModuleNotFoundError::new_err(format!(
        "music21-rs does not record from a microphone; music21 records with {RECORDER}"
    ));
    Python::attach(|py| {
        let _ = error.value(py).setattr("name", RECORDER);
    });
    error
}

/// music21's `samplesFromRecording`, which records from a microphone:
/// refused.
#[pyfunction]
#[pyo3(signature = (*_arguments, **_keywords))]
fn samplesFromRecording(
    _arguments: &Bound<'_, PyTuple>,
    _keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    Err(refused())
}
