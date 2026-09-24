//! music21's `figuredBass.possibility`, over the crate's.
//!
//! A possibility is a tuple of pitches, one per part, from the highest part
//! down to the bass. Each rule reads the pitches' values and asks the crate;
//! `partPairs` hands back the very objects it was given, paired.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;

use crate::Walkable;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::figuredbass::possibility as rs;
use music21_rs_crate::{Error as RsError, Pitch as RsPitch};

use crate::pitch::pitch_from_any;

pyo3::create_exception!(
    music21_rs_facade,
    PossibilityException,
    crate::Music21Exception
);

/// The names this facade replaces in `music21.figuredBass.possibility`.
pub const NAMES: &[&str] = &[
    "voiceCrossing",
    "isIncomplete",
    "upperPartsWithinLimit",
    "pitchesWithinLimit",
    "limitPartToPitch",
    "parallelFifths",
    "parallelOctaves",
    "hiddenFifths",
    "hiddenOctaves",
    "voiceOverlap",
    "partMovementsWithinLimits",
    "upperPartsSame",
    "partsSame",
    "couldBeItalianA6Resolution",
    "partPairs",
    "PossibilityException",
];

/// A figured-bass error as the exception music21 raises there: a part a
/// possibility has not got is Python's own `ValueError`, and everything else
/// music21's `PossibilityException`.
fn possibility_error(error: RsError) -> PyErr {
    if let Some(error) = crate::pitch::specific_error(&error) {
        return error;
    }
    let message = crate::pitch::message(&error);
    if message.contains("has no part") {
        return pyo3::exceptions::PyValueError::new_err(message);
    }
    PossibilityException::new_err(message)
}

/// The refusal `zip(strict=True)` gives two possibilities of different
/// lengths, which is what music21 pairs parts with.
fn unpaired(a: usize, b: usize) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(format!(
        "zip() argument 2 is {} than argument 1",
        if b < a { "shorter" } else { "longer" }
    ))
}

/// Two possibilities' pitches, refused as music21 refuses them when their
/// parts cannot be paired.
fn paired(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<(Vec<RsPitch>, Vec<RsPitch>)> {
    let (a, b) = (pitches(a)?, pitches(b)?);
    if a.len() != b.len() {
        return Err(unpaired(a.len(), b.len()));
    }
    Ok((a, b))
}

/// A possibility's pitches, in order.
fn pitches(possibility: &Bound<'_, PyAny>) -> PyResult<Vec<RsPitch>> {
    possibility
        .walk()?
        .map(|pitch| pitch_from_any(&pitch?))
        .collect()
}

#[pyfunction]
fn voiceCrossing(possibA: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(rs::voice_crossing(&pitches(possibA)?))
}

#[pyfunction]
fn isIncomplete(
    possibA: &Bound<'_, PyAny>,
    pitchNamesToContain: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let names = pitchNamesToContain
        .walk()?
        .map(|name| name?.extract::<String>())
        .collect::<PyResult<Vec<_>>>()?;
    Ok(rs::is_incomplete(&pitches(possibA)?, &names))
}

#[pyfunction]
#[pyo3(signature = (possibA, maxSemitoneSeparation = Some(12)))]
fn upperPartsWithinLimit(
    possibA: &Bound<'_, PyAny>,
    maxSemitoneSeparation: Option<i32>,
) -> PyResult<bool> {
    Ok(rs::upper_parts_within_limit(
        &pitches(possibA)?,
        maxSemitoneSeparation,
    ))
}

#[pyfunction]
#[pyo3(signature = (possibA, maxPitch = None))]
fn pitchesWithinLimit(
    possibA: &Bound<'_, PyAny>,
    maxPitch: Option<&Bound<'_, PyAny>>,
) -> PyResult<bool> {
    let limit = match maxPitch {
        Some(pitch) => pitch_from_any(pitch)?,
        None => rs::DEFAULT_MAX_PITCH.clone(),
    };
    Ok(rs::pitches_within_limit(&pitches(possibA)?, &limit))
}

#[pyfunction]
#[pyo3(signature = (possibA, partPitchLimits = None))]
fn limitPartToPitch(
    possibA: &Bound<'_, PyAny>,
    partPitchLimits: Option<&Bound<'_, PyDict>>,
) -> PyResult<bool> {
    let mut limits = Vec::new();
    if let Some(given) = partPitchLimits {
        for (part, pitch) in given.iter() {
            limits.push((part.extract::<usize>()?, pitch_from_any(&pitch)?));
        }
    }
    rs::limit_part_to_pitch(&pitches(possibA)?, &limits).map_err(possibility_error)
}

#[pyfunction]
fn parallelFifths(possibA: &Bound<'_, PyAny>, possibB: &Bound<'_, PyAny>) -> PyResult<bool> {
    let (a, b) = paired(possibA, possibB)?;
    rs::parallel_fifths(&a, &b).map_err(possibility_error)
}

#[pyfunction]
fn parallelOctaves(possibA: &Bound<'_, PyAny>, possibB: &Bound<'_, PyAny>) -> PyResult<bool> {
    let (a, b) = paired(possibA, possibB)?;
    rs::parallel_octaves(&a, &b).map_err(possibility_error)
}

#[pyfunction]
fn hiddenFifths(possibA: &Bound<'_, PyAny>, possibB: &Bound<'_, PyAny>) -> PyResult<bool> {
    let (a, b) = paired(possibA, possibB)?;
    rs::hidden_fifths(&a, &b).map_err(possibility_error)
}

#[pyfunction]
fn hiddenOctaves(possibA: &Bound<'_, PyAny>, possibB: &Bound<'_, PyAny>) -> PyResult<bool> {
    let (a, b) = paired(possibA, possibB)?;
    rs::hidden_octaves(&a, &b).map_err(possibility_error)
}

#[pyfunction]
fn voiceOverlap(possibA: &Bound<'_, PyAny>, possibB: &Bound<'_, PyAny>) -> PyResult<bool> {
    let (a, b) = paired(possibA, possibB)?;
    rs::voice_overlap(&a, &b).map_err(possibility_error)
}

#[pyfunction]
#[pyo3(signature = (possibA, possibB, partMovementLimits = None))]
fn partMovementsWithinLimits(
    possibA: &Bound<'_, PyAny>,
    possibB: &Bound<'_, PyAny>,
    partMovementLimits: Option<&Bound<'_, PyAny>>,
) -> PyResult<bool> {
    let mut limits = Vec::new();
    if let Some(given) = partMovementLimits.filter(|given| !given.is_none()) {
        for limit in given.walk()? {
            limits.push(crate::fbrules::pair::<usize, i32>(&limit?)?);
        }
    }
    let (a, b) = (pitches(possibA)?, pitches(possibB)?);
    rs::part_movements_within_limits(&a, &b, &limits).map_err(possibility_error)
}

#[pyfunction]
fn upperPartsSame(possibA: &Bound<'_, PyAny>, possibB: &Bound<'_, PyAny>) -> PyResult<bool> {
    let (a, b) = paired(possibA, possibB)?;
    rs::upper_parts_same(&a, &b).map_err(possibility_error)
}

#[pyfunction]
#[pyo3(signature = (possibA, possibB, partsToCheck = None))]
fn partsSame(
    possibA: &Bound<'_, PyAny>,
    possibB: &Bound<'_, PyAny>,
    partsToCheck: Option<Vec<usize>>,
) -> PyResult<bool> {
    if partsToCheck.is_none() {
        return Ok(true);
    }
    let (a, b) = paired(possibA, possibB)?;
    rs::parts_same(&a, &b, partsToCheck.as_deref()).map_err(possibility_error)
}

#[pyfunction]
#[pyo3(signature = (possibA, possibB, threePartChordInfo = None, restrictDoublings = true))]
fn couldBeItalianA6Resolution(
    possibA: &Bound<'_, PyAny>,
    possibB: &Bound<'_, PyAny>,
    threePartChordInfo: Option<&Bound<'_, PyAny>>,
    restrictDoublings: bool,
) -> PyResult<bool> {
    let sixth = match threePartChordInfo.filter(|given| !given.is_none()) {
        Some(given) => {
            let [bass, root, third, fifth]: [RsPitch; 4] =
                pitches(given)?.try_into().map_err(|_| {
                    pyo3::exceptions::PyValueError::new_err(
                        "threePartChordInfo is a bass, a root, a third and a fifth",
                    )
                })?;
            Some(rs::ItalianSixth {
                bass,
                root,
                third,
                fifth,
            })
        }
        None => None,
    };
    rs::could_be_italian_a6_resolution(
        &pitches(possibA)?,
        &pitches(possibB)?,
        sixth.as_ref(),
        restrictDoublings,
    )
    .map_err(possibility_error)
}

/// Each part's pitch in `possibA` beside its pitch in `possibB`: the objects
/// given, paired.
#[pyfunction]
fn partPairs<'py>(
    possibA: &Bound<'py, PyAny>,
    possibB: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let py = possibA.py();
    let a: Vec<Bound<'py, PyAny>> = possibA.walk()?.collect::<PyResult<_>>()?;
    let b: Vec<Bound<'py, PyAny>> = possibB.walk()?.collect::<PyResult<_>>()?;
    if a.len() != b.len() {
        return Err(unpaired(a.len(), b.len()));
    }
    let pairs = PyList::empty(py);
    for (from, to) in a.into_iter().zip(b) {
        pairs.append(PyTuple::new(py, [from, to])?)?;
    }
    Ok(pairs)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(voiceCrossing, m)?)?;
    m.add_function(wrap_pyfunction!(isIncomplete, m)?)?;
    m.add_function(wrap_pyfunction!(upperPartsWithinLimit, m)?)?;
    m.add_function(wrap_pyfunction!(pitchesWithinLimit, m)?)?;
    m.add_function(wrap_pyfunction!(limitPartToPitch, m)?)?;
    m.add_function(wrap_pyfunction!(parallelFifths, m)?)?;
    m.add_function(wrap_pyfunction!(parallelOctaves, m)?)?;
    m.add_function(wrap_pyfunction!(hiddenFifths, m)?)?;
    m.add_function(wrap_pyfunction!(hiddenOctaves, m)?)?;
    m.add_function(wrap_pyfunction!(voiceOverlap, m)?)?;
    m.add_function(wrap_pyfunction!(partMovementsWithinLimits, m)?)?;
    m.add_function(wrap_pyfunction!(upperPartsSame, m)?)?;
    m.add_function(wrap_pyfunction!(partsSame, m)?)?;
    m.add_function(wrap_pyfunction!(couldBeItalianA6Resolution, m)?)?;
    m.add_function(wrap_pyfunction!(partPairs, m)?)?;
    Ok(())
}
