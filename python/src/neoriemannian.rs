//! music21's `analysis.neoRiemannian`, over the crate's.
//!
//! Each function reads the chord it is given, asks the crate for the
//! transformation, and hands back a new chord of new pitches carrying the
//! given chord's `quarterLength`, as music21 does.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21_rs_crate::Chord as RsChord;
use music21_rs_crate::analysis::neoriemannian::{
    self as rs, ChainOrder, Mediant, MediantSide, Respell, Transform,
};

use crate::chord::{Chord, chord_from_any};
use crate::pitch::Pitch;

pyo3::create_exception!(music21_rs_facade, LRPException, crate::Music21Exception);

error_into!(lrp_error, LRPException);

/// The names this facade replaces in `music21.analysis.neoRiemannian`.
pub const NAMES: &[&str] = &[
    "L",
    "P",
    "R",
    "S",
    "N",
    "isNeoR",
    "isChromaticMediant",
    "LRP_combinations",
    "completeHexatonic",
    "hexatonicSystem",
    "chromaticMediants",
    "disjunctMediants",
    "_simplerEnharmonics",
    "LRPException",
];

/// The crate chord a Python chord holds, or the one its contents spell.
fn chord_of(value: &Bound<'_, PyAny>) -> PyResult<RsChord> {
    if let Ok(chord) = value.extract::<PyRef<Chord>>() {
        return Ok(chord.inner.clone());
    }
    chord_from_any(Some(value))
}

/// A new Python chord of `chord`'s pitches, lasting as long as `source`.
fn chord_object<'py>(source: &Bound<'py, PyAny>, chord: RsChord) -> PyResult<Bound<'py, PyAny>> {
    let py = source.py();

    let mut pitches = Vec::with_capacity(chord.pitches().len());
    for pitch in chord.pitches() {
        let inferred = pitch.spelling_is_inferred();
        pitches.push(crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            Pitch::wrap(pitch, inferred),
        )?);
    }

    let class = crate::installed_class(py, "music21.chord", "Chord")
        .unwrap_or_else(|| py.get_type::<Chord>().into_any());
    let keywords = PyDict::new(py);
    keywords.set_item("quarterLength", source.getattr("quarterLength")?)?;
    class.call((PyList::new(py, pitches)?,), Some(&keywords))
}

/// Python chords for each of `chords`, each lasting as long as `source`.
fn chord_list<'py>(
    source: &Bound<'py, PyAny>,
    chords: Vec<RsChord>,
) -> PyResult<Bound<'py, PyList>> {
    let objects = chords
        .into_iter()
        .map(|chord| chord_object(source, chord))
        .collect::<PyResult<Vec<_>>>()?;
    PyList::new(source.py(), objects)
}

/// Whether a chord is one the transformations can start from.
fn is_triad(chord: &RsChord) -> bool {
    chord.is_major_triad() || chord.is_minor_triad()
}

/// One transformation, or `c` itself where music21 is told not to raise.
fn transform<'py>(
    c: &Bound<'py, PyAny>,
    transform: Transform,
    raise_exception: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let chord = chord_of(c)?;
    if !raise_exception && !is_triad(&chord) {
        return Ok(c.clone());
    }

    let made = transform.apply(&chord).map_err(lrp_error)?;
    chord_object(c, made)
}

/// The transformations a string of letters names, up to the first letter
/// that names none, and that letter's refusal if there is one. music21
/// refuses a letter only when it reaches it.
fn transforms_until_refused(symbols: &str) -> (Vec<Transform>, Option<PyErr>) {
    let mut found = Vec::with_capacity(symbols.len());
    for symbol in symbols.chars() {
        match Transform::from_symbol(symbol) {
            Ok(transform) => found.push(transform),
            Err(error) => return (found, Some(lrp_error(error))),
        }
    }
    (found, None)
}

/// A string of letters as transformations, refusing any letter that names
/// none.
fn transforms_of(symbols: &str) -> PyResult<Vec<Transform>> {
    Transform::parse_chain(symbols).map_err(lrp_error)
}

#[pyfunction]
#[pyo3(signature = (c, raiseException = true))]
fn L<'py>(c: &Bound<'py, PyAny>, raiseException: bool) -> PyResult<Bound<'py, PyAny>> {
    transform(c, Transform::L, raiseException)
}

#[pyfunction]
#[pyo3(signature = (c, raiseException = true))]
fn P<'py>(c: &Bound<'py, PyAny>, raiseException: bool) -> PyResult<Bound<'py, PyAny>> {
    transform(c, Transform::P, raiseException)
}

#[pyfunction]
#[pyo3(signature = (c, raiseException = true))]
fn R<'py>(c: &Bound<'py, PyAny>, raiseException: bool) -> PyResult<Bound<'py, PyAny>> {
    transform(c, Transform::R, raiseException)
}

#[pyfunction]
fn S<'py>(c: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let made = rs::slide(&chord_of(c)?).map_err(lrp_error)?;
    chord_object(c, made)
}

#[pyfunction]
fn N<'py>(c: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let made = rs::nebenverwandt(&chord_of(c)?).map_err(lrp_error)?;
    chord_object(c, made)
}

#[pyfunction]
fn _simplerEnharmonics<'py>(c: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let made = rs::simpler_enharmonics(&chord_of(c)?).map_err(lrp_error)?;
    chord_object(c, made)
}

/// The letter of the transformation relating the chords, or `False`.
#[pyfunction]
#[pyo3(signature = (c1, c2, transforms = "LRP"))]
fn isNeoR(c1: &Bound<'_, PyAny>, c2: &Bound<'_, PyAny>, transforms: &str) -> PyResult<Py<PyAny>> {
    let py = c1.py();
    let (valid, refused) = transforms_until_refused(transforms);

    let found = rs::is_neo_r(&chord_of(c1)?, &chord_of(c2)?, &valid).map_err(lrp_error)?;
    if let Some(found) = found {
        return Ok(found
            .symbol()
            .to_string()
            .into_pyobject(py)?
            .into_any()
            .unbind());
    }
    if let Some(refused) = refused {
        return Err(refused);
    }
    Ok(false.into_pyobject(py)?.to_owned().into_any().unbind())
}

/// The abbreviation of the mediant relating the chords, or `False`.
#[pyfunction]
fn isChromaticMediant(c1: &Bound<'_, PyAny>, c2: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = c1.py();
    let found = rs::is_chromatic_mediant(&chord_of(c1)?, &chord_of(c2)?).map_err(lrp_error)?;
    match found {
        Some(mediant) => Ok(mediant.code().into_pyobject(py)?.into_any().unbind()),
        None => Ok(false.into_pyobject(py)?.to_owned().into_any().unbind()),
    }
}

#[pyfunction]
#[pyo3(signature = (
    c,
    transformationString,
    raiseException = true,
    leftOrdered = false,
    simplifyEnharmonics = false,
    eachOne = false,
))]
fn LRP_combinations<'py>(
    c: &Bound<'py, PyAny>,
    transformationString: &str,
    raiseException: bool,
    leftOrdered: bool,
    simplifyEnharmonics: bool,
    eachOne: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let chord = chord_of(c)?;
    if !raiseException && !is_triad(&chord) {
        return Ok(c.clone());
    }

    let transforms = transforms_of(transformationString)?;
    let order = if leftOrdered {
        ChainOrder::RightToLeft
    } else {
        ChainOrder::LeftToRight
    };
    let respell = if simplifyEnharmonics {
        Respell::Simplify
    } else {
        Respell::Keep
    };

    if eachOne {
        let chain = rs::lrp_chain(&chord, &transforms, order, respell).map_err(lrp_error)?;
        return Ok(chord_list(c, chain)?.into_any());
    }

    // No transformations at all hand back `c` itself, as music21's loop
    // leaves it.
    if transforms.is_empty() && respell == Respell::Keep {
        return Ok(c.clone());
    }
    let made = rs::lrp_combination(&chord, &transforms, order, respell).map_err(lrp_error)?;
    chord_object(c, made)
}

#[pyfunction]
#[pyo3(signature = (c, simplifyEnharmonics = false, raiseException = true))]
fn completeHexatonic<'py>(
    c: &Bound<'py, PyAny>,
    simplifyEnharmonics: bool,
    raiseException: bool,
) -> PyResult<Option<Bound<'py, PyList>>> {
    let respell = if simplifyEnharmonics {
        Respell::Simplify
    } else {
        Respell::Keep
    };

    match rs::complete_hexatonic(&chord_of(c)?, respell) {
        Ok(cycle) => Ok(Some(chord_list(c, cycle)?)),
        Err(_) if !raiseException => Ok(None),
        Err(error) => Err(lrp_error(error)),
    }
}

#[pyfunction]
fn hexatonicSystem(c: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    let system = rs::hexatonic_system(&chord_of(c)?).map_err(lrp_error)?;
    Ok(system.name())
}

#[pyfunction]
#[pyo3(signature = (c, transformation = "UFM"))]
fn chromaticMediants<'py>(
    c: &Bound<'py, PyAny>,
    transformation: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let mediant = Mediant::from_code(transformation).map_err(lrp_error)?;
    let made = rs::chromatic_mediant(&chord_of(c)?, mediant).map_err(lrp_error)?;
    chord_object(c, made)
}

#[pyfunction]
#[pyo3(signature = (c, upperOrLower = "upper"))]
fn disjunctMediants<'py>(c: &Bound<'py, PyAny>, upperOrLower: &str) -> PyResult<Bound<'py, PyAny>> {
    let side = match upperOrLower {
        "upper" => MediantSide::Upper,
        "lower" => MediantSide::Lower,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "upperOrLower must be one of ['upper', 'lower']",
            ));
        }
    };
    let made = rs::disjunct_mediant(&chord_of(c)?, side).map_err(lrp_error)?;
    chord_object(c, made)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(L, m)?)?;
    m.add_function(wrap_pyfunction!(P, m)?)?;
    m.add_function(wrap_pyfunction!(R, m)?)?;
    m.add_function(wrap_pyfunction!(S, m)?)?;
    m.add_function(wrap_pyfunction!(N, m)?)?;
    m.add_function(wrap_pyfunction!(_simplerEnharmonics, m)?)?;
    m.add_function(wrap_pyfunction!(isNeoR, m)?)?;
    m.add_function(wrap_pyfunction!(isChromaticMediant, m)?)?;
    m.add_function(wrap_pyfunction!(LRP_combinations, m)?)?;
    m.add_function(wrap_pyfunction!(completeHexatonic, m)?)?;
    m.add_function(wrap_pyfunction!(hexatonicSystem, m)?)?;
    m.add_function(wrap_pyfunction!(chromaticMediants, m)?)?;
    m.add_function(wrap_pyfunction!(disjunctMediants, m)?)?;
    Ok(())
}
