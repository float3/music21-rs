//! music21's `analysis.correlate`: what a stream's notes sound against the
//! dynamics they sound under.
//!
//! `analysis::pitch_to_dynamic` is the whole of it -- a pair for each pitch
//! and each dynamic it sounds under, the dynamic as its place in music21's
//! `shortNames` -- and the facade reads the stream into the crate and hands
//! the pairs back.
//!
//! music21 keeps the source and destination *objects* it matched in `data`,
//! which its own private `_findActive` fills; the crate answers the pairs
//! those lead to and no objects, so `pitchToDynamic` here leaves `data`
//! alone. Nothing in music21 reads it -- its own plotting asks for the
//! pairs -- and the installed class still inherits `_findActive`, so a
//! caller asking for it gets music21's own reading over the same stream.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use music21_rs_crate::FloatType;
use music21_rs_crate::analysis::correlate::pitch_to_dynamic;

/// The names this module hands out.
pub const NAMES: &[&str] = &["ActivityMatch"];

pyo3::create_exception!(
    music21_rs_facade,
    CorrelateException,
    crate::Music21Exception
);

/// music21's `analysis.correlate.ActivityMatch`: a stream asked what
/// sounds while something else sounds.
#[pyclass(
    name = "ActivityMatch",
    module = "music21.analysis.correlate",
    subclass,
    skip_from_py_object
)]
pub struct ActivityMatch {
    /// The stream asked.
    #[pyo3(get, set)]
    streamObj: Py<PyAny>,
    /// What music21 keeps the matched objects in, which stays as it was:
    /// see the module's own note.
    #[pyo3(get, set)]
    data: Py<PyAny>,
}

#[pymethods]
impl ActivityMatch {
    /// music21's `ActivityMatch(streamObj)`, which takes a stream and
    /// nothing else, in music21's words.
    #[new]
    #[pyo3(signature = (streamObj = None))]
    fn new(py: Python<'_>, streamObj: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(streamObj) = streamObj.filter(|given| !given.is_none()) else {
            return Ok(Self {
                streamObj: py.None(),
                data: py.None(),
            });
        };
        let is_stream = streamObj
            .getattr("classes")
            .and_then(|classes| classes.contains("Stream"))
            .unwrap_or(false)
            || streamObj
                .extract::<PyRef<'_, crate::stream::Stream>>()
                .is_ok();
        if !is_stream {
            return Err(CorrelateException::new_err(
                "non-stream provided as argument",
            ));
        }
        Ok(Self {
            streamObj: streamObj.clone().unbind(),
            data: py.None(),
        })
    }

    /// Every pitch against every dynamic it sounds under: music21's
    /// `pitchToDynamic`, the pitch as its pitch space and the dynamic as
    /// its place among music21's `shortNames`.
    ///
    /// Asked for the pairs themselves (`dataPoints`), each match is one
    /// pair; asked for coordinates, each pair comes once with how many
    /// times it was matched, in the order first met -- which is counting
    /// and not a reading of the music, so it is done here.
    #[pyo3(signature = (dataPoints = true))]
    fn pitchToDynamic<'py>(
        &self,
        py: Python<'py>,
        #[pyo3(from_py_with = crate::truthy)] dataPoints: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let stream = crate::stream::crate_stream(self.streamObj.bind(py))?;
        let pairs = pitch_to_dynamic(&stream)
            .map_err(|error| CorrelateException::new_err(crate::pitch::message(&error)))?;
        let out = PyList::empty(py);
        if dataPoints {
            for (sounds, dynamic) in pairs {
                out.append(PyTuple::new(
                    py,
                    [
                        sounds.into_pyobject(py)?.into_any(),
                        dynamic.into_pyobject(py)?.into_any(),
                    ],
                )?)?;
            }
            return Ok(out);
        }
        let mut counted: Vec<((FloatType, usize), usize)> = Vec::new();
        for pair in pairs {
            match counted.iter_mut().find(|(known, _)| *known == pair) {
                Some((_, count)) => *count += 1,
                None => counted.push((pair, 1)),
            }
        }
        for ((sounds, dynamic), count) in counted {
            out.append(PyTuple::new(
                py,
                [
                    sounds.into_pyobject(py)?.into_any(),
                    dynamic.into_pyobject(py)?.into_any(),
                    count.into_pyobject(py)?.into_any(),
                ],
            )?)?;
        }
        Ok(out)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ActivityMatch>()?;
    Ok(())
}
