//! music21's `meter.tools`, over the crate's `meter::tools`: meters
//! written as text read into ratios, and the ways music21 conventionally
//! divides a meter, as the tuples music21 hands back.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use num_bigint::BigInt;
use pyo3::prelude::*;
use pyo3::types::{PyFrozenSet, PyTuple};

use music21::{FloatType, UnsignedIntegerType};

use music21::meter::tools::{self as rs, SlashError};

use crate::meter::{MeterException, TimeSignatureException};

/// music21's `MeterTerminalTuple`, built once.
static TERMINAL_TUPLE: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

fn terminal_tuple(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    let class = TERMINAL_TUPLE.get_or_try_init(py, || {
        let made = py
            .import("collections")?
            .getattr("namedtuple")?
            .call1(("MeterTerminalTuple", "numerator denominator division"))?;
        made.setattr("__module__", "music21.meter.tools")?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    Ok(class.bind(py))
}

/// The exception music21 raises for a meter it cannot read.
fn slash_error(py: Python<'_>, error: SlashError) -> PyErr {
    match &error {
        SlashError::Meter(message) => MeterException::new_err(message.clone()),
        SlashError::TimeSignature(message) => TimeSignatureException::new_err(message.clone()),
        SlashError::Unreadable(message) => {
            PyErr::from_type(py.get_type::<crate::Music21Exception>(), message.clone())
        }
    }
}

/// A list of options as music21's tuple of tuples of strings.
fn options<'py>(py: Python<'py>, found: Vec<Vec<String>>) -> PyResult<Bound<'py, PyTuple>> {
    let made = found
        .into_iter()
        .map(|option| PyTuple::new(py, option))
        .collect::<PyResult<Vec<_>>>()?;
    PyTuple::new(py, made)
}

/// The numerator, the denominator and how the beat is felt.
#[pyfunction]
fn slashToTuple<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    let read = rs::slash_to_tuple(value).map_err(|error| slash_error(py, error))?;
    let division = crate::common::module(py, "music21.common.enums")?
        .getattr("MeterDivision")?
        .getattr(read.division.music21_name())?;
    terminal_tuple(py)?.call1((read.numerator, read.denominator, division))
}

/// Each part's numerator and denominator, leaving out what is not one.
#[pyfunction]
fn slashCompoundToFraction<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, rs::slash_compound_to_fraction(value))
}

/// Each part's numerator and denominator, and whether a numerator took the
/// denominator after it.
#[pyfunction]
fn slashMixedToFraction<'py>(py: Python<'py>, valueSrc: &str) -> PyResult<Bound<'py, PyTuple>> {
    let (parts, summed) =
        rs::slash_mixed_to_fraction(valueSrc).map_err(|error| slash_error(py, error))?;
    (PyTuple::new(py, parts)?, summed).into_pyobject(py)
}

/// The numerators of neighbours sharing a denominator, run together.
#[pyfunction]
fn fractionToSlashMixed<'py>(
    py: Python<'py>,
    fList: Vec<(UnsignedIntegerType, UnsignedIntegerType)>,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, rs::fraction_to_slash_mixed(&fList))
}

/// The sum, not reduced.
#[pyfunction]
fn fractionSum(numDenomTuple: Vec<(BigInt, BigInt)>) -> (BigInt, BigInt) {
    rs::fraction_sum(&numDenomTuple)
}

/// The nearest fraction with a denominator of at most sixteen.
#[pyfunction]
fn proportionToFraction(value: FloatType) -> PyResult<(BigInt, BigInt)> {
    if value.is_nan() {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "cannot convert NaN to integer ratio",
        ));
    }
    rs::proportion_to_fraction(value).ok_or_else(|| {
        pyo3::exceptions::PyOverflowError::new_err("cannot convert Infinity to integer ratio")
    })
}

/// The same ratio in shorter notes.
#[pyfunction]
fn divisionOptionsFractionsUpward(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    PyTuple::new(py, rs::fractions_upward(n, d))
}

/// The same ratio in longer notes.
#[pyfunction]
fn divisionOptionsFractionsDownward(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    PyTuple::new(py, rs::fractions_downward(n, d))
}

/// A single unit split into smaller ones.
#[pyfunction]
fn divisionOptionsAdditiveMultiplesDownward(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    options(py, rs::additive_multiples_downward(n, d))
}

/// Even groupings in the meter's own note value.
#[pyfunction]
fn divisionOptionsAdditiveMultiples(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    options(py, rs::additive_multiples(n, d))
}

/// The bar halved, and halved again while it stays even.
#[pyfunction]
fn divisionOptionsAdditiveMultiplesEvenDivision(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    options(py, rs::additive_multiples_halved(n, d))
}

/// One unit at a time, then twice as many half as long.
#[pyfunction]
fn divisionOptionsAdditiveMultiplesUpward(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    options(py, rs::additive_multiples_upward(n, d))
}

/// Every way the rules divide the meter, in music21's order.
#[pyfunction]
fn divisionOptionsAlgo(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    options(py, rs::division_options_algo(n, d))
}

/// The divisions no rule produces.
#[pyfunction]
fn divisionOptionsPreset(
    py: Python<'_>,
    n: UnsignedIntegerType,
    d: UnsignedIntegerType,
) -> PyResult<Bound<'_, PyTuple>> {
    options(py, rs::division_options_preset(n, d))
}

static BUILT: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// music21's `meter.tools`, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.meter.tools")?;
        m.add_function(wrap_pyfunction!(slashToTuple, &m)?)?;
        m.add_function(wrap_pyfunction!(slashCompoundToFraction, &m)?)?;
        m.add_function(wrap_pyfunction!(slashMixedToFraction, &m)?)?;
        m.add_function(wrap_pyfunction!(fractionToSlashMixed, &m)?)?;
        m.add_function(wrap_pyfunction!(fractionSum, &m)?)?;
        m.add_function(wrap_pyfunction!(proportionToFraction, &m)?)?;
        m.add_function(wrap_pyfunction!(divisionOptionsFractionsUpward, &m)?)?;
        m.add_function(wrap_pyfunction!(divisionOptionsFractionsDownward, &m)?)?;
        m.add_function(wrap_pyfunction!(
            divisionOptionsAdditiveMultiplesDownward,
            &m
        )?)?;
        m.add_function(wrap_pyfunction!(divisionOptionsAdditiveMultiples, &m)?)?;
        m.add_function(wrap_pyfunction!(
            divisionOptionsAdditiveMultiplesEvenDivision,
            &m
        )?)?;
        m.add_function(wrap_pyfunction!(
            divisionOptionsAdditiveMultiplesUpward,
            &m
        )?)?;
        m.add_function(wrap_pyfunction!(divisionOptionsAlgo, &m)?)?;
        m.add_function(wrap_pyfunction!(divisionOptionsPreset, &m)?)?;
        m.add("MeterTerminalTuple", terminal_tuple(py)?)?;
        let denominators = rs::VALID_DENOMINATORS;
        m.add("validDenominators", PyTuple::new(py, denominators)?)?;
        m.add("validDenominatorsSet", PyFrozenSet::new(py, denominators)?)?;
        m.add(
            "MeterDivision",
            crate::common::module(py, "music21.common.enums")?.getattr("MeterDivision")?,
        )?;
        m.add("MeterException", py.get_type::<MeterException>())?;
        m.add(
            "TimeSignatureException",
            py.get_type::<TimeSignatureException>(),
        )?;
        m.add("Music21Exception", py.get_type::<crate::Music21Exception>())?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
