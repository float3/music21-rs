//! A part cut into segments of braille music, measure by measure and
//! grouping by grouping: music21's `braille.segment` and
//! `braille.noteGrouping`.
//!
//! music21 marks the notes of the part it transcribes with attributes for
//! the time it transcribes them -- the slurs over them, how their beams
//! read, a full measure's rest -- and writes each sign's English onto the
//! element. Here each element of a segment is a [`BrailleElement`] that
//! carries those beside a copy of the element.

use std::collections::{BTreeMap, VecDeque};

use super::basic::{
    self, NoteContext, Transcription, barline_to_braille, chord_to_braille, clef_to_braille,
    dynamic_to_braille, note_to_braille, number_to_braille, rest_to_braille,
    text_expression_to_braille, transcribe_heading, transcribe_signatures, yield_dots,
};
use super::lookup::{self, symbol};
use super::text::{BrailleKeyboard, BrailleText};
use crate::{
    defaults::{FloatType, IntegerType},
    duration::Duration,
    error::{Error, Result},
    makenotation::op_frac,
    meter::TimeSignature,
    note::Note,
    pitch::Pitch,
    rest::Rest,
    stream::{Stream, StreamElement, StreamKind},
};

/// Which kind of grouping an element goes in, in the order a measure's
/// groupings are written: music21's `Affinity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Affinity {
    /// Key and time signatures.
    Signature = 3,
    /// Tempo words.
    TempoText = 4,
    /// A metronome mark.
    MetronomeMark = 5,
    /// Text of more than one word.
    LongTextExpression = 6,
    /// Voices written in accord.
    Inaccord = 7,
    /// The first half of a group of notes split to fit a line.
    SplitNoteGroupingA = 8,
    /// Notes, rests, chords, dynamics, clefs, short texts.
    NoteGrouping = 9,
    /// The second half of a split group, and barlines.
    SplitNoteGroupingB = 10,
}

impl Affinity {
    /// music21's name for a grouping of this kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Signature => "Signature Grouping",
            Self::TempoText => "Tempo Text Grouping",
            Self::MetronomeMark => "Metronome Mark Grouping",
            Self::LongTextExpression => "Long Text Expression Grouping",
            Self::Inaccord => "Inaccord Grouping",
            Self::NoteGrouping => "Note Grouping",
            Self::SplitNoteGroupingA => "Split Note Grouping A",
            Self::SplitNoteGroupingB => "Split Note Grouping B",
        }
    }
}

/// The hand a keyboard part is played by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hand {
    /// The left hand: music21's `'left'`.
    Left,
    /// The right hand: music21's `'right'`.
    Right,
}

/// Where a grouping stands: its measure, its place among the measure's
/// groupings, its kind and its hand: music21's `SegmentKey`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentKey {
    /// The measure's number.
    pub measure: IntegerType,
    /// Which run of groupings in the measure, counting from nought.
    pub ordinal: usize,
    /// The kind of grouping.
    pub affinity: Affinity,
    /// The hand, at a keyboard.
    pub hand: Option<Hand>,
}

/// An element of a segment with what music21 keeps on it while
/// transcribing: where it stands, its affinity, how it sorts, its English,
/// its note context, and a key signature's predecessor.
#[derive(Clone, Debug)]
pub struct BrailleElement {
    /// A copy of the element, changed as music21 changes it.
    pub element: StreamElement,
    /// Its offset in its measure or voice.
    pub offset: FloatType,
    /// Its affinity.
    pub affinity: Affinity,
    /// How it sorts among elements at its offset: music21's
    /// `classSortOrder` as braille sets it.
    pub sort_order: i32,
    /// Its English: music21's `editorial.brailleEnglish`.
    pub english: Option<Vec<String>>,
    /// Its slurs and beams, for a note.
    pub context: NoteContext,
    /// The key signature before it, for a key signature: music21's
    /// `outgoingKeySig`.
    pub outgoing_key: Option<Option<IntegerType>>,
    /// Where its leaves start in the part, to find what music21 marked on
    /// it.
    pub leaf: usize,
    /// For a voice, the English of each element it holds once written,
    /// nothing for one not written.
    pub voice_english: Vec<Option<Vec<String>>>,
}

/// A grouping of elements written together: music21's
/// `BrailleElementGrouping`.
#[derive(Clone, Debug)]
pub struct BrailleElementGrouping {
    /// The elements.
    pub elements: Vec<BrailleElement>,
    /// The key signature in force, as sharps.
    pub key_signature: Option<IntegerType>,
    /// The time signature in force.
    pub time_signature: TimeSignature,
    /// Whether chords are written from the top down; nothing reads as
    /// upward, as in music21.
    pub descending_chords: Option<bool>,
    /// Whether clefs are written.
    pub show_clef_signs: bool,
    /// Whether a fingering's upper choice is written first.
    pub upper_first_in_note_fingering: bool,
    /// Whether the grouping ends with a music hyphen.
    pub with_hyphen: bool,
    /// How many times it is repeated after itself.
    pub num_repeats: usize,
}

impl Default for BrailleElementGrouping {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            key_signature: Some(0),
            time_signature: TimeSignature::new(4, 4).expect("4/4 is a meter"),
            descending_chords: Some(true),
            show_clef_signs: false,
            upper_first_in_note_fingering: true,
            with_hyphen: false,
            num_repeats: 0,
        }
    }
}

/// music21's `repr` of an element, as a segment's English shows one it has
/// not transcribed.
pub(crate) fn element_repr(element: &StreamElement) -> String {
    match element {
        StreamElement::Note(note) => format!("<music21.note.Note {}>", note.pitch().name()),
        StreamElement::Rest(rest) => {
            let duration = rest.duration();
            let name = duration.full_name().to_lowercase();
            let text = if name.chars().count() < 15 {
                name.replace(' ', "-")
            } else {
                let length = duration.quarter_length();
                if length.fract() == 0.0 {
                    format!("{}ql", length as i64)
                } else {
                    format!("{}ql", crate::statistics::python_repr(length))
                }
            };
            format!("<music21.note.Rest {text}>")
        }
        StreamElement::Chord(chord) => {
            let names: Vec<String> = chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect();
            format!("<music21.chord.Chord {}>", names.join(" "))
        }
        StreamElement::Clef(clef) => format!("<music21.clef.{}>", clef.kind().class_name()),
        StreamElement::Barline(barline) => {
            format!("<music21.bar.Barline type={}>", barline.bar_type().as_str())
        }
        StreamElement::Dynamic(dynamic) => {
            format!("<music21.dynamics.Dynamic {}>", dynamic.value())
        }
        StreamElement::TextExpression(text) => {
            let content = text.content();
            let shown = if content.chars().count() >= 13 {
                format!("{}...", content.chars().take(10).collect::<String>())
            } else {
                content.to_string()
            };
            format!(
                "<music21.expressions.TextExpression {}>",
                python_str_repr(&shown)
            )
        }
        StreamElement::TempoText(tempo) => {
            format!(
                "<music21.tempo.TempoText {}>",
                python_str_repr(tempo.text())
            )
        }
        StreamElement::KeySignature(signature) => match signature.sharps() {
            Some(sharps) => format!(
                "<music21.key.KeySignature of {}>",
                sharps_description(sharps)
            ),
            None => "<music21.key.KeySignature of pitches: []>".to_string(),
        },
        StreamElement::Key(key) => {
            // A key's `str` is its tonic and mode alone.
            let tonic = music21_pitch_name(key.tonic_pitch());
            let tonic = if key.mode() == "minor" {
                tonic.to_lowercase()
            } else {
                tonic
            };
            format!("{tonic} {}", key.mode())
        }
        StreamElement::MetronomeMark(mark) => {
            let mut sounding = "";
            let mut number = mark.number();
            if number.is_none() && mark.number_sounding().is_some() {
                sounding = " (playback only)";
                number = mark.number_sounding();
            }
            let number = number.map_or_else(|| "None".to_string(), python_number);
            let referent = mark.referent().full_name();
            match mark.text() {
                Some(text) => {
                    format!("<music21.tempo.MetronomeMark {text} {referent}={number}{sounding}>")
                }
                None => format!("<music21.tempo.MetronomeMark {referent}={number}{sounding}>"),
            }
        }
        StreamElement::Unpitched(unpitched) => match unpitched
            .stored_instrument()
            .and_then(|instrument| instrument.name())
        {
            Some(name) => format!("<music21.note.Unpitched {}>", python_str_repr(name)),
            None => "<music21.note.Unpitched>".to_string(),
        },
        StreamElement::PercussionChord(chord) => {
            let members: Vec<String> = chord
                .members()
                .iter()
                .map(|member| match member {
                    crate::percussion::PercussionNote::Note(note) => {
                        note.pitch().name_with_octave()
                    }
                    crate::percussion::PercussionNote::Unpitched(unpitched) => match unpitched
                        .stored_instrument()
                        .and_then(|instrument| instrument.name())
                    {
                        Some(name) => name.to_string(),
                        None => format!("unpitched[{}]", unpitched.display_name()),
                    },
                })
                .collect();
            format!(
                "<music21.percussion.PercussionChord [{}]>",
                members.join(" ")
            )
        }
        StreamElement::TimeSignature(meter) => format!(
            "<music21.meter.TimeSignature {}/{}>",
            meter.numerator(),
            meter.denominator()
        ),
        StreamElement::Stream(stream) => match stream.id() {
            Some(id) => format!("<music21.stream.{:?} {id}>", stream.kind()),
            None => format!("<music21.stream.{:?}>", stream.kind()),
        },
        other => format!("<music21.{other:?}>"),
    }
}

/// A pitch's name as music21 spells it, a flat as `-`.
fn music21_pitch_name(pitch: &Pitch) -> String {
    let modifier = match pitch
        .written_accidental()
        .map(|accidental| accidental.name())
    {
        Some("flat") => "-",
        Some("double-flat") => "--",
        Some("sharp") => "#",
        Some("double-sharp") => "##",
        _ => "",
    };
    format!("{}{modifier}", pitch.step().as_char())
}

/// How music21 describes a key signature of so many sharps.
fn sharps_description(sharps: IntegerType) -> String {
    match sharps {
        0 => "no sharps or flats".to_string(),
        1 => "1 sharp".to_string(),
        -1 => "1 flat".to_string(),
        sharps if sharps > 0 => format!("{sharps} sharps"),
        sharps => format!("{} flats", -sharps),
    }
}

/// A number as music21 keeps it, a whole one as an integer.
fn python_number(number: FloatType) -> String {
    if number.fract() == 0.0 {
        format!("{}", number as i64)
    } else {
        crate::statistics::python_repr(number)
    }
}

/// A text as Python's `repr` writes it, in single quotes unless it holds
/// one.
fn python_str_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// The affinity and sort order braille gives an element, or nothing for one
/// it cannot write: music21's `setAffinityCode`.
fn affinity_of(element: &StreamElement) -> Option<(Affinity, i32)> {
    Some(match element {
        StreamElement::Note(_) | StreamElement::Rest(_) | StreamElement::Chord(_) => {
            (Affinity::NoteGrouping, 10)
        }
        StreamElement::ChordSymbol(_) => (Affinity::NoteGrouping, 10),
        StreamElement::Dynamic(_) => (Affinity::NoteGrouping, 9),
        StreamElement::Clef(_) => (Affinity::NoteGrouping, 7),
        StreamElement::Barline(_) => (Affinity::SplitNoteGroupingB, 0),
        StreamElement::KeySignature(_) | StreamElement::Key(_) => (Affinity::Signature, 1),
        StreamElement::TimeSignature(_) => (Affinity::Signature, 2),
        StreamElement::TempoText(_) => (Affinity::TempoText, 3),
        StreamElement::MetronomeMark(_) => (Affinity::MetronomeMark, 4),
        StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice => {
            (Affinity::Inaccord, 10)
        }
        StreamElement::TextExpression(text) => {
            if text.content().split_whitespace().count() > 1 {
                (Affinity::LongTextExpression, 8)
            } else {
                (Affinity::NoteGrouping, 8)
            }
        }
        _ => return None,
    })
}

/// Whether music21 counts an element a `GeneralNote`.
fn is_general_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Rest(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

/// Whether music21 counts an element a `NotRest`: a stream's `notes`.
fn is_not_rest(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

/// A measure's or voice's elements as music21 iterates it, each with its
/// offset and where its leaves start in the part: its left barline, what
/// it holds, its right barline at its end.
fn walk_of(stream: &Stream, leaf_base: usize) -> Vec<(FloatType, StreamElement, usize)> {
    let mut out = Vec::new();
    if let Some(barline) = stream.left_barline() {
        out.push((0.0, StreamElement::Barline(barline.clone()), leaf_base));
    }
    let mut leaf = leaf_base;
    for event in stream.events() {
        let element = event.element().clone();
        let leaves = match &element {
            StreamElement::Stream(inner) => inner.leaves().len(),
            _ => 1,
        };
        out.push((event.offset(), element, leaf));
        leaf += leaves;
    }
    if let Some(barline) = stream.right_barline() {
        out.push((
            stream.end_offset(),
            StreamElement::Barline(barline.clone()),
            leaf,
        ));
    }
    out
}

/// The elements of a measure or voice braille writes, clefs before the
/// note they stand before, sorted by offset and braille's sort order:
/// music21's `extractBrailleElements`.
pub fn extract_braille_elements(
    stream: &Stream,
    leaf_base: usize,
    contexts: &[NoteContext],
) -> BrailleElementGrouping {
    let mut grouping = BrailleElementGrouping::default();
    let mut last_clef: Option<(FloatType, StreamElement, usize)> = None;
    for (offset, element, leaf) in walk_of(stream, leaf_base) {
        if matches!(element, StreamElement::Clef(_)) {
            last_clef = Some((offset, element, leaf));
            continue;
        }
        if let StreamElement::Barline(barline) = &element
            && barline.bar_type().as_str() == "regular"
        {
            continue;
        }
        let Some((affinity, sort_order)) = affinity_of(&element) else {
            continue;
        };
        let attaches = is_general_note(&element)
            || matches!(&element, StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice);
        if attaches && let Some((clef_offset, clef, clef_leaf)) = last_clef.take() {
            grouping.elements.push(BrailleElement {
                english: Some(vec![element_repr(&clef)]),
                element: clef,
                offset: clef_offset,
                affinity,
                sort_order: 7,
                context: NoteContext::default(),
                outgoing_key: None,
                leaf: clef_leaf,
                voice_english: Vec::new(),
            });
        }
        grouping.elements.push(BrailleElement {
            english: Some(vec![element_repr(&element)]),
            context: contexts.get(leaf).copied().unwrap_or_default(),
            element,
            offset,
            affinity,
            sort_order,
            outgoing_key: None,
            leaf,
            voice_english: Vec::new(),
        });
    }
    let order = |left: &BrailleElement, right: &BrailleElement| {
        left.offset
            .total_cmp(&right.offset)
            .then(left.sort_order.cmp(&right.sort_order))
    };
    grouping.elements.sort_by(order);
    let count = grouping.elements.len();
    if count >= 2
        && matches!(
            grouping.elements[count - 1].element,
            StreamElement::Dynamic(_)
        )
        && matches!(
            grouping.elements[count - 2].element,
            StreamElement::Barline(_)
        )
    {
        grouping.elements[count - 1].sort_order = -1;
        grouping.elements.sort_by(order);
    }
    grouping
}

/// The type of a note's first beam, as music21's `beams.getByNumber(1)`
/// reads it.
fn first_beam_type(element: &StreamElement) -> Option<String> {
    let beams = match element {
        StreamElement::Note(note) => note.beams(),
        StreamElement::Chord(chord) => chord.beams(),
        _ => return None,
    };
    beams
        .beams()
        .iter()
        .find(|beam| beam.number() == Some(1))
        .and_then(|beam| beam.beam_type())
        .map(|kind| kind.as_str().to_string())
}

/// The whole beat a place falls in, as music21's `int(element.beat)`.
fn whole_beat(meter: Option<&TimeSignature>, offset: FloatType) -> Option<i64> {
    meter
        .and_then(|meter| meter.beat_at_offset(offset).ok())
        .map(i64::from)
}

/// Marks the notes of a measure that begin and continue a beamed group of
/// three or more notes of one value shorter than an eighth, which braille
/// writes as eighths after the first: music21's `prepareBeamedNotes`. The
/// marks go in `contexts`, by element.
pub fn prepare_beamed_notes(
    items: &[(FloatType, StreamElement)],
    meter: Option<&TimeSignature>,
) -> Vec<(bool, bool)> {
    let mut flags = vec![(false, false); items.len()];
    // The notes, and the notes and rests, by place in `items`.
    let notes: Vec<usize> = (0..items.len())
        .filter(|&i| is_not_rest(&items[i].1))
        .collect();
    let general: Vec<usize> = (0..items.len())
        .filter(|&i| is_general_note(&items[i].1))
        .collect();
    let starts: Vec<usize> = notes
        .iter()
        .copied()
        .filter(|&i| first_beam_type(&items[i].1).as_deref() == Some("start"))
        .collect();
    let stops: Vec<usize> = notes
        .iter()
        .copied()
        .filter(|&i| first_beam_type(&items[i].1).as_deref() == Some("stop"))
        .collect();
    if starts.len() != stops.len() {
        return flags;
    }
    let length = |i: usize| items[i].1.quarter_length();
    // music21 compares lengths as exact fractions; a sixth summed one way and
    // another differs as a float.
    let same_length = |a: FloatType, b: FloatType| (a - b).abs() < 1e-9;
    for (start, stop) in starts.into_iter().zip(stops) {
        if length(start) == 0.5 {
            continue;
        }
        let (Some(start_index), Some(stop_index)) = (
            general.iter().position(|&i| i == start),
            general.iter().position(|&i| i == stop),
        ) else {
            continue;
        };
        if (stop_index as i64) - (start_index as i64) + 1 < 3 {
            continue;
        }
        let mut same = (start_index + 1..=stop_index).all(|index| {
            let i = general[index];
            same_length(length(i), length(start)) && !matches!(items[i].1, StreamElement::Rest(_))
        });
        if let Some(&after) = general.get(stop_index + 1)
            && matches!(items[after].1, StreamElement::Rest(_))
            && whole_beat(meter, items[after].0) == whole_beat(meter, items[stop].0)
        {
            same = false;
        }
        if !same {
            continue;
        }
        if let Some(&after) = general.get(stop_index + 1)
            && length(after) == 0.5
        {
            continue;
        }
        flags[start].0 = true;
        if start_index >= 1 {
            let before = general[start_index - 1];
            if matches!(items[before].1, StreamElement::Rest(_))
                && whole_beat(meter, items[before].0) == whole_beat(meter, items[start].0)
                && same_length(length(before), length(start))
            {
                flags[start].1 = true;
            }
        } else {
            // music21 reads the note before the first as the last of the
            // measure, as Python indexes a list from its end.
            let before = general[general.len() - 1];
            if matches!(items[before].1, StreamElement::Rest(_))
                && whole_beat(meter, items[before].0) == whole_beat(meter, items[start].0)
                && same_length(length(before), length(start))
            {
                flags[start].1 = true;
            }
        }
        for index in start_index + 1..=stop_index {
            flags[general[index]].1 = true;
        }
    }
    flags
}

/// The options music21's translation takes, as a segment reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentOptions {
    /// Whether a key signature cancels the one before it.
    pub cancel_outgoing_key_sig: bool,
    /// Whether chords are written from the top down; nothing reads as
    /// upward until a clef says otherwise.
    pub descending_chords: Option<bool>,
    /// How many dummy rests open each segment.
    pub dummy_rest_length: Option<usize>,
    /// How many cells a line holds.
    pub max_line_length: usize,
    /// Whether clefs are written.
    pub show_clef_signs: bool,
    /// Whether a segment starts with its first measure's number.
    pub show_first_measure_number: bool,
    /// Which hand's sign starts the lines.
    pub show_hand: Option<Hand>,
    /// Whether a segment starts with a heading.
    pub show_heading: bool,
    /// Whether a long slur and a tie on one note are both written.
    pub show_long_slurs_and_ties_together: Option<bool>,
    /// Whether a short slur and a tie on one note are both written.
    pub show_short_slurs_and_ties_together: bool,
    /// Whether a long phrase's slur is written with brackets rather than
    /// doubled.
    pub slur_long_phrase_with_brackets: bool,
    /// Whether octave marks are left out.
    pub suppress_octave_marks: bool,
    /// Whether a fingering's upper choice is written first.
    pub upper_first_in_note_fingering: bool,
}

impl Default for SegmentOptions {
    fn default() -> Self {
        Self {
            cancel_outgoing_key_sig: true,
            descending_chords: None,
            dummy_rest_length: None,
            max_line_length: 40,
            show_clef_signs: false,
            show_first_measure_number: true,
            show_hand: None,
            show_heading: true,
            show_long_slurs_and_ties_together: None,
            show_short_slurs_and_ties_together: false,
            slur_long_phrase_with_brackets: true,
            suppress_octave_marks: false,
            upper_first_in_note_fingering: true,
        }
    }
}

/// The most notes a slur written after each note covers: music21's
/// `SEGMENT_MAXNOTESFORSHORTSLUR`.
const MAX_NOTES_FOR_SHORT_SLUR: usize = 4;

/// The most elements a segment holds before a double or final barline
/// ends it: music21's `MAX_ELEMENTS_IN_SEGMENT`.
const MAX_ELEMENTS_IN_SEGMENT: usize = 48;

/// The slurs of a part as each note writes them, by leaf: short slurs after
/// each note, long ones with brackets or doubled at their ends: music21's
/// `prepareSlurredNotes`.
pub fn prepare_slurred_notes(part: &Stream, options: &SegmentOptions) -> Vec<NoteContext> {
    let leaves = part.leaves();
    let mut contexts = vec![NoteContext::default(); leaves.len()];
    if part.spanners().is_empty() {
        return contexts;
    }
    let show_long = options
        .show_long_slurs_and_ties_together
        .unwrap_or(options.slur_long_phrase_with_brackets);
    // The part's notes in music21's flattened order, by leaf.
    let mut notes: Vec<(FloatType, i32, bool, usize)> = leaves
        .iter()
        .enumerate()
        .filter(|(_, (_, element))| is_not_rest(element))
        .map(|(leaf, (offset, element))| {
            let grace = element.duration().is_some_and(Duration::is_grace);
            (*offset, element.class_sort_order(), !grace, leaf)
        })
        .collect();
    notes.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });
    let notes: Vec<usize> = notes.into_iter().map(|(_, _, _, leaf)| leaf).collect();
    let tie_of = |leaf: usize| -> Option<crate::notation::TieType> {
        match leaves[leaf].1 {
            StreamElement::Note(note) => note.tie().map(|tie| tie.tie_type()),
            StreamElement::Chord(chord) => chord.tie().map(|tie| tie.tie_type()),
            _ => None,
        }
    };
    let at = |index: i64| -> usize {
        let count = notes.len() as i64;
        notes[(((index % count) + count) % count) as usize]
    };
    for spanner in part.spanners() {
        if spanner.kind() != crate::spanner::SpannerKind::Slur {
            continue;
        }
        let spanned = spanner.spanned();
        let (Some(Some(first)), Some(Some(last))) = (spanned.first(), spanned.last()) else {
            continue;
        };
        let (Some(begin), Some(end)) = (
            notes.iter().position(|leaf| leaf == first),
            notes.iter().position(|leaf| leaf == last),
        ) else {
            continue;
        };
        let (mut begin, mut end) = (begin as i64, end as i64);
        let delta = (end - begin).unsigned_abs() as usize + 1;
        let short = delta <= MAX_NOTES_FOR_SHORT_SLUR;
        if (short && !options.show_short_slurs_and_ties_together) || (!short && !show_long) {
            if tie_of(at(begin)) == Some(crate::notation::TieType::Start) {
                begin += 1;
            }
            if tie_of(at(end)) == Some(crate::notation::TieType::Stop) {
                end -= 1;
            }
        }
        if short {
            for index in begin..end {
                contexts[at(index)].short_slur = true;
            }
        } else if options.slur_long_phrase_with_brackets {
            contexts[at(begin)].begin_long_bracket_slur = true;
            contexts[at(end)].end_long_bracket_slur = true;
        } else {
            contexts[at(begin + 1)].begin_long_double_slur = true;
            contexts[at(end - 1)].end_long_double_slur = true;
        }
    }
    contexts
}

/// A part cut into segments of groupings, ready to transcribe: music21's
/// `BrailleSegment`.
#[derive(Clone, Debug)]
pub struct BrailleSegment {
    /// The groupings, by key.
    pub groupings: BTreeMap<SegmentKey, BrailleElementGrouping>,
    /// The options it is written with.
    pub options: SegmentOptions,
    /// Whether chords are written from the top down, as the clefs met so
    /// far say.
    pub descending_chords: Option<bool>,
    /// Whether the segment ends in the middle of a measure.
    pub end_hyphen: bool,
    /// Whether it begins in the middle of a measure.
    pub begins_mid_measure: bool,
    /// What music21 marked on each of the part's leaves.
    pub contexts: Vec<NoteContext>,
}

impl BrailleSegment {
    fn new(options: SegmentOptions, contexts: &[NoteContext]) -> Self {
        Self {
            groupings: BTreeMap::new(),
            options,
            descending_chords: options.descending_chords,
            end_hyphen: false,
            begins_mid_measure: false,
            contexts: contexts.to_vec(),
        }
    }
}

/// A key signature's sharps, as braille reads it.
fn sharps_of(element: &StreamElement) -> Option<Option<IntegerType>> {
    match element {
        StreamElement::Key(key) => Some(Some(key.sharps())),
        StreamElement::KeySignature(signature) => Some(signature.sharps()),
        _ => None,
    }
}

/// The part's measures with where each one's leaves start.
fn measures_of(part: &Stream) -> Vec<(&Stream, usize)> {
    let mut out = Vec::new();
    let mut leaf = 0;
    for event in part.events() {
        match event.element() {
            StreamElement::Stream(inner) => {
                if inner.kind() == StreamKind::Measure {
                    out.push((&**inner, leaf));
                }
                leaf += inner.leaves().len();
            }
            _ => leaf += 1,
        }
    }
    out
}

/// The meter standing at each measure of a part, as music21 finds a
/// measure's context: the last a measure stated, or one in the part before
/// it.
fn meters_of(part: &Stream) -> Vec<Option<TimeSignature>> {
    let loose: Vec<(FloatType, TimeSignature)> = part
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some((event.offset(), meter.clone())),
            _ => None,
        })
        .collect();
    let mut stated: Option<TimeSignature> = None;
    let mut out = Vec::new();
    for event in part.events() {
        let StreamElement::Stream(measure) = event.element() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        for inner in measure.events() {
            if let StreamElement::TimeSignature(meter) = inner.element()
                && inner.offset() <= 0.0
            {
                stated = Some(meter.clone());
            }
        }
        let meter = stated.clone().or_else(|| {
            loose
                .iter()
                .rev()
                .find(|(at, _)| *at <= event.offset())
                .map(|(_, meter)| meter.clone())
        });
        out.push(meter);
    }
    out
}

/// A part's measures cut into segments of groupings: music21's
/// `getRawSegments`. A segment ends after a double or final barline once it
/// holds more than 48 elements.
pub fn get_raw_segments(
    part: &Stream,
    hand: Option<Hand>,
    contexts: &[NoteContext],
    options: SegmentOptions,
) -> Vec<BrailleSegment> {
    let mut segments = Vec::new();
    let mut contexts = contexts.to_vec();
    let mut current = BrailleSegment::new(options, &contexts);
    let mut in_current = 0;
    let mut start_new = false;
    let meters = meters_of(part);
    for (index, (measure, leaf_base)) in measures_of(part).into_iter().enumerate() {
        let walk = walk_of(measure, leaf_base);
        let items: Vec<(FloatType, StreamElement)> = walk
            .iter()
            .filter(|(_, element, _)| !matches!(element, StreamElement::Stream(_)))
            .map(|(offset, element, _)| (*offset, element.clone()))
            .collect();
        let leaves: Vec<usize> = walk
            .iter()
            .filter(|(_, element, _)| !matches!(element, StreamElement::Stream(_)))
            .map(|(_, _, leaf)| *leaf)
            .collect();
        let flags = prepare_beamed_notes(&items, meters.get(index).and_then(Option::as_ref));
        for ((item, leaf), (start, continued)) in items.iter().zip(&leaves).zip(flags) {
            if is_not_rest(&item.1) && *leaf < contexts.len() {
                contexts[*leaf].beam_start = start;
                contexts[*leaf].beam_continue = continued;
            }
        }
        let elements = extract_braille_elements(measure, leaf_base, &contexts);
        let number = IntegerType::from(measure.number());
        let mut ordinal = 0;
        let mut previous: Option<Affinity> = None;
        for element in elements.elements {
            if start_new {
                if element.offset != 0.0 {
                    current.end_hyphen = true;
                }
                segments.push(std::mem::replace(
                    &mut current,
                    BrailleSegment::new(options, &contexts),
                ));
                if element.offset != 0.0 {
                    current.begins_mid_measure = true;
                }
                in_current = 0;
                if previous.is_some() {
                    ordinal += 1;
                }
                start_new = false;
            }
            if let StreamElement::Barline(barline) = &element.element
                && in_current > MAX_ELEMENTS_IN_SEGMENT
                && matches!(barline.bar_type().as_str(), "double" | "final")
            {
                start_new = true;
            }
            if previous.is_some_and(|previous| element.affinity < previous) {
                ordinal += 1;
            }
            let affinity = match element.affinity {
                Affinity::SplitNoteGroupingA => Affinity::Inaccord,
                Affinity::SplitNoteGroupingB => Affinity::NoteGrouping,
                other => other,
            };
            let key = SegmentKey {
                measure: number,
                ordinal,
                affinity,
                hand,
            };
            previous = Some(element.affinity);
            current
                .groupings
                .entry(key)
                .or_default()
                .elements
                .push(element);
            in_current += 1;
        }
    }
    current.contexts = contexts;
    segments.push(current);
    segments
}

/// Whether two elements are equal as music21 compares them.
fn elements_equal(left: &BrailleElement, right: &BrailleElement) -> bool {
    super::equality::equal(&left.element, &right.element)
}

/// Whether two groupings hold equal elements: music21's
/// `areGroupingsIdentical`.
fn groupings_identical(left: &[BrailleElement], right: &[BrailleElement]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| elements_equal(left, right))
}

impl BrailleSegment {
    /// Repeated measures counted, key signatures given their predecessors,
    /// full measures' rests marked and each grouping told the signatures
    /// and options in force: music21's `addGroupingAttributes`.
    pub fn add_grouping_attributes(&mut self) {
        let mut current_key: Option<IntegerType> = Some(0);
        let mut current_meter = TimeSignature::new(4, 4).expect("4/4 is a meter");
        let keys: Vec<SegmentKey> = self.groupings.keys().copied().collect();
        let mut previous: Option<SegmentKey> = None;
        for key in keys {
            if let Some(previous_key) = previous {
                if key.ordinal >= 1 {
                    self.groupings
                        .get_mut(&previous_key)
                        .expect("a grouping")
                        .with_hyphen = true;
                }
                if previous_key.ordinal == 0
                    && previous_key.affinity == Affinity::NoteGrouping
                    && key.ordinal == 0
                    && key.affinity == Affinity::NoteGrouping
                {
                    let previous_elements = &self.groupings[&previous_key].elements;
                    let these = &self.groupings[&key].elements;
                    let repeated = if previous_elements
                        .first()
                        .is_some_and(|first| matches!(first.element, StreamElement::Clef(_)))
                    {
                        groupings_identical(&previous_elements[1..], these)
                    } else {
                        groupings_identical(previous_elements, these)
                    };
                    if repeated {
                        self.groupings
                            .get_mut(&previous_key)
                            .expect("a grouping")
                            .num_repeats += 1;
                        self.groupings.remove(&key);
                        continue;
                    }
                }
            }
            let descending = &mut self.descending_chords;
            let grouping = self.groupings.get_mut(&key).expect("a grouping");
            if key.affinity == Affinity::Signature {
                for element in &mut grouping.elements {
                    if let StreamElement::TimeSignature(meter) = &element.element {
                        current_meter = meter.clone();
                    } else if let Some(sharps) = sharps_of(&element.element) {
                        element.outgoing_key = Some(current_key);
                        current_key = sharps;
                    }
                }
            } else if matches!(key.affinity, Affinity::Inaccord | Affinity::NoteGrouping) {
                if let Some(StreamElement::Clef(clef)) =
                    grouping.elements.first().map(|e| &e.element)
                {
                    match clef.kind().class_name() {
                        "TrebleClef" | "Treble8vbClef" | "Treble8vaClef" | "AltoClef" => {
                            *descending = Some(true);
                        }
                        "BassClef" | "Bass8vbClef" | "Bass8vaClef" | "TenorClef" => {
                            *descending = Some(false);
                        }
                        _ => {}
                    }
                }
                let general: Vec<usize> = (0..grouping.elements.len())
                    .filter(|&i| is_general_note(&grouping.elements[i].element))
                    .collect();
                if let [only] = general.as_slice()
                    && let StreamElement::Rest(rest) = &mut grouping.elements[*only].element
                {
                    rest.set_full_measure(Some(true));
                }
            }
            grouping.key_signature = current_key;
            grouping.time_signature = current_meter.clone();
            grouping.descending_chords = *descending;
            grouping.show_clef_signs = self.options.show_clef_signs;
            grouping.upper_first_in_note_fingering = self.options.upper_first_in_note_fingering;
            previous = Some(key);
        }
        if self.end_hyphen
            && let Some(last) = previous
        {
            self.groupings
                .get_mut(&last)
                .expect("a grouping")
                .with_hyphen = true;
        }
    }

    /// Runs of three or more notes with one articulation written with it
    /// doubled on the first and once on the last, and a staccato or tenuto
    /// over a tie written as a short slur: music21's `fixArticulations`.
    pub fn fix_articulations(&mut self) {
        // music21's `consolidate`: each run of note groupings with nothing
        // else between, as one.
        let mut runs: Vec<Vec<(SegmentKey, usize)>> = Vec::new();
        let mut in_run = false;
        for (key, grouping) in &self.groupings {
            if key.affinity != Affinity::NoteGrouping {
                in_run = false;
                continue;
            }
            if !in_run {
                runs.push(Vec::new());
                in_run = true;
            }
            let run = runs.last_mut().expect("a run");
            for (index, element) in grouping.elements.iter().enumerate() {
                if matches!(element.element, StreamElement::Note(_)) {
                    run.push((*key, index));
                }
            }
        }
        for notes in runs {
            for start in 0..notes.len() {
                let mut article = 0;
                while let Some(articulation) = self
                    .note(notes[start])
                    .articulations()
                    .get(article)
                    .cloned()
                {
                    article += 1;
                    if articulation.is_a("TechnicalIndication") && !articulation.is_a("Bowing") {
                        continue;
                    }
                    self.fix_one_articulation(&articulation, &notes, start);
                }
            }
        }
    }

    fn note(&self, (key, index): (SegmentKey, usize)) -> &Note {
        match &self.groupings[&key].elements[index].element {
            StreamElement::Note(note) => note,
            _ => unreachable!("only notes are gathered"),
        }
    }

    fn element_mut(&mut self, (key, index): (SegmentKey, usize)) -> &mut BrailleElement {
        &mut self.groupings.get_mut(&key).expect("a grouping").elements[index]
    }

    fn note_mut(&mut self, place: (SegmentKey, usize)) -> &mut Note {
        match &mut self.element_mut(place).element {
            StreamElement::Note(note) => note,
            _ => unreachable!("only notes are gathered"),
        }
    }

    fn fix_one_articulation(
        &mut self,
        articulation: &crate::articulations::Articulation,
        notes: &[(SegmentKey, usize)],
        start: usize,
    ) {
        let name = articulation.name();
        let count = notes.len() as i64;
        let wrap = |index: i64| notes[(((index % count) + count) % count) as usize];
        if (articulation.is_a("Staccato") || articulation.is_a("Tenuto"))
            && let Some(tie) = self.note(notes[start]).tie().map(|tie| tie.tie_type())
        {
            if tie == crate::notation::TieType::Stop {
                let before = wrap(start as i64 - 1);
                self.note_mut(before).set_tie(None);
                self.element_mut(before).context.short_slur = true;
            } else {
                let after = wrap(start as i64 + 1);
                self.note_mut(after).set_tie(None);
                self.element_mut(notes[start]).context.short_slur = true;
            }
            self.note_mut(notes[start]).set_tie(None);
        }
        let mut sequential = 0;
        for &place in &notes[start + 1..] {
            if self
                .note(place)
                .articulations()
                .iter()
                .any(|other| other.name() == name)
            {
                sequential += 1;
                continue;
            }
            break;
        }
        if sequential < 3 {
            return;
        }
        self.note_mut(notes[start])
            .articulations_mut()
            .push(articulation.clone());
        for &place in &notes[start + 1..start + sequential] {
            let articulations = self.note_mut(place).articulations_mut();
            // music21 removes while iterating, which skips the element after
            // each one removed.
            let mut index = 0;
            while index < articulations.len() {
                if articulations[index].name() == name {
                    drop(articulations.remove(index));
                }
                index += 1;
            }
        }
    }
}

/// A part's segments, each told its options, its groupings their
/// attributes and its articulations fixed: music21's `findSegments`.
pub fn find_segments(
    part: &Stream,
    hand: Option<Hand>,
    options: SegmentOptions,
) -> Vec<BrailleSegment> {
    let contexts = prepare_slurred_notes(part, &options);
    let mut segments = get_raw_segments(part, hand, &contexts, options);
    for segment in &mut segments {
        segment.add_grouping_attributes();
        segment.fix_articulations();
    }
    segments
}

/// Writes the elements of a grouping one after another, octaves where
/// needed: music21's `NoteGroupingTranscriber`.
#[derive(Clone, Debug)]
pub struct NoteGroupingTranscriber {
    /// Whether the first note shows its octave.
    pub show_leading_octave: bool,
    previous_note: Option<Pitch>,
    previous_element: Option<usize>,
    trans: Vec<String>,
}

impl Default for NoteGroupingTranscriber {
    fn default() -> Self {
        Self {
            show_leading_octave: true,
            previous_note: None,
            previous_element: None,
            trans: Vec::new(),
        }
    }
}

impl NoteGroupingTranscriber {
    /// A grouping in braille, its elements' English written onto them, and
    /// a music hyphen where it ends with one: music21's `transcribeGroup`.
    ///
    /// # Errors
    ///
    /// Empty text before something dotted after it, which music21 cannot
    /// look at the end of.
    pub fn transcribe_group(&mut self, grouping: &mut BrailleElementGrouping) -> Result<String> {
        self.previous_note = None;
        self.previous_element = None;
        self.trans = Vec::new();
        for index in 0..grouping.elements.len() {
            self.transcribe_one_element(grouping, index)?;
        }
        if grouping.with_hyphen {
            self.trans.push(symbol("music_hyphen"));
        }
        Ok(self.trans.concat())
    }

    fn octave_shown(&self, pitch: &Pitch) -> bool {
        match &self.previous_note {
            None => self.show_leading_octave,
            Some(previous) => basic::show_octave_with_note(Some(previous), pitch),
        }
    }

    fn transcribe_one_element(
        &mut self,
        grouping: &mut BrailleElementGrouping,
        index: usize,
    ) -> Result<()> {
        let descending = grouping.descending_chords.unwrap_or(false);
        let show_clef_signs = grouping.show_clef_signs;
        let upper_first = grouping.upper_first_in_note_fingering;
        let item = &mut grouping.elements[index];
        let written: Option<Transcription> = match &item.element {
            StreamElement::Note(note) => {
                let shown = self.octave_shown(note.pitch());
                let written = note_to_braille(note, shown, upper_first, item.context);
                self.previous_note = Some(note.pitch().clone());
                Some(written)
            }
            StreamElement::Rest(rest) => Some(rest_to_braille(rest)),
            StreamElement::Chord(chord) => {
                let mut pitches = chord.pitches();
                pitches.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
                let chosen = if descending {
                    pitches.last().cloned()
                } else {
                    pitches.first().cloned()
                };
                chosen.map(|pitch| {
                    let shown = self.octave_shown(&pitch);
                    let written = chord_to_braille(chord, descending, shown);
                    self.previous_note = Some(pitch);
                    written
                })
            }
            StreamElement::ChordSymbol(symbol) => symbol.pitches().ok().and_then(|pitches| {
                let mut chord = crate::chord::Chord::new(pitches.as_slice()).ok()?;
                chord.set_duration(symbol.duration().clone());
                let mut sorted = pitches.clone();
                sorted.sort_by(|left, right| left.ps().total_cmp(&right.ps()));
                let chosen = if descending {
                    sorted.last()
                } else {
                    sorted.first()
                }
                .cloned()?;
                let shown = self.octave_shown(&chosen);
                let written = chord_to_braille(&chord, descending, shown);
                self.previous_note = Some(chosen);
                Some(written)
            }),
            StreamElement::Dynamic(dynamic) => {
                self.previous_note = None;
                self.show_leading_octave = true;
                Some(dynamic_to_braille(dynamic, true))
            }
            StreamElement::TextExpression(text) => {
                self.previous_note = None;
                self.show_leading_octave = true;
                Some(text_expression_to_braille(text, true))
            }
            StreamElement::Barline(barline) => Some(barline_to_braille(barline)),
            StreamElement::Clef(clef) => {
                if show_clef_signs {
                    self.previous_note = None;
                    self.show_leading_octave = true;
                    Some(clef_to_braille(clef, false))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(written) = written {
            item.english = Some(written.english);
            self.trans.push(written.braille);
        }
        self.optionally_add_dot_to_previous(grouping, index)?;
        self.previous_element = Some(index);
        Ok(())
    }

    /// A dot after a dynamic, a clef written out, or text not ending in a
    /// full stop, before the next element unless that is itself a dynamic
    /// or text: music21's `optionallyAddDotToPrevious`.
    fn optionally_add_dot_to_previous(
        &mut self,
        grouping: &mut BrailleElementGrouping,
        index: usize,
    ) -> Result<()> {
        let Some(previous) = self.previous_element else {
            return Ok(());
        };
        let Some(last) = self.trans.last() else {
            return Ok(());
        };
        if matches!(
            grouping.elements[index].element,
            StreamElement::Dynamic(_) | StreamElement::TextExpression(_)
        ) {
            return Ok(());
        }
        let dotted = match &grouping.elements[previous].element {
            StreamElement::Dynamic(_) => true,
            StreamElement::Clef(_) => grouping.show_clef_signs,
            StreamElement::TextExpression(text) => match text.content().chars().last() {
                Some(character) => character != '.',
                None => return Err(Error::Notation("string index out of range".to_string())),
            },
            _ => false,
        };
        if !dotted {
            return Ok(());
        }
        let Some(first) = last.chars().next() else {
            return Ok(());
        };
        if let Some(dot) = yield_dots(first).into_iter().next() {
            let at = self.trans.len() - 1;
            self.trans.insert(at, dot.clone());
            grouping.elements[previous]
                .english
                .get_or_insert_with(Vec::new)
                .push(format!("Dot 3 {dot}"));
        }
        Ok(())
    }
}

/// A grouping in braille from a fresh transcriber: music21's
/// `transcribeNoteGrouping`.
///
/// # Errors
///
/// What [`NoteGroupingTranscriber::transcribe_group`] refuses.
pub fn transcribe_note_grouping(
    grouping: &mut BrailleElementGrouping,
    show_leading_octave: bool,
) -> Result<String> {
    let mut transcriber = NoteGroupingTranscriber {
        show_leading_octave,
        ..NoteGroupingTranscriber::default()
    };
    transcriber.transcribe_group(grouping)
}

/// A measure of groupings split in two at the middle of its bar, or nearer
/// its start by `beat_division_offset` divisions of a beat, tied notes kept
/// with the first half and each half beamed again: music21's
/// `splitNoteGrouping`.
///
/// # Errors
///
/// An offset past the beat's divisions, or a meter that cannot be halved.
pub fn split_note_grouping(
    grouping: &mut BrailleElementGrouping,
    beat_division_offset: usize,
) -> Result<Option<(BrailleElementGrouping, BrailleElementGrouping)>> {
    Ok(split_with_sources(grouping, beat_division_offset)?
        .map(|((first, _), (second, _))| (first, second)))
}

/// A grouping and which of the split grouping's elements it holds.
type Half = (BrailleElementGrouping, Vec<usize>);

/// An element of a half rebeamed: where it came from, as beamed, and its
/// marks for beginning and continuing a beamed group.
type Rebeamed = (usize, StreamElement, (bool, bool));

/// [`split_note_grouping`], each half with where its elements came from.
fn split_with_sources(
    grouping: &mut BrailleElementGrouping,
    beat_division_offset: usize,
) -> Result<Option<(Half, Half)>> {
    let meter = grouping.time_signature.clone();
    let mut offset = 0.0;
    if beat_division_offset != 0 {
        let divisions = meter.beat_division_durations()?;
        if beat_division_offset > divisions.len() {
            // music21's `BrailleSegmentException`, which the segment takes
            // as no split.
            return Ok(None);
        }
        if let Some(duration) = divisions.get(divisions.len() - beat_division_offset) {
            offset = op_frac(offset + op_frac(duration.quarter_length()));
        }
    }
    let mut beats = meter.beat_sequence().clone();
    let (start_zero, mut end_zero) = match beats.partition_by_count(2, false) {
        Ok(()) => beats.level_span(0)[0],
        Err(_) => {
            beats.partition_by_count(3, false)?;
            let spans = beats.level_span(0);
            (spans[0].0, spans[spans.len() - 2].1)
        }
    };
    end_zero -= offset;
    let elements = &grouping.elements;
    // music21 iterates the measure it builds from the grouping, sorted by
    // offset and sort order.
    let mut order: Vec<usize> = (0..elements.len()).collect();
    order.sort_by(|&a, &b| {
        elements[a]
            .offset
            .total_cmp(&elements[b].offset)
            .then(elements[a].sort_order.cmp(&elements[b].sort_order))
    });
    let mut left: Vec<usize> = Vec::new();
    let mut right: Vec<usize> = Vec::new();
    for &index in &order {
        let at = elements[index].offset;
        let barline = matches!(elements[index].element, StreamElement::Barline(_));
        if at >= start_zero && (at < end_zero || (at == end_zero && barline)) {
            left.push(index);
        } else {
            right.push(index);
        }
    }
    let highest = elements
        .iter()
        .map(|element| op_frac(element.offset + element.element.quarter_length()))
        .fold(0.0, FloatType::max);
    let mut offsets: Vec<FloatType> = elements.iter().map(|element| element.offset).collect();
    // Tied notes at the start of the second half go with the first, put
    // after what it holds.
    while let Some(position) = right
        .iter()
        .position(|&i| is_not_rest(&elements[i].element))
    {
        let index = right[position];
        let tied = match &elements[index].element {
            StreamElement::Note(note) => note.tie().is_some(),
            StreamElement::Chord(chord) => chord.tie().is_some(),
            _ => false,
        };
        if !tied {
            break;
        }
        offsets[index] = left
            .iter()
            .map(|&i| op_frac(offsets[i] + elements[i].element.quarter_length()))
            .fold(0.0, FloatType::max);
        left.push(index);
        right.remove(position);
        end_zero += elements[index].element.quarter_length();
    }
    // Each half beamed as a measure of its own, filled out with a rest, and
    // its beamed notes marked again.
    let rebeam =
        |indices: &[usize], rest_at: FloatType, rest_length: FloatType| -> Result<Vec<Rebeamed>> {
            // music21 makes the rest with `quarterLength=`, which it passes over
            // when nought, leaving a quarter rest that may overfill the bar and
            // keep it from being beamed again.
            let rest_length = if rest_length == 0.0 { 1.0 } else { rest_length };
            let rest = Rest::new(Duration::new(rest_length.max(0.0))?);
            // The measure's events in the order a stream sorts them: by offset,
            // in the order they were put in.
            let mut placed: Vec<(FloatType, Option<usize>, StreamElement)> = indices
                .iter()
                .map(|&i| (offsets[i], Some(i), elements[i].element.clone()))
                .collect();
            placed.push((rest_at, None, StreamElement::Rest(rest)));
            placed.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.insert(0.0, meter.clone());
            for (at, _, element) in &placed {
                measure.insert(*at, element.clone());
            }
            crate::makenotation::make_beams(&mut measure)?;
            let beamed: Vec<(FloatType, StreamElement)> = measure
                .events()
                .iter()
                .filter(|event| !matches!(event.element(), StreamElement::TimeSignature(_)))
                .map(|event| (event.offset(), event.element().clone()))
                .collect();
            let flags = prepare_beamed_notes(&beamed, Some(&meter));
            Ok(placed
                .iter()
                .zip(beamed)
                .zip(flags)
                .filter_map(|(((_, source, _), (_, element)), flags)| {
                    source.map(|source| (source, element, flags))
                })
                .collect())
        };
    let left_beamed = rebeam(&left, end_zero, highest - end_zero)?;
    let right_beamed = rebeam(&right, 0.0, end_zero)?;
    // music21 changes the grouping's own elements: their offsets, beams and
    // marks stay as the split left them.
    for (index, element, (start, continued)) in left_beamed.iter().chain(&right_beamed) {
        let item = &mut grouping.elements[*index];
        item.offset = offsets[*index];
        item.element = element.clone();
        if is_not_rest(&item.element) {
            item.context.beam_start = *start;
            item.context.beam_continue = *continued;
        }
    }
    let half = |beamed: &[Rebeamed]| -> Half {
        let mut half = grouping.clone();
        half.elements = beamed
            .iter()
            .map(|(index, _, _)| grouping.elements[*index].clone())
            .collect();
        (half, beamed.iter().map(|(index, _, _)| *index).collect())
    };
    Ok(Some((half(&left_beamed), half(&right_beamed))))
}

impl BrailleSegment {
    /// The segment in braille: its heading and first measure number, then
    /// each grouping in turn: music21's `transcribe`.
    ///
    /// # Errors
    ///
    /// Something too wide for a line, or that music21 cannot write.
    pub fn transcribe(&mut self) -> Result<String> {
        let hand = match self.options.show_hand {
            Some(Hand::Right) => Some("right"),
            Some(Hand::Left) => Some("left"),
            None => None,
        };
        let mut writer = SegmentWriter {
            text: BrailleText::new(self.options.max_line_length, hand)?,
            keys: self.groupings.keys().copied().collect(),
            current: None,
            previous: None,
            last_note: None,
        };
        if self.options.show_heading {
            writer.extract_heading(self)?;
        }
        if self.options.show_first_measure_number {
            writer.extract_measure_number(self)?;
        }
        if let Some(length) = self.options.dummy_rest_length {
            let dummy = lookup::rest("dummy").unwrap_or_default().repeat(length);
            writer.text.add_signatures(&dummy)?;
        }
        writer.previous = None;
        while let Some(key) = writer.keys.pop_front() {
            writer.current = Some(key);
            match key.affinity {
                Affinity::NoteGrouping => writer.extract_note_grouping(self)?,
                Affinity::Signature => writer.extract_signature_grouping(self)?,
                Affinity::LongTextExpression => writer.extract_long_expression_grouping(self)?,
                Affinity::Inaccord => writer.extract_inaccord_grouping(self)?,
                Affinity::TempoText => writer.extract_tempo_text_grouping(self)?,
                _ => {}
            }
            writer.previous = writer.current;
        }
        Ok(writer.text.render())
    }

    /// The segment's groupings in English, as music21's `__str__` lists
    /// them.
    pub fn english(&self) -> String {
        let mut entries = Vec::new();
        let mut previous: Option<SegmentKey> = None;
        for (key, grouping) in &self.groupings {
            if previous.is_some_and(|previous| previous.affinity == Affinity::SplitNoteGroupingA) {
                previous = Some(*key);
                continue;
            }
            entries.push(format!(
                "Measure {}, {} {}:\n{}\n===",
                key.measure,
                key.affinity.name(),
                key.ordinal + 1,
                grouping_english(grouping)
            ));
            previous = Some(*key);
        }
        [
            "---begin segment---".to_string(),
            "<music21.braille.segment BrailleSegment>".to_string(),
            entries.join("\n"),
            "---end segment---".to_string(),
        ]
        .join("\n")
    }
}

/// A grouping's English, element by element: music21's
/// `BrailleElementGrouping.__str__`.
fn grouping_english(grouping: &BrailleElementGrouping) -> String {
    let mut lines = Vec::new();
    let mut previous_was_voice = false;
    for element in &grouping.elements {
        if let StreamElement::Stream(voice) = &element.element
            && voice.kind() == StreamKind::Voice
        {
            if previous_was_voice {
                lines.push(format!("full inaccord {}", symbol("full_inaccord")));
            }
            for (index, inner) in voice.events().iter().enumerate() {
                lines.push(match element.voice_english.get(index) {
                    Some(Some(english)) => english.join("\n"),
                    _ => element_repr(inner.element()),
                });
            }
            previous_was_voice = true;
        } else {
            lines.push(match &element.english {
                Some(english) => english.join("\n"),
                None => element_repr(&element.element),
            });
            previous_was_voice = false;
        }
    }
    if grouping.num_repeats > 0 {
        lines.push(format!("** Grouping x {} **", grouping.num_repeats + 1));
    }
    if grouping.with_hyphen {
        lines.push(format!("music hyphen {}", symbol("music_hyphen")));
    }
    lines.join("\n")
}

/// The English of each element of a voice, from the grouping it was
/// written in, found by where each stands among the part's leaves.
fn voice_english_of(
    voice: &Stream,
    leaf_base: usize,
    written: &BrailleElementGrouping,
) -> Vec<Option<Vec<String>>> {
    let mut leaf = leaf_base;
    voice
        .events()
        .iter()
        .map(|event| {
            let english = written
                .elements
                .iter()
                .find(|element| {
                    element.leaf == leaf && !matches!(element.element, StreamElement::Stream(_))
                })
                .and_then(|element| element.english.clone());
            leaf += match event.element() {
                StreamElement::Stream(inner) => inner.leaves().len(),
                _ => 1,
            };
            english
        })
        .collect()
}

/// The English music21 writes on the key signature, time signature, tempo
/// words and metronome mark of a heading, in that order: on what it
/// transcribes on the way, which leaves out signatures with nothing to
/// write and a metronome mark with no number.
fn heading_english(
    sharps: Option<Option<IntegerType>>,
    meter: Option<&TimeSignature>,
    tempo: Option<&crate::tempo::TempoText>,
    mark: Option<&crate::tempo::MetronomeMark>,
    line_length: usize,
) -> [Option<Vec<String>>; 4] {
    let mut out: [Option<Vec<String>>; 4] = [None, None, None, None];
    out[2] = tempo.map(|text| basic::tempo_to_braille(text, line_length).english);
    if sharps.is_some() || meter.is_some() || mark.is_some() {
        out[3] = mark.and_then(|found| {
            basic::metronome_mark_to_braille(found).map(|written| written.english)
        });
        let empty = meter.is_none() && sharps.is_none_or(|sharps| sharps == Some(0));
        if !empty {
            out[0] = sharps.map(|sharps| basic::key_sig_to_braille(sharps, None).english);
            out[1] = meter.map(|time| basic::time_sig_to_braille(time).english);
        }
    }
    out
}

/// The state of a segment being written.
struct SegmentWriter {
    text: BrailleText,
    keys: VecDeque<SegmentKey>,
    current: Option<SegmentKey>,
    previous: Option<SegmentKey>,
    last_note: Option<Pitch>,
}

impl SegmentWriter {
    fn extract_measure_number(&mut self, segment: &BrailleSegment) -> Result<()> {
        let first = self
            .keys
            .front()
            .copied()
            .or_else(|| segment.groupings.keys().next().copied());
        let Some(first) = first else {
            return Ok(());
        };
        let mut number = number_to_braille(&first.measure.to_string(), true, false)?;
        if segment.begins_mid_measure {
            number.push_str(&symbol("dot"));
        }
        self.text.add_measure_number(&number)
    }

    fn extract_heading(&mut self, segment: &mut BrailleSegment) -> Result<()> {
        let mut sharps: Option<Option<IntegerType>> = None;
        let mut meter: Option<TimeSignature> = None;
        let mut tempo: Option<crate::tempo::TempoText> = None;
        let mut mark: Option<crate::tempo::MetronomeMark> = None;
        // music21 works through the keys still to write, or, with none
        // left, through a fresh list of them all that it then drops.
        let from_writer = !self.keys.is_empty();
        let mut keys: VecDeque<SegmentKey> = if from_writer {
            std::mem::take(&mut self.keys)
        } else {
            segment.groupings.keys().copied().collect()
        };
        let mut key_place: Option<(SegmentKey, usize)> = None;
        let mut meter_place: Option<(SegmentKey, usize)> = None;
        let mut tempo_place: Option<(SegmentKey, usize)> = None;
        let mut mark_place: Option<(SegmentKey, usize)> = None;
        while let Some(&key) = keys.front() {
            if key.affinity > Affinity::MetronomeMark {
                break;
            }
            keys.pop_front();
            let grouping = &segment.groupings[&key];
            match key.affinity {
                Affinity::Signature => {
                    if grouping.elements.len() >= 2 {
                        sharps = sharps_of(&grouping.elements[0].element);
                        key_place = sharps.map(|_| (key, 0));
                        if let StreamElement::TimeSignature(time) = &grouping.elements[1].element {
                            meter = Some(time.clone());
                            meter_place = Some((key, 1));
                        }
                    } else if let Some(only) = grouping.elements.first() {
                        if let Some(found) = sharps_of(&only.element) {
                            sharps = Some(found);
                            key_place = Some((key, 0));
                        } else if let StreamElement::TimeSignature(time) = &only.element {
                            meter = Some(time.clone());
                            meter_place = Some((key, 0));
                        }
                    }
                }
                Affinity::TempoText => {
                    if let Some(StreamElement::TempoText(text)) =
                        grouping.elements.first().map(|e| &e.element)
                    {
                        tempo = Some(text.clone());
                        tempo_place = Some((key, 0));
                    }
                }
                Affinity::MetronomeMark => {
                    if let Some(StreamElement::MetronomeMark(found)) =
                        grouping.elements.first().map(|e| &e.element)
                    {
                        mark = Some(found.clone());
                        mark_place = Some((key, 0));
                    }
                }
                _ => {}
            }
        }
        if from_writer {
            self.keys = keys;
        }
        if sharps.is_none() && meter.is_none() && tempo.is_none() && mark.is_none() {
            return Ok(());
        }
        let line_length = self.text.line_length;
        let heading = transcribe_heading(
            sharps,
            meter.as_ref(),
            tempo.as_ref(),
            mark.as_ref(),
            line_length,
        )?;
        // What music21 transcribes on its way to the heading takes its
        // English: the tempo words, the metronome mark with a number, and
        // the signatures unless there are none to write.
        let mut set = |place: Option<(SegmentKey, usize)>, english: Option<Vec<String>>| {
            if let (Some((key, index)), Some(english)) = (place, english) {
                segment
                    .groupings
                    .get_mut(&key)
                    .expect("a grouping")
                    .elements[index]
                    .english = Some(english);
            }
        };
        if let Some(text) = &tempo {
            set(
                tempo_place,
                Some(basic::tempo_to_braille(text, line_length).english),
            );
        }
        if sharps.is_some() || meter.is_some() || mark.is_some() {
            if let Some(found) = &mark {
                set(
                    mark_place,
                    basic::metronome_mark_to_braille(found).map(|written| written.english),
                );
            }
            let empty = meter.is_none() && sharps.is_none_or(|sharps| sharps == Some(0));
            if !empty {
                if let Some(sharps) = sharps {
                    set(
                        key_place,
                        Some(basic::key_sig_to_braille(sharps, None).english),
                    );
                }
                if let Some(time) = &meter {
                    set(meter_place, Some(basic::time_sig_to_braille(time).english));
                }
            }
        }
        self.text.add_heading(&heading)
    }

    fn extract_inaccord_grouping(&mut self, segment: &mut BrailleSegment) -> Result<()> {
        let key = self.current.expect("a grouping being written");
        let grouping = segment.groupings[&key].clone();
        let mut pending_clef: Option<BrailleElement> = None;
        let mut seen_voice = false;
        let mut written: Vec<(usize, Vec<Option<Vec<String>>>)> = Vec::new();
        for element in &grouping.elements {
            if matches!(element.element, StreamElement::Clef(_)) {
                pending_clef = Some(element.clone());
                continue;
            }
            let StreamElement::Stream(voice) = &element.element else {
                continue;
            };
            let mut inner = extract_braille_elements(voice, element.leaf, &segment.contexts);
            if let Some(clef) = pending_clef.take() {
                inner.elements.insert(0, clef);
            }
            inner.descending_chords = grouping.descending_chords;
            inner.show_clef_signs = grouping.show_clef_signs;
            inner.upper_first_in_note_fingering = grouping.upper_first_in_note_fingering;
            let mut braille = if seen_voice {
                symbol("full_inaccord")
            } else {
                String::new()
            };
            braille.push_str(&transcribe_note_grouping(&mut inner, true)?);
            written.push((element.leaf, voice_english_of(voice, element.leaf, &inner)));
            self.text.add_inaccord(&braille)?;
            seen_voice = true;
        }
        let stored = segment.groupings.get_mut(&key).expect("a grouping");
        for (leaf, english) in written {
            if let Some(element) = stored.elements.iter_mut().find(|element| {
                element.leaf == leaf && matches!(element.element, StreamElement::Stream(_))
            }) {
                element.voice_english = english;
            }
        }
        Ok(())
    }

    fn extract_long_expression_grouping(&mut self, segment: &mut BrailleSegment) -> Result<()> {
        let key = self.current.expect("a grouping being written");
        let grouping = segment.groupings.get_mut(&key).expect("a grouping");
        if let Some(first) = grouping.elements.first_mut()
            && let StreamElement::TextExpression(text) = &first.element
        {
            let written = text_expression_to_braille(text, true);
            first.english = Some(written.english);
            self.text.add_long_expression(&written.braille)?;
        }
        Ok(())
    }

    fn show_leading_octave_from(
        &mut self,
        grouping: &BrailleElementGrouping,
        suppress: bool,
    ) -> bool {
        if let (Some(previous), Some(current)) = (self.previous, self.current)
            && (previous.affinity != Affinity::NoteGrouping
                || current.affinity != Affinity::NoteGrouping
                || (current.measure == previous.measure
                    && current.ordinal == previous.ordinal + 1
                    && current.hand == previous.hand))
        {
            self.last_note = None;
        }
        if suppress {
            return false;
        }
        let notes: Vec<&Pitch> = grouping
            .elements
            .iter()
            .filter_map(|element| match &element.element {
                StreamElement::Note(note) => Some(note.pitch()),
                _ => None,
            })
            .collect();
        let mut show = true;
        if let (Some(first), Some(last)) = (notes.first(), notes.last()) {
            if let Some(previous) = &self.last_note {
                show = basic::show_octave_with_note(Some(previous), first);
            }
            self.last_note = Some((*last).clone());
        }
        show
    }

    fn needs_split_to_fit(&mut self, braille: &str) -> bool {
        let quarter = self.text.line_length / 4;
        let left = self.text.line_length - self.text.current_line().text_location;
        left > quarter && braille.chars().count() > quarter
    }

    fn extract_note_grouping(&mut self, segment: &mut BrailleSegment) -> Result<()> {
        let key = self.current.expect("a grouping being written");
        let suppress = segment.options.suppress_octave_marks;
        let mut grouping = segment.groupings[&key].clone();
        let show_leading = self.show_leading_octave_from(&grouping, suppress);
        let mut transcriber = NoteGroupingTranscriber {
            show_leading_octave: show_leading,
            ..NoteGroupingTranscriber::default()
        };
        let mut braille = transcriber.transcribe_group(&mut grouping)?;
        let add_space = self
            .text
            .optional_add_keyboard_symbols_and_dots(Some(&braille))?;
        if self.text.current_line().can_append(&braille, add_space) {
            self.text.current_line().append(&braille, add_space)?;
        } else {
            let mut split = false;
            if self.needs_split_to_fit(&braille)
                && let Some((first, second)) =
                    self.split_and_transcribe(segment, &mut grouping, show_leading, add_space)?
            {
                split = true;
                self.text.current_line().append(&first, add_space)?;
                self.text.add_to_new_line(&second)?;
            }
            if !split {
                if !show_leading && !suppress {
                    transcriber.show_leading_octave = true;
                    braille = transcriber.transcribe_group(&mut grouping)?;
                }
                self.text.current_line().last_hyphen_to_space();
                self.text.add_to_new_line(&braille)?;
            }
        }
        segment.groupings.insert(key, grouping.clone());
        self.add_repeat_symbols(grouping.num_repeats)?;
        Ok(())
    }

    fn split_and_transcribe(
        &mut self,
        segment: &mut BrailleSegment,
        grouping: &mut BrailleElementGrouping,
        show_leading_octave_on_first: bool,
        add_space_to_first: bool,
    ) -> Result<Option<(String, String)>> {
        let mut transcriber = NoteGroupingTranscriber::default();
        let mut offset = 0;
        let mut halves = None;
        let mut first_braille = String::new();
        while offset < 10 {
            let Some(((mut first, sources), second)) = split_with_sources(grouping, offset)? else {
                return Ok(None);
            };
            transcriber.show_leading_octave = show_leading_octave_on_first;
            first.with_hyphen = true;
            first_braille = transcriber.transcribe_group(&mut first)?;
            // The halves hold the grouping's own elements in music21, so what
            // writing the first half says of each stays on it.
            for (element, source) in first.elements.iter().zip(sources) {
                grouping.elements[source].english = element.english.clone();
            }
            let fits = self
                .text
                .current_line()
                .can_append(&first_braille, add_space_to_first);
            halves = Some((first, second));
            if fits {
                break;
            }
            offset += 1;
        }
        let (first, (mut second, second_sources)) = halves.expect("at least one split is tried");
        transcriber.show_leading_octave = !segment.options.suppress_octave_marks;
        let second_braille = transcriber.transcribe_group(&mut second)?;
        for (element, source) in second.elements.iter().zip(second_sources) {
            grouping.elements[source].english = element.english.clone();
        }
        let key = self.current.expect("a grouping being written");
        segment.groupings.insert(
            SegmentKey {
                affinity: Affinity::SplitNoteGroupingA,
                ..key
            },
            first,
        );
        segment.groupings.insert(
            SegmentKey {
                affinity: Affinity::SplitNoteGroupingB,
                ..key
            },
            second,
        );
        Ok(Some((first_braille, second_braille)))
    }

    fn add_repeat_symbols(&mut self, times: usize) -> Result<()> {
        if (1..3).contains(&times) {
            for _ in 0..times {
                self.text.add_signatures(&symbol("repeat"))?;
            }
        } else if times >= 3 {
            let number = number_to_braille(&times.to_string(), true, false)?;
            self.text.add_signatures(&(symbol("repeat") + &number))?;
            self.last_note = None;
        }
        Ok(())
    }

    fn extract_signature_grouping(&mut self, segment: &mut BrailleSegment) -> Result<()> {
        let key = self.current.expect("a grouping being written");
        let grouping = segment.groupings.get_mut(&key).expect("a grouping");
        let (key_index, meter_index) = match grouping.elements.len() {
            0 => (None, None),
            1 => {
                if sharps_of(&grouping.elements[0].element).is_some() {
                    (Some(0), None)
                } else {
                    (None, Some(0))
                }
            }
            _ => (Some(0), Some(1)),
        };
        let sharps = key_index.and_then(|index| sharps_of(&grouping.elements[index].element));
        let outgoing = if segment.options.cancel_outgoing_key_sig {
            key_index.and_then(|index| grouping.elements[index].outgoing_key)
        } else {
            None
        };
        let meter = meter_index.and_then(|index| match &grouping.elements[index].element {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        });
        let written = transcribe_signatures(sharps, meter.as_ref(), outgoing);
        if let Some(index) = key_index
            && let Some(sharps) = sharps
        {
            grouping.elements[index].english =
                Some(basic::key_sig_to_braille(sharps, outgoing).english);
        }
        if let (Some(index), Some(meter)) = (meter_index, &meter) {
            grouping.elements[index].english = Some(basic::time_sig_to_braille(meter).english);
        }
        if !written.braille.is_empty() {
            self.text.add_signatures(&written.braille)?;
        }
        Ok(())
    }

    fn extract_tempo_text_grouping(&mut self, segment: &mut BrailleSegment) -> Result<()> {
        let current = self.current.expect("a grouping being written");
        self.keys.push_front(current);
        let previous = self.previous.ok_or_else(|| {
            Error::Notation("'NoneType' object has no attribute 'affinity'".to_string())
        })?;
        if previous.affinity == Affinity::Signature {
            self.keys.push_front(previous);
        }
        self.extract_heading(segment)?;
        self.extract_measure_number(segment)
    }
}

/// A piano's two staves cut into segments of groupings for each hand,
/// written side by side: music21's `BrailleGrandSegment`.
#[derive(Clone, Debug)]
pub struct BrailleGrandSegment {
    /// The groupings of both hands, by key.
    pub groupings: BTreeMap<SegmentKey, BrailleElementGrouping>,
    /// How many cells a line holds.
    pub line_length: usize,
    /// What music21 marked on each leaf of the right hand's part.
    pub right_contexts: Vec<NoteContext>,
    /// What music21 marked on each leaf of the left hand's part.
    pub left_contexts: Vec<NoteContext>,
}

impl BrailleGrandSegment {
    /// The keys of the two hands, paired measure by measure: music21's
    /// `yieldCombinedGroupingKeys`.
    pub fn combined_keys(&self) -> Vec<(Option<SegmentKey>, Option<SegmentKey>)> {
        let mut keys: Vec<SegmentKey> = self.groupings.keys().copied().collect();
        keys.sort_by_key(|key| {
            (
                key.measure,
                key.ordinal,
                key.affinity,
                if key.hand == Some(Hand::Right) { -1 } else { 1 },
            )
        });
        let matches = |one: &SegmentKey, other: &SegmentKey| {
            one.measure == other.measure
                && one.ordinal == other.ordinal
                && one.affinity == other.affinity
        };
        let mut out = Vec::new();
        let mut stored_right: Option<SegmentKey> = None;
        let mut stored_left: Option<SegmentKey> = None;
        for key in keys {
            match key.hand {
                Some(Hand::Right) => {
                    if let Some(left) = stored_left.take() {
                        if matches(&key, &left)
                            || (key.affinity == Affinity::NoteGrouping
                                && matches(
                                    &SegmentKey {
                                        affinity: Affinity::Inaccord,
                                        ..key
                                    },
                                    &left,
                                ))
                        {
                            out.push((Some(key), Some(left)));
                        } else {
                            out.push((None, Some(left)));
                            stored_right = Some(key);
                        }
                    } else {
                        stored_right = Some(key);
                    }
                }
                Some(Hand::Left) => {
                    if let Some(right) = stored_right.take() {
                        if matches(&key, &right) {
                            out.push((Some(right), Some(key)));
                        } else if right.affinity < Affinity::Inaccord {
                            out.push((Some(right), None));
                            out.push((None, Some(key)));
                        } else {
                            out.push((Some(right), None));
                            stored_left = Some(key);
                        }
                    } else {
                        stored_left = Some(key);
                    }
                }
                None => {}
            }
        }
        if let Some(right) = stored_right {
            out.push((Some(right), None));
        }
        if let Some(left) = stored_left {
            out.push((None, Some(left)));
        }
        out
    }

    /// The two hands in braille: a heading, then each measure's groupings
    /// side by side: music21's `transcribe`.
    ///
    /// # Errors
    ///
    /// Something too wide for a line, or that music21 cannot write.
    pub fn transcribe(&mut self) -> Result<String> {
        let mut pairs: VecDeque<(Option<SegmentKey>, Option<SegmentKey>)> =
            self.combined_keys().into();
        let mut keyboard = BrailleKeyboard::new(self.line_length);
        let Some(last) = pairs.back().copied() else {
            return Ok(keyboard.text.render());
        };
        let highest = last.0.or(last.1).map_or(0, |key| key.measure);
        keyboard.highest_measure_number_length = highest.to_string().chars().count();
        // The heading.
        let mut sharps: Option<Option<IntegerType>> = None;
        let mut meter: Option<TimeSignature> = None;
        let mut tempo: Option<crate::tempo::TempoText> = None;
        let mut mark: Option<crate::tempo::MetronomeMark> = None;
        let mut places: [Option<(SegmentKey, usize)>; 4] = [None; 4];
        while let Some(&(right, _)) = pairs.front() {
            // music21 looks the right hand's grouping up by a key that may be
            // nothing, which finds nothing rather than failing, and then
            // reads the missing key's affinity.
            let use_key = right.ok_or_else(|| {
                Error::Notation("'NoneType' object has no attribute 'affinity'".to_string())
            })?;
            if use_key.affinity > Affinity::MetronomeMark {
                break;
            }
            pairs.pop_front();
            let grouping = &self.groupings[&use_key];
            match use_key.affinity {
                Affinity::Signature => {
                    if grouping.elements.len() >= 2 {
                        sharps = sharps_of(&grouping.elements[0].element);
                        places[0] = sharps.map(|_| (use_key, 0));
                        if let StreamElement::TimeSignature(time) = &grouping.elements[1].element {
                            meter = Some(time.clone());
                            places[1] = Some((use_key, 1));
                        }
                    } else if let Some(only) = grouping.elements.first() {
                        // music21 asks whether the grouping, not its element,
                        // is a key signature, so a lone signature is taken
                        // for a time signature, which a key signature cannot
                        // be written as.
                        match &only.element {
                            StreamElement::TimeSignature(time) => {
                                meter = Some(time.clone());
                                places[1] = Some((use_key, 0));
                            }
                            _ => {
                                return Err(Error::Notation(
                                    "'KeySignature' object has no attribute 'symbol'".to_string(),
                                ));
                            }
                        }
                    }
                }
                Affinity::TempoText => {
                    if let Some(StreamElement::TempoText(text)) =
                        grouping.elements.first().map(|e| &e.element)
                    {
                        tempo = Some(text.clone());
                        places[2] = Some((use_key, 0));
                    }
                }
                Affinity::MetronomeMark => {
                    if let Some(StreamElement::MetronomeMark(found)) =
                        grouping.elements.first().map(|e| &e.element)
                    {
                        mark = Some(found.clone());
                        places[3] = Some((use_key, 0));
                    }
                }
                _ => {}
            }
        }
        if let Ok(heading) = transcribe_heading(
            sharps,
            meter.as_ref(),
            tempo.as_ref(),
            mark.as_ref(),
            self.line_length,
        ) {
            let english = heading_english(
                sharps,
                meter.as_ref(),
                tempo.as_ref(),
                mark.as_ref(),
                self.line_length,
            );
            for (place, english) in places.into_iter().zip(english) {
                if let (Some((key, index)), Some(english)) = (place, english) {
                    self.groupings.get_mut(&key).expect("a grouping").elements[index].english =
                        Some(english);
                }
            }
            keyboard.text.add_heading(&heading)?;
        }
        while let Some((right, left)) = pairs.pop_front() {
            let writes = right.is_some_and(|key| key.affinity >= Affinity::Inaccord)
                || left.is_some_and(|key| key.affinity >= Affinity::Inaccord);
            if !writes {
                continue;
            }
            let measure = right.or(left).map_or(0, |key| key.measure);
            let number = number_to_braille(&measure.to_string(), false, false)?;
            let right_braille = self.braille_from_key(right)?;
            let left_braille = self.braille_from_key(left)?;
            keyboard.add_note_groupings(&number, &right_braille, &left_braille)?;
        }
        Ok(keyboard.text.render())
    }

    fn braille_from_key(&mut self, key: Option<SegmentKey>) -> Result<String> {
        let Some(key) = key else {
            return Ok(String::new());
        };
        let contexts = if key.hand == Some(Hand::Left) {
            &self.left_contexts
        } else {
            &self.right_contexts
        };
        let grouping = self.groupings.get_mut(&key).expect("a grouping");
        if key.affinity == Affinity::Inaccord {
            let mut voices = Vec::new();
            let (descending, clefs, upper) = (
                grouping.descending_chords,
                grouping.show_clef_signs,
                grouping.upper_first_in_note_fingering,
            );
            for element in &mut grouping.elements {
                let StreamElement::Stream(voice) = &element.element else {
                    continue;
                };
                let mut inner = extract_braille_elements(voice, element.leaf, contexts);
                inner.descending_chords = descending;
                inner.show_clef_signs = clefs;
                inner.upper_first_in_note_fingering = upper;
                voices.push(transcribe_note_grouping(&mut inner, true)?);
                element.voice_english = voice_english_of(voice, element.leaf, &inner);
            }
            Ok(voices.join(&symbol("full_inaccord")))
        } else {
            transcribe_note_grouping(grouping, true)
        }
    }

    /// The two hands' groupings in English, as music21's `__str__` lists
    /// them.
    pub fn english(&self) -> String {
        let mut pairs = String::new();
        for (right, left) in self.combined_keys() {
            let right_full = right.map_or_else(String::new, |key| {
                format!(
                    "Measure {} Right, {} {}:\n{}",
                    key.measure,
                    key.affinity.name(),
                    key.ordinal + 1,
                    grouping_english(&self.groupings[&key])
                )
            });
            let left_full = left.map_or_else(String::new, |key| {
                format!(
                    "\nMeasure {} Left, {} {}:\n{}",
                    key.measure,
                    key.affinity.name(),
                    key.ordinal + 1,
                    grouping_english(&self.groupings[&key])
                )
            });
            pairs.push_str(&[right_full, left_full, "====\n".to_string()].join("\n"));
        }
        [
            "---begin grand segment---".to_string(),
            "<music21.braille.segment BrailleGrandSegment>\n===".to_string(),
            pairs,
            "---end grand segment---".to_string(),
        ]
        .join("\n")
    }
}
