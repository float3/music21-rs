//! music21's `chord.Chord` over `music21-rs`, with music21's names,
//! properties and `repr`.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyException, PyIndexError, PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs::{
    Chord as RsChord, Duration as RsDuration, Interval as RsInterval, Note as RsNote,
    Pitch as RsPitch, Scale, ScaleType, Volume as RsVolume,
};

use crate::interval::interval_from_any;
use crate::notation::{Lyric, Style, StyleOwner, Tie, Volume, tie_from_any, volume_from_any};
use crate::note::{Duration, Note, duration_from_any, note_from_any};
use crate::pitch::{Accidental, Pitch, message, pitch_from_any};

pyo3::create_exception!(music21_rs_facade, ChordException, PyException);

fn chord_error(error: music21_rs::Error) -> PyErr {
    ChordException::new_err(message(&error))
}

/// music21's `chord.Chord`.
#[pyclass(
    name = "Chord",
    module = "music21.chord",
    subclass,
    skip_from_py_object
)]
pub struct Chord {
    /// The pitches and the analysis read off them. Every method that asks a
    /// musical question goes through this.
    pub(crate) inner: RsChord,
    /// The `Note` objects music21 hands back from `chord[i]` and `.notes`.
    /// They are the same objects every time, so notation written through one
    /// of them sticks; their pitches mirror `inner`, and structural changes
    /// rebuild them from it.
    notes: Vec<Py<Note>>,
    /// The chord's own `Duration`, kept as the Python object music21 hands
    /// back so `chord.duration is d` holds and edits through it stick.
    duration: Option<Py<Duration>>,
    /// The chord's own `Volume`, likewise.
    volume: Option<Py<Volume>>,
}

impl Chord {
    /// Builds the facade around a chord, giving each of its notes a Python
    /// object of its own.
    pub(crate) fn from_inner(py: Python<'_>, inner: RsChord) -> PyResult<Self> {
        let mut chord = Self {
            inner,
            notes: Vec::new(),
            duration: None,
            volume: None,
        };
        chord.rebuild_notes(py)?;
        Ok(chord)
    }

    /// Replaces the pitches and everything read off them, rebuilding the
    /// note objects to match. Notation on the old notes does not survive a
    /// structural change, which is what music21 does too.
    fn replace_inner(&mut self, py: Python<'_>, inner: RsChord) -> PyResult<()> {
        self.inner = inner;
        self.rebuild_notes(py)
    }

    fn rebuild_notes(&mut self, py: Python<'_>) -> PyResult<()> {
        self.notes = self
            .inner
            .notes()
            .iter()
            .cloned()
            .map(|note| Py::new(py, Note::wrap(note)))
            .collect::<PyResult<_>>()?;
        Ok(())
    }

    /// The chord with the notation its note objects carry written back onto
    /// it, for the few questions that read notation rather than pitch.
    fn with_note_notation(&self, py: Python<'_>) -> PyResult<RsChord> {
        let notes: Vec<RsNote> = self
            .notes
            .iter()
            .map(|note| note.borrow(py).inner.clone())
            .collect();
        let mut chord = RsChord::new(notes.as_slice()).map_err(chord_error)?;
        if let Some(duration) = self.inner.duration() {
            chord.set_duration(duration.clone());
        }
        Ok(chord)
    }

    /// The note object holding the first pitch that matches, the way
    /// music21's per-note accessors take a pitch.
    /// music21 hands back the pitches it dropped when reducing in place,
    /// and the reduced chord when not.
    fn deliver_reduced(
        &mut self,
        py: Python<'_>,
        reduced: RsChord,
        removed: Vec<RsPitch>,
        in_place: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        if in_place {
            self.replace_inner(py, reduced)?;
            let dropped: Vec<Pitch> = removed
                .into_iter()
                .map(|pitch| Pitch::wrap(pitch, false))
                .collect();
            return Ok(Some(PyList::new(py, dropped)?.into_any().unbind()));
        }
        Ok(Some(
            Py::new(py, Self::from_inner(py, reduced)?)?.into_any(),
        ))
    }

    /// The note a per-note setter writes to: the one the target names, or
    /// the first note when music21 lets the target be left out.
    fn first_or_named(
        &self,
        py: Python<'_>,
        target: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<Note>> {
        match target.filter(|target| !target.is_none()) {
            Some(target) => self.note_object(py, target),
            None => self
                .notes
                .first()
                .map(|note| note.clone_ref(py))
                .ok_or_else(|| ChordException::new_err("the chord has no notes")),
        }
    }

    fn note_object(&self, py: Python<'_>, target: &Bound<'_, PyAny>) -> PyResult<Py<Note>> {
        let wanted = if let Ok(name) = target.extract::<String>() {
            RsPitch::from_name(name).map_err(chord_error)?
        } else if let Ok(index) = target.extract::<usize>() {
            return self
                .notes
                .get(index)
                .map(|note| note.clone_ref(py))
                .ok_or_else(|| PyIndexError::new_err("list index out of range"));
        } else {
            pitch_from_any(target)?
        };
        self.notes
            .iter()
            .find(|note| {
                note.borrow(py).inner.pitch().name_with_octave() == wanted.name_with_octave()
            })
            .or_else(|| {
                self.notes
                    .iter()
                    .find(|note| note.borrow(py).inner.pitch().name() == wanted.name())
            })
            .map(|note| note.clone_ref(py))
            .ok_or_else(|| {
                ChordException::new_err(format!(
                    "the given pitch is not in the Chord: {}",
                    wanted.name_with_octave()
                ))
            })
    }

    fn duration_object(&mut self, py: Python<'_>) -> PyResult<Py<Duration>> {
        if let Some(duration) = &self.duration {
            return Ok(duration.clone_ref(py));
        }
        let created = Py::new(
            py,
            Duration::wrap(
                self.inner
                    .duration()
                    .cloned()
                    .unwrap_or_else(RsDuration::quarter),
            ),
        )?;
        self.duration = Some(created.clone_ref(py));
        Ok(created)
    }

    fn quarter_length(&self, py: Python<'_>) -> f64 {
        match &self.duration {
            Some(duration) => duration.borrow(py).inner.quarter_length(),
            None => self
                .inner
                .duration()
                .map_or(1.0, music21_rs::Duration::quarter_length),
        }
    }

    fn pitch_facades(&self) -> Vec<Pitch> {
        self.inner
            .pitches()
            .into_iter()
            .map(|pitch| Pitch::wrap(pitch, false))
            .collect()
    }

    fn optional_pitch(pitch: Option<&RsPitch>) -> Option<Pitch> {
        pitch.cloned().map(|pitch| Pitch::wrap(pitch, false))
    }
}

/// Reads whatever music21's `Chord(...)` accepts: a space-separated string, a
/// sequence of names, pitches, notes, chords or MIDI numbers, or nothing.
/// A sequence of plain integers is spelled the way music21 spells one, which
/// is why it does not go through the note path.
fn chord_from_any(value: Option<&Bound<'_, PyAny>>) -> PyResult<RsChord> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(RsChord::empty());
    };
    if let Ok(text) = value.extract::<String>() {
        return RsChord::new(text).map_err(chord_error);
    }
    let items: Vec<Bound<'_, PyAny>> = value
        .try_iter()
        .map_err(|_| ChordException::new_err("Chord needs a string or a sequence"))?
        .collect::<PyResult<_>>()?;
    let all_integers = !items.is_empty()
        && items.iter().all(|item| {
            item.extract::<String>().is_err()
                && item.extract::<i32>().is_ok()
                && item.extract::<PyRef<Pitch>>().is_err()
        });
    if all_integers {
        let midi: Vec<i32> = items
            .iter()
            .map(|item| item.extract::<i32>())
            .collect::<PyResult<_>>()?;
        return RsChord::new(midi).map_err(chord_error);
    }
    let mut notes: Vec<RsNote> = Vec::with_capacity(items.len());
    for item in &items {
        if let Ok(chord) = item.extract::<PyRef<Chord>>() {
            notes.extend(chord.inner.notes().iter().cloned());
        } else {
            notes.push(note_from_any(item).map_err(|_| {
                PyTypeError::new_err(format!(
                    "Could not process input argument {}",
                    item.repr()
                        .map_or_else(|_| "?".to_string(), |r| r.to_string())
                ))
            })?);
        }
    }
    RsChord::new(notes.as_slice()).map_err(chord_error)
}

/// pyo3 hands a `Vec<u8>` to Python as `bytes`; pitch-class lists must come
/// back as a list of numbers, so they travel as `u32`.
fn into_numbers(values: Vec<u8>) -> Vec<u32> {
    values.into_iter().map(u32::from).collect()
}

/// music21 refuses a bare string where a sequence of notes or pitches is
/// wanted, since a string is itself iterable and would come apart letter by
/// letter.
fn require_iterable(value: &Bound<'_, PyAny>, field: &str) -> PyResult<()> {
    if value.extract::<String>().is_ok() || value.try_iter().is_err() {
        return Err(PyTypeError::new_err(format!(
            "{field} must be set with an iterable"
        )));
    }
    Ok(())
}

fn scale_of(key: &Bound<'_, PyAny>) -> PyResult<Scale> {
    let tonic = pitch_from_any(&key.getattr("tonic")?)?;
    let mode: String = key
        .getattr("mode")
        .and_then(|mode| mode.extract::<String>())
        .unwrap_or_else(|_| "major".to_string());
    let scale_type = match mode.as_str() {
        "minor" => ScaleType::Minor,
        _ => ScaleType::Major,
    };
    Ok(Scale::new(scale_type, tonic))
}

#[pymethods]
impl Chord {
    #[new]
    #[pyo3(signature = (notes = None, **keywords))]
    fn new(
        py: Python<'_>,
        notes: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut chord = Self::from_inner(py, chord_from_any(notes)?)?;
        if let Some(keywords) = keywords {
            if let Some(value) = keywords.get_item("quarterLength")? {
                chord.set_quarterLength(keywords.py(), value.extract::<f64>()?)?;
            }
            if let Some(value) = keywords.get_item("duration")? {
                chord.set_duration(&value)?;
            }
        }
        Ok(chord)
    }

    // ---- contents --------------------------------------------------------

    #[getter]
    fn get_pitches<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.pitch_facades())
    }

    #[setter]
    fn set_pitches(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        require_iterable(value, "pitches")?;
        self.inner = chord_from_any(Some(value))?;
        Ok(())
    }

    #[setter]
    fn set_pitchNames(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        require_iterable(value, "pitchNames")?;
        self.inner = chord_from_any(Some(value))?;
        Ok(())
    }

    #[getter]
    fn get_notes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(
            py,
            self.notes
                .iter()
                .map(|note| note.clone_ref(py))
                .collect::<Vec<_>>(),
        )
    }

    #[setter]
    fn set_notes(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        require_iterable(value, "notes")?;
        let mut notes: Vec<RsNote> = Vec::new();
        for item in value.try_iter()? {
            let item = item?;
            let note = item.extract::<PyRef<Note>>().map_err(|_| {
                PyTypeError::new_err("every element of notes must be a note.Note object")
            })?;
            notes.push(note.inner.clone());
        }
        self.inner = RsChord::new(notes.as_slice()).map_err(chord_error)?;
        Ok(())
    }

    #[getter]
    fn pitchNames(&self) -> Vec<String> {
        self.inner.pitch_names()
    }

    #[getter]
    fn pitchClasses(&self) -> Vec<u32> {
        into_numbers(self.inner.note_pitch_classes())
    }

    #[getter]
    fn orderedPitchClasses(&self) -> Vec<u32> {
        into_numbers(self.inner.pitch_classes())
    }

    #[getter]
    fn orderedPitchClassesString(&self) -> String {
        self.inner.ordered_pitch_classes_string()
    }

    #[getter]
    fn multisetCardinality(&self) -> usize {
        self.inner.multiset_cardinality()
    }

    #[getter]
    fn pitchClassCardinality(&self) -> usize {
        self.inner.pitch_class_cardinality()
    }

    fn __len__(&self) -> usize {
        self.inner.notes().len()
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<Note>> {
        if let Ok(index) = key.extract::<isize>() {
            let length = self.notes.len() as isize;
            let resolved = if index < 0 { index + length } else { index };
            if resolved < 0 || resolved >= length {
                return Err(PyIndexError::new_err("list index out of range"));
            }
            return Ok(self.notes[resolved as usize].clone_ref(py));
        }
        let wanted = if let Ok(name) = key.extract::<String>() {
            name.to_uppercase()
        } else {
            pitch_from_any(key)?.name_with_octave()
        };
        self.notes
            .iter()
            .find(|note| note.borrow(py).inner.pitch().name_with_octave() == wanted)
            .map(|note| note.clone_ref(py))
            .ok_or_else(|| {
                PyKeyError::new_err(format!(
                    "No note in the chord matches {}",
                    key.repr()
                        .map_or_else(|_| wanted.clone(), |r| r.to_string())
                ))
            })
    }

    fn __setitem__(&mut self, key: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let length = self.inner.notes().len() as isize;
        let resolved = match key.extract::<isize>() {
            Ok(index) if index < 0 => index + length,
            Ok(index) => index,
            Err(_) => {
                let wanted = if let Ok(name) = key.extract::<String>() {
                    name.to_uppercase()
                } else {
                    pitch_from_any(key)?.name_with_octave()
                };
                self.inner
                    .notes()
                    .iter()
                    .position(|note| note.pitch().name_with_octave() == wanted)
                    .map(|index| index as isize)
                    .ok_or_else(|| {
                        PyKeyError::new_err(format!("No note in the chord matches {wanted}"))
                    })?
            }
        };
        if resolved < 0 || resolved >= length {
            return Err(PyIndexError::new_err("list index out of range"));
        }
        let note = note_from_any(value)
            .map_err(|_| PyValueError::new_err("Chord index must be set to a valid note object"))?;
        let mut notes = self.inner.notes().to_vec();
        notes[resolved as usize] = note;
        let duration = self.inner.duration().cloned();
        self.inner = RsChord::new(notes.as_slice()).map_err(chord_error)?;
        if let Some(duration) = duration {
            self.inner.set_duration(duration);
        }
        Ok(())
    }

    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let notes = slf.borrow().get_notes(py)?;
        notes.into_any().try_iter().map(Bound::into_any)
    }

    #[pyo3(signature = (notes, *, runSort = true))]
    fn add(&mut self, py: Python<'_>, notes: &Bound<'_, PyAny>, runSort: bool) -> PyResult<()> {
        let added = if notes.extract::<String>().is_ok() || notes.try_iter().is_err() {
            vec![note_from_any(notes)?]
        } else {
            chord_from_any(Some(notes))?.notes().to_vec()
        };
        let mut grown = self.with_note_notation(py)?;
        grown.add(added.as_slice()).map_err(chord_error)?;
        if runSort {
            grown = grown.sort_ascending();
        }
        self.replace_inner(py, grown)
    }

    fn remove(&mut self, py: Python<'_>, removeItem: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut reduced = self.with_note_notation(py)?;
        let result = if let Ok(name) = removeItem.extract::<String>() {
            reduced.remove_named(&name)
        } else {
            reduced.remove(&pitch_from_any(removeItem)?)
        };
        result.map_err(|error| PyValueError::new_err(message(&error)))?;
        self.replace_inner(py, reduced)
    }

    // ---- duration --------------------------------------------------------

    #[getter]
    fn get_duration(&mut self, py: Python<'_>) -> PyResult<Py<Duration>> {
        self.duration_object(py)
    }

    #[setter]
    fn set_duration(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let inner = duration_from_any(value)?;
        self.inner.set_duration(inner.clone());
        self.duration = match value.extract::<Py<Duration>>() {
            Ok(object) => Some(object),
            Err(_) => Some(Py::new(value.py(), Duration::wrap(inner))?),
        };
        Ok(())
    }

    #[getter]
    fn get_quarterLength(&self, py: Python<'_>) -> f64 {
        self.quarter_length(py)
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        let inner = RsDuration::new(value).map_err(chord_error)?;
        self.inner.set_duration(inner.clone());
        match &self.duration {
            Some(duration) => duration.borrow_mut(py).inner = inner,
            None => self.duration = Some(Py::new(py, Duration::wrap(inner))?),
        }
        Ok(())
    }

    // ---- names -----------------------------------------------------------

    #[getter]
    fn commonName(&self) -> String {
        self.inner.common_name()
    }

    #[getter]
    fn pitchedCommonName(&self) -> String {
        self.inner.pitched_common_name()
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner.full_name()
    }

    #[getter]
    fn quality(&self) -> &'static str {
        self.inner.quality().as_str()
    }

    // ---- members ---------------------------------------------------------

    #[pyo3(signature = (newroot = None, *, find = None))]
    fn root(
        &mut self,
        newroot: Option<&Bound<'_, PyAny>>,
        find: Option<bool>,
    ) -> PyResult<Option<Pitch>> {
        if let Some(newroot) = newroot.filter(|value| !value.is_none()) {
            self.inner.set_root(Some(pitch_from_any(newroot)?));
            return Ok(None);
        }
        match find {
            Some(true) => {
                self.inner.set_root(None);
                Ok(Self::optional_pitch(self.inner.found_root()))
            }
            Some(false) => Ok(Self::optional_pitch(self.inner.root())),
            None => Ok(Self::optional_pitch(self.inner.root())),
        }
    }

    #[pyo3(signature = (newbass = None, *, find = None, allow_add = false))]
    fn bass(
        &mut self,
        newbass: Option<&Bound<'_, PyAny>>,
        find: Option<bool>,
        allow_add: bool,
    ) -> PyResult<Option<Pitch>> {
        if let Some(newbass) = newbass.filter(|value| !value.is_none()) {
            let bass = pitch_from_any(newbass)?;
            let known = self
                .inner
                .pitches()
                .iter()
                .any(|pitch| pitch.name() == bass.name());
            if !known && !allow_add {
                return Err(ChordException::new_err(format!(
                    "Pitch {} not found in chord",
                    bass.name_with_octave()
                )));
            }
            self.inner.set_bass(Some(bass));
            return Ok(None);
        }
        match find {
            Some(true) => {
                self.inner.set_bass(None);
                Ok(Self::optional_pitch(self.inner.found_bass()))
            }
            Some(false) => Ok(Self::optional_pitch(self.inner.overridden_bass())),
            None => Ok(Self::optional_pitch(self.inner.bass())),
        }
    }

    #[getter]
    fn third(&self) -> Option<Pitch> {
        Self::optional_pitch(self.inner.third())
    }

    #[getter]
    fn fifth(&self) -> Option<Pitch> {
        Self::optional_pitch(self.inner.fifth())
    }

    #[getter]
    fn seventh(&self) -> Option<Pitch> {
        Self::optional_pitch(self.inner.seventh())
    }

    #[pyo3(signature = (chordStep, testRoot = None))]
    fn getChordStep(
        &self,
        chordStep: u8,
        testRoot: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Pitch>> {
        match testRoot.filter(|value| !value.is_none()) {
            Some(testRoot) => {
                let root = pitch_from_any(testRoot)?;
                Ok(Self::optional_pitch(
                    self.inner.chord_step_with_root(chordStep, &root),
                ))
            }
            None => {
                if self.inner.root().is_none() {
                    return Err(ChordException::new_err(
                        "Cannot run getChordStep without a root",
                    ));
                }
                Ok(Self::optional_pitch(self.inner.chord_step(chordStep)))
            }
        }
    }

    #[pyo3(signature = (chordStep, testRoot = None))]
    fn semitonesFromChordStep(
        &self,
        chordStep: u8,
        testRoot: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<u8>> {
        match testRoot.filter(|value| !value.is_none()) {
            Some(testRoot) => {
                let root = pitch_from_any(testRoot)?;
                Ok(self
                    .inner
                    .semitones_from_chord_step_with_root(chordStep, &root))
            }
            None => Ok(self.inner.semitones_from_chord_step(chordStep)),
        }
    }

    fn isSeventhOfType(&self, intervalArray: Vec<u32>) -> bool {
        let array: Vec<u8> = intervalArray.iter().map(|value| *value as u8).collect();
        self.inner.is_seventh_of_type(&array)
    }

    /// music21's `formatVectorString`, which is a static method there and
    /// works on any list of pitch classes.
    #[staticmethod]
    fn formatVectorString(vectorList: Vec<u32>) -> String {
        let values: Vec<u8> = vectorList.iter().map(|value| *value as u8).collect();
        music21_rs::format_vector_string(&values)
    }

    fn intervalFromChordStep(&self, chordStep: u8) -> Option<crate::interval::Interval> {
        self.inner
            .interval_from_chord_step(chordStep)
            .map(crate::interval::Interval::wrap)
    }

    fn hasRepeatedChordStep(&self, chordStep: u8) -> bool {
        self.inner.has_repeated_chord_step(chordStep)
    }

    fn containsTriad(&self) -> bool {
        self.inner.contains_triad()
    }

    fn containsSeventh(&self) -> bool {
        self.inner.contains_seventh()
    }

    fn hasAnyRepeatedDiatonicNote(&self) -> bool {
        self.inner.has_any_repeated_diatonic_note()
    }

    fn hasAnyEnharmonicSpelledPitches(&self) -> bool {
        self.inner.has_any_enharmonic_spelled_pitches()
    }

    // ---- inversion -------------------------------------------------------

    #[pyo3(signature = (newInversion = None, *, find = true, testRoot = None, transposeOnSet = true))]
    fn inversion(
        &mut self,
        newInversion: Option<&Bound<'_, PyAny>>,
        find: bool,
        testRoot: Option<&Bound<'_, PyAny>>,
        transposeOnSet: bool,
    ) -> PyResult<Option<i32>> {
        let _ = find;
        if self.inner.pitches().is_empty() {
            return Ok(Some(-1));
        }
        if let Some(newInversion) = newInversion.filter(|value| !value.is_none()) {
            let Ok(inversion) = newInversion.extract::<i32>() else {
                return Err(ChordException::new_err(format!(
                    "Inversion must be an integer, got: {}",
                    newInversion.get_type()
                )));
            };
            if !transposeOnSet {
                return Err(ChordException::new_err(
                    "music21-rs reads the inversion off the pitches, so it cannot record one \
                     without transposing",
                ));
            }
            let inversion = u8::try_from(inversion).map_err(|_| {
                ChordException::new_err("Could not invert chord: inversion may not exist")
            })?;
            self.inner.set_inversion(inversion).map_err(chord_error)?;
            return Ok(None);
        }
        if let Some(testRoot) = testRoot.filter(|value| !value.is_none()) {
            let root = pitch_from_any(testRoot)?;
            return Ok(Some(
                self.inner.inversion_from_root(&root).map_or(-1, i32::from),
            ));
        }
        Ok(Some(self.inner.inversion().map_or(-1, i32::from)))
    }

    fn inversionName(&self) -> PyResult<Option<i32>> {
        self.inner.inversion_name().map_err(chord_error)
    }

    fn inversionText(&self) -> String {
        self.inner.inversion_text()
    }

    // ---- set theory ------------------------------------------------------

    #[getter]
    fn normalOrder(&self) -> Vec<u32> {
        into_numbers(self.inner.normal_order())
    }

    #[getter]
    fn normalOrderString(&self) -> String {
        self.inner.normal_order_string()
    }

    #[getter]
    fn primeForm(&self) -> Vec<u32> {
        into_numbers(self.inner.prime_form())
    }

    #[getter]
    fn primeFormString(&self) -> String {
        self.inner.prime_form_string()
    }

    #[getter]
    fn intervalVector(&self) -> Vec<u32> {
        self.inner
            .interval_class_vector()
            .map(into_numbers)
            .unwrap_or_else(|| vec![0; 6])
    }

    #[getter]
    fn intervalVectorString(&self) -> String {
        self.inner.interval_vector_string()
    }

    #[getter]
    fn forteClass(&self) -> String {
        self.inner
            .forte_class()
            .unwrap_or_else(|| "N/A".to_string())
    }

    #[getter]
    fn forteClassNumber(&self) -> Option<u8> {
        self.inner.forte_class_number()
    }

    #[getter]
    fn forteClassTn(&self) -> String {
        self.inner
            .forte_class_tn()
            .unwrap_or_else(|| "N/A".to_string())
    }

    #[getter]
    fn forteClassTnI(&self) -> String {
        self.inner
            .forte_class_tni()
            .unwrap_or_else(|| "N/A".to_string())
    }

    #[getter]
    fn geometricNormalForm(&self) -> Vec<u32> {
        into_numbers(self.inner.geometric_normal_form())
    }

    #[getter]
    fn hasZRelation(&self) -> bool {
        self.inner.has_z_relation()
    }

    fn getZRelation(&self, py: Python<'_>) -> PyResult<Option<Chord>> {
        match self
            .inner
            .z_relation()
            .and_then(|name| RsChord::from_forte_class(&name).ok())
        {
            Some(related) => Ok(Some(Self::from_inner(py, related)?)),
            None => Ok(None),
        }
    }

    fn areZRelations(&self, other: &Chord) -> bool {
        self.inner.are_z_relations(&other.inner)
    }

    #[getter]
    fn isPrimeFormInversion(&self) -> bool {
        self.inner.is_prime_form_inversion()
    }

    #[pyo3(signature = (*, requireIntervallicEvenness = false))]
    fn isTranspositionallySymmetrical(&self, requireIntervallicEvenness: bool) -> bool {
        self.inner
            .is_transpositionally_symmetrical(requireIntervallicEvenness)
    }

    // ---- predicates ------------------------------------------------------

    fn isTriad(&self) -> bool {
        self.inner.is_triad()
    }

    fn isSeventh(&self) -> bool {
        self.inner.is_seventh()
    }

    fn isMajorTriad(&self) -> bool {
        self.inner.is_major_triad()
    }

    fn isMinorTriad(&self) -> bool {
        self.inner.is_minor_triad()
    }

    fn isDiminishedTriad(&self) -> bool {
        self.inner.is_diminished_triad()
    }

    fn isAugmentedTriad(&self) -> bool {
        self.inner.is_augmented_triad()
    }

    fn isDominantSeventh(&self) -> bool {
        self.inner.is_dominant_seventh()
    }

    fn isDiminishedSeventh(&self) -> bool {
        self.inner.is_diminished_seventh()
    }

    fn isHalfDiminishedSeventh(&self) -> bool {
        self.inner.is_half_diminished_seventh()
    }

    fn isFalseDiminishedSeventh(&self) -> bool {
        self.inner.is_false_diminished_seventh()
    }

    fn isIncompleteMajorTriad(&self) -> bool {
        self.inner.is_incomplete_major_triad()
    }

    fn isIncompleteMinorTriad(&self) -> bool {
        self.inner.is_incomplete_minor_triad()
    }

    fn isConsonant(&self) -> bool {
        self.inner.is_consonant()
    }

    fn isNinth(&self) -> bool {
        self.inner.is_ninth()
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_augmented_sixth(permitAnyInversion)
    }

    #[pyo3(signature = (*, restrictDoublings = false, permitAnyInversion = false))]
    fn isItalianAugmentedSixth(&self, restrictDoublings: bool, permitAnyInversion: bool) -> bool {
        self.inner
            .is_italian_augmented_sixth(permitAnyInversion, restrictDoublings)
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isFrenchAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_french_augmented_sixth(permitAnyInversion)
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isGermanAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_german_augmented_sixth(permitAnyInversion)
    }

    #[pyo3(signature = (*, permitAnyInversion = false))]
    fn isSwissAugmentedSixth(&self, permitAnyInversion: bool) -> bool {
        self.inner.is_swiss_augmented_sixth(permitAnyInversion)
    }

    fn canBeDominantV(&self) -> bool {
        self.inner.can_be_dominant_v()
    }

    fn canBeTonic(&self) -> bool {
        self.inner.can_be_tonic()
    }

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isChord(&self) -> bool {
        true
    }

    #[getter]
    fn isNote(&self) -> bool {
        false
    }

    // ---- reshaping -------------------------------------------------------

    #[pyo3(signature = (forceOctave = None, *, inPlace = false, leaveRedundantPitches = false))]
    fn closedPosition(
        &mut self,
        py: Python<'_>,
        forceOctave: Option<i32>,
        inPlace: bool,
        leaveRedundantPitches: bool,
    ) -> PyResult<Option<Chord>> {
        let closed = self
            .inner
            .closed_position(forceOctave, leaveRedundantPitches);
        if inPlace {
            self.replace_inner(py, closed)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, closed)?))
        }
    }

    #[pyo3(signature = (forceOctave = None, *, inPlace = false, leaveRedundantPitches = false))]
    fn semiClosedPosition(
        &mut self,
        py: Python<'_>,
        forceOctave: Option<i32>,
        inPlace: bool,
        leaveRedundantPitches: bool,
    ) -> PyResult<Option<Chord>> {
        let moved = self
            .inner
            .semi_closed_position(forceOctave, leaveRedundantPitches);
        if inPlace {
            self.replace_inner(py, moved)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, moved)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn removeRedundantPitches(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let (reduced, removed) = self.inner.remove_redundant_pitches_reporting();
        self.deliver_reduced(py, reduced, removed, inPlace)
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn removeRedundantPitchNames(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let (reduced, removed) = self.inner.remove_redundant_pitch_names_reporting();
        self.deliver_reduced(py, reduced, removed, inPlace)
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn removeRedundantPitchClasses(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let (reduced, removed) = self.inner.remove_redundant_pitch_classes_reporting();
        self.deliver_reduced(py, reduced, removed, inPlace)
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortDiatonicAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_diatonic_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortChromaticAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_chromatic_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (*, inPlace = false))]
    fn sortFrequencyAscending(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Chord>> {
        let sorted = self.inner.sort_frequency_ascending();
        if inPlace {
            self.replace_inner(py, sorted)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, sorted)?))
        }
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        &mut self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Chord>> {
        let interval: RsInterval = interval_from_any(value)?;
        let moved = self.inner.transpose(&interval).map_err(chord_error)?;
        if inPlace {
            self.replace_inner(py, moved)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, moved)?))
        }
    }

    /// music21's `scaleDegrees`: each pitch's degree in the given key, with
    /// the accidental that alters it.
    fn scaleDegrees<'py>(
        &self,
        py: Python<'py>,
        scaleObj: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<(Option<usize>, Option<Accidental>)>> {
        let _ = py;
        let scale = scale_of(scaleObj)?;
        Ok(self
            .inner
            .scale_degrees(&scale)
            .map_err(chord_error)?
            .into_iter()
            .map(|(degree, accidental)| (degree, accidental.map(Accidental::from_inner)))
            .collect())
    }

    // ---- notation --------------------------------------------------------

    #[getter]
    fn style(slf: &Bound<'_, Self>) -> Style {
        Style {
            owner: StyleOwner::Chord(slf.clone().unbind()),
        }
    }

    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.inner.color().is_some()
    }

    /// music21's `getColor`: the note's own colour when it has one, and the
    /// chord's otherwise.
    fn getColor(&self, py: Python<'_>, pitchTarget: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
        let note = self.note_object(py, pitchTarget)?;
        let color = note.borrow(py).inner.color().map(str::to_string);
        Ok(color.or_else(|| self.inner.color().map(str::to_string)))
    }

    #[pyo3(signature = (value, pitchTarget = None))]
    fn setColor(
        &mut self,
        py: Python<'_>,
        value: Option<String>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        match pitchTarget.filter(|target| !target.is_none()) {
            None => {
                self.inner.set_color(value);
                Ok(())
            }
            Some(target) => {
                let note = self.note_object(py, target)?;
                note.borrow_mut(py).inner.set_color(value);
                Ok(())
            }
        }
    }

    fn getNotehead(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<&'static str> {
        let note = self.note_object(py, p)?;
        let notehead = note.borrow(py).inner.notehead();
        Ok(notehead.as_str())
    }

    #[pyo3(signature = (nh, pitchTarget = None))]
    fn setNotehead(
        &mut self,
        py: Python<'_>,
        nh: Option<&str>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_notehead(nh)
    }

    fn getNoteheadFill(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<Option<bool>> {
        let note = self.note_object(py, p)?;
        let fill = note.borrow(py).inner.notehead_fill();
        Ok(fill)
    }

    #[pyo3(signature = (nh, pitchTarget = None))]
    fn setNoteheadFill(
        &mut self,
        py: Python<'_>,
        nh: &Bound<'_, PyAny>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_noteheadFill(nh)
    }

    fn getStemDirection(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<&'static str> {
        let note = self.note_object(py, p)?;
        let direction = note.borrow(py).inner.stem_direction();
        Ok(direction.as_str())
    }

    #[pyo3(signature = (stemDirection, pitchTarget = None))]
    fn setStemDirection(
        &mut self,
        py: Python<'_>,
        stemDirection: Option<&str>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_stemDirection(stemDirection)
    }

    fn getTie(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<Option<Tie>> {
        match self.note_object(py, p) {
            Ok(note) => {
                let tie = note.borrow(py).inner.tie().cloned().map(Tie::wrap);
                Ok(tie)
            }
            Err(_) => Ok(None),
        }
    }

    #[pyo3(signature = (tieObjOrStr, pitchTarget = None))]
    fn setTie(
        &mut self,
        py: Python<'_>,
        tieObjOrStr: &Bound<'_, PyAny>,
        pitchTarget: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let target = self.first_or_named(py, pitchTarget)?;
        target.borrow_mut(py).set_tie(Some(tieObjOrStr))
    }

    #[getter]
    fn get_tie(&self, py: Python<'_>) -> Option<Tie> {
        self.notes
            .iter()
            .find_map(|note| note.borrow(py).inner.tie().cloned())
            .map(Tie::wrap)
    }

    #[setter]
    fn set_tie(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let tie = match value.filter(|value| !value.is_none()) {
            Some(value) => Some(tie_from_any(value)?),
            None => None,
        };
        for note in &self.notes {
            note.borrow_mut(py).inner.set_tie(tie.clone());
        }
        Ok(())
    }

    fn getVolume(&self, py: Python<'_>, p: &Bound<'_, PyAny>) -> PyResult<Volume> {
        let note = self.note_object(py, p)?;
        let volume = note.borrow(py).inner.volume();
        Ok(Volume::wrap(volume))
    }

    #[pyo3(signature = (vol, target = None))]
    fn setVolume(
        &mut self,
        py: Python<'_>,
        vol: &Bound<'_, PyAny>,
        target: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let note = self.first_or_named(py, target)?;
        let parsed = volume_from_any(vol)?;
        note.borrow_mut(py).inner.set_volume(Some(parsed));
        Ok(())
    }

    fn hasVolumeInformation(&self, py: Python<'_>) -> bool {
        self.volume.is_some() || self.hasComponentVolumes(py)
    }

    /// music21's `simplifyEnharmonics`: respells the chord so its pitches
    /// read as simply as they can.
    #[pyo3(signature = (*, inPlace = false, keyContext = None))]
    fn simplifyEnharmonics(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
        keyContext: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<Chord>> {
        let key_context = match keyContext.filter(|value| !value.is_none()) {
            Some(value) => {
                let sharps: i32 = value.getattr("sharps")?.extract()?;
                Some(music21_rs::KeySignature::new(sharps))
            }
            None => None,
        };
        let simplified = self
            .inner
            .simplify_enharmonics(key_context)
            .map_err(chord_error)?;
        if inPlace {
            self.replace_inner(py, simplified)?;
            Ok(None)
        } else {
            Ok(Some(Self::from_inner(py, simplified)?))
        }
    }

    fn hasComponentVolumes(&self, py: Python<'_>) -> bool {
        self.notes
            .iter()
            .any(|note| note.borrow(py).inner.has_volume_information())
    }

    fn setVolumes(&mut self, py: Python<'_>, volumes: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut parsed: Vec<RsVolume> = Vec::new();
        for item in volumes.try_iter()? {
            parsed.push(volume_from_any(&item?)?);
        }
        if parsed.is_empty() {
            return Err(ChordException::new_err(
                "setVolumes needs at least one volume",
            ));
        }
        self.volume = None;
        for (index, note) in self.notes.iter().enumerate() {
            note.borrow_mut(py)
                .inner
                .set_volume(Some(parsed[index % parsed.len()].clone()));
        }
        Ok(())
    }

    #[getter]
    fn get_volume(&mut self, py: Python<'_>) -> PyResult<Py<Volume>> {
        if let Some(volume) = &self.volume {
            return Ok(volume.clone_ref(py));
        }
        let velocities: Vec<i32> = self
            .notes
            .iter()
            .filter_map(|note| note.borrow(py).inner.volume().velocity())
            .collect();
        let inner = if velocities.is_empty() {
            RsVolume::new()
        } else {
            let total: i32 = velocities.iter().sum();
            let mean = f64::from(total) / velocities.len() as f64;
            RsVolume::from_velocity(mean.round_ties_even() as i32)
        };
        let created = Py::new(py, Volume::wrap(inner))?;
        self.volume = Some(created.clone_ref(py));
        Ok(created)
    }

    #[setter]
    fn set_volume(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        for note in &self.notes {
            note.borrow_mut(py).inner.set_volume(None);
        }
        self.volume = match value.extract::<Py<Volume>>() {
            Ok(object) => Some(object),
            Err(_) => Some(Py::new(py, Volume::wrap(volume_from_any(value)?))?),
        };
        Ok(())
    }

    #[getter]
    fn lyrics(&self, py: Python<'_>) -> Vec<Lyric> {
        self.notes
            .first()
            .map(|note| note.borrow(py).lyrics())
            .unwrap_or_default()
    }

    #[getter]
    fn get_lyric(&self, py: Python<'_>) -> Option<String> {
        self.notes
            .first()
            .and_then(|note| note.borrow(py).inner.lyric())
    }

    #[setter]
    fn set_lyric(&mut self, py: Python<'_>, value: Option<&str>) -> PyResult<()> {
        match self.notes.first() {
            Some(note) => note.borrow_mut(py).set_lyric(value),
            None => Ok(()),
        }
    }

    #[pyo3(signature = (text, lyricNumber = None, *, applyRaw = false, lyricIdentifier = None))]
    fn addLyric(
        &mut self,
        py: Python<'_>,
        text: &Bound<'_, PyAny>,
        lyricNumber: Option<i32>,
        applyRaw: bool,
        lyricIdentifier: Option<String>,
    ) -> PyResult<()> {
        match self.notes.first() {
            Some(note) => {
                note.borrow_mut(py)
                    .addLyric(text, lyricNumber, applyRaw, lyricIdentifier)
            }
            None => Err(ChordException::new_err(
                "an empty chord has nothing to sing",
            )),
        }
    }

    /// music21's `annotateIntervals`: the interval from the lowest pitch up
    /// to each of the others, written on as lyrics.
    #[pyo3(signature = (*, inPlace = false, stripSpecifiers = true, sortPitches = true, returnList = false))]
    fn annotateIntervals(
        &mut self,
        py: Python<'_>,
        inPlace: bool,
        stripSpecifiers: bool,
        sortPitches: bool,
        returnList: bool,
    ) -> PyResult<Option<Py<PyAny>>> {
        let names = self
            .inner
            .annotate_intervals(stripSpecifiers, sortPitches)
            .map_err(chord_error)?;
        if returnList {
            return Ok(Some(PyList::new(py, names)?.into_any().unbind()));
        }
        if inPlace {
            for name in names {
                let text = name.into_pyobject(py)?;
                self.addLyric(py, text.as_any(), None, false, None)?;
            }
            return Ok(None);
        }
        let mut annotated = Self::from_inner(py, self.inner.clone())?;
        for name in names {
            let text = name.into_pyobject(py)?;
            annotated.addLyric(py, text.as_any(), None, false, None)?;
        }
        Ok(Some(Py::new(py, annotated)?.into_any()))
    }

    /// music21's `Pitch.getStringHarmonic`, which reads the notehead off the
    /// chord: a chord whose second note is a diamond sounds the harmonic its
    /// two pitches pick out.
    pub(crate) fn getStringHarmonic(&self, py: Python<'_>) -> PyResult<Option<Chord>> {
        let sounded = self
            .with_note_notation(py)?
            .string_harmonic()
            .map_err(chord_error)?;
        match sounded {
            Some(chord) => Ok(Some(Self::from_inner(py, chord)?)),
            None => Ok(None),
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        let py = other.py();
        let Ok(other) = other.extract::<PyRef<Chord>>() else {
            return false;
        };
        if self.quarter_length(py) != other.quarter_length(py) {
            return false;
        }
        let mut ours: Vec<String> = self
            .inner
            .pitches()
            .iter()
            .map(RsPitch::name_with_octave)
            .collect();
        let mut theirs: Vec<String> = other
            .inner
            .pitches()
            .iter()
            .map(RsPitch::name_with_octave)
            .collect();
        ours.sort();
        theirs.sort();
        ours == theirs
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let borrowed = slf.borrow();
        let names: Vec<String> = borrowed
            .inner
            .pitches()
            .iter()
            .map(RsPitch::name_with_octave)
            .collect();
        Ok(format!(
            "<music21.chord.{} {}>",
            slf.get_type().qualname()?,
            names.join(" ")
        ))
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__(py)
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Self> {
        let mut copied = Self::from_inner(py, self.inner.clone())?;
        for (target, source) in copied.notes.iter().zip(&self.notes) {
            target.borrow_mut(py).inner = source.borrow(py).inner.clone();
        }
        if let Some(duration) = &self.duration {
            copied.duration = Some(Py::new(py, duration.borrow(py).clone())?);
        }
        if let Some(volume) = &self.volume {
            copied.volume = Some(Py::new(py, volume.borrow(py).clone())?);
        }
        Ok(copied)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Chord>()?;
    let exception = py.get_type::<ChordException>();
    exception.setattr("__module__", "music21.chord")?;
    m.add("ChordException", exception)?;
    Ok(())
}
