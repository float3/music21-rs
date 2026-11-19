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
//!
//! **This is the lesser class, and it only answers where music21 is not
//! there to.** music21's own `Editorial` is a dict subclass with the whole
//! dict API, and music21 keeps it where its copying carries it along; a
//! note or a chord of this package's therefore answers with music21's
//! wherever music21 can be imported (`crate::music21_editorial`), and with
//! one of these only for a caller who has no music21 at all. The same
//! bargain `style.Style` strikes, and for the same reason: being quietly
//! worse would be worse than being absent.

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
    /// What it holds may point back at the note it is on (`n.editorial.x
    /// = n`), a cycle through Rust that Python's collector frees only if it
    /// is shown the dictionary.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.held)
    }

    fn __clear__(&mut self) {
        Python::attach(|py| self.held.bind(py).clear());
    }

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

    /// A copy holding copies of what was written, as music21's deep copy
    /// of a note carries its editorial along.
    fn __deepcopy__(&self, py: Python<'_>, memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        let copied = py
            .import("copy")?
            .getattr("deepcopy")?
            .call1((self.held.bind(py), memo))?;
        Ok(Self {
            held: copied.cast_into::<PyDict>()?.unbind(),
        })
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
/// The longest a comment's text is shown in its repr before it is cut.
const SHOWN_TEXT: usize = 20;

/// music21's `editorial.Comment`: a comment or footnote on a note, with
/// how sure it is and where it comes from. Its members are plain
/// attributes, as music21's are.
#[pyclass(
    name = "Comment",
    module = "music21.editorial",
    extends = crate::prebase::ProtoM21Object,
    subclass,
    dict
)]
pub struct Comment;

#[pymethods]
impl Comment {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, pyo3::types::PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        crate::base::built(Comment)
    }

    /// music21's `Comment(text=None)`.
    #[pyo3(signature = (text = None))]
    fn __init__(slf: &Bound<'_, Self>, text: Option<Py<PyAny>>) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("text", text.unwrap_or_else(|| py.None()))?;
        slf.setattr("isFootnote", false)?;
        slf.setattr("isReference", false)?;
        slf.setattr("levelInformation", py.None())
    }

    /// The text, cut short past twenty characters.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let text = slf.getattr("text")?;
        if text.is_none() {
            return Ok(String::new());
        }
        let text: String = text.str()?.extract()?;
        let shown = match text.chars().count() < SHOWN_TEXT {
            true => text,
            false => format!(
                "{}...",
                text.chars().take(SHOWN_TEXT - 3).collect::<String>()
            ),
        };
        Ok(pyo3::types::PyString::new(slf.py(), &shown)
            .repr()?
            .to_string())
    }

    /// How it is drawn, made the first time it is asked for.
    #[getter]
    fn style<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let held = slf.getattr("__dict__")?;
        if let Some(style) = held
            .get_item("_style")
            .ok()
            .filter(|style| !style.is_none())
        {
            return Ok(style);
        }
        let style = crate::subconverters::music21_name(py, "music21.style", "Style")?.call0()?;
        held.set_item("_style", &style)?;
        Ok(style)
    }

    /// Whether a style has been made for it.
    #[getter]
    fn hasStyleInformation(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Ok(slf.getattr("__dict__")?.get_item("_style").is_ok())
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Editorial>()?;
    m.add_class::<Comment>()?;
    Ok(())
}
