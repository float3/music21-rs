//! music21's `analysis` package, over the crate's: the readings of a line
//! that are functions rather than classes.
//!
//! `analysis.patel`'s two measures of how even a rhythm and a melody are,
//! `analysis.metrical`'s two labellings of a line -- the beat each note
//! falls on, and Thomassen's melodic accent -- `analysis.pitchAnalysis`'s
//! count of a property of every pitch, and `analysis.segmentByRests`'s
//! `Segmenter`, which cuts a line into the runs of notes between its
//! rests. A segment is the stream's own notes, as every lookup here hands
//! back what the stream holds.
//!
//! The classes still outside it (`KeyWeightKeyAnalysis`, `Ambitus`,
//! `WindowedAnalysis`, `KeyAnalyzer`) are colour tables and logs of
//! solutions around arithmetic the crate already has; `Stream.analyze` is
//! the way to the key and the ambitus.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "nPVI",
    "melodicIntervalVariability",
    "labelBeatDepth",
    "thomassenMelodicAccent",
    "pitchAttributeCount",
    "Segmenter",
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

/// music21's `analysis.pitchAnalysis.pitchAttributeCount`: how often each
/// value of some property of a stream's pitches comes.
///
/// The pitches are the stream's own `pitches`, and the property is read off
/// each by name, as music21 reads it, so any property a pitch answers may
/// be counted. The answer is a `collections.Counter`, which is what
/// music21 answers, so `most_common` orders ties by which value was met
/// first.
#[pyfunction]
#[pyo3(signature = (s, pitchAttr = "name".to_string()))]
fn pitchAttributeCount<'py>(
    s: &Bound<'py, PyAny>,
    pitchAttr: String,
) -> PyResult<Bound<'py, PyAny>> {
    let py = s.py();
    let counts = py.import("collections")?.getattr("Counter")?.call0()?;
    for pitch in s.getattr("pitches")?.walk()? {
        let value = pitch?.getattr(pitchAttr.as_str())?;
        let so_far = match counts.get_item(&value) {
            Ok(so_far) => so_far.extract::<usize>()?,
            Err(_) => 0,
        };
        counts.set_item(&value, so_far + 1)?;
    }
    Ok(counts)
}

/// music21's `analysis.segmentByRests.Segmenter`: a line cut into the runs
/// of notes between its rests.
///
/// Both answers are the stream's own objects -- a segment is the very notes
/// it holds, and an interval is built on two of them, so `noteStart` is one
/// of the score's notes. Only single notes count, as music21's walk
/// collects notes, rests and clefs and nothing else: a chord between two
/// notes neither joins a run nor breaks one.
#[pyclass(
    name = "Segmenter",
    module = "music21.analysis.segmentByRests",
    subclass,
    skip_from_py_object
)]
pub struct Segmenter;

#[pymethods]
impl Segmenter {
    #[new]
    fn make() -> Self {
        Self
    }

    /// The runs of notes between rests and clefs, each a list of the
    /// stream's own notes: music21's `getSegmentsList`.
    ///
    /// With `removeEmptyLists` off, a rest or clef standing after another
    /// gives an empty run, as music21's does.
    #[classmethod]
    #[pyo3(signature = (workOrPart, removeEmptyLists = true))]
    fn getSegmentsList<'py>(
        _cls: &Bound<'py, pyo3::types::PyType>,
        workOrPart: &Bound<'py, PyAny>,
        removeEmptyLists: bool,
    ) -> PyResult<Vec<Vec<Py<PyAny>>>> {
        let py = workOrPart.py();
        let (stream, leaves) = crate::stream::read_with_leaves(workOrPart)?;
        Ok(
            music21_rs_crate::analysis::segment_by_rests::segments_list(&stream, removeEmptyLists)
                .into_iter()
                .map(|run| {
                    run.into_iter()
                        .filter_map(|position| {
                            leaves.get(position).map(|object| object.clone_ref(py))
                        })
                        .collect()
                })
                .collect(),
        )
    }

    /// The interval between each two notes standing next to each other with
    /// no rest or clef between them: music21's `getIntervalList`.
    #[classmethod]
    fn getIntervalList<'py>(
        _cls: &Bound<'py, pyo3::types::PyType>,
        workOrPart: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = workOrPart.py();
        let class = crate::class_of(py, "music21.interval", "Interval")?;
        let mut intervals = Vec::new();
        for run in Self::getSegmentsList(&py.get_type::<Self>(), workOrPart, true)? {
            for pair in run.windows(2) {
                intervals.push(class.call1((pair[0].bind(py), pair[1].bind(py)))?);
            }
        }
        Ok(intervals)
    }
}

/// Adds the analysis functions and classes to a module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(nPVI, m)?)?;
    m.add_function(wrap_pyfunction!(melodicIntervalVariability, m)?)?;
    m.add_function(wrap_pyfunction!(labelBeatDepth, m)?)?;
    m.add_function(wrap_pyfunction!(thomassenMelodicAccent, m)?)?;
    m.add_function(wrap_pyfunction!(pitchAttributeCount, m)?)?;
    m.add_class::<Segmenter>()?;
    Ok(())
}
