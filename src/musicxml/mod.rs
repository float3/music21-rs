//! MusicXML export: music21's `musicxml.m21ToXml`.
//!
//! [`to_musicxml`] writes a score as a `score-partwise` document, element for
//! element as music21's `ScoreExporter` writes it with `makeNotation=False`:
//! the score is taken as already notated, so its measures, beams, ties and
//! the accidentals it shows are written as they stand rather than worked out
//! again. A score is parts holding measures, a measure may hold voices, and a
//! stream holding no parts is written as one part.
//!
//! [`ExportOptions::make_notation`] asks for the notation to be worked out
//! first, as music21's exporter does by default: a part of loose notes, or a
//! score whose lengths no single note value writes, is then written as
//! music21 writes it.
//!
//! The writer does no file IO; it hands back the document as a string.

mod notate;
mod partstaff;
mod read;
mod tree;

pub use read::from_musicxml;

use crate::articulations::Articulation;
use crate::bar::{Barline, Ending, RepeatDirection};
use crate::chord::Chord;
use crate::chordsymbol::ChordSymbol;
use crate::clef::Clef;
use crate::defaults::FloatType;
use crate::duration::{Duration, DurationType, Tuplet};
use crate::dynamics::Dynamic;
use crate::error::{Error, Result};
use crate::expressions::{ArpeggioType, Expression, Ornament};
use crate::instrument::Instrument;
use crate::interval::Interval;
use crate::key::KeySignature;
use crate::metadata::{Metadata, namespace_name};
use crate::meter::TimeSignature;
use crate::notation::{
    Beam, BeamDirection, BeamType, Beams, Lyric, Notehead, Placement, StemDirection, Tie,
};
use crate::note::Note;
use crate::pitch::{Accidental, Pitch};
use crate::repeat::{RepeatExpression, RepeatExpressionKind};
use crate::rest::Rest;
use crate::spanner::{
    PedalForm, PedalObject, PedalObjectKind, PedalType, SlideType, Spanner, SpannerKind,
};
use crate::stream::{StaffGroup, Stream, StreamElement, StreamKind};
use crate::tempo::{MetricModulation, MetronomeMark};

use tree::Element;

/// How many divisions of a quarter note durations are counted in: music21's
/// `defaults.divisionsPerQuarter`, `32 * 3 * 3 * 5 * 7`, which counts every
/// tuplet music21 writes in whole numbers.
pub const DIVISIONS_PER_QUARTER: FloatType = 10080.0;

/// The MusicXML version written.
pub const MUSICXML_VERSION: &str = "4.0";

/// What a document says about how it was made, where the score does not say.
///
/// The defaults are music21's own, so that a score written here reads as the
/// one music21 writes; a caller making a document of its own will want its
/// own title and author, or none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    /// The `<encoding-date>`, as `YYYY-MM-DD`. music21 writes the day it ran;
    /// the crate reads no clock, so the date is written only when given.
    pub encoding_date: Option<String>,
    /// The `<software>` line written when the score carries no metadata.
    /// A score with metadata names its software there instead.
    pub software: String,
    /// The movement title of a score with no title of its own: music21's
    /// `defaults.title`, `Music21 Fragment`.
    pub default_title: Option<String>,
    /// The composer named when the score names no contributor: music21's
    /// `defaults.author`, `Music21`.
    pub default_author: Option<String>,
    /// Whether to work out the notation the score leaves unsaid before
    /// writing it, as music21's exporter does with `makeNotation=True`:
    /// gaps filled with unprinted rests, a part with no measures cut into
    /// them, notes running past a barline cut and tied, accidentals decided,
    /// notes beamed and tuplets bracketed where the part has none yet, and
    /// lengths no single note value writes cut into tied values. A measure
    /// or a voice handed in alone is written as a part of one measure, given
    /// the meter and clef its notes fit where it states none. The score
    /// handed in is not changed.
    ///
    /// Off, the score is written as it stands, which is music21's
    /// `makeNotation=False`: a caller that has made its notation already
    /// gets exactly what it made.
    pub make_notation: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            encoding_date: None,
            software: format!("music21-rs v.{}", env!("CARGO_PKG_VERSION")),
            default_title: Some("Music21 Fragment".to_string()),
            default_author: Some("Music21".to_string()),
            make_notation: false,
        }
    }
}

/// Writes a score as a MusicXML document.
///
/// ```
/// use music21_rs::{Duration, Note, Stream, StreamKind, TimeSignature};
/// use music21_rs::musicxml::{ExportOptions, to_musicxml};
///
/// let mut measure = Stream::with_kind(StreamKind::Measure);
/// measure.set_number(1);
/// measure.insert(0.0, TimeSignature::new(1, 4)?);
/// measure.push(Note::from_name("D#4")?.with_duration(Duration::quarter()));
/// let mut part = Stream::with_kind(StreamKind::Part);
/// part.push(measure);
/// let mut score = Stream::with_kind(StreamKind::Score);
/// score.push(part);
///
/// let xml = to_musicxml(&score, &ExportOptions::default())?;
/// assert!(xml.contains("<step>D</step>"));
/// assert!(xml.contains("<alter>1</alter>"));
/// assert!(xml.contains("<duration>10080</duration>"));
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// With [`ExportOptions::make_notation`] a part or a stream of loose notes
/// may be handed in as readily as a score:
///
/// ```
/// use music21_rs::{Duration, Note, Stream, StreamKind, TimeSignature};
/// use music21_rs::musicxml::{ExportOptions, to_musicxml};
///
/// let mut part = Stream::with_kind(StreamKind::Part);
/// part.insert(0.0, TimeSignature::new(3, 4)?);
/// part.insert(0.0, Note::from_name("C4")?.with_duration(Duration::new(5.0)?));
/// let options = ExportOptions {
///     make_notation: true,
///     ..ExportOptions::default()
/// };
/// let xml = to_musicxml(&part, &options)?;
/// // A dotted half tied to a half, in two measures.
/// assert_eq!(xml.matches("<measure ").count(), 2);
/// assert!(xml.contains("<tie type=\"start\" />"));
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// A part with no measures, a score nested in a score, a duration no single
/// note value writes, and anything the crate cannot yet write: each as
/// music21's `MusicXMLExportException` says it where music21 raises. Making
/// the notation first leaves only what no notation can write: a length no
/// tie of note values reaches that is not an unprinted rest.
pub fn to_musicxml(score: &Stream, options: &ExportOptions) -> Result<String> {
    // music21's exporter turns a part at sounding pitch to the pitch its
    // instruments read (`toWrittenPitch`) before it writes; making the
    // notation does that itself, at the point music21 does it.
    let prepared;
    let score = if options.make_notation {
        prepared = notate::notated(score)?;
        &prepared
    } else if holds_sounding_pitch(score) {
        prepared = score.to_written_pitch()?;
        &prepared
    } else {
        score
    };
    let root = score_element(score, options)?;
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE score-partwise  PUBLIC \
         \"-//Recordare//DTD MusicXML {MUSICXML_VERSION} Partwise//EN\" \
         \"http://www.musicxml.org/dtds/partwise.dtd\">\n{}",
        root.dump()
    ))
}

/// Whether anything in the stream, itself included, says it is at sounding
/// pitch: nothing else is moved by writing it at written pitch.
fn holds_sounding_pitch(stream: &Stream) -> bool {
    stream.at_sounding_pitch() == Some(true)
        || stream
            .events()
            .iter()
            .filter_map(|event| event.element().as_stream())
            .any(holds_sounding_pitch)
}

fn export_error(message: impl Into<String>) -> Error {
    Error::MusicXml(message.into())
}

// ------------------------------------------------------------------ score

/// music21's `ScoreExporter.parse`.
fn score_element(score: &Stream, options: &ExportOptions) -> Result<Element> {
    if score.is_empty() {
        return score_element(&empty_score(), options);
    }
    let parts: Vec<&Stream> = if has_part_like_streams(score) {
        for event in score.events() {
            if let StreamElement::Stream(inner) = event.element()
                && matches!(inner.kind(), StreamKind::Score | StreamKind::Opus)
            {
                return Err(export_error(
                    "Exporting scores nested inside scores is not supported",
                ));
            }
        }
        score
            .events()
            .iter()
            .filter_map(|event| event.element().as_stream())
            .filter(|stream| matches!(stream.kind(), StreamKind::Part | StreamKind::PartStaff))
            .collect()
    } else {
        vec![score]
    };

    let groups = score.staff_groups();
    let joined = joinable_groups(&parts, groups);
    let voice_bases = voice_bases(&parts, groups, &joined);
    let numbered = id_locals(score, &parts);
    let mut shared = Shared::default();
    let mut exported = Vec::with_capacity(parts.len());
    for (index, part) in parts.iter().copied().enumerate() {
        let spanners = numbered.clone();
        let mut role = StaffRole::default();
        for group in &joined {
            let parts_of = groups[*group].parts();
            if let Some(at) = parts_of.iter().position(|position| *position == index) {
                role.previous = at.checked_sub(1).map(|before| parts[parts_of[before]]);
                role.later = at > 0;
                if at == 0 {
                    role.followers = parts_of[1..]
                        .iter()
                        .map(|position| parts[*position])
                        .collect();
                }
            }
        }
        let previous = role.previous;
        exported.push(Some(PartExporter::new(part, &mut shared, &role)?.parse(
            spanners,
            previous,
            &voice_bases,
        )?));
    }

    // music21's `joinPartStaffs`: each group's staves written into its first.
    for group in &joined {
        let positions = groups[*group].parts();
        let mut roots: Vec<Element> = positions
            .iter()
            .map(|position| {
                exported[*position]
                    .as_mut()
                    .map(|part| std::mem::replace(&mut part.part, Element::new("part")))
                    .expect("a staff is joined into one group only")
            })
            .collect();
        let staves: Vec<&Stream> = positions.iter().map(|position| parts[*position]).collect();
        partstaff::join(
            &mut roots,
            differs_across(&staves, first_key, same_key),
            differs_across(&staves, first_meter, same_meter),
        )?;
        let first = roots.swap_remove(0);
        if let Some(part) = exported[positions[0]].as_mut() {
            part.part = first;
        }
        for position in &positions[1..] {
            exported[*position] = None;
        }
    }

    let mut root = Element::new("score-partwise");
    root.set("version", MUSICXML_VERSION);
    let metadata = score.metadata();
    set_titles(&mut root, metadata, options);
    set_identification(&mut root, metadata, options);
    set_defaults(&mut root);
    set_part_list(root.sub("part-list"), &exported, groups, &joined);
    for (index, part) in exported.into_iter().flatten().enumerate() {
        add_divider_comment(&mut root, &format!("Part {}", index + 1));
        root.push(part.part);
    }
    Ok(root)
}

/// Where a part stands among the staves of a group written as one part.
#[derive(Default)]
struct StaffRole<'a> {
    /// The staff before it in its group.
    previous: Option<&'a Stream>,
    /// The staves after it, where it is its group's first.
    followers: Vec<&'a Stream>,
    /// Whether it is a staff after its group's first.
    later: bool,
}

/// music21's `joinableGroups`: the staff groups whose staves are written as
/// one part, by index into the score's groups. A group of more than one
/// `PartStaff`, each with measures; a second group over the same staves is
/// passed over; and if any two overlap, none is joined.
fn joinable_groups(parts: &[&Stream], groups: &[StaffGroup]) -> Vec<usize> {
    let mut joinable: Vec<usize> = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let staves = group.parts();
        if staves.len() <= 1 {
            continue;
        }
        let fits = staves.iter().all(|position| {
            parts.get(*position).is_some_and(|part| {
                part.kind() == StreamKind::PartStaff && !part.measures().is_empty()
            })
        });
        if !fits
            || joinable
                .iter()
                .any(|known| groups[*known].parts() == staves)
        {
            continue;
        }
        joinable.push(index);
    }
    let mut seen: Vec<usize> = Vec::new();
    for index in &joinable {
        for position in groups[*index].parts() {
            if seen.contains(position) {
                return Vec::new();
            }
            seen.push(*position);
        }
    }
    joinable
}

/// music21's `renumberVoicesWithinStaffGroups`: in a group of staves, a voice
/// with no id of its own is numbered on from the voices of the staves above
/// it in the measures starting where its measure does. Each measure is
/// given the number its first such voice takes after one.
fn voice_bases(
    parts: &[&Stream],
    groups: &[StaffGroup],
    joined: &[usize],
) -> std::collections::HashMap<usize, u32> {
    let mut bases = std::collections::HashMap::new();
    for group in joined {
        let mut measures: Vec<(FloatType, usize, &Stream)> = Vec::new();
        for (staff, position) in groups[*group].parts().iter().enumerate() {
            for event in parts[*position].events() {
                if let Some(measure) = event
                    .element()
                    .as_stream()
                    .filter(|stream| stream.kind() == StreamKind::Measure)
                {
                    measures.push((event.offset(), staff, measure));
                }
            }
        }
        measures.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        let mut next = 0;
        let mut stack_offset = None;
        for (offset, _, measure) in measures {
            if stack_offset != Some(offset) {
                stack_offset = Some(offset);
                next = 0;
            }
            bases.insert(measure as *const Stream as usize, next);
            next += measure
                .voices()
                .iter()
                .filter(|voice| voice.id().is_none())
                .count() as u32;
        }
    }
    bases
}

/// The first of some kind of mark anywhere in a staff.
fn first_of<'a>(
    staff: &'a Stream,
    pick: impl Fn(&'a StreamElement) -> bool,
) -> Option<&'a StreamElement> {
    staff
        .recurse()
        .into_iter()
        .map(|(_, element)| element)
        .find(|element| pick(element))
}

fn first_key(staff: &Stream) -> Option<&StreamElement> {
    first_of(staff, |element| {
        matches!(
            element,
            StreamElement::Key(_) | StreamElement::KeySignature(_)
        )
    })
}

fn first_meter(staff: &Stream) -> Option<&StreamElement> {
    first_of(staff, |element| {
        matches!(element, StreamElement::TimeSignature(_))
    })
}

/// Whether a later staff's first mark of a kind differs from the first
/// staff's: music21's `isMultiAttribute`.
fn differs_across(
    staves: &[&Stream],
    first: impl Fn(&Stream) -> Option<&StreamElement>,
    same: impl Fn(&StreamElement, &StreamElement) -> bool,
) -> bool {
    let mut initial: Option<&StreamElement> = None;
    for staff in staves {
        match initial {
            None => initial = first(staff),
            Some(initial) => {
                if let Some(later) = first(staff)
                    && !same(later, initial)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// music21's `==` between two key signatures: a key equals a key of the same
/// tonic and mode, a signature a signature of the same sharps, and a key
/// never a bare signature.
fn same_key(left: &StreamElement, right: &StreamElement) -> bool {
    match (left, right) {
        (StreamElement::Key(left), StreamElement::Key(right)) => {
            left.tonic().name() == right.tonic().name() && left.mode() == right.mode()
        }
        (StreamElement::KeySignature(left), StreamElement::KeySignature(right)) => {
            left.sharps() == right.sharps()
                && left.altered_pitches().ok().map(|pitches| {
                    pitches
                        .iter()
                        .map(Pitch::name_with_octave)
                        .collect::<Vec<_>>()
                }) == right.altered_pitches().ok().map(|pitches| {
                    pitches
                        .iter()
                        .map(Pitch::name_with_octave)
                        .collect::<Vec<_>>()
                })
        }
        _ => false,
    }
}

/// music21's `ratioEqual` between two meters.
fn same_meter(left: &StreamElement, right: &StreamElement) -> bool {
    match (left, right) {
        (StreamElement::TimeSignature(left), StreamElement::TimeSignature(right)) => {
            left.numerator() == right.numerator() && left.denominator() == right.denominator()
        }
        _ => false,
    }
}

/// music21's `setPartList`: each part's `<score-part>`, with a staff group
/// opened before the first part it spans and closed after the last, groups
/// numbered from one in the order they open.
fn set_part_list(
    part_list: &mut Element,
    parts: &[Option<ExportedPart>],
    groups: &[StaffGroup],
    joined: &[usize],
) {
    let mut numbers: Vec<Option<usize>> = vec![None; groups.len()];
    let mut next = 1;
    for (index, part) in parts.iter().enumerate() {
        let Some(part) = part else {
            continue;
        };
        for (group_index, group) in groups.iter().enumerate() {
            if joined.contains(&group_index) {
                continue;
            }
            if group.parts().first() == Some(&index) {
                let mut start = staff_group_element(group);
                start.set("number", next.to_string());
                numbers[group_index] = Some(next);
                next += 1;
                part_list.push(start);
            }
        }
        part_list.push(part.score_part.clone());
        for (group_index, group) in groups.iter().enumerate() {
            if joined.contains(&group_index) {
                continue;
            }
            if group.parts().last() == Some(&index) {
                let mut stop = Element::new("part-group");
                stop.set("type", "stop");
                if let Some(number) = numbers[group_index] {
                    stop.set("number", number.to_string());
                }
                part_list.push(stop);
            }
        }
    }
}

/// music21's `staffGroupToXmlPartGroup`.
fn staff_group_element(group: &StaffGroup) -> Element {
    let mut element = Element::new("part-group");
    element.set("type", "start");
    if let Some(name) = group.name() {
        element.sub_text("group-name", name);
    }
    if group.name_hidden() {
        element.sub("group-name-display").set("print-object", "no");
    }
    if let Some(abbreviation) = group.abbreviation() {
        element.sub_text("group-abbreviation", abbreviation);
    }
    if let Some(symbol) = group.symbol() {
        element.sub_text("group-symbol", symbol);
    }
    let barline = element.sub("group-barline");
    if let Some(bar_together) = group.bar_together() {
        barline.set_text(bar_together.as_str());
    }
    element
}

/// A note at one end of a spanner's range: the element holding it and its
/// pitch space.
type Extreme = (*const StreamElement, FloatType);

/// The number each arpeggio across several chords is written with in one
/// measure, one to sixteen and round again, in the order they are met:
/// music21's `getArpeggioNumber`.
#[derive(Default)]
struct ArpeggioNumbers {
    known: std::cell::RefCell<Vec<(*const Spanner, u32)>>,
}

impl ArpeggioNumbers {
    fn number(&self, spanner: &Spanner) -> u32 {
        let mut known = self.known.borrow_mut();
        if let Some((_, number)) = known.iter().find(|(held, _)| std::ptr::eq(*held, spanner)) {
            return *number;
        }
        let number = known.len() as u32 % 16 + 1;
        known.push((spanner, number));
        number
    }
}

/// A spanner with the number it is written with and the elements it joins.
#[derive(Clone)]
struct Numbered<'a> {
    spanner: &'a Spanner,
    id_local: u32,
    /// Each element joined, or nothing for one standing in no stream.
    elements: Vec<Option<&'a StreamElement>>,
}

impl Numbered<'_> {
    fn is(joined: Option<&Option<&StreamElement>>, element: *const StreamElement) -> bool {
        joined.is_some_and(|joined| joined.is_some_and(|joined| std::ptr::eq(joined, element)))
    }

    fn holds(&self, element: *const StreamElement) -> bool {
        self.elements
            .iter()
            .any(|joined| Self::is(Some(joined), element))
    }

    fn is_first(&self, element: *const StreamElement) -> bool {
        Self::is(self.elements.first(), element)
    }

    fn is_last(&self, element: *const StreamElement) -> bool {
        Self::is(self.elements.last(), element)
    }

    /// The lowest and the highest note the spanner joins, each as the
    /// element holding it and its place in that element's chord: music21's
    /// `noteExtremes`, which takes the first of equal pitches either way.
    fn note_extremes(&self) -> Option<(Extreme, Extreme)> {
        let mut notes: Vec<Extreme> = Vec::new();
        for element in self.elements.iter().flatten() {
            match element {
                StreamElement::Note(note) => notes.push((*element, note.pitch().ps())),
                StreamElement::Chord(chord) => {
                    for note in chord.notes() {
                        notes.push((*element, note.pitch().ps()));
                    }
                }
                _ => {}
            }
        }
        let first = *notes.first()?;
        let (mut lowest, mut highest) = (first, first);
        for note in notes {
            if note.1 < lowest.1 {
                lowest = note;
            }
            if note.1 > highest.1 {
                highest = note;
            }
        }
        Some((lowest, highest))
    }
}

/// music21's `MetricModulation.updateByContext`, as its exporter sets it off
/// by asking each modulation of a part for its two marks: one with a side
/// that has no number yet takes it from the tempo in force before it -- the
/// last mark, tempo word or modulation standing earlier in the part, and
/// none standing where it stands itself.
fn resolved_modulations(part: &Stream) -> Vec<(*const StreamElement, MetricModulation)> {
    let mut elements = part.recurse();
    elements.sort_by(|left, right| left.0.total_cmp(&right.0));
    // Every tempo so far with where it stands, as the mark it sounds at.
    let mut tempos: Vec<(FloatType, Option<MetronomeMark>)> = Vec::new();
    let mut resolved = Vec::new();
    for (offset, element) in elements {
        match element {
            StreamElement::MetronomeMark(mark) => tempos.push((offset, Some(mark.clone()))),
            StreamElement::TempoText(text) => tempos.push((offset, Some(text.metronome_mark()))),
            StreamElement::MetricModulation(modulation) => {
                let previous = tempos
                    .iter()
                    .rev()
                    .find(|(at, _)| *at < offset)
                    .and_then(|(_, mark)| mark.as_ref());
                let mut modulation = (**modulation).clone();
                let unnumbered =
                    |side: Option<&MetronomeMark>| side.is_some_and(|mark| mark.number().is_none());
                if unnumbered(modulation.old_metronome()) {
                    modulation.update_from(previous);
                }
                if unnumbered(modulation.new_metronome()) {
                    modulation.update_from(previous);
                }
                tempos.push((offset, modulation.new_metronome().cloned()));
                resolved.push((element as *const StreamElement, modulation));
            }
            _ => {}
        }
    }
    resolved
}

/// music21's `setIdLocals` over the score's spanners: each kind numbered
/// one to six and round again -- the number MusicXML pairs a spanner's start
/// with its stop by -- in the order the score holds them, its own first and
/// then each part's. Each is resolved to the elements it joins, wherever in
/// the score they stand.
fn id_locals<'a>(score: &'a Stream, parts: &[&'a Stream]) -> Vec<Numbered<'a>> {
    let resolve = |holder: &'a Stream, spanner: &'a Spanner| {
        let leaves = holder.leaves();
        spanner
            .spanned()
            .iter()
            .map(|position| {
                position
                    .and_then(|position| leaves.get(position))
                    .map(|(_, element)| *element)
            })
            .collect::<Vec<_>>()
    };
    let mut held: Vec<(&'a Spanner, Vec<Option<&'a StreamElement>>)> = Vec::new();
    for spanner in score.spanners() {
        held.push((spanner, resolve(score, spanner)));
    }
    for part in parts {
        if std::ptr::eq(*part, score) {
            continue;
        }
        for spanner in part.spanners() {
            held.push((spanner, resolve(part, spanner)));
        }
    }
    let mut counts: Vec<(&'static str, u32)> = Vec::new();
    held.into_iter()
        .map(|(spanner, elements)| {
            let class = spanner.kind().numbering_class();
            let index = match counts.iter_mut().find(|(known, _)| *known == class) {
                Some((_, count)) => {
                    *count += 1;
                    *count - 1
                }
                None => {
                    counts.push((class, 1));
                    0
                }
            };
            Numbered {
                spanner,
                id_local: index % 6 + 1,
                elements,
            }
        })
        .collect()
}

/// Whether a stream holds parts: music21's `hasPartLikeStreams`.
fn has_part_like_streams(stream: &Stream) -> bool {
    stream.events().iter().any(|event| {
        event
            .element()
            .as_stream()
            .is_some_and(|inner| matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff))
    })
}

/// music21's `emptyObject`: "This Page Intentionally Left Blank", a whole
/// rest in one measure.
fn empty_score() -> Stream {
    let mut measure = Stream::with_kind(StreamKind::Measure);
    measure.push(Rest::new(Duration::whole()));
    let mut part = Stream::with_kind(StreamKind::Part);
    part.push(measure);
    let mut score = Stream::with_kind(StreamKind::Score);
    score.push(part);
    let mut metadata = Metadata::new();
    metadata.add_text("title", "This Page Intentionally Left Blank");
    score.set_metadata(Some(metadata));
    score
}

/// music21's `addDividerComment`.
fn add_divider_comment(root: &mut Element, comment: &str) {
    let length = comment.chars().count().min(60);
    let low = (60 - length) / 2;
    let high = (60 - length).div_ceil(2);
    root.push_comment(format!(
        "{} {comment} {}",
        "=".repeat(low),
        "=".repeat(high)
    ));
}

/// music21's `setTitles`.
fn set_titles(root: &mut Element, metadata: Option<&Metadata>, options: &ExportOptions) {
    let title = metadata.and_then(|md| md.first_text("title"));
    if let Some(title) = title {
        root.sub("work").sub_text("work-title", title);
    }
    if let Some(number) = metadata.and_then(|md| md.first_text("movementNumber")) {
        root.sub_text("movement-number", number);
    }
    let movement_title = metadata
        .and_then(|md| md.first_text("movementName"))
        .or(title)
        .or(options.default_title.as_deref());
    if let Some(movement_title) = movement_title {
        root.sub_text("movement-title", movement_title);
    }
}

/// music21's `setIdentification`, with `setEncoding` and
/// `metadataToMiscellaneous`.
fn set_identification(root: &mut Element, metadata: Option<&Metadata>, options: &ExportOptions) {
    let identification = root.sub("identification");
    let mut found_one = false;
    if let Some(md) = metadata {
        for (_, contributor) in md.contributors() {
            let creator = identification.sub("creator");
            if let Some(role) = contributor.role() {
                creator.set("type", role);
            }
            creator.set_text(contributor.text());
            found_one = true;
        }
    }
    if !found_one && let Some(author) = &options.default_author {
        let creator = identification.sub("creator");
        creator.set("type", "composer");
        creator.set_text(author.as_str());
    }
    if let Some(md) = metadata {
        for copyright in md.get("copyright") {
            let rights = identification.sub("rights");
            if let Some(role) = copyright.role() {
                rights.set("type", role);
            }
            rights.set_text(copyright.text());
        }
    }

    let encoding = identification.sub("encoding");
    if let Some(date) = &options.encoding_date {
        encoding.sub_text("encoding-date", date.as_str());
    }
    match metadata {
        Some(md) => {
            let mut found_music21 = false;
            for software in md.get("software") {
                if software.text().contains("music21 v.") {
                    if found_music21 {
                        continue;
                    }
                    found_music21 = true;
                }
                encoding.sub_text("software", software.text());
            }
        }
        None => encoding.sub_text("software", options.software.as_str()),
    }
    for element in ["beam", "stem", "accidental"] {
        let supports = encoding.sub("supports");
        supports.set("element", element);
        supports.set("type", "yes");
    }

    if let Some(md) = metadata {
        let mut miscellaneous = Element::new("miscellaneous");
        let (mut skipped_name, mut skipped_number, mut skipped_title) = (false, false, false);
        for (unique_name, value) in md.all() {
            if crate::metadata::is_contributor_unique_name(unique_name) {
                continue;
            }
            match unique_name {
                "software" | "copyright" => continue,
                "movementName" if !skipped_name => {
                    skipped_name = true;
                    continue;
                }
                "movementNumber" if !skipped_number => {
                    skipped_number = true;
                    continue;
                }
                "title" if !skipped_title => {
                    skipped_title = true;
                    continue;
                }
                _ => {}
            }
            let name = namespace_name(unique_name).unwrap_or(unique_name);
            if name.starts_with("m21FileInfo:") {
                continue;
            }
            let field = miscellaneous.sub("miscellaneous-field");
            field.set("name", name);
            field.set_text(value.text());
        }
        if miscellaneous.len() > 0 {
            identification.push(miscellaneous);
        }
    }
}

/// music21's `setDefaults` for a score with no layout of its own.
fn set_defaults(root: &mut Element) {
    let scaling = root.sub("defaults").sub("scaling");
    scaling.sub_text("millimeters", "7");
    scaling.sub_text("tenths", "40");
}

// ------------------------------------------------------------------- part

/// What the parts of one score share while they are written: the MIDI
/// channels, instrument ids and part ids already given out.
#[derive(Default)]
struct Shared {
    midi_channels: Vec<Option<u8>>,
    instrument_ids: Vec<String>,
    part_ids: Vec<String>,
}

struct ExportedPart {
    part: Element,
    score_part: Element,
}

/// An instrument with the offset it starts at within its part.
struct PlacedInstrument {
    offset: FloatType,
    instrument: Instrument,
}

/// music21's `PartExporter`.
struct PartExporter<'a> {
    stream: &'a Stream,
    instruments: Vec<PlacedInstrument>,
}

impl<'a> PartExporter<'a> {
    /// music21's `instrumentSetup`.
    fn new(stream: &'a Stream, shared: &mut Shared, role: &StaffRole<'_>) -> Result<Self> {
        let mut instruments = instruments_of(stream);
        if instruments.is_empty() {
            let mut default = Instrument::new();
            default.set_part_name(Some(String::new()));
            instruments.push(PlacedInstrument {
                offset: 0.0,
                instrument: default,
            });
        }

        // music21 gives a part whose id is taken, or who has none, a random
        // one; the part's own id, then the first free `P<n>`, is used here.
        let first = &mut instruments[0].instrument;
        let taken = |id: &str| shared.part_ids.iter().any(|known| known == id);
        if first.part_id().is_none_or(taken) {
            let fresh = stream
                .id()
                .filter(|id| !taken(id))
                .map(str::to_string)
                .unwrap_or_else(|| {
                    (1..)
                        .map(|number| format!("P{number}"))
                        .find(|candidate| !taken(candidate))
                        .expect("there is always an unused number")
                });
            first.set_part_id(Some(fresh));
        }
        let part_id = first.part_id().unwrap_or_default().to_string();

        // music21's `mergeInstrumentStreamPartStaffAware`: the first staff of
        // a group plays the instruments of every staff in it, and the staves
        // after it leave theirs to the first.
        if role.later {
            return Ok(Self {
                stream,
                instruments,
            });
        }
        for follower in &role.followers {
            let mut theirs = instruments_of(follower);
            if theirs.is_empty() {
                let mut default = Instrument::new();
                default.set_part_name(Some(String::new()));
                theirs.push(PlacedInstrument {
                    offset: 0.0,
                    instrument: default,
                });
            }
            instruments.extend(theirs);
            instruments = deduplicate(instruments);
        }

        let mut seen: Vec<String> = Vec::new();
        for (index, placed) in instruments.iter_mut().enumerate() {
            let instrument = &mut placed.instrument;
            if seen.iter().any(|kind| kind == instrument.kind()) {
                continue;
            }
            seen.push(instrument.kind().to_string());
            if instrument
                .midi_channel()
                .is_none_or(|channel| shared.midi_channels.contains(&Some(channel)))
            {
                let used: Vec<u8> = shared.midi_channels.iter().flatten().copied().collect();
                // music21 warns and carries on when it runs out of channels.
                let _ = instrument.auto_assign_midi_channel(&used, 16);
            }
            shared.midi_channels.push(instrument.midi_channel());
            let duplicate = instrument
                .instrument_id()
                .is_none_or(|id| shared.instrument_ids.iter().any(|known| known == id));
            if duplicate {
                let fresh = (1..)
                    .map(|number| format!("{part_id}-I{number}"))
                    .find(|candidate| !shared.instrument_ids.contains(candidate))
                    .expect("there is always an unused number");
                instrument.set_instrument_id(Some(fresh));
            }
            shared
                .instrument_ids
                .push(instrument.instrument_id().unwrap_or_default().to_string());
            if index == 0 {
                shared.part_ids.push(part_id.clone());
            }
        }
        Ok(Self {
            stream,
            instruments,
        })
    }

    fn part_id(&self) -> &str {
        self.instruments[0].instrument.part_id().unwrap_or_default()
    }

    /// music21's `PartExporter.parse`, with `makeNotation=False`.
    fn parse(
        self,
        spanners: Vec<Numbered<'a>>,
        previous: Option<&'a Stream>,
        voice_bases: &'a std::collections::HashMap<usize, u32>,
    ) -> Result<ExportedPart> {
        let measures: Vec<(FloatType, &Stream)> = self
            .stream
            .events()
            .iter()
            .filter_map(|event| {
                event
                    .element()
                    .as_stream()
                    .filter(|stream| stream.kind() == StreamKind::Measure)
                    .map(|stream| (event.offset(), stream))
            })
            .collect();
        if measures.is_empty() {
            return Err(export_error(
                "Cannot export with makeNotation=False if there are no measures",
            ));
        }

        let mut part = Element::new("part");
        part.set("id", self.part_id());
        let mut context = PartContext {
            instruments: &self.instruments,
            own_instruments: self
                .stream
                .recurse()
                .into_iter()
                .filter_map(|(offset, element)| match element {
                    StreamElement::Instrument(instrument) => Some((offset, &**instrument)),
                    _ => None,
                })
                .collect(),
            own_clefs: self
                .stream
                .recurse()
                .into_iter()
                .filter_map(|(offset, element)| match element {
                    StreamElement::Clef(clef)
                        if clef.places_pitches() && clef.lowest_line().is_some() =>
                    {
                        Some((offset, clef))
                    }
                    _ => None,
                })
                .collect(),
            meter: None,
            last_divisions: None,
            spanners,
            clef: None,
            previous,
            voice_bases,
            modulations: resolved_modulations(self.stream),
        };
        for (offset, measure) in measures {
            add_divider_comment(&mut part, &format!("Measure {}", measure.number()));
            part.push(MeasureExporter::new(measure, offset, &mut context).parse()?);
        }
        Ok(ExportedPart {
            score_part: self.score_part(),
            part,
        })
    }

    /// music21's `getXmlScorePart`.
    fn score_part(&self) -> Element {
        let mut score_part = Element::new("score-part");
        score_part.set("id", self.part_id());
        let name = score_part.sub("part-name");
        name.set_text(part_name(self.stream).unwrap_or_default());
        if self.stream.name_hidden() {
            name.set("print-object", "no");
        }
        if let Some(abbreviation) = part_abbreviation(self.stream) {
            let element = score_part.sub("part-abbreviation");
            element.set_text(abbreviation);
            if self.stream.abbreviation_hidden() {
                element.set("print-object", "no");
            }
        }
        let mut seen: Vec<&str> = Vec::new();
        for placed in &self.instruments {
            let instrument = &placed.instrument;
            if seen.contains(&instrument.kind()) {
                continue;
            }
            if instrument.name().is_some()
                || instrument.abbreviation().is_some()
                || instrument.midi_program().is_some()
            {
                score_part.push(score_instrument(instrument));
                seen.push(instrument.kind());
            }
        }
        let mut seen: Vec<&str> = Vec::new();
        for placed in &self.instruments {
            let instrument = &placed.instrument;
            if seen.contains(&instrument.kind()) {
                continue;
            }
            if instrument.midi_program().is_some() || instrument.is_a("UnpitchedPercussion") {
                score_part.push(midi_instrument(instrument));
                seen.push(instrument.kind());
            }
        }
        score_part
    }
}

/// Every instrument anywhere in a part, with where it starts.
fn instruments_of(stream: &Stream) -> Vec<PlacedInstrument> {
    stream
        .recurse()
        .into_iter()
        .filter_map(|(offset, element)| match element {
            StreamElement::Instrument(instrument) => Some(PlacedInstrument {
                offset,
                instrument: (**instrument).clone(),
            }),
            _ => None,
        })
        .collect()
}

/// music21's `instrument.deduplicate`: at each offset where the instruments
/// agree on their names, where each says the same or nothing, the names are
/// shared out, one instrument of each kind is kept, and a bare `Instrument`
/// gives way to one of a kind.
fn deduplicate(mut instruments: Vec<PlacedInstrument>) -> Vec<PlacedInstrument> {
    instruments.sort_by(|left, right| left.offset.total_cmp(&right.offset));
    let mut out: Vec<PlacedInstrument> = Vec::new();
    let mut start = 0;
    while start < instruments.len() {
        let offset = instruments[start].offset;
        let end = instruments[start..]
            .iter()
            .position(|placed| placed.offset != offset)
            .map_or(instruments.len(), |length| start + length);
        let group = &instruments[start..end];
        let agreed = |read: fn(&Instrument) -> Option<&str>| -> Option<Option<String>> {
            let mut said: Option<&str> = None;
            for placed in group {
                match (said, read(&placed.instrument)) {
                    (_, None) => {}
                    (None, Some(name)) => said = Some(name),
                    (Some(known), Some(name)) if known == name => {}
                    _ => return None,
                }
            }
            Some(said.map(str::to_string))
        };
        match (agreed(Instrument::part_name), agreed(Instrument::name)) {
            (Some(part_name), Some(name)) if group.len() > 1 => {
                let has_specific = group
                    .iter()
                    .any(|placed| placed.instrument.kind() != "Instrument");
                let mut kept: Vec<PlacedInstrument> = Vec::new();
                for placed in group {
                    let kind = placed.instrument.kind();
                    if (has_specific && kind == "Instrument")
                        || kept.iter().any(|known| known.instrument.kind() == kind)
                    {
                        continue;
                    }
                    let mut instrument = placed.instrument.clone();
                    instrument.set_part_name(part_name.clone());
                    instrument.set_name(name.clone());
                    kept.push(PlacedInstrument { offset, instrument });
                }
                out.extend(kept);
            }
            _ => out.extend(group.iter().map(|placed| PlacedInstrument {
                offset,
                instrument: placed.instrument.clone(),
            })),
        }
        start = end;
    }
    out
}

/// music21's `Part.partName`: the part's own, else the first instrument
/// held directly that names one.
fn part_name(part: &Stream) -> Option<&str> {
    part.name().or_else(|| {
        held_instruments(part)
            .into_iter()
            .find_map(|instrument| instrument.part_name().or(instrument.name()))
    })
}

/// Every instrument a part holds, those inside its measures too.
fn held_instruments(part: &Stream) -> Vec<&Instrument> {
    part.recurse()
        .into_iter()
        .filter_map(|(_, element)| match element {
            StreamElement::Instrument(instrument) => Some(&**instrument),
            _ => None,
        })
        .collect()
}

/// music21's `Part.partAbbreviation`, read the same way.
fn part_abbreviation(part: &Stream) -> Option<&str> {
    part.abbreviation().or_else(|| {
        held_instruments(part)
            .into_iter()
            .find_map(|instrument| instrument.part_abbreviation().or(instrument.abbreviation()))
    })
}

/// music21's `instrumentToXmlScoreInstrument`.
fn score_instrument(instrument: &Instrument) -> Element {
    let mut element = Element::new("score-instrument");
    element.set("id", instrument.instrument_id().unwrap_or_default());
    element.sub_text(
        "instrument-name",
        instrument.name().unwrap_or("None").to_string(),
    );
    if let Some(abbreviation) = instrument.abbreviation() {
        element.sub_text("instrument-abbreviation", abbreviation);
    }
    element
}

/// music21's `instrumentToXmlMidiInstrument`.
fn midi_instrument(instrument: &Instrument) -> Element {
    let mut element = Element::new("midi-instrument");
    element.set("id", instrument.instrument_id().unwrap_or_default());
    let channel = instrument.midi_channel().unwrap_or(0);
    element.sub_text("midi-channel", (u32::from(channel) + 1).to_string());
    if let Some(program) = instrument.midi_program() {
        element.sub_text("midi-program", (u32::from(program) + 1).to_string());
    }
    if instrument.is_a("UnpitchedPercussion")
        && let Some(pitch) = instrument.percussion_pitch()
    {
        element.sub_text("midi-unpitched", (u32::from(pitch) + 1).to_string());
    }
    element
}

/// What a measure needs to know of the part around it.
struct PartContext<'a> {
    instruments: &'a [PlacedInstrument],
    /// The instruments standing in this staff itself, in the order it holds
    /// them, which is where a note looks for the one that plays it.
    own_instruments: Vec<(FloatType, &'a Instrument)>,
    /// The clefs placing pitches that stand in this staff, in order.
    own_clefs: Vec<(FloatType, &'a Clef)>,
    /// The metre in force where the measure being written starts.
    meter: Option<TimeSignature>,
    last_divisions: Option<FloatType>,
    /// The score's spanners, each with the number it is written with and
    /// the elements it joins.
    spanners: Vec<Numbered<'a>>,
    /// The part's metric modulations as they are once each has taken its
    /// numbers from the tempo in force before it.
    modulations: Vec<(*const StreamElement, MetricModulation)>,
    /// The clef in force where the writer has got to.
    clef: Option<Clef>,
    /// The staff before this one where it is one of several written as one
    /// part: a key or meter that staff already says is not said again.
    previous: Option<&'a Stream>,
    /// Where the voices with no id of their own start numbering, measure by
    /// measure, in a staff written with others.
    voice_bases: &'a std::collections::HashMap<usize, u32>,
}

// ---------------------------------------------------------------- measure

/// music21's `MeasureExporter`.
struct MeasureExporter<'a, 'b> {
    stream: &'a Stream,
    /// Where the measure starts within its part.
    start: FloatType,
    context: &'a mut PartContext<'b>,
    root: Element,
    offset_in_measure: FloatType,
    current_voice: Option<String>,
    next_free_voice: u32,
    arpeggios: ArpeggioNumbers,
    /// Where in the measure the element being written stands.
    element_offset: FloatType,
}

impl<'a, 'b> MeasureExporter<'a, 'b> {
    fn new(stream: &'a Stream, start: FloatType, context: &'a mut PartContext<'b>) -> Self {
        let next_free_voice = context
            .voice_bases
            .get(&(stream as *const Stream as usize))
            .map_or(1, |base| base + 1);
        Self {
            stream,
            start,
            context,
            root: Element::new("measure"),
            offset_in_measure: 0.0,
            current_voice: None,
            next_free_voice,
            arpeggios: ArpeggioNumbers::default(),
            element_offset: 0.0,
        }
    }

    /// music21's `_matchesPreviousPartStaffInGroup`: whether the staff above
    /// this one says the same at the start of its measure of this number.
    fn matches_previous_staff(
        &self,
        element: &StreamElement,
        pick: impl Fn(&StreamElement) -> bool,
        same: impl Fn(&StreamElement, &StreamElement) -> bool,
    ) -> bool {
        let Some(previous) = self.context.previous else {
            return false;
        };
        let Some(measure) = previous
            .measures()
            .into_iter()
            .find(|measure| measure.number() == self.stream.number())
        else {
            return false;
        };
        measure
            .events()
            .iter()
            .filter(|event| event.offset() == 0.0)
            .map(|event| event.element())
            .find(|candidate| pick(candidate))
            .is_some_and(|theirs| same(element, theirs))
    }

    /// music21's `MeasureExporter.parse`.
    fn parse(mut self) -> Result<Element> {
        let transposition = self.transposition();
        // setMxAttributes
        self.root.set("number", self.stream.number_with_suffix());
        self.root.set(
            "implicit",
            if self.stream.number_hidden() {
                "yes"
            } else {
                "no"
            },
        );
        self.set_attributes_for_start_of_measure(transposition.as_ref())?;
        // music21's `setLeftBarline` and `setRightBarline`: a barline where
        // the measure has one, or where an ending opens or closes on it.
        let ending = self.stream.ending();
        let opens = ending.filter(|ending| ending.starts());
        if self.stream.left_barline().is_some() || opens.is_some() {
            let barline = barline_element(
                self.stream.left_barline(),
                "left",
                opens.map(|ending| (ending, "start")),
            );
            self.root.push(barline);
        }
        self.main_elements_parse()?;
        let closes = ending.filter(|ending| ending.stops());
        if self.stream.right_barline().is_some() || closes.is_some() {
            let barline = barline_element(
                self.stream.right_barline(),
                "right",
                closes.map(|ending| (ending, "stop")),
            );
            self.root.push(barline);
        }
        if let Some(meter) = last_of(self.stream, |element| match element {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        }) {
            self.context.meter = Some(meter);
        }
        Ok(self.root)
    }

    /// music21's `setTranspose`: the transposition of an instrument starting
    /// within this measure.
    fn transposition(&self) -> Option<Interval> {
        let end = self.start + self.stream.end_offset();
        self.context
            .instruments
            .iter()
            .find(|placed| {
                placed.offset >= self.start && (placed.offset < end || placed.offset == self.start)
            })
            .and_then(|placed| placed.instrument.transposition().cloned())
    }

    /// music21's `setMxAttributesObjectForStartOfMeasure`.
    fn set_attributes_for_start_of_measure(
        &mut self,
        transposition: Option<&Interval>,
    ) -> Result<()> {
        let mut attributes = Element::new("attributes");
        if self.context.last_divisions != Some(DIVISIONS_PER_QUARTER) {
            attributes.sub_text("divisions", format_int(DIVISIONS_PER_QUARTER));
            self.context.last_divisions = Some(DIVISIONS_PER_QUARTER);
        }
        for event in self
            .stream
            .events()
            .iter()
            .filter(|event| event.offset() == 0.0)
        {
            if let Some(key) = key_element(event.element()) {
                let is_key = |element: &StreamElement| {
                    matches!(
                        element,
                        StreamElement::Key(_) | StreamElement::KeySignature(_)
                    )
                };
                if !self.matches_previous_staff(event.element(), is_key, same_key) {
                    attributes.push(key?);
                }
                break;
            }
        }
        for event in self
            .stream
            .events()
            .iter()
            .filter(|event| event.offset() == 0.0)
        {
            if let StreamElement::TimeSignature(meter) = event.element() {
                let is_meter =
                    |element: &StreamElement| matches!(element, StreamElement::TimeSignature(_));
                if !self.matches_previous_staff(event.element(), is_meter, same_meter) {
                    attributes.push(time_signature_element(meter)?);
                }
                self.context.meter = Some(meter.clone());
                break;
            }
        }
        for event in self
            .stream
            .events()
            .iter()
            .filter(|event| event.offset() == 0.0)
        {
            if let StreamElement::Clef(clef) = event.element() {
                attributes.push(clef_element(clef)?);
                break;
            }
        }
        if let Some(interval) = transposition {
            attributes.push(transpose_element(interval));
        }
        if attributes.len() > 0 || attributes.has_attributes() {
            self.root.push(attributes);
        }
        Ok(())
    }

    /// music21's `mainElementsParse`.
    fn main_elements_parse(&mut self) -> Result<()> {
        let voices: Vec<&Stream> = self.stream.voices();
        if voices.is_empty() {
            return self.parse_flat_elements(self.stream, false, false);
        }
        self.parse_flat_elements(self.stream, false, true)?;
        let count = voices.len();
        for (index, voice) in voices.into_iter().enumerate() {
            self.parse_flat_elements(voice, true, index + 1 != count)?;
        }
        Ok(())
    }

    /// music21's `parseFlatElements`: the elements of a measure or voice,
    /// grouped by offset, marks before notes within each group.
    fn parse_flat_elements(
        &mut self,
        stream: &Stream,
        is_voice: bool,
        backup_afterwards: bool,
    ) -> Result<()> {
        self.offset_in_measure = 0.0;
        self.current_voice = if is_voice {
            Some(match stream.id() {
                Some(id) => id.to_string(),
                None => {
                    let number = self.next_free_voice;
                    self.next_free_voice += 1;
                    number.to_string()
                }
            })
        } else {
            None
        };

        let events: Vec<(FloatType, &StreamElement)> = stream
            .events()
            .iter()
            .filter(|event| !matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice))
            .map(|event| (event.offset(), event.element()))
            .collect();
        let mut index = 0;
        while index < events.len() {
            let group_offset = events[index].0;
            let mut end = index;
            while end < events.len() && events[end].0 == group_offset {
                end += 1;
            }
            let group = &events[index..end];
            let forward = group_offset - self.offset_in_measure;
            if forward > 0.0
                && group.iter().any(|(_, element)| {
                    is_general_note(element) || matches!(element, StreamElement::Clef(_))
                })
            {
                self.move_forward(forward);
            }
            let mut notes_for_later = Vec::new();
            for &(offset, element) in group {
                if is_written_as_note(element) {
                    notes_for_later.push((offset, element));
                } else {
                    self.parse_one_element(offset, element)?;
                }
            }
            for (offset, element) in notes_for_later {
                // An unprinted rest of a length no note value writes is
                // left as a gap, which the next element is moved past.
                if let StreamElement::Rest(rest) = element
                    && rest.hidden()
                    && rest.quarter_length() != 0.0
                    && rest.duration().components().is_empty()
                {
                    continue;
                }
                self.parse_one_element(offset, element)?;
            }
            index = end;
        }

        if backup_afterwards && self.offset_in_measure != 0.0 {
            self.move_backward(self.offset_in_measure);
        }
        self.current_voice = None;
        Ok(())
    }

    /// music21's `moveForward`.
    fn move_forward(&mut self, by: FloatType) {
        let amount = divisions(by);
        if amount != 0 {
            self.root
                .sub("forward")
                .sub_text("duration", amount.to_string());
            self.offset_in_measure += by;
        }
    }

    /// music21's `moveBackward`.
    fn move_backward(&mut self, by: FloatType) {
        let amount = divisions(by);
        if amount != 0 {
            self.root
                .sub("backup")
                .sub_text("duration", amount.to_string());
            self.offset_in_measure -= by;
        }
    }

    /// music21's `parseOneElement`.
    fn parse_one_element(&mut self, offset: FloatType, element: &StreamElement) -> Result<()> {
        let position = element as *const StreamElement;
        // Asked before the writer moves past the element, which is where a
        // pedal's resumed line counts its offset from.
        let (before, after) =
            related_spanners(position, &self.context.spanners, self.offset_in_measure);
        if is_written_as_note(element) {
            self.offset_in_measure += element.quarter_length();
        }
        for direction in before {
            self.root.push(direction);
        }
        self.write_element(offset, element, position)?;
        for direction in after {
            self.root.push(direction);
        }
        Ok(())
    }

    /// The element itself, as `parseOneElement` writes it between the
    /// directions its spanners open and close with.
    fn write_element(
        &mut self,
        offset: FloatType,
        element: &StreamElement,
        position: *const StreamElement,
    ) -> Result<()> {
        self.element_offset = offset;
        match element {
            StreamElement::Note(note) => {
                let mxnote = self.note_element(note, 0, None, Some(position))?;
                self.root.push(mxnote);
            }
            StreamElement::Chord(chord) => self.chord_elements(chord, Some(position))?,
            StreamElement::Unpitched(stroke) => {
                let mxnote =
                    self.written_note_element(stroke.written(), 0, None, Some(position), true)?;
                self.root.push(mxnote);
            }
            StreamElement::PercussionChord(chord) => {
                // Unlike a chord of pitches, its members stay in the order
                // they were given.
                let written = chord.written();
                for (index, note) in written.notes().iter().enumerate() {
                    let element = self.written_note_element(
                        note,
                        index,
                        Some(written),
                        Some(position),
                        chord.is_unpitched(index),
                    )?;
                    self.root.push(element);
                }
            }
            StreamElement::Rest(rest) => {
                let mxnote = self.rest_element(rest, offset, Some(position))?;
                self.root.push(mxnote);
            }
            StreamElement::Dynamic(dynamic) => self.dynamic_element(dynamic, offset),
            StreamElement::MetronomeMark(mark) => self.tempo_elements(mark, offset)?,
            StreamElement::MetricModulation(modulation) => {
                let resolved = self
                    .context
                    .modulations
                    .iter()
                    .find(|(held, _)| std::ptr::eq(*held, position))
                    .map_or(&**modulation, |(_, resolved)| resolved);
                let direction = self.metric_modulation_element(resolved, offset)?;
                self.root.push(direction);
            }
            StreamElement::TempoText(text) => {
                let direction = self.tempo_words(text.text(), text.placement(), offset);
                self.root.push(direction);
            }
            StreamElement::TextExpression(text) => {
                // music21's `textExpressionToXml`.
                let words = Element::with_text("words", text.content());
                let direction = self.direction(words, text.placement(), offset, false);
                self.root.push(direction);
            }
            StreamElement::RepeatExpression(mark) => {
                let direction = self.repeat_expression_element(mark, offset);
                self.root.push(direction);
            }
            StreamElement::KeySignature(_) | StreamElement::Key(_) => {
                if let Some(key) = key_element(element) {
                    let key = key?;
                    self.wrap_in_attributes(key);
                }
            }
            StreamElement::TimeSignature(meter) => {
                let meter = time_signature_element(meter)?;
                self.wrap_in_attributes(meter);
            }
            StreamElement::Clef(clef) => {
                self.context.clef = Some(clef.clone());
                let clef = clef_element(clef)?;
                self.wrap_in_attributes(clef);
            }
            // music21 writes nothing for an instrument inside a measure: it
            // is read into the part list instead.
            StreamElement::Instrument(_) => {}
            StreamElement::ChordSymbol(symbol) => {
                let harmony = self.harmony_element(symbol, offset)?;
                self.root.push(harmony);
            }
            StreamElement::Stream(inner) => {
                return Err(export_error(format!(
                    "a {} inside a measure cannot be written",
                    inner.kind()
                )));
            }
            StreamElement::PedalObject(object) => self.pedal_object_elements(object, position),
            StreamElement::RehearsalMark(mark) => {
                // music21's `rehearsalMarkToXml`: a mark is centred on where
                // it stands, as its class starts every mark out.
                let mut rehearsal = Element::with_text("rehearsal", mark.content());
                if let Some(enclosure) = mark.enclosure() {
                    rehearsal.set("enclosure", enclosure);
                }
                rehearsal.set("halign", "center");
                rehearsal.set("valign", "middle");
                let direction = self.direction(rehearsal, mark.placement(), offset, true);
                self.root.push(direction);
            }
            // music21 writes nothing for a barline standing inside a
            // measure: `Barline` is one of `ignoreOnParseClasses`. A
            // manuscript's break is no class its exporter knows.
            StreamElement::Barline(_) | StreamElement::Break(_) => {}
        }
        Ok(())
    }

    /// music21's `pedalObjectToXml`: a bounce or a gap in a held pedal, as
    /// the pedal mark holding it is drawn. One no pedal mark holds writes
    /// nothing.
    fn pedal_object_elements(&mut self, object: &PedalObject, position: *const StreamElement) {
        let Some(pedal) = self
            .context
            .spanners
            .iter()
            .find(|numbered| {
                numbered.spanner.kind() == SpannerKind::PedalMark && numbered.holds(position)
            })
            .map(|numbered| numbered.spanner.pedal().unwrap_or_default())
        else {
            return;
        };
        let lined = pedal.form.is_some_and(PedalForm::has_line);
        // Soft and silent pedals have no start of their own in MusicXML, and
        // music21 writes them, and a pedal that does not say, as `sustain`.
        let down = match pedal.pedal_type {
            Some(PedalType::Sostenuto) => "sostenuto",
            Some(PedalType::Sustain) => "start",
            _ => "sustain",
        };
        let types: &[&str] = match object.kind() {
            PedalObjectKind::Bounce => match pedal.form {
                Some(PedalForm::Line | PedalForm::SymbolLine) => &["change"],
                Some(PedalForm::SymbolAlt) => &[down],
                Some(PedalForm::Symbol) => &["stop", down],
                None => return,
            },
            PedalObjectKind::GapStart => &["discontinue"],
            PedalObjectKind::GapEnd => &["resume"],
        };
        for kind in types {
            let mut mark = Element::new("pedal");
            mark.set("type", *kind);
            mark.set(if lined { "line" } else { "sign" }, "yes");
            let direction = self.direction(mark, object.placement(), self.element_offset, true);
            self.root.push(direction);
        }
    }

    /// music21's `wrapObjectInAttributes`: a mark after the start of a
    /// measure gets an `<attributes>` of its own; one at the start is already
    /// in the measure's first.
    fn wrap_in_attributes(&mut self, element: Element) {
        if self.offset_in_measure == 0.0 {
            return;
        }
        let mut attributes = Element::new("attributes");
        attributes.push(element);
        self.root.push(attributes);
    }

    // --------------------------------------------------------- notes

    /// music21's `noteToXml`, for a note alone or the `index`th of a chord.
    fn note_element(
        &self,
        note: &Note,
        index: usize,
        chord: Option<&Chord>,
        position: Option<*const StreamElement>,
    ) -> Result<Element> {
        self.written_note_element(note, index, chord, position, false)
    }

    /// `noteToXml` for a note, or `unpitchedToXml` for a stroke with no
    /// pitch, which is written where it is displayed and has no accidental.
    fn written_note_element(
        &self,
        note: &Note,
        index: usize,
        chord: Option<&Chord>,
        position: Option<*const StreamElement>,
        unpitched: bool,
    ) -> Result<Element> {
        let in_chord = index != 0;
        let mut mxnote = Element::new("note");
        if chord.is_some()
            && let Some(color) = note.color()
        {
            mxnote.set("color", normalize_color(color)?);
        }
        if let Some(color) = chord.map_or(note.color(), Chord::color) {
            mxnote.set("color", normalize_color(color)?);
        }
        let volume = match chord {
            Some(chord) => chord.has_volume_information().then(|| chord.volume()),
            None => note.has_volume_information().then(|| note.volume()),
        };
        if let Some(scalar) = volume.and_then(|volume| volume.velocity_scalar()) {
            let velocity = scalar * 100.0 * (127.0 / 90.0);
            mxnote.set("dynamics", format!("{velocity:.2}"));
        }
        // An id is written only where it is a name XML takes as one.
        if let Some(id) = note.id()
            && id
                .chars()
                .next()
                .is_some_and(|first| first.is_alphabetic() || first == '_')
            && id
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        {
            mxnote.set("id", id);
        }
        let duration = chord
            .map_or_else(|| note.duration(), Chord::duration)
            .cloned()
            .unwrap_or_default();
        if note.hidden() {
            mxnote.set("print-object", "no");
            mxnote.set("print-spacing", "yes");
        }
        if chord
            .map_or(note.articulations(), Chord::articulations)
            .iter()
            .any(|articulation| articulation.is_a("Pizzicato"))
        {
            mxnote.set("pizzicato", "yes");
        }

        if let Some(grace) = duration.grace() {
            let element = mxnote.sub("grace");
            element.set("slash", yes_no(grace.slash()));
            if let Some(share) = grace.steal_time_previous() {
                element.set("steal-time-previous", fraction_to_percent(share));
            }
            if let Some(share) = grace.steal_time_following() {
                element.set("steal-time-following", fraction_to_percent(share));
            }
        }
        if in_chord {
            mxnote.sub("chord");
        }
        if unpitched {
            let displayed = mxnote.sub("unpitched");
            displayed.sub_text("display-step", note.pitch().step().as_char().to_string());
            displayed.sub_text("display-octave", note.pitch().implicit_octave().to_string());
        } else {
            mxnote.push(pitch_element(note.pitch()));
        }
        if !duration.is_grace() {
            mxnote.sub_text("duration", divisions(duration.quarter_length()).to_string());
        }
        if let Some(tie) = note.tie() {
            for element in tie_elements(tie) {
                mxnote.push(element);
            }
        }
        let stored = chord.map_or(note.stored_instrument(), Chord::stored_instrument);
        self.set_note_instrument(&mut mxnote, stored)?;
        if let Some(voice) = &self.current_voice {
            mxnote.sub_text("voice", voice.as_str());
        }
        // A grace note with no written value of its own is an eighth.
        let (written_type, dots) = match written_type(&duration) {
            Err(_) if duration.is_grace() && duration.components().is_empty() => ("eighth", 0),
            other => other?,
        };
        let written = mxnote.sub("type");
        written.set_text(written_type);
        if let Some(size) = note.size() {
            written.set("size", size.as_str());
        }
        for _ in 0..dots {
            mxnote.sub("dot");
        }
        if !unpitched
            && let Some(accidental) = note.pitch().written_accidental()
            && accidental.display_status() != Some(false)
        {
            mxnote.push(accidental_element(accidental, "accidental")?);
        }
        if let Some(modification) = time_modification(&duration)? {
            mxnote.push(modification);
        }

        let chord_stem = chord.map_or(note.stem_direction(), Chord::stem_direction);
        let stem = if !in_chord && chord_stem != StemDirection::Unspecified {
            Some(chord_stem)
        } else if chord.is_some() && note.stem_direction() != StemDirection::Unspecified {
            Some(note.stem_direction())
        } else {
            None
        };
        if let Some(stem) = stem {
            let text = match stem {
                StemDirection::NoStem => "none",
                other => other.as_str(),
            };
            mxnote.sub_text("stem", text);
        }

        if let Some(notehead) = notehead_for(note, chord)? {
            mxnote.push(notehead);
        }
        if !in_chord {
            let beams = chord.map_or(note.beams(), Chord::beams);
            for element in beam_elements(beams)? {
                mxnote.push(element);
            }
        }

        let spanners: &[Numbered<'_>] = match position {
            Some(_) => &self.context.spanners,
            None => &[],
        };
        let pitch = note.pitch().ps();
        let mut notations = note_notations(
            note,
            index,
            chord,
            position,
            spanners,
            &self.arpeggios,
            pitch,
        )?;
        if !in_chord {
            notations.extend(tuplet_notations(&duration)?);
        }
        if !notations.is_empty() {
            let mxnotations = mxnote.sub("notations");
            for element in notations {
                mxnotations.push(element);
            }
        }
        if !in_chord {
            let lyrics = chord.map_or(note.lyrics(), Chord::lyrics);
            for lyric in lyrics {
                if lyric.explicit_text().is_none() {
                    continue;
                }
                mxnote.push(lyric_element(lyric)?);
            }
        }
        Ok(mxnote)
    }

    /// music21's `setNoteInstrument`: which instrument plays a note, where
    /// a part has more than one.
    ///
    /// The instrument is the one kept on the note or its chord, or else the
    /// last one standing in the staff at or before the note; it is named by
    /// the id of the part's first instrument of the same kind.
    fn set_note_instrument(&self, mxnote: &mut Element, stored: Option<&Instrument>) -> Result<()> {
        if self.context.instruments.len() <= 1 {
            return Ok(());
        }
        let at = self.start + self.element_offset;
        let default;
        let closest = match stored.or_else(|| {
            self.context
                .own_instruments
                .iter()
                .rev()
                .find(|(offset, _)| *offset <= at)
                .map(|(_, instrument)| *instrument)
        }) {
            Some(instrument) => instrument,
            None => {
                default = Instrument::new();
                &default
            }
        };
        let named = self
            .context
            .instruments
            .iter()
            .find(|placed| placed.instrument.kind() == closest.kind())
            .ok_or_else(|| {
                export_error(format!(
                    "Instrument instance {} for a note not found in instrumentStream",
                    closest.kind()
                ))
            })?;
        let element = mxnote.sub("instrument");
        if let Some(id) = named.instrument.instrument_id() {
            element.set("id", id);
        }
        Ok(())
    }

    /// music21's `chordToXml`: a `<note>` for each of the chord's notes,
    /// in the order the chord holds them, every one after the first marked `<chord/>`.
    fn chord_elements(
        &mut self,
        chord: &Chord,
        position: Option<*const StreamElement>,
    ) -> Result<()> {
        for (index, note) in chord.notes().iter().enumerate() {
            let element = self.note_element(note, index, Some(chord), position)?;
            self.root.push(element);
        }
        Ok(())
    }

    /// music21's `restToXml`.
    fn rest_element(
        &self,
        rest: &Rest,
        offset: FloatType,
        position: Option<*const StreamElement>,
    ) -> Result<Element> {
        let mut mxnote = Element::new("note");
        if rest.hidden() {
            mxnote.set("print-object", "no");
            mxnote.set("print-spacing", "yes");
        }
        let duration = rest.duration();
        let full_measure = self
            .meter_at(offset)
            .is_some_and(|meter| meter.bar_quarter_length() == duration.quarter_length());
        let mxrest = mxnote.sub("rest");
        if full_measure {
            mxrest.set("measure", "yes");
        }
        if rest.step_shift() != 0 {
            // music21 counts from the middle line of the clef in force, a
            // treble clef's where there is none that places pitches.
            // The clef is the last one standing in the staff at or before
            // the rest, wherever the writer has got to in another voice.
            let at = self.start + offset;
            let lowest = self
                .context
                .own_clefs
                .iter()
                .rev()
                .find(|(clef_offset, _)| *clef_offset <= at)
                .and_then(|(_, clef)| clef.lowest_line())
                .or_else(|| Clef::treble().lowest_line())
                .unwrap_or(31);
            let (step, octave) = step_and_octave(lowest + 4 + rest.step_shift());
            mxrest.sub_text("display-step", step.to_string());
            mxrest.sub_text("display-octave", octave.to_string());
        }
        mxnote.sub_text("duration", divisions(duration.quarter_length()).to_string());
        if let Some(tie) = rest.tie() {
            for element in tie_elements(tie) {
                mxnote.push(element);
            }
        }
        if let Some(voice) = &self.current_voice {
            mxnote.sub_text("voice", voice.as_str());
        }
        if !full_measure {
            let (written_type, dots) = written_type(duration)?;
            let written = mxnote.sub("type");
            written.set_text(written_type);
            if let Some(size) = rest.size() {
                written.set("size", size.as_str());
            }
            for _ in 0..dots {
                mxnote.sub("dot");
            }
        }
        if let Some(modification) = time_modification(duration)? {
            mxnote.push(modification);
        }
        let mut notations = notations(
            Marks {
                expressions: rest.expressions(),
                articulations: rest.articulations(),
                tie: rest.tie(),
                index: 0,
                last: 0,
                arpeggios: &self.arpeggios,
                pitch: None,
            },
            position,
            &self.context.spanners,
        )?;
        notations.extend(tuplet_notations(duration)?);
        if !notations.is_empty() {
            let mxnotations = mxnote.sub("notations");
            for element in notations {
                mxnotations.push(element);
            }
        }
        for lyric in rest.lyrics() {
            if lyric.explicit_text().is_none() {
                continue;
            }
            mxnote.push(lyric_element(lyric)?);
        }
        Ok(mxnote)
    }

    /// The metre in force at an offset within the measure: music21's
    /// `getContextByClass(TimeSignature)` from a rest.
    fn meter_at(&self, offset: FloatType) -> Option<TimeSignature> {
        self.stream
            .events()
            .iter()
            .rev()
            .filter(|event| event.offset() <= offset)
            .find_map(|event| match event.element() {
                StreamElement::TimeSignature(meter) => Some(meter.clone()),
                _ => None,
            })
            .or_else(|| self.context.meter.clone())
    }

    // ---------------------------------------------------- directions

    /// music21's `setOffsetOptional`: an `<offset>` for a direction that does
    /// not stand where the writer has got to.
    fn offset_element(&self, offset: FloatType, sound: bool) -> Option<Element> {
        offset_element_from(offset, self.offset_in_measure, sound)
    }

    /// music21's `placeInDirection`.
    fn direction(
        &self,
        inner: Element,
        placement: Option<Placement>,
        offset: FloatType,
        sound: bool,
    ) -> Element {
        let mut direction = Element::new("direction");
        direction.sub("direction-type").push(inner);
        if let Some(placement) = placement {
            direction.set("placement", placement.as_str());
        }
        if let Some(element) = self.offset_element(offset, sound) {
            direction.push(element);
        }
        direction
    }

    /// music21's `segnoToXml` and `codaToXml` for a sign, and
    /// `textExpressionToXml` for words: a sign stands two staff lines up,
    /// and words are justified as their kind of mark is.
    fn repeat_expression_element(&self, mark: &RepeatExpression, offset: FloatType) -> Element {
        if mark.use_symbol() && mark.kind().has_symbol() {
            let tag = match mark.kind() {
                RepeatExpressionKind::Segno => "segno",
                _ => "coda",
            };
            let mut sign = Element::new(tag);
            sign.set("default-y", "20");
            return self.direction(sign, mark.placement(), offset, true);
        }
        let mut words = Element::with_text("words", mark.text());
        words.set("justify", mark.kind().justify());
        self.direction(words, mark.placement(), offset, false)
    }

    /// music21's `chordSymbolToXml`, and `noChordToXml` for a symbol saying
    /// nothing sounds.
    fn harmony_element(&self, symbol: &ChordSymbol, offset: FloatType) -> Result<Element> {
        let mut harmony = Element::new("harmony");
        if let Some(placement) = symbol.placement() {
            harmony.set("placement", placement.as_str());
        }
        let alter_of = |pitch: &Pitch| {
            pitch
                .written_accidental()
                .map(|accidental| accidental.alter())
        };
        if symbol.is_no_chord() {
            let root = harmony.sub("root");
            let step = root.sub("root-step");
            step.set_text("C");
            step.set("text", "");
            let kind = harmony.sub("kind");
            kind.set_text("none");
            kind.set(
                "text",
                symbol.kind_text().ok_or_else(|| {
                    export_error("NoChord object's chordKindStr must be non-empty")
                })?,
            );
        } else {
            let root = symbol.root();
            let written = harmony.sub("root");
            written.sub_text("root-step", root.step().as_char().to_string());
            if let Some(alter) = alter_of(root) {
                written.sub_text("root-alter", num_to_int_or_float(alter));
            }
            let mut kind = symbol.kind().unwrap_or("none");
            for (alias, target) in crate::chordsymbol::CHORD_KIND_ALIASES {
                if target == kind {
                    kind = alias;
                }
            }
            let written = harmony.sub("kind");
            written.set_text(kind);
            if let Some(text) = symbol.kind_text() {
                written.set("text", text);
            }
            // music21 reads the inversion off the chord the symbol sounds,
            // counted from the symbol's own root.
            let sounded = symbol.pitches().ok().filter(|pitches| !pitches.is_empty());
            let chord = sounded
                .as_ref()
                .and_then(|pitches| Chord::new(pitches.as_slice()).ok())
                .map(|mut chord| {
                    chord.set_root(Some(root.clone()));
                    chord
                });
            if let Some(inversion) = chord.as_ref().and_then(Chord::inversion)
                && inversion != 0
            {
                harmony.sub_text("inversion", inversion.to_string());
            }
            let bass = symbol.bass().cloned().or_else(|| {
                sounded
                    .as_ref()
                    .and_then(|pitches| pitches.first().cloned())
            });
            if let Some(bass) = bass
                && bass.name() != root.name()
            {
                let written = harmony.sub("bass");
                written.sub_text("bass-step", bass.step().as_char().to_string());
                if let Some(alter) = alter_of(&bass) {
                    written.sub_text("bass-alter", num_to_int_or_float(alter));
                }
            }
            for modification in symbol.chord_step_modifications() {
                let degree = harmony.sub("degree");
                degree.sub_text("degree-value", modification.degree().to_string());
                degree.sub_text(
                    "degree-alter",
                    num_to_int_or_float(modification.interval().semitones()),
                );
                degree.sub_text(
                    "degree-type",
                    modification.modification_type().music21_name(),
                );
            }
        }
        if let Some(element) = self.offset_element(offset, true) {
            harmony.push(element);
        }
        Ok(harmony)
    }

    /// music21's `dynamicToXml`.
    fn dynamic_element(&mut self, dynamic: &Dynamic, offset: FloatType) {
        let mut dynamics = Element::new("dynamics");
        if DYNAMIC_MARKS.contains(&dynamic.value()) {
            dynamics.sub(dynamic_tag(dynamic.value()));
        } else {
            dynamics.sub_text("other-dynamics", dynamic.value());
        }
        // A dynamic's style starts music21 off at these positions.
        dynamics.set("default-x", "-36");
        dynamics.set("default-y", "-80");
        let mut direction = self.direction(dynamics, dynamic.placement(), offset, true);
        let volume = (dynamic.volume_scalar() * 127.0) as i64;
        direction.sub("sound").set("dynamics", volume.to_string());
        self.root.push(direction);
    }

    /// music21's `tempoIndicationToXml` for a metronome mark: the mark, and
    /// its word as a direction of its own where the word was written.
    fn tempo_elements(&mut self, mark: &MetronomeMark, offset: FloatType) -> Result<()> {
        let shown = !mark.number_implicit() && mark.number().is_some();
        let direction = if shown {
            let mut metronome = Element::new("metronome");
            let (beat_unit, dots) = written_type(mark.referent())?;
            metronome.sub_text("beat-unit", beat_unit);
            for _ in 0..dots {
                metronome.sub("beat-unit-dot");
            }
            if let Some(number) = mark.number() {
                metronome.sub_text("per-minute", num_to_int_or_float(number));
            }
            metronome.set("parentheses", yes_no(mark.parentheses()));
            self.direction(metronome, mark.placement(), offset, true)
        } else {
            self.direction(Element::new("words"), mark.placement(), offset, true)
        };
        let mut direction = direction;
        if let Some(bpm) = mark.sounding_quarter_bpm().filter(|bpm| *bpm != 0.0) {
            direction
                .sub("sound")
                .set("tempo", num_to_int_or_float(bpm));
        }
        self.root.push(direction);

        if let Some(text) = mark.text_expression(false) {
            let direction = self.tempo_words(text, None, offset);
            self.root.push(direction);
        }
        Ok(())
    }

    /// music21's `tempoIndicationToXml` for a metric modulation: the two
    /// note values side by side, neither with its number, and the speed the
    /// new one is played at.
    fn metric_modulation_element(
        &self,
        modulation: &MetricModulation,
        offset: FloatType,
    ) -> Result<Element> {
        let mut metronome = Element::new("metronome");
        for side in [modulation.old_metronome(), modulation.new_metronome()] {
            let side = side.ok_or_else(|| {
                export_error("a metric modulation needs a mark on both sides to be written")
            })?;
            let (beat_unit, dots) = written_type(side.referent())?;
            metronome.sub_text("beat-unit", beat_unit);
            for _ in 0..dots {
                metronome.sub("beat-unit-dot");
            }
        }
        metronome.set("parentheses", yes_no(modulation.parentheses()));
        let mut direction = self.direction(metronome, modulation.placement(), offset, true);
        if let Some(bpm) = modulation
            .new_metronome()
            .and_then(MetronomeMark::sounding_quarter_bpm)
            .filter(|bpm| *bpm != 0.0)
        {
            direction
                .sub("sound")
                .set("tempo", num_to_int_or_float(bpm));
        }
        Ok(direction)
    }

    /// A tempo said in words, as music21's `TempoText.applyTextFormatting`
    /// sets them: bold, four and a half staff lines up.
    fn tempo_words(&self, text: &str, placement: Option<Placement>, offset: FloatType) -> Element {
        let mut words = Element::with_text("words", text);
        words.set("default-y", "45");
        words.set("font-weight", "bold");
        self.direction(words, placement, offset, false)
    }
}

/// music21's `noteToNotations`: what goes inside a note's `<notations>`,
/// less the tuplet brackets, which the caller adds after.
///
/// Expressions come from the chord where the note is one of a chord's. An
/// arpeggio is written on every note of it, a bracket against arpeggiating
/// on the bottom and top notes only, and everything else on the first note
/// alone; ornaments are gathered into one `<ornaments>` at the end.
fn note_notations(
    note: &Note,
    index: usize,
    chord: Option<&Chord>,
    position: Option<*const StreamElement>,
    spanners: &[Numbered<'_>],
    arpeggios: &ArpeggioNumbers,
    pitch: FloatType,
) -> Result<Vec<Element>> {
    notations(
        Marks {
            expressions: chord.map_or(note.expressions(), Chord::expressions),
            articulations: chord.map_or(note.articulations(), Chord::articulations),
            tie: note.tie(),
            index,
            last: chord.map_or(0, |chord| chord.notes().len().saturating_sub(1)),
            arpeggios,
            pitch: Some(pitch),
        },
        position,
        spanners,
    )
}

/// What goes into one `<note>`'s notations: the expressions and
/// articulations it is written with, its tie, and which note of its chord it
/// is, the last being the top.
struct Marks<'a> {
    expressions: &'a [Expression],
    articulations: &'a [Articulation],
    tie: Option<&'a Tie>,
    index: usize,
    last: usize,
    /// The numbers the measure's arpeggios across chords are written with.
    arpeggios: &'a ArpeggioNumbers,
    /// The note's pitch space, where it is a note.
    pitch: Option<FloatType>,
}

/// music21's `noteToNotations`, for a note or a rest.
fn notations(
    marks: Marks<'_>,
    position: Option<*const StreamElement>,
    spanners: &[Numbered<'_>],
) -> Result<Vec<Element>> {
    let index = marks.index;
    let first = index == 0;
    let mut notations = Vec::new();
    let (articulations, technical) = articulation_elements(marks.articulations, index);
    let mut ornaments: Option<Element> = None;
    for expression in marks.expressions {
        match expression {
            Expression::Arpeggio(ArpeggioType::NonArpeggio) => {
                let last = marks.last;
                let side = if first {
                    Some("bottom")
                } else if index == last {
                    Some("top")
                } else {
                    None
                };
                if let Some(side) = side {
                    let mut element = Element::new("non-arpeggiate");
                    element.set("type", side);
                    notations.push(element);
                }
            }
            Expression::Arpeggio(arpeggio) => {
                let mut element = Element::new("arpeggiate");
                if *arpeggio != ArpeggioType::Normal {
                    element.set("direction", arpeggio.as_str());
                }
                notations.push(element);
            }
            Expression::Fermata(fermata) if first => {
                let mut element = Element::new("fermata");
                element.set("type", fermata.fermata_type().as_str());
                if let Some(shape @ ("angled" | "square")) = fermata.shape() {
                    element.set_text(shape);
                }
                notations.push(element);
            }
            Expression::Ornament(ornament) if first => {
                let written = ornaments.get_or_insert_with(|| Element::new("ornaments"));
                written.push(ornament_element(ornament));
                for mark in ornament_accidental_marks(ornament)? {
                    written.push(mark);
                }
            }
            Expression::Fermata(_) | Expression::Ornament(_) => {}
        }
    }
    if let Some(tie) = marks.tie {
        notations.extend(tied_elements(tie));
    }
    if let Some(position) = position {
        notations.extend(spanner_notations(position, spanners, &marks));
    }
    notations.extend(articulations);
    notations.extend(technical);
    notations.extend(ornaments);
    Ok(notations)
}

/// The `<articulations>` and `<technical>` of a note, the `index`th of its
/// chord where it is one: a fingering is written on the note it is the
/// turn of, anything else on the first note only. A pizzicato is written as
/// an attribute of the note instead, and a string numbered below one not at
/// all.
fn articulation_elements(
    articulations: &[Articulation],
    index: usize,
) -> (Option<Element>, Option<Element>) {
    let mut fingering = 0;
    let mut applicable = Vec::new();
    for articulation in articulations {
        if articulation.is_a("Fingering") {
            if fingering == index {
                applicable.push(articulation);
            }
            fingering += 1;
        } else if index == 0 && !articulation.is_a("HammerOn") && !articulation.is_a("PullOff") {
            applicable.push(articulation);
        }
    }
    let mut written: Option<Element> = None;
    let mut technical: Option<Element> = None;
    for articulation in applicable {
        if articulation.is_a("Pizzicato")
            || (articulation.is_a("StringIndication") && articulation.number() < 1)
        {
            continue;
        }
        if articulation.is_a("TechnicalIndication") {
            technical
                .get_or_insert_with(|| Element::new("technical"))
                .push(technical_element(articulation));
        } else {
            written
                .get_or_insert_with(|| Element::new("articulations"))
                .push(articulation_element(articulation));
        }
    }
    (written, technical)
}

/// music21's `xmlObjects.ARTICULATION_MARKS_REV`, in the order it is
/// searched: the staccato and the accent last, as they are what others
/// descend from.
const ARTICULATION_NAMES: [(&str, &str); 15] = [
    ("StrongAccent", "strong-accent"),
    ("Staccatissimo", "staccatissimo"),
    ("Spiccato", "spiccato"),
    ("Tenuto", "tenuto"),
    ("DetachedLegato", "detached-legato"),
    ("Scoop", "scoop"),
    ("Plop", "plop"),
    ("Doit", "doit"),
    ("Falloff", "falloff"),
    ("BreathMark", "breath-mark"),
    ("Caesura", "caesura"),
    ("Stress", "stress"),
    ("Unstress", "unstress"),
    ("Staccato", "staccato"),
    ("Accent", "accent"),
];

/// music21's `articulationToXmlArticulation`.
fn articulation_element(articulation: &Articulation) -> Element {
    let name = ARTICULATION_NAMES
        .iter()
        .find(|(class, _)| articulation.is_a(class))
        .map_or("other-articulation", |(_, name)| name);
    let mut element = Element::new(name);
    if let Some(placement) = articulation.placement() {
        element.set("placement", placement);
    }
    match name {
        "strong-accent" => {
            element.set("type", articulation.point_direction().unwrap_or("None"));
        }
        "breath-mark" => {
            if let Some(symbol) = articulation.symbol() {
                element.set_text(symbol);
            }
        }
        "other-articulation" => {
            if let Some(text) = articulation.display_text() {
                element.set_text(text);
            }
        }
        _ => {}
    }
    element
}

/// music21's `xmlObjects.TECHNICAL_MARKS_REV`, in the order it is searched.
const TECHNICAL_NAMES: [(&str, &str); 20] = [
    ("UpBow", "up-bow"),
    ("DownBow", "down-bow"),
    ("StringHarmonic", "harmonic"),
    ("OpenString", "open-string"),
    ("StringThumbPosition", "thumb-position"),
    ("Fingering", "fingering"),
    ("FrettedPluck", "pluck"),
    ("DoubleTongue", "double-tongue"),
    ("TripleTongue", "triple-tongue"),
    ("Stopped", "stopped"),
    ("SnapPizzicato", "snap-pizzicato"),
    ("StringIndication", "string"),
    ("FretBend", "bend"),
    ("FretTap", "tap"),
    ("FretIndication", "fret"),
    ("OrganHeel", "heel"),
    ("OrganToe", "toe"),
    ("HarpFingerNails", "fingernails"),
    ("HandbellIndication", "handbell"),
    ("Harmonic", "harmonic"),
];

/// music21's `articulationToXmlTechnical`.
fn technical_element(articulation: &Articulation) -> Element {
    use crate::articulations::Finger;
    let name = TECHNICAL_NAMES
        .iter()
        .find(|(class, _)| articulation.is_a(class))
        .map_or("other-technical", |(_, name)| name);
    let mut element = Element::new(name);
    if let Some(placement) = articulation.placement() {
        element.set("placement", placement);
    }
    match name {
        "fingering" => {
            element.set_text(match articulation.finger() {
                Some(Finger::Number(number)) => number.to_string(),
                Some(Finger::Written(text)) => text.clone(),
                None => "None".to_string(),
            });
            element.set("alternate", yes_no(articulation.alternate()));
            element.set("substitution", yes_no(articulation.substitution()));
        }
        "heel" | "toe" => element.set("substitution", yes_no(articulation.substitution())),
        "string" | "fret" => element.set_text(articulation.number().to_string()),
        "bend" => {
            let alter = element.sub("bend-alter");
            if let Some(interval) = articulation.bend_alter() {
                alter.set_text(num_to_int_or_float(interval.semitones()));
            }
            if articulation.pre_bend() {
                element.sub("pre-bend");
            }
            if let Some(release) = articulation.release() {
                let offset = (DIVISIONS_PER_QUARTER * release) as i64;
                element.sub("release").set("offset", offset.to_string());
            }
            if let Some(with_bar) = articulation.with_bar() {
                element.sub_text("with-bar", with_bar);
            }
        }
        "harmonic" => {
            if articulation.is_a("StringHarmonic") {
                match articulation.harmonic_type() {
                    Some("artificial") => {
                        element.sub("artificial");
                    }
                    Some("natural") => {
                        element.sub("natural");
                    }
                    _ => {}
                }
                match articulation.pitch_type() {
                    Some("base") => {
                        element.sub("base-pitch");
                    }
                    Some("sounding") => {
                        element.sub("sounding-pitch");
                    }
                    Some("touching") => {
                        element.sub("touching-pitch");
                    }
                    _ => {}
                }
            }
        }
        "handbell" | "other-technical" => {
            if let Some(text) = articulation.display_text() {
                element.set_text(text);
            }
        }
        _ => {}
    }
    element
}

/// music21's `relatedSpanners`: the directions a hairpin opens with before
/// the element it starts on, and closes with after the one it ends on. A
/// hairpin over one element does both. A pedal drawn as a sign and a line
/// resumes its line straight after the sign, at the pedal mark's own offset
/// counted from `offset_in_measure`: music21's `makePedalResumeLineXml`.
fn related_spanners(
    position: *const StreamElement,
    spanners: &[Numbered<'_>],
    offset_in_measure: FloatType,
) -> (Vec<Element>, Vec<Element>) {
    let mut before = Vec::new();
    let mut after = Vec::new();
    // music21 asks the octave lines first, then the hairpins, the lines and
    // the pedal marks, and of each only those holding the element.
    let of_kind = |wanted: fn(SpannerKind) -> bool| {
        spanners
            .iter()
            .filter(move |numbered| wanted(numbered.spanner.kind()))
    };
    let ordered = of_kind(|kind| kind == SpannerKind::Ottava)
        .chain(of_kind(SpannerKind::is_wedge))
        .chain(of_kind(|kind| kind == SpannerKind::Line))
        .chain(of_kind(|kind| kind == SpannerKind::PedalMark));
    for numbered in ordered {
        let (spanner, id_local) = (numbered.spanner, numbered.id_local);
        if !numbered.holds(position) {
            continue;
        }
        let single = numbered.elements.len() == 1;
        let starts = single || numbered.is_first(position);
        let stops = single || numbered.is_last(position);
        for (here, start) in [(starts, true), (stops, false)] {
            if !here {
                continue;
            }
            let mark = match spanner.kind() {
                SpannerKind::Ottava => {
                    let mut shift = Element::new("octave-shift");
                    shift.set("number", id_local.to_string());
                    let octave = spanner.shift();
                    // An `8va` is written `down`: the notes are engraved
                    // below where they sound.
                    shift.set(
                        "type",
                        match (start, octave.is_some_and(|octave| octave.up())) {
                            (false, _) => "stop",
                            (true, true) => "down",
                            (true, false) => "up",
                        },
                    );
                    if let Some(octave) = octave {
                        shift.set("size", octave.magnitude().to_string());
                    }
                    shift
                }
                SpannerKind::Line => {
                    let mut bracket = Element::new("bracket");
                    bracket.set("number", id_local.to_string());
                    bracket.set("line-type", spanner.line_type().unwrap_or("solid"));
                    bracket.set("type", if start { "start" } else { "stop" });
                    let ends = spanner.ends();
                    let (tick, height) = if start {
                        (ends.start, ends.start_height)
                    } else {
                        (ends.end, ends.end_height)
                    };
                    bracket.set("line-end", tick.as_str());
                    if let Some(height) = height {
                        bracket.set("end-length", num_to_int_or_float(height));
                    }
                    bracket
                }
                SpannerKind::PedalMark => pedal_element(spanner, id_local, start),
                _ => wedge_element(spanner, id_local, start),
            };
            let mut direction = Element::new("direction");
            if let Some(placement) = spanner.placement() {
                direction.set("placement", placement.as_str());
            }
            direction.sub("direction-type").push(mark);
            if start {
                before.push(direction);
            } else {
                after.push(direction);
            }
            if start
                && spanner.kind() == SpannerKind::PedalMark
                && spanner.pedal().and_then(|pedal| pedal.form) == Some(PedalForm::SymbolLine)
            {
                let mut line = Element::new("pedal");
                line.set("type", "resume");
                line.set("line", "yes");
                let mut direction = Element::new("direction");
                direction.sub("direction-type").push(line);
                if let Some(placement) = spanner.placement() {
                    direction.set("placement", placement.as_str());
                }
                if let Some(offset) = offset_element_from(spanner.offset(), offset_in_measure, true)
                {
                    direction.push(offset);
                }
                before.push(direction);
            }
        }
    }
    (before, after)
}

/// music21's `setOffsetOptional`: an `<offset>` for a direction at `offset`
/// that does not stand where the writer has got to, `offset_in_measure`.
fn offset_element_from(
    offset: FloatType,
    offset_in_measure: FloatType,
    sound: bool,
) -> Option<Element> {
    // music21 counts its place in the measure as a float, and holds an
    // offset no float spells exactly -- a triplet's -- as a `Fraction`,
    // which compares unequal to every float.
    if offset == offset_in_measure && !is_fraction_offset(offset) {
        return None;
    }
    let difference = ((offset - offset_in_measure) * DIVISIONS_PER_QUARTER) as i64;
    let mut element = Element::with_text("offset", difference.to_string());
    if sound {
        element.set("sound", "yes");
    }
    Some(element)
}

/// A pedal mark's `<pedal>`, where it goes down or where it comes up:
/// music21's `_spannerStartParameters` and `_spannerEndParameters`.
///
/// MusicXML has no start for any pedal but the sustaining and sostenuto
/// ones, so music21 writes the soft and silent pedals as a plain start.
fn pedal_element(spanner: &Spanner, id_local: u32, start: bool) -> Element {
    let pedal = spanner.pedal().unwrap_or_default();
    let mut element = Element::new("pedal");
    element.set("number", id_local.to_string());
    let line = pedal.form.is_some_and(PedalForm::has_line);
    if start {
        element.set(
            "type",
            if pedal.pedal_type == Some(PedalType::Sostenuto) {
                "sostenuto"
            } else {
                "start"
            },
        );
        if pedal.form == Some(PedalForm::Line) {
            element.set("line", "yes");
        } else {
            element.set("sign", "yes");
        }
        if pedal.abbreviated {
            element.set("abbreviated", "yes");
        }
    } else {
        element.set("type", "stop");
        if line {
            element.set("line", "yes");
        } else {
            element.set("sign", "yes");
        }
    }
    element
}

/// A hairpin's `<wedge>`, where it opens or where it closes: a crescendo
/// opens from nothing and a diminuendo closes to nothing.
fn wedge_element(spanner: &Spanner, id_local: u32, start: bool) -> Element {
    let crescendo = spanner.kind() == SpannerKind::Crescendo;
    let mut wedge = Element::new("wedge");
    wedge.set("number", id_local.to_string());
    wedge.set(
        "type",
        match (start, crescendo) {
            (true, true) => "crescendo",
            (true, false) => "diminuendo",
            (false, _) => "stop",
        },
    );
    if start == crescendo {
        wedge.set("spread", "0");
    } else if let Some(spread) = spanner.spread() {
        wedge.set("spread", num_to_int_or_float(spread));
    }
    wedge
}

/// music21's `objectAttachedSpannersToNotations`: a slur's start on its
/// first note and its stop on its last, and nothing on the notes between.
fn spanner_notations(
    position: *const StreamElement,
    spanners: &[Numbered<'_>],
    marks: &Marks<'_>,
) -> Vec<Element> {
    let mut notations = Vec::new();
    // music21's `appendArpeggioMarkSpannersToNotations`: on every note of
    // every chord the arpeggio holds, or a bracket on its lowest and highest
    // notes alone; numbered when it holds more than one note.
    for numbered in spanners {
        let spanner = numbered.spanner;
        if spanner.kind() != SpannerKind::ArpeggioMark || !numbered.holds(position) {
            continue;
        }
        let mut element = if spanner.arpeggio() == ArpeggioType::NonArpeggio {
            let Some(((low, low_ps), (high, high_ps))) = numbered.note_extremes() else {
                continue;
            };
            let is = |at: *const StreamElement, ps: FloatType| {
                std::ptr::eq(at, position) && marks.pitch == Some(ps)
            };
            let side = if is(low, low_ps) {
                "bottom"
            } else if is(high, high_ps) {
                "top"
            } else {
                continue;
            };
            let mut element = Element::new("non-arpeggiate");
            element.set("type", side);
            element
        } else {
            let mut element = Element::new("arpeggiate");
            if spanner.arpeggio() != ArpeggioType::Normal {
                element.set("direction", spanner.arpeggio().as_str());
            }
            element
        };
        let notes: usize = numbered
            .elements
            .iter()
            .flatten()
            .map(|element| match element {
                StreamElement::Chord(chord) => chord.notes().len(),
                _ => 1,
            })
            .sum();
        if numbered.elements.len() > 1 || notes > 1 {
            element.set("number", marks.arpeggios.number(spanner).to_string());
        }
        notations.push(element);
    }
    if marks.index != 0 {
        return notations;
    }
    for numbered in spanners {
        let (spanner, id_local) = (numbered.spanner, numbered.id_local);
        if spanner.kind() != SpannerKind::Slur {
            continue;
        }
        let mut slur = Element::new("slur");
        if numbered.is_first(position) {
            slur.set("type", "start");
            if let Some(line_type) = spanner.line_type() {
                slur.set("line-type", line_type);
            }
            if let Some(placement) = spanner.placement() {
                slur.set("placement", placement.as_str());
            }
        } else if numbered.is_last(position) {
            slur.set("type", "stop");
        } else {
            continue;
        }
        slur.set("number", id_local.to_string());
        notations.push(slur);
    }
    // A glissando, or a slide where it is played without steps, written as
    // a slur is; only its start carries its words.
    for numbered in spanners {
        let (spanner, id_local) = (numbered.spanner, numbered.id_local);
        if spanner.kind() != SpannerKind::Glissando {
            continue;
        }
        let details = spanner.glissando_details().cloned().unwrap_or_default();
        let mut glissando = Element::new(if details.slide_type == SlideType::Continuous {
            "slide"
        } else {
            "glissando"
        });
        glissando.set("number", id_local.to_string());
        if let Some(line_type) = spanner.line_type() {
            glissando.set("line-type", line_type);
        }
        if numbered.is_first(position) {
            if let Some(label) = details.label {
                glissando.set_text(label);
            }
            glissando.set("type", "start");
        } else if numbered.is_last(position) {
            glissando.set("type", "stop");
        } else {
            continue;
        }
        notations.push(glissando);
    }
    // A tremolo between notes, then a trill's wavy line: where it starts,
    // where it stops, and both on a line over one note. These go in an
    // `<ornaments>` of their own.
    let mut ornaments = Element::new("ornaments");
    for numbered in spanners {
        let spanner = numbered.spanner;
        if spanner.kind() != SpannerKind::TremoloSpanner || !numbered.holds(position) {
            continue;
        }
        let mut tremolo = Element::with_text(
            "tremolo",
            spanner.number_of_marks().unwrap_or(3).to_string(),
        );
        if numbered.is_first(position) {
            tremolo.set("type", "start");
            if let Some(placement) = spanner.placement() {
                tremolo.set("placement", placement.as_str());
            }
        } else if numbered.is_last(position) {
            tremolo.set("type", "stop");
        }
        ornaments.push(tremolo);
    }
    for numbered in spanners {
        let (spanner, id_local) = (numbered.spanner, numbered.id_local);
        if spanner.kind() != SpannerKind::TrillExtension {
            continue;
        }
        let first = numbered.is_first(position);
        let last = numbered.is_last(position);
        if !first && !last {
            continue;
        }
        let mut line = Element::new("wavy-line");
        line.set("number", id_local.to_string());
        if first {
            line.set("type", "start");
            if let Some(placement) = spanner.placement() {
                line.set("placement", placement.as_str());
            }
        } else {
            line.set("type", "stop");
        }
        ornaments.push(line);
        if first && last {
            let mut stop = Element::new("wavy-line");
            stop.set("number", id_local.to_string());
            stop.set("type", "stop");
            ornaments.push(stop);
        }
    }
    if ornaments.len() > 0 {
        notations.push(ornaments);
    }
    notations
}

/// music21's `expressionToXml` for an ornament.
fn ornament_element(ornament: &Ornament) -> Element {
    let tag = if ornament.is_a("Turn") {
        match (ornament.is_a("InvertedTurn"), ornament.is_delayed()) {
            (true, true) => "delayed-inverted-turn",
            (true, false) => "inverted-turn",
            (false, true) => "delayed-turn",
            (false, false) => "turn",
        }
    } else {
        [
            ("Trill", "trill-mark"),
            ("InvertedMordent", "inverted-mordent"),
            ("Mordent", "mordent"),
            ("Shake", "shake"),
            ("Schleifer", "schleifer"),
            ("Tremolo", "tremolo"),
        ]
        .into_iter()
        .find(|(class, _)| ornament.is_a(class))
        .map_or("other-ornament", |(_, tag)| tag)
    };
    let mut element = Element::new(tag);
    if let Some(placement) = ornament.placement() {
        element.set("placement", placement);
    }
    if ornament.is_a("Tremolo") {
        element.set("type", "single");
        element.set_text(ornament.number_of_marks().to_string());
    }
    element
}

/// music21's `ornamentToMxAccidentalMarks`: the accidentals an ornament's
/// neighbours are shown with, above or below it.
fn ornament_accidental_marks(ornament: &Ornament) -> Result<Vec<Element>> {
    let shown = |accidental: Option<&Accidental>| {
        accidental
            .filter(|accidental| accidental.display_status() == Some(true))
            .cloned()
    };
    let mut marks = Vec::new();
    let mut mark = |accidental: Accidental, placement: &str| -> Result<()> {
        let mut element = accidental_element(&accidental, "accidental-mark")?;
        element.set("placement", placement);
        marks.push(element);
        Ok(())
    };
    if ornament.is_a("Turn") {
        if let Some(upper) = shown(ornament.upper_accidental()) {
            mark(upper, "above")?;
        }
        if let Some(lower) = shown(ornament.lower_accidental()) {
            mark(lower, "below")?;
        }
    } else if (ornament.is_a("GeneralMordent") || ornament.is_a("Trill"))
        && let Some(accidental) = shown(ornament.accidental())
    {
        let below = ornament.direction() == Some(crate::interval::IntervalDirection::Descending);
        mark(accidental, if below { "below" } else { "above" })?;
    }
    Ok(marks)
}

/// music21's `setBarline`, with `barlineToXml` and `repeatToXml`: a plain
/// barline says how it is drawn, a repeat sign only which way it repeats.
fn barline_element(
    barline: Option<&Barline>,
    location: &str,
    ending: Option<(&Ending, &str)>,
) -> Element {
    let mut element = Element::new("barline");
    let repeat = barline.and_then(Barline::repeat_direction);
    if let Some(barline) = barline
        && repeat.is_none()
    {
        element.sub_text("bar-style", barline.bar_type().musicxml_bar_style());
    }
    element.set("location", location);
    if let Some((ending, kind)) = ending {
        let written = element.sub("ending");
        written.set("number", ending.number_text());
        written.set("type", kind);
    }
    if let (Some(barline), Some(direction)) = (barline, repeat) {
        let written = element.sub("repeat");
        written.set(
            "direction",
            match direction {
                RepeatDirection::Start => "forward",
                RepeatDirection::End => "backward",
            },
        );
        if let Some(times) = barline.repeat_times() {
            written.set("times", times.to_string());
        }
    }
    element
}

/// The last thing of one sort a stream holds.
fn last_of<T>(stream: &Stream, read: impl Fn(&StreamElement) -> Option<T>) -> Option<T> {
    stream
        .events()
        .iter()
        .rev()
        .find_map(|event| read(event.element()))
}

/// Whether an element is one of music21's `GeneralNote`s.
fn is_general_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::Rest(_)
            | StreamElement::ChordSymbol(_)
    )
}

/// Whether an element is written as a `<note>`, which a chord symbol is not
/// unless it asks to be.
fn is_written_as_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::Rest(_)
    )
}

/// The step and octave of a diatonic note number, as music21's
/// `diatonicNoteNum` setter reads one: the octave truncated toward nought.
fn step_and_octave(number: i32) -> (char, i32) {
    let octave = (number - 1) / 7;
    let index = (number - 1 - 7 * octave).rem_euclid(7);
    (['C', 'D', 'E', 'F', 'G', 'A', 'B'][index as usize], octave)
}

/// music21's `xmlObjects.fractionToPercent`: a share as a whole percent,
/// truncated.
fn fraction_to_percent(share: FloatType) -> String {
    ((share * 100.0) as i64).to_string()
}

/// A length in divisions, rounded as Python's `round` rounds.
fn divisions(quarter_length: FloatType) -> i64 {
    (quarter_length * DIVISIONS_PER_QUARTER).round_ties_even() as i64
}

fn format_int(value: FloatType) -> String {
    (value as i64).to_string()
}

/// music21's `common.numToIntOrFloat`, written as Python writes the result.
fn num_to_int_or_float(value: FloatType) -> String {
    let rounded = value.round_ties_even();
    if (rounded - value).abs() < 1e-6 {
        format!("{}", rounded as i64)
    } else {
        format!("{value}")
    }
}

/// music21's `typeToMusicXMLType`.
fn musicxml_type(duration_type: DurationType) -> Result<&'static str> {
    match duration_type {
        DurationType::Longa => Ok("long"),
        DurationType::TwentyFortyEighth => Err(export_error(
            "Cannot convert \"2048th\" duration to MusicXML (too short).",
        )),
        DurationType::DuplexMaxima => Err(export_error(
            "Cannot convert \"duplex-maxima\" duration to MusicXML (too long).",
        )),
        DurationType::Zero => Err(export_error(
            "Cannot convert durations without types to MusicXML.",
        )),
        other => Ok(other.music21_name()),
    }
}

/// The one written value a duration is, as music21's `type` and `dots`
/// read it, and the errors `typeToMusicXMLType` raises for the rest.
fn written_type(duration: &Duration) -> Result<(&'static str, u32)> {
    let components = duration.components();
    match components.as_slice() {
        [(duration_type, dots)] => Ok((musicxml_type(*duration_type)?, *dots)),
        [] if duration.quarter_length() == 0.0 => musicxml_type(DurationType::Zero).map(|t| (t, 0)),
        [] => Err(export_error(
            "Cannot convert inexpressible durations to MusicXML.",
        )),
        _ => Err(export_error(
            "Cannot convert complex durations to MusicXML. Try exporting with \
             makeNotation=True or manually running splitAtDurations()",
        )),
    }
}

/// The `<time-modification>` a duration inside tuplets carries.
fn time_modification(duration: &Duration) -> Result<Option<Element>> {
    let tuplets = duration.tuplets();
    match tuplets.as_slice() {
        [] => Ok(None),
        [tuplet] => tuplet_to_time_modification(tuplet).map(Some),
        _ => {
            let multiplier = duration.aggregate_tuplet_multiplier();
            let (numerator, denominator) = (
                *multiplier.numer().unwrap_or(&1),
                *multiplier.denom().unwrap_or(&1),
            );
            let composite = Tuplet::ratio(
                u32::try_from(denominator).unwrap_or(1),
                u32::try_from(numerator).unwrap_or(1),
            );
            tuplet_to_time_modification(&composite).map(Some)
        }
    }
}

/// music21's `tupletToTimeModification`.
fn tuplet_to_time_modification(tuplet: &Tuplet) -> Result<Element> {
    let mut element = Element::new("time-modification");
    element.sub_text("actual-notes", tuplet.actual().to_string());
    element.sub_text("normal-notes", tuplet.normal().to_string());
    if let Some((duration_type, dots)) = tuplet.duration_normal() {
        element.sub_text("normal-type", musicxml_type(duration_type)?);
        for _ in 0..dots {
            element.sub("normal-dot");
        }
    }
    Ok(element)
}

/// Every tuplet bracket a duration's tuplets open or close, numbered from
/// the outermost.
fn tuplet_notations(duration: &Duration) -> Result<Vec<Element>> {
    let mut out = Vec::new();
    for (index, tuplet) in duration.tuplets().iter().enumerate() {
        out.extend(tuplet_elements(tuplet, index + 1)?);
    }
    Ok(out)
}

/// music21's `tupletToXmlTuplet`: a bracket's start, with how it is drawn
/// and what it shows, or its stop, or both for a tuplet of one note.
fn tuplet_elements(tuplet: &Tuplet, number: usize) -> Result<Vec<Element>> {
    use crate::duration::{TupletBracket, TupletShow, TupletType};
    let types: &[&str] = match tuplet.tuplet_type() {
        None => return Ok(Vec::new()),
        Some(TupletType::Start) => &["start"],
        Some(TupletType::Stop) => &["stop"],
        Some(TupletType::StartStop) => &["start", "stop"],
    };
    let mut out = Vec::new();
    for kind in types {
        let mut element = Element::new("tuplet");
        element.set("type", *kind);
        element.set("number", number.to_string());
        if *kind == "start" {
            element.set("bracket", yes_no(tuplet.bracket() != TupletBracket::None));
            if let Some(placement) = tuplet.placement() {
                element.set("placement", placement.as_str());
            }
            let actual = tuplet.actual_show();
            let normal = tuplet.normal_show();
            let shows_number = |show: Option<TupletShow>| {
                matches!(show, Some(TupletShow::Both | TupletShow::Number))
            };
            let shows_type = |show: Option<TupletShow>| {
                matches!(show, Some(TupletShow::Both | TupletShow::Type))
            };
            if actual.is_none() {
                element.set("show-number", "none");
            } else if shows_number(actual) && shows_number(normal) {
                element.set("show-number", "both");
            } else if actual == Some(TupletShow::Both) {
                // music21 asks whether the actual count is shown as `both` or
                // as `actual`, which it never is.
                element.set("show-number", "actual");
            }
            if shows_type(actual) && shows_type(normal) {
                element.set("show-type", "both");
            } else if shows_type(actual) {
                element.set("show-type", "actual");
            }
            if tuplet.bracket() == TupletBracket::Slur {
                element.set("line-shape", "curved");
            }
            for (tag, count, value) in [
                ("tuplet-actual", tuplet.actual(), tuplet.duration_actual()),
                ("tuplet-normal", tuplet.normal(), tuplet.duration_normal()),
            ] {
                let side = element.sub(tag);
                side.sub_text("tuplet-number", count.to_string());
                if let Some((duration_type, dots)) = value {
                    side.sub_text("tuplet-type", musicxml_type(duration_type)?);
                    for _ in 0..dots {
                        side.sub("tuplet-dot");
                    }
                }
            }
        }
        out.push(element);
    }
    Ok(out)
}

/// music21's `pitchToXml`.
fn pitch_element(pitch: &Pitch) -> Element {
    let mut element = Element::new("pitch");
    element.sub_text("step", pitch.step().as_char().to_string());
    if let Some(accidental) = pitch.written_accidental() {
        element.sub_text("alter", num_to_int_or_float(accidental.alter()));
    }
    element.sub_text("octave", pitch.implicit_octave().to_string());
    element
}

/// The accidental names music21 writes as they stand: its own names, and the
/// MusicXML ones it has no name for.
const KNOWN_ACCIDENTALS: [&str; 40] = [
    "natural",
    "sharp",
    "double-sharp",
    "triple-sharp",
    "quadruple-sharp",
    "flat",
    "double-flat",
    "triple-flat",
    "quadruple-flat",
    "half-sharp",
    "one-and-a-half-sharp",
    "half-flat",
    "one-and-a-half-flat",
    "double-sharp-down",
    "double-sharp-up",
    "flat-flat-down",
    "flat-flat-up",
    "arrow-down",
    "arrow-up",
    "other",
    "sharp-down",
    "sharp-up",
    "natural-down",
    "natural-up",
    "flat-down",
    "flat-up",
    "slash-quarter-sharp",
    "slash-sharp",
    "slash-flat",
    "double-slash-flat",
    "sharp-1",
    "sharp-2",
    "sharp-3",
    "sharp-5",
    "flat-1",
    "flat-2",
    "flat-3",
    "flat-4",
    "sori",
    "koron",
];

/// music21's `accidentalToMx`.
fn accidental_element(accidental: &Accidental, tag: &'static str) -> Result<Element> {
    let name = match accidental.name() {
        "half-sharp" => "quarter-sharp",
        "one-and-a-half-sharp" => "three-quarters-sharp",
        "half-flat" => "quarter-flat",
        "one-and-a-half-flat" => "three-quarters-flat",
        "double-flat" => "flat-flat",
        other if KNOWN_ACCIDENTALS.contains(&other) => other,
        _ => "other",
    };
    let mut element = Element::with_text(tag, name);
    let style = accidental.display_style();
    if matches!(style, "parentheses" | "both") {
        element.set("parentheses", "yes");
    }
    if matches!(style, "bracket" | "both") {
        element.set("bracket", "yes");
    }
    if let Some(color) = accidental.color() {
        element.set("color", normalize_color(color)?);
    }
    Ok(element)
}

/// music21's `tieToXmlTie`.
fn tie_elements(tie: &Tie) -> Vec<Element> {
    tie_like("tie", tie)
}

fn tie_like(tag: &'static str, tie: &Tie) -> Vec<Element> {
    let continues = tie.tie_type() == crate::notation::TieType::Continue;
    let mut first = Element::new(tag);
    first.set(
        "type",
        if continues {
            "stop"
        } else {
            tie.tie_type().as_str()
        },
    );
    let mut out = vec![first];
    if continues {
        let mut second = Element::new(tag);
        second.set("type", "start");
        out.push(second);
    }
    out
}

/// music21's `tieToXmlTied`.
fn tied_elements(tie: &Tie) -> Vec<Element> {
    use crate::notation::{TieStyle, TieType};
    if tie.style() == TieStyle::Hidden {
        return Vec::new();
    }
    let mut out = tie_like("tied", tie);
    let last = out.last_mut().expect("a tie writes at least one element");
    if tie.style() != TieStyle::Normal && tie.tie_type() != TieType::Stop {
        last.set("line-type", tie.style().as_str());
    }
    if let Some(placement) = tie.placement() {
        last.set("placement", placement.as_str());
        last.set(
            "orientation",
            match placement {
                Placement::Above => "over",
                Placement::Below => "under",
            },
        );
    }
    out
}

/// music21's `dealWithNotehead` and `noteheadToXml`.
fn notehead_for(note: &Note, chord: Option<&Chord>) -> Result<Option<Element>> {
    let marked =
        |notehead: Notehead, parenthesis: bool, fill: Option<bool>, color: Option<&str>| {
            notehead != Notehead::Normal || parenthesis || fill.is_some() || color.is_some()
        };
    let (notehead, parenthesis, fill, color) = if marked(
        note.notehead(),
        note.notehead_parenthesis(),
        note.notehead_fill(),
        note.color(),
    ) {
        (
            note.notehead(),
            note.notehead_parenthesis(),
            note.notehead_fill(),
            note.color(),
        )
    } else if let Some(chord) = chord
        && marked(
            chord.notehead(),
            chord.notehead_parenthesis(),
            chord.notehead_fill(),
            chord.color(),
        )
    {
        (
            chord.notehead(),
            chord.notehead_parenthesis(),
            chord.notehead_fill(),
            chord.color(),
        )
    } else {
        return Ok(None);
    };
    let mut element = Element::with_text("notehead", notehead.as_str());
    if let Some(fill) = fill {
        element.set("filled", yes_no(fill));
    }
    element.set("parentheses", yes_no(parenthesis));
    if let Some(color) = color {
        element.set("color", normalize_color(color)?);
    }
    Ok(Some(element))
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// music21's `beamsToXml`.
fn beam_elements(beams: &Beams) -> Result<Vec<Element>> {
    beams.beams().iter().map(beam_element).collect()
}

/// music21's `beamToXml`.
fn beam_element(beam: &Beam) -> Result<Element> {
    let text = match beam.beam_type() {
        Some(BeamType::Start) => "begin",
        Some(BeamType::Continue) => "continue",
        Some(BeamType::Stop) => "end",
        Some(BeamType::PartialBeam) => match beam.direction() {
            Some(BeamDirection::Left) => "backward hook",
            Some(BeamDirection::Right) => "forward hook",
            None => {
                return Err(export_error(
                    "partial beam defined without a proper direction set (set to None)",
                ));
            }
        },
        None => {
            return Err(export_error("unexpected beam type encountered (None)"));
        }
    };
    let mut element = Element::with_text("beam", text);
    element.set(
        "number",
        beam.number()
            .map_or_else(|| "None".to_string(), |number| number.to_string()),
    );
    Ok(element)
}

/// music21's `lyricToXml`.
fn lyric_element(lyric: &Lyric) -> Result<Element> {
    let mut element = Element::new("lyric");
    let write_syllable = |element: &mut Element, lyric: &Lyric| {
        if let Some(syllabic) = lyric.explicit_syllabic() {
            element.sub_text("syllabic", syllabic.as_str());
        }
        element.sub_text("text", lyric.explicit_text().unwrap_or_default());
    };
    if lyric.is_composite() {
        for (index, component) in lyric.components().iter().enumerate() {
            if component.is_composite() {
                continue;
            }
            if index >= 1 {
                let elision = element.sub("elision");
                if !component.elision_before().is_empty() {
                    elision.set_text(component.elision_before());
                }
            }
            write_syllable(&mut element, component);
        }
    } else {
        write_syllable(&mut element, lyric);
    }
    element.set("name", lyric.identifier());
    element.set("number", lyric.number().to_string());
    if let Some(justify) = lyric.justify() {
        element.set("justify", justify.as_str());
    }
    if let Some(placement) = lyric.placement() {
        element.set("placement", placement.as_str());
    }
    if lyric.is_hidden() {
        element.set("print-object", "no");
    }
    if let Some(color) = lyric.color() {
        element.set("color", normalize_color(color)?);
    }
    Ok(element)
}

/// The `<key>` for a key or a key signature, and nothing for anything else.
fn key_element(element: &StreamElement) -> Option<Result<Element>> {
    match element {
        StreamElement::KeySignature(signature) => Some(key_signature_element(signature, None)),
        StreamElement::Key(key) => {
            let mut signature = key.key_signature();
            signature.set_color(key.color().map(str::to_string));
            Some(key_signature_element(&signature, Some(key.mode())))
        }
        _ => None,
    }
}

/// music21's `keySignatureToXml`, with the mode a `Key` carries.
fn key_signature_element(key: &KeySignature, mode: Option<&str>) -> Result<Element> {
    let mut element = Element::new("key");
    if let Some(color) = key.color() {
        element.set("color", normalize_color(color)?);
    }
    let altered = key.altered_pitches()?;
    match key.sharps().filter(|_| !key.is_non_traditional()) {
        Some(sharps) => {
            element.sub_text("fifths", sharps.to_string());
            if let Some(mode) = mode {
                element.sub_text("mode", mode);
            }
        }
        None => {
            for pitch in &altered {
                element.sub_text("key-step", pitch.step().as_char().to_string());
                let alter = pitch.written_accidental().map_or(0.0, Accidental::alter);
                element.sub_text("key-alter", num_to_int_or_float(alter));
            }
        }
    }
    for (index, pitch) in altered.iter().enumerate() {
        if let Some(octave) = pitch.octave() {
            let mut octave_element = Element::with_text("key-octave", octave.to_string());
            octave_element.set("number", (index + 1).to_string());
            element.push(octave_element);
        }
    }
    Ok(element)
}

/// music21's `timeSignatureToXml`.
fn time_signature_element(meter: &TimeSignature) -> Result<Element> {
    let mut element = Element::new("time");
    if let Some(color) = meter.color() {
        element.set("color", normalize_color(color)?);
    }
    let parts: Vec<(String, String)> = meter
        .display_sequence()
        .flattened()
        .iter()
        .map(|part| (part.numerator().to_string(), part.denominator().to_string()))
        .collect();
    let parts = if meter.summed_numerator() {
        fraction_to_slash_mixed(parts)
    } else {
        parts
    };
    for (beats, beat_type) in parts {
        element.sub_text("beats", beats);
        element.sub_text("beat-type", beat_type);
    }
    if meter.symbolize_denominator() {
        element.set("symbol", "note");
    } else if let Some(symbol) = meter.symbol() {
        element.set("symbol", symbol);
    }
    if meter.is_hidden() {
        element.set("print-object", "no");
    }
    Ok(element)
}

/// music21's `meter.tools.fractionToSlashMixed`: neighbouring parts over one
/// denominator written as one, their numerators summed with `+`.
fn fraction_to_slash_mixed(parts: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (numerator, denominator) in parts {
        match out.last_mut() {
            Some((numerators, last)) if *last == denominator => {
                numerators.push('+');
                numerators.push_str(&numerator);
            }
            _ => out.push((numerator, denominator)),
        }
    }
    out
}

/// music21's `clefToXml`.
fn clef_element(clef: &Clef) -> Result<Element> {
    let mut element = Element::new("clef");
    if let Some(color) = clef.color() {
        element.set("color", normalize_color(color)?);
    }
    element.sub_text("sign", clef.sign().unwrap_or("G"));
    if let Some(line) = clef.line() {
        element.sub_text("line", line.to_string());
    }
    if clef.octave_change() != 0 {
        element.sub_text("clef-octave-change", clef.octave_change().to_string());
    }
    Ok(element)
}

/// music21's `intervalToXmlTranspose`.
fn transpose_element(interval: &Interval) -> Element {
    let generic = interval.generic().directed();
    let steps = generic.abs() - 1;
    let (mut octave_shift, mut diatonic) = (steps / 7, steps % 7);
    let mut chromatic = (interval.semitones().abs() as i64) % 12;
    if generic < 0 {
        diatonic = -diatonic;
        octave_shift = -octave_shift;
        chromatic = -chromatic;
    }
    let mut element = Element::new("transpose");
    element.sub_text("diatonic", diatonic.to_string());
    element.sub_text("chromatic", chromatic.to_string());
    if octave_shift != 0 {
        element.sub_text("octave-change", octave_shift.to_string());
    }
    element
}

/// music21's `xmlObjects.DYNAMIC_MARKS`.
const DYNAMIC_MARKS: [&str; 27] = [
    "p",
    "pp",
    "ppp",
    "pppp",
    "ppppp",
    "pppppp",
    "f",
    "ff",
    "fff",
    "ffff",
    "fffff",
    "ffffff",
    "mp",
    "mf",
    "sf",
    "sfp",
    "sfpp",
    "fp",
    "rf",
    "rfz",
    "sfz",
    "sffz",
    "fz",
    "n",
    "pf",
    "sfzp",
    "other-dynamics",
];

/// The tag a dynamic mark is written as, which is the mark itself.
fn dynamic_tag(mark: &str) -> &'static str {
    DYNAMIC_MARKS
        .iter()
        .find(|known| **known == mark)
        .copied()
        .unwrap_or("other-dynamics")
}

/// Whether music21 holds this offset as a `Fraction` rather than a float:
/// `opFrac` keeps a float only where its denominator is a power of two
/// below 65535.
fn is_fraction_offset(offset: FloatType) -> bool {
    let scaled = offset * 32768.0;
    scaled.fract() != 0.0
}

/// music21's `normalizeColor`: a hex colour upper-cased, and a CSS colour
/// name written as its hex value.
fn normalize_color(color: &str) -> Result<String> {
    if color.is_empty() {
        return Ok(String::new());
    }
    if color.contains('#') {
        return Ok(color.to_uppercase());
    }
    css_color_hex(color)
        .map(str::to_string)
        .ok_or_else(|| export_error(format!("{color:?} is not a CSS colour name")))
}

/// The hex value of a CSS3 colour name, as `#FF0000` for `red`, whatever
/// case the name is written in; `None` for a name CSS does not have.
///
/// ```
/// use music21_rs::musicxml::css_color_hex;
///
/// assert_eq!(css_color_hex("Red"), Some("#FF0000"));
/// assert_eq!(css_color_hex("grey"), css_color_hex("gray"));
/// assert_eq!(css_color_hex("reddish"), None);
/// ```
pub fn css_color_hex(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    CSS_COLORS
        .binary_search_by(|(known, _)| known.cmp(&name.as_str()))
        .ok()
        .map(|index| CSS_COLORS[index].1)
}

/// The CSS3 colour names and their hex values, sorted by name: the table
/// music21 reads a colour name out of, through `webcolors`.
pub const CSS_COLORS: [(&str, &str); 147] = [
    ("aliceblue", "#F0F8FF"),
    ("antiquewhite", "#FAEBD7"),
    ("aqua", "#00FFFF"),
    ("aquamarine", "#7FFFD4"),
    ("azure", "#F0FFFF"),
    ("beige", "#F5F5DC"),
    ("bisque", "#FFE4C4"),
    ("black", "#000000"),
    ("blanchedalmond", "#FFEBCD"),
    ("blue", "#0000FF"),
    ("blueviolet", "#8A2BE2"),
    ("brown", "#A52A2A"),
    ("burlywood", "#DEB887"),
    ("cadetblue", "#5F9EA0"),
    ("chartreuse", "#7FFF00"),
    ("chocolate", "#D2691E"),
    ("coral", "#FF7F50"),
    ("cornflowerblue", "#6495ED"),
    ("cornsilk", "#FFF8DC"),
    ("crimson", "#DC143C"),
    ("cyan", "#00FFFF"),
    ("darkblue", "#00008B"),
    ("darkcyan", "#008B8B"),
    ("darkgoldenrod", "#B8860B"),
    ("darkgray", "#A9A9A9"),
    ("darkgreen", "#006400"),
    ("darkgrey", "#A9A9A9"),
    ("darkkhaki", "#BDB76B"),
    ("darkmagenta", "#8B008B"),
    ("darkolivegreen", "#556B2F"),
    ("darkorange", "#FF8C00"),
    ("darkorchid", "#9932CC"),
    ("darkred", "#8B0000"),
    ("darksalmon", "#E9967A"),
    ("darkseagreen", "#8FBC8F"),
    ("darkslateblue", "#483D8B"),
    ("darkslategray", "#2F4F4F"),
    ("darkslategrey", "#2F4F4F"),
    ("darkturquoise", "#00CED1"),
    ("darkviolet", "#9400D3"),
    ("deeppink", "#FF1493"),
    ("deepskyblue", "#00BFFF"),
    ("dimgray", "#696969"),
    ("dimgrey", "#696969"),
    ("dodgerblue", "#1E90FF"),
    ("firebrick", "#B22222"),
    ("floralwhite", "#FFFAF0"),
    ("forestgreen", "#228B22"),
    ("fuchsia", "#FF00FF"),
    ("gainsboro", "#DCDCDC"),
    ("ghostwhite", "#F8F8FF"),
    ("gold", "#FFD700"),
    ("goldenrod", "#DAA520"),
    ("gray", "#808080"),
    ("green", "#008000"),
    ("greenyellow", "#ADFF2F"),
    ("grey", "#808080"),
    ("honeydew", "#F0FFF0"),
    ("hotpink", "#FF69B4"),
    ("indianred", "#CD5C5C"),
    ("indigo", "#4B0082"),
    ("ivory", "#FFFFF0"),
    ("khaki", "#F0E68C"),
    ("lavender", "#E6E6FA"),
    ("lavenderblush", "#FFF0F5"),
    ("lawngreen", "#7CFC00"),
    ("lemonchiffon", "#FFFACD"),
    ("lightblue", "#ADD8E6"),
    ("lightcoral", "#F08080"),
    ("lightcyan", "#E0FFFF"),
    ("lightgoldenrodyellow", "#FAFAD2"),
    ("lightgray", "#D3D3D3"),
    ("lightgreen", "#90EE90"),
    ("lightgrey", "#D3D3D3"),
    ("lightpink", "#FFB6C1"),
    ("lightsalmon", "#FFA07A"),
    ("lightseagreen", "#20B2AA"),
    ("lightskyblue", "#87CEFA"),
    ("lightslategray", "#778899"),
    ("lightslategrey", "#778899"),
    ("lightsteelblue", "#B0C4DE"),
    ("lightyellow", "#FFFFE0"),
    ("lime", "#00FF00"),
    ("limegreen", "#32CD32"),
    ("linen", "#FAF0E6"),
    ("magenta", "#FF00FF"),
    ("maroon", "#800000"),
    ("mediumaquamarine", "#66CDAA"),
    ("mediumblue", "#0000CD"),
    ("mediumorchid", "#BA55D3"),
    ("mediumpurple", "#9370DB"),
    ("mediumseagreen", "#3CB371"),
    ("mediumslateblue", "#7B68EE"),
    ("mediumspringgreen", "#00FA9A"),
    ("mediumturquoise", "#48D1CC"),
    ("mediumvioletred", "#C71585"),
    ("midnightblue", "#191970"),
    ("mintcream", "#F5FFFA"),
    ("mistyrose", "#FFE4E1"),
    ("moccasin", "#FFE4B5"),
    ("navajowhite", "#FFDEAD"),
    ("navy", "#000080"),
    ("oldlace", "#FDF5E6"),
    ("olive", "#808000"),
    ("olivedrab", "#6B8E23"),
    ("orange", "#FFA500"),
    ("orangered", "#FF4500"),
    ("orchid", "#DA70D6"),
    ("palegoldenrod", "#EEE8AA"),
    ("palegreen", "#98FB98"),
    ("paleturquoise", "#AFEEEE"),
    ("palevioletred", "#DB7093"),
    ("papayawhip", "#FFEFD5"),
    ("peachpuff", "#FFDAB9"),
    ("peru", "#CD853F"),
    ("pink", "#FFC0CB"),
    ("plum", "#DDA0DD"),
    ("powderblue", "#B0E0E6"),
    ("purple", "#800080"),
    ("red", "#FF0000"),
    ("rosybrown", "#BC8F8F"),
    ("royalblue", "#4169E1"),
    ("saddlebrown", "#8B4513"),
    ("salmon", "#FA8072"),
    ("sandybrown", "#F4A460"),
    ("seagreen", "#2E8B57"),
    ("seashell", "#FFF5EE"),
    ("sienna", "#A0522D"),
    ("silver", "#C0C0C0"),
    ("skyblue", "#87CEEB"),
    ("slateblue", "#6A5ACD"),
    ("slategray", "#708090"),
    ("slategrey", "#708090"),
    ("snow", "#FFFAFA"),
    ("springgreen", "#00FF7F"),
    ("steelblue", "#4682B4"),
    ("tan", "#D2B48C"),
    ("teal", "#008080"),
    ("thistle", "#D8BFD8"),
    ("tomato", "#FF6347"),
    ("turquoise", "#40E0D0"),
    ("violet", "#EE82EE"),
    ("wheat", "#F5DEB3"),
    ("white", "#FFFFFF"),
    ("whitesmoke", "#F5F5F5"),
    ("yellow", "#FFFF00"),
    ("yellowgreen", "#9ACD32"),
];

#[cfg(test)]
mod tests {
    //! Each expectation is the one music21's own docstring for the method
    //! prints, read off `m21ToXml.py`.

    use super::*;
    use crate::notation::{BeamType, Syllabic, TieStyle, TieType};
    use crate::volume::Volume;

    static NO_BASES: std::sync::LazyLock<std::collections::HashMap<usize, u32>> =
        std::sync::LazyLock::new(std::collections::HashMap::new);

    fn with_measure<T>(run: impl FnOnce(&mut MeasureExporter<'_, '_>) -> T) -> T {
        let stream = Stream::with_kind(StreamKind::Measure);
        let mut context = PartContext {
            instruments: &[],
            own_instruments: Vec::new(),
            own_clefs: Vec::new(),
            meter: None,
            last_divisions: None,
            spanners: Vec::new(),
            clef: None,
            previous: None,
            voice_bases: &NO_BASES,
            modulations: Vec::new(),
        };
        let mut exporter = MeasureExporter::new(&stream, 0.0, &mut context);
        run(&mut exporter)
    }

    #[test]
    fn a_segno_and_a_coda_are_signs_and_the_rest_are_words() {
        // music21's `segnoToXml` and `codaToXml` docstrings.
        let sign = |kind| {
            with_measure(|exporter| {
                exporter
                    .repeat_expression_element(&RepeatExpression::new(kind), 0.0)
                    .dump()
            })
        };
        assert_eq!(
            sign(RepeatExpressionKind::Segno),
            "<direction>
  <direction-type>
    <segno default-y=\"20\" />
  </direction-type>
</direction>"
        );
        assert_eq!(
            sign(RepeatExpressionKind::Coda),
            "<direction>
  <direction-type>
    <coda default-y=\"20\" />
  </direction-type>
</direction>"
        );
        assert_eq!(
            sign(RepeatExpressionKind::Fine),
            "<direction>
  <direction-type>
    <words justify=\"right\">fine</words>
  </direction-type>
</direction>"
        );
    }

    #[test]
    fn a_pedal_goes_down_and_comes_up() {
        use crate::spanner::Pedal;
        // music21's `_spannerStartParameters` and `_spannerEndParameters`.
        let pedal = |pedal_type, form| {
            Spanner::pedal_mark(
                Pedal {
                    pedal_type,
                    form,
                    abbreviated: false,
                },
                vec![0, 1],
            )
        };
        let symbol = pedal(Some(PedalType::Sustain), Some(PedalForm::Symbol));
        assert_eq!(
            pedal_element(&symbol, 1, true).dump(),
            "<pedal number=\"1\" sign=\"yes\" type=\"start\" />"
        );
        assert_eq!(
            pedal_element(&symbol, 1, false).dump(),
            "<pedal number=\"1\" sign=\"yes\" type=\"stop\" />"
        );
        let line = pedal(Some(PedalType::Sostenuto), Some(PedalForm::Line));
        assert_eq!(
            pedal_element(&line, 2, true).dump(),
            "<pedal line=\"yes\" number=\"2\" type=\"sostenuto\" />"
        );
        assert_eq!(
            pedal_element(&line, 2, false).dump(),
            "<pedal line=\"yes\" number=\"2\" type=\"stop\" />"
        );
    }

    #[test]
    fn glissandi_a_tremolo_and_a_pedal_with_a_bounce_are_written_as_music21_writes_them() {
        use crate::spanner::{Glissando, Pedal};
        // Read off music21's own export of the same two bars.
        let bar = |number, names: [&str; 4]| {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_number(number);
            for name in names {
                measure.push(Note::from_name(name).unwrap());
            }
            measure
        };
        let mut first = bar(1, ["C4", "D4", "E4", "F4"]);
        first.insert(3.0, PedalObject::new(PedalObjectKind::Bounce));
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(first);
        part.push(bar(2, ["G4", "A4", "B4", "C5"]));
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part);
        // The notes of the first bar stand at 0 to 3, the bounce at 4, and
        // the notes of the second at 5 to 8.
        let kinds: Vec<bool> = score
            .leaves()
            .iter()
            .map(|(_, element)| matches!(element, StreamElement::PedalObject(_)))
            .collect();
        assert_eq!(kinds.iter().position(|bounce| *bounce), Some(4));

        // A pedal from the second note to the seventh, its bounce joined
        // last, so that is where it comes up.
        let pedal = Spanner::pedal_mark(
            Pedal {
                pedal_type: Some(PedalType::Sustain),
                form: Some(PedalForm::SymbolLine),
                abbreviated: false,
            },
            vec![1, 7, 4],
        );
        score.add_spanner(pedal);
        score.add_spanner(Spanner::glissando(vec![2, 3]));
        let mut slide = Spanner::glissando(vec![5, 6]);
        slide.set_glissando_details(Some(Glissando {
            slide_type: SlideType::Continuous,
            label: Some("gl.".to_string()),
        }));
        score.add_spanner(slide);
        let mut tremolo = Spanner::tremolo(3, vec![7, 8]).unwrap();
        tremolo.set_placement(Some(Placement::Above));
        score.add_spanner(tremolo);

        let written = to_musicxml(&score, &ExportOptions::default()).unwrap();
        let direction = |pedal: &str, offset: &str| {
            format!(
                "      <direction>\n        <direction-type>\n          {pedal}\n        \
                 </direction-type>\n{offset}      </direction>\n"
            )
        };
        // The sign, and the line resumed where the pedal mark stands: the
        // start of the part, a quarter before the note it goes down on.
        assert!(written.contains(&format!(
            "{}{}",
            direction("<pedal number=\"1\" sign=\"yes\" type=\"start\" />", ""),
            direction(
                "<pedal line=\"yes\" type=\"resume\" />",
                "        <offset sound=\"yes\">-10080</offset>\n"
            )
        )));
        // The bounce, and after it the pedal coming up.
        assert!(written.contains(&format!(
            "{}{}",
            direction("<pedal line=\"yes\" type=\"change\" />", ""),
            direction("<pedal line=\"yes\" number=\"1\" type=\"stop\" />", "")
        )));
        for notation in [
            "<glissando line-type=\"wavy\" number=\"1\" type=\"start\" />",
            "<glissando line-type=\"wavy\" number=\"1\" type=\"stop\" />",
            "<slide line-type=\"wavy\" number=\"2\" type=\"start\">gl.</slide>",
            "<slide line-type=\"wavy\" number=\"2\" type=\"stop\" />",
            "<tremolo placement=\"above\" type=\"start\">3</tremolo>",
            "<tremolo type=\"stop\">3</tremolo>",
        ] {
            assert!(written.contains(notation), "{notation} in\n{written}");
        }
    }

    #[test]
    fn a_bounce_is_written_as_its_pedal_is_drawn() {
        use crate::spanner::Pedal;
        // music21's `pedalObjectToXml`.
        let written = |form, pedal_type, kind| {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_number(1);
            measure.push(Note::from_name("C4").unwrap());
            measure.insert(1.0, PedalObject::new(kind));
            measure.push(Note::from_name("D4").unwrap());
            let mut part = Stream::with_kind(StreamKind::Part);
            part.push(measure);
            part.add_spanner(Spanner::pedal_mark(
                Pedal {
                    pedal_type,
                    form: Some(form),
                    abbreviated: false,
                },
                vec![0, 1, 2],
            ));
            let mut score = Stream::with_kind(StreamKind::Score);
            score.push(part);
            let document = to_musicxml(&score, &ExportOptions::default()).unwrap();
            document
                .lines()
                .map(str::trim)
                .filter(|line| line.starts_with("<pedal") && !line.contains("number"))
                .collect::<Vec<_>>()
                .join("")
        };
        let sustain = Some(PedalType::Sustain);
        assert_eq!(
            written(PedalForm::Line, sustain, PedalObjectKind::Bounce),
            "<pedal line=\"yes\" type=\"change\" />"
        );
        assert_eq!(
            written(PedalForm::Symbol, sustain, PedalObjectKind::Bounce),
            "<pedal sign=\"yes\" type=\"stop\" /><pedal sign=\"yes\" type=\"start\" />"
        );
        assert_eq!(
            written(
                PedalForm::SymbolAlt,
                Some(PedalType::Sostenuto),
                PedalObjectKind::Bounce
            ),
            "<pedal sign=\"yes\" type=\"sostenuto\" />"
        );
        assert_eq!(
            written(
                PedalForm::Symbol,
                Some(PedalType::Soft),
                PedalObjectKind::Bounce
            ),
            "<pedal sign=\"yes\" type=\"stop\" /><pedal sign=\"yes\" type=\"sustain\" />"
        );
        assert_eq!(
            written(PedalForm::Line, sustain, PedalObjectKind::GapStart),
            "<pedal line=\"yes\" type=\"discontinue\" />"
        );
        assert_eq!(
            written(PedalForm::Symbol, sustain, PedalObjectKind::GapEnd),
            "<pedal sign=\"yes\" type=\"resume\" />"
        );
    }

    #[test]
    fn a_mark_at_a_triplet_offset_always_says_its_offset() {
        // music21 holds 8/3 as a Fraction, which equals no float, so the
        // offset is written even where it comes to nothing.
        with_measure(|exporter| {
            exporter.offset_in_measure = 8.0 / 3.0;
            let written = exporter.offset_element(8.0 / 3.0, true).unwrap();
            assert_eq!(written.dump(), "<offset sound=\"yes\">0</offset>");
            exporter.offset_in_measure = 2.5;
            assert!(exporter.offset_element(2.5, true).is_none());
        });
    }

    #[test]
    fn a_stroke_is_written_where_it_is_displayed() {
        // music21's `unpitchedToXml` docstring.
        let stroke = crate::percussion::Unpitched::from_display_name("D5")
            .unwrap()
            .with_duration(Duration::quarter());
        let written = with_measure(|exporter| {
            exporter
                .written_note_element(stroke.written(), 0, None, None, true)
                .unwrap()
        });
        assert_eq!(
            written.dump(),
            [
                "<note>",
                "  <unpitched>",
                "    <display-step>D</display-step>",
                "    <display-octave>5</display-octave>",
                "  </unpitched>",
                "  <duration>10080</duration>",
                "  <type>quarter</type>",
                "</note>",
            ]
            .join(
                "
"
            )
        );
    }

    #[test]
    fn a_colour_is_written_in_hex() {
        assert_eq!(normalize_color("").unwrap(), "");
        assert_eq!(normalize_color("red").unwrap(), "#FF0000");
        assert_eq!(normalize_color("#00ff00").unwrap(), "#00FF00");
        assert!(normalize_color("reddish").is_err());
    }

    #[test]
    fn a_note_writes_as_music21_writes_it() {
        let mut note = Note::from_name("D#5")
            .unwrap()
            .with_duration(Duration::new(3.0).unwrap());
        note.set_volume(Some(Volume::from_velocity_scalar(0.5).unwrap()));
        note.set_color(Some("#C0C0C0".to_string()));
        let xml =
            with_measure(|exporter| exporter.note_element(&note, 0, None, None).unwrap()).dump();
        assert_eq!(
            xml,
            "<note color=\"#C0C0C0\" dynamics=\"70.56\">
  <pitch>
    <step>D</step>
    <alter>1</alter>
    <octave>5</octave>
  </pitch>
  <duration>30240</duration>
  <type>half</type>
  <dot />
  <accidental>sharp</accidental>
  <notehead color=\"#C0C0C0\" parentheses=\"no\">normal</notehead>
</note>"
        );
    }

    #[test]
    fn a_chord_writes_a_note_for_each_of_its_notes() {
        let chord = Chord::new("A-2 D3 E4")
            .unwrap()
            .with_duration(Duration::half());
        let root = with_measure(|exporter| {
            exporter.chord_elements(&chord, None).unwrap();
            exporter.root.clone()
        });
        assert_eq!(
            root.dump(),
            "<measure>
  <note>
    <pitch>
      <step>A</step>
      <alter>-1</alter>
      <octave>2</octave>
    </pitch>
    <duration>20160</duration>
    <type>half</type>
    <accidental>flat</accidental>
  </note>
  <note>
    <chord />
    <pitch>
      <step>D</step>
      <octave>3</octave>
    </pitch>
    <duration>20160</duration>
    <type>half</type>
  </note>
  <note>
    <chord />
    <pitch>
      <step>E</step>
      <octave>4</octave>
    </pitch>
    <duration>20160</duration>
    <type>half</type>
  </note>
</measure>"
        );
    }

    #[test]
    fn a_rest_filling_its_bar_is_a_measure_rest() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, TimeSignature::new(4, 4).unwrap());
        let rest = Rest::new(Duration::half());
        let mut context = PartContext {
            instruments: &[],
            own_instruments: Vec::new(),
            own_clefs: Vec::new(),
            meter: None,
            last_divisions: None,
            spanners: Vec::new(),
            clef: None,
            previous: None,
            voice_bases: &NO_BASES,
            modulations: Vec::new(),
        };
        let exporter = MeasureExporter::new(&measure, 0.0, &mut context);
        assert_eq!(
            exporter.rest_element(&rest, 0.0, None).unwrap().dump(),
            "<note>\n  <rest />\n  <duration>20160</duration>\n  <type>half</type>\n</note>"
        );

        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, TimeSignature::new(2, 4).unwrap());
        let exporter = MeasureExporter::new(&measure, 0.0, &mut context);
        assert_eq!(
            exporter.rest_element(&rest, 0.0, None).unwrap().dump(),
            "<note>\n  <rest measure=\"yes\" />\n  <duration>20160</duration>\n</note>"
        );
    }

    #[test]
    fn a_triplet_carries_its_time_modification() {
        let rest = Rest::new(Duration::new(1.0 / 3.0).unwrap());
        let xml = with_measure(|exporter| exporter.rest_element(&rest, 0.0, None).unwrap()).dump();
        assert_eq!(
            xml,
            "<note>
  <rest />
  <duration>3360</duration>
  <type>eighth</type>
  <time-modification>
    <actual-notes>3</actual-notes>
    <normal-notes>2</normal-notes>
    <normal-type>eighth</normal-type>
  </time-modification>
</note>"
        );
    }

    #[test]
    fn a_duration_needing_a_tie_is_refused() {
        let note = Note::from_name("C4")
            .unwrap()
            .with_duration(Duration::new(5.0).unwrap());
        let error =
            with_measure(|exporter| exporter.note_element(&note, 0, None, None).unwrap_err());
        assert_eq!(
            error,
            Error::MusicXml(
                "Cannot convert complex durations to MusicXML. Try exporting with \
                 makeNotation=True or manually running splitAtDurations()"
                    .to_string()
            )
        );
    }

    #[test]
    fn moving_forward_and_back_counts_divisions() {
        let root = with_measure(|exporter| {
            exporter.move_forward(1.0);
            exporter.move_backward(2.0);
            exporter.root.clone()
        });
        assert_eq!(
            root.dump(),
            "<measure>
  <forward>
    <duration>10080</duration>
  </forward>
  <backup>
    <duration>20160</duration>
  </backup>
</measure>"
        );
    }

    #[test]
    fn ties_are_written_twice_over() {
        let tie = Tie::new(TieType::Continue);
        let written: Vec<String> = tie_elements(&tie).iter().map(Element::dump).collect();
        assert_eq!(written, ["<tie type=\"stop\" />", "<tie type=\"start\" />"]);
        let tied: Vec<String> = tied_elements(&tie).iter().map(Element::dump).collect();
        assert_eq!(tied, ["<tied type=\"stop\" />", "<tied type=\"start\" />"]);
        let mut hidden = Tie::new(TieType::Continue);
        hidden.set_style(TieStyle::Hidden);
        assert!(tied_elements(&hidden).is_empty());
    }

    #[test]
    fn beams_name_their_ends() {
        let mut beam = Beam::new(BeamType::Start, None);
        beam.set_number(Some(1));
        assert_eq!(
            beam_element(&beam).unwrap().dump(),
            "<beam number=\"1\">begin</beam>"
        );
        beam.set_beam_type(Some(BeamType::Stop));
        assert_eq!(
            beam_element(&beam).unwrap().dump(),
            "<beam number=\"1\">end</beam>"
        );
        beam.set_beam_type(Some(BeamType::PartialBeam));
        beam.set_direction(Some(BeamDirection::Left));
        assert_eq!(
            beam_element(&beam).unwrap().dump(),
            "<beam number=\"1\">backward hook</beam>"
        );
        beam.set_direction(None);
        assert_eq!(
            beam_element(&beam).unwrap_err(),
            Error::MusicXml(
                "partial beam defined without a proper direction set (set to None)".to_string()
            )
        );
    }

    #[test]
    fn key_signatures_traditional_and_not() {
        assert_eq!(
            key_signature_element(&KeySignature::new(-3), None)
                .unwrap()
                .dump(),
            "<key>\n  <fifths>-3</fifths>\n</key>"
        );
        let pitches = vec![
            Pitch::from_name("C#").unwrap(),
            Pitch::from_name("E-4").unwrap(),
        ];
        assert_eq!(
            key_signature_element(&KeySignature::from_altered_pitches(pitches), None)
                .unwrap()
                .dump(),
            "<key>
  <key-step>C</key-step>
  <key-alter>1</key-alter>
  <key-step>E</key-step>
  <key-alter>-1</key-alter>
  <key-octave number=\"2\">4</key-octave>
</key>"
        );
    }

    #[test]
    fn a_meter_written_in_parts_writes_each() {
        assert_eq!(
            time_signature_element(&TimeSignature::new(3, 4).unwrap())
                .unwrap()
                .dump(),
            "<time>\n  <beats>3</beats>\n  <beat-type>4</beat-type>\n</time>"
        );
        assert_eq!(
            time_signature_element(&TimeSignature::from_ratio_string("3/4+2/4").unwrap())
                .unwrap()
                .dump(),
            "<time>
  <beats>3</beats>
  <beat-type>4</beat-type>
  <beats>2</beats>
  <beat-type>4</beat-type>
</time>"
        );
    }

    #[test]
    fn clefs_write_their_sign_line_and_octave() {
        assert_eq!(
            clef_element(&Clef::of_kind(crate::clef::ClefKind::GClef))
                .unwrap()
                .dump(),
            "<clef>\n  <sign>G</sign>\n</clef>"
        );
        assert_eq!(
            clef_element(&Clef::of_kind(crate::clef::ClefKind::Treble8vbClef))
                .unwrap()
                .dump(),
            "<clef>
  <sign>G</sign>
  <line>2</line>
  <clef-octave-change>-1</clef-octave-change>
</clef>"
        );
        assert_eq!(
            clef_element(&Clef::of_kind(crate::clef::ClefKind::PercussionClef))
                .unwrap()
                .dump(),
            "<clef>\n  <sign>percussion</sign>\n</clef>"
        );
    }

    #[test]
    fn a_transposition_is_steps_semitones_and_octaves() {
        let dump = |name: &str| transpose_element(&Interval::from_name(name).unwrap()).dump();
        assert_eq!(
            dump("P5"),
            "<transpose>\n  <diatonic>4</diatonic>\n  <chromatic>7</chromatic>\n</transpose>"
        );
        assert_eq!(
            dump("A13"),
            "<transpose>
  <diatonic>5</diatonic>
  <chromatic>10</chromatic>
  <octave-change>1</octave-change>
</transpose>"
        );
        assert_eq!(
            dump("-M9"),
            "<transpose>
  <diatonic>-1</diatonic>
  <chromatic>-2</chromatic>
  <octave-change>-1</octave-change>
</transpose>"
        );
    }

    #[test]
    fn a_dynamic_is_a_direction_with_its_loudness() {
        let root = with_measure(|exporter| {
            exporter.dynamic_element(&Dynamic::new("ppp"), 1.0);
            exporter.root.clone()
        });
        assert_eq!(
            root.dump(),
            "<measure>
  <direction>
    <direction-type>
      <dynamics default-x=\"-36\" default-y=\"-80\">
        <ppp />
      </dynamics>
    </direction-type>
    <offset sound=\"yes\">10080</offset>
    <sound dynamics=\"19\" />
  </direction>
</measure>"
        );
    }

    #[test]
    fn a_metronome_mark_writes_its_beat_and_its_word() {
        let mark =
            MetronomeMark::with_number_and_text(40.0, "slow").with_referent(Duration::half());
        let root = with_measure(|exporter| {
            exporter.tempo_elements(&mark, 0.0).unwrap();
            exporter.root.clone()
        });
        assert_eq!(
            root.dump(),
            "<measure>
  <direction>
    <direction-type>
      <metronome parentheses=\"no\">
        <beat-unit>half</beat-unit>
        <per-minute>40</per-minute>
      </metronome>
    </direction-type>
    <sound tempo=\"80\" />
  </direction>
  <direction>
    <direction-type>
      <words default-y=\"45\" font-weight=\"bold\">slow</words>
    </direction-type>
  </direction>
</measure>"
        );

        let sounding = MetronomeMark::default().with_number_sounding(60.0);
        let root = with_measure(|exporter| {
            exporter.tempo_elements(&sounding, 0.0).unwrap();
            exporter.root.clone()
        });
        assert_eq!(
            root.dump(),
            "<measure>
  <direction>
    <direction-type>
      <words />
    </direction-type>
    <sound tempo=\"60\" />
  </direction>
</measure>"
        );
    }

    #[test]
    fn a_chord_symbol_is_a_harmony() {
        let dump = |figure: &str| {
            let symbol = ChordSymbol::parse_music21(figure).unwrap();
            with_measure(|exporter| exporter.harmony_element(&symbol, 0.0).unwrap()).dump()
        };
        // MusicXML calls a dominant seventh `dominant`.
        assert_eq!(
            dump("C7"),
            "<harmony>
  <root>
    <root-step>C</root-step>
  </root>
  <kind>dominant</kind>
</harmony>"
        );
        assert_eq!(
            dump("F sus add 9"),
            "<harmony>
  <root>
    <root-step>F</root-step>
  </root>
  <kind>suspended-fourth</kind>
  <degree>
    <degree-value>9</degree-value>
    <degree-alter>0</degree-alter>
    <degree-type>add</degree-type>
  </degree>
</harmony>"
        );
        let no_chord = ChordSymbol::no_chord(None);
        assert_eq!(
            with_measure(|exporter| exporter.harmony_element(&no_chord, 0.0).unwrap()).dump(),
            "<harmony>
  <root>
    <root-step text=\"\">C</root-step>
  </root>
  <kind text=\"N.C.\">none</kind>
</harmony>"
        );
    }

    #[test]
    fn a_lyric_names_its_verse() {
        let mut lyric = Lyric::new("hel");
        lyric.set_syllabic(Syllabic::Begin);
        assert_eq!(
            lyric_element(&lyric).unwrap().dump(),
            "<lyric name=\"1\" number=\"1\">
  <syllabic>begin</syllabic>
  <text>hel</text>
</lyric>"
        );
    }

    #[test]
    fn a_whole_score_writes_as_a_music21_fragment() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.set_number(1);
        measure.insert(0.0, TimeSignature::new(1, 4).unwrap());
        measure.insert(0.0, Clef::treble());
        measure.push(Note::from_name("D#4").unwrap());
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(measure);
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part);
        let options = ExportOptions {
            encoding_date: Some("2026-09-28".to_string()),
            software: "music21 v.11.0.0".to_string(),
            ..ExportOptions::default()
        };
        assert_eq!(
            to_musicxml(&score, &options).unwrap(),
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>
<!DOCTYPE score-partwise  PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">
<score-partwise version=\"4.0\">
  <movement-title>Music21 Fragment</movement-title>
  <identification>
    <creator type=\"composer\">Music21</creator>
    <encoding>
      <encoding-date>2026-09-28</encoding-date>
      <software>music21 v.11.0.0</software>
      <supports element=\"beam\" type=\"yes\" />
      <supports element=\"stem\" type=\"yes\" />
      <supports element=\"accidental\" type=\"yes\" />
    </encoding>
  </identification>
  <defaults>
    <scaling>
      <millimeters>7</millimeters>
      <tenths>40</tenths>
    </scaling>
  </defaults>
  <part-list>
    <score-part id=\"P1\">
      <part-name />
    </score-part>
  </part-list>
  <!--=========================== Part 1 ===========================-->
  <part id=\"P1\">
    <!--========================= Measure 1 ==========================-->
    <measure implicit=\"no\" number=\"1\">
      <attributes>
        <divisions>10080</divisions>
        <time>
          <beats>1</beats>
          <beat-type>4</beat-type>
        </time>
        <clef>
          <sign>G</sign>
          <line>2</line>
        </clef>
      </attributes>
      <note>
        <pitch>
          <step>D</step>
          <alter>1</alter>
          <octave>4</octave>
        </pitch>
        <duration>10080</duration>
        <type>quarter</type>
        <accidental>sharp</accidental>
      </note>
    </measure>
  </part>
</score-partwise>"
        );
    }

    #[test]
    fn a_part_that_changes_instrument_says_who_plays_each_note() {
        // music21's `setNoteInstrument`: the last instrument standing at or
        // before a note plays it, unless the note keeps one of its own.
        let measure = |number, instrument: &str, stored: Option<&str>| {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_number(number);
            measure.insert(0.0, Instrument::of_kind(instrument).unwrap());
            let mut note = Note::from_name("C4")
                .unwrap()
                .with_duration(Duration::quarter());
            note.set_stored_instrument(stored.map(|kind| Instrument::of_kind(kind).unwrap()));
            measure.insert(0.0, note);
            measure
        };
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure(1, "Violin", None));
        part.insert(1.0, measure(2, "Viola", None));
        part.insert(2.0, measure(3, "Viola", Some("Violin")));
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part);
        let written = to_musicxml(&score, &ExportOptions::default()).unwrap();
        let ids: Vec<&str> = written
            .lines()
            .filter_map(|line| line.trim().strip_prefix("<instrument id=\""))
            .map(|rest| rest.trim_end_matches("\" />"))
            .collect();
        assert_eq!(ids.len(), 3);
        assert_ne!(ids[0], ids[1]);
        assert_eq!(ids[0], ids[2]);
        for id in &ids {
            assert!(written.contains(&format!("<score-instrument id=\"{id}\">")));
        }
    }

    #[test]
    fn a_part_without_measures_is_refused() {
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(Note::from_name("C4").unwrap());
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part);
        assert_eq!(
            to_musicxml(&score, &ExportOptions::default()).unwrap_err(),
            Error::MusicXml(
                "Cannot export with makeNotation=False if there are no measures".to_string()
            )
        );
    }

    #[test]
    fn divider_comments_are_centred_in_sixty_signs() {
        let mut root = Element::new("score-partwise");
        add_divider_comment(&mut root, "second accidental below");
        assert_eq!(
            root.dump(),
            "<score-partwise>\n  <!--================== second accidental below ===================-->\n</score-partwise>"
        );
    }
}
