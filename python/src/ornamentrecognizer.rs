//! music21's `alpha.analysis.ornamentRecognizer`: whether a run of notes
//! is an ornament written out, and which.
//!
//! The notes an ornament is played as are its *busy* notes, and the note it
//! is written on, where one is given, its *simple* note. Every question --
//! how long each busy note lasts, whether they play a trill or a turn, and
//! which -- is the crate's `alpha::analysis::ornament_recognizer`, asked of
//! the notes read as values.
//!
//! **The ornament answered is built as music21 builds it.** music21 hands
//! a trill the very accidental object of the note it goes to and resolves
//! its pitches from the note it starts on, so the facade builds the class
//! the crate named -- music21's own where installed -- and writes the length
//! and the nachschlag the crate decided onto it, and that accidental, and
//! has it resolve. Where the crate finds no ornament, the answer is
//! `False`, as music21's is.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyList};

use music21::alpha::analysis::ornament_recognizer::{
    TrillRecognizer as RsTrillRecognizer, TurnRecognizer as RsTurnRecognizer,
    ornament_note_quarter_length, ornament_total_quarter_length,
};
use music21::expressions::{Ornament as RsOrnament, OrnamentKind};
use music21::{IntegerType, StreamElement};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "OrnamentRecognizer",
    "TrillRecognizer",
    "TurnRecognizer",
    "calculateTrillNoteDuration",
];

/// Notes given as a list or a stream, read as the values they are; nothing
/// given is no notes.
fn notes_read(notes: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<StreamElement>> {
    let Some(notes) = notes.filter(|given| !given.is_none()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for note in notes.walk()? {
        let note = note?;
        let Some(value) = crate::stream::element_written(&note)? else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "cannot read {} as a note",
                note.repr()?
            )));
        };
        out.push(value);
    }
    Ok(out)
}

/// The `n`th of the notes given, as the object the caller handed over.
fn nth<'py>(notes: &Bound<'py, PyAny>, n: usize) -> PyResult<Bound<'py, PyAny>> {
    notes.get_item(n)
}

/// A note's MIDI number, as music21 reads it off the note's pitch.
fn midi(note: &Bound<'_, PyAny>) -> PyResult<IntegerType> {
    note.getattr("pitch")?.getattr("midi")?.extract()
}

/// The ornament class of music21's the crate's answer is, built afresh with
/// the length each of its notes lasts.
fn ornament_object<'py>(py: Python<'py>, found: &RsOrnament) -> PyResult<Bound<'py, PyAny>> {
    let name = match found.kind() {
        OrnamentKind::Trill => "Trill",
        OrnamentKind::InvertedTrill => "InvertedTrill",
        OrnamentKind::Turn => "Turn",
        _ => "InvertedTurn",
    };
    let made = crate::class_of(py, "music21.expressions", name)?.call0()?;
    made.setattr(
        "quarterLength",
        crate::duration::op_frac(py, found.quarter_length())?,
    )?;
    Ok(made)
}

/// music21's `OrnamentRecognizer`: how long the notes of an ornament
/// written out last.
#[pyclass(
    name = "OrnamentRecognizer",
    module = "music21.alpha.analysis.ornamentRecognizer",
    subclass,
    skip_from_py_object
)]
pub struct OrnamentRecognizer;

#[pymethods]
impl OrnamentRecognizer {
    #[new]
    fn new() -> Self {
        Self
    }

    /// How long each busy note lasts, the busy notes taken as one
    /// ornament: music21's `calculateOrnamentNoteQl`.
    #[pyo3(signature = (busyNotes, simpleNotes = None))]
    fn calculateOrnamentNoteQl<'py>(
        &self,
        busyNotes: &Bound<'py, PyAny>,
        simpleNotes: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let busy = notes_read(Some(busyNotes))?;
        if busy.is_empty() {
            return Err(pyo3::exceptions::PyZeroDivisionError::new_err(
                "float division by zero",
            ));
        }
        let simple = notes_read(simpleNotes)?;
        crate::duration::op_frac(busyNotes.py(), ornament_note_quarter_length(&busy, &simple))
    }

    /// How long the busy notes last together, or the first simple note
    /// where one is given: music21's `calculateOrnamentTotalQl`.
    #[pyo3(signature = (busyNotes, simpleNotes = None))]
    fn calculateOrnamentTotalQl<'py>(
        &self,
        busyNotes: &Bound<'py, PyAny>,
        simpleNotes: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let busy = notes_read(Some(busyNotes))?;
        let simple = notes_read(simpleNotes)?;
        crate::duration::op_frac(
            busyNotes.py(),
            ornament_total_quarter_length(&busy, &simple),
        )
    }
}

/// music21's `TrillRecognizer`: whether busy notes are a trill written out.
#[pyclass(
    name = "TrillRecognizer",
    module = "music21.alpha.analysis.ornamentRecognizer",
    extends = OrnamentRecognizer,
    subclass,
    skip_from_py_object
)]
pub struct TrillRecognizer {
    /// Whether the last notes may leave the trill as a nachschlag.
    #[pyo3(get, set)]
    checkNachschlag: bool,
    /// The most semitones the two trilled notes may be apart.
    #[pyo3(get, set)]
    acceptableInterval: IntegerType,
    /// The fewest busy notes a trill with a nachschlag may have.
    #[pyo3(get, set)]
    minimumLengthForNachschlag: usize,
}

#[pymethods]
impl TrillRecognizer {
    /// music21's `TrillRecognizer(checkNachschlag=False)`.
    #[new]
    #[pyo3(signature = (checkNachschlag = false))]
    fn new(
        #[pyo3(from_py_with = crate::truthy)] checkNachschlag: bool,
    ) -> PyClassInitializer<Self> {
        let made = RsTrillRecognizer {
            check_nachschlag: checkNachschlag,
            ..RsTrillRecognizer::default()
        };
        crate::base::built(OrnamentRecognizer).add_subclass(Self {
            checkNachschlag: made.check_nachschlag,
            acceptableInterval: made.acceptable_interval,
            minimumLengthForNachschlag: made.minimum_length_for_nachschlag,
        })
    }

    /// The trill the busy notes play, or `False`: music21's `recognize`.
    /// Given a simple note, the trill is from whichever of the two pitches
    /// it sounds.
    #[pyo3(signature = (busyNotes, simpleNotes = None))]
    fn recognize<'py>(
        &self,
        busyNotes: &Bound<'py, PyAny>,
        simpleNotes: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = busyNotes.py();
        let recognizer = RsTrillRecognizer {
            check_nachschlag: self.checkNachschlag,
            acceptable_interval: self.acceptableInterval,
            minimum_length_for_nachschlag: self.minimumLengthForNachschlag,
        };
        let simple = notes_read(simpleNotes)?;
        let found = recognizer
            .recognize(&notes_read(Some(busyNotes))?, &simple)
            .map_err(|error| {
                pyo3::exceptions::PyAttributeError::new_err(crate::pitch::message(&error))
            })?;
        let Some(found) = found else {
            return Ok(PyBool::new(py, false).to_owned().into_any());
        };

        let (mut start, mut end) = (nth(busyNotes, 0)?, nth(busyNotes, 1)?);
        if let Some(simple_notes) = simpleNotes.filter(|_| !simple.is_empty())
            && midi(&nth(simple_notes, 0)?)? == midi(&end)?
        {
            std::mem::swap(&mut start, &mut end);
        }
        let trill = ornament_object(py, &found)?;
        if found.nachschlag() {
            trill.setattr("nachschlag", true)?;
        }
        let accidental = end.getattr("pitch")?.getattr("accidental")?;
        if !accidental.is_none() {
            trill.setattr("accidental", accidental)?;
        }
        trill.call_method1("resolveOrnamentalPitches", (start,))?;
        Ok(trill)
    }
}

/// music21's `TurnRecognizer`: whether busy notes are a turn written out.
#[pyclass(
    name = "TurnRecognizer",
    module = "music21.alpha.analysis.ornamentRecognizer",
    extends = OrnamentRecognizer,
    subclass,
    skip_from_py_object
)]
pub struct TurnRecognizer {
    /// Kept as music21 keeps it; no turn reads it.
    #[pyo3(get, set)]
    acceptableInterval: IntegerType,
    /// Kept as music21 keeps it; no turn reads it.
    #[pyo3(get, set)]
    minimumLengthForNachschlag: usize,
    /// The steps a turn may take, read afresh at each question.
    #[pyo3(get, set)]
    acceptableIntervals: Py<PyAny>,
}

impl TurnRecognizer {
    /// The recognizer as the crate keeps one, its steps read off the list
    /// as it stands.
    fn read(&self, py: Python<'_>) -> PyResult<RsTurnRecognizer> {
        let mut steps = Vec::new();
        for step in self.acceptableIntervals.bind(py).walk()? {
            steps.push(crate::interval::interval_from_any(&step?)?);
        }
        Ok(RsTurnRecognizer {
            acceptable_intervals: steps,
        })
    }
}

#[pymethods]
impl TurnRecognizer {
    /// music21's `TurnRecognizer()`, which takes minor, major and
    /// augmented seconds each way.
    #[new]
    fn new(py: Python<'_>) -> PyResult<PyClassInitializer<Self>> {
        let steps = PyList::empty(py);
        for step in RsTurnRecognizer::default().acceptable_intervals {
            steps.append(crate::interval::interval_object(py, step)?)?;
        }
        Ok(crate::base::built(OrnamentRecognizer).add_subclass(Self {
            acceptableInterval: 3,
            minimumLengthForNachschlag: 6,
            acceptableIntervals: steps.into_any().unbind(),
        }))
    }

    /// Whether a turn may take this step: music21's
    /// `isAcceptableInterval`. Anything but an interval is not one.
    fn isAcceptableInterval(&self, intervalToCheck: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !intervalToCheck.hasattr("directedName")? {
            return Ok(false);
        }
        let step = crate::interval::interval_from_any(intervalToCheck)?;
        Ok(self
            .read(intervalToCheck.py())?
            .is_acceptable_interval(&step))
    }

    /// The turn the busy notes play, or `False`: music21's `recognize`.
    #[pyo3(signature = (busyNotes, simpleNotes = None))]
    fn recognize<'py>(
        &self,
        busyNotes: &Bound<'py, PyAny>,
        simpleNotes: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = busyNotes.py();
        let found = self
            .read(py)?
            .recognize(&notes_read(Some(busyNotes))?, &notes_read(simpleNotes)?)
            .map_err(|error| {
                pyo3::exceptions::PyAttributeError::new_err(crate::pitch::message(&error))
            })?;
        match found {
            Some(found) => ornament_object(py, &found),
            None => Ok(PyBool::new(py, false).to_owned().into_any()),
        }
    }
}

/// How long each of so many trill notes lasts in a length: music21's
/// `calculateTrillNoteDuration`.
#[pyfunction]
fn calculateTrillNoteDuration<'py>(
    numTrillNotes: usize,
    totalDuration: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if numTrillNotes == 0 {
        return Err(pyo3::exceptions::PyZeroDivisionError::new_err(
            "division by zero",
        ));
    }
    let total: f64 = totalDuration.getattr("quarterLength")?.extract()?;
    crate::duration::op_frac(totalDuration.py(), total / numTrillNotes as f64)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<OrnamentRecognizer>()?;
    m.add_class::<TrillRecognizer>()?;
    m.add_class::<TurnRecognizer>()?;
    m.add_function(wrap_pyfunction!(calculateTrillNoteDuration, m)?)?;
    Ok(())
}
