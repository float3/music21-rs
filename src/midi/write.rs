//! A score written as a standard MIDI file: music21's
//! `midi.translate.streamToMidiFile` and the byte writing of `midi.base`.

use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::instrument::Instrument;
use crate::notation::Lyric;
use crate::note::Note;
use crate::stream::{Stream, StreamElement, StreamEvent};
use crate::volume::Volume;

/// music21's `defaults.ticksPerQuarter`, its MusicXML divisions.
const TICKS_PER_QUARTER: i64 = 10080;
/// music21's `defaults.ticksAtStart`: the delay before the first note and
/// after the last, a quarter.
const TICKS_AT_START: i64 = TICKS_PER_QUARTER;

const NOTE_OFF: u8 = 0x80;
const NOTE_ON: u8 = 0x90;
const PROGRAM_CHANGE: u8 = 0xC0;
const PITCH_BEND: u8 = 0xE0;
const SEQUENCE_TRACK_NAME: u8 = 0x03;
const LYRIC: u8 = 0x05;
const END_OF_TRACK: u8 = 0x2F;
const SET_TEMPO: u8 = 0x51;
const TIME_SIGNATURE: u8 = 0x58;
const KEY_SIGNATURE: u8 = 0x59;

/// How [`to_midi`] writes a score: music21's `streamToMidiFile` keywords.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// Whether the notes start a quarter late, leaving a beat of silence
    /// before the first: music21's `addStartDelay`, off by default.
    pub add_start_delay: bool,
    /// Whether each track ends a quarter after its last event: music21's
    /// `addEndDelay`, on by default.
    pub add_end_delay: bool,
    /// The channels, counted from one, a part may be given:
    /// music21's `acceptableChannelList`. `None` is every channel but the
    /// tenth, which is the drums'.
    pub acceptable_channels: Option<Vec<u8>>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            add_start_delay: false,
            add_end_delay: true,
            acceptable_channels: None,
        }
    }
}

/// Writes a stream as a standard MIDI file, as music21's
/// `streamToMidiFile(...).writestr()` writes it.
///
/// A score is a track for each part after a conductor track holding every
/// tempo, meter and key signature, the first of each at any one time; any
/// other stream is one part. The conductor says a tempo of 120 and a meter
/// of `4/4` where the score says neither. Each part names itself after its
/// first instrument, takes a channel for each instrument's program, and
/// plays each note at the velocity its volume comes to under the dynamic
/// in force where it stands and its articulations; tied notes sound as one.
/// A pitch between the keys is played on a channel of its own, bent from
/// the key below. Unpitched strokes sound on the tenth channel, as the drum
/// their instrument's percussion map names. Chord symbols sound as the
/// chords they are, for as long as they last.
///
/// ```
/// use music21_rs::midi::{from_midi, to_midi, ExportOptions};
/// use music21_rs::{Duration, Note, Stream, StreamKind};
///
/// let mut part = Stream::with_kind(StreamKind::Part);
/// part.push(Note::from_name("C4")?.with_duration(Duration::half()));
/// part.push(Note::from_name("E4")?.with_duration(Duration::half()));
/// let bytes = to_midi(&part, &ExportOptions::default())?;
/// assert_eq!(&bytes[..4], b"MThd");
/// assert_eq!(from_midi(&bytes)?.notes().len(), 2);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A score holding repeats, which music21 expands first and this does not
/// yet; more pitches between the keys sounding at once than there are
/// channels to bend; and a run of tied notes after the first lasting no
/// time.
pub fn to_midi(stream: &Stream, options: &ExportOptions) -> Result<Vec<u8>> {
    let tracks = midi_tracks(stream, options)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"MThd");
    put_number(&mut bytes, 6, 4);
    put_number(&mut bytes, 1, 2);
    put_number(&mut bytes, tracks.len() as i64, 2);
    put_number(&mut bytes, TICKS_PER_QUARTER, 2);
    for track in tracks {
        bytes.extend_from_slice(b"MTrk");
        put_number(&mut bytes, track.len() as i64, 4);
        bytes.extend_from_slice(&track);
    }
    Ok(bytes)
}

// ------------------------------------------------------------------ events

#[derive(Clone, Debug)]
enum Kind {
    NoteOn,
    NoteOff,
    PitchBend,
    ProgramChange,
    Meta(u8),
}

#[derive(Clone, Debug)]
struct Event {
    kind: Kind,
    channel: Option<u8>,
    /// A note's key or a bend's low seven bits; a program's number.
    first: u8,
    /// A note's velocity or a bend's high seven bits.
    second: u8,
    /// A meta event's data.
    data: Vec<u8>,
    cent_shift: Option<IntegerType>,
    /// The note-off of a note-on, and the other way round.
    corresponding: Option<usize>,
}

impl Event {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            channel: Some(1),
            first: 0,
            second: 0,
            data: Vec::new(),
            cent_shift: None,
            corresponding: None,
        }
    }

    fn meta(kind: u8, data: Vec<u8>) -> Self {
        Self {
            data,
            ..Self::new(Kind::Meta(kind))
        }
    }

    /// music21's `MidiEvent.sortOrder`: a note-off before a bend, a bend
    /// before anything else at one time.
    fn sort_order(&self) -> i32 {
        match self.kind {
            Kind::NoteOff => -20,
            Kind::PitchBend => -10,
            _ => 0,
        }
    }

    /// music21's `MidiEvent.setPitchBend`, over two semitones either way.
    fn bend(cents: FloatType) -> Self {
        let range = 200.0;
        let cents = cents.clamp(-range, range);
        let center = 0x2000;
        let shift = if cents > 0.0 {
            (cents / range * FloatType::from(0x4000 - 1 - center)).round_ties_even()
        } else if cents < 0.0 {
            (cents / range * FloatType::from(center)).round_ties_even()
        } else {
            0.0
        };
        let target = center + shift as i32;
        Self {
            first: (target & 0x7F) as u8,
            second: ((target >> 7) & 0x7F) as u8,
            ..Self::new(Kind::PitchBend)
        }
    }

    /// music21's `MidiEvent.getBytes`.
    fn write(&self, out: &mut Vec<u8>) {
        let channel = self.channel.unwrap_or(1).saturating_sub(1);
        match self.kind {
            Kind::NoteOn | Kind::NoteOff | Kind::PitchBend => {
                let status = match self.kind {
                    Kind::NoteOn => NOTE_ON,
                    Kind::NoteOff => NOTE_OFF,
                    _ => PITCH_BEND,
                };
                out.extend_from_slice(&[status + channel, self.first, self.second]);
            }
            Kind::ProgramChange => out.extend_from_slice(&[PROGRAM_CHANGE + channel, self.first]),
            Kind::Meta(kind) => {
                out.extend_from_slice(&[0xFF, kind]);
                put_variable_length(out, self.data.len() as i64);
                out.extend_from_slice(&self.data);
            }
        }
    }
}

/// music21's `putNumber`: `length` bytes, the most significant first.
fn put_number(out: &mut Vec<u8>, number: i64, length: usize) {
    for index in 0..length {
        out.push(((number >> (8 * (length - 1 - index))) & 0xFF) as u8);
    }
}

/// music21's `putVariableLengthNumber`.
fn put_variable_length(out: &mut Vec<u8>, mut number: i64) {
    let mut bytes = vec![(number & 0x7F) as u8];
    number >>= 7;
    while number > 0 {
        bytes.push((number & 0x7F) as u8 | 0x80);
        number >>= 7;
    }
    bytes.reverse();
    out.extend_from_slice(&bytes);
}

/// music21's `offsetToMidiTicks` and `durationToMidiTicks`, rounding as
/// Python rounds.
fn ticks(quarter_length: FloatType) -> i64 {
    (quarter_length * TICKS_PER_QUARTER as FloatType).round_ties_even() as i64
}

// ------------------------------------------------------------------ prepare

/// One track's elements, flattened, with the instrument it starts with.
struct Track {
    elements: Vec<(FloatType, StreamElement)>,
    conductor: bool,
}

/// music21's `prepareStreamForMidi` and the walk of
/// `streamHierarchyToMidiTracks` up to its packets: the conductor first,
/// then each part, ties stripped and flattened.
fn tracks_of(stream: &Stream) -> Result<Vec<Track>> {
    if has_repeats(stream) {
        return Err(Error::Midi(
            "writing a score with repeats as MIDI needs them expanded, which is not ported"
                .to_string(),
        ));
    }
    let mut stream = stream.clone();
    let conductor = conductor_of(&mut stream);
    let mut parts: Vec<Stream> = if stream.has_part_like_streams() {
        stream
            .events()
            .iter()
            .filter_map(|event| event.element().as_stream().cloned())
            .collect()
    } else {
        vec![stream]
    };
    let mut tracks = vec![Track {
        elements: conductor,
        conductor: true,
    }];
    for part in &mut parts {
        part.strip_ties(true)?;
        tracks.push(Track {
            elements: flattened(part),
            conductor: false,
        });
    }
    Ok(tracks)
}

/// Whether anything in the stream makes music21's `expandRepeats` play a
/// passage twice or jump.
fn has_repeats(stream: &Stream) -> bool {
    let measure_repeats = |stream: &Stream| {
        stream.ending().is_some()
            || stream
                .left_barline()
                .is_some_and(|barline| barline.repeat_direction().is_some())
            || stream
                .right_barline()
                .is_some_and(|barline| barline.repeat_direction().is_some())
    };
    if measure_repeats(stream) {
        return true;
    }
    stream.recurse().iter().any(|(_, element)| match element {
        StreamElement::Stream(inner) => measure_repeats(inner),
        StreamElement::Barline(barline) => barline.repeat_direction().is_some(),
        StreamElement::RepeatExpression(_) => true,
        _ => false,
    })
}

/// music21's `conductorStream`: every tempo, meter and key signature taken
/// out of the stream, keeping for each kind the first at each later time,
/// with a tempo of 120 and a meter of `4/4` where there are none.
fn conductor_of(stream: &mut Stream) -> Vec<(FloatType, StreamElement)> {
    let kinds: [fn(&StreamElement) -> bool; 3] = [
        |element| matches!(element, StreamElement::MetronomeMark(_)),
        |element| matches!(element, StreamElement::TimeSignature(_)),
        |element| {
            matches!(
                element,
                StreamElement::KeySignature(_) | StreamElement::Key(_)
            )
        },
    ];
    let mut conductor: Vec<StreamEvent> = Vec::new();
    for kind in kinds {
        let mut last = -1.0;
        for (offset, element) in stream.recurse() {
            if !kind(element) {
                continue;
            }
            if offset > last {
                conductor.push(StreamEvent::new(offset, element.clone()));
            }
            last = offset;
        }
    }
    stream.retain_leaves(&mut |_, element| !kinds.iter().any(|kind| kind(element)));
    if !conductor
        .iter()
        .any(|event| matches!(event.element(), StreamElement::MetronomeMark(_)))
    {
        conductor.push(StreamEvent::new(
            0.0,
            StreamElement::MetronomeMark(crate::tempo::MetronomeMark::new(120.0)),
        ));
    }
    if !conductor
        .iter()
        .any(|event| matches!(event.element(), StreamElement::TimeSignature(_)))
    {
        conductor.push(StreamEvent::new(
            0.0,
            StreamElement::TimeSignature(
                crate::meter::TimeSignature::new(4, 4).expect("four-four is a meter"),
            ),
        ));
    }
    conductor.sort_by(crate::makenotation::event_order);
    conductor
        .into_iter()
        .map(|event| (event.offset(), event.element().clone()))
        .collect()
}

/// A stream's leaves in the order music21's `flatten` gives them, each at
/// its offset as music21's fractions hold it, so that a note and a dynamic
/// reached by different sums stand at one time.
fn flattened(stream: &Stream) -> Vec<(FloatType, StreamElement)> {
    let mut events: Vec<StreamEvent> = stream
        .leaves()
        .into_iter()
        .map(|(offset, element)| {
            StreamEvent::new(crate::makenotation::op_frac(offset), element.clone())
        })
        .collect();
    events.sort_by(crate::makenotation::event_order);
    events
        .into_iter()
        .map(|event| (event.offset(), event.element().clone()))
        .collect()
}

// ------------------------------------------------------------------ packets

#[derive(Clone, Debug)]
struct Packet {
    track: usize,
    offset: i64,
    event: usize,
    cent_shift: Option<IntegerType>,
    duration: i64,
    last_instrument: Option<Instrument>,
    initial_channel: Option<u8>,
}

/// The first instrument of a track as music21's `packetStorage` keeps it:
/// one standing at the start with a program, unpitched percussion for a
/// track of strokes, the conductor, or whatever came first.
#[derive(Clone, Debug)]
enum Initial {
    None,
    Conductor,
    Percussion,
    Instrument(Box<Instrument>),
}

impl Initial {
    fn program(&self) -> Option<u8> {
        match self {
            Self::Instrument(instrument) => instrument.midi_program(),
            _ => None,
        }
    }
}

fn midi_tracks(stream: &Stream, options: &ExportOptions) -> Result<Vec<Vec<u8>>> {
    let tracks = tracks_of(stream)?;
    let (by_program, dynamic) = channel_instrument_data(&tracks, options);

    let mut events: Vec<Event> = Vec::new();
    let mut packets: Vec<Packet> = Vec::new();
    let mut initials: Vec<(Initial, Option<u8>)> = Vec::new();
    for (id, track) in tracks.iter().enumerate() {
        let first = track
            .elements
            .iter()
            .find_map(|(offset, element)| match element {
                StreamElement::Instrument(instrument) => {
                    Some((*offset, instrument.as_ref().clone()))
                }
                _ => None,
            });
        let initial = match first {
            Some((offset, instrument)) if offset == 0.0 && instrument.midi_program().is_some() => {
                Initial::Instrument(Box::new(instrument))
            }
            first => {
                let strokes = track.elements.iter().any(|(_, element)| {
                    matches!(
                        element,
                        StreamElement::Unpitched(_) | StreamElement::PercussionChord(_)
                    )
                });
                let sounds = track
                    .elements
                    .iter()
                    .any(|(_, element)| crate::makenotation::is_general_note(element));
                if strokes {
                    Initial::Percussion
                } else if id == 0 && !sounds {
                    Initial::Conductor
                } else {
                    first.map_or(Initial::None, |(_, instrument)| {
                        Initial::Instrument(Box::new(instrument))
                    })
                }
            }
        };
        let channel = match &initial {
            Initial::None => Some(by_program.get(&None).copied().unwrap_or(1)),
            Initial::Percussion => Some(10),
            Initial::Conductor => None,
            Initial::Instrument(instrument) => Some(
                by_program
                    .get(&instrument.midi_program())
                    .copied()
                    .unwrap_or(1),
            ),
        };
        let mut track_packets = track_packets(id, track, options, &mut events)?;
        for packet in &mut track_packets {
            packet.initial_channel = channel;
        }
        packets.extend(track_packets);
        initials.push((initial, channel));
    }

    let packets = assign_channels(packets, &mut events, &dynamic, &initials)?;

    let mut written = Vec::new();
    for (id, (initial, channel)) in initials.iter().enumerate() {
        let mut out: Vec<u8> = Vec::new();
        start_events(&mut out, initial, *channel);
        let mut last = 0;
        for packet in packets.iter().filter(|packet| packet.track == id) {
            let delta = packet.offset - last;
            if delta < 0 {
                return Err(Error::Midi("got a negative delta time".to_string()));
            }
            put_variable_length(&mut out, delta);
            events[packet.event].write(&mut out);
            last = packet.offset;
        }
        put_variable_length(
            &mut out,
            if options.add_end_delay {
                TICKS_AT_START
            } else {
                0
            },
        );
        Event::meta(END_OF_TRACK, Vec::new()).write(&mut out);
        written.push(out);
    }
    Ok(written)
}

/// music21's `getStartEvents`: the track's name, and the first
/// instrument's program where it has one.
fn start_events(out: &mut Vec<u8>, initial: &Initial, channel: Option<u8>) {
    let name = match initial {
        Initial::Conductor => return,
        Initial::Instrument(instrument) => instrument.best_name().unwrap_or("").to_string(),
        Initial::Percussion => percussion().best_name().unwrap_or("").to_string(),
        Initial::None => String::new(),
    };
    put_variable_length(out, 0);
    Event::meta(SEQUENCE_TRACK_NAME, name.into_bytes()).write(out);
    if let Some(program) = initial.program() {
        put_variable_length(out, 0);
        Event {
            first: program,
            channel,
            ..Event::new(Kind::ProgramChange)
        }
        .write(out);
    }
}

/// music21's `UnpitchedPercussion()`, which a track of strokes with no
/// instrument of its own is named after.
fn percussion() -> Instrument {
    Instrument::of_kind("UnpitchedPercussion").unwrap_or_default()
}

/// music21's `channelInstrumentData`: a channel for each program the parts
/// play, an instrument's own channel where it names one, and the channels
/// left over to bend microtones on.
fn channel_instrument_data(
    tracks: &[Track],
    options: &ExportOptions,
) -> (Vec<(Option<u8>, u8)>, Vec<u8>) {
    let mut acceptable: Vec<u8> = options
        .acceptable_channels
        .clone()
        .unwrap_or_else(|| (1..=9).chain(11..=16).collect());
    let mut programs: Vec<Option<u8>> = Vec::new();
    let mut by_program: Vec<(Option<u8>, u8)> = Vec::new();
    let mut assigned: Vec<u8> = Vec::new();
    for track in tracks.iter().filter(|track| !track.conductor) {
        let mut any = false;
        for (_, element) in &track.elements {
            let StreamElement::Instrument(instrument) = element else {
                continue;
            };
            let program = instrument.midi_program();
            if let Some(channel) = instrument.midi_channel()
                && !by_program.has(&program)
            {
                let mut channel = channel + 1;
                if let Some(index) = acceptable.iter().position(|ok| *ok == channel) {
                    acceptable.remove(index);
                } else if channel != 10 {
                    channel = 1;
                }
                if !assigned.contains(&channel) {
                    assigned.push(channel);
                }
                by_program.push((program, channel));
            }
            if !programs.contains(&program) {
                programs.push(program);
            }
            any = true;
        }
        if !any && !programs.contains(&None) {
            programs.push(None);
        }
    }
    let needed: Vec<Option<u8>> = programs
        .into_iter()
        .filter(|program| !by_program.has(program))
        .collect();
    for (index, program) in needed.into_iter().enumerate() {
        let channel = if index + 1 < acceptable.len() {
            acceptable[index]
        } else {
            acceptable.first().copied().unwrap_or(1)
        };
        if !assigned.contains(&channel) {
            assigned.push(channel);
        }
        by_program.push((program, channel));
    }
    let dynamic = acceptable
        .into_iter()
        .filter(|channel| !assigned.contains(channel))
        .collect();
    (by_program, dynamic)
}

/// A list of pairs read as a map, in the order its keys were added.
trait Keyed<K, V> {
    fn get(&self, key: &K) -> Option<&V>;
    fn has(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
}

impl<K: PartialEq, V> Keyed<K, V> for Vec<(K, V)> {
    fn get(&self, key: &K) -> Option<&V> {
        self.iter()
            .find(|(held, _)| held == key)
            .map(|(_, value)| value)
    }
}

/// music21's `streamToPackets` over one flattened track.
fn track_packets(
    id: usize,
    track: &Track,
    options: &ExportOptions,
    events: &mut Vec<Event>,
) -> Result<Vec<Packet>> {
    let as_stream = Stream::from_events(
        track
            .elements
            .iter()
            .map(|(offset, element)| StreamEvent::new(*offset, element.clone()))
            .collect::<Vec<_>>(),
    );
    let in_force = crate::volume::dynamics_in_force(&as_stream);
    let leaves = as_stream.leaves();
    let dynamic_at = |index: usize| -> Option<FloatType> {
        in_force[index].and_then(|position| match leaves[position].1 {
            StreamElement::Dynamic(dynamic) => Some(dynamic.volume_scalar()),
            _ => None,
        })
    };

    let mut packets = Vec::new();
    let mut last_instrument: Option<Instrument> = None;
    let mut percussion_in_force: Option<u8> = None;
    for (index, (offset, element)) in track.elements.iter().enumerate() {
        if let StreamElement::Instrument(instrument) = element {
            last_instrument = Some(instrument.as_ref().clone());
            if instrument.is_a("UnpitchedPercussion")
                && let Some(pitch) = instrument.percussion_pitch()
            {
                percussion_in_force = Some(pitch);
            }
        }
        let list = element_events(element, dynamic_at(index), percussion_in_force)?;
        let Some(list) = list else {
            continue;
        };
        let duration = ticks(element.quarter_length());
        let mut note_played = false;
        // The note-offs come in the order of the note-ons they end.
        let mut waiting: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        for mut event in list {
            if matches!(event.kind, Kind::NoteOn) {
                note_played = true;
            }
            let mut at = ticks(*offset);
            if note_played && options.add_start_delay {
                at += TICKS_AT_START;
            }
            let is_off = matches!(event.kind, Kind::NoteOff);
            if is_off {
                at += duration;
            }
            let index = events.len();
            if matches!(event.kind, Kind::NoteOn) {
                waiting.push_back(index);
            } else if is_off {
                let on = waiting.pop_front().expect("a note-off follows its note-on");
                events[on].corresponding = Some(index);
                event.corresponding = Some(on);
            }
            packets.push(Packet {
                track: id,
                offset: at,
                event: index,
                cent_shift: event.cent_shift,
                duration: if is_off { 0 } else { duration },
                last_instrument: last_instrument.clone(),
                initial_channel: None,
            });
            events.push(event);
        }
    }
    packets.sort_by_key(|packet| (packet.offset, events[packet.event].sort_order()));
    Ok(packets)
}

/// music21's `elementToMidiEventList`: what an element says as events, in
/// the order music21 lists them, or nothing.
fn element_events(
    element: &StreamElement,
    dynamic: Option<FloatType>,
    percussion_in_force: Option<u8>,
) -> Result<Option<Vec<Event>>> {
    let shift = |articulations: &[crate::articulations::Articulation]| -> FloatType {
        articulations
            .iter()
            .map(crate::articulations::Articulation::volume_shift)
            .sum()
    };
    let velocity = |volume: &Volume, dynamic: Option<FloatType>, shift: FloatType| -> u8 {
        (volume.realized_with(dynamic, shift, 0.5, true) * 127.0).round_ties_even() as u8
    };
    let stroke_pitch = |stored: Option<&Instrument>| -> u8 {
        stored
            .filter(|instrument| instrument.is_a("UnpitchedPercussion"))
            .and_then(Instrument::percussion_pitch)
            .or(percussion_in_force)
            .unwrap_or(60)
    };
    Ok(Some(match element {
        StreamElement::Note(note) => single(
            note,
            note.pitch().midi(),
            pitch_shift(note.pitch()),
            velocity(&note.volume(), dynamic, shift(note.articulations())),
            1,
        ),
        StreamElement::Unpitched(stroke) => {
            let note = stroke.written();
            single(
                note,
                IntegerType::from(stroke_pitch(stroke.stored_instrument())),
                None,
                velocity(&note.volume(), dynamic, shift(note.articulations())),
                10,
            )
        }
        StreamElement::Chord(chord) => {
            let members: Vec<(IntegerType, Option<IntegerType>, u8)> =
                if chord.has_component_volumes() {
                    chord
                        .notes()
                        .iter()
                        .map(|note| {
                            (
                                note.pitch().midi(),
                                pitch_shift(note.pitch()),
                                velocity(&note.volume(), None, 0.0),
                            )
                        })
                        .collect()
                } else {
                    let loud = velocity(&chord.volume(), dynamic, shift(chord.articulations()));
                    chord
                        .notes()
                        .iter()
                        .map(|note| (note.pitch().midi(), pitch_shift(note.pitch()), loud))
                        .collect()
                };
            together(&lyric_text(chord.lyrics()), &members, 1)
        }
        StreamElement::ChordSymbol(symbol) => {
            let loud = velocity(&Volume::default(), dynamic, 0.0);
            let members: Vec<_> = symbol
                .pitches()
                .unwrap_or_default()
                .iter()
                .map(|pitch| (pitch.midi(), pitch_shift(pitch), loud))
                .collect();
            together("", &members, 1)
        }
        StreamElement::PercussionChord(chord) => {
            let written = chord.written();
            let component = written.has_component_volumes();
            let loud = velocity(&written.volume(), dynamic, shift(written.articulations()));
            let members: Vec<_> = written
                .notes()
                .iter()
                .enumerate()
                .map(|(index, note)| {
                    let velocity = if component {
                        velocity(&note.volume(), None, 0.0)
                    } else {
                        loud
                    };
                    if chord.is_unpitched(index) {
                        (
                            IntegerType::from(stroke_pitch(written.stored_instrument())),
                            None,
                            velocity,
                        )
                    } else {
                        (note.pitch().midi(), pitch_shift(note.pitch()), velocity)
                    }
                })
                .collect();
            together(&lyric_text(written.lyrics()), &members, 10)
        }
        StreamElement::TimeSignature(meter) => {
            let numerator = meter.numerator();
            if numerator > 255 {
                return Ok(Some(Vec::new()));
            }
            let denominator = FloatType::from(meter.denominator()).log2() as u8;
            vec![Event::meta(
                TIME_SIGNATURE,
                vec![numerator as u8, denominator, 24, 8],
            )]
        }
        StreamElement::KeySignature(signature) => {
            key_events(signature.sharps().unwrap_or(0), false)
        }
        StreamElement::Key(key) => key_events(
            key.key_signature().sharps().unwrap_or(0),
            key.mode() == "minor",
        ),
        StreamElement::MetronomeMark(mark) => {
            if mark.number().is_none() && mark.number_sounding().is_none() {
                return Ok(None);
            }
            let Some(bpm) = mark.sounding_quarter_bpm() else {
                return Ok(None);
            };
            let mut data = Vec::new();
            put_number(&mut data, (60_000_000.0 / bpm).round_ties_even() as i64, 3);
            vec![Event::meta(SET_TEMPO, data)]
        }
        StreamElement::Instrument(instrument) => vec![Event {
            first: instrument.midi_program().unwrap_or(0),
            ..Event::new(Kind::ProgramChange)
        }],
        _ => return Ok(None),
    }))
}

fn key_events(sharps: IntegerType, minor: bool) -> Vec<Event> {
    vec![Event::meta(
        KEY_SIGNATURE,
        vec![sharps.rem_euclid(256) as u8, u8::from(minor)],
    )]
}

/// The cents a pitch between the keys is bent by, from the key it is
/// played on.
fn pitch_shift(pitch: &crate::pitch::Pitch) -> Option<IntegerType> {
    (!pitch.is_twelve_tone()).then(|| pitch.cent_shift_from_midi())
}

/// music21's `GeneralNote.lyric`: every lyric's text, a line each.
fn lyric_text(lyrics: &[Lyric]) -> String {
    lyrics
        .iter()
        .filter_map(Lyric::explicit_text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// music21's `noteToMidiEvents`.
fn single(
    note: &Note,
    key: IntegerType,
    cent_shift: Option<IntegerType>,
    velocity: u8,
    channel: u8,
) -> Vec<Event> {
    let mut events = Vec::new();
    let lyric = lyric_text(note.lyrics());
    if !lyric.is_empty() {
        events.push(Event::meta(LYRIC, lyric.into_bytes()));
    }
    let on = Event {
        first: key as u8,
        second: velocity,
        channel: Some(channel),
        cent_shift,
        ..Event::new(Kind::NoteOn)
    };
    let off = Event {
        kind: Kind::NoteOff,
        second: 0,
        ..on.clone()
    };
    events.push(on);
    events.push(off);
    events
}

/// music21's `chordToMidiEvents`: every note-on, then every note-off.
fn together(
    lyric: &str,
    members: &[(IntegerType, Option<IntegerType>, u8)],
    channel: u8,
) -> Vec<Event> {
    let mut events = Vec::new();
    if !lyric.is_empty() {
        events.push(Event::meta(LYRIC, lyric.as_bytes().to_vec()));
    }
    for (key, cent_shift, velocity) in members {
        events.push(Event {
            first: *key as u8,
            second: *velocity,
            cent_shift: *cent_shift,
            ..Event::new(Kind::NoteOn)
        });
    }
    for (key, cent_shift, _) in members {
        events.push(Event {
            first: *key as u8,
            second: 0,
            channel: Some(channel),
            cent_shift: *cent_shift,
            ..Event::new(Kind::NoteOff)
        });
    }
    events
}

/// music21's `assignPacketsToChannels`: every event given its track's
/// channel, a note sounding a pitch between the keys moved to a channel of
/// its own while another note sounds, and a bend before it and after it.
fn assign_channels(
    packets: Vec<Packet>,
    events: &mut Vec<Event>,
    dynamic: &[u8],
    initials: &[(Initial, Option<u8>)],
) -> Result<Vec<Packet>> {
    let mut unique: Vec<((i64, i64, u8), Vec<IntegerType>)> = Vec::new();
    let mut post: Vec<Packet> = Vec::new();
    let mut used_tracks: Vec<usize> = Vec::new();
    let bend = |events: &mut Vec<Event>, cents: FloatType, channel: Option<u8>| {
        events.push(Event {
            channel,
            ..Event::bend(cents)
        });
        events.len() - 1
    };
    let plain = |track: usize, offset: i64, event: usize| Packet {
        track,
        offset,
        event,
        cent_shift: None,
        duration: 0,
        last_instrument: None,
        initial_channel: None,
    };
    for packet in packets {
        if !used_tracks.contains(&packet.track) {
            used_tracks.push(packet.track);
        }
        let kind = events[packet.event].kind.clone();
        if !matches!(kind, Kind::NoteOn) {
            if !matches!(kind, Kind::NoteOff) {
                events[packet.event].channel = packet.initial_channel;
            }
            let (track, offset, shift, channel) = (
                packet.track,
                packet.offset,
                packet.cent_shift,
                events[packet.event].channel,
            );
            post.push(packet);
            if matches!(kind, Kind::NoteOff) && shift.is_some_and(|shift| shift != 0) {
                let event = bend(events, 0.0, channel);
                post.push(plain(track, offset, event));
            }
            continue;
        }
        events[packet.event].channel = packet.initial_channel;
        let start = packet.offset;
        let end = packet.offset + packet.duration;
        let shift = packet.cent_shift.filter(|shift| *shift != 0);
        let mut exclude: Vec<u8> = Vec::new();
        for ((used_start, used_stop, used_channel), shifts) in &unique {
            let (used_start, used_stop) = (*used_start, *used_stop);
            let overlaps = (start <= used_start && used_start < end)
                || (start < used_stop && used_stop < end)
                || (used_start <= start && start < used_stop)
                || (used_start < end && end < used_stop);
            if overlaps
                && (!shifts.is_empty() || shift.is_some())
                && !exclude.contains(used_channel)
            {
                exclude.push(*used_channel);
            }
        }
        let corresponding = events[packet.event]
            .corresponding
            .expect("a note-on has its note-off");
        let channel = if exclude.is_empty() {
            let channel = events[packet.event].channel;
            events[corresponding].channel = channel;
            channel
        } else {
            let channel = dynamic
                .iter()
                .copied()
                .find(|channel| !exclude.contains(channel))
                .ok_or_else(|| {
                    Error::Midi(
                        "no unused channels available for microtone/instrument assignment"
                            .to_string(),
                    )
                })?;
            events[packet.event].channel = Some(channel);
            events[corresponding].channel = Some(channel);
            if let Some(instrument) = &packet.last_instrument {
                events.push(Event {
                    first: instrument.midi_program().unwrap_or(0),
                    channel: Some(channel),
                    ..Event::new(Kind::ProgramChange)
                });
                post.push(plain(packet.track, start, events.len() - 1));
            }
            Some(channel)
        };
        if let Some(shift) = shift {
            let event = bend(events, FloatType::from(shift), channel);
            post.push(plain(packet.track, start, event));
        }
        let key = (
            packet.offset,
            packet.offset + packet.duration,
            channel.unwrap_or(1),
        );
        let held = match unique.iter().position(|(held, _)| *held == key) {
            Some(index) => index,
            None => {
                unique.push((key, Vec::new()));
                unique.len() - 1
            }
        };
        if let Some(shift) = shift {
            unique[held].1.push(shift);
        }
        post.push(packet);
    }
    for track in used_tracks {
        if track == 0 {
            continue;
        }
        let Some(channel) = initials.get(track).and_then(|(_, channel)| *channel) else {
            continue;
        };
        let event = bend(events, 0.0, Some(channel));
        post.push(plain(track, 0, event));
    }
    post.sort_by_key(|packet| (packet.offset, events[packet.event].sort_order()));
    Ok(post)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notation::{Tie, TieType};
    use crate::{Duration, Note, StreamKind};

    fn variable_length(number: i64) -> Vec<u8> {
        let mut out = Vec::new();
        put_variable_length(&mut out, number);
        out
    }

    #[test]
    fn numbers_are_written_seven_bits_a_byte() {
        assert_eq!(variable_length(0), [0x00]);
        assert_eq!(variable_length(0x7F), [0x7F]);
        assert_eq!(variable_length(0x80), [0x81, 0x00]);
        assert_eq!(variable_length(10080), [0xCE, 0x60]);
    }

    #[test]
    fn a_bend_spans_two_semitones_either_way() {
        let middle = Event::bend(0.0);
        assert_eq!((middle.first, middle.second), (0x00, 0x40));
        let up = Event::bend(200.0);
        assert_eq!((up.first, up.second), (0x7F, 0x7F));
        let down = Event::bend(-300.0);
        assert_eq!((down.first, down.second), (0x00, 0x00));
    }

    #[test]
    fn tied_notes_sound_once_after_a_conductor_saying_120_and_four_four() {
        let mut part = Stream::with_kind(StreamKind::Part);
        let mut first = Note::from_name("C4").unwrap();
        first.set_tie(Some(Tie::new(TieType::Start)));
        let mut second = Note::from_name("C4")
            .unwrap()
            .with_duration(Duration::half());
        second.set_tie(Some(Tie::new(TieType::Stop)));
        part.push(first);
        part.push(second);
        let bytes = to_midi(&part, &ExportOptions::default()).unwrap();
        // Two tracks.
        assert_eq!(&bytes[10..12], [0, 2]);
        // 500,000 microseconds a quarter, and four-four.
        assert!(
            bytes
                .windows(6)
                .any(|window| window == [0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20])
        );
        assert!(
            bytes
                .windows(7)
                .any(|window| window == [0xFF, 0x58, 0x04, 0x04, 0x02, 0x18, 0x08])
        );
        let note_ons = bytes
            .windows(2)
            .filter(|window| window == &[0x90, 60])
            .count();
        assert_eq!(note_ons, 1);
        // The one note lasts three quarters.
        assert!(
            bytes
                .windows(4)
                .any(|window| window == [0x81, 0xEC, 0x20, 0x80])
        );
    }
}
