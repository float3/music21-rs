//! music21's `prebase.ProtoM21Object`: what nearly everything music21
//! hands out is built on, and what answers `classes`, `classSet` and the
//! `<module.Class ...>` form of `repr`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFrozenSet, PyTuple, PyType};

/// The names this module hands out.
pub const NAMES: &[&str] = &["ProtoM21Object"];

/// `classes` for each class, worked out once: music21's
/// `_classTupleCacheDict`.
static CLASS_TUPLES: pyo3::sync::PyOnceLock<Py<PyDict>> = pyo3::sync::PyOnceLock::new();

/// `classSet` for each class: music21's `_classSetCacheDict`.
static CLASS_SETS: pyo3::sync::PyOnceLock<Py<PyDict>> = pyo3::sync::PyOnceLock::new();

fn cache<'py>(py: Python<'py>, held: &pyo3::sync::PyOnceLock<Py<PyDict>>) -> Bound<'py, PyDict> {
    held.get_or_init(py, || PyDict::new(py).unbind())
        .bind(py)
        .clone()
}

/// music21's `ProtoM21Object`: no state of its own, only how an object
/// names its classes and shows itself.
#[pyclass(name = "ProtoM21Object", module = "music21.prebase", subclass)]
#[derive(Default)]
pub struct ProtoM21Object;

#[pymethods]
impl ProtoM21Object {
    /// A bare one. Whatever is given is for a subclass.
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(_arguments: &Bound<'_, PyTuple>, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    #[classattr]
    fn _DOC_ORDER() -> Vec<&'static str> {
        vec!["classes", "classSet"]
    }

    #[classattr]
    fn _DOC_ATTR(py: Python<'_>) -> Py<PyDict> {
        PyDict::new(py).unbind()
    }

    #[classattr]
    fn _classTupleCacheDict(py: Python<'_>) -> Py<PyDict> {
        cache(py, &CLASS_TUPLES).unbind()
    }

    #[classattr]
    fn _classSetCacheDict(py: Python<'_>) -> Py<PyDict> {
        cache(py, &CLASS_SETS).unbind()
    }

    /// The names of this object's class and of every class it is a kind
    /// of, in method resolution order.
    #[getter]
    fn classes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        class_names(&slf.get_type())
    }

    /// Every class this object is a kind of, three ways at once: the
    /// classes themselves, their names, and their names with the module
    /// before them, with and without `music21.`.
    #[getter]
    fn classSet<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        class_set(&slf.get_type())
    }

    /// `<module.Class ...>`, with what `_reprInternal` says inside.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let class = slf.get_type();
        let module: String = slf.getattr("__module__")?.extract()?;
        let mut head = String::from("<");
        if module != "__main__" {
            head.push_str(&module);
            head.push('.');
        }
        head.push_str(&class.qualname()?.to_string());
        if head.contains(".base.") && !head.contains("music21.base") {
            head = head.replace(".base", "");
        }
        let inside: String = slf.call_method0("_reprInternal")?.str()?.to_string();
        if !inside.is_empty() && !inside.starts_with(':') {
            head.push(' ');
        }
        head.push_str(inside.trim());
        head.push('>');
        Ok(head)
    }

    /// What `repr` shows inside the brackets: here, where the object is.
    fn _reprInternal(slf: &Bound<'_, Self>) -> String {
        format!("object at {:#x}", slf.as_ptr() as usize)
    }
}

/// The names along a class's method resolution order, worked out once a
/// class.
pub(crate) fn class_names<'py>(class: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = class.py();
    let held = cache(py, &CLASS_TUPLES);
    if let Some(found) = held.get_item(class)? {
        return Ok(found);
    }
    let mut names = Vec::new();
    for ancestor in class.mro().iter() {
        names.push(ancestor.getattr("__name__")?);
    }
    let made = PyTuple::new(py, names)?.into_any();
    held.set_item(class, &made)?;
    Ok(made)
}

/// The class set of a class, worked out once a class.
pub(crate) fn class_set<'py>(class: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = class.py();
    let held = cache(py, &CLASS_SETS);
    if let Some(found) = held.get_item(class)? {
        return Ok(found);
    }
    let mut members: Vec<Bound<'py, PyAny>> = Vec::new();
    for ancestor in class.mro().iter() {
        let name: String = ancestor.getattr("__name__")?.extract()?;
        let module: String = ancestor.getattr("__module__")?.extract()?;
        let qualified = format!("{module}.{name}");
        let partial = qualified.replace("music21.", "");
        members.push(name.into_pyobject(py)?.into_any());
        members.push(qualified.into_pyobject(py)?.into_any());
        members.push(partial.into_pyobject(py)?.into_any());
        members.push(ancestor);
    }
    let made = PyFrozenSet::new(py, &members)?.into_any();
    held.set_item(class, &made)?;
    Ok(made)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ProtoM21Object>()
}
