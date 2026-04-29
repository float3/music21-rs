use std::collections::BTreeMap;

use crate::{
    defaults::{FloatType, IntegerType},
    duration::Duration,
    error::{Error, Result},
    note::Note,
    pitch::Pitch,
    stream::{Stream, StreamElement},
};

/// Default MIDI pulses per quarter note used by the byte import/export helpers.
pub const DEFAULT_TICKS_PER_QUARTER: u16 = 480;

/// A note event in quarter-length time.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidiNote {
    /// MIDI key number, from 0 to 127.
    pub pitch: u8,
    /// MIDI note-on velocity, from 0 to 127.
    pub velocity: u8,
    /// MIDI channel, from 0 to 15.
    pub channel: u8,
    /// Start offset measured in quarter lengths.
    pub start: FloatType,
    /// Duration measured in quarter lengths.
    pub duration: FloatType,
}

impl MidiNote {
    /// Creates a MIDI note event.
    pub fn new(pitch: u8, start: FloatType, duration: FloatType, velocity: u8) -> Result<Self> {
        Self::with_channel(pitch, start, duration, velocity, 0)
    }

    /// Creates a MIDI note event with an explicit channel.
    pub fn with_channel(
        pitch: u8,
        start: FloatType,
        duration: FloatType,
        velocity: u8,
        channel: u8,
    ) -> Result<Self> {
        if pitch > 127 {
            return Err(Error::Midi(format!("MIDI pitch out of range: {pitch}")));
        }
        if velocity > 127 {
            return Err(Error::Midi(format!(
                "MIDI velocity out of range: {velocity}"
            )));
        }
        if channel > 15 {
            return Err(Error::Midi(format!("MIDI channel out of range: {channel}")));
        }
        if !start.is_finite() || start < 0.0 {
            return Err(Error::Midi(format!("invalid MIDI note start: {start}")));
        }
        if !duration.is_finite() || duration < 0.0 {
            return Err(Error::Midi(format!(
                "invalid MIDI note duration: {duration}"
            )));
        }

        Ok(Self {
            pitch,
            velocity,
            channel,
            start,
            duration,
        })
    }
}

/// Extracts MIDI note events from a stream.
pub fn midi_notes_from_stream(stream: &Stream) -> Result<Vec<MidiNote>> {
    let mut notes = Vec::new();
    for event in stream.events() {
        let start = event.offset();
        let duration = event.element().quarter_length();
        match event.element() {
            StreamElement::Note(note) => {
                notes.push(note_to_midi_note(note, start, duration)?);
            }
            StreamElement::Chord(chord) => {
                for note in chord.notes() {
                    notes.push(note_to_midi_note(note, start, duration)?);
                }
            }
            StreamElement::Rest(_) => {}
        }
    }
    Ok(notes)
}

/// Builds a stream from MIDI note events.
pub fn stream_from_midi_notes(notes: &[MidiNote]) -> Result<Stream> {
    let mut stream = Stream::new();
    for midi_note in notes {
        let note = Note::from_pitch(Pitch::from_midi(midi_note.pitch as IntegerType)?)?
            .with_duration(Duration::new(midi_note.duration)?);
        stream.insert(midi_note.start, note);
    }
    Ok(stream)
}

/// Writes a minimal format-0 Standard MIDI File.
pub fn write_midi_bytes(notes: &[MidiNote], tempo_bpm: FloatType) -> Result<Vec<u8>> {
    if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
        return Err(Error::Midi(format!("invalid tempo: {tempo_bpm}")));
    }

    let mut events = Vec::new();
    for note in notes {
        validate_note(*note)?;
        let start_tick = quarter_to_tick(note.start)?;
        let end_tick = quarter_to_tick(note.start + note.duration)?;
        events.push((
            start_tick,
            1_u8,
            [0x90 | note.channel, note.pitch, note.velocity],
        ));
        events.push((end_tick, 0_u8, [0x80 | note.channel, note.pitch, 0]));
    }
    events.sort_by_key(|event| (event.0, event.1));

    let mut track = Vec::new();
    write_vlq(0, &mut track);
    track.extend([0xFF, 0x51, 0x03]);
    let micros_per_quarter = (60_000_000.0 / tempo_bpm).round() as u32;
    track.extend([
        ((micros_per_quarter >> 16) & 0xFF) as u8,
        ((micros_per_quarter >> 8) & 0xFF) as u8,
        (micros_per_quarter & 0xFF) as u8,
    ]);

    let mut last_tick = 0_u32;
    for (tick, _, bytes) in events {
        write_vlq(tick.saturating_sub(last_tick), &mut track);
        track.extend(bytes);
        last_tick = tick;
    }
    write_vlq(0, &mut track);
    track.extend([0xFF, 0x2F, 0x00]);

    let mut out = Vec::new();
    out.extend(b"MThd");
    out.extend(6_u32.to_be_bytes());
    out.extend(0_u16.to_be_bytes());
    out.extend(1_u16.to_be_bytes());
    out.extend(DEFAULT_TICKS_PER_QUARTER.to_be_bytes());
    out.extend(b"MTrk");
    out.extend((track.len() as u32).to_be_bytes());
    out.extend(track);
    Ok(out)
}

/// Reads note events from a Standard MIDI File.
pub fn read_midi_bytes(bytes: &[u8]) -> Result<Vec<MidiNote>> {
    read_midi_bytes_with_tempo(bytes).map(|(notes, _tempo)| notes)
}

/// Reads note events and the first tempo marking from a Standard MIDI File.
pub fn read_midi_bytes_with_tempo(bytes: &[u8]) -> Result<(Vec<MidiNote>, Option<FloatType>)> {
    let mut pos = 0;
    expect(bytes, &mut pos, b"MThd")?;
    let header_len = read_u32(bytes, &mut pos)?;
    if header_len < 6 {
        return Err(Error::Midi("MIDI header is too short".to_string()));
    }
    let _format = read_u16(bytes, &mut pos)?;
    let tracks = read_u16(bytes, &mut pos)?;
    let division = read_u16(bytes, &mut pos)?;
    pos += (header_len - 6) as usize;

    if division & 0x8000 != 0 {
        return Err(Error::Midi(
            "SMPTE MIDI time division is not supported".to_string(),
        ));
    }

    let mut all_notes = Vec::new();
    let mut first_tempo = None;
    for _ in 0..tracks {
        expect(bytes, &mut pos, b"MTrk")?;
        let len = read_u32(bytes, &mut pos)? as usize;
        let end = pos
            .checked_add(len)
            .ok_or_else(|| Error::Midi("MIDI track length overflow".to_string()))?;
        if end > bytes.len() {
            return Err(Error::Midi("MIDI track exceeds file length".to_string()));
        }
        let (mut notes, tempo) = read_track(&bytes[pos..end], division)?;
        if first_tempo.is_none() {
            first_tempo = tempo;
        }
        all_notes.append(&mut notes);
        pos = end;
    }
    all_notes.sort_by(|left, right| {
        left.start
            .partial_cmp(&right.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok((all_notes, first_tempo))
}

fn note_to_midi_note(note: &Note, start: FloatType, duration: FloatType) -> Result<MidiNote> {
    let pitch = note.pitch().ps().round() as IntegerType;
    if !(0..=127).contains(&pitch) {
        return Err(Error::Midi(format!("pitch {pitch} is outside MIDI range")));
    }
    MidiNote::new(pitch as u8, start, duration, 64)
}

fn validate_note(note: MidiNote) -> Result<()> {
    MidiNote::with_channel(
        note.pitch,
        note.start,
        note.duration,
        note.velocity,
        note.channel,
    )
    .map(|_| ())
}

fn quarter_to_tick(value: FloatType) -> Result<u32> {
    if !value.is_finite() || value < 0.0 {
        return Err(Error::Midi(format!("invalid quarter offset: {value}")));
    }
    Ok((value * DEFAULT_TICKS_PER_QUARTER as FloatType).round() as u32)
}

fn tick_to_quarter(value: u32, division: u16) -> FloatType {
    value as FloatType / division as FloatType
}

fn write_vlq(mut value: u32, out: &mut Vec<u8>) {
    let mut buffer = [0_u8; 5];
    let mut idx = buffer.len() - 1;
    buffer[idx] = (value & 0x7F) as u8;
    value >>= 7;
    while value > 0 {
        idx -= 1;
        buffer[idx] = ((value & 0x7F) as u8) | 0x80;
        value >>= 7;
    }
    out.extend(&buffer[idx..]);
}

fn read_vlq(bytes: &[u8], pos: &mut usize) -> Result<u32> {
    let mut value = 0_u32;
    for _ in 0..4 {
        let byte = *bytes
            .get(*pos)
            .ok_or_else(|| Error::Midi("unexpected end of VLQ".to_string()))?;
        *pos += 1;
        value = (value << 7) | (byte & 0x7F) as u32;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(Error::Midi("VLQ is too long".to_string()))
}

fn read_track(track: &[u8], division: u16) -> Result<(Vec<MidiNote>, Option<FloatType>)> {
    let mut pos = 0;
    let mut tick = 0_u32;
    let mut running_status = None;
    let mut active: BTreeMap<(u8, u8), Vec<(u32, u8)>> = BTreeMap::new();
    let mut notes = Vec::new();
    let mut tempo = None;

    while pos < track.len() {
        tick = tick.saturating_add(read_vlq(track, &mut pos)?);
        let byte = *track
            .get(pos)
            .ok_or_else(|| Error::Midi("unexpected end of MIDI event".to_string()))?;
        let status = if byte & 0x80 != 0 {
            pos += 1;
            running_status = Some(byte);
            byte
        } else {
            running_status
                .ok_or_else(|| Error::Midi("running status without status byte".to_string()))?
        };

        match status {
            0xFF => {
                let meta_type = read_byte(track, &mut pos)?;
                let len = read_vlq(track, &mut pos)? as usize;
                if pos + len > track.len() {
                    return Err(Error::Midi("meta event exceeds track length".to_string()));
                }
                if meta_type == 0x51 && len == 3 {
                    let micros = ((track[pos] as u32) << 16)
                        | ((track[pos + 1] as u32) << 8)
                        | track[pos + 2] as u32;
                    tempo = Some(60_000_000.0 / micros as FloatType);
                } else if meta_type == 0x2F {
                    break;
                }
                pos += len;
            }
            0xF0 | 0xF7 => {
                let len = read_vlq(track, &mut pos)? as usize;
                pos = pos
                    .checked_add(len)
                    .ok_or_else(|| Error::Midi("sysex length overflow".to_string()))?;
                if pos > track.len() {
                    return Err(Error::Midi("sysex event exceeds track length".to_string()));
                }
            }
            _ => {
                let event_type = status & 0xF0;
                let channel = status & 0x0F;
                let data_len = match event_type {
                    0xC0 | 0xD0 => 1,
                    0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => 2,
                    _ => return Err(Error::Midi(format!("unsupported MIDI status {status:#X}"))),
                };
                let data1 = read_byte(track, &mut pos)?;
                let data2 = if data_len == 2 {
                    read_byte(track, &mut pos)?
                } else {
                    0
                };

                if event_type == 0x90 && data2 > 0 {
                    active
                        .entry((channel, data1))
                        .or_default()
                        .push((tick, data2));
                } else if (event_type == 0x80 || event_type == 0x90)
                    && let Some(stack) = active.get_mut(&(channel, data1))
                    && let Some((start_tick, velocity)) = stack.pop()
                {
                    notes.push(MidiNote::with_channel(
                        data1,
                        tick_to_quarter(start_tick, division),
                        tick_to_quarter(tick.saturating_sub(start_tick), division),
                        velocity,
                        channel,
                    )?);
                }
            }
        }
    }

    Ok((notes, tempo))
}

fn expect(bytes: &[u8], pos: &mut usize, expected: &[u8]) -> Result<()> {
    if bytes.get(*pos..(*pos).saturating_add(expected.len())) == Some(expected) {
        *pos += expected.len();
        Ok(())
    } else {
        Err(Error::Midi(format!(
            "expected MIDI chunk {:?}",
            String::from_utf8_lossy(expected)
        )))
    }
}

fn read_byte(bytes: &[u8], pos: &mut usize) -> Result<u8> {
    let byte = *bytes
        .get(*pos)
        .ok_or_else(|| Error::Midi("unexpected end of MIDI data".to_string()))?;
    *pos += 1;
    Ok(byte)
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> Result<u16> {
    let start = *pos;
    *pos += 2;
    let data = bytes
        .get(start..*pos)
        .ok_or_else(|| Error::Midi("unexpected end of MIDI u16".to_string()))?;
    Ok(u16::from_be_bytes([data[0], data[1]]))
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32> {
    let start = *pos;
    *pos += 4;
    let data = bytes
        .get(start..*pos)
        .ok_or_else(|| Error::Midi("unexpected end of MIDI u32".to_string()))?;
    Ok(u32::from_be_bytes([data[0], data[1], data[2], data[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midi_roundtrip_bytes() {
        let notes = vec![MidiNote::new(60, 0.0, 1.0, 90).unwrap()];
        let bytes = write_midi_bytes(&notes, 120.0).unwrap();
        assert!(bytes.starts_with(b"MThd"));
        let (roundtrip, tempo) = read_midi_bytes_with_tempo(&bytes).unwrap();
        assert_eq!(tempo, Some(120.0));
        assert_eq!(roundtrip, notes);
    }

    #[test]
    fn stream_converts_to_midi_notes() {
        let mut stream = Stream::new();
        stream.push(
            Note::from_name("C4")
                .unwrap()
                .with_duration(Duration::half()),
        );
        let notes = midi_notes_from_stream(&stream).unwrap();
        assert_eq!(notes[0].pitch, 60);
        assert_eq!(notes[0].duration, 2.0);
    }

    #[test]
    fn midi_note_validation_rejects_invalid_values() {
        assert!(MidiNote::with_channel(128, 0.0, 1.0, 64, 0).is_err());
        assert!(MidiNote::with_channel(60, 0.0, 1.0, 128, 0).is_err());
        assert!(MidiNote::with_channel(60, 0.0, 1.0, 64, 16).is_err());
        assert!(MidiNote::with_channel(60, -0.25, 1.0, 64, 0).is_err());
        assert!(MidiNote::with_channel(60, 0.0, FloatType::INFINITY, 64, 0).is_err());
        assert!(write_midi_bytes(&[], 0.0).is_err());
    }

    #[test]
    fn midi_reader_rejects_invalid_files() {
        assert!(read_midi_bytes(b"not midi").is_err());

        let mut short_header = Vec::new();
        short_header.extend(b"MThd");
        short_header.extend(4_u32.to_be_bytes());
        short_header.extend([0, 0, 0, 1]);
        assert!(read_midi_bytes(&short_header).is_err());

        let mut smpte = Vec::new();
        smpte.extend(b"MThd");
        smpte.extend(6_u32.to_be_bytes());
        smpte.extend(0_u16.to_be_bytes());
        smpte.extend(0_u16.to_be_bytes());
        smpte.extend(0x8000_u16.to_be_bytes());
        assert!(read_midi_bytes(&smpte).is_err());
    }
}
