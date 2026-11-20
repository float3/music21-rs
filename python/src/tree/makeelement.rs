//! music21's `Verticality.makeElement`: a chord of what sounds at one
//! offset, its notes tied where they began before it or go on after, or a
//! rest where nothing sounds.
//!
//! It works on the score's own objects -- it copies them, or with
//! `copyPitches=False` keeps their pitches, groups them by part and
//! gathers their articulations and expressions -- so it is written over
//! the Python objects, as music21 writes it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::PyString;

use super::PitchedTimespan;
use super::verticality::VerticalityException;
use crate::duration::op_frac;
use crate::stream::answers_to_class;

/// music21's `gatherArticulations` and `gatherExpressions`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gather {
    /// None of them.
    Nothing,
    /// Every one.
    All,
    /// One of each class, from the lowest note: music21's `'single'`.
    OnePerClass,
}

impl Gather {
    /// `True`, `False` or `'single'`.
    pub(crate) fn of(given: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let Some(given) = given else {
            return Ok(Self::OnePerClass);
        };
        if given.extract::<String>().is_ok_and(|said| said == "single") {
            return Ok(Self::OnePerClass);
        }
        Ok(match given.is_truthy()? {
            true => Self::All,
            false => Self::Nothing,
        })
    }
}

/// How the element is made: music21's keywords.
pub(crate) struct Options {
    pub(crate) add_ties: bool,
    pub(crate) group_by_part: bool,
    pub(crate) remove_redundant: bool,
    pub(crate) articulations: Gather,
    pub(crate) expressions: Gather,
    pub(crate) copy_pitches: bool,
}

/// A class of this package's, or the installed music21's of that name.
fn class_of<'py, T: pyo3::PyTypeInfo>(
    py: Python<'py>,
    module: &str,
    name: &str,
) -> Bound<'py, PyAny> {
    crate::installed_class(py, module, name).unwrap_or_else(|| py.get_type::<T>().into_any())
}

/// The tie type of a note's tie, if it has one.
fn tie_type(note: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let tie = note.getattr("tie")?;
    if tie.is_none() {
        return Ok(None);
    }
    Ok(Some(tie.getattr("type")?.extract()?))
}

/// Whether two tie types are a start and a stop.
fn start_and_stop(first: &str, second: &str) -> bool {
    matches!((first, second), ("start", "stop") | ("stop", "start"))
}

/// One making of a chord, with the notes gathered so far by pitch.
struct Making<'a, 'py> {
    py: Python<'py>,
    options: &'a Options,
    offset: Bound<'py, PyAny>,
    length: Bound<'py, PyAny>,
    duration: Bound<'py, PyAny>,
    /// The notes by key, in the order first added.
    notes: Vec<(String, Bound<'py, PyAny>)>,
    seen: HashSet<String>,
    /// How many redundant pitches were kept, for their keys.
    bust: usize,
}

impl<'py> Making<'_, 'py> {
    /// The note kept under a key.
    fn kept(&self, key: &str) -> Option<&Bound<'py, PyAny>> {
        self.notes
            .iter()
            .find(|(held, _)| held == key)
            .map(|(_, note)| note)
    }

    /// Keeps a note under a key, in place of one kept there before.
    fn keep(&mut self, key: String, note: Bound<'py, PyAny>) {
        match self.notes.iter_mut().find(|(held, _)| *held == key) {
            Some(entry) => entry.1 = note,
            None => self.notes.push((key, note)),
        }
    }

    /// A copy of a note lasting the chord's length, tied as the timespan
    /// it came from runs before and after it: music21's `newNote`.
    fn new_note(
        &self,
        span: &Bound<'py, PyAny>,
        note: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self.py;
        let made = py.import("copy")?.call_method1("deepcopy", (note,))?;
        made.setattr("duration", &self.duration)?;
        if !self.options.copy_pitches {
            made.setattr("pitch", note.getattr("pitch")?)?;
        }
        if made.getattr("stemDirection")?.extract::<String>()? != "noStem" {
            made.setattr("stemDirection", "unspecified")?;
        }
        if !self.options.add_ties {
            return Ok(made);
        }

        // How the timespan runs past this chord, either side.
        let started: f64 = self.offset.sub(span.getattr("offset")?)?.extract()?;
        let ends: f64 = span
            .getattr("endTime")?
            .sub(self.offset.add(&self.length)?)?
            .extract()?;
        let added = if started == 0.0 && ends <= 0.0 {
            None
        } else if started > 0.0 {
            Some(if ends > 0.0 { "continue" } else { "stop" })
        } else if ends > 0.0 {
            Some("start")
        } else {
            return Err(VerticalityException::new_err(
                "What possibility was missed?",
            ));
        };

        let tie = made.getattr("tie")?;
        let held = tie_type(&made)?;
        match (held.as_deref(), added) {
            (Some(held), Some(added)) if start_and_stop(held, added) => {
                tie.setattr("type", "continue")?;
            }
            (Some("continue"), _) | (Some(_), None) => {
                tie.setattr("placement", py.None())?;
            }
            (_, Some(added)) => {
                let made_tie = class_of::<crate::notation::Tie>(py, "music21.tie", "Tie");
                made.setattr("tie", made_tie.call1((added,))?)?;
            }
            (None, None) => {}
        }
        Ok(made)
    }

    /// Adds a note unless its pitch is there already, where it keeps the
    /// one whose tie says more: music21's `conditionalAdd`.
    fn add(&mut self, span: &Bound<'py, PyAny>, note: &Bound<'py, PyAny>) -> PyResult<()> {
        let pitch = note.getattr("pitch")?;
        let key: String = pitch.getattr("nameWithOctave")?.extract()?;
        let group = match self.options.group_by_part {
            true => self.part_group(span, note, &pitch)?,
            false => None,
        };

        if self.seen.insert(key.clone()) {
            let made = self.new_note(span, note)?;
            self.keep(key, made);
            return Ok(());
        }
        if !self.options.remove_redundant {
            let made = self.new_note(span, note)?;
            self.keep(format!("{key}{}", self.bust), made);
            self.bust += 1;
            return Ok(());
        }
        if let (Some(group), Some(kept)) = (&group, self.kept(&key)) {
            kept.getattr("groups")?.call_method1("append", (group,))?;
            kept.getattr("pitch")?
                .getattr("groups")?
                .call_method1("append", (group,))?;
        }
        if !self.options.add_ties {
            return Ok(());
        }
        self.better_tied(span, note, key)
    }

    /// Puts the id of the part a note is in among its groups and its
    /// pitch's, and answers it.
    fn part_group(
        &self,
        span: &Bound<'py, PyAny>,
        note: &Bound<'py, PyAny>,
        pitch: &Bound<'py, PyAny>,
    ) -> PyResult<Option<String>> {
        let part = span
            .getattr("element")?
            .call_method1("getContextByClass", ("Part",))?;
        if part.is_none() {
            return Ok(None);
        }
        // Spaces are not allowed in group names.
        let group = part.getattr("id")?.str()?.to_string().replace(' ', "_");
        pitch.getattr("groups")?.call_method1("append", (&group,))?;
        note.getattr("groups")?.call_method1("append", (&group,))?;
        Ok(Some(group))
    }

    /// Keeps a note in place of the one kept at its pitch where its tie
    /// says more.
    fn better_tied(
        &mut self,
        span: &Bound<'py, PyAny>,
        note: &Bound<'py, PyAny>,
        key: String,
    ) -> PyResult<()> {
        let Some(kept) = self.kept(&key).cloned() else {
            return Ok(());
        };
        let old = tie_type(&kept)?;
        if old.as_deref() == Some("continue") {
            return Ok(());
        }
        let possible = self.new_note(span, note)?;
        possible.setattr("groups", kept.getattr("groups")?)?;
        let Some(new) = tie_type(&possible)? else {
            return Ok(());
        };
        let Some(old) = old else {
            self.keep(key, possible);
            return Ok(());
        };
        if start_and_stop(&old, &new) {
            kept.getattr("tie")?.setattr("type", "continue")?;
            return Ok(());
        }
        if new == "continue" {
            self.keep(key, possible);
            return Ok(());
        }
        if new == old {
            return Ok(());
        }
        Err(VerticalityException::new_err("Did I miss one? "))
    }

    /// The notes of a chord's timespan, the first carrying the chord's
    /// own articulations and expressions.
    fn add_chord(&mut self, span: &Bound<'py, PyAny>, chord: &Bound<'py, PyAny>) -> PyResult<()> {
        let py = self.py;
        let note_class = PyString::new(py, "Note");
        let mut notes = Vec::new();
        for held in chord.try_iter()? {
            let held = held?;
            if answers_to_class(&held, &note_class)? {
                notes.push(held);
            }
        }
        let Some(first) = notes.first().cloned() else {
            return Ok(());
        };
        let articulations = chord.getattr("articulations")?;
        let expressions = chord.getattr("expressions")?;
        let first = if articulations.is_truthy()? || expressions.is_truthy()? {
            let pitch = first.getattr("pitch")?;
            let copied = py.import("copy")?.call_method1("deepcopy", (&first,))?;
            copied.setattr(
                "articulations",
                copied.getattr("articulations")?.add(&articulations)?,
            )?;
            copied.setattr(
                "expressions",
                copied.getattr("expressions")?.add(&expressions)?,
            )?;
            if !self.options.copy_pitches {
                copied.setattr("pitch", pitch)?;
            }
            copied
        } else {
            first
        };
        self.add(span, &first)?;
        for note in notes.iter().skip(1) {
            self.add(span, note)?;
        }
        Ok(())
    }
}

/// The articulations or expressions of a note that go onto the chord:
/// none of a kind that attaches to the first of a tie on a note not
/// starting one, or to the last on one not ending one.
fn gathered<'py>(
    note: &Bound<'py, PyAny>,
    marks: &str,
    gather: Gather,
    seen: &mut Vec<Bound<'py, PyAny>>,
    onto: &Bound<'py, PyAny>,
) -> PyResult<()> {
    if gather == Gather::Nothing {
        return Ok(());
    }
    let tie = tie_type(note)?;
    for mark in note.getattr(marks)?.try_iter()? {
        let mark = mark?;
        let attach: String = mark.getattr("tieAttach")?.extract()?;
        if attach == "first" && tie.as_deref().is_some_and(|tie| tie != "start") {
            continue;
        }
        if attach == "last" && tie.as_deref().is_some_and(|tie| tie != "stop") {
            continue;
        }
        let kind = mark.get_type().into_any();
        if gather == Gather::OnePerClass && seen.iter().any(|held| held.is(&kind)) {
            continue;
        }
        onto.getattr(marks)?.call_method1("append", (&mark,))?;
        seen.push(kind);
    }
    Ok(())
}

/// music21's `makeElement` over a verticality's timespans sounding there.
pub(crate) fn make_element<'py>(
    py: Python<'py>,
    offset: &Bound<'py, PyAny>,
    length: Bound<'py, PyAny>,
    sounding: &[Bound<'py, PyAny>],
    has_pitches: bool,
    options: &Options,
) -> PyResult<Bound<'py, PyAny>> {
    let length = op_frac(py, length.extract()?)?;
    if !has_pitches {
        let rest = class_of::<crate::rest::Rest>(py, "music21.note", "Rest").call0()?;
        rest.getattr("duration")?
            .setattr("quarterLength", &length)?;
        return Ok(rest);
    }

    let chord = class_of::<crate::chord::Chord>(py, "music21.chord", "Chord").call0()?;
    chord
        .getattr("duration")?
        .setattr("quarterLength", &length)?;
    let mut making = Making {
        py,
        options,
        offset: offset.clone(),
        length,
        duration: chord.getattr("duration")?,
        notes: Vec::new(),
        seen: HashSet::new(),
        bust: 0,
    };
    let chord_base = PyString::new(py, "ChordBase");
    let note_class = PyString::new(py, "Note");
    for span in sounding {
        if span.cast::<PitchedTimespan>().is_err() {
            continue;
        }
        let element = span.getattr("element")?;
        if answers_to_class(&element, &chord_base)? {
            making.add_chord(span, &element)?;
        } else if answers_to_class(&element, &note_class)? {
            making.add(span, &element)?;
        }
    }

    // Lowest first, and the marks of each kind from the lowest; a stable
    // sort, as Python's is.
    let mut notes = Vec::new();
    for (_, note) in making.notes {
        let ps: f64 = note.getattr("pitch")?.getattr("ps")?.extract()?;
        notes.push((ps, note));
    }
    notes.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut articulations = Vec::new();
    let mut expressions = Vec::new();
    for (_, note) in notes {
        chord.call_method1("add", (&note,))?;
        gathered(
            &note,
            "articulations",
            options.articulations,
            &mut articulations,
            &chord,
        )?;
        gathered(
            &note,
            "expressions",
            options.expressions,
            &mut expressions,
            &chord,
        )?;
    }
    Ok(chord)
}
