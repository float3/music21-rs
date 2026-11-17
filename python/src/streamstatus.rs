//! music21's `stream.streamStatus`: what a stream knows of its own
//! notation, whether its beams, accidentals and tuplet brackets have been
//! made.
//!
//! Each stream makes one on first asking, its `client` the stream. What it
//! has not been told it works out from what the stream holds, the first
//! time it is asked, and keeps.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyModule;

/// The module music21 keeps this in.
const MODULE: &str = "music21.stream.streamStatus";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// music21's `StreamStatus`: the notation state of the stream it serves.
#[pyclass(
    name = "StreamStatus",
    module = "music21.stream.streamStatus",
    subclass
)]
#[derive(Default)]
pub struct StreamStatus {
    /// The stream this reports on.
    #[pyo3(get, set)]
    client: Option<Py<PyAny>>,
    /// Whether accidentals have been made, where known.
    #[pyo3(get, set)]
    _accidentals: Option<bool>,
    /// Whether beams have been made, where known.
    #[pyo3(get, set)]
    _beams: Option<bool>,
    /// Whether tuplet brackets have been made, where known.
    #[pyo3(get, set)]
    _tuplets: Option<bool>,
    /// music21 keeps these and reads none of them.
    #[pyo3(get, set)]
    _concertPitch: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    _dirty: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    _enharmonics: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    _measures: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    _ornaments: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    _rests: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    _ties: Option<Py<PyAny>>,
}

impl StreamStatus {
    /// The stream, which a status made bare has none of.
    fn client_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.client {
            Some(client) => Ok(client.bind(py).clone()),
            None => Err(pyo3::exceptions::PyAttributeError::new_err(
                "'NoneType' object has no attribute 'recurse'",
            )),
        }
    }

    /// Everything the stream holds at any depth that is one of `class`.
    fn recursed<'py>(&self, py: Python<'py>, class: &str) -> PyResult<Bound<'py, PyAny>> {
        self.client_of(py)?
            .call_method0("recurse")?
            .call_method1("getElementsByClass", (class,))
    }
}

/// `bool(value)` for a value given, `None` kept as unknown.
fn told(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<bool>> {
    value
        .filter(|value| !value.is_none())
        .map(|value| value.is_truthy())
        .transpose()
}

#[pymethods]
impl StreamStatus {
    #[new]
    #[pyo3(signature = (client = None))]
    fn new(client: Option<Py<PyAny>>) -> Self {
        Self {
            client,
            ..Self::default()
        }
    }

    /// A copy knowing what this one knows, serving no stream: the stream
    /// copying it makes itself the copy's client.
    #[pyo3(signature = (memo = None))]
    fn __deepcopy__(&self, py: Python<'_>, memo: Option<&Bound<'_, PyAny>>) -> Self {
        let _ = memo;
        let copy = |value: &Option<Py<PyAny>>| value.as_ref().map(|value| value.clone_ref(py));
        Self {
            client: None,
            _accidentals: self._accidentals,
            _beams: self._beams,
            _tuplets: self._tuplets,
            _concertPitch: copy(&self._concertPitch),
            _dirty: copy(&self._dirty),
            _enharmonics: copy(&self._enharmonics),
            _measures: copy(&self._measures),
            _ornaments: copy(&self._ornaments),
            _rests: copy(&self._rests),
            _ties: copy(&self._ties),
        }
    }

    /// Whether any pitch the stream holds has an accidental whose display
    /// has been decided, which is taken to mean accidentals were made.
    fn haveAccidentalsBeenMade(&self, py: Python<'_>) -> PyResult<bool> {
        for pitch in self.client_of(py)?.getattr("pitches")?.try_iter()? {
            let accidental = pitch?.getattr("accidental")?;
            if accidental.is_none() {
                continue;
            }
            if !accidental.getattr("displayStatus")?.is_none() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether any note or chord the stream holds has beams, which is
    /// taken to mean beams were made.
    fn haveBeamsBeenMade(&self, py: Python<'_>) -> PyResult<bool> {
        for found in self.recursed(py, "NotRest")?.try_iter()? {
            let beams = found?.getattr("beams")?;
            if beams.is_none() {
                continue;
            }
            if beams.getattr("beamsList")?.is_truthy()? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether the tuplet brackets have been made: `None` where nothing
    /// the stream holds is a tuplet, true where the first tuplet of one
    /// has a bracket type, false otherwise.
    fn haveTupletBracketsBeenMade(&self, py: Python<'_>) -> PyResult<Option<bool>> {
        let mut first_typed = Vec::new();
        for found in self.recursed(py, "GeneralNote")?.try_iter()? {
            let tuplets = found?.getattr("duration")?.getattr("tuplets")?;
            if !tuplets.is_truthy()? {
                continue;
            }
            first_typed.push(!tuplets.get_item(0)?.getattr("type")?.is_none());
        }
        Ok(music21::stream::status::tuplet_brackets_made(first_typed))
    }

    /// Whether accidentals have been made, worked out on first asking.
    #[getter]
    fn get_accidentals(&mut self, py: Python<'_>) -> PyResult<bool> {
        if let Some(known) = self._accidentals {
            return Ok(known);
        }
        let made = self.haveAccidentalsBeenMade(py)?;
        self._accidentals = Some(made);
        Ok(made)
    }

    #[setter]
    fn set_accidentals(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self._accidentals = told(value)?;
        Ok(())
    }

    /// Whether beams have been made, worked out on first asking.
    #[getter]
    fn get_beams(&mut self, py: Python<'_>) -> PyResult<bool> {
        if let Some(known) = self._beams {
            return Ok(known);
        }
        let made = self.haveBeamsBeenMade(py)?;
        self._beams = Some(made);
        Ok(made)
    }

    #[setter]
    fn set_beams(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self._beams = told(value)?;
        Ok(())
    }

    /// Whether tuplet brackets have been made, worked out on first asking:
    /// a stream with no tuplets needs none, so has them all.
    #[getter]
    fn get_tuplets(&mut self, py: Python<'_>) -> PyResult<bool> {
        if let Some(known) = self._tuplets {
            return Ok(known);
        }
        let made = self.haveTupletBracketsBeenMade(py)?.unwrap_or(true);
        self._tuplets = Some(made);
        Ok(made)
    }

    #[setter]
    fn set_tuplets(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self._tuplets = told(value)?;
        Ok(())
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.client)?;
        visit.call(&self._concertPitch)?;
        visit.call(&self._dirty)?;
        visit.call(&self._enharmonics)?;
        visit.call(&self._measures)?;
        visit.call(&self._ornaments)?;
        visit.call(&self._rests)?;
        visit.call(&self._ties)
    }

    fn __clear__(&mut self) {
        self.client = None;
        self._concertPitch = None;
        self._dirty = None;
        self._enharmonics = None;
        self._measures = None;
        self._ornaments = None;
        self._rests = None;
        self._ties = None;
    }
}

/// music21's `stream.streamStatus`, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<StreamStatus>()?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
