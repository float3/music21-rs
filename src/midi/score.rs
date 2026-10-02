//! A MIDI file read as a score: music21's `midi` reader and
//! `midi.translate.midiFileToStream`.
//!
//! Every track holding notes becomes a part; the tracks holding none are
//! read together as the conductor, whose meters, keys and tempos each part
//! after them takes a copy of. A part's notes are gathered into chords where
//! they start together and end together, moved onto the nearest sixteenth or
//! triplet eighth, cut into measures, put into voices where they overlap,
//! tied across barlines and filled out with rests.

use std::collections::HashMap;

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::instrument::{Instrument, SearchLanguage};
use crate::key::KeySignature;
use crate::makenotation::{
    QUANTIZATION_DIVISORS, make_measures_by, make_rests, make_ties, make_voices, op_frac, quantize,
    sorted_events,
};
use crate::meter::TimeSignature;
use crate::note::Note;
use crate::percussion::{PercussionChord, PercussionNote, Unpitched};
use crate::pitch::Pitch;
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::MetronomeMark;
use crate::volume::Volume;

fn midi_error(message: impl Into<String>) -> Error {
    Error::Midi(message.into())
}

/// What an event is, as far as a score cares.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    NoteOn,
    NoteOff,
    ProgramChange,
    /// Any other channel message.
    Channel,
    /// A meta event, with its type byte.
    Meta(u8),
    SysEx,
    /// What is left when the data ran out mid-event.
    Nothing,
}

/// One event of a track: music21's `MidiEvent`.
#[derive(Clone, Debug)]
struct Event {
    kind: Kind,
    /// Counted from one.
    channel: u8,
    /// The pitch of a note, or the program of a program change.
    first: u8,
    /// The velocity of a note.
    second: u8,
    data: Vec<u8>,
}

impl Event {
    fn is_note_on(&self) -> bool {
        self.kind == Kind::NoteOn && self.second != 0
    }

    fn is_note_off(&self) -> bool {
        self.kind == Kind::NoteOff || (self.kind == Kind::NoteOn && self.second == 0)
    }
}

const SEQUENCE_TRACK_NAME: u8 = 0x03;
const INSTRUMENT_NAME: u8 = 0x04;
const LYRIC: u8 = 0x05;
const SET_TEMPO: u8 = 0x51;
const TIME_SIGNATURE: u8 = 0x58;
const KEY_SIGNATURE: u8 = 0x59;

/// A number of so many bytes, high byte first, and what follows it.
fn number(bytes: &[u8], length: usize) -> Result<(u64, &[u8])> {
    if bytes.len() < length {
        return Err(midi_error("the MIDI data ends inside a number"));
    }
    let value = bytes[..length]
        .iter()
        .fold(0_u64, |total, byte| (total << 8) | u64::from(*byte));
    Ok((value, &bytes[length..]))
}

/// A number written seven bits to a byte, and what follows it.
fn variable_length(bytes: &[u8]) -> (u64, &[u8]) {
    let mut value: u64 = 0;
    for (index, byte) in bytes.iter().enumerate() {
        value = (value << 7) | u64::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return (value, &bytes[index + 1..]);
        }
    }
    (value, &[])
}

/// One event read off the front of a track's data: music21's
/// `MidiEvent.read`. `status` is the running status, carried from the event
/// before. Nothing is answered for a byte that starts no event music21
/// knows.
fn read_event<'a>(bytes: &'a [u8], status: &mut Option<u8>) -> Option<(Event, &'a [u8])> {
    let mut event = Event {
        kind: Kind::Nothing,
        channel: 0,
        first: 0,
        second: 0,
        data: Vec::new(),
    };
    if bytes.len() < 2 {
        return Some((event, &[]));
    }
    // A data byte where a status byte should be runs on under the last
    // status.
    let (first, rest): (u8, &[u8]) = if bytes[0] < 0x80 {
        (status.unwrap_or(0x90), bytes)
    } else {
        if bytes[0] != 0xFF {
            *status = Some(bytes[0]);
        }
        (bytes[0], &bytes[1..])
    };
    let message = first & 0xF0;
    if (0x80..=0xE0).contains(&message) {
        event.channel = (first & 0x0F) + 1;
        let one = *rest.first()?;
        let two = rest.get(1).copied().unwrap_or(0);
        return match message {
            0xC0 | 0xD0 => {
                if one > 127 {
                    return None;
                }
                event.kind = if message == 0xC0 {
                    Kind::ProgramChange
                } else {
                    Kind::Channel
                };
                event.first = one;
                Some((event, &rest[1..]))
            }
            0x80 | 0x90 => {
                event.kind = if message == 0x90 {
                    Kind::NoteOn
                } else {
                    Kind::NoteOff
                };
                event.first = one;
                event.second = two;
                Some((event, rest.get(2..).unwrap_or(&[])))
            }
            _ => {
                event.kind = Kind::Channel;
                Some((event, rest.get(2..).unwrap_or(&[])))
            }
        };
    }
    if first == 0xF0 || first == 0xF7 {
        let (length, after) = variable_length(rest);
        let length = (length as usize).min(after.len());
        event.kind = Kind::SysEx;
        event.data = after[..length].to_vec();
        return Some((event, &after[length..]));
    }
    if first == 0xFF {
        let kind = *rest.first()?;
        let (length, after) = variable_length(&rest[1..]);
        let length = (length as usize).min(after.len());
        event.kind = Kind::Meta(kind);
        event.data = after[..length].to_vec();
        return Some((event, &after[length..]));
    }
    None
}

/// A track: its events, each with the tick it happens at.
type Track = Vec<(u64, Event)>;

/// A track's events, each with the tick it happens at: music21's
/// `processDataToEvents` and `getTimeForEvents`.
fn track_events(mut data: &[u8]) -> Track {
    let mut events = Vec::new();
    let mut time: u64 = 0;
    let mut status: Option<u8> = None;
    while !data.is_empty() {
        let (delta, after) = variable_length(data);
        match read_event(after, &mut status) {
            Some((event, rest)) => {
                time += delta;
                events.push((time, event));
                data = rest;
            }
            // What starts no event is passed over, its time with it.
            None => data = after,
        }
    }
    events
}

/// A file's tracks and how many ticks a quarter takes: music21's
/// `MidiFile.readstr`.
fn read_file(bytes: &[u8]) -> Result<(Vec<Track>, FloatType)> {
    if bytes.get(..4) != Some(b"MThd") {
        return Err(midi_error("badly formatted midi bytes: no MThd header"));
    }
    let (length, rest) = number(&bytes[4..], 4)?;
    if length != 6 {
        return Err(midi_error("badly formatted midi bytes"));
    }
    let (format, rest) = number(rest, 2)?;
    if format > 1 {
        return Err(midi_error(format!(
            "cannot handle midi file format: {format}"
        )));
    }
    let (count, rest) = number(rest, 2)?;
    let (division, mut rest) = number(rest, 2)?;
    if division & 0x8000 != 0 {
        return Err(midi_error(
            "a file timed in frames a second, not ticks a quarter, is not read",
        ));
    }
    let ticks = (division & 0x7FFF) as FloatType;
    if ticks == 0.0 {
        return Err(midi_error("a quarter of no ticks"));
    }
    let mut tracks = Vec::new();
    for _ in 0..count {
        if rest.get(..4) != Some(b"MTrk") {
            return Err(midi_error("badly formed midi string: missing leading MTrk"));
        }
        let (length, after) = number(&rest[4..], 4)?;
        let length = (length as usize).min(after.len());
        tracks.push(track_events(&after[..length]));
        rest = &after[length..];
    }
    Ok((tracks, ticks))
}

/// A note as the file times it: when it starts, when it stops, and the
/// event that started it.
struct Timed<'a> {
    on: u64,
    off: u64,
    event: &'a Event,
}

/// music21's `getNotesFromEvents`: each note-on with the note-off after it.
/// Read backwards, so a note-on takes the nearest note-off of its pitch and
/// channel after it, and one with none after it is dropped.
fn notes_of(events: &[(u64, Event)]) -> Vec<Timed<'_>> {
    let mut waiting: HashMap<(u8, u8), u64> = HashMap::new();
    let mut notes = Vec::new();
    for (time, event) in events.iter().rev() {
        if event.is_note_off() {
            waiting.insert((event.first, event.channel), *time);
        } else if event.is_note_on()
            && let Some(off) = waiting.get(&(event.first, event.channel))
        {
            notes.push(Timed {
                on: *time,
                off: *off,
                event,
            });
        }
    }
    notes.reverse();
    notes
}

/// The instrument a percussion key plays: music21's `PercussionMapper`.
fn percussion_instrument(pitch: u8) -> Instrument {
    let class = match pitch {
        35 | 36 => "BassDrum",
        37 | 38 | 40 => "SnareDrum",
        41 | 43 | 45 | 47 | 48 | 50 => "TomTom",
        42 | 44 | 46 => "HiHatCymbal",
        49 | 57 => "CrashCymbals",
        54 => "Tambourine",
        56 => "Cowbell",
        58 => "Vibraslap",
        60 | 61 => "BongoDrums",
        62..=64 => "CongaDrum",
        65 | 66 => "Timbales",
        67 | 68 => "Agogo",
        70 => "Maracas",
        71 | 72 => "Whistle",
        76 | 77 => "Woodblock",
        80 | 81 => "Triangle",
        _ => "UnpitchedPercussion",
    };
    Instrument::of_kind(class).unwrap_or_default()
}

/// music21's `midiEventToInstrument`.
fn instrument_of(event: &Event) -> Instrument {
    let mut decoded = String::new();
    let mut instrument = match event.kind {
        Kind::ProgramChange if event.channel == 10 => {
            let mut made = Instrument::of_kind("UnpitchedPercussion").unwrap_or_default();
            made.set_midi_program(Some(event.first));
            made
        }
        Kind::ProgramChange => {
            Instrument::from_midi_program(event.first).unwrap_or_else(|_| Instrument::new())
        }
        _ => match std::str::from_utf8(&event.data) {
            Ok(text) => {
                decoded = text.split('\0').next().unwrap_or("").trim().to_string();
                Instrument::from_name(&decoded, SearchLanguage::All)
                    .unwrap_or_else(|_| Instrument::new())
            }
            Err(_) => Instrument::new(),
        },
    };
    // A meta event is on no channel, which music21 counts as the one before
    // the first.
    instrument.set_midi_channel(event.channel.checked_sub(1));
    if !decoded.is_empty() {
        let lower = decoded.to_lowercase();
        let numbered = |prefix: &str| {
            let rest = lower.replace(prefix, "");
            !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
        };
        if lower == "instrument" || lower == "inst" || numbered("instrument ") || numbered("inst ")
        {
            return instrument;
        }
        match event.kind {
            Kind::Meta(SEQUENCE_TRACK_NAME) => instrument.set_part_name(Some(decoded)),
            Kind::Meta(INSTRUMENT_NAME) => instrument.set_name(Some(decoded)),
            _ => {}
        }
    }
    instrument
}

/// music21's `getMetaEvents`: the meters, keys, tempos and instruments a
/// track states, each with its tick.
fn meta_of(events: &[(u64, Event)]) -> Result<Vec<(u64, StreamElement)>> {
    let mut found = Vec::new();
    let mut last_program: Option<u8> = None;
    for (time, event) in events {
        let element: Option<StreamElement> = match event.kind {
            Kind::Meta(TIME_SIGNATURE) if event.data.len() >= 2 => {
                let denominator = 2_u32
                    .checked_pow(u32::from(event.data[1]))
                    .ok_or_else(|| midi_error("a time signature over too large a value"))?;
                Some(TimeSignature::new(u32::from(event.data[0]), denominator)?.into())
            }
            Kind::Meta(KEY_SIGNATURE) if event.data.len() >= 2 => {
                let sharps = if event.data[0] > 12 {
                    IntegerType::from(event.data[0]) - 256
                } else {
                    IntegerType::from(event.data[0])
                };
                let mode = if event.data[1] == 1 { "minor" } else { "major" };
                Some(StreamElement::Key(KeySignature::new(sharps).as_key(mode)))
            }
            Kind::Meta(SET_TEMPO) if event.data.len() >= 3 => {
                let (micros, _) = number(&event.data, 3)?;
                if micros == 0 {
                    return Err(midi_error("a tempo of no time to the quarter"));
                }
                let bpm = (60_000_000.0 / micros as FloatType * 100.0).round_ties_even() / 100.0;
                Some(MetronomeMark::new(bpm).into())
            }
            Kind::Meta(INSTRUMENT_NAME | SEQUENCE_TRACK_NAME) => {
                let mut instrument = instrument_of(event);
                if let Some(program) = last_program {
                    instrument.set_midi_program(Some(program));
                }
                Some(instrument.into())
            }
            Kind::ProgramChange => {
                last_program = Some(event.first);
                Some(instrument_of(event).into())
            }
            _ => None,
        };
        if let Some(element) = element {
            found.push((*time, element));
        }
    }
    Ok(found)
}

/// music21's `instrument.deduplicate` over the events of one part: where
/// every instrument it holds is of one kind they are made one, the first,
/// and a generic one among others of a kind goes. music21 gathers the
/// instruments of the whole part for this and not only those standing
/// together, so a program change after a track name is folded into the
/// instrument the name made.
fn deduplicate(events: &mut Vec<StreamEvent>) {
    let places: Vec<usize> = (0..events.len())
        .filter(|index| matches!(events[*index].element(), StreamElement::Instrument(_)))
        .collect();
    let held: Vec<&Instrument> = places
        .iter()
        .filter_map(|index| match events[*index].element() {
            StreamElement::Instrument(instrument) => Some(instrument.as_ref()),
            _ => None,
        })
        .collect();
    let Some(settled) = crate::instrument::settle(&held) else {
        return;
    };
    for (place, becomes) in places.into_iter().zip(settled).rev() {
        match becomes {
            Some(named) => events[place] = StreamEvent::new(events[place].offset(), named),
            None => {
                events.remove(place);
            }
        }
    }
}

/// The length so many ticks last, or a grace note's where they are none.
fn duration_of(ticks: u64, per_quarter: FloatType) -> Result<Duration> {
    if ticks == 0 {
        return Ok(Duration::quarter().grace_duration());
    }
    Duration::new(op_frac(ticks as FloatType / per_quarter))
}

fn volume_of(velocity: u8) -> Volume {
    let mut volume = Volume::from_velocity(IntegerType::from(velocity));
    volume.set_velocity_is_relative(false);
    volume
}

/// A stroke of the instrument a percussion key plays.
fn stroke_of(pitch: u8) -> Unpitched {
    let mut stroke = Unpitched::new();
    stroke
        .written_mut()
        .set_stored_instrument(Some(percussion_instrument(pitch)));
    stroke
}

/// One track as a part, or as more of the conductor where it has no notes:
/// music21's `midiTrackToStream`. The answer is whether the track had notes.
fn track_to_part(
    events: &[(u64, Event)],
    per_quarter: FloatType,
    conductor: &[StreamEvent],
    is_first: bool,
) -> Result<(Stream, bool)> {
    let notes = notes_of(events);
    let mut lyrics: HashMap<u64, String> = HashMap::new();
    for (time, event) in events {
        if event.kind == Kind::Meta(LYRIC)
            && let Ok(text) = std::str::from_utf8(&event.data)
        {
            lyrics.insert(*time, text.to_string());
        }
    }

    let mut held: Vec<StreamEvent> = Vec::new();
    for (tick, element) in meta_of(events)? {
        held.push(StreamEvent::new(
            op_frac(tick as FloatType / per_quarter),
            element,
        ));
    }
    deduplicate(&mut held);

    // Notes starting together and ending together are one chord; ones that
    // start together and end apart need voices.
    let tolerance =
        per_quarter / FloatType::from(QUANTIZATION_DIVISORS.into_iter().max().unwrap_or(4));
    let mut gathered = vec![false; notes.len()];
    let mut voices_required = false;
    for index in 0..notes.len() {
        if gathered[index] {
            continue;
        }
        let start = notes[index].on;
        let stop = notes[index].off;
        let mut members = vec![index];
        for other in index + 1..notes.len() {
            if (notes[other].on as FloatType - start as FloatType).abs() >= tolerance {
                break;
            }
            if (notes[other].off as FloatType - stop as FloatType).abs() > tolerance {
                voices_required = true;
                continue;
            }
            members.push(other);
            gathered[other] = true;
        }
        let lyric = lyrics.get(&start);
        let element: StreamElement = if members.len() > 1 {
            // The chord is as long as the last note gathered.
            let last = &notes[members[members.len() - 1]];
            let duration = duration_of(last.off - last.on, per_quarter)?;
            let percussion = members
                .iter()
                .any(|member| notes[*member].event.channel == 10);
            if percussion {
                let strokes: Vec<PercussionNote> = members
                    .iter()
                    .map(|member| PercussionNote::Unpitched(stroke_of(notes[*member].event.first)))
                    .collect();
                let mut chord = PercussionChord::new(strokes)?.with_duration(duration);
                let volumes: Vec<Volume> = members
                    .iter()
                    .map(|member| volume_of(notes[*member].event.second))
                    .collect();
                chord.written_mut().set_volumes(&volumes)?;
                chord.into()
            } else {
                let mut pitched = Vec::new();
                for member in &members {
                    let event = notes[*member].event;
                    let mut note =
                        Note::from_pitch(Pitch::from_midi(IntegerType::from(event.first))?);
                    note.set_volume(Some(volume_of(event.second)));
                    pitched.push(note);
                }
                let mut chord = Chord::new(pitched)?.with_duration(duration);
                // A chord's lyrics are kept on its first note.
                if let (Some(lyric), Some(singer)) = (lyric, chord.notes_mut().first_mut()) {
                    singer.set_lyric(Some(lyric))?;
                }
                chord.into()
            }
        } else {
            let event = notes[index].event;
            let duration = duration_of(stop - start, per_quarter)?;
            if event.channel == 10 {
                let mut stroke = stroke_of(event.first).with_duration(duration);
                stroke
                    .written_mut()
                    .set_volume(Some(volume_of(event.second)));
                if let Some(lyric) = lyric {
                    stroke.written_mut().set_lyric(Some(lyric))?;
                }
                stroke.into()
            } else {
                let mut note = Note::from_pitch(Pitch::from_midi(IntegerType::from(event.first))?)
                    .with_duration(duration);
                note.set_volume(Some(volume_of(event.second)));
                if let Some(lyric) = lyric {
                    note.set_lyric(Some(lyric))?;
                }
                note.into()
            }
        };
        held.push(StreamEvent::new(
            op_frac(start as FloatType / per_quarter),
            element,
        ));
    }

    let mut part = Stream::with_kind(StreamKind::Part).with_events(sorted_events(held));
    quantize(&mut part, &QUANTIZATION_DIVISORS)?;
    if notes.is_empty() {
        return Ok((part, false));
    }

    // The conductor's meters, keys and tempos, a copy for each part; only
    // the first part shows the tempos.
    let mut events = part.events().to_vec();
    let mut meters: Vec<(FloatType, TimeSignature)> = Vec::new();
    for event in conductor {
        match event.element() {
            StreamElement::TimeSignature(meter) => {
                meters.push((event.offset(), meter.clone()));
                events.push(event.clone());
            }
            StreamElement::Key(_) | StreamElement::KeySignature(_) => events.push(event.clone()),
            StreamElement::MetronomeMark(mark) => {
                let mut mark = mark.clone();
                if !is_first {
                    mark.set_number_implicit(true);
                }
                events.push(StreamEvent::new(event.offset(), mark));
            }
            _ => {}
        }
    }
    if !meters.is_empty() && !meters.iter().any(|(offset, _)| *offset == 0.0) {
        meters.push((0.0, TimeSignature::new(4, 4)?));
    }
    let part = part.with_events(sorted_events(events));

    let mut part = make_measures_by(&part, &meters)?;
    if voices_required {
        let mut events = part.events().to_vec();
        for event in &mut events {
            if let StreamElement::Stream(measure) = event.element_mut()
                && measure.kind() == StreamKind::Measure
            {
                make_voices(measure);
            }
        }
        part = part.with_events(events);
    }
    make_ties(&mut part)?;
    make_rests(&mut part)?;
    Ok((part, true))
}

/// Reads a standard MIDI file into a score: music21's
/// `converter.parse` of one.
///
/// Every track with notes is a part. Notes are moved onto the nearest
/// sixteenth or triplet eighth, gathered into chords and voices, cut into
/// measures by the meters the file states (`4/4` where it states none),
/// tied across barlines and filled out with rests. Channel 10 is read as
/// unpitched percussion, each key the instrument General MIDI gives it. A
/// file timed in frames a second is refused, as is a format 2 file.
///
/// The bytes are the file's own; the library opens no file.
pub fn from_midi(bytes: &[u8]) -> Result<Stream> {
    let (tracks, per_quarter) = read_file(bytes)?;
    if tracks.is_empty() {
        return Err(midi_error("no tracks are defined in this MIDI file."));
    }
    let mut conductor: Vec<StreamEvent> = Vec::new();
    let mut parts: Vec<StreamEvent> = Vec::new();
    let mut seen_notes = false;
    for track in &tracks {
        let has_notes = track.iter().any(|(_, event)| event.is_note_on());
        if has_notes {
            let is_first = !seen_notes;
            seen_notes = true;
            let (part, _) = track_to_part(track, per_quarter, &conductor, is_first)?;
            parts.push(StreamEvent::new(0.0, part));
        } else {
            // A track with no notes adds to the conductor, which is
            // quantized again as a whole each time.
            let (more, _) = track_to_part(track, per_quarter, &[], false)?;
            conductor.extend(more.events().iter().cloned());
            let mut all = Stream::new().with_events(sorted_events(conductor));
            let mut events = all.events().to_vec();
            deduplicate(&mut events);
            all = all.with_events(events);
            quantize(&mut all, &QUANTIZATION_DIVISORS)?;
            conductor = all.events().to_vec();
        }
    }
    Ok(Stream::with_kind(StreamKind::Score).with_events(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-track file: a tempo, a meter, then these notes as
    /// (delta, status, key, velocity).
    fn file(events: &[(u8, u8, u8, u8)]) -> Vec<u8> {
        let mut track: Vec<u8> = vec![
            0, 0xFF, 0x51, 3, 0x07, 0xA1, 0x20, // 120 to the minute
            0, 0xFF, 0x58, 4, 3, 2, 24, 8, // 3/4
        ];
        for (delta, status, key, velocity) in events {
            track.extend([*delta, *status, *key, *velocity]);
        }
        track.extend([0, 0xFF, 0x2F, 0]);
        let mut bytes = b"MThd\0\0\0\x06\0\0\0\x01\0\x60MTrk".to_vec();
        bytes.extend((track.len() as u32).to_be_bytes());
        bytes.extend(track);
        bytes
    }

    #[test]
    fn a_track_is_a_part_of_measures() {
        // Four quarters in 3/4, 96 ticks each: a bar and a bar filled out
        // with a rest.
        let mut events = Vec::new();
        for key in [60, 62, 64, 65] {
            events.push((0, 0x90, key, 80));
            events.push((96, 0x80, key, 0));
        }
        let score = from_midi(&file(&events)).unwrap();
        let part = score.parts()[0];
        let measures = part.measures();
        assert_eq!(measures.len(), 2);
        assert_eq!(measures[0].pitches().len(), 3);
        let last: Vec<FloatType> = measures[1]
            .events()
            .iter()
            .filter(|event| matches!(event.element(), StreamElement::Rest(_)))
            .map(|event| event.element().quarter_length())
            .collect();
        assert_eq!(last, [2.0]);
    }

    #[test]
    fn notes_together_are_a_chord_and_a_long_note_is_tied() {
        // A triad for a beat, then one note for a whole 3/4 bar and a beat.
        let events = [
            (0, 0x90, 60, 80),
            (0, 0x90, 64, 80),
            (0, 0x90, 67, 80),
            (96, 0x80, 60, 0),
            (0, 0x80, 64, 0),
            (0, 0x80, 67, 0),
            (0, 0x90, 72, 80),
            (127, 0x80, 72, 0),
        ];
        let mut bytes = file(&events[..7]);
        // The last note lasts 288 ticks, which takes two bytes to say.
        let end = bytes.len() - 4;
        bytes.splice(end..end, [0x82, 0x20, 0x80, 72, 0]);
        let length = (bytes.len() - 22) as u32;
        bytes[18..22].copy_from_slice(&length.to_be_bytes());
        let score = from_midi(&bytes).unwrap();
        let part = score.parts()[0];
        let measures = part.measures();
        assert!(matches!(
            measures[0]
                .events()
                .iter()
                .find(|event| event.element().is_note_or_chord())
                .map(StreamEvent::element),
            Some(StreamElement::Chord(_))
        ));
        let ties: Vec<_> = part
            .notes()
            .into_iter()
            .filter_map(|(_, element)| match element {
                StreamElement::Note(note) => note.tie().map(|tie| tie.tie_type()),
                _ => None,
            })
            .collect();
        assert_eq!(
            ties,
            [
                crate::notation::TieType::Start,
                crate::notation::TieType::Stop
            ]
        );
    }

    #[test]
    fn what_is_no_midi_file_is_refused() {
        assert!(from_midi(b"RIFF").is_err());
    }
}
