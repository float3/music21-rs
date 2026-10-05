//! NoteWorthy Composer's text format, `.nwctxt`: music21's
//! `noteworthy.translate`.
//!
//! A file is a line per object, each `|Command|Name:value|Name:value`, and
//! [`from_noteworthy`] reads it into a score of a part per staff. The binary
//! `.nwc` format is not read.

use crate::bar::{Barline, BarlineType, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::clef::{Clef, ClefKind};
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{Duration, DurationType, Tuplet};
use crate::dynamics::Dynamic;
use crate::error::{Error, Result};
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
    Error::Noteworthy(message.into())
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
/// use music21_rs::noteworthy::from_noteworthy;
///
/// let score = from_noteworthy(
///     "!NoteWorthyComposer(2.0)\n|AddStaff|\n|Clef|Type:Bass\n|TimeSig|Signature:4/4\n|Note|Dur:Whole|Pos:1\n",
/// )?;
/// let pitches = score.pitches();
/// assert_eq!(pitches[0].name_with_octave(), "E3");
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A file with no staff, a line music21 cannot read -- an attribute with no
/// value, a clef, barline style, flow mark or note length it does not know,
/// an octave shift on an alto or tenor clef -- or a number that is not one.
pub fn from_noteworthy(text: &str) -> Result<Stream> {
    let mut reader = Reader::new();
    for line in text.lines() {
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
                    return Err(error(format!(
                        "Cannot unpack value from {attribute} in {line}"
                    )));
                }
            }
        }
        reader.command(command, &attributes)?;
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
    number: IntegerType,
    left: Option<Barline>,
    right: Option<Barline>,
    items: Vec<Item>,
    inserted: usize,
}

impl MeasureBuilt {
    fn new(number: IntegerType) -> Self {
        Self {
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

    /// music21's `append`.
    fn append(&mut self, element: StreamElement, serial: Option<usize>) {
        let at = highest_time(&self.items);
        self.insert(at, Content::Element(Box::new(element), serial));
    }

    fn has_voices(&self) -> bool {
        self.items
            .iter()
            .any(|item| matches!(item.content, Content::Voice(_)))
    }

    /// music21's truth of a stream: whether it holds anything, barlines
    /// included.
    fn holds_anything(&self) -> bool {
        !self.items.is_empty() || self.left.is_some() || self.right.is_some()
    }

    /// Where music21's `getShortestStream` appends: the measure, or the
    /// shortest of its voices, the first of those equally short.
    fn append_shortest(&mut self, element: StreamElement, serial: Option<usize>) {
        if !self.has_voices() {
            self.append(element, serial);
            return;
        }
        let shortest = sorted_places(&self.items)
            .into_iter()
            .filter(|place| matches!(self.items[*place].content, Content::Voice(_)))
            .min_by(|left, right| {
                voice_length(&self.items[*left]).total_cmp(&voice_length(&self.items[*right]))
            });
        if let Some(place) = shortest
            && let Content::Voice(voice) = &mut self.items[place].content
        {
            append_to_voice(voice, element, serial);
        }
    }

    /// music21's `getVoiceAtDuration`: the voice with this id, made where
    /// there is none -- taking the notes the measure holds when it is the
    /// first -- and filled with a rest up to `length`.
    fn voice_at(&mut self, id: usize, length: FloatType) -> usize {
        let found = self
            .items
            .iter()
            .position(|item| matches!(&item.content, Content::Voice(voice) if voice.id == id));
        let place = match found {
            Some(place) => place,
            None => {
                let mut voice = VoiceBuilt {
                    id,
                    items: Vec::new(),
                };
                if !self.has_voices() {
                    let notes: Vec<usize> = sorted_places(&self.items)
                        .into_iter()
                        .filter(|place| {
                            matches!(&self.items[*place].content,
                                Content::Element(element, _) if is_general_note(element))
                        })
                        .collect();
                    for place in &notes {
                        if let Content::Element(element, serial) = &self.items[*place].content {
                            append_to_voice(&mut voice, (**element).clone(), *serial);
                        }
                    }
                    let mut index = 0;
                    self.items.retain(|_| {
                        let keep = !notes.contains(&index);
                        index += 1;
                        keep
                    });
                }
                let at = highest_time(&self.items);
                self.insert(at, Content::Voice(voice));
                self.items.len() - 1
            }
        };
        if let Content::Voice(voice) = &mut self.items[place].content {
            let missing = length - highest_time(&voice.items);
            if missing > 0.0
                && let Ok(duration) = Duration::new(missing)
            {
                let mut rest = Rest::new(duration);
                rest.set_step_shift(3);
                append_to_voice(voice, StreamElement::Rest(rest), None);
            }
        }
        place
    }
}

fn voice_length(item: &Item) -> FloatType {
    match &item.content {
        Content::Voice(voice) => highest_time(&voice.items),
        Content::Element(element, _) => element.quarter_length(),
    }
}

fn append_to_voice(voice: &mut VoiceBuilt, element: StreamElement, serial: Option<usize>) {
    let at = highest_time(&voice.items);
    let inserted = voice.items.len();
    voice.items.push(Item {
        offset: at,
        content: Content::Element(Box::new(element), serial),
        inserted,
    });
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
    within_slur: bool,
    slur_start: Option<usize>,
    within_tie: bool,
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
            measure: MeasureBuilt::new(0),
            measure_number: 0,
            ending: 1,
            repeated: Vec::new(),
            endings: Vec::new(),
            metadata: None,
            clef: ClefState::Treble,
            key: KeySignature::new(0),
            within_slur: false,
            slur_start: None,
            within_tie: false,
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
                    self.measure
                        .append(StreamElement::Dynamic(Dynamic::new(style)), None);
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
            "Rest" => {
                let mut rest = Rest::new(Duration::quarter());
                let serial = self.serial();
                let duration = self.duration(attributes.required("Dur")?, serial)?;
                rest.set_duration(duration);
                self.measure
                    .append_shortest(StreamElement::Rest(rest), Some(serial));
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
                self.measure
                    .append(StreamElement::TextExpression(words), None);
            }
            "TimeSig" => {
                let signature = match attributes.required("Signature")? {
                    "AllaBreve" => "2/2",
                    "Common" => "4/4",
                    other => other,
                };
                let meter = crate::meter::TimeSignature::from_ratio_string(signature)?;
                self.measure
                    .append(StreamElement::TimeSignature(meter), None);
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

    fn add_staff(&mut self, attributes: &Attributes) {
        self.new_part();
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
        let measure = std::mem::replace(&mut self.measure, MeasureBuilt::new(0));
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
        let measure = std::mem::replace(&mut self.measure, MeasureBuilt::new(number));
        self.part()?.append(PartItem::Measure(measure));
        Ok(())
    }

    fn bar(&mut self, attributes: &Attributes) -> Result<()> {
        self.accidentals.clear();
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
        self.measure.append(
            StreamElement::RepeatExpression(RepeatExpression::new(kind)),
            None,
        );
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
        self.measure.append(StreamElement::Clef(clef), None);
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
        self.measure
            .append(StreamElement::KeySignature(self.key.clone()), None);
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

    /// music21's `setDurationForObject`: the length a `Dur` value gives,
    /// and the slur it begins or ends.
    fn duration(&mut self, written: &str, serial: usize) -> Result<Duration> {
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
        let mut slurred = false;
        for part in written.split(',') {
            match part {
                "Grace" => duration = duration.grace_duration(),
                "Slur" => {
                    if !self.within_slur {
                        self.slur_start = Some(serial);
                    }
                    slurred = true;
                }
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
        if self.within_slur && !slurred {
            if let (Some(start), Some(part)) = (self.slur_start, self.part.as_mut()) {
                part.slurs.push((start, serial));
            }
            self.within_slur = false;
        } else {
            self.within_slur = slurred;
        }
        Ok(duration)
    }

    /// music21's `setTieFromPitchInfo`: whether a note begins or ends a tie.
    fn tie(&mut self, position: &str) -> Option<Tie> {
        if position.ends_with('^') {
            let begins = !self.within_tie;
            self.within_tie = true;
            return begins.then(|| Tie::new(TieType::Start));
        }
        if self.within_tie {
            self.within_tie = false;
            return Some(Tie::new(TieType::Stop));
        }
        None
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
        let serial = self.serial();
        let duration = self.duration(written, serial)?;
        let tie = self.tie(position);
        let lyric = self.lyric().map(str::to_string);
        let element = if self.clef == ClefState::Percussion {
            let octave = pitch.octave().unwrap_or(4);
            let step = pitch.step().as_char();
            let mut stroke = Unpitched::displayed_at(step, octave)?;
            let note = stroke.written_mut();
            note.set_duration(duration);
            if tie.is_some() {
                note.set_tie(tie);
            }
            if let Some(lyric) = lyric {
                note.add_lyric(&lyric, None, false)?;
            }
            StreamElement::Unpitched(stroke)
        } else {
            let mut note = Note::from_pitch(pitch).with_duration(duration);
            if tie.is_some() {
                note.set_tie(tie);
            }
            if let Some(lyric) = lyric {
                note.add_lyric(&lyric, None, false)?;
            }
            StreamElement::Note(note)
        };
        self.measure.append_shortest(element, Some(serial));
        Ok(())
    }

    fn chord(&mut self, attributes: &Attributes) -> Result<()> {
        let durations = attributes.list("Dur");
        let positions = attributes.list("Pos");
        let length = if self.measure.has_voices() {
            sorted_places(&self.measure.items)
                .into_iter()
                .filter_map(|place| match &self.measure.items[place].content {
                    Content::Voice(voice) => Some(highest_time(&voice.items)),
                    Content::Element(..) => None,
                })
                .fold(FloatType::INFINITY, FloatType::min)
        } else {
            highest_time(&self.measure.items)
        };
        let rest_beside = attributes.get("Dur2");
        let mut voice = 0;
        for written in &durations {
            let serial = self.serial();
            let duration = self.duration(written, serial)?;
            // music21 takes the position of the first equal duration.
            let index = durations
                .iter()
                .position(|other| other == written)
                .unwrap_or(0);
            let position = positions
                .get(index)
                .ok_or_else(|| error("a chord with no Pos"))?
                .clone();
            let mut pitches = Vec::new();
            for one in position.split(',') {
                pitches.push(self.pitch(one)?);
            }
            let notes: Vec<Note> = pitches.into_iter().map(Note::from_pitch).collect();
            let mut chord = Chord::new(notes)?.with_duration(duration);
            if let Some(tie) = self.tie(&position) {
                for note in chord.notes_mut() {
                    note.set_tie(Some(tie.clone()));
                }
            }
            if let Some(lyric) = self.lyric().map(str::to_string) {
                chord.add_lyric(&lyric, None, false)?;
            }
            if durations.len() == 1 {
                self.measure
                    .append_shortest(StreamElement::Chord(chord), Some(serial));
            } else {
                let place = self.measure.voice_at(voice, length);
                if let Content::Voice(built) = &mut self.measure.items[place].content {
                    append_to_voice(built, StreamElement::Chord(chord), Some(serial));
                }
            }
            voice += 1;
        }
        if let Some(written) = rest_beside {
            let serial = self.serial();
            let mut rest = Rest::new(Duration::quarter());
            rest.set_step_shift(3);
            let duration = self.duration(written, serial)?;
            rest.set_duration(duration);
            let place = self.measure.voice_at(voice, length);
            if let Content::Voice(built) = &mut self.measure.items[place].content {
                append_to_voice(built, StreamElement::Rest(rest), Some(serial));
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Stream> {
        let measure = std::mem::replace(&mut self.measure, MeasureBuilt::new(0));
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
}
