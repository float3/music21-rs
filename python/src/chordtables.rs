//! music21's `chord.tables` module: the Forte tables of set classes, read
//! without a chord in hand.
//!
//! These are lookups over data the crate already carries — `xtask
//! verify-tables` checks it against music21's own `chord/tables.py` — so the
//! facade is nothing but argument shapes. `ChordTableAddress` itself stays
//! music21's: it is a namedtuple of four numbers and there is nothing here
//! to put in its place.

#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use music21_rs_crate::chord::tables;

/// The names the `chord.tables` facade replaces in `music21.chord.tables`.
pub const NAMES: &[&str] = &[
    "ChordTablesException",
    "forteIndexToInversionsAvailable",
    "_validateAddress",
    "addressToTransposedNormalForm",
    "addressToPrimeForm",
    "addressToIntervalVector",
    "intervalVectorToAddress",
    "addressToZAddress",
    "addressToCommonNames",
    "addressToForteName",
    "seekChordTablesAddress",
];

pyo3::create_exception!(
    music21_rs_facade,
    ChordTablesException,
    crate::Music21Exception
);

error_into!(tables_error, ChordTablesException);

/// An address as music21 takes one: a cardinality, a Forte class number, and
/// an inversion that may be left out or given as `None`.
///
/// It arrives as any sequence of two or three numbers — a tuple, a list, or
/// the `ChordTableAddress` namedtuple an earlier call handed back.
fn address_of(value: &Bound<'_, PyAny>) -> PyResult<(u8, u8, Option<i8>)> {
    let mut parts: Vec<Option<i32>> = Vec::new();
    for item in value.try_iter()? {
        let item = item?;
        parts.push(if item.is_none() {
            None
        } else {
            Some(item.extract()?)
        });
    }
    let cardinality = parts.first().copied().flatten().ok_or_else(|| {
        ChordTablesException::new_err("an address needs a cardinality and a class number")
    })?;
    let forte_class = parts.get(1).copied().flatten().ok_or_else(|| {
        ChordTablesException::new_err("an address needs a cardinality and a class number")
    })?;
    let inversion = parts.get(2).copied().flatten();
    // Out-of-range numbers are the caller's mistake and are reported as
    // such, rather than as a set class nobody asked about.
    Ok((
        u8::try_from(cardinality).map_err(|_| {
            ChordTablesException::new_err(format!("cardinality {cardinality} not valid"))
        })?,
        u8::try_from(forte_class)
            .map_err(|_| ChordTablesException::new_err(format!("index {forte_class} not valid")))?,
        match inversion {
            None => None,
            Some(given) => Some(i8::try_from(given).map_err(|_| {
                ChordTablesException::new_err(format!("inversion {given} not valid"))
            })?),
        },
    ))
}

/// Pitch classes as music21 hands them back: a tuple of plain numbers, not
/// the bytes a list of `u8` would come across as.
fn numbers<'py>(py: Python<'py>, values: Vec<u8>) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, values.into_iter().map(u32::from))
}

/// music21's `ChordTableAddress` namedtuple, which stays music21's own.
fn address_object(
    py: Python<'_>,
    cardinality: u8,
    forte_class: u8,
    inversion: Option<i8>,
    pitch_class_original: Option<u8>,
) -> PyResult<Py<PyAny>> {
    let class = py
        .import("music21.chord.tables")?
        .getattr("ChordTableAddress")?;
    Ok(class
        .call1((cardinality, forte_class, inversion, pitch_class_original))?
        .unbind())
}

/// music21's `forteIndexToInversionsAvailable`.
#[pyfunction]
#[pyo3(name = "forteIndexToInversionsAvailable")]
fn forte_index_to_inversions_available(card: i32, index: i32) -> PyResult<Vec<i8>> {
    let card = u8::try_from(card)
        .map_err(|_| ChordTablesException::new_err(format!("cardinality {card} not valid")))?;
    let index = u8::try_from(index)
        .map_err(|_| ChordTablesException::new_err(format!("index {index} not valid")))?;
    tables::inversions_available(card, index).map_err(tables_error)
}

/// music21's `_validateAddress`: the address with the inversion filled in.
#[pyfunction]
#[pyo3(name = "_validateAddress")]
fn validate_address(address: &Bound<'_, PyAny>) -> PyResult<(u8, u8, i8)> {
    let (cardinality, forte_class, inversion) = address_of(address)?;
    tables::read_address(cardinality, forte_class, inversion).map_err(tables_error)
}

/// music21's `addressToTransposedNormalForm`.
#[pyfunction]
#[pyo3(name = "addressToTransposedNormalForm")]
fn address_to_transposed_normal_form<'py>(
    py: Python<'py>,
    address: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let (cardinality, forte_class, inversion) = address_of(address)?;
    numbers(
        py,
        tables::normal_form(cardinality, forte_class, inversion).map_err(tables_error)?,
    )
}

/// music21's `addressToPrimeForm`, which reads the inversion out of the
/// address and then ignores it: a prime form is the same either way.
#[pyfunction]
#[pyo3(name = "addressToPrimeForm")]
fn address_to_prime_form<'py>(
    py: Python<'py>,
    address: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let (cardinality, forte_class, _) = address_of(address)?;
    numbers(
        py,
        tables::prime_form(cardinality, forte_class).map_err(tables_error)?,
    )
}

/// music21's `addressToIntervalVector`.
#[pyfunction]
#[pyo3(name = "addressToIntervalVector")]
fn address_to_interval_vector<'py>(
    py: Python<'py>,
    address: &Bound<'_, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let (cardinality, forte_class, inversion) = address_of(address)?;
    numbers(
        py,
        tables::interval_class_vector(cardinality, forte_class, inversion).map_err(tables_error)?,
    )
}

/// music21's `intervalVectorToAddress`: every set class with this interval
/// vector, with the inversion and the original pitch class unknown.
#[pyfunction]
#[pyo3(name = "intervalVectorToAddress")]
fn interval_vector_to_address(
    py: Python<'_>,
    vector: &Bound<'_, PyAny>,
) -> PyResult<Vec<Py<PyAny>>> {
    let mut counts = Vec::new();
    for item in vector.try_iter()? {
        // A count no set class could have is not an error, just nothing.
        let Ok(count) = item?.extract::<i32>() else {
            return Ok(Vec::new());
        };
        match u8::try_from(count) {
            Ok(count) => counts.push(count),
            Err(_) => return Ok(Vec::new()),
        }
    }
    let found = tables::set_classes_with_interval_vector(&counts).map_err(|error| {
        // The one failure music21 reports as a plain `ValueError`.
        pyo3::exceptions::PyValueError::new_err(crate::pitch::message(&error))
    })?;
    found
        .into_iter()
        .map(|(cardinality, forte_class)| address_object(py, cardinality, forte_class, None, None))
        .collect()
}

/// music21's `addressToZAddress`: the set class with the same interval
/// vector, where there is one.
#[pyfunction]
#[pyo3(name = "addressToZAddress")]
fn address_to_z_address(py: Python<'_>, address: &Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
    let (cardinality, forte_class, _) = address_of(address)?;
    let Some((cardinality, forte_class, inversion)) =
        tables::z_related_class(cardinality, forte_class).map_err(tables_error)?
    else {
        return Ok(None);
    };
    Ok(Some(address_object(
        py,
        cardinality,
        forte_class,
        Some(inversion),
        None,
    )?))
}

/// music21's `addressToCommonNames`.
#[pyfunction]
#[pyo3(name = "addressToCommonNames")]
fn address_to_common_names(address: &Bound<'_, PyAny>) -> PyResult<Option<Vec<&'static str>>> {
    let (cardinality, forte_class, inversion) = address_of(address)?;
    tables::common_names(cardinality, forte_class, inversion).map_err(tables_error)
}

/// music21's `addressToForteName`.
#[pyfunction]
#[pyo3(name = "addressToForteName", signature = (address, classification = "tn"))]
fn address_to_forte_name(address: &Bound<'_, PyAny>, classification: &str) -> PyResult<String> {
    let (cardinality, forte_class, inversion) = address_of(address)?;
    tables::forte_name(
        cardinality,
        forte_class,
        inversion,
        classification.eq_ignore_ascii_case("tni"),
    )
    .map_err(tables_error)
}

/// music21's `seekChordTablesAddress`: where a chord's set class sits.
#[pyfunction]
#[pyo3(name = "seekChordTablesAddress")]
fn seek_chord_tables_address(py: Python<'_>, c: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    // The chord's own pitch classes, in order and without repeats, however
    // the chord answers for them: one of ours, or one of music21's.
    let mut pitch_classes: Vec<u8> = Vec::new();
    for item in c.getattr("orderedPitchClasses")?.try_iter()? {
        let value: i32 = item?.extract()?;
        pitch_classes.push(value.rem_euclid(12) as u8);
    }
    let address = tables::address_of_pitch_classes(&pitch_classes).map_err(tables_error)?;
    address_object(
        py,
        address.cardinality,
        address.forte_class,
        Some(address.inversion),
        Some(address.pitch_class_original),
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(forte_index_to_inversions_available, m)?)?;
    m.add_function(wrap_pyfunction!(validate_address, m)?)?;
    m.add_function(wrap_pyfunction!(address_to_transposed_normal_form, m)?)?;
    m.add_function(wrap_pyfunction!(address_to_prime_form, m)?)?;
    m.add_function(wrap_pyfunction!(address_to_interval_vector, m)?)?;
    m.add_function(wrap_pyfunction!(interval_vector_to_address, m)?)?;
    m.add_function(wrap_pyfunction!(address_to_z_address, m)?)?;
    m.add_function(wrap_pyfunction!(address_to_common_names, m)?)?;
    m.add_function(wrap_pyfunction!(address_to_forte_name, m)?)?;
    m.add_function(wrap_pyfunction!(seek_chord_tables_address, m)?)?;
    Ok(())
}
