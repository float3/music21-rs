//! NoteWorthy Composer's text format, `.nwctxt`: music21's
//! `noteworthy.translate`.
//!
//! A file is a line per object, each `|Command|Name:value|Name:value`, and
//! [`from_noteworthy`] reads it into a score of a part per staff. The binary
//! `.nwc` format is read by [`from_nwc`], which writes it out as those lines
//! first, as music21 does.

mod binary;
pub mod placement;

pub use binary::{
    ALTERATION_TEXTS, BAR_STYLES, CLEF_NAMES, DURATION_VALUES, FLAT_MASK, MIDI_INSTRUMENTS, Member,
    NwcFont, NwcHeader, NwcObject, NwcReader, NwcStaff, OCTAVE_SHIFT_NAMES, Object, SHARP_MASK,
    dump_lines, from_nwc, nwc_lines,
};

use crate::bar::{Barline, BarlineType, BracketPlace, Ending, RepeatDirection};
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
use crate::notation::{Placement as Side, Tie, TieType};
use crate::note::Note;
use crate::percussion::Unpitched;
use crate::pitch::Pitch;
use crate::pitch::accidental::Accidental;
use crate::repeat::{RepeatExpression, RepeatExpressionKind};
use crate::rest::Rest;
use crate::spanner::Spanner;
use crate::stream::{Stream, StreamElement, StreamKind};
use crate::tempo::MetronomeMark;

use placement::{Placement, Placing};

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

    /// The name music21's reader keeps for this clef as `currentClef`.
    fn music21_name(self) -> &'static str {
        match self {
            Self::Treble => "TREBLE",
            Self::TrebleDown => "TREBLE8dw",
            Self::TrebleUp => "TREBLE8up",
            Self::Bass => "BASS",
            Self::BassDown => "BASS8dw",
            Self::BassUp => "BASS8up",
            Self::Alto => "ALTO",
            Self::Tenor => "TENOR",
            Self::Percussion => "PERCUSSION",
        }
    }

    /// The step and octave of a staff position under this clef.
    fn step_and_octave(self, position: IntegerType) -> (char, IntegerType) {
        let (octave, lowest, steps) = self.table();
        // Wider than the position, so one as far off the staff as an
        // integer goes neither overflows nor panics.
        let from_lowest = i64::from(position) - i64::from(lowest);
        let step = steps[usize::try_from(from_lowest.rem_euclid(7)).unwrap_or(0)];
        let octave = i64::from(octave) + from_lowest.div_euclid(7);
        let octave = IntegerType::try_from(octave).unwrap_or(if octave < 0 {
            IntegerType::MIN
        } else {
            IntegerType::MAX
        });
        (step, octave)
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
}

/// An element being placed, with its number among the notes, chords and
/// rests read.
type Placed = (StreamElement, Option<usize>);

/// The measure being read and the part's slurs, as the placing
/// algorithm sees them: a measure is its number, a voice its place among
/// the measure's items, a note its number.
struct Built<'a> {
    measure: &'a mut MeasureBuilt,
    slurs: Option<&'a mut Vec<(usize, usize)>>,
}

impl Placing for Built<'_> {
    type Measure = usize;
    type Voice = usize;
    type Element = Placed;
    type Handle = usize;
    type Error = Error;

    fn copy_measure(&self, measure: &usize) -> usize {
        *measure
    }

    fn copy_handle(&self, handle: &usize) -> usize {
        *handle
    }

    fn same_measure(&self, first: &usize, second: &usize) -> bool {
        first == second
    }

    fn same_handle(&self, first: &usize, second: &usize) -> bool {
        first == second
    }

    fn handle(&self, element: &Placed) -> Result<usize> {
        element
            .1
            .ok_or_else(|| error("a note placed with no number"))
    }

    fn highest_time(&self, _measure: &usize) -> Result<FloatType> {
        Ok(highest_time(&self.measure.items))
    }

    fn length(&self, element: &Placed) -> Result<FloatType> {
        Ok(element.0.quarter_length())
    }

    fn voices(&self, _measure: &usize) -> Result<Vec<usize>> {
        Ok(self.measure.voices())
    }

    fn voice_end(&self, voice: &usize) -> Result<FloatType> {
        Ok(voice_length(&self.measure.items[*voice]))
    }

    fn gather_voice(&mut self, _measure: &usize) -> Result<usize> {
        let measure = &mut *self.measure;
        let notes: Vec<usize> = sorted_places(&measure.items)
            .into_iter()
            .filter(|place| {
                matches!(&measure.items[*place].content,
                    Content::Element(element, _) if is_general_note(element))
            })
            .collect();
        let mut voice = VoiceBuilt {
            id: 0,
            items: Vec::new(),
        };
        for place in &notes {
            let item = &measure.items[*place];
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
        measure.items.retain(|_| {
            let keep = !notes.contains(&index);
            index += 1;
            keep
        });
        measure.insert(0.0, Content::Voice(voice));
        Ok(measure.items.len() - 1)
    }

    fn new_voice(&mut self, _measure: &usize, id: usize) -> Result<usize> {
        self.measure.insert(
            0.0,
            Content::Voice(VoiceBuilt {
                id,
                items: Vec::new(),
            }),
        );
        Ok(self.measure.items.len() - 1)
    }

    fn insert(&mut self, _measure: &usize, offset: FloatType, element: Placed) -> Result<()> {
        let (element, serial) = element;
        self.measure
            .insert(offset, Content::Element(Box::new(element), serial));
        Ok(())
    }

    fn insert_in(&mut self, voice: &usize, offset: FloatType, element: Placed) -> Result<()> {
        let (element, serial) = element;
        if let Content::Voice(voice) = &mut self.measure.items[*voice].content {
            let inserted = voice.items.len();
            voice.items.push(Item {
                offset,
                content: Content::Element(Box::new(element), serial),
                inserted,
            });
        }
        Ok(())
    }

    fn set_ties(&mut self, element: &mut Placed, ties: &[Option<TieType>]) -> Result<()> {
        let notes: Vec<&mut Note> = match &mut element.0 {
            StreamElement::Note(note) => vec![note],
            StreamElement::Unpitched(stroke) => vec![stroke.written_mut()],
            StreamElement::Chord(chord) => chord.notes_mut().iter_mut().collect(),
            _ => Vec::new(),
        };
        for (note, tie) in notes.into_iter().zip(ties) {
            if let Some(kind) = tie {
                note.set_tie(Some(Tie::new(*kind)));
            }
        }
        Ok(())
    }

    fn slur(&mut self, _measure: &usize, start: &usize, end: &usize) -> Result<()> {
        let slurs = self
            .slurs
            .as_mut()
            .ok_or_else(|| error("a slur before the first staff"))?;
        slurs.push((*start, *end));
        Ok(())
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
    /// How many measures have been begun, which numbers each.
    measures_begun: usize,
    /// Where the next object goes, and the slurs and ties left open, by
    /// measure number and note number.
    placement: Placement<usize, usize>,
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
            measures_begun: 1,
            placement: Placement::default(),
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
                    self.at_position(StreamElement::Dynamic(Dynamic::new(style)))?;
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
                self.simultaneous(vec![(
                    (StreamElement::Rest(rest), Some(serial)),
                    slurred(written),
                )])?;
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
                    Side::Above
                } else {
                    Side::Below
                }));
                self.at_position(StreamElement::TextExpression(words))?;
            }
            "TimeSig" => {
                let signature = meter_ratio(attributes.required("Signature")?);
                let meter = crate::meter::TimeSignature::from_ratio_string(signature)?;
                self.at_position(StreamElement::TimeSignature(meter))?;
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
        MeasureBuilt::new(number, self.measures_begun - 1)
    }

    /// The measure being read and the part's slurs, as [`Placing`] asks
    /// for them.
    fn built(&mut self) -> (Built<'_>, &mut Placement<usize, usize>) {
        let slurs = self.part.as_mut().map(|part| &mut part.slurs);
        (
            Built {
                measure: &mut self.measure,
                slurs,
            },
            &mut self.placement,
        )
    }

    /// music21's `insertAtPosition`: a clef, key, dynamic or other mark
    /// where the next note goes.
    fn at_position(&mut self, element: StreamElement) -> Result<()> {
        let id = self.measure.id;
        let (mut built, placement) = self.built();
        placement.mark(&mut built, &id, (element, None))
    }

    fn add_staff(&mut self, attributes: &Attributes) {
        self.new_part();
        self.placement.new_staff();
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
        let id = self.measure.id;
        let (built, placement) = self.built();
        placement.barline(&built, &id)?;
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
        match bar_style(style)? {
            BarStyle::RepeatOpen => {
                self.close_measure(0)?;
                self.measure.left = Some(Barline::repeat(RepeatDirection::Start, None));
            }
            BarStyle::RepeatClose => {
                self.measure.right = Some(Barline::repeat(RepeatDirection::End, None));
                self.close_measure(0)?;
            }
            BarStyle::Closing(kind) => {
                self.measure.right = Some(Barline::new(kind));
                self.close_measure(0)?;
            }
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
        let kind = flow_kind(attributes.required("Style")?)?;
        self.at_position(StreamElement::RepeatExpression(RepeatExpression::new(kind)))?;
        Ok(())
    }

    fn clef(&mut self, attributes: &Attributes) -> Result<()> {
        let (clef, state) = clef_of(attributes.required("Type")?, attributes.get("OctaveShift"))?;
        self.at_position(StreamElement::Clef(clef))?;
        self.clef = state;
        Ok(())
    }

    fn key(&mut self, attributes: &Attributes) -> Result<()> {
        self.key = KeySignature::new(key_sharps(attributes.required("Signature")?));
        self.at_position(StreamElement::KeySignature(self.key.clone()))?;
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

    /// music21's `setTieFromPitchInfo`: the ties of an element about to go
    /// at the current position, from its `Pos` value, each note's staff
    /// step under the clef in force and whether it ends `^`.
    fn tied(&mut self, element: &mut Placed, positions: &str) -> Result<()> {
        let steps: Vec<((char, IntegerType), bool)> = positions
            .split(',')
            .filter_map(|position| {
                let place = first_number(position)?;
                Some((self.clef.step_and_octave(place), position.ends_with('^')))
            })
            .collect();
        let id = self.measure.id;
        let (mut built, placement) = self.built();
        placement.tie(&mut built, &id, element, &steps)
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
        let serial = self.serial();
        let lyric = self.lyric().map(str::to_string);
        let mut note = Note::from_pitch(pitch.clone()).with_duration(duration.clone());
        if let Some(lyric) = lyric {
            note.add_lyric(&lyric, None, false)?;
        }
        let element = if self.clef == ClefState::Percussion {
            let octave = pitch.octave().unwrap_or(4);
            let step = pitch.step().as_char();
            let mut stroke = Unpitched::displayed_at(step, octave)?;
            let written_note = stroke.written_mut();
            written_note.set_duration(duration);
            *written_note.lyrics_mut() = note.lyrics().to_vec();
            StreamElement::Unpitched(stroke)
        } else {
            StreamElement::Note(note)
        };
        let mut element = (element, Some(serial));
        self.tied(&mut element, position)?;
        self.simultaneous(vec![(element, slurred(written))])
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
        self.with_lyric(&mut chords[0].0.0)?;
        self.simultaneous(chords)
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
        self.with_lyric(&mut chord.0.0)?;
        self.simultaneous(vec![
            chord,
            ((StreamElement::Rest(rest), Some(serial)), slurred(written)),
        ])
    }

    /// music21's `createChord`: one group of a chord's notes.
    fn chord_of(&mut self, written: &str, position: &str) -> Result<(Placed, bool)> {
        let duration = duration_of(written)?;
        let serial = self.serial();
        let mut pitches = Vec::new();
        for one in position.split(',') {
            pitches.push(self.pitch(one)?);
        }
        let notes: Vec<Note> = pitches.into_iter().map(Note::from_pitch).collect();
        let chord = Chord::new(notes)?.with_duration(duration);
        let mut element = (StreamElement::Chord(chord), Some(serial));
        self.tied(&mut element, position)?;
        Ok((element, slurred(written)))
    }

    /// music21's `addCurrentLyric`: the syllable sung now, on a chord.
    fn with_lyric(&self, element: &mut StreamElement) -> Result<()> {
        if let (Some(lyric), StreamElement::Chord(chord)) = (self.lyric(), element) {
            chord.add_lyric(lyric, None, false)?;
        }
        Ok(())
    }

    /// music21's `appendSimultaneous`: notes, chords or rests starting
    /// together at the current position, each with whether it is slurred.
    fn simultaneous(&mut self, elements: Vec<(Placed, bool)>) -> Result<()> {
        let id = self.measure.id;
        let (mut built, placement) = self.built();
        placement.place(&mut built, &id, elements)
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

/// music21's `createClef`: the clef a `Type` and `OctaveShift` name, and
/// the name music21's reader keeps for it as `currentClef`.
///
/// # Errors
///
/// An octave shift or clef music21 does not know, or a shifted alto or
/// tenor clef.
pub fn clef_named(kind: &str, shift: Option<&str>) -> Result<(Clef, &'static str)> {
    let (clef, state) = clef_of(kind, shift)?;
    Ok((clef, state.music21_name()))
}

fn clef_of(kind: &str, shift: Option<&str>) -> Result<(Clef, ClefState)> {
    let shift = match shift {
        None => 0,
        Some("Octave Down") => -1,
        Some("Octave Up") => 1,
        Some(other) => {
            return Err(error(format!(
                "Did not get a proper octave shift from {other}"
            )));
        }
    };
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
    Ok((clef, state))
}

/// music21's `createKey`: sharps less flats in a `Signature` value.
#[must_use]
pub fn key_sharps(signature: &str) -> IntegerType {
    signature.chars().fold(0, |sharps, mark| match mark {
        '#' => sharps + 1,
        'b' => sharps - 1,
        _ => sharps,
    })
}

/// music21's `createTimeSignature`: a `Signature` value as a ratio.
#[must_use]
pub fn meter_ratio(signature: &str) -> &str {
    match signature {
        "AllaBreve" => "2/2",
        "Common" => "4/4",
        other => other,
    }
}

/// music21's `createOtherRepetitions`: the mark a flow `Style` names.
///
/// # Errors
///
/// A style music21 does not know.
pub fn flow_kind(style: &str) -> Result<RepeatExpressionKind> {
    match style {
        "DCalFine" => Ok(RepeatExpressionKind::DaCapoAlFine),
        "Coda" | "ToCoda" => Ok(RepeatExpressionKind::Coda),
        "Segno" => Ok(RepeatExpressionKind::Segno),
        "DSalCoda" => Ok(RepeatExpressionKind::DalSegnoAlCoda),
        "Fine" => Ok(RepeatExpressionKind::Fine),
        _ => Err(error(format!("Cannot get style from {style}"))),
    }
}

/// What a barline `Style` does to the measure: music21's
/// `createBarlines`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarStyle {
    /// The next measure opens with a start repeat.
    RepeatOpen,
    /// The measure closes with an end repeat.
    RepeatClose,
    /// The measure closes with a barline of this kind.
    Closing(BarlineType),
}

/// The barline a `Style` names.
///
/// # Errors
///
/// A style music21 does not know.
pub fn bar_style(style: &str) -> Result<BarStyle> {
    match style {
        "MasterRepeatOpen" | "LocalRepeatOpen" => Ok(BarStyle::RepeatOpen),
        "MasterRepeatClose" | "LocalRepeatClose" => Ok(BarStyle::RepeatClose),
        "Double" => Ok(BarStyle::Closing(BarlineType::Double)),
        "SectionOpen" => Ok(BarStyle::Closing(BarlineType::HeavyLight)),
        "SectionClose" => Ok(BarStyle::Closing(BarlineType::Final)),
        other => Err(error(format!("cannot find a style {other} in our list"))),
    }
}

/// music21's `setDurationForObject`: the length a `Dur` value gives, with
/// its dots, grace and triplet.
///
/// # Errors
///
/// A length music21 does not know.
pub fn duration_of(written: &str) -> Result<Duration> {
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
#[must_use]
pub fn first_number(position: &str) -> Option<IntegerType> {
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
#[must_use]
pub fn lyrics(text: &str) -> Vec<String> {
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
                    let ending = Ending::new(
                        vec![*number],
                        spanned.first() == Some(&here),
                        spanned.last() == Some(&here),
                    );
                    measure.set_ending(Some(ending.placed(BracketPlace::LastMeasure)));
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

    /// A position as far from the staff as an integer goes gives some
    /// step and octave rather than overflowing: the fuzzer found one.
    #[test]
    fn an_extreme_position_does_not_overflow() {
        for clef in ["TREBLE", "BASS", "TREBLE8dw", "ALTO"] {
            let _ = step_and_octave(clef, IntegerType::MIN);
            let _ = step_and_octave(clef, IntegerType::MAX);
        }
    }

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
