//! music21's `noteworthy.translate.NoteworthyTranslator`, line by line.
//!
//! music21's translator reads a `.nwctxt` file one line at a time into
//! streams it keeps on itself (`currentMeasure`, `currentPart`, `score`),
//! and its methods are called on their own with streams and notes a
//! caller made. So the stateful half is here, over those Python objects;
//! what each line means -- lengths, clefs, keys, meters, barline styles,
//! flow marks, staff positions, verses -- is the crate's
//! `music21::noteworthy`, which its own reader uses too.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::PathBuf;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21::notation::TieType;
use music21::noteworthy::placement::{OpenSlur, Placement, Placing, TiedStep};
use music21::noteworthy::{
    BarStyle, clef_named, duration_of, first_number, flow_kind, key_sharps, lyrics, meter_ratio,
    step_and_octave,
};

use crate::Walkable;

/// The module music21 keeps the translator in.
pub(crate) const MODULE: &str = "music21.noteworthy.translate";

/// The clef music21's translator starts in.
const TREBLE: &str = "TREBLE";

/// The clef under which notes are unpitched strokes.
const PERCUSSION: &str = "PERCUSSION";

/// What a `Dur` value says to slur a note to the next.
const SLUR: &str = "Slur";

pyo3::create_exception!(
    music21_rs_facade,
    NoteworthyTranslateException,
    crate::Music21Exception
);

/// music21's class `name` in `module`: the provided or installed one where
/// music21 can be imported, the wheel's own where not.
fn class<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(found) = py.import(module).and_then(|module| module.getattr(name)) {
        return Ok(found);
    }
    py.import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?
        .getattr(name)
}

fn translate_error(error: music21::Error) -> PyErr {
    NoteworthyTranslateException::new_err(crate::pitch::message(&error))
}

/// An attribute of a line, as music21 reads `attributes['Name']`.
fn required<'py>(attributes: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    attributes.get_item(name)
}

/// An attribute of a line where it was written.
fn optional<'py>(
    attributes: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let found = attributes.call_method1("get", (name,))?;
    Ok((!found.is_none()).then_some(found))
}

/// music21's `NoteworthyTranslator`: reads `.nwctxt` lines into a score,
/// keeping the measure, part and score it builds on itself.
#[pyclass(
    name = "NoteworthyTranslator",
    module = "music21.noteworthy.translate",
    subclass,
    dict
)]
pub struct NoteworthyTranslator {
    /// Where the next object goes, and the slurs and ties left open: the
    /// crate's placing state, over the Python measures and notes.
    placement: Placement<Py<PyAny>, Py<PyAny>>,
}

#[pymethods]
impl NoteworthyTranslator {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(_arguments: &Bound<'_, PyTuple>, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self {
            placement: Placement::default(),
        }
    }

    /// The measures and notes the placing state holds, shown to Python's
    /// collector.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.placement.position_measure)?;
        for open in &self.placement.open_slurs {
            visit.call(&open.start)?;
            visit.call(&open.measure)?;
        }
        for tied in &self.placement.tied_steps {
            visit.call(&tied.measure)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.placement = Placement::default();
    }

    /// music21's `positionMeasure`: the measure placed in last.
    #[getter]
    fn get_positionMeasure(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.placement
            .position_measure
            .as_ref()
            .map(|measure| measure.clone_ref(py))
    }

    #[setter]
    fn set_positionMeasure(&mut self, value: Option<Py<PyAny>>) {
        self.placement.position_measure = value;
    }

    /// music21's `measurePosition`: where the next object goes in it.
    #[getter]
    fn get_measurePosition<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::duration::op_frac(py, self.placement.measure_position)
    }

    #[setter]
    fn set_measurePosition(&mut self, value: f64) {
        self.placement.measure_position = value;
    }

    /// music21's `openSlurs`: each slur begun and not yet joined, as the
    /// note it starts at, its measure, where that note ends and its place
    /// in its group.
    #[getter]
    fn get_openSlurs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let made = PyList::empty(py);
        for open in &self.placement.open_slurs {
            made.append((
                open.start.clone_ref(py),
                open.measure.clone_ref(py),
                crate::duration::op_frac(py, open.end)?,
                open.group,
            ))?;
        }
        Ok(made)
    }

    /// music21's `tiedSteps`: each tie begun, by its staff step, with its
    /// measure (none where carried over a barline) and where it ends.
    #[getter]
    fn get_tiedSteps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let made = PyDict::new(py);
        for tied in &self.placement.tied_steps {
            let measure = tied.measure.as_ref().map(|measure| measure.clone_ref(py));
            made.set_item(
                (tied.step.0.to_string(), tied.step.1),
                (measure, crate::duration::op_frac(py, tied.end)?),
            )?;
        }
        Ok(made)
    }

    /// Open slurs as music21 lists them: `(start, measure, end, group)`.
    #[setter]
    fn set_openSlurs(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut open = Vec::new();
        for slur in value.walk()? {
            let (start, measure, end, group): (Py<PyAny>, Py<PyAny>, f64, usize) =
                slur?.extract()?;
            open.push(OpenSlur {
                start,
                measure,
                end,
                group,
            });
        }
        self.placement.open_slurs = open;
        Ok(())
    }

    /// Ties begun as music21 keeps them: `{(step, octave): (measure or
    /// None, end)}`.
    #[setter]
    fn set_tiedSteps(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut tied = Vec::new();
        for pair in value.call_method0("items")?.walk()? {
            let pair = pair?;
            let (step, octave): (String, i32) = pair.get_item(0)?.extract()?;
            let (measure, end): (Option<Py<PyAny>>, f64) = pair.get_item(1)?.extract()?;
            let step = step.chars().next().ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err("a tied step needs a step name")
            })?;
            tied.push(TiedStep {
                step: (step, octave),
                measure,
                end,
            });
        }
        self.placement.tied_steps = tied;
        Ok(())
    }

    /// music21's starting state: no part yet, an empty measure and score,
    /// the treble clef and no key.
    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let streams = |name: &str| class(py, "music21.stream", name)?.call0();
        slf.setattr("currentPart", py.None())?;
        slf.setattr("currentMeasure", streams("Measure")?)?;
        slf.setattr("measureNumber", 0)?;
        slf.setattr("currentEnding", 1)?;
        slf.setattr("repeatedMeasures", PyList::empty(py))?;
        slf.setattr("score", streams("Score")?)?;
        slf.setattr("currentClef", TREBLE)?;
        slf.setattr("currentKey", key_signature(py, 0)?)?;
        slf.setattr("lyricPosition", 0)?;
        slf.setattr("lyrics", PyList::empty(py))?;
        slf.setattr("activeAccidentals", PyDict::new(py))?;
        Ok(())
    }

    /// music21's `parseFile`: the score a `.nwctxt` file holds.
    fn parseFile<'py>(slf: &Bound<'py, Self>, filePath: PathBuf) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let shown = filePath.display().to_string();
        let text = crate::plumbing::filetools::readFileEncodingSafe(py, filePath, "utf-8")
            .map_err(|_| NoteworthyTranslateException::new_err(format!("cannot open {shown}: ")))?;
        let lines: Vec<&str> = text.split('\n').collect();
        Self::parseList(slf, &PyList::new(py, lines)?.into_any())
    }

    /// music21's `parseString`: the score `.nwctxt` text holds.
    fn parseString<'py>(slf: &Bound<'py, Self>, data: &str) -> PyResult<Bound<'py, PyAny>> {
        let lines: Vec<&str> = data.lines().collect();
        Self::parseList(slf, &PyList::new(slf.py(), lines)?.into_any())
    }

    /// music21's `parseList`: each line's command and attributes, read in
    /// turn into the score, which is handed back.
    fn parseList<'py>(
        slf: &Bound<'py, Self>,
        dataList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        for line in dataList.walk()? {
            let line: String = line?.extract()?;
            let line = line.trim_end();
            if !line.starts_with('|') {
                continue;
            }
            let sections: Vec<&str> = line.split('|').collect();
            let command = sections.get(1).copied().unwrap_or("");
            let attributes = line_attributes(py, command, &sections, line)?;
            if let Err(cause) = Self::command(slf, command, &attributes) {
                let raised = NoteworthyTranslateException::new_err(format!(
                    "Cannot create object from \"{}\"",
                    attributes.str()?
                ));
                raised.set_cause(py, Some(cause));
                return Err(raised);
            }
        }

        // What is left of the last measure, and the last part.
        let measure = slf.getattr("currentMeasure")?;
        let part = slf.getattr("currentPart")?;
        if measure.is_truthy()? {
            part.call_method1("append", (&measure,))?;
        }
        let score = slf.getattr("score")?;
        score.call_method1("insert", (0, part))?;
        Ok(score)
    }

    /// music21's `setDurationForObject`: the length of a `Dur` value, with
    /// its dots, grace and triplet, given to a note, chord or rest.
    #[staticmethod]
    fn setDurationForObject(generalNote: &Bound<'_, PyAny>, durationInfo: &str) -> PyResult<()> {
        let py = generalNote.py();
        let duration = duration_of(durationInfo).map_err(translate_error)?;
        generalNote.setattr("duration", crate::duration::Duration::object(py, duration)?)
    }

    /// music21's `setTieFromPitchInfo`: the ties of a note or chord about
    /// to go at the current position. A position ending `^` is tied to the
    /// note on the same staff step, under whatever clef, that starts where
    /// the tied one ends.
    fn setTieFromPitchInfo(
        slf: &Bound<'_, Self>,
        noteOrChord: &Bound<'_, PyAny>,
        pitchInfo: &str,
    ) -> PyResult<()> {
        let clef: String = slf.getattr("currentClef")?.extract()?;
        let mut steps = Vec::new();
        for position in pitchInfo.split(',') {
            let place = first_number(position).ok_or_else(|| {
                pyo3::exceptions::PyAttributeError::new_err(
                    "'NoneType' object has no attribute 'group'",
                )
            })?;
            steps.push((step_and_octave(&clef, place), position.ends_with('^')));
        }
        let measure = slf.getattr("currentMeasure")?.unbind();
        let mut streams = Streams::new(slf.py())?;
        let mut element = noteOrChord.clone();
        slf.borrow_mut()
            .placement
            .tie(&mut streams, &measure, &mut element, &steps)
    }

    /// music21's `getPitchFromPositionInfo`: a `Pos:` value's pitch, its
    /// tie and notehead marks dropped.
    fn getPitchFromPositionInfo<'py>(
        slf: &Bound<'py, Self>,
        posInfo: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::getOnePitchFromPosition(slf, position_only(posInfo))
    }

    /// music21's `getMultiplePitchesFromPositionInfo`: a chord's `Pos:`
    /// value's pitches.
    fn getMultiplePitchesFromPositionInfo<'py>(
        slf: &Bound<'py, Self>,
        posInfo: &str,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        posInfo
            .split(',')
            .map(|one| Self::getPitchFromPositionInfo(slf, one))
            .collect()
    }

    /// music21's `getOnePitchFromPosition`: the pitch at a position, with
    /// the accidental written before it, else one written earlier in the
    /// measure on that pitch, else the key's.
    fn getOnePitchFromPosition<'py>(
        slf: &Bound<'py, Self>,
        pos: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (written, position) = written_accidental(pos);
        let position: i32 = position.parse().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "invalid literal for int() with base 10: '{position}'"
            ))
        })?;
        let step_octave = Self::getStepAndOctaveFromPosition(slf, position)?;
        let (step, octave): (String, i32) = step_octave.extract()?;

        let pitch = class(py, "music21.pitch", "Pitch")?.call1((format!("{step}{octave}"),))?;
        let name: String = pitch.getattr("nameWithOctave")?.extract()?;
        let active = slf.getattr("activeAccidentals")?;
        let accidental = class(py, "music21.pitch", "Accidental")?;
        if let Some(written) = written {
            pitch.setattr("accidental", accidental.call1((written,))?)?;
            active.set_item(&name, written)?;
            return Ok(pitch);
        }
        if active.contains(&name)? {
            pitch.setattr("accidental", accidental.call1((active.get_item(&name)?,))?)?;
            return Ok(pitch);
        }
        let from_key = slf
            .getattr("currentKey")?
            .call_method1("accidentalByStep", (step,))?;
        if !from_key.is_none() {
            pitch.setattr("accidental", from_key)?;
        }
        Ok(pitch)
    }

    /// music21's `getStepAndOctaveFromPosition`: the step and octave of a
    /// staff position under `currentClef`.
    fn getStepAndOctaveFromPosition<'py>(
        slf: &Bound<'py, Self>,
        positionNote: i32,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let clef: String = slf.getattr("currentClef")?.extract()?;
        let (step, octave) = step_and_octave(&clef, positionNote);
        PyTuple::new(
            slf.py(),
            [
                PyString::new(slf.py(), &step.to_string()).into_any(),
                octave.into_pyobject(slf.py())?.into_any(),
            ],
        )
    }

    /// music21's `translateNote`: a note, or an unpitched stroke under the
    /// percussion clef, at the current position.
    fn translateNote(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let duration_info: String = required(attributes, "Dur")?.extract()?;
        let pitch_info: String = required(attributes, "Pos")?.extract()?;
        let pitch = Self::getPitchFromPositionInfo(slf, &pitch_info)?;
        let clef: String = slf.getattr("currentClef")?.extract()?;
        let made = if clef == PERCUSSION {
            let stroke = class(py, "music21.note", "Unpitched")?.call0()?;
            stroke.setattr("displayStep", pitch.getattr("step")?)?;
            stroke.setattr("displayOctave", pitch.getattr("octave")?)?;
            stroke
        } else {
            class(py, "music21.note", "Note")?.call1((pitch,))?
        };
        Self::setDurationForObject(&made, &duration_info)?;
        Self::setTieFromPitchInfo(slf, &made, &pitch_info)?;
        Self::addCurrentLyric(slf, &made)?;
        let group = PyList::new(py, [(made, duration_info)])?;
        Self::appendSimultaneous(slf, &group.into_any())
    }

    /// music21's `translateChord`: a chord for each `Dur` and the `Pos`
    /// beside it, and one for `Dur2` and `Pos2`, starting together.
    fn translateChord(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let durations: Vec<String> = required(attributes, "Dur")?.extract()?;
        let positions: Vec<String> = required(attributes, "Pos")?.extract()?;
        let mut groups: Vec<(String, String)> = durations.into_iter().zip(positions).collect();
        if let Some(second) = optional(attributes, "Dur2")? {
            groups.push((second.extract()?, required(attributes, "Pos2")?.extract()?));
        }
        let chords = PyList::empty(py);
        for (duration_info, pitch_info) in &groups {
            let chord = Self::createChord(slf, duration_info, pitch_info)?;
            chords.append((chord, duration_info))?;
        }
        if let Ok(first) = chords.get_item(0) {
            Self::addCurrentLyric(slf, &first.get_item(0)?)?;
        }
        Self::appendSimultaneous(slf, &chords.into_any())
    }

    /// music21's `translateRestChord`: a rest and the chord sounding with
    /// it.
    fn translateRestChord(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let chord_duration: String = required(attributes, "Dur2")?.extract()?;
        let chord_pitches: String = required(attributes, "Pos2")?.extract()?;
        let rest_duration: String = required(attributes, "Dur")?.extract()?;
        let chord = Self::createChord(slf, &chord_duration, &chord_pitches)?;
        let rest = class(py, "music21.note", "Rest")?.call0()?;
        Self::setDurationForObject(&rest, &rest_duration)?;
        Self::addCurrentLyric(slf, &chord)?;
        let group = PyList::empty(py);
        group.append((chord, chord_duration))?;
        group.append((rest, rest_duration))?;
        Self::appendSimultaneous(slf, &group.into_any())
    }

    /// music21's `createChord`: a chord from one group's `Dur` and `Pos`.
    fn createChord<'py>(
        slf: &Bound<'py, Self>,
        durationInfo: &str,
        pitchInfo: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let chord = class(slf.py(), "music21.chord", "Chord")?.call0()?;
        Self::setDurationForObject(&chord, durationInfo)?;
        chord.setattr(
            "pitches",
            Self::getMultiplePitchesFromPositionInfo(slf, pitchInfo)?,
        )?;
        Self::setTieFromPitchInfo(slf, &chord, pitchInfo)?;
        Ok(chord)
    }

    /// music21's `addCurrentLyric`: the syllable sung now, if any.
    fn addCurrentLyric(slf: &Bound<'_, Self>, generalNote: &Bound<'_, PyAny>) -> PyResult<()> {
        let syllables = slf.getattr("lyrics")?;
        let at: usize = slf.getattr("lyricPosition")?.extract()?;
        if syllables.is_truthy()? && at < syllables.len()? {
            generalNote.call_method1("addLyric", (syllables.get_item(at)?,))?;
        }
        Ok(())
    }

    /// music21's `appendSimultaneous`: notes, chords or rests, each with its
    /// `Dur` value, at the current position, which moves on by the shortest
    /// of them. Where several start together, or a voice still sounds, each
    /// goes in a voice free there, the longest first, or a new one.
    fn appendSimultaneous(slf: &Bound<'_, Self>, generalNotes: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut elements = Vec::new();
        for pair in generalNotes.walk()? {
            let (note, duration_info): (Bound<'_, PyAny>, String) = pair?.extract()?;
            elements.push((note, duration_info.split(',').any(|part| part == SLUR)));
        }
        let measure = slf.getattr("currentMeasure")?.unbind();
        let mut streams = Streams::new(slf.py())?;
        slf.borrow_mut()
            .placement
            .place(&mut streams, &measure, elements)
    }

    /// music21's `setSlurs`: slurs begun, carried on or ended at notes just
    /// placed together, each with whether it is slurred.
    fn setSlurs(slf: &Bound<'_, Self>, generalNotes: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut group = Vec::new();
        for pair in generalNotes.walk()? {
            let (note, slurred): (Bound<'_, PyAny>, bool) = pair?.extract()?;
            let length: f64 = note.getattr("quarterLength")?.extract()?;
            group.push((note.unbind(), slurred, length));
        }
        let measure = slf.getattr("currentMeasure")?.unbind();
        let mut streams = Streams::new(slf.py())?;
        slf.borrow_mut()
            .placement
            .slurs(&mut streams, &measure, &group)
    }

    /// music21's `currentPosition`: the end of the measure's contents when
    /// first asked in a measure, then where notes placed moved it on to.
    fn currentPosition<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let measure = slf.getattr("currentMeasure")?.unbind();
        let streams = Streams::new(py)?;
        let offset = slf.borrow_mut().placement.position(&streams, &measure)?;
        crate::duration::op_frac(py, offset)
    }

    /// music21's `insertAtPosition`: a clef, key, dynamic or other mark at
    /// the current position.
    fn insertAtPosition(slf: &Bound<'_, Self>, element: &Bound<'_, PyAny>) -> PyResult<()> {
        let measure = slf.getattr("currentMeasure")?.unbind();
        let mut streams = Streams::new(slf.py())?;
        slf.borrow_mut()
            .placement
            .mark(&mut streams, &measure, element.clone())
    }

    /// music21's `translateRest`: a rest at the current position.
    fn translateRest(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let duration_info: String = required(attributes, "Dur")?.extract()?;
        let rest = class(py, "music21.note", "Rest")?.call0()?;
        Self::setDurationForObject(&rest, &duration_info)?;
        let group = PyList::new(py, [(rest, duration_info)])?;
        Self::appendSimultaneous(slf, &group.into_any())
    }

    /// music21's `createClef`: the clef `Type` and `OctaveShift` name, at
    /// the current position, and `currentClef` with it.
    fn createClef(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let kind: String = required(attributes, "Type")?.extract()?;
        let shift: Option<String> = optional(attributes, "OctaveShift")?
            .map(|shift| shift.extract())
            .transpose()?;
        let (clef, named) = clef_named(&kind, shift.as_deref()).map_err(translate_error)?;
        let made = crate::clef::Clef::object(py, clef)?;
        Self::insertAtPosition(slf, made.bind(py))?;
        slf.setattr("currentClef", named)
    }

    /// music21's `createKey`: a key signature of the sharps less the flats
    /// written, at the current position, and `currentKey` with it.
    fn createKey(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let signature: String = required(attributes, "Signature")?.extract()?;
        let key = key_signature(slf.py(), key_sharps(&signature))?;
        Self::insertAtPosition(slf, &key)?;
        slf.setattr("currentKey", key)
    }

    /// music21's `createTimeSignature`: a meter at the current position.
    fn createTimeSignature(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let signature: String = required(attributes, "Signature")?.extract()?;
        let meter =
            class(slf.py(), "music21.meter", "TimeSignature")?.call1((meter_ratio(&signature),))?;
        Self::insertAtPosition(slf, &meter)
    }

    /// music21's `createPart`: the part read so far closed with its last
    /// measure and put in the score, and a new part begun.
    fn createPart(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let part = slf.getattr("currentPart")?;
        if !part.is_none() {
            part.call_method1("append", (slf.getattr("currentMeasure")?,))?;
            slf.getattr("score")?.call_method1("insert", (0, &part))?;
        }
        slf.setattr("currentPart", class(py, "music21.stream", "Part")?.call0()?)?;
        slf.setattr(
            "currentMeasure",
            class(py, "music21.stream", "Measure")?.call0()?,
        )?;
        slf.setattr("measureNumber", 0)
    }

    /// music21's `createBarlines`: the measure closed, with the barline a
    /// `Style` names, and the next begun. A tie running to the barline goes
    /// on into the next measure; any other open tie is dropped.
    fn createBarlines(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("activeAccidentals", PyDict::new(py))?;
        let measure = slf.getattr("currentMeasure")?;
        let streams = Streams::new(py)?;
        let held = measure.clone().unbind();
        slf.borrow_mut().placement.barline(&streams, &held)?;

        let measure_class = class(py, "music21.stream", "Measure")?;
        let part = slf.getattr("currentPart")?;
        let number = slf.getattr("measureNumber")?;
        let Some(style) = optional(attributes, "Style")? else {
            // A plain barline.
            part.call_method1("append", (&measure,))?;
            let keywords = PyDict::new(py);
            keywords.set_item("number", &number)?;
            let next = measure_class.call((), Some(&keywords))?;
            let repeated = slf.getattr("repeatedMeasures")?;
            if repeated.len()? > 0 {
                repeated.call_method1("append", (&next,))?;
            }
            slf.setattr("currentMeasure", next)?;
            return slf.setattr("measureNumber", number.add(1)?);
        };

        let repeated = slf.getattr("repeatedMeasures")?;
        if repeated.len()? > 0 {
            let keywords = PyDict::new(py);
            keywords.set_item("number", slf.getattr("currentEnding")?)?;
            let bracket = class(py, "music21.spanner", "RepeatBracket")?
                .call((&repeated,), Some(&keywords))?;
            part.call_method1("append", (bracket,))?;
            slf.setattr("repeatedMeasures", PyList::empty(py))?;
        }

        let style: String = style.extract()?;
        let kind = music21::noteworthy::bar_style(&style).map_err(translate_error)?;
        let repeat = class(py, "music21.bar", "Repeat")?;
        let direction = |way: &str| -> PyResult<Bound<'_, PyAny>> {
            let keywords = PyDict::new(py);
            keywords.set_item("direction", way)?;
            repeat.call((), Some(&keywords))
        };
        let next = measure_class.call0()?;
        match kind {
            BarStyle::RepeatOpen => {
                part.call_method1("append", (&measure,))?;
                next.setattr("leftBarline", direction("start")?)?;
            }
            BarStyle::RepeatClose => {
                measure.setattr("rightBarline", direction("end")?)?;
                part.call_method1("append", (&measure,))?;
            }
            BarStyle::Closing(barline) => {
                let made = class(py, "music21.bar", "Barline")?.call1((barline.as_str(),))?;
                measure.setattr("rightBarline", made)?;
                part.call_method1("append", (&measure,))?;
            }
        }
        next.setattr("number", &number)?;
        slf.setattr("currentMeasure", next)?;
        slf.setattr("measureNumber", number.add(1)?)
    }

    /// music21's `createOtherRepetitions`: a *Coda*, *Segno*, *Fine* or
    /// jump at the current position.
    fn createOtherRepetitions(
        slf: &Bound<'_, Self>,
        attributes: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let style: String = required(attributes, "Style")?.extract()?;
        let kind = flow_kind(&style).map_err(|_| {
            NoteworthyTranslateException::new_err(format!(
                "Cannot get style from {}",
                attributes
                    .str()
                    .map(|said| said.to_string())
                    .unwrap_or_default()
            ))
        })?;
        let made = class(slf.py(), "music21.repeat", kind.class_name())?.call0()?;
        Self::insertAtPosition(slf, &made)
    }

    /// music21's `createDynamicVariance`: a crescendo or diminuendo at the
    /// current position; any other style is passed over.
    fn createDynamicVariance(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let style: String = required(attributes, "Style")?.extract()?;
        let name = match style.as_str() {
            "Crescendo" => "Crescendo",
            "Decrescendo" => "Diminuendo",
            _ => return Ok(()),
        };
        let made = class(slf.py(), "music21.dynamics", name)?.call0()?;
        Self::insertAtPosition(slf, &made)
    }

    /// music21's `createDynamics`: a dynamic at the current position.
    fn createDynamics(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(style) = optional(attributes, "Style")? else {
            return Ok(());
        };
        let made = class(slf.py(), "music21.dynamics", "Dynamic")?.call1((style,))?;
        Self::insertAtPosition(slf, &made)
    }

    /// music21's `createLyrics`: a verse's syllables, one per note.
    #[staticmethod]
    fn createLyrics(attributes: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
        let text: String = required(attributes, "Text")?.extract()?;
        Ok(lyrics(&text))
    }

    /// music21's `createStaff`: a new part, named, with nothing left open
    /// from the one before.
    fn createStaff(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        Self::createPart(slf)?;
        slf.borrow_mut().placement.new_staff();
        slf.setattr("currentKey", key_signature(py, 0)?)?;
        slf.setattr("activeAccidentals", PyDict::new(py))?;
        slf.setattr("lyrics", PyList::empty(py))?;
        slf.setattr("lyricPosition", 0)?;
        let name = attributes.call_method1("get", ("Name",))?;
        let part = slf.getattr("currentPart")?;
        part.setattr("partName", &name)?;
        part.setattr("partAbbreviation", &name)
    }

    /// music21's `createStaffInstrument`: the instrument of a MIDI program,
    /// transposed as the staff says.
    fn createStaffInstrument(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let number = |name: &str| -> PyResult<i64> {
            match optional(attributes, name)? {
                Some(value) => py
                    .import("builtins")?
                    .getattr("int")?
                    .call1((value,))?
                    .extract(),
                None => Ok(0),
            }
        };
        let made = class(py, "music21.instrument", "instrumentFromMidiProgram")?
            .call1((number("Patch")?,))?;
        let transposition =
            class(py, "music21.interval", "Interval")?.call1((number("Trans")?,))?;
        made.setattr("transposition", transposition)?;
        slf.getattr("currentPart")?
            .call_method1("append", (made,))?;
        Ok(())
    }

    /// music21's `createText`: words at the current position, above the
    /// staff where written below it and below where written above.
    fn createText(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let words = class(py, "music21.expressions", "TextExpression")?
            .call1((required(attributes, "Text")?,))?;
        let position = attributes.call_method1("get", ("Pos",))?;
        let place: i64 = py
            .import("builtins")?
            .getattr("int")?
            .call1((&position,))?
            .extract()?;
        words.setattr("placement", if place < 0 { "above" } else { "below" })?;
        words.getattr("style")?.setattr("relativeY", &position)?;
        Self::insertAtPosition(slf, &words)
    }

    /// music21's `createSongInfo`: the title and author, as metadata on
    /// the score.
    fn createSongInfo(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let made = class(slf.py(), "music21.metadata", "Metadata")?.call0()?;
        made.setattr("title", attributes.call_method1("get", ("Title",))?)?;
        made.setattr("author", attributes.call_method1("get", ("Author",))?)?;
        slf.getattr("score")?.call_method1("append", (made,))?;
        Ok(())
    }

    /// music21's `createEnding`: the measures from here are an ending of
    /// the number written first.
    fn createEnding(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let endings: String = required(attributes, "Endings")?.extract()?;
        let measure = slf.getattr("currentMeasure")?;
        if measure.getattr("leftBarline")?.is_none() {
            measure.setattr("leftBarline", class(py, "music21.bar", "Barline")?.call0()?)?;
        }
        slf.setattr("repeatedMeasures", PyList::new(py, [&measure])?)?;
        let first = endings.split(',').next().unwrap_or("");
        let number: i64 = py
            .import("builtins")?
            .getattr("int")?
            .call1((first,))?
            .extract()?;
        slf.setattr("currentEnding", number)
    }

    /// music21's `createTempo`: a metronome mark at the start of the
    /// measure.
    fn createTempo(slf: &Bound<'_, Self>, attributes: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let tempo: i64 = py
            .import("builtins")?
            .getattr("int")?
            .call1((required(attributes, "Tempo")?,))?
            .extract()?;
        let keywords = PyDict::new(py);
        keywords.set_item("number", tempo)?;
        let mark = class(py, "music21.tempo", "MetronomeMark")?.call((), Some(&keywords))?;
        slf.getattr("currentMeasure")?
            .call_method1("insert", (0, mark))?;
        Ok(())
    }
}

impl NoteworthyTranslator {
    /// One line's command, as music21's `parseList` dispatches it.
    fn command(
        slf: &Bound<'_, Self>,
        command: &str,
        attributes: &Bound<'_, PyDict>,
    ) -> PyResult<()> {
        let attributes = attributes.as_any();
        let lyric_moves = |slf: &Bound<'_, Self>| -> PyResult<()> {
            let at = slf.getattr("lyricPosition")?;
            slf.setattr("lyricPosition", at.add(1)?)
        };
        match command {
            "AddStaff" => Self::createStaff(slf, attributes),
            "Bar" => Self::createBarlines(slf, attributes),
            "Chord" => {
                Self::translateChord(slf, attributes)?;
                lyric_moves(slf)
            }
            "Clef" => Self::createClef(slf, attributes),
            "Dynamic" => Self::createDynamics(slf, attributes),
            "DynamicVariance" => Self::createDynamicVariance(slf, attributes),
            "Ending" => Self::createEnding(slf, attributes),
            "Flow" => Self::createOtherRepetitions(slf, attributes),
            "Key" => Self::createKey(slf, attributes),
            "Lyric1" => slf.setattr("lyrics", Self::createLyrics(attributes)?),
            "Note" => {
                Self::translateNote(slf, attributes)?;
                lyric_moves(slf)
            }
            "Rest" => Self::translateRest(slf, attributes),
            "RestChord" => {
                Self::translateRestChord(slf, attributes)?;
                lyric_moves(slf)
            }
            "SongInfo" => Self::createSongInfo(slf, attributes),
            "StaffInstrument" => Self::createStaffInstrument(slf, attributes),
            "Tempo" => Self::createTempo(slf, attributes),
            "Text" => Self::createText(slf, attributes),
            "TimeSig" => Self::createTimeSignature(slf, attributes),
            _ => Ok(()),
        }
    }
}

/// The Python streams and notes the translator places into, as the
/// crate's placing algorithm sees them: measures and placed notes by
/// identity.
struct Streams<'py> {
    py: Python<'py>,
    voice: Bound<'py, PyAny>,
    general: Bound<'py, PyAny>,
    chord: Bound<'py, PyAny>,
    tie: Bound<'py, PyAny>,
    slur: Bound<'py, PyAny>,
}

impl<'py> Streams<'py> {
    fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            py,
            voice: class(py, "music21.stream", "Voice")?,
            general: class(py, "music21.note", "GeneralNote")?,
            chord: class(py, "music21.chord", "Chord")?,
            tie: class(py, "music21.tie", "Tie")?,
            slur: class(py, "music21.spanner", "Slur")?,
        })
    }

    /// A voice numbered `id`.
    fn voice_numbered(&self, id: usize) -> PyResult<Bound<'py, PyAny>> {
        let keywords = PyDict::new(self.py);
        keywords.set_item("id", id)?;
        self.voice.call((), Some(&keywords))
    }
}

impl<'py> Placing for Streams<'py> {
    type Measure = Py<PyAny>;
    type Voice = Bound<'py, PyAny>;
    type Element = Bound<'py, PyAny>;
    type Handle = Py<PyAny>;
    type Error = PyErr;

    fn copy_measure(&self, measure: &Py<PyAny>) -> Py<PyAny> {
        measure.clone_ref(self.py)
    }

    fn copy_handle(&self, handle: &Py<PyAny>) -> Py<PyAny> {
        handle.clone_ref(self.py)
    }

    fn same_measure(&self, first: &Py<PyAny>, second: &Py<PyAny>) -> bool {
        first.is(second)
    }

    fn same_handle(&self, first: &Py<PyAny>, second: &Py<PyAny>) -> bool {
        first.is(second)
    }

    fn handle(&self, element: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        Ok(element.clone().unbind())
    }

    fn highest_time(&self, measure: &Py<PyAny>) -> PyResult<f64> {
        measure.bind(self.py).getattr("highestTime")?.extract()
    }

    fn length(&self, element: &Bound<'py, PyAny>) -> PyResult<f64> {
        element.getattr("quarterLength")?.extract()
    }

    fn voices(&self, measure: &Py<PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        measure.bind(self.py).getattr("voices")?.walk()?.collect()
    }

    fn voice_end(&self, voice: &Bound<'py, PyAny>) -> PyResult<f64> {
        voice.getattr("highestTime")?.extract()
    }

    fn gather_voice(&mut self, measure: &Py<PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let measure = measure.bind(self.py);
        let first = self.voice_numbered(0)?;
        let held: Vec<Bound<'py, PyAny>> = measure
            .call_method1("getElementsByClass", (&self.general,))?
            .walk()?
            .collect::<PyResult<_>>()?;
        let mut placed = Vec::with_capacity(held.len());
        for note in held {
            placed.push((note.getattr("offset")?, note));
        }
        for (_, note) in &placed {
            measure.call_method1("remove", (note,))?;
        }
        for (at, note) in placed {
            first.call_method1("insert", (at, note))?;
        }
        measure.call_method1("insert", (0, &first))?;
        Ok(first)
    }

    fn new_voice(&mut self, measure: &Py<PyAny>, id: usize) -> PyResult<Bound<'py, PyAny>> {
        let made = self.voice_numbered(id)?;
        measure.bind(self.py).call_method1("insert", (0, &made))?;
        Ok(made)
    }

    fn insert(
        &mut self,
        measure: &Py<PyAny>,
        offset: f64,
        element: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let offset = crate::duration::op_frac(self.py, offset)?;
        measure
            .bind(self.py)
            .call_method1("insert", (offset, element))?;
        Ok(())
    }

    fn insert_in(
        &mut self,
        voice: &Bound<'py, PyAny>,
        offset: f64,
        element: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let offset = crate::duration::op_frac(self.py, offset)?;
        voice.call_method1("insert", (offset, element))?;
        Ok(())
    }

    fn set_ties(
        &mut self,
        element: &mut Bound<'py, PyAny>,
        ties: &[Option<TieType>],
    ) -> PyResult<()> {
        let notes: Vec<Bound<'py, PyAny>> = if element.is_instance(&self.chord)? {
            element.walk()?.collect::<PyResult<_>>()?
        } else {
            vec![element.clone()]
        };
        for (note, tie) in notes.iter().zip(ties) {
            let kind = match tie {
                Some(TieType::Start) => "start",
                Some(TieType::Stop) => "stop",
                Some(TieType::Continue) => "continue",
                _ => continue,
            };
            note.setattr("tie", self.tie.call1((kind,))?)?;
        }
        Ok(())
    }

    fn slur(&mut self, measure: &Py<PyAny>, start: &Py<PyAny>, end: &Py<PyAny>) -> PyResult<()> {
        let made = self.slur.call1((start.bind(self.py), end.bind(self.py)))?;
        measure.bind(self.py).call_method1("append", (made,))?;
        Ok(())
    }
}

/// A line's attributes as music21's `parseList` reads them: `Name:value`
/// pairs, a chord's `Dur` and `Pos` kept as lists of every one written.
fn line_attributes<'py>(
    py: Python<'py>,
    command: &str,
    sections: &[&str],
    line: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let attributes = PyDict::new(py);
    for attribute in sections.iter().skip(2) {
        let Some((name, value)) = attribute.split_once(':') else {
            if attribute.trim().is_empty() {
                continue;
            }
            return Err(NoteworthyTranslateException::new_err(format!(
                "Cannot unpack value from {attribute} in {line}"
            )));
        };
        if command == "Chord" && matches!(name, "Dur" | "Pos") {
            match attributes.get_item(name)? {
                Some(listed) => listed.call_method1("append", (value,)).map(|_| ())?,
                None => attributes.set_item(name, PyList::new(py, [value])?)?,
            }
            continue;
        }
        attributes.set_item(name, value)?;
    }
    Ok(attributes)
}

/// The wheel's key signature of so many sharps.
fn key_signature(py: Python<'_>, sharps: i32) -> PyResult<Bound<'_, PyAny>> {
    class(py, "music21.key", "KeySignature")?.call1((sharps,))
}

/// A position's written accidental, as music21 spells it, and the rest.
pub(crate) fn written_accidental(pos: &str) -> (Option<&'static str>, &str) {
    let spelled = match pos.chars().next() {
        Some('n') => "n",
        Some('b') => "-",
        Some('#') => "#",
        Some('x') => "##",
        Some('v') => "--",
        _ => return (None, pos),
    };
    (Some(spelled), &pos[1..])
}

/// A `Pos:` value without the tie and notehead marks after it.
pub(crate) fn position_only(posInfo: &str) -> &str {
    posInfo
        .trim_end_matches('^')
        .trim_end_matches('x')
        .trim_end_matches('X')
        .trim_end_matches('z')
}

/// music21's `noteworthy.translate`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_class::<NoteworthyTranslator>()?;
    let exception = py.get_type::<NoteworthyTranslateException>();
    exception.setattr("__module__", MODULE)?;
    m.add("NoteworthyTranslateException", exception)?;
    m.add(
        "environLocal",
        crate::environment::module(py)?
            .getattr("Environment")?
            .call1(("noteworthy.translate",))?,
    )?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_loses_its_marks_and_keeps_its_accidental() {
        assert_eq!(position_only("b3^"), "b3");
        assert_eq!(written_accidental("b3"), (Some("-"), "3"));
        assert_eq!(written_accidental("x0"), (Some("##"), "0"));
        assert_eq!(written_accidental("-4"), (None, "-4"));
    }
}
