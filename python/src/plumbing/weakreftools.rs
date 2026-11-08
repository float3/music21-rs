//! music21's `common.weakrefTools`: weak references where an object can
//! have one, and the object itself where it cannot.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyWeakrefReference;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.weakrefTools";

/// A weak reference to `referent`, or `referent` itself where it cannot
/// have one: `None`, a number, text, or a weak reference already.
#[pyfunction]
pub(crate) fn wrapWeakref(referent: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    match PyWeakrefReference::new(referent) {
        Ok(reference) => Ok(reference.into_any().unbind()),
        Err(error) if error.is_instance_of::<PyTypeError>(referent.py()) => {
            Ok(referent.clone().unbind())
        }
        Err(error) => Err(error),
    }
}

/// What a weak reference refers to, or anything else as it is.
#[pyfunction]
pub(crate) fn unwrapWeakref(referent: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    if referent.is_instance_of::<PyWeakrefReference>() {
        return Ok(referent.call0()?.unbind());
    }
    Ok(referent.clone().unbind())
}

/// music21's `common.weakrefTools`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_function(wrap_pyfunction!(wrapWeakref, &m)?)?;
    m.add_function(wrap_pyfunction!(unwrapWeakref, &m)?)?;
    m.add("__all__", ["wrapWeakref", "unwrapWeakref"])?;
    Ok(m)
}
