//! music21's `analysis.reduction`: a score reduced to the notes an
//! analyst marked.
//!
//! The reduction is the crate's `analysis::reduction::ScoreReduction`: a
//! score's reductive notes read off the lyrics that mark them, each
//! sounded in the part its specification names. The facade holds the
//! score music21 hands it, reads it afresh for the reduction and answers
//! music21's own streams (`stream::music21_from_crate`).
//!
//! **A score set here is a copy, as music21 copies it.** music21's setter
//! deep-copies what it is given so that the score a caller keeps is never
//! altered, wraps a stream with no parts in a `Score`, and records what
//! the copy was made by -- all of which it does to its own objects, so
//! the facade asks music21 for them.
//!
//! `ReductiveNote` and `PartReduction` stay music21's. The first is a
//! specification read off a lyric, which the crate answers through
//! `ReductiveParameters`; the second draws a graph of how busy each part
//! is, which is presentation. Neither has an example of its own.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;

use music21_rs_crate::analysis::reduction::ScoreReduction as RsScoreReduction;

/// The names this module hands out.
pub const NAMES: &[&str] = &["ScoreReduction"];

pyo3::create_exception!(
    music21_rs_facade,
    ScoreReductionException,
    crate::Music21Exception
);

error_into!(pub(crate) reduction_error, ScoreReductionException);

/// A stream held as music21 holds one: a copy of what the caller gave, in
/// a score where it has no parts of its own.
fn held<'py>(value: &Bound<'py, PyAny>, method: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = value.py();
    let stream = value
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false);
    if !stream {
        return Err(ScoreReductionException::new_err("cannot set a non Stream"));
    }
    let copied = py.import("copy")?.call_method1("deepcopy", (value,))?;
    let parts = value
        .call_method0("hasPartLikeStreams")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false);
    let made = if parts {
        copied
    } else {
        // A score of the same kind as the stream it wraps: music21's
        // where music21 will hold it, this package's where it will not.
        let score = crate::stream::stream_class_like(py, "Score", Some(&copied))?.call0()?;
        score.call_method1("insert", (0.0, copied))?;
        score
    };
    // music21 records what the copy was made by, which its own streams
    // read back.
    let keywords = pyo3::types::PyDict::new(py);
    keywords.set_item("recurse", true)?;
    let _ = made.call_method("setDerivationMethod", (method,), Some(&keywords));
    Ok(made)
}

/// music21's `analysis.reduction.ScoreReduction`: a score and the
/// reduction its marked notes make.
#[pyclass(
    name = "ScoreReduction",
    module = "music21.analysis.reduction",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct ScoreReduction {
    /// The score being reduced, as music21 keeps its copy.
    _score: Py<PyAny>,
    /// A chordal reduction of it, where a caller has one.
    _chordReduction: Py<PyAny>,
}

#[pymethods]
impl ScoreReduction {
    /// music21's `ScoreReduction()`.
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(py: Python<'_>, keywords: Option<&Bound<'_, pyo3::types::PyDict>>) -> Self {
        let _ = keywords;
        Self {
            _score: py.None(),
            _chordReduction: py.None(),
        }
    }

    /// The score being reduced: music21's `score`, which keeps a copy of
    /// what it was given.
    #[getter]
    fn score(&self, py: Python<'_>) -> Py<PyAny> {
        self._score.clone_ref(py)
    }

    #[setter]
    fn set_score(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self._score = held(value, "ScoreReduction")?.unbind();
        Ok(())
    }

    /// A chordal reduction of it: music21's `chordReduction`, likewise a
    /// copy.
    #[getter]
    fn chordReduction(&self, py: Python<'_>) -> Py<PyAny> {
        self._chordReduction.clone_ref(py)
    }

    #[setter]
    fn set_chordReduction(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self._chordReduction = held(value, "ScoreReduction")?.unbind();
        Ok(())
    }

    /// The reduction itself: music21's `reduce`.
    fn reduce(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let (score, chords) = {
            let me = slf.borrow();
            (me._score.clone_ref(py), me._chordReduction.clone_ref(py))
        };
        if score.bind(py).is_none() && chords.bind(py).is_none() {
            return Err(ScoreReductionException::new_err(
                "no score defined to reduce",
            ));
        }
        let mut reduction = RsScoreReduction::new();
        if !score.bind(py).is_none() {
            reduction.set_score(crate::stream::crate_stream(score.bind(py))?);
        }
        if !chords.bind(py).is_none() {
            reduction.set_chord_reduction(crate::stream::crate_stream(chords.bind(py))?);
        }
        let made = reduction.reduce().map_err(reduction_error)?;
        Ok(crate::stream::music21_from_crate(py, &made)?.unbind())
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScoreReduction>()?;
    Ok(())
}
