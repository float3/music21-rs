//! music21's `note.Note` and `duration.Duration` over `music21-rs`, enough of
//! each for the chord facade to hand real notes back and forth.

#![allow(non_snake_case)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs::{
    Duration as RsDuration, Note as RsNote, Notehead as RsNotehead, Pitch as RsPitch,
    StemDirection as RsStemDirection,
};

use crate::interval::transpose_pitch_by_any;
use crate::notation::{Lyric, Style, StyleOwner, Tie, Volume, tie_from_any, volume_from_any};
use crate::pitch::{Pitch, message, pitch_from_any};

pyo3::create_exception!(music21_rs_facade, NoteException, PyException);
pyo3::create_exception!(music21_rs_facade, NotRestException, PyException);

pub(crate) fn note_error(error: music21_rs::Error) -> PyErr {
    NoteException::new_err(message(&error))
}

/// The error music21 raises out of the `NotRest` properties — notehead, its
/// fill and parentheses, and stem direction — which is a different class
/// from the one its `Note` methods raise.
fn not_rest_error(error: music21_rs::Error) -> PyErr {
    NotRestException::new_err(message(&error))
}

/// music21's `duration.Duration`, over the crate's quarter-length duration.
#[pyclass(name = "Duration", module = "music21.duration", skip_from_py_object)]
pub struct Duration {
    pub(crate) inner: RsDuration,
    /// The object this duration belongs to. music21's `GeneralNote.duration`
    /// setter writes itself here, and reads the failure to do so as "not a
    /// Duration at all", so keeping the slot is what lets music21's own
    /// classes take one of ours.
    client: Option<Py<PyAny>>,
}

impl Clone for Duration {
    /// A copy of a duration belongs to nobody yet, as music21's does.
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            client: None,
        }
    }
}

impl Duration {
    pub(crate) fn wrap(inner: RsDuration) -> Self {
        Self {
            inner,
            client: None,
        }
    }

    /// The duration music21 builds from a bare `Duration(**keywords)`: a
    /// quarter length if one was given, else a note value by name, else the
    /// quarter every note starts with.
    pub(crate) fn from_keywords(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let Some(keywords) = keywords else {
            return Ok(Self::wrap(RsDuration::quarter()));
        };
        if let Some(value) = keywords.get_item("quarterLength")? {
            return Ok(Self::wrap(
                RsDuration::new(value.extract::<f64>()?).map_err(note_error)?,
            ));
        }
        if let Some(value) = keywords.get_item("type")? {
            let name = value.extract::<String>()?;
            let kind = music21_rs::duration::DurationType::from_music21_name(&name)
                .ok_or_else(|| NoteException::new_err(format!("no such duration type: {name}")))?;
            return Ok(Self::wrap(RsDuration::from_type(kind)));
        }
        Ok(Self::wrap(RsDuration::quarter()))
    }

    /// Whether any of the keywords say something about the duration, which
    /// is how music21 decides between the quarter it gives every note and a
    /// duration built from what it was told.
    pub(crate) fn keywords_say_duration(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<bool> {
        let Some(keywords) = keywords else {
            return Ok(false);
        };
        for name in ["quarterLength", "type", "dots", "duration"] {
            if keywords.contains(name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Reads a duration argument: a `Duration`, a quarter length, or a type name
/// such as `"half"`.
pub(crate) fn duration_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsDuration> {
    if let Ok(facade) = value.extract::<PyRef<Duration>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(name) = value.extract::<String>() {
        return music21_rs::duration::DurationType::from_music21_name(&name)
            .map(RsDuration::from_type)
            .ok_or_else(|| NoteException::new_err(format!("no such duration type: {name}")));
    }
    if let Ok(quarter_length) = value.extract::<f64>() {
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    if let Ok(quarter_length) = value
        .getattr("quarterLength")
        .and_then(|value| value.extract::<f64>())
    {
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    Err(NoteException::new_err(format!(
        "cannot read a duration from {}",
        value.repr()?
    )))
}

#[pymethods]
impl Duration {
    /// music21's `Duration(value, **keywords)`, where `type`, `dots` and
    /// `quarterLength` are the keywords its `GeneralNote` passes through when
    /// a note is built with them.
    #[new]
    #[pyo3(signature = (value = None, **keywords))]
    fn new(
        value: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut duration = match value.filter(|value| !value.is_none()) {
            Some(value) => Self::wrap(duration_from_any(value)?),
            None => Self::from_keywords(keywords)?,
        };
        if let Some(keywords) = keywords
            && let Some(dots) = keywords.get_item("dots")?
        {
            duration.set_dots(dots.extract::<u32>()?)?;
        }
        Ok(duration)
    }

    #[getter]
    fn get_quarterLength(&self) -> f64 {
        self.inner.quarter_length()
    }

    #[setter]
    fn set_quarterLength(&mut self, value: f64) -> PyResult<()> {
        self.inner.set_quarter_length(value).map_err(note_error)
    }

    #[getter]
    fn get_type(&self) -> String {
        self.inner.duration_type().map_or_else(
            || "complex".to_string(),
            |kind| kind.music21_name().to_string(),
        )
    }

    #[setter]
    fn set_type(&mut self, value: &str) -> PyResult<()> {
        let kind = music21_rs::duration::DurationType::from_music21_name(value)
            .ok_or_else(|| NoteException::new_err(format!("no such duration type: {value}")))?;
        let dots = self.inner.dots();
        self.inner = RsDuration::from_type_with_dots(kind, dots);
        Ok(())
    }

    #[getter]
    fn get_dots(&self) -> u32 {
        self.inner.dots()
    }

    #[setter]
    fn set_dots(&mut self, value: u32) -> PyResult<()> {
        let kind = self.inner.duration_type().ok_or_else(|| {
            NoteException::new_err("cannot set dots on a duration with no note value")
        })?;
        self.inner = RsDuration::from_type_with_dots(kind, value);
        Ok(())
    }

    #[getter]
    fn ordinal(&self) -> Option<usize> {
        self.inner.ordinal()
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner
            .full_name()
            .unwrap_or_else(|| "Duration".to_string())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .getattr("quarterLength")
            .ok()
            .and_then(|value| value.extract::<f64>().ok())
            .is_some_and(|value| value == self.inner.quarter_length())
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.duration.Duration {:?}>",
            self.inner.quarter_length()
        )
    }

    #[getter]
    fn get_client(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.client.as_ref().map(|client| client.clone_ref(py))
    }

    #[setter]
    fn set_client(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.client = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// music21's `note.Note`: a pitch with a duration.
#[pyclass(name = "Note", module = "music21.note", subclass, skip_from_py_object)]
pub struct Note {
    pub(crate) inner: RsNote,
    /// The `Pitch` object music21 hands back from `.pitch`. It is the same
    /// object every time, so `chord.pitches[0].getEnharmonic(inPlace=True)`
    /// reaches the chord; `inner` mirrors whatever it holds.
    pitch: Py<Pitch>,
    /// The `Volume` object music21 hands back from `.volume`, once something
    /// has asked for one. music21 makes it on demand and reads its mere
    /// existence as `hasVolumeInformation`.
    volume: Option<Py<Volume>>,
    /// The `Duration` object music21 hands back from `.duration`, once
    /// something has asked for one. Notes a chord builds are given the
    /// chord's, which is what makes `chord.duration is chord[0].duration`
    /// hold; a note that came with its own keeps it.
    duration: Option<Py<Duration>>,
    /// The chord this note is part of: music21's `_chordAttached`, which its
    /// own `ChordBase` sets on every note it takes in. An edit to the pitch
    /// has to reach the chord through it.
    chord: Option<Py<PyAny>>,
}

impl Note {
    /// Builds the facade around a note, giving its pitch a Python object of
    /// its own.
    pub(crate) fn wrap(py: Python<'_>, inner: RsNote) -> PyResult<Self> {
        let pitch = Py::new(py, Pitch::wrap(inner.pitch().clone(), false))?;
        Ok(Self {
            inner,
            pitch,
            volume: None,
            duration: None,
            chord: None,
        })
    }

    /// A Python note object whose pitch already points back at it.
    pub(crate) fn object(py: Python<'_>, inner: RsNote) -> PyResult<Py<Self>> {
        let note = Py::new(py, Self::wrap(py, inner)?)?;
        Self::claim_pitch(py, &note);
        Ok(note)
    }

    /// A note built around a pitch object the caller already holds, so that
    /// `chord.Chord([p1, p2]).pitches[0] is p1`, as music21's is.
    pub(crate) fn object_for_pitch(py: Python<'_>, pitch: Py<Pitch>) -> PyResult<Py<Self>> {
        let inner = RsNote::from_pitch(pitch.borrow(py).inner.clone());
        let note = Py::new(
            py,
            Self {
                inner,
                pitch,
                volume: None,
                duration: None,
                chord: None,
            },
        )?;
        Self::claim_pitch(py, &note);
        Ok(note)
    }

    /// Points the note's pitch object back at the note, so an edit through
    /// the pitch finds its way home.
    fn claim_pitch(py: Python<'_>, note: &Py<Self>) {
        let pitch = note.borrow(py).pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(note.clone_ref(py));
    }

    /// Takes the value a `Pitch` object now holds and passes it on: into the
    /// note, and through the note into its chord. The pitch object itself is
    /// left alone, because it is the one calling and is already borrowed.
    pub(crate) fn adopt_pitch(py: Python<'_>, note: &Py<Self>, pitch: &RsPitch) -> PyResult<()> {
        note.borrow_mut(py).inner.set_pitch(pitch.clone());
        Self::tell_chord(py, note, pitch)
    }

    /// Sends the note's own pitch the other way, out to its pitch object and
    /// to its chord: what a setter on the note ends with.
    pub(crate) fn broadcast_pitch(py: Python<'_>, note: &Py<Self>) -> PyResult<()> {
        let (object, value) = {
            let me = note.borrow(py);
            (me.pitch.clone_ref(py), me.inner.pitch().clone())
        };
        object.borrow_mut(py).inner = value.clone();
        Self::tell_chord(py, note, &value)
    }

    /// Writes a new pitch for this note into the chord holding it, when a
    /// chord of ours is holding it.
    fn tell_chord(py: Python<'_>, note: &Py<Self>, pitch: &RsPitch) -> PyResult<()> {
        let Some(chord) = note
            .borrow(py)
            .chord
            .as_ref()
            .map(|chord| chord.clone_ref(py))
        else {
            return Ok(());
        };
        // music21's own `ChordBase` writes itself here as well; it keeps its
        // own pitches and wants nothing from us.
        let Ok(chord) = chord.cast_bound::<crate::chord::Chord>(py).cloned() else {
            return Ok(());
        };
        crate::chord::Chord::adopt_note_pitch(&chord, note, pitch)
    }

    /// Tells the note which chord holds it, and makes sure its pitch object
    /// knows the note.
    pub(crate) fn attach_to_chord(py: Python<'_>, note: &Py<Self>, chord: &Bound<'_, PyAny>) {
        note.borrow_mut(py).chord = Some(chord.clone().unbind());
        Self::claim_pitch(py, note);
    }

    /// The duration this note actually has: the object music21 hands out
    /// when something has asked for one, since an edit through that object
    /// is an edit to the note, and `inner`'s otherwise.
    pub(crate) fn duration_value(&self, py: Python<'_>) -> Option<RsDuration> {
        match &self.duration {
            Some(object) => Some(object.borrow(py).inner.clone()),
            None => self.inner.duration().cloned(),
        }
    }

    /// The `Duration` object for this note, made on first asking as music21
    /// makes one on first asking.
    pub(crate) fn duration_object(&mut self, py: Python<'_>) -> PyResult<Py<Duration>> {
        if let Some(object) = &self.duration {
            return Ok(object.clone_ref(py));
        }
        let inner = self
            .inner
            .duration()
            .cloned()
            .unwrap_or_else(RsDuration::quarter);
        let created = Py::new(py, Duration::wrap(inner))?;
        self.duration = Some(created.clone_ref(py));
        Ok(created)
    }

    /// Hands this note a `Duration` object to share, the way a chord shares
    /// its own with the notes it builds.
    pub(crate) fn share_duration(&mut self, py: Python<'_>, duration: &Py<Duration>) {
        self.inner.set_duration(duration.borrow(py).inner.clone());
        self.duration = Some(duration.clone_ref(py));
    }

    fn quarter_length(&self, py: Python<'_>) -> f64 {
        self.duration_value(py)
            .as_ref()
            .map_or(1.0, RsDuration::quarter_length)
    }

    /// This note with whatever its duration and volume objects now say
    /// written into it, for the answers the crate reads off a whole note.
    pub(crate) fn synced(&self, py: Python<'_>) -> RsNote {
        let mut note = self.inner.clone();
        if let Some(duration) = self.duration_value(py) {
            note.set_duration(duration);
        }
        if let Some(volume) = &self.volume {
            note.set_volume(Some(volume.borrow(py).inner.clone()));
        }
        note
    }

    /// music21 orders notes by pitch alone, and refuses anything without a
    /// `.pitch` — its `__lt__` answers `NotImplemented` and Python raises.
    /// The message is written out here rather than left to Python because
    /// pyo3 puts the module into the type name, so CPython's own wording
    /// would say `music21.note.Note` where music21 says `Note`.
    fn ordered(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        operator: &str,
        compare: impl Fn(f64, f64) -> bool,
    ) -> PyResult<bool> {
        let Ok(pitch) = other
            .getattr("pitch")
            .and_then(|pitch| pitch_from_any(&pitch))
        else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "'{operator}' not supported between instances of '{}' and '{}'",
                slf.get_type().name()?,
                other.get_type().name()?,
            )));
        };
        Ok(compare(slf.borrow().inner.pitch().ps(), pitch.ps()))
    }

    /// A detached copy: new pitch and duration objects, and no chord.
    fn copied(&self, py: Python<'_>) -> PyResult<Self> {
        Self::wrap(py, self.synced(py))
    }
}

/// Reads a note argument: a `Note`, a pitch, or a name. A facade note comes
/// back with its duration object's value written in, since that object is
/// where an edit like `n.duration.type = 'half'` landed.
pub(crate) fn note_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsNote> {
    if let Ok(facade) = value.extract::<PyRef<Note>>() {
        return Ok(facade.synced(value.py()));
    }
    Ok(RsNote::from_pitch(pitch_from_any(value)?))
}

#[pymethods]
impl Note {
    #[new]
    #[pyo3(signature = (pitch = None, **keywords))]
    fn new(
        py: Python<'_>,
        pitch: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let inner = match pitch.filter(|value| !value.is_none()) {
            Some(value) => RsNote::from_pitch(pitch_from_any(value)?),
            None => match crate::pitch::pitch_from_keywords(py, keywords)? {
                Some(pitch) => RsNote::from_pitch(pitch),
                None => RsNote::from_name("C4").map_err(note_error)?,
            },
        };
        let mut note = Self::wrap(py, inner)?;
        // music21 hands the same keywords on to `Duration`, so `type='eighth',
        // dots=2` is an eighth with two dots and not a quarter.
        if Duration::keywords_say_duration(keywords)? {
            let duration = match keywords.and_then(|keywords| keywords.get_item("duration").ok()?) {
                Some(value) => value,
                None => Py::new(py, Duration::new(None, keywords)?)?.into_bound(py).into_any(),
            };
            note.set_duration(py, &duration)?;
        }
        Ok(note)
    }

    /// music21's `.pitch`, the same object every time: an edit through it
    /// is an edit to the note, and to the chord holding the note.
    #[getter]
    pub(crate) fn get_pitch(slf: &Bound<'_, Self>) -> Py<Pitch> {
        let py = slf.py();
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(slf.clone().unbind());
        pitch
    }

    #[setter]
    fn set_pitch(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let pitch = pitch_from_any(value)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    #[getter]
    fn get_name(&self) -> String {
        self.inner.pitch_name()
    }

    #[setter]
    fn set_name(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        let name = {
            let me = slf.borrow();
            match me.inner.pitch().octave() {
                Some(octave) if !value.chars().any(|ch| ch.is_ascii_digit()) => {
                    format!("{value}{octave}")
                }
                _ => value.to_string(),
            }
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    #[getter]
    fn nameWithOctave(&self) -> String {
        self.inner.pitch_name_with_octave()
    }

    #[getter]
    fn step(&self) -> String {
        self.inner.step().to_string()
    }

    #[getter]
    fn get_octave(&self) -> Option<i32> {
        self.inner.octave()
    }

    #[setter]
    fn set_octave(slf: &Bound<'_, Self>, value: Option<i32>) -> PyResult<()> {
        let name = {
            let me = slf.borrow();
            match value {
                Some(octave) => format!("{}{octave}", me.inner.pitch().name()),
                None => me.inner.pitch().name(),
            }
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    /// music21's `.pitches`, the chord-shaped view of a note: its one pitch
    /// in a tuple.
    #[getter]
    fn get_pitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(slf.py(), [Self::get_pitch(slf)])
    }

    /// Setting it takes the first pitch of a list or tuple and ignores the
    /// rest, since a note has only one; anything that is not a sequence is
    /// refused.
    #[setter]
    fn set_pitches(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let refused = || {
            NoteException::new_err(format!(
                "cannot set pitches with provided object: {}",
                value.str().map_or_else(|_| "?".to_string(), |v| v.to_string())
            ))
        };
        if !value.is_instance_of::<PyList>() && !value.is_instance_of::<PyTuple>() {
            return Err(refused());
        }
        let Some(first) = value.try_iter()?.next() else {
            return Err(refused());
        };
        Self::set_pitch(slf, &first?)
    }

    #[getter]
    fn fullName(&self, py: Python<'_>) -> String {
        self.synced(py).full_name()
    }

    /// music21's `.duration`, the same object every time: `n.duration.type =
    /// 'half'` is how music21's own doctests lengthen a note.
    #[getter]
    fn get_duration(&mut self, py: Python<'_>) -> PyResult<Py<Duration>> {
        self.duration_object(py)
    }

    #[setter]
    fn set_duration(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let inner = duration_from_any(value)?;
        self.inner.set_duration(inner.clone());
        self.duration = match value.extract::<Py<Duration>>() {
            Ok(object) => Some(object),
            Err(_) => Some(Py::new(py, Duration::wrap(inner))?),
        };
        Ok(())
    }

    #[getter]
    fn get_quarterLength(&self, py: Python<'_>) -> f64 {
        self.quarter_length(py)
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        let inner = RsDuration::new(value).map_err(note_error)?;
        self.inner.set_duration(inner.clone());
        match &self.duration {
            Some(duration) => duration.borrow_mut(py).inner = inner,
            None => self.duration = Some(Py::new(py, Duration::wrap(inner))?),
        }
        Ok(())
    }

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isNote(&self) -> bool {
        true
    }

    #[getter]
    fn isChord(&self) -> bool {
        false
    }

    // ---- notation --------------------------------------------------------

    #[getter]
    fn get_tie(&self) -> Option<Tie> {
        self.inner.tie().cloned().map(Tie::wrap)
    }

    #[setter]
    pub(crate) fn set_tie(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let tie = match value.filter(|value| !value.is_none()) {
            Some(value) => Some(tie_from_any(value)?),
            None => None,
        };
        self.inner.set_tie(tie);
        Ok(())
    }

    #[getter]
    fn get_notehead(&self) -> &'static str {
        self.inner.notehead().as_str()
    }

    #[setter]
    pub(crate) fn set_notehead(&mut self, value: Option<&str>) -> PyResult<()> {
        let notehead = match value {
            None | Some("") => RsNotehead::Normal,
            Some(name) => RsNotehead::from_name(name).map_err(not_rest_error)?,
        };
        self.inner.set_notehead(notehead);
        Ok(())
    }

    #[getter]
    fn get_noteheadFill(&self) -> Option<bool> {
        self.inner.notehead_fill()
    }

    #[setter]
    pub(crate) fn set_noteheadFill(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let fill = if value.is_none() {
            None
        } else if let Ok(flag) = value.extract::<bool>() {
            Some(flag)
        } else {
            match value.extract::<String>()?.as_str() {
                "none" | "default" => None,
                "filled" | "yes" => Some(true),
                "notfilled" | "no" => Some(false),
                other => {
                    return Err(not_rest_error(music21_rs::Error::Notation(format!(
                        "not a valid notehead fill value: '{other}'"
                    ))));
                }
            }
        };
        self.inner.set_notehead_fill(fill);
        Ok(())
    }

    #[getter]
    fn get_noteheadParenthesis(&self) -> bool {
        self.inner.notehead_parenthesis()
    }

    #[setter]
    fn set_noteheadParenthesis(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let parenthesis = if let Ok(flag) = value.extract::<bool>() {
            flag
        } else if let Ok(number) = value.extract::<i64>() {
            number != 0
        } else {
            match value.extract::<String>()?.as_str() {
                "yes" => true,
                "no" => false,
                other => {
                    return Err(not_rest_error(music21_rs::Error::Notation(format!(
                        "notehead parentheses must be True or False, not '{other}'"
                    ))));
                }
            }
        };
        self.inner.set_notehead_parenthesis(parenthesis);
        Ok(())
    }

    #[getter]
    fn get_stemDirection(&self) -> &'static str {
        self.inner.stem_direction().as_str()
    }

    #[setter]
    pub(crate) fn set_stemDirection(&mut self, value: Option<&str>) -> PyResult<()> {
        let direction = match value {
            None => RsStemDirection::Unspecified,
            Some(name) => RsStemDirection::from_name(name).map_err(not_rest_error)?,
        };
        self.inner.set_stem_direction(direction);
        Ok(())
    }

    #[getter]
    fn style(slf: &Bound<'_, Self>) -> Style {
        Style {
            owner: StyleOwner::Note(slf.clone().unbind()),
        }
    }

    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.inner.color().is_some()
    }

    /// music21's `.volume`, made on first asking and the same object after
    /// that, so `n.volume.velocity = 20` sticks.
    #[getter]
    fn get_volume(&mut self, py: Python<'_>) -> PyResult<Py<Volume>> {
        if let Some(volume) = &self.volume {
            return Ok(volume.clone_ref(py));
        }
        let created = Py::new(py, Volume::wrap(self.inner.volume()))?;
        self.volume = Some(created.clone_ref(py));
        Ok(created)
    }

    #[setter]
    fn set_volume(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.inner.set_volume(None);
            self.volume = None;
            return Ok(());
        };
        let inner = volume_from_any(value)?;
        self.inner.set_volume(Some(inner.clone()));
        self.volume = match value.extract::<Py<Volume>>() {
            Ok(object) => Some(object),
            Err(_) => Some(Py::new(py, Volume::wrap(inner))?),
        };
        Ok(())
    }

    /// Whether this note carries a volume at all. music21 asks only whether
    /// the object is there, which is why reading `.volume` once makes this
    /// true.
    fn hasVolumeInformation(&self) -> bool {
        self.volume.is_some()
    }

    #[getter]
    pub(crate) fn lyrics(&self) -> Vec<Lyric> {
        self.inner
            .lyrics()
            .iter()
            .cloned()
            .map(Lyric::wrap)
            .collect()
    }

    #[getter]
    fn get_lyric(&self) -> Option<String> {
        self.inner.lyric()
    }

    #[setter]
    pub(crate) fn set_lyric(&mut self, value: Option<&str>) -> PyResult<()> {
        self.inner.set_lyric(value).map_err(note_error)
    }

    #[pyo3(signature = (text, lyricNumber = None, *, applyRaw = false, lyricIdentifier = None))]
    pub(crate) fn addLyric(
        &mut self,
        text: &Bound<'_, PyAny>,
        lyricNumber: Option<i32>,
        applyRaw: bool,
        lyricIdentifier: Option<String>,
    ) -> PyResult<()> {
        let text = if text.is_none() {
            String::new()
        } else {
            text.str()?.to_string()
        };
        self.inner.add_lyric(&text, applyRaw).map_err(note_error)?;
        if let Some(lyric) = self.inner.lyrics_mut().last_mut() {
            if let Some(number) = lyricNumber {
                lyric.set_number(number).map_err(note_error)?;
            }
            if lyricIdentifier.is_some() {
                lyric.set_identifier(lyricIdentifier);
            }
        }
        Ok(())
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<Self>>> {
        let py = slf.py();
        let pitch = transpose_pitch_by_any(slf.borrow().inner.pitch(), value)?;
        if inPlace {
            slf.borrow_mut().inner.set_pitch(pitch);
            Self::broadcast_pitch(py, &slf.clone().unbind())?;
            return Ok(None);
        }
        let mut moved = RsNote::from_pitch(pitch);
        if let Some(duration) = slf.borrow().inner.duration().cloned() {
            moved.set_duration(duration);
        }
        Ok(Some(Self::object(py, moved)?))
    }

    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<Note>>().is_ok_and(|other| {
            other.inner.pitch() == self.inner.pitch()
                && other.quarter_length(py) == self.quarter_length(py)
        })
    }

    fn __lt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, "<", |left, right| left < right)
    }

    fn __le__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, "<=", |left, right| left <= right)
    }

    fn __gt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, ">", |left, right| left > right)
    }

    fn __ge__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, ">=", |left, right| left >= right)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.note.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().inner.pitch_name()
        ))
    }

    fn __deepcopy__(&self, py: Python<'_>, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.copied(py)
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Self> {
        self.copied(py)
    }

    /// music21's `_chordAttached`, which its own `ChordBase` sets on every
    /// note it takes in. Keeping the slot is what lets music21's chord
    /// classes hold facade notes at all.
    #[getter(_chordAttached)]
    fn get_chordAttached(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.chord.as_ref().map(|chord| chord.clone_ref(py))
    }

    #[setter(_chordAttached)]
    fn set_chordAttached(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.chord = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Note>()?;
    m.add_class::<Duration>()?;
    let exception = py.get_type::<NoteException>();
    exception.setattr("__module__", "music21.note")?;
    m.add("NoteException", exception)?;
    let not_rest = py.get_type::<NotRestException>();
    not_rest.setattr("__module__", "music21.note")?;
    m.add("NotRestException", not_rest)?;
    Ok(())
}
