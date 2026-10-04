//! Reading MusicXML: music21's `musicxml.xmlToM21`.
//!
//! The document is read measure by measure into a list of elements, each
//! with its offset, the voice and staff it was written in and the order it
//! was met in, and the score is put together from those at the end -- which
//! is where music21 sorts its streams and splits a part written on several
//! staves into one part for each.
//!
//! What the crate does not model is passed over as music21's reader passes
//! over what it does not know: page layout, fonts, positions, credits.

use crate::articulations::{Articulation, ArticulationKind, Finger};
use crate::bar::{Barline, BarlineType, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::{ChordStepModification, ChordSymbol};
use crate::clef::Clef;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::{
    Duration, DurationType, Grace, Tuplet, TupletBracket, TupletShow, TupletType,
};
use crate::dynamics::Dynamic;
use crate::error::{Error, Result};
use crate::expressions::{
    ArpeggioType, Expression, Fermata, FermataType, Ornament, OrnamentDelay, OrnamentKind,
    RehearsalMark, TextExpression,
};
use crate::instrument::{Instrument, SearchLanguage};
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::metadata::{Metadata, MetadataValue};
use crate::meter::TimeSignature;
use crate::notation::{
    Beam, BeamDirection, BeamType, Beams, Justification, Lyric, NoteSize, Notehead, Placement,
    StemDirection, Syllabic, Tie, TieStyle, TieType,
};
use crate::note::Note;
use crate::percussion::{PercussionChord, PercussionNote, Unpitched};
use crate::pitch::{Accidental, Pitch};
use crate::repeat::{RepeatExpression, RepeatExpressionKind};
use crate::rest::Rest;
use crate::spanner::{
    Glissando, LineEnd, LineEnds, OctaveShift, Pedal, PedalForm, PedalObject, PedalObjectKind,
    PedalType, SlideType, Spanner, SpannerKind,
};
use crate::stream::{BarTogether, StaffGroup, Stream, StreamElement, StreamEvent, StreamKind};
use crate::tempo::{MetricModulation, MetronomeMark};
use crate::volume::Volume;
use crate::xml::Xml;

/// What may be drawn round a mark: music21's `style.Enclosure`.
const ENCLOSURES: [&str; 15] = [
    "rectangle",
    "square",
    "oval",
    "circle",
    "bracket",
    "triangle",
    "diamond",
    "pentagon",
    "hexagon",
    "heptagon",
    "octagon",
    "nonagon",
    "decagon",
    "inverted-bracket",
    "none",
];

/// The staff an element is written on where nothing says: music21's
/// `NO_STAFF_ASSIGNED`.
const NO_STAFF: i32 = 0;

fn import_error(message: impl Into<String>) -> Error {
    Error::MusicXml(message.into())
}

/// Reads a MusicXML document -- the text of a `.musicxml` file, or of the
/// score inside a compressed `.mxl` -- into a score: music21's
/// `converter.parse` for the format.
///
/// ```
/// use music21_rs::musicxml::from_musicxml;
///
/// let score = from_musicxml(
///     "<score-partwise><part-list><score-part id=\"P1\"><part-name>Flute</part-name>\
///      </score-part></part-list><part id=\"P1\"><measure number=\"1\">\
///      <attributes><divisions>1</divisions></attributes>\
///      <note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration>\
///      <type>whole</type></note></measure></part></score-partwise>",
/// )?;
/// let part = score.parts()[0];
/// assert_eq!(part.name(), Some("Flute"));
/// assert_eq!(part.measures()[0].pitches()[0].name_with_octave(), "C4");
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn from_musicxml(document: &str) -> Result<Stream> {
    let root = Xml::parse(document)?;
    if root.tag != "score-partwise" {
        return Err(import_error(format!(
            "only a score-partwise document can be read, not <{}>",
            root.tag
        )));
    }
    Importer::default().score(&root)
}

// ------------------------------------------------------------------ numbers

/// A number snapped to the nearest fraction music21 would hold it as:
/// `opFrac`, which keeps a float whose denominator is a small power of two
/// and otherwise takes the closest fraction with a denominator up to 65535.
fn op_frac(value: FloatType) -> FloatType {
    if (value * 32768.0).fract() == 0.0 {
        return value;
    }
    let negative = value < 0.0;
    let target = value.abs();
    // Continued fractions, stopping at the last convergent under the limit.
    let (mut p0, mut q0, mut p1, mut q1) = (0i64, 1i64, 1i64, 0i64);
    let mut x = target;
    let (mut best_p, mut best_q) = (target.round() as i64, 1i64);
    for _ in 0..40 {
        let a = x.floor();
        let (p2, q2) = (a as i64 * p1 + p0, a as i64 * q1 + q0);
        if q2 > 65535 {
            // The best semiconvergent under the limit, as Python's
            // `limit_denominator` takes it.
            let k = (65535 - q0) / q1.max(1);
            let (sp, sq) = (p0 + k * p1, q0 + k * q1);
            let semi = (sp as FloatType / sq as FloatType - target).abs();
            let conv = (p1 as FloatType / q1 as FloatType - target).abs();
            (best_p, best_q) = if semi < conv { (sp, sq) } else { (p1, q1) };
            break;
        }
        (p0, q0, p1, q1) = (p1, q1, p2, q2);
        (best_p, best_q) = (p1, q1);
        let rest = x - a;
        if rest.abs() < 1e-12 {
            break;
        }
        x = 1.0 / rest;
    }
    let snapped = best_p as FloatType / best_q as FloatType;
    if negative { -snapped } else { snapped }
}

fn parse_float(text: &str) -> Option<FloatType> {
    text.trim().parse::<FloatType>().ok()
}

fn parse_int(text: &str) -> Option<IntegerType> {
    let text = text.trim();
    text.parse::<IntegerType>().ok().or_else(|| {
        // Python's `int(float_text)` is not accepted by music21 either, but
        // a whole number written with a point is common in the wild.
        text.parse::<FloatType>()
            .ok()
            .filter(|value| value.fract() == 0.0)
            .map(|value| value as IntegerType)
    })
}

/// How many strokes a `<tremolo>` says, three where it says nothing a
/// number can be read from: music21's `xmlToTremolo`.
fn tremolo_marks(mark: &Xml) -> Result<u8> {
    let marks = parse_int(mark.stripped()).unwrap_or(3);
    u8::try_from(marks)
        .ok()
        .filter(|marks| *marks <= 8)
        .ok_or_else(|| import_error("Number of marks must be a number from 0 to 8"))
}

fn yes(value: Option<&str>) -> bool {
    value == Some("yes")
}

fn placement_of(element: &Xml) -> Option<Placement> {
    element
        .get("placement")
        .and_then(|name| Placement::from_name(name).ok())
}

/// music21's `musicXMLTypeToType`.
fn duration_type(name: &str) -> Result<DurationType> {
    let name = match name {
        "long" => "longa",
        "32th" => "32nd",
        other => other,
    };
    DurationType::from_music21_name(name)
        .ok_or_else(|| import_error(format!("found unknown MusicXML type: {name}")))
}

// ------------------------------------------------------------ what is read

/// One element of a measure as it was read.
#[derive(Clone, Debug)]
struct Item {
    offset: FloatType,
    /// The order it was put into its stream, which settles the order of
    /// two elements music21 sorts alike.
    seq: usize,
    staff: i32,
    /// What spanners know it by; nothing for a copy.
    uid: Option<usize>,
    element: StreamElement,
}

impl Item {
    /// music21's `classSortOrder`.
    fn order(&self) -> i32 {
        match &self.element {
            StreamElement::TextExpression(_) | StreamElement::RehearsalMark(_) => -30,
            StreamElement::Instrument(_) => -25,
            StreamElement::Stream(stream) if stream.kind() == StreamKind::Voice => 5,
            StreamElement::Stream(_) => -20,
            StreamElement::Barline(_) => -5,
            StreamElement::Clef(_) => 0,
            StreamElement::MetronomeMark(_)
            | StreamElement::TempoText(_)
            | StreamElement::MetricModulation(_) => 1,
            StreamElement::KeySignature(_) | StreamElement::Key(_) => 2,
            StreamElement::TimeSignature(_) => 4,
            StreamElement::Dynamic(_) => 10,
            StreamElement::ChordSymbol(_) => 19,
            _ => 20,
        }
    }

    fn is_grace(&self) -> bool {
        self.element.duration().is_some_and(Duration::is_grace)
    }

    /// Whether it lasts, as a note, chord or rest does: music21's
    /// `GeneralNote` that is not a chord symbol.
    fn is_note_or_rest(&self) -> bool {
        matches!(
            self.element,
            StreamElement::Note(_)
                | StreamElement::Chord(_)
                | StreamElement::Rest(_)
                | StreamElement::Unpitched(_)
                | StreamElement::PercussionChord(_)
        )
    }
}

/// Elements in the order music21 sorts a stream into: by offset, then by
/// class, grace notes ahead of the note they lean on, then as inserted.
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
    padding_left: FloatType,
    padding_right: FloatType,
    items: Vec<Item>,
    voices: Vec<VoiceIr>,
}

impl MeasureIr {
    /// Where the last thing in the measure ends: music21's `highestTime`.
    fn highest_time(&self) -> FloatType {
        let end = |item: &Item| item.offset + item.element.quarter_length();
        self.items
            .iter()
            .chain(self.voices.iter().flat_map(|voice| &voice.items))
            .map(end)
            .fold(0.0, FloatType::max)
    }

    /// Where the last thing standing in the measure itself ends, its voices
    /// counted as empty: what music21's `highestTime` answers while the
    /// measure is still being read, each voice's length having been worked
    /// out before anything was put in it.
    fn highest_time_outside_voices(&self) -> FloatType {
        self.items
            .iter()
            .map(|item| item.offset + item.element.quarter_length())
            .fold(0.0, FloatType::max)
    }
}

/// A spanner while its ends are still being met.
#[derive(Clone, Debug)]
struct Building {
    kind: SpannerKind,
    id_local: Option<String>,
    complete: bool,
    uids: Vec<usize>,
    placement: Option<Placement>,
    line_type: Option<String>,
    ends: LineEnds,
    shift: Option<OctaveShift>,
    pedal: Pedal,
    arpeggio: ArpeggioType,
    slide: SlideType,
    marks: u8,
    /// Whether it has been put into a part already.
    placed: bool,
}

impl Building {
    fn new(kind: SpannerKind, id_local: Option<&str>) -> Self {
        Self {
            kind,
            id_local: id_local.map(str::to_string),
            complete: false,
            uids: Vec::new(),
            placement: match kind {
                SpannerKind::Crescendo | SpannerKind::Diminuendo => Some(Placement::Below),
                SpannerKind::Ottava | SpannerKind::Line => Some(Placement::Above),
                _ => None,
            },
            line_type: (kind == SpannerKind::Glissando).then(|| "wavy".to_string()),
            ends: LineEnds::default(),
            shift: None,
            pedal: Pedal::default(),
            arpeggio: ArpeggioType::Normal,
            slide: SlideType::Chromatic,
            marks: 3,
            placed: false,
        }
    }

    fn add(&mut self, uid: usize) {
        if !self.uids.contains(&uid) {
            self.uids.push(uid);
        }
    }

    /// music21's `insertFirstSpannedElement`.
    fn add_first(&mut self, uid: usize) {
        if !self.uids.contains(&uid) {
            self.uids.insert(0, uid);
        }
    }

    /// The finished spanner, its elements named by where they stand.
    fn finish(&self, position_of: &dyn Fn(usize) -> Option<usize>) -> Spanner {
        let positions: Vec<Option<usize>> = self.uids.iter().map(|uid| position_of(*uid)).collect();
        let mut spanner = match self.kind {
            SpannerKind::Crescendo | SpannerKind::Diminuendo => {
                let mut wedge = Spanner::wedge(self.kind, Vec::new());
                wedge = with_positions(wedge, positions);
                wedge
            }
            SpannerKind::Line => {
                let mut line = with_positions(Spanner::line(Vec::new()), positions);
                line.set_ends(self.ends);
                line
            }
            _ => Spanner::with_unplaced(self.kind, positions),
        };
        spanner.set_placement(self.placement);
        if self.kind != SpannerKind::Crescendo && self.kind != SpannerKind::Diminuendo {
            spanner.set_line_type(self.line_type.clone());
        }
        if self.kind == SpannerKind::Line {
            spanner.set_line_type(Some(
                self.line_type
                    .clone()
                    .unwrap_or_else(|| "solid".to_string()),
            ));
        }
        spanner.set_shift(self.shift);
        if self.kind == SpannerKind::PedalMark {
            spanner.set_pedal(Some(self.pedal));
        }
        if self.kind == SpannerKind::Glissando {
            spanner.set_glissando_details(Some(Glissando {
                slide_type: self.slide,
                label: None,
            }));
        }
        if self.kind == SpannerKind::TremoloSpanner {
            // Read already within the range a tremolo allows.
            let _ = spanner.set_number_of_marks(Some(self.marks));
        }
        spanner.set_arpeggio(self.arpeggio);
        spanner
    }
}

/// A spanner of the same kind and drawing over other positions.
fn with_positions(template: Spanner, positions: Vec<Option<usize>>) -> Spanner {
    let mut spanner = Spanner::with_unplaced(template.kind(), positions);
    spanner.set_placement(template.placement());
    spanner.set_line_type(template.line_type().map(str::to_string));
    spanner.set_spread(template.spread());
    spanner
}

/// A first or second ending while its measures are still being met:
/// music21's `RepeatBracket`.
#[derive(Clone, Debug)]
struct Bracket {
    numbers: Vec<u32>,
    measures: Vec<usize>,
    complete: bool,
}

/// music21's `RepeatBracket.number` setter: nothing at all is nought,
/// which is written as no number; otherwise `1`, `1, 2` or `1-3`.
fn bracket_numbers(text: &str) -> Vec<u32> {
    if text.is_empty() {
        return vec![0];
    }
    let number = |piece: &str| piece.trim().parse::<u32>().ok();
    let read = if let Some((low, high)) = text.split_once('-') {
        number(low)
            .zip(number(high))
            .map(|(low, high)| (low..=high).collect())
    } else if text.contains(',') {
        text.split(',').map(number).collect::<Option<Vec<u32>>>()
    } else {
        number(text).map(|number| vec![number])
    };
    // music21 falls back to a first ending where it cannot read one.
    read.unwrap_or_else(|| vec![1])
}

// ----------------------------------------------------------------- importer

#[derive(Default)]
struct Importer {
    spanners: Vec<Building>,
    /// The spanners waiting for the next note, chord or rest, which becomes
    /// the first thing the one at the front joins.
    pending: std::collections::VecDeque<usize>,
    /// Spanners in the order music21's score holds them once it is read, by
    /// where each stands in `spanners`: one finished in a part may still be
    /// handed the first note of the next by the queue.
    finished: Vec<usize>,
    next_uid: usize,
    finale: bool,
}

struct PartGroup<'a> {
    element: &'a Xml,
    number: IntegerType,
    part_ids: Vec<String>,
}

/// A part as it goes into the score, with the uid of each leaf in the order
/// the score's leaves run.
struct BuiltPart {
    /// The id the part list knows it by, a staff of a part written on
    /// several being `<part>-Staff<n>`.
    key: String,
    stream: Stream,
    uids: Vec<Option<usize>>,
}

impl Importer {
    fn uid(&mut self) -> usize {
        self.next_uid += 1;
        self.next_uid
    }

    /// music21's `xmlRootToScore`.
    fn score(mut self, root: &Xml) -> Result<Stream> {
        let metadata = self.metadata(root);
        let (score_parts, groups) = part_list(root);

        let mut built: Vec<BuiltPart> = Vec::new();
        let mut staff_groups: Vec<StaffGroup> = Vec::new();
        for part in root.find_all("part") {
            let id = match part.get("id") {
                Some(id) => id.to_string(),
                None => match score_parts.first() {
                    Some((id, _)) => id.clone(),
                    None => continue,
                },
            };
            let Some((_, score_part)) = score_parts.iter().find(|(known, _)| *known == id) else {
                continue;
            };
            let first = built.len();
            let staves = PartParser::new(&mut self, part, score_part, &id).parse()?;
            let name = staves
                .first()
                .and_then(|staff| staff.stream.name().map(str::to_string));
            let count = staves.len();
            let split = staves
                .first()
                .is_some_and(|staff| staff.stream.kind() == StreamKind::PartStaff && count > 0)
                && staves.iter().all(|staff| staff.key != id);
            built.extend(staves);
            if split {
                let mut group = StaffGroup::new((first..first + count).collect());
                group.set_name(name);
                group.set_symbol(Some("brace"))?;
                group.set_bar_together(Some(BarTogether::Yes));
                group.set_name_hidden(true);
                staff_groups.push(group);
            }
        }

        // music21's `partGroups`.
        for group in &groups {
            let mut positions = Vec::new();
            for part_id in &group.part_ids {
                match built.iter().position(|part| part.key == *part_id) {
                    Some(position) => positions.push(position),
                    None => {
                        let prefix = format!("{part_id}-Staff");
                        let mut staves: Vec<(usize, &str)> = built
                            .iter()
                            .enumerate()
                            .filter(|(_, part)| part.key.starts_with(&prefix))
                            .map(|(position, part)| (position, part.key.as_str()))
                            .collect();
                        staves.sort_by(|left, right| left.1.cmp(right.1));
                        if staves.is_empty() {
                            return Err(import_error(format!(
                                "Cannot find part in m21PartObjectsById dictionary by Id: {part_id}"
                            )));
                        }
                        positions.extend(staves.into_iter().map(|(position, _)| position));
                    }
                }
            }
            let mut read = StaffGroup::new(positions);
            let text = |tag: &str| {
                group
                    .element
                    .find(tag)
                    .and_then(Xml::text)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
            };
            read.set_name(text("group-name"));
            read.set_abbreviation(text("group-abbreviation"));
            match group.element.find("group-symbol") {
                Some(_) => {
                    if let Some(symbol) = text("group-symbol") {
                        read.set_symbol(Some(&symbol))?;
                    }
                }
                None => read.set_symbol(Some("brace"))?,
            }
            read.set_bar_together(Some(match text("group-barline").as_deref() {
                Some("no") => BarTogether::No,
                Some("Mensurstrich") => BarTogether::Mensurstrich,
                _ => BarTogether::Yes,
            }));
            staff_groups.push(read);
        }

        // Whatever was completed and never put into a part goes into the
        // score, an arpeggio across parts among them.
        for building in &mut self.spanners {
            if building.kind == SpannerKind::ArpeggioMark {
                building.complete = true;
            }
        }
        let late: Vec<usize> = (0..self.spanners.len())
            .filter(|index| self.spanners[*index].complete && !self.spanners[*index].placed)
            .collect();
        self.finished.extend(late);

        let mut uids: Vec<Option<usize>> = Vec::new();
        let mut events = Vec::new();
        for part in built {
            uids.extend(part.uids);
            events.push(StreamEvent::new(0.0, part.stream));
        }
        let mut assembled = Stream::from_events(events);
        assembled.set_kind(StreamKind::Score);
        assembled.set_metadata(Some(metadata));
        for group in staff_groups {
            assembled.add_staff_group(group);
        }
        let position_of = |uid: usize| uids.iter().position(|held| *held == Some(uid));
        for index in &self.finished {
            assembled.add_spanner(self.spanners[*index].finish(&position_of));
        }
        Ok(assembled)
    }

    /// music21's `xmlMetadata`.
    fn metadata(&mut self, root: &Xml) -> Metadata {
        let mut metadata = Metadata::new();
        let add = |metadata: &mut Metadata, element: &Xml, tag: &str, name: &str| {
            if let Some(text) = element.find(tag).and_then(Xml::text)
                && !text.is_empty()
            {
                metadata.add_text(name, text);
            }
        };
        if let Some(work) = root.find("work") {
            add(&mut metadata, work, "work-title", "title");
            add(&mut metadata, work, "work-number", "number");
            add(&mut metadata, work, "opus", "opusNumber");
        }
        add(&mut metadata, root, "movement-number", "movementNumber");
        add(&mut metadata, root, "movement-title", "movementName");
        // A title that only repeats the movement name was put there by a
        // writer for the sake of renderers, and is taken out again.
        let texts = |metadata: &Metadata, name: &str| -> Vec<String> {
            metadata
                .get(name)
                .iter()
                .map(|value| value.text().to_string())
                .collect()
        };
        if texts(&metadata, "title") == texts(&metadata, "movementName") {
            metadata.set("title", Vec::new());
        }

        let Some(identification) = root.find("identification") else {
            return metadata;
        };
        for creator in identification.find_all("creator") {
            let name = creator.stripped();
            match creator.get("type") {
                Some(role) if crate::metadata::is_contributor_unique_name(role) => {
                    metadata.add(role, MetadataValue::with_role(name, role));
                }
                Some(role) => {
                    metadata.add("otherContributor", MetadataValue::with_role(name, role));
                }
                None => metadata.add("otherContributor", MetadataValue::new(name)),
            }
        }
        if let Some(rights) = identification.find("rights") {
            // music21 writes a copyright that says nothing as `None`.
            let text = match rights.text() {
                Some(text) => text.trim(),
                None => "None",
            };
            metadata.add(
                "copyright",
                match rights.get("type") {
                    Some(role) => MetadataValue::with_role(text, role),
                    None => MetadataValue::new(text),
                },
            );
        }
        if let Some(encoding) = identification.find("encoding") {
            let mut found = false;
            for software in encoding.find_all("software") {
                let text = software.stripped();
                if text.is_empty() {
                    continue;
                }
                if !found && text.contains("Finale") {
                    self.finale = true;
                }
                found = true;
                metadata.add_text("software", text);
            }
        }
        if let Some(miscellaneous) = identification.find("miscellaneous") {
            for field in miscellaneous.find_all("miscellaneous-field") {
                let Some(name) = field.get("name") else {
                    continue;
                };
                let value = field.text().unwrap_or("");
                // A namespaced name is kept under the unique name it has.
                let unique = crate::metadata::STANDARD_PROPERTIES
                    .iter()
                    .find(|(unique, namespaced, _)| *unique == name || *namespaced == name)
                    .map_or(name, |(unique, _, _)| unique);
                metadata.add_text(unique, value);
            }
        }
        metadata
    }
}

/// music21's `parsePartList`: each `<score-part>` by id, and the groups.
fn part_list(root: &Xml) -> (Vec<(String, &Xml)>, Vec<PartGroup<'_>>) {
    let mut score_parts = Vec::new();
    let mut groups: Vec<PartGroup<'_>> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let Some(list) = root.find("part-list") else {
        return (score_parts, groups);
    };
    for element in &list.children {
        match element.tag.as_str() {
            "score-part" => {
                let id = element.get("id").unwrap_or_default().to_string();
                for index in &open {
                    groups[*index].part_ids.push(id.clone());
                }
                score_parts.push((id, element));
            }
            "part-group" => {
                let number = element.get("number").and_then(parse_int).unwrap_or(1);
                match element.get("type") {
                    Some("start") => {
                        open.push(groups.len());
                        groups.push(PartGroup {
                            element,
                            number,
                            part_ids: Vec::new(),
                        });
                    }
                    Some("stop") => open.retain(|index| groups[*index].number != number),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    (score_parts, groups)
}

// --------------------------------------------------------------------- part

/// music21's `PartParser`.
struct PartParser<'a, 'x> {
    importer: &'a mut Importer,
    part: &'x Xml,
    score_part: &'x Xml,
    id: String,
    name: Option<String>,
    abbreviation: Option<String>,
    name_hidden: bool,
    abbreviation_hidden: bool,
    /// The part's instruments with where each starts.
    instruments: Vec<(FloatType, Instrument)>,
    measures: Vec<MeasureIr>,
    brackets: Vec<Bracket>,
    multi_staff: bool,
    max_staves: i32,
    last_divisions: FloatType,
    last_meter: Option<TimeSignature>,
    last_measure_was_short: bool,
    last_measure_offset: FloatType,
    last_measure_number: IntegerType,
    last_number_suffix: Option<String>,
    last_clefs: Vec<(i32, Clef)>,
    active_tuplets: [Option<Tuplet>; 7],
    first_measure_parsed: bool,
}

impl<'a, 'x> PartParser<'a, 'x> {
    fn new(importer: &'a mut Importer, part: &'x Xml, score_part: &'x Xml, id: &str) -> Self {
        let multi_staff = part.find_all("measure").any(|measure| {
            measure.find_all("attributes").any(|attributes| {
                attributes
                    .find_all("staves")
                    .any(|staves| parse_int(staves.stripped()).is_some_and(|count| count > 1))
            })
        });
        Self {
            importer,
            part,
            score_part,
            id: id.to_string(),
            name: None,
            abbreviation: None,
            name_hidden: false,
            abbreviation_hidden: false,
            instruments: Vec::new(),
            measures: Vec::new(),
            brackets: Vec::new(),
            multi_staff,
            max_staves: 1,
            last_divisions: 10080.0,
            last_meter: None,
            last_measure_was_short: false,
            last_measure_offset: 0.0,
            last_measure_number: 0,
            last_number_suffix: None,
            last_clefs: Vec::new(),
            active_tuplets: Default::default(),
            first_measure_parsed: false,
        }
    }

    /// music21's `PartParser.parse`: the part, or one part for each of its
    /// staves.
    fn parse(mut self) -> Result<Vec<BuiltPart>> {
        self.score_part_info()?;
        let part = self.part;
        let mut last_forward: Option<(usize, bool)> = None;
        for measure in part.find_all("measure") {
            last_forward = self.measure(measure)?;
        }
        self.remove_finale_ending_rest(last_forward);

        // Endings, from the brackets over the measures.
        for bracket in &self.brackets {
            for (place, index) in bracket.measures.iter().enumerate() {
                let measure = &mut self.measures[*index];
                if measure.ending.is_none() {
                    measure.ending = Some(Ending::new(
                        bracket.numbers.clone(),
                        place == 0,
                        place + 1 == bracket.measures.len(),
                    ));
                }
            }
        }

        // Spanners completed by now belong to this part, octave lines last.
        let mut ours: Vec<usize> = Vec::new();
        for pass in [false, true] {
            for (index, building) in self.importer.spanners.iter_mut().enumerate() {
                let ottava = building.kind == SpannerKind::Ottava;
                if building.complete && !building.placed && ottava == pass {
                    building.placed = true;
                    ours.push(index);
                }
            }
        }
        self.importer.finished.extend(ours);

        if self.max_staves > 1 {
            self.separate_staves()
        } else {
            let kind = if self.multi_staff {
                StreamKind::PartStaff
            } else {
                StreamKind::Part
            };
            let measures = std::mem::take(&mut self.measures);
            let built = self.build(kind, None, measures, &self.instruments.clone());
            Ok(vec![BuiltPart {
                key: self.id.clone(),
                stream: built.0,
                uids: built.1,
            }])
        }
    }

    /// Puts a part together out of its measures.
    fn build(
        &self,
        kind: StreamKind,
        id: Option<String>,
        measures: Vec<MeasureIr>,
        instruments: &[(FloatType, Instrument)],
    ) -> (Stream, Vec<Option<usize>>) {
        let mut items: Vec<(Item, Vec<Option<usize>>)> = Vec::new();
        let mut seq = 0;
        for (offset, instrument) in instruments {
            items.push((
                Item {
                    offset: *offset,
                    seq,
                    staff: NO_STAFF,
                    uid: None,
                    element: instrument.clone().into(),
                },
                vec![None],
            ));
            seq += 1;
        }
        for measure in measures {
            let offset = measure.offset;
            let (stream, uids) = build_measure(measure);
            items.push((
                Item {
                    offset,
                    seq,
                    staff: NO_STAFF,
                    uid: None,
                    element: stream.into(),
                },
                uids,
            ));
            seq += 1;
        }
        items.sort_by(|(left, _), (right, _)| {
            left.offset
                .total_cmp(&right.offset)
                .then(left.order().cmp(&right.order()))
                .then(left.seq.cmp(&right.seq))
        });
        let mut uids = Vec::new();
        let mut events = Vec::new();
        for (item, leaf_uids) in items {
            uids.extend(leaf_uids);
            events.push(StreamEvent::new(item.offset, item.element));
        }
        let mut stream = Stream::from_events(events);
        stream.set_kind(kind);
        stream.set_id(id.or_else(|| {
            self.instruments
                .first()
                .and_then(|(_, instrument)| instrument.best_name().map(str::to_string))
        }));
        stream.set_name(self.name.clone());
        stream.set_abbreviation(self.abbreviation.clone());
        // A staff split off a part is a fresh stream, which prints both.
        if kind != StreamKind::PartStaff || self.max_staves <= 1 {
            stream.set_name_hidden(self.name_hidden);
            stream.set_abbreviation_hidden(self.abbreviation_hidden);
        }
        (stream, uids)
    }

    /// music21's `parseXmlScorePart` and `getDefaultInstrument`.
    fn score_part_info(&mut self) -> Result<()> {
        let clean = |text: &str| text.trim().replace('\n', " ");
        let score_part = self.score_part;
        let named = |tag: &str| {
            score_part
                .find(tag)
                .and_then(Xml::text)
                .filter(|text| !text.is_empty())
                .map(clean)
        };
        let part_name = named("part-name");
        let part_abbreviation = named("part-abbreviation");
        let name_hidden = score_part
            .find("part-name")
            .is_some_and(|name| name.get("print-object") == Some("no"));
        let abbreviation_hidden = score_part
            .find("part-abbreviation")
            .is_some_and(|name| name.get("print-object") == Some("no"));

        let adjusted =
            |text: &str| -> Option<u8> { parse_int(text).map(|value| (value - 1).max(0) as u8) };
        let mut instrument: Option<Instrument> = None;
        let mut unpitched = false;
        if let Some(midi) = score_part.find("midi-instrument") {
            let unpitched_text = midi.child_text("midi-unpitched");
            let program_text = midi.child_text("midi-program");
            if !unpitched_text.is_empty() {
                unpitched = true;
                let pitch = adjusted(unpitched_text).unwrap_or(0);
                instrument = Some(percussion_instrument(pitch));
            } else if !program_text.is_empty() {
                let mut read = adjusted(program_text)
                    .and_then(|program| Instrument::from_midi_program(program).ok())
                    .unwrap_or_default();
                if let Some(channel) = midi
                    .find("midi-channel")
                    .and_then(Xml::text)
                    .and_then(adjusted)
                {
                    read.set_midi_channel(Some(channel));
                }
                instrument = Some(read);
            }
        }
        let mut instrument = instrument.unwrap_or_default();
        let score_instrument = score_part.find("score-instrument");
        if let Some(score_instrument) = score_instrument
            && !unpitched
        {
            let name = score_instrument.child_text("instrument-name");
            if !name.is_empty() {
                let mut from_name =
                    Instrument::from_name(name, SearchLanguage::All).unwrap_or_default();
                from_name.set_midi_channel(instrument.midi_channel());
                if from_name.midi_program() == instrument.midi_program() || instrument.is_a("Piano")
                {
                    instrument = from_name;
                }
            }
        }
        instrument.set_part_id(Some(self.id.clone()));
        instrument.set_part_name(part_name.clone());
        instrument.set_part_abbreviation(part_abbreviation.clone());
        if let Some(score_instrument) = score_instrument {
            let named = |tag: &str| {
                score_instrument
                    .find(tag)
                    .and_then(Xml::text)
                    .filter(|text| !text.is_empty())
            };
            if let Some(name) = named("instrument-name") {
                instrument.set_name(Some(clean(name)));
            }
            if let Some(abbreviation) = named("instrument-abbreviation") {
                instrument.set_abbreviation(Some(clean(abbreviation)));
            }
            if let Some(sound) = named("instrument-sound") {
                instrument.set_sound(Some(sound.to_string()));
            }
        }
        self.name = part_name;
        self.abbreviation = part_abbreviation;
        self.name_hidden = name_hidden;
        self.abbreviation_hidden = abbreviation_hidden;
        self.instruments.push((0.0, instrument));
        Ok(())
    }

    /// music21's `xmlMeasureToMeasure`. Hands back where a rest a `<forward>`
    /// of Finale's made stands, when it is the last thing read.
    fn measure(&mut self, element: &'x Xml) -> Result<Option<(usize, bool)>> {
        let mut parser = MeasureParser::new(self, element);
        parser.parse()?;
        let MeasureParser {
            mut measure,
            staves,
            transposition,
            full_measure_rest,
            marked_full,
            finale_forward,
            use_voices,
            divisions,
            ..
        } = parser;
        self.last_divisions = divisions;
        self.max_staves = self.max_staves.max(staves);
        if let Some(transposition) = transposition {
            self.update_transposition(transposition);
        }
        self.first_measure_parsed = true;

        // music21's `setLastMeasureInfo`.
        if measure.number != self.last_measure_number {
            self.last_measure_number = measure.number;
            self.last_number_suffix = measure.suffix.clone();
        }
        let own_meter = measure.items.iter().find_map(|item| match &item.element {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        });
        if let Some(meter) = own_meter {
            self.last_meter = Some(meter);
        } else if self.last_meter.is_none() {
            self.last_meter = Some(TimeSignature::new(4, 4)?);
        }
        let bar_length = self
            .last_meter
            .as_ref()
            .map_or(4.0, TimeSignature::bar_quarter_length);

        if full_measure_rest {
            // The first rest as music21 walks the measure: what stands in
            // the measure itself in order, each voice read through where it
            // stands, which is at the start and after the signatures.
            let mut walk: Vec<(FloatType, i32, bool, usize, Option<usize>)> = Vec::new();
            for (index, voice) in measure.voices.iter().enumerate() {
                let first = sorted(voice.items.clone())
                    .into_iter()
                    .find(|item| matches!(item.element, StreamElement::Rest(_)))
                    .and_then(|item| item.uid);
                walk.push((0.0, 5, false, index, first));
            }
            for item in &measure.items {
                let rest = matches!(item.element, StreamElement::Rest(_));
                walk.push((
                    item.offset,
                    item.order(),
                    !item.is_grace(),
                    item.seq + measure.voices.len(),
                    item.uid.filter(|_| rest),
                ));
            }
            walk.sort_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then(left.1.cmp(&right.1))
                    .then(left.2.cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            });
            let first_uid = walk.into_iter().find_map(|entry| entry.4);
            let first = measure
                .items
                .iter_mut()
                .chain(measure.voices.iter_mut().flat_map(|voice| &mut voice.items))
                .filter(|item| first_uid.is_some() && item.uid == first_uid)
                .find_map(|item| match &mut item.element {
                    StreamElement::Rest(rest) => Some((item.uid, rest)),
                    _ => None,
                });
            if let Some((uid, rest)) = first {
                let marked_full = uid.is_some_and(|uid| marked_full.contains(&uid));
                let duration = rest.duration().clone();
                let whole = matches!(
                    duration.type_and_dots(),
                    Some((DurationType::Whole | DurationType::Breve, 0))
                ) && duration.tuplets().is_empty();
                if marked_full || (duration.quarter_length() != bar_length && whole) {
                    rest.set_duration(Duration::new(bar_length)?);
                }
            }
        }

        measure.offset = self.last_measure_offset;
        let index = self.measures.len();

        // music21's `adjustTimeAttributesFromMeasure`.
        let highest = measure.highest_time();
        let has_notes = measure
            .items
            .iter()
            .chain(measure.voices.iter().flat_map(|voice| &voice.items))
            .any(Item::is_note_or_rest);
        let shift = if highest == bar_length {
            highest
        } else if highest > bar_length {
            let difference = highest - bar_length;
            let near = |unit: FloatType| {
                let multiple = (difference / unit).round() * unit;
                (difference - multiple).abs() < 1e-6
            };
            if difference > 0.5 || near(0.0625) || near(1.0 / 12.0) {
                highest
            } else {
                bar_length
            }
        } else if highest == 0.0 && !has_notes {
            // Some writers leave the rest out of an empty measure.
            let uid = self.importer.uid();
            measure.items.push(Item {
                offset: 0.0,
                seq: usize::MAX,
                staff: NO_STAFF,
                uid: Some(uid),
                element: Rest::new(Duration::new(bar_length)?).into(),
            });
            self.last_measure_was_short = false;
            bar_length
        } else {
            // A short measure is a pickup where it opens the part or follows
            // another short one, and is cut short at its end otherwise.
            if self.last_measure_offset == 0.0 {
                if highest < bar_length {
                    measure.padding_left = bar_length - highest;
                }
            } else if self.last_measure_was_short {
                if highest < bar_length {
                    measure.padding_left = bar_length - highest;
                    self.last_measure_was_short = false;
                }
            } else {
                if highest < bar_length {
                    measure.padding_right = bar_length - highest;
                }
                self.last_measure_was_short = highest < bar_length;
            }
            highest
        };
        self.last_measure_offset += shift;
        self.measures.push(measure);
        Ok(finale_forward.map(|place| (index, place.is_some() && !use_voices)))
    }

    /// music21's `removeFinaleIncorrectEndingForwardRest`: a hidden rest
    /// Finale's `<forward>` left as the last thing in the last measure.
    fn remove_finale_ending_rest(&mut self, last_forward: Option<(usize, bool)>) {
        let Some((index, true)) = last_forward else {
            return;
        };
        let Some(measure) = self.measures.get_mut(index) else {
            return;
        };
        let last = sorted(measure.items.clone()).into_iter().rfind(|item| {
            item.is_note_or_rest() || matches!(item.element, StreamElement::ChordSymbol(_))
        });
        if let Some(last) = last
            && matches!(&last.element, StreamElement::Rest(rest) if rest.hidden())
        {
            measure.items.retain(|item| item.seq != last.seq);
        }
    }

    /// music21's `updateTransposition`.
    fn update_transposition(&mut self, transposition: Interval) {
        let active = self.instruments.last().cloned();
        match active {
            Some((_, instrument))
                if instrument.transposition().is_none() && !self.first_measure_parsed => {}
            Some((_, instrument)) => {
                let differs = instrument
                    .transposition()
                    .is_none_or(|held| held.directed_name() != transposition.directed_name());
                if differs {
                    self.instruments
                        .push((self.last_measure_offset, instrument.clone()));
                }
            }
            None => self
                .instruments
                .push((self.last_measure_offset, Instrument::new())),
        }
        if let Some((_, instrument)) = self.instruments.last_mut() {
            instrument.set_transposition(Some(transposition));
        }
    }

    /// music21's `separateOutPartStaves`: a part written on several staves
    /// becomes one part for each, what is written on no staff in particular
    /// going into every one.
    fn separate_staves(&mut self) -> Result<Vec<BuiltPart>> {
        let mut keys: Vec<i32> = Vec::new();
        for measure in &self.measures {
            for item in measure
                .items
                .iter()
                .chain(measure.voices.iter().flat_map(|voice| &voice.items))
            {
                if item.staff != NO_STAFF && !keys.contains(&item.staff) {
                    keys.push(item.staff);
                }
            }
        }
        keys.sort_unstable();

        let source = std::mem::take(&mut self.measures);
        let mut taken: Vec<usize> = Vec::new();
        let mut out = Vec::new();
        for key in keys {
            let mut measures = Vec::new();
            for measure in &source {
                let mut copy = MeasureIr {
                    offset: measure.offset,
                    number: measure.number,
                    suffix: measure.suffix.clone(),
                    // A measure made for a staff shows its number, whatever
                    // the one it was made from did.
                    hidden: false,
                    left: measure.left.clone(),
                    right: measure.right.clone(),
                    ending: measure.ending.clone(),
                    padding_left: measure.padding_left,
                    padding_right: measure.padding_right,
                    items: Vec::new(),
                    voices: Vec::new(),
                };
                let mut seq = 0usize;
                let mut take = |items: &[Item], into: &mut Vec<Item>, seq: &mut usize| {
                    for item in sorted(items.to_vec()) {
                        if item.staff != NO_STAFF && item.staff != key {
                            continue;
                        }
                        let mut item = item;
                        // The element itself goes to the first staff that
                        // takes it, and a copy to every one after.
                        if taken.contains(&item.seq_key()) {
                            item.uid = None;
                        } else {
                            taken.push(item.seq_key());
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
                // music21's `flattenUnnecessaryVoices`.
                copy.voices.retain(|voice| !voice.items.is_empty());
                if copy.voices.len() == 1 {
                    let voice = copy.voices.remove(0);
                    for mut item in sorted(voice.items) {
                        item.seq = seq;
                        seq += 1;
                        copy.items.push(item);
                    }
                }
                measures.push(copy);
            }
            let id = format!("{}-Staff{key}", self.id);
            let instruments = self.instruments.clone();
            let (stream, uids) = self.build(
                StreamKind::PartStaff,
                Some(id.clone()),
                measures,
                &instruments,
            );
            out.push(BuiltPart {
                key: id,
                stream,
                uids,
            });
        }
        Ok(out)
    }
}

impl Item {
    /// A number no two elements read share, whatever measure or voice they
    /// stand in: the uid where there is one.
    fn seq_key(&self) -> usize {
        self.uid.unwrap_or(usize::MAX)
    }
}

/// A measure put together: the stream, and the uid of each leaf in order.
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
                // Voices are made before anything is read into the measure.
                seq: index,
                staff: NO_STAFF,
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
    stream.set_padding_left(measure.padding_left);
    stream.set_padding_right(measure.padding_right);
    (stream, uids)
}

// ------------------------------------------------------------------ measure

/// music21's `MeasureParser`.
struct MeasureParser<'p, 'a, 'x> {
    part: &'p mut PartParser<'a, 'x>,
    element: &'x Xml,
    measure: MeasureIr,
    seq: usize,
    divisions: FloatType,
    staves: i32,
    transposition: Option<Interval>,
    use_voices: bool,
    last_voice: Option<String>,
    offset: FloatType,
    /// Notes of a chord being gathered, with the lyrics written on them.
    chord_notes: Vec<&'x Xml>,
    chord_lyrics: Vec<&'x Xml>,
    /// The uid of the last note, chord or rest read.
    last_note: Option<usize>,
    rests: usize,
    notes: usize,
    full_measure_rest: bool,
    /// The rests that said they fill their measure.
    marked_full: Vec<usize>,
    /// Whether the rest being read said so.
    marking_full: bool,
    /// Whether the last thing read was a rest made of Finale's `<forward>`;
    /// the inner value is its seq.
    finale_forward: Option<Option<usize>>,
    pedal_starts: Vec<(usize, FloatType)>,
}

impl<'p, 'a, 'x> MeasureParser<'p, 'a, 'x> {
    fn new(part: &'p mut PartParser<'a, 'x>, element: &'x Xml) -> Self {
        let divisions = part.last_divisions;
        Self {
            part,
            element,
            measure: MeasureIr::default(),
            seq: 0,
            divisions,
            staves: 1,
            transposition: None,
            use_voices: false,
            last_voice: None,
            offset: 0.0,
            chord_notes: Vec::new(),
            chord_lyrics: Vec::new(),
            last_note: None,
            rests: 0,
            notes: 0,
            full_measure_rest: false,
            marked_full: Vec::new(),
            marking_full: false,
            finale_forward: None,
            pedal_starts: Vec::new(),
        }
    }

    fn parse(&mut self) -> Result<()> {
        self.measure_attributes();
        self.voice_information();
        let element = self.element;
        for (index, child) in element.children.iter().enumerate() {
            match child.tag.as_str() {
                "note" => self.note(child, element.children.get(index + 1))?,
                "backup" => {
                    if let Some(ticks) = parse_float(child.child_text("duration")) {
                        self.offset = op_frac(self.offset - ticks / self.divisions).max(0.0);
                    }
                }
                "forward" => self.forward(child)?,
                "direction" => self.direction(child)?,
                "attributes" => self.attributes(child)?,
                "harmony" => self.harmony(child)?,
                "sound" => {
                    if child.get("tempo").is_some() {
                        let total = self.offset + self.offset_of(child);
                        self.sound_tempo(child, None, staff_number(child), total);
                    }
                }
                "barline" => self.barline(child)?,
                _ => {}
            }
        }
        if self.rests == 1 && self.notes == 0 {
            self.full_measure_rest = true;
        }
        Ok(())
    }

    /// Puts an element into the measure itself.
    fn insert(
        &mut self,
        offset: FloatType,
        staff: i32,
        element: impl Into<StreamElement>,
    ) -> usize {
        let uid = self.part.importer.uid();
        self.measure.items.push(Item {
            offset: op_frac(offset),
            seq: self.seq,
            staff,
            uid: Some(uid),
            element: element.into(),
        });
        self.seq += 1;
        uid
    }

    /// music21's `insertInMeasureOrVoice`, for a note, chord or rest.
    fn insert_note(&mut self, source: &Xml, staff: i32, element: StreamElement) -> usize {
        let uid = self.part.importer.uid();
        let item = Item {
            offset: self.offset,
            seq: self.seq,
            staff,
            uid: Some(uid),
            element,
        };
        self.seq += 1;
        if self.use_voices {
            let written = source.child_text("voice");
            let voice = if written.is_empty() {
                self.last_voice.clone().unwrap_or_else(|| "1".to_string())
            } else {
                self.last_voice = Some(written.to_string());
                written.to_string()
            };
            if let Some(found) = self.measure.voices.iter_mut().find(|held| held.id == voice) {
                found.items.push(item);
                return uid;
            }
        }
        self.measure.items.push(item);
        uid
    }

    /// music21's `parseMeasureAttributes` and `parseMeasureNumbers`.
    fn measure_attributes(&mut self) {
        self.measure.hidden = yes(self.element.get("implicit"));
        let raw = self.element.get("number").unwrap_or("");
        // music21's `getNumFromStr`: the digits, and whatever is left.
        let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
        let rest: String = raw.chars().filter(|c| !c.is_ascii_digit()).collect();
        if let Ok(number) = digits.parse::<IntegerType>() {
            self.measure.number = number;
        }
        if !rest.is_empty() {
            self.measure.suffix = Some(rest);
        }
        let last = self.part.last_measure_number;
        if self.measure.suffix.as_deref() == Some("X") && self.measure.number != last + 1 {
            let mut suffix = format!("X{}", self.measure.number);
            if let Some(before) = &self.part.last_number_suffix {
                suffix = format!("{before}{suffix}");
            }
            self.measure.number = last;
            self.measure.suffix = Some(suffix);
        }
    }

    /// music21's `updateVoiceInformation`: a measure naming more than one
    /// voice holds a voice for each.
    fn voice_information(&mut self) {
        let mut ids: Vec<String> = Vec::new();
        for tag in ["note", "forward"] {
            for element in self.element.find_all(tag) {
                let voice = element.child_text("voice");
                if !voice.is_empty() && !ids.iter().any(|held| held == voice) {
                    ids.push(voice.to_string());
                }
            }
        }
        if ids.len() > 1 {
            ids.sort();
            self.measure.voices = ids
                .into_iter()
                .map(|id| VoiceIr {
                    id,
                    items: Vec::new(),
                })
                .collect();
            self.use_voices = true;
        }
    }

    /// music21's `xmlToOffset`.
    fn offset_of(&self, element: &Xml) -> FloatType {
        parse_float(element.child_text("offset")).map_or(0.0, |ticks| ticks / self.divisions)
    }

    /// music21's `xmlForward`.
    fn forward(&mut self, element: &'x Xml) -> Result<()> {
        let Some(ticks) = parse_float(element.child_text("duration")) else {
            return Ok(());
        };
        let change = op_frac(ticks / self.divisions);
        if self.part.importer.finale {
            // Finale writes a hidden rest as a `<forward>`. One of no
            // length is a quarter rest: music21 takes a length of nought
            // for no length given.
            let length = if change == 0.0 { 1.0 } else { change };
            let mut rest = Rest::new(Duration::new(length)?);
            rest.set_hidden(true);
            let seq = self.seq;
            self.insert_note(element, staff_number(element), rest.into());
            self.finale_forward = Some(Some(seq));
        }
        self.offset = op_frac(self.offset + change);
        Ok(())
    }

    /// music21's `parseAttributesTag`.
    fn attributes(&mut self, attributes: &'x Xml) -> Result<()> {
        for child in &attributes.children {
            match child.tag.as_str() {
                "time" => {
                    if let Some(meter) = time_signature(child)? {
                        self.insert(self.offset, staff_number(child), meter);
                    }
                }
                "clef" => {
                    let clef = clef_of(child)?;
                    let staff = staff_number(child);
                    self.insert(self.offset, staff, clef.clone());
                    self.part.last_clefs.retain(|(held, _)| *held != staff);
                    self.part.last_clefs.push((staff, clef));
                }
                "key" => {
                    let key = key_signature(child)?;
                    self.insert(self.offset, staff_number(child), key);
                }
                "divisions" => {
                    if let Some(divisions) = parse_float(child.stripped()) {
                        self.divisions = op_frac(divisions);
                    }
                }
                "staves" => {
                    if let Some(staves) = parse_int(child.stripped()) {
                        self.staves = staves;
                    }
                }
                "transpose" => self.transposition = Some(transpose_interval(child)?),
                _ => {}
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------- notes

    /// music21's `xmlToNote`.
    fn note(&mut self, element: &'x Xml, next: Option<&'x Xml>) -> Result<()> {
        let next_is_chord =
            next.is_some_and(|next| next.tag == "note" && next.find("chord").is_some());
        let is_rest = element.find("rest").is_some();
        let mut is_chord = element.find("chord").is_some();
        if next_is_chord {
            is_chord = true;
            let voice = element.child_text("voice");
            if element.find("voice").is_some() && !voice.is_empty() {
                self.last_voice = Some(voice.to_string());
            }
        }

        let mut increment = 0.0;
        if is_chord {
            self.chord_notes.push(element);
            self.chord_lyrics.extend(element.find_all("lyric"));
        } else {
            let staff = staff_number(element);
            let (mut read, uid) = if is_rest {
                self.rests += 1;
                let rest = self.rest(element)?;
                (ReadNote::Rest(rest), None)
            } else {
                self.notes += 1;
                let (read, uid) = self.simple_note(element, true)?;
                (read, Some(uid))
            };
            let lyrics = lyrics_from(element.find_all("lyric"))?;
            let element_value: StreamElement = match &mut read {
                ReadNote::Rest(rest) => {
                    *rest.lyrics_mut() = lyrics;
                    rest.clone().into()
                }
                ReadNote::Note(note) => {
                    *note.lyrics_mut() = lyrics;
                    note.clone().into()
                }
                ReadNote::Unpitched(stroke) => {
                    *stroke.written_mut().lyrics_mut() = lyrics;
                    stroke.clone().into()
                }
            };
            increment = element_value.quarter_length();
            let placed = self.insert_note(element, staff, element_value);
            match uid {
                // The note was known to its spanners by a number of its own
                // before it had a place.
                Some(early) => self.rename(early, placed),
                None => self.free_pending(placed),
            }
            if is_rest {
                self.rest_spanners(element, placed)?;
                if std::mem::take(&mut self.marking_full) {
                    self.marked_full.push(placed);
                }
            }
            self.last_note = Some(placed);
        }

        if !self.chord_notes.is_empty() && !next_is_chord {
            let members = std::mem::take(&mut self.chord_notes);
            let lyrics = lyrics_from(std::mem::take(&mut self.chord_lyrics).into_iter())?;
            let (chord, member_uids) = self.chord(&members, lyrics)?;
            increment = chord.quarter_length();
            let source = members
                .iter()
                .find(|member| member.find("voice").is_some())
                .copied()
                .unwrap_or(element);
            let placed = self.insert_note(source, staff_number(members[0]), chord);
            for early in member_uids {
                self.rename(early, placed);
            }
            self.free_pending(placed);
            self.last_note = Some(placed);
        }

        self.offset = op_frac(self.offset + increment);
        self.finale_forward = None;
        Ok(())
    }

    /// Spanners that knew an element by one number know it by another.
    fn rename(&mut self, from: usize, to: usize) {
        for building in &mut self.part.importer.spanners {
            let mut renamed: Vec<usize> = Vec::new();
            for uid in &building.uids {
                let uid = if *uid == from { to } else { *uid };
                if !renamed.contains(&uid) {
                    renamed.push(uid);
                }
            }
            building.uids = renamed;
        }
    }

    /// music21's `freePendingSpannedElementAssignment`.
    fn free_pending(&mut self, uid: usize) {
        let importer = &mut *self.part.importer;
        if let Some(index) = importer.pending.pop_front() {
            importer.spanners[index].add_first(uid);
        }
    }

    /// music21's `xmlToSimpleNote`: a note or a stroke, known to the
    /// spanners by the number handed back.
    fn simple_note(&mut self, element: &'x Xml, free_spanners: bool) -> Result<(ReadNote, usize)> {
        let duration = self.duration(element)?;
        let mut note = match element.find("unpitched") {
            None => Note::from_pitch(pitch_of(element)?),
            Some(unpitched) => {
                let step = unpitched
                    .child_text("display-step")
                    .chars()
                    .next()
                    .unwrap_or('B');
                let octave = parse_int(unpitched.child_text("display-octave")).unwrap_or(4);
                Unpitched::displayed_at(step, octave)?.written().clone()
            }
        };
        note.set_duration(duration);

        let beams: Vec<&Xml> = element.find_all("beam").collect();
        if !beams.is_empty() {
            note.set_beams(beams_of(&beams)?);
        }
        if let Some(stem) = element.find("stem") {
            note.set_stem_direction(StemDirection::from_name(stem.stripped())?);
        }
        if let Some(notehead) = element.find("notehead") {
            if let Some(text) = notehead.text().filter(|text| !text.is_empty()) {
                note.set_notehead(Notehead::from_name(text)?);
            }
            if let Some(filled) = notehead.get("filled") {
                note.set_notehead_fill(Some(filled == "yes"));
            }
            if let Some(color) = notehead.get("color") {
                note.set_color(Some(color.to_string()));
            }
            if let Some(parentheses) = notehead.get("parentheses") {
                note.set_notehead_parenthesis(parentheses == "yes");
            }
        }

        let uid = self.part.importer.uid();
        if free_spanners {
            // music21 hands a waiting spanner the note as first read, and a
            // grace note is a copy made after that: the spanner is left
            // holding a note no stream holds.
            let joined = if element.find("grace").is_some() {
                self.part.importer.uid()
            } else {
                uid
            };
            self.free_pending(joined);
        }
        let mut general = General {
            color: note.color().map(str::to_string),
            ..General::default()
        };
        self.general_note(element, &mut general, note.duration().cloned(), uid, false)?;
        note.set_color(general.color);
        note.set_hidden(general.hidden);
        if let Some(volume) = general.volume {
            note.set_volume(Some(volume));
        }
        note.set_size(general.size);
        note.set_tie(general.tie);
        if let Some(duration) = general.duration {
            note.set_duration(duration);
        }
        *note.articulations_mut() = general.articulations;
        *note.expressions_mut() = general.expressions;

        Ok(if element.find("unpitched").is_some() {
            let mut stroke = Unpitched::new();
            *stroke.written_mut() = note;
            (ReadNote::Unpitched(stroke), uid)
        } else {
            (ReadNote::Note(note), uid)
        })
    }

    /// music21's `xmlToChord`.
    fn chord(
        &mut self,
        members: &[&'x Xml],
        lyrics: Vec<Lyric>,
    ) -> Result<(StreamElement, Vec<usize>)> {
        let mut notes: Vec<(ReadNote, usize)> = Vec::new();
        for member in members {
            notes.push(self.simple_note(member, false)?);
        }
        let percussion = members
            .iter()
            .any(|member| member.find("unpitched").is_some());

        // The chord takes the first note's beams, and each kind of mark
        // once, read off the notes lowest first.
        let mut order: Vec<usize> = (0..notes.len()).collect();
        order.sort_by(|left, right| {
            let ps = |index: &usize| notes[*index].0.written().pitch().ps();
            ps(left).total_cmp(&ps(right))
        });
        let mut articulations: Vec<Articulation> = Vec::new();
        let mut expressions: Vec<Expression> = Vec::new();
        let mut uids = Vec::new();
        for index in order {
            let (read, uid) = &notes[index];
            uids.push(*uid);
            for articulation in read.written().articulations() {
                let repeats = ["Fingering", "StringIndication", "FretIndication"]
                    .iter()
                    .any(|class| articulation.is_a(class));
                if !repeats
                    && articulations
                        .iter()
                        .any(|held| held.kind() == articulation.kind())
                {
                    continue;
                }
                articulations.push(articulation.clone());
            }
            for expression in read.written().expressions() {
                let same = |held: &Expression| match (held, expression) {
                    (Expression::Ornament(left), Expression::Ornament(right)) => {
                        left.kind() == right.kind()
                    }
                    (Expression::Fermata(_), Expression::Fermata(_)) => true,
                    (Expression::Arpeggio(_), Expression::Arpeggio(_)) => true,
                    _ => false,
                };
                if !expressions.iter().any(same) {
                    expressions.push(expression.clone());
                }
            }
        }
        let beams = notes
            .first()
            .map(|(read, _)| read.written().beams().clone())
            .unwrap_or_default();
        let duration = notes
            .first()
            .and_then(|(read, _)| read.written().duration().cloned());
        let stem = notes
            .first()
            .map_or(StemDirection::Unspecified, |(read, _)| {
                read.written().stem_direction()
            });
        for (read, _) in &mut notes {
            let written = read.written_mut();
            written.articulations_mut().clear();
            written.expressions_mut().clear();
        }
        if let Some((first, _)) = notes.first_mut() {
            first.written_mut().set_beams(Beams::new());
            // A chord's lyrics are kept on its first note.
            *first.written_mut().lyrics_mut() = lyrics;
        }

        let element: StreamElement = if percussion {
            let members: Vec<PercussionNote> = notes
                .into_iter()
                .map(|(read, _)| match read {
                    ReadNote::Unpitched(stroke) => PercussionNote::Unpitched(stroke),
                    ReadNote::Note(note) => PercussionNote::Note(note),
                    ReadNote::Rest(_) => unreachable!("a chord holds no rests"),
                })
                .collect();
            let mut chord = PercussionChord::new(members)?;
            let written = chord.written_mut();
            if let Some(duration) = duration {
                written.set_duration(duration);
            }
            written.set_beams(beams);
            *written.articulations_mut() = articulations;
            *written.expressions_mut() = expressions;
            chord.into()
        } else {
            let members: Vec<Note> = notes
                .into_iter()
                .map(|(read, _)| match read {
                    ReadNote::Note(note) => note,
                    _ => unreachable!("a chord of pitches holds notes"),
                })
                .collect();
            let mut chord = Chord::new(members)?;
            if let Some(duration) = duration {
                chord.set_duration(duration);
            }
            chord.set_beams(beams);
            // A chord reads its stem off its first note where it has none.
            let _ = stem;
            *chord.articulations_mut() = articulations;
            *chord.expressions_mut() = expressions;
            chord.into()
        };
        Ok((element, uids))
    }

    /// music21's `xmlToRest`.
    fn rest(&mut self, element: &'x Xml) -> Result<Rest> {
        let duration = self.duration(element)?;
        let mut rest = Rest::new(duration);
        let tag = element
            .find("rest")
            .ok_or_else(|| import_error("a rest without a <rest> tag"))?;
        if tag.get("measure") == Some("yes") {
            let written = element.child_text("type");
            if written.is_empty() || matches!(written, "whole" | "breve") {
                self.full_measure_rest = true;
                self.marking_full = true;
            }
        }
        let step = tag.child_text("display-step");
        if !step.is_empty() {
            let name = format!("{step}{}", tag.child_text("display-octave"));
            let displayed = Pitch::from_name(&name)?;
            let staff = match staff_number(element) {
                NO_STAFF => 1,
                staff => staff,
            };
            let middle = self
                .part
                .last_clefs
                .iter()
                .find(|(held, _)| *held == staff)
                .and_then(|(_, clef)| clef.lowest_line().filter(|_| clef.places_pitches()))
                .map_or(35, |lowest| lowest + 4);
            rest.set_step_shift(displayed.diatonic_note_number() - middle);
        }
        let mut general = General::default();
        let uid = 0;
        self.general_note(
            element,
            &mut general,
            Some(rest.duration().clone()),
            uid,
            true,
        )?;
        rest.set_hidden(general.hidden);
        rest.set_size(general.size);
        if let Some(duration) = general.duration {
            rest.set_duration(duration);
        }
        *rest.articulations_mut() = general.articulations;
        *rest.expressions_mut() = general.expressions;
        Ok(rest)
    }

    /// The spanners a rest's notations start or stop, which can only be
    /// joined once the rest has a number.
    fn rest_spanners(&mut self, element: &'x Xml, uid: usize) -> Result<()> {
        for notations in element.find_all("notations") {
            self.notation_spanners(notations, uid)?;
        }
        Ok(())
    }

    /// music21's `xmlNoteToGeneralNoteHelper`.
    fn general_note(
        &mut self,
        element: &'x Xml,
        general: &mut General,
        duration: Option<Duration>,
        uid: usize,
        is_rest: bool,
    ) -> Result<()> {
        if let Some(color) = element.get("color") {
            general.color = Some(color.to_string());
        }
        general.hidden = element.get("print-object") == Some("no");
        if !is_rest && let Some(dynamics) = element.get("dynamics").and_then(parse_float) {
            general.volume = Some(Volume::from_velocity_scalar(dynamics * (90.0 / 12700.0))?);
        }
        if element.get("pizzicato") == Some("yes")
            && let Some(kind) = ArticulationKind::from_class_name("Pizzicato")
        {
            general.articulations.push(Articulation::of_kind(kind));
        }
        if let Some(written) = element.find("type")
            && let Some(size) = written.get("size")
        {
            general.size = NoteSize::from_name(size);
        }
        if element.find("tie").is_some() {
            general.tie = tie_of(element);
        }
        if let Some(grace) = element.find("grace") {
            // A grace note is written as an eighth where nothing says.
            let base = duration.unwrap_or_default();
            let mut written = Duration::default();
            written.clear();
            written.set_tuplets(Vec::new());
            for (kind, dots) in base.components() {
                written.add_duration_tuple(kind, dots);
            }
            if base.components().is_empty() {
                written.add_duration_tuple(DurationType::Eighth, 0);
            }
            let mut value = written.grace_duration();
            let mut marks = Grace::new();
            marks.set_slash(matches!(grace.get("slash"), Some("yes") | None));
            let share = |name: &str| {
                grace
                    .get(name)
                    .and_then(parse_int)
                    .map(|percent| FloatType::from(percent) / 100.0)
            };
            marks.set_steal_time_previous(share("steal-time-previous"));
            marks.set_steal_time_following(share("steal-time-following"));
            value.set_grace(Some(marks));
            general.duration = Some(value);
        }
        for notations in element.find_all("notations") {
            self.notations(notations, general)?;
            if !is_rest {
                self.notation_spanners(notations, uid)?;
            }
        }
        Ok(())
    }

    /// music21's `xmlToDuration`.
    fn duration(&mut self, element: &'x Xml) -> Result<Duration> {
        let quarter_length = match element.find("duration") {
            Some(duration) => {
                let ticks = parse_float(duration.stripped())
                    .ok_or_else(|| import_error("a <duration> that is not a number"))?;
                op_frac(ticks / self.divisions)
            }
            None => 0.0,
        };
        let written = element.child_text("type");
        if written.is_empty() {
            // Some rests say how long they last and nothing else.
            return Duration::new(quarter_length);
        }
        let kind = duration_type(written)?;
        let dots = element.find_all("dot").count() as u32;
        let tuplets = if element.find("time-modification").is_some() {
            self.tuplets(element)?
        } else {
            Vec::new()
        };
        if kind.quarter_length_with_dots(dots) == quarter_length && tuplets.is_empty() {
            return Duration::new(quarter_length);
        }
        // Made from the length first, as music21 makes it, so its writing
        // counts as worked out even with the written value put in its place.
        let mut duration = Duration::new(quarter_length)?;
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

    /// music21's `xmlToTuplets`.
    fn tuplets(&mut self, element: &'x Xml) -> Result<Vec<Tuplet>> {
        let modification = element
            .find("time-modification")
            .ok_or_else(|| import_error("Note without time-modification in xmlToTuplets"))?;
        let count = |tag: &str, fallback: u32| {
            parse_int(modification.child_text(tag)).map_or(fallback, |value| value.max(1) as u32)
        };
        let normal_type = match modification.child_text("normal-type") {
            "" => element.child_text("type"),
            written => written,
        };
        let kind = duration_type(normal_type)?;
        let dots = modification.find_all("normal-dot").count() as u32;
        let time_mod = Tuplet::new(
            count("actual-notes", 3),
            count("normal-notes", 2),
            kind,
            dots,
        );

        let notations = element.find("notations");
        if notations.is_none() {
            self.part.active_tuplets[0] = Some(time_mod);
        }
        let multiplier = |tuplet: &Tuplet| -> (i64, i64) {
            // actual notes in the time of normal: each lasts normal/actual,
            // scaled by the written values of the two sides.
            let ratio = tuplet.multiplier();
            (
                i64::from(*ratio.numer().unwrap_or(&1)),
                i64::from(*ratio.denom().unwrap_or(&1)),
            )
        };
        let divide = |remaining: (i64, i64), by: (i64, i64)| -> (i64, i64) {
            reduce(remaining.0 * by.1, remaining.1 * by.0)
        };
        let mut remaining = multiplier(&time_mod);
        let mut returned: [Option<Tuplet>; 8] = Default::default();
        // Which of the returned tuplets are the active ones themselves.
        let mut from_active = [false; 8];
        let mut to_remove: Vec<usize> = Vec::new();
        let mut to_stop: Vec<usize> = Vec::new();

        if let Some(notations) = notations {
            for written in notations.find_all("tuplet") {
                let index = written
                    .get("number")
                    .and_then(parse_int)
                    .map_or(1, |number| number.clamp(0, 6) as usize);
                let tuplet_type = written.get("type");
                if tuplet_type == Some("stop") {
                    if let Some(active) = self.part.active_tuplets[index] {
                        // One that started on this very note starts and
                        // stops on it. music21 asks by value, and so finds
                        // an equal tuplet of another number too.
                        if returned.iter().flatten().any(|held| *held == active)
                            && from_active[index]
                            && let Some(held) = &mut returned[index]
                        {
                            held.set_tuplet_type(Some(TupletType::StartStop));
                        }
                        to_remove.push(index);
                        to_stop.push(index);
                    }
                    continue;
                }
                let actual = written.find("tuplet-actual");
                let normal = written.find("tuplet-normal");
                let mut tuplet = match (actual, normal) {
                    (Some(actual), Some(normal)) => {
                        let number = |side: &Xml, fallback: u32| {
                            parse_int(side.child_text("tuplet-number"))
                                .map_or(fallback, |value| value.max(1) as u32)
                        };
                        let mut tuplet = Tuplet::ratio(number(actual, 3), number(normal, 2));
                        let side = |side: &Xml| -> Result<Option<(DurationType, u32)>> {
                            match side.find("tuplet-type") {
                                Some(written) if written.text().is_some() => Ok(Some((
                                    duration_type(written.stripped())?,
                                    written.find_all("tuplet-dot").count() as u32,
                                ))),
                                _ => Ok(None),
                            }
                        };
                        if let Some(value) = side(actual)? {
                            tuplet.set_duration_actual(Some(value));
                        }
                        if let Some(value) = side(normal)? {
                            tuplet.set_duration_normal(Some(value));
                        }
                        tuplet
                    }
                    _ => time_mod,
                };
                tuplet
                    .set_tuplet_type(tuplet_type.and_then(|name| TupletType::from_name(name).ok()));
                let bracket = written.get("bracket");
                if let Some(bracket) = bracket {
                    tuplet.set_bracket(if bracket == "yes" {
                        TupletBracket::Bracket
                    } else {
                        TupletBracket::None
                    });
                }
                let show_number = written.get("show-number");
                if show_number == Some("none") {
                    tuplet.set_actual_show(None);
                    if bracket.is_none() {
                        tuplet.set_bracket(TupletBracket::None);
                    }
                } else if show_number == Some("both") {
                    tuplet.set_normal_show(Some(TupletShow::Number));
                }
                let both_or_type = |shown: Option<TupletShow>| {
                    Some(if shown.is_some() {
                        TupletShow::Both
                    } else {
                        TupletShow::Type
                    })
                };
                if written.get("show-type") == Some("actual") {
                    tuplet.set_actual_show(both_or_type(tuplet.actual_show()));
                } else if show_number == Some("both") {
                    tuplet.set_actual_show(both_or_type(tuplet.actual_show()));
                    tuplet.set_normal_show(both_or_type(tuplet.normal_show()));
                }
                if written.get("line-shape") == Some("curved") {
                    tuplet.set_bracket(TupletBracket::Slur);
                }
                tuplet.set_placement(placement_of(written));
                remaining = divide(remaining, multiplier(&tuplet));
                returned[index] = Some(tuplet);
                from_active[index] = true;
                self.part.active_tuplets[index] = Some(tuplet);
            }
        }

        for index in 1..self.part.active_tuplets.len() {
            let Some(active) = &self.part.active_tuplets[index] else {
                continue;
            };
            if returned.iter().flatten().any(|held| held == active) {
                continue;
            }
            let mut copy = *active;
            copy.set_tuplet_type(if to_stop.contains(&index) {
                Some(TupletType::Stop)
            } else {
                None
            });
            remaining = divide(remaining, multiplier(&copy));
            returned[index] = Some(copy);
        }
        if remaining != (1, 1) {
            let mut remainder = Tuplet::ratio(remaining.1.max(1) as u32, remaining.0.max(1) as u32);
            remainder.set_duration_normal(time_mod.duration_normal());
            remainder.set_duration_actual(time_mod.duration_actual());
            returned[7] = Some(remainder);
        }
        for index in to_remove {
            self.part.active_tuplets[index] = None;
        }
        Ok(returned.into_iter().flatten().collect())
    }

    // ------------------------------------------------------- notations

    /// music21's `xmlNotations`, apart from the spanners.
    fn notations(&mut self, notations: &'x Xml, general: &mut General) -> Result<()> {
        for technical in notations.find_all("technical") {
            for mark in &technical.children {
                if let Some(read) = technical_of(mark) {
                    general.articulations.push(read);
                }
            }
        }
        for articulations in notations.find_all("articulations") {
            for mark in &articulations.children {
                if let Some(read) = articulation_of(mark) {
                    general.articulations.push(read);
                }
            }
        }
        for written in notations.find_all("fermata") {
            let mut fermata = Fermata::new();
            match written.get("type") {
                Some("upright") => fermata.set_fermata_type(FermataType::Upright),
                Some("inverted") => fermata.set_fermata_type(FermataType::Inverted),
                _ => {}
            }
            let shape = written.stripped();
            if !shape.is_empty() {
                fermata.set_shape(Some(shape.to_string()));
            }
            general.expressions.push(Expression::Fermata(fermata));
        }
        for tag in ["arpeggiate", "non-arpeggiate"] {
            for written in notations.find_all(tag) {
                let kind = if tag == "non-arpeggiate" {
                    ArpeggioType::NonArpeggio
                } else {
                    ArpeggioType::from_name(written.get("direction").unwrap_or("normal"))?
                };
                match written.get("number") {
                    None => general.expressions.push(Expression::Arpeggio(kind)),
                    Some(number) => general.arpeggio_spanners.push((number.to_string(), kind)),
                }
            }
        }
        let mut latest: Option<usize> = None;
        for ornaments in notations.find_all("ornaments") {
            for mark in &ornaments.children {
                if mark.tag == "accidental-mark" {
                    let Some(index) = latest else {
                        continue;
                    };
                    let mut accidental = accidental_of(mark)?;
                    accidental.set_display_status(Some(true));
                    if let Expression::Ornament(ornament) = &mut general.expressions[index] {
                        if ornament.is_a("Turn") {
                            if mark.get("placement") == Some("below") {
                                ornament.set_lower_accidental(Some(accidental));
                            } else {
                                ornament.set_upper_accidental(Some(accidental));
                            }
                        } else if ornament.is_a("GeneralMordent") || ornament.is_a("Trill") {
                            ornament.set_accidental(Some(accidental))?;
                        }
                    }
                    continue;
                }
                let class = match mark.tag.as_str() {
                    "trill-mark" => "Trill",
                    "turn" | "delayed-turn" => "Turn",
                    "inverted-turn" | "delayed-inverted-turn" => "InvertedTurn",
                    "shake" => "Shake",
                    "mordent" => "Mordent",
                    "inverted-mordent" => "InvertedMordent",
                    "schleifer" => "Schleifer",
                    "other-ornament" => "Ornament",
                    "tremolo" => {
                        if matches!(mark.get("type"), Some("start" | "stop")) {
                            continue;
                        }
                        "Tremolo"
                    }
                    _ => continue,
                };
                let Some(kind) = OrnamentKind::from_class_name(class) else {
                    continue;
                };
                let mut ornament = Ornament::of_kind(kind);
                match mark.tag.as_str() {
                    "delayed-turn" | "delayed-inverted-turn" => {
                        ornament.set_delay(OrnamentDelay::Default);
                    }
                    "turn" | "inverted-turn" => ornament.set_delay(OrnamentDelay::NoDelay),
                    "tremolo" => {
                        ornament.set_number_of_marks(tremolo_marks(mark)?)?;
                    }
                    _ => {}
                }
                if mark.tag != "tremolo" {
                    if let Some(placement) = mark.get("placement") {
                        ornament.set_placement(Some(placement.to_string()));
                    }
                    latest = Some(general.expressions.len());
                }
                general
                    .expressions
                    .push(Expression::Ornament(Box::new(ornament)));
            }
        }
        Ok(())
    }

    /// music21's `xmlNotationsToSpanners` and the arpeggios across chords.
    fn notation_spanners(&mut self, notations: &'x Xml, uid: usize) -> Result<()> {
        for tag in ["arpeggiate", "non-arpeggiate"] {
            for written in notations.find_all(tag) {
                let Some(number) = written.get("number") else {
                    continue;
                };
                let kind = if tag == "non-arpeggiate" {
                    ArpeggioType::NonArpeggio
                } else {
                    ArpeggioType::from_name(written.get("direction").unwrap_or("normal"))
                        .unwrap_or(ArpeggioType::Normal)
                };
                let spanners = &mut self.part.importer.spanners;
                let found = spanners.iter().position(|held| {
                    held.kind == SpannerKind::ArpeggioMark
                        && !held.complete
                        && held.id_local.as_deref() == Some(number)
                });
                let index = found.unwrap_or_else(|| {
                    let mut building = Building::new(SpannerKind::ArpeggioMark, Some(number));
                    building.arpeggio = kind;
                    spanners.push(building);
                    spanners.len() - 1
                });
                spanners[index].add(uid);
            }
        }
        for ornaments in notations.find_all("ornaments") {
            for mark in &ornaments.children {
                match mark.tag.as_str() {
                    "wavy-line" => {
                        self.one_spanner(mark, Some(uid), SpannerKind::TrillExtension, false);
                    }
                    // music21's `xmlToTremolo`: a tremolo that starts or
                    // stops is one between notes.
                    "tremolo" if matches!(mark.get("type"), Some("start" | "stop")) => {
                        let index =
                            self.one_spanner(mark, Some(uid), SpannerKind::TremoloSpanner, false);
                        self.part.importer.spanners[index].marks = tremolo_marks(mark)?;
                    }
                    _ => {}
                }
            }
        }
        for slur in notations.find_all("slur") {
            let index = self.one_spanner(slur, Some(uid), SpannerKind::Slur, false);
            let building = &mut self.part.importer.spanners[index];
            if let Some(line_type) = slur.get("line-type") {
                building.line_type = Some(line_type.to_string());
            }
            if let Some(placement) = placement_of(slur) {
                building.placement = Some(placement);
            }
        }
        for tag in ["glissando", "slide"] {
            for written in notations.find_all(tag) {
                let index = self.one_spanner(written, Some(uid), SpannerKind::Glissando, false);
                let building = &mut self.part.importer.spanners[index];
                if tag == "slide" {
                    building.slide = SlideType::Continuous;
                }
                match written.get("line-type") {
                    Some(line_type) => {
                        let lower = line_type.to_lowercase();
                        if !["solid", "dashed", "dotted", "wavy"].contains(&lower.as_str()) {
                            return Err(import_error(format!("not a valid value: {line_type}")));
                        }
                        building.line_type = Some(lower);
                    }
                    None if tag == "slide" => building.line_type = Some("solid".to_string()),
                    None => {}
                }
            }
        }
        Ok(())
    }

    /// music21's `xmlOneSpanner`: the open spanner of a kind and number, or
    /// a new one.
    fn one_spanner(
        &mut self,
        element: &Xml,
        target: Option<usize>,
        kind: SpannerKind,
        allow_duplicates: bool,
    ) -> usize {
        let id = element.get("number");
        let spanners = &mut self.part.importer.spanners;
        let found = spanners
            .iter()
            .position(|held| held.kind == kind && !held.complete && held.id_local.as_deref() == id);
        let index = match found {
            Some(index) if !allow_duplicates => index,
            _ => {
                let mut building = Building::new(kind, id);
                if let Some(placement) = placement_of(element) {
                    building.placement = Some(placement);
                }
                spanners.push(building);
                spanners.len() - 1
            }
        };
        if let Some(target) = target {
            spanners[index].add(target);
        }
        if element.get("type") == Some("stop") {
            spanners[index].complete = true;
        }
        index
    }

    // ------------------------------------------------------ directions

    /// music21's `xmlDirection`.
    fn direction(&mut self, direction: &'x Xml) -> Result<()> {
        let total = self.offset_of(direction) + self.offset;
        let staff = staff_number(direction);
        let mut metronome_added = false;
        for direction_type in direction.find_all("direction-type") {
            for specific in &direction_type.children {
                self.direction_type(specific, direction, staff, total)?;
                if specific.tag == "metronome" {
                    metronome_added = true;
                }
            }
        }
        if !metronome_added
            && let Some(sound) = direction
                .find_all("sound")
                .find(|sound| sound.get("tempo").is_some())
        {
            self.sound_tempo(sound, Some(direction), staff, total);
        }
        Ok(())
    }

    /// music21's `setSoundTempo`: how fast the music is played, where no
    /// mark says.
    fn sound_tempo(&mut self, sound: &Xml, direction: Option<&Xml>, staff: i32, total: FloatType) {
        let Some(tempo) = sound
            .get("tempo")
            .and_then(parse_float)
            .filter(|qpm| *qpm != 0.0)
        else {
            return;
        };
        let mut mark = MetronomeMark::default().with_number_sounding(tempo);
        if let Some(direction) = direction {
            mark.set_placement(placement_of(direction));
        }
        self.insert(total, staff, mark);
    }

    /// music21's `setDirectionInDirectionType`.
    fn direction_type(
        &mut self,
        specific: &'x Xml,
        direction: &'x Xml,
        staff: i32,
        total: FloatType,
    ) -> Result<()> {
        match specific.tag.as_str() {
            "dynamics" => {
                for written in &specific.children {
                    let text = match (written.tag.as_str(), written.stripped()) {
                        ("other-dynamic", text) if !text.is_empty() => text,
                        (tag, _) => tag,
                    };
                    let mut dynamic = Dynamic::new(text);
                    dynamic.set_placement(placement_of(direction));
                    self.insert(total, staff, dynamic);
                }
            }
            "wedge" | "bracket" | "dashes" | "octave-shift" | "pedal" => {
                // music21 warns and carries on when one cannot be read.
                if let Ok(made) = self.direction_spanners(specific, staff, total) {
                    for index in made {
                        let building = &mut self.part.importer.spanners[index];
                        if let Some(placement) = placement_of(specific) {
                            building.placement = Some(placement);
                        }
                        if building.kind != SpannerKind::Crescendo
                            && building.kind != SpannerKind::Diminuendo
                            && let Some(line_type) = specific.get("line-type")
                        {
                            building.line_type = Some(line_type.to_string());
                        }
                    }
                }
            }
            "coda" | "segno" => {
                let kind = if specific.tag == "segno" {
                    RepeatExpressionKind::Segno
                } else {
                    RepeatExpressionKind::Coda
                };
                self.insert(total, staff, RepeatExpression::new(kind));
            }
            "rehearsal" => {
                // music21's `xmlToRehearsalMark`.
                let mut mark = RehearsalMark::new(specific.stripped());
                if let Some(enclosure) = specific.get("enclosure") {
                    let lower = enclosure.to_lowercase();
                    if !ENCLOSURES.contains(&lower.as_str()) {
                        return Err(import_error(format!(
                            "Not a supported enclosure: '{enclosure}'"
                        )));
                    }
                    mark.set_enclosure(Some(lower));
                }
                mark.set_placement(placement_of(direction));
                self.insert(total, staff, mark);
            }
            "metronome" => {
                let tempo = metronome(specific, placement_of(direction))?;
                self.insert(total, staff, tempo);
            }
            "words" => {
                let text = specific.stripped();
                let placement = placement_of(direction);
                // Words that say where to go next are a repeat mark.
                let repeat = RepeatExpressionKind::ALL
                    .into_iter()
                    .find(|kind| RepeatExpression::new(*kind).is_valid_text(text));
                match repeat {
                    Some(kind) => {
                        let mut mark = RepeatExpression::new(kind);
                        mark.set_text(text);
                        // The words are placed, not the mark: one drawn as
                        // its sign, a coda or a segno, stands where music21
                        // puts a sign whatever side its words were on.
                        if !mark.use_symbol() {
                            mark.set_placement(placement);
                        }
                        self.insert(total, staff, mark);
                    }
                    None => {
                        let mut words = TextExpression::new(text);
                        words.set_placement(placement);
                        self.insert(total, staff, words);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// music21's `xmlDirectionTypeToSpanners`. Hands back the spanners it
    /// started.
    fn direction_spanners(
        &mut self,
        element: &'x Xml,
        staff: i32,
        total: FloatType,
    ) -> Result<Vec<usize>> {
        let last = self.last_note;
        let id = element.get("number");
        let mut made = Vec::new();
        let open = |spanners: &[Building], wanted: &dyn Fn(SpannerKind) -> bool| {
            spanners.iter().position(|held| {
                wanted(held.kind) && !held.complete && held.id_local.as_deref() == id
            })
        };
        match element.tag.as_str() {
            "wedge" => {
                let kind = match element.get("type") {
                    Some("crescendo") => Some(SpannerKind::Crescendo),
                    Some("diminuendo") => Some(SpannerKind::Diminuendo),
                    Some("stop") => None,
                    other => {
                        return Err(import_error(format!("Unknown type, {other:?}.")));
                    }
                };
                match kind {
                    Some(kind) => {
                        let index = self.one_spanner(element, None, kind, true);
                        self.part.importer.pending.push_back(index);
                        made.push(index);
                    }
                    None => {
                        let spanners = &mut self.part.importer.spanners;
                        let index = open(spanners, &SpannerKind::is_wedge)
                            .ok_or_else(|| import_error("Error in getting DynamicWedges"))?;
                        spanners[index].complete = true;
                        if let Some(last) = last {
                            spanners[index].add(last);
                        }
                    }
                }
            }
            "bracket" | "dashes" => {
                let dashes = element.tag == "dashes";
                let tick = || -> Result<LineEnd> {
                    match element.get("line-end") {
                        Some(name) => LineEnd::from_name(name),
                        None => Ok(LineEnd::Down),
                    }
                };
                let height = element.get("end-length").and_then(parse_float);
                let Importer {
                    spanners, pending, ..
                } = &mut *self.part.importer;
                match element.get("type") {
                    Some("start") => {
                        let mut building = Building::new(SpannerKind::Line, id);
                        if dashes {
                            building.ends.start = LineEnd::None;
                            building.line_type = Some("dashed".to_string());
                        } else {
                            building.ends.start_height = height;
                            building.ends.start = tick()?;
                            if let Some(line_type) = element.get("line-type") {
                                building.line_type = Some(line_type.to_string());
                            }
                        }
                        spanners.push(building);
                        pending.push_back(spanners.len() - 1);
                        made.push(spanners.len() - 1);
                    }
                    Some("stop") => {
                        let Some(index) = open(spanners, &|kind| kind == SpannerKind::Line) else {
                            return Ok(made);
                        };
                        let building = &mut spanners[index];
                        building.complete = true;
                        if dashes {
                            building.ends.end = LineEnd::None;
                            building.line_type = Some("dashed".to_string());
                        } else {
                            building.ends.end = tick()?;
                            if height.is_some() {
                                building.ends.end_height = height;
                            }
                            if let Some(line_type) = element.get("line-type") {
                                building.line_type = Some(line_type.to_string());
                            }
                        }
                        if let Some(last) = last {
                            building.add(last);
                        }
                    }
                    other => {
                        return Err(import_error(format!(
                            "unidentified mxType of mxBracket: {other:?}"
                        )));
                    }
                }
            }
            "octave-shift" => {
                let Importer {
                    spanners, pending, ..
                } = &mut *self.part.importer;
                match element.get("type") {
                    Some(direction @ ("up" | "down")) => {
                        let size = element.get("size").and_then(parse_int).unwrap_or(8);
                        // An `up` shift is a line under the notes.
                        let up = direction == "down";
                        let name = match (size, up) {
                            (15, true) => "15ma",
                            (15, false) => "15mb",
                            (22, true) => "22da",
                            (22, false) => "22db",
                            (_, true) => "8va",
                            (_, false) => "8vb",
                        };
                        let mut building = Building::new(SpannerKind::Ottava, id);
                        building.shift = Some(OctaveShift::from_name(name, false)?);
                        building.placement = Some(if up {
                            Placement::Above
                        } else {
                            Placement::Below
                        });
                        spanners.push(building);
                        pending.push_back(spanners.len() - 1);
                        made.push(spanners.len() - 1);
                    }
                    Some(kind @ ("continue" | "stop")) => {
                        let index = open(spanners, &|kind| kind == SpannerKind::Ottava)
                            .ok_or_else(|| import_error("Error in getting Ottava"))?;
                        if kind == "continue" {
                            pending.push_back(index);
                        } else {
                            spanners[index].complete = true;
                            if let Some(last) = last {
                                spanners[index].add(last);
                            }
                        }
                    }
                    other => {
                        return Err(import_error(format!(
                            "unidentified mxType of octave-shift: {other:?}"
                        )));
                    }
                }
            }
            "pedal" => {
                let Importer {
                    spanners, pending, ..
                } = &mut *self.part.importer;
                match element.get("type") {
                    Some(kind @ ("start" | "sostenuto")) => {
                        let mut building = Building::new(SpannerKind::PedalMark, id);
                        building.pedal.pedal_type = Some(if kind == "start" {
                            PedalType::Sustain
                        } else {
                            PedalType::Sostenuto
                        });
                        if element.get("line") == Some("yes") {
                            building.pedal.form = Some(PedalForm::Line);
                        } else if element.get("line") == Some("no")
                            || element.get("sign") == Some("yes")
                        {
                            building.pedal.form = Some(PedalForm::Symbol);
                        }
                        building.pedal.abbreviated = element.get("abbreviated") == Some("yes");
                        spanners.push(building);
                        pending.push_back(spanners.len() - 1);
                        self.pedal_starts.push((spanners.len() - 1, total));
                        made.push(spanners.len() - 1);
                    }
                    Some(kind @ ("continue" | "stop" | "discontinue" | "resume" | "change")) => {
                        let index = open(spanners, &|kind| kind == SpannerKind::PedalMark)
                            .ok_or_else(|| import_error("Error in getting PedalMark"))?;
                        match kind {
                            "stop" => {
                                spanners[index].complete = true;
                                if let Some(last) = last {
                                    spanners[index].add(last);
                                }
                            }
                            "resume" => {
                                let started = self
                                    .pedal_starts
                                    .iter()
                                    .find(|(held, _)| *held == index)
                                    .map(|(_, at)| *at);
                                if spanners[index].pedal.form == Some(PedalForm::Symbol)
                                    && started == Some(total)
                                {
                                    spanners[index].pedal.form = Some(PedalForm::SymbolLine);
                                } else {
                                    self.pedal_object(PedalObjectKind::GapEnd, index, staff, total);
                                }
                            }
                            "discontinue" => {
                                self.pedal_object(PedalObjectKind::GapStart, index, staff, total);
                            }
                            "change" => {
                                self.pedal_object(PedalObjectKind::Bounce, index, staff, total);
                            }
                            // music21 remembers nothing of a pedal that
                            // carries on.
                            _ => {}
                        }
                    }
                    other => {
                        return Err(import_error(format!(
                            "unidentified mxType of pedal: {other:?}"
                        )));
                    }
                }
            }
            _ => {}
        }
        Ok(made)
    }

    /// A bounce or a gap in a held pedal, put where the direction stands and
    /// joined by the pedal mark it belongs to.
    fn pedal_object(&mut self, kind: PedalObjectKind, pedal: usize, staff: i32, total: FloatType) {
        let uid = self.insert(total, staff, PedalObject::new(kind));
        self.part.importer.spanners[pedal].add(uid);
    }

    /// music21's `xmlHarmony` and `xmlToChordSymbol`.
    fn harmony(&mut self, harmony: &'x Xml) -> Result<()> {
        let mut kind = harmony.child_text("kind").to_string();
        let kind_element = harmony.find("kind");
        let mut kind_text = String::new();
        if !kind.is_empty() {
            for (alias, target) in crate::chordsymbol::CHORD_KIND_ALIASES {
                if alias == kind {
                    kind = target.to_string();
                }
            }
            let written = kind_element.and_then(|kind| kind.get("text")).unwrap_or("");
            if !(written.is_empty() && kind != "none") {
                kind_text = written.to_string();
            }
        }
        let pitch_of = |step: &Xml, alter: Option<&Xml>| -> Result<Option<Pitch>> {
            let name = match step.text().filter(|text| !text.is_empty()) {
                Some(text) => text,
                None => match step.get("text") {
                    Some(text) => text,
                    None => return Ok(None),
                },
            };
            let mut pitch = Pitch::from_name(name.trim())?;
            if let Some(alter) = alter.and_then(|alter| parse_float(alter.stripped())) {
                pitch.set_written_accidental(Some(Accidental::new(alter)?));
            }
            Ok(Some(pitch))
        };
        let bass = match harmony.find("bass") {
            Some(bass) => {
                let step = bass
                    .find("bass-step")
                    .ok_or_else(|| import_error("bass-step missing"))?;
                pitch_of(step, bass.find("bass-alter"))?
            }
            None => None,
        };
        let root = match harmony.find("root") {
            Some(root) => match root.find("root-step") {
                Some(step) => pitch_of(step, root.find("root-alter"))?,
                None => None,
            },
            None => None,
        };

        let mut symbol = if kind == "none" {
            ChordSymbol::no_chord(Some(kind_text.clone()).filter(|text| !text.is_empty()))
        } else {
            let root = root.ok_or_else(|| {
                import_error("reading a chord symbol given by function is not supported yet")
            })?;
            let mut symbol = match ChordSymbol::from_kind(root.clone(), &kind, bass.clone()) {
                Ok(symbol) => symbol,
                // A kind music21 has no notes for is still a symbol of that
                // kind on its root.
                Err(_) => {
                    let mut symbol = ChordSymbol::parse_music21(root.name())?;
                    symbol.set_root(root);
                    symbol.set_bass(bass);
                    symbol.with_kind(&kind)
                }
            };
            symbol.set_kind_text(Some(kind_text));
            symbol
        };
        if !symbol.is_no_chord() {
            for degree in harmony.find_all("degree") {
                let value = parse_int(degree.child_text("degree-value"))
                    .ok_or_else(|| import_error("degree-value missing"))?;
                let alter = parse_int(degree.child_text("degree-alter")).unwrap_or(0);
                let modification = ChordStepModification::new(
                    crate::chordsymbol::ChordStepModificationType::from_music21_name(
                        degree.child_text("degree-type"),
                    )?,
                    value.clamp(0, 255) as u8,
                    alter,
                )?;
                symbol.add_chord_step_modification(modification);
            }
        }
        symbol.set_placement(placement_of(harmony));
        let offset = self.offset + self.offset_of(harmony);
        self.insert(offset, staff_number(harmony), symbol);
        Ok(())
    }

    /// music21's `xmlBarline`.
    fn barline(&mut self, element: &'x Xml) -> Result<()> {
        let style = element.child_text("bar-style");
        let right = matches!(element.get("location"), Some("right") | None);
        let barline = match element.find("repeat") {
            Some(repeat) => {
                let written = match repeat.get("direction").map(str::to_lowercase).as_deref() {
                    Some("forward") => RepeatDirection::Start,
                    Some("backward") => RepeatDirection::End,
                    Some(other) => {
                        return Err(import_error(format!(
                            "cannot handle mx direction format: {other}"
                        )));
                    }
                    None => return Err(import_error("Repeat sign direction is required")),
                };
                // A repeat closing a measure ends the passage, whatever the
                // file says; only an end says how many times.
                let direction = if right { RepeatDirection::End } else { written };
                let times = repeat
                    .get("times")
                    .and_then(parse_int)
                    .map(|times| times.max(0) as u32)
                    .filter(|_| written == RepeatDirection::End);
                // The sign is drawn as its direction draws it, not as the
                // file's bar style says.
                Barline::repeat(direction, times)
            }
            None => {
                let mut barline = Barline::default();
                if !style.is_empty() {
                    barline.set_bar_type(BarlineType::from_name(style)?);
                }
                barline
            }
        };

        if let Some(ending) = element.find("ending") {
            let index = self.part.measures.len();
            let open = self
                .part
                .brackets
                .iter()
                .position(|bracket| !bracket.complete);
            let at = match open {
                Some(at) => {
                    if !self.part.brackets[at].measures.contains(&index) {
                        self.part.brackets[at].measures.push(index);
                    }
                    at
                }
                None => {
                    self.part.brackets.push(Bracket {
                        numbers: vec![0],
                        measures: vec![index],
                        complete: false,
                    });
                    self.part.brackets.len() - 1
                }
            };
            let bracket = &mut self.part.brackets[at];
            if ending.get("type") == Some("start") {
                bracket.numbers = bracket_numbers(ending.get("number").unwrap_or("1"));
                if let Some(text) = ending.text() {
                    let digits = text.strip_suffix('.').unwrap_or(text);
                    if !digits.is_empty()
                        && digits.chars().all(|c| c.is_ascii_digit())
                        && let Ok(number) = digits.parse::<u32>()
                    {
                        bracket.numbers = vec![number];
                    }
                }
            }
            if matches!(ending.get("type"), Some("stop" | "discontinue")) {
                bracket.complete = true;
            }
        }

        match element.get("location") {
            Some("left") => self.measure.left = Some(barline),
            Some("right") | None => self.measure.right = Some(barline),
            // A barline in the middle of a measure is an element of its own,
            // which music21 appends: after the last thing read so far ends,
            // whatever the file's place for it, and at the very start of a
            // measure whose notes are all in voices.
            Some(_) => {
                let end = self.measure.highest_time_outside_voices();
                self.insert(end, NO_STAFF, barline);
            }
        }
        Ok(())
    }
}

/// General MIDI's drum for a pitch as the instrument music21 has for it:
/// its `PercussionMapper.midiPitchToInstrument`, and a bare unpitched
/// instrument where it has none.
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
        _ => {
            let mut bare = Instrument::of_kind("UnpitchedPercussion").unwrap_or_default();
            bare.set_percussion_pitch(Some(pitch));
            return bare;
        }
    };
    let mut instrument = Instrument::of_kind(class).unwrap_or_default();
    // An instrument that names its drums apart takes the one asked for.
    let usual = instrument.percussion_pitch();
    instrument.set_percussion_pitch(Some(pitch));
    if instrument.modifier().is_none() {
        instrument.set_percussion_pitch(usual);
    }
    instrument
}

/// What is read of a note before it is known whether it has a pitch.
enum ReadNote {
    Note(Note),
    Unpitched(Unpitched),
    Rest(Rest),
}

impl ReadNote {
    fn written(&self) -> &Note {
        match self {
            Self::Note(note) => note,
            Self::Unpitched(stroke) => stroke.written(),
            Self::Rest(_) => unreachable!("a rest is not written as a note"),
        }
    }

    fn written_mut(&mut self) -> &mut Note {
        match self {
            Self::Note(note) => note,
            Self::Unpitched(stroke) => stroke.written_mut(),
            Self::Rest(_) => unreachable!("a rest is not written as a note"),
        }
    }
}

/// What a `<note>` says that a note, a stroke and a rest all take.
#[derive(Default)]
struct General {
    color: Option<String>,
    hidden: bool,
    volume: Option<Volume>,
    size: Option<NoteSize>,
    tie: Option<Tie>,
    duration: Option<Duration>,
    articulations: Vec<Articulation>,
    expressions: Vec<Expression>,
    arpeggio_spanners: Vec<(String, ArpeggioType)>,
}

fn reduce(numerator: i64, denominator: i64) -> (i64, i64) {
    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a.abs() } else { gcd(b, a % b) }
    }
    let divisor = gcd(numerator, denominator).max(1);
    (numerator / divisor, denominator / divisor)
}

/// music21's `getStaffNumber`.
fn staff_number(element: &Xml) -> i32 {
    match element.tag.as_str() {
        "harmony" | "forward" | "note" | "direction" => {
            parse_int(element.child_text("staff")).unwrap_or(NO_STAFF)
        }
        "clef" | "staff-details" | "staff-layout" => {
            element.get("number").and_then(parse_int).unwrap_or(1)
        }
        "measure-style" | "key" | "time" | "transpose" => element
            .get("number")
            .and_then(parse_int)
            .unwrap_or(NO_STAFF),
        _ => NO_STAFF,
    }
}

/// music21's `xmlToPitch`.
fn pitch_of(note: &Xml) -> Result<Pitch> {
    let Some(written) = note.find("pitch") else {
        return Pitch::from_name("C4");
    };
    let step = written.child_text("step");
    let octave = written.child_text("octave");
    let mut pitch = Pitch::from_name(format!("{step}{octave}"))?;
    let alter = parse_float(written.child_text("alter"));
    let accidental = note
        .find("accidental")
        .filter(|written| !written.stripped().is_empty());
    match accidental {
        Some(written) => {
            if let Ok(mut read) = accidental_of(written) {
                read.set_display_status(Some(true));
                if let Some(alter) = alter
                    && alter != read.alter()
                {
                    read.set_alter_independently(alter);
                }
                pitch.set_written_accidental(Some(read));
            }
        }
        None => {
            if let Some(alter) = alter {
                let mut read = Accidental::new(alter)
                    .map_err(|_| import_error(format!("incorrect accidental {alter} for pitch")))?;
                read.set_display_status(Some(false));
                pitch.set_written_accidental(Some(read));
            }
        }
    }
    Ok(pitch)
}

/// music21's `xmlToAccidental`.
fn accidental_of(written: &Xml) -> Result<Accidental> {
    let name = written.stripped().to_lowercase();
    let name = match name.as_str() {
        "quarter-sharp" => "half-sharp",
        "three-quarters-sharp" => "one-and-a-half-sharp",
        "quarter-flat" => "half-flat",
        "three-quarters-flat" => "one-and-a-half-flat",
        "flat-flat" => "double-flat",
        "sharp-sharp" => "double-sharp",
        other => other,
    };
    let mut accidental = Accidental::natural();
    accidental.set_allowing_non_standard_value(name)?;
    if let Some(color) = written.get("color") {
        accidental.set_color(Some(color.to_string()));
    }
    match (yes(written.get("parentheses")), yes(written.get("bracket"))) {
        (true, true) => accidental.set_display_style("both")?,
        (true, false) => accidental.set_display_style("parentheses")?,
        (false, true) => accidental.set_display_style("bracket")?,
        (false, false) => {}
    }
    Ok(accidental)
}

/// music21's `xmlToBeams`.
fn beams_of(written: &[&Xml]) -> Result<Beams> {
    let mut beams = Beams::new();
    for (index, beam) in written.iter().enumerate() {
        let text = beam.text().map_or("begin", str::trim);
        let (beam_type, direction) = match text {
            "begin" => (BeamType::Start, None),
            "continue" => (BeamType::Continue, None),
            "end" => (BeamType::Stop, None),
            "forward hook" => (BeamType::PartialBeam, Some(BeamDirection::Right)),
            "backward hook" => (BeamType::PartialBeam, Some(BeamDirection::Left)),
            other => {
                return Err(import_error(format!(
                    "unexpected beam type encountered ({other})"
                )));
            }
        };
        let mut read = Beam::new(beam_type, direction);
        read.set_number(Some(index as u32 + 1));
        beams.beams_mut().push(read);
    }
    Ok(beams)
}

/// music21's `xmlToTie`.
fn tie_of(note: &Xml) -> Option<Tie> {
    let types: Vec<&str> = note
        .find_all("tie")
        .filter_map(|tie| tie.get("type"))
        .collect();
    let tie_type = if types.len() == 1 {
        TieType::from_name(types[0]).ok()?
    } else if types.contains(&"stop") && types.contains(&"start") {
        TieType::Continue
    } else {
        TieType::Start
    };
    let mut tie = Tie::new(tie_type);
    if let Some(first) = note
        .find("notations")
        .and_then(|notations| notations.find("tied"))
    {
        if let Some(style) = first.get("line-type").filter(|style| *style != "wavy")
            && let Ok(style) = TieStyle::from_name(style)
        {
            tie.set_style(style);
        }
        match first.get("placement") {
            Some(placement) => tie.set_placement(Placement::from_name(placement).ok()),
            None => match first.get("orientation") {
                Some("over") => tie.set_placement(Some(Placement::Above)),
                Some("under") => tie.set_placement(Some(Placement::Below)),
                _ => {}
            },
        }
    }
    Some(tie)
}

/// music21's `updateLyricsFromList` and `xmlToLyric`.
fn lyrics_from<'x>(written: impl Iterator<Item = &'x Xml>) -> Result<Vec<Lyric>> {
    let mut out = Vec::new();
    let mut current = 1;
    for lyric in written {
        let texts: Vec<&Xml> = lyric.find_all("text").collect();
        let syllabics: Vec<&Xml> = lyric.find_all("syllabic").collect();
        let elisions: Vec<&Xml> = lyric.find_all("elision").collect();
        let mut read = Lyric::unsung();
        if texts.is_empty() {
            // A lyric that only extends one: music21 hands back a bare one.
            out.push(read);
            current += 1;
            continue;
        }
        if texts.len() == 1 {
            if let Some(text) = texts[0].text() {
                read.set_text(text.trim());
            }
            if let Some(syllabic) = syllabics.first()
                && let Ok(syllabic) = Syllabic::from_name(syllabic.stripped())
            {
                read.set_syllabic(syllabic);
            }
        } else if texts.len() > 1 {
            let mut components = Vec::new();
            for (index, text) in texts.iter().enumerate() {
                let mut component = Lyric::unsung();
                if let Some(text) = text.text() {
                    component.set_text(text.trim());
                }
                if let Some(syllabic) = syllabics.get(index) {
                    if let Ok(syllabic) = Syllabic::from_name(syllabic.stripped()) {
                        component.set_syllabic(syllabic);
                    }
                    if index >= 1
                        && let Some(elision) = elisions.get(index - 1)
                    {
                        component.set_elision_before(elision.text().unwrap_or(""));
                    }
                }
                components.push(component);
            }
            read.set_components(components);
        }
        match lyric.get("number") {
            Some(number) => match number.trim().parse::<IntegerType>() {
                Ok(number) => read.set_number(number),
                Err(_) => {
                    read.set_number(0);
                    read.set_identifier(Some(number.to_string()));
                }
            },
            None => read.set_number(0),
        }
        if let Some(name) = lyric.get("name") {
            read.set_identifier(Some(name.to_string()));
        }
        if let Some(justify) = lyric.get("justify") {
            read.set_justify(Some(Justification::from_name(justify)?));
        }
        if let Some(placement) = lyric.get("placement") {
            read.set_placement(Placement::from_name(placement).ok());
        }
        // music21 reads `print-object` straight into `hideObjectOnPrint`,
        // so the lyric a file says to print is the one it hides.
        if let Some(printed) = lyric.get("print-object") {
            read.set_hidden(printed == "yes");
        }
        if let Some(color) = lyric.get("color") {
            read.set_color(Some(color.to_string()));
        }
        if read.number() == 0 {
            read.set_number(current);
        }
        out.push(read);
        current += 1;
    }
    Ok(out)
}

/// music21's `xmlToArticulation`.
fn articulation_of(mark: &Xml) -> Option<Articulation> {
    let class = match mark.tag.as_str() {
        "accent" => "Accent",
        "strong-accent" => "StrongAccent",
        "staccato" => "Staccato",
        "staccatissimo" => "Staccatissimo",
        "spiccato" => "Spiccato",
        "tenuto" => "Tenuto",
        "detached-legato" => "DetachedLegato",
        "scoop" => "Scoop",
        "plop" => "Plop",
        "doit" => "Doit",
        "falloff" => "Falloff",
        "breath-mark" => "BreathMark",
        "caesura" => "Caesura",
        "stress" => "Stress",
        "unstress" => "Unstress",
        "other-articulation" => "Articulation",
        _ => return None,
    };
    let mut articulation = Articulation::of_kind(ArticulationKind::from_class_name(class)?);
    if let Some(placement) = mark.get("placement") {
        articulation.set_placement(Some(placement.to_string()));
    }
    let text = mark.stripped();
    match mark.tag.as_str() {
        "strong-accent" => {
            if let Some(direction) = mark.get("type") {
                articulation.set_point_direction(Some(direction.to_string()));
            }
        }
        "breath-mark" if !text.is_empty() => articulation.set_symbol(Some(text.to_string())),
        "other-articulation" if !text.is_empty() => {
            articulation.set_display_text(Some(text.to_string()));
        }
        _ => {}
    }
    Some(articulation)
}

/// music21's `xmlTechnicalToArticulation`.
fn technical_of(mark: &Xml) -> Option<Articulation> {
    let class = match mark.tag.as_str() {
        "up-bow" => "UpBow",
        "down-bow" => "DownBow",
        "harmonic" => "StringHarmonic",
        "open-string" => "OpenString",
        "thumb-position" => "StringThumbPosition",
        "fingering" => "Fingering",
        "pluck" => "FrettedPluck",
        "double-tongue" => "DoubleTongue",
        "triple-tongue" => "TripleTongue",
        "stopped" => "Stopped",
        "snap-pizzicato" => "SnapPizzicato",
        "string" => "StringIndication",
        "bend" => "FretBend",
        "tap" => "FretTap",
        "fret" => "FretIndication",
        "heel" => "OrganHeel",
        "toe" => "OrganToe",
        "fingernails" => "HarpFingerNails",
        "handbell" => "HandbellIndication",
        "other-technical" => "TechnicalIndication",
        _ => return None,
    };
    let mut technical = Articulation::of_kind(ArticulationKind::from_class_name(class)?);
    let text = mark.stripped();
    match mark.tag.as_str() {
        "fingering" => {
            technical.set_finger(Some(Finger::from_written(mark.text().unwrap_or(""))));
            if let Some(substitution) = mark.get("substitution") {
                technical.set_substitution(substitution == "yes");
            }
            if let Some(alternate) = mark.get("alternate") {
                technical.set_alternate(alternate == "yes");
            }
        }
        "handbell" | "other-technical" if !text.is_empty() => {
            technical.set_display_text(Some(text.to_string()));
        }
        "fret" | "string" => {
            if let Some(number) = parse_int(text) {
                technical.set_number(number);
            }
        }
        "harmonic" => {
            if mark.find("artificial").is_some() {
                technical.set_harmonic_type(Some("artificial".to_string()));
            } else if mark.find("natural").is_some() {
                technical.set_harmonic_type(Some("natural".to_string()));
            }
        }
        "heel" | "toe" => {
            if let Some(substitution) = mark.get("substitution") {
                technical.set_substitution(substitution == "yes");
            }
        }
        _ => {}
    }
    if let Some(placement) = mark.get("placement") {
        technical.set_placement(Some(placement.to_string()));
    }
    Some(technical)
}

/// music21's `xmlToTempoIndication`, for a mark with one note value.
/// music21's `xmlToTempoIndication`: a `<metronome>` is a tempo mark, or
/// with two note values a metric modulation, placed as its direction says.
fn metronome(written: &Xml, placement: Option<Placement>) -> Result<StreamElement> {
    let mut referents: Vec<Duration> = Vec::new();
    let mut numbers: Vec<FloatType> = Vec::new();
    for child in &written.children {
        match child.tag.as_str() {
            "beat-unit" => {
                referents.push(Duration::from_type(duration_type(child.stripped())?));
            }
            "beat-unit-dot" => {
                let last = referents
                    .last_mut()
                    .ok_or_else(|| import_error("encountered metronome components out of order"))?;
                let (kind, dots) = last
                    .type_and_dots()
                    .ok_or_else(|| import_error("a beat unit with no written value"))?;
                *last = Duration::from_type_with_dots(kind, dots + 1);
            }
            "per-minute" => {
                if let Some(number) = parse_float(child.stripped()) {
                    numbers.push(number);
                }
            }
            _ => {}
        }
    }
    let parentheses = written.get("parentheses") == Some("yes");
    if referents.len() > 1 {
        // All a file gives are the two note values: neither side has a
        // number until the mark in force before it is known.
        let mut sides = referents.into_iter();
        let mut modulation = MetricModulation::new();
        if let Some(old) = sides.next() {
            modulation.set_old_referent(old, None);
        }
        if let Some(new) = sides.next() {
            modulation.set_new_referent(new);
        }
        modulation.set_parentheses(parentheses);
        modulation.set_placement(placement);
        return Ok(modulation.into());
    }
    let mut mark = MetronomeMark::default();
    if let Some(number) = numbers.first() {
        mark.set_number(Some(*number));
    }
    if let Some(referent) = referents.into_iter().next() {
        mark.set_referent(referent);
    }
    if parentheses {
        mark.set_parentheses(true);
    }
    mark.set_placement(placement);
    Ok(mark.into())
}

/// music21's `xmlToTimeSignature`.
fn time_signature(written: &Xml) -> Result<Option<TimeSignature>> {
    if written.find("senza-misura").is_some() {
        return Err(import_error(
            "reading a senza misura time signature is not supported yet",
        ));
    }
    let mut numerators = Vec::new();
    let mut denominators = Vec::new();
    for child in &written.children {
        match child.tag.as_str() {
            "beats" => numerators.push(child.stripped()),
            "beat-type" => denominators.push(child.stripped()),
            "interchangeable" => break,
            _ => {}
        }
    }
    let ratio = numerators
        .iter()
        .zip(&denominators)
        .map(|(numerator, denominator)| format!("{numerator}/{denominator}"))
        .collect::<Vec<_>>()
        .join("+");
    let mut meter = TimeSignature::from_ratio_string(&ratio)
        .map_err(|_| import_error(format!("Cannot process time signature {ratio}")))?;
    if let Some(color) = written.get("color") {
        meter.set_color(Some(color.to_string()));
    }
    meter.set_hidden(written.get("print-object") == Some("no"));
    match written.get("symbol") {
        Some(symbol @ ("common" | "cut" | "single-number" | "normal")) => {
            meter.set_symbol(Some(symbol.to_string()));
        }
        Some("note") => meter.set_symbolize_denominator(true),
        _ => {}
    }
    Ok(Some(meter))
}

/// music21's `xmlToClef`.
fn clef_of(written: &Xml) -> Result<Clef> {
    let sign = written.child_text("sign");
    let mut clef = if matches!(
        sign.to_lowercase().as_str(),
        "tab" | "percussion" | "none" | "jianpu"
    ) {
        Clef::from_string(sign, 0)?
    } else {
        let line = match written.find("line") {
            Some(line) => line.stripped(),
            None if sign == "G" => "2",
            None => "4",
        };
        let octave_change = parse_int(written.child_text("clef-octave-change")).unwrap_or(0);
        Clef::from_string(&format!("{sign}{line}"), octave_change)?
    };
    if let Some(color) = written.get("color") {
        clef.set_color(Some(color.to_string()));
    }
    Ok(clef)
}

/// music21's `xmlToKeySignature`: a key where the mode is said and is one
/// music21 knows, a signature otherwise.
fn key_signature(written: &Xml) -> Result<StreamElement> {
    let color = written.get("color").map(str::to_string);
    if written.find("fifths").is_none() {
        let mut steps = Vec::new();
        let mut alters = Vec::new();
        for child in &written.children {
            match child.tag.as_str() {
                "key-step" if !child.stripped().is_empty() => steps.push(child.stripped()),
                "key-alter" => {
                    if let Some(alter) = parse_float(child.stripped()) {
                        alters.push(alter);
                    }
                }
                _ => {}
            }
        }
        if steps.len() != alters.len() {
            return Err(import_error(
                "For non traditional signatures each step must have an alter",
            ));
        }
        let mut pitches = Vec::new();
        for (step, alter) in steps.into_iter().zip(alters) {
            let mut pitch = Pitch::from_name(step)?;
            pitch.set_written_accidental(Some(Accidental::new(alter)?));
            pitches.push(pitch);
        }
        let mut signature = KeySignature::from_altered_pitches(pitches);
        signature.set_color(color);
        return Ok(signature.into());
    }
    let sharps = parse_int(written.child_text("fifths")).unwrap_or(0);
    let mut signature = KeySignature::new(sharps);
    let mode = written.find("mode").and_then(Xml::text).unwrap_or("");
    if !mode.is_empty()
        && let Ok(mut key) = signature.try_as_key(Some(mode), None)
    {
        key.set_color(color);
        return Ok(key.into());
    }
    signature.set_color(color);
    Ok(signature.into())
}

/// music21's `xmlTransposeToInterval`.
fn transpose_interval(written: &Xml) -> Result<Interval> {
    let mut diatonic = written
        .find("diatonic")
        .and_then(|d| parse_int(d.stripped()));
    let chromatic = written
        .find("chromatic")
        .and_then(|c| parse_int(c.stripped()));
    let mut octave_change = 0;
    if let Some(octaves) = written
        .find("octave-change")
        .and_then(|o| parse_int(o.stripped()))
    {
        octave_change = octaves * 12;
        if let Some(diatonic) = &mut diatonic {
            *diatonic += 7 * octaves;
        }
    }
    match (diatonic, chromatic) {
        (Some(diatonic), Some(chromatic)) => {
            let actual = |steps: IntegerType| if steps < 0 { steps - 1 } else { steps + 1 };
            Interval::from_generic_and_chromatic(actual(diatonic), chromatic + octave_change)
                .or_else(|_| {
                    let steps = diatonic - octave_change * 7 / 12;
                    Interval::from_generic_and_chromatic(actual(steps), chromatic + octave_change)
                })
        }
        (None, Some(chromatic)) => Interval::from_semitones(chromatic + octave_change),
        _ => Interval::from_name("P1"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musicxml::{ExportOptions, to_musicxml};

    /// A score of one part holding these measures.
    fn score(measures: &str) -> String {
        format!(
            "<?xml version=\"1.0\"?><score-partwise version=\"4.0\"><part-list>             <score-part id=\"P1\"><part-name>Flute</part-name></score-part></part-list>             <part id=\"P1\">{measures}</part></score-partwise>"
        )
    }

    const NOTE: &str = "<note><pitch><step>C</step><octave>4</octave></pitch>                        <duration>1</duration><type>quarter</type></note>";

    #[test]
    fn offsets_are_snapped_as_music21_snaps_them() {
        // music21's `opFrac`: a third is a third, not the float nearest it.
        assert_eq!(op_frac(0.333_333_333_333_3), 1.0 / 3.0);
        assert_eq!(op_frac(0.25), 0.25);
        assert_eq!(op_frac(1.0 / 3.0 + 1.0 / 3.0 + 1.0 / 3.0), 1.0);
        assert_eq!(op_frac(-2.0 / 3.0), -2.0 / 3.0);
    }

    #[test]
    fn an_ending_is_numbered_as_music21_numbers_it() {
        assert_eq!(bracket_numbers(""), [0]);
        assert_eq!(bracket_numbers("2"), [2]);
        assert_eq!(bracket_numbers("1, 2"), [1, 2]);
        assert_eq!(bracket_numbers("1-3"), [1, 2, 3]);
        assert_eq!(bracket_numbers("first"), [1]);
    }

    #[test]
    fn a_part_and_its_notes_are_read() {
        let read = from_musicxml(&score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions>             <key><fifths>-1</fifths><mode>major</mode></key>             <time><beats>2</beats><beat-type>4</beat-type></time>             <clef><sign>G</sign><line>2</line></clef></attributes>{NOTE}             <note><rest/><duration>1</duration><type>quarter</type></note></measure>"
        )))
        .unwrap();
        let part = read.parts()[0];
        assert_eq!(part.name(), Some("Flute"));
        let measure = part.measures()[0];
        assert_eq!(measure.number(), 1);
        let kinds: Vec<&str> = measure
            .events()
            .iter()
            .map(|event| match event.element() {
                StreamElement::Clef(_) => "clef",
                StreamElement::Key(_) => "key",
                StreamElement::TimeSignature(_) => "time",
                StreamElement::Note(_) => "note",
                StreamElement::Rest(_) => "rest",
                _ => "other",
            })
            .collect();
        // Sorted as music21 sorts a measure, whatever order the file had.
        assert_eq!(kinds, ["clef", "key", "time", "note", "rest"]);
        assert_eq!(measure.events()[4].offset(), 1.0);
    }

    #[test]
    fn notes_sharing_a_stem_are_a_chord() {
        let read = from_musicxml(&score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{NOTE}             <note><chord/><pitch><step>E</step><octave>4</octave></pitch>             <duration>1</duration><type>quarter</type></note></measure>"
        )))
        .unwrap();
        let measure = read.parts()[0].measures()[0];
        let chords: Vec<_> = measure
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Chord(chord) => Some(chord),
                _ => None,
            })
            .collect();
        assert_eq!(chords.len(), 1);
        assert_eq!(chords[0].notes().len(), 2);
    }

    #[test]
    fn two_voices_are_two_streams() {
        let voiced = |voice: u8| {
            format!(
                "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>                 <voice>{voice}</voice><type>quarter</type></note>"
            )
        };
        let read = from_musicxml(&score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{}             <backup><duration>1</duration></backup>{}</measure>",
            voiced(1),
            voiced(2)
        )))
        .unwrap();
        let measure = read.parts()[0].measures()[0];
        let voices = measure.voices();
        assert_eq!(voices.len(), 2);
        assert_eq!(voices[1].id(), Some("2"));
        // The second voice was backed up to the start of the measure.
        assert_eq!(voices[1].events()[0].offset(), 0.0);
    }

    #[test]
    fn a_slur_joins_the_notes_it_is_written_on() {
        let slurred = |kind: &str| {
            format!(
                "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>                 <type>quarter</type><notations><slur number=\"1\" type=\"{kind}\"/>                 </notations></note>"
            )
        };
        let read = from_musicxml(&score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>{}{NOTE}{}             </measure>",
            slurred("start"),
            slurred("stop")
        )))
        .unwrap();
        let spanners = read.spanners();
        assert_eq!(spanners.len(), 1);
        assert_eq!(spanners[0].kind(), SpannerKind::Slur);
        let leaves = read.leaves();
        let ends: Vec<usize> = spanners[0].spanned().iter().flatten().copied().collect();
        assert_eq!(ends.len(), 2);
        assert!(matches!(leaves[ends[0]].1, StreamElement::Note(_)));
        assert_eq!(leaves[ends[1]].0, 2.0);
    }

    #[test]
    fn a_hairpin_started_after_a_part_ends_takes_the_next_part_first_note() {
        let wedge = |kind: &str| {
            format!(
                "<direction><direction-type><wedge type=\"{kind}\"/></direction-type></direction>"
            )
        };
        let measure = |step: &str, after: &str| {
            format!(
                "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
                 <note><pitch><step>{step}</step><octave>4</octave></pitch><duration>1</duration>\
                 <type>quarter</type></note>{after}</measure>"
            )
        };
        let document = format!(
            "<?xml version=\"1.0\"?><score-partwise version=\"4.0\"><part-list>\
             <score-part id=\"P1\"><part-name>One</part-name></score-part>\
             <score-part id=\"P2\"><part-name>Two</part-name></score-part></part-list>\
             <part id=\"P1\">{}</part><part id=\"P2\">{}</part></score-partwise>",
            measure("C", &(wedge("diminuendo") + &wedge("stop"))),
            measure("D", ""),
        );
        let read = from_musicxml(&document).unwrap();
        let spanners = read.spanners();
        assert_eq!(spanners.len(), 1);
        assert_eq!(spanners[0].kind(), SpannerKind::Diminuendo);
        let leaves = read.leaves();
        let named: Vec<String> = spanners[0]
            .spanned()
            .iter()
            .flatten()
            .map(|position| match leaves[*position].1 {
                StreamElement::Note(note) => note.pitch().name_with_octave(),
                _ => String::new(),
            })
            .collect();
        // The stop names the note it follows, and the queue then puts the
        // next note read, in the next part, first.
        assert_eq!(named, ["D4", "C4"]);
    }

    #[test]
    fn a_part_on_two_staves_is_two_parts_in_a_brace() {
        let on_staff = |staff: u8| {
            format!(
                "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>                 <type>quarter</type><staff>{staff}</staff></note>"
            )
        };
        let read = from_musicxml(&score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions><staves>2</staves>             </attributes>{}<backup><duration>1</duration></backup>{}</measure>",
            on_staff(1),
            on_staff(2)
        )))
        .unwrap();
        let parts = read.parts();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].kind(), StreamKind::PartStaff);
        assert_eq!(parts[1].id(), Some("P1-Staff2"));
        let groups = read.staff_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].parts(), [0, 1]);
        assert_eq!(groups[0].symbol(), Some("brace"));
    }

    #[test]
    fn what_is_read_is_written_back() {
        let document = score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions>             <time><beats>1</beats><beat-type>4</beat-type></time>             <clef><sign>G</sign><line>2</line></clef></attributes>{NOTE}</measure>"
        ));
        let written = to_musicxml(
            &from_musicxml(&document).unwrap(),
            &ExportOptions::default(),
        )
        .unwrap();
        assert!(written.contains("<part-name>Flute</part-name>"));
        assert!(written.contains("<step>C</step>"));
        assert!(written.contains("<type>quarter</type>"));
        assert!(written.contains("<beat-type>4</beat-type>"));
    }

    /// A quarter note carrying these notations.
    fn noted(notations: &str) -> String {
        format!(
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>\
             <type>quarter</type><notations>{notations}</notations></note>"
        )
    }

    /// One measure of a quarter to the division holding these.
    fn measure_of(contents: &str) -> String {
        score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
             {contents}</measure>"
        ))
    }

    fn written_back(read: &Stream) -> String {
        to_musicxml(read, &ExportOptions::default()).unwrap()
    }

    #[test]
    fn a_glissando_and_a_slide_are_read() {
        let read = from_musicxml(&measure_of(&format!(
            "{}{}{}",
            noted("<glissando number=\"1\" type=\"start\" line-type=\"dashed\">gl.</glissando>"),
            noted("<glissando number=\"1\" type=\"stop\"/><slide number=\"2\" type=\"start\"/>"),
            noted("<slide number=\"2\" type=\"stop\"/>"),
        )))
        .unwrap();
        let spanners = read.spanners();
        assert_eq!(spanners.len(), 2);
        assert!(
            spanners
                .iter()
                .all(|spanner| spanner.kind() == SpannerKind::Glissando && spanner.len() == 2)
        );
        // music21 reads the line and how it is played, and not the words.
        assert_eq!(spanners[0].line_type(), Some("dashed"));
        assert_eq!(
            spanners[0].glissando_details(),
            Some(&Glissando {
                slide_type: SlideType::Chromatic,
                label: None
            })
        );
        // A slide is drawn solid where nothing says.
        assert_eq!(spanners[1].line_type(), Some("solid"));
        assert_eq!(
            spanners[1]
                .glissando_details()
                .map(|slide| slide.slide_type),
            Some(SlideType::Continuous)
        );
        let written = written_back(&read);
        assert!(written.contains("<glissando line-type=\"dashed\" number=\"1\" type=\"start\" />"));
        assert!(written.contains("<slide line-type=\"solid\" number=\"2\" type=\"stop\" />"));
    }

    #[test]
    fn a_glissando_drawn_no_way_music21_knows_is_refused() {
        let read = from_musicxml(&measure_of(&noted(
            "<glissando number=\"1\" type=\"start\" line-type=\"zigzag\"/>",
        )));
        assert!(read.is_err());
    }

    #[test]
    fn a_tremolo_that_starts_and_stops_is_a_spanner() {
        let read = from_musicxml(&measure_of(&format!(
            "{}{}{}",
            noted("<ornaments><tremolo type=\"start\" placement=\"above\">2</tremolo></ornaments>"),
            noted("<ornaments><tremolo type=\"stop\">2</tremolo></ornaments>"),
            noted("<ornaments><tremolo type=\"single\">1</tremolo></ornaments>"),
        )))
        .unwrap();
        let spanners = read.spanners();
        assert_eq!(spanners.len(), 1);
        assert_eq!(spanners[0].kind(), SpannerKind::TremoloSpanner);
        assert_eq!(spanners[0].number_of_marks(), Some(2));
        assert_eq!(spanners[0].placement(), Some(Placement::Above));
        assert_eq!(spanners[0].len(), 2);
        // The tremolo on one note is an ornament of that note still.
        let ornamented = read
            .leaves()
            .iter()
            .filter(|(_, element)| match element {
                StreamElement::Note(note) => !note.expressions().is_empty(),
                _ => false,
            })
            .count();
        assert_eq!(ornamented, 1);
        let written = written_back(&read);
        assert!(written.contains("<tremolo placement=\"above\" type=\"start\">2</tremolo>"));
        assert!(written.contains("<tremolo type=\"stop\">2</tremolo>"));
    }

    fn pedal(attributes: &str) -> String {
        format!("<direction><direction-type><pedal {attributes}/></direction-type></direction>")
    }

    #[test]
    fn a_pedal_keeps_its_bounces_and_gaps() {
        let read = from_musicxml(&measure_of(&format!(
            "{}{NOTE}{}{NOTE}{}{NOTE}{}{NOTE}{}",
            pedal("type=\"start\" line=\"yes\" number=\"1\""),
            pedal("type=\"change\" line=\"yes\" number=\"1\""),
            pedal("type=\"discontinue\" line=\"yes\" number=\"1\""),
            pedal("type=\"resume\" line=\"yes\" number=\"1\""),
            pedal("type=\"stop\" line=\"yes\" number=\"1\""),
        )))
        .unwrap();
        let spanners = read.spanners();
        assert_eq!(spanners.len(), 1);
        assert_eq!(spanners[0].kind(), SpannerKind::PedalMark);
        let leaves = read.leaves();
        let joined: Vec<String> = spanners[0]
            .spanned()
            .iter()
            .flatten()
            .map(|position| match leaves[*position] {
                (offset, StreamElement::PedalObject(object)) => {
                    format!("{} at {offset}", object.kind().class_name())
                }
                (offset, _) => format!("note at {offset}"),
            })
            .collect();
        assert_eq!(
            joined,
            [
                "note at 0",
                "PedalBounce at 1",
                "PedalGapStart at 2",
                "PedalGapEnd at 3",
                "note at 3"
            ]
        );
        let written = written_back(&read);
        for kind in ["change", "discontinue", "resume"] {
            assert!(written.contains(&format!("<pedal line=\"yes\" type=\"{kind}\" />")));
        }
    }

    #[test]
    fn a_sign_with_a_line_resumed_at_once_is_one_pedal() {
        let read = from_musicxml(&measure_of(&format!(
            "{NOTE}{}{}{NOTE}{}",
            pedal("type=\"start\" sign=\"yes\" number=\"1\""),
            pedal("type=\"resume\" line=\"yes\" number=\"1\""),
            pedal("type=\"stop\" line=\"yes\" number=\"1\""),
        )))
        .unwrap();
        let spanners = read.spanners();
        assert_eq!(
            spanners[0].pedal().and_then(|pedal| pedal.form),
            Some(PedalForm::SymbolLine)
        );
        assert!(
            read.leaves()
                .iter()
                .all(|(_, element)| !matches!(element, StreamElement::PedalObject(_)))
        );
        // The line resumes where the pedal mark stands, the start of the
        // part, a quarter before the note it goes down on.
        let written = written_back(&read);
        assert!(written.contains(
            "<pedal line=\"yes\" type=\"resume\" />\n        </direction-type>\n        \
             <offset sound=\"yes\">-10080</offset>"
        ));
    }

    #[test]
    fn a_barline_inside_a_measure_stands_where_music21_appends_it() {
        let barline = "<barline location=\"middle\"><bar-style>dashed</bar-style></barline>";
        let inner = |document: &str| -> Vec<FloatType> {
            let read = from_musicxml(document).unwrap();
            let measure = read.parts()[0].measures()[0];
            measure
                .events()
                .iter()
                .filter(|event| matches!(event.element(), StreamElement::Barline(_)))
                .map(StreamEvent::offset)
                .collect()
        };
        // After what has been read, wherever the file would have it.
        assert_eq!(
            inner(&measure_of(&format!(
                "{NOTE}{NOTE}{barline}{NOTE}{NOTE}{barline}"
            ))),
            [2.0, 4.0]
        );
        // And at the start of a measure whose notes are all in voices.
        let voiced = |voice: u8| {
            format!(
                "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>\
                 <voice>{voice}</voice><type>quarter</type></note>"
            )
        };
        assert_eq!(
            inner(&measure_of(&format!(
                "{}{barline}<backup><duration>1</duration></backup>{}",
                voiced(1),
                voiced(2)
            ))),
            [0.0]
        );
        // Nothing is written for one.
        let read = from_musicxml(&measure_of(&format!("{NOTE}{barline}{NOTE}"))).unwrap();
        assert!(!written_back(&read).contains("<barline"));
    }

    #[test]
    fn a_lyric_says_how_it_is_drawn() {
        let read = from_musicxml(&measure_of(
            "<note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>\
             <type>quarter</type><lyric number=\"1\" justify=\"left\" placement=\"below\" \
             print-object=\"yes\"><syllabic>single</syllabic><text>la</text></lyric></note>",
        ))
        .unwrap();
        let leaves = read.leaves();
        let note = leaves
            .iter()
            .find_map(|(_, element)| match element {
                StreamElement::Note(note) => Some(note),
                _ => None,
            })
            .expect("the measure holds a note");
        let lyric = &note.lyrics()[0];
        assert_eq!(lyric.justify(), Some(Justification::Left));
        assert_eq!(lyric.placement(), Some(Placement::Below));
        // music21 hides the lyric a file says to print.
        assert!(lyric.is_hidden());
        assert!(written_back(&read).contains(
            "<lyric justify=\"left\" name=\"1\" number=\"1\" placement=\"below\" \
             print-object=\"no\">"
        ));
    }

    #[test]
    fn a_rehearsal_mark_is_read_and_written_centred() {
        let read = from_musicxml(&measure_of(&format!(
            "<direction placement=\"above\"><direction-type>             <rehearsal enclosure=\"square\">A</rehearsal></direction-type></direction>{NOTE}"
        )))
        .unwrap();
        let leaves = read.leaves();
        let mark = leaves
            .iter()
            .find_map(|(_, element)| match element {
                StreamElement::RehearsalMark(mark) => Some(mark),
                _ => None,
            })
            .expect("the measure holds a rehearsal mark");
        assert_eq!(mark.content(), "A");
        assert_eq!(mark.enclosure(), Some("square"));
        assert_eq!(mark.placement(), Some(Placement::Above));
        assert!(written_back(&read).contains(
            "<rehearsal enclosure=\"square\" halign=\"center\" valign=\"middle\">A</rehearsal>"
        ));
        // music21 raises on an enclosure it has no name for.
        assert!(
            from_musicxml(&measure_of(
                "<direction><direction-type><rehearsal enclosure=\"star\">A</rehearsal>                 </direction-type></direction>"
            ))
            .is_err()
        );
    }

    #[test]
    fn a_time_signature_may_be_left_unprinted() {
        let read = from_musicxml(&score(
            "<measure number=\"1\"><attributes><divisions>1</divisions>             <time print-object=\"no\"><beats>3</beats><beat-type>4</beat-type></time>             </attributes></measure>",
        ))
        .unwrap();
        let leaves = read.leaves();
        let meter = leaves
            .iter()
            .find_map(|(_, element)| match element {
                StreamElement::TimeSignature(meter) => Some(meter),
                _ => None,
            })
            .expect("the measure holds a time signature");
        assert!(meter.is_hidden());
        assert!(written_back(&read).contains("<time print-object=\"no\">"));
    }

    #[test]
    fn a_metric_modulation_takes_its_numbers_from_the_tempo_before_it() {
        // Read off music21: a dotted quarter at 110, then a quarter becoming
        // a dotted quarter, sounds at 247.5 quarters a minute.
        let read = from_musicxml(&score(&format!(
            "<measure number=\"1\"><attributes><divisions>1</divisions></attributes>\
             <direction><direction-type><metronome><beat-unit>quarter</beat-unit>\
             <beat-unit-dot/><per-minute>110</per-minute></metronome></direction-type>\
             </direction>{NOTE}</measure><measure number=\"2\">\
             <direction placement=\"above\"><direction-type><metronome>\
             <beat-unit>quarter</beat-unit><beat-unit>quarter</beat-unit><beat-unit-dot/>\
             </metronome></direction-type></direction>{NOTE}</measure>"
        )))
        .unwrap();
        let leaves = read.leaves();
        let modulation = leaves
            .iter()
            .find_map(|(_, element)| match element {
                StreamElement::MetricModulation(modulation) => Some(modulation),
                _ => None,
            })
            .expect("the second measure holds a metric modulation");
        // As read, neither side has a number: a file gives the note values.
        assert_eq!(
            modulation.old_metronome().and_then(|old| old.number()),
            None
        );
        assert_eq!(modulation.placement(), Some(Placement::Above));
        let written = written_back(&read);
        assert!(written.contains(
            "<metronome parentheses=\"no\">\n            <beat-unit>quarter</beat-unit>\n            \
             <beat-unit>quarter</beat-unit>\n            <beat-unit-dot />\n          \
             </metronome>\n        </direction-type>\n        <sound tempo=\"247.5\" />"
        ));
    }

    #[test]
    fn words_naming_a_coda_place_no_sign() {
        let words = |text: &str| {
            format!(
                "<direction placement=\"above\"><direction-type><words>{text}</words>                 </direction-type></direction>"
            )
        };
        let read = from_musicxml(&measure_of(&format!(
            "{}{NOTE}{}",
            words("Coda"),
            words("Fine")
        )))
        .unwrap();
        let placed: Vec<Option<Placement>> = read
            .leaves()
            .iter()
            .filter_map(|(_, element)| match element {
                StreamElement::RepeatExpression(mark) => Some(mark.placement()),
                _ => None,
            })
            .collect();
        // The coda is drawn as its sign, which music21 places nowhere; the
        // words of a fine stay where they were written.
        assert_eq!(placed, [None, Some(Placement::Above)]);
    }

    #[test]
    fn a_document_that_is_not_a_score_is_refused() {
        assert!(from_musicxml("<opus/>").is_err());
        assert!(from_musicxml("not xml at all").is_err());
    }
}
