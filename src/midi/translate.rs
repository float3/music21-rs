//! music21's `midi.translate` below any score object: what a note, a chord,
//! an instrument, a meter, a key or a tempo is as MIDI events, and what
//! such events say back, timed in ticks.
//!
//! The events are laid out in the order music21's translating functions
//! list them, each delta time before the event it times, so that a caller
//! building music21's event objects builds the same list. The file reader
//! and writer ([`from_midi`](super::from_midi), [`to_midi`](super::to_midi))
//! use the same encodings.

use std::collections::HashMap;
use std::hash::Hash;

use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::instrument::{Instrument, SearchLanguage};

use super::base::{
    CHANNEL_VOICE_MESSAGES, EventType, Param, ReadEvent, file_header_bytes, put_number,
    put_variable_length_number,
};

/// music21's `defaults.ticksPerQuarter`, its MusicXML divisions.
pub const TICKS_PER_QUARTER: i64 = 10080;

/// music21's `defaults.ticksAtStart`: the delay before the first note and
/// after the last, a quarter.
pub const TICKS_AT_START: i64 = TICKS_PER_QUARTER;

/// The channel General MIDI keeps for drums, counted from one.
pub const PERCUSSION_CHANNEL: u8 = 10;

/// MIDI clock pulses to a metronome click: one a quarter.
const PULSES_PER_CLICK: u8 = 24;
/// Thirty-second notes to a quarter.
const THIRTY_SECONDS_PER_QUARTER: u8 = 8;
/// What a tempo of a quarter a minute lasts, in microseconds.
const MICROSECONDS_PER_MINUTE: FloatType = 60_000_000.0;
/// The bytes a tempo is written in.
const TEMPO_BYTES: usize = 3;
/// The key a stroke sounds on where nothing names one.
pub const DEFAULT_STROKE_KEY: u8 = 60;

const NOTE_ON: u8 = 0x90;
const NOTE_OFF: u8 = 0x80;
const PROGRAM_CHANGE: u8 = 0xC0;
const SEQUENCE_TRACK_NAME: u8 = 0x03;
const INSTRUMENT_NAME: u8 = 0x04;
const LYRIC: u8 = 0x05;
const END_OF_TRACK: u8 = 0x2F;
const SET_TEMPO: u8 = 0x51;
const TIME_SIGNATURE: u8 = 0x58;
const KEY_SIGNATURE: u8 = 0x59;

/// Whether the first note waits a quarter: music21's `addStartDelay`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartDelay {
    /// The music starts at once.
    Without,
    /// The music starts [`TICKS_AT_START`] late.
    Quarter,
}

/// Whether a track ends a quarter after its last event: music21's
/// `addEndDelay`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndDelay {
    /// The track ends with its last event.
    Without,
    /// The track ends [`TICKS_AT_START`] later.
    Quarter,
}

/// Whether a list of events carries a delta time before each:
/// music21's `includeDeltaTime`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deltas {
    /// A delta time before each event.
    Included,
    /// The events alone.
    Omitted,
}

/// A key's mode as MIDI writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Written `0`; also any key that names no mode.
    Major,
    /// Written `1`.
    Minor,
}

/// One entry of a translated list: a delta time or an event.
#[derive(Clone, Debug, PartialEq)]
pub enum Laid {
    /// Ticks waited, and the channel music21 gives the delta time where it
    /// gives one.
    Delta {
        /// Ticks since the event before.
        ticks: i64,
        /// Its channel, counted from one; `None` leaves music21's default.
        channel: Option<u8>,
    },
    /// An event, and the cents its note is bent by where it is between
    /// the keys.
    Event {
        /// The event itself.
        event: ReadEvent,
        /// music21's `centShift`.
        cent_shift: Option<IntegerType>,
    },
}

impl Laid {
    fn delta(ticks: i64, channel: Option<u8>) -> Self {
        Self::Delta { ticks, channel }
    }

    fn event(event_type: EventType, channel: Option<u8>, data: Param) -> Self {
        Self::Event {
            event: ReadEvent {
                event_type,
                channel,
                parameter1: data,
                parameter2: None,
            },
            cent_shift: None,
        }
    }

    fn meta(kind: u8, data: Vec<u8>) -> Self {
        Self::event(EventType::Meta(kind), None, Param::Bytes(data))
    }

    fn note(message: u8, channel: u8, sounding: &Sounding, velocity: u8) -> Self {
        Self::Event {
            event: ReadEvent {
                event_type: EventType::ChannelVoice(message),
                channel: Some(channel),
                parameter1: Param::Int(i64::from(sounding.key)),
                parameter2: Some(velocity),
            },
            cent_shift: sounding.cent_shift,
        }
    }
}

/// A key sounding: which, bent by how many cents, how loud.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sounding {
    /// The MIDI key.
    pub key: u8,
    /// The cents a pitch between the keys is bent by.
    pub cent_shift: Option<IntegerType>,
    /// The note-on's velocity.
    pub velocity: u8,
}

/// An offset in ticks, a quarter later with the start delay: music21's
/// `offsetToMidiTicks`, rounding as Python rounds.
///
/// ```
/// use music21::midi::translate::{StartDelay, offset_ticks};
///
/// assert_eq!(offset_ticks(20.5, StartDelay::Without), 206640);
/// assert_eq!(offset_ticks(0.0, StartDelay::Quarter), 10080);
/// ```
pub fn offset_ticks(offset: FloatType, delay: StartDelay) -> i64 {
    let ticks = duration_ticks(offset);
    match delay {
        StartDelay::Without => ticks,
        StartDelay::Quarter => ticks + TICKS_AT_START,
    }
}

/// A length in quarters as ticks: music21's `durationToMidiTicks`.
pub fn duration_ticks(quarter_length: FloatType) -> i64 {
    (quarter_length * TICKS_PER_QUARTER as FloatType).round_ties_even() as i64
}

/// The four bytes of a meter, or nothing for a numerator MIDI cannot
/// hold: music21's `timeSignatureToMidiEvents`.
///
/// ```
/// use music21::midi::translate::meter_data;
///
/// assert_eq!(meter_data(5, 4), Some([5, 2, 24, 8]));
/// assert_eq!(meter_data(267, 32), None);
/// ```
pub fn meter_data(numerator: u32, denominator: u32) -> Option<[u8; 4]> {
    let numerator = u8::try_from(numerator).ok()?;
    let power = FloatType::from(denominator).log2() as u8;
    Some([
        numerator,
        power,
        PULSES_PER_CLICK,
        THIRTY_SECONDS_PER_QUARTER,
    ])
}

/// The numerator and denominator a meter's bytes say, or nothing for too
/// few bytes or a denominator past counting: music21's
/// `midiEventsToTimeSignature`.
pub fn meter_of(data: &[u8]) -> Option<(u32, u32)> {
    let [numerator, power, ..] = data else {
        return None;
    };
    let denominator = 2_u32.checked_pow(u32::from(*power))?;
    Some((u32::from(*numerator), denominator))
}

/// The two bytes of a key: sharps, flats below nought, then the mode.
///
/// ```
/// use music21::midi::translate::{Mode, key_data};
///
/// assert_eq!(key_data(-5, Mode::Minor), [0xFB, 1]);
/// ```
pub fn key_data(sharps: IntegerType, mode: Mode) -> [u8; 2] {
    [sharps.rem_euclid(256) as u8, u8::from(mode == Mode::Minor)]
}

/// The sharps and mode a key's bytes say: music21's `midiEventsToKey`,
/// which reads a first byte past twelve as flats.
pub fn key_of(data: &[u8]) -> Option<(IntegerType, Mode)> {
    let [sharps, mode, ..] = data else {
        return None;
    };
    let sharps = if *sharps > 12 {
        IntegerType::from(*sharps) - 256
    } else {
        IntegerType::from(*sharps)
    };
    let mode = if *mode == 1 { Mode::Minor } else { Mode::Major };
    Some((sharps, mode))
}

/// The three bytes of a tempo of so many quarters a minute: the
/// microseconds a quarter lasts.
///
/// ```
/// use music21::midi::translate::tempo_data;
///
/// assert_eq!(tempo_data(90.0), *b"\n,+");
/// ```
pub fn tempo_data(quarter_bpm: FloatType) -> [u8; 3] {
    let micros = (MICROSECONDS_PER_MINUTE / quarter_bpm).round_ties_even() as i128;
    let written = put_number(micros, TEMPO_BYTES);
    [written[0], written[1], written[2]]
}

/// The quarters a minute a tempo's bytes say, to two places, or nothing
/// for too few bytes or a quarter lasting no time: music21's
/// `midiEventsToTempo`.
pub fn tempo_of(data: &[u8]) -> Option<FloatType> {
    let bytes = data.get(..TEMPO_BYTES)?;
    let micros = bytes
        .iter()
        .fold(0_u64, |total, byte| (total << 8) | u64::from(*byte));
    if micros == 0 {
        return None;
    }
    Some((MICROSECONDS_PER_MINUTE / micros as FloatType * 100.0).round_ties_even() / 100.0)
}

/// A delta time before an event where they are included.
fn timed(out: &mut Vec<Laid>, deltas: Deltas, ticks: i64, channel: Option<u8>) {
    if deltas == Deltas::Included {
        out.push(Laid::delta(ticks, channel));
    }
}

/// A note's events: its lyric where it has one, the note-on and the
/// note-off `ticks` later: music21's `noteToMidiEvents`.
pub fn note_events(
    sounding: &Sounding,
    lyric: &[u8],
    ticks: i64,
    channel: u8,
    deltas: Deltas,
) -> Vec<Laid> {
    let mut out = Vec::new();
    if !lyric.is_empty() {
        timed(&mut out, deltas, 0, Some(channel));
        out.push(Laid::meta(LYRIC, lyric.to_vec()));
    }
    timed(&mut out, deltas, 0, None);
    out.push(Laid::note(NOTE_ON, channel, sounding, sounding.velocity));
    timed(&mut out, deltas, ticks, Some(channel));
    out.push(Laid::note(NOTE_OFF, channel, sounding, 0));
    out
}

/// A chord's events: its lyric, every note-on, then every note-off, the
/// first `ticks` later: music21's `chordToMidiEvents`, whose note-ons are
/// all on the first channel whatever channel it is given.
pub fn chord_events(
    members: &[Sounding],
    lyric: &[u8],
    ticks: i64,
    channel: u8,
    deltas: Deltas,
) -> Vec<Laid> {
    let mut out = Vec::new();
    if !lyric.is_empty() {
        timed(&mut out, deltas, 0, None);
        out.push(Laid::meta(LYRIC, lyric.to_vec()));
    }
    for member in members {
        timed(&mut out, deltas, 0, None);
        out.push(Laid::note(NOTE_ON, 1, member, member.velocity));
    }
    for (index, member) in members.iter().enumerate() {
        let wait = if index == 0 { ticks } else { 0 };
        timed(&mut out, deltas, wait, None);
        out.push(Laid::note(NOTE_OFF, channel, member, 0));
    }
    out
}

/// A program change: music21's `instrumentToMidiEvents`.
pub fn program_events(program: u8, channel: u8, deltas: Deltas) -> Vec<Laid> {
    let mut out = Vec::new();
    timed(&mut out, deltas, 0, Some(channel));
    out.push(Laid::event(
        EventType::ChannelVoice(PROGRAM_CHANGE),
        Some(channel),
        Param::Int(i64::from(program)),
    ));
    out
}

/// What a track starts with: its name, then its program where it has one:
/// music21's `getStartEvents`.
pub fn start_events(name: &[u8], program: Option<u8>, channel: u8) -> Vec<Laid> {
    let mut out = vec![Laid::delta(0, Some(channel))];
    out.push(Laid::event(
        EventType::Meta(SEQUENCE_TRACK_NAME),
        Some(channel),
        Param::Bytes(name.to_vec()),
    ));
    if let Some(program) = program {
        out.extend(program_events(program, channel, Deltas::Included));
    }
    out
}

/// What a track ends with: music21's `getEndEvents`.
pub fn end_events(delay: EndDelay) -> Vec<Laid> {
    let ticks = match delay {
        EndDelay::Without => 0,
        EndDelay::Quarter => TICKS_AT_START,
    };
    vec![
        Laid::delta(ticks, None),
        Laid::meta(END_OF_TRACK, Vec::new()),
    ]
}

/// A meta event's single entry, a delta time before it where included.
fn one_meta(kind: u8, data: Vec<u8>, deltas: Deltas) -> Vec<Laid> {
    let mut out = Vec::new();
    timed(&mut out, deltas, 0, None);
    out.push(Laid::Event {
        event: ReadEvent {
            event_type: EventType::Meta(kind),
            channel: Some(1),
            parameter1: Param::Bytes(data),
            parameter2: None,
        },
        cent_shift: None,
    });
    out
}

/// A meter's events, none where MIDI cannot hold it.
pub fn meter_events(numerator: u32, denominator: u32, deltas: Deltas) -> Vec<Laid> {
    match meter_data(numerator, denominator) {
        Some(data) => one_meta(TIME_SIGNATURE, data.to_vec(), deltas),
        None => Vec::new(),
    }
}

/// A key's events. music21 names no channel on it.
pub fn key_events(sharps: IntegerType, mode: Mode, deltas: Deltas) -> Vec<Laid> {
    let mut out = Vec::new();
    timed(&mut out, deltas, 0, None);
    out.push(Laid::meta(KEY_SIGNATURE, key_data(sharps, mode).to_vec()));
    out
}

/// A tempo's events.
pub fn tempo_events(quarter_bpm: FloatType, deltas: Deltas) -> Vec<Laid> {
    one_meta(SET_TEMPO, tempo_data(quarter_bpm).to_vec(), deltas)
}

/// What an instrument is read from: music21's `midiEventToInstrument`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source<'a> {
    /// A program change's program.
    Program(u8),
    /// A name, decoded, from a track-name meta event.
    TrackName(&'a str),
    /// A name, decoded, from an instrument-name meta event.
    InstrumentName(&'a str),
    /// A name from any other event.
    OtherName(&'a str),
    /// Nothing an instrument can be told from.
    Nothing,
}

/// The instrument a percussion key plays, a tambourine taken to be
/// headless: music21's `_midiNumberToPercussionInstrument`.
///
/// A key the map says nothing of is a bare unpitched percussion
/// instrument keeping that key.
pub fn percussion_instrument(key: u8) -> Instrument {
    let mut made = crate::instrument::for_percussion_key_or_unmapped(key);
    made.set_headless(true);
    made
}

/// Whether a name is one MIDI software writes for no instrument in
/// particular: `Instrument`, `Inst 3`.
fn is_placeholder(name: &str) -> bool {
    let lower = name.to_lowercase();
    let numbered = |prefix: &str| {
        let rest = lower.replace(prefix, "");
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    };
    lower == "instrument" || lower == "inst" || numbered("instrument ") || numbered("inst ")
}

/// The instrument an event names, on its channel: music21's
/// `midiEventToInstrument`. A name is cut at its first nul, as MuseScore
/// ends them so, and kept as the part's or instrument's name unless it is
/// a placeholder.
pub fn instrument_of(source: Source<'_>, channel: u8) -> Instrument {
    let name = match source {
        Source::TrackName(name) | Source::InstrumentName(name) | Source::OtherName(name) => {
            name.split('\0').next().unwrap_or("").trim()
        }
        _ => "",
    };
    let mut instrument = match source {
        Source::Program(program) if channel == PERCUSSION_CHANNEL => {
            let mut made = Instrument::of_kind("UnpitchedPercussion").unwrap_or_default();
            made.set_midi_program(Some(program));
            made
        }
        Source::Program(program) => {
            Instrument::from_midi_program(program).unwrap_or_else(|_| Instrument::new())
        }
        Source::Nothing => Instrument::new(),
        _ => Instrument::from_name(name, SearchLanguage::All).unwrap_or_else(|_| Instrument::new()),
    };
    // Counted from nought, as music21 counts it.
    instrument.set_midi_channel(channel.checked_sub(1));
    if name.is_empty() || is_placeholder(name) {
        return instrument;
    }
    match source {
        Source::TrackName(_) => instrument.set_part_name(Some(name.to_string())),
        Source::InstrumentName(_) => instrument.set_name(Some(name.to_string())),
        _ => {}
    }
    instrument
}

/// The source an event's type and data name, for [`instrument_of`].
pub fn source_of(event_type: EventType, text: &str) -> Source<'_> {
    match event_type {
        EventType::Meta(SEQUENCE_TRACK_NAME) => Source::TrackName(text),
        EventType::Meta(INSTRUMENT_NAME) => Source::InstrumentName(text),
        _ => Source::OtherName(text),
    }
}

/// Each event paired with the tick it falls on, from a list of delta times
/// (`Some`) and events (`None`): music21's `getTimeForEvents`. An event
/// counts only right after a delta time; anything out of that order is
/// passed over. The answer is each event's tick and its place in the list.
pub fn event_ticks(deltas: &[Option<u64>]) -> Vec<(u64, usize)> {
    let mut out = Vec::new();
    let mut now: u64 = 0;
    let mut index = 0;
    while index + 1 < deltas.len() {
        let (Some(wait), None) = (deltas[index], deltas[index + 1]) else {
            index += 1;
            continue;
        };
        now = now.saturating_add(wait);
        out.push((now, index + 1));
        index += 2;
    }
    out
}

/// What an event is to a note: one starting or ending, keyed by its pitch
/// and channel, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteMark<K> {
    /// A note-on of some velocity.
    On(K),
    /// A note-off, or a note-on of none.
    Off(K),
    /// Anything else.
    Other,
}

/// A note as events time it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimedNote {
    /// The tick it starts on.
    pub on: u64,
    /// The tick it stops on.
    pub off: u64,
    /// Where its note-on stands in the list.
    pub index: usize,
}

/// Each note-on with the nearest note-off of its key after it, in the
/// order they start: music21's `getNotesFromEvents`. A note-on with no
/// note-off after it is dropped.
pub fn notes_of<K: Eq + Hash + Copy>(events: &[(u64, NoteMark<K>)]) -> Vec<TimedNote> {
    let mut waiting: HashMap<K, u64> = HashMap::new();
    let mut notes = Vec::new();
    for (index, (time, mark)) in events.iter().enumerate().rev() {
        match mark {
            NoteMark::Off(key) => {
                waiting.insert(*key, *time);
            }
            NoteMark::On(key) => {
                if let Some(off) = waiting.get(key) {
                    notes.push(TimedNote {
                        on: *time,
                        off: *off,
                        index,
                    });
                }
            }
            NoteMark::Other => {}
        }
    }
    notes.reverse();
    notes
}

/// An instrument as channels are planned from it: its program and the
/// channel it names, counted from nought as music21 counts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Played {
    /// Its program, where it names one.
    pub program: Option<u8>,
    /// Its channel, counted from nought, where it names one.
    pub channel: Option<u8>,
}

/// An instrument that named a channel the plan could not give it, and
/// was put on the first channel instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Misplaced {
    /// Which part, of those planned for.
    pub part: usize,
    /// Which instrument of that part.
    pub instrument: usize,
    /// The channel it named, counted from one.
    pub channel: u8,
    /// The channels still free then.
    pub acceptable: Vec<u8>,
}

/// Which channel each program plays on, and the channels left over to
/// bend microtones on: music21's `channelInstrumentData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChannelPlan {
    /// Each program, `None` for a part with no instrument, and its
    /// channel counted from one, in the order they were given channels.
    pub by_program: Vec<(Option<u8>, u8)>,
    /// The channels no program took.
    pub dynamic: Vec<u8>,
    /// The instruments put on the first channel against what they named.
    pub misplaced: Vec<Misplaced>,
}

impl ChannelPlan {
    /// The channel a program plays on.
    pub fn channel(&self, program: Option<u8>) -> Option<u8> {
        self.by_program
            .iter()
            .find(|(held, _)| *held == program)
            .map(|(_, channel)| *channel)
    }
}

/// The channels for the instruments of each part (a conductor's left
/// out), on `acceptable` channels counted from one, every channel but the
/// drums' where none are given: music21's `channelInstrumentData`.
///
/// An instrument naming a channel keeps it; the other programs take free
/// channels in turn, always leaving one free, and share the first where
/// too few are left.
pub fn plan_channels(parts: &[Vec<Played>], acceptable: Option<&[u8]>) -> ChannelPlan {
    let mut free: Vec<u8> = acceptable
        .map(<[u8]>::to_vec)
        .unwrap_or_else(|| (1..PERCUSSION_CHANNEL).chain(11..=16).collect());
    let mut plan = ChannelPlan::default();
    let mut programs: Vec<Option<u8>> = Vec::new();
    let mut assigned: Vec<u8> = Vec::new();

    // Instruments naming their own channels first.
    for (part, instruments) in parts.iter().enumerate() {
        for (index, played) in instruments.iter().enumerate() {
            if let Some(named) = played.channel
                && plan.channel(played.program).is_none()
            {
                let mut channel = named.saturating_add(1);
                if let Some(place) = free.iter().position(|ok| *ok == channel) {
                    free.remove(place);
                } else if channel != PERCUSSION_CHANNEL {
                    plan.misplaced.push(Misplaced {
                        part,
                        instrument: index,
                        channel,
                        acceptable: free.clone(),
                    });
                    channel = 1;
                }
                if !assigned.contains(&channel) {
                    assigned.push(channel);
                }
                plan.by_program.push((played.program, channel));
            }
            if !programs.contains(&played.program) {
                programs.push(played.program);
            }
        }
        if instruments.is_empty() && !programs.contains(&None) {
            programs.push(None);
        }
    }

    // The rest in turn, one channel always left free.
    let needed: Vec<Option<u8>> = programs
        .into_iter()
        .filter(|program| plan.channel(*program).is_none())
        .collect();
    for (index, program) in needed.into_iter().enumerate() {
        let channel = if index + 1 < free.len() {
            free[index]
        } else {
            free.first().copied().unwrap_or(1)
        };
        if !assigned.contains(&channel) {
            assigned.push(channel);
        }
        plan.by_program.push((program, channel));
    }
    plan.dynamic = free
        .into_iter()
        .filter(|channel| !assigned.contains(channel))
        .collect();
    plan
}

/// An event of music21's text notation for MIDI that is not a note: its
/// status, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsupported {
    /// A meta event, `FF`.
    Meta(String),
    /// Any channel message but a note-on or note-off.
    Channel(String),
}

/// A file written from music21's text notation for MIDI, and the events
/// it left out: music21's `midiAsciiStringToBinaryString`.
///
/// Each track is a list of events, each `delta status key velocity`: the
/// delta in decimal, the status and key in hexadecimal, the velocity in
/// decimal. Only note-ons and note-offs are written, on the channel of the
/// track's place, counted from one. A file of one track is format 1
/// whatever `format` says.
///
/// ```
/// use music21::midi::translate::ascii_file;
///
/// let (bytes, unsupported) = ascii_file(1, 960, &[vec!["0 90 31 15".to_string()]])?;
/// assert_eq!(&bytes[14..], b"MTrk\x00\x00\x00\x04\x00\x901\x0f");
/// assert!(unsupported.is_empty());
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// An event of fewer than four fields, or a field that is no number.
pub fn ascii_file(
    format: i128,
    ticks_per_quarter: i128,
    tracks: &[Vec<String>],
) -> Result<(Vec<u8>, Vec<Unsupported>)> {
    let format = if tracks.len() == 1 { 1 } else { format };
    let mut out = file_header_bytes(format, tracks.len(), ticks_per_quarter).map_err(midi_error)?;
    let mut unsupported = Vec::new();
    for (index, track) in tracks.iter().enumerate() {
        let channel = u8::try_from(index % 16).unwrap_or(0);
        let mut data = Vec::new();
        for line in track {
            let fields: Vec<&str> = line.split(' ').collect();
            let [delta, status, key, velocity, ..] = fields[..] else {
                return Err(Error::midi(format!("an event of too few fields: {line:?}")));
            };
            let delta: i128 = parsed(delta, 10)?;
            let key = parsed(key, 16)?;
            let velocity = parsed(velocity, 10)?;
            let message = match status.chars().next() {
                _ if status == "FF" => {
                    unsupported.push(Unsupported::Meta(status.to_string()));
                    continue;
                }
                Some('8') => NOTE_OFF,
                Some('9') => NOTE_ON,
                _ => {
                    unsupported.push(Unsupported::Channel(status.to_string()));
                    continue;
                }
            };
            data.extend(put_variable_length_number(delta).map_err(midi_error)?);
            let bytes = [message + channel, byte(key)?, byte(velocity)?];
            data.extend_from_slice(&bytes);
        }
        out.extend_from_slice(super::base::TRACK_HEADER);
        out.extend(put_number(data.len() as i128, 4));
        out.extend(data);
    }
    Ok((out, unsupported))
}

/// A field of the text notation, read in `radix`.
fn parsed(field: &str, radix: u32) -> Result<i128> {
    i128::from_str_radix(field, radix)
        .map_err(|_| Error::midi(format!("not a number in base {radix}: {field:?}")))
}

/// A number written as one byte.
fn byte(value: i128) -> Result<u8> {
    u8::try_from(value).map_err(|_| Error::midi(format!("not a byte: {value}")))
}

fn midi_error(error: super::base::MidiDataError) -> Error {
    Error::midi(error.to_string())
}

/// The name music21 gives a channel message's status, for a warning.
pub fn status_name(status: u8) -> Option<&'static str> {
    super::base::name_of(CHANNEL_VOICE_MESSAGES, status & 0xF0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_meter_round_trips_through_its_bytes() {
        let data = meter_data(3, 16).unwrap();
        assert_eq!(meter_of(&data), Some((3, 16)));
        assert_eq!(meter_of(&[3]), None);
    }

    #[test]
    fn a_key_of_flats_reads_back_negative() {
        assert_eq!(key_of(&key_data(-2, Mode::Minor)), Some((-2, Mode::Minor)));
        assert_eq!(key_of(&[2, 0]), Some((2, Mode::Major)));
    }

    #[test]
    fn a_tempo_reads_back_to_two_places() {
        assert_eq!(tempo_of(&tempo_data(90.0)), Some(90.0));
        assert_eq!(tempo_of(&[0, 0, 0]), None);
    }

    #[test]
    fn a_note_is_four_entries_and_two_without_deltas() {
        let sounding = Sounding {
            key: 61,
            cent_shift: None,
            velocity: 90,
        };
        let laid = note_events(&sounding, b"", 25200, 1, Deltas::Included);
        assert_eq!(laid.len(), 4);
        assert_eq!(
            laid[2],
            Laid::Delta {
                ticks: 25200,
                channel: Some(1)
            }
        );
        assert_eq!(
            note_events(&sounding, b"la", 0, 9, Deltas::Omitted).len(),
            3
        );
    }

    #[test]
    fn a_chord_turns_every_note_on_before_any_off() {
        let member = |key| Sounding {
            key,
            cent_shift: None,
            velocity: 90,
        };
        let laid = chord_events(&[member(48), member(68)], b"", 10080, 3, Deltas::Omitted);
        let kinds: Vec<_> = laid
            .iter()
            .map(|entry| match entry {
                Laid::Event { event, .. } => (event.event_type, event.channel),
                Laid::Delta { .. } => unreachable!(),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                (EventType::ChannelVoice(NOTE_ON), Some(1)),
                (EventType::ChannelVoice(NOTE_ON), Some(1)),
                (EventType::ChannelVoice(NOTE_OFF), Some(3)),
                (EventType::ChannelVoice(NOTE_OFF), Some(3)),
            ]
        );
    }

    #[test]
    fn events_are_timed_only_after_a_delta() {
        let ticks = event_ticks(&[Some(5), None, None, Some(3), Some(2), None]);
        assert_eq!(ticks, [(5, 1), (7, 5)]);
    }

    #[test]
    fn a_note_on_takes_the_nearest_note_off_after_it() {
        let events = [
            (0, NoteMark::On(60)),
            (1, NoteMark::On(62)),
            (2, NoteMark::Off(62)),
            (3, NoteMark::Off(60)),
            (4, NoteMark::On(64)),
        ];
        let notes = notes_of(&events);
        assert_eq!(
            notes,
            [
                TimedNote {
                    on: 0,
                    off: 3,
                    index: 0
                },
                TimedNote {
                    on: 1,
                    off: 2,
                    index: 1
                },
            ]
        );
    }

    #[test]
    fn a_placeholder_name_names_nothing() {
        let named = instrument_of(Source::TrackName("Inst 3"), 2);
        assert_eq!(named.part_name(), None);
        assert_eq!(named.midi_channel(), Some(1));
    }

    #[test]
    fn a_named_channel_is_kept_and_one_left_free() {
        let played = |program, channel| Played { program, channel };
        let parts = [
            vec![played(Some(0), Some(4))],
            vec![played(Some(40), None)],
            vec![],
        ];
        let plan = plan_channels(&parts, Some(&[1, 2, 4]));
        assert_eq!(plan.by_program, [(Some(0), 1), (Some(40), 1), (None, 2)]);
        assert_eq!(plan.misplaced.len(), 1);
        assert_eq!(plan.dynamic, [4]);

        let plan = plan_channels(&parts[1..], None);
        assert_eq!(plan.by_program, [(Some(40), 1), (None, 2)]);
        assert_eq!(plan.dynamic.len(), 13);
    }

    #[test]
    fn text_notation_leaves_out_what_is_no_note() {
        let tracks = [vec!["0 FF 2F 0".to_string(), "0 C0 05 0".to_string()]];
        let (_, unsupported) = ascii_file(1, 960, &tracks).unwrap();
        assert_eq!(
            unsupported,
            [
                Unsupported::Meta("FF".into()),
                Unsupported::Channel("C0".into())
            ]
        );
    }
}
