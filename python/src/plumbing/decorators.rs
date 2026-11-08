//! music21's `common.decorators`: `deprecated`, `cacheMethod` and the
//! decorator that lets a decorator be used with or without arguments.
//!
//! Each decorated function comes back as an object of a class here that
//! calls it, carries its name and docs as `functools.wraps` copies them,
//! and binds to an instance as a function in a class body does.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::sync::atomic::{AtomicBool, Ordering};

use pyo3::prelude::*;
use pyo3::types::{PyCFunction, PyDict, PyTuple};

use music21::common::decorators::deprecation_message;

use super::built::bound_to;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.decorators";

/// Copies a wrapped function's name, docs and `__dict__` onto its wrapper,
/// as `functools.wraps` does.
fn wraps(wrapper: &Bound<'_, PyAny>, wrapped: &Bound<'_, PyAny>) -> PyResult<()> {
    wrapper
        .py()
        .import("functools")?
        .call_method1("update_wrapper", (wrapper, wrapped))?;
    Ok(())
}

/// What a decorator built with `optional_arg_decorator` is: called with a
/// function alone it decorates it, called with anything else it is a
/// decorator that will.
#[pyclass(
    frozen,
    dict,
    module = "music21.common.decorators",
    name = "_OptionalArgDecorator"
)]
pub(crate) struct OptionalArgDecorator {
    decorator: Py<PyAny>,
}

#[pymethods]
impl OptionalArgDecorator {
    #[pyo3(signature = (*arguments, **keywords))]
    fn __call__(
        &self,
        py: Python<'_>,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let decorator = self.decorator.bind(py);
        let name = decorator
            .getattr("__name__")?
            .cast_into::<pyo3::types::PyString>()?;
        // A decorator called on something that has an attribute of its own
        // name is taken for a method, and that something for its instance.
        let is_bound_method = match arguments.get_item(0) {
            Ok(first) => first.hasattr(&name)?,
            Err(_) => false,
        };
        let mut rest: Vec<Bound<'_, PyAny>> = arguments.iter().collect();
        let instance = if is_bound_method {
            Some(rest.remove(0))
        } else {
            None
        };
        let no_keywords = match keywords {
            Some(keywords) => keywords.is_empty(),
            None => true,
        };
        if rest.len() == 1 && no_keywords && rest[0].is_callable() {
            let mut called = Vec::new();
            called.extend(instance);
            called.push(rest.remove(0));
            return Ok(decorator.call1(PyTuple::new(py, called)?)?.unbind());
        }
        let decorator = decorator.clone().unbind();
        let instance = instance.map(Bound::unbind);
        let rest: Vec<Py<PyAny>> = rest.into_iter().map(Bound::unbind).collect();
        let keywords = keywords.map(|keywords| keywords.clone().unbind());
        let real = PyCFunction::new_closure(
            py,
            Some(c"real_decorator"),
            None,
            move |args: &Bound<'_, PyTuple>,
                  _: Option<&Bound<'_, PyDict>>|
                  -> PyResult<Py<PyAny>> {
                let py = args.py();
                let to_be_decorated = args.get_item(0)?;
                let mut called: Vec<Bound<'_, PyAny>> = Vec::new();
                called.extend(instance.as_ref().map(|instance| instance.bind(py).clone()));
                called.push(to_be_decorated);
                called.extend(rest.iter().map(|argument| argument.bind(py).clone()));
                let keywords = keywords.as_ref().map(|keywords| keywords.bind(py).clone());
                Ok(decorator
                    .bind(py)
                    .call(PyTuple::new(py, called)?, keywords.as_ref())?
                    .unbind())
            },
        )?;
        Ok(real.into_any().unbind())
    }

    fn __get__(
        slf: &Bound<'_, Self>,
        instance: Option<&Bound<'_, PyAny>>,
        owner: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let _ = owner;
        match instance.filter(|instance| !instance.is_none()) {
            Some(instance) => bound_to(slf.as_any(), instance),
            None => Ok(slf.clone().into_any().unbind()),
        }
    }
}

/// Lets a decorator be used bare, `@decorator`, or with arguments,
/// `@decorator(a, b)`, which then reach it after the decorated function.
#[pyfunction]
pub(crate) fn optional_arg_decorator<'py>(fn_: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let made = Bound::new(
        fn_.py(),
        OptionalArgDecorator {
            decorator: fn_.clone().unbind(),
        },
    )?
    .into_any();
    wraps(&made, fn_)?;
    Ok(made)
}

/// A deprecated function: warns the first time it is called, then calls
/// through.
#[pyclass(
    frozen,
    dict,
    module = "music21.common.decorators",
    name = "_Deprecated"
)]
pub(crate) struct Deprecated {
    method: Py<PyAny>,
    message: String,
    warned: AtomicBool,
}

/// The names IPython asks every object for, which are not calls anybody
/// made: they warn of nothing.
const IPYTHON_PROBES: [&str; 3] = [
    "_ipython_canary_method_should_not_exist_",
    "_repr_mimebundle_",
    "_is_coroutine",
];

#[pymethods]
impl Deprecated {
    #[pyo3(signature = (*arguments, **keywords))]
    fn __call__(
        &self,
        py: Python<'_>,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let probe = match arguments.get_item(1) {
            Ok(second) => PyTuple::new(py, IPYTHON_PROBES)?.contains(second)?,
            Err(_) => false,
        };
        if !probe && !self.warned.load(Ordering::Relaxed) {
            let message = std::ffi::CString::new(self.message.clone())?;
            PyErr::warn(py, &deprecation_warning(py)?, &message, 1)?;
            self.warned.store(true, Ordering::Relaxed);
        }
        Ok(self.method.bind(py).call(arguments, keywords)?.unbind())
    }

    fn __get__(
        slf: &Bound<'_, Self>,
        instance: Option<&Bound<'_, PyAny>>,
        owner: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let _ = owner;
        match instance.filter(|instance| !instance.is_none()) {
            Some(instance) => bound_to(slf.as_any(), instance),
            None => Ok(slf.clone().into_any().unbind()),
        }
    }
}

/// music21's `Music21DeprecationWarning`, from wherever `music21` is
/// answered, and `UserWarning`, which it is one of, where it cannot be.
fn deprecation_warning(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    match py
        .import("music21.exceptions21")
        .and_then(|module| module.getattr("Music21DeprecationWarning"))
    {
        Ok(warning) => Ok(warning),
        Err(_) => Ok(py.get_type::<pyo3::exceptions::PyUserWarning>().into_any()),
    }
}

/// `deprecated` itself, before `optional_arg_decorator` lets it be used
/// bare: marks `method` deprecated and hands back what warns when it is
/// called.
#[pyfunction]
#[pyo3(name = "deprecated", signature = (method, startDate = None, removeDate = None, message = None))]
pub(crate) fn deprecated_function<'py>(
    method: &Bound<'py, PyAny>,
    startDate: Option<&str>,
    removeDate: Option<&str>,
    message: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = method.py();
    let name = if method.hasattr("__qualname__")? {
        method.getattr("__qualname__")?
    } else {
        method.getattr("__name__")?
    };
    method
        .getattr("__dict__")?
        .set_item("_isDeprecated", true)?;
    let message = deprecation_message(&name.str()?.to_cow()?, startDate, removeDate, message);
    let made = Bound::new(
        py,
        Deprecated {
            method: method.clone().unbind(),
            message,
            warned: AtomicBool::new(false),
        },
    )?
    .into_any();
    wraps(&made, method)?;
    Ok(made)
}

/// A zero-argument method whose answer is kept in its instance's `_cache`
/// under its qualified name, so `clearCache` can drop it with the rest.
#[pyclass(
    frozen,
    dict,
    module = "music21.common.decorators",
    name = "_CacheMethod"
)]
pub(crate) struct CacheMethod {
    method: Py<PyAny>,
    key: Py<PyAny>,
}

#[pymethods]
impl CacheMethod {
    #[pyo3(signature = (instance, *arguments, **keywords))]
    fn __call__(
        &self,
        py: Python<'_>,
        instance: &Bound<'_, PyAny>,
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let cache = instance.getattr("_cache")?;
        let key = self.key.bind(py);
        if cache.contains(key)? {
            return Ok(cache.get_item(key)?.unbind());
        }
        let mut called = vec![instance.clone()];
        called.extend(arguments.iter());
        let answer = self
            .method
            .bind(py)
            .call(PyTuple::new(py, called)?, keywords)?;
        cache.set_item(key, answer)?;
        Ok(cache.get_item(key)?.unbind())
    }

    fn __get__(
        slf: &Bound<'_, Self>,
        instance: Option<&Bound<'_, PyAny>>,
        owner: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let _ = owner;
        match instance.filter(|instance| !instance.is_none()) {
            Some(instance) => bound_to(slf.as_any(), instance),
            None => Ok(slf.clone().into_any().unbind()),
        }
    }
}

/// Keeps a zero-argument method's answer in its instance's `_cache`, under
/// the method's qualified name, and answers from there after.
#[pyfunction]
pub(crate) fn cacheMethod<'py>(method: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let key = if method.hasattr("__qualname__")? {
        method.getattr("__qualname__")?
    } else {
        method.getattr("__name__")?
    };
    let made = Bound::new(
        method.py(),
        CacheMethod {
            method: method.clone().unbind(),
            key: key.unbind(),
        },
    )?
    .into_any();
    wraps(&made, method)?;
    Ok(made)
}

/// `deprecated`, usable bare or with its dates and message.
fn deprecated<'py>(module: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyAny>> {
    optional_arg_decorator(wrap_pyfunction!(deprecated_function, module)?.as_any())
}

/// `deprecated(startDate, removeDate, message)` from the built module, for
/// another module's deprecated members.
pub(crate) fn deprecated_with<'py>(
    decorators: &Bound<'py, PyModule>,
    startDate: &str,
    removeDate: &str,
    message: &str,
) -> PyResult<Bound<'py, PyAny>> {
    decorators
        .getattr("deprecated")?
        .call1((startDate, removeDate, message))
}

/// music21's `common.decorators`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_function(wrap_pyfunction!(optional_arg_decorator, &m)?)?;
    m.add("deprecated", deprecated(&m)?)?;
    m.add_function(wrap_pyfunction!(cacheMethod, &m)?)?;
    m.add(
        "__all__",
        ["optional_arg_decorator", "deprecated", "cacheMethod"],
    )?;
    Ok(m)
}
