//! music21's `figuredBass.resolution`, over the crate's.
//!
//! Each function reads a possibility's pitches, asks the crate how the chord
//! resolves, and hands back the resolved possibility as a tuple of new
//! pitches. `showResolutions` draws a score and stays music21's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use music21_rs_crate::figuredbass::resolution::{self as rs, AugmentedSixth, ChordInfo};
use music21_rs_crate::{Error as RsError, Pitch as RsPitch};

use crate::pitch::{Pitch, pitch_from_any};

pyo3::create_exception!(
    music21_rs_facade,
    ResolutionException,
    crate::Music21Exception
);

error_into!(resolution_error, ResolutionException);

/// The names this facade replaces in `music21.figuredBass.resolution`.
pub const NAMES: &[&str] = &[
    "augmentedSixthToDominant",
    "augmentedSixthToMajorTonic",
    "augmentedSixthToMinorTonic",
    "dominantSeventhToMajorTonic",
    "dominantSeventhToMinorTonic",
    "dominantSeventhToMajorSubmediant",
    "dominantSeventhToMinorSubmediant",
    "dominantSeventhToMajorSubdominant",
    "dominantSeventhToMinorSubdominant",
    "diminishedSeventhToMajorTonic",
    "diminishedSeventhToMinorTonic",
    "diminishedSeventhToMajorSubdominant",
    "diminishedSeventhToMinorSubdominant",
    "ResolutionException",
];

fn pitches(possibility: &Bound<'_, PyAny>) -> PyResult<Vec<RsPitch>> {
    possibility
        .try_iter()?
        .map(|pitch| pitch_from_any(&pitch?))
        .collect()
}

/// A chord's notes as music21 hands them over: a list of its bass, root,
/// third, fifth and seventh, any of which may be `None`.
pub(crate) fn chord_info(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<ChordInfo>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let mut notes: Vec<Option<RsPitch>> = Vec::new();
    for note in value.try_iter()? {
        let note = note?;
        notes.push(if note.is_none() {
            None
        } else {
            Some(pitch_from_any(&note)?)
        });
    }
    notes.resize(5, None);
    let mut notes = notes.into_iter();
    let mut next = || notes.next().flatten();
    Ok(Some(ChordInfo {
        bass: next(),
        root: next(),
        third: next(),
        fifth: next(),
        seventh: next(),
    }))
}

/// A resolved possibility as music21 hands one back: a tuple of pitches.
pub(crate) fn possibility_object<'py>(
    py: Python<'py>,
    pitches: Vec<RsPitch>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut objects = Vec::with_capacity(pitches.len());
    for pitch in pitches {
        let inferred = pitch.spelling_is_inferred();
        objects.push(crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            Pitch::wrap(pitch, inferred),
        )?);
    }
    PyTuple::new(py, objects)
}

/// The kind of augmented sixth music21 numbers so, refused as music21
/// refuses a number it has no rule for.
fn sixth_kind(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<AugmentedSixth>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    value
        .extract::<u8>()
        .ok()
        .and_then(AugmentedSixth::from_number)
        .map(Some)
        .ok_or_else(|| {
            ResolutionException::new_err(format!(
                "Unknown augSixthType: {}",
                value
                    .repr()
                    .map_or_else(|_| "?".to_string(), |repr| repr.to_string())
            ))
        })
}

type Resolved<'py> = PyResult<Bound<'py, PyTuple>>;

fn answer<'py>(py: Python<'py>, resolved: Result<Vec<RsPitch>, RsError>) -> Resolved<'py> {
    possibility_object(py, resolved.map_err(resolution_error)?)
}

#[pyfunction]
#[pyo3(signature = (augSixthPossib, augSixthType = None, augSixthChordInfo = None))]
fn augmentedSixthToDominant<'py>(
    augSixthPossib: &Bound<'py, PyAny>,
    augSixthType: Option<&Bound<'py, PyAny>>,
    augSixthChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(augSixthChordInfo)?;
    answer(
        augSixthPossib.py(),
        rs::augmented_sixth_to_dominant(
            &pitches(augSixthPossib)?,
            sixth_kind(augSixthType)?,
            info.as_ref(),
        ),
    )
}

#[pyfunction]
#[pyo3(signature = (augSixthPossib, augSixthType = None, augSixthChordInfo = None))]
fn augmentedSixthToMajorTonic<'py>(
    augSixthPossib: &Bound<'py, PyAny>,
    augSixthType: Option<&Bound<'py, PyAny>>,
    augSixthChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(augSixthChordInfo)?;
    answer(
        augSixthPossib.py(),
        rs::augmented_sixth_to_major_tonic(
            &pitches(augSixthPossib)?,
            sixth_kind(augSixthType)?,
            info.as_ref(),
        ),
    )
}

#[pyfunction]
#[pyo3(signature = (augSixthPossib, augSixthType = None, augSixthChordInfo = None))]
fn augmentedSixthToMinorTonic<'py>(
    augSixthPossib: &Bound<'py, PyAny>,
    augSixthType: Option<&Bound<'py, PyAny>>,
    augSixthChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(augSixthChordInfo)?;
    answer(
        augSixthPossib.py(),
        rs::augmented_sixth_to_minor_tonic(
            &pitches(augSixthPossib)?,
            sixth_kind(augSixthType)?,
            info.as_ref(),
        ),
    )
}

#[pyfunction]
#[pyo3(signature = (domPossib, resolveV43toI6 = false, domChordInfo = None))]
fn dominantSeventhToMajorTonic<'py>(
    domPossib: &Bound<'py, PyAny>,
    resolveV43toI6: bool,
    domChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(domChordInfo)?;
    answer(
        domPossib.py(),
        rs::dominant_seventh_to_major_tonic(&pitches(domPossib)?, resolveV43toI6, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (domPossib, resolveV43toi6 = false, domChordInfo = None))]
fn dominantSeventhToMinorTonic<'py>(
    domPossib: &Bound<'py, PyAny>,
    resolveV43toi6: bool,
    domChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(domChordInfo)?;
    answer(
        domPossib.py(),
        rs::dominant_seventh_to_minor_tonic(&pitches(domPossib)?, resolveV43toi6, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (domPossib, domChordInfo = None))]
fn dominantSeventhToMajorSubmediant<'py>(
    domPossib: &Bound<'py, PyAny>,
    domChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(domChordInfo)?;
    answer(
        domPossib.py(),
        rs::dominant_seventh_to_major_submediant(&pitches(domPossib)?, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (domPossib, domChordInfo = None))]
fn dominantSeventhToMinorSubmediant<'py>(
    domPossib: &Bound<'py, PyAny>,
    domChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(domChordInfo)?;
    answer(
        domPossib.py(),
        rs::dominant_seventh_to_minor_submediant(&pitches(domPossib)?, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (domPossib, domChordInfo = None))]
fn dominantSeventhToMajorSubdominant<'py>(
    domPossib: &Bound<'py, PyAny>,
    domChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(domChordInfo)?;
    answer(
        domPossib.py(),
        rs::dominant_seventh_to_major_subdominant(&pitches(domPossib)?, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (domPossib, domChordInfo = None))]
fn dominantSeventhToMinorSubdominant<'py>(
    domPossib: &Bound<'py, PyAny>,
    domChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(domChordInfo)?;
    answer(
        domPossib.py(),
        rs::dominant_seventh_to_minor_subdominant(&pitches(domPossib)?, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (dimPossib, doubledRoot = false, dimChordInfo = None))]
fn diminishedSeventhToMajorTonic<'py>(
    dimPossib: &Bound<'py, PyAny>,
    doubledRoot: bool,
    dimChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(dimChordInfo)?;
    answer(
        dimPossib.py(),
        rs::diminished_seventh_to_major_tonic(&pitches(dimPossib)?, doubledRoot, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (dimPossib, doubledRoot = false, dimChordInfo = None))]
fn diminishedSeventhToMinorTonic<'py>(
    dimPossib: &Bound<'py, PyAny>,
    doubledRoot: bool,
    dimChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(dimChordInfo)?;
    answer(
        dimPossib.py(),
        rs::diminished_seventh_to_minor_tonic(&pitches(dimPossib)?, doubledRoot, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (dimPossib, dimChordInfo = None))]
fn diminishedSeventhToMajorSubdominant<'py>(
    dimPossib: &Bound<'py, PyAny>,
    dimChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(dimChordInfo)?;
    answer(
        dimPossib.py(),
        rs::diminished_seventh_to_major_subdominant(&pitches(dimPossib)?, info.as_ref()),
    )
}

#[pyfunction]
#[pyo3(signature = (dimPossib, dimChordInfo = None))]
fn diminishedSeventhToMinorSubdominant<'py>(
    dimPossib: &Bound<'py, PyAny>,
    dimChordInfo: Option<&Bound<'py, PyAny>>,
) -> Resolved<'py> {
    let info = chord_info(dimChordInfo)?;
    answer(
        dimPossib.py(),
        rs::diminished_seventh_to_minor_subdominant(&pitches(dimPossib)?, info.as_ref()),
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(augmentedSixthToDominant, m)?)?;
    m.add_function(wrap_pyfunction!(augmentedSixthToMajorTonic, m)?)?;
    m.add_function(wrap_pyfunction!(augmentedSixthToMinorTonic, m)?)?;
    m.add_function(wrap_pyfunction!(dominantSeventhToMajorTonic, m)?)?;
    m.add_function(wrap_pyfunction!(dominantSeventhToMinorTonic, m)?)?;
    m.add_function(wrap_pyfunction!(dominantSeventhToMajorSubmediant, m)?)?;
    m.add_function(wrap_pyfunction!(dominantSeventhToMinorSubmediant, m)?)?;
    m.add_function(wrap_pyfunction!(dominantSeventhToMajorSubdominant, m)?)?;
    m.add_function(wrap_pyfunction!(dominantSeventhToMinorSubdominant, m)?)?;
    m.add_function(wrap_pyfunction!(diminishedSeventhToMajorTonic, m)?)?;
    m.add_function(wrap_pyfunction!(diminishedSeventhToMinorTonic, m)?)?;
    m.add_function(wrap_pyfunction!(diminishedSeventhToMajorSubdominant, m)?)?;
    m.add_function(wrap_pyfunction!(diminishedSeventhToMinorSubdominant, m)?)?;
    Ok(())
}
