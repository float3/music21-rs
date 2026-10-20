//! music21's `audioSearch`: a recording turned into notes, and the part
//! of a score a player is in.
//!
//! Every step of it is the crate's `audiosearch`, which
//! `audiosearch_parity` holds to music21 -- the histogram and the
//! parabola through a peak, the frequency a signal sounds by
//! autocorrelation, the thresholds between a scale's pitches and the
//! pitch a frequency falls nearest, the smoothing, the runs of one pitch
//! joined into notes, the quarter length estimated and a length snapped
//! to it, and which part of a score a follower takes.
//!
//! **The three members that read audio stay music21's.** `wave` is
//! Python's own reader, `getFrequenciesFromMicrophone` records, and
//! `getFrequenciesFromPartialAudioFile` keeps a handle open across calls
//! to read one window at a time -- which is file IO, not music, and the
//! crate does none. They reach the crate anyway, since what they read is
//! handed straight to `autocorrelationFunction`.
//!
//! **A note keeps the very pitch object it was made from.** music21's
//! `joinConsecutiveIdenticalPitches` writes the rest frequency onto the
//! first pitch it is given and hands each note it makes the `Pitch`
//! object of its run's last pitch, so the facade asks the crate's
//! by-position twin `joined_pitches` which pitch each run ends on and
//! builds the note on that very object.
//!
//! **A transcription is music21's own score.** `notesAndDurationsToStream`
//! writes the length onto *every* note it is given -- the leading rests it
//! then leaves out included -- and puts the caller's own objects into a
//! part, so the facade asks `transcribed_notes` what each is given and
//! which are kept, writes the lengths back, and builds music21's
//! `stream.Score` where music21 can be asked and this package's where it
//! cannot. Nothing of the stream is built from the crate's values: the
//! notes in it are the caller's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use music21_rs_crate::audiosearch::{
    REST_FREQUENCY, autocorrelation_frequency, decision_process, detect_pitch_frequencies,
    histogram, interpolation, joined_pitches, normalize_input_frequency,
    pitch_frequencies_to_objects, prepare_thresholds, quantize_duration, quarter_length_estimation,
    smooth_frequencies, transcribed_notes,
};
use music21_rs_crate::scale::Scale as RsScale;
use music21_rs_crate::{Duration as RsDuration, Note as RsNote, Rest as RsRest};
use music21_rs_crate::{FloatType, Pitch as RsPitch};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "histogram",
    "autocorrelationFunction",
    "prepareThresholds",
    "interpolation",
    "normalizeInputFrequency",
    "pitchFrequenciesToObjects",
    "detectPitchFrequencies",
    "smoothFrequencies",
    "joinConsecutiveIdenticalPitches",
    "quantizeDuration",
    "quarterLengthEstimation",
    "notesAndDurationsToStream",
    "decisionProcess",
];

pyo3::create_exception!(
    music21_rs_facade,
    AudioSearchException,
    crate::Music21Exception
);

error_into!(pub(crate) audio_error, AudioSearchException);

/// The scale a caller named, where it named one.
fn scale_given(useScale: Option<&Bound<'_, PyAny>>) -> PyResult<Option<RsScale>> {
    match useScale.filter(|scale| !scale.is_none()) {
        Some(scale) => Ok(Some(crate::scale::crate_scale(scale)?)),
        None => Ok(None),
    }
}

/// A pitch as music21 hands one back.
fn pitch_object(py: Python<'_>, pitch: RsPitch) -> PyResult<Py<PyAny>> {
    Ok(crate::installed_new(
        py,
        "music21.pitch",
        "Pitch",
        crate::pitch::Pitch::wrap(pitch, false),
    )?
    .into_any())
}

/// A list of frequencies read as the crate holds them, which is what
/// every one of these takes.
fn frequencies(values: &Bound<'_, PyAny>) -> PyResult<Vec<FloatType>> {
    let mut out = Vec::new();
    for value in values.walk()? {
        out.push(value?.extract::<FloatType>()?);
    }
    Ok(out)
}

/// The class a stream of this shape is built as: music21's own where
/// music21 can be asked, and this package's where it cannot.
fn stream_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(found) = py
        .import("music21.stream")
        .and_then(|module| module.getattr(name))
    {
        return Ok(found);
    }
    py.import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?
        .getattr(name)
}

/// The same for the metadata a transcription is titled by.
fn metadata_class<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    if let Some(installed) = crate::installed_class(py, "music21.metadata", "Metadata") {
        return Ok(installed);
    }
    if let Ok(found) = py
        .import("music21.metadata")
        .and_then(|module| module.getattr("Metadata"))
    {
        return Ok(found);
    }
    Ok(py.get_type::<crate::metadata::Metadata>().into_any())
}

/// music21's `histogram`: how many of the data fall in each of so many
/// bins, and where the bins divide.
///
/// music21 starts the list of edges with the least of the data *as it was
/// given*, so a list of whole numbers has a whole number there and floats
/// after it; the facade hands that one back as it came.
#[pyfunction]
#[pyo3(name = "histogram")]
pub fn histogram_py<'py>(
    py: Python<'py>,
    data: &Bound<'py, PyAny>,
    bins: usize,
) -> PyResult<Bound<'py, PyTuple>> {
    let read = frequencies(data)?;
    let (counts, edges) = histogram(&read, bins).map_err(audio_error)?;
    let least = data.call_method0("__iter__").and_then(|_| {
        py.import("builtins")?
            .getattr("min")?
            .call1((data,))
            .map(Bound::unbind)
    });
    let out = PyList::empty(py);
    for (at, edge) in edges.iter().enumerate() {
        match (at, &least) {
            (0, Ok(least)) => out.append(least.bind(py))?,
            _ => out.append(*edge)?,
        }
    }
    PyTuple::new(py, [PyList::new(py, counts)?.into_any(), out.into_any()])
}

/// music21's `autocorrelationFunction`: the frequency a recorded signal
/// sounds, found by correlating it with itself.
///
/// music21 refuses this without numpy, which it needs for the
/// convolution; the crate needs none, so the facade answers.
#[pyfunction]
#[pyo3(name = "autocorrelationFunction")]
pub fn autocorrelation_py(
    recordedSignal: &Bound<'_, PyAny>,
    recordSampleRateIn: FloatType,
) -> PyResult<FloatType> {
    let signal = frequencies(recordedSignal)?;
    autocorrelation_frequency(&signal, recordSampleRateIn).map_err(audio_error)
}

/// music21's `prepareThresholds`: the thresholds between a scale's
/// pitches, and the pitches.
#[pyfunction]
#[pyo3(name = "prepareThresholds")]
#[pyo3(signature = (useScale = None))]
pub fn prepare_thresholds_py<'py>(
    py: Python<'py>,
    useScale: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let scale = scale_given(useScale)?;
    let (thresholds, pitches) = prepare_thresholds(scale.as_ref()).map_err(audio_error)?;
    let objects = PyList::empty(py);
    for pitch in pitches {
        objects.append(pitch_object(py, pitch)?)?;
    }
    PyTuple::new(
        py,
        [PyList::new(py, thresholds)?.into_any(), objects.into_any()],
    )
}

/// music21's `interpolation`: where between three points about a peak the
/// top of the parabola through them lies.
#[pyfunction]
#[pyo3(name = "interpolation")]
pub fn interpolation_py(correlation: &Bound<'_, PyAny>, peak: usize) -> PyResult<FloatType> {
    let read = frequencies(correlation)?;
    Ok(interpolation(&read, peak))
}

/// music21's `normalizeInputFrequency`: the scale pitch a frequency falls
/// nearest, in the octave it sounds, and that pitch's own frequency.
#[pyfunction]
#[pyo3(name = "normalizeInputFrequency")]
#[pyo3(signature = (inputPitchFrequency, thresholds = None, pitches = None))]
pub fn normalize_input_frequency_py<'py>(
    py: Python<'py>,
    inputPitchFrequency: FloatType,
    thresholds: Option<&Bound<'py, PyAny>>,
    pitches: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let given = thresholds
        .filter(|values| !values.is_none())
        .zip(pitches.filter(|values| !values.is_none()));
    let (thresholds, pitches) = match given {
        Some((thresholds, pitches)) => {
            let mut read = Vec::new();
            for pitch in pitches.walk()? {
                read.push(crate::pitch::pitch_from_any(&pitch?)?);
            }
            (frequencies(thresholds)?, read)
        }
        None => prepare_thresholds(None).map_err(audio_error)?,
    };
    let (frequency, placed) = normalize_input_frequency(inputPitchFrequency, &thresholds, &pitches)
        .map_err(audio_error)?;
    PyTuple::new(
        py,
        [
            frequency.into_pyobject(py)?.into_any(),
            pitch_object(py, placed)?.into_bound(py),
        ],
    )
}

/// music21's `pitchFrequenciesToObjects`: the pitch each frequency is
/// nearest, and the frequencies of those pitches to be drawn.
#[pyfunction]
#[pyo3(name = "pitchFrequenciesToObjects")]
#[pyo3(signature = (detectedPitchesFreq, useScale = None))]
pub fn pitch_frequencies_to_objects_py<'py>(
    py: Python<'py>,
    detectedPitchesFreq: &Bound<'py, PyAny>,
    useScale: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let read = frequencies(detectedPitchesFreq)?;
    let scale = scale_given(useScale)?;
    let (pitches, plotted) =
        pitch_frequencies_to_objects(&read, scale.as_ref()).map_err(audio_error)?;
    let objects = PyList::empty(py);
    for pitch in pitches {
        objects.append(pitch_object(py, pitch)?)?;
    }
    PyTuple::new(
        py,
        [objects.into_any(), PyList::new(py, plotted)?.into_any()],
    )
}

/// music21's `detectPitchFrequencies`: the frequency of the pitch each
/// frequency is nearest, in the octave it sounds.
#[pyfunction]
#[pyo3(name = "detectPitchFrequencies")]
#[pyo3(signature = (freqFromAQList, useScale = None))]
pub fn detect_pitch_frequencies_py<'py>(
    py: Python<'py>,
    freqFromAQList: &Bound<'py, PyAny>,
    useScale: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let read = frequencies(freqFromAQList)?;
    let scale = scale_given(useScale)?;
    let found = detect_pitch_frequencies(&read, scale.as_ref()).map_err(audio_error)?;
    PyList::new(py, found)
}

/// music21's `smoothFrequencies`: each frequency averaged with its
/// neighbours and rounded, so that one bad reading does not sound.
#[pyfunction]
#[pyo3(name = "smoothFrequencies")]
#[pyo3(signature = (frequencyList, *, smoothLevels = 7, inPlace = false))]
pub fn smooth_frequencies_py<'py>(
    py: Python<'py>,
    frequencyList: &Bound<'py, PyAny>,
    smoothLevels: usize,
    inPlace: bool,
) -> PyResult<Option<Bound<'py, PyList>>> {
    let read = frequencies(frequencyList)?;
    let smoothed = smooth_frequencies(&read, smoothLevels).map_err(audio_error)?;
    if inPlace {
        for (at, value) in smoothed.iter().enumerate() {
            frequencyList.set_item(at, *value)?;
        }
        return Ok(None);
    }
    Ok(Some(PyList::new(py, smoothed)?))
}

/// music21's `joinConsecutiveIdenticalPitches`: a note or a rest for each
/// run of one pitch, and how many pitches each stands for.
///
/// music21 writes the rest frequency onto the first pitch it is given and
/// hands each note the very `Pitch` object its run ends on, both of which
/// the facade does on the caller's own objects.
#[pyfunction]
#[pyo3(name = "joinConsecutiveIdenticalPitches")]
pub fn join_consecutive_identical_pitches_py<'py>(
    py: Python<'py>,
    detectedPitchObjects: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut objects = Vec::new();
    for pitch in detectedPitchObjects.walk()? {
        objects.push(pitch?);
    }
    let first = objects
        .first()
        .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("list index out of range"))?;
    first.setattr("frequency", REST_FREQUENCY)?;
    let mut read = Vec::with_capacity(objects.len());
    for pitch in &objects {
        read.push(crate::pitch::pitch_from_any(pitch)?);
    }
    let joined = joined_pitches(&read).map_err(audio_error)?;
    let notes = PyList::empty(py);
    let lengths = PyList::empty(py);
    for one in &joined {
        lengths.append(one.length)?;
        match one.pitch {
            Some(place) => {
                let sounded = read.get(place).cloned().ok_or_else(|| {
                    pyo3::exceptions::PyIndexError::new_err("list index out of range")
                })?;
                let note = crate::installed_new(
                    py,
                    "music21.note",
                    "Note",
                    crate::note::Note::wrap(py, RsNote::from_pitch(sounded))?,
                )?;
                let note = note.into_bound(py).into_any();
                // music21 hands the note the very pitch object of the
                // run's last pitch, which a caller may be holding.
                note.setattr(
                    "pitch",
                    objects.get(place).ok_or_else(|| {
                        pyo3::exceptions::PyIndexError::new_err("list index out of range")
                    })?,
                )?;
                notes.append(note)?;
            }
            None => {
                let rest = crate::installed_new(
                    py,
                    "music21.note",
                    "Rest",
                    crate::rest::Rest::wrap(py, RsRest::new(RsDuration::quarter())),
                )?;
                notes.append(rest.into_bound(py))?;
            }
        }
    }
    PyTuple::new(py, [notes.into_any(), lengths.into_any()])
}

/// music21's `quantizeDuration`: a length snapped to the nearest of the
/// six it writes.
#[pyfunction]
#[pyo3(name = "quantizeDuration")]
pub fn quantize_duration_py(length: FloatType) -> FloatType {
    quantize_duration(length)
}

/// music21's `quarterLengthEstimation`: how many samples a quarter note
/// lasts, from how many each note lasted.
#[pyfunction]
#[pyo3(name = "quarterLengthEstimation")]
#[pyo3(signature = (durationList, mostRepeatedQuarterLength = 1.0))]
pub fn quarter_length_estimation_py(
    durationList: &Bound<'_, PyAny>,
    mostRepeatedQuarterLength: FloatType,
) -> PyResult<FloatType> {
    let lengths = lengths_of(durationList)?;
    quarter_length_estimation(&lengths, mostRepeatedQuarterLength).map_err(audio_error)
}

/// How many samples each note lasted, as the crate counts them.
fn lengths_of(values: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
    let mut out = Vec::new();
    for value in values.walk()? {
        out.push(value?.extract::<usize>()?);
    }
    Ok(out)
}

/// music21's `notesAndDurationsToStream`: the notes as a score, each as
/// long as its share of the estimated quarter.
#[pyfunction]
#[pyo3(name = "notesAndDurationsToStream")]
#[pyo3(signature = (
    notesList,
    durationList,
    scNotes = None,
    removeRestsAtBeginning = true,
    qle = None,
))]
pub fn notes_and_durations_to_stream_py<'py>(
    py: Python<'py>,
    notesList: &Bound<'py, PyAny>,
    durationList: &Bound<'py, PyAny>,
    scNotes: Option<&Bound<'py, PyAny>>,
    removeRestsAtBeginning: bool,
    qle: Option<FloatType>,
) -> PyResult<Bound<'py, PyTuple>> {
    let _ = qle;
    let mut objects = Vec::new();
    for note in notesList.walk()? {
        objects.push(note?);
    }
    let mut read = Vec::with_capacity(objects.len());
    for note in &objects {
        read.push(crate::stream::element_read(note)?.ok_or_else(|| {
            AudioSearchException::new_err("a transcription holds notes and rests alone")
        })?);
    }
    let lengths = lengths_of(durationList)?;
    let score_notes = match scNotes.filter(|notes| !notes.is_none()) {
        Some(notes) => Some(crate::stream::crate_stream(notes)?),
        None => None,
    };
    let (given, estimate) = transcribed_notes(
        &read,
        &lengths,
        score_notes.as_ref(),
        removeRestsAtBeginning,
    )
    .map_err(audio_error)?;
    let part = stream_class(py, "Part")?.call0()?;
    for one in &given {
        let note = objects
            .get(one.note)
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("list index out of range"))?;
        // music21 writes the length onto every note it is given, the
        // rests it then leaves out included.
        note.setattr("quarterLength", one.quarter_length)?;
        if one.kept {
            part.call_method1("append", (note,))?;
        }
    }
    let score = stream_class(py, "Score")?.call0()?;
    let metadata = metadata_class(py)?.call0()?;
    metadata.setattr("title", "Automatic Music21 Transcription")?;
    score.setattr("metadata", metadata)?;
    score.call_method1("insert", (0.0, &part))?;
    let answer = if score_notes.is_some() {
        estimate.into_pyobject(py)?.into_any()
    } else {
        part.len()?.into_pyobject(py)?.into_any()
    };
    PyTuple::new(py, [score, answer])
}

/// music21's `decisionProcess`: which of the parts a score follower takes
/// as where the player is, and how many doubtful guesses it has made.
#[pyfunction]
#[pyo3(name = "decisionProcess")]
#[pyo3(signature = (
    partsList,
    notePrediction,
    beginningData,
    lastNotePosition,
    countdown,
    firstNotePage = None,
    lastNotePage = None,
))]
#[allow(clippy::too_many_arguments)]
pub fn decision_process_py<'py>(
    py: Python<'py>,
    partsList: &Bound<'py, PyAny>,
    notePrediction: FloatType,
    beginningData: &Bound<'py, PyAny>,
    lastNotePosition: FloatType,
    countdown: usize,
    firstNotePage: Option<FloatType>,
    lastNotePage: Option<FloatType>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut parts = Vec::new();
    for part in partsList.walk()? {
        let part = part?;
        let id: usize = part.getattr("id")?.str()?.to_str()?.parse().map_err(|_| {
            AudioSearchException::new_err("a part of a search must be named by its place")
        })?;
        let likely: FloatType = part.getattr("matchProbability")?.extract()?;
        parts.push((id, likely));
    }
    let beginnings = frequencies(beginningData)?;
    let page = firstNotePage.zip(lastNotePage);
    let (position, countdown) = decision_process(
        &parts,
        notePrediction,
        &beginnings,
        lastNotePosition,
        countdown,
        page,
    )
    .map_err(audio_error)?;
    PyTuple::new(
        py,
        [
            position.into_pyobject(py)?.into_any(),
            countdown.into_pyobject(py)?.into_any(),
        ],
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(histogram_py, m)?)?;
    m.add_function(wrap_pyfunction!(autocorrelation_py, m)?)?;
    m.add_function(wrap_pyfunction!(prepare_thresholds_py, m)?)?;
    m.add_function(wrap_pyfunction!(interpolation_py, m)?)?;
    m.add_function(wrap_pyfunction!(normalize_input_frequency_py, m)?)?;
    m.add_function(wrap_pyfunction!(pitch_frequencies_to_objects_py, m)?)?;
    m.add_function(wrap_pyfunction!(detect_pitch_frequencies_py, m)?)?;
    m.add_function(wrap_pyfunction!(smooth_frequencies_py, m)?)?;
    m.add_function(wrap_pyfunction!(join_consecutive_identical_pitches_py, m)?)?;
    m.add_function(wrap_pyfunction!(quantize_duration_py, m)?)?;
    m.add_function(wrap_pyfunction!(quarter_length_estimation_py, m)?)?;
    m.add_function(wrap_pyfunction!(notes_and_durations_to_stream_py, m)?)?;
    m.add_function(wrap_pyfunction!(decision_process_py, m)?)?;
    Ok(())
}
