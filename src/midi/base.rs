//! music21's `midi.base`: MIDI as bytes, below any score.
//!
//! The numbers MIDI writes (big-endian and seven bits to a byte), the kinds
//! of message, one event read off the front of a track's data or written
//! back, a track's chunk, and a file's header. What an event holds is kept
//! as music21 keeps it: a type, a channel counted from one, and two
//! parameters, the first of which may be bytes.

use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

/// The byte a meta event starts with.
pub const METAEVENT_MARKER: u8 = 0xFF;

/// A track chunk's tag.
pub const TRACK_HEADER: &[u8; 4] = b"MTrk";

/// A file header's tag.
pub const FILE_HEADER: &[u8; 4] = b"MThd";

/// The messages of a channel, by music21's names: the high nibble of the
/// status byte.
pub const CHANNEL_VOICE_MESSAGES: &[(&str, u8)] = &[
    ("NOTE_OFF", 0x80),
    ("NOTE_ON", 0x90),
    ("POLYPHONIC_KEY_PRESSURE", 0xA0),
    ("CONTROLLER_CHANGE", 0xB0),
    ("PROGRAM_CHANGE", 0xC0),
    ("CHANNEL_KEY_PRESSURE", 0xD0),
    ("PITCH_BEND", 0xE0),
];

/// The controller numbers that set a channel's mode.
pub const CHANNEL_MODE_MESSAGES: &[(&str, u8)] = &[
    ("ALL_SOUND_OFF", 0x78),
    ("RESET_ALL_CONTROLLERS", 0x79),
    ("LOCAL_CONTROL", 0x7A),
    ("ALL_NOTES_OFF", 0x7B),
    ("OMNI_MODE_OFF", 0x7C),
    ("OMNI_MODE_ON", 0x7D),
    ("MONO_MODE_ON", 0x7E),
    ("POLY_MODE_ON", 0x7F),
];

/// The meta events, by the byte after [`METAEVENT_MARKER`]; `UNKNOWN`
/// stands for any other.
pub const META_EVENTS: &[(&str, u8)] = &[
    ("SEQUENCE_NUMBER", 0x00),
    ("TEXT_EVENT", 0x01),
    ("COPYRIGHT_NOTICE", 0x02),
    ("SEQUENCE_TRACK_NAME", 0x03),
    ("INSTRUMENT_NAME", 0x04),
    ("LYRIC", 0x05),
    ("MARKER", 0x06),
    ("CUE_POINT", 0x07),
    ("PROGRAM_NAME", 0x08),
    ("SOUND_SET_UNSUPPORTED", 0x09),
    ("MIDI_CHANNEL_PREFIX", 0x20),
    ("MIDI_PORT", 0x21),
    ("END_OF_TRACK", 0x2F),
    ("SET_TEMPO", 0x51),
    ("SMPTE_OFFSET", 0x54),
    ("TIME_SIGNATURE", 0x58),
    ("KEY_SIGNATURE", 0x59),
    ("SEQUENCER_SPECIFIC_META_EVENT", 0x7F),
    ("UNKNOWN", 0xFF),
];

/// The two system exclusive events.
pub const SYSEX_EVENTS: &[(&str, u8)] = &[("F0_SYSEX_EVENT", 0xF0), ("F7_SYSEX_EVENT", 0xF7)];

const PROGRAM_CHANGE: u8 = 0xC0;
const CHANNEL_KEY_PRESSURE: u8 = 0xD0;
const CONTROLLER_CHANGE: u8 = 0xB0;
const PITCH_BEND: u8 = 0xE0;
const NOTE_ON: u8 = 0x90;
const NOTE_OFF: u8 = 0x80;
const LOCAL_CONTROL: u8 = 0x7A;
const MONO_MODE_ON: u8 = 0x7E;
const UNKNOWN_META: u8 = 0xFF;

/// How far a variable-length number is read before it is given up on.
const LONGEST_NUMBER: usize = 999;

/// What goes wrong with MIDI's bytes, in music21's words.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MidiDataError {
    /// The bytes ran out where more were needed: Python's `IndexError`.
    #[error("index out of range")]
    OutOfRange,
    /// A byte value outside a byte: Python's `ValueError` from `bytes`.
    #[error("bytes must be in range(0, 256)")]
    NotAByte,
    /// No end was found to a variable-length number.
    #[error("did not find the end of the number!")]
    Unterminated,
    /// A variable-length number past what 64 bits hold.
    #[error("the number is too large")]
    TooLarge,
    /// A negative number, which has no variable-length form.
    #[error("cannot putVariableLengthNumber() when number is negative: {0}")]
    Negative(i128),
    /// A number past a byte, to be written as one.
    #[error("Cannot place a number > 255 in a list: {0}")]
    NotInList(i128),
    /// A status nibble that is no channel message.
    #[error("{0} is not a valid ChannelVoiceMessages")]
    NotChannelVoice(u8),
    /// A program change or channel pressure past 127; the message is the
    /// status nibble's.
    #[error("Cannot have a {} followed by a byte > 127: {byte}", channel_voice_repr(*.message))]
    DataTooLarge {
        /// The status nibble.
        message: u8,
        /// The byte after it.
        byte: u8,
    },
    /// A status byte that starts no event music21 reads.
    #[error("Unknown midi event type {0:#x}")]
    UnknownEventType(u8),
    /// A channel message written without both its parameters.
    #[error("Cannot write MIDI event without parameter1 and parameter2 set.")]
    MissingParameters,
    /// A channel message's data neither a byte nor bytes.
    #[error("cannot parse Miscellaneous Message")]
    IncorrectData,
    /// An event of no type that is written.
    #[error("unknown midi event type")]
    UnwrittenType,
    /// A track chunk without its tag.
    #[error("badly formed midi string: missing leading MTrk")]
    NotATrack,
    /// Bytes that do not start with [`FILE_HEADER`]; the first twenty.
    #[error("badly formatted midi bytes, got: {0:?}")]
    NotAFile(Vec<u8>),
    /// A file header of other than six bytes.
    #[error("badly formatted midi bytes")]
    BadHeaderLength,
    /// A file of format 2, whose tracks are separate pieces.
    #[error("cannot handle midi file format: {0}")]
    UnreadFormat(u64),
    /// SMPTE time of a frame rate MIDI does not have.
    #[error("cannot handle ticks per frame: {0}")]
    TicksPerFrame(u64),
    /// Ticks to a quarter note too many for the header to hold.
    #[error("Cannot write midi bytes unless self.ticksPerQuarterNote is a multiple of 1024")]
    UnwrittenDivision,
    /// A channel other than 1 to 16.
    #[error("bad channel value: {0}")]
    BadChannel(i64),
}

/// Shorthand for this module's results.
pub type MidiResult<T> = std::result::Result<T, MidiDataError>;

/// `<ChannelVoiceMessages.NAME: 0x..>`, as music21's enum shows itself.
fn channel_voice_repr(value: u8) -> String {
    let name = name_of(CHANNEL_VOICE_MESSAGES, value).unwrap_or("?");
    format!("<ChannelVoiceMessages.{name}: 0x{value:02X}>")
}

/// The name a table gives a value.
pub fn name_of(table: &[(&'static str, u8)], value: u8) -> Option<&'static str> {
    table
        .iter()
        .find(|(_, known)| *known == value)
        .map(|(name, _)| *name)
}

fn in_table(table: &[(&str, u8)], value: u8) -> bool {
    table.iter().any(|(_, known)| *known == value)
}

/// The first `length` bytes as one big-endian number, and what follows.
///
/// ```
/// use music21::midi::base::get_number;
///
/// assert_eq!(get_number(b"tests", 2).unwrap(), (29797, &b"sts"[..]));
/// assert_eq!(get_number(b"test", 0).unwrap(), (0, &b"test"[..]));
/// ```
pub fn get_number(bytes: &[u8], length: usize) -> MidiResult<(u64, &[u8])> {
    if bytes.len() < length {
        return Err(MidiDataError::OutOfRange);
    }
    let mut total: u64 = 0;
    for byte in &bytes[..length] {
        total = total
            .checked_mul(256)
            .ok_or(MidiDataError::TooLarge)?
            .wrapping_add(u64::from(*byte));
    }
    Ok((total, &bytes[length..]))
}

/// A number's low `length` bytes, and the rest of it with those taken
/// away: music21's `getNumber` given a number.
///
/// ```
/// use music21::midi::base::split_number;
///
/// assert_eq!(split_number(516, 1), (4, 512));
/// ```
pub fn split_number(number: i128, length: u32) -> (i128, i128) {
    let bits = 8 * length;
    if bits >= 127 {
        return (number, 0);
    }
    let low = number - ((number >> bits) << bits);
    (low, number - low)
}

/// A number written seven bits to a byte, high bit set on all but the
/// last, and what follows it.
///
/// ```
/// use music21::midi::base::get_variable_length_number;
///
/// assert_eq!(get_variable_length_number(b"A-u").unwrap(), (65, &b"-u"[..]));
/// assert_eq!(get_variable_length_number(b"\xff\x7f").unwrap(), (16383, &b""[..]));
/// ```
pub fn get_variable_length_number(bytes: &[u8]) -> MidiResult<(u64, &[u8])> {
    let mut total: u64 = 0;
    for index in 0..LONGEST_NUMBER {
        let Some(byte) = bytes.get(index) else {
            return Err(MidiDataError::OutOfRange);
        };
        if total >> 57 != 0 {
            return Err(MidiDataError::TooLarge);
        }
        total = (total << 7) + u64::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return Ok((total, &bytes[index + 1..]));
        }
    }
    Err(MidiDataError::Unterminated)
}

/// A number as `length` big-endian bytes, only its low bytes kept where
/// it is longer.
///
/// ```
/// use music21::midi::base::put_number;
///
/// assert_eq!(put_number(258, 2), b"\x01\x02");
/// assert_eq!(put_number(258, 1), b"\x02");
/// ```
pub fn put_number(number: i128, length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| {
            let shift = 8 * (length - 1 - index);
            let shifted = if shift >= 128 {
                if number < 0 { -1 } else { 0 }
            } else {
                number >> shift
            };
            (shifted & 0xFF) as u8
        })
        .collect()
}

/// The fewest bytes MIDI writes a number in, seven bits to a byte.
///
/// ```
/// use music21::midi::base::put_variable_length_number;
///
/// assert_eq!(put_variable_length_number(127).unwrap(), b"\x7f");
/// assert_eq!(put_variable_length_number(128).unwrap(), b"\x81\x00");
/// assert!(put_variable_length_number(-1).is_err());
/// ```
pub fn put_variable_length_number(number: i128) -> MidiResult<Vec<u8>> {
    if number < 0 {
        return Err(MidiDataError::Negative(number));
    }
    let mut left = number;
    let mut out = Vec::new();
    loop {
        out.push(((left & 0x7F) as u8) | 0x80);
        left >>= 7;
        if left == 0 {
            break;
        }
    }
    out.reverse();
    if let Some(last) = out.last_mut() {
        *last &= 0x7F;
    }
    Ok(out)
}

/// Numbers as one byte each, a negative one counted down from 256.
///
/// ```
/// use music21::midi::base::put_numbers_as_list;
///
/// assert_eq!(put_numbers_as_list(&[0, 0, 0, -3]).unwrap(), b"\x00\x00\x00\xfd");
/// assert!(put_numbers_as_list(&[256]).is_err());
/// ```
pub fn put_numbers_as_list(numbers: &[i128]) -> MidiResult<Vec<u8>> {
    let mut out = Vec::with_capacity(numbers.len());
    for number in numbers {
        let byte = if *number < 0 {
            number.rem_euclid(256)
        } else {
            *number
        };
        if byte >= 256 {
            return Err(MidiDataError::NotInList(byte));
        }
        out.push(byte as u8);
    }
    Ok(out)
}

/// A pitch bend of so many cents, within a range of so many semitones, as
/// the event's two parameters, least significant first; beyond the range
/// it is the range's end.
///
/// ```
/// use music21::midi::base::pitch_bend;
///
/// assert_eq!(pitch_bend(0.0, 2.0), (0, 64));
/// assert_eq!(pitch_bend(50.0, 2.0), (0, 80));
/// assert_eq!(pitch_bend(-300.0, 4.0), (0, 16));
/// ```
pub fn pitch_bend(cents: f64, bend_range: f64) -> (u8, u8) {
    let center: i64 = 0x2000;
    let top_span = (0x4000 - 1 - center) as f64;
    let bottom_span = center as f64;
    let cent_range = bend_range * 100.0;
    let cents = cents.clamp(-cent_range, cent_range);
    let scalar = cents / cent_range;
    let shift = if cents > 0.0 {
        (scalar * top_span).round_ties_even() as i64
    } else if cents < 0.0 {
        (scalar * bottom_span).round_ties_even() as i64
    } else {
        0
    };
    let target = (center + shift).max(0);
    let written = put_variable_length_number(i128::from(target)).unwrap_or_default();
    let mut most = written.first().copied().unwrap_or(0) & 0x7F;
    let least = if let Some(second) = written.get(1) {
        second & 0x7F
    } else {
        let only = most;
        most = 0;
        only
    };
    (least, most)
}

/// What kind of message an event is, with its value in music21's enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventType {
    /// A status nibble, from `0x80` to `0xE0`.
    ChannelVoice(u8),
    /// A controller number, from `0x78` to `0x7F`.
    ChannelMode(u8),
    /// The byte after [`METAEVENT_MARKER`], `0xFF` where unknown.
    Meta(u8),
    /// `0xF0` or `0xF7`.
    SysEx(u8),
}

/// What an event's parameter or data holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Param {
    /// A number, which written is one byte.
    Int(i64),
    /// Bytes, written as they are.
    Bytes(Vec<u8>),
    /// Text, which only a meta event writes, as ASCII.
    Text(String),
}

/// What reading an event sets on it: its type always, its channel and
/// second parameter where the message has them, and its first parameter
/// (music21's `data`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadEvent {
    /// What kind of message it is.
    pub event_type: EventType,
    /// Its channel, counted from one, where the message names one.
    pub channel: Option<u8>,
    /// Its first parameter or its data.
    pub parameter1: Param,
    /// Its second parameter, where the message has one.
    pub parameter2: Option<u8>,
}

/// A channel message off the front of `bytes`, and how many bytes it took:
/// music21's `MidiEvent.parseChannelVoiceMessage`.
///
/// ```
/// use music21::midi::base::{EventType, Param, parse_channel_voice};
///
/// let (event, used) = parse_channel_voice(&[0x92, 60, 120, b'h']).unwrap();
/// assert_eq!(event.event_type, EventType::ChannelVoice(0x90));
/// assert_eq!(event.channel, Some(3));
/// assert_eq!(event.parameter1, Param::Int(60));
/// assert_eq!(used, 3);
/// ```
pub fn parse_channel_voice(bytes: &[u8]) -> MidiResult<(ReadEvent, usize)> {
    let (Some(&status), Some(&first)) = (bytes.first(), bytes.get(1)) else {
        return Err(MidiDataError::OutOfRange);
    };
    let second = bytes.get(2).copied().unwrap_or(0);
    let message = status & 0xF0;
    if !in_table(CHANNEL_VOICE_MESSAGES, message) {
        return Err(MidiDataError::NotChannelVoice(message));
    }
    let channel = Some((status & 0x0F) + 1);
    let event = |event_type, parameter1, parameter2| ReadEvent {
        event_type,
        channel,
        parameter1,
        parameter2,
    };
    let voice = EventType::ChannelVoice(message);
    if message == PROGRAM_CHANGE || message == CHANNEL_KEY_PRESSURE {
        if first > 127 {
            return Err(MidiDataError::DataTooLarge {
                message,
                byte: first,
            });
        }
        return Ok((event(voice, Param::Int(i64::from(first)), None), 2));
    }
    if message == CONTROLLER_CHANGE && in_table(CHANNEL_MODE_MESSAGES, first) {
        let mode = EventType::ChannelMode(first);
        if first == LOCAL_CONTROL {
            let on = *bytes.get(2).ok_or(MidiDataError::OutOfRange)? == 0x7F;
            return Ok((event(mode, Param::Bytes(vec![u8::from(on)]), None), 3));
        }
        if first == MONO_MODE_ON {
            let value = *bytes.get(2).ok_or(MidiDataError::OutOfRange)?;
            return Ok((event(mode, Param::Int(i64::from(value)), None), 3));
        }
        return Ok((event(mode, Param::Int(i64::from(first)), Some(second)), 3));
    }
    Ok((event(voice, Param::Int(i64::from(first)), Some(second)), 3))
}

/// The running status after an event whose first byte is `first`: a
/// status byte other than a meta event's replaces it.
pub fn status_after(first: u8, last: Option<u8>) -> Option<u8> {
    if first < 0x80 || first == METAEVENT_MARKER {
        return last;
    }
    Some(first)
}

/// One event off the front of `bytes`, and how many of them it took:
/// music21's `MidiEvent.read`. A data byte first is read under the running
/// status `last`, or a note-on's where there is none. Fewer than two bytes
/// are no event, and `None`.
pub fn read_event(bytes: &[u8], last: Option<u8>) -> MidiResult<Option<(ReadEvent, usize)>> {
    if bytes.len() < 2 {
        return Ok(None);
    }
    let mut owned;
    let mut taken_back = 0;
    let mut data = bytes;
    if bytes[0] < 0x80 {
        owned = vec![last.unwrap_or(NOTE_ON)];
        owned.extend_from_slice(bytes);
        data = &owned;
        taken_back = 1;
    }
    let status = data[0];
    let first = data[1];
    let (event, used) = if in_table(CHANNEL_VOICE_MESSAGES, status & 0xF0) {
        parse_channel_voice(data)?
    } else if in_table(SYSEX_EVENTS, status) {
        let (event, used) = with_length(&data[1..], EventType::SysEx(status))?;
        (event, used + 1)
    } else if status == METAEVENT_MARKER {
        let kind = if in_table(META_EVENTS, first) {
            first
        } else {
            UNKNOWN_META
        };
        let (event, used) = with_length(&data[2..], EventType::Meta(kind))?;
        (event, used + 2)
    } else {
        return Err(MidiDataError::UnknownEventType(status));
    };
    Ok(Some((event, used - taken_back)))
}

/// An event whose data follows its length, as much of it as there is.
fn with_length(bytes: &[u8], event_type: EventType) -> MidiResult<(ReadEvent, usize)> {
    let (length, after) = get_variable_length_number(bytes)?;
    let length = usize::try_from(length)
        .unwrap_or(usize::MAX)
        .min(after.len());
    let used = bytes.len() - after.len() + length;
    let event = ReadEvent {
        event_type,
        channel: None,
        parameter1: Param::Bytes(after[..length].to_vec()),
        parameter2: None,
    };
    Ok((event, used))
}

/// A parameter as the bytes it is written as.
fn param_bytes(param: &Param) -> MidiResult<Vec<u8>> {
    match param {
        Param::Int(value) => Ok(vec![one_byte(i128::from(*value))?]),
        Param::Bytes(bytes) => Ok(bytes.clone()),
        Param::Text(_) => Err(MidiDataError::IncorrectData),
    }
}

fn one_byte(value: i128) -> MidiResult<u8> {
    u8::try_from(value).map_err(|_| MidiDataError::NotAByte)
}

/// An event as MIDI writes it, from its type, its channel counted from one
/// and its two parameters: music21's `MidiEvent.getBytes`.
///
/// ```
/// use music21::midi::base::{EventType, Param, event_bytes};
///
/// let written = event_bytes(
///     EventType::ChannelVoice(0x90),
///     10,
///     Some(&Param::Int(60)),
///     Some(&Param::Int(127)),
/// );
/// assert_eq!(written.unwrap(), b"\x99<\x7f");
/// ```
pub fn event_bytes(
    event_type: EventType,
    channel: i64,
    parameter1: Option<&Param>,
    parameter2: Option<&Param>,
) -> MidiResult<Vec<u8>> {
    match event_type {
        EventType::ChannelVoice(message) => {
            let status = one_byte(i128::from(channel) - 1 + i128::from(message))?;
            let mut out = vec![status];
            if message != PROGRAM_CHANGE && message != CHANNEL_KEY_PRESSURE {
                let (Some(first), Some(second)) = (parameter1, parameter2) else {
                    return Err(MidiDataError::MissingParameters);
                };
                out.extend(param_bytes(first)?);
                out.extend(param_bytes(second)?);
                return Ok(out);
            }
            match parameter1 {
                Some(Param::Int(value)) if message == PROGRAM_CHANGE => {
                    out.push(one_byte(i128::from(*value))?);
                }
                Some(Param::Int(value)) => {
                    out.push(
                        one_byte(i128::from(*value)).map_err(|_| MidiDataError::IncorrectData)?,
                    );
                }
                Some(Param::Bytes(bytes)) => out.extend(bytes),
                _ => return Err(MidiDataError::IncorrectData),
            }
            Ok(out)
        }
        EventType::ChannelMode(controller) => {
            let Some(data) = parameter1 else {
                return Err(MidiDataError::UnwrittenType);
            };
            let status = one_byte(i128::from(CONTROLLER_CHANGE) + i128::from(channel) - 1)?;
            let mut out = vec![status, controller];
            out.extend(param_bytes(data)?);
            Ok(out)
        }
        EventType::SysEx(kind) => {
            let Some(Param::Bytes(data)) = parameter1 else {
                return Err(MidiDataError::UnwrittenType);
            };
            let mut out = vec![kind];
            out.extend(put_variable_length_number(data.len() as i128)?);
            out.extend(data);
            Ok(out)
        }
        EventType::Meta(kind) => {
            let (length, data) = match parameter1 {
                Some(Param::Bytes(data)) => (data.len(), data.clone()),
                // Its length in characters, as music21 counts it.
                Some(Param::Text(text)) => (
                    text.chars().count(),
                    text.nfkd()
                        .filter(char::is_ascii)
                        .collect::<String>()
                        .into_bytes(),
                ),
                _ => return Err(MidiDataError::UnwrittenType),
            };
            let mut out = vec![METAEVENT_MARKER, kind];
            out.extend(put_variable_length_number(length as i128)?);
            out.extend(data);
            Ok(out)
        }
    }
}

/// Whether an event sounds a note: a note-on of some velocity.
pub fn is_note_on(event_type: EventType, velocity: Option<i64>) -> bool {
    event_type == EventType::ChannelVoice(NOTE_ON) && velocity != Some(0)
}

/// Whether an event ends a note: a note-off, or a note-on of no velocity.
pub fn is_note_off(event_type: EventType, velocity: Option<i64>) -> bool {
    event_type == EventType::ChannelVoice(NOTE_OFF)
        || (event_type == EventType::ChannelVoice(NOTE_ON) && velocity == Some(0))
}

/// Where an event goes among those at one time: a note-off first, then a
/// pitch bend, then the rest.
pub fn sort_order(event_type: Option<EventType>) -> i64 {
    match event_type {
        Some(EventType::ChannelVoice(NOTE_OFF)) => -20,
        Some(EventType::ChannelVoice(PITCH_BEND)) => -10,
        _ => 0,
    }
}

/// One event of a track as read: the ticks since the one before, the
/// running status it was read under and left, and what it set, `None`
/// where too few bytes were left to read one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackEvent {
    /// Ticks since the event before.
    pub delta: u64,
    /// The running status it was read under.
    pub status_before: Option<u8>,
    /// The running status it left.
    pub status_after: Option<u8>,
    /// What it set.
    pub event: Option<ReadEvent>,
}

/// A track's chunk off the front of `bytes`: its data, its events and
/// what follows it. An event music21 cannot read is skipped, as music21
/// skips it.
pub fn read_track(bytes: &[u8]) -> MidiResult<(&[u8], Vec<TrackEvent>, &[u8])> {
    if !bytes.starts_with(TRACK_HEADER) {
        return Err(MidiDataError::NotATrack);
    }
    let (length, after) = get_number(&bytes[4..], 4)?;
    let length = usize::try_from(length)
        .unwrap_or(usize::MAX)
        .min(after.len());
    let (data, rest) = after.split_at(length);
    Ok((data, track_events(data)?, rest))
}

/// A track's data read into events.
pub fn track_events(mut data: &[u8]) -> MidiResult<Vec<TrackEvent>> {
    let mut events = Vec::new();
    let mut status = None;
    while !data.is_empty() {
        let (delta, after) = get_variable_length_number(data)?;
        let read = match read_event(after, status) {
            Ok(read) => read,
            Err(MidiDataError::OutOfRange) => return Err(MidiDataError::OutOfRange),
            Err(_) => {
                data = after;
                continue;
            }
        };
        let status_before = status;
        let event = match read {
            Some((event, used)) => {
                status = status_after(after[0], status);
                data = &after[used..];
                Some(event)
            }
            None => {
                data = &[];
                None
            }
        };
        events.push(TrackEvent {
            delta,
            status_before,
            status_after: status,
            event,
        });
    }
    Ok(events)
}

/// A track's chunk around its data.
pub fn track_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = TRACK_HEADER.to_vec();
    out.extend(put_number(data.len() as i128, 4));
    out.extend(data);
    out
}

/// How a file counts time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Division {
    /// So many ticks to a quarter note, as most files count.
    TicksPerQuarterNote(u64),
    /// So many ticks to a second, from SMPTE frames.
    TicksPerSecond(u64),
}

/// A file's header: its format, how many tracks follow, how it counts
/// time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileHeader {
    /// 0 for one track, 1 for tracks played together.
    pub format: u64,
    /// How many tracks it says follow.
    pub tracks: u64,
    /// How it counts time.
    pub division: Division,
}

/// A file's header off the front of `bytes`, and what follows it. A
/// format past 1 is refused, after it is read: the error carries it.
pub fn read_file_header(bytes: &[u8]) -> MidiResult<(FileHeader, &[u8])> {
    if !bytes.starts_with(FILE_HEADER) {
        return Err(MidiDataError::NotAFile(
            bytes[..bytes.len().min(20)].to_vec(),
        ));
    }
    let (length, rest) = get_number(&bytes[4..], 4)?;
    if length != 6 {
        return Err(MidiDataError::BadHeaderLength);
    }
    let (format, rest) = get_number(rest, 2)?;
    if format > 1 {
        return Err(MidiDataError::UnreadFormat(format));
    }
    let (tracks, rest) = get_number(rest, 2)?;
    let (division, rest) = get_number(rest, 2)?;
    let division = if division & 0x8000 != 0 {
        // The high byte is the frames a second, negated in two's complement.
        let frames = 256 - (division >> 8);
        let ticks = division & 0xFF;
        if ![24, 25, 29, 30].contains(&ticks) {
            return Err(MidiDataError::TicksPerFrame(ticks));
        }
        let ticks = if ticks == 29 { 30 } else { ticks };
        Division::TicksPerSecond(ticks * frames)
    } else {
        Division::TicksPerQuarterNote(division & 0x7FFF)
    };
    let header = FileHeader {
        format,
        tracks,
        division,
    };
    Ok((header, rest))
}

/// A file's header, for a format, a count of tracks and ticks to a
/// quarter note.
pub fn file_header_bytes(format: i128, tracks: usize, ticks: i128) -> MidiResult<Vec<u8>> {
    if ticks & 0x8000 != 0 {
        return Err(MidiDataError::UnwrittenDivision);
    }
    let mut out = FILE_HEADER.to_vec();
    out.extend(put_number(6, 4));
    out.extend(put_number(format, 2));
    out.extend(put_number(tracks as i128, 2));
    out.extend(put_number(ticks, 2));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_variable_length_number_reads_back() {
        for number in [0, 127, 128, 255, 8192, 16383, 16384, 16384 * 16384] {
            let written = put_variable_length_number(number).unwrap();
            let (read, rest) = get_variable_length_number(&written).unwrap();
            assert_eq!(i128::from(read), number);
            assert!(rest.is_empty());
        }
        assert_eq!(
            get_variable_length_number("中国".as_bytes()),
            Err(MidiDataError::OutOfRange)
        );
        assert_eq!(
            get_variable_length_number("中xy".as_bytes()).unwrap(),
            (210638584, &b"y"[..])
        );
    }

    #[test]
    fn a_pitch_bend_is_split_most_significant_second() {
        assert_eq!(pitch_bend(100.0, 2.0), (0, 96));
        assert_eq!(pitch_bend(101.0, 2.0), (40, 96));
        assert_eq!(pitch_bend(300.0, 2.0), (127, 127));
        assert_eq!(pitch_bend(-196.0, 2.0), (36, 1));
        assert_eq!(pitch_bend(-197.0, 2.0), (123, 0));
        assert_eq!(pitch_bend(-399.0, 4.0), (20, 0));
    }

    #[test]
    fn a_program_change_past_127_is_refused() {
        let refused = parse_channel_voice(&[0xCF, 200]).unwrap_err();
        assert_eq!(
            refused.to_string(),
            "Cannot have a <ChannelVoiceMessages.PROGRAM_CHANGE: 0xC0> followed by a byte > 127: 200"
        );
    }

    #[test]
    fn a_track_reads_into_events() {
        let track = b"MTrk\x00\x00\x00\x16\x00\xff\x03\x00\x00\xe0\x00@\x00\x90CZ\x88\x00\x80C\x00\x88\x00\xff/\x00";
        let (data, events, rest) = read_track(track).unwrap();
        assert_eq!(data.len(), 22);
        assert!(rest.is_empty());
        let types: Vec<_> = events
            .iter()
            .map(|event| event.event.as_ref().unwrap().event_type)
            .collect();
        assert_eq!(
            types,
            [
                EventType::Meta(0x03),
                EventType::ChannelVoice(0xE0),
                EventType::ChannelVoice(0x90),
                EventType::ChannelVoice(0x80),
                EventType::Meta(0x2F),
            ]
        );
        assert_eq!(events[3].delta, 1024);
        let written: Vec<u8> = events
            .iter()
            .flat_map(|event| {
                let read = event.event.as_ref().unwrap();
                let second = read.parameter2.map(|value| Param::Int(i64::from(value)));
                let mut out = put_variable_length_number(i128::from(event.delta)).unwrap();
                out.extend(
                    event_bytes(
                        read.event_type,
                        i64::from(read.channel.unwrap_or(1)),
                        Some(&read.parameter1),
                        second.as_ref(),
                    )
                    .unwrap(),
                );
                out
            })
            .collect();
        assert_eq!(track_bytes(&written), track.to_vec());
    }

    #[test]
    fn running_status_reads_a_data_byte_under_the_last_status() {
        let (event, used) = read_event(&[60, 0], Some(0x93)).unwrap().unwrap();
        assert_eq!(event.event_type, EventType::ChannelVoice(0x90));
        assert_eq!(event.channel, Some(4));
        assert_eq!(used, 2);
    }

    #[test]
    fn a_header_reads_back() {
        let written = file_header_bytes(1, 2, 10080).unwrap();
        let (header, rest) = read_file_header(&written).unwrap();
        assert!(rest.is_empty());
        assert_eq!(
            header,
            FileHeader {
                format: 1,
                tracks: 2,
                division: Division::TicksPerQuarterNote(10080),
            }
        );
        assert_eq!(
            file_header_bytes(1, 0, 0x8000),
            Err(MidiDataError::UnwrittenDivision)
        );
    }
}
