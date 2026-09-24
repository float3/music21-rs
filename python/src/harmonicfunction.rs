//! music21's `analysis.harmonicFunction`, over the crate's.
//!
//! `HarmonicFunction` is music21's string enum, built here from the crate's
//! eighteen labels; the two functions look figures and labels up in the
//! crate's tables.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;

use music21_rs_crate::analysis::harmonic_function::{FunctionDetail, HarmonicFunction};

use crate::key::Key;
use crate::roman::RomanNumeral;

/// The module music21 keeps these in.
const MODULE: &str = "music21.analysis.harmonicFunction";

/// The names this facade replaces in `music21.analysis.harmonicFunction`.
pub const NAMES: &[&str] = &["HarmonicFunction", "functionToRoman", "romanToFunction"];

/// music21's `common.enums.StrEnum`, whose repr leaves the value out, built
/// on the standard library's so the wheel needs no music21 to make it.
const ENUM_BUILDER: &str = r#"
import enum

def build(members, module):
    class HarmonicFunction(enum.StrEnum):
        def __repr__(self):
            return f'<{self.__class__.__name__}.{self.name}>'

    built = HarmonicFunction('HarmonicFunction', members, module=module)
    built.__qualname__ = 'HarmonicFunction'
    return built
"#;

/// The enum class, built once.
static ENUM: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// music21's `HarmonicFunction`: the installed class where music21 has one,
/// so the members handed back are the ones music21 compares against.
fn enum_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    if let Some(installed) = crate::installed_class(py, MODULE, "HarmonicFunction") {
        return Ok(installed);
    }
    let built = ENUM.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        let builder = PyModule::from_code(
            py,
            &std::ffi::CString::new(ENUM_BUILDER)?,
            c"music21_rs_harmonic_function.py",
            c"music21_rs_harmonic_function",
        )?;
        let members = PyTuple::new(
            py,
            HarmonicFunction::ALL
                .into_iter()
                .map(|function| (function.music21_name(), function.symbol())),
        )?;
        Ok(builder.getattr("build")?.call1((members, MODULE))?.unbind())
    })?;
    Ok(built.bind(py).clone())
}

/// The enum member for a crate function.
fn member<'py>(py: Python<'py>, function: HarmonicFunction) -> PyResult<Bound<'py, PyAny>> {
    enum_class(py)?.getattr(function.music21_name())
}

/// The mode whose table a key or scale is read with: a minor key's own,
/// anything else major's.
fn mode_of(key_or_scale: &Bound<'_, PyAny>) -> String {
    key_or_scale
        .extract::<PyRef<Key>>()
        .map_or_else(|_| "major".to_string(), |key| key.inner.mode().to_string())
}

#[pyfunction]
#[pyo3(signature = (thisHarmonicFunction, keyOrScale = None))]
fn functionToRoman<'py>(
    py: Python<'py>,
    thisHarmonicFunction: &Bound<'py, PyAny>,
    keyOrScale: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    // A label none of the eighteen is no function, as music21's lookup
    // finds none.
    let Some(function) = thisHarmonicFunction
        .extract::<String>()
        .ok()
        .and_then(|label| HarmonicFunction::from_symbol(&label).ok())
    else {
        return Ok(None);
    };

    // A key given by name is built as music21 builds it; C when none is.
    let key_class = crate::installed_class(py, "music21.key", "Key")
        .unwrap_or_else(|| py.get_type::<Key>().into_any());
    let key_or_scale = match keyOrScale.filter(|value| !value.is_none()) {
        Some(value) if value.extract::<String>().is_ok() => key_class.call1((value,))?,
        Some(value) => value.clone(),
        None => key_class.call1(("C",))?,
    };

    let figure = function.figure(&mode_of(&key_or_scale));
    let numeral_class = crate::installed_class(py, "music21.roman", "RomanNumeral")
        .unwrap_or_else(|| py.get_type::<RomanNumeral>().into_any());
    numeral_class.call1((figure, key_or_scale)).map(Some)
}

#[pyfunction]
#[pyo3(signature = (rn, onlyHauptHarmonicFunction = false))]
fn romanToFunction<'py>(
    py: Python<'py>,
    rn: PyRef<'py, RomanNumeral>,
    onlyHauptHarmonicFunction: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let detail = if onlyHauptHarmonicFunction {
        FunctionDetail::MainOnly
    } else {
        FunctionDetail::Full
    };
    HarmonicFunction::from_roman(&rn.inner, detail)
        .map(|function| member(py, function))
        .transpose()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("HarmonicFunction", enum_class(m.py())?)?;
    m.add_function(wrap_pyfunction!(functionToRoman, m)?)?;
    m.add_function(wrap_pyfunction!(romanToFunction, m)?)?;
    Ok(())
}
