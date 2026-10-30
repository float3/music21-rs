//! music21's `search.segment`: a part's notes written as text and cut into
//! overlapping stretches, and every stretch of some scores compared with
//! every other.
//!
//! The cutting, the translating and the comparing are all the crate's
//! (`search::notes_sorted`, the `translate_*` functions,
//! `cut_translation_into_segments` and `score_similarity`); the facade
//! reads a music21 stream into the crate and hands back music21's own
//! shapes -- a pair of lists for a part, a dictionary per part of a score,
//! and the nine-part tuples `scoreSimilarity` answers.
//!
//! What is not offered is said rather than answered differently:
//! music21's `jitter`, which moves each stretch's start at random, and a
//! comparison by `Levenshtein` where that library is installed, since the
//! crate carries `difflib`'s ratio alone.
//!
//! `indexScoreFilePaths`, `indexOnePath`, `saveScoreDict` and
//! `loadScoreDict` stay music21's: they read and write files, which this
//! crate does not do. They reach the crate all the same, each going
//! through `indexScoreParts`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::IntegerType;
use music21_rs_crate::error::Result as RsResult;
use music21_rs_crate::search::{
    Searched, Segments, Translation, cut_translation_into_segments, notes_sorted, score_similarity,
    translate_diatonic_stream_to_string, translate_intervals_and_speed, translate_stream_to_string,
    translate_stream_to_string_no_rhythm, translate_stream_to_string_only_rhythm,
};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "translateMonophonicPartToSegments",
    "indexScoreParts",
    "scoreSimilarity",
];

/// Which translation a caller asked for: music21's `algorithm`, named by
/// the function given.
#[derive(Clone, Copy)]
enum Translator {
    /// `translateStreamToString`: the pitch and the length of each note.
    Plain,
    /// `translateStreamToStringNoRhythm`: the pitches alone, music21's
    /// default here.
    NoRhythm,
    /// `translateStreamToStringOnlyRhythm`: the lengths alone.
    OnlyRhythm,
    /// `translateDiatonicStreamToString`: each note as the step it moves
    /// by from the one before.
    Diatonic,
    /// `translateIntervalsAndSpeed`: the same, and how the length changed.
    IntervalsAndSpeed,
}

impl Translator {
    /// The translation a function stands for, read by its name so that
    /// music21's own function and this package's answer alike.
    fn of(algorithm: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(algorithm) = algorithm.filter(|given| !given.is_none()) else {
            return Ok(Self::NoRhythm);
        };
        let named: String = algorithm
            .getattr("__name__")
            .and_then(|name| name.extract())
            .unwrap_or_default();
        Ok(match named.as_str() {
            "translateStreamToString" => Self::Plain,
            "translateStreamToStringNoRhythm" => Self::NoRhythm,
            "translateStreamToStringOnlyRhythm" => Self::OnlyRhythm,
            "translateDiatonicStreamToString" => Self::Diatonic,
            "translateIntervalsAndSpeed" => Self::IntervalsAndSpeed,
            _ => {
                return Err(crate::stream::StreamException::new_err(format!(
                    "{named} is not a translation this package carries"
                )));
            }
        })
    }

    fn run(self, notes: &[Searched<'_>]) -> RsResult<Translation> {
        Ok(match self {
            Self::Plain => translate_stream_to_string(notes),
            Self::NoRhythm => translate_stream_to_string_no_rhythm(notes),
            Self::OnlyRhythm => translate_stream_to_string_only_rhythm(notes),
            Self::Diatonic => translate_diatonic_stream_to_string(notes)?,
            Self::IntervalsAndSpeed => translate_intervals_and_speed(notes)?,
        })
    }
}

/// One part cut into segments, as the crate cuts it.
fn cut(
    part: &Bound<'_, PyAny>,
    segmentLengths: usize,
    overlap: usize,
    translator: Translator,
) -> PyResult<Segments> {
    let (stream, _) = crate::stream::read_with_leaves(part)?;
    let notes = notes_sorted(&stream);
    let translation = translator
        .run(&notes)
        .map_err(|error| crate::stream::stream_error(&error))?;
    cut_translation_into_segments(&translation, segmentLengths, overlap)
        .map_err(|error| crate::stream::stream_error(&error))
}

/// A measure pair as music21 answers it, a tuple of the two numbers.
fn measures_of<'py>(
    py: Python<'py>,
    measures: (Option<IntegerType>, Option<IntegerType>),
) -> PyResult<Bound<'py, PyTuple>> {
    let number = |value: Option<IntegerType>| -> PyResult<Bound<'py, PyAny>> {
        Ok(match value {
            Some(value) => value.into_pyobject(py)?.into_any(),
            None => py.None().into_bound(py),
        })
    };
    PyTuple::new(py, [number(measures.0)?, number(measures.1)?])
}

/// The pair music21's `translateMonophonicPartToSegments` answers.
fn answered<'py>(
    py: Python<'py>,
    cut: &Segments,
) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>)> {
    let segments = PyList::empty(py);
    let measures = PyList::empty(py);
    for (segment, pair) in cut.segments.iter().zip(&cut.measures) {
        segments.append(segment)?;
        measures.append(measures_of(py, *pair)?)?;
    }
    Ok((segments, measures))
}

/// music21's `translateMonophonicPartToSegments`: a part's notes written
/// as text and cut into overlapping stretches, with the measures each
/// stretch runs from and to.
#[pyfunction]
#[pyo3(signature = (inputStream, *, segmentLengths = 30, overlap = 12, algorithm = None, jitter = 0))]
fn translateMonophonicPartToSegments<'py>(
    py: Python<'py>,
    inputStream: &Bound<'py, PyAny>,
    segmentLengths: usize,
    overlap: usize,
    algorithm: Option<&Bound<'py, PyAny>>,
    jitter: i64,
) -> PyResult<(Bound<'py, PyList>, Bound<'py, PyList>)> {
    if jitter != 0 {
        return Err(crate::stream::StreamException::new_err(
            "a jitter moves each segment's start at random, which this package does not offer",
        ));
    }
    let cut = cut(
        inputStream,
        segmentLengths,
        overlap,
        Translator::of(algorithm)?,
    )?;
    answered(py, &cut)
}

/// music21's `indexScoreParts`: every part of a score cut the same way,
/// each as a dictionary of the two lists.
#[pyfunction]
#[pyo3(signature = (scoreFile, **keywords))]
fn indexScoreParts<'py>(
    py: Python<'py>,
    scoreFile: &Bound<'py, PyAny>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    let read = |name: &str, default: usize| -> PyResult<usize> {
        match keywords.and_then(|keywords| keywords.get_item(name).ok().flatten()) {
            Some(value) => value.extract(),
            None => Ok(default),
        }
    };
    let segmentLengths = read("segmentLengths", 30)?;
    let overlap = read("overlap", 12)?;
    let jitter = match keywords.and_then(|keywords| keywords.get_item("jitter").ok().flatten()) {
        Some(value) => value.extract()?,
        None => 0i64,
    };
    if jitter != 0 {
        return Err(crate::stream::StreamException::new_err(
            "a jitter moves each segment's start at random, which this package does not offer",
        ));
    }
    let algorithm = keywords.and_then(|keywords| keywords.get_item("algorithm").ok().flatten());
    let translator = Translator::of(algorithm.as_ref())?;

    let out = PyList::empty(py);
    for part in scoreFile.getattr("parts")?.walk()? {
        let cut = cut(&part?, segmentLengths, overlap, translator)?;
        let (segments, measures) = answered(py, &cut)?;
        let indexed = PyDict::new(py);
        indexed.set_item("segmentList", segments)?;
        indexed.set_item("measureList", measures)?;
        out.append(indexed)?;
    }
    Ok(out)
}

/// An index of scores read out of music21's dictionary of them.
fn index_of(scoreDict: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Vec<Segments>)>> {
    let mut index = Vec::new();
    for name in scoreDict.call_method0("keys")?.walk()? {
        let name = name?;
        let score = scoreDict.get_item(&name)?;
        let mut parts = Vec::new();
        for part in score.walk()? {
            let part = part?;
            let mut segments = Segments {
                segments: Vec::new(),
                measures: Vec::new(),
            };
            for segment in part.get_item("segmentList")?.walk()? {
                segments.segments.push(segment?.extract()?);
            }
            for pair in part.get_item("measureList")?.walk()? {
                let pair = pair?;
                segments
                    .measures
                    .push((pair.get_item(0)?.extract()?, pair.get_item(1)?.extract()?));
            }
            parts.push(segments);
        }
        index.push((name.extract()?, parts));
    }
    Ok(index)
}

/// music21's `scoreSimilarity`: how alike every pair of segments of an
/// index of scores is, by `difflib`'s ratio.
///
/// A comparison by `Levenshtein` is refused rather than answered by
/// `difflib` instead, the two being different measures: music21 reaches
/// for that library where it is installed, and its own examples force
/// `difflib` because of it.
#[pyfunction]
#[pyo3(signature = (scoreDict, minimumLength = 20, giveUpdates = false, includeReverse = false, forceDifflib = false))]
fn scoreSimilarity<'py>(
    py: Python<'py>,
    scoreDict: &Bound<'py, PyAny>,
    minimumLength: usize,
    #[pyo3(from_py_with = crate::truthy)] giveUpdates: bool,
    #[pyo3(from_py_with = crate::truthy)] includeReverse: bool,
    #[pyo3(from_py_with = crate::truthy)] forceDifflib: bool,
) -> PyResult<Bound<'py, PyList>> {
    if !forceDifflib && py.import("Levenshtein").is_ok() {
        return Err(crate::stream::StreamException::new_err(
            "this package compares segments by difflib's ratio alone; \
             ask for forceDifflib=True, as music21's own examples do",
        ));
    }
    let index = index_of(scoreDict)?;
    if giveUpdates {
        let print = py.import("builtins")?.getattr("print")?;
        for (number, (name, _)) in index.iter().enumerate() {
            print.call1((format!("Comparing {name} ({}/{})", number + 1, index.len()),))?;
        }
    }
    let out = PyList::empty(py);
    for row in score_similarity(&index, minimumLength, includeReverse) {
        let made = PyTuple::new(
            py,
            [
                row.this.score.into_pyobject(py)?.into_any(),
                row.this.part.into_pyobject(py)?.into_any(),
                row.this.segment.into_pyobject(py)?.into_any(),
                measures_of(py, row.this.measures)?.into_any(),
                row.that.score.into_pyobject(py)?.into_any(),
                row.that.part.into_pyobject(py)?.into_any(),
                row.that.segment.into_pyobject(py)?.into_any(),
                measures_of(py, row.that.measures)?.into_any(),
                row.ratio.into_pyobject(py)?.into_any(),
            ],
        )?;
        out.append(made)?;
    }
    Ok(out)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(translateMonophonicPartToSegments, m)?)?;
    m.add_function(wrap_pyfunction!(indexScoreParts, m)?)?;
    m.add_function(wrap_pyfunction!(scoreSimilarity, m)?)?;
    Ok(())
}
