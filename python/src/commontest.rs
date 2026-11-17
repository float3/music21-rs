//! music21's `test.commonTest`, as far as music21's own tests reach for
//! it: `testCopyAll`, which each module's `Test` class calls to see that
//! every class it defines copies.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyException, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyFunction, PyModule};

/// The module music21 keeps these in.
const MODULE: &str = "music21.test.commonTest";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// What a name starting or ending with one of these is left out for: a
/// private name, a test, an exception.
const PASSED_OVER: [&str; 3] = ["_", "Test", "Exception"];

/// Whether `testCopyAll` leaves a name out.
fn passed_over(name: &str) -> bool {
    PASSED_OVER
        .iter()
        .any(|part| name.starts_with(part) || name.ends_with(part))
}

/// Makes each class `globals_` holds that the test's own module defines,
/// where it can be made with no arguments, and fails the test where one
/// cannot be copied or deep-copied.
#[pyfunction]
fn testCopyAll(testInstance: &Bound<'_, PyAny>, globals_: &Bound<'_, PyDict>) -> PyResult<()> {
    let py = testInstance.py();
    let own_module = testInstance.get_type().getattr("__module__")?;
    let copy = py.import("copy")?;
    for (part, obj) in globals_.iter() {
        let name: String = part.extract()?;
        if passed_over(&name) || !obj.is_callable() || obj.is_instance_of::<PyFunction>() {
            continue;
        }
        let Ok(module) = obj.getattr("__module__") else {
            continue;
        };
        if !module.eq(&own_module)? {
            continue;
        }
        let instance = match obj.call0() {
            Ok(instance) => instance,
            Err(error) if error.is_instance_of::<PyTypeError>(py) => continue,
            Err(error) => return Err(error),
        };
        for (way, said) in [("copy", "copy"), ("deepcopy", "deepcopy")] {
            if let Err(error) = copy.call_method1(way, (&instance,)) {
                if !error.is_instance_of::<PyException>(py) {
                    return Err(error);
                }
                let message = format!("Could not {said} obj {name}: {}", error.value(py).str()?);
                testInstance.call_method1("fail", (message,))?;
            }
        }
    }
    Ok(())
}

/// music21's `test.commonTest`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_function(wrap_pyfunction!(testCopyAll, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

#[cfg(test)]
mod tests {
    use super::passed_over;

    #[test]
    fn private_names_tests_and_exceptions_are_passed_over() {
        for name in ["_cache", "TestExternal", "MidiException", "cache_"] {
            assert!(passed_over(name), "{name}");
        }
        assert!(!passed_over("MidiTrack"));
    }
}
