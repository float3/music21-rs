//! music21's `tablature`: a chord diagram -- which string is stopped at
//! which fret, and with which finger.
//!
//! A fret note and a fret board are values the crate keeps, so these carry
//! them and answer music21's questions off them: the note on a string, the
//! notes lowest string first, and what the board sounds, each string's
//! open pitch raised by the fret stopping it.
//!
//! `ChordWithFretBoard` is a chord symbol with a board under it, and
//! music21 builds it out of its own `ChordSymbol`; this carries the board
//! and the figure, and the symbol's own notes are the harmony facade's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21_rs_crate::tablature::{
    FirstFret as RsFirstFret, FretBoard as RsFretBoard, FretNote as RsFretNote,
};

use crate::Walkable;

pyo3::create_exception!(
    music21_rs_facade,
    TablatureException,
    crate::Music21Exception
);

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "FretNote",
    "FretBoard",
    "FirstFret",
    "GuitarFretBoard",
    "UkeleleFretBoard",
    "BassGuitarFretBoard",
    "MandolinFretBoard",
    "TablatureException",
];

/// music21's `tablature.FretNote`: one string stopped at one fret, with
/// the finger that stops it where the diagram says.
#[pyclass(
    name = "FretNote",
    module = "music21.tablature",
    subclass,
    skip_from_py_object
)]
pub struct FretNote {
    pub(crate) inner: RsFretNote,
}

impl FretNote {
    pub(crate) fn wrap(inner: RsFretNote) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl FretNote {
    /// music21's `FretNote(string=None, fret=None, fingering=None)`.
    #[new]
    #[pyo3(signature = (string = None, fret = None, fingering = None, **_keywords))]
    fn new(
        string: Option<i32>,
        fret: Option<i32>,
        fingering: Option<i32>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> Self {
        Self::wrap(RsFretNote::new(string, fret, fingering))
    }

    /// Which string it stops, counted from the highest as music21 counts.
    #[getter]
    fn string(&self) -> Option<i32> {
        self.inner.string
    }

    #[setter]
    fn set_string(&mut self, value: Option<i32>) {
        self.inner.string = value;
    }

    /// Which fret stops it, nought for an open string.
    #[getter]
    fn fret(&self) -> Option<i32> {
        self.inner.fret
    }

    #[setter]
    fn set_fret(&mut self, value: Option<i32>) {
        self.inner.fret = value;
    }

    /// Which finger stops it, where the diagram says.
    #[getter]
    fn fingering(&self) -> Option<i32> {
        self.inner.fingering
    }

    #[setter]
    fn set_fingering(&mut self, value: Option<i32>) {
        self.inner.fingering = value;
    }

    /// Whether the finger's number is drawn: music21's
    /// `displayFingerNumber`.
    #[getter]
    fn displayFingerNumber(&self) -> bool {
        self.inner.display_finger_number
    }

    #[setter]
    fn set_displayFingerNumber(&mut self, value: bool) {
        self.inner.display_finger_number = value;
    }

    /// What music21 says this object is.
    #[getter]
    fn classes(&self) -> Vec<String> {
        vec!["FretNote".to_string(), "ProtoM21Object".to_string()]
    }

    /// music21's `_reprInternal`, which says what the note stops and with
    /// what; a note that stops nothing says nothing at all.
    fn __repr__(&self) -> String {
        let said = self.inner.to_string();
        if said.is_empty() {
            "<music21.tablature.FretNote>".to_string()
        } else {
            format!("<music21.tablature.FretNote {said}>")
        }
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsFretNote>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's `tablature.FirstFret`: which fret a diagram starts at, and
/// where that number is written.
#[pyclass(
    name = "FirstFret",
    module = "music21.tablature",
    subclass,
    skip_from_py_object
)]
pub struct FirstFret {
    pub(crate) inner: RsFirstFret,
}

#[pymethods]
impl FirstFret {
    #[new]
    #[pyo3(signature = (fretNum, location = None))]
    fn new(fretNum: i32, location: Option<String>) -> Self {
        let mut inner = RsFirstFret::new(fretNum);
        if let Some(location) = location {
            inner.location = location;
        }
        Self { inner }
    }

    /// The fret the diagram starts at.
    #[getter]
    fn fretNum(&self) -> i32 {
        self.inner.fret
    }

    #[setter]
    fn set_fretNum(&mut self, value: i32) {
        self.inner.fret = value;
    }

    /// Which side of the diagram the number is written on.
    #[getter]
    fn location(&self) -> &str {
        &self.inner.location
    }

    #[setter]
    fn set_location(&mut self, value: String) {
        self.inner.location = value;
    }
}

/// The fret notes a caller handed over, read as the crate's values.
fn notes_of(given: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<RsFretNote>> {
    let Some(given) = given.filter(|given| !given.is_none()) else {
        return Ok(Vec::new());
    };
    let mut read = Vec::new();
    for note in given.walk()? {
        let note = note?;
        match note.extract::<PyRef<'_, FretNote>>() {
            Ok(held) => read.push(held.inner.clone()),
            // music21's own `FretNote`, or anything answering to the three
            // numbers a fret note is.
            Err(_) => read.push(RsFretNote::new(
                note.getattr("string")?.extract()?,
                note.getattr("fret")?.extract()?,
                note.getattr("fingering")?.extract()?,
            )),
        }
    }
    Ok(read)
}

/// music21's `tablature.FretBoard`: a chord diagram, its strings and the
/// notes stopping them.
#[pyclass(
    name = "FretBoard",
    module = "music21.tablature",
    subclass,
    skip_from_py_object
)]
pub struct FretBoard {
    pub(crate) inner: RsFretBoard,
}

impl FretBoard {
    pub(crate) fn wrap(inner: RsFretBoard) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl FretBoard {
    /// music21's `FretBoard(numStrings=6, fretNotes=None, displayFrets=4)`.
    #[new]
    #[pyo3(signature = (numStrings = 6, fretNotes = None, displayFrets = 4, **_keywords))]
    fn new(
        numStrings: usize,
        fretNotes: Option<&Bound<'_, PyAny>>,
        displayFrets: usize,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Ok(Self::wrap(RsFretBoard::new(
            numStrings,
            notes_of(fretNotes)?,
            displayFrets,
        )))
    }

    /// How many strings it has.
    #[getter]
    fn numStrings(&self) -> usize {
        self.inner.strings
    }

    #[setter]
    fn set_numStrings(&mut self, value: usize) {
        self.inner.strings = value;
    }

    /// How many frets the diagram shows.
    #[getter]
    fn displayFrets(&self) -> usize {
        self.inner.display_frets
    }

    #[setter]
    fn set_displayFrets(&mut self, value: usize) {
        self.inner.display_frets = value;
    }

    /// The notes stopping its strings, in the order they were given.
    #[getter]
    fn fretNotes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for note in &self.inner.fret_notes {
            out.append(Bound::new(py, FretNote::wrap(note.clone()))?)?;
        }
        Ok(out)
    }

    #[setter]
    fn set_fretNotes(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.fret_notes = notes_of(value)?;
        Ok(())
    }

    /// What its open strings are tuned to, lowest first.
    #[getter]
    fn tuning<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for pitch in &self.inner.tuning {
            out.append(crate::installed_new(
                py,
                "music21.pitch",
                "Pitch",
                crate::pitch::Pitch::wrap(pitch.clone(), false),
            )?)?;
        }
        Ok(out)
    }

    /// The note stopping a string, or nothing where none does: music21's
    /// `getFretNoteByString`.
    fn getFretNoteByString(&self, requestedString: i32) -> Option<FretNote> {
        self.inner
            .fret_note_by_string(requestedString)
            .map(|note| FretNote::wrap(note.clone()))
    }

    /// Its notes, the lowest string first: music21's
    /// `fretNotesLowestFirst`.
    fn fretNotesLowestFirst<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for note in self.inner.fret_notes_lowest_first() {
            out.append(Bound::new(py, FretNote::wrap(note.clone()))?)?;
        }
        Ok(out)
    }

    /// What the diagram sounds, each string's open pitch raised by the
    /// fret stopping it and nothing for a string nobody stops: music21's
    /// `getPitches`.
    fn getPitches<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        let sounded = self
            .inner
            .pitches()
            .map_err(|error| TablatureException::new_err(crate::pitch::message(&error)))?;
        for pitch in sounded {
            match pitch {
                None => out.append(py.None())?,
                Some(pitch) => out.append(crate::installed_new(
                    py,
                    "music21.pitch",
                    "Pitch",
                    crate::pitch::Pitch::wrap(pitch, false),
                )?)?,
            }
        }
        Ok(out)
    }

    /// What music21 says this object is.
    #[getter]
    fn classes(&self) -> Vec<String> {
        vec!["FretBoard".to_string(), "ProtoM21Object".to_string()]
    }

    /// music21's `_reprInternal`: how many strings it has, how many notes
    /// stop them and how many frets are drawn, under the name of whatever
    /// kind of board it is.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let me = slf.borrow();
        Ok(format!(
            "<music21.tablature.{} {} strings, {} notes, {} frets>",
            slf.get_type().name()?,
            me.inner.strings,
            me.inner.fret_notes.len(),
            me.inner.display_frets
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.get_type().call0()?;
        {
            let me = slf.borrow();
            let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
            copy.inner = me.inner.clone();
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsFretBoard>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's boards of a known instrument, each a `FretBoard` tuned as
/// that instrument is.
macro_rules! fret_boards {
    ($(($class:ident, $name:literal, $made:ident, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(
                name = $name,
                module = "music21.tablature",
                extends = FretBoard,
                subclass
            )]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (fretNotes = None, displayFrets = 4, **_keywords))]
                fn new(
                    fretNotes: Option<&Bound<'_, PyAny>>,
                    displayFrets: usize,
                    _keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    let board = RsFretBoard::$made(notes_of(fretNotes)?, displayFrets);
                    Ok(PyClassInitializer::from(FretBoard::wrap(board))
                        .add_subclass($class))
                }
            }
        )*

        pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<FretNote>()?;
            m.add_class::<FretBoard>()?;
            m.add_class::<FirstFret>()?;
            $(m.add_class::<$class>()?;)*
            Ok(())
        }
    };
}

fret_boards!(
    (
        GuitarFretBoard,
        "GuitarFretBoard",
        guitar,
        "music21's `GuitarFretBoard`: six strings, tuned E A D G B E."
    ),
    (
        UkeleleFretBoard,
        "UkeleleFretBoard",
        ukulele,
        "music21's `UkeleleFretBoard`: four strings, tuned G C E A."
    ),
    (
        BassGuitarFretBoard,
        "BassGuitarFretBoard",
        bass_guitar,
        "music21's `BassGuitarFretBoard`: four strings, tuned E A D G."
    ),
    (
        MandolinFretBoard,
        "MandolinFretBoard",
        mandolin,
        "music21's `MandolinFretBoard`: four strings, tuned G D A E."
    ),
);
