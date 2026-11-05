//! The `harte` crate's reading of a Harte label, for the harte-rs package.
//!
//! harte-rs is a pure-Python package whose `Harte` is a [`Chord`] read from a
//! label, as harte-library's is a music21 `Chord`. The label is read here, by
//! the crate this wheel's chords come from, and [`HarteLabel::fill`] writes
//! the chord it sounds into the `Harte` being built.

// The class attributes carry harte-library's names.
#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::chord::{Chord, ChordException, chord_error};

fn harte_error(error: harte::Error) -> PyErr {
    match error {
        harte::Error::Harte(message) => ChordException::new_err(message),
        harte::Error::Music21(error) => chord_error(error),
    }
}

/// A Harte label, `C:maj7/3`, read into its parts and the chord it sounds.
#[pyclass(name = "_HarteLabel", module = "music21_rs", frozen)]
pub struct HarteLabel {
    inner: harte::Harte,
}

#[pymethods]
impl HarteLabel {
    #[new]
    fn new(label: &str) -> PyResult<Self> {
        harte::Harte::new(label)
            .map(|inner| Self { inner })
            .map_err(harte_error)
    }

    /// The label as it was given.
    #[getter]
    fn label(&self) -> &str {
        self.inner.label()
    }

    /// The root as written, `Bb`, or `None` for no chord.
    #[getter]
    fn root(&self) -> Option<&str> {
        self.inner.root_name()
    }

    /// The bass as a degree above the root, `1` when none was written.
    #[getter]
    fn bass(&self) -> Option<&str> {
        self.inner.bass_degree()
    }

    #[getter]
    fn shorthand(&self) -> Option<&str> {
        self.inner.shorthand()
    }

    /// The degrees written in parentheses, or `None` when there were none.
    #[getter]
    fn degrees(&self) -> Option<Vec<String>> {
        self.inner.degrees().map(<[String]>::to_vec)
    }

    #[getter]
    fn unwrapped(&self) -> Option<Vec<String>> {
        self.inner.unwrap_shorthand().map(<[String]>::to_vec)
    }

    /// Every degree the chord sounds, in order.
    #[getter]
    fn sounding_degrees(&self) -> Vec<String> {
        self.inner.sounding_degrees().to_vec()
    }

    fn bass_is_root(&self) -> bool {
        self.inner.bass_is_root()
    }

    fn contains_shorthand(&self) -> bool {
        self.inner.contains_shorthand()
    }

    fn prettify(&self) -> String {
        self.inner.prettify()
    }

    /// Makes `chord` the chord this label sounds, keeping its duration.
    fn fill(&self, py: Python<'_>, chord: &Bound<'_, Chord>) -> PyResult<()> {
        let mut built = self.inner.chord().clone();
        let mut target = chord.borrow_mut();
        if let Some(duration) = target.inner.duration().cloned() {
            built.set_duration(duration);
        }
        target.replace_value(py, built)
    }

    /// A label never changes, so a copy of one is the label itself.
    fn __copy__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    #[pyo3(signature = (_memo = None))]
    fn __deepcopy__(slf: Py<Self>, _memo: Option<&Bound<'_, PyAny>>) -> Py<Self> {
        slf
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .cast::<Self>()
            .is_ok_and(|other| self.inner == other.get().inner)
    }

    /// The music21 interval a Harte degree stands for, `b3` as `m3`.
    #[staticmethod]
    fn convert_interval(degree: &str) -> PyResult<String> {
        harte::convert_interval(degree).map_err(|error| match error {
            harte::Error::Harte(message) => PyValueError::new_err(message),
            other => harte_error(other),
        })
    }

    /// Where a degree sorts among the others.
    #[staticmethod]
    fn degree_sort_key(degree: &str) -> f64 {
        harte::degree_sort_key(degree)
    }

    /// Each shorthand and the degrees it stands for, in harte-library's order.
    #[classattr]
    fn SHORTHAND_DEGREES() -> Vec<(&'static str, Vec<&'static str>)> {
        harte::SHORTHAND_DEGREES
            .iter()
            .map(|(name, degrees)| (*name, degrees.to_vec()))
            .collect()
    }

    /// The degrees `prettify` folds into each shorthand, in the order tried.
    #[classattr]
    fn DEGREE_SHORTHANDS() -> Vec<(Vec<&'static str>, &'static str)> {
        harte::DEGREE_SHORTHANDS
            .iter()
            .map(|(degrees, name)| (degrees.to_vec(), *name))
            .collect()
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<HarteLabel>()
}
