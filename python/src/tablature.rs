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
use pyo3::types::{PyDict, PyList, PyTuple};

use music21::tablature::{
    FirstFret as RsFirstFret, FretBoard as RsFretBoard, FretNote as RsFretNote,
};

use crate::Walkable;
use crate::plumbing::built::{method, new_class};

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
    fn set_displayFingerNumber(&mut self, #[pyo3(from_py_with = crate::truthy)] value: bool) {
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

/// Fret notes as a list of this package's.
fn notes_list<'py, 'a>(
    py: Python<'py>,
    notes: impl Iterator<Item = &'a RsFretNote>,
) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for note in notes {
        out.append(Bound::new(py, FretNote::wrap(note.clone()))?)?;
    }
    Ok(out)
}

/// A pitch of the crate's, as the `Pitch` music21 would hand back.
fn pitch_object(py: Python<'_>, pitch: music21::Pitch) -> PyResult<Py<crate::pitch::Pitch>> {
    crate::installed_new(
        py,
        "music21.pitch",
        "Pitch",
        crate::pitch::Pitch::wrap(pitch, false),
    )
}

/// A board's tuning as a list of pitches, lowest string first.
fn tuning_list<'py>(py: Python<'py>, board: &RsFretBoard) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for pitch in &board.tuning {
        out.append(pitch_object(py, pitch.clone())?)?;
    }
    Ok(out)
}

/// A tuning given as pitches, or their names.
fn tuning_of(value: &Bound<'_, PyAny>) -> PyResult<Vec<music21::Pitch>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    value
        .walk()?
        .map(|pitch| crate::pitch::pitch_from_any(&pitch?))
        .collect()
}

/// What a board sounds, string by string, `None` for a string nobody
/// stops: music21's `getPitches`.
fn pitches_list<'py>(py: Python<'py>, board: &RsFretBoard) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    let sounded = board
        .pitches()
        .map_err(|error| TablatureException::new_err(crate::pitch::message(&error)))?;
    for pitch in sounded {
        match pitch {
            None => out.append(py.None())?,
            Some(pitch) => out.append(pitch_object(py, pitch)?)?,
        }
    }
    Ok(out)
}

/// Where an object keeps its board: under this name in its `__dict__`.
const BOARD: &str = "_fretBoard";

/// The module the boards are music21's of.
const MODULE: &str = "music21.tablature";

/// A board as an object keeps it in its `__dict__`. A `FretBoard` keeps
/// its board there rather than in a layout of its own, so that
/// `ChordWithFretBoard` can be a `ChordSymbol` and a `FretBoard` both.
#[pyclass(name = "_FretBoardValue", module = "music21_rs", skip_from_py_object)]
pub struct BoardValue {
    inner: RsFretBoard,
}

#[pymethods]
impl BoardValue {
    /// A blank board, or the one written out by `__reduce__`.
    #[new]
    #[pyo3(signature = (written = None))]
    fn new(written: Option<&str>) -> PyResult<Self> {
        let Some(written) = written else {
            return Ok(Self { inner: blank() });
        };
        let inner = serde_json::from_str(written)
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
        Ok(Self { inner })
    }

    fn __deepcopy__(&self, _memo: Option<&Bound<'_, PyAny>>) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }

    fn __copy__(&self) -> Self {
        self.__deepcopy__(None)
    }

    fn __reduce__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        let written = serde_json::to_string(&slf.borrow().inner)
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
        PyTuple::new(
            slf.py(),
            [
                slf.get_type().into_any(),
                PyTuple::new(slf.py(), [written])?.into_any(),
            ],
        )
    }
}

/// The board music21's `FretBoard()` starts with: six strings, no notes,
/// four frets shown.
fn blank() -> RsFretBoard {
    RsFretBoard::new(6, Vec::new(), 4)
}

/// An object's board, a blank one first made where it has none yet.
fn board_of<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, BoardValue>> {
    let held = object.getattr("__dict__")?;
    if let Ok(found) = held.get_item(BOARD) {
        return Ok(found.cast_into::<BoardValue>()?);
    }
    let made = Bound::new(object.py(), BoardValue { inner: blank() })?;
    held.set_item(BOARD, &made)?;
    Ok(made)
}

/// Reads an object's board.
fn read_board<R>(object: &Bound<'_, PyAny>, read: impl FnOnce(&RsFretBoard) -> R) -> PyResult<R> {
    let held = board_of(object)?;
    let held = held.borrow();
    Ok(read(&held.inner))
}

/// Changes an object's board.
fn change_board<R>(
    object: &Bound<'_, PyAny>,
    change: impl FnOnce(&mut RsFretBoard) -> R,
) -> PyResult<R> {
    let held = board_of(object)?;
    let mut held = held.borrow_mut();
    Ok(change(&mut held.inner))
}

/// Gives an object a board.
fn set_board(object: &Bound<'_, PyAny>, board: RsFretBoard) -> PyResult<()> {
    change_board(object, |held| *held = board)
}

/// music21's `FretBoard(numStrings=6, fretNotes=None, displayFrets=4)`.
#[pyfunction]
#[pyo3(signature = (slf, numStrings = 6, fretNotes = None, displayFrets = 4, **_keywords))]
fn board_init(
    slf: &Bound<'_, PyAny>,
    numStrings: usize,
    fretNotes: Option<&Bound<'_, PyAny>>,
    displayFrets: usize,
    _keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    set_board(
        slf,
        RsFretBoard::new(numStrings, notes_of(fretNotes)?, displayFrets),
    )
}

/// How many strings it has.
#[pyfunction]
fn num_strings(slf: &Bound<'_, PyAny>) -> PyResult<usize> {
    read_board(slf, |board| board.strings)
}

#[pyfunction]
fn set_num_strings(slf: &Bound<'_, PyAny>, value: usize) -> PyResult<()> {
    change_board(slf, |board| board.strings = value)
}

/// How many frets the diagram shows.
#[pyfunction]
fn display_frets(slf: &Bound<'_, PyAny>) -> PyResult<usize> {
    read_board(slf, |board| board.display_frets)
}

#[pyfunction]
fn set_display_frets(slf: &Bound<'_, PyAny>, value: usize) -> PyResult<()> {
    change_board(slf, |board| board.display_frets = value)
}

/// The notes stopping its strings, in the order they were given.
#[pyfunction]
fn fret_notes<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let notes = read_board(slf, |board| board.fret_notes.clone())?;
    notes_list(slf.py(), notes.iter())
}

#[pyfunction]
fn set_fret_notes(slf: &Bound<'_, PyAny>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let notes = notes_of(value)?;
    change_board(slf, |board| board.fret_notes = notes)
}

/// What its open strings are tuned to, lowest first.
#[pyfunction]
fn tuning<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let board = read_board(slf, RsFretBoard::clone)?;
    tuning_list(slf.py(), &board)
}

#[pyfunction]
fn set_tuning(slf: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
    let tuning = tuning_of(value)?;
    change_board(slf, |board| board.tuning = tuning)
}

/// The note stopping a string, or nothing where none does: music21's
/// `getFretNoteByString`.
#[pyfunction]
fn note_by_string(slf: &Bound<'_, PyAny>, requestedString: i32) -> PyResult<Option<FretNote>> {
    read_board(slf, |board| {
        board
            .fret_note_by_string(requestedString)
            .map(|note| FretNote::wrap(note.clone()))
    })
}

/// Its notes, the lowest string first: music21's `fretNotesLowestFirst`.
#[pyfunction]
fn notes_lowest_first<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let board = read_board(slf, RsFretBoard::clone)?;
    notes_list(slf.py(), board.fret_notes_lowest_first().into_iter())
}

/// What the diagram sounds, each string's open pitch raised by the fret
/// stopping it and nothing for a string nobody stops: music21's
/// `getPitches`.
#[pyfunction]
fn board_pitches<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let board = read_board(slf, RsFretBoard::clone)?;
    pitches_list(slf.py(), &board)
}

/// music21's `_reprInternal`: how many strings it has, how many notes stop
/// them and how many frets are drawn.
#[pyfunction]
fn board_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    read_board(slf, |board| {
        format!(
            "{} strings, {} notes, {} frets",
            board.strings,
            board.fret_notes.len(),
            board.display_frets
        )
    })
}

/// A copy holding a copy of the board, as music21's `deepcopy` makes.
#[pyfunction]
#[pyo3(signature = (slf, _memo = None))]
fn board_copy<'py>(
    slf: &Bound<'py, PyAny>,
    _memo: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let copied = slf.get_type().call0()?;
    set_board(&copied, read_board(slf, RsFretBoard::clone)?)?;
    Ok(copied)
}

/// Pickled as the package's other objects are: by the name of its class,
/// which the package's `_thawed` finds again with or without music21, and
/// what it keeps.
#[pyfunction]
fn board_reduce<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py = slf.py();
    let class = slf.get_type();
    let thaw = py
        .import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?
        .getattr("_thawed")?;
    let named = (class.getattr("__module__")?, class.getattr("__qualname__")?);
    let kept = slf.getattr("__dict__")?.call_method0("copy")?;
    PyTuple::new(py, [thaw, named.into_pyobject(py)?.into_any(), kept])
}

/// The boards of a known instrument, each tuned as that instrument is: its
/// class name, how the crate makes it, and music21's docs.
type Made = fn(Vec<RsFretNote>, usize) -> RsFretBoard;
const INSTRUMENTS: [(&str, Made, &str); 4] = [
    (
        "GuitarFretBoard",
        RsFretBoard::guitar,
        "music21's `GuitarFretBoard`: six strings, tuned E A D G B E.",
    ),
    (
        "UkeleleFretBoard",
        RsFretBoard::ukulele,
        "music21's `UkeleleFretBoard`: four strings, tuned G C E A.",
    ),
    (
        "BassGuitarFretBoard",
        RsFretBoard::bass_guitar,
        "music21's `BassGuitarFretBoard`: four strings, tuned E A D G.",
    ),
    (
        "MandolinFretBoard",
        RsFretBoard::mandolin,
        "music21's `MandolinFretBoard`: four strings, tuned G D A E.",
    ),
];

/// The class attribute naming which of [`INSTRUMENTS`] a class is.
const INSTRUMENT: &str = "_instrument";

/// A known instrument's `__init__(fretNotes=None, displayFrets=4)`.
#[pyfunction]
#[pyo3(signature = (slf, fretNotes = None, displayFrets = 4, **_keywords))]
fn instrument_init(
    slf: &Bound<'_, PyAny>,
    fretNotes: Option<&Bound<'_, PyAny>>,
    displayFrets: usize,
    _keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let at: usize = slf.getattr(INSTRUMENT)?.extract()?;
    let (_, made, _) = INSTRUMENTS[at];
    set_board(slf, made(notes_of(fretNotes)?, displayFrets))
}

/// music21's `tablature.FretBoard`: a chord diagram, its strings and the
/// notes stopping them. A Python class whose members are Rust, keeping its
/// board in its `__dict__`: one with a layout of its own could not be a
/// second base of `ChordWithFretBoard`, which is a `ChordSymbol` too.
fn fret_board_class<'py>(m: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyAny>> {
    let py = m.py();
    let property = py.import("builtins")?.getattr("property")?;
    let wrapped =
        |function: Bound<'py, PyAny>, name: &str| method(function, MODULE, "FretBoard", name);
    let pair = |get: Bound<'py, PyAny>, set: Bound<'py, PyAny>| property.call1((get, set));
    let members = vec![
        (
            "__init__",
            wrapped(wrap_pyfunction!(board_init, m)?.into_any(), "__init__")?,
        ),
        (
            "numStrings",
            pair(
                wrap_pyfunction!(num_strings, m)?.into_any(),
                wrap_pyfunction!(set_num_strings, m)?.into_any(),
            )?,
        ),
        (
            "displayFrets",
            pair(
                wrap_pyfunction!(display_frets, m)?.into_any(),
                wrap_pyfunction!(set_display_frets, m)?.into_any(),
            )?,
        ),
        (
            "fretNotes",
            pair(
                wrap_pyfunction!(fret_notes, m)?.into_any(),
                wrap_pyfunction!(set_fret_notes, m)?.into_any(),
            )?,
        ),
        (
            "tuning",
            pair(
                wrap_pyfunction!(tuning, m)?.into_any(),
                wrap_pyfunction!(set_tuning, m)?.into_any(),
            )?,
        ),
        (
            "getFretNoteByString",
            wrapped(
                wrap_pyfunction!(note_by_string, m)?.into_any(),
                "getFretNoteByString",
            )?,
        ),
        (
            "fretNotesLowestFirst",
            wrapped(
                wrap_pyfunction!(notes_lowest_first, m)?.into_any(),
                "fretNotesLowestFirst",
            )?,
        ),
        (
            "getPitches",
            wrapped(wrap_pyfunction!(board_pitches, m)?.into_any(), "getPitches")?,
        ),
        (
            "_reprInternal",
            wrapped(wrap_pyfunction!(board_repr, m)?.into_any(), "_reprInternal")?,
        ),
        (
            "__deepcopy__",
            wrapped(wrap_pyfunction!(board_copy, m)?.into_any(), "__deepcopy__")?,
        ),
        (
            "__copy__",
            wrapped(wrap_pyfunction!(board_copy, m)?.into_any(), "__copy__")?,
        ),
        (
            "__reduce__",
            wrapped(wrap_pyfunction!(board_reduce, m)?.into_any(), "__reduce__")?,
        ),
    ];
    new_class(
        py,
        MODULE,
        "FretBoard",
        "A chord diagram: its strings, the notes stopping them and how many frets are drawn.",
        vec![py.get_type::<crate::prebase::ProtoM21Object>().into_any()],
        members,
    )
}

/// music21's `ChordWithFretBoard`: a chord symbol with a diagram of where
/// it is fingered, a `ChordSymbol` and a `FretBoard` both, as music21's is.
/// `FretBoard` is its second base.
#[pyclass(
    name = "ChordWithFretBoard",
    module = "music21.tablature",
    extends = crate::harmony::ChordSymbol,
    subclass
)]
pub struct ChordWithFretBoard;

#[pymethods]
impl ChordWithFretBoard {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        py: Python<'_>,
        _arguments: &Bound<'_, pyo3::types::PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(crate::harmony::ChordSymbol::initializer(py)?.add_subclass(Self))
    }

    /// music21's `ChordWithFretBoard(figure, numStrings=6, fretNotes=None,
    /// displayFrets=4, **keywords)`: the symbol read from `figure`, the
    /// rest the board's.
    #[pyo3(signature = (figure = None, numStrings = 6, fretNotes = None, displayFrets = 4, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        figure: Option<&Bound<'_, PyAny>>,
        numStrings: usize,
        fretNotes: Option<&Bound<'_, PyAny>>,
        displayFrets: usize,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let arguments = PyDict::new(py);
        if let Some(keywords) = keywords {
            arguments.update(keywords.as_mapping())?;
        }
        arguments.set_item("figure", figure)?;
        py.get_type::<crate::harmony::ChordSymbol>()
            .getattr("__init__")?
            .call((slf,), Some(&arguments))?;
        let board = RsFretBoard::new(numStrings, notes_of(fretNotes)?, displayFrets);
        set_board(slf.as_any(), board)
    }

    /// A copy: the chord symbol's, with a copy of the board.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf
            .py()
            .get_type::<crate::harmony::ChordSymbol>()
            .getattr("__deepcopy__")?
            .call1((slf, memo))?;
        set_board(&copied, read_board(slf.as_any(), RsFretBoard::clone)?)?;
        Ok(copied)
    }

    /// What music21 says this is: the names of its classes, in method
    /// resolution order, `FretBoard` among them.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        slf.get_type()
            .getattr("__mro__")?
            .try_iter()?
            .map(|class| class?.getattr("__name__")?.extract())
            .collect()
    }

    /// music21's `getFretNotesFromFigure`, which music21 leaves to do and
    /// answers `None`.
    fn getFretNotesFromFigure(&self) -> Option<()> {
        None
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<FretNote>()?;
    m.add_class::<BoardValue>()?;
    let board = fret_board_class(m)?;
    m.add("FretBoard", &board)?;
    m.add_class::<FirstFret>()?;
    m.add_class::<ChordWithFretBoard>()?;

    // Each known instrument's board, a `FretBoard` that makes itself with
    // that instrument's tuning.
    for (at, (name, _, doc)) in INSTRUMENTS.iter().enumerate() {
        let init = method(
            wrap_pyfunction!(instrument_init, m)?.into_any(),
            MODULE,
            name,
            "__init__",
        )?;
        let members = vec![
            (INSTRUMENT, at.into_pyobject(py)?.into_any()),
            ("__init__", init),
        ];
        m.add(
            *name,
            new_class(py, MODULE, name, doc, vec![board.clone()], members)?,
        )?;
    }
    Ok(())
}
