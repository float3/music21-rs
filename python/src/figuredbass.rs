//! music21's `figuredBass.notation`, over the crate's figured bass.
//!
//! A column of figures says how far above the bass each note of the chord
//! stands, and an accidental beside a number says how that note is spelled.
//! Most of a column is left out in practice, so it is read in two forms: as
//! written, and expanded to every note it stands for.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use music21_rs::figuredbass::{
    EXTENDER, Figure as RsFigure, Modifier as RsModifier, Notation as RsNotation,
};

use crate::pitch::{Accidental, Pitch, message, pitch_from_any};

/// The names the `figuredbass` facade replaces in
/// `music21.figuredBass.notation`.
pub const NAMES: &[&str] = &[
    "Notation",
    "Figure",
    "Modifier",
    "NotationException",
    "ModifierException",
    "convertToPitch",
];

pyo3::create_exception!(music21_rs_facade, NotationException, PyException);
pyo3::create_exception!(music21_rs_facade, ModifierException, PyException);

fn modifier_error(error: music21_rs::Error) -> PyErr {
    ModifierException::new_err(message(&error))
}

fn notation_error(error: music21_rs::Error) -> PyErr {
    NotationException::new_err(message(&error))
}

/// music21's `Modifier`: the accidental written beside a figure.
#[pyclass(
    name = "Modifier",
    module = "music21.figuredBass.notation",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Modifier {
    pub(crate) inner: RsModifier,
}

#[pymethods]
impl Modifier {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<(Py<PyAny>, (), Py<PyAny>)> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsModifier>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (modifierString = None))]
    fn new(modifierString: Option<&str>) -> PyResult<Self> {
        Ok(Self {
            inner: RsModifier::new(modifierString).map_err(modifier_error)?,
        })
    }

    #[getter]
    fn modifierString(&self) -> Option<&str> {
        self.inner.written()
    }

    #[getter]
    fn accidental(&self) -> Option<Accidental> {
        self.inner.accidental().cloned().map(Accidental::from_inner)
    }

    /// music21's `modifyPitchName`: the note's name, spelled as this
    /// modifier asks.
    fn modifyPitchName(&self, pitchNameToAlter: &str) -> PyResult<String> {
        let pitch = music21_rs::Pitch::from_name(pitchNameToAlter).map_err(modifier_error)?;
        Ok(self.inner.modify(&pitch).map_err(modifier_error)?.name())
    }

    /// music21's `modifyPitch`: the same, on a pitch object.
    #[pyo3(signature = (pitchToAlter, *, inPlace = false))]
    fn modifyPitch(
        &self,
        py: Python<'_>,
        pitchToAlter: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Py<PyAny>> {
        let modified = self
            .inner
            .modify(&pitch_from_any(pitchToAlter)?)
            .map_err(modifier_error)?;
        if inPlace {
            pitchToAlter.setattr(
                "accidental",
                Accidental::from_inner(modified.accidental().clone()),
            )?;
            return Ok(py.None());
        }
        Ok(Pitch::wrap(modified, false)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    fn __repr__(&self) -> String {
        format!("<music21.figuredBass.notation.Modifier {}>", self.inner)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner)
    }
}

/// music21's `Figure`: one number of a column, with its modifier.
#[pyclass(
    name = "Figure",
    module = "music21.figuredBass.notation",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Figure {
    pub(crate) inner: RsFigure,
}

#[pymethods]
impl Figure {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<(Py<PyAny>, (), Py<PyAny>)> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsFigure>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (number = Some(1), modifierString = Some(String::new()), *, extender = false))]
    fn new(number: Option<i32>, modifierString: Option<String>, extender: bool) -> PyResult<Self> {
        let modifier = RsModifier::new(modifierString.as_deref()).map_err(modifier_error)?;
        Ok(Self {
            inner: RsFigure::new(number, modifier, extender),
        })
    }

    #[getter]
    fn get_number(&self) -> Option<i32> {
        self.inner.number()
    }

    #[setter]
    fn set_number(&mut self, value: Option<i32>) {
        self.inner.set_number(value);
    }

    #[getter]
    fn modifierString(&self) -> Option<&str> {
        self.inner.modifier().written()
    }

    #[getter]
    fn modifier(&self) -> Modifier {
        Modifier {
            inner: self.inner.modifier().clone(),
        }
    }

    #[getter]
    fn hasExtender(&self) -> bool {
        self.inner.has_extender()
    }

    /// music21's `isPureExtender`: a line and no number, which carries the
    /// column forward from the note before.
    #[getter]
    fn isPureExtender(&self) -> bool {
        self.inner.is_pure_extender()
    }

    fn __repr__(&self) -> String {
        format!("<music21.figuredBass.notation.Figure {}>", self.inner)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner)
    }
}

/// music21's `Notation`: one column of figured bass.
#[pyclass(
    name = "Notation",
    module = "music21.figuredBass.notation",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Notation {
    pub(crate) inner: RsNotation,
}

/// A list of optional numbers, as music21 hands them back: a tuple with
/// `None` where a figure was written as a bare accidental.
fn numbers_tuple<'py>(py: Python<'py>, numbers: &[Option<i32>]) -> PyResult<Bound<'py, PyTuple>> {
    let values: Vec<Py<PyAny>> = numbers
        .iter()
        .map(|number| match number {
            Some(number) => Ok(number.into_pyobject(py)?.into_any().unbind()),
            None => Ok(py.None()),
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, values)
}

/// The same for the marks, which are strings or nothing.
fn marks_tuple<'py>(py: Python<'py>, marks: &[Option<String>]) -> PyResult<Bound<'py, PyTuple>> {
    let values: Vec<Py<PyAny>> = marks
        .iter()
        .map(|mark| match mark {
            Some(mark) => Ok(mark.into_pyobject(py)?.into_any().unbind()),
            None => Ok(py.None()),
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, values)
}

#[pymethods]
impl Notation {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<(Py<PyAny>, (), Py<PyAny>)> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsNotation>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (notationColumn = None))]
    fn new(notationColumn: Option<&str>) -> PyResult<Self> {
        Ok(Self {
            inner: RsNotation::parse(notationColumn.unwrap_or("")).map_err(notation_error)?,
        })
    }

    #[getter]
    fn notationColumn(&self) -> &str {
        self.inner.column()
    }

    #[getter]
    fn figureStrings(&self) -> Vec<String> {
        self.inner.figure_strings().to_vec()
    }

    #[getter]
    fn origNumbers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        numbers_tuple(py, self.inner.original_numbers())
    }

    #[getter]
    fn origModStrings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        marks_tuple(py, self.inner.original_modifiers())
    }

    #[getter]
    fn numbers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        numbers_tuple(py, self.inner.numbers())
    }

    #[getter]
    fn modifierStrings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        marks_tuple(py, self.inner.modifier_strings())
    }

    #[getter]
    fn modifiers<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(
            py,
            self.inner.figures().iter().map(|figure| Modifier {
                inner: figure.modifier().clone(),
            }),
        )
    }

    #[getter]
    fn figures(&self) -> Vec<Figure> {
        self.inner
            .figures()
            .iter()
            .map(|figure| Figure {
                inner: figure.clone(),
            })
            .collect()
    }

    #[getter]
    fn figuresFromNotationColumn(&self) -> Vec<Figure> {
        self.inner
            .figures_as_written()
            .iter()
            .map(|figure| Figure {
                inner: figure.clone(),
            })
            .collect()
    }

    #[getter]
    fn extenders(&self) -> Vec<bool> {
        self.inner.extenders().to_vec()
    }

    #[getter]
    fn hasExtenders(&self) -> bool {
        self.inner.has_extenders()
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.figuredBass.notation.Notation {}>",
            self.inner.column()
        )
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner)
    }
}

/// music21's `convertToPitch`: a pitch, or the name of one, as a pitch.
#[pyfunction]
pub fn convertToPitch(py: Python<'_>, pitchString: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    if pitchString.hasattr("nameWithOctave")? {
        return Ok(pitchString.clone().unbind());
    }
    let name: String = pitchString.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "Cannot convert {pitchString} to a music21 Pitch."
        ))
    })?;
    let pitch = music21_rs::Pitch::from_name(&name).map_err(|_| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Cannot convert string {name} to a music21 Pitch."
        ))
    })?;
    Ok(Pitch::wrap(pitch, false)
        .into_pyobject(py)?
        .into_any()
        .unbind())
}

/// The sentinel number a figure carries when it is nothing but an extender.
pub const EXTENDER_SENTINEL: i32 = EXTENDER;

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Notation>()?;
    m.add_class::<Figure>()?;
    m.add_class::<Modifier>()?;
    m.add("NotationException", py.get_type::<NotationException>())?;
    m.add("ModifierException", py.get_type::<ModifierException>())?;
    m.add_function(wrap_pyfunction!(convertToPitch, m)?)?;
    m.add("EXTENDER_SENTINEL", EXTENDER_SENTINEL)?;
    Ok(())
}
