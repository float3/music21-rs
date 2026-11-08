//! music21's `common` package, built in Rust over the crate's
//! `common` module: the package and each of its thirteen modules, the
//! plumbing ones (`classTools`, `enums`...) built in [`crate::plumbing`].
//!
//! Each is a Python module object with music21's names in it, its private
//! ones too, and its `__all__`. `provide_music21` answers each, as it does
//! every module of music21's built whole here; nothing is installed into a
//! real music21.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;

mod formats;
mod misc;
mod numbertools;
mod stringtools;

pub(crate) use numbertools::offset_ql;

/// The package's dotted name.
const PACKAGE: &str = "music21.common";

/// music21's own order for the modules `common` star-imports, by the
/// name each has in the package.
const SUBMODULES: [&str; 13] = [
    "classTools",
    "decorators",
    "enums",
    "fileTools",
    "formats",
    "misc",
    "numberTools",
    "objects",
    "pathTools",
    "parallel",
    "stringTools",
    "types",
    "weakrefTools",
];

/// The package and its modules, by music21's dotted names, built once: a
/// class built twice would be two classes, and music21 compares them by
/// identity.
static MODULES: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

/// The dotted names of the package and its modules.
pub(crate) fn names() -> impl Iterator<Item = String> {
    std::iter::once(PACKAGE.to_owned())
        .chain(SUBMODULES.iter().map(|name| format!("{PACKAGE}.{name}")))
}

/// The package or one of its modules, by its dotted name.
pub(crate) fn module<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    let built = MODULES.get_or_try_init(py, || build(py).map(Bound::unbind))?;
    match built.bind(py).get_item(name)? {
        Some(module) => Ok(module.cast_into::<PyModule>()?),
        None => Err(pyo3::exceptions::PyKeyError::new_err(name.to_owned())),
    }
}

fn build(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let all = crate::plumbing::modules(py)?;
    for (name, module) in [
        ("formats", formats::module(py)?),
        ("misc", misc::module(py)?),
        ("numberTools", numbertools::module(py)?),
        ("stringTools", stringtools::module(py)?),
    ] {
        all.set_item(format!("{PACKAGE}.{name}"), module)?;
    }
    let package = PyModule::new(py, PACKAGE)?;
    for name in SUBMODULES {
        let module = module_in(&all, name)?;
        // `from music21.common.<name> import *`, as music21's `__init__`
        // does: the module's `__all__`, or what it has that is not
        // private.
        for exported in star_names(&module)? {
            package.add(exported.as_str(), module.getattr(exported.as_str())?)?;
        }
    }
    package.add("DEBUG_OFF", music21::common::DEBUG_OFF)?;
    package.add("DEBUG_USER", music21::common::DEBUG_USER)?;
    package.add("DEBUG_DEVEL", music21::common::DEBUG_DEVEL)?;
    package.add("DEBUG_ALL", music21::common::DEBUG_ALL)?;
    package.add("__all__", SUBMODULES.to_vec())?;
    all.set_item(PACKAGE, package)?;
    Ok(all)
}

fn module_in<'py>(all: &Bound<'py, PyDict>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    let dotted = format!("{PACKAGE}.{name}");
    match all.get_item(&dotted)? {
        Some(module) => Ok(module.cast_into::<PyModule>()?),
        None => Err(pyo3::exceptions::PyKeyError::new_err(dotted)),
    }
}

/// The names `from module import *` takes.
fn star_names(module: &Bound<'_, PyModule>) -> PyResult<Vec<String>> {
    if let Ok(all) = module.getattr("__all__") {
        return all.extract();
    }
    let mut names = Vec::new();
    for key in module.dict().keys() {
        let key: String = key.extract()?;
        if !key.starts_with('_') {
            names.push(key);
        }
    }
    Ok(names)
}

/// A crate error as the `ValueError` music21 raises for it, its message
/// without the crate's prefix.
pub(crate) fn value_error(error: &music21::Error) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(crate::pitch::message(error))
}
