//! music21's `common` plumbing modules, each built as a module of its own
//! under music21's dotted name: `classTools`, `decorators`, `enums`,
//! `fileTools`, `objects`, `parallel`, `pathTools`, `types` and
//! `weakrefTools`.
//!
//! [`crate::common`] builds them, once, with the rest of `common`.

pub(crate) mod built;
pub(crate) mod classtools;
pub(crate) mod decorators;
pub(crate) mod enums;
pub(crate) mod filetools;
pub(crate) mod objects;
pub(crate) mod parallel;
pub(crate) mod pathtools;
pub(crate) mod types;
pub(crate) mod weakreftools;

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Every module here, by its dotted name, newly built: only
/// [`crate::common`] builds them, once.
pub(crate) fn modules(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let all = PyDict::new(py);
    let decorators = decorators::module(py)?;
    let enums = enums::module(py)?;
    all.set_item(classtools::MODULE, classtools::module(py)?)?;
    all.set_item(objects::MODULE, objects::module(py, &decorators)?)?;
    all.set_item(types::MODULE, types::module(py, &enums)?)?;
    all.set_item(decorators::MODULE, decorators)?;
    all.set_item(enums::MODULE, enums)?;
    all.set_item(weakreftools::MODULE, weakreftools::module(py)?)?;
    all.set_item(parallel::MODULE, parallel::module(py)?)?;
    all.set_item(pathtools::MODULE, pathtools::module(py)?)?;
    all.set_item(filetools::MODULE, filetools::module(py)?)?;
    Ok(all)
}

/// The built `common.StrEnum`, never music21's: the wheel's classes are
/// built on it as the wheel is imported, and that must not import music21.
pub(crate) fn str_enum(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    crate::common::module(py, enums::MODULE)?.getattr("StrEnum")
}

/// A member of one of music21's `common.enums`: music21's own where it is
/// installed, so it is the member music21 compares against, and the built
/// one where `provide_music21` answers for it.
pub(crate) fn enum_member<'py>(
    py: Python<'py>,
    class: &str,
    member: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let module = match crate::real_music21(py, enums::MODULE) {
        Ok(module) => module.into_any(),
        Err(_) => crate::common::module(py, enums::MODULE)?.into_any(),
    };
    module.getattr(class)?.getattr(member)
}
