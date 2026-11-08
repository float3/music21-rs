//! music21's `common.enums`: the `StrEnum`, `HexEnum` and `BooleanEnum`
//! bases music21's enums are built on, and the enums of this module.
//!
//! Each is a real `enum.Enum` subclass, built by the enum metaclass as a
//! `class` statement would build it, with the members the crate lists in
//! [`music21::common::enums`] and methods written in Rust.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyString, PyTuple};

use music21::common::enums::{
    APPEND_SPANNERS, ELEMENT_SEARCH, GATHER_SPANNERS, METER_DIVISION, Member, MemberValue,
    OFFSET_SPECIAL, ORNAMENT_DELAY, hex_numbers,
};

use super::built::{class_method, method, new_class, static_method, super_of};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.enums";

/// `HexEnum` and `BooleanEnum`, which their methods call `super()` on.
static HEX_ENUM: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static BOOLEAN_ENUM: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// An enum member's repr with its value left out: `<Enclosure.SQUARE>`.
#[pyfunction]
fn member_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let class = super::built::class_name(slf)?;
    let name = slf.getattr("name")?;
    Ok(format!("<{class}.{name}>"))
}

/// `IntEnum`'s repr, its numbers in hexadecimal: `<X.NOTE_ON: 0x90>`.
#[pyfunction]
fn hex_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let class = stored(slf.py(), &HEX_ENUM)?;
    let plain = super_of(&class, slf)?.call_method0("__repr__")?;
    Ok(hex_numbers(&plain.str()?.to_cow()?))
}

/// Whether `item` is one of the enum's members or values; deprecated in
/// music21 for `item in cls`.
#[pyfunction]
fn hasValue(cls: &Bound<'_, PyAny>, item: &Bound<'_, PyAny>) -> PyResult<bool> {
    cls.contains(item)
}

/// Whether a value is a pair of a boolean and something else, which is how
/// a `BooleanEnum` member is both true or false and equal to that thing.
#[pyfunction]
fn is_bool_tuple(v: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Ok(pair) = v.cast::<PyTuple>() else {
        return Ok(false);
    };
    Ok(pair.len() == 2 && pair.get_item(0)?.is_instance_of::<PyBool>())
}

/// A `BooleanEnum` member equals another member as `Enum` decides, and
/// anything else its value equals, or, for a pair, the boolean itself or
/// whatever the second item equals.
#[pyfunction]
#[pyo3(name = "__eq__")]
fn boolean_eq(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = slf.py();
    if other.is_instance(&slf.getattr("__class__")?)? {
        let class = stored(py, &BOOLEAN_ENUM)?;
        return Ok(super_of(&class, slf)?
            .call_method1("__eq__", (other,))?
            .unbind());
    }
    let value = slf.getattr("value")?;
    if value.eq(other)? {
        return Ok(PyBool::new(py, true).to_owned().into_any().unbind());
    }
    if is_bool_tuple(&value)? {
        if value.get_item(0)?.is(other) {
            return Ok(PyBool::new(py, true).to_owned().into_any().unbind());
        }
        return Ok(value
            .get_item(1)?
            .rich_compare(other, pyo3::basic::CompareOp::Eq)?
            .unbind());
    }
    Ok(PyBool::new(py, false).to_owned().into_any().unbind())
}

/// A `BooleanEnum` member is true or false as its value is, or as the
/// boolean of a pair is.
#[pyfunction]
#[pyo3(name = "__bool__")]
fn boolean_bool(slf: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let value = slf.getattr("value")?;
    if is_bool_tuple(&value)? {
        return Ok(value.get_item(0)?.unbind());
    }
    Ok(PyBool::new(slf.py(), value.is_truthy()?)
        .to_owned()
        .into_any()
        .unbind())
}

/// A class built once and kept.
fn stored<'py>(py: Python<'py>, cell: &PyOnceLock<Py<PyAny>>) -> PyResult<Bound<'py, PyAny>> {
    cell.get(py)
        .map(|class| class.bind(py).clone())
        .ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("music21.common.enums is not built yet")
        })
}

/// `enum.StrEnum`, or on Python before 3.11, which has none, a `str` enum
/// whose `str()` is its value as `StrEnum`'s is.
fn standard_str_enum(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let module = py.import("enum")?;
    if let Ok(found) = module.getattr("StrEnum") {
        return Ok(found);
    }
    let text = py.get_type::<PyString>().into_any();
    let string = text.getattr("__str__")?;
    new_class(
        py,
        "enum",
        "StrEnum",
        "",
        vec![text, module.getattr("Enum")?],
        vec![("__str__", string)],
    )
}

/// One of this module's enums, on `base`, with a table's members.
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
            MemberValue::Bool(flag) => PyBool::new(py, *flag).to_owned().into_any(),
            MemberValue::Text(text) => PyString::new(py, text).into_any(),
        };
        values.push((*member, value));
    }
    new_class(py, MODULE, name, doc, vec![base.clone()], values)
}

/// music21's `common.enums`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    let enum_module = py.import("enum")?;
    let repr = |class: &str| {
        method(
            wrap_pyfunction!(member_repr, &m)?.into_any(),
            MODULE,
            class,
            "__repr__",
        )
    };

    let str_enum = new_class(
        py,
        MODULE,
        "StrEnum",
        "A string enum whose repr leaves the value out, as music21's always has.",
        vec![standard_str_enum(py)?],
        vec![("__repr__", repr("StrEnum")?)],
    )?;
    let hex_enum = new_class(
        py,
        MODULE,
        "HexEnum",
        "An `IntEnum` whose repr gives its value in hexadecimal.",
        vec![enum_module.getattr("IntEnum")?],
        vec![
            (
                "__repr__",
                method(
                    wrap_pyfunction!(hex_repr, &m)?.into_any(),
                    MODULE,
                    "HexEnum",
                    "__repr__",
                )?,
            ),
            (
                "hasValue",
                class_method(
                    wrap_pyfunction!(hasValue, &m)?.into_any(),
                    MODULE,
                    "HexEnum",
                    "hasValue",
                )?,
            ),
        ],
    )?;
    let _ = HEX_ENUM.set(py, hex_enum.clone().unbind());
    let boolean_enum = new_class(
        py,
        MODULE,
        "BooleanEnum",
        "An enum standing in for a boolean: each member is true or false, and \
         equal to its value, or to both parts of a (boolean, value) pair.",
        vec![enum_module.getattr("Enum")?],
        vec![
            (
                "is_bool_tuple",
                static_method(
                    wrap_pyfunction!(is_bool_tuple, &m)?.into_any(),
                    MODULE,
                    "BooleanEnum",
                    "is_bool_tuple",
                )?,
            ),
            (
                "__eq__",
                method(
                    wrap_pyfunction!(boolean_eq, &m)?.into_any(),
                    MODULE,
                    "BooleanEnum",
                    "__eq__",
                )?,
            ),
            (
                "__bool__",
                method(
                    wrap_pyfunction!(boolean_bool, &m)?.into_any(),
                    MODULE,
                    "BooleanEnum",
                    "__bool__",
                )?,
            ),
            ("__repr__", repr("BooleanEnum")?),
        ],
    )?;
    let _ = BOOLEAN_ENUM.set(py, boolean_enum.clone().unbind());

    m.add("StrEnum", &str_enum)?;
    m.add("HexEnum", &hex_enum)?;
    m.add("ContainsEnum", &hex_enum)?;
    m.add("BooleanEnum", &boolean_enum)?;
    let tables: [(&str, &Bound<'_, PyAny>, &str, &[Member]); 6] = [
        (
            "ElementSearch",
            &str_enum,
            "The directions `getContextByClass` can search in.",
            ELEMENT_SEARCH,
        ),
        (
            "OffsetSpecial",
            &str_enum,
            "Offsets that are not numbers; `AT_END` is `'highestTime'`.",
            OFFSET_SPECIAL,
        ),
        (
            "GatherSpanners",
            &boolean_enum,
            "Which spanners an excerpt gathers: all, none, or only those all of whose members it holds.",
            GATHER_SPANNERS,
        ),
        (
            "AppendSpanners",
            &str_enum,
            "Whether the spanners an object starts or ends are written with it, instead of it, or not at all.",
            APPEND_SPANNERS,
        ),
        (
            "OrnamentDelay",
            &str_enum,
            "The delays an ornament can have that are not a length: none, or half the note.",
            ORNAMENT_DELAY,
        ),
        (
            "MeterDivision",
            &str_enum,
            "How a time signature is divided.",
            METER_DIVISION,
        ),
    ];
    for (name, base, doc, members) in tables {
        m.add(name, enum_of(base, name, doc, members)?)?;
    }
    Ok(m)
}
