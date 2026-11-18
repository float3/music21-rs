//! music21's pedal family in `expressions`: `PedalType`, `PedalForm`, and
//! the objects marking a held pedal's bounces and gaps, `PedalBounce`,
//! `PedalGapStart` and `PedalGapEnd`. `PedalMark`, the spanner holding
//! them, is the spanner module's.
//!
//! The crate's `spanner::PedalObjectKind` names the kinds, and its
//! `PedalType` and `PedalForm` the values the enums take.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::PyClass;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21::spanner::{PedalForm, PedalType};

use crate::base::Music21Object;

/// The module music21 keeps these in.
const MODULE: &str = "music21.expressions";

/// music21's `PedalObject`: what marks a change in a held pedal. Its kind
/// is its class, read by name as the crate's `PedalObjectKind`.
#[pyclass(
    name = "PedalObject",
    module = "music21.expressions",
    extends = Music21Object,
    subclass,
    weakref
)]
pub struct PedalObject {
    /// `above`, `below`, or nothing.
    #[pyo3(get, set)]
    placement: Option<String>,
}

impl crate::base::Footing for PedalObject {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        Music21Object::under(PedalObject { placement: None }).add_subclass(value)
    }
}

#[pymethods]
impl PedalObject {
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> PyClassInitializer<Self> {
        crate::base::built(PedalObject { placement: None })
    }

    /// music21's `_reprInternal`: where it stands, or `uninserted`.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        if slf.getattr("activeSite")?.is_none() {
            return Ok("uninserted".to_string());
        }
        Ok(format!("at {}", slf.getattr("offset")?.str()?))
    }
}

/// The pedal objects music21 has, a class for each kind.
macro_rules! pedal_objects {
    ($(($class:ident, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(module = "music21.expressions", extends = PedalObject, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (**_keywords))]
                fn new(_keywords: Option<&Bound<'_, PyDict>>) -> PyClassInitializer<Self> {
                    crate::base::built($class)
                }
            }
        )*

        fn add_objects(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<PedalObject>()?;
            $(m.add_class::<$class>()?;)*
            Ok(())
        }
    };
}

pedal_objects!(
    (
        PedalBounce,
        "music21's `PedalBounce`: the pedal let up and put down again at once."
    ),
    (
        PedalGapStart,
        "music21's `PedalGapStart`: where the pedal line stops being drawn."
    ),
    (
        PedalGapEnd,
        "music21's `PedalGapEnd`: where the pedal line is drawn again."
    ),
);

/// A `StrEnum` of music21's, its members named as music21 names them and
/// valued as the crate writes them.
fn str_enum<'py>(
    py: Python<'py>,
    name: &str,
    members: &[(&str, &str)],
) -> PyResult<Bound<'py, PyAny>> {
    let listed = PyList::new(
        py,
        members
            .iter()
            .map(|(member, value)| PyTuple::new(py, [*member, *value]))
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let keywords = PyDict::new(py);
    keywords.set_item("module", MODULE)?;
    crate::plumbing::str_enum(py)?.call((name, listed), Some(&keywords))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    add_objects(m)?;
    let types: Vec<(&str, &str)> = [
        ("Sustain", PedalType::Sustain),
        ("Sostenuto", PedalType::Sostenuto),
        ("Soft", PedalType::Soft),
        ("Silent", PedalType::Silent),
    ]
    .iter()
    .map(|(member, kind)| (*member, kind.as_str()))
    .collect();
    m.add("PedalType", str_enum(py, "PedalType", &types)?)?;
    let forms: Vec<(&str, &str)> = [
        ("Line", PedalForm::Line),
        ("Symbol", PedalForm::Symbol),
        ("SymbolAlt", PedalForm::SymbolAlt),
        ("SymbolLine", PedalForm::SymbolLine),
    ]
    .iter()
    .map(|(member, form)| (*member, form.as_str()))
    .collect();
    m.add("PedalForm", str_enum(py, "PedalForm", &forms)?)?;
    Ok(())
}
