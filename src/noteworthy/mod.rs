//! NoteWorthy Composer's text format, `.nwctxt`: music21's
//! `noteworthy.translate`.
//!
//! A file is a line per object, each `|Command|Name:value|Name:value`, and
//! [`from_noteworthy`] reads it into a score of a part per staff. The binary
//! `.nwc` format is read by [`from_nwc`], which writes it out as those lines
//! first, as music21 does.

mod binary;

pub use binary::{
    ALTERATION_TEXTS, BAR_STYLES, CLEF_NAMES, DURATION_VALUES, FLAT_MASK, MIDI_INSTRUMENTS,
    OCTAVE_SHIFT_NAMES, SHARP_MASK, from_nwc, nwc_lines,
};

use crate::bar::{Barline, BarlineType, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, DurationType, Tuplet};
use crate::dynamics::Dynamic;
use crate::error::{Error, Position, Result};
use crate::expressions::TextExpression;
use crate::instrument::Instrument;
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::metadata::{Metadata, MetadataValue};
use crate::notation::{Placement, Tie, TieType};
use crate::note::Note;
use crate::percussion::Unpitched;
use crate::pitch::Pitch;
use crate::pitch::accidental::Accidental;
use crate::repeat::{RepeatExpression, RepeatExpressionKind};
use crate::rest::Rest;
use crate::spanner::Spanner;
use crate::stream::{Stream, StreamElement, StreamKind};
use crate::tempo::MetronomeMark;

fn error(message: impl Into<String>) -> Error {
    Error::noteworthy(message.into())
}

/// Reads the text of a NoteWorthy Composer `.nwctxt` file into a score, as
/// music21's `converter.parse` reads one.
///
/// Each staff is a part, every one standing at the start of the score. Its
/// notes, chords and rests are spelled from their places on the staff under
/// the clef in force, with the key signature's accidentals and any written
/// earlier in the measure; slurs, ties, triplets, grace notes and the
/// staff's first verse of lyrics are read, as are barlines and repeats,
/// endings, clefs, keys, meters, tempos, dynamics, words, *Coda*, *Segno*,
/// *Fine* and the jumps to them, each staff's instrument by its MIDI program,
/// and the song's title. A percussion clef's notes are unpitched strokes.
///
/// music21's own habits are kept: its first two measures are both numbered
/// nought, a staff's name keeps the quotation marks the file writes round
/// it, words are placed above the staff where the file puts them below, and
/// a chord with a rest beside it puts what the measure holds so far into a
/// voice. What music21 passes over -- fonts, page setup, crescendo and
/// diminuendo marks, performance styles, a second verse -- is passed over.
///
/// ```
/// use music21::noteworthy::from_noteworthy;
///
/// let score = from_noteworthy(
///     "!NoteWorthyComposer(2.0)\n|AddStaff|\n|Clef|Type:Bass\n|TimeSig|Signature:4/4\n|Note|Dur:Whole|Pos:1\n",
/// )?;
/// let pitches = score.pitches();
/// assert_eq!(pitches[0].name_with_octave(), "E3");
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A file with no staff, a line music21 cannot read -- an attribute with no
/// value, a clef, barline style, flow mark or note length it does not know,
/// an octave shift on an alto or tenor clef -- or a number that is not one.
pub fn from_noteworthy(text: &str) -> Result<Stream> {
    from_lines(text.lines(), true)
}

/// [`from_noteworthy`] over lines already split, as music21's `parseList`
/// takes them. What goes wrong on a line says which where the lines are
/// `numbered`, the lines of a file rather than ones made from a binary one.
fn from_lines<'a>(lines: impl IntoIterator<Item = &'a str>, numbered: bool) -> Result<Stream> {
    let mut reader = Reader::new();
    for (index, line) in lines.into_iter().enumerate() {
        let here = |error: Error| {
            if numbered {
                return error.placed(|| Position::line(index + 1));
            }
            error
        };
        let line = line.trim_end();
        if !line.starts_with('|') {
            continue;
        }
        let sections: Vec<&str> = line.split('|').collect();
        let command = sections.get(1).copied().unwrap_or("");
        let mut attributes = Attributes::default();
        for attribute in sections.iter().skip(2) {
            match attribute.split_once(':') {
                Some((name, value)) => attributes.add(
                    name,
                    value,
                    command == "Chord" && matches!(name, "Dur" | "Pos"),
                ),
                None if attribute.trim().is_empty() => {}
                None => {
                    return Err(here(error(format!(
                        "Cannot unpack value from {attribute} in {line}"
                    ))));
                }
            }
        }
        reader.command(command, &attributes).map_err(here)?;
    }
    reader.finish()
}

/// A line's attributes, in the order written; `Dur` and `Pos` of a chord
/// keep every value given.
#[derive(Default)]
struct Attributes {
    values: Vec<(String, Vec<String>)>,
}

impl Attributes {
    fn add(&mut self, name: &str, value: &str, listed: bool) {
        match self.values.iter_mut().find(|(known, _)| known == name) {
            Some((_, values)) if listed => values.push(value.to_string()),
            Some((_, values)) => *values = vec![value.to_string()],
            None => self
                .values
                .push((name.to_string(), vec![value.to_string()])),
        }
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(known, _)| known == name)
            .and_then(|(_, values)| values.last())
            .map(String::as_str)
    }

    fn list(&self, name: &str) -> Vec<String> {
        self.values
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, values)| values.clone())
            .unwrap_or_default()
    }

    fn required(&self, name: &str) -> Result<&str> {
        self.get(name)
            .ok_or_else(|| error(format!("a line with no {name}")))
    }
}

/// The clef notes are read under, as music21's reader names it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClefState {
    Treble,
    TrebleDown,
    TrebleUp,
    Bass,
    BassDown,
    BassUp,
    Alto,
    Tenor,
    Percussion,
}

impl ClefState {
    /// The clef music21's reader keeps as `currentClef`: `TREBLE`,
    /// `TREBLE8dw`, `TREBLE8up`, `BASS`, `BASS8dw`, `BASS8up`, `ALTO`,
    /// `TENOR` or `PERCUSSION`; anything else is read as treble.
    fn named(name: &str) -> Self {
        match name {
            "TREBLE8dw" => Self::TrebleDown,
            "TREBLE8up" => Self::TrebleUp,
            "BASS" => Self::Bass,
            "PERCUSSION" => Self::Percussion,
            "BASS8dw" => Self::BassDown,
            "BASS8up" => Self::BassUp,
            "ALTO" => Self::Alto,
            "TENOR" => Self::Tenor,
            _ => Self::Treble,
        }
    }

    /// The step and octave of a staff position under this clef.
    fn step_and_octave(self, position: IntegerType) -> (char, IntegerType) {
        let (octave, lowest, steps) = self.table();
        let from_lowest = position - lowest;
        let step = steps[usize::try_from(from_lowest.rem_euclid(7)).unwrap_or(0)];
        (step, octave + from_lowest.div_euclid(7))
    }

    /// music21's `getStepAndOctaveFromPosition` tables: the octave of the
    /// lowest of seven positions, that position, and the steps from it.
    fn table(self) -> (IntegerType, IntegerType, [char; 7]) {
        const TREBLE: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
        match self {
            Self::TrebleDown => (4, 1, TREBLE),
            Self::TrebleUp => (6, 1, TREBLE),
            Self::Bass | Self::Percussion => (3, -1, TREBLE),
            Self::BassDown => (2, -1, TREBLE),
            Self::BassUp => (4, -1, TREBLE),
            Self::Alto => (4, 0, TREBLE),
            Self::Tenor => (3, -5, TREBLE),
            Self::Treble => (5, 1, TREBLE),
        }
    }
}

/// The positions of a staff under a clef, as music21's
/// `noteworthy.dictionaries` lays them out: the octave of the lowest of
/// seven positions, that position, and the steps from it. The clef is
/// named as music21's reader keeps it (`TREBLE`, `BASS8dw`, `ALTO`, ...).
#[must_use]
pub fn staff_positions(clef: &str) -> (IntegerType, IntegerType, [char; 7]) {
    ClefState::named(clef).table()
}

/// The step and octave of a staff position under a clef: music21's
/// `getStepAndOctaveFromPosition`. Position 1 under the treble clef is
/// C5, and each position up or down is a step.
#[must_use]
pub fn step_and_octave(clef: &str, position: IntegerType) -> (char, IntegerType) {
    ClefState::named(clef).step_and_octave(position)
}

/// Something standing in a measure or a voice while the file is read.
struct Item {
    offset: FloatType,
    content: Content,
    /// The order it was put where it stands.
    inserted: usize,
}

enum Content {
    /// An element, with its number among the notes, chords and rests read
    /// where it is one.
    Element(Box<StreamElement>, Option<usize>),
    Voice(VoiceBuilt),
}

struct VoiceBuilt {
    id: usize,
    items: Vec<Item>,
}

fn is_general_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Rest(_)
            | StreamElement::Unpitched(_)
    )
}

/// The end of the last thing in a list: music21's `highestTime`.
fn highest_time(items: &[Item]) -> FloatType {
    items
        .iter()
        .map(|item| {
            item.offset
                + match &item.content {
                    Content::Element(element, _) => element.quarter_length(),
                    Content::Voice(voice) => highest_time(&voice.items),
                }
        })
        .fold(0.0, FloatType::max)
}

struct MeasureBuilt {
    /// Which measure of the file this is, for the slurs and ties that
    /// remember where they were left open.
    id: usize,
    number: IntegerType,
    left: Option<Barline>,
    right: Option<Barline>,
    items: Vec<Item>,
    inserted: usize,
}

impl MeasureBuilt {
    fn new(number: IntegerType, id: usize) -> Self {
        Self {
            id,
            number,
            left: None,
            right: None,
            items: Vec::new(),
            inserted: 0,
        }
    }

    fn insert(&mut self, offset: FloatType, content: Content) {
        self.items.push(Item {
            offset,
            content,
            inserted: self.inserted,
        });
        self.inserted += 1;
    }

    /// music21's truth of a stream: whether it holds anything, barlines
    /// included.
    fn holds_anything(&self) -> bool {
        !self.items.is_empty() || self.left.is_some() || self.right.is_some()
    }

    /// The places of the voices, in music21's order.
    fn voices(&self) -> Vec<usize> {
        sorted_places(&self.items)
            .into_iter()
            .filter(|place| matches!(self.items[*place].content, Content::Voice(_)))
            .collect()
    }

    /// music21's `appendSimultaneous`: elements starting together at
    /// `offset`. Where there are several, or the measure has voices, each
    /// goes in a voice that is free there, the longest of them first, or a
    /// new one; the notes the measure holds become its first voice where
    /// it has none.
    fn put_together(&mut self, offset: FloatType, elements: Vec<(StreamElement, usize)>) {
        let mut voices = self.voices();
        if voices.is_empty() && elements.len() > 1 {
            let notes: Vec<usize> = sorted_places(&self.items)
                .into_iter()
                .filter(|place| {
                    matches!(&self.items[*place].content,
                        Content::Element(element, _) if is_general_note(element))
                })
                .collect();
            let mut voice = VoiceBuilt {
                id: 0,
                items: Vec::new(),
            };
            for place in &notes {
                let item = &self.items[*place];
                if let Content::Element(element, serial) = &item.content {
                    let inserted = voice.items.len();
                    voice.items.push(Item {
                        offset: item.offset,
                        content: Content::Element(element.clone(), *serial),
                        inserted,
                    });
                }
            }
            let mut index = 0;
            self.items.retain(|_| {
                let keep = !notes.contains(&index);
                index += 1;
                keep
            });
            self.insert(0.0, Content::Voice(voice));
            voices = self.voices();
        }
        let mut free: Vec<usize> = voices
            .iter()
            .copied()
            .filter(|place| voice_length(&self.items[*place]) <= offset)
            .collect();
        free.sort_by(|left, right| {
            voice_length(&self.items[*right]).total_cmp(&voice_length(&self.items[*left]))
        });
        let mut free: std::collections::VecDeque<usize> = free.into();
        let mut count = voices.len();
        for (element, serial) in elements {
            if count == 0 {
                self.insert(offset, Content::Element(Box::new(element), Some(serial)));
                continue;
            }
            let place = free.pop_front().unwrap_or_else(|| {
                self.insert(
                    0.0,
                    Content::Voice(VoiceBuilt {
                        id: count,
                        items: Vec::new(),
                    }),
                );
                count += 1;
                self.items.len() - 1
            });
            if let Content::Voice(voice) = &mut self.items[place].content {
                let inserted = voice.items.len();
                voice.items.push(Item {
                    offset,
                    content: Content::Element(Box::new(element), Some(serial)),
                    inserted,
                });
            }
        }
    }
}

fn voice_length(item: &Item) -> FloatType {
    match &item.content {
        Content::Voice(voice) => highest_time(&voice.items),
        Content::Element(element, _) => element.quarter_length(),
    }
}

/// The places of a list's items in music21's order: offset, then class,
/// grace notes ahead of the rest, then the order they were put there.
fn sorted_places(items: &[Item]) -> Vec<usize> {
    let mut places: Vec<usize> = (0..items.len()).collect();
    places.sort_by(|left, right| {
        let key = |item: &Item| {
            let (order, grace) = match &item.content {
                Content::Element(element, _) => (
                    element.class_sort_order(),
                    element.duration().is_some_and(Duration::is_grace),
                ),
                Content::Voice(_) => (5, false),
            };
            (item.offset, order, !grace, item.inserted)
        };
        let (left, right) = (key(&items[*left]), key(&items[*right]));
        left.0
            .total_cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
            .then(left.3.cmp(&right.3))
    });
    places
}

/// What a part holds while it is read: its measures and instruments in the
/// order they were appended, each where the part ended at the time.
enum PartItem {
    Measure(MeasureBuilt),
    Instrument(Box<Instrument>),
}

struct PartBuilt {
    name: Option<String>,
    items: Vec<(FloatType, PartItem)>,
    /// Slurs, by the numbers of the notes they join.
    slurs: Vec<(usize, usize)>,
}

impl PartBuilt {
    fn measures(&self) -> usize {
        self.items
            .iter()
            .filter(|(_, item)| matches!(item, PartItem::Measure(_)))
            .count()
    }

    fn end(&self) -> FloatType {
        self.items
            .iter()
            .map(|(offset, item)| {
                offset
                    + match item {
                        PartItem::Measure(measure) => highest_time(&measure.items),
                        PartItem::Instrument(_) => 0.0,
                    }
            })
            .fold(0.0, FloatType::max)
    }

    fn append(&mut self, item: PartItem) {
        let at = self.end();
        self.items.push((at, item));
    }
}

struct Reader {
    parts: Vec<PartBuilt>,
    part: Option<PartBuilt>,
    measure: MeasureBuilt,
    measure_number: IntegerType,
    ending: u32,
    /// The measures an ending spans so far, the one being read included.
    /// music21 keeps the list from one staff to the next, so an ending left
    /// open at the end of a staff goes on into the next.
    repeated: Vec<Place>,
    /// Endings, as the measures they span and the ending's number.
    endings: Vec<(Vec<Place>, u32)>,
    metadata: Option<Metadata>,
    clef: ClefState,
    key: KeySignature,
    /// Where the next object goes in the measure, once the measure has
    /// been placed in: music21's `measurePosition`.
    position: Option<FloatType>,
    /// How many measures have been begun, which numbers each.
    measures_begun: usize,
    /// Slurs begun and not yet joined to the note they run to.
    open_slurs: Vec<OpenSlur>,
    /// Ties begun, by the staff step they were begun on, with the measure
    /// they were begun in (none where carried over a barline) and where
    /// the tied note ends.
    tied_steps: Vec<((char, IntegerType), Option<usize>, FloatType)>,
    lyric_position: usize,
    lyrics: Vec<String>,
    /// Accidentals written so far in the measure, by the step and octave
    /// they were written on.
    accidentals: Vec<(String, &'static str)>,
    /// How many notes, chords and rests have been read.
    serials: usize,
}

impl Reader {
    fn new() -> Self {
        Self {
            parts: Vec::new(),
            part: None,
            measure: MeasureBuilt::new(0, 0),
            measure_number: 0,
            ending: 1,
            repeated: Vec::new(),
            endings: Vec::new(),
            metadata: None,
            clef: ClefState::Treble,
            key: KeySignature::new(0),
            position: None,
            measures_begun: 1,
            open_slurs: Vec::new(),
            tied_steps: Vec::new(),
            lyric_position: 0,
            lyrics: Vec::new(),
            accidentals: Vec::new(),
            serials: 0,
        }
    }

    fn part(&mut self) -> Result<&mut PartBuilt> {
        self.part
            .as_mut()
            .ok_or_else(|| error("an object before the first staff"))
    }

    fn command(&mut self, command: &str, attributes: &Attributes) -> Result<()> {
        match command {
            "AddStaff" => self.add_staff(attributes),
            "Bar" => self.bar(attributes)?,
            "Chord" => {
                self.chord(attributes)?;
                self.lyric_position += 1;
            }
            "Clef" => self.clef(attributes)?,
            "Dynamic" => {
                if let Some(style) = attributes.get("Style") {
                    self.at_position(StreamElement::Dynamic(Dynamic::new(style)));
                }
            }
            "Ending" => self.ending(attributes)?,
            "Flow" => self.flow(attributes)?,
            "Key" => self.key(attributes)?,
            "Lyric1" => self.lyrics = lyrics(attributes.required("Text")?),
            "Note" => {
                self.note(attributes)?;
                self.lyric_position += 1;
            }
            "RestChord" => {
                self.rest_chord(attributes)?;
                self.lyric_position += 1;
            }
            "Rest" => {
                let written = attributes.required("Dur")?;
                let mut rest = Rest::new(Duration::quarter());
                rest.set_duration(duration_of(written)?);
                let serial = self.serial();
                self.simultaneous(vec![(StreamElement::Rest(rest), serial, slurred(written))]);
            }
            "SongInfo" => {
                let mut metadata = Metadata::new();
                if let Some(title) = attributes.get("Title") {
                    metadata.set("title", vec![MetadataValue::new(title)]);
                }
                self.metadata = Some(metadata);
            }
            "StaffInstrument" => self.staff_instrument(attributes)?,
            "Tempo" => {
                let tempo: IntegerType = number(attributes.required("Tempo")?)?;
                self.measure.insert(
                    0.0,
                    Content::Element(
                        Box::new(StreamElement::MetronomeMark(MetronomeMark::new(
                            FloatType::from(tempo),
                        ))),
                        None,
                    ),
                );
            }
            "Text" => {
                let mut words = TextExpression::new(attributes.required("Text")?);
                let position: IntegerType = number(attributes.required("Pos")?)?;
                words.set_placement(Some(if position < 0 {
                    Placement::Above
                } else {
                    Placement::Below
                }));
                self.at_position(StreamElement::TextExpression(words));
            }
            "TimeSig" => {
                let signature = match attributes.required("Signature")? {
                    "AllaBreve" => "2/2",
                    "Common" => "4/4",
                    other => other,
                };
                let meter = crate::meter::TimeSignature::from_ratio_string(signature)?;
                self.at_position(StreamElement::TimeSignature(meter));
            }
            _ => {}
        }
        Ok(())
    }

    /// Where the measure being read will stand once it is closed.
    fn place(&self) -> Result<Place> {
        let part = self
            .part
            .as_ref()
            .ok_or_else(|| error("an object before the first staff"))?;
        Ok(Place {
            part: self.parts.len(),
            measure: part.measures(),
        })
    }

    fn serial(&mut self) -> usize {
        self.serials += 1;
        self.serials - 1
    }

    /// A measure to read into next, numbered `number`.
    fn begun(&mut self, number: IntegerType) -> MeasureBuilt {
        self.measures_begun += 1;
        self.position = None;
        MeasureBuilt::new(number, self.measures_begun - 1)
    }

    /// music21's `currentPosition`: the end of what the measure holds the
    /// first time it is asked in a measure, and then where the last notes
    /// placed moved it on to.
    fn current_position(&mut self) -> FloatType {
        *self
            .position
            .get_or_insert_with(|| highest_time(&self.measure.items))
    }

    /// music21's `insertAtPosition`: a clef, key, dynamic or other mark
    /// where the next note goes.
    fn at_position(&mut self, element: StreamElement) {
        let offset = self.current_position();
        self.measure
            .insert(offset, Content::Element(Box::new(element), None));
    }

    fn add_staff(&mut self, attributes: &Attributes) {
        self.new_part();
        self.open_slurs.clear();
        self.tied_steps.clear();
        self.key = KeySignature::new(0);
        self.accidentals.clear();
        self.lyrics.clear();
        self.lyric_position = 0;
        if let Some(part) = &mut self.part {
            part.name = attributes.get("Name").map(str::to_string);
        }
    }

    /// music21's `createPart`: the part being read is closed with the
    /// measure it is in, however empty, and a new one begun.
    fn new_part(&mut self) {
        let fresh = self.begun(0);
        let measure = std::mem::replace(&mut self.measure, fresh);
        if let Some(mut part) = self.part.take() {
            part.append(PartItem::Measure(measure));
            self.parts.push(part);
        }
        self.part = Some(PartBuilt {
            name: None,
            items: Vec::new(),
            slurs: Vec::new(),
        });
        self.measure_number = 0;
    }

    /// Closes the measure being read and begins another numbered `number`.
    fn close_measure(&mut self, number: IntegerType) -> Result<()> {
        let fresh = self.begun(number);
        let measure = std::mem::replace(&mut self.measure, fresh);
        self.part()?.append(PartItem::Measure(measure));
        Ok(())
    }

    fn bar(&mut self, attributes: &Attributes) -> Result<()> {
        self.accidentals.clear();
        // A tie running to the barline goes on into the next measure; any
        // other open tie is dropped.
        let end = highest_time(&self.measure.items);
        let here = self.measure.id;
        self.tied_steps
            .retain(|(_, measure, tied_end)| *measure == Some(here) && *tied_end == end);
        for (_, measure, tied_end) in &mut self.tied_steps {
            *measure = None;
            *tied_end = 0.0;
        }
        let Some(style) = attributes.get("Style") else {
            self.close_measure(self.measure_number)?;
            if !self.repeated.is_empty() {
                let next = self.place()?;
                self.repeated.push(next);
            }
            self.measure_number += 1;
            return Ok(());
        };
        if !self.repeated.is_empty() {
            let spanned = std::mem::take(&mut self.repeated);
            self.endings.push((spanned, self.ending));
        }
        match style {
            "MasterRepeatOpen" | "LocalRepeatOpen" => {
                self.close_measure(0)?;
                self.measure.left = Some(Barline::repeat(RepeatDirection::Start, None));
            }
            "MasterRepeatClose" | "LocalRepeatClose" => {
                self.measure.right = Some(Barline::repeat(RepeatDirection::End, None));
                self.close_measure(0)?;
            }
            "Double" | "SectionOpen" | "SectionClose" => {
                self.measure.right = Some(Barline::new(match style {
                    "Double" => BarlineType::Double,
                    "SectionOpen" => BarlineType::HeavyLight,
                    _ => BarlineType::Final,
                }));
                self.close_measure(0)?;
            }
            other => return Err(error(format!("cannot find a style {other} in our list"))),
        }
        self.measure.number = self.measure_number;
        self.measure_number += 1;
        Ok(())
    }

    fn ending(&mut self, attributes: &Attributes) -> Result<()> {
        let endings = attributes.required("Endings")?;
        if self.measure.left.is_none() {
            self.measure.left = Some(Barline::new(BarlineType::Regular));
        }
        self.repeated = vec![self.place()?];
        let first = endings.split(',').next().unwrap_or("");
        self.ending = number(first)?;
        Ok(())
    }

    fn flow(&mut self, attributes: &Attributes) -> Result<()> {
        let kind = match attributes.required("Style")? {
            "DCalFine" => RepeatExpressionKind::DaCapoAlFine,
            "Coda" | "ToCoda" => RepeatExpressionKind::Coda,
            "Segno" => RepeatExpressionKind::Segno,
            "DSalCoda" => RepeatExpressionKind::DalSegnoAlCoda,
            "Fine" => RepeatExpressionKind::Fine,
            _ => return Err(error("Cannot get style from a flow mark")),
        };
        self.at_position(StreamElement::RepeatExpression(RepeatExpression::new(kind)));
        Ok(())
    }

    fn clef(&mut self, attributes: &Attributes) -> Result<()> {
        let shift = match attributes.get("OctaveShift") {
            None => 0,
            Some("Octave Down") => -1,
            Some("Octave Up") => 1,
            Some(other) => {
                return Err(error(format!(
                    "Did not get a proper octave shift from {other}"
                )));
            }
        };
        let kind = attributes.required("Type")?;
        let (clef, state) = match (kind, shift) {
            ("Treble", 0) => (ClefKind::TrebleClef, ClefState::Treble),
            ("Treble", -1) => (ClefKind::Treble8vbClef, ClefState::TrebleDown),
            ("Treble", _) => (ClefKind::Treble8vaClef, ClefState::TrebleUp),
            ("Bass", 0) => (ClefKind::BassClef, ClefState::Bass),
            ("Bass", -1) => (ClefKind::Bass8vbClef, ClefState::BassDown),
            ("Bass", _) => (ClefKind::Bass8vaClef, ClefState::BassUp),
            ("Alto", 0) => (ClefKind::AltoClef, ClefState::Alto),
            ("Alto", _) => return Err(error("cannot shift octaves on an alto clef")),
            ("Tenor", 0) => (ClefKind::TenorClef, ClefState::Tenor),
            ("Tenor", _) => return Err(error("cannot shift octaves on a tenor clef")),
            ("Percussion", _) => (ClefKind::PercussionClef, ClefState::Percussion),
            (other, _) => {
                return Err(error(format!(
                    "Did not find a proper clef in type, {other}"
                )));
            }
        };
        let mut clef = Clef::of_kind(clef);
        if state == ClefState::Percussion {
            clef.set_line(Some(2));
        }
        self.at_position(StreamElement::Clef(clef));
        self.clef = state;
        Ok(())
    }

    fn key(&mut self, attributes: &Attributes) -> Result<()> {
        let signature = attributes.required("Signature")?;
        let sharps = signature.chars().fold(0, |sharps, mark| match mark {
            '#' => sharps + 1,
            'b' => sharps - 1,
            _ => sharps,
        });
        self.key = KeySignature::new(sharps);
        self.at_position(StreamElement::KeySignature(self.key.clone()));
        Ok(())
    }

    fn staff_instrument(&mut self, attributes: &Attributes) -> Result<()> {
        let patch: IntegerType = attributes.get("Patch").map_or(Ok(0), number)?;
        let program = u8::try_from(patch).map_err(|_| error(format!("no MIDI program {patch}")))?;
        let mut instrument = Instrument::from_midi_program(program)?;
        let transposition: IntegerType = attributes.get("Trans").map_or(Ok(0), number)?;
        instrument.set_transposition(Some(Interval::from_semitones(transposition)?));
        self.part()?
            .append(PartItem::Instrument(Box::new(instrument)));
        Ok(())
    }

    /// music21's `setTieFromPitchInfo`: the tie of each note about to go at
    /// the current position, from its `Pos` value. A position ending `^` is
    /// tied to the note on the same staff step, under whatever clef, that
    /// starts where it ends.
    fn tie_notes(&mut self, notes: &mut [&mut Note], positions: &str, length: FloatType) {
        let offset = self.current_position();
        let end = offset + length;
        let here = self.measure.id;
        for (note, position) in notes.iter_mut().zip(positions.split(',')) {
            let tied_on = position.ends_with('^');
            let Some(place) = first_number(position) else {
                continue;
            };
            let step = self.clef.step_and_octave(place);
            let mut tied_from = false;
            if let Some(at) = self
                .tied_steps
                .iter()
                .position(|(tied, _, _)| *tied == step)
            {
                let (_, measure, tied_end) = self.tied_steps.remove(at);
                tied_from = measure.is_none_or(|measure| measure == here) && tied_end == offset;
            }
            let kind = match (tied_from, tied_on) {
                (true, true) => Some(TieType::Continue),
                (false, true) => Some(TieType::Start),
                (true, false) => Some(TieType::Stop),
                (false, false) => None,
            };
            if let Some(kind) = kind {
                note.set_tie(Some(Tie::new(kind)));
            }
            if tied_on {
                self.tied_steps.push((step, Some(here), end));
            }
        }
    }

    /// music21's `getPitchFromPositionInfo`: a pitch from one `Pos` value,
    /// its tie and stem marks dropped.
    fn pitch(&mut self, position: &str) -> Result<Pitch> {
        let position = position
            .trim_end_matches('^')
            .trim_end_matches('x')
            .trim_end_matches('X')
            .trim_end_matches('z');
        let (accidental, place) = match position.chars().next() {
            Some('n') => (Some("natural"), &position[1..]),
            Some('b') => (Some("flat"), &position[1..]),
            Some('#') => (Some("sharp"), &position[1..]),
            Some('x') => (Some("double-sharp"), &position[1..]),
            Some('v') => (Some("double-flat"), &position[1..]),
            _ => (None, position),
        };
        let place: IntegerType = number(place)?;
        let (mut octave, lowest, steps) = self.clef.table();
        let mut place = place;
        while place < lowest || place > lowest + 6 {
            if place < lowest {
                place += 7;
                octave -= 1;
            }
            if place > lowest + 6 {
                place -= 7;
                octave += 1;
            }
        }
        let step = steps[usize::try_from(place - lowest).unwrap_or(0)];
        let name = format!("{step}{octave}");
        let mut pitch = Pitch::from_name(&name)?;
        if let Some(accidental) = accidental {
            pitch.set_accidental(Accidental::new(accidental)?);
            self.accidentals.retain(|(written, _)| written != &name);
            self.accidentals.push((name, accidental));
        } else if let Some((_, accidental)) = self
            .accidentals
            .iter()
            .find(|(written, _)| written == &name)
        {
            pitch.set_accidental(Accidental::new(*accidental)?);
        } else if let Some(accidental) = self.key.accidental_by_step(step)? {
            pitch.set_accidental(accidental);
        }
        Ok(pitch)
    }

    fn lyric(&self) -> Option<&str> {
        self.lyrics.get(self.lyric_position).map(String::as_str)
    }

    fn note(&mut self, attributes: &Attributes) -> Result<()> {
        let written = attributes.required("Dur")?;
        let position = attributes.required("Pos")?;
        let pitch = self.pitch(position)?;
        let duration = duration_of(written)?;
        let length = duration.quarter_length();
        let serial = self.serial();
        let lyric = self.lyric().map(str::to_string);
        let mut note = Note::from_pitch(pitch.clone()).with_duration(duration.clone());
        self.tie_notes(&mut [&mut note], position, length);
        if let Some(lyric) = lyric {
            note.add_lyric(&lyric, None, false)?;
        }
        let element = if self.clef == ClefState::Percussion {
            let octave = pitch.octave().unwrap_or(4);
            let step = pitch.step().as_char();
            let mut stroke = Unpitched::displayed_at(step, octave)?;
            let written_note = stroke.written_mut();
            written_note.set_duration(duration);
            written_note.set_tie(note.tie().cloned());
            *written_note.lyrics_mut() = note.lyrics().to_vec();
            StreamElement::Unpitched(stroke)
        } else {
            StreamElement::Note(note)
        };
        self.simultaneous(vec![(element, serial, slurred(written))]);
        Ok(())
    }

    /// music21's `translateChord`: a chord for each `Dur` and the `Pos`
    /// beside it, and one more for `Dur2` and `Pos2`, all starting together.
    fn chord(&mut self, attributes: &Attributes) -> Result<()> {
        attributes.required("Dur")?;
        attributes.required("Pos")?;
        let mut groups: Vec<(String, String)> = attributes
            .list("Dur")
            .into_iter()
            .zip(attributes.list("Pos"))
            .collect();
        if let Some(written) = attributes.get("Dur2") {
            groups.push((
                written.to_string(),
                attributes.required("Pos2")?.to_string(),
            ));
        }
        let mut chords = Vec::new();
        for (written, position) in &groups {
            chords.push(self.chord_of(written, position)?);
        }
        self.with_lyric(&mut chords[0].0)?;
        self.simultaneous(chords);
        Ok(())
    }

    /// music21's `translateRestChord`: the chord of `Dur2` and `Pos2`, and a
    /// rest of `Dur` sounding with it.
    fn rest_chord(&mut self, attributes: &Attributes) -> Result<()> {
        let mut chord =
            self.chord_of(attributes.required("Dur2")?, attributes.required("Pos2")?)?;
        let written = attributes.required("Dur")?;
        let mut rest = Rest::new(Duration::quarter());
        rest.set_duration(duration_of(written)?);
        let serial = self.serial();
        self.with_lyric(&mut chord.0)?;
        self.simultaneous(vec![
            chord,
            (StreamElement::Rest(rest), serial, slurred(written)),
        ]);
        Ok(())
    }

    /// music21's `createChord`: one group of a chord's notes.
    fn chord_of(&mut self, written: &str, position: &str) -> Result<(StreamElement, usize, bool)> {
        let duration = duration_of(written)?;
        let length = duration.quarter_length();
        let serial = self.serial();
        let mut pitches = Vec::new();
        for one in position.split(',') {
            pitches.push(self.pitch(one)?);
        }
        let notes: Vec<Note> = pitches.into_iter().map(Note::from_pitch).collect();
        let mut chord = Chord::new(notes)?.with_duration(duration);
        let mut notes: Vec<&mut Note> = chord.notes_mut().iter_mut().collect();
        self.tie_notes(&mut notes, position, length);
        Ok((StreamElement::Chord(chord), serial, slurred(written)))
    }

    /// music21's `addCurrentLyric`: the syllable sung now, on a chord.
    fn with_lyric(&self, element: &mut StreamElement) -> Result<()> {
        if let (Some(lyric), StreamElement::Chord(chord)) = (self.lyric(), element) {
            chord.add_lyric(lyric, None, false)?;
        }
        Ok(())
    }

    /// music21's `appendSimultaneous`: notes, chords or rests starting
    /// together at the current position, which then moves on by the
    /// shortest of them, and the slurs they begin, carry on or end.
    fn simultaneous(&mut self, elements: Vec<(StreamElement, usize, bool)>) {
        let offset = self.current_position();
        let group: Vec<(usize, bool, FloatType)> = elements
            .iter()
            .map(|(element, serial, slurred)| (*serial, *slurred, element.quarter_length()))
            .collect();
        self.measure.put_together(
            offset,
            elements
                .into_iter()
                .map(|(element, serial, _)| (element, serial))
                .collect(),
        );
        self.join_slurs(offset, &group);
        let shortest = group
            .iter()
            .map(|(_, _, length)| *length)
            .fold(FloatType::INFINITY, FloatType::min);
        self.position = Some(offset + shortest);
    }

    /// music21's `setSlurs`: a slurred note is slurred to the next note
    /// starting once it has ended, the one in the same place in its group
    /// if there is one, else the first slurred one, else the first.
    /// Slurred notes in a row make one slur.
    fn join_slurs(&mut self, offset: FloatType, group: &[(usize, bool, FloatType)]) {
        let here = self.measure.id;
        let ended: Vec<OpenSlur> = self
            .open_slurs
            .iter()
            .filter(|open| open.measure != here || open.end <= offset)
            .copied()
            .collect();
        let first_slurred = group
            .iter()
            .position(|(_, slurred, _)| *slurred)
            .unwrap_or(0);
        for (index, (serial, slurred, length)) in group.iter().enumerate() {
            let mut arriving: Vec<OpenSlur> = ended
                .iter()
                .filter(|open| {
                    let lands = if open.group < group.len() {
                        open.group
                    } else {
                        first_slurred
                    };
                    lands == index
                })
                .copied()
                .collect();
            let end = offset + length;
            if *slurred && !arriving.is_empty() {
                let start = arriving.remove(0).start;
                self.open_slurs.retain(|open| open.start != start);
                self.open_slurs.push(OpenSlur {
                    start,
                    measure: here,
                    end,
                    group: index,
                });
            } else if *slurred {
                self.open_slurs.push(OpenSlur {
                    start: *serial,
                    measure: here,
                    end,
                    group: index,
                });
            }
            for open in arriving {
                if let Some(part) = self.part.as_mut() {
                    part.slurs.push((open.start, *serial));
                }
                self.open_slurs.retain(|kept| kept.start != open.start);
            }
        }
    }

    fn finish(mut self) -> Result<Stream> {
        let fresh = self.begun(0);
        let measure = std::mem::replace(&mut self.measure, fresh);
        let mut part = self
            .part
            .take()
            .ok_or_else(|| error("a file with no staff"))?;
        if measure.holds_anything() {
            part.append(PartItem::Measure(measure));
        }
        self.parts.push(part);

        let mut score = Stream::with_kind(StreamKind::Score);
        if let Some(metadata) = self.metadata {
            score.set_metadata(Some(metadata));
        }
        for (index, part) in self.parts.into_iter().enumerate() {
            score.insert(0.0, assemble(part, index, &self.endings));
        }
        Ok(score)
    }
}

/// A value rebuilt with dots, as music21's `dots` setter rebuilds its one
/// component: the grace and tuplets it had kept.
fn with_dots(duration: &Duration, dots: u32) -> Duration {
    let Some((value, _)) = duration.type_and_dots() else {
        return duration.clone();
    };
    let mut dotted = Duration::from_type_with_dots(value, dots);
    dotted.set_tuplets(duration.tuplets());
    dotted.set_grace(duration.grace().cloned());
    dotted.set_expression_is_inferred(duration.expression_is_inferred());
    dotted
}

/// A slur begun at a note and not yet joined: the note's number, the
/// measure it is in, where it ends and its place in its group.
#[derive(Clone, Copy)]
struct OpenSlur {
    start: usize,
    measure: usize,
    end: FloatType,
    group: usize,
}

/// music21's `setDurationForObject`: the length a `Dur` value gives, with
/// its dots, grace and triplet.
fn duration_of(written: &str) -> Result<Duration> {
    let mut parts = written.split(',');
    let length = match parts.next().unwrap_or("") {
        "Whole" => 4.0,
        "Half" => 2.0,
        "4th" => 1.0,
        "8th" => 0.5,
        "16th" => 0.25,
        "32nd" => 0.125,
        "64th" => 0.0625,
        other => return Err(error(format!("no note length {other}"))),
    };
    let mut duration = Duration::new(length)?;
    for part in written.split(',') {
        match part {
            "Grace" => duration = duration.grace_duration(),
            "Dotted" => duration = with_dots(&duration, 1),
            "DblDotted" => duration = with_dots(&duration, 2),
            "Triplet" | "Triplet=First" | "Triplet=End" => {
                let value = duration
                    .type_and_dots()
                    .map_or(DurationType::Quarter, |(value, _)| value);
                duration.append_tuplet(Tuplet::new(3, 2, value, 0));
            }
            _ => {}
        }
    }
    Ok(duration)
}

/// Whether a `Dur` value slurs the note to the next.
fn slurred(written: &str) -> bool {
    written.split(',').any(|part| part == "Slur")
}

/// The staff position in a `Pos` value: its first whole number, as
/// music21 finds it with `-?\d+`.
fn first_number(position: &str) -> Option<IntegerType> {
    let start = position.find(|c: char| c.is_ascii_digit())?;
    let negative = position[..start].ends_with('-');
    let digits: String = position[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let value: IntegerType = digits.parse().ok()?;
    Some(if negative { -value } else { value })
}

/// The parts of `Dur` and `Pos` that are numbers, read as Python's `int`.
fn number<T: std::str::FromStr>(text: &str) -> Result<T> {
    text.trim()
        .parse()
        .map_err(|_| error(format!("{text} is not a number")))
}

/// music21's `createLyrics`: a verse split into the syllables sung to each
/// note, a syllable after a hyphen written with one before it.
fn lyrics(text: &str) -> Vec<String> {
    let text = text
        .trim_matches('"')
        .replace("\r\n", " ")
        .replace(['\r', '\n'], " ");
    let mut syllables = Vec::new();
    for word in text.split(' ') {
        let mut first = true;
        for part in word.split('-') {
            for piece in part.split('\n') {
                let syllable = if piece.is_empty() {
                    " - ".to_string()
                } else if first {
                    piece.to_string()
                } else {
                    format!(" -{piece}")
                };
                first = false;
                syllables.push(syllable);
            }
        }
    }
    syllables
}

/// A measure by the part it is in and its number among that part's
/// measures, both counted from nought.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Place {
    part: usize,
    measure: usize,
}

/// A part read, put together as a stream: its measures and instruments where
/// they stood, each measure's elements in music21's order, the endings
/// opening or closing on its measures, and its slurs.
fn assemble(part: PartBuilt, index: usize, endings: &[(Vec<Place>, u32)]) -> Stream {
    let mut stream = Stream::with_kind(StreamKind::Part);
    stream.set_name(part.name.clone());
    stream.set_abbreviation(part.name.clone());
    // Each note read, by its number, at its place among the part's leaves.
    let mut places: Vec<(usize, usize)> = Vec::new();
    let mut leaves = 0;
    let mut measures = 0;
    for (offset, item) in part.items {
        match item {
            PartItem::Instrument(instrument) => {
                stream.insert(offset, StreamElement::Instrument(instrument));
                leaves += 1;
            }
            PartItem::Measure(built) => {
                let mut measure = Stream::with_kind(StreamKind::Measure);
                measure.set_number(built.number);
                measure.set_left_barline(built.left);
                measure.set_right_barline(built.right);
                // A bracket is marked where it opens and where it closes.
                let here = Place {
                    part: index,
                    measure: measures,
                };
                if let Some((spanned, number)) = endings.iter().find(|(spanned, _)| {
                    spanned.first() == Some(&here) || spanned.last() == Some(&here)
                }) {
                    measure.set_ending(Some(Ending::new(
                        vec![*number],
                        spanned.first() == Some(&here),
                        spanned.last() == Some(&here),
                    )));
                }
                for place in sorted_places(&built.items) {
                    let item = &built.items[place];
                    match &item.content {
                        Content::Element(element, serial) => {
                            if let Some(serial) = serial {
                                places.push((*serial, leaves));
                            }
                            leaves += 1;
                            measure.insert(item.offset, (**element).clone());
                        }
                        Content::Voice(voice) => {
                            let mut inner = Stream::with_kind(StreamKind::Voice);
                            inner.set_id(Some(voice.id.to_string()));
                            for place in sorted_places(&voice.items) {
                                let held = &voice.items[place];
                                if let Content::Element(element, serial) = &held.content {
                                    if let Some(serial) = serial {
                                        places.push((*serial, leaves));
                                    }
                                    leaves += 1;
                                    inner.insert(held.offset, (**element).clone());
                                }
                            }
                            measure.insert(item.offset, inner);
                        }
                    }
                }
                stream.insert(offset, measure);
                measures += 1;
            }
        }
    }
    let place_of = |serial: usize| {
        places
            .iter()
            .find(|(read, _)| *read == serial)
            .map(|(_, place)| *place)
    };
    for (start, end) in part.slurs {
        stream.add_spanner(Spanner::with_unplaced(
            crate::spanner::SpannerKind::Slur,
            vec![place_of(start), place_of(end)],
        ));
    }
    stream
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_is_a_step_and_octave_under_its_clef() {
        assert_eq!(step_and_octave("BASS", 3), ('G', 3));
        assert_eq!(step_and_octave("TREBLE", 1), ('C', 5));
        assert_eq!(step_and_octave("TREBLE", 0), ('B', 4));
        assert_eq!(step_and_octave("TREBLE", -6), ('C', 4));
        assert_eq!(step_and_octave("TENOR", 2), ('C', 4));
    }

    #[test]
    fn a_refusal_says_its_line() {
        let text = "!NoteWorthyComposer(2.0)\n|AddStaff|\n|Clef|Type:Bass\n\
                    |Note|Dur:Whole|Pos:1\n|Note|Dur:Eon|Pos:2\n";
        let error = from_noteworthy(text).unwrap_err();
        assert_eq!(error.to_string(), "NoteWorthy error: no note length Eon");
        assert_eq!(error.position(), Some(&Position::line(5)));

        let error = from_noteworthy("|AddStaff|\n|Note|Dur:4th|Pos:x\n").unwrap_err();
        assert_eq!(error.position(), Some(&Position::line(2)));
    }

    fn names(score: &Stream) -> Vec<String> {
        score
            .pitches()
            .iter()
            .map(|pitch| pitch.name_with_octave())
            .collect()
    }

    #[test]
    fn a_note_is_spelled_from_its_place_under_the_clef() {
        let score = from_noteworthy(
            "|AddStaff|\n|Clef|Type:Bass\n|Note|Dur:4th|Pos:b3\n|Note|Dur:4th|Pos:3\n|Bar\n|Note|Dur:4th|Pos:3\n",
        )
        .unwrap();
        // An accidental lasts to the barline.
        assert_eq!(names(&score), ["Gb3", "Gb3", "G3"]);
    }

    #[test]
    fn a_key_signature_spells_the_notes_after_it() {
        let score = from_noteworthy(
            "|AddStaff|\n|Key|Signature:F#,C#,G#,D#\n|Chord|Dur:Half|Pos:1,3,#5\n|Chord|Dur:Half|Pos:1,3,5\n",
        )
        .unwrap();
        assert_eq!(names(&score), ["C#5", "E5", "G#5", "C#5", "E5", "G#5"]);
    }

    #[test]
    fn the_first_two_measures_are_both_numbered_nought() {
        let score =
            from_noteworthy("|AddStaff|\n|Note|Dur:Whole|Pos:1\n|Bar\n|Note|Dur:Whole|Pos:1\n|Bar\n|Note|Dur:Whole|Pos:1\n")
                .unwrap();
        let numbers: Vec<IntegerType> = score.parts()[0]
            .measures()
            .iter()
            .map(|measure| measure.number())
            .collect();
        assert_eq!(numbers, [0, 0, 1]);
    }

    #[test]
    fn a_verse_is_split_into_syllables() {
        assert_eq!(lyrics("\"Hello world\""), ["Hello", "world"]);
        assert_eq!(
            lyrics("\"A-ve Ma-ri-a\""),
            ["A", " -ve", "Ma", " -ri", " -a"]
        );
    }

    #[test]
    fn a_line_with_an_attribute_and_no_value_is_refused() {
        assert!(from_noteworthy("|AddStaff|\n|Note|Dur\n").is_err());
        assert!(from_noteworthy("|Note|Dur:4th|Pos:1\n").is_err());
    }

    /// The notes, chords and rests of the first measure: where each starts
    /// in it, how long it is and what it sounds.
    fn first_measure(lines: &str) -> Vec<(FloatType, FloatType, String)> {
        let score = from_noteworthy(&format!("|AddStaff|\n|Clef|Type:Treble\n{lines}")).unwrap();
        let mut out: Vec<(FloatType, FloatType, String)> = score.parts()[0].measures()[0]
            .leaves()
            .into_iter()
            .filter_map(|(offset, element)| {
                let sounds = match element {
                    StreamElement::Note(note) => note.pitch().name_with_octave(),
                    StreamElement::Chord(chord) => chord
                        .notes()
                        .iter()
                        .map(|note| note.pitch().name_with_octave())
                        .collect::<Vec<_>>()
                        .join(" "),
                    StreamElement::Rest(_) => "rest".to_string(),
                    _ => return None,
                };
                Some((offset, element.quarter_length(), sounds))
            })
            .collect();
        out.sort_by(|left, right| left.partial_cmp(right).unwrap());
        out
    }

    #[test]
    fn a_chord_of_two_lengths_is_two_chords_starting_together() {
        assert_eq!(
            first_measure(
                "|Note|Dur:4th|Pos:0\n\
                 |Chord|Dur:8th|Pos:-4,n-3,b-2,#-1,x0,v1,2x|Opts:Stem=Down|Dur2:8th,DblDotted|Pos2:3x\n\
                 |Note|Dur:4th|Pos:-5\n"
            ),
            [
                (0.0, 1.0, "B4".to_string()),
                (1.0, 0.5, "E4 F4 Gb4 A#4 B##4 Cbb5 D5".to_string()),
                (1.0, 0.875, "E5".to_string()),
                (1.5, 1.0, "D4".to_string()),
            ]
        );
    }

    #[test]
    fn a_rest_chord_is_a_rest_and_a_chord_starting_together() {
        assert_eq!(
            first_measure(
                "|Note|Dur:4th|Pos:0\n\
                 |RestChord|Dur:Half|Opts:Stem=Up|Dur2:4th|Pos2:-1,1\n\
                 |RestChord|Dur:4th|Dur2:4th|Pos2:-5\n\
                 |Note|Dur:4th|Pos:0\n"
            ),
            [
                (0.0, 1.0, "B4".to_string()),
                (1.0, 1.0, "A4 C5".to_string()),
                (1.0, 2.0, "rest".to_string()),
                (2.0, 1.0, "D4".to_string()),
                (2.0, 1.0, "rest".to_string()),
                (3.0, 1.0, "B4".to_string()),
            ]
        );
    }

    /// A part's notes, chords and rests as music21 writes their reprs.
    fn shown(element: &StreamElement) -> String {
        match element {
            StreamElement::Note(note) => format!("Note {}", note.pitch().name()),
            StreamElement::Chord(chord) => format!(
                "Chord {}",
                chord
                    .notes()
                    .iter()
                    .map(|note| note.pitch().name_with_octave())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            _ => "other".to_string(),
        }
    }

    fn treble(lines: &[&str]) -> Stream {
        let mut text = String::from("|AddStaff|\n|Clef|Type:Treble\n");
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        from_noteworthy(&text).unwrap()
    }

    /// Every slur of the first part, as what it joins.
    fn slurs(lines: &[&str]) -> Vec<Vec<String>> {
        let score = treble(lines);
        let part = &score.parts()[0];
        let leaves = part.leaves();
        let mut found: Vec<Vec<String>> = part
            .spanners()
            .iter()
            .map(|slur| {
                slur.spanned()
                    .iter()
                    .map(|place| place.map_or("none".to_string(), |at| shown(leaves[at].1)))
                    .collect()
            })
            .collect();
        found.sort();
        found
    }

    /// Every note of the first part, with its offset in the part and its tie.
    fn ties(lines: &[&str]) -> Vec<(String, FloatType, Option<TieType>)> {
        let score = treble(lines);
        let part = &score.parts()[0];
        let mut found = Vec::new();
        for (offset, element) in part.leaves() {
            let notes: Vec<&Note> = match element {
                StreamElement::Note(note) => vec![note],
                StreamElement::Chord(chord) => chord.notes().iter().collect(),
                _ => continue,
            };
            for note in notes {
                found.push((
                    note.pitch().name_with_octave(),
                    offset,
                    note.tie().map(Tie::tie_type),
                ));
            }
        }
        found.sort_by(|left, right| left.1.total_cmp(&right.1).then(left.0.cmp(&right.0)));
        found
    }

    fn strings(groups: &[&[&str]]) -> Vec<Vec<String>> {
        let mut made: Vec<Vec<String>> = groups
            .iter()
            .map(|group| group.iter().map(|one| (*one).to_string()).collect())
            .collect();
        made.sort();
        made
    }

    #[test]
    fn notes_after_a_group_follow_the_shortest_of_it() {
        assert_eq!(
            first_measure(
                "|Chord|Dur:8th|Pos:-4|Dur2:8th,DblDotted|Pos2:3\n\
                 |Note|Dur:4th|Pos:-5\n\
                 |Note|Dur:4th|Pos:-4\n"
            ),
            [
                (0.0, 0.5, "E4".to_string()),
                (0.0, 0.875, "E5".to_string()),
                (0.5, 1.0, "D4".to_string()),
                (1.5, 1.0, "E4".to_string()),
            ]
        );
    }

    #[test]
    fn a_slur_runs_to_the_next_note_once_it_ends() {
        assert_eq!(
            slurs(&[
                "|RestChord|Dur:Half|Dur2:4th,Slur|Pos2:-1,1",
                "|Note|Dur:4th|Pos:0"
            ]),
            strings(&[&["Chord A4 C5", "Note B"]])
        );
        assert_eq!(
            slurs(&[
                "|Chord|Dur:Half|Pos:1|Dur2:4th|Pos2:3",
                "|Note|Dur:4th,Slur|Pos:2",
                "|Bar|",
                "|Note|Dur:4th|Pos:4",
            ]),
            strings(&[&["Note D", "Note F"]])
        );
        assert_eq!(
            slurs(&[
                "|Note|Dur:4th,Slur|Pos:1",
                "|AddStaff|",
                "|Clef|Type:Treble",
                "|Note|Dur:4th|Pos:1",
            ]),
            Vec::<Vec<String>>::new()
        );
    }

    #[test]
    fn a_slur_runs_to_the_same_group() {
        assert_eq!(
            slurs(&[
                "|Chord|Dur:4th,Slur|Pos:1|Dur2:4th,Slur|Pos2:3",
                "|Chord|Dur:4th|Pos:2|Dur2:4th|Pos2:4",
            ]),
            strings(&[&["Chord C5", "Chord D5"], &["Chord E5", "Chord F5"]])
        );
        assert_eq!(
            slurs(&[
                "|Chord|Dur:4th,Slur|Pos:1|Dur2:4th,Slur|Pos2:3",
                "|Note|Dur:4th|Pos:2",
            ]),
            strings(&[&["Chord C5", "Note D"], &["Chord E5", "Note D"]])
        );
    }

    #[test]
    fn a_tie_follows_its_position_and_pitch() {
        let start = Some(TieType::Start);
        let stop = Some(TieType::Stop);
        let at = |name: &str, offset: FloatType, tie| (name.to_string(), offset, tie);
        assert_eq!(
            ties(&[
                "|Chord|Dur:4th|Pos:1^|Dur2:4th|Pos2:3",
                "|Chord|Dur:4th|Pos:1|Dur2:4th|Pos2:3"
            ]),
            [
                at("C5", 0.0, start),
                at("E5", 0.0, None),
                at("C5", 1.0, stop),
                at("E5", 1.0, None)
            ]
        );
        assert_eq!(
            ties(&[
                "|Note|Dur:4th|Pos:-6^",
                "|Clef|Type:Bass",
                "|Note|Dur:4th|Pos:6"
            ]),
            [at("C4", 0.0, start), at("C4", 1.0, stop)]
        );
        assert_eq!(
            ties(&[
                "|Note|Dur:4th|Pos:1^",
                "|Note|Dur:4th|Pos:2",
                "|Note|Dur:4th|Pos:1"
            ]),
            [
                at("C5", 0.0, start),
                at("D5", 1.0, None),
                at("C5", 2.0, None)
            ]
        );
        assert_eq!(
            ties(&["|Note|Dur:Whole|Pos:1^", "|Bar|", "|Note|Dur:Whole|Pos:1"]),
            [at("C5", 0.0, start), at("C5", 4.0, stop)]
        );
        assert_eq!(
            ties(&[
                "|Chord|Dur:4th|Pos:1^|Dur2:Half|Pos2:3",
                "|Bar|",
                "|Note|Dur:4th|Pos:1"
            ]),
            [
                at("C5", 0.0, start),
                at("E5", 0.0, None),
                at("C5", 2.0, None)
            ]
        );
    }

    #[test]
    fn a_direction_goes_where_the_next_note_will() {
        for line in [
            "|Dynamic|Style:p|Pos:-8",
            "|Clef|Type:Bass",
            "|Key|Signature:F#",
            "|Text|Text:\"dolce\"|Pos:8",
        ] {
            let score = treble(&[
                "|Chord|Dur:4th|Pos:-1|Dur2:Whole|Pos2:3",
                line,
                "|Note|Dur:4th|Pos:1",
            ]);
            let measure = &score.parts()[0].measures()[0];
            let placed: Vec<FloatType> = measure
                .leaves()
                .into_iter()
                .filter(|(_, element)| {
                    matches!(
                        element,
                        StreamElement::Dynamic(_)
                            | StreamElement::KeySignature(_)
                            | StreamElement::TextExpression(_)
                            | StreamElement::Note(_)
                    ) || matches!(element, StreamElement::Clef(clef) if clef.kind() == ClefKind::BassClef)
                })
                .map(|(offset, _)| offset)
                .collect();
            assert_eq!(placed, [1.0, 1.0], "{line}");
        }
    }
}
