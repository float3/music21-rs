//! MuseScore's `.mscx`, read into a score.
//!
//! A MuseScore file keeps a score's parts apart from what is written on
//! their staves: each `<Part>` names its instrument and how many staves it
//! has, and after the parts comes one `<Staff>` for every staff of the
//! score, holding its measures. A measure holds a `<voice>` for each of up
//! to four voices, and a voice is its notes and rests in order, with the
//! clefs, signatures, texts and marks standing between them. Nothing in the
//! file says where a note starts; that is worked out by adding up the
//! lengths of what comes before it, a `<location>` moving the place along
//! where a mark stands between two notes.

use std::collections::HashMap;

use num::rational::Ratio;
use num::{One, ToPrimitive, Zero};

use super::beams::{self, BeamMode, Member};
use crate::articulations::{Articulation, ArticulationKind, Finger};
use crate::bar::{Barline, BarlineType, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::ChordSymbol;
use crate::clef::Clef;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{
    Duration, DurationType, Grace, Tuplet, TupletBracket, TupletShow, TupletType,
};
use crate::dynamics::Dynamic;
use crate::error::{Error, Result};
use crate::expressions::{
    ArpeggioType, Expression, Fermata, FermataType, Ornament, OrnamentDelay, OrnamentKind,
    TextExpression,
};
use crate::instrument::{Instrument, SearchLanguage};
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::makenotation::op_frac;
use crate::metadata::{Metadata, MetadataValue};
use crate::meter::TimeSignature;
use crate::notation::{
    Beams, Lyric, NoteSize, Notehead, Placement, StemDirection, Syllabic, Tie, TieType,
};
use crate::note::Note;
use crate::pitch::{Accidental, Pitch};
use crate::repeat::{RepeatExpression, RepeatExpressionKind};
use crate::rest::Rest;
use crate::spanner::{Pedal, PedalForm, PedalType, Spanner, SpannerKind};
use crate::stream::{BarTogether, StaffGroup, Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::MetronomeMark;
use crate::volume::Volume;
use crate::xml::Xml;

/// A length or a place in time, in whole notes, as MuseScore counts.
type Frac = Ratio<i64>;

fn mscx_error(message: impl Into<String>) -> Error {
    Error::MuseScore(message.into())
}

fn refused(what: impl std::fmt::Display) -> Error {
    mscx_error(format!("{what} cannot be read yet"))
}

/// A fraction as MuseScore writes one: `3/4`, or a whole number.
fn fraction(text: &str) -> Result<Frac> {
    let text = text.trim();
    let bad = || mscx_error(format!("{text:?} is not a fraction"));
    match text.split_once('/') {
        Some((numerator, denominator)) => {
            let numerator: i64 = numerator.trim().parse().map_err(|_| bad())?;
            let denominator: i64 = denominator.trim().parse().map_err(|_| bad())?;
            if denominator == 0 {
                return Err(bad());
            }
            Ok(Frac::new(numerator, denominator))
        }
        None => Ok(Frac::from_integer(text.parse().map_err(|_| bad())?)),
    }
}

fn integer(element: &Xml) -> Result<i64> {
    let text = element.stripped();
    text.parse()
        .map_err(|_| mscx_error(format!("<{}> holds {text:?}, not a number", element.tag)))
}

fn child_integer(element: &Xml, tag: &str) -> Result<Option<i64>> {
    element.find(tag).map(integer).transpose()
}

fn flag(element: &Xml, tag: &str) -> bool {
    element
        .find(tag)
        .is_some_and(|found| found.stripped() != "0" && found.stripped() != "false")
}

/// A length in quarter notes, snapped as music21 snaps one.
fn quarters(value: Frac) -> FloatType {
    op_frac((value * 4).to_f64().unwrap_or(0.0))
}

/// A place in MuseScore's ticks, 480 to a quarter.
fn ticks(value: Frac) -> i64 {
    (value * beams::WHOLE).floor().to_integer()
}

/// Text as MuseScore writes it, its formatting taken off and each symbol
/// read as the character that draws it.
fn plain_text(element: &Xml) -> Result<String> {
    let mut out = String::new();
    plain_into(element, &mut out)?;
    Ok(out)
}

fn plain_into(element: &Xml, out: &mut String) -> Result<()> {
    if element.tag == "sym" {
        out.push(symbol_named(element.text().unwrap_or("").trim())?);
        return Ok(());
    }
    // An older file wrote a name as a small HTML page.
    if matches!(element.tag.as_str(), "head" | "style" | "meta" | "title") {
        return Ok(());
    }
    if element.tag == "br" {
        out.push('\n');
        return Ok(());
    }
    if let Some(text) = element.text() {
        out.push_str(text);
    }
    for child in &element.children {
        plain_into(child, out)?;
        if let Some(tail) = child.tail() {
            out.push_str(tail);
        }
    }
    Ok(())
}

/// The character a symbol inside a text is drawn with: its place in the
/// Standard Music Font Layout, which is what MuseScore names its symbols by.
fn symbol_named(name: &str) -> Result<char> {
    Ok(match name {
        "segno" => '\u{E047}',
        "coda" => '\u{E048}',
        "codaSquare" => '\u{E049}',
        "segnoSerpent1" => '\u{E04A}',
        "segnoSerpent2" => '\u{E04B}',
        "accidentalFlat" => '\u{E260}',
        "accidentalNatural" => '\u{E261}',
        "accidentalSharp" => '\u{E262}',
        "accidentalDoubleSharp" => '\u{E263}',
        "accidentalDoubleFlat" => '\u{E264}',
        "dynamicPiano" => '\u{E520}',
        "dynamicMezzo" => '\u{E521}',
        "dynamicForte" => '\u{E522}',
        "dynamicRinforzando" => '\u{E523}',
        "dynamicSforzando" => '\u{E524}',
        "dynamicZ" => '\u{E525}',
        "dynamicNiente" => '\u{E526}',
        "keyboardPedalPed" => '\u{E650}',
        "keyboardPedalUp" => '\u{E655}',
        "metNoteDoubleWhole" => '\u{ECA0}',
        "metNoteWhole" => '\u{ECA2}',
        "metNoteHalfUp" => '\u{ECA3}',
        "metNoteHalfDown" => '\u{ECA4}',
        "metNoteQuarterUp" => '\u{ECA5}',
        "metNoteQuarterDown" => '\u{ECA6}',
        "metNote8thUp" => '\u{ECA7}',
        "metNote8thDown" => '\u{ECA8}',
        "metNote16thUp" => '\u{ECA9}',
        "metNote16thDown" => '\u{ECAA}',
        "metAugmentationDot" => '\u{ECB7}',
        "space" => ' ',
        other => return Err(refused(format!("The symbol {other:?} in a text"))),
    })
}

// ------------------------------------------------------------ vocabulary

/// A written value by MuseScore's name for it, with its length in whole
/// notes and its place in MuseScore's order of values.
fn duration_named(name: &str) -> Result<(DurationType, Frac, u8)> {
    Ok(match name {
        "long" => (DurationType::Longa, Frac::from_integer(4), 0),
        "breve" => (DurationType::Breve, Frac::from_integer(2), 1),
        "whole" => (DurationType::Whole, Frac::one(), 2),
        "half" => (DurationType::Half, Frac::new(1, 2), 3),
        "quarter" => (DurationType::Quarter, Frac::new(1, 4), 4),
        "eighth" => (DurationType::Eighth, Frac::new(1, 8), 5),
        "16th" => (DurationType::Sixteenth, Frac::new(1, 16), 6),
        "32nd" => (DurationType::ThirtySecond, Frac::new(1, 32), 7),
        "64th" => (DurationType::SixtyFourth, Frac::new(1, 64), 8),
        "128th" => (DurationType::HundredTwentyEighth, Frac::new(1, 128), 9),
        "256th" => (DurationType::TwoHundredFiftySixth, Frac::new(1, 256), 10),
        "512th" => (DurationType::FiveHundredTwelfth, Frac::new(1, 512), 11),
        "1024th" => (DurationType::TenTwentyFourth, Frac::new(1, 1024), 12),
        other => return Err(mscx_error(format!("a note of the value {other:?}"))),
    })
}

fn dotted(length: Frac, dots: u32) -> Frac {
    let mut total = length;
    let mut added = length;
    for _ in 0..dots {
        added /= 2;
        total += added;
    }
    total
}

/// A clef by MuseScore's name: the sign and line MusicXML writes it with,
/// and how many octaves it sounds away.
fn clef_named(name: &str) -> Result<Clef> {
    let (sign, line, octaves) = match name {
        "G" | "G8vbp" | "C_19C" => ("G", 2, 0),
        "G15mb" => ("G", 2, -2),
        "G8vb" | "G8vbo" | "G8vbc" => ("G", 2, -1),
        "G8va" => ("G", 2, 1),
        "G15ma" => ("G", 2, 2),
        "G1" => ("G", 1, 0),
        "C1" | "C1_F18C" | "C1_F20C" => ("C", 1, 0),
        "C2" => ("C", 2, 0),
        "C3" | "C3_F18C" | "C3_F20C" => ("C", 3, 0),
        "C4" | "C4_F18C" | "C4_F20C" => ("C", 4, 0),
        "C4_8VB" => ("C", 4, -1),
        "C5" => ("C", 5, 0),
        "F" | "F_F18C" | "F_19C" => ("F", 4, 0),
        "F15mb" => ("F", 4, -2),
        "F8vb" => ("F", 4, -1),
        "F8va" => ("F", 4, 1),
        "F15ma" => ("F", 4, 2),
        "F3" => ("F", 3, 0),
        "F5" => ("F", 5, 0),
        "PERC" | "PERC2" => return Clef::from_string("percussion", 0),
        "TAB" | "TAB4" | "TAB2" | "TAB4_SERIF" => return Err(refused("A tablature clef")),
        other => return Err(mscx_error(format!("a clef of the kind {other:?}"))),
    };
    Clef::from_string(&format!("{sign}{line}"), octaves)
}

/// The accidental a MuseScore accidental draws, by music21's name.
fn accidental_named(subtype: &str) -> Result<&'static str> {
    Ok(match subtype {
        "accidentalSharp" => "sharp",
        "accidentalFlat" => "flat",
        "accidentalNatural" => "natural",
        "accidentalDoubleSharp" | "accidentalSharpSharp" => "double-sharp",
        "accidentalDoubleFlat" => "double-flat",
        "accidentalTripleSharp" => "triple-sharp",
        "accidentalTripleFlat" => "triple-flat",
        "accidentalNaturalSharp" => "natural-sharp",
        "accidentalNaturalFlat" => "natural-flat",
        other => return Err(refused(format!("The accidental {other:?}"))),
    })
}

/// The spelling a tonal pitch class stands for: MuseScore's `tpc`, a place on
/// the line of fifths where C is fourteen.
fn spelling(tpc: i64) -> Result<(char, i64)> {
    if !(-1..=33).contains(&tpc) {
        return Err(mscx_error(format!("the tonal pitch class {tpc}")));
    }
    let step = b"FCGDAEB"[((tpc + 1) % 7) as usize] as char;
    let alter = (tpc + 1) / 7 - 2;
    Ok((step, alter))
}

fn natural_semitone(step: char) -> i64 {
    match step {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    }
}

/// The letter and octave of a pitch, from its MIDI number and its spelling.
fn pitch_of(midi: i64, tpc: i64) -> Result<Pitch> {
    let (step, alter) = spelling(tpc)?;
    let natural = midi - alter;
    let octave = (natural - natural_semitone(step)).div_euclid(12) - 1;
    Pitch::from_name(format!("{step}{octave}"))
}

// ------------------------------------------------------------ what is read

/// A tuplet of one voice of one measure.
#[derive(Clone, Debug)]
struct TupletRead {
    actual: u32,
    normal: u32,
    base: (DurationType, u32),
    /// MuseScore's `bracketType`: 0 as the beams decide, 1 a bracket, 2 none.
    bracket: i64,
    /// MuseScore's `numberType`: 0 the number, 1 the ratio, 2 nothing.
    number: i64,
    placement: Option<Placement>,
    parent: Option<usize>,
    /// What it holds in order: notes and rests by their index among the
    /// voice's notes, tuplets by their own.
    elements: Vec<Held>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    Note(usize),
    Tuplet(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraceKind {
    Acciaccatura,
    Appoggiatura,
    Before,
    After,
}

#[derive(Clone, Debug)]
struct NoteRead {
    pitch: Pitch,
    tie_start: bool,
    tie_stop: bool,
    /// Which way the tie it starts curves, where the file says.
    tie_placement: Option<Placement>,
    /// How hard it is struck, of 127, where the file says.
    velocity: Option<i64>,
    hidden: bool,
    small: bool,
    /// The shape of its head, where it is not the usual one.
    head: Option<Notehead>,
    /// Whether its head is filled in, where the file fixes that.
    head_filled: Option<bool>,
    /// The fingerings written on it.
    fingerings: Vec<Articulation>,
    /// The colour it is drawn in, where that is not the usual one.
    color: Option<String>,
}

#[derive(Clone, Debug)]
struct LyricRead {
    number: IntegerType,
    syllabic: Option<Syllabic>,
    text: String,
}

/// A note, chord or rest as it was read.
#[derive(Clone, Debug)]
struct ChordRest {
    tick: Frac,
    /// How long it lasts, in whole notes.
    length: Frac,
    value: Option<(DurationType, u32)>,
    order: u8,
    measure_rest: bool,
    is_rest: bool,
    tuplet: Option<usize>,
    mode: BeamMode,
    grace: Option<GraceKind>,
    /// Its place among the grace notes of its note, as the file counts.
    grace_index: usize,
    graces_before: Vec<ChordRest>,
    graces_after: Vec<ChordRest>,
    notes: Vec<NoteRead>,
    hidden: bool,
    small: bool,
    stem: StemDirection,
    /// Whether it is drawn with no stem.
    no_stem: bool,
    /// The way the stems of the beam starting on it go, where a beam is
    /// written out before it.
    beam_stem: StemDirection,
    lyrics: Vec<LyricRead>,
    articulations: Vec<Articulation>,
    /// Ornaments and an arpeggio, in the order written.
    expressions: Vec<Expression>,
    fermatas: Vec<Fermata>,
    step_shift: IntegerType,
    staff_move: i64,
    /// What spanners know it by.
    uid: usize,
}

/// A spanner as it was started: what it is, where it starts and where the
/// file says it ends.
#[derive(Clone, Debug)]
struct SpannerRead {
    kind: SpannerStart,
    start: Position,
    end: Position,
}

#[derive(Clone, Debug)]
enum SpannerStart {
    Slur(Option<Placement>, Option<&'static str>),
    Wedge(SpannerKind),
    Pedal(Pedal),
    Volta { numbers: Vec<u32> },
}

/// A place in the score: a measure, a moment in it, a track.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position {
    measure: i64,
    tick: Frac,
    track: usize,
    /// Which of the grace notes leaning on the note there, where it is one
    /// of them and not the note itself.
    grace: Option<usize>,
}

#[derive(Clone, Debug)]
enum What {
    ChordRest(Box<ChordRest>),
    Clef(Clef),
    Key(StreamElement),
    Meter(TimeSignature),
    Element(StreamElement),
    /// A barline, and whether a fermata stands on it.
    Barline(Barline, bool),
}

#[derive(Clone, Debug)]
struct Event {
    tick: Frac,
    what: What,
}

/// One voice of one measure of one staff.
#[derive(Clone, Debug, Default)]
struct VoiceRead {
    events: Vec<Event>,
    tuplets: Vec<TupletRead>,
}

/// One measure of one staff.
#[derive(Clone, Debug, Default)]
struct StaffMeasure {
    voices: Vec<VoiceRead>,
    len: Option<Frac>,
    irregular: bool,
    number_offset: i64,
    start_repeat: bool,
    end_repeat: Option<u32>,
    /// Whether a section ends with it, the numbering starting again after.
    section_break: bool,
    /// Whether a section ended just before it.
    starts_section: bool,
    /// The signs and words saying where to go next: those standing at its
    /// start, and those at its end.
    marks_at_start: Vec<StreamElement>,
    marks_at_end: Vec<StreamElement>,
}

/// A part, as the head of the file describes it.
#[derive(Clone, Debug)]
struct PartRead {
    staves: usize,
    /// The global index of its first staff.
    first_staff: usize,
    name: Option<String>,
    abbreviation: Option<String>,
    name_hidden: bool,
    abbreviation_hidden: bool,
    instrument: Instrument,
    default_clefs: Vec<String>,
    /// The brackets starting on its first staff, each its kind and how many
    /// staves it joins, the innermost first.
    brackets: Vec<(i64, usize)>,
    /// Whether any of its staves starts a bracket.
    bracket_found: bool,
    /// How far the written notes stand from the sounding ones: diatonic
    /// steps and semitones.
    transposition: (i64, i64),
}

// ----------------------------------------------------------------- reading

/// What reading a staff needs of the measures across the score.
#[derive(Clone, Debug)]
struct Grid {
    /// Where each measure starts, in whole notes from the top.
    starts: Vec<Frac>,
    lengths: Vec<Frac>,
    meters: Vec<(i64, i64)>,
}

struct Reader {
    /// Whether the file is in a format older than 4.1.
    old_format: bool,
    /// Whether a marker is of the kind its label names. From format 4.6 on
    /// it says its kind apart, and one that says none marks the end.
    markers_by_label: bool,
    next_uid: usize,
    spanners: Vec<SpannerRead>,
}

impl Reader {
    fn uid(&mut self) -> usize {
        self.next_uid += 1;
        self.next_uid
    }

    /// A staff's measures, each with its voices read.
    fn staff(
        &mut self,
        staff: &Xml,
        staff_index: usize,
        transposition: (i64, i64),
    ) -> Result<Vec<StaffMeasure>> {
        let mut measures: Vec<StaffMeasure> = Vec::new();
        let mut section_ended = false;
        for element in &staff.children {
            match element.tag.as_str() {
                // The measure MuseScore draws for a run of empty ones stands
                // in the file beside the measures it covers, which are the
                // music.
                "Measure" if element.find("multiMeasureRest").is_some() => {}
                "Measure" => {
                    let index = measures.len() as i64;
                    let mut measure = self.measure(element, staff_index, index, transposition)?;
                    measure.starts_section = std::mem::take(&mut section_ended);
                    section_ended = measure.section_break;
                    measures.push(measure);
                }
                // Frames hold titles and text for the page; a section may end
                // on one.
                "VBox" | "HBox" | "TBox" | "FBox" => {
                    if element.find_all("LayoutBreak").any(ends_section) {
                        section_ended = true;
                    }
                }
                "eid" => {}
                other => return Err(mscx_error(format!("<{other}> in a staff"))),
            }
        }
        Ok(measures)
    }

    fn measure(
        &mut self,
        element: &Xml,
        staff_index: usize,
        index: i64,
        transposition: (i64, i64),
    ) -> Result<StaffMeasure> {
        let mut measure = StaffMeasure {
            len: element.get("len").map(fraction).transpose()?,
            ..StaffMeasure::default()
        };
        let mut voice = 0usize;
        for held in &element.children {
            match held.tag.as_str() {
                "voice" => {
                    let track = staff_index * 4 + voice;
                    let read = self.voice(held, index, track, transposition)?;
                    measure.voices.push(read);
                    voice += 1;
                }
                "irregular" => measure.irregular = held.stripped() != "0",
                "noOffset" => measure.number_offset = integer(held)?,
                "startRepeat" => measure.start_repeat = true,
                "endRepeat" => measure.end_repeat = Some(integer(held)?.max(0) as u32),
                "LayoutBreak" => {
                    if ends_section(held) {
                        measure.section_break = true;
                    }
                }
                "measureRepeatCount" => {
                    return Err(refused(format!("<{}>", held.tag)));
                }
                "Marker" if held.child_text("visible") != "0" => {
                    match marker_of(held, self.markers_by_label)? {
                        (false, mark) => measure.marks_at_start.push(mark),
                        (true, mark) => measure.marks_at_end.push(mark),
                    }
                }
                "Jump" if held.child_text("visible") != "0" => {
                    measure.marks_at_end.push(jump_of(held)?);
                }
                "Marker" | "Jump" => {}
                // What places the measure on the page.
                "eid"
                | "linkedMain"
                | "stretch"
                | "measureNumberMode"
                | "breakMultiMeasureRest"
                | "vspacer"
                | "vspacerDown"
                | "vspacerUp"
                | "vspacerFixed"
                | "visible"
                | "stemless"
                | "hideIfEmpty"
                | "MeasureNumber"
                | "MMRestRange"
                | "SystemDivider"
                | "Spacer"
                | "StaffTypeChange" => {}
                other => return Err(mscx_error(format!("<{other}> in a measure"))),
            }
        }
        Ok(measure)
    }

    fn voice(
        &mut self,
        element: &Xml,
        measure: i64,
        track: usize,
        transposition: (i64, i64),
    ) -> Result<VoiceRead> {
        let mut read = VoiceRead::default();
        let mut tick = Frac::zero();
        let mut tuplet: Option<usize> = None;
        let mut graces: Vec<ChordRest> = Vec::new();
        let mut fermatas: Vec<Fermata> = Vec::new();
        let mut beam_stem = StemDirection::Unspecified;
        let mut notes = 0usize;
        for held in &element.children {
            match held.tag.as_str() {
                "location" => {
                    if let Some(fractions) = held.find("fractions") {
                        tick += fraction(fractions.stripped())?;
                    }
                    if held.find("measures").is_some() {
                        return Err(mscx_error("a <location> in a voice that moves measures"));
                    }
                }
                "Chord" | "Rest" => {
                    let mut chord_rest =
                        self.chord_rest(held, tick, tuplet, &read.tuplets, transposition)?;
                    chord_rest.beam_stem =
                        std::mem::replace(&mut beam_stem, StemDirection::Unspecified);
                    let grace = chord_rest.grace.map(|_| graces.len());
                    self.spanners_of(
                        held,
                        Position {
                            measure,
                            tick,
                            track,
                            grace,
                        },
                    )?;
                    if let Some(index) = grace {
                        chord_rest.grace_index = index;
                        graces.push(*chord_rest);
                        continue;
                    }
                    chord_rest.fermatas = std::mem::take(&mut fermatas);
                    // Graces before the note come first, those after it
                    // follow it.
                    let (after, before): (Vec<ChordRest>, Vec<ChordRest>) =
                        std::mem::take(&mut graces)
                            .into_iter()
                            .partition(|grace| grace.grace == Some(GraceKind::After));
                    chord_rest.graces_before = before;
                    // MuseScore keeps the graces after a note last first.
                    chord_rest.graces_after = after.into_iter().rev().collect();
                    if let Some(open) = tuplet {
                        read.tuplets[open].elements.push(Held::Note(notes));
                    }
                    notes += 1;
                    let length = chord_rest.length;
                    read.events.push(Event {
                        tick,
                        what: What::ChordRest(chord_rest),
                    });
                    tick += length;
                }
                "Tuplet" => {
                    let made = tuplet_of(held, tuplet)?;
                    let index = read.tuplets.len();
                    if let Some(open) = tuplet {
                        read.tuplets[open].elements.push(Held::Tuplet(index));
                    }
                    read.tuplets.push(made);
                    tuplet = Some(index);
                }
                "endTuplet" => {
                    let open =
                        tuplet.ok_or_else(|| mscx_error("an <endTuplet/> with no tuplet"))?;
                    tuplet = read.tuplets[open].parent;
                }
                "Clef" => {
                    let name = match held.find("concertClefType") {
                        Some(clef) => clef.stripped(),
                        None => held.child_text("subtype"),
                    };
                    let name = match held.find("transposingClefType") {
                        Some(written) => written.stripped(),
                        None => name,
                    };
                    read.events.push(Event {
                        tick,
                        what: What::Clef(clef_named(name)?),
                    });
                }
                "KeySig" => {
                    if !flag(held, "isCourtesy") {
                        read.events.push(Event {
                            tick,
                            what: What::Key(key_of(held, transposition, self.old_format)?),
                        });
                    }
                }
                "TimeSig" => {
                    if !flag(held, "isCourtesy") {
                        read.events.push(Event {
                            tick,
                            what: What::Meter(meter_of(held)?),
                        });
                    }
                }
                "BarLine" => {
                    // A fermata written before a barline stands on it.
                    let fermata = !std::mem::take(&mut fermatas).is_empty();
                    if let Some(barline) = barline_of(held)? {
                        read.events.push(Event {
                            tick,
                            what: What::Barline(barline, fermata),
                        });
                    }
                }
                "Fermata" => fermatas.push(fermata_of(held)),
                // A breath is taken after the note before it.
                "Breath" => {
                    let mark = breath_of(held)?;
                    let before =
                        read.events
                            .iter_mut()
                            .rev()
                            .find_map(|event| match &mut event.what {
                                What::ChordRest(before) => Some(before),
                                _ => None,
                            });
                    match before {
                        Some(before) => before.articulations.push(mark),
                        None => return Err(mscx_error("a breath mark with no note before it")),
                    }
                }
                "Sticking" | "PlayTechAnnotation" if held.child_text("visible") == "0" => {}
                "Sticking" | "PlayTechAnnotation" => {
                    let default = if held.tag == "Sticking" {
                        Placement::Below
                    } else {
                        Placement::Above
                    };
                    if let Some(element) = words_of(held, default)? {
                        read.events.push(Event {
                            tick,
                            what: What::Element(element),
                        });
                    }
                }
                // A mark that is hidden has nothing here to say so with, and
                // is left out, as MuseScore leaves it out of what it
                // exports.
                "Dynamic" | "StaffText" | "SystemText" | "Expression" | "Text"
                    if held.child_text("visible") == "0" => {}
                "Dynamic" => {
                    let mut dynamic = Dynamic::new(held.child_text("subtype"));
                    dynamic.set_placement(Some(placement_of(held, Placement::Below)));
                    read.events.push(Event {
                        tick,
                        what: What::Element(dynamic.into()),
                    });
                }
                "Tempo" => {
                    for element in tempo_of(held)? {
                        read.events.push(Event {
                            tick,
                            what: What::Element(element),
                        });
                    }
                }
                "StaffText" | "SystemText" | "Expression" | "Text" => {
                    let default = if held.tag == "Expression" {
                        Placement::Below
                    } else {
                        Placement::Above
                    };
                    if let Some(element) = words_of(held, default)? {
                        read.events.push(Event {
                            tick,
                            what: What::Element(element),
                        });
                    }
                }
                "Harmony" => {
                    read.events.push(Event {
                        tick,
                        what: What::Element(harmony_of(held)?.into()),
                    });
                }
                "Spanner" => self.spanner(
                    held,
                    Position {
                        measure,
                        tick,
                        track,
                        grace: None,
                    },
                )?,
                // A beam written out says which way its stems go; which
                // notes it joins is still the beam modes' to say.
                "Beam" => {
                    beam_stem = match held.child_text("StemDirection") {
                        "up" => StemDirection::Up,
                        "down" => StemDirection::Down,
                        _ => StemDirection::Unspecified,
                    };
                }
                // What is drawn and not played.
                "Symbol" | "Image" | "RehearsalMark" | "StaffState" | "Ambitus" | "eid"
                | "LayoutBreak" | "Segment" | "PlayCountText" => {}
                other => return Err(refused(format!("A <{other}>"))),
            }
        }
        if !graces.is_empty() {
            return Err(mscx_error("grace notes with no note to lean on"));
        }
        Ok(read)
    }

    /// A `<Chord>` or `<Rest>`.
    fn chord_rest(
        &mut self,
        element: &Xml,
        tick: Frac,
        tuplet: Option<usize>,
        tuplets: &[TupletRead],
        transposition: (i64, i64),
    ) -> Result<Box<ChordRest>> {
        let is_rest = element.tag == "Rest";
        let written = element.child_text("durationType");
        let dots = child_integer(element, "dots")?.unwrap_or(0).max(0) as u32;
        let (value, base, order, measure_rest) = if written == "measure" {
            let length = element
                .find("duration")
                .map(|duration| fraction(duration.stripped()))
                .transpose()?
                .ok_or_else(|| mscx_error("a measure rest that does not say how long it is"))?;
            (None, length, 0, true)
        } else {
            let (kind, length, order) = duration_named(written)?;
            (Some((kind, dots)), dotted(length, dots), order, false)
        };
        // A tuplet shortens what it holds by its ratio, and one inside
        // another by both.
        let mut length = base;
        let mut open = tuplet;
        while let Some(index) = open {
            let held = &tuplets[index];
            length = length * Frac::from_integer(i64::from(held.normal))
                / Frac::from_integer(i64::from(held.actual));
            open = held.parent;
        }
        let grace = [
            ("acciaccatura", GraceKind::Acciaccatura),
            ("appoggiatura", GraceKind::Appoggiatura),
            ("grace4", GraceKind::Before),
            ("grace16", GraceKind::Before),
            ("grace32", GraceKind::Before),
            ("grace8after", GraceKind::After),
            ("grace16after", GraceKind::After),
            ("grace32after", GraceKind::After),
        ]
        .into_iter()
        .find(|(tag, _)| element.find(tag).is_some())
        .map(|(_, kind)| kind);
        let mut read = ChordRest {
            tick,
            length: if grace.is_some() {
                Frac::zero()
            } else {
                length
            },
            value,
            order,
            measure_rest,
            is_rest,
            tuplet: if grace.is_some() { None } else { tuplet },
            mode: BeamMode::Auto,
            grace,
            grace_index: 0,
            graces_before: Vec::new(),
            graces_after: Vec::new(),
            notes: Vec::new(),
            hidden: element.child_text("visible") == "0",
            small: flag(element, "small"),
            stem: StemDirection::Unspecified,
            no_stem: false,
            beam_stem: StemDirection::Unspecified,
            lyrics: Vec::new(),
            articulations: Vec::new(),
            expressions: Vec::new(),
            fermatas: Vec::new(),
            step_shift: 0,
            staff_move: child_integer(element, "staffMove")?.unwrap_or(0),
            uid: self.uid(),
        };
        for held in &element.children {
            match held.tag.as_str() {
                "BeamMode" => {
                    read.mode = BeamMode::from_name(held.stripped()).ok_or_else(|| {
                        mscx_error(format!("the beam mode {:?}", held.stripped()))
                    })?;
                }
                "StemDirection" => {
                    read.stem = match held.stripped() {
                        "up" => StemDirection::Up,
                        "down" => StemDirection::Down,
                        _ => StemDirection::Unspecified,
                    };
                }
                "Stem" if held.child_text("visible") == "0" => read.no_stem = true,
                "noStem" if held.stripped() != "0" => read.no_stem = true,
                "Note" => read.notes.push(note_of(held, transposition)?),
                "Lyrics" => read.lyrics.push(lyric_of(held)?),
                "Articulation" => {
                    if let Some(articulation) = articulation_of(held)? {
                        read.articulations.push(articulation);
                    }
                }
                "offset" if is_rest => {
                    let y: FloatType = held.get("y").and_then(|y| y.parse().ok()).unwrap_or(0.0);
                    let steps = -2.0 * y;
                    read.step_shift = if steps > 0.0 {
                        (steps + 0.5) as IntegerType
                    } else {
                        (steps - 0.5) as IntegerType
                    };
                }
                "Ornament" => read.expressions.push(ornament_of(held)?),
                "Arpeggio" => read.expressions.push(arpeggio_of(held)?),
                "Tremolo" | "TremoloSingleChord" | "TremoloTwoChord" | "ChordLine" => {
                    return Err(refused(format!("A <{}>", held.tag)));
                }
                // Read elsewhere, or only drawn.
                "durationType" | "dots" | "duration" | "visible" | "small" | "staffMove"
                | "acciaccatura" | "appoggiatura" | "grace4" | "grace16" | "grace32"
                | "grace8after" | "grace16after" | "grace32after" | "Spanner" | "eid" | "Stem"
                | "Hook" | "NoteDot" | "Beam" | "offset" | "linkedMain" | "stemDirection"
                | "noStem" | "showStemSlash" | "combineVoice" | "ChordRest" | "StemSlash" => {}
                other => return Err(refused(format!("A <{other}> in a chord"))),
            }
        }
        if !is_rest && read.notes.is_empty() {
            return Err(mscx_error("a chord with no notes"));
        }
        Ok(Box::new(read))
    }

    /// The spanners a note or rest starts: a slur written inside it.
    fn spanners_of(&mut self, element: &Xml, here: Position) -> Result<()> {
        for held in element.find_all("Spanner") {
            self.spanner(held, here)?;
        }
        Ok(())
    }

    /// A `<Spanner>`: where one starts, with where it ends; the end itself
    /// says nothing that the start did not.
    fn spanner(&mut self, element: &Xml, here: Position) -> Result<()> {
        let kind = element.get("type").unwrap_or("");
        let Some(next) = element.find("next") else {
            return Ok(());
        };
        let body = element.find(kind);
        if matches!(kind, "HairPin" | "Pedal")
            && body.is_some_and(|body| body.child_text("visible") == "0")
        {
            return Ok(());
        }
        let start = SpannerStart::from(kind, body)?;
        let Some(start) = start else {
            return Ok(());
        };
        let end = moved(here, next.find("location"))?;
        self.spanners.push(SpannerRead {
            kind: start,
            start: here,
            end,
        });
        Ok(())
    }
}

impl SpannerStart {
    fn from(kind: &str, body: Option<&Xml>) -> Result<Option<Self>> {
        Ok(Some(match kind {
            "Slur" => Self::Slur(
                match body.map_or("", |body| body.child_text("up")) {
                    "up" => Some(Placement::Above),
                    "down" => Some(Placement::Below),
                    _ => None,
                },
                match body.map_or("", |body| body.child_text("lineType")) {
                    "1" => Some("dotted"),
                    "2" | "3" => Some("dashed"),
                    _ => None,
                },
            ),
            "HairPin" => {
                let subtype = body.map_or("0", |body| body.child_text("subtype"));
                match subtype {
                    "0" | "" => Self::Wedge(SpannerKind::Crescendo),
                    "1" => Self::Wedge(SpannerKind::Diminuendo),
                    other => {
                        return Err(refused(format!("A hairpin of the kind {other}")));
                    }
                }
            }
            "Volta" => {
                let body = body.ok_or_else(|| mscx_error("a volta that says nothing"))?;
                let numbers = body
                    .child_text("endings")
                    .split(',')
                    .map(|number| number.trim().parse::<u32>())
                    .collect::<std::result::Result<Vec<u32>, _>>()
                    .map_err(|_| {
                        mscx_error(format!(
                            "the volta endings {:?}",
                            body.child_text("endings")
                        ))
                    })?;
                Self::Volta { numbers }
            }
            "Pedal" => {
                let body = body.ok_or_else(|| mscx_error("a pedal line that says nothing"))?;
                if body.find("beginHookType").is_some() {
                    return Err(refused("A pedal line that starts with a hook"));
                }
                let text = body.find("beginText").map(Xml::all_text);
                if text.as_deref() == Some("") {
                    return Err(refused("A pedal line carried on from another"));
                }
                let sostenuto = body
                    .find("beginText")
                    .and_then(|text| text.find("sym"))
                    .is_some_and(|sym| {
                        matches!(sym.stripped(), "keyboardPedalSost" | "keyboardPedalS")
                    });
                Self::Pedal(Pedal {
                    pedal_type: Some(if sostenuto {
                        PedalType::Sostenuto
                    } else {
                        PedalType::Sustain
                    }),
                    form: Some(if body.child_text("lineVisible") == "0" {
                        PedalForm::Symbol
                    } else {
                        PedalForm::Line
                    }),
                    abbreviated: false,
                })
            }
            // A tie is read on its note.
            "Tie" | "LaissezVib" | "PartialTie" => return Ok(None),
            other => return Err(refused(format!("A {other} spanner"))),
        }))
    }
}

/// A position moved by a relative `<location>`.
fn moved(from: Position, location: Option<&Xml>) -> Result<Position> {
    // A place names a grace note outright, and the note itself by naming
    // none.
    let mut to = Position {
        grace: None,
        ..from
    };
    let Some(location) = location else {
        return Ok(to);
    };
    to.grace = child_integer(location, "grace")?.and_then(|index| usize::try_from(index).ok());
    if let Some(measures) = child_integer(location, "measures")? {
        to.measure += measures;
    }
    if let Some(fractions) = location.find("fractions") {
        to.tick += fraction(fractions.stripped())?;
    }
    let staves = child_integer(location, "staves")?.unwrap_or(0);
    let voices = child_integer(location, "voices")?.unwrap_or(0);
    let track = from.track as i64 + staves * 4 + voices;
    to.track = usize::try_from(track).map_err(|_| mscx_error("a spanner ending on no staff"))?;
    Ok(to)
}

fn tuplet_of(element: &Xml, parent: Option<usize>) -> Result<TupletRead> {
    let count = |tag: &str| -> Result<u32> {
        let value = child_integer(element, tag)?
            .ok_or_else(|| mscx_error(format!("a tuplet with no <{tag}>")))?;
        u32::try_from(value)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| mscx_error(format!("a tuplet of {value}")))
    };
    let base_name = element.child_text("baseNote");
    let (kind, _, _) = duration_named(base_name)?;
    let base_dots = child_integer(element, "baseDots")?.unwrap_or(0).max(0) as u32;
    let placement = match element.child_text("direction") {
        "up" => Some(Placement::Above),
        "down" => Some(Placement::Below),
        _ => None,
    };
    Ok(TupletRead {
        actual: count("actualNotes")?,
        normal: count("normalNotes")?,
        base: (kind, base_dots),
        bracket: child_integer(element, "bracketType")?.unwrap_or(0),
        number: child_integer(element, "numberType")?.unwrap_or(0),
        placement,
        parent,
        elements: Vec::new(),
    })
}

fn note_of(element: &Xml, transposition: (i64, i64)) -> Result<NoteRead> {
    let midi =
        child_integer(element, "pitch")?.ok_or_else(|| mscx_error("a note with no pitch"))?;
    let concert = child_integer(element, "tpc")?.ok_or_else(|| mscx_error("a note with no tpc"))?;
    // A transposing instrument's notes are written as it reads them.
    let written_tpc = child_integer(element, "tpc2")?.unwrap_or(concert);
    let (tpc, midi) = if transposition == (0, 0) {
        (concert, midi)
    } else {
        (written_tpc, midi - transposition.1)
    };
    let mut pitch = pitch_of(midi, tpc)?;
    let (_, alter) = spelling(tpc)?;
    match element.find("Accidental") {
        Some(written) => {
            let name = accidental_named(written.child_text("subtype"))?;
            let mut accidental = Accidental::natural();
            accidental.set_allowing_non_standard_value(name)?;
            accidental.set_display_status(Some(true));
            match written.child_text("bracket") {
                "1" => accidental.set_display_style("parentheses")?,
                "2" => accidental.set_display_style("bracket")?,
                _ => {}
            }
            pitch.set_written_accidental(Some(accidental));
        }
        None if alter != 0 => {
            let mut accidental = Accidental::new(alter as FloatType)?;
            accidental.set_display_status(Some(false));
            pitch.set_written_accidental(Some(accidental));
        }
        None => {}
    }
    let mut read = NoteRead {
        pitch,
        tie_start: false,
        tie_stop: false,
        tie_placement: None,
        velocity: child_integer(element, "velocity")?.filter(|velocity| *velocity != 0),
        hidden: element.child_text("visible") == "0",
        small: flag(element, "small"),
        head: None,
        head_filled: None,
        fingerings: Vec::new(),
        color: None,
    };
    for held in &element.children {
        match held.tag.as_str() {
            "head" => read.head = head_named(held.stripped())?,
            "headType" => {
                read.head_filled = match held.stripped() {
                    "quarter" => Some(true),
                    "half" | "whole" => Some(false),
                    "auto" | "breve" => None,
                    other => return Err(refused(format!("A notehead of the kind {other:?}"))),
                };
            }
            "Fingering" => {
                if let Some(fingering) = fingering_of(held)? {
                    read.fingerings.push(fingering);
                }
            }
            "color" => read.color = Some(color_of(held)?),
            // Shape notes are drawn by the degree of the scale a note is.
            "headScheme" => {
                if !matches!(held.stripped(), "auto" | "normal") {
                    return Err(refused("A note drawn as a shape note"));
                }
            }
            "tuning" => {
                if held
                    .stripped()
                    .parse::<FloatType>()
                    .is_ok_and(|cents| cents != 0.0)
                {
                    return Err(refused("A note tuned away from its pitch"));
                }
            }
            "Spanner" => match held.get("type") {
                Some("Tie" | "LaissezVib" | "PartialTie") => {}
                Some(other) => return Err(refused(format!("A {other} on a note"))),
                None => {}
            },
            // Read above or below, or only drawn or played.
            "pitch" | "tpc" | "tpc2" | "Accidental" | "Tie" | "endSpanner" | "visible"
            | "small" | "velocity" | "veloType" | "veloOffset" | "play" | "eid" | "NoteDot"
            | "Events" | "mirror" | "dotPosition" | "fixed" | "fixedLine" | "offset" | "linked"
            | "linkedMain" | "track" | "Symbol" | "fret" | "string" | "ghost" | "dead" | "z"
            | "autoplace" | "LaissezVib" | "PartialTie" => {}
            other => return Err(refused(format!("A <{other}> on a note"))),
        }
    }
    for held in element.find_all("Spanner") {
        if held.get("type") == Some("Tie") {
            if held.find("next").is_some() {
                read.tie_start = true;
                read.tie_placement = match held.find("Tie").map(|tie| tie.child_text("up")) {
                    Some("up") => Some(Placement::Above),
                    Some("down") => Some(Placement::Below),
                    _ => None,
                };
            }
            if held.find("prev").is_some() {
                read.tie_stop = true;
            }
        }
    }
    // MuseScore 3 wrote a tie as a `<Tie>` on the note it starts from.
    if element.find("Tie").is_some() {
        read.tie_start = true;
    }
    if element.find("endSpanner").is_some() {
        read.tie_stop = true;
    }
    Ok(read)
}

/// The shape of a notehead by MuseScore's name for it; nothing for the
/// usual one.
fn head_named(name: &str) -> Result<Option<Notehead>> {
    let written = match name {
        "normal" => return Ok(None),
        "cross" => "x",
        "plus" => "cross",
        "xcircle" => "circle-x",
        "circled" => "circled",
        "triangle-up" => "triangle",
        "triangle-down" => "inverted triangle",
        "slashed1" => "slashed",
        "slashed2" => "back slashed",
        "diamond" | "slash" | "do" | "re" | "mi" | "la" | "ti" => name,
        "sol" => "so",
        other => return Err(refused(format!("A notehead of the shape {other:?}"))),
    };
    Notehead::from_name(written).map(Some)
}

/// A colour as MuseScore's export writes one.
fn color_of(element: &Xml) -> Result<String> {
    let part = |name: &str| -> Result<i64> {
        element
            .get(name)
            .and_then(|text| text.parse::<i64>().ok())
            .filter(|value| (0..=255).contains(value))
            .ok_or_else(|| mscx_error(format!("a colour with no {name}")))
    };
    if part("a")? != 255 {
        return Err(refused("A colour that is partly transparent"));
    }
    Ok(format!(
        "#{:02X}{:02X}{:02X}",
        part("r")?,
        part("g")?,
        part("b")?
    ))
}

/// A fingering written on a note: a finger, a plucking finger, or the
/// string it is played on.
fn fingering_of(element: &Xml) -> Result<Option<Articulation>> {
    if element.child_text("visible") == "0" {
        return Ok(None);
    }
    let text = element
        .find("text")
        .map(plain_text)
        .transpose()?
        .unwrap_or_default();
    let text = text.trim();
    let style = element.child_text("style");
    let made = |class: &str| {
        ArticulationKind::from_class_name(class)
            .map(Articulation::of_kind)
            .ok_or_else(|| mscx_error(format!("no articulation {class}")))
    };
    let plucked = matches!(text, "p" | "i" | "m" | "a" | "c");
    Ok(Some(match style {
        "String Number" | "string_number" => match text.parse::<IntegerType>() {
            Ok(0) => made("OpenString")?,
            Ok(number) if number > 0 => {
                let mut string = made("StringIndication")?;
                string.set_number(number);
                string
            }
            _ => return Ok(None),
        },
        "RH Guitar Fingering" | "guitar_fingering_rh" => made("FrettedPluck")?,
        "" | "Fingering" | "fingering" if plucked => made("FrettedPluck")?,
        "" | "Fingering" | "fingering" | "LH Guitar Fingering" | "guitar_fingering_lh" => {
            let mut fingering = made("Fingering")?;
            fingering.set_finger(Some(Finger::from_written(text)));
            fingering
        }
        other => return Err(refused(format!("A fingering in the style {other:?}"))),
    }))
}

/// A breath mark or a caesura.
fn breath_of(element: &Xml) -> Result<Articulation> {
    let symbol = element.child_text("symbol");
    let made = |class: &str| {
        ArticulationKind::from_class_name(class)
            .map(Articulation::of_kind)
            .ok_or_else(|| mscx_error(format!("no articulation {class}")))
    };
    let mut mark = if symbol.contains("aesura") {
        made("Caesura")?
    } else {
        let mut breath = made("BreathMark")?;
        breath.set_symbol(Some(
            match symbol {
                "breathMarkTick" => "tick",
                "breathMarkUpbow" => "upbow",
                "breathMarkSalzedo" => "salzedo",
                _ => "comma",
            }
            .to_string(),
        ));
        breath
    };
    if element.child_text("placement") == "below" {
        mark.set_placement(Some("below".to_string()));
    }
    Ok(mark)
}

fn lyric_of(element: &Xml) -> Result<LyricRead> {
    let number = child_integer(element, "no")?.unwrap_or(0) + 1;
    let syllabic = match element.child_text("syllabic") {
        "" => None,
        written => Some(Syllabic::from_name(written)?),
    };
    Ok(LyricRead {
        number: number as IntegerType,
        syllabic,
        text: element
            .find("text")
            .map(plain_text)
            .transpose()?
            .unwrap_or_default(),
    })
}

fn articulation_of(element: &Xml) -> Result<Option<Articulation>> {
    let subtype = element.child_text("subtype");
    let base = subtype
        .strip_suffix("Above")
        .or_else(|| subtype.strip_suffix("Below"))
        .unwrap_or(subtype);
    let class = match base {
        "articAccent" => "Accent",
        "articStaccato" => "Staccato",
        "articStaccatissimo" | "articStaccatissimoWedge" => "Staccatissimo",
        "articStaccatissimoStroke" => "Spiccato",
        "articTenuto" => "Tenuto",
        "articMarcato" => "StrongAccent",
        "articTenutoStaccato" => "DetachedLegato",
        "articStress" => "Stress",
        "articUnstress" => "Unstress",
        "stringsUpBow" => "UpBow",
        "stringsDownBow" => "DownBow",
        "stringsHarmonic" => "StringHarmonic",
        "brassMuteOpen" => "OpenString",
        "stringsThumbPosition" => "StringThumbPosition",
        "brassMuteClosed" => "Stopped",
        "pluckedSnapPizzicatoAbove" | "pluckedSnapPizzicato" => "SnapPizzicato",
        other => return Err(refused(format!("The articulation {other:?}"))),
    };
    let kind = ArticulationKind::from_class_name(class)
        .ok_or_else(|| mscx_error(format!("no articulation {class}")))?;
    let mut articulation = Articulation::of_kind(kind);
    if class == "StrongAccent" {
        let up = !subtype.ends_with("Below");
        articulation.set_point_direction(Some(if up { "up" } else { "down" }.to_string()));
    } else {
        // Above, below, or wherever it falls; any other number is an older
        // file's, which MuseScore reads as below.
        match element.child_text("anchor") {
            "" | "2" => {}
            "0" => articulation.set_placement(Some("above".to_string())),
            _ => articulation.set_placement(Some("below".to_string())),
        }
    }
    Ok(Some(articulation))
}

/// Whether a `<LayoutBreak>` ends a section, after which measures are
/// numbered from one again.
fn ends_section(element: &Xml) -> bool {
    element.child_text("subtype") == "section" && element.child_text("startWithMeasureOne") != "0"
}

/// Words saying where to go next: a repeat mark where they are ones music21
/// reads as one, and plain words otherwise.
fn repeat_words(text: &str) -> StreamElement {
    let repeat = RepeatExpressionKind::ALL
        .into_iter()
        .find(|kind| RepeatExpression::new(*kind).is_valid_text(text));
    match repeat {
        Some(kind) => {
            let mut mark = RepeatExpression::new(kind);
            mark.set_text(text);
            // The words are placed, not the mark: one drawn as its sign, a
            // coda or a segno, stands where a sign stands.
            if !mark.use_symbol() {
                mark.set_placement(Some(Placement::Above));
            }
            mark.into()
        }
        None => {
            let mut words = TextExpression::new(text);
            words.set_placement(Some(Placement::Above));
            words.into()
        }
    }
}

/// A `<Marker>`: a segno or a coda sign, which stands at the start of its
/// measure, or *Fine* or *To Coda*, which stand at its end. The first
/// answer is whether it stands at the end.
fn marker_of(element: &Xml, by_label: bool) -> Result<(bool, StreamElement)> {
    let sign = |kind: RepeatExpressionKind| (false, RepeatExpression::new(kind).into());
    let text = element
        .find("text")
        .map(plain_text)
        .transpose()?
        .unwrap_or_default();
    let kind = match element.child_text("markerType") {
        "" if by_label => element.child_text("label"),
        "" => "fine",
        kind => kind,
    };
    Ok(match kind {
        "segno" | "varsegno" => sign(RepeatExpressionKind::Segno),
        "codab" | "varcoda" | "codetta" => sign(RepeatExpressionKind::Coda),
        "fine" if !text.trim().is_empty() => (true, repeat_words(text.trim())),
        "coda" | "codasym" | "dacoda" | "dadblcoda" => (
            true,
            repeat_words(if text.trim().is_empty() {
                "To Coda"
            } else {
                text.trim()
            }),
        ),
        other => return Err(refused(format!("A marker of the kind {other:?}"))),
    })
}

/// A `<Jump>`: *D.C.*, *D.S.* and their kin, in the words written or else
/// the words MuseScore has for where the jump goes.
fn jump_of(element: &Xml) -> Result<StreamElement> {
    let text = element
        .find("text")
        .map(plain_text)
        .transpose()?
        .unwrap_or_default();
    let text = text.trim();
    let to = (
        element.child_text("jumpTo"),
        element.child_text("playUntil"),
        element.child_text("continueAt"),
    );
    let words = match to {
        ("segno", "end", "") => "D.S.",
        _ if !text.is_empty() => text,
        ("start", "end", "") => "D.C.",
        ("start", "fine", "") => "D.C. al Fine",
        ("start", "coda", "codab") => "D.C. al Coda",
        ("segno", "coda", "codab") => "D.S. al Coda",
        ("segno", "fine", "") => "D.S. al Fine",
        _ => return Err(refused("A jump that does not say in words where it goes")),
    };
    Ok(repeat_words(words))
}

fn ornament_of(element: &Xml) -> Result<Expression> {
    if element.find("Accidental").is_some() {
        return Err(refused("An ornament with an accidental"));
    }
    let subtype = element.child_text("subtype");
    let (class, turn) = match subtype {
        "ornamentTrill" => ("Trill", false),
        "ornamentTurn" => ("Turn", true),
        "ornamentTurnInverted" => ("InvertedTurn", true),
        "ornamentMordent"
        | "ornamentPrallMordent"
        | "ornamentUpMordent"
        | "ornamentDownMordent" => ("Mordent", false),
        "ornamentShortTrill"
        | "ornamentTremblement"
        | "ornamentUpPrall"
        | "ornamentPrecompMordentUpperPrefix"
        | "ornamentPrallDown"
        | "ornamentPrallUp"
        | "ornamentLinePrall" => ("InvertedMordent", false),
        "ornamentPrecompSlide" => ("Schleifer", false),
        other => return Err(refused(format!("The ornament {other:?}"))),
    };
    let kind = OrnamentKind::from_class_name(class)
        .ok_or_else(|| mscx_error(format!("no ornament {class}")))?;
    let mut ornament = Ornament::of_kind(kind);
    if turn {
        ornament.set_delay(OrnamentDelay::NoDelay);
    }
    Ok(ornament.into())
}

/// An arpeggio on one chord. One drawn across staves joins chords, which
/// is not read.
fn arpeggio_of(element: &Xml) -> Result<Expression> {
    if child_integer(element, "span")?.is_some_and(|span| span > 1) {
        return Err(refused("An arpeggio across staves"));
    }
    Ok(Expression::Arpeggio(match element.child_text("subtype") {
        "0" | "" | "arpeggiato" => ArpeggioType::Normal,
        "1" | "4" | "up" | "upStraight" => ArpeggioType::Up,
        "2" | "5" | "down" | "downStraight" => ArpeggioType::Down,
        "3" | "bracket" => ArpeggioType::NonArpeggio,
        other => return Err(refused(format!("An arpeggio of the kind {other:?}"))),
    }))
}

fn fermata_of(element: &Xml) -> Fermata {
    let mut fermata = Fermata::new();
    let subtype = element.child_text("subtype");
    if subtype.ends_with("Below") {
        fermata.set_fermata_type(FermataType::Inverted);
    } else {
        fermata.set_fermata_type(FermataType::Upright);
    }
    let shape = subtype
        .strip_suffix("Above")
        .or_else(|| subtype.strip_suffix("Below"))
        .unwrap_or(subtype);
    let shape = match shape {
        "fermata" => None,
        "fermataShort" => Some("angled"),
        "fermataLong" => Some("square"),
        "fermataVeryShort" => Some("double-angled"),
        "fermataVeryLong" => Some("double-square"),
        "fermataLongHenze" => Some("double-dot"),
        "fermataShortHenze" => Some("half-curve"),
        _ => None,
    };
    fermata.set_shape(shape.map(str::to_string));
    fermata
}

/// Where a mark is placed: what it says, else its default.
fn placement_of(element: &Xml, default: Placement) -> Placement {
    match element.child_text("placement") {
        "above" => Placement::Above,
        "below" => Placement::Below,
        _ => match element.child_text("direction") {
            "up" => Placement::Above,
            "down" => Placement::Below,
            _ => default,
        },
    }
}

/// A key signature as it is written, for a transposing instrument in the
/// key it reads.
fn key_of(element: &Xml, transposition: (i64, i64), old_format: bool) -> Result<StreamElement> {
    if flag(element, "custom") || element.find("KeySym").is_some() {
        return Err(refused("A key signature of its own making"));
    }
    // Before format 4.1 a signature said the key its staff is written in,
    // as `<accidental>`; since then it says the sounding key, and the
    // written one beside it where a transposing instrument has another.
    // MuseScore reads each format by its own rule and so does this.
    let mut sharps = if old_format {
        child_integer(element, "accidental")?.unwrap_or(0)
    } else {
        let concert = child_integer(element, "concertKey")?.unwrap_or(0);
        match child_integer(element, "actualKey")? {
            Some(actual) => actual,
            None => concert - 7 * transposition.1 + 12 * transposition.0,
        }
    };
    while sharps > 7 {
        sharps -= 12;
    }
    while sharps < -7 {
        sharps += 12;
    }
    let signature = KeySignature::new(sharps as IntegerType);
    let mode = element.child_text("mode");
    if !mode.is_empty()
        && mode != "none"
        && mode != "unknown"
        && let Ok(key) = signature.try_as_key(Some(mode), None)
    {
        return Ok(key.into());
    }
    Ok(signature.into())
}

fn meter_of(element: &Xml) -> Result<TimeSignature> {
    let numerator =
        child_integer(element, "sigN")?.ok_or_else(|| mscx_error("a meter with no <sigN>"))?;
    let denominator =
        child_integer(element, "sigD")?.ok_or_else(|| mscx_error("a meter with no <sigD>"))?;
    let written = element.child_text("textN");
    let ratio = if written.is_empty() {
        format!("{numerator}/{denominator}")
    } else {
        format!("{written}/{denominator}")
    };
    let mut meter = TimeSignature::from_ratio_string(&ratio)
        .map_err(|_| mscx_error(format!("the meter {ratio}")))?;
    match element.child_text("subtype") {
        "1" => meter.set_symbol(Some("common".to_string())),
        "2" => meter.set_symbol(Some("cut".to_string())),
        _ => {}
    }
    Ok(meter)
}

/// A barline written in a voice. A repeat is not one: that is the
/// measure's.
fn barline_of(element: &Xml) -> Result<Option<Barline>> {
    if element.child_text("visible") == "0" {
        return Ok(Some(Barline::new(BarlineType::None)));
    }
    let kind = match element.child_text("subtype") {
        "normal" | "" => BarlineType::Regular,
        "double" => BarlineType::Double,
        "end" => BarlineType::Final,
        "reverse-end" => BarlineType::HeavyLight,
        "heavy" => BarlineType::Heavy,
        "double-heavy" => BarlineType::HeavyHeavy,
        "dashed" => BarlineType::Dashed,
        "dotted" => BarlineType::Dotted,
        "start-repeat" | "end-repeat" | "end-start-repeat" => return Ok(None),
        other => return Err(mscx_error(format!("a barline of the kind {other:?}"))),
    };
    Ok(Some(Barline::new(kind)))
}

/// A tempo mark: the mark, and whatever words stand beside it.
fn tempo_of(element: &Xml) -> Result<Vec<StreamElement>> {
    let per_second: FloatType = element
        .child_text("tempo")
        .parse()
        .map_err(|_| mscx_error("a tempo that is not a number"))?;
    let bpm = (per_second * 60.0 * 100.0).round() / 100.0;
    if element.child_text("visible") == "0" {
        return Ok(vec![
            MetronomeMark::default().with_number_sounding(bpm).into(),
        ]);
    }
    let placement = placement_of(element, Placement::Above);
    let text = element.find("text");
    let found = text.and_then(metronome_in);
    let mut out: Vec<StreamElement> = Vec::new();
    match found {
        Some((left, referent, number)) => {
            if !left.trim().is_empty() {
                let mut words = TextExpression::new(left.trim());
                words.set_placement(Some(placement));
                out.push(words.into());
            }
            let mut mark = MetronomeMark::default();
            mark.set_number(Some(number));
            mark.set_referent(referent);
            mark.set_placement(Some(placement));
            out.push(mark.into());
        }
        None => {
            if let Some(text) = text {
                let written = plain_text(text)?;
                if !written.trim().is_empty() {
                    let mut words = TextExpression::new(written.trim());
                    words.set_placement(Some(placement));
                    out.push(words.into());
                }
            }
            let mut mark = MetronomeMark::default().with_number_sounding(bpm);
            mark.set_placement(Some(placement));
            out.push(mark.into());
        }
    }
    Ok(out)
}

/// The metronome mark in a tempo's text: the words before it, the note it
/// counts and how many to the minute.
fn metronome_in(text: &Xml) -> Option<(String, Duration, FloatType)> {
    let mut before = String::new();
    if let Some(leading) = text.text() {
        before.push_str(leading);
    }
    for (index, child) in text.children.iter().enumerate() {
        if child.tag == "sym" {
            let name = child.text().unwrap_or("").trim();
            let referent = match name {
                "metNoteWhole" => Duration::from_type(DurationType::Whole),
                "metNoteHalfUp" => Duration::from_type(DurationType::Half),
                "metNoteQuarterUp" => Duration::from_type(DurationType::Quarter),
                "metNote8thUp" => Duration::from_type(DurationType::Eighth),
                "metNote16thUp" => Duration::from_type(DurationType::Sixteenth),
                _ => return None,
            };
            let mut referent = referent;
            let mut rest = String::new();
            let mut dots = 0;
            if let Some(tail) = child.tail() {
                rest.push_str(tail);
            }
            for later in &text.children[index + 1..] {
                if later.tag == "sym" && later.text().unwrap_or("").trim() == "metAugmentationDot" {
                    dots += 1;
                } else {
                    rest.push_str(&plain_text(later).ok()?);
                }
                if let Some(tail) = later.tail() {
                    rest.push_str(tail);
                }
            }
            if dots > 0
                && let Some((kind, _)) = referent.type_and_dots()
            {
                referent = Duration::from_type_with_dots(kind, dots);
            }
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('=')?.trim_start();
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                .collect();
            let number: FloatType = digits.replace(',', ".").parse().ok()?;
            let before = before.trim_end_matches('(').to_string();
            return Some((before, referent, number));
        }
        before.push_str(&plain_text(child).ok()?);
        if let Some(tail) = child.tail() {
            before.push_str(tail);
        }
    }
    None
}

/// Words written over or under the staff; words saying where to go next
/// are a repeat mark.
fn words_of(element: &Xml, default: Placement) -> Result<Option<StreamElement>> {
    let text = element
        .find("text")
        .map(plain_text)
        .transpose()?
        .unwrap_or_default();
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let placement = placement_of(element, default);
    let repeat = RepeatExpressionKind::ALL
        .into_iter()
        .find(|kind| RepeatExpression::new(*kind).is_valid_text(text));
    Ok(Some(match repeat {
        Some(kind) => {
            let mut mark = RepeatExpression::new(kind);
            mark.set_text(text);
            if !mark.use_symbol() {
                mark.set_placement(Some(placement));
            }
            mark.into()
        }
        None => {
            let mut words = TextExpression::new(text);
            words.set_placement(Some(placement));
            words.into()
        }
    }))
}

/// A chord symbol: its root and bass as tonal pitch classes, and the name
/// of its kind as written, which is read as music21 reads a figure.
fn harmony_of(element: &Xml) -> Result<ChordSymbol> {
    let info = element.find("harmonyInfo").unwrap_or(element);
    let name = info.child_text("name");
    let Some(root) = child_integer(info, "root")? else {
        return Err(refused("A chord symbol with no root"));
    };
    let pitch = |tpc: i64| -> Result<(Pitch, String)> {
        let (step, alter) = spelling(tpc)?;
        let mut pitch = Pitch::from_name(step.to_string())?;
        if alter != 0 {
            pitch.set_written_accidental(Some(Accidental::new(alter as FloatType)?));
        }
        let written = match alter {
            -2 => "--",
            -1 => "-",
            1 => "#",
            2 => "##",
            _ => "",
        };
        Ok((pitch, format!("{step}{written}")))
    };
    let (root, root_name) = pitch(root)?;
    let bass = match child_integer(info, "bass")? {
        Some(bass) => Some(bass),
        None => child_integer(info, "base")?,
    }
    .map(pitch)
    .transpose()?;
    let mut figure = format!("{root_name}{name}");
    if let Some((_, bass_name)) = &bass {
        figure.push('/');
        figure.push_str(bass_name);
    }
    let unread = || refused(format!("The chord symbol {figure:?}"));
    let parsed = ChordSymbol::parse_music21(&figure).map_err(|_| unread())?;
    let kind = parsed
        .kind()
        .filter(|kind| !kind.is_empty())
        .ok_or_else(unread)?
        .to_string();
    // The text of the kind itself: the name up to where its added and
    // altered degrees begin.
    let mut kind_text = name;
    if !parsed.chord_step_modifications().is_empty() {
        kind_text = "";
        for (end, _) in name.char_indices().skip(1).chain([(name.len(), ' ')]) {
            let alone = ChordSymbol::parse_music21(format!("{root_name}{}", &name[..end]));
            if let Ok(alone) = alone
                && alone.kind() == Some(kind.as_str())
                && alone.chord_step_modifications().is_empty()
            {
                kind_text = &name[..end];
            }
        }
    }
    let mut symbol =
        ChordSymbol::from_kind(root, &kind, bass.map(|(pitch, _)| pitch)).map_err(|_| unread())?;
    symbol.set_kind_text(Some(kind_text.to_string()));
    symbol.set_chord_step_modifications(parsed.chord_step_modifications().to_vec());
    Ok(symbol)
}

// ---------------------------------------------------------------- the head

fn metadata_of(score: &Xml, program: &str) -> Metadata {
    let mut tags: Vec<(String, String)> = score
        .find_all("metaTag")
        .filter_map(|tag| {
            let name = tag.get("name")?.to_string();
            let value = tag.all_text();
            (!value.is_empty()).then_some((name, value))
        })
        .collect();
    // MuseScore keeps its tags sorted by name.
    tags.sort_by(|left, right| left.0.cmp(&right.0));
    let get = |name: &str| {
        tags.iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.clone())
    };
    let mut metadata = Metadata::new();
    if let Some(number) = get("workNumber") {
        metadata.add_text("number", number);
    }
    if let Some(number) = get("movementNumber") {
        metadata.add_text("movementNumber", number);
    }
    // A work title that only repeats the movement's is left out, as the
    // MusicXML reader leaves it out, so that both read one score alike.
    let title = get("workTitle").filter(|title| Some(title) != get("movementTitle").as_ref());
    if let Some(title) = title {
        metadata.add_text("title", title);
    }
    if let Some(title) = get("movementTitle") {
        metadata.add_text("movementName", title);
    }
    for role in ["arranger", "composer", "lyricist", "poet", "translator"] {
        if let Some(name) = get(role) {
            let unique = if crate::metadata::is_contributor_unique_name(role) {
                role
            } else {
                "otherContributor"
            };
            metadata.add(unique, MetadataValue::with_role(name.trim(), role));
        }
    }
    if let Some(copyright) = get("copyright") {
        metadata.add("copyright", MetadataValue::new(copyright.trim()));
    }
    if !program.is_empty() {
        metadata.add_text("software", program);
    }
    const NAMED: [&str; 12] = [
        "arranger",
        "composer",
        "lyricist",
        "poet",
        "translator",
        "copyright",
        "source",
        "workTitle",
        "workNumber",
        "movementTitle",
        "movementNumber",
        "originalFormat",
    ];
    for (name, value) in &tags {
        if NAMED.contains(&name.as_str()) {
            continue;
        }
        let unique = crate::metadata::STANDARD_PROPERTIES
            .iter()
            .find(|(unique, namespaced, _)| unique == name || namespaced == name)
            .map_or(name.as_str(), |(unique, _, _)| unique);
        metadata.add_text(unique, value.clone());
    }
    metadata
}

/// music21's `getDefaultInstrument`, from what MuseScore says of the
/// instrument: a program and a name.
fn instrument_of(program: Option<u8>, channel: Option<u8>, name: Option<&str>) -> Instrument {
    let mut instrument = program
        .and_then(|program| Instrument::from_midi_program(program).ok())
        .unwrap_or_default();
    if program.is_some() {
        instrument.set_midi_channel(channel);
    }
    if let Some(name) = name.filter(|name| !name.is_empty()) {
        let mut from_name = Instrument::from_name(name, SearchLanguage::All).unwrap_or_default();
        from_name.set_midi_channel(instrument.midi_channel());
        if from_name.midi_program() == instrument.midi_program() || instrument.is_a("Piano") {
            instrument = from_name;
        }
    }
    instrument
}

fn parts_of(score: &Xml, hides_single_name: bool) -> Result<Vec<PartRead>> {
    let mut parts = Vec::new();
    let mut first_staff = 0;
    let mut channel: usize = 0;
    let listed: Vec<&Xml> = score.find_all("Part").collect();
    let hidden = hides_single_name && listed.len() == 1;
    for (index, part) in listed.into_iter().enumerate() {
        let staves = part.find_all("Staff").count();
        if staves == 0 {
            return Err(mscx_error("a part with no staff"));
        }
        for staff in part.find_all("Staff") {
            if let Some(kind) = staff.find("StaffType")
                && let Some(group) = kind.get("group")
                && group != "pitched"
            {
                return Err(refused(format!("A {group} staff")));
            }
        }
        let instrument = part
            .find("Instrument")
            .ok_or_else(|| mscx_error("a part with no instrument"))?;
        if flag(instrument, "useDrumset") {
            return Err(refused("A drum set"));
        }
        let text = |element: &Xml, tag: &str| -> Result<Option<String>> {
            Ok(element
                .find(tag)
                .map(plain_text)
                .transpose()?
                .map(|text| text.trim().replace('\n', " "))
                .filter(|text| !text.is_empty()))
        };
        let long_name = text(instrument, "longName")?;
        let short_name = text(instrument, "shortName")?;
        let track_name = text(instrument, "trackName")?;
        let part_track = text(part, "trackName")?;
        // A part with no name of its own goes by its track's, which is not
        // printed.
        let (name, name_hidden) = match long_name {
            Some(name) => (Some(name), hidden),
            None => (part_track.clone(), part_track.is_some() || hidden),
        };
        let abbreviation_hidden = name_hidden && short_name.is_some();
        let mut brackets: Vec<(i64, i64, usize)> = Vec::new();
        let mut bracket_found = false;
        for (staff_index, staff) in part.find_all("Staff").enumerate() {
            for bracket in staff.find_all("bracket") {
                let number = |name: &str| {
                    bracket
                        .get(name)
                        .and_then(|value| value.parse::<i64>().ok())
                        .unwrap_or(0)
                };
                if number("type") < 0 {
                    continue;
                }
                bracket_found = true;
                if staff_index == 0 {
                    brackets.push((
                        number("col"),
                        number("type"),
                        number("span").max(0) as usize,
                    ));
                }
            }
        }
        brackets.sort_by_key(|(column, _, _)| *column);
        let channels: Vec<&Xml> = instrument.find_all("Channel").collect();
        let program = channels
            .first()
            .and_then(|first| first.find("program"))
            .and_then(|program| program.get("value"))
            .and_then(|value| value.parse::<u8>().ok());
        // The channel the file gives the instrument's first sound, or the
        // next one free where it gives none; the tenth is the drums'.
        let written = channels
            .first()
            .and_then(|first| first.find("midiChannel"))
            .map(integer)
            .transpose()?;
        let mut first_channel = written.map(|written| written.rem_euclid(16) as u8);
        for _ in 0..channels.len().max(1) {
            if channel % 16 == 9 {
                channel += 1;
            }
            first_channel.get_or_insert((channel % 16) as u8);
            channel += 1;
        }
        let mut made = instrument_of(program, first_channel, track_name.as_deref());
        made.set_part_id(Some(format!("P{}", index + 1)));
        made.set_part_name(name.clone());
        made.set_part_abbreviation(short_name.clone());
        if let Some(track_name) = &track_name {
            made.set_name(Some(track_name.clone()));
        }
        let sound = instrument.child_text("instrumentId");
        if !sound.is_empty() {
            made.set_sound(Some(sound.to_string()));
        }
        let transposition = (
            child_integer(instrument, "transposeDiatonic")?.unwrap_or(0),
            child_integer(instrument, "transposeChromatic")?.unwrap_or(0),
        );
        if transposition.1 != 0 {
            // The interval from the written note to the sounding one, the
            // octaves counted into both halves.
            let octaves = transposition.1 / 12;
            let diatonic = transposition.0 % 7 + 7 * octaves;
            let steps = if diatonic < 0 {
                diatonic - 1
            } else {
                diatonic + 1
            };
            let interval = Interval::from_generic_and_chromatic(
                steps as IntegerType,
                transposition.1 as IntegerType,
            )
            .or_else(|_| Interval::from_semitones(transposition.1 as IntegerType))?;
            made.set_transposition(Some(interval));
        }
        // The clef a staff starts with where its first measure writes
        // none: the staff's own, else a treble clef, whatever clef its
        // instrument is usually written in. A transposing instrument's is
        // the one it reads.
        let mut default_clefs = vec![String::from("G"); staves];
        for (staff_index, staff) in part.find_all("Staff").enumerate() {
            for tag in [
                "defaultClef",
                "defaultConcertClef",
                "defaultTransposingClef",
            ] {
                if let Some(clef) = staff.find(tag) {
                    default_clefs[staff_index] = clef.stripped().to_string();
                }
            }
        }
        parts.push(PartRead {
            staves,
            first_staff,
            name,
            abbreviation: short_name,
            name_hidden,
            abbreviation_hidden,
            instrument: made,
            default_clefs,
            brackets: brackets
                .into_iter()
                .map(|(_, kind, span)| (kind, span))
                .collect(),
            bracket_found,
            transposition,
        });
        first_staff += staves;
    }
    if parts.is_empty() {
        return Err(mscx_error("a score with no parts"));
    }
    Ok(parts)
}

// --------------------------------------------------------------- assembly

/// One element of a measure as music21 would hold it once read.
#[derive(Clone, Debug)]
struct Item {
    offset: FloatType,
    seq: usize,
    staff: usize,
    uid: Option<usize>,
    element: StreamElement,
}

impl Item {
    fn order(&self) -> i32 {
        self.element.class_sort_order()
    }

    fn is_grace(&self) -> bool {
        self.element.duration().is_some_and(Duration::is_grace)
    }
}

fn sorted(mut items: Vec<Item>) -> Vec<Item> {
    items.sort_by(|left, right| {
        left.offset
            .total_cmp(&right.offset)
            .then(left.order().cmp(&right.order()))
            .then(right.is_grace().cmp(&left.is_grace()))
            .then(left.seq.cmp(&right.seq))
    });
    items
}

#[derive(Clone, Debug, Default)]
struct VoiceIr {
    id: String,
    items: Vec<Item>,
}

#[derive(Clone, Debug, Default)]
struct MeasureIr {
    offset: FloatType,
    number: IntegerType,
    suffix: Option<String>,
    hidden: bool,
    left: Option<Barline>,
    right: Option<Barline>,
    ending: Option<Ending>,
    items: Vec<Item>,
    voices: Vec<VoiceIr>,
}

fn build_measure(measure: MeasureIr) -> (Stream, Vec<Option<usize>>) {
    let mut entries: Vec<(Item, Vec<Option<usize>>)> = Vec::new();
    for (index, voice) in measure.voices.into_iter().enumerate() {
        let mut uids = Vec::new();
        let mut events = Vec::new();
        for item in sorted(voice.items) {
            uids.push(item.uid);
            events.push(StreamEvent::new(item.offset, item.element));
        }
        let mut stream = Stream::from_events(events);
        stream.set_kind(StreamKind::Voice);
        stream.set_id(Some(voice.id));
        entries.push((
            Item {
                offset: 0.0,
                seq: index,
                staff: 0,
                uid: None,
                element: stream.into(),
            },
            uids,
        ));
    }
    let voices = entries.len();
    for mut item in measure.items {
        item.seq = item.seq.saturating_add(voices);
        let uid = item.uid;
        entries.push((item, vec![uid]));
    }
    entries.sort_by(|(left, _), (right, _)| {
        left.offset
            .total_cmp(&right.offset)
            .then(left.order().cmp(&right.order()))
            .then(right.is_grace().cmp(&left.is_grace()))
            .then(left.seq.cmp(&right.seq))
    });
    let mut uids = Vec::new();
    let mut events = Vec::new();
    for (item, leaf_uids) in entries {
        uids.extend(leaf_uids);
        events.push(StreamEvent::new(item.offset, item.element));
    }
    let mut stream = Stream::from_events(events);
    stream.set_kind(StreamKind::Measure);
    stream.set_number(measure.number);
    stream.set_number_suffix(measure.suffix);
    stream.set_number_hidden(measure.hidden);
    stream.set_left_barline(measure.left);
    stream.set_right_barline(measure.right);
    stream.set_ending(measure.ending);
    (stream, uids)
}

/// The tuplets a note is written inside, outermost first, each marked where
/// it starts and stops.
fn tuplets_for(
    tuplets: &[TupletRead],
    innermost: Option<usize>,
    note: usize,
    brackets: &[bool],
) -> Vec<Tuplet> {
    let mut chain = Vec::new();
    let mut open = innermost;
    let mut held = Held::Note(note);
    while let Some(index) = open {
        let tuplet = &tuplets[index];
        let first = tuplet.elements.first() == Some(&held);
        let last = tuplet.elements.last() == Some(&held);
        let mut made = Tuplet::new(tuplet.actual, tuplet.normal, tuplet.base.0, tuplet.base.1);
        made.set_tuplet_type(match (first, last) {
            (true, true) => Some(TupletType::StartStop),
            (true, false) => Some(TupletType::Start),
            (false, true) => Some(TupletType::Stop),
            _ => None,
        });
        let bracket = match tuplet.bracket {
            1 => true,
            2 => false,
            // As the beams decide.
            _ => brackets[index],
        };
        made.set_bracket(if bracket {
            TupletBracket::Bracket
        } else {
            TupletBracket::None
        });
        match tuplet.number {
            1 => {
                made.set_normal_show(Some(TupletShow::Number));
            }
            2 => made.set_actual_show(None),
            _ => {}
        }
        made.set_placement(tuplet.placement);
        chain.push(made);
        held = Held::Tuplet(index);
        open = tuplet.parent;
    }
    chain.reverse();
    chain
}

fn collect_notes(tuplets: &[TupletRead], index: usize, out: &mut Vec<usize>) {
    for held in &tuplets[index].elements {
        match held {
            Held::Note(note) => out.push(*note),
            Held::Tuplet(inner) => collect_notes(tuplets, *inner, out),
        }
    }
}

/// The note, chord or rest a read one stands for.
fn element_of(
    read: &ChordRest,
    tuplets: Vec<Tuplet>,
    beams: Option<Beams>,
    beam_stem: StemDirection,
) -> Result<StreamElement> {
    // Its own stem direction, else its beam's; a note written with no
    // stem has neither.
    let stem = match read.stem {
        _ if read.no_stem => StemDirection::NoStem,
        _ if read.order < 3 => StemDirection::Unspecified,
        StemDirection::Unspecified => beam_stem,
        own => own,
    };
    let duration = duration_of(read, tuplets)?;
    let lyrics: Vec<Lyric> = read
        .lyrics
        .iter()
        .map(|lyric| {
            let mut made = Lyric::unsung();
            made.set_text(lyric.text.trim());
            made.set_syllabic(lyric.syllabic.unwrap_or(Syllabic::Single));
            made.set_number(lyric.number);
            made
        })
        .collect();
    // Fermatas, then an arpeggio, then the ornaments.
    let is_arpeggio = |expression: &&Expression| matches!(expression, Expression::Arpeggio(_));
    let expressions: Vec<Expression> = read
        .fermatas
        .iter()
        .cloned()
        .map(Expression::Fermata)
        .chain(read.expressions.iter().filter(is_arpeggio).cloned())
        .chain(
            read.expressions
                .iter()
                .filter(|expression| !is_arpeggio(expression))
                .cloned(),
        )
        .collect();
    if read.is_rest {
        let mut rest = Rest::new(duration);
        rest.set_hidden(read.hidden);
        if read.small {
            rest.set_size(Some(NoteSize::Cue));
        }
        rest.set_step_shift(read.step_shift);
        *rest.lyrics_mut() = lyrics;
        *rest.expressions_mut() = expressions;
        *rest.articulations_mut() = read.articulations.clone();
        return Ok(rest.into());
    }
    let mut notes: Vec<Note> = Vec::new();
    for written in &read.notes {
        let mut note = Note::from_pitch(written.pitch.clone());
        note.set_duration(duration.clone());
        // MuseScore hides a note's head and its stem each on its own, so a
        // hidden note is one with no head and not one left off the page.
        if written.hidden {
            note.set_notehead(Notehead::NoneShape);
        }
        if written.small || read.small {
            note.set_size(Some(NoteSize::Cue));
        }
        let tie = match (written.tie_start, written.tie_stop) {
            (true, true) => Some(TieType::Continue),
            (true, false) => Some(TieType::Start),
            (false, true) => Some(TieType::Stop),
            (false, false) => None,
        };
        note.set_tie(tie.map(|tie_type| {
            let mut tie = Tie::new(tie_type);
            if tie_type == TieType::Start {
                tie.set_placement(written.tie_placement);
            }
            tie
        }));
        if let Some(velocity) = written.velocity {
            // MuseScore's MusicXML writes a velocity as a percentage of
            // ninety to two places, which is as fine as the two can agree.
            let percent = (velocity as FloatType * 10000.0 / 90.0).round() / 100.0;
            note.set_volume(Some(Volume::from_velocity_scalar(
                percent * 90.0 / 12700.0,
            )?));
        }
        if let Some(head) = written.head {
            note.set_notehead(head);
        }
        if written.color.is_some() {
            note.set_color(written.color.clone());
        }
        if written.head_filled.is_some() {
            note.set_notehead_fill(written.head_filled);
        }
        note.set_stem_direction(stem);
        notes.push(note);
    }
    // The marks of the chord, each note's fingerings among them: those of
    // the lowest note first, then what is written on the chord itself, then
    // the fingerings of the notes above.
    let mut articulations: Vec<Articulation> = Vec::new();
    for (index, written) in read.notes.iter().enumerate() {
        articulations.extend(written.fingerings.iter().cloned());
        if index == 0 {
            articulations.extend(read.articulations.iter().cloned());
        }
    }
    if notes.len() == 1 {
        let mut note = notes.remove(0);
        if let Some(beams) = beams {
            note.set_beams(beams);
        }
        *note.lyrics_mut() = lyrics;
        *note.articulations_mut() = articulations;
        *note.expressions_mut() = expressions;
        return Ok(note.into());
    }
    if let Some(first) = notes.first_mut() {
        *first.lyrics_mut() = lyrics;
    }
    let mut chord = Chord::new(notes)?;
    chord.set_duration(duration);
    if let Some(beams) = beams {
        chord.set_beams(beams);
    }
    *chord.articulations_mut() = articulations;
    *chord.expressions_mut() = expressions;
    Ok(chord.into())
}

fn duration_of(read: &ChordRest, tuplets: Vec<Tuplet>) -> Result<Duration> {
    let quarter_length = quarters(read.length);
    let Some((kind, dots)) = read.value else {
        return Duration::new(quarter_length);
    };
    if let Some(grace) = read.grace {
        let mut written = Duration::default();
        written.clear();
        written.set_tuplets(Vec::new());
        written.add_duration_tuple(kind, dots);
        let mut value = written.grace_duration();
        let mut marks = Grace::new();
        marks.set_slash(grace == GraceKind::Acciaccatura);
        value.set_grace(Some(marks));
        return Ok(value);
    }
    if tuplets.is_empty() {
        return Duration::new(quarter_length);
    }
    let mut duration = Duration::default();
    duration.clear();
    duration.set_tuplets(Vec::new());
    duration.add_duration_tuple(kind, dots);
    duration.set_tuplets(tuplets);
    if (duration.quarter_length() - quarter_length).abs() > 1e-7 {
        duration.set_linked(false);
        duration.set_quarter_length(quarter_length)?;
    }
    Ok(duration)
}

/// A part put together out of its measures, with the uid of each leaf.
struct BuiltPart {
    stream: Stream,
    uids: Vec<Option<usize>>,
}

/// Reads a MuseScore file into a score.
///
/// The text is an uncompressed `.mscx` as MuseScore 4 saves it, or as
/// MuseScore 3 does where the two formats are the same. A `.mscz` is a zip
/// archive holding one; the library opens no file and unpacks nothing, so
/// the caller hands over the text of the `.mscx` inside it.
///
/// Every `<Part>` is a part, and a part on several staves -- a piano -- is
/// a part for each staff joined by a brace, as a MusicXML reader makes it.
/// Pitches are spelled as the file spells them, and a transposing
/// instrument's are written as it reads them.
///
/// # What is read
///
/// - Parts, their names and abbreviations, instruments, MIDI programs and
///   channels, transpositions, and the brackets and braces joining staves.
/// - Measures and their numbers -- pickups, measures left out of the count,
///   sections that start again from one -- and up to four voices in each.
/// - Notes, chords and rests, measure rests among them: lengths, dots,
///   tuplets and tuplets inside tuplets, ties, written accidentals,
///   noteheads, colours, velocities, stems the file fixes, and grace notes
///   before and after a note.
/// - Clefs, key signatures, meters, tempo marks, dynamics, staff and system
///   text, lyrics and chord symbols.
/// - Articulations, fingerings, breath marks, fermatas on notes and on
///   barlines, ornaments and arpeggios.
/// - Barlines, repeats, endings, and the signs and words that send a
///   player elsewhere: segno, coda, *To Coda*, *Fine*, *D.C.* and *D.S.*
/// - Slurs, hairpins and pedal lines.
/// - The title, composer and the rest of the file's `metaTag`s.
///
/// A MuseScore file leaves some of what a page shows to be worked out when
/// the page is laid out, and that is worked out here as MuseScore does it:
/// beams from the meter and the beam modes of the notes, whether a tuplet
/// takes a bracket, the clef and key signature a staff starts with when
/// none is written, and the final barline.
///
/// # What is not read
///
/// What is only drawn -- frames, page and system breaks, positions, fonts,
/// rehearsal marks, and whatever is marked invisible -- is passed over.
/// MuseScore 4 keeps a score's style in a file of its own beside the
/// `.mscx`, which this function is not given; where the style decides
/// something, MuseScore's default is taken.
///
/// What the crate has no value for, or cannot read yet, is refused with an
/// [`Error::MuseScore`] naming it rather than dropped: percussion and
/// tablature staves, files in a format older than MuseScore 3's, tremolos,
/// trill, octave, glissando and text lines, arpeggios across staves,
/// ornaments with accidentals, key signatures of a score's own making,
/// measure repeats, fret diagrams, figured bass, instrument changes, shape
/// notes, and a chord symbol whose name is none the crate knows.
///
/// ```
/// use music21_rs::musescore::from_mscx;
///
/// let score = from_mscx(
///     r#"<museScore version="4.20"><Score>
///     <Part><Staff/><trackName>Flute</trackName>
///       <Instrument><longName>Flute</longName><trackName>Flute</trackName>
///       <Channel><program value="73"/></Channel></Instrument></Part>
///     <Staff id="1"><Measure><voice>
///       <TimeSig><sigN>2</sigN><sigD>4</sigD></TimeSig>
///       <Chord><durationType>quarter</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>
///       <Chord><durationType>quarter</durationType><Note><pitch>63</pitch><tpc>11</tpc></Note></Chord>
///     </voice></Measure></Staff>
///     </Score></museScore>"#,
/// )?;
/// let names: Vec<String> = score.parts()[0]
///     .pitches()
///     .iter()
///     .map(|pitch| pitch.name_with_octave())
///     .collect();
/// assert_eq!(names, ["C4", "Eb4"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_mscx(document: &str) -> Result<Stream> {
    let root = Xml::parse(document)?;
    if root.tag != "museScore" {
        return Err(mscx_error(format!(
            "a MuseScore file starts with <museScore>, not <{}>",
            root.tag
        )));
    }
    let version = root.get("version").unwrap_or("");
    let format: FloatType = version.parse().unwrap_or(0.0);
    if format < 3.0 {
        return Err(refused(format!("A file in MuseScore's format {version:?}")));
    }
    let score = root
        .find("Score")
        .ok_or_else(|| mscx_error("a MuseScore file with no <Score>"))?;

    let program_version = root.child_text("programVersion");
    let program = if program_version.is_empty() {
        String::new()
    } else {
        let studio = program_version
            .split('.')
            .take(2)
            .map(|piece| piece.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>();
        let renamed = studio.first().copied().unwrap_or(0) > 4
            || (studio.first() == Some(&4) && studio.get(1).copied().unwrap_or(0) >= 4);
        if renamed {
            format!("MuseScore Studio {program_version}")
        } else {
            format!("MuseScore {program_version}")
        }
    };
    let metadata = metadata_of(score, &program);
    // A score of one part does not print the part's name, unless its style
    // says otherwise. MuseScore 4 keeps the style in a file of its own,
    // which is not read; MuseScore 3 wrote it into the score.
    let hides_single_name = score
        .find("Style")
        .and_then(|style| style.find("hideInstrumentNameIfOneInstrument"))
        .is_none_or(|hide| hide.stripped() != "0");
    let parts = parts_of(score, hides_single_name)?;
    let staves: Vec<&Xml> = score.find_all("Staff").collect();
    let staff_count: usize = parts.iter().map(|part| part.staves).sum();
    if staves.len() != staff_count {
        return Err(mscx_error(format!(
            "{} staves are written for parts with {staff_count}",
            staves.len()
        )));
    }

    let mut reader = Reader {
        old_format: format < 4.1,
        markers_by_label: format < 4.6,
        next_uid: 0,
        spanners: Vec::new(),
    };
    let mut read_staves: Vec<Vec<StaffMeasure>> = Vec::new();
    for part in &parts {
        for index in 0..part.staves {
            let global = part.first_staff + index;
            read_staves.push(reader.staff(staves[global], global, part.transposition)?);
        }
    }
    let measure_count = read_staves.first().map_or(0, Vec::len);
    if read_staves.iter().any(|staff| staff.len() != measure_count) {
        return Err(mscx_error("staves with different numbers of measures"));
    }

    let grid = grid_of(&mut read_staves)?;
    Assembler {
        parts: &parts,
        staves: &read_staves,
        grid: &grid,
        spanners: &reader.spanners,
        uids: HashMap::new(),
        placed: Vec::new(),
    }
    .score(metadata)
}

/// Where each measure starts and how long it is.
///
/// A clef, a key or a meter written at the very end of a measure is the
/// next measure's, and is moved to its start on the way.
fn grid_of(staves: &mut [Vec<StaffMeasure>]) -> Result<Grid> {
    let count = staves.first().map_or(0, Vec::len);
    let mut grid = Grid {
        starts: Vec::with_capacity(count),
        lengths: Vec::with_capacity(count),
        meters: Vec::with_capacity(count),
    };
    let mut meter = (4, 4);
    let mut start = Frac::zero();
    for index in 0..count {
        // The meter a measure starts with, from whichever staff says one.
        for staff in staves.iter() {
            let found = staff[index]
                .voices
                .iter()
                .flat_map(|voice| &voice.events)
                .find_map(|event| match &event.what {
                    What::Meter(written) if event.tick.is_zero() => Some(written),
                    _ => None,
                });
            if let Some(written) = found {
                meter = (
                    i64::from(written.numerator()),
                    i64::from(written.denominator()),
                );
                break;
            }
        }
        let length = staves
            .iter()
            .find_map(|staff| staff[index].len)
            .unwrap_or_else(|| Frac::new(meter.0, meter.1));
        grid.starts.push(start);
        grid.lengths.push(length);
        grid.meters.push(meter);
        start += length;
        if index + 1 < count {
            for staff in staves.iter_mut() {
                let mut moved: Vec<Event> = Vec::new();
                for voice in &mut staff[index].voices {
                    let (ending, kept): (Vec<Event>, Vec<Event>) =
                        std::mem::take(&mut voice.events)
                            .into_iter()
                            .partition(|event| {
                                event.tick == length
                                    && matches!(
                                        event.what,
                                        What::Clef(_) | What::Key(_) | What::Meter(_)
                                    )
                            });
                    voice.events = kept;
                    moved.extend(ending);
                }
                if moved.is_empty() {
                    continue;
                }
                let next = &mut staff[index + 1];
                if next.voices.is_empty() {
                    next.voices.push(VoiceRead::default());
                }
                for (place, mut event) in moved.into_iter().enumerate() {
                    event.tick = Frac::zero();
                    next.voices[0].events.insert(place, event);
                }
            }
        }
    }
    Ok(grid)
}

struct Assembler<'a> {
    parts: &'a [PartRead],
    staves: &'a [Vec<StaffMeasure>],
    grid: &'a Grid,
    spanners: &'a [SpannerRead],
    /// Each note's uid by where it stands: measure, tick and track.
    uids: HashMap<(i64, Frac, usize, Option<usize>), usize>,
    /// Every note, chord and rest in the order MuseScore writes them out:
    /// part by part, measure by measure, staff by staff, voice by voice.
    placed: Vec<Placed>,
}

/// A note, chord or rest by where it stands, from the top of the score.
#[derive(Clone, Copy, Debug)]
struct Placed {
    part: usize,
    track: usize,
    start: Frac,
    end: Frac,
    uid: usize,
}

impl Assembler<'_> {
    fn score(mut self, metadata: Metadata) -> Result<Stream> {
        let mut built: Vec<BuiltPart> = Vec::new();
        let mut groups: Vec<StaffGroup> = Vec::new();
        let mut part_positions: Vec<Vec<usize>> = Vec::new();
        for (index, part) in self.parts.iter().enumerate() {
            let first = built.len();
            let staves = self.part(index, part)?;
            let count = staves.len();
            built.extend(staves);
            part_positions.push((first..first + count).collect());
            if count > 1 {
                let mut group = StaffGroup::new((first..first + count).collect());
                group.set_name(part.name.clone());
                group.set_symbol(Some("brace"))?;
                group.set_bar_together(Some(BarTogether::Yes));
                group.set_name_hidden(true);
                groups.push(group);
            }
        }
        groups.extend(self.part_groups(&part_positions)?);
        let mut uids: Vec<Option<usize>> = Vec::new();
        let mut events = Vec::new();
        for part in built {
            uids.extend(part.uids);
            events.push(StreamEvent::new(0.0, part.stream));
        }
        let mut assembled = Stream::from_events(events);
        assembled.set_kind(StreamKind::Score);
        assembled.set_metadata(Some(metadata));
        for group in groups {
            assembled.add_staff_group(group);
        }
        let position_of = |uid: usize| uids.iter().position(|held| *held == Some(uid));
        // A part's spanners in the order they start: measure by measure,
        // and in a measure staff by staff and voice by voice.
        let part_of = |track: usize| {
            self.parts
                .iter()
                .position(|part| {
                    (part.first_staff..part.first_staff + part.staves).contains(&(track / 4))
                })
                .unwrap_or(0)
        };
        let mut order: Vec<&SpannerRead> = self.spanners.iter().collect();
        order.sort_by_key(|spanner| {
            (
                part_of(spanner.start.track),
                spanner.start.measure,
                spanner.start.track,
                spanner.start.tick,
            )
        });
        for spanner in order {
            let (start, end) = (spanner.start, spanner.end);
            let made = match &spanner.kind {
                SpannerStart::Slur(placement, line_type) => {
                    let first = self
                        .uids
                        .get(&(start.measure, start.tick, start.track, start.grace))
                        .copied();
                    let last = self
                        .uids
                        .get(&(end.measure, end.tick, end.track, end.grace))
                        .copied();
                    let mut positions = Vec::new();
                    for uid in [first, last].into_iter().flatten() {
                        if let Some(position) = position_of(uid)
                            && !positions.contains(&Some(position))
                        {
                            positions.push(Some(position));
                        }
                    }
                    if positions.is_empty() {
                        continue;
                    }
                    let mut slur = Spanner::with_unplaced(SpannerKind::Slur, positions);
                    slur.set_placement(*placement);
                    slur.set_line_type(line_type.map(str::to_string));
                    slur
                }
                SpannerStart::Wedge(_) | SpannerStart::Pedal(_) => {
                    let part = part_of(start.track);
                    let mut positions: Vec<Option<usize>> = Vec::new();
                    if let Some(first) = self.line_start(part, start, end)
                        && let Some(position) = position_of(first)
                    {
                        positions.push(Some(position));
                    }
                    if let Some(last) = self.line_end(part, start, end)
                        && let Some(position) = position_of(last)
                        && !positions.contains(&Some(position))
                    {
                        positions.push(Some(position));
                    }
                    match &spanner.kind {
                        SpannerStart::Wedge(kind) => {
                            let template = Spanner::wedge(*kind, Vec::new());
                            let mut wedge = Spanner::with_unplaced(*kind, positions);
                            wedge.set_placement(template.placement());
                            wedge.set_spread(template.spread());
                            wedge
                        }
                        SpannerStart::Pedal(pedal) => {
                            let mut mark =
                                Spanner::with_unplaced(SpannerKind::PedalMark, positions);
                            mark.set_pedal(Some(*pedal));
                            mark
                        }
                        _ => continue,
                    }
                }
                SpannerStart::Volta { .. } => continue,
            };
            if made.is_empty() {
                continue;
            }
            assembled.add_spanner(made);
        }
        Ok(assembled)
    }

    /// The note a line under the staff starts on. A line belongs to a staff
    /// and runs from one moment to another, not from note to note: it
    /// starts on the note of its staff standing where it starts, or on the
    /// first note of its voice to start under it, or else on the note
    /// sounding as it starts.
    fn line_start(&self, part: usize, start: Position, end: Position) -> Option<usize> {
        let from = *self.grid.starts.get(usize::try_from(start.measure).ok()?)? + start.tick;
        let to = *self.grid.starts.get(usize::try_from(end.measure).ok()?)? + end.tick;
        let staff = start.track / 4;
        let here = |placed: &&Placed| placed.part == part && placed.track / 4 == staff;
        let exact = self
            .placed
            .iter()
            .filter(here)
            .filter(|placed| placed.start == from)
            .min_by_key(|placed| placed.track);
        if let Some(exact) = exact {
            return Some(exact.uid);
        }
        let mut tracks: Vec<usize> = vec![start.track];
        tracks.extend((staff * 4..staff * 4 + 4).filter(|track| *track != start.track));
        for track in tracks {
            let voice = || {
                self.placed
                    .iter()
                    .filter(here)
                    .filter(move |placed| placed.track == track)
            };
            let under = voice()
                .filter(|placed| placed.start >= from && placed.start < to)
                .min_by_key(|placed| placed.start);
            let sounding = || voice().find(|placed| placed.start <= from && from < placed.end);
            if let Some(found) = under.or_else(sounding) {
                return Some(found.uid);
            }
        }
        None
    }

    /// The note a line under the staff ends on: the note of its staff
    /// ending where it ends, or else the last note of its voice to start
    /// under it.
    fn line_end(&self, part: usize, start: Position, end: Position) -> Option<usize> {
        let from = *self.grid.starts.get(usize::try_from(start.measure).ok()?)? + start.tick;
        let to = *self.grid.starts.get(usize::try_from(end.measure).ok()?)? + end.tick;
        let staff = start.track / 4;
        let here = |placed: &&Placed| placed.part == part && placed.track / 4 == staff;
        let exact = self
            .placed
            .iter()
            .filter(here)
            .filter(|placed| placed.end == to)
            .min_by_key(|placed| placed.track);
        if let Some(exact) = exact {
            return Some(exact.uid);
        }
        let mut tracks: Vec<usize> = vec![start.track];
        tracks.extend((staff * 4..staff * 4 + 4).filter(|track| *track != start.track));
        for track in tracks {
            let voice = || {
                self.placed
                    .iter()
                    .filter(here)
                    .filter(move |placed| placed.track == track)
            };
            let last = voice()
                .filter(|placed| placed.start < to && placed.end > from)
                .max_by_key(|placed| placed.start);
            if let Some(found) = last {
                return Some(found.uid);
            }
        }
        None
    }

    /// The brackets joining parts, as MuseScore's part list says them: a
    /// bracket on a part's first staff joins the parts its span reaches, a
    /// brace over exactly the staves of one part being that part's own.
    fn part_groups(&self, positions: &[Vec<usize>]) -> Result<Vec<StaffGroup>> {
        let mut groups: Vec<(i64, Vec<usize>)> = Vec::new();
        // Each open group's index and the staff it ends before.
        let mut open: Vec<Option<(usize, usize)>> = vec![None; 8];
        let mut staff_count = 0usize;
        let last = self.parts.len().saturating_sub(1);
        for (index, part) in self.parts.iter().enumerate() {
            let mut starting: Vec<(i64, usize)> = Vec::new();
            for (kind, span) in &part.brackets {
                if *span == part.staves && *kind == 1 {
                    continue;
                }
                if index < last {
                    starting.push((*kind, staff_count + span));
                }
            }
            if !part.bracket_found && part.staves > 1 {
                starting.push((-1, index + part.staves));
            }
            for (kind, end) in starting {
                if let Some(slot) = open.iter().position(Option::is_none) {
                    open[slot] = Some((groups.len(), end));
                    groups.push((kind, Vec::new()));
                }
            }
            for (group, _) in open.iter().flatten() {
                groups[*group].1.extend(positions[index].iter().copied());
            }
            staff_count += part.staves;
            for slot in open.iter_mut() {
                if let Some((_, end)) = slot
                    && staff_count >= *end
                {
                    *slot = None;
                }
            }
        }
        let mut out = Vec::new();
        for (kind, parts) in groups {
            let mut group = StaffGroup::new(parts);
            group.set_symbol(Some(match kind {
                0 => "bracket",
                1 => "brace",
                2 => "square",
                3 => "line",
                _ => "none",
            }))?;
            group.set_bar_together(Some(BarTogether::Yes));
            out.push(group);
        }
        Ok(out)
    }

    /// A part, or one part for each of its staves.
    fn part(&mut self, index: usize, part: &PartRead) -> Result<Vec<BuiltPart>> {
        let measures = self.measures(index, part)?;
        if part.staves == 1 {
            let (stream, uids) = self.build(part, StreamKind::Part, None, measures);
            return Ok(vec![BuiltPart { stream, uids }]);
        }
        let mut out = Vec::new();
        let mut taken: Vec<usize> = Vec::new();
        for key in 1..=part.staves {
            let mut split = Vec::new();
            for measure in &measures {
                let mut copy = MeasureIr {
                    offset: measure.offset,
                    number: measure.number,
                    suffix: measure.suffix.clone(),
                    hidden: false,
                    left: measure.left.clone(),
                    right: measure.right.clone(),
                    ending: measure.ending.clone(),
                    items: Vec::new(),
                    voices: Vec::new(),
                };
                let mut seq = 0usize;
                let mut take = |items: &[Item], into: &mut Vec<Item>, seq: &mut usize| {
                    for item in sorted(items.to_vec()) {
                        if item.staff != 0 && item.staff != key {
                            continue;
                        }
                        let mut item = item;
                        let seq_key = item.uid.unwrap_or(usize::MAX);
                        if taken.contains(&seq_key) {
                            item.uid = None;
                        } else {
                            taken.push(seq_key);
                        }
                        item.seq = *seq;
                        *seq += 1;
                        into.push(item);
                    }
                };
                take(&measure.items, &mut copy.items, &mut seq);
                for voice in &measure.voices {
                    let mut items = Vec::new();
                    take(&voice.items, &mut items, &mut seq);
                    copy.voices.push(VoiceIr {
                        id: voice.id.clone(),
                        items,
                    });
                }
                copy.voices.retain(|voice| !voice.items.is_empty());
                if copy.voices.len() == 1 {
                    let voice = copy.voices.remove(0);
                    for mut item in sorted(voice.items) {
                        item.seq = seq;
                        seq += 1;
                        copy.items.push(item);
                    }
                }
                split.push(copy);
            }
            let id = format!("P{}-Staff{key}", index + 1);
            let (stream, uids) = self.build(part, StreamKind::PartStaff, Some(id), split);
            out.push(BuiltPart { stream, uids });
        }
        Ok(out)
    }

    fn build(
        &self,
        part: &PartRead,
        kind: StreamKind,
        id: Option<String>,
        measures: Vec<MeasureIr>,
    ) -> (Stream, Vec<Option<usize>>) {
        let mut uids = vec![None];
        let mut events = vec![StreamEvent::new(0.0, part.instrument.clone())];
        for measure in measures {
            let offset = measure.offset;
            let (stream, leaf_uids) = build_measure(measure);
            uids.extend(leaf_uids);
            events.push(StreamEvent::new(offset, stream));
        }
        let mut stream = Stream::from_events(events);
        stream.set_kind(kind);
        stream.set_id(id.or_else(|| part.instrument.best_name().map(str::to_string)));
        stream.set_name(part.name.clone());
        stream.set_abbreviation(part.abbreviation.clone());
        if kind != StreamKind::PartStaff {
            stream.set_name_hidden(part.name_hidden);
            stream.set_abbreviation_hidden(part.abbreviation_hidden);
        }
        (stream, uids)
    }

    /// Every measure of a part, all its staves together, as music21 holds
    /// a measure it has read before separating the staves.
    fn measures(&mut self, part_index: usize, part: &PartRead) -> Result<Vec<MeasureIr>> {
        let mut out: Vec<MeasureIr> = Vec::new();
        let mut number: IntegerType = 1;
        let mut irregular_count: IntegerType = 1;
        let mut last_number: IntegerType = 0;
        let mut last_suffix: Option<String> = None;
        let mut offset = 0.0;
        let mut last_meter: Option<TimeSignature> = None;
        let mut open_voltas: Vec<(Vec<u32>, i64)> = Vec::new();
        let count = self.grid.lengths.len();
        for index in 0..count {
            // What is said of a measure as a whole is said on the score's
            // first staff.
            let first_staff = &self.staves[0][index];
            // The measure's number, as MuseScore's MusicXML writes it and
            // music21 reads that.
            if first_staff.starts_section {
                number = 1;
                irregular_count = 1;
            }
            number += first_staff.number_offset as IntegerType;
            let (mut measure_number, mut suffix, hidden) =
                if first_staff.irregular && irregular_count + number == 2 {
                    (0, None, true)
                } else if first_staff.irregular {
                    let written = irregular_count;
                    irregular_count += 1;
                    (written, Some("X".to_string()), true)
                } else {
                    number += 1;
                    (number - 1, None, false)
                };
            if suffix.as_deref() == Some("X") && measure_number != last_number + 1 {
                let mut written = format!("X{measure_number}");
                if let Some(before) = &last_suffix {
                    written = format!("{before}{written}");
                }
                measure_number = last_number;
                suffix = Some(written);
            }
            if measure_number != last_number {
                last_number = measure_number;
                last_suffix = suffix.clone();
            }

            let mut ir = MeasureIr {
                number: measure_number,
                suffix,
                hidden,
                ..MeasureIr::default()
            };
            let mut seq = 0usize;
            let mut push = |items: &mut Vec<Item>,
                            offset: FloatType,
                            staff: usize,
                            uid: Option<usize>,
                            element: StreamElement| {
                items.push(Item {
                    offset,
                    seq,
                    staff,
                    uid,
                    element,
                });
                seq += 1;
            };

            // What the measure's staves hold, staff by staff, voice by voice.
            let mut voice_items: Vec<(String, Vec<Item>)> = Vec::new();
            let mut plain: Vec<Item> = Vec::new();
            let mut right: Option<Barline> = None;
            let mut right_fermata = false;
            // Signs and words saying where to go next are the first part's;
            // a sign stands ahead of the notes it stands with.
            if part_index == 0 {
                let target = usize::from(part.staves > 1);
                for mark in &first_staff.marks_at_start {
                    push(&mut plain, 0.0, target, None, mark.clone());
                }
            }
            for staff in 0..part.staves {
                let global = part.first_staff + staff;
                let staff_number = if part.staves > 1 { staff + 1 } else { 0 };
                let key_staff = staff + 1;
                let read = &self.staves[global][index];
                // A staff with no clef at its very start takes its
                // instrument's.
                if index == 0 {
                    let has_clef = read
                        .voices
                        .iter()
                        .flat_map(|voice| &voice.events)
                        .any(|event| matches!(event.what, What::Clef(_)) && event.tick.is_zero());
                    if !has_clef {
                        let clef = clef_named(&part.default_clefs[staff])?;
                        push(&mut plain, 0.0, key_staff, None, clef.into());
                    }
                    let has_key = read
                        .voices
                        .iter()
                        .flat_map(|voice| &voice.events)
                        .any(|event| matches!(event.what, What::Key(_)) && event.tick.is_zero());
                    if !has_key && staff == 0 {
                        push(&mut plain, 0.0, 0, None, KeySignature::new(0).into());
                    }
                }
                for (voice_index, voice) in read.voices.iter().enumerate() {
                    let track = global * 4 + voice_index;
                    let voice_id = if part.staves > 1 {
                        (staff * 4 + voice_index + 1).to_string()
                    } else {
                        (voice_index + 1).to_string()
                    };
                    let groups = self.beams_for(voice, index, voice_index, global)?;
                    let mut note_index = 0usize;
                    for event in &voice.events {
                        let at = quarters(event.tick);
                        match &event.what {
                            What::ChordRest(read_cr) => {
                                let beams = groups.beams.get(&note_index).cloned();
                                let tuplets = tuplets_for(
                                    &voice.tuplets,
                                    read_cr.tuplet,
                                    note_index,
                                    &groups.brackets,
                                );
                                note_index += 1;
                                // A chord may be written on the staff beside its
                                // own; a rest is not.
                                let staff_move = if read_cr.is_rest {
                                    0
                                } else {
                                    read_cr.staff_move
                                };
                                let cr_staff = if part.staves > 1 {
                                    (staff as i64 + 1 + staff_move).clamp(1, part.staves as i64)
                                        as usize
                                } else {
                                    0
                                };
                                let items =
                                    match voice_items.iter_mut().find(|(id, _)| *id == voice_id) {
                                        Some((_, items)) => items,
                                        None => {
                                            voice_items.push((voice_id.clone(), Vec::new()));
                                            &mut voice_items.last_mut().expect("just pushed").1
                                        }
                                    };
                                for (grace_index, grace) in read_cr.graces_before.iter().enumerate()
                                {
                                    let grace_beams = grace_beams(&read_cr.graces_before)
                                        .get(grace_index)
                                        .cloned()
                                        .flatten();
                                    let element = element_of(
                                        grace,
                                        Vec::new(),
                                        grace_beams,
                                        grace.beam_stem,
                                    )?;
                                    self.uids.insert(
                                        (index as i64, event.tick, track, Some(grace.grace_index)),
                                        grace.uid,
                                    );
                                    push(items, at, cr_staff, Some(grace.uid), element);
                                }
                                let beam_stem = groups
                                    .stems
                                    .get(&(note_index - 1))
                                    .copied()
                                    .unwrap_or(StemDirection::Unspecified);
                                let element = element_of(read_cr, tuplets, beams, beam_stem)?;
                                self.uids
                                    .insert((index as i64, event.tick, track, None), read_cr.uid);
                                let begins = self.grid.starts[index] + event.tick;
                                self.placed.push(Placed {
                                    part: part_index,
                                    track,
                                    start: begins,
                                    end: begins + read_cr.length,
                                    uid: read_cr.uid,
                                });
                                push(items, at, cr_staff, Some(read_cr.uid), element);
                                let end = quarters(event.tick + read_cr.length);
                                for (grace_index, grace) in read_cr.graces_after.iter().enumerate()
                                {
                                    let grace_beams = grace_beams(&read_cr.graces_after)
                                        .get(grace_index)
                                        .cloned()
                                        .flatten();
                                    let element = element_of(
                                        grace,
                                        Vec::new(),
                                        grace_beams,
                                        grace.beam_stem,
                                    )?;
                                    self.uids.insert(
                                        (index as i64, event.tick, track, Some(grace.grace_index)),
                                        grace.uid,
                                    );
                                    push(items, end, cr_staff, Some(grace.uid), element);
                                }
                            }
                            What::Clef(clef) => {
                                // A clef at the end of a measure is the next
                                // measure's.
                                if event.tick == self.grid.lengths[index] {
                                    continue;
                                }
                                push(&mut plain, at, key_staff, None, clef.clone().into());
                            }
                            What::Key(key) => {
                                if event.tick == self.grid.lengths[index] {
                                    continue;
                                }
                                if staff == 0 {
                                    push(&mut plain, at, 0, None, key.clone());
                                }
                            }
                            What::Meter(meter) => {
                                if event.tick == self.grid.lengths[index] {
                                    continue;
                                }
                                if staff == 0 {
                                    push(&mut plain, at, 0, None, meter.clone().into());
                                }
                            }
                            What::Element(element) => {
                                let target = if part.staves > 1 { staff + 1 } else { 0 };
                                let _ = staff_number;
                                push(&mut plain, at, target, None, element.clone());
                            }
                            What::Barline(barline, fermata) => {
                                if event.tick == self.grid.lengths[index] && staff == 0 {
                                    right = Some(barline.clone());
                                    right_fermata = *fermata;
                                }
                            }
                        }
                    }
                }
            }
            if part_index == 0 {
                let target = usize::from(part.staves > 1);
                let end = quarters(self.grid.lengths[index]);
                for mark in &first_staff.marks_at_end {
                    push(&mut plain, end, target, None, mark.clone());
                }
            }
            if part.staves == 1 {
                for item in &mut plain {
                    item.staff = 0;
                }
            }

            // Repeats and endings.
            if first_staff.start_repeat {
                ir.left = Some(Barline::repeat(RepeatDirection::Start, None));
            }
            if let Some(times) = first_staff.end_repeat {
                let times = (times > 2).then_some(times);
                ir.right = Some(Barline::repeat(RepeatDirection::End, times));
            } else {
                // An ordinary barline written out is what a measure ends
                // with anyway, and stands in the way of any other.
                ir.right = match right {
                    Some(barline)
                        if barline.bar_type() == BarlineType::Regular && !right_fermata =>
                    {
                        None
                    }
                    Some(barline) => Some(barline),
                    None => self.drawn_barline(index),
                };
            }
            for spanner in self.spanners {
                if let SpannerStart::Volta { numbers } = &spanner.kind
                    && spanner.start.measure == index as i64
                    && spanner.start.track / 4 >= part.first_staff
                    && spanner.start.track / 4 < part.first_staff + part.staves
                {
                    let end = self.measure_at(spanner.end);
                    open_voltas.push((numbers.clone(), end));
                }
            }
            if let Some(position) = open_voltas.iter().position(|(_, end)| index as i64 <= *end) {
                let (numbers, end) = open_voltas[position].clone();
                let starts = self.volta_start(&numbers, index, part);
                let stops = index as i64 == end;
                ir.ending = Some(Ending::new(numbers.clone(), starts, stops));
                // A bracket starts and ends on a barline, an ordinary one
                // where the measure has no other.
                if starts && ir.left.is_none() {
                    ir.left = Some(Barline::default());
                }
                if stops {
                    if ir.right.is_none() {
                        ir.right = Some(Barline::default());
                    }
                    open_voltas.remove(position);
                }
            }

            let use_voices = voice_items.len() > 1;
            if use_voices {
                voice_items.sort_by(|left, right| left.0.cmp(&right.0));
                ir.voices = voice_items
                    .into_iter()
                    .map(|(id, items)| VoiceIr { id, items })
                    .collect();
                ir.items = plain;
            } else {
                ir.items = plain;
                for (_, items) in voice_items {
                    ir.items.extend(items);
                }
            }

            // Where the measure stands, as music21 works it out.
            let own_meter = ir.items.iter().find_map(|item| match &item.element {
                StreamElement::TimeSignature(meter) => Some(meter.clone()),
                _ => None,
            });
            if let Some(meter) = own_meter {
                last_meter = Some(meter);
            } else if last_meter.is_none() {
                last_meter = Some(TimeSignature::new(4, 4)?);
            }
            ir.offset = offset;
            offset = op_frac(offset + quarters(self.grid.lengths[index]));
            let _ = part_index;
            out.push(ir);
        }
        Ok(out)
    }

    /// The barline a measure ends with where it writes none of its own: a
    /// final one at the end of the score, which MuseScore draws without
    /// being told. The double barline it draws before a courtesy key
    /// signature depends on where a system ends, which is the page's to
    /// say, and is not one of these.
    fn drawn_barline(&self, index: usize) -> Option<Barline> {
        (index + 1 == self.staves[0].len()).then(|| Barline::new(BarlineType::Final))
    }

    /// Whether a volta starting at this measure is the first measure of it.
    fn volta_start(&self, _numbers: &[u32], index: usize, part: &PartRead) -> bool {
        self.spanners.iter().any(|spanner| {
            matches!(spanner.kind, SpannerStart::Volta { .. })
                && spanner.start.measure == index as i64
                && spanner.start.track / 4 >= part.first_staff
                && spanner.start.track / 4 < part.first_staff + part.staves
        })
    }

    /// The last measure a spanner ending here covers.
    fn measure_at(&self, end: Position) -> i64 {
        let Some(start) = self.grid.starts.get(end.measure.max(0) as usize) else {
            return self.grid.starts.len() as i64 - 1;
        };
        let at = *start + end.tick;
        let mut last = 0;
        for (index, begins) in self.grid.starts.iter().enumerate() {
            if *begins < at {
                last = index as i64;
            }
        }
        last
    }

    /// The beams of every note of a voice.
    fn beams_for(
        &self,
        voice: &VoiceRead,
        index: usize,
        voice_index: usize,
        _staff: usize,
    ) -> Result<Beamed> {
        let (numerator, denominator) = self.grid.meters[index];
        let groups_table = beams::default_groups(numerator, denominator);
        let bar = Frac::new(numerator, denominator);
        let anacrusis = if index == 0 && self.grid.lengths[0] < bar {
            ticks(bar - self.grid.lengths[0])
        } else {
            0
        };
        let context = beams::Context {
            groups: &groups_table,
            denominator,
            anacrusis,
            voice: voice_index,
        };
        let mut members = Vec::new();
        let mut written_stems = Vec::new();
        for event in &voice.events {
            if let What::ChordRest(read) = &event.what {
                members.push(member_of(read));
                written_stems.push((read.beam_stem, read.staff_move != 0));
            }
        }
        let groups = beams::beam_groups(&context, &members);
        let mut beams = HashMap::new();
        let mut stems = HashMap::new();
        for group in &groups {
            let held: Vec<&Member> = group.iter().map(|index| &members[*index]).collect();
            for (at, made) in group.iter().zip(beams::beams_of_group(&held)) {
                if !members[*at].is_rest {
                    beams.insert(*at, made);
                }
            }
            // A beam written out with a direction turns every stem under
            // it, unless it runs between two staves, where the stems of
            // each staff go their own way.
            let direction = group
                .iter()
                .map(|at| written_stems[*at].0)
                .find(|stem| *stem != StemDirection::Unspecified);
            let crosses = group.iter().any(|at| written_stems[*at].1);
            if let Some(direction) = direction
                && !crosses
            {
                for at in group {
                    stems.insert(*at, direction);
                }
            }
        }
        // Whether each tuplet that leaves its bracket to MuseScore takes
        // one.
        let under: Vec<Vec<usize>> = (0..voice.tuplets.len())
            .map(|index| {
                let mut notes = Vec::new();
                collect_notes(&voice.tuplets, index, &mut notes);
                notes
            })
            .collect();
        let tuplet_ticks =
            |index: usize| -> i64 { under[index].iter().map(|note| members[*note].ticks).sum() };
        let brackets = voice
            .tuplets
            .iter()
            .enumerate()
            .map(|(index, tuplet)| {
                let own: Vec<usize> = tuplet
                    .elements
                    .iter()
                    .filter_map(|held| match held {
                        Held::Note(note) => Some(*note),
                        Held::Tuplet(_) => None,
                    })
                    .collect();
                beams::tuplet_has_bracket(
                    &context,
                    &members,
                    &groups,
                    &beams::TupletShape {
                        notes: &under[index],
                        own: &own,
                        nests: own.len() != tuplet.elements.len(),
                    },
                    &tuplet_ticks,
                )
            })
            .collect();
        Ok(Beamed {
            beams,
            stems,
            brackets,
        })
    }
}

struct Beamed {
    beams: HashMap<usize, Beams>,
    /// Whether each tuplet of the voice is drawn with a bracket, where it
    /// leaves that to MuseScore.
    brackets: Vec<bool>,
    /// The stem direction each beamed note takes from its beam.
    stems: HashMap<usize, StemDirection>,
}

fn member_of(read: &ChordRest) -> Member {
    let dots = read.value.map_or(0, |(_, dots)| dots);
    Member {
        tick: ticks(read.tick),
        ticks: ticks(read.length),
        value: (if read.measure_rest { 14 } else { read.order }, dots),
        is_rest: read.is_rest,
        mode: read.mode,
        tuplet: read.tuplet,
    }
}

/// The beams of a run of grace notes.
fn grace_beams(graces: &[ChordRest]) -> Vec<Option<Beams>> {
    let members: Vec<Member> = graces.iter().map(member_of).collect();
    let mut out = vec![None; graces.len()];
    for group in beams::grace_groups(&members) {
        let held: Vec<&Member> = group.iter().map(|index| &members[*index]).collect();
        for (at, made) in group.iter().zip(beams::beams_of_group(&held)) {
            out[*at] = Some(made);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(voice: &str) -> String {
        format!(
            r#"<museScore version="4.70"><Score>
            <Part><Staff/><trackName>Flute</trackName>
            <Instrument><longName>Flute</longName><trackName>Flute</trackName>
            <Channel><program value="73"/></Channel></Instrument></Part>
            <Staff id="1"><Measure><voice>
            <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>{voice}</voice></Measure></Staff>
            </Score></museScore>"#
        )
    }

    fn chord(value: &str, pitch: i64, tpc: i64) -> String {
        format!(
            "<Chord><durationType>{value}</durationType><Note><pitch>{pitch}</pitch>\
             <tpc>{tpc}</tpc></Note></Chord>"
        )
    }

    #[test]
    fn a_tonal_pitch_class_spells_its_note() {
        assert_eq!(spelling(14).unwrap(), ('C', 0));
        assert_eq!(spelling(21).unwrap(), ('C', 1));
        assert_eq!(spelling(-1).unwrap(), ('F', -2));
        // The letter and the octave; the accidental is the note's to add.
        assert_eq!(pitch_of(60, 26).unwrap().name_with_octave(), "B3");
        assert_eq!(pitch_of(59, 7).unwrap().name_with_octave(), "C4");
    }

    #[test]
    fn notes_follow_one_another_and_eighths_are_beamed() {
        let body = [
            chord("eighth", 60, 14),
            chord("eighth", 62, 16),
            chord("quarter", 64, 18),
            chord("half", 65, 13),
        ]
        .concat();
        let score = from_mscx(&document(&body)).unwrap();
        let notes = score.parts()[0].notes();
        let offsets: Vec<FloatType> = notes.iter().map(|(offset, _)| *offset).collect();
        assert_eq!(offsets, [0.0, 0.5, 1.0, 2.0]);
        let beams: Vec<usize> = notes
            .iter()
            .map(|(_, element)| match element {
                StreamElement::Note(note) => note.beams().len(),
                _ => 0,
            })
            .collect();
        assert_eq!(beams, [1, 1, 0, 0]);
    }

    #[test]
    fn a_triplet_shortens_its_notes() {
        let body = format!(
            "<Tuplet><normalNotes>2</normalNotes><actualNotes>3</actualNotes>\
             <baseNote>eighth</baseNote></Tuplet>{}{}{}<endTuplet/>\
             <Chord><durationType>quarter</durationType><dots>1</dots><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>\
             <Rest><durationType>quarter</durationType></Rest>",
            chord("eighth", 60, 14),
            chord("eighth", 62, 16),
            chord("eighth", 64, 18),
        );
        let score = from_mscx(&document(&body)).unwrap();
        let lengths: Vec<FloatType> = score.parts()[0].measures()[0]
            .events()
            .iter()
            .map(|event| event.element().quarter_length())
            .filter(|length| *length > 0.0)
            .collect();
        assert_eq!(lengths.len(), 5);
        assert!((lengths[0] - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(lengths[3], 1.5);
    }

    /// The notes of the first measure of the first part.
    fn notes_of(score: &Stream) -> Vec<Note> {
        score.parts()[0].measures()[0]
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Note(note) => Some(note.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_grace_note_is_slashed_only_where_it_is_an_acciaccatura() {
        let grace = |kind: &str| {
            format!(
                "<Chord><durationType>eighth</durationType><{kind}/>\
                 <Note><pitch>62</pitch><tpc>16</tpc></Note></Chord>"
            )
        };
        let body = [
            grace("acciaccatura"),
            chord("half", 60, 14),
            grace("appoggiatura"),
            chord("half", 60, 14),
        ]
        .concat();
        let notes = notes_of(&from_mscx(&document(&body)).unwrap());
        let slashed: Vec<Option<bool>> = notes
            .iter()
            .map(|note| {
                note.duration()
                    .and_then(Duration::grace)
                    .map(|grace| grace.slash())
            })
            .collect();
        assert_eq!(slashed, [Some(true), None, Some(false), None]);
    }

    #[test]
    fn a_stem_points_where_the_file_fixes_it_and_nowhere_otherwise() {
        let body = format!(
            "<Chord><durationType>quarter</durationType><StemDirection>up</StemDirection>\
             <Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>{}\
             <Chord><durationType>half</durationType><noStem>1</noStem>\
             <Note><pitch>72</pitch><tpc>14</tpc></Note></Chord>",
            chord("quarter", 72, 14),
        );
        let stems: Vec<StemDirection> = notes_of(&from_mscx(&document(&body)).unwrap())
            .iter()
            .map(Note::stem_direction)
            .collect();
        assert_eq!(
            stems,
            [
                StemDirection::Up,
                StemDirection::Unspecified,
                StemDirection::NoStem
            ]
        );
    }

    #[test]
    fn a_notehead_a_colour_and_a_fingering_are_read_off_the_note() {
        let body = "<Chord><durationType>whole</durationType><Note><pitch>60</pitch>\
                    <tpc>14</tpc><head>cross</head>\
                    <color r=\"255\" g=\"38\" b=\"0\" a=\"255\"/>\
                    <Fingering><text>3</text></Fingering></Note></Chord>";
        let notes = notes_of(&from_mscx(&document(body)).unwrap());
        assert_eq!(notes[0].notehead(), Notehead::from_name("x").unwrap());
        assert_eq!(notes[0].color(), Some("#FF2600"));
        assert_eq!(notes[0].articulations().len(), 1);
        assert!(notes[0].articulations()[0].is_a("Fingering"));
    }

    #[test]
    fn a_tuplet_takes_a_bracket_unless_a_beam_sets_it_apart() {
        let triplet = |value: &str| {
            format!(
                "<Tuplet><normalNotes>2</normalNotes><actualNotes>3</actualNotes>                 <baseNote>{value}</baseNote></Tuplet>{}{}{}<endTuplet/>",
                chord(value, 60, 14),
                chord(value, 62, 16),
                chord(value, 64, 18),
            )
        };
        let body = [
            triplet("eighth"),
            triplet("quarter"),
            chord("quarter", 60, 14),
        ]
        .concat();
        let brackets: Vec<TupletBracket> = notes_of(&from_mscx(&document(&body)).unwrap())
            .iter()
            .filter_map(|note| {
                note.duration()
                    .and_then(|length| length.tuplets().first().cloned())
            })
            .map(|tuplet| tuplet.bracket())
            .collect();
        // Three beamed eighths, then three quarters no beam can join.
        assert_eq!(brackets[..3], [TupletBracket::None; 3]);
        assert_eq!(brackets[3..], [TupletBracket::Bracket; 3]);
    }

    /// A score of one whole note with something written in its measure.
    fn marked(version: &str, mark: &str) -> String {
        format!(
            r#"<museScore version="{version}"><Score>
            <Part><Staff/><trackName>Flute</trackName>
            <Instrument><longName>Flute</longName><trackName>Flute</trackName>
            <Channel><program value="73"/></Channel></Instrument></Part>
            <Staff id="1"><Measure>{mark}<voice>
            <TimeSig><sigN>4</sigN><sigD>4</sigD></TimeSig>{}</voice></Measure></Staff>
            </Score></museScore>"#,
            chord("whole", 60, 14)
        )
    }

    /// The signs and jumps of the first measure, each with its offset.
    fn repeat_marks(score: &Stream) -> Vec<(FloatType, RepeatExpressionKind)> {
        score.parts()[0].measures()[0]
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::RepeatExpression(mark) => Some((event.offset(), mark.kind())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_marker_is_of_the_kind_the_file_says_in_the_way_its_format_says_it() {
        let segno = "<Marker><text><sym>segno</sym></text><label>segno</label></Marker>";
        let typed = "<Marker><text><sym>segno</sym></text><label>segno</label>\
                     <markerType>segno</markerType></Marker>";
        // Before format 4.6 the label says what a marker is.
        let old = from_mscx(&marked("4.40", segno)).unwrap();
        assert_eq!(repeat_marks(&old), [(0.0, RepeatExpressionKind::Segno)]);
        // From 4.6 on its kind is said apart, and one that says none is the
        // end of the piece, as MuseScore reads it.
        let new = from_mscx(&marked("4.70", typed)).unwrap();
        assert_eq!(repeat_marks(&new), [(0.0, RepeatExpressionKind::Segno)]);
        let untyped = from_mscx(&marked("4.70", segno)).unwrap();
        assert!(
            !repeat_marks(&untyped)
                .iter()
                .any(|(_, kind)| *kind == RepeatExpressionKind::Segno)
        );
        // A jump stands at the end of its measure.
        let jump = "<Jump><text>D.C.</text><jumpTo>start</jumpTo><playUntil>end</playUntil>\
                    <continueAt></continueAt></Jump>";
        let jumped = from_mscx(&marked("4.70", jump)).unwrap();
        assert_eq!(repeat_marks(&jumped), [(4.0, RepeatExpressionKind::DaCapo)]);
    }

    #[test]
    fn what_the_model_has_no_value_for_is_refused_by_name() {
        let tremolo = "<Chord><durationType>whole</durationType><Tremolo><subtype>r8</subtype>\
                       </Tremolo><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord>";
        let error = from_mscx(&document(tremolo)).unwrap_err().to_string();
        assert!(
            error.contains("Tremolo") && error.contains("cannot be read yet"),
            "{error}"
        );
        let trill = "<Spanner type=\"Trill\"><Trill/><next><location><measures>1</measures>\
                     </location></next></Spanner>";
        let error = from_mscx(&document(&[trill, &chord("whole", 60, 14)].concat()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("Trill"), "{error}");
    }

    #[test]
    fn what_is_not_a_musescore_file_is_refused() {
        assert!(from_mscx("<score-partwise/>").is_err());
        assert!(from_mscx("not xml").is_err());
        assert!(from_mscx(r#"<museScore version="2.06"><Score/></museScore>"#).is_err());
    }
}
