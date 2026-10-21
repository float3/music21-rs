//! music21's `analysis.reduceChords`: a score thinned out to the few chords
//! that carry each measure.
//!
//! The whole reduction is the crate's `analysis::reduce_chords`, which
//! `reduce_chords_parity` holds to music21 chord for chord on twelve corpus
//! scores -- every chord of every measure with its offset and its length.
//! What the facade swaps is `ChordReducer`: the reduction itself, the
//! weighing of a measure's chords, and the reduction of one measure to the
//! few that weigh most.
//!
//! **The steps that edit music21's timespan tree are refused rather than
//! answered differently.** `collapseArpeggios`, `splitByBass`,
//! `fillBassGaps` and the rest each take music21's own *mutable*
//! `TimespanTree` and walk it while editing it, taking spans out and
//! putting new ones in as they go. The crate reads a score as a list of
//! spans of its own -- a reading, not a thing to edit -- and its reducer
//! runs music21's steps over that list in music21's order, which is what
//! `run` answers with. A member that hands the tree back half-edited has
//! nothing here to be, so it says so. `run` itself does not call any of
//! them.
//!
//! **A measure is reduced in place, by position.** music21's
//! `reduceMeasureToNChords` removes the chords it does not keep from the
//! measure it was given, moves the ones it keeps and lengthens them, and
//! hands the same measure back -- so the facade asks the crate which chords
//! are kept and where (`reduced_measure_places`) and writes that onto
//! music21's own measure. A caller holding one of those chords still holds
//! it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use music21_rs_crate::analysis::reduce_chords::{
    ChordWeight, ReductionOptions, WeighedChord, measure_chord_weights, reduce_chords,
    reduced_measure_places,
};
use music21_rs_crate::chord::Chord as RsChord;
use music21_rs_crate::duration::Duration as RsDuration;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["ChordReducer", "ChordReducerException"];

pyo3::create_exception!(
    music21_rs_facade,
    ChordReducerException,
    crate::Music21Exception
);

error_into!(pub(crate) reducer_error, ChordReducerException);

/// What a member that wants music21's mutable timespan tree says.
fn wants_a_tree(member: &str) -> PyErr {
    ChordReducerException::new_err(format!(
        "{member} walks music21's own mutable timespan tree and edits it as it goes, \
         which the crate reads as a list of spans of its own: `run` runs every step \
         over that list in music21's order, and no step of it can be asked for on its own"
    ))
}

/// One element of a measure as the weighing reads it: the chord it sounds
/// and how strong its beat is.
///
/// music21 weighs whatever it was handed by the pitches it sounds, so a
/// note counts as the chord of one note it is; a chord's duration is the
/// one the object carries.
fn weighed(object: &Bound<'_, PyAny>) -> PyResult<(RsChord, f64)> {
    let pitches = object.getattr("pitches")?;
    let mut held = Vec::new();
    for pitch in pitches.walk()? {
        held.push(crate::pitch::pitch_from_any(&pitch?)?);
    }
    let mut chord = RsChord::new(held).map_err(reducer_error)?;
    let length: f64 = object.getattr("quarterLength")?.extract()?;
    chord.set_duration(RsDuration::new(length).map_err(reducer_error)?);
    let strength: f64 = object
        .getattr("beatStrength")
        .and_then(|found| found.extract())
        .unwrap_or(1.0);
    Ok((chord, strength))
}

/// One element of a measure read as the weighing wants it: the chord it
/// sounds, where it stands and how strong its beat is.
type Weighed = (RsChord, f64, f64);

/// Every element a caller handed over as a measure, read as the chords it
/// sounds with where each stands.
fn measure_chords(measureObject: &Bound<'_, PyAny>) -> PyResult<Vec<Weighed>> {
    let mut read = Vec::new();
    for object in measureObject.walk()? {
        let object = object?;
        if object.getattr("pitches").is_err() {
            continue;
        }
        let offset: f64 = object
            .getattr("offset")
            .and_then(|found| found.extract())
            .unwrap_or(0.0);
        let (chord, strength) = weighed(&object)?;
        read.push((chord, offset, strength));
    }
    Ok(read)
}

/// The algorithm a caller named, read off the function's own name as
/// music21's own method is: so `cr.qlbsmpConsonance` and a function of a
/// caller's own with that name are read alike, and anything else is
/// refused rather than weighed by something it did not ask for.
fn weight_named(value: Option<&Bound<'_, PyAny>>) -> PyResult<ChordWeight> {
    let Some(value) = value.filter(|given| !given.is_none()) else {
        return Ok(ChordWeight::Length);
    };
    let name: String = value
        .getattr("__name__")
        .and_then(|found| found.extract())
        .unwrap_or_default();
    ChordWeight::named(&name).ok_or_else(|| {
        ChordReducerException::new_err(format!(
            "this package weighs a measure's chords by one of music21's own four \
             algorithms, and {name:?} is none of them"
        ))
    })
}

/// music21's `analysis.reduceChords.ChordReducer`.
#[pyclass(
    name = "ChordReducer",
    module = "music21.analysis.reduceChords",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct ChordReducer;

#[pymethods]
impl ChordReducer {
    /// music21's `ChordReducer()`.
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    /// music21 keeps where in the measure the chord being weighed stands,
    /// and how many elements the measure has, so that its weight
    /// algorithms can read them.
    #[pyo3(signature = (**_keywords))]
    fn __init__(slf: &Bound<'_, Self>, _keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        slf.setattr("positionInMeasure", 0)?;
        slf.setattr("numberOfElementsInMeasure", 0)?;
        Ok(())
    }

    /// The score reduced: music21's `run`, which answers music21's own
    /// score.
    #[pyo3(signature = (
        inputScore,
        allowableChords = None,
        closedPosition = false,
        forbiddenChords = None,
        maximumNumberOfChords = 3
    ))]
    fn run<'py>(
        slf: &Bound<'py, Self>,
        inputScore: &Bound<'py, PyAny>,
        allowableChords: Option<&Bound<'py, PyAny>>,
        closedPosition: bool,
        forbiddenChords: Option<&Bound<'py, PyAny>>,
        maximumNumberOfChords: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        if !crate::stream::answers_to_class(
            inputScore,
            pyo3::types::PyString::new(py, "Score").as_any(),
        )? {
            return Err(ChordReducerException::new_err(
                "Must be called on a stream.Score",
            ));
        }
        let read = |given: Option<&Bound<'py, PyAny>>| -> PyResult<Vec<RsChord>> {
            let Some(given) = given.filter(|given| !given.is_none()) else {
                return Ok(Vec::new());
            };
            let mut found = Vec::new();
            for chord in given.walk()? {
                let chord = chord?;
                if !crate::stream::answers_to_class(
                    &chord,
                    pyo3::types::PyString::new(py, "Chord").as_any(),
                )? {
                    return Err(ChordReducerException::new_err(
                        "All allowableChords must be Chords",
                    ));
                }
                found.push(crate::chord::chord_from_any(Some(&chord))?);
            }
            Ok(found)
        };
        let options = ReductionOptions {
            allowable_chords: read(allowableChords)?,
            forbidden_chords: read(forbiddenChords)?,
            closed_position: closedPosition,
            maximum_number_of_chords: maximumNumberOfChords,
        };
        let score = crate::stream::crate_stream(inputScore)?;
        let made = reduce_chords(&score, &options).map_err(reducer_error)?;
        crate::stream::music21_from_crate(py, &made)
    }

    /// How much each chord of a measure weighs: music21's
    /// `computeMeasureChordWeights`, a dictionary keyed by the pitch
    /// classes each chord sounds.
    #[pyo3(signature = (measureObject, weightAlgorithm = None))]
    fn computeMeasureChordWeights<'py>(
        slf: &Bound<'py, Self>,
        measureObject: &Bound<'py, PyAny>,
        weightAlgorithm: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let algorithm = weight_named(weightAlgorithm)?;
        let read = measure_chords(measureObject)?;
        let chords: Vec<WeighedChord<'_>> = read
            .iter()
            .map(|(chord, offset, strength)| WeighedChord {
                offset: *offset,
                chord,
                beat_strength: *strength,
            })
            .collect();
        let found = PyDict::new(py);
        for (classes, weight) in measure_chord_weights(&chords, algorithm) {
            let key = PyTuple::new(py, classes.iter().map(|class| i64::from(*class)))?;
            found.set_item(key, weight)?;
        }
        Ok(found)
    }

    /// The measure reduced to the few chords weighing most in it:
    /// music21's `reduceMeasureToNChords`, which edits the measure it was
    /// given and hands the same one back.
    #[pyo3(signature = (
        measureObject,
        maximumNumberOfChords = 1,
        weightAlgorithm = None,
        trimBelow = 0.25
    ))]
    fn reduceMeasureToNChords<'py>(
        slf: &Bound<'py, Self>,
        measureObject: &Bound<'py, PyAny>,
        maximumNumberOfChords: usize,
        weightAlgorithm: Option<&Bound<'py, PyAny>>,
        trimBelow: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let algorithm = weight_named(weightAlgorithm)?;
        // music21 weighs the measure flattened, so a chord inside a voice
        // counts; what it then edits is the measure it was given.
        let flattened = measureObject
            .call_method0("flatten")
            .and_then(|flat| flat.getattr("notes"))
            .unwrap_or_else(|_| measureObject.clone());
        let read = measure_chords(&flattened)?;
        let chords: Vec<WeighedChord<'_>> = read
            .iter()
            .map(|(chord, offset, strength)| WeighedChord {
                offset: *offset,
                chord,
                beat_strength: *strength,
            })
            .collect();
        // Where everything that is not a chord stands, which the evening
        // out of syncopations reads.
        let mut others = Vec::new();
        for element in measureObject.walk()? {
            let element = element?;
            if element.getattr("pitches").is_err() {
                others.push(
                    element
                        .getattr("offset")
                        .and_then(|found| found.extract::<f64>())
                        .unwrap_or(0.0),
                );
            }
        }
        let kept = reduced_measure_places(
            &chords,
            &others,
            maximumNumberOfChords,
            trimBelow,
            algorithm,
        );
        // The objects of the measure itself, which is what music21 edits.
        let mut sounding = Vec::new();
        for element in measureObject.walk()? {
            let element = element?;
            if element.getattr("pitches").is_ok() {
                sounding.push(element);
            }
        }
        if kept.is_empty() {
            let length = measureObject
                .getattr("duration")?
                .getattr("quarterLength")?;
            for element in &sounding {
                measureObject.call_method1("remove", (element,))?;
            }
            let keywords = PyDict::new(py);
            keywords.set_item("quarterLength", length)?;
            let rest = crate::class_of(py, "music21.note", "Rest")?.call((), Some(&keywords))?;
            measureObject.call_method1("insert", (0.0, rest))?;
            return Ok(measureObject.clone());
        }
        for (at, element) in sounding.iter().enumerate() {
            match kept.iter().find(|place| place.at == at) {
                None => {
                    measureObject.call_method1("remove", (element,))?;
                }
                Some(place) => {
                    element.setattr("offset", place.offset)?;
                    element.setattr("quarterLength", place.length)?;
                    // music21 unsays what each accidental of a kept chord
                    // had been told to show, the chord having moved.
                    for pitch in element.getattr("pitches")?.walk()? {
                        let accidental = pitch?.getattr("accidental")?;
                        if !accidental.is_none() {
                            accidental.setattr("displayStatus", py.None())?;
                        }
                    }
                }
            }
        }
        Ok(measureObject.clone())
    }

    /// How long a chord sounds: music21's `quarterLengthOnly`.
    fn quarterLengthOnly(slf: &Bound<'_, Self>, chordObject: &Bound<'_, PyAny>) -> PyResult<f64> {
        Self::weighed_by(slf, chordObject, ChordWeight::Length)
    }

    /// That times how strong its beat is: music21's
    /// `quarterLengthBeatStrength`.
    fn quarterLengthBeatStrength(
        slf: &Bound<'_, Self>,
        chordObject: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        Self::weighed_by(slf, chordObject, ChordWeight::LengthByBeat)
    }

    /// The same, the last chord of the measure counting as a full beat:
    /// music21's `quarterLengthBeatStrengthMeasurePosition`.
    fn quarterLengthBeatStrengthMeasurePosition(
        slf: &Bound<'_, Self>,
        chordObject: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        Self::weighed_by(slf, chordObject, ChordWeight::LengthByBeatInMeasure)
    }

    /// The same again, a tenth of it where the chord is dissonant:
    /// music21's `qlbsmpConsonance`, which is what a reduction weighs by.
    fn qlbsmpConsonance(slf: &Bound<'_, Self>, chordObject: &Bound<'_, PyAny>) -> PyResult<f64> {
        Self::weighed_by(slf, chordObject, ChordWeight::Consonance)
    }

    /// music21's `collapseArpeggios`.
    fn collapseArpeggios(&self, _scoreTree: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(wants_a_tree("collapseArpeggios"))
    }

    /// music21's `splitByBass`.
    fn splitByBass(&self, _scoreTree: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(wants_a_tree("splitByBass"))
    }

    /// music21's `removeZeroDurationTimespans`.
    fn removeZeroDurationTimespans(&self, _scoreTree: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(wants_a_tree("removeZeroDurationTimespans"))
    }

    /// music21's `removeVerticalDissonances`.
    #[pyo3(signature = (**_keywords))]
    fn removeVerticalDissonances(&self, _keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        Err(wants_a_tree("removeVerticalDissonances"))
    }

    /// music21's `removeShortTimespans`.
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn removeShortTimespans(
        &self,
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        Err(wants_a_tree("removeShortTimespans"))
    }

    /// music21's `removeNonChordTones`.
    fn removeNonChordTones(&self, _scoreTree: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(wants_a_tree("removeNonChordTones"))
    }

    /// music21's `fillBassGaps`.
    #[pyo3(signature = (*_arguments))]
    fn fillBassGaps(&self, _arguments: &Bound<'_, PyTuple>) -> PyResult<()> {
        Err(wants_a_tree("fillBassGaps"))
    }

    /// music21's `fillMeasureGaps`.
    #[pyo3(signature = (*_arguments))]
    fn fillMeasureGaps(&self, _arguments: &Bound<'_, PyTuple>) -> PyResult<()> {
        Err(wants_a_tree("fillMeasureGaps"))
    }

    /// music21's `fuseTimespansByPart`.
    #[pyo3(signature = (*_arguments))]
    fn fuseTimespansByPart(&self, _arguments: &Bound<'_, PyTuple>) -> PyResult<()> {
        Err(wants_a_tree("fuseTimespansByPart"))
    }

    /// music21's `alignHockets`.
    fn alignHockets(&self, _scoreTree: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(wants_a_tree("alignHockets"))
    }
}

impl ChordReducer {
    /// One chord weighed by an algorithm, in the place the reducer says it
    /// stands: music21 keeps `positionInMeasure` and
    /// `numberOfElementsInMeasure` on itself and its algorithms read them.
    fn weighed_by(
        slf: &Bound<'_, Self>,
        chordObject: &Bound<'_, PyAny>,
        algorithm: ChordWeight,
    ) -> PyResult<f64> {
        let (chord, strength) = weighed(chordObject)?;
        let at: i64 = slf
            .getattr("positionInMeasure")
            .and_then(|found| found.extract())
            .unwrap_or(0);
        let of: i64 = slf
            .getattr("numberOfElementsInMeasure")
            .and_then(|found| found.extract())
            .unwrap_or(0);
        let length = chord
            .duration()
            .map_or(0.0, music21_rs_crate::duration::Duration::quarter_length);
        Ok(algorithm.of(&chord, length, strength, at == of - 1))
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ChordReducer>()?;
    Ok(())
}
