//! music21's `analysis` package, over the crate's: the readings of a line
//! that are functions rather than classes.
//!
//! `analysis.patel`'s two measures of how even a rhythm and a melody are,
//! and `analysis.metrical`'s two labellings of a line -- the beat each
//! note falls on, and Thomassen's melodic accent. The classes around them
//! (`KeyWeightKeyAnalysis`, `Ambitus`, `WindowedAnalysis`, `KeyAnalyzer`)
//! are colour tables and logs of solutions around arithmetic the crate
//! already has; `Stream.analyze` is the way to the key and the ambitus.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "nPVI",
    "melodicIntervalVariability",
    "labelBeatDepth",
    "thomassenMelodicAccent",
];

/// music21's `analysis.patel.nPVI`: the normalized pairwise variability
/// index of a line's note lengths, which says how even its rhythm is.
#[pyfunction]
fn nPVI(streamForAnalysis: &Bound<'_, PyAny>) -> PyResult<f64> {
    let read = crate::stream::crate_stream(streamForAnalysis)?;
    music21_rs_crate::analysis::patel::n_pvi(&read)
        .map_err(|error| crate::stream::stream_error(&error))
}

/// music21's `analysis.patel.melodicIntervalVariability`: how varied the
/// steps of a melody are, as the standard deviation of their semitones.
///
/// The keywords are `findConsecutiveNotes`', which is where music21 passes
/// them on to.
#[pyfunction]
#[pyo3(signature = (streamForAnalysis, **skipKeywords))]
fn melodicIntervalVariability(
    streamForAnalysis: &Bound<'_, PyAny>,
    skipKeywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<f64> {
    let read = crate::stream::crate_stream(streamForAnalysis)?;
    let options = crate::stream::consecutive_options(skipKeywords)?;
    music21_rs_crate::analysis::patel::melodic_interval_variability(&read, &options)
        .map_err(|error| crate::stream::stream_error(&error))
}

/// music21's `analysis.metrical.labelBeatDepth`: a lyric under each note
/// saying how deep in the beat hierarchy it falls.
///
/// The answer is a new stream, as every edit in this package is; music21
/// writes the lyrics onto the part it is given.
#[pyfunction]
fn labelBeatDepth<'py>(streamIn: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    crate::stream::changed(streamIn, false, |stream| {
        music21_rs_crate::analysis::metrical::label_beat_depth(stream)
    })
}

/// music21's `analysis.metrical.thomassenMelodicAccent`: the melodic
/// accent of each note of a line, written onto the notes themselves.
///
/// music21 writes `melodicAccent` onto every note's `editorial` and
/// answers nothing, and so does this -- onto the stream's own notes,
/// which is what the accent is for.
#[pyfunction]
fn thomassenMelodicAccent(streamIn: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = streamIn.py();
    let read = crate::stream::read_with_leaves(streamIn)?;
    let accents = music21_rs_crate::analysis::metrical::thomassen_melodic_accent(&read.0)
        .map_err(|error| crate::stream::stream_error(&error))?;
    for (position, accent) in accents.iter().enumerate() {
        if let Some(object) = read.1.get(position) {
            object
                .bind(py)
                .getattr("editorial")?
                .setattr("melodicAccent", *accent)?;
        }
    }
    Ok(())
}

/// Adds the analysis functions to a module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(nPVI, m)?)?;
    m.add_function(wrap_pyfunction!(melodicIntervalVariability, m)?)?;
    m.add_function(wrap_pyfunction!(labelBeatDepth, m)?)?;
    m.add_function(wrap_pyfunction!(thomassenMelodicAccent, m)?)?;
    Ok(())
}
