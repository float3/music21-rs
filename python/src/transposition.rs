//! music21's `analysis.transposition`, over the crate's.
//!
//! The checker keeps music21's four attributes and fills them as music21
//! does, each method asking the crate afresh from the pitches.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use music21_rs_crate::analysis::transposition::TranspositionChecker as RsChecker;

use crate::Walkable;
use crate::pitch::{Pitch, pitch_from_any};

error_into!(checker_error, PyTypeError);

/// The names this facade replaces in `music21.analysis.transposition`.
pub const NAMES: &[&str] = &["TranspositionChecker"];

/// music21's `TranspositionChecker`.
#[pyclass(
    name = "TranspositionChecker",
    module = "music21.analysis.transposition",
    subclass,
    skip_from_py_object
)]
pub struct TranspositionChecker {
    #[pyo3(get, set)]
    pitches: Py<PyAny>,
    #[pyo3(get, set)]
    allTranspositions: Py<PyAny>,
    #[pyo3(get, set)]
    allNormalOrders: Py<PyAny>,
    #[pyo3(get, set)]
    distinctNormalOrders: Py<PyAny>,
}

impl TranspositionChecker {
    /// The crate's checker of the pitches held now.
    fn checker(&self, py: Python<'_>) -> PyResult<RsChecker> {
        let pitches = self
            .pitches
            .bind(py)
            .walk()?
            .map(|pitch| pitch_from_any(&pitch?))
            .collect::<PyResult<Vec<_>>>()?;
        RsChecker::new(pitches).map_err(checker_error)
    }
}

#[pymethods]
impl TranspositionChecker {
    #[new]
    #[pyo3(signature = (pitches = None))]
    fn new(py: Python<'_>, pitches: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let pitches = match pitches {
            Some(pitches) if pitches.is_truthy()? => pitches,
            _ => {
                return Err(PyTypeError::new_err(
                    "Must have at least one element in list",
                ));
            }
        };
        if pitches.try_iter().is_err() {
            return Err(PyTypeError::new_err("Must be a list or tuple"));
        }

        let empty = || PyList::empty(py).into_any().unbind();
        Ok(Self {
            pitches: pitches.clone().unbind(),
            allTranspositions: empty(),
            allNormalOrders: empty(),
            distinctNormalOrders: empty(),
        })
    }

    fn getTranspositions<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let transpositions = slf
            .borrow()
            .checker(py)?
            .transpositions()
            .map_err(checker_error)?;

        let mut rows = Vec::with_capacity(transpositions.len());
        for pitches in transpositions {
            let row = pitches
                .into_iter()
                .map(|pitch| {
                    let inferred = pitch.spelling_is_inferred();
                    crate::installed_new(py, "music21.pitch", "Pitch", Pitch::wrap(pitch, inferred))
                })
                .collect::<PyResult<Vec<_>>>()?;
            rows.push(PyList::new(py, row)?);
        }
        let rows = PyList::new(py, rows)?;
        slf.borrow_mut().allTranspositions = rows.clone().into_any().unbind();
        Ok(rows)
    }

    fn listNormalOrders<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let orders = slf
            .borrow()
            .checker(py)?
            .normal_orders()
            .map_err(checker_error)?;
        let orders = PyList::new(py, int_lists(orders))?;
        slf.borrow_mut().allNormalOrders = orders.clone().into_any().unbind();
        Ok(orders)
    }

    fn listDistinctNormalOrders<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let orders = slf
            .borrow()
            .checker(py)?
            .distinct_normal_orders()
            .map_err(checker_error)?;
        let orders = PyList::new(py, int_lists(orders))?;
        slf.borrow_mut().distinctNormalOrders = orders.clone().into_any().unbind();
        Ok(orders)
    }

    fn numDistinctTranspositions(slf: &Bound<'_, Self>) -> PyResult<usize> {
        Ok(Self::listDistinctNormalOrders(slf)?.len())
    }

    /// A chord of each distinct normal order, built from its pitch-class
    /// numbers as music21 builds it.
    fn getChordsOfDistinctTranspositions<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let orders = Self::listDistinctNormalOrders(slf)?;
        let chord_class = crate::installed_class(py, "music21.chord", "Chord")
            .unwrap_or_else(|| py.get_type::<crate::chord::Chord>().into_any());
        let chords = orders
            .iter()
            .map(|order| chord_class.call1((order,)))
            .collect::<PyResult<Vec<_>>>()?;
        PyList::new(py, chords)
    }

    fn getPitchesOfDistinctTranspositions<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let pitches = Self::getChordsOfDistinctTranspositions(slf)?
            .iter()
            .map(|chord| {
                chord
                    .getattr("pitches")?
                    .cast_into::<PyTuple>()
                    .map_err(PyErr::from)
            })
            .collect::<PyResult<Vec<_>>>()?;
        PyList::new(py, pitches)
    }
}

/// Normal orders as lists of ints; pyo3 would hand `Vec<u8>` over as
/// `bytes`.
fn int_lists(orders: Vec<Vec<u8>>) -> Vec<Vec<u32>> {
    orders
        .into_iter()
        .map(|order| order.into_iter().map(u32::from).collect())
        .collect()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TranspositionChecker>()?;
    Ok(())
}
