//! music21's `stream.enums`: `StaffType`, `GivenElementsBehavior`,
//! `RecursionType` and `ShowNumber`.
//!
//! Each is a real `enum.Enum` subclass with the members the crate lists in
//! [`music21::stream::enums`]; all but `StaffType` are built on
//! `common.enums.StrEnum`, as music21's are.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyModule, PyString};

use music21::common::enums::{Member, MemberValue};
use music21::stream::enums::{GIVEN_ELEMENTS_BEHAVIOR, RECURSION_TYPE, SHOW_NUMBER, STAFF_TYPE};

use crate::plumbing::built::new_class;

/// The module music21 keeps these in.
const MODULE: &str = "music21.stream.enums";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// One of the enums, on `base`, with a table's members.
fn enum_of<'py>(
    base: &Bound<'py, PyAny>,
    name: &str,
    doc: &str,
    members: &[Member],
) -> PyResult<Bound<'py, PyAny>> {
    let py = base.py();
    let mut values = Vec::new();
    for (member, value) in members {
        let value = match value {
            MemberValue::Text(text) => PyString::new(py, text).into_any(),
            MemberValue::Bool(flag) => pyo3::types::PyBool::new(py, *flag).to_owned().into_any(),
        };
        values.push((*member, value));
    }
    new_class(py, MODULE, name, doc, vec![base.clone()], values)
}

/// music21's `stream.enums`, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let plain = py.import("enum")?.getattr("Enum")?;
        let text = crate::plumbing::str_enum(py)?;
        let tables: [(&str, &Bound<'_, PyAny>, &str, &[Member]); 4] = [
            (
                "StaffType",
                &plain,
                "What a staff is for, as MusicXML's staff-type has it, upper-cased; \
                 `OTHER` is anything else.",
                STAFF_TYPE,
            ),
            (
                "GivenElementsBehavior",
                &text,
                "How a stream built with elements places them: one after another, \
                 at their own offsets, or inserted.",
                GIVEN_ELEMENTS_BEHAVIOR,
            ),
            (
                "RecursionType",
                &text,
                "The order a context search looks through a stream in.",
                RECURSION_TYPE,
            ),
            (
                "ShowNumber",
                &text,
                "Whether a measure's number is shown: as usual, always or never.",
                SHOW_NUMBER,
            ),
        ];
        for (name, base, doc, members) in tables {
            m.add(name, enum_of(base, name, doc, members)?)?;
        }
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// The member of music21's `StaffType` with this value: music21's own
/// where it imports, so it is the member music21 compares against, and the
/// built one otherwise.
pub(crate) fn staff_type<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    let module = match crate::real_music21(py, MODULE) {
        Ok(module) => module,
        Err(_) => module(py)?,
    };
    module.getattr("StaffType")?.call1((value,))
}
