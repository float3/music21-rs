//! music21's `musedata.base40` and `musedata.base12_26`: Hewlett's pitch
//! numbers, over the crate's `musedata::base40` and `base12_26`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule};

use music21::musedata::base12_26;
use music21::musedata::base40::{self, Base40Error, Direction};

/// The modules music21 keeps these in.
const BASE40: &str = "music21.musedata.base40";
const BASE12_26: &str = "music21.musedata.base12_26";

static BUILT_BASE40: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static BUILT_BASE12_26: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(music21_rs_facade, Base40Exception, crate::Music21Exception);

fn refused(error: Base40Error) -> PyErr {
    Base40Exception::new_err(error.message().to_owned())
}

/// A table of numbers against names, as music21's dictionary of it.
fn by_number<'py>(py: Python<'py>, rows: &[(i32, &str)]) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (number, name) in rows {
        out.set_item(number, name)?;
    }
    Ok(out)
}

/// The same table turned round, each name against its number, the unused
/// numbers left out.
fn by_name<'py>(py: Python<'py>, rows: &[(i32, &'static str)]) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    for (name, number) in base12_26::named(rows) {
        out.set_item(name, number)?;
    }
    Ok(out)
}

fn interval_out(
    py: Python<'_>,
    found: Result<music21::interval::Interval, Base40Error>,
) -> PyResult<Py<PyAny>> {
    crate::interval::interval_object(py, found.map_err(refused)?)
}

/// The interval a distance between two base-40 numbers is, octaves and
/// direction kept.
#[pyfunction]
fn base40DeltaToInterval(py: Python<'_>, delta: i32) -> PyResult<Py<PyAny>> {
    interval_out(py, base40::delta_to_interval(delta))
}

/// The pitch a base-40 number is, octave and all.
#[pyfunction]
fn base40ToPitch(py: Python<'_>, base40Num: i32) -> PyResult<Py<PyAny>> {
    let pitch = base40::to_pitch(base40Num).map_err(refused)?;
    let inferred = pitch.spelling_is_inferred();
    Ok(crate::installed_new(
        py,
        "music21.pitch",
        "Pitch",
        crate::pitch::Pitch::wrap(pitch, inferred),
    )?
    .into_any())
}

/// The base-40 number of a pitch, or of the pitch a name with its octave
/// writes.
#[pyfunction]
fn pitchToBase40(pitchToConvert: &Bound<'_, PyAny>) -> PyResult<i32> {
    let pitch = crate::pitch::pitch_from_any(pitchToConvert)?;
    if pitch.octave().is_none() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "unsupported operand type(s) for -: 'NoneType' and 'int'",
        ));
    }
    base40::from_pitch(&pitch).map_err(refused)
}

/// The interval between two base-40 numbers, read from their distance.
#[pyfunction]
fn base40Interval(py: Python<'_>, base40NumA: i32, base40NumB: i32) -> PyResult<Py<PyAny>> {
    interval_out(py, base40::interval(base40NumA, base40NumB))
}

/// The interval between the pitches two base-40 numbers are.
#[pyfunction]
fn base40ActualInterval(py: Python<'_>, base40NumA: i32, base40NumB: i32) -> PyResult<Py<PyAny>> {
    interval_out(py, base40::actual_interval(base40NumA, base40NumB))
}

fn names<'py>(py: Python<'py>, found: Vec<String>) -> PyResult<Bound<'py, PyList>> {
    PyList::new(py, found)
}

/// The other spellings of a name one way, `'up'` or `'down'`.
#[pyfunction]
#[pyo3(signature = (nameStr, direction = "up", allowDoubleAccidentals = true))]
fn _quickEnharmonicString<'py>(
    py: Python<'py>,
    nameStr: &str,
    direction: &str,
    allowDoubleAccidentals: bool,
) -> PyResult<Bound<'py, PyList>> {
    let direction = match direction {
        "up" => Direction::Up,
        "down" => Direction::Down,
        other => {
            return Err(Base40Exception::new_err(format!(
                "Not a valid direction, {other}"
            )));
        }
    };
    names(
        py,
        base40::quick_enharmonics(nameStr, direction, allowDoubleAccidentals),
    )
}

/// The higher spellings of a name, each a diminished second above the
/// last.
#[pyfunction]
#[pyo3(signature = (nameStr, allowDoubleAccidentals = true))]
fn quickHigherEnharmonicString<'py>(
    py: Python<'py>,
    nameStr: &str,
    allowDoubleAccidentals: bool,
) -> PyResult<Bound<'py, PyList>> {
    names(
        py,
        base40::quick_enharmonics(nameStr, Direction::Up, allowDoubleAccidentals),
    )
}

/// The lower spellings of a name, each a diminished second below the
/// last.
#[pyfunction]
#[pyo3(signature = (nameStr, allowDoubleAccidentals = true))]
fn quickLowerEnharmonicString<'py>(
    py: Python<'py>,
    nameStr: &str,
    allowDoubleAccidentals: bool,
) -> PyResult<Bound<'py, PyList>> {
    names(
        py,
        base40::quick_enharmonics(nameStr, Direction::Down, allowDoubleAccidentals),
    )
}

/// The lower spellings of a name, then the higher.
#[pyfunction]
#[pyo3(signature = (nameStr, allowDoubleAccidentals = true))]
fn quickEnharmonicString<'py>(
    py: Python<'py>,
    nameStr: &str,
    allowDoubleAccidentals: bool,
) -> PyResult<Bound<'py, PyList>> {
    names(
        py,
        base40::quick_enharmonic_string(nameStr, allowDoubleAccidentals),
    )
}

/// music21's `musedata.base40`, built.
pub(crate) fn base40_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_BASE40.get_or_try_init(py, || {
        let m = PyModule::new(py, BASE40)?;
        m.add("base40Equivalent", by_number(py, &base40::EQUIVALENTS)?)?;
        m.add("base40Representation", by_name(py, &base40::EQUIVALENTS)?)?;
        m.add("base40IntervalTable", by_number(py, &base40::INTERVALS)?)?;
        let exception = py.get_type::<Base40Exception>();
        exception.setattr("__module__", BASE40)?;
        m.add("Base40Exception", exception)?;
        m.add_function(wrap_pyfunction!(base40DeltaToInterval, &m)?)?;
        m.add_function(wrap_pyfunction!(base40ToPitch, &m)?)?;
        m.add_function(wrap_pyfunction!(pitchToBase40, &m)?)?;
        m.add_function(wrap_pyfunction!(base40Interval, &m)?)?;
        m.add_function(wrap_pyfunction!(base40ActualInterval, &m)?)?;
        m.add_function(wrap_pyfunction!(_quickEnharmonicString, &m)?)?;
        m.add_function(wrap_pyfunction!(quickHigherEnharmonicString, &m)?)?;
        m.add_function(wrap_pyfunction!(quickLowerEnharmonicString, &m)?)?;
        m.add_function(wrap_pyfunction!(quickEnharmonicString, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `musedata.base12_26`, built: its tables, which are all it has.
pub(crate) fn base12_26_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_BASE12_26.get_or_try_init(py, || {
        let m = PyModule::new(py, BASE12_26)?;
        m.add(
            "base12Equivalent",
            by_number(py, &base12_26::BASE12_EQUIVALENTS)?,
        )?;
        m.add(
            "base12Representation",
            by_name(py, &base12_26::BASE12_EQUIVALENTS)?,
        )?;
        m.add(
            "Base12IntervalTable",
            by_number(py, &base12_26::BASE12_INTERVALS)?,
        )?;
        m.add(
            "base26Equivalent",
            by_number(py, &base12_26::BASE26_EQUIVALENTS)?,
        )?;
        m.add(
            "base26Representation",
            by_name(py, &base12_26::BASE26_EQUIVALENTS)?,
        )?;
        m.add(
            "Base26IntervalTable",
            by_number(py, &base12_26::BASE26_INTERVALS)?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
