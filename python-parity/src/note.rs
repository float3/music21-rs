//! music21's `note.Note` and `duration.Duration` over `music21-rs`, enough of
//! each for the chord facade to hand real notes back and forth.

#![allow(non_snake_case)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::{Duration as RsDuration, Note as RsNote, Pitch as RsPitch};

use crate::interval::transpose_pitch_by_any;
use crate::pitch::{Pitch, message, pitch_from_any};

pyo3::create_exception!(music21_rs_facade, NoteException, PyException);

pub(crate) fn note_error(error: music21_rs::Error) -> PyErr {
    NoteException::new_err(message(&error))
}

/// music21's `duration.Duration`, over the crate's quarter-length duration.
#[pyclass(name = "Duration", module = "music21.duration", skip_from_py_object)]
#[derive(Clone)]
pub struct Duration {
    pub(crate) inner: RsDuration,
}

impl Duration {
    pub(crate) fn wrap(inner: RsDuration) -> Self {
        Self { inner }
    }
}

/// Reads a duration argument: a `Duration`, a quarter length, or a type name
/// such as `"half"`.
pub(crate) fn duration_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsDuration> {
    if let Ok(facade) = value.extract::<PyRef<Duration>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(name) = value.extract::<String>() {
        return music21_rs::duration::DurationType::from_music21_name(&name)
            .map(RsDuration::from_type)
            .ok_or_else(|| NoteException::new_err(format!("no such duration type: {name}")));
    }
    if let Ok(quarter_length) = value.extract::<f64>() {
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    if let Ok(quarter_length) = value
        .getattr("quarterLength")
        .and_then(|value| value.extract::<f64>())
    {
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    Err(NoteException::new_err(format!(
        "cannot read a duration from {}",
        value.repr()?
    )))
}

#[pymethods]
impl Duration {
    #[new]
    #[pyo3(signature = (value = None, **_keywords))]
    fn new(
        value: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let inner = match value.filter(|value| !value.is_none()) {
            Some(value) => duration_from_any(value)?,
            None => RsDuration::new(1.0).map_err(note_error)?,
        };
        Ok(Self { inner })
    }

    #[getter]
    fn get_quarterLength(&self) -> f64 {
        self.inner.quarter_length()
    }

    #[setter]
    fn set_quarterLength(&mut self, value: f64) -> PyResult<()> {
        self.inner.set_quarter_length(value).map_err(note_error)
    }

    #[getter]
    fn get_type(&self) -> String {
        self.inner.duration_type().map_or_else(
            || "complex".to_string(),
            |kind| kind.music21_name().to_string(),
        )
    }

    #[setter]
    fn set_type(&mut self, value: &str) -> PyResult<()> {
        let kind = music21_rs::duration::DurationType::from_music21_name(value)
            .ok_or_else(|| NoteException::new_err(format!("no such duration type: {value}")))?;
        let dots = self.inner.dots();
        self.inner = RsDuration::from_type_with_dots(kind, dots);
        Ok(())
    }

    #[getter]
    fn dots(&self) -> u32 {
        self.inner.dots()
    }

    #[getter]
    fn ordinal(&self) -> Option<usize> {
        self.inner.ordinal()
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner
            .full_name()
            .unwrap_or_else(|| "Duration".to_string())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .getattr("quarterLength")
            .ok()
            .and_then(|value| value.extract::<f64>().ok())
            .is_some_and(|value| value == self.inner.quarter_length())
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.duration.Duration {:?}>",
            self.inner.quarter_length()
        )
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// music21's `note.Note`: a pitch with a duration.
#[pyclass(name = "Note", module = "music21.note", subclass, skip_from_py_object)]
#[derive(Clone)]
pub struct Note {
    pub(crate) inner: RsNote,
}

impl Note {
    pub(crate) fn wrap(inner: RsNote) -> Self {
        Self { inner }
    }

    fn quarter_length(&self) -> f64 {
        self.inner
            .duration()
            .map_or(1.0, RsDuration::quarter_length)
    }
}

/// Reads a note argument: a `Note`, a pitch, or a name.
pub(crate) fn note_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsNote> {
    if let Ok(facade) = value.extract::<PyRef<Note>>() {
        return Ok(facade.inner.clone());
    }
    Ok(RsNote::from_pitch(pitch_from_any(value)?))
}

#[pymethods]
impl Note {
    #[new]
    #[pyo3(signature = (pitch = None, **keywords))]
    fn new(
        pitch: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let inner = match pitch.filter(|value| !value.is_none()) {
            Some(value) => RsNote::from_pitch(pitch_from_any(value)?),
            None => RsNote::from_name("C4").map_err(note_error)?,
        };
        let mut note = Self { inner };
        if let Some(keywords) = keywords {
            if let Some(value) = keywords.get_item("quarterLength")? {
                note.set_quarterLength(value.extract::<f64>()?)?;
            }
            if let Some(value) = keywords.get_item("duration")? {
                note.inner.set_duration(duration_from_any(&value)?);
            }
        }
        Ok(note)
    }

    #[getter]
    fn get_pitch(&self) -> Pitch {
        Pitch::wrap(self.inner.pitch().clone(), false)
    }

    #[setter]
    fn set_pitch(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let duration = self.inner.duration().cloned();
        self.inner = RsNote::from_pitch(pitch_from_any(value)?);
        if let Some(duration) = duration {
            self.inner.set_duration(duration);
        }
        Ok(())
    }

    #[getter]
    fn get_name(&self) -> String {
        self.inner.pitch_name()
    }

    #[setter]
    fn set_name(&mut self, value: &str) -> PyResult<()> {
        let octave = self.inner.pitch().octave();
        let name = match octave {
            Some(octave) if !value.chars().any(|ch| ch.is_ascii_digit()) => {
                format!("{value}{octave}")
            }
            _ => value.to_string(),
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        let duration = self.inner.duration().cloned();
        self.inner = RsNote::from_pitch(pitch);
        if let Some(duration) = duration {
            self.inner.set_duration(duration);
        }
        Ok(())
    }

    #[getter]
    fn nameWithOctave(&self) -> String {
        self.inner.pitch_name_with_octave()
    }

    #[getter]
    fn step(&self) -> String {
        self.inner.step().to_string()
    }

    #[getter]
    fn octave(&self) -> Option<i32> {
        self.inner.octave()
    }

    #[getter]
    fn pitches(&self) -> Vec<Pitch> {
        self.inner
            .pitches()
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect()
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner.full_name()
    }

    #[getter]
    fn get_duration(&self) -> Duration {
        Duration::wrap(
            self.inner
                .duration()
                .cloned()
                .unwrap_or_else(RsDuration::quarter),
        )
    }

    #[setter]
    fn set_duration(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.set_duration(duration_from_any(value)?);
        Ok(())
    }

    #[getter]
    fn get_quarterLength(&self) -> f64 {
        self.quarter_length()
    }

    #[setter]
    fn set_quarterLength(&mut self, value: f64) -> PyResult<()> {
        self.inner
            .set_duration(RsDuration::new(value).map_err(note_error)?);
        Ok(())
    }

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isNote(&self) -> bool {
        true
    }

    #[getter]
    fn isChord(&self) -> bool {
        false
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(&mut self, value: &Bound<'_, PyAny>, inPlace: bool) -> PyResult<Option<Self>> {
        let pitch = transpose_pitch_by_any(self.inner.pitch(), value)?;
        let mut moved = RsNote::from_pitch(pitch);
        if let Some(duration) = self.inner.duration().cloned() {
            moved.set_duration(duration);
        }
        if inPlace {
            self.inner = moved;
            Ok(None)
        } else {
            Ok(Some(Self { inner: moved }))
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<Note>>().is_ok_and(|other| {
            other.inner.pitch() == self.inner.pitch()
                && other.quarter_length() == self.quarter_length()
        })
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.note.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().inner.pitch_name()
        ))
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Note>()?;
    m.add_class::<Duration>()?;
    let exception = py.get_type::<NoteException>();
    exception.setattr("__module__", "music21.note")?;
    m.add("NoteException", exception)?;
    Ok(())
}
