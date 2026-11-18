//! music21's `midi.translate`: notes, chords, instruments, meters, keys,
//! tempos and whole streams as `midi.base`'s events, tracks and files, and
//! back, over the crate's `midi::translate` and its MIDI reader and writer.
//!
//! The crate lays the events out and encodes and decodes their data; these
//! functions read what they need off music21's objects and build music21's
//! objects from the answer. A stream is written by the crate's writer and
//! read back as a `MidiFile`, and a file or track is read by the crate's
//! reader, so the packet machinery music21 does this through is not here.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::ffi::CString;

use pyo3::exceptions::{PyIndexError, PyNotImplementedError, PyTypeError, PyUserWarning};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyList, PyModule, PyTuple};

use music21::midi::base::EventType;
use music21::midi::translate::{
    self as raw, Deltas, EndDelay, Laid, Mode, NoteMark, PERCUSSION_CHANNEL, Sounding, Source,
    StartDelay, TICKS_PER_QUARTER, Unsupported,
};

use crate::Walkable;
use crate::midibase::{self, MidiEvent};

/// The module music21 keeps these in.
const MODULE: &str = "music21.midi.translate";

/// The only encoding the crate's reader and writer read and write text in.
const UTF8: &str = "utf-8";

/// NOTE_OFF's status.
const NOTE_OFF: u8 = 0x80;

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    TranslateException,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, TranslateWarning, PyUserWarning);

/// Whether a music21 object is of a class, by the names it answers.
fn is_a(object: &Bound<'_, PyAny>, class: &str) -> PyResult<bool> {
    if object.is_none() {
        return Ok(false);
    }
    object.getattr("classes")?.contains(class)
}

/// A class of music21's, as this package answers for it.
fn class<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    crate::class_of(py, module, name)
}

/// Warns a `TranslateWarning`, as music21 does.
fn warn(py: Python<'_>, message: &str) -> PyResult<()> {
    let text = CString::new(message.replace('\0', "")).unwrap_or_default();
    PyErr::warn(py, &py.get_type::<TranslateWarning>(), &text, 2)
}

/// A refusal of what this port does not do.
fn unported(what: &str) -> PyErr {
    PyNotImplementedError::new_err(format!("{what} is not ported to music21-rs"))
}

/// Refuses an encoding other than UTF-8, the only one the crate reads
/// and writes text in.
fn utf8_only(encoding: &str) -> PyResult<()> {
    if encoding.eq_ignore_ascii_case(UTF8) || encoding.eq_ignore_ascii_case("utf8") {
        return Ok(());
    }
    Err(unported(&format!("encoding {encoding:?} for a whole file")))
}

/// Text encoded as music21 encodes it, dropping what cannot be.
fn encoded(text: &Bound<'_, PyAny>, encoding: &str) -> PyResult<Vec<u8>> {
    text.call_method1("encode", (encoding, "ignore"))?
        .extract::<Vec<u8>>()
}

/// The objects a translated list stands for, as a Python list, each
/// note-on paired with its note-off as music21 pairs them.
fn listed<'py>(
    py: Python<'py>,
    track: Option<&Bound<'py, PyAny>>,
    laid: &[Laid],
) -> PyResult<Bound<'py, PyList>> {
    let none = py.None().into_bound(py);
    let objects = midibase::laid_objects(py, track.unwrap_or(&none), laid)?;
    pair_notes(&objects, laid)?;
    PyList::new(py, objects)
}

/// Sets `correspondingEvent` on each note-on and the note-off of the same
/// place among the note-offs.
fn pair_notes(objects: &[Bound<'_, PyAny>], laid: &[Laid]) -> PyResult<()> {
    let mut ons = Vec::new();
    let mut offs = Vec::new();
    for (object, entry) in objects.iter().zip(laid) {
        let Laid::Event { event, .. } = entry else {
            continue;
        };
        match event.event_type {
            EventType::ChannelVoice(NOTE_OFF) => offs.push(object),
            EventType::ChannelVoice(0x90) => ons.push(object),
            _ => {}
        }
    }
    for (on, off) in ons.iter().zip(offs) {
        on.setattr("correspondingEvent", off)?;
        off.setattr("correspondingEvent", on)?;
    }
    Ok(())
}

/// Whether a list includes delta times.
fn deltas(included: bool) -> Deltas {
    if included {
        Deltas::Included
    } else {
        Deltas::Omitted
    }
}

/// The velocity a volume's cached realized value comes to.
fn velocity_of(volume: &Bound<'_, PyAny>) -> PyResult<u8> {
    let realized: f64 = volume.getattr("cachedRealized")?.extract()?;
    Ok((realized * 127.0).round_ties_even().clamp(0.0, 127.0) as u8)
}

/// A pitched note's key and the cents it is bent by.
fn pitched(note: &Bound<'_, PyAny>) -> PyResult<(u8, Option<i32>)> {
    let pitch = note.getattr("pitch")?;
    let key: u8 = pitch.getattr("midi")?.extract()?;
    if pitch.call_method0("isTwelveTone")?.is_truthy()? {
        return Ok((key, None));
    }
    Ok((
        key,
        Some(pitch.call_method0("getCentShiftFromMidi")?.extract()?),
    ))
}

/// The key a stroke sounds on: its instrument's, or the one in force where
/// it stands, or middle C: music21's `_get_unpitched_pitch_value`.
fn stroke_key(stroke: &Bound<'_, PyAny>) -> PyResult<u8> {
    let mapped = "MidiPercussionMapMixin";
    let stored = stroke.getattr("storedInstrument")?;
    let instrument = if is_a(&stored, mapped)? {
        stored
    } else {
        let chord = stroke
            .getattr("_chordAttached")
            .unwrap_or_else(|_| stroke.py().None().into_bound(stroke.py()));
        let holder = if chord.is_none() { stroke } else { &chord };
        holder.call_method1("getContextByClass", (mapped,))?
    };
    if instrument.is_none() {
        return Ok(raw::DEFAULT_STROKE_KEY);
    }
    let key = instrument.getattr("percMapPitch")?;
    if key.is_none() {
        return Ok(raw::DEFAULT_STROKE_KEY);
    }
    key.extract()
}

/// The lyric music21 writes for a note or chord, encoded.
fn lyric_bytes(element: &Bound<'_, PyAny>, encoding: &str) -> PyResult<Vec<u8>> {
    let lyric = element.getattr("lyric")?;
    if lyric.is_none() {
        return Ok(Vec::new());
    }
    encoded(&lyric, encoding)
}

/// A length's ticks.
fn ticks_of(duration: &Bound<'_, PyAny>) -> PyResult<i64> {
    Ok(raw::duration_ticks(
        duration.getattr("quarterLength")?.extract()?,
    ))
}

/// music21's `opFrac`.
fn op_frac<'py>(py: Python<'py>, value: f64) -> PyResult<Bound<'py, PyAny>> {
    crate::common::module(py, "music21.common.numberTools")?
        .getattr("opFrac")?
        .call1((value,))
}

/// The instrument object for a crate instrument.
fn instrument_object(py: Python<'_>, made: music21::instrument::Instrument) -> PyResult<Py<PyAny>> {
    crate::instrument::Instrument::object(py, made)
}

/// An event's data as bytes, and the event itself where a list of a delta
/// time and an event was given.
fn event_in<'py>(events: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if events.is_instance_of::<MidiEvent>() {
        return Ok(events.clone());
    }
    events.get_item(1)
}

fn data_of(event: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    event.getattr("data")?.extract()
}

/// The crate's type of an event object.
fn type_of(event: &Bound<'_, PyAny>) -> PyResult<Option<EventType>> {
    midibase::event_type(&event.getattr("type")?)
}

// ------------------------------------------------------------- durations

/// An offset in ticks, a quarter later with `addStartDelay`.
#[pyfunction]
#[pyo3(signature = (o, addStartDelay = false))]
fn offsetToMidiTicks(o: f64, #[pyo3(from_py_with = crate::truthy)] addStartDelay: bool) -> i64 {
    let delay = if addStartDelay {
        StartDelay::Quarter
    } else {
        StartDelay::Without
    };
    raw::offset_ticks(o, delay)
}

/// A duration's ticks, with no start delay.
#[pyfunction]
fn durationToMidiTicks(d: &Bound<'_, PyAny>) -> PyResult<i64> {
    ticks_of(d)
}

/// The duration so many ticks last, set on `inputM21DurationObject` where
/// one is given.
#[pyfunction]
#[pyo3(signature = (ticks, ticksPerQuarter = TICKS_PER_QUARTER, inputM21DurationObject = None))]
fn ticksToDuration<'py>(
    py: Python<'py>,
    ticks: i64,
    ticksPerQuarter: i64,
    inputM21DurationObject: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let duration = match inputM21DurationObject {
        Some(given) if !given.is_none() => given,
        _ => class(py, "music21.duration", "Duration")?.call0()?,
    };
    let length = op_frac(py, ticks as f64 / ticksPerQuarter as f64)?;
    duration.setattr("quarterLength", length)?;
    Ok(duration)
}

// ---------------------------------------------------------- track ends

/// What a track starts with: its name and its instrument's program.
#[pyfunction]
#[pyo3(signature = (midiTrack = None, channel = 1, instrumentObj = None, *, encoding = UTF8))]
fn getStartEvents<'py>(
    py: Python<'py>,
    midiTrack: Option<Bound<'py, PyAny>>,
    channel: u8,
    instrumentObj: Option<Bound<'py, PyAny>>,
    encoding: &str,
) -> PyResult<Bound<'py, PyList>> {
    let instrument = instrumentObj.filter(|given| !given.is_none());
    let mut name = Vec::new();
    let mut program = None;
    if let Some(instrument) = &instrument {
        if is_a(instrument, "Conductor")? {
            return Ok(PyList::empty(py));
        }
        let best = instrument.call_method0("bestName")?;
        if !best.is_none() {
            name = encoded(&best, encoding)?;
        }
        program = instrument.getattr("midiProgram")?.extract()?;
    }
    listed(
        py,
        midiTrack.as_ref(),
        &raw::start_events(&name, program, channel),
    )
}

/// What a track ends with, a quarter after its last event unless
/// `addEndDelay` is false.
#[pyfunction]
#[pyo3(signature = (midiTrack = None, *, addEndDelay = true))]
fn getEndEvents<'py>(
    py: Python<'py>,
    midiTrack: Option<Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] addEndDelay: bool,
) -> PyResult<Bound<'py, PyList>> {
    let delay = if addEndDelay {
        EndDelay::Quarter
    } else {
        EndDelay::Without
    };
    listed(py, midiTrack.as_ref(), &raw::end_events(delay))
}

// ---------------------------------------------------- notes and chords

/// A note or stroke as its events.
#[pyfunction]
#[pyo3(signature = (inputM21, *, includeDeltaTime = true, channel = 1, encoding = UTF8))]
fn noteToMidiEvents<'py>(
    py: Python<'py>,
    inputM21: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeDeltaTime: bool,
    channel: u8,
    encoding: &str,
) -> PyResult<Bound<'py, PyList>> {
    let (key, cent_shift) = if is_a(inputM21, "Unpitched")? {
        (stroke_key(inputM21)?, None)
    } else {
        pitched(inputM21)?
    };
    let sounding = Sounding {
        key,
        cent_shift,
        velocity: velocity_of(&inputM21.getattr("volume")?)?,
    };
    let laid = raw::note_events(
        &sounding,
        &lyric_bytes(inputM21, encoding)?,
        ticks_of(&inputM21.getattr("duration")?)?,
        channel,
        deltas(includeDeltaTime),
    );
    listed(py, None, &laid)
}

/// A chord or percussion chord as its events.
#[pyfunction]
#[pyo3(signature = (inputM21, *, includeDeltaTime = true, channel = 1, encoding = UTF8))]
fn chordToMidiEvents<'py>(
    py: Python<'py>,
    inputM21: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeDeltaTime: bool,
    channel: u8,
    encoding: &str,
) -> PyResult<Bound<'py, PyList>> {
    let own = inputM21.call_method0("hasComponentVolumes")?.is_truthy()?;
    let chord_velocity = velocity_of(&inputM21.getattr("volume")?)?;
    let mut members = Vec::new();
    for member in inputM21.walk()? {
        let member = member?;
        let (key, cent_shift) = if is_a(&member, "Note")? {
            pitched(&member)?
        } else if is_a(&member, "Unpitched")? {
            (stroke_key(&member)?, None)
        } else {
            return Err(PyTypeError::new_err(
                "ChordBase can only contain Note and Unpitched as members",
            ));
        };
        let velocity = if own {
            velocity_of(&member.getattr("volume")?)?
        } else {
            chord_velocity
        };
        members.push(Sounding {
            key,
            cent_shift,
            velocity,
        });
    }
    let laid = raw::chord_events(
        &members,
        &lyric_bytes(inputM21, encoding)?,
        ticks_of(&inputM21.getattr("duration")?)?,
        channel,
        deltas(includeDeltaTime),
    );
    listed(py, None, &laid)
}

/// A note, chord or other class of music21's of a length so many ticks
/// last, or a grace note where they are none.
fn sounding_of<'py>(
    py: Python<'py>,
    made: &Bound<'py, PyAny>,
    arguments: Bound<'py, PyTuple>,
    ticks: i64,
    per_quarter: i64,
) -> PyResult<Bound<'py, PyAny>> {
    if ticks == 0 {
        let object = made.call1(&arguments)?;
        let keywords = PyDict::new(py);
        keywords.set_item("inPlace", true)?;
        object.call_method("getGrace", (), Some(&keywords))?;
        return Ok(object);
    }
    let keywords = PyDict::new(py);
    keywords.set_item("duration", ticksToDuration(py, ticks, per_quarter, None)?)?;
    made.call(arguments, Some(&keywords))
}

/// A volume of a velocity read from MIDI, which is absolute.
fn set_velocity(volume: &Bound<'_, PyAny>, velocity: &Bound<'_, PyAny>) -> PyResult<()> {
    volume.setattr("velocity", velocity)?;
    volume.setattr("velocityIsRelative", false)
}

/// The percussion instrument a key plays, as an object.
fn percussion_object(py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    instrument_object(py, raw::percussion_instrument(key.extract()?))
}

/// The note, or the stroke on channel 10, a `TimedNoteEvent` stands for.
#[pyfunction]
#[pyo3(signature = (timedNoteEvent, ticksPerQuarter = TICKS_PER_QUARTER))]
fn midiEventsToNote<'py>(
    py: Python<'py>,
    timedNoteEvent: &Bound<'py, PyAny>,
    ticksPerQuarter: i64,
) -> PyResult<Bound<'py, PyAny>> {
    let (on, off, event): (i64, i64, Bound<'py, PyAny>) = timedNoteEvent.extract()?;
    let channel: i64 = event.getattr("channel")?.extract()?;
    let stroke = channel == i64::from(PERCUSSION_CHANNEL);
    let name = if stroke { "Unpitched" } else { "Note" };
    let made = sounding_of(
        py,
        &class(py, "music21.note", name)?,
        PyTuple::empty(py),
        off - on,
        ticksPerQuarter,
    )?;
    let key = event.getattr("pitch")?;
    if stroke {
        made.setattr("storedInstrument", percussion_object(py, &key)?)?;
    } else {
        made.getattr("pitch")?.setattr("midi", key)?;
    }
    set_velocity(&made.getattr("volume")?, &event.getattr("velocity")?)?;
    Ok(made)
}

/// The chord, or the percussion chord where any note is on channel 10,
/// `TimedNoteEvent`s stand for; timed by the last of them, as music21
/// times it.
#[pyfunction]
#[pyo3(signature = (timedNoteList, ticksPerQuarter = TICKS_PER_QUARTER))]
fn midiEventsToChord<'py>(
    py: Python<'py>,
    timedNoteList: &Bound<'py, PyAny>,
    ticksPerQuarter: i64,
) -> PyResult<Bound<'py, PyAny>> {
    let timed: Vec<(i64, i64, Bound<'py, PyAny>)> = timedNoteList
        .walk()?
        .map(|item| item?.extract())
        .collect::<PyResult<_>>()?;
    let Some((on, off, _)) = timed.last() else {
        return Err(PyIndexError::new_err("list index out of range"));
    };
    let mut keys = Vec::new();
    let mut volumes = Vec::new();
    let mut percussion = false;
    let volume_class = class(py, "music21.volume", "Volume")?;
    for (_, _, event) in &timed {
        let channel: i64 = event.getattr("channel")?.extract()?;
        percussion |= channel == i64::from(PERCUSSION_CHANNEL);
        keys.push(event.getattr("pitch")?);
        let keywords = PyDict::new(py);
        keywords.set_item("velocity", event.getattr("velocity")?)?;
        let volume = volume_class.call((), Some(&keywords))?;
        volume.setattr("velocityIsRelative", false)?;
        volumes.push(volume);
    }

    // A percussion chord holds strokes; a chord holds pitches.
    let made = if percussion {
        let stroke_class = class(py, "music21.note", "Unpitched")?;
        let mut strokes = Vec::new();
        // Each stroke takes its volume as it is made: a percussion chord
        // sets none on what it holds.
        for (key, volume) in keys.iter().zip(&volumes) {
            let stroke = stroke_class.call0()?;
            stroke.setattr("storedInstrument", percussion_object(py, key)?)?;
            stroke.setattr("volume", volume)?;
            strokes.push(stroke);
        }
        let held = PyList::new(py, strokes)?;
        sounding_of(
            py,
            &class(py, "music21.percussion", "PercussionChord")?,
            PyTuple::new(py, [held])?,
            off - on,
            ticksPerQuarter,
        )?
    } else {
        let made = sounding_of(
            py,
            &class(py, "music21.chord", "Chord")?,
            PyTuple::empty(py),
            off - on,
            ticksPerQuarter,
        )?;
        let pitch_class = class(py, "music21.pitch", "Pitch")?;
        let mut pitches = Vec::new();
        for key in &keys {
            let pitch = pitch_class.call0()?;
            pitch.setattr("midi", key)?;
            pitches.push(pitch);
        }
        made.setattr("pitches", PyTuple::new(py, pitches)?)?;
        made
    };
    if !percussion {
        made.call_method1("setVolumes", (PyList::new(py, volumes)?,))?;
    }
    Ok(made)
}

// ---------------------------------------------- instruments and meta

/// An instrument as a program change, nothing for a conductor.
#[pyfunction]
#[pyo3(signature = (inputM21, includeDeltaTime = true, midiTrack = None, channel = 1))]
fn instrumentToMidiEvents<'py>(
    py: Python<'py>,
    inputM21: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeDeltaTime: bool,
    midiTrack: Option<Bound<'py, PyAny>>,
    channel: u8,
) -> PyResult<Bound<'py, PyList>> {
    if is_a(inputM21, "Conductor")? {
        return Ok(PyList::empty(py));
    }
    let program: Option<u8> = inputM21.getattr("midiProgram")?.extract()?;
    let laid = raw::program_events(program.unwrap_or(0), channel, deltas(includeDeltaTime));
    listed(py, midiTrack.as_ref(), &laid)
}

/// The instrument a program change or a name event says, on the event's
/// channel; a name that cannot be decoded is warned of and names nothing.
#[pyfunction]
#[pyo3(signature = (event, *, encoding = UTF8))]
fn midiEventToInstrument<'py>(
    py: Python<'py>,
    event: &Bound<'py, PyAny>,
    encoding: &str,
) -> PyResult<Py<PyAny>> {
    let channel: u8 = event.getattr("channel")?.extract()?;
    let data = event.getattr("data")?;
    if let Ok(bytes) = data.cast::<PyBytes>() {
        let text: String = match bytes.call_method1("decode", (encoding,)) {
            Ok(decoded) => decoded.extract()?,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyUnicodeDecodeError>(py) => {
                let shown = event.repr()?;
                warn(
                    py,
                    &format!(
                        "Unable to determine instrument from {shown}; getting generic Instrument"
                    ),
                )?;
                return instrument_object(py, raw::instrument_of(Source::Nothing, channel));
            }
            Err(error) => return Err(error),
        };
        let source = match type_of(event)? {
            Some(kind) => raw::source_of(kind, &text),
            None => Source::OtherName(&text),
        };
        return instrument_object(py, raw::instrument_of(source, channel));
    }

    // A program; one past MIDI's names no instrument.
    let source = match data.extract::<i64>().ok().map(u8::try_from) {
        Some(Ok(program)) if program < 128 => Source::Program(program),
        _ => Source::Nothing,
    };
    instrument_object(py, raw::instrument_of(source, channel))
}

/// A meter's events, none with a warning where MIDI cannot hold it.
#[pyfunction]
#[pyo3(signature = (ts, includeDeltaTime = true))]
fn timeSignatureToMidiEvents<'py>(
    py: Python<'py>,
    ts: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeDeltaTime: bool,
) -> PyResult<Bound<'py, PyList>> {
    let numerator: u32 = ts.getattr("numerator")?.extract()?;
    let denominator: u32 = ts.getattr("denominator")?.extract()?;
    let laid = raw::meter_events(numerator, denominator, deltas(includeDeltaTime));
    if laid.is_empty() {
        let shown = ts.str()?;
        warn(
            py,
            &format!(
                "TimeSignature with numerator > 255 cannot be stored in MIDI. Ignoring {shown}"
            ),
        )?;
    }
    listed(py, None, &laid)
}

/// The meter a TIME_SIGNATURE event, or a delta time and one, says.
#[pyfunction]
fn midiEventsToTimeSignature<'py>(
    py: Python<'py>,
    eventList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let data = data_of(&event_in(eventList)?)?;
    let (numerator, denominator) =
        raw::meter_of(&data).ok_or_else(|| PyIndexError::new_err("list index out of range"))?;
    class(py, "music21.meter", "TimeSignature")?.call1((format!("{numerator}/{denominator}"),))
}

/// A key or key signature's events; music21 names no channel on them.
#[pyfunction]
#[pyo3(signature = (ks, includeDeltaTime = true))]
fn keySignatureToMidiEvents<'py>(
    py: Python<'py>,
    ks: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeDeltaTime: bool,
) -> PyResult<Bound<'py, PyList>> {
    let sharps: i32 = ks.getattr("sharps")?.extract()?;
    let minor = is_a(ks, "Key")? && ks.getattr("mode")?.eq("minor")?;
    let mode = if minor { Mode::Minor } else { Mode::Major };
    listed(
        py,
        None,
        &raw::key_events(sharps, mode, deltas(includeDeltaTime)),
    )
}

/// The key a KEY_SIGNATURE event, or a delta time and one, says.
#[pyfunction]
fn midiEventsToKey<'py>(
    py: Python<'py>,
    eventList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let data = data_of(&event_in(eventList)?)?;
    let (sharps, mode) =
        raw::key_of(&data).ok_or_else(|| PyIndexError::new_err("list index out of range"))?;
    let mode = match mode {
        Mode::Minor => "minor",
        Mode::Major => "major",
    };
    class(py, "music21.key", "KeySignature")?
        .call1((sharps,))?
        .call_method1("asKey", (mode,))
}

/// A tempo's events, `None` for a mark that says no number.
#[pyfunction]
#[pyo3(signature = (tempoIndication, includeDeltaTime = true))]
fn tempoToMidiEvents<'py>(
    py: Python<'py>,
    tempoIndication: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] includeDeltaTime: bool,
) -> PyResult<Option<Bound<'py, PyList>>> {
    if tempoIndication.getattr("number")?.is_none()
        && tempoIndication.getattr("numberSounding")?.is_none()
    {
        return Ok(None);
    }
    let bpm: f64 = tempoIndication
        .call_method0("getSoundingMetronomeMark")?
        .call_method0("getQuarterBPM")?
        .extract()?;
    Ok(Some(listed(
        py,
        None,
        &raw::tempo_events(bpm, deltas(includeDeltaTime)),
    )?))
}

/// The metronome mark a SET_TEMPO event, or a delta time and one, says.
#[pyfunction]
fn midiEventsToTempo<'py>(
    py: Python<'py>,
    eventList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let data = data_of(&event_in(eventList)?)?;
    let bpm = raw::tempo_of(&data).ok_or_else(|| {
        pyo3::exceptions::PyZeroDivisionError::new_err("a tempo of no time to the quarter")
    })?;
    let keywords = PyDict::new(py);
    keywords.set_item("number", bpm)?;
    class(py, "music21.tempo", "MetronomeMark")?.call((), Some(&keywords))
}

// --------------------------------------------------------- elements

/// What an element is as events, with no delta times, or `None`.
#[pyfunction]
#[pyo3(signature = (el, encoding = UTF8))]
fn elementToMidiEventList<'py>(
    py: Python<'py>,
    el: &Bound<'py, PyAny>,
    encoding: &str,
) -> PyResult<Option<Bound<'py, PyList>>> {
    if is_a(el, "Rest")? {
        return Ok(None);
    }
    let found = if is_a(el, "Note")? {
        noteToMidiEvents(py, el, false, 1, encoding)?
    } else if is_a(el, "Unpitched")? {
        noteToMidiEvents(py, el, false, PERCUSSION_CHANNEL, encoding)?
    } else if is_a(el, "Chord")? {
        chordToMidiEvents(py, el, false, 1, encoding)?
    } else if is_a(el, "PercussionChord")? {
        chordToMidiEvents(py, el, false, PERCUSSION_CHANNEL, encoding)?
    } else if is_a(el, "TimeSignature")? {
        timeSignatureToMidiEvents(py, el, false)?
    } else if is_a(el, "KeySignature")? {
        keySignatureToMidiEvents(py, el, false)?
    } else if is_a(el, "MetronomeMark")? {
        return tempoToMidiEvents(py, el, false);
    } else if is_a(el, "Instrument")? {
        instrumentToMidiEvents(py, el, false, None, 1)?
    } else {
        return Ok(None);
    };
    Ok(Some(found))
}

/// An event with its track, tick, source object and the instrument last
/// in force, as music21 sorts and channels them.
#[pyfunction]
#[pyo3(signature = (trackId, offset, midiEvent, obj = None, lastInstrument = None))]
fn getPacketFromMidiEvent<'py>(
    py: Python<'py>,
    trackId: Bound<'py, PyAny>,
    offset: Bound<'py, PyAny>,
    midiEvent: Bound<'py, PyAny>,
    obj: Option<Bound<'py, PyAny>>,
    lastInstrument: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let none = py.None().into_bound(py);
    let obj = obj.unwrap_or_else(|| none.clone());
    let packet = PyDict::new(py);
    packet.set_item("trackId", trackId)?;
    packet.set_item("offset", offset)?;
    packet.set_item("midiEvent", &midiEvent)?;
    packet.set_item("obj", &obj)?;
    packet.set_item("centShift", midiEvent.getattr("centShift")?)?;

    // A note-off shares its note's object but lasts no time.
    let off = type_of(&midiEvent)? == Some(EventType::ChannelVoice(NOTE_OFF));
    let duration = if off || obj.is_none() {
        0
    } else {
        ticks_of(&obj.getattr("duration")?)?
    };
    packet.set_item("duration", duration)?;
    packet.set_item("lastInstrument", lastInstrument.unwrap_or(none))?;
    Ok(packet)
}

/// The packets of one track, or all of them where no track is named.
#[pyfunction]
#[pyo3(signature = (packetsSrc, trackIdFilter = None))]
fn filterPacketsByTrackId<'py>(
    py: Python<'py>,
    packetsSrc: Bound<'py, PyAny>,
    trackIdFilter: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(wanted) = trackIdFilter.filter(|given| !given.is_none()) else {
        return Ok(packetsSrc);
    };
    let out = PyList::empty(py);
    for packet in packetsSrc.walk()? {
        let packet = packet?;
        if packet.get_item("trackId")?.eq(&wanted)? {
            out.append(packet)?;
        }
    }
    Ok(out.into_any())
}

// --------------------------------------------------- events of a track

/// Each event of a track with the tick it falls on.
#[pyfunction]
fn getTimeForEvents<'py>(py: Python<'py>, mt: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let events: Vec<Bound<'py, PyAny>> = mt.getattr("events")?.walk()?.collect::<PyResult<_>>()?;
    let mut waits = Vec::with_capacity(events.len());
    for event in &events {
        let wait = if event.call_method0("isDeltaTime")?.is_truthy()? {
            Some(event.getattr("time")?.extract::<u64>()?)
        } else {
            None
        };
        waits.push(wait);
    }
    let out = PyList::empty(py);
    for (tick, index) in raw::event_ticks(&waits) {
        out.append((tick, &events[index]))?;
    }
    Ok(out)
}

/// The `TimedNoteEvent` class.
fn timed_note_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    module(py)?.getattr("TimedNoteEvent")
}

/// Each note-on of timed events with the note-off that ends it, as
/// `TimedNoteEvent`s in the order they start.
#[pyfunction]
fn getNotesFromEvents<'py>(
    py: Python<'py>,
    timedEvents: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let timed: Vec<(u64, Bound<'py, PyAny>)> = timedEvents
        .walk()?
        .map(|item| item?.extract())
        .collect::<PyResult<_>>()?;
    let mut marks = Vec::with_capacity(timed.len());
    for (tick, event) in &timed {
        let key = (
            event.getattr("pitch")?.extract::<Option<i64>>()?,
            event.getattr("channel")?.extract::<Option<i64>>()?,
        );
        let mark = if event.call_method0("isNoteOff")?.is_truthy()? {
            NoteMark::Off(key)
        } else if event.call_method0("isNoteOn")?.is_truthy()? {
            NoteMark::On(key)
        } else {
            NoteMark::Other
        };
        marks.push((*tick, mark));
    }
    let made = timed_note_class(py)?;
    let out = PyList::empty(py);
    for note in raw::notes_of(&marks) {
        out.append(made.call1((note.on, note.off, &timed[note.index].1))?)?;
    }
    Ok(out)
}

/// Each tick's lyric, the last where several fall on one.
#[pyfunction]
#[pyo3(signature = (events, *, encoding = UTF8))]
fn lyricTimingsFromEvents<'py>(
    py: Python<'py>,
    events: &Bound<'py, PyAny>,
    encoding: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let lyric = EventType::Meta(0x05);
    let out = PyDict::new(py);
    for item in events.walk()? {
        let (tick, event): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let data = event.getattr("data")?;
        if type_of(&event)? != Some(lyric) || !data.is_instance_of::<PyBytes>() {
            continue;
        }
        match data.call_method1("decode", (encoding,)) {
            Ok(text) => out.set_item(tick, text)?,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyUnicodeDecodeError>(py) => {
                let shown = event.repr()?;
                warn(
                    py,
                    &format!("Unable to decode lyrics from {shown} as {encoding}"),
                )?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(out)
}

/// The meters, keys, tempos and instruments timed events state, each
/// with its tick; a name event takes the program last changed to.
#[pyfunction]
#[pyo3(signature = (events, *, encoding = UTF8))]
fn getMetaEvents<'py>(
    py: Python<'py>,
    events: &Bound<'py, PyAny>,
    encoding: &str,
) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    let mut last_program: Option<Bound<'py, PyAny>> = None;
    for item in events.walk()? {
        let (tick, event): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let made: Bound<'py, PyAny> = match type_of(&event)? {
            Some(EventType::Meta(0x58)) => midiEventsToTimeSignature(py, &event)?,
            Some(EventType::Meta(0x59)) => midiEventsToKey(py, &event)?,
            Some(EventType::Meta(0x51)) => midiEventsToTempo(py, &event)?,
            Some(EventType::Meta(0x03 | 0x04)) => {
                let made = midiEventToInstrument(py, &event, encoding)?.into_bound(py);
                if let Some(program) = &last_program {
                    made.setattr("midiProgram", program)?;
                }
                made
            }
            Some(EventType::ChannelVoice(0xC0)) => {
                let program = event.getattr("parameter1")?;
                if !program.is_instance_of::<pyo3::types::PyInt>() {
                    continue;
                }
                let made = midiEventToInstrument(py, &event, UTF8)?.into_bound(py);
                last_program = Some(program);
                made
            }
            _ => continue,
        };
        out.append((tick, made))?;
    }
    Ok(out)
}

// ------------------------------------------------------------- streams

/// A track's data, after its chunk's tag and length.
fn track_data(track: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let chunk: Vec<u8> = track.call_method0("getBytes")?.extract()?;
    Ok(chunk.get(8..).unwrap_or_default().to_vec())
}

/// Refuses the reading keywords the crate's reader does not take.
fn reading_as_read(
    quantize: bool,
    keywords: Option<&Bound<'_, PyDict>>,
    encoding: &str,
) -> PyResult<()> {
    utf8_only(encoding)?;
    if !quantize {
        return Err(unported("quantizePost=False"));
    }
    let Some(keywords) = keywords else {
        return Ok(());
    };
    for (name, value) in keywords.iter() {
        let name: String = name.extract()?;
        if name == "quarterLengthDivisors" {
            let divisors: Vec<i64> = value.extract()?;
            if divisors.is_empty() || divisors == [4, 3] {
                continue;
            }
            return Err(unported("quarterLengthDivisors other than (4, 3)"));
        }
        return Err(unported(&format!("the keyword {name}")));
    }
    Ok(())
}

/// Puts what a score read holds into the score given to fill, where one
/// was given.
fn filled<'py>(
    read: Bound<'py, PyAny>,
    inputM21: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(target) = inputM21.filter(|given| !given.is_none()) else {
        return Ok(read);
    };
    let parts: Vec<Bound<'py, PyAny>> = read.walk()?.collect::<PyResult<_>>()?;
    for part in parts {
        target.call_method1("insert", (0, part))?;
    }
    Ok(target)
}

/// Ticks to a quarter as a file can say them.
fn per_quarter(value: i64) -> PyResult<u16> {
    u16::try_from(value).map_err(|_| unported(&format!("{value} ticks to a quarter")))
}

/// One track read as a part, alone.
#[pyfunction]
#[pyo3(signature = (
    mt, *, ticksPerQuarter = TICKS_PER_QUARTER, quantizePost = true, inputM21 = None,
    conductorPart = None, isFirst = None, quarterLengthDivisors = None,
    encoding = UTF8, **keywords
))]
#[allow(clippy::too_many_arguments)]
fn midiTrackToStream<'py>(
    py: Python<'py>,
    mt: &Bound<'py, PyAny>,
    ticksPerQuarter: i64,
    #[pyo3(from_py_with = crate::truthy)] quantizePost: bool,
    inputM21: Option<Bound<'py, PyAny>>,
    conductorPart: Option<Bound<'py, PyAny>>,
    isFirst: Option<Bound<'py, PyAny>>,
    quarterLengthDivisors: Option<Bound<'py, PyAny>>,
    encoding: &str,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = isFirst;
    let all = keywords.cloned().unwrap_or_else(|| PyDict::new(py));
    if let Some(divisors) = quarterLengthDivisors {
        all.set_item("quarterLengthDivisors", divisors)?;
    }
    reading_as_read(quantizePost, Some(&all), encoding)?;
    if inputM21.is_some_and(|given| !given.is_none()) {
        return Err(unported("inputM21 for one track"));
    }
    if conductorPart.is_some_and(|given| !given.is_none()) {
        return Err(unported("conductorPart"));
    }
    let read = music21::midi::part_from_track(&track_data(mt)?, per_quarter(ticksPerQuarter)?);
    crate::formats::handed_back(py, read)
}

/// Tracks read as a score, each with notes a part.
#[pyfunction]
#[pyo3(signature = (
    midiTracks, ticksPerQuarter = TICKS_PER_QUARTER, quantizePost = true, inputM21 = None,
    *, encoding = UTF8, **keywords
))]
fn midiTracksToStreams<'py>(
    py: Python<'py>,
    midiTracks: &Bound<'py, PyAny>,
    ticksPerQuarter: i64,
    #[pyo3(from_py_with = crate::truthy)] quantizePost: bool,
    inputM21: Option<Bound<'py, PyAny>>,
    encoding: &str,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    reading_as_read(quantizePost, keywords, encoding)?;
    let mut data = Vec::new();
    for track in midiTracks.walk()? {
        data.push(track_data(&track?)?);
    }
    let slices: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
    let read = music21::midi::from_tracks(&slices, per_quarter(ticksPerQuarter)?);
    filled(crate::formats::handed_back(py, read)?, inputM21)
}

/// A `MidiFile` read as a score.
#[pyfunction]
#[pyo3(signature = (mf, *, inputM21 = None, quantizePost = true, encoding = UTF8, **keywords))]
fn midiFileToStream<'py>(
    py: Python<'py>,
    mf: &Bound<'py, PyAny>,
    inputM21: Option<Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] quantizePost: bool,
    encoding: &str,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let tracks = mf.getattr("tracks")?;
    if !tracks.is_truthy()? {
        return Err(crate::stream::StreamException::new_err(
            "no tracks are defined in this MIDI file.",
        ));
    }
    let ticks: i64 = mf.getattr("ticksPerQuarterNote")?.extract()?;
    midiTracksToStreams(
        py,
        &tracks,
        ticks,
        quantizePost,
        inputM21,
        encoding,
        keywords,
    )
}

/// A new, empty `MidiFile`.
fn new_file(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    midibase::module(py)?.getattr("MidiFile")?.call0()
}

/// The MIDI file at a path read as a score.
#[pyfunction]
#[pyo3(signature = (filePath, *, inputM21 = None, encoding = UTF8, **keywords))]
fn midiFilePathToStream<'py>(
    py: Python<'py>,
    filePath: &Bound<'py, PyAny>,
    inputM21: Option<Bound<'py, PyAny>>,
    encoding: &str,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let file = new_file(py)?;
    file.call_method1("open", (filePath,))?;
    file.call_method0("read")?;
    file.call_method0("close")?;
    let (quantize, rest) = split_quantize(py, keywords)?;
    midiFileToStream(py, &file, inputM21, quantize, encoding, Some(&rest))
}

/// `quantizePost` out of keywords, and the rest of them.
fn split_quantize<'py>(
    py: Python<'py>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<(bool, Bound<'py, PyDict>)> {
    let rest = keywords
        .map(|given| given.copy())
        .transpose()?
        .unwrap_or_else(|| PyDict::new(py));
    let Some(given) = rest.get_item("quantizePost")? else {
        return Ok((true, rest));
    };
    rest.del_item("quantizePost")?;
    Ok((crate::truthy(&given)?, rest))
}

/// A file's bytes read as a score.
#[pyfunction]
#[pyo3(signature = (strData, **keywords))]
fn midiStringToStream<'py>(
    py: Python<'py>,
    strData: &Bound<'py, PyAny>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let file = new_file(py)?;
    file.call_method1("readstr", (strData,))?;
    let (quantize, rest) = split_quantize(py, keywords)?;
    let encoding: String = match rest.get_item("encoding")? {
        Some(given) => {
            rest.del_item("encoding")?;
            given.extract()?
        }
        None => UTF8.to_string(),
    };
    let input = rest.get_item("inputM21")?;
    if input.is_some() {
        rest.del_item("inputM21")?;
    }
    midiFileToStream(py, &file, input, quantize, &encoding, Some(&rest))
}

/// A file written from music21's text notation for MIDI.
#[pyfunction]
#[pyo3(signature = (midiFormat = 1, ticksPerQuarterNote = 960, tracksEventsList = None))]
fn midiAsciiStringToBinaryString<'py>(
    py: Python<'py>,
    midiFormat: i128,
    ticksPerQuarterNote: i128,
    tracksEventsList: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let mut tracks = Vec::new();
    if let Some(listed) = tracksEventsList.filter(|given| !given.is_none()) {
        for track in listed.walk()? {
            let mut lines = Vec::new();
            for line in track?.walk()? {
                lines.push(line?.str()?.to_string());
            }
            tracks.push(lines);
        }
    }
    let (bytes, unsupported) = raw::ascii_file(midiFormat, ticksPerQuarterNote, &tracks)
        .map_err(crate::formats::format_error)?;
    let environment = module(py)?.getattr("environLocal")?;
    for left_out in unsupported {
        let message = match left_out {
            Unsupported::Meta(status) => format!("Unsupported meta event: 0x{status}"),
            Unsupported::Channel(status) => format!("Unsupported midi event: 0x{status}"),
        };
        environment.call_method1("warn", (message,))?;
    }
    Ok(PyBytes::new(py, &bytes))
}

/// A stream written as a `MidiFile`: a conductor track, then a track per
/// part, by the crate's writer.
#[pyfunction]
#[pyo3(signature = (
    inputM21, *, addStartDelay = false, addEndDelay = true, acceptableChannelList = None,
    encoding = UTF8
))]
fn streamToMidiFile<'py>(
    py: Python<'py>,
    inputM21: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] addStartDelay: bool,
    #[pyo3(from_py_with = crate::truthy)] addEndDelay: bool,
    acceptableChannelList: Option<Vec<u8>>,
    encoding: &str,
) -> PyResult<Bound<'py, PyAny>> {
    utf8_only(encoding)?;
    let stream = crate::stream::crate_stream(inputM21)?;
    let mut options = music21::midi::ExportOptions::default();
    options.add_start_delay = addStartDelay;
    options.add_end_delay = addEndDelay;
    options.acceptable_channels = acceptableChannelList;
    let bytes = music21::midi::to_midi(&stream, &options).map_err(crate::formats::format_error)?;
    let file = new_file(py)?;
    file.call_method1("readstr", (PyBytes::new(py, &bytes),))?;
    Ok(file)
}

/// The tracks [`streamToMidiFile`] writes.
#[pyfunction]
#[pyo3(signature = (
    inputM21, *, acceptableChannelList = None, addStartDelay = false, addEndDelay = true,
    encoding = UTF8
))]
fn streamHierarchyToMidiTracks<'py>(
    py: Python<'py>,
    inputM21: &Bound<'py, PyAny>,
    acceptableChannelList: Option<Vec<u8>>,
    #[pyo3(from_py_with = crate::truthy)] addStartDelay: bool,
    #[pyo3(from_py_with = crate::truthy)] addEndDelay: bool,
    encoding: &str,
) -> PyResult<Bound<'py, PyAny>> {
    streamToMidiFile(
        py,
        inputM21,
        addStartDelay,
        addEndDelay,
        acceptableChannelList,
        encoding,
    )?
    .getattr("tracks")
}

/// Any object written as a `MidiFile`: a stream at sounding pitch as it
/// is, anything else a copy in a stream of its own.
#[pyfunction]
#[pyo3(signature = (music21Object, *, addStartDelay = false, addEndDelay = true, encoding = UTF8))]
fn music21ObjectToMidiFile<'py>(
    py: Python<'py>,
    music21Object: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] addStartDelay: bool,
    #[pyo3(from_py_with = crate::truthy)] addEndDelay: bool,
    encoding: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let written = if is_a(music21Object, "Stream")? {
        let sounding = music21Object.getattr("atSoundingPitch")?;
        if sounding.is_instance_of::<pyo3::types::PyBool>() && !sounding.is_truthy()? {
            music21Object.call_method0("toSoundingPitch")?
        } else {
            music21Object.clone()
        }
    } else {
        let copied = py
            .import("copy")?
            .call_method1("deepcopy", (music21Object,))?;
        let holder = class(py, "music21.stream", "Stream")?.call0()?;
        holder.call_method1("insert", (0, copied))?;
        holder
    };
    streamToMidiFile(py, &written, addStartDelay, addEndDelay, None, encoding)
}

// ------------------------------------------------- preparing streams

/// Streams held anywhere in `s`, and `s` itself first.
fn streams_within<'py>(s: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut out = vec![s.clone()];
    let inner = s
        .call_method0("recurse")?
        .call_method1("getElementsByClass", ("Stream",))?;
    for held in inner.walk()? {
        out.push(held?);
    }
    Ok(out)
}

/// Everything of a class anywhere in `s`.
fn all_of<'py>(s: &Bound<'py, PyAny>, klass: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    s.call_method0("recurse")?
        .call_method1("getElementsByClass", (klass,))?
        .walk()?
        .collect()
}

/// Takes the tempos, meters and keys out of `s` into a part of their own,
/// the first of each class at each offset, with a tempo of 120 and a meter
/// of 4/4 where it has none, sorting before every other part.
#[pyfunction]
fn conductorStream<'py>(py: Python<'py>, s: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let parts = s
        .call_method1("getElementsByClass", ("Stream",))?
        .call_method1("getElementsByOffset", (0,))?;
    let mut lowest: Option<i64> = None;
    for part in parts.walk()? {
        // A stream never given a priority has music21's default, nought.
        let priority: i64 = part?
            .getattr("priority")
            .and_then(|held| held.extract())
            .unwrap_or(0);
        lowest = Some(lowest.map_or(priority, |held| held.min(priority)));
    }
    let conductor = class(py, "music21.stream", "Part")?.call0()?;
    conductor.setattr("priority", lowest.unwrap_or(0) - 1)?;
    conductor.call_method1(
        "insert",
        (0, class(py, "music21.instrument", "Conductor")?.call0()?),
    )?;

    for klass in ["MetronomeMark", "TimeSignature", "KeySignature"] {
        let mut last = -1.0;
        for element in all_of(s, klass)? {
            let offset: f64 = element
                .call_method1("getOffsetInHierarchy", (s,))?
                .extract()?;
            if offset > last {
                conductor.call_method1("insert", (offset, &element))?;
            }
            last = offset;
        }
        for held in streams_within(s)? {
            held.call_method1("removeByClass", (klass,))?;
        }
    }

    // What every conductor says where the score does not.
    if !conductor
        .call_method1("getElementsByClass", ("MetronomeMark",))?
        .is_truthy()?
    {
        let keywords = PyDict::new(py);
        keywords.set_item("number", 120)?;
        let mark = class(py, "music21.tempo", "MetronomeMark")?.call((), Some(&keywords))?;
        conductor.call_method1("insert", (0, mark))?;
    }
    if !conductor
        .call_method1("getElementsByClass", ("TimeSignature",))?
        .is_truthy()?
    {
        let meter = class(py, "music21.meter", "TimeSignature")?.call1(("4/4",))?;
        conductor.call_method1("insert", (0, meter))?;
    }
    Ok(conductor)
}

/// A copy of a score ready to write: repeats played out, a conductor part
/// first, volumes realized, and parts throughout.
#[pyfunction]
fn prepareStreamForMidi<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let s = if all_of(s, "Measure")?.is_empty() {
        py.import("copy")?.call_method1("deepcopy", (s,))?
    } else {
        s.call_method0("expandRepeats")?
    };
    let conductor = conductorStream(py, &s)?;
    let realize = class(py, "music21.volume", "realizeVolume")?;
    if s.call_method0("hasPartLikeStreams")?.is_truthy()? {
        for part in s.call_method1("getElementsByClass", ("Stream",))?.walk()? {
            realize.call1((part?,))?;
        }
        s.call_method1("insert", (0, conductor))?;
        return Ok(s);
    }
    realize.call1((&s,))?;
    let out = class(py, "music21.stream", "Score")?.call0()?;
    out.call_method1("insert", (0, conductor))?;
    out.call_method1("insert", (0, s))?;
    Ok(out)
}

/// Copies of the conductor's meters, keys and tempos into `target`, the
/// tempos hidden and implicit unless it is the first part.
#[pyfunction]
#[pyo3(signature = (conductorPart, target, *, isFirst = false))]
fn insertConductorEvents<'py>(
    py: Python<'py>,
    conductorPart: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] isFirst: bool,
) -> PyResult<()> {
    let classes = PyTuple::new(py, ["TimeSignature", "KeySignature", "MetronomeMark"])?;
    let copy = py.import("copy")?.getattr("deepcopy")?;
    for element in conductorPart
        .call_method1("getElementsByClass", (classes,))?
        .walk()?
    {
        let element = element?;
        let copied = copy.call1((&element,))?;
        if is_a(&copied, "TempoIndication")? && !isFirst {
            copied
                .getattr("style")?
                .setattr("hideObjectOnPrint", true)?;
            copied.setattr("numberImplicit", true)?;
        }
        let offset = conductorPart.call_method1("elementOffset", (&element,))?;
        target.call_method1("insert", (offset, copied))?;
    }
    Ok(())
}

/// Which channel each program plays on, and the channels left to bend
/// microtones on, for a prepared score.
#[pyfunction]
#[pyo3(signature = (s, acceptableChannelList = None))]
fn channelInstrumentData<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    acceptableChannelList: Option<Vec<u8>>,
) -> PyResult<Bound<'py, PyTuple>> {
    // The parts, a conductor of no notes taking no channel.
    let mut parts = Vec::new();
    if s.call_method0("hasPartLikeStreams")?.is_truthy()? {
        for part in s.call_method1("getElementsByClass", ("Stream",))?.walk()? {
            let part = part?;
            if all_of(&part, "GeneralNote")?.is_empty() && !all_of(&part, "Conductor")?.is_empty() {
                continue;
            }
            parts.push(part);
        }
    } else {
        parts.push(s.clone());
    }

    let mut played = Vec::new();
    let mut objects = Vec::new();
    for part in &parts {
        let instruments = all_of(part, "Instrument")?;
        let mut each = Vec::new();
        for instrument in &instruments {
            each.push(raw::Played {
                program: instrument.getattr("midiProgram")?.extract()?,
                channel: instrument.getattr("midiChannel")?.extract()?,
            });
        }
        played.push(each);
        objects.push(instruments);
    }
    let plan = raw::plan_channels(&played, acceptableChannelList.as_deref());
    for misplaced in &plan.misplaced {
        let shown = objects[misplaced.part][misplaced.instrument].str()?;
        warn(
            py,
            &format!(
                "{shown} specified 1-indexed MIDI channel {} but acceptable channels were {:?}. \
                 Defaulting to channel 1.",
                misplaced.channel, misplaced.acceptable
            ),
        )?;
    }
    let by_program = PyDict::new(py);
    for (program, channel) in &plan.by_program {
        by_program.set_item(program, channel)?;
    }
    PyTuple::new(
        py,
        [
            by_program.into_any(),
            PyList::new(py, &plan.dynamic)?.into_any(),
        ],
    )
}

/// A flattened, sorted stream's events as packets, sorted by tick and
/// then as events at one tick sort.
#[pyfunction]
#[pyo3(signature = (s, trackId = 1, addStartDelay = false, encoding = UTF8))]
fn streamToPackets<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    trackId: i64,
    #[pyo3(from_py_with = crate::truthy)] addStartDelay: bool,
    encoding: &str,
) -> PyResult<Bound<'py, PyList>> {
    let delay = if addStartDelay {
        StartDelay::Quarter
    } else {
        StartDelay::Without
    };
    let mut packets: Vec<(i64, i64, Bound<'py, PyDict>)> = Vec::new();
    let mut last_instrument: Option<Bound<'py, PyAny>> = None;
    for element in s.walk()? {
        let element = element?;
        let events = elementToMidiEventList(py, &element, encoding)?;
        if is_a(&element, "Instrument")? {
            last_instrument = Some(element.clone());
        }
        let Some(events) = events else {
            continue;
        };
        let offset: f64 = s.call_method1("elementOffset", (&element,))?.extract()?;
        let length = ticks_of(&element.getattr("duration")?)?;

        // What comes before the first note-on is not delayed.
        let mut sounded = false;
        for event in events.iter() {
            let kind = type_of(&event)?;
            sounded |= kind == Some(EventType::ChannelVoice(0x90));
            let at = raw::offset_ticks(offset, if sounded { delay } else { StartDelay::Without });
            let at = if kind == Some(EventType::ChannelVoice(NOTE_OFF)) {
                at + length
            } else {
                at
            };
            let order: i64 = event.getattr("sortOrder")?.extract()?;
            let packet = getPacketFromMidiEvent(
                py,
                trackId.into_pyobject(py)?.into_any(),
                at.into_pyobject(py)?.into_any(),
                event,
                Some(element.clone()),
                last_instrument.clone(),
            )?;
            packets.push((at, order, packet));
        }
    }
    packets.sort_by_key(|(at, order, _)| (*at, *order));
    PyList::new(py, packets.into_iter().map(|(_, _, packet)| packet))
}

/// Each part's packets and the instrument it starts with, by track.
#[pyfunction]
#[pyo3(signature = (substreamList, *, addStartDelay = false, encoding = UTF8))]
fn packetStorageFromSubstreamList<'py>(
    py: Python<'py>,
    substreamList: &Bound<'py, PyAny>,
    #[pyo3(from_py_with = crate::truthy)] addStartDelay: bool,
    encoding: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let storage = PyDict::new(py);
    for (track, part) in substreamList.walk()?.enumerate() {
        let flat = part?.call_method0("flatten")?;
        let mut first = flat
            .call_method1("getElementsByClass", ("Instrument",))?
            .call_method0("first")?;

        // An instrument not at the start or naming no program stands for
        // nothing; strokes call for percussion, a first part of no notes
        // for the conductor.
        let stands = !first.is_none()
            && flat
                .call_method1("elementOffset", (&first,))?
                .extract::<f64>()?
                == 0.0
            && !first.getattr("midiProgram")?.is_none();
        if !stands {
            let strokes = PyTuple::new(py, ["Unpitched", "PercussionChord"])?;
            if flat
                .call_method1("getElementsByClass", (strokes,))?
                .is_truthy()?
            {
                first = class(py, "music21.instrument", "UnpitchedPercussion")?.call0()?;
            } else if track == 0 && !flat.getattr("notesAndRests")?.is_truthy()? {
                first = class(py, "music21.instrument", "Conductor")?.call0()?;
            }
        }
        let packets = streamToPackets(
            py,
            &flat,
            i64::try_from(track).unwrap_or(i64::MAX),
            addStartDelay,
            encoding,
        )?;
        let bundle = PyDict::new(py);
        bundle.set_item("rawPackets", packets)?;
        bundle.set_item("initInstrument", first)?;
        storage.set_item(track, bundle)?;
    }
    Ok(storage)
}

/// Puts each bundle's first channel on it and on each of its packets:
/// percussion on the tenth, the conductor on none, any other by its
/// program.
#[pyfunction]
fn updatePacketStorageWithChannelInfo(
    packetStorage: &Bound<'_, PyAny>,
    channelByInstrument: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = packetStorage.py();
    for bundle in packetStorage.call_method0("values")?.walk()? {
        let bundle = bundle?;
        let first = bundle.get_item("initInstrument")?;
        let channel = if first.is_none() {
            channelByInstrument.call_method1("get", (py.None(), 1))?
        } else if is_a(&first, "UnpitchedPercussion")? {
            PERCUSSION_CHANNEL.into_pyobject(py)?.into_any()
        } else if is_a(&first, "Conductor")? {
            py.None().into_bound(py)
        } else {
            channelByInstrument.get_item(first.getattr("midiProgram")?)?
        };
        bundle.set_item("initChannel", &channel)?;
        for packet in bundle.get_item("rawPackets")?.walk()? {
            packet?.set_item("initChannel", &channel)?;
        }
    }
    Ok(())
}

/// Packets' events with the delta times between them, each delta time on
/// its event's channel.
#[pyfunction]
fn packetsToDeltaSeparatedEvents<'py>(
    py: Python<'py>,
    packets: &Bound<'py, PyAny>,
    midiTrack: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let delta = midibase::module(py)?.getattr("DeltaTime")?;
    let out = PyList::empty(py);
    let mut last: i64 = 0;
    for packet in packets.walk()? {
        let packet = packet?;
        let event = packet.get_item("midiEvent")?;
        let offset: i64 = packet.get_item("offset")?.extract()?;
        if offset < last {
            return Err(TranslateException::new_err("got a negative delta time"));
        }
        let keywords = PyDict::new(py);
        keywords.set_item("time", offset - last)?;
        keywords.set_item("channel", event.getattr("channel")?)?;
        out.append(delta.call((midiTrack,), Some(&keywords))?)?;
        out.append(event)?;
        last = offset;
    }
    Ok(out)
}

/// One track's packets as a `MidiTrack`, between its start and end events.
#[pyfunction]
#[pyo3(signature = (packets, trackId = 1, channel = 1, instrumentObj = None, *, addEndDelay = true))]
fn packetsToMidiTrack<'py>(
    py: Python<'py>,
    packets: Bound<'py, PyAny>,
    trackId: i64,
    channel: u8,
    instrumentObj: Option<Bound<'py, PyAny>>,
    #[pyo3(from_py_with = crate::truthy)] addEndDelay: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let track = midibase::module(py)?
        .getattr("MidiTrack")?
        .call1((&trackId,))?;
    let events = track.getattr("events")?;
    let start = getStartEvents(py, Some(track.clone()), channel, instrumentObj, UTF8)?;
    events.call_method1("extend", (start,))?;
    let wanted = trackId.into_pyobject(py)?.into_any();
    let own = filterPacketsByTrackId(py, packets, Some(wanted))?;
    events.call_method1(
        "extend",
        (packetsToDeltaSeparatedEvents(py, &own, &track)?,),
    )?;
    events.call_method1(
        "extend",
        (getEndEvents(py, Some(track.clone()), addEndDelay)?,),
    )?;
    track.call_method0("updateEvents")?;
    Ok(track)
}

/// The music21 module `midi.translate`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        for (exception, name) in [
            (py.get_type::<TranslateException>(), "TranslateException"),
            (py.get_type::<TranslateWarning>(), "TranslateWarning"),
        ] {
            exception.setattr("__module__", MODULE)?;
            m.add(name, exception)?;
        }

        // A named tuple, as music21's is.
        let timed = py
            .import("collections")?
            .getattr("namedtuple")?
            .call1(("TimedNoteEvent", ("onTime", "offTime", "event")))?;
        timed.setattr("__module__", MODULE)?;
        m.add("TimedNoteEvent", timed)?;

        macro_rules! add {
            ($($name:ident),* $(,)?) => {
                $(m.add_function(wrap_pyfunction!($name, &m)?)?;)*
            };
        }
        add!(
            offsetToMidiTicks,
            durationToMidiTicks,
            ticksToDuration,
            getStartEvents,
            getEndEvents,
            music21ObjectToMidiFile,
            midiEventsToNote,
            noteToMidiEvents,
            midiEventsToChord,
            chordToMidiEvents,
            instrumentToMidiEvents,
            midiEventToInstrument,
            midiEventsToTimeSignature,
            timeSignatureToMidiEvents,
            midiEventsToKey,
            keySignatureToMidiEvents,
            midiEventsToTempo,
            tempoToMidiEvents,
            getPacketFromMidiEvent,
            elementToMidiEventList,
            filterPacketsByTrackId,
            getTimeForEvents,
            getNotesFromEvents,
            lyricTimingsFromEvents,
            getMetaEvents,
            midiTrackToStream,
            midiTracksToStreams,
            streamToMidiFile,
            streamHierarchyToMidiTracks,
            midiFilePathToStream,
            midiAsciiStringToBinaryString,
            midiStringToStream,
            midiFileToStream,
            conductorStream,
            prepareStreamForMidi,
            insertConductorEvents,
            channelInstrumentData,
            streamToPackets,
            packetStorageFromSubstreamList,
            updatePacketStorageWithChannelInfo,
            packetsToDeltaSeparatedEvents,
            packetsToMidiTrack,
        );
        let mapper = py
            .import("music21_rs")
            .or_else(|_| py.import("music21_rs_facade"))?
            .getattr("PercussionMapper")?
            .call0()?;
        m.add("PERCUSSION_MAPPER", mapper)?;
        let environment = crate::environment::module(py)?.getattr("Environment")?;
        m.add("environLocal", environment.call1(("midi.translate",))?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
