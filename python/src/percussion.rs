//! music21's `percussion`: several drums struck at once, which is not a
//! chord because what is struck has no pitch.
//!
//! The members are kept as the objects they are -- a stroke
//! (`note.Unpitched`) or a note, music21's own or this package's -- since
//! a caller holding one is holding the chord's, as music21's are. What the
//! chord *is* the crate says: `percussion::PercussionChord` over those
//! members, which is what its pitches are read from.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &["PercussionChord"];

/// music21's `percussion.PercussionChord`.
#[pyclass(
    name = "PercussionChord",
    module = "music21.percussion",
    subclass,
    skip_from_py_object
)]
pub struct PercussionChord {
    /// What is struck, in the order it was given.
    notes: Vec<Py<PyAny>>,
}

impl PercussionChord {
    /// The objects a caller handed over, each a note or a stroke, which is
    /// all music21 holds in one of these.
    fn read(given: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<Py<PyAny>>> {
        let Some(given) = given.filter(|given| !given.is_none()) else {
            return Ok(Vec::new());
        };
        let mut held = Vec::new();
        for object in given.walk()? {
            let object = object?;
            if !struck(&object) {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "every element of notes must be a note.Note or note.Unpitched object",
                ));
            }
            held.push(object.unbind());
        }
        Ok(held)
    }
}

/// Whether an object is a note rather than a stroke, which is what music21
/// asks of each member before it reads a pitch off it.
fn sounds(object: &Bound<'_, PyAny>) -> bool {
    if object.extract::<PyRef<'_, crate::note::Note>>().is_ok() {
        return true;
    }
    object
        .getattr("classSet")
        .and_then(|classes| classes.contains("music21.note.Note"))
        .unwrap_or(false)
}

/// What one member is called where the chord says what it strikes:
/// music21's `_reprInternal`, which names a note by its pitch and a stroke
/// by the instrument it stands for, falling back to where it is written.
fn called(object: &Bound<'_, PyAny>) -> PyResult<String> {
    if sounds(object) {
        return object
            .getattr("pitch")?
            .getattr("nameWithOctave")?
            .extract();
    }
    if let Some(instrument) = object
        .getattr("storedInstrument")
        .ok()
        .filter(|instrument| !instrument.is_none())
    {
        return Ok(instrument.getattr("instrumentName")?.str()?.to_string());
    }
    let written = object.getattr("displayName")?.str()?.to_string();
    Ok(format!("unpitched[{written}]"))
}

/// The class a note is built with: the one standing in music21's module
/// where anything is installed there, and this package's own otherwise.
fn note_class(py: Python<'_>) -> Bound<'_, PyAny> {
    match crate::installed_class(py, "music21.note", "Note") {
        Some(installed) => installed,
        None => py.get_type::<crate::note::Note>().into_any(),
    }
}

/// Whether an object is something a percussion chord may hold: a note or a
/// stroke, this package's or music21's.
fn struck(object: &Bound<'_, PyAny>) -> bool {
    if object.extract::<PyRef<'_, crate::note::Note>>().is_ok() {
        return true;
    }
    let named = |name: &str| {
        object
            .getattr("classSet")
            .and_then(|classes| classes.contains(name))
            .unwrap_or(false)
    };
    // A rest answers to neither, which is what music21 refuses.
    (named("music21.note.Note") || named("music21.note.Unpitched")) && !named("music21.note.Rest")
}

#[pymethods]
impl PercussionChord {
    /// Refuses what music21 answers through a site, which the package
    /// keeps none of.
    fn __getattr__(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        crate::stream::no_site(slf.as_any(), name)
    }

    /// music21's `PercussionChord(notes=None)`.
    #[new]
    #[pyo3(signature = (notes = None, **_keywords))]
    fn new(
        notes: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> PyResult<Self> {
        Ok(Self {
            notes: Self::read(notes)?,
        })
    }

    /// Nothing here is a chord, whatever it holds: music21's `isChord`,
    /// which it writes `False` on this class and nowhere else.
    #[classattr]
    fn isChord() -> bool {
        false
    }

    /// What music21 says this object is.
    #[getter]
    fn classes(&self) -> Vec<String> {
        vec![
            "PercussionChord".to_string(),
            "ChordBase".to_string(),
            "NotRest".to_string(),
            "GeneralNote".to_string(),
            "Music21Object".to_string(),
        ]
    }

    /// What is struck, in order: music21's `notes`, the objects
    /// themselves.
    #[getter]
    fn notes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.notes.iter().map(|note| note.bind(py)))
    }

    #[setter]
    fn set_notes(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.notes = Self::read(value)?;
        Ok(())
    }

    /// How many things are struck.
    fn __len__(&self) -> usize {
        self.notes.len()
    }

    /// What it sounds: the pitches of the members that have one, in order,
    /// a stroke having none. music21's `pitches`.
    #[getter]
    fn pitches<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let mut found = Vec::new();
        for note in &self.notes {
            let note = note.bind(py);
            // A stroke is displayed at a place on the staff and sounds
            // nothing, so only a real note has a pitch to give.
            if sounds(note) {
                found.push(note.getattr("pitch")?);
            }
        }
        PyTuple::new(py, found)
    }

    /// Writing the pitches replaces everything struck with notes of those
    /// pitches, strokes and all, which is music21's own setter -- so a
    /// chord written to sounds only what it was given.
    #[setter]
    fn set_pitches(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let class = note_class(py);
        let mut built = Vec::new();
        for pitch in value.walk()? {
            // music21 hands whatever it was given to `Note`, so a number
            // or a name is as good as a pitch here.
            built.push(class.call1((pitch?,))?.unbind());
        }
        slf.borrow_mut().notes = built;
        Ok(())
    }

    /// Two of these are equal where they strike the same things in the
    /// same order, which music21 says is different from a chord because
    /// what is struck cannot be compared by pitch.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py = slf.py();
        let Ok(theirs) = other.getattr("notes") else {
            return Ok(false);
        };
        let mine = slf.borrow().notes.len();
        let theirs: Vec<Bound<'_, PyAny>> = theirs.walk()?.collect::<PyResult<_>>()?;
        if mine != theirs.len() {
            return Ok(false);
        }
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .notes
            .iter()
            .map(|note| note.clone_ref(py))
            .collect();
        for (mine, theirs) in held.iter().zip(theirs) {
            if !mine.bind(py).eq(&theirs)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// music21 hashes one of these by where it sits, there being nothing
    /// in it to hash that equality does not already read.
    fn __hash__(slf: &Bound<'_, Self>) -> usize {
        (slf.as_ptr() as usize) >> 4
    }

    /// music21's own `repr` for one of these: what it strikes, in order,
    /// and where it sits when it strikes nothing at all.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .notes
            .iter()
            .map(|note| note.clone_ref(py))
            .collect();
        if held.is_empty() {
            return Ok(format!(
                "<music21.percussion.PercussionChord object at 0x{:x}>",
                slf.as_ptr() as usize
            ));
        }
        let mut names = Vec::with_capacity(held.len());
        for note in &held {
            names.push(called(note.bind(py))?);
        }
        Ok(format!(
            "<music21.percussion.PercussionChord [{}]>",
            names.join(" ")
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copier = py.import("copy")?.getattr("deepcopy")?;
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .notes
            .iter()
            .map(|note| note.clone_ref(py))
            .collect();
        let copies = PyList::empty(py);
        for note in held {
            let note = note.bind(py);
            let copy = match memo {
                Some(memo) => copier.call1((note, memo))?,
                None => copier.call1((note,))?,
            };
            copies.append(copy)?;
        }
        let copied = slf.get_type().call0()?;
        copied
            .extract::<PyRefMut<'_, Self>>()?
            .set_notes(Some(copies.as_any()))?;
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PercussionChord>()?;
    Ok(())
}
