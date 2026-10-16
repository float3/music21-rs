//! music21's `editorial.Editorial`: where an analysis leaves what it
//! worked out about a note.
//!
//! A dictionary with attribute access, as music21's is -- `n.editorial
//! .melodicAccent` reads what `thomassenMelodicAccent` wrote, and
//! `n.editorial.harmonicInterval` what `attachIntervalsBetweenStreams`
//! did. Three names answer `None` before anything writes them and two
//! answer an empty list, which is music21's own `predefinedNones` and
//! `predefinedLists`; anything else raises until written, as music21's
//! does.
//!
//! The crate has none of this: nothing it reads or writes depends on it,
//! and what an analysis found is the answer it hands back. It is here
//! because music21's analyses put their answers on the note and a caller
//! reads them there.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// The names this module hands out.
pub const NAMES: &[&str] = &["Editorial"];

/// The names music21 answers `None` for before anything has written them.
const PREDEFINED_NONES: [&str; 3] = ["ficta", "harmonicInterval", "melodicInterval"];

/// The names music21 answers an empty list for.
const PREDEFINED_LISTS: [&str; 2] = ["footnotes", "comments"];

/// music21's `editorial.Editorial`.
#[pyclass(
    name = "Editorial",
    module = "music21.editorial",
    subclass,
    skip_from_py_object,
    mapping
)]
pub struct Editorial {
    held: Py<PyDict>,
}

impl Editorial {
    /// An empty one, as every note starts with.
    pub(crate) fn new(py: Python<'_>) -> Self {
        Self {
            held: PyDict::new(py).unbind(),
        }
    }
}

#[pymethods]
impl Editorial {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn make(py: Python<'_>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let made = Self::new(py);
        if let Some(keywords) = keywords {
            for (key, value) in keywords {
                made.held.bind(py).set_item(key, value)?;
            }
        }
        Ok(made)
    }

    /// Reads a name: what was written under it, an empty list or `None`
    /// for the names music21 answers those for, and otherwise an error.
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let held = self.held.bind(py);
        if let Some(value) = held.get_item(name)? {
            return Ok(value);
        }
        if PREDEFINED_LISTS.contains(&name) {
            let made = PyList::empty(py);
            held.set_item(name, &made)?;
            return Ok(made.into_any());
        }
        if PREDEFINED_NONES.contains(&name) {
            held.set_item(name, py.None())?;
            return Ok(py.None().into_bound(py));
        }
        Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "Editorial does not have an attribute {name}"
        )))
    }

    /// Writes a name, as music21's `__setattr__` does.
    fn __setattr__(&self, py: Python<'_>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.held.bind(py).set_item(name, value)
    }

    fn __delattr__(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        let held = self.held.bind(py);
        if held.contains(name)? {
            return held.del_item(name);
        }
        Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "No such attribute: {name}"
        )))
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.held.bind(py).get_item(key)?.ok_or_else(|| {
            pyo3::exceptions::PyKeyError::new_err(
                key.repr()
                    .map(|shown| shown.to_string())
                    .unwrap_or_default(),
            )
        })
    }

    fn __setitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.held.bind(py).set_item(key, value)
    }

    fn __contains__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.held.bind(py).contains(key)
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.held.bind(py).len()
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.held.bind(py).as_any().try_iter().map(Bound::into_any)
    }

    /// The names written so far: a dictionary's own `keys`.
    fn keys<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.held.bind(py).keys().into_any()
    }

    fn get<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.held.bind(py).get_item(key)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let shown = self.held.bind(py).repr()?;
        Ok(format!("<music21.editorial.Editorial {shown}>"))
    }
}

/// Adds the editorial class to a module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Editorial>()?;
    Ok(())
}
