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
//! `analysis.floatingKey`'s `KeyAnalyzer` is here too: a key for every
//! measure, each measure's own reading smoothed by its neighbours'. Its
//! `weightAlgorithm` is a Python function, as music21's is, so the
//! smoothing is done here over the readings the crate works out rather
//! than in the crate, which takes a function of its own.
//!
//! The classes still outside it (`KeyWeightKeyAnalysis`, `Ambitus`,
//! `WindowedAnalysis`) are colour tables and logs of solutions around
//! arithmetic the crate already has; `Stream.analyze` is the way to the
//! key and the ambitus.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::analysis::KeyEstimate;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "nPVI",
    "melodicIntervalVariability",
    "labelBeatDepth",
    "thomassenMelodicAccent",
    "pitchAttributeCount",
    "attributeCount",
    "Segmenter",
    "divide",
    "KeyAnalyzer",
    "FloatingKeyException",
];

// music21's own `FloatingKeyException` is an `AnalysisException`, which is
// a `Music21Exception`; this one is the base alone, there being no type
// here for the class in between. Nothing in music21 catches the middle
// one, and nothing here is installed over `analysis.floatingKey`.
pyo3::create_exception!(
    music21_rs_facade,
    FloatingKeyException,
    crate::Music21Exception
);

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

/// music21's `analysis.elements.attributeCount`: how often each value of
/// some property of a stream's elements comes.
///
/// The elements are whatever is handed in -- a stream, which gives its own
/// elements, or any iterator over them, as music21 takes either -- and the
/// property is read off each by name, an element that has none passed
/// over. The answer is a `collections.Counter`, which is what music21
/// answers, so `most_common` orders ties by which value was met first.
#[pyfunction]
#[pyo3(signature = (streamOrStreamIter, attrName = "quarterLength".to_string()))]
fn attributeCount<'py>(
    streamOrStreamIter: &Bound<'py, PyAny>,
    attrName: String,
) -> PyResult<Bound<'py, PyAny>> {
    let py = streamOrStreamIter.py();
    let counts = py.import("collections")?.getattr("Counter")?.call0()?;
    for element in streamOrStreamIter.walk()? {
        let element = element?;
        if !element.hasattr(attrName.as_str())? {
            continue;
        }
        let value = element.getattr(attrName.as_str())?;
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

/// music21's `analysis.floatingKey.divide`: how much a neighbouring
/// measure's reading counts at a distance, its coefficient over the
/// distance plus one.
#[pyfunction]
fn divide(coefficient: f64, distance: i32) -> f64 {
    music21_rs_crate::analysis::floating_key::divide(coefficient, distance)
}

/// music21's `analysis.floatingKey.KeyAnalyzer`: the key of every measure,
/// read one measure at a time and then smoothed by the measures around it.
///
/// A measure is read as every part's measure at that place, by the
/// Aarden-Essen weighting music21's `analyze('key')` reads with, and a
/// measure with no notes has no reading. `windowSize` is how many measures
/// either side count, and `weightAlgorithm` how much each counts at its
/// distance -- a Python function, as music21's is, so a caller may weigh
/// the window however it likes.
///
/// music21 keeps each measure's readings once asked for; this reads them
/// again, so a stream edited between two questions is answered as it
/// stands.
#[pyclass(
    name = "KeyAnalyzer",
    module = "music21.analysis.floatingKey",
    subclass,
    skip_from_py_object
)]
pub struct KeyAnalyzer {
    /// The stream being read, as music21's `stream` attribute.
    stream: Py<PyAny>,
    /// How many measures either side of each are taken into account.
    #[pyo3(get, set)]
    windowSize: usize,
    /// How many measures there are to read.
    #[pyo3(get, set)]
    numMeasures: usize,
    /// What the last `getRawKeyByMeasure` found, as music21 keeps it.
    rawKeyByMeasure: Py<PyAny>,
    /// How a neighbour's reading is weighed at its distance.
    weightAlgorithm: Py<PyAny>,
}

#[pymethods]
impl KeyAnalyzer {
    /// music21's `KeyAnalyzer(s)`, which refuses a stream with no measures
    /// in it and nothing at all.
    #[new]
    #[pyo3(signature = (s = None))]
    fn make(py: Python<'_>, s: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(s) = s.filter(|given| !given.is_none()) else {
            return Err(FloatingKeyException::new_err("Need a Stream to initialize"));
        };
        let stream = crate::stream::crate_stream(s)?;
        let analyzer = music21_rs_crate::analysis::floating_key::KeyAnalyzer::new(&stream)
            .map_err(|error| FloatingKeyException::new_err(crate::pitch::message(&error)))?;
        Ok(Self {
            stream: s.clone().unbind(),
            windowSize: analyzer.window_size,
            numMeasures: analyzer.measure_count(),
            rawKeyByMeasure: pyo3::types::PyList::empty(py).into_any().unbind(),
            weightAlgorithm: crate::class_of(py, "music21.analysis.floatingKey", "divide")?
                .unbind(),
        })
    }

    #[getter]
    fn get_stream(&self, py: Python<'_>) -> Py<PyAny> {
        self.stream.clone_ref(py)
    }

    #[setter]
    fn set_stream(&mut self, value: &Bound<'_, PyAny>) {
        self.stream = value.clone().unbind();
    }

    #[getter]
    fn get_rawKeyByMeasure(&self, py: Python<'_>) -> Py<PyAny> {
        self.rawKeyByMeasure.clone_ref(py)
    }

    #[setter]
    fn set_rawKeyByMeasure(&mut self, value: &Bound<'_, PyAny>) {
        self.rawKeyByMeasure = value.clone().unbind();
    }

    #[getter]
    fn get_weightAlgorithm(&self, py: Python<'_>) -> Py<PyAny> {
        self.weightAlgorithm.clone_ref(py)
    }

    #[setter]
    fn set_weightAlgorithm(&mut self, value: &Bound<'_, PyAny>) {
        self.weightAlgorithm = value.clone().unbind();
    }

    /// Every measure's reading on its own, nothing for a measure with no
    /// notes: music21's `getRawKeyByMeasure`. Each key carries how well it
    /// fitted and the keys that fitted less well, as music21's does.
    fn getRawKeyByMeasure<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let found = pyo3::types::PyList::empty(py);
        for reading in slf.borrow().readings(py)? {
            match reading {
                None => found.append(py.None())?,
                Some(estimates) => found.append(key_of(py, &estimates)?)?,
            }
        }
        let answer = found.into_any();
        slf.borrow_mut().rawKeyByMeasure = answer.clone().unbind();
        Ok(answer)
    }

    /// A measure's readings as a dictionary of each key's name with case
    /// against its coefficient, nothing for a measure with none: music21's
    /// `getInterpretationByMeasure`.
    fn getInterpretationByMeasure<'py>(
        slf: &Bound<'py, Self>,
        mIndex: usize,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let readings = slf.borrow().readings(py)?;
        let Some(Some(estimates)) = readings.get(mIndex) else {
            return Ok(None);
        };
        let found = PyDict::new(py);
        for estimate in estimates {
            found.set_item(
                crate::spelling::music21_name(&estimate.key().tonic_pitch_name_with_case()),
                estimate.score(),
            )?;
        }
        Ok(Some(found.into_any()))
    }

    /// The key of every measure that has a reading, each smoothed by the
    /// readings of the measures around it: music21's
    /// `smoothInterpretationByMeasure`.
    ///
    /// A measure with no reading has no key and is left out, and each
    /// measure is smoothed by its neighbours' own readings rather than
    /// their smoothed ones, as music21 smooths them.
    fn smoothInterpretationByMeasure<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py = slf.py();
        let me = slf.borrow();
        let raw = me.readings(py)?;
        let weigh = me.weightAlgorithm.bind(py);
        let window = me.windowSize as isize;
        let class = crate::class_of(py, "music21.key", "Key")?;
        let mut smoothed = Vec::new();
        for index in 0..raw.len() {
            let Some(estimates) = &raw[index] else {
                continue;
            };
            let mut readings: Vec<(String, f64)> = estimates.iter().map(named).collect();
            for distance in -window..=window {
                let neighbour = index as isize + distance;
                if distance == 0 || neighbour < 0 || neighbour as usize >= raw.len() {
                    continue;
                }
                let Some(theirs) = &raw[neighbour as usize] else {
                    continue;
                };
                for (name, value) in readings.iter_mut() {
                    let Some(found) = theirs.iter().find(|estimate| named(estimate).0 == *name)
                    else {
                        continue;
                    };
                    *value += weigh.call1((found.score(), distance))?.extract::<f64>()?;
                }
            }
            let mut best: Option<&(String, f64)> = None;
            for reading in readings.iter() {
                if best.is_none_or(|(_, value)| reading.1 > *value) {
                    best = Some(reading);
                }
            }
            if let Some((name, _)) = best {
                smoothed.push(class.call1((name.as_str(),))?);
            }
        }
        Ok(smoothed)
    }

    /// Every measure's key, smoothed: music21's `run`, which reads the
    /// measures and then smooths them.
    fn run<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Self::getRawKeyByMeasure(slf)?;
        Self::smoothInterpretationByMeasure(slf)
    }
}

impl KeyAnalyzer {
    /// Every measure's readings, best first, as the crate reads them.
    fn readings(&self, py: Python<'_>) -> PyResult<Vec<Option<Vec<KeyEstimate>>>> {
        let stream = crate::stream::crate_stream(self.stream.bind(py))?;
        let mut analyzer = music21_rs_crate::analysis::floating_key::KeyAnalyzer::new(&stream)
            .map_err(|error| FloatingKeyException::new_err(crate::pitch::message(&error)))?;
        analyzer.window_size = self.windowSize;
        Ok(analyzer.raw_key_by_measure())
    }
}

/// A reading as its key's name with case, spelled as music21 spells one --
/// `A-` rather than the crate's own `Ab` -- beside its coefficient.
fn named(estimate: &KeyEstimate) -> (String, f64) {
    (
        crate::spelling::music21_name(&estimate.key().tonic_pitch_name_with_case()),
        estimate.score(),
    )
}

/// The key a measure's readings settle on, carrying how well it fitted and
/// the keys that fitted less well, as music21's analysis answers one.
pub(crate) fn key_of<'py>(
    py: Python<'py>,
    estimates: &[KeyEstimate],
) -> PyResult<Bound<'py, PyAny>> {
    let Some((best, rest)) = estimates.split_first() else {
        return Ok(py.None().into_bound(py));
    };
    let found = crate::key::Key::object(py, best.key().clone())?.into_bound(py);
    found.setattr("correlationCoefficient", best.score())?;
    let others = pyo3::types::PyList::empty(py);
    for estimate in rest {
        let other = crate::key::Key::object(py, estimate.key().clone())?.into_bound(py);
        other.setattr("correlationCoefficient", estimate.score())?;
        others.append(other)?;
    }
    found.setattr("alternateInterpretations", others)?;
    Ok(found)
}

/// Adds the analysis functions and classes to a module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(nPVI, m)?)?;
    m.add_function(wrap_pyfunction!(melodicIntervalVariability, m)?)?;
    m.add_function(wrap_pyfunction!(labelBeatDepth, m)?)?;
    m.add_function(wrap_pyfunction!(thomassenMelodicAccent, m)?)?;
    m.add_function(wrap_pyfunction!(pitchAttributeCount, m)?)?;
    m.add_function(wrap_pyfunction!(attributeCount, m)?)?;
    m.add_function(wrap_pyfunction!(divide, m)?)?;
    m.add_class::<Segmenter>()?;
    m.add_class::<KeyAnalyzer>()?;
    Ok(())
}
