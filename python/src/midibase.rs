//! music21's `midi.base`: MIDI's events, tracks and files as objects, over
//! the crate's `midi::base`.
//!
//! The crate reads and writes the bytes; these classes hold what an event,
//! a track and a file are as music21 holds them, each attribute any Python
//! object, as music21's are. The four kinds of message are `HexEnum`s
//! built at module load.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyBytes, PyInt, PyList, PyModule, PyString, PyTuple};

use music21::midi::base::{
    self as raw, CHANNEL_MODE_MESSAGES, CHANNEL_VOICE_MESSAGES, EventType, META_EVENTS,
    METAEVENT_MARKER, MidiDataError, Param, ReadEvent, SYSEX_EVENTS,
};

use music21::midi::translate::Laid;

use crate::plumbing::built::new_class;
use crate::prebase::ProtoM21Object;

/// The module music21 keeps these in.
const MODULE: &str = "music21.midi.base";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(music21_rs_facade, MidiException, crate::Music21Exception);

/// An enum's name, its members and its docstring.
type EnumTable = (&'static str, &'static [(&'static str, u8)], &'static str);

/// The four enums, in the order an event's type is tried against them.
const ENUMS: [EnumTable; 4] = [
    (
        "ChannelVoiceMessages",
        CHANNEL_VOICE_MESSAGES,
        "The main messages of a channel, such as NOTE_ON and NOTE_OFF.",
    ),
    (
        "ChannelModeMessages",
        CHANNEL_MODE_MESSAGES,
        "The controller changes that set a channel's mode.",
    ),
    (
        "MetaEvents",
        META_EVENTS,
        "The meta events, UNKNOWN for any other.",
    ),
    (
        "SysExEvents",
        SYSEX_EVENTS,
        "The two system exclusive events.",
    ),
];

/// One of the module's enums by name.
fn enum_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    module(py)?.getattr(name)
}

/// The enum member for a crate event type.
fn member<'py>(py: Python<'py>, kind: EventType) -> PyResult<Bound<'py, PyAny>> {
    let (name, value) = match kind {
        EventType::ChannelVoice(value) => (ENUMS[0].0, value),
        EventType::ChannelMode(value) => (ENUMS[1].0, value),
        EventType::Meta(value) => (ENUMS[2].0, value),
        EventType::SysEx(value) => (ENUMS[3].0, value),
    };
    enum_class(py, name)?.call1((value,))
}

/// The crate's event type for a member of one of the enums; nothing for
/// any other type, such as `'DeltaTime'`.
pub(crate) fn event_type(value: &Bound<'_, PyAny>) -> PyResult<Option<EventType>> {
    let py = value.py();
    let makers: [fn(u8) -> EventType; 4] = [
        EventType::ChannelVoice,
        EventType::ChannelMode,
        EventType::Meta,
        EventType::SysEx,
    ];
    for ((name, _, _), make) in ENUMS.iter().zip(makers) {
        if value.is_instance(&enum_class(py, name)?)? {
            return Ok(Some(make(value.getattr("value")?.extract()?)));
        }
    }
    Ok(None)
}

/// A parameter as the crate holds one, or nothing for `None`; a value of
/// no type MIDI writes is refused.
fn param(value: &Bound<'_, PyAny>) -> PyResult<Option<Param>> {
    if value.is_none() {
        return Ok(None);
    }
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(Some(Param::Bytes(bytes.as_bytes().to_vec())));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(Some(Param::Text(text.to_string())));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(Some(Param::Int(value.extract().unwrap_or(i64::MAX))));
    }
    Err(PyTypeError::new_err(format!(
        "can't concat {} to bytes",
        crate::plumbing::built::class_name(value)?
    )))
}

/// The Python object for a parameter the crate read.
fn param_object<'py>(py: Python<'py>, value: &Param) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Param::Int(number) => Ok(number.into_pyobject(py)?.into_any()),
        Param::Bytes(bytes) => Ok(PyBytes::new(py, bytes).into_any()),
        Param::Text(text) => Ok(PyString::new(py, text).into_any()),
    }
}

/// The exception music21 raises for a crate error.
fn raised(py: Python<'_>, error: MidiDataError) -> PyErr {
    match error {
        MidiDataError::OutOfRange => PyIndexError::new_err(error.to_string()),
        MidiDataError::NotAByte | MidiDataError::NotChannelVoice(_) => {
            PyValueError::new_err(error.to_string())
        }
        // music21 names the builtin `format` here, not the file's format.
        MidiDataError::UnreadFormat(_) => {
            MidiException::new_err("cannot handle midi file format: <built-in function format>")
        }
        MidiDataError::NotAFile(start) => {
            let shown = PyBytes::new(py, &start)
                .repr()
                .map(|shown| shown.to_string())
                .unwrap_or_default();
            MidiException::new_err(format!("badly formatted midi bytes, got: {shown}"))
        }
        _ => MidiException::new_err(error.to_string()),
    }
}

/// A Python int.
fn int(py: Python<'_>, value: i64) -> Py<PyAny> {
    PyInt::new(py, value).into_any().unbind()
}

/// `bytes` as Python bytes.
fn py_bytes<'py>(py: Python<'py>, bytes: &[u8]) -> Bound<'py, PyAny> {
    PyBytes::new(py, bytes).into_any()
}

/// Bytes given as Python bytes, or a refusal naming what they were.
fn bytes_of(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    Ok(value.cast::<PyBytes>()?.as_bytes().to_vec())
}

/// music21's `getNumber`: the first `length` bytes as one big-endian
/// number and what follows, or a number's low bytes and the rest of it.
#[pyfunction]
fn getNumber<'py>(
    py: Python<'py>,
    midiBytes: &Bound<'py, PyAny>,
    length: u32,
) -> PyResult<Bound<'py, PyTuple>> {
    if let Ok(bytes) = midiBytes.cast::<PyBytes>() {
        let (number, rest) =
            raw::get_number(bytes.as_bytes(), length as usize).map_err(|e| raised(py, e))?;
        return PyTuple::new(
            py,
            [number.into_pyobject(py)?.into_any(), py_bytes(py, rest)],
        );
    }
    let (low, high) = raw::split_number(midiBytes.extract()?, length);
    PyTuple::new(py, [low, high])
}

/// music21's `getVariableLengthNumber`: a number written seven bits to a
/// byte, and the bytes after it.
#[pyfunction]
fn getVariableLengthNumber<'py>(
    py: Python<'py>,
    midiBytes: &Bound<'py, PyBytes>,
) -> PyResult<Bound<'py, PyTuple>> {
    let (number, rest) =
        raw::get_variable_length_number(midiBytes.as_bytes()).map_err(|e| raised(py, e))?;
    PyTuple::new(
        py,
        [number.into_pyobject(py)?.into_any(), py_bytes(py, rest)],
    )
}

/// music21's `putNumber`: a number as `length` big-endian bytes.
#[pyfunction]
fn putNumber(py: Python<'_>, num: i128, length: usize) -> Bound<'_, PyAny> {
    py_bytes(py, &raw::put_number(num, length))
}

/// music21's `putVariableLengthNumber`: a number in the fewest bytes MIDI
/// writes it in.
#[pyfunction]
fn putVariableLengthNumber(py: Python<'_>, x: i128) -> PyResult<Bound<'_, PyAny>> {
    let written = raw::put_variable_length_number(x).map_err(|e| raised(py, e))?;
    Ok(py_bytes(py, &written))
}

/// music21's `putNumbersAsList`: numbers as a byte each, a negative one
/// counted down from 256.
#[pyfunction]
fn putNumbersAsList<'py>(
    py: Python<'py>,
    numList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut numbers = Vec::new();
    for number in numList.try_iter()? {
        numbers.push(number?.extract::<i128>()?);
    }
    let written = raw::put_numbers_as_list(&numbers).map_err(|e| raised(py, e))?;
    Ok(py_bytes(py, &written))
}

/// music21's `MidiEvent`: one event of a track, its type one of the four
/// enums, its channel counted from one, its parameters a pitch and a
/// velocity, or data.
#[pyclass(name = "MidiEvent", module = "music21.midi.base", extends = ProtoM21Object, subclass, dict)]
pub struct MidiEvent {
    /// The track it belongs to.
    #[pyo3(get, set)]
    track: Py<PyAny>,
    /// What kind of event it is.
    #[pyo3(get, set, name = "type")]
    kind: Py<PyAny>,
    /// Its time in ticks; the delta times place it.
    #[pyo3(get, set)]
    time: Py<PyAny>,
    /// Counted from one.
    #[pyo3(get, set)]
    channel: Py<PyAny>,
    /// A pitch, or the first data value.
    #[pyo3(get, set)]
    parameter1: Py<PyAny>,
    /// A velocity, or the second data value.
    #[pyo3(get, set)]
    parameter2: Py<PyAny>,
    /// The microtone a note's pitch carried, where it had one.
    #[pyo3(get, set)]
    centShift: Py<PyAny>,
    /// A note-on's note-off, or a note-off's note-on.
    #[pyo3(get, set)]
    correspondingEvent: Py<PyAny>,
    /// The running status, carried from the event before.
    #[pyo3(get, set)]
    lastStatusByte: Py<PyAny>,
}

impl MidiEvent {
    fn made(
        py: Python<'_>,
        track: Option<Py<PyAny>>,
        kind: Py<PyAny>,
        time: Py<PyAny>,
        channel: Py<PyAny>,
    ) -> Self {
        Self {
            track: track.unwrap_or_else(|| py.None()),
            kind,
            time,
            channel,
            parameter1: py.None(),
            parameter2: py.None(),
            centShift: py.None(),
            correspondingEvent: py.None(),
            lastStatusByte: py.None(),
        }
    }

    /// Its type, as the crate holds one, where it is one of the enums'.
    fn crate_type(&self, py: Python<'_>) -> PyResult<Option<EventType>> {
        event_type(self.kind.bind(py))
    }

    /// Its velocity, where it is a number.
    fn velocity_number(&self, py: Python<'_>) -> Option<i64> {
        let value = self.parameter2.bind(py);
        if !value.is_instance_of::<PyInt>() {
            return None;
        }
        value.extract().ok()
    }

    /// What reading set, put on it.
    fn apply(&mut self, py: Python<'_>, read: &ReadEvent) -> PyResult<()> {
        if let Some(channel) = read.channel {
            self.channel = channel.into_pyobject(py)?.into_any().unbind();
        }
        self.kind = member(py, read.event_type)?.unbind();
        self.parameter1 = param_object(py, &read.parameter1)?.unbind();
        if let Some(second) = read.parameter2 {
            self.parameter2 = second.into_pyobject(py)?.into_any().unbind();
        }
        Ok(())
    }

    /// Sets the type and channel a channel message's status byte names, as
    /// music21 has done before it refuses what follows.
    fn take_status(&mut self, py: Python<'_>, status: u8) -> PyResult<()> {
        self.channel = ((status & 0x0F) + 1).into_pyobject(py)?.into_any().unbind();
        self.kind = member(py, EventType::ChannelVoice(status & 0xF0))?.unbind();
        Ok(())
    }

    fn is_note(&self, py: Python<'_>) -> PyResult<bool> {
        Ok(matches!(
            self.crate_type(py)?,
            Some(EventType::ChannelVoice(0x80 | 0x90))
        ))
    }
}

#[pymethods]
impl MidiEvent {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.track)?;
        visit.call(&self.kind)?;
        visit.call(&self.parameter1)?;
        visit.call(&self.parameter2)?;
        visit.call(&self.correspondingEvent)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (track = None, r#type = None, time = None, channel = None))]
    fn new(
        py: Python<'_>,
        track: Option<Py<PyAny>>,
        r#type: Option<Py<PyAny>>,
        time: Option<Py<PyAny>>,
        channel: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let kind = match r#type {
            Some(kind) => kind,
            None => enum_class(py, "MetaEvents")?.getattr("UNKNOWN")?.unbind(),
        };
        let time = time.unwrap_or_else(|| int(py, 0));
        let channel = channel.unwrap_or_else(|| int(py, 1));
        Ok(crate::base::built(Self::made(
            py, track, kind, time, channel,
        )))
    }

    /// Where it goes among events at one time: a note-off, then a pitch
    /// bend, then the rest.
    #[getter]
    fn sortOrder(&self, py: Python<'_>) -> PyResult<i64> {
        Ok(raw::sort_order(self.crate_type(py)?))
    }

    /// The type, time, track, channel and what it holds.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let this = slf.borrow();
        let track = this.track.bind(py);
        let track_index = if track.is_none() {
            "None".to_string()
        } else if track.is_instance_of::<PyInt>() {
            track.str()?.to_string()
        } else {
            track.getattr("index")?.str()?.to_string()
        };
        let kind = this.kind.bind(py);
        let hex_enum = crate::common::module(py, "music21.common.enums")?.getattr("HexEnum")?;
        let shown_type = if kind.is_instance(&hex_enum)? {
            kind.getattr("name")?.str()?.to_string()
        } else if kind.is_none() {
            "None".to_string()
        } else {
            kind.repr()?.to_string()
        };
        let mut out = format!("{shown_type}, ");
        let time = this.time.bind(py);
        if !time.eq(0)? {
            out.push_str(&format!("t={}, ", time.repr()?));
        }
        out.push_str(&format!("track={track_index}"));
        let is_note = this.is_note(py)?;
        let second_none = this.parameter2.bind(py).is_none();
        let channel = this.channel.clone_ref(py);
        drop(this);
        if slf.call_method0("isChannelEvent")?.is_truthy()? {
            out.push_str(&format!(", channel={}", channel.bind(py).repr()?));
        }
        let names: &[&str] = if is_note {
            &["pitch", "velocity"]
        } else if second_none {
            &["data"]
        } else {
            &["parameter1", "parameter2"]
        };
        for name in names {
            let value = slf.getattr(*name)?;
            if !value.is_none() {
                out.push_str(&format!(", {name}={}", value.repr()?));
            }
        }
        Ok(out)
    }

    /// A note's pitch: the first parameter, of a note-on or note-off only.
    #[getter]
    fn get_pitch<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let first = self.parameter1.bind(py);
        if self.is_note(py)? && !first.is_none() {
            return py.get_type::<PyInt>().call1((first,));
        }
        Ok(py.None().into_bound(py))
    }

    #[setter]
    fn set_pitch(&mut self, value: Py<PyAny>) {
        self.parameter1 = value;
    }

    /// The second parameter, where it is a number.
    #[getter]
    fn get_velocity<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        let second = self.parameter2.bind(py);
        if !second.is_instance_of::<PyInt>() {
            return py.None().into_bound(py);
        }
        second.clone()
    }

    #[setter]
    fn set_velocity(&mut self, value: Py<PyAny>) {
        self.parameter2 = value;
    }

    /// The first parameter; text set is kept as its UTF-8 bytes, and a
    /// boolean as one byte.
    #[getter]
    fn get_data(&self, py: Python<'_>) -> Py<PyAny> {
        self.parameter1.clone_ref(py)
    }

    #[setter]
    fn set_data(&mut self, py: Python<'_>, value: Bound<'_, PyAny>) {
        self.parameter1 = if let Ok(text) = value.cast::<PyString>() {
            py_bytes(py, text.to_string().as_bytes()).unbind()
        } else if let Ok(flag) = value.cast::<PyBool>() {
            py_bytes(py, &[u8::from(flag.is_true())]).unbind()
        } else {
            value.unbind()
        };
    }

    /// Whether the channel matters to it: a channel voice or mode message.
    fn isChannelEvent(&self, py: Python<'_>) -> PyResult<bool> {
        Ok(matches!(
            self.crate_type(py)?,
            Some(EventType::ChannelVoice(_) | EventType::ChannelMode(_))
        ))
    }

    /// Makes it a pitch bend of so many cents, within `bendRange`
    /// semitones each way.
    #[pyo3(signature = (cents, bendRange = 2.0))]
    fn setPitchBend(&mut self, py: Python<'_>, cents: f64, bendRange: f64) -> PyResult<()> {
        let (least, most) = raw::pitch_bend(cents, bendRange);
        self.parameter1 = least.into_pyobject(py)?.into_any().unbind();
        self.parameter2 = most.into_pyobject(py)?.into_any().unbind();
        Ok(())
    }

    /// Reads a channel message off the front of the bytes into this event,
    /// and gives back the rest.
    fn parseChannelVoiceMessage<'py>(
        &mut self,
        py: Python<'py>,
        midiBytes: &Bound<'py, PyBytes>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let bytes = midiBytes.as_bytes();
        if bytes.len() < 2 {
            return Err(PyValueError::new_err(format!(
                "length of {} must be at least 2",
                midiBytes.repr()?
            )));
        }
        match raw::parse_channel_voice(bytes) {
            Ok((read, used)) => {
                self.apply(py, &read)?;
                Ok(py_bytes(py, &bytes[used..]))
            }
            Err(error @ MidiDataError::DataTooLarge { .. }) => {
                self.take_status(py, bytes[0])?;
                Err(raised(py, error))
            }
            Err(error) => Err(raised(py, error)),
        }
    }

    /// Reads an event off the front of the bytes into this one, under the
    /// running status it carries, and gives back the rest.
    fn read<'py>(
        &mut self,
        py: Python<'py>,
        midiBytes: &Bound<'py, PyBytes>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let bytes = midiBytes.as_bytes();
        if bytes.len() < 2 {
            return Ok(py_bytes(py, b""));
        }
        let last: Option<u8> = self.lastStatusByte.bind(py).extract()?;
        if let Some(status) = raw::status_after(bytes[0], last).filter(|_| bytes[0] >= 0x80) {
            self.lastStatusByte = status.into_pyobject(py)?.into_any().unbind();
        }
        match raw::read_event(bytes, last) {
            Ok(Some((read, used))) => {
                self.apply(py, &read)?;
                Ok(py_bytes(py, &bytes[used..]))
            }
            Ok(None) => Ok(py_bytes(py, b"")),
            Err(error @ MidiDataError::DataTooLarge { .. }) => {
                self.take_status(py, bytes[0])?;
                Err(raised(py, error))
            }
            Err(error) => Err(raised(py, error)),
        }
    }

    /// The event as MIDI writes it.
    fn getBytes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let this = slf.borrow();
        let kind = this.kind.bind(py);
        let shown = kind.repr()?.to_string();
        let unknown = || MidiException::new_err(format!("unknown midi event type: {shown}"));
        let Some(event_type) = this.crate_type(py)? else {
            return Err(unknown());
        };
        let channel: i64 = this.channel.bind(py).extract()?;
        let first = param(this.parameter1.bind(py))?;
        let second = param(this.parameter2.bind(py))?;
        match raw::event_bytes(event_type, channel, first.as_ref(), second.as_ref()) {
            Ok(written) => Ok(py_bytes(py, &written)),
            Err(MidiDataError::IncorrectData) => {
                let data = this.parameter1.clone_ref(py);
                drop(this);
                Err(MidiException::new_err(format!(
                    "Got incorrect data for {} in .data: {}, cannot parse Miscellaneous Message",
                    slf.repr()?,
                    data.bind(py).repr()?
                )))
            }
            Err(MidiDataError::UnwrittenType) => Err(unknown()),
            Err(error) => Err(raised(py, error)),
        }
    }

    /// Whether it sounds a note: a note-on of some velocity.
    fn isNoteOn(&self, py: Python<'_>) -> PyResult<bool> {
        let Some(kind) = self.crate_type(py)? else {
            return Ok(false);
        };
        Ok(raw::is_note_on(kind, self.velocity_number(py)))
    }

    /// Whether it ends a note: a note-off, or a note-on of no velocity.
    fn isNoteOff(&self, py: Python<'_>) -> PyResult<bool> {
        let Some(kind) = self.crate_type(py)? else {
            return Ok(false);
        };
        Ok(raw::is_note_off(kind, self.velocity_number(py)))
    }

    /// Whether it is a delta time.
    fn isDeltaTime(&self, py: Python<'_>) -> PyResult<bool> {
        self.kind.bind(py).eq("DeltaTime")
    }

    /// Whether `other` ends the note this starts: a note-off of its pitch
    /// on its channel.
    fn matchedNoteOff(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !slf.call_method0("isNoteOn")?.is_truthy()?
            || !other.call_method0("isNoteOff")?.is_truthy()?
        {
            return Ok(false);
        }
        Ok(slf.getattr("pitch")?.eq(other.getattr("pitch")?)?
            && slf.getattr("channel")?.eq(other.getattr("channel")?)?)
    }
}

/// music21's `DeltaTime`: the ticks since the event before, which a
/// track's events are paired with.
#[pyclass(name = "DeltaTime", module = "music21.midi.base", extends = MidiEvent, subclass)]
pub struct DeltaTime;

#[pymethods]
impl DeltaTime {
    #[new]
    #[pyo3(signature = (track = None, time = None, channel = None))]
    fn new(
        py: Python<'_>,
        track: Option<Py<PyAny>>,
        time: Option<Py<PyAny>>,
        channel: Option<Py<PyAny>>,
    ) -> PyClassInitializer<Self> {
        let kind = PyString::new(py, "DeltaTime").into_any().unbind();
        let time = time.unwrap_or_else(|| int(py, 0));
        let channel = channel.unwrap_or_else(|| int(py, 1));
        crate::base::built(MidiEvent::made(py, track, kind, time, channel)).add_subclass(Self)
    }

    /// Its time and track, `(empty)` where no time passes.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let shown = MidiEvent::_reprInternal(slf.as_super())?.replace("'DeltaTime', ", "");
        if slf.as_super().borrow().time.bind(slf.py()).eq(0)? {
            return Ok(format!("(empty) {shown}"));
        }
        Ok(shown)
    }

    /// Never: no channel is written for it.
    fn isChannelEvent(&self) -> bool {
        false
    }

    /// Reads its time off the front of the bytes, and gives it back with
    /// the rest.
    fn readUntilLowByte<'py>(
        slf: &Bound<'py, Self>,
        oldBytes: &Bound<'py, PyBytes>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let (time, rest) =
            raw::get_variable_length_number(oldBytes.as_bytes()).map_err(|e| raised(py, e))?;
        let time = time.into_pyobject(py)?.into_any();
        slf.as_super().borrow_mut().time = time.clone().unbind();
        PyTuple::new(py, [time, py_bytes(py, rest)])
    }

    /// Its time as MIDI writes it.
    fn getBytes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let time: i128 = slf.as_super().borrow().time.bind(py).extract()?;
        let written = raw::put_variable_length_number(time).map_err(|e| raised(py, e))?;
        Ok(py_bytes(py, &written))
    }
}

/// music21's `MidiTrack`: a track's index, its data, and its events, delta
/// times and events in turn.
#[pyclass(name = "MidiTrack", module = "music21.midi.base", extends = ProtoM21Object, subclass, dict)]
pub struct MidiTrack {
    /// Which track of its file it is.
    #[pyo3(get, set)]
    index: Py<PyAny>,
    /// Its events, a delta time before each.
    #[pyo3(get, set)]
    events: Py<PyAny>,
    /// Its data as read, after the chunk's tag and length.
    #[pyo3(get, set)]
    data: Py<PyAny>,
}

/// A new event of a track: a delta time or an event, by its class.
fn track_event<'py>(
    py: Python<'py>,
    track: &Bound<'py, MidiTrack>,
    delta: Option<u64>,
) -> PyResult<Bound<'py, PyAny>> {
    let owner = Some(track.clone().into_any().unbind());
    match delta {
        Some(ticks) => {
            let time = Some(ticks.into_pyobject(py)?.into_any().unbind());
            Ok(Bound::new(py, DeltaTime::new(py, owner, time, None))?.into_any())
        }
        None => Ok(Bound::new(py, MidiEvent::new(py, owner, None, None, None)?)?.into_any()),
    }
}

#[pymethods]
impl MidiTrack {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.events)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (index = None))]
    fn new(py: Python<'_>, index: Option<Py<PyAny>>) -> PyClassInitializer<Self> {
        let index = index.unwrap_or_else(|| int(py, 0));
        crate::base::built(Self {
            index,
            events: PyList::empty(py).into_any().unbind(),
            data: py_bytes(py, b"").unbind(),
        })
    }

    /// Every track's tag.
    #[classattr]
    fn headerId(py: Python<'_>) -> Py<PyAny> {
        py_bytes(py, raw::TRACK_HEADER).unbind()
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        self.data.bind(py).len()
    }

    /// How many bytes its data is.
    #[getter]
    fn length(&self, py: Python<'_>) -> PyResult<usize> {
        self.data.bind(py).len()
    }

    /// Reads a track's chunk off the front of the bytes into this track,
    /// and gives back what follows it.
    fn read<'py>(
        slf: &Bound<'py, Self>,
        midiBytes: &Bound<'py, PyBytes>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let bytes = midiBytes.as_bytes();
        if !bytes.starts_with(raw::TRACK_HEADER) {
            return Err(raised(py, MidiDataError::NotATrack));
        }
        let (length, after) = raw::get_number(&bytes[4..], 4).map_err(|e| raised(py, e))?;
        let length = usize::try_from(length)
            .unwrap_or(usize::MAX)
            .min(after.len());
        let data = py_bytes(py, &after[..length]);
        slf.borrow_mut().data = data.clone().unbind();
        let rest = py_bytes(py, &after[length..]);
        slf.call_method1("processDataToEvents", (data,))?;
        Ok(rest)
    }

    /// Reads a track's data into delta times and events, after any it
    /// holds; one music21 cannot read is skipped.
    #[pyo3(signature = (trackData = None))]
    fn processDataToEvents(slf: &Bound<'_, Self>, trackData: Option<Vec<u8>>) -> PyResult<()> {
        let py = slf.py();
        let data = trackData.unwrap_or_default();
        let read = raw::track_events(&data).map_err(|e| raised(py, e))?;
        let events = slf.borrow().events.clone_ref(py);
        let events = events.bind(py);
        for found in read {
            let delta = track_event(py, slf, Some(found.delta))?;
            let event = track_event(py, slf, None)?;
            {
                let event = event.cast::<MidiEvent>()?;
                let mut held = event.borrow_mut();
                held.lastStatusByte = found.status_after.into_pyobject(py)?.into_any().unbind();
                if let Some(read) = &found.event {
                    held.apply(py, read)?;
                }
            }
            events.call_method1("append", (delta,))?;
            events.call_method1("append", (event,))?;
        }
        Ok(())
    }

    /// The track's chunk, from its events; an event that cannot be
    /// written is left out, with a warning.
    fn getBytes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let events = slf.borrow().events.clone_ref(py);
        let mut data = Vec::new();
        for event in events.bind(py).try_iter()? {
            let event = event?;
            match event.call_method0("getBytes") {
                Ok(written) => data.extend(bytes_of(&written)?),
                Err(error) if error.is_instance_of::<MidiException>(py) => {
                    let message = format!(
                        "Conversion error for {}: {}; ignored.",
                        event.repr()?,
                        error.value(py).str()?
                    );
                    environ_local(py)?.call_method1("warn", (message,))?;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(py_bytes(py, &raw::track_bytes(&data)))
    }

    /// Its index and how many events it holds.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "{} -- {} events",
            self.index.bind(py).str()?,
            self.events.bind(py).len()?
        ))
    }

    /// Makes this the track of every event it holds.
    fn updateEvents(slf: &Bound<'_, Self>) -> PyResult<()> {
        let events = slf.borrow().events.clone_ref(slf.py());
        for event in events.bind(slf.py()).try_iter()? {
            event?.setattr("track", slf)?;
        }
        Ok(())
    }

    /// Whether any of its events is a note-on.
    fn hasNotes(&self, py: Python<'_>) -> PyResult<bool> {
        for event in self.events.bind(py).try_iter()? {
            if event?.call_method0("isNoteOn")?.is_truthy()? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Puts every event it holds on one channel, from 1 to 16.
    fn setChannel(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let channel: Option<i64> = value.extract().ok();
        let Some(channel) = channel.filter(|channel| (1..=16).contains(channel)) else {
            return Err(MidiException::new_err(format!(
                "bad channel value: {}",
                value.str()?
            )));
        };
        for event in self.events.bind(py).try_iter()? {
            event?.setattr("channel", channel)?;
        }
        Ok(())
    }

    /// The channels its channel events are on, in order.
    fn getChannels<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let found = pyo3::types::PySet::empty(py)?;
        for event in self.events.bind(py).try_iter()? {
            let event = event?;
            if event.call_method0("isChannelEvent")?.is_truthy()? {
                found.add(event.getattr("channel")?)?;
            }
        }
        py.import("builtins")?.getattr("sorted")?.call1((found,))
    }

    /// The programs its program changes name, each once, in order.
    fn getProgramChanges<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let found = PyList::empty(py);
        let program_change = enum_class(py, "ChannelVoiceMessages")?.getattr("PROGRAM_CHANGE")?;
        for event in self.events.bind(py).try_iter()? {
            let event = event?;
            let data = event.getattr("data")?;
            if event.getattr("type")?.eq(&program_change)?
                && data.is_instance_of::<PyInt>()
                && !found.contains(&data)?
            {
                found.append(data)?;
            }
        }
        Ok(found)
    }
}

/// music21's `MidiFile`: a file's format, its tracks and how it counts
/// time, read from bytes or a file and written back.
#[pyclass(name = "MidiFile", module = "music21.midi.base", extends = ProtoM21Object, subclass, dict)]
pub struct MidiFile {
    /// The file open to read or write.
    #[pyo3(get, set)]
    file: Py<PyAny>,
    /// 0 or 1; 1 is written.
    #[pyo3(get, set)]
    format: Py<PyAny>,
    /// Its tracks.
    #[pyo3(get, set)]
    tracks: Py<PyAny>,
    /// Ticks to a quarter note, which is what most files count.
    #[pyo3(get, set)]
    ticksPerQuarterNote: Py<PyAny>,
    /// Ticks to a second, which few files count.
    #[pyo3(get, set)]
    ticksPerSecond: Py<PyAny>,
}

#[pymethods]
impl MidiFile {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.tracks)?;
        visit.call(&self.file)?;
        Ok(())
    }

    #[new]
    fn new(py: Python<'_>) -> PyResult<PyClassInitializer<Self>> {
        let ticks = crate::defaults::defaults_module(py)?.getattr("ticksPerQuarter")?;
        Ok(crate::base::built(Self {
            file: py.None(),
            format: int(py, 1),
            tracks: PyList::empty(py).into_any().unbind(),
            ticksPerQuarterNote: ticks.unbind(),
            ticksPerSecond: py.None(),
        }))
    }

    /// Every file's tag.
    #[classattr]
    fn headerId(py: Python<'_>) -> Py<PyAny> {
        py_bytes(py, raw::FILE_HEADER).unbind()
    }

    /// Opens a file to read, `'rb'`, or to write, `'wb'`.
    #[pyo3(signature = (filename, attrib = "rb"))]
    fn open(&mut self, py: Python<'_>, filename: &Bound<'_, PyAny>, attrib: &str) -> PyResult<()> {
        if attrib != "rb" && attrib != "wb" {
            let arguments = (
                "cannot read or write unless in binary mode, not:",
                attrib.to_owned(),
            );
            return Err(MidiException::new_err(arguments));
        }
        let opened = py
            .import("builtins")?
            .getattr("open")?
            .call1((filename, attrib))?;
        self.file = opened.unbind();
        Ok(())
    }

    /// Takes a file-like object, such as a `BytesIO`, as the open file.
    fn openFileLike(&mut self, fileLike: Py<PyAny>) {
        self.file = fileLike;
    }

    /// How many tracks it has.
    fn _reprInternal(&self, py: Python<'_>) -> PyResult<String> {
        let count = self.tracks.bind(py).len()?;
        let plural = if count == 1 { "" } else { "s" };
        Ok(format!("{count} track{plural}"))
    }

    /// Closes the file, where one is open.
    fn close(&self, py: Python<'_>) -> PyResult<()> {
        let file = self.file.bind(py);
        if !file.is_truthy()? {
            return Ok(());
        }
        file.call_method0("close")?;
        Ok(())
    }

    /// Reads the open file.
    fn read(slf: &Bound<'_, Self>) -> PyResult<()> {
        let file = slf.borrow().file.clone_ref(slf.py());
        if !file.bind(slf.py()).is_truthy()? {
            return Err(PyTypeError::new_err("No file is open."));
        }
        let read = file.bind(slf.py()).call_method0("read")?;
        slf.call_method1("readstr", (read,))?;
        Ok(())
    }

    /// Reads a file's bytes: its format, how it counts time, and its
    /// tracks.
    fn readstr(slf: &Bound<'_, Self>, midiBytes: &Bound<'_, PyBytes>) -> PyResult<()> {
        let py = slf.py();
        let bytes = midiBytes.as_bytes();
        let (header, rest) = match raw::read_file_header(bytes) {
            Ok(read) => read,
            Err(error @ MidiDataError::UnreadFormat(format)) => {
                slf.borrow_mut().format = format.into_pyobject(py)?.into_any().unbind();
                return Err(raised(py, error));
            }
            Err(error) => return Err(raised(py, error)),
        };
        {
            let mut this = slf.borrow_mut();
            this.format = header.format.into_pyobject(py)?.into_any().unbind();
            match header.division {
                raw::Division::TicksPerQuarterNote(ticks) => {
                    this.ticksPerQuarterNote = ticks.into_pyobject(py)?.into_any().unbind();
                }
                raw::Division::TicksPerSecond(ticks) => {
                    this.ticksPerSecond = ticks.into_pyobject(py)?.into_any().unbind();
                }
            }
        }
        let tracks = slf.borrow().tracks.clone_ref(py);
        let mut remaining = py_bytes(py, rest);
        for index in 0..header.tracks {
            let track = Bound::new(
                py,
                MidiTrack::new(py, Some(index.into_pyobject(py)?.into_any().unbind())),
            )?;
            remaining = track.call_method1("read", (remaining,))?;
            tracks.bind(py).call_method1("append", (track,))?;
        }
        Ok(())
    }

    /// Writes the file's bytes to the open file.
    fn write(slf: &Bound<'_, Self>) -> PyResult<()> {
        let file = slf.borrow().file.clone_ref(slf.py());
        if !file.bind(slf.py()).is_truthy()? {
            return Err(PyTypeError::new_err("No file is open."));
        }
        let written = slf.call_method0("writestr")?;
        file.bind(slf.py()).call_method1("write", (written,))?;
        Ok(())
    }

    /// The file's bytes: its header, then each track's chunk.
    fn writestr<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let mut out = bytes_of(&slf.call_method0("writeMThdStr")?)?;
        let tracks = slf.borrow().tracks.clone_ref(py);
        for track in tracks.bind(py).try_iter()? {
            out.extend(bytes_of(&track?.call_method0("getBytes")?)?);
        }
        Ok(py_bytes(py, &out))
    }

    /// The file's header, from its format, its tracks and its ticks to a
    /// quarter note.
    fn writeMThdStr<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let written = raw::file_header_bytes(
            self.format.bind(py).extract()?,
            self.tracks.bind(py).len()?,
            self.ticksPerQuarterNote.bind(py).extract()?,
        )
        .map_err(|e| raised(py, e))?;
        Ok(py_bytes(py, &written))
    }
}

/// The delta times and events a translated list stands for, each on
/// `track`: what music21's translating functions hand back.
pub(crate) fn laid_objects<'py>(
    py: Python<'py>,
    track: &Bound<'py, PyAny>,
    laid: &[Laid],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let owner = (!track.is_none()).then(|| track.clone().unbind());
    let mut out = Vec::with_capacity(laid.len());
    for entry in laid {
        let made = match entry {
            Laid::Delta { ticks, channel } => {
                let time = Some(ticks.into_pyobject(py)?.into_any().unbind());
                let channel = match channel {
                    Some(channel) => Some(channel.into_pyobject(py)?.into_any().unbind()),
                    None => None,
                };
                Bound::new(
                    py,
                    DeltaTime::new(
                        py,
                        owner.as_ref().map(|held| held.clone_ref(py)),
                        time,
                        channel,
                    ),
                )?
                .into_any()
            }
            Laid::Event { event, cent_shift } => {
                let made = Bound::new(
                    py,
                    MidiEvent::new(
                        py,
                        owner.as_ref().map(|held| held.clone_ref(py)),
                        None,
                        None,
                        None,
                    )?,
                )?;
                {
                    let mut held = made.borrow_mut();
                    held.apply(py, event)?;
                    if let Some(cents) = cent_shift {
                        held.centShift = cents.into_pyobject(py)?.into_any().unbind();
                    }
                }
                made.into_any()
            }
        };
        out.push(made);
    }
    Ok(out)
}

/// The module's `environLocal`.
fn environ_local(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    module(py)?.getattr("environLocal")
}

/// music21's `midi.base`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let hex_enum = crate::common::module(py, "music21.common.enums")?.getattr("HexEnum")?;
        for (name, members, doc) in ENUMS {
            let mut values = Vec::new();
            for (member, value) in members {
                values.push((*member, value.into_pyobject(py)?.into_any()));
            }
            m.add(
                name,
                new_class(py, MODULE, name, doc, vec![hex_enum.clone()], values)?,
            )?;
        }
        let exception = py.get_type::<MidiException>();
        exception.setattr("__module__", MODULE)?;
        m.add("MidiException", exception)?;
        m.add("METAEVENT_MARKER", METAEVENT_MARKER)?;
        m.add_class::<MidiEvent>()?;
        m.add_class::<DeltaTime>()?;
        m.add_class::<MidiTrack>()?;
        m.add_class::<MidiFile>()?;
        m.add_function(wrap_pyfunction!(getNumber, &m)?)?;
        m.add_function(wrap_pyfunction!(getVariableLengthNumber, &m)?)?;
        m.add_function(wrap_pyfunction!(putNumber, &m)?)?;
        m.add_function(wrap_pyfunction!(putVariableLengthNumber, &m)?)?;
        m.add_function(wrap_pyfunction!(putNumbersAsList, &m)?)?;
        let environment = crate::environment::module(py)?.getattr("Environment")?;
        m.add("environLocal", environment.call1(("midi.base",))?)?;
        let all = [
            "ChannelModeMessages",
            "ChannelVoiceMessages",
            "DeltaTime",
            "METAEVENT_MARKER",
            "MetaEvents",
            "MidiEvent",
            "MidiException",
            "MidiFile",
            "MidiTrack",
            "SysExEvents",
            "getNumber",
            "getVariableLengthNumber",
            "putNumber",
            "putNumbersAsList",
            "putVariableLengthNumber",
        ];
        m.add("__all__", all.to_vec())?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
