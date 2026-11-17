//! music21's `sorting`: `SortTuple`, the six numbers deciding which of two
//! things comes first in a stream, and its three tuples at offset nought.
//!
//! `SortTuple` is a real named tuple, built on `collections.namedtuple`
//! with methods written in Rust over [`music21::sorting`]: it compares
//! against another tuple as a tuple does, and against a bare offset by its
//! own, a tuple stored at the end standing after every offset but
//! infinity.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule, PyTuple};

use music21::sorting::{Comparison, SORT_TUPLE_FIELDS, at_end_compared, is_flag_field, short_repr};

use crate::plumbing::built::{method, new_class};

/// The module music21 keeps these in.
const MODULE: &str = "music21.sorting";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(music21_rs_facade, SortingException, crate::Music21Exception);

/// The comparison each of a tuple's comparison methods makes, with the
/// method of `tuple`'s that makes it against another tuple.
const COMPARISONS: [(&str, Comparison); 6] = [
    ("__lt__", Comparison::Less),
    ("__le__", Comparison::LessEqual),
    ("__eq__", Comparison::Equal),
    ("__ne__", Comparison::NotEqual),
    ("__gt__", Comparison::Greater),
    ("__ge__", Comparison::GreaterEqual),
];

/// `slf` against a bare offset, `other`: the comparison its own offset
/// makes, unless it is stored at the end.
fn against_offset(
    slf: &Bound<'_, PyAny>,
    other: &Bound<'_, PyAny>,
    comparison: Comparison,
) -> PyResult<bool> {
    if slf.get_item(0)?.eq(1)? {
        let infinite = other.eq(f64::INFINITY)?;
        return Ok(at_end_compared(comparison, infinite));
    }
    let offset = slf.get_item(1)?;
    Ok(match comparison {
        Comparison::Less => offset.lt(other)?,
        Comparison::LessEqual => offset.lt(other)? || offset.eq(other)?,
        Comparison::Equal => offset.eq(other)?,
        Comparison::NotEqual => !offset.eq(other)?,
        Comparison::Greater => offset.gt(other)?,
        Comparison::GreaterEqual => offset.gt(other)? || offset.eq(other)?,
    })
}

/// A comparison: as `tuple` makes it against a tuple, else against a bare
/// offset; `NotImplemented` where the offset refuses with `ValueError`.
fn compared(
    slf: &Bound<'_, PyAny>,
    other: &Bound<'_, PyAny>,
    name: &str,
    comparison: Comparison,
) -> PyResult<Py<PyAny>> {
    let py = slf.py();
    if other.is_instance_of::<PyTuple>() {
        let tuple = py.get_type::<PyTuple>();
        return Ok(tuple.getattr(name)?.call1((slf, other))?.unbind());
    }
    match against_offset(slf, other, comparison) {
        Ok(answer) => Ok(pyo3::types::PyBool::new(py, answer)
            .to_owned()
            .into_any()
            .unbind()),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(py.NotImplemented()),
        Err(error) => Err(error),
    }
}

#[pyfunction]
fn sort_lt(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    compared(slf, other, "__lt__", Comparison::Less)
}

#[pyfunction]
fn sort_le(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    compared(slf, other, "__le__", Comparison::LessEqual)
}

#[pyfunction]
fn sort_eq(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    compared(slf, other, "__eq__", Comparison::Equal)
}

#[pyfunction]
fn sort_ne(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    compared(slf, other, "__ne__", Comparison::NotEqual)
}

#[pyfunction]
fn sort_gt(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    compared(slf, other, "__gt__", Comparison::Greater)
}

#[pyfunction]
fn sort_ge(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    compared(slf, other, "__ge__", Comparison::GreaterEqual)
}

/// The tuple written short: `'1.0 <0.20.323>'`, or `'End <4.7.[Grace].200>'`
/// for a grace note stored at the end.
#[pyfunction]
fn shortRepr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let part = |at: usize| -> PyResult<String> { Ok(slf.get_item(at)?.str()?.to_string()) };
    Ok(short_repr(
        slf.get_item(0)?.is_truthy()?,
        &part(1)?,
        &part(2)?,
        &part(3)?,
        !slf.get_item(4)?.eq(0)?,
        &part(5)?,
    ))
}

/// A new tuple, the same but for the fields named; a name that is no
/// field's is a `ValueError`.
#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn modify<'py>(
    slf: &Bound<'py, PyAny>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    slf.call_method("_replace", (), keywords)
}

/// Each field of two tuples combined: `atEnd` and `isNotGrace` by
/// `flags`, the rest by `numbers`.
fn combined<'py>(
    slf: &Bound<'py, PyAny>,
    other: &Bound<'py, PyAny>,
    flags: &str,
    numbers: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let class = slf.get_type();
    if !other.is_instance(&class)? {
        return Err(SortingException::new_err(
            "Cannot add attributes from a different class",
        ));
    }
    let builtins = py.import("builtins")?;
    let operator = py.import("operator")?;
    let mut out = Vec::new();
    for (at, field) in SORT_TUPLE_FIELDS.iter().enumerate() {
        let mine = slf.get_item(at)?;
        let theirs = other.get_item(at)?;
        let joined = if is_flag_field(field) {
            builtins.getattr(flags)?.call1((mine, theirs))?
        } else {
            operator.getattr(numbers)?.call1((mine, theirs))?
        };
        out.push(joined);
    }
    class.call1(PyTuple::new(py, out)?)
}

/// Every field of this tuple and another added, `atEnd` and `isNotGrace`
/// taking the larger of the two.
#[pyfunction]
fn add<'py>(slf: &Bound<'py, PyAny>, other: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    combined(slf, other, "max", "add")
}

/// Every field of another tuple subtracted from this one's, `atEnd` and
/// `isNotGrace` taking the smaller of the two.
#[pyfunction]
fn sub<'py>(slf: &Bound<'py, PyAny>, other: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    combined(slf, other, "min", "sub")
}

/// music21's `SortTuple`, built.
fn sort_tuple_class<'py>(m: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let fields = PyTuple::new(py, SORT_TUPLE_FIELDS)?;
    let base = py
        .import("collections")?
        .getattr("namedtuple")?
        .call1(("SortTuple", fields))?;
    base.setattr("__module__", MODULE)?;
    let as_method =
        |function: Bound<'py, PyAny>, name: &str| method(function, MODULE, "SortTuple", name);
    let comparisons = [
        wrap_pyfunction!(sort_lt, m)?,
        wrap_pyfunction!(sort_le, m)?,
        wrap_pyfunction!(sort_eq, m)?,
        wrap_pyfunction!(sort_ne, m)?,
        wrap_pyfunction!(sort_gt, m)?,
        wrap_pyfunction!(sort_ge, m)?,
    ];
    let mut members = vec![("__slots__", PyTuple::empty(py).into_any())];
    for ((name, _), function) in COMPARISONS.iter().zip(comparisons) {
        members.push((name, as_method(function.into_any(), name)?));
    }
    members.push((
        "shortRepr",
        as_method(wrap_pyfunction!(shortRepr, m)?.into_any(), "shortRepr")?,
    ));
    members.push((
        "modify",
        as_method(wrap_pyfunction!(modify, m)?.into_any(), "modify")?,
    ));
    members.push((
        "add",
        as_method(wrap_pyfunction!(add, m)?.into_any(), "add")?,
    ));
    members.push((
        "sub",
        as_method(wrap_pyfunction!(sub, m)?.into_any(), "sub")?,
    ));
    new_class(
        py,
        MODULE,
        "SortTuple",
        "The six numbers deciding which of two things comes first in a stream, \
         compared in order. One compares against a bare offset by its own, and \
         one stored at the end comes after every offset but infinity.",
        vec![base],
        members,
    )
}

/// music21's `sorting`, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<SortingException>();
        exception.setattr("__module__", MODULE)?;
        m.add("SortingException", exception)?;
        let class = sort_tuple_class(&m)?;
        for (name, priority) in [
            ("ZeroSortTupleDefault", 0.0),
            ("ZeroSortTupleLow", f64::NEG_INFINITY),
            ("ZeroSortTupleHigh", f64::INFINITY),
        ] {
            let priority = if priority == 0.0 {
                0_i64.into_pyobject(py)?.into_any()
            } else {
                priority.into_pyobject(py)?.into_any()
            };
            m.add(name, class.call1((0, 0.0, priority, 0, 1, 0))?)?;
        }
        m.add("SortTuple", class)?;
        m.add("INFINITY", f64::INFINITY)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// The built `SortTuple`.
pub(crate) fn sort_tuple(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    module(py)?.getattr("SortTuple")
}
