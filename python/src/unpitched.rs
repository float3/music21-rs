//! music21's `note.Unpitched`: a stroke written on a line or space of the
//! staff that sounds no pitch, as a drum's.
//!
//! What a stroke has besides where it is displayed is what a note has --
//! its length, stem, notehead, beams, tie, words, marks and instrument --
//! so it keeps a note standing at the display position, as the crate's
//! `percussion::Unpitched` does, and answers those members from it. The
//! note's pitch is the display position and is not handed out: a stroke
//! has no `pitch`, as music21's has none.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use crate::note::Note;

/// The members a stroke answers from the note it keeps: music21's
/// `NotRest` and `GeneralNote`, but for the pitch.
const WRITTEN: &[&str] = &[
    "storedInstrument",
    "beams",
    "stemDirection",
    "notehead",
    "noteheadFill",
    "noteheadParenthesis",
    "tie",
    "lyrics",
    "lyric",
    "addLyric",
    "insertLyric",
    "articulations",
    "expressions",
    "volume",
    "hasVolumeInformation",
    "editorial",
    "style",
    "hasStyleInformation",
];

/// The members a stroke keeps its length by, answered as the note's.
const LENGTHS: &[&str] = &["duration", "quarterLength"];

/// The members two strokes are compared by: music21's `equalityAttributes`
/// of `Unpitched` and the classes it is built on.
const COMPARED: &[&str] = &[
    "displayStep",
    "displayOctave",
    "storedInstrument",
    "duration",
    "tie",
    "notehead",
    "noteheadFill",
    "noteheadParenthesis",
    "beams",
];

/// music21's `note.Unpitched`.
#[pyclass(
    name = "Unpitched",
    module = "music21.note",
    extends = crate::base::NotRest,
    subclass,
    skip_from_py_object,
    weakref
)]
pub struct Unpitched {
    /// A note standing where the stroke is displayed, carrying everything
    /// written about it.
    written: Py<Note>,
}

impl Unpitched {
    /// The facade around a stroke of the crate's.
    pub(crate) fn of(py: Python<'_>, stroke: &music21::Unpitched) -> PyResult<Self> {
        Ok(Self {
            written: Note::object(py, stroke.written().clone())?,
        })
    }

    /// The written note's pitch, which is where the stroke is displayed.
    fn place<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.written.bind(py).getattr("pitch")
    }
}

#[pymethods]
impl Unpitched {
    /// music21's `Unpitched(displayName=None, **keywords)`: displayed on
    /// the middle line, `B4`, or where the name would stand in treble
    /// clef, with the notation members given as keywords.
    #[new]
    #[pyo3(signature = (displayName = None, **keywords))]
    fn new(
        py: Python<'_>,
        displayName: Option<&str>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let stroke = match displayName.filter(|name| !name.is_empty()) {
            Some(name) => music21::Unpitched::from_display_name(name),
            None => Ok(music21::Unpitched::new()),
        }
        .map_err(crate::note::note_error)?;
        let written = Note::object(py, stroke.written().clone())?;
        if let Some(keywords) = keywords {
            for (name, value) in keywords.iter() {
                let name: String = name.extract()?;
                if WRITTEN.contains(&name.as_str()) || LENGTHS.contains(&name.as_str()) {
                    written.bind(py).setattr(name.as_str(), value)?;
                }
            }
        }
        Ok(crate::base::built(Self { written }))
    }

    /// The notation members, answered by the note the stroke keeps; any
    /// other name is refused, as Python refuses it.
    fn __getattr__(slf: &Bound<'_, Self>, name: &str) -> PyResult<Py<PyAny>> {
        if WRITTEN.contains(&name) {
            let written = slf.borrow().written.clone_ref(slf.py());
            return Ok(written.bind(slf.py()).getattr(name)?.unbind());
        }
        crate::stream::no_site(slf.as_any(), name)
    }

    /// The notation members are written onto the note the stroke keeps;
    /// everything else as on any object.
    fn __setattr__(slf: &Bound<'_, Self>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if WRITTEN.contains(&name) {
            let written = slf.borrow().written.clone_ref(py);
            return written.bind(py).setattr(name, value);
        }
        crate::plain_setattr(slf, name, value)
    }

    /// How long it lasts, the note's duration, which a stream reads off
    /// the class as well as off the object.
    #[getter]
    fn get_duration(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.written.bind(py).getattr("duration")?.unbind())
    }

    #[setter]
    fn set_duration(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.written.bind(py).setattr("duration", value)
    }

    #[getter]
    fn get_quarterLength(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.written.bind(py).getattr("quarterLength")?.unbind())
    }

    #[setter]
    fn set_quarterLength(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.written.bind(py).setattr("quarterLength", value)
    }

    /// The letter of the line or space it is displayed on.
    #[getter]
    fn get_displayStep(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.place(py)?.getattr("step")?.unbind())
    }

    #[setter]
    fn set_displayStep(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.place(py)?.setattr("step", value)
    }

    /// The octave it is displayed in.
    #[getter]
    fn get_displayOctave(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.place(py)?.getattr("implicitOctave")?.unbind())
    }

    #[setter]
    fn set_displayOctave(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.place(py)?.setattr("octave", value)
    }

    /// Where it is displayed, as a new pitch, which never has an
    /// accidental: music21's `displayPitch()`.
    fn displayPitch<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let keywords = PyDict::new(py);
        keywords.set_item("step", self.get_displayStep(py)?)?;
        keywords.set_item("octave", self.get_displayOctave(py)?)?;
        py.get_type::<crate::Pitch>().call((), Some(&keywords))
    }

    /// Where it is displayed, as a name: music21's `displayName`.
    #[getter]
    fn displayName(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.displayPitch(py)?.getattr("nameWithOctave")?.unbind())
    }

    /// A stroke sounds no pitch.
    #[getter]
    fn pitches<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    #[getter]
    fn isNote(&self) -> bool {
        false
    }

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isChord(&self) -> bool {
        false
    }

    /// What music21 says this is: its own class and every class it is a
    /// kind of, as music21's `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }

    /// Two strokes are equal where they are displayed alike, are played by
    /// the same instrument and are written alike, as music21 compares them.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !other.is_instance(&slf.get_type())? {
            return Ok(false);
        }
        for name in COMPARED {
            if !slf.getattr(*name)?.eq(other.getattr(*name)?)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    /// music21's `repr`: the instrument playing it, where one is kept.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let class = slf.get_type().qualname()?;
        let instrument = slf.getattr("storedInstrument")?;
        if !instrument.is_truthy()? {
            return Ok(format!("<music21.note.{class}>"));
        }
        let name = instrument.getattr("instrumentName")?.repr()?;
        Ok(format!("<music21.note.{class} {name}>"))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let deepcopy = py.import("copy")?.getattr("deepcopy")?;
        let written = slf.borrow().written.clone_ref(py);
        let copied = match memo {
            Some(memo) => deepcopy.call1((written, memo))?,
            None => deepcopy.call1((written,))?,
        };
        crate::copy_as_same_type(
            slf,
            Self {
                written: copied.extract()?,
            },
        )
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None).map(crate::base::shallow)
    }

    /// Pickled as the note it keeps, which pickles itself.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let extra = PyDict::new(slf.py());
        extra.set_item("written", slf.borrow().written.bind(slf.py()))?;
        crate::pickled_extra(slf, &(), Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (_, extra) = crate::unpickled_extra::<_, ()>(slf, state)?;
        if let Some(extra) = extra
            && let Ok(written) = extra.bind(py).get_item("written")
        {
            slf.borrow_mut().written = written.extract()?;
        }
        Ok(())
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.written)
    }
}

/// Whether an object is one of these.
pub(crate) fn is_unpitched(object: &Bound<'_, PyAny>) -> bool {
    object.cast::<Unpitched>().is_ok()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Unpitched>()?;
    Ok(())
}
