//! music21's `common` package, built in Rust over the crate's
//! `common` module, and the way any module of music21's built whole here
//! reaches `provide_music21`.
//!
//! A module here is a Python module object with music21's names in it,
//! listed by its dotted name under `__music21_rust_modules__` on the
//! extension. `provide_music21` answers each from that module's names, its
//! private ones included, and keeps its `__all__`. Nothing is installed
//! into a real music21.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

mod formats;
mod misc;
mod numbertools;
mod stringtools;

pub(crate) use numbertools::offset_ql;

/// The attribute of the extension listing the modules built whole here.
const PROVIDED: &str = "__music21_rust_modules__";

/// Lists a module built here under music21's dotted name, for
/// `provide_music21` to answer. Any part of the wheel can add one.
pub(crate) fn provide(
    extension: &Bound<'_, PyModule>,
    name: &str,
    module: &Bound<'_, PyModule>,
) -> PyResult<()> {
    let listed = match extension.getattr(PROVIDED) {
        Ok(listed) => listed.cast_into::<PyList>()?,
        Err(_) => {
            let listed = PyList::empty(extension.py());
            extension.add(PROVIDED, &listed)?;
            listed
        }
    };
    listed.append(PyTuple::new(
        extension.py(),
        [
            name.into_pyobject(extension.py())?.into_any(),
            module.clone().into_any(),
        ],
    )?)
}

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

/// Builds `music21.common` and the modules of it ported so far, and lists
/// them all for `provide_music21`.
pub(crate) fn register(extension: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = extension.py();
    let modules = [
        ("formats", formats::module(py)?),
        ("misc", misc::module(py)?),
        ("numberTools", numbertools::module(py)?),
        ("stringTools", stringtools::module(py)?),
    ];
    let package = PyModule::new(py, "music21.common")?;
    let mut all = Vec::new();
    for name in SUBMODULES {
        let Some((_, module)) = modules.iter().find(|(own, _)| *own == name) else {
            continue;
        };
        all.push(name);
        // `from music21.common.<name> import *`, as music21's `__init__`
        // does: each module's `__all__`.
        let exported = module.getattr("__all__")?;
        for exported in exported.try_iter()? {
            let exported = exported?;
            let exported: String = exported.extract()?;
            package.add(exported.as_str(), module.getattr(exported.as_str())?)?;
        }
    }
    package.add("DEBUG_OFF", 0)?;
    package.add("DEBUG_USER", 1)?;
    package.add("DEBUG_DEVEL", 63)?;
    package.add("DEBUG_ALL", 255)?;
    package.add("__all__", all)?;
    provide(extension, "music21.common", &package)?;
    for (name, module) in &modules {
        provide(extension, &format!("music21.common.{name}"), module)?;
    }
    Ok(())
}

/// A crate error as the `ValueError` music21 raises for it, its message
/// without the crate's prefix.
pub(crate) fn value_error(error: &music21::Error) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(crate::pitch::message(error))
}
