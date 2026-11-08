//! music21's `common` plumbing modules, each built as a module of its own
//! under music21's dotted name: `classTools`, `decorators`, `enums`,
//! `fileTools`, `objects`, `parallel`, `pathTools`, `types` and
//! `weakrefTools`.
//!
//! They are handed out together in `__music21_submodules__`, which
//! `provide_music21` answers `music21.common.<name>` from.

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
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;

/// The built modules, by music21's dotted names, built once: a class built
/// twice would be two classes, and music21 compares them by identity.
static MODULES: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

/// Every module here, by its dotted name.
pub(crate) fn modules(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let built = MODULES.get_or_try_init(py, || -> PyResult<Py<PyDict>> {
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
        Ok(all.unbind())
    })?;
    Ok(built.bind(py).clone())
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
        Err(_) => modules(py)?.as_any().get_item(enums::MODULE)?,
    };
    module.getattr(class)?.getattr(member)
}
