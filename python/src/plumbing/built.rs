//! Python classes built while the module loads, whose methods are Rust.
//!
//! Some of music21's plumbing has to be a real Python class: `RelativeCounter`
//! is a `collections.Counter`, the enums are `enum.Enum`s, and
//! `SlottedObjectMixin` is mixed into classes that have other bases and
//! slots of their own, which a class with a Rust layout cannot be. Those are
//! built here with `types.new_class`, so the metaclass and its namespace are
//! Python's own, and each method in them is a `#[pyfunction]` wrapped in
//! [`Method`], which binds to an instance the way a function in a class body
//! does.

use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyTuple};

/// A Rust function in a class body: called through the class it is the
/// function itself, through an instance it is bound to that instance, as a
/// Python function is.
///
/// Its name, qualified name and docs are kept in its own `__dict__`, so that
/// music21's `deprecated` reads `defaultlist.__init__` off it as it reads
/// one off a function.
#[pyclass(frozen, dict, name = "function", module = "music21_rs")]
pub(crate) struct Method {
    function: Py<PyAny>,
}

#[pymethods]
impl Method {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        Ok(self.function.bind(py).call(args, kwargs)?.unbind())
    }

    fn __get__(
        slf: &Bound<'_, Self>,
        instance: Option<&Bound<'_, PyAny>>,
        owner: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let _ = owner;
        let Some(instance) = instance.filter(|instance| !instance.is_none()) else {
            return Ok(slf.clone().into_any().unbind());
        };
        bound_to(slf.as_any(), instance)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let name = slf.getattr("__qualname__")?;
        Ok(format!("<function {name}>"))
    }
}

/// A callable bound to an instance, as `types.MethodType` binds one.
pub(crate) fn bound_to(
    callable: &Bound<'_, PyAny>,
    instance: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let py = callable.py();
    Ok(py
        .import("types")?
        .getattr("MethodType")?
        .call1((callable, instance))?
        .unbind())
}

/// A Rust function as a method of the class `class` of `module`, named
/// `name` there.
pub(crate) fn method<'py>(
    function: Bound<'py, PyAny>,
    module: &str,
    class: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = function.py();
    let doc = function.getattr("__doc__")?;
    let made = Bound::new(
        py,
        Method {
            function: function.unbind(),
        },
    )?
    .into_any();
    made.setattr("__name__", name)?;
    made.setattr("__qualname__", format!("{class}.{name}"))?;
    made.setattr("__module__", module)?;
    made.setattr("__doc__", doc)?;
    Ok(made)
}

/// The same as a `staticmethod`, which is what `__new__` has to be.
pub(crate) fn static_method<'py>(
    function: Bound<'py, PyAny>,
    module: &str,
    class: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = function.py();
    let made = method(function, module, class, name)?;
    py.import("builtins")?
        .getattr("staticmethod")?
        .call1((made,))
}

/// The same as a `classmethod`.
pub(crate) fn class_method<'py>(
    function: Bound<'py, PyAny>,
    module: &str,
    class: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = function.py();
    let made = method(function, module, class, name)?;
    py.import("builtins")?
        .getattr("classmethod")?
        .call1((made,))
}

/// A class built as a `class` statement builds it: its metaclass found from
/// its bases and its namespace prepared by that metaclass, then filled with
/// `members` in order. An enum's members are its plain values; a [`Method`]
/// is a descriptor, so an enum keeps it as a method.
pub(crate) fn new_class<'py>(
    py: Python<'py>,
    module: &str,
    name: &str,
    doc: &str,
    bases: Vec<Bound<'py, PyAny>>,
    members: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyAny>> {
    new_class_of(py, module, name, doc, bases, members, None)
}

/// [`new_class`] with a metaclass of its own, as `metaclass=` names one.
pub(crate) fn new_class_of<'py>(
    py: Python<'py>,
    module: &str,
    name: &str,
    doc: &str,
    bases: Vec<Bound<'py, PyAny>>,
    members: Vec<(&str, Bound<'py, PyAny>)>,
    metaclass: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut filled: Vec<(String, Py<PyAny>)> = vec![
        (
            "__module__".to_string(),
            module.into_pyobject(py)?.into_any().unbind(),
        ),
        (
            "__qualname__".to_string(),
            name.into_pyobject(py)?.into_any().unbind(),
        ),
        (
            "__doc__".to_string(),
            doc.into_pyobject(py)?.into_any().unbind(),
        ),
    ];
    for (key, value) in members {
        filled.push((key.to_string(), value.unbind()));
    }
    let body = PyCFunction::new_closure(
        py,
        Some(c"exec_body"),
        None,
        move |args: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
            let py = args.py();
            let namespace = args.get_item(0)?;
            for (key, value) in &filled {
                namespace.set_item(key, value.bind(py))?;
            }
            Ok(())
        },
    )?;
    let keywords = PyDict::new(py);
    if let Some(metaclass) = metaclass {
        keywords.set_item("metaclass", metaclass)?;
    }
    py.import("types")?.getattr("new_class")?.call1((
        name,
        PyTuple::new(py, bases)?,
        keywords,
        body,
    ))
}

/// The class's name as music21 writes it in messages: `type(x).__name__`.
pub(crate) fn class_name(object: &Bound<'_, PyAny>) -> PyResult<String> {
    object.getattr("__class__")?.getattr("__name__")?.extract()
}

/// `super(class, object)`, for a method that calls the next one along.
pub(crate) fn super_of<'py>(
    class: &Bound<'py, PyAny>,
    object: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    object
        .py()
        .import("builtins")?
        .getattr("super")?
        .call1((class, object))
}

/// Runs `body` inside a context manager's `with`, handing it what
/// `__enter__` gave: `__exit__` is told of an error and may swallow it, as
/// a `with` statement does.
pub(crate) fn with_context<'py, T>(
    manager: &Bound<'py, PyAny>,
    body: impl FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
) -> PyResult<Option<T>> {
    let py = manager.py();
    let entered = manager.call_method0("__enter__")?;
    match body(&entered) {
        Ok(value) => {
            manager.call_method1("__exit__", (py.None(), py.None(), py.None()))?;
            Ok(Some(value))
        }
        Err(error) => {
            let value = error.value(py);
            let swallowed = manager
                .call_method1("__exit__", (value.get_type(), value, error.traceback(py)))?
                .is_truthy()?;
            if swallowed {
                return Ok(None);
            }
            Err(error)
        }
    }
}
