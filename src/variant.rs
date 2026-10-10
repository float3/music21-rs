//! Other readings of a passage, kept beside it: music21's `variant` module.
//!
//! A [`Variant`] stands in a stream at the offset of the passage it is
//! another reading of, holding that reading as a stream of its own. It
//! takes no time in the stream holding it. Its replacement length says how
//! much of the stream it stands in for, which may be more than it holds (a
//! deletion), less (an elongation) or the same (a replacement).
//!
//! The functions here merge another version of a stream into it as
//! variants ([`merge_variants`] and those it chooses between), add one
//! ([`add_variant`]), find what one stands in for ([`replaced_elements`]),
//! and make the variants of a group the stream's reading
//! ([`Stream::activate_variants`]). They do as music21's do, the slips
//! included: measures are found by their numbers where music21 finds them
//! by number, and a function music21 fails in fails here too, as each one's
//! documentation says.

use std::collections::{HashMap, VecDeque};

use crate::braille::equality::equal;
use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::makenotation::op_frac;
use crate::rest::Rest;
use crate::search::{difflib, iterated, translate_stream_to_string};
use crate::stream::{Stream, StreamElement, StreamEvent, StreamKind};

/// How a [`Variant`]'s reading compares in length with the passage it
/// stands in for: music21's `lengthType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LengthType {
    /// As long as the passage.
    Replacement,
    /// Longer than it.
    Elongation,
    /// Shorter than it.
    Deletion,
}

impl LengthType {
    /// music21's name for the length type.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replacement => "replacement",
            Self::Elongation => "elongation",
            Self::Deletion => "deletion",
        }
    }
}

/// Another reading of a passage: music21's `Variant`.
///
/// ```
/// use music21_rs::variant::{LengthType, Variant};
/// use music21_rs::{Duration, Note};
///
/// let mut variant = Variant::named("ossia");
/// variant.contents_mut().push(Note::from_name("D4")?.with_duration(Duration::half()));
/// assert_eq!(variant.groups(), ["ossia"]);
/// assert_eq!(variant.replacement_quarter_length(), 2.0);
///
/// variant.set_replacement_quarter_length(Some(4.0));
/// assert_eq!(variant.length_type(), LengthType::Deletion);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Variant {
    contents: Stream,
    groups: Vec<String>,
    replacement_quarter_length: Option<FloatType>,
}

impl Variant {
    /// A variant holding nothing, in no group.
    pub fn new() -> Self {
        Self::default()
    }

    /// A variant holding nothing, in the one group `name`: music21's
    /// `Variant(name=...)`.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            groups: vec![name.into()],
            ..Self::default()
        }
    }

    /// A variant holding a stream's elements where they stand in it.
    pub fn from_stream(contents: Stream) -> Self {
        Self {
            contents,
            ..Self::default()
        }
    }

    /// The reading the variant holds: music21's `containedSite`.
    pub fn contents(&self) -> &Stream {
        &self.contents
    }

    /// The same, to change.
    pub fn contents_mut(&mut self) -> &mut Stream {
        &mut self.contents
    }

    /// The names of the readings the variant belongs to: music21's
    /// `groups`.
    pub fn groups(&self) -> &[String] {
        &self.groups
    }

    /// The same, to change.
    pub fn groups_mut(&mut self) -> &mut Vec<String> {
        &mut self.groups
    }

    /// How much of the stream holding it the variant stands in for: the
    /// length set, or what the reading lasts: music21's
    /// `replacementQuarterLength`.
    pub fn replacement_quarter_length(&self) -> FloatType {
        self.replacement_quarter_length
            .unwrap_or_else(|| self.contained_highest_time())
    }

    /// Sets how much of the stream the variant stands in for, or with
    /// `None` makes it what the reading lasts.
    pub fn set_replacement_quarter_length(&mut self, length: Option<FloatType>) {
        self.replacement_quarter_length = length;
    }

    /// When the last thing the reading holds ends: music21's
    /// `containedHighestTime`.
    pub fn contained_highest_time(&self) -> FloatType {
        highest_time(&self.contents)
    }

    /// Where the last thing the reading holds starts: music21's
    /// `containedHighestOffset`.
    pub fn contained_highest_offset(&self) -> FloatType {
        self.contents
            .events()
            .iter()
            .map(|event| event.offset())
            .fold(0.0, FloatType::max)
    }

    /// Whether the reading is as long as what it stands in for, longer or
    /// shorter: music21's `lengthType`.
    pub fn length_type(&self) -> LengthType {
        let difference = self.replacement_quarter_length() - self.contained_highest_time();
        if difference > 0.0 {
            LengthType::Deletion
        } else if difference < 0.0 {
            LengthType::Elongation
        } else {
            LengthType::Replacement
        }
    }

    /// The variant with its reading transposed.
    ///
    /// # Errors
    ///
    /// As [`Stream::transpose`].
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        Ok(Self {
            contents: self.contents.transpose(interval)?,
            ..self.clone()
        })
    }
}

impl From<Variant> for StreamElement {
    fn from(variant: Variant) -> Self {
        StreamElement::Variant(Box::new(variant))
    }
}

/// When the last thing a stream holds ends, as music21's `highestTime`
/// has it: each end snapped to the fraction it stands for, as music21
/// keeps its offsets and lengths as fractions.
pub fn highest_time(stream: &Stream) -> FloatType {
    stream
        .events()
        .iter()
        .map(|event| op_frac(event.offset() + length_of(event.element())))
        .fold(0.0, FloatType::max)
}

/// How long an element lasts, a stream as [`highest_time`] has it.
fn length_of(element: &StreamElement) -> FloatType {
    match element {
        StreamElement::Stream(inner) => highest_time(inner),
        other => other.quarter_length(),
    }
}

fn variant_error(message: impl Into<String>) -> Error {
    Error::Variant(message.into())
}

/// Where each leaf of a stream went, composed with where they went next.
fn then(first: Vec<Option<usize>>, next: &[Option<usize>]) -> Vec<Option<usize>> {
    first
        .into_iter()
        .map(|place| place.and_then(|at| next.get(at).copied().flatten()))
        .collect()
}

/// Every leaf of a stream where it was.
fn unmoved(stream: &Stream) -> Vec<Option<usize>> {
    (0..stream.leaf_total()).map(Some).collect()
}

// ------------------------------------------------------------ classes

/// music21's class names of an element, the most particular first: what
/// music21's `classes` holds, as far as variants ask.
fn classes(element: &StreamElement) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = match element {
        StreamElement::Note(_) => vec!["Note", "NotRest", "GeneralNote"],
        StreamElement::Unpitched(_) => vec!["Unpitched", "NotRest", "GeneralNote"],
        StreamElement::Chord(chord) if chord.numeral().is_some() => vec![
            "RomanNumeral",
            "Harmony",
            "Chord",
            "ChordBase",
            "NotRest",
            "GeneralNote",
        ],
        StreamElement::Chord(_) => vec!["Chord", "ChordBase", "NotRest", "GeneralNote"],
        StreamElement::PercussionChord(_) => {
            vec!["PercussionChord", "ChordBase", "NotRest", "GeneralNote"]
        }
        StreamElement::ChordSymbol(symbol) => {
            let mut names = vec![
                "ChordSymbol",
                "Harmony",
                "Chord",
                "ChordBase",
                "NotRest",
                "GeneralNote",
            ];
            if symbol.is_no_chord() {
                names.insert(0, "NoChord");
            }
            names
        }
        StreamElement::Rest(_) => vec!["Rest", "GeneralNote"],
        StreamElement::Stream(stream) => match stream.kind() {
            StreamKind::Measure => vec!["Measure", "Stream", "StreamCore"],
            StreamKind::Voice => vec!["Voice", "Stream", "StreamCore"],
            StreamKind::Part => vec!["Part", "Stream", "StreamCore"],
            StreamKind::PartStaff => vec!["PartStaff", "Part", "Stream", "StreamCore"],
            StreamKind::Score => vec!["Score", "Stream", "StreamCore"],
            StreamKind::Opus => vec!["Opus", "Stream", "StreamCore"],
            StreamKind::Stream => vec!["Stream", "StreamCore"],
        },
        StreamElement::Clef(clef) => {
            let kind = clef.kind();
            let mut names = vec![kind.class_name()];
            names.extend(kind.parents().iter().copied());
            if !names.contains(&"Clef") {
                names.push("Clef");
            }
            names
        }
        StreamElement::Instrument(_) => vec!["Instrument"],
        StreamElement::KeySignature(_) => vec!["KeySignature"],
        StreamElement::Key(_) => vec!["Key", "KeySignature"],
        StreamElement::TimeSignature(_) => vec!["TimeSignature", "TimeSignatureBase"],
        StreamElement::MetronomeMark(_) => vec!["MetronomeMark", "TempoIndication"],
        StreamElement::TempoText(_) => vec!["TempoText", "TempoIndication"],
        StreamElement::MetricModulation(_) => vec!["MetricModulation", "TempoIndication"],
        StreamElement::TextExpression(_) => vec!["TextExpression", "Expression"],
        StreamElement::RepeatExpression(_) => vec!["RepeatExpression", "Expression"],
        StreamElement::Dynamic(_) => vec!["Dynamic"],
        StreamElement::Barline(_) => vec!["Barline"],
        StreamElement::PedalObject(_) => vec!["PedalObject"],
        StreamElement::RehearsalMark(_) => vec!["RehearsalMark"],
        StreamElement::Break(_) => vec!["Break"],
        StreamElement::Layout(layout) => vec![layout.class_name(), "LayoutBase"],
        StreamElement::TextBox(_) => vec!["TextBox"],
        StreamElement::Variant(_) => vec!["Variant"],
    };
    names.push("Music21Object");
    names
}

/// Whether an element is one of a class, by music21's name for it.
fn is_a(element: &StreamElement, class: &str) -> bool {
    classes(element).contains(&class)
}

/// music21's `GeneralNote`: a note, chord or rest of any kind.
fn is_general_note(element: &StreamElement) -> bool {
    is_a(element, "GeneralNote")
}

fn is_measure(element: &StreamElement) -> bool {
    matches!(element, StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure)
}

fn measure_number(element: &StreamElement) -> Option<IntegerType> {
    match element {
        StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure => Some(inner.number()),
        _ => None,
    }
}

fn set_measure_number(element: &mut StreamElement, number: IntegerType) {
    if let StreamElement::Stream(inner) = element
        && inner.kind() == StreamKind::Measure
    {
        inner.set_number(number);
    }
}

/// music21's `getElementsByOffset`, as the variant functions call it --
/// each element starting in the span, and none for having started before
/// it -- for an element at `offset` lasting `length`. With no end, the span
/// is the one offset.
fn in_span(
    offset: FloatType,
    length: FloatType,
    start: FloatType,
    end: Option<FloatType>,
    include_end: bool,
) -> bool {
    let start = op_frac(start);
    let (end, zero_length_search) = match end {
        None => (start, true),
        Some(end) => {
            let end = op_frac(end);
            (end, end <= start)
        }
    };
    if offset > end {
        return false;
    }
    if op_frac(offset + length) < start {
        return false;
    }
    if zero_length_search && length == 0.0 {
        return true;
    }
    if offset < start {
        return false;
    }
    !(!include_end && offset == end)
}

/// A number as music21 holds it, exactly: an offset snapped to a fraction
/// is a `Fraction` unless it is a binary fraction, which is a float.
#[derive(Clone, Copy, Debug)]
struct Exact {
    numerator: i128,
    denominator: i128,
    /// Whether music21 holds it as a `Fraction` rather than a float.
    fraction: bool,
}

impl Exact {
    /// A float's exact value.
    fn of_float(value: FloatType) -> Self {
        if value == 0.0 || !value.is_finite() {
            return Self {
                numerator: 0,
                denominator: 1,
                fraction: false,
            };
        }
        let bits = value.to_bits();
        let negative = bits >> 63 == 1;
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let mantissa = if exponent == 0 {
            (bits & 0xf_ffff_ffff_ffff) << 1
        } else {
            (bits & 0xf_ffff_ffff_ffff) | 0x10_0000_0000_0000
        };
        let mut numerator = mantissa as i128;
        let mut power = exponent.max(1) - 1075;
        while numerator % 2 == 0 && power < 0 {
            numerator /= 2;
            power += 1;
        }
        let (numerator, denominator) = if power >= 0 {
            (numerator << power.min(60), 1)
        } else {
            (numerator, 1_i128 << (-power).min(100))
        };
        Self {
            numerator: if negative { -numerator } else { numerator },
            denominator,
            fraction: false,
        }
    }

    /// An offset as music21 keeps it: snapped by `opFrac`.
    fn held(value: FloatType) -> Self {
        let snapped = op_frac(value);
        match crate::duration::limited_fraction(snapped.abs(), 65535) {
            Some((numerator, denominator))
                if denominator & (denominator - 1) != 0
                    && (numerator as FloatType / denominator as FloatType - snapped.abs())
                        .abs()
                        < 1e-9 =>
            {
                Self {
                    numerator: if snapped < 0.0 { -numerator } else { numerator },
                    denominator,
                    fraction: true,
                }
            }
            _ => Self::of_float(snapped),
        }
    }

    /// Two numbers music21 holds added as Python adds them: exactly when
    /// both are fractions, and as floats otherwise.
    fn plus(self, other: Self) -> Self {
        if self.fraction && other.fraction {
            Self {
                numerator: self.numerator * other.denominator + other.numerator * self.denominator,
                denominator: self.denominator * other.denominator,
                fraction: true,
            }
        } else {
            Self::of_float(self.to_float() + other.to_float())
        }
    }

    fn to_float(self) -> FloatType {
        self.numerator as FloatType / self.denominator as FloatType
    }

    fn cmp(self, other: Self) -> std::cmp::Ordering {
        (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
    }
}

/// music21's `getElementsByOffset` at one offset, given as music21 holds
/// it: an element there, or of no length ending there. The offset is
/// snapped as the start of the span but is its end as given, so a float
/// short of the fraction an element stands at finds nothing.
fn at_offset(offset: FloatType, length: FloatType, at: Exact) -> bool {
    let start = Exact::held(at.to_float());
    let held = Exact::held(offset);
    if held.cmp(at).is_gt() {
        return false;
    }
    if Exact::held(offset + length).cmp(start).is_lt() {
        return false;
    }
    if length == 0.0 {
        return true;
    }
    !held.cmp(start).is_lt()
}

// --------------------------------------------------------------- site

/// One element of a stream being worked on, known by an id that stays with
/// it however it moves, with the index it stood at in the stream it came
/// from, if it came from it.
#[derive(Clone, Debug)]
struct Held {
    id: usize,
    origin: Option<usize>,
    order: usize,
    offset: FloatType,
    element: StreamElement,
}

/// A stream being worked on as music21 works on one in place: elements
/// taken out and put in, each found again by its id, kept in the order
/// music21 sorts them in -- by offset, class, grace notes first, and the
/// order they were put in.
#[derive(Clone, Debug)]
struct Site {
    held: Vec<Held>,
    next: usize,
}

impl Site {
    fn of(stream: &Stream) -> Self {
        let held: Vec<Held> = stream
            .events()
            .iter()
            .enumerate()
            .map(|(index, event)| Held {
                id: index,
                origin: Some(index),
                order: index,
                offset: event.offset(),
                element: event.element().clone(),
            })
            .collect();
        let next = held.len();
        let mut site = Self { held, next };
        site.sort();
        site
    }

    fn sort(&mut self) {
        let grace = |held: &Held| held.element.duration().is_some_and(Duration::is_grace);
        self.held.sort_by(|left, right| {
            left.offset
                .total_cmp(&right.offset)
                .then(
                    left.element
                        .class_sort_order()
                        .cmp(&right.element.class_sort_order()),
                )
                .then(grace(right).cmp(&grace(left)))
                .then(left.order.cmp(&right.order))
        });
    }

    fn get(&self, id: usize) -> Option<&Held> {
        self.held.iter().find(|held| held.id == id)
    }

    fn get_mut(&mut self, id: usize) -> Option<&mut Held> {
        self.held.iter_mut().find(|held| held.id == id)
    }

    fn offset(&self, id: usize) -> Result<FloatType> {
        self.get(id)
            .map(|held| held.offset)
            .ok_or_else(|| variant_error("the element is not in the stream"))
    }

    fn variant(&self, id: usize) -> Result<&Variant> {
        match self.get(id).map(|held| &held.element) {
            Some(StreamElement::Variant(variant)) => Ok(variant),
            _ => Err(variant_error("Variant not found in stream")),
        }
    }

    fn variant_mut(&mut self, id: usize) -> Result<&mut Variant> {
        match self.get_mut(id).map(|held| &mut held.element) {
            Some(StreamElement::Variant(variant)) => Ok(variant),
            _ => Err(variant_error("Variant not found in stream")),
        }
    }

    /// Puts in a new element, after anything already here that sorts
    /// alike, and answers its id.
    fn insert(&mut self, offset: FloatType, element: StreamElement) -> usize {
        let id = self.next;
        self.next += 1;
        self.held.push(Held {
            id,
            origin: None,
            order: id,
            offset: op_frac(offset),
            element,
        });
        self.sort();
        id
    }

    /// Puts back an element taken out, at a new offset, after anything here
    /// that sorts alike.
    fn reinsert(&mut self, mut held: Held, offset: FloatType) {
        held.offset = op_frac(offset);
        held.order = self.next;
        self.next += 1;
        self.held.push(held);
        self.sort();
    }

    fn remove(&mut self, id: usize) -> Option<Held> {
        let at = self.held.iter().position(|held| held.id == id)?;
        Some(self.held.remove(at))
    }

    /// When the last element ends: music21's `highestTime`.
    fn highest_time(&self) -> FloatType {
        self.held
            .iter()
            .map(|held| op_frac(held.offset + length_of(&held.element)))
            .fold(0.0, FloatType::max)
    }

    /// The ids of the elements starting in a span, in order.
    fn in_span(&self, start: FloatType, end: Option<FloatType>, include_end: bool) -> Vec<usize> {
        self.held
            .iter()
            .filter(|held| {
                in_span(
                    held.offset,
                    length_of(&held.element),
                    start,
                    end,
                    include_end,
                )
            })
            .map(|held| held.id)
            .collect()
    }

    /// The ids of the variants, in order.
    fn variants(&self) -> Vec<usize> {
        self.held
            .iter()
            .filter(|held| matches!(held.element, StreamElement::Variant(_)))
            .map(|held| held.id)
            .collect()
    }

    /// The ids of the measures, in order.
    fn measures(&self) -> Vec<usize> {
        self.held
            .iter()
            .filter(|held| is_measure(&held.element))
            .map(|held| held.id)
            .collect()
    }

    /// The stream as worked on: `base` holding what is here now, its
    /// spanners moved with what they name. The answer is also where each of
    /// `base`'s leaves went.
    fn finish(self, mut base: Stream) -> (Stream, Vec<Option<usize>>) {
        let events = self
            .held
            .into_iter()
            .map(|held| (StreamEvent::new(held.offset, held.element), held.origin))
            .collect();
        let moved = base.replace_events(events);
        (base, moved)
    }
}

/// Puts an element in a variant's reading after what sorts alike there.
fn put(contents: &mut Stream, offset: FloatType, element: StreamElement) {
    contents.insert_sorted(vec![StreamEvent::new(op_frac(offset), element)]);
}

/// Moves everything in a variant's reading later by `shift`.
fn shift_contents(contents: &mut Stream, shift: FloatType) {
    let events: Vec<(StreamEvent, Option<usize>)> = contents
        .events()
        .iter()
        .enumerate()
        .map(|(index, event)| {
            (
                StreamEvent::new(op_frac(event.offset() + shift), event.element().clone()),
                Some(index),
            )
        })
        .collect();
    contents.replace_events(events);
}

// -------------------------------------------------------- replacement

/// What a variant at `id` stands in for among a site's elements, each with
/// its offset in music21's answer, in the order music21 sorts them: see
/// [`replaced_elements`].
fn replaced(
    site: &Site,
    id: usize,
    class_list: &[&str],
    keep_original_offsets: bool,
    include_spacers: bool,
) -> Result<Vec<(usize, FloatType)>> {
    let variant = site.variant(id)?;
    let start = site.offset(id)?;
    let spacer = if include_spacers {
        variant
            .contents
            .events()
            .iter()
            .find_map(|event| match event.element() {
                StreamElement::Rest(rest) if rest.hidden() => {
                    Some(rest.duration().quarter_length())
                }
                _ => None,
            })
            .ok_or_else(|| variant_error("'NoneType' object has no attribute 'duration'"))?
    } else {
        0.0
    };
    let mut wanted: Vec<&str> = variant
        .contents
        .events()
        .iter()
        .map(|event| classes(event.element())[0])
        .collect();
    wanted.extend_from_slice(class_list);
    let of_class = |held_id: usize| {
        site.get(held_id)
            .is_some_and(|held| wanted.iter().any(|class| is_a(&held.element, class)))
    };
    let mut found: Vec<(usize, FloatType)> = Vec::new();
    match variant.length_type() {
        LengthType::Replacement | LengthType::Elongation => {
            let end = start + variant.replacement_quarter_length() + spacer;
            for held_id in site.in_span(start, Some(end), false) {
                if of_class(held_id) {
                    found.push((held_id, site.offset(held_id)?));
                }
            }
        }
        LengthType::Deletion => {
            let middle = start + variant.contained_highest_time();
            let end = start + variant.replacement_quarter_length();
            for held_id in site.in_span(start, Some(middle), false) {
                if of_class(held_id) {
                    found.push((held_id, site.offset(held_id)?));
                }
            }
            for held_id in site.in_span(middle, Some(end), false) {
                if found.iter().any(|(seen, _)| *seen == held_id) {
                    return Err(variant_error("the object is already found in this Stream"));
                }
                found.push((held_id, op_frac(middle - start + site.offset(held_id)?)));
            }
        }
    }
    found.retain(|(held_id, _)| *held_id != id);
    if !keep_original_offsets {
        for (_, offset) in &mut found {
            *offset = op_frac(*offset - start);
        }
    }
    // Sorted in the stream music21 answers with, which they went into in
    // this order.
    let grace = |held_id: usize| {
        site.get(held_id)
            .is_some_and(|held| held.element.duration().is_some_and(Duration::is_grace))
    };
    let sort_order = |held_id: usize| {
        site.get(held_id)
            .map_or(0, |held| held.element.class_sort_order())
    };
    found.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then(sort_order(left.0).cmp(&sort_order(right.0)))
            .then(grace(right.0).cmp(&grace(left.0)))
    });
    Ok(found)
}

/// What the variant at event `variant` of a stream stands in for: the
/// elements of the stream of the classes of what the variant holds (and of
/// `class_list`) starting where it does and within its replacement length,
/// at their offsets from the variant's unless `keep_original_offsets`:
/// music21's `replacedElements`. With `include_spacers`, the span goes on
/// by as long as the first hidden rest the variant holds.
///
/// A variant shorter than what it replaces stands in, past what it holds,
/// for everything there whatever its class, which music21 puts that much
/// later again.
///
/// # Errors
///
/// No variant at `variant`, or with `include_spacers` no hidden rest in it.
pub fn replaced_elements(
    stream: &Stream,
    variant: usize,
    class_list: &[&str],
    keep_original_offsets: bool,
    include_spacers: bool,
) -> Result<Stream> {
    let site = Site::of(stream);
    let found = replaced(
        &site,
        variant,
        class_list,
        keep_original_offsets,
        include_spacers,
    )?;
    let mut out = Stream::with_kind(stream.kind());
    let events: Vec<StreamEvent> = found
        .into_iter()
        .filter_map(|(id, offset)| {
            site.get(id)
                .map(|held| StreamEvent::new(offset, held.element.clone()))
        })
        .collect();
    out.replace_events(events.into_iter().map(|event| (event, None)).collect());
    Ok(out)
}

/// Takes out of a stream what the variant at event `variant` stands in for:
/// music21's `removeReplacedElementsFromStream`.
///
/// # Errors
///
/// As [`replaced_elements`].
pub fn remove_replaced_elements_from_stream(
    stream: &mut Stream,
    variant: usize,
    class_list: &[&str],
) -> Result<()> {
    let mut site = Site::of(stream);
    for (id, _) in replaced(&site, variant, class_list, false, false)? {
        site.remove(id);
    }
    let (out, _) = site.finish(stream.clone());
    *stream = out;
    Ok(())
}

// --------------------------------------------------------------- hashes

/// The notes, rests and chords a stream holds itself.
fn own_notes(stream: &Stream) -> Vec<crate::search::Searched<'_>> {
    iterated(stream)
        .into_iter()
        .filter(|found| is_general_note(found.element))
        .collect()
}

/// Each measure of a stream as the text of its notes and rests:
/// music21's `getMeasureHashes`.
pub fn measure_hashes(stream: &Stream) -> Vec<String> {
    stream
        .measures()
        .into_iter()
        .map(|measure| translate_stream_to_string(&own_notes(measure)).text)
        .collect()
}

fn measure_hash(measure: &Stream) -> String {
    translate_stream_to_string(&own_notes(measure)).text
}

/// Python's `difflib` opcodes between two lists of items compared equal or
/// not.
fn opcodes_of<'a, T: PartialEq>(a: &'a [T], b: &'a [T]) -> Vec<difflib::Opcode> {
    // Each item stands for the first item equal to it.
    let mut seen: Vec<&'a T> = Vec::new();
    let mut id = |item: &'a T| match seen.iter().position(|known| *known == item) {
        Some(at) => at,
        None => {
            seen.push(item);
            seen.len() - 1
        }
    };
    let a: Vec<usize> = a.iter().map(&mut id).collect();
    let b: Vec<usize> = b.iter().map(&mut id).collect();
    difflib::opcodes(&a, &b)
}

// ------------------------------------------------------------ measures

/// The event indices of a stream's own measures numbered `start` to `end`,
/// or from `start` on, counting them from one where every measure is
/// numbered nought: music21's `measures` and `measure` finding them.
fn numbered(stream: &Stream, start: IntegerType, end: Option<IntegerType>) -> Vec<usize> {
    let measures: Vec<(usize, IntegerType)> = stream
        .events()
        .iter()
        .enumerate()
        .filter_map(|(index, event)| measure_number(event.element()).map(|number| (index, number)))
        .collect();
    let numbered = measures.iter().any(|(_, number)| *number != 0);
    measures
        .iter()
        .enumerate()
        .filter(|(count, (_, number))| {
            let number = if numbered {
                *number
            } else {
                *count as IntegerType + 1
            };
            number >= start && end.is_none_or(|end| number <= end)
        })
        .map(|(_, (index, _))| *index)
        .collect()
}

/// How long music21's `measures` of those measures lasts: from the start
/// of the first to the end of the last.
fn span_length(stream: &Stream, measures: &[usize]) -> FloatType {
    let Some(&first) = measures.first() else {
        return 0.0;
    };
    let start = stream.events()[first].offset();
    measures
        .iter()
        .map(|&index| {
            let event = &stream.events()[index];
            op_frac(op_frac(event.offset() - start) + length_of(event.element()))
        })
        .fold(0.0, FloatType::max)
}

/// The measures, as music21's `measures` holds them: from the first's
/// offset on.
fn measures_stream(stream: &Stream, measures: &[usize]) -> Stream {
    let mut out = Stream::new();
    let Some(&first) = measures.first() else {
        return out;
    };
    let start = stream.events()[first].offset();
    for &index in measures {
        let event = &stream.events()[index];
        put(
            &mut out,
            op_frac(event.offset() - start),
            event.element().clone(),
        );
    }
    out
}

fn no_such_measure() -> Error {
    variant_error("'NoneType' object has no attribute 'getOffsetBySite'")
}

// ---------------------------------------------------------- add_variant

/// The variant [`add_variant`] adds.
fn made_variant(
    start_offset: FloatType,
    contents: Option<&Stream>,
    variant_name: Option<&str>,
    variant_groups: Option<&[String]>,
    replacement_quarter_length: Option<FloatType>,
) -> Variant {
    let mut variant = Variant::new();
    if let Some(groups) = variant_groups {
        variant.groups = groups.to_vec();
    }
    if let Some(name) = variant_name {
        variant.groups.push(name.to_string());
    }
    variant.replacement_quarter_length = replacement_quarter_length;
    match contents {
        None => {}
        Some(stream) if stream.kind() == StreamKind::Measure => {
            variant
                .contents
                .push(StreamElement::Stream(Box::new(stream.clone())));
        }
        Some(stream) => {
            let measures = stream.measures();
            if measures.is_empty() {
                for event in stream.events() {
                    put(
                        &mut variant.contents,
                        event.offset() + start_offset,
                        event.element().clone(),
                    );
                }
            } else {
                for measure in measures {
                    variant
                        .contents
                        .push(StreamElement::Stream(Box::new(measure.clone())));
                }
            }
        }
    }
    variant
}

/// Adds a variant to a stream at `start_offset`, holding `contents`: a
/// measure, the measures of a stream, or, for a stream with none, its
/// elements -- each put as far after the variant's start as `start_offset`,
/// as music21 puts them. The variant is in `variant_groups` and
/// `variant_name`, and stands in for `replacement_quarter_length`, or for
/// as long as it lasts: music21's `addVariant`.
///
/// ```
/// use music21_rs::tinynotation::from_tiny_notation;
/// use music21_rs::variant::add_variant;
///
/// let mut line = from_tiny_notation("4/4 c4 d e f g1")?;
/// let other = from_tiny_notation("4/4 c4 d e g")?;
/// add_variant(&mut line, 0.0, Some(other.measures()[0]), Some("draft"), None, None);
/// assert_eq!(line.events().iter().filter(|event| matches!(
///     event.element(),
///     music21_rs::StreamElement::Variant(_)
/// )).count(), 1);
/// # Ok::<(), music21_rs::Error>(())
/// ```
pub fn add_variant(
    stream: &mut Stream,
    start_offset: FloatType,
    contents: Option<&Stream>,
    variant_name: Option<&str>,
    variant_groups: Option<&[String]>,
    replacement_quarter_length: Option<FloatType>,
) {
    added(
        stream,
        start_offset,
        contents,
        variant_name,
        variant_groups,
        replacement_quarter_length,
    );
}

fn added(
    stream: &mut Stream,
    start_offset: FloatType,
    contents: Option<&Stream>,
    variant_name: Option<&str>,
    variant_groups: Option<&[String]>,
    replacement_quarter_length: Option<FloatType>,
) -> Vec<Option<usize>> {
    let variant = made_variant(
        start_offset,
        contents,
        variant_name,
        variant_groups,
        replacement_quarter_length,
    );
    stream.insert_sorted(vec![StreamEvent::new(
        start_offset,
        StreamElement::from(variant),
    )])
}

// --------------------------------------------------------------- merges

/// Merges another version of a stream into a copy of it as variants named
/// `variant_name`: a score part by part ([`merge_variant_scores`]), a
/// stream of measures measure by measure
/// ([`merge_variant_measure_streams`]), or notes and rests of the same
/// length note by note ([`merge_variants_equal_duration`]): music21's
/// `mergeVariants`.
///
/// # Errors
///
/// As each of those, or a stream none of them merges.
pub fn merge_variants(x: &Stream, y: &Stream, variant_name: &str) -> Result<Stream> {
    if x.kind() == StreamKind::Score {
        merge_variant_scores(x, y, variant_name)
    } else if !x.measures().is_empty() {
        merge_variant_measure_streams(x, y, variant_name)
    } else if x
        .events()
        .iter()
        .any(|event| is_general_note(event.element()))
        && highest_time(x) == highest_time(y)
    {
        merge_variants_equal_duration(&[x, y], &[variant_name])
    } else {
        Err(variant_error(
            "Could not determine what merging method to use. Try using a more specific merging function.",
        ))
    }
}

/// Merges another version of a score into a copy of it, each part into the
/// part at its place, as [`merge_variant_measure_streams`] merges them:
/// music21's `mergeVariantScores`.
///
/// # Errors
///
/// Scores of different numbers of parts, or as
/// [`merge_variant_measure_streams`].
pub fn merge_variant_scores(a: &Stream, v: &Stream, variant_name: &str) -> Result<Stream> {
    let parts = |score: &Stream| -> Vec<usize> {
        score
            .events()
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                matches!(event.element(), StreamElement::Stream(inner)
                    if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff))
            })
            .map(|(index, _)| index)
            .collect()
    };
    let (ours, theirs) = (parts(a), parts(v));
    if ours.len() != theirs.len() {
        return Err(variant_error(
            "These scores do not have the same number of parts and cannot be merged.",
        ));
    }
    let mut out = a.clone();
    for (&part, &other) in ours.iter().zip(&theirs) {
        let StreamElement::Stream(y) = v.events()[other].element() else {
            continue;
        };
        out.edit_nested(&[part], |stream| {
            match merge_measure_streams_into(stream, y, variant_name) {
                Ok(moved) => (Ok(()), moved),
                Err(error) => (Err(error), unmoved(stream)),
            }
        })
        .0?;
    }
    Ok(out)
}

/// Merges another version of a stream of measures into a copy of it: the
/// measures of the two compared by their notes and rests as `difflib`
/// compares lists, and a variant added where they differ -- replacing
/// measures, taking them out or putting new ones in: music21's
/// `mergeVariantMeasureStreams`.
///
/// Measures are found by their numbers counted from one, as music21 finds
/// them, so a stream not numbered from one finds others than those that
/// differ, or none.
///
/// # Errors
///
/// A measure music21 looks for by number that is not there.
pub fn merge_variant_measure_streams(x: &Stream, y: &Stream, variant_name: &str) -> Result<Stream> {
    let mut out = x.clone();
    merge_measure_streams_into(&mut out, y, variant_name)?;
    Ok(out)
}

fn merge_measure_streams_into(
    out: &mut Stream,
    y: &Stream,
    variant_name: &str,
) -> Result<Vec<Option<usize>>> {
    let mut moved = unmoved(out);
    let regions = opcodes_of(&measure_hashes(out), &measure_hashes(y));
    for region in regions {
        let (x_start, x_end) = (region.i1 as IntegerType, region.i2 as IntegerType);
        let (y_start, y_end) = (region.j1 as IntegerType, region.j2 as IntegerType);
        let start_offset = if region.i1 >= out.measures().len() {
            highest_time(out)
        } else {
            let index = *numbered(out, x_start + 1, Some(x_start + 1))
                .first()
                .ok_or_else(no_such_measure)?;
            out.events()[index].offset()
        };
        let (contents, length) = match region.tag {
            difflib::OpcodeTag::Equal => continue,
            difflib::OpcodeTag::Replace => (
                Some(measures_stream(y, &numbered(y, y_start + 1, Some(y_end)))),
                span_length(out, &numbered(out, x_start + 1, Some(x_end))),
            ),
            difflib::OpcodeTag::Delete => (
                None,
                span_length(out, &numbered(out, x_start + 1, Some(x_end))),
            ),
            difflib::OpcodeTag::Insert => (
                Some(measures_stream(y, &numbered(y, y_start + 1, Some(y_end)))),
                0.0,
            ),
        };
        let step = added(
            out,
            start_offset,
            contents.as_ref(),
            Some(variant_name),
            None,
            Some(length),
        );
        moved = then(moved, &step);
    }
    Ok(moved)
}

/// Merges other versions of a stream, each as long as it, into a copy of
/// it: part by part and measure by measure where it has them, each stretch
/// of notes and rests that differs a variant, named by `variant_names` in
/// turn: music21's `mergeVariantsEqualDuration`.
///
/// # Errors
///
/// A version of another length, one with fewer parts or measures, or a
/// stream holding measures or parts where notes are compared.
pub fn merge_variants_equal_duration(
    streams: &[&Stream],
    variant_names: &[&str],
) -> Result<Stream> {
    let Some((first, others)) = streams.split_first() else {
        return Err(variant_error("list index out of range"));
    };
    let mut out = (*first).clone();
    let mut names: Vec<Option<String>> = vec![None];
    names.extend(variant_names.iter().map(|name| Some(name.to_string())));
    names.resize(streams.len(), None);
    let pairs: Vec<(&Stream, Option<String>)> = others
        .iter()
        .copied()
        .zip(names.into_iter().skip(1))
        .collect();
    equal_duration_into(&mut out, &pairs)?;
    Ok(out)
}

fn equal_duration_into(
    out: &mut Stream,
    others: &[(&Stream, Option<String>)],
) -> Result<Vec<Option<usize>>> {
    let mut moved = unmoved(out);
    let out_of_range = || variant_error("list index out of range");
    for (other, name) in others {
        if highest_time(out) != highest_time(other) {
            return Err(variant_error("cannot merge streams of different lengths"));
        }
        let name = name.as_deref();
        let part_events: Vec<usize> = out
            .events()
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                matches!(event.element(), StreamElement::Stream(inner)
                    if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff))
            })
            .map(|(index, _)| index)
            .collect();
        let step = if part_events.is_empty() {
            let theirs = other.measures();
            let ours = numbered_all(out);
            if ours.is_empty() {
                merged_into(out, &[], other, name)?
            } else {
                let mut step = unmoved(out);
                for (j, &index) in ours.iter().enumerate() {
                    let measure = theirs.get(j).ok_or_else(out_of_range)?;
                    step = then(step, &merged_into(out, &[index], measure, name)?);
                }
                step
            }
        } else {
            let their_parts = other.parts();
            let mut step = unmoved(out);
            for (i, &part_index) in part_events.iter().enumerate() {
                let their_part = their_parts.get(i).ok_or_else(out_of_range)?;
                let StreamElement::Stream(part) = out.events()[part_index].element() else {
                    continue;
                };
                let ours = numbered_all(part);
                if ours.is_empty() {
                    step = then(step, &merged_into(out, &[part_index], their_part, name)?);
                } else {
                    let theirs = their_part.measures();
                    for (j, &index) in ours.iter().enumerate() {
                        let measure = theirs.get(j).ok_or_else(out_of_range)?;
                        step = then(
                            step,
                            &merged_into(out, &[part_index, index], measure, name)?,
                        );
                    }
                }
            }
            step
        };
        moved = then(moved, &step);
    }
    Ok(moved)
}

/// The event indices of a stream's own measures.
fn numbered_all(stream: &Stream) -> Vec<usize> {
    stream
        .events()
        .iter()
        .enumerate()
        .filter(|(_, event)| is_measure(event.element()))
        .map(|(index, _)| index)
        .collect()
}

/// [`merge_notes`] into the stream at `path` within `out`.
fn merged_into(
    out: &mut Stream,
    path: &[usize],
    other: &Stream,
    name: Option<&str>,
) -> Result<Vec<Option<usize>>> {
    let (result, moved) = out.edit_nested(path, |stream| match merge_notes(stream, other, name) {
        Ok(moved) => (Ok(()), moved),
        Err(error) => (Err(error), unmoved(stream)),
    });
    result.map(|()| moved)
}

fn holds_measures_or_parts(stream: &Stream) -> bool {
    stream.events().iter().any(|event| {
        matches!(event.element(), StreamElement::Stream(inner)
            if matches!(inner.kind(), StreamKind::Measure | StreamKind::Part | StreamKind::PartStaff))
    })
}

/// music21's `_mergeVariants`: the notes and rests of two streams of the
/// same length walked together, and each stretch where `b`'s differ from
/// `a`'s, or start where none of `a`'s does, a variant in `a`.
fn merge_notes(a: &mut Stream, b: &Stream, name: Option<&str>) -> Result<Vec<Option<usize>>> {
    if holds_measures_or_parts(a) || holds_measures_or_parts(b) {
        return Err(variant_error(
            "_mergeVariants cannot merge streams which contain measures or parts.",
        ));
    }
    if highest_time(a) != highest_time(b) {
        return Err(variant_error(
            "_mergeVariants cannot merge streams which are of different lengths",
        ));
    }
    let notes = |stream: &Stream| -> Vec<StreamEvent> {
        stream
            .flatten()
            .events()
            .iter()
            .filter(|event| is_general_note(event.element()))
            .cloned()
            .collect()
    };
    let (a_notes, b_notes) = (notes(a), notes(b));
    let mut moved = unmoved(a);
    let (mut i, mut j) = (0, 0);
    let mut in_variant = false;
    let mut buffer: Vec<&StreamEvent> = Vec::new();
    let mut variant_start = 0.0;
    let mut insert = |a: &mut Stream, buffer: &[&StreamEvent], start: FloatType| {
        let mut variant = Variant::new();
        for event in buffer {
            put(
                &mut variant.contents,
                event.offset() - start,
                event.element().clone(),
            );
        }
        if let Some(name) = name {
            variant.groups.push(name.to_string());
        }
        let step = a.insert_sorted(vec![StreamEvent::new(start, StreamElement::from(variant))]);
        moved = then(std::mem::take(&mut moved), &step);
    };
    while i < a_notes.len() && j < b_notes.len() {
        let (ours, theirs) = (&a_notes[i], &b_notes[j]);
        if ours.offset() == theirs.offset() {
            if !equal(ours.element(), theirs.element()) {
                if !in_variant {
                    variant_start = theirs.offset();
                    in_variant = true;
                    buffer = vec![theirs];
                } else {
                    buffer.push(theirs);
                }
            } else if in_variant {
                insert(a, &buffer, variant_start);
                in_variant = false;
                buffer.clear();
            }
            i += 1;
            j += 1;
        } else if ours.offset() > theirs.offset() {
            if !in_variant {
                variant_start = theirs.offset();
                buffer = vec![theirs];
                in_variant = true;
            } else {
                buffer.push(theirs);
            }
            j += 1;
        } else {
            i += 1;
        }
    }
    if in_variant {
        insert(a, &buffer, variant_start);
    }
    drop(insert);
    Ok(moved)
}

/// Merges the measures of an ossia part that hold notes into a copy of a
/// part, each as a variant named `ossia_name` where the ossia's measure
/// stands -- the measure of the same number, with
/// `compare_by_measure_number`, or else at the same offset -- or, with
/// `recurse_in_measures`, each stretch of its notes that differs as a
/// variant inside the part's measure: music21's `mergePartAsOssia`.
///
/// # Errors
///
/// No measure of the part where the ossia has one, or as
/// [`merge_variants_equal_duration`].
pub fn merge_part_as_ossia(
    main: &Stream,
    ossia: &Stream,
    ossia_name: &str,
    compare_by_measure_number: bool,
    recurse_in_measures: bool,
) -> Result<Stream> {
    let mut out = main.clone();
    let no_measure = || variant_error("'NoneType' object has no attribute 'highestTime'");
    for event in ossia.events() {
        let StreamElement::Stream(measure) = event.element() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure
            || !measure
                .events()
                .iter()
                .any(|inner| is_a(inner.element(), "NotRest"))
        {
            continue;
        }
        if compare_by_measure_number {
            let target = numbered(&out, measure.number(), Some(measure.number()))
                .first()
                .copied();
            if recurse_in_measures {
                let index = target.ok_or_else(no_measure)?;
                let pairs = [(&**measure, Some(ossia_name.to_string()))];
                out.edit_nested(&[index], |stream| {
                    match equal_duration_into(stream, &pairs) {
                        Ok(moved) => (Ok(()), moved),
                        Err(error) => (Err(error), unmoved(stream)),
                    }
                })
                .0?;
            } else {
                let index = target.ok_or_else(no_such_measure)?;
                let offset = out.events()[index].offset();
                added(
                    &mut out,
                    offset,
                    Some(measure),
                    Some(ossia_name),
                    None,
                    None,
                );
            }
        } else {
            let offset = event.offset();
            if recurse_in_measures {
                let index = out
                    .events()
                    .iter()
                    .enumerate()
                    .find(|(_, held)| {
                        is_measure(held.element())
                            && at_offset(
                                held.offset(),
                                length_of(held.element()),
                                Exact::held(offset),
                            )
                    })
                    .map(|(index, _)| index)
                    .ok_or_else(no_measure)?;
                let pairs = [(&**measure, Some(ossia_name.to_string()))];
                out.edit_nested(&[index], |stream| {
                    match equal_duration_into(stream, &pairs) {
                        Ok(moved) => (Ok(()), moved),
                        Err(error) => (Err(error), unmoved(stream)),
                    }
                })
                .0?;
            } else {
                added(
                    &mut out,
                    offset,
                    Some(measure),
                    Some(ossia_name),
                    None,
                    None,
                );
            }
        }
    }
    Ok(out)
}

// --------------------------------------------------------------- refine

/// One choice on the way to music21's `_getBestListAndScore`: for each
/// element of the variant, which element of the region it is, or none for a
/// measure the variant adds, and how bad the match is.
#[allow(clippy::too_many_arguments)]
fn best_list(
    x: &[&StreamElement],
    y: &[&StreamElement],
    badness: &mut HashMap<(isize, isize, bool), FloatType>,
    lists: &mut HashMap<(isize, isize, bool), Vec<Option<usize>>>,
    is_none: bool,
    x_index: isize,
    y_index: isize,
) -> Result<(Vec<Option<usize>>, FloatType)> {
    let key = (x_index, y_index, is_none);
    if y_index >= y.len() as isize {
        lists.insert(key, Vec::new());
        badness.insert(key, 0.0);
        return Ok((Vec::new(), 0.0));
    }
    if let Some(&bad) = badness.get(&key) {
        return Ok((lists.get(&key).cloned().unwrap_or_default(), bad));
    }
    let similarity = if x_index == -1 && y_index == -1 {
        0.0
    } else if is_none {
        0.5
    } else {
        diff_score(x[x_index as usize], y[y_index as usize])?
    };
    let (mut best_score, mut best_normalized, mut best) = (1.0, 1.0, Vec::new());
    let mut consider = |(list, bad): (Vec<Option<usize>>, FloatType)| {
        let normalized = if list.is_empty() {
            0.0
        } else {
            bad / list.len() as FloatType
        };
        if normalized <= best_normalized {
            best_score = bad;
            best_normalized = normalized;
            best = list;
        }
    };
    consider(best_list(x, y, badness, lists, true, x_index, y_index + 1)?);
    for k in (x_index + 1)..(x.len() as isize) {
        consider(best_list(x, y, badness, lists, false, k, y_index + 1)?);
    }
    let mut list = best;
    if is_none {
        list.insert(0, None);
    } else if x_index != -1 {
        list.insert(0, Some(x_index as usize));
    }
    let bad = best_score + similarity;
    badness.insert(key, bad);
    lists.insert(key, list.clone());
    Ok((list, bad))
}

/// music21's `_diffScore`: nought for two measures of the same notes and
/// rests and 0.4 for others, and a thousandth for each number apart.
fn diff_score(x: &StreamElement, y: &StreamElement) -> Result<FloatType> {
    let (StreamElement::Stream(x), StreamElement::Stream(y)) = (x, y) else {
        return Err(variant_error("object has no attribute 'notesAndRests'"));
    };
    if x.kind() != StreamKind::Measure || y.kind() != StreamKind::Measure {
        return Err(variant_error("'Stream' object has no attribute 'number'"));
    }
    let base = if measure_hash(x) == measure_hash(y) {
        0.0
    } else {
        0.4
    };
    Ok(base + FloatType::from(x.number() - y.number()) * 0.001)
}

/// Refines the variant at event `variant` of a stream measure by measure,
/// as music21's `refineVariant` does in place: the measures it replaces
/// matched with the measures it holds, and where a run of them match, each
/// stretch of notes that differs a variant inside the stream's measure.
/// The variant itself is taken out.
///
/// music21 adds the runs that do not match to a copy it then lets go, so
/// they are not kept here either. The measures of a matching run are found
/// by their numbers counted from one, as music21 finds them, and the
/// variant's groups name the first run's variants only: music21 puts a
/// `None` before them for each run.
///
/// # Errors
///
/// No variant at `variant`; something other than a measure in what it
/// replaces or holds; or as [`merge_variants_equal_duration`].
pub fn refine_variant(stream: &mut Stream, variant: usize) -> Result<()> {
    let site = Site::of(stream);
    let held = site.variant(variant)?.clone();
    let region = replaced(&site, variant, &[], false, false)?;
    let region_elements: Vec<&StreamElement> = region
        .iter()
        .filter_map(|(id, _)| site.get(*id).map(|held| &held.element))
        .collect();
    let variant_elements: Vec<&StreamElement> = held
        .contents
        .events()
        .iter()
        .map(StreamEvent::element)
        .collect();
    let (chosen, _) = best_list(
        &region_elements,
        &variant_elements,
        &mut HashMap::new(),
        &mut HashMap::new(),
        false,
        -1,
        -1,
    )?;
    let ours: Vec<Option<usize>> = (0..region.len()).map(Some).collect();
    // The region as the stream music21 answers with: what each element is
    // in the stream.
    let mut region_stream = Stream::new();
    region_stream.replace_events(
        region
            .iter()
            .filter_map(|(id, offset)| {
                site.get(*id)
                    .map(|held| (StreamEvent::new(*offset, held.element.clone()), None))
            })
            .collect(),
    );
    let mut names: Vec<Option<String>> = held.groups.iter().cloned().map(Some).collect();
    for step in opcodes_of(&ours, &chosen) {
        if region.get(step.i1).is_none() {
            return Err(variant_error("index out of range"));
        }
        if step.tag != difflib::OpcodeTag::Equal {
            continue;
        }
        let ours_matched = numbered(
            &region_stream,
            step.i1 as IntegerType + 1,
            Some(step.i2 as IntegerType),
        );
        let theirs_matched = numbered(
            &held.contents,
            step.j1 as IntegerType + 1,
            Some(step.j2 as IntegerType),
        );
        // music21's names: a `None` put first and the list cut to two.
        names.insert(0, None);
        names.resize(2, None);
        let name = names[1].clone();
        if span_length(&region_stream, &ours_matched)
            != span_length(&held.contents, &theirs_matched)
        {
            return Err(variant_error("cannot merge streams of different lengths"));
        }
        if ours_matched.is_empty() {
            if !theirs_matched.is_empty() {
                return Err(variant_error(
                    "_mergeVariants cannot merge streams which contain measures or parts.",
                ));
            }
            continue;
        }
        let theirs: Vec<&Stream> = theirs_matched
            .iter()
            .filter_map(|&index| held.contents.events()[index].element().as_stream())
            .collect();
        for (j, &index) in ours_matched.iter().enumerate() {
            let measure = theirs
                .get(j)
                .ok_or_else(|| variant_error("list index out of range"))?;
            // The measure is the stream's own: found by the place it came
            // from.
            let id = region[index].0;
            let origin = site
                .get(id)
                .and_then(|held| held.origin)
                .ok_or_else(|| variant_error("the measure is not in the stream"))?;
            merged_into(stream, &[origin], measure, name.as_deref())?;
        }
    }
    let mut site = Site::of(stream);
    site.remove(variant);
    let (out, _) = site.finish(stream.clone());
    *stream = out;
    Ok(())
}

// ---------------------------------------------------- replacements made

/// Makes each variant of a copy of a stream that only inserts or only
/// deletes into one that replaces: an insertion at the start, or a
/// deletion anywhere but the end, takes in the element after it, and the
/// others the element before it, moving back to start there: music21's
/// `makeAllVariantsReplacements`. With `recurse`, the variants of the
/// streams the stream holds are made so, and not the stream's own, as
/// music21 does; music21's `variantNames` leaves every variant as it is,
/// so it is not taken.
///
/// # Errors
///
/// A variant with no element before it to take in, or one that cannot
/// say what it replaces.
pub fn make_all_variants_replacements(stream: &Stream, recurse: bool) -> Result<Stream> {
    let mut out = stream.clone();
    if recurse {
        let mut paths: Vec<Vec<usize>> = Vec::new();
        collect_stream_paths(&out, &mut Vec::new(), &mut paths);
        for path in paths {
            out.edit_nested(&path, |inner| match fix_variants(inner) {
                Ok(moved) => (Ok(()), moved),
                Err(error) => (Err(error), unmoved(inner)),
            })
            .0?;
        }
    } else {
        fix_variants(&mut out)?;
    }
    Ok(out)
}

/// The paths to every stream a stream holds, a stream before those it
/// holds.
fn collect_stream_paths(stream: &Stream, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    for (index, event) in stream.events().iter().enumerate() {
        if let StreamElement::Stream(inner) = event.element() {
            path.push(index);
            out.push(path.clone());
            collect_stream_paths(inner, path, out);
            path.pop();
        }
    }
}

/// The first of the measures, notes and rests of a list of elements, by
/// music21's class name.
fn first_of_kinds<'a>(
    elements: impl IntoIterator<Item = &'a StreamElement>,
) -> Option<&'static str> {
    elements
        .into_iter()
        .find(|element| {
            ["Measure", "Note", "Rest"]
                .iter()
                .any(|class| is_a(element, class))
        })
        .map(|element| classes(element)[0])
}

/// music21's `_doVariantFixingOnStream`.
fn fix_variants(stream: &mut Stream) -> Result<Vec<Option<usize>>> {
    #[derive(PartialEq)]
    enum Kind {
        Insertion,
        Deletion,
    }
    let mut site = Site::of(stream);
    for id in site.variants() {
        let variant = site.variant(id)?;
        let length = variant.replacement_quarter_length();
        let kind = match variant.length_type() {
            LengthType::Elongation if length == 0.0 => Kind::Insertion,
            LengthType::Deletion if variant.contained_highest_time() == 0.0 => Kind::Deletion,
            _ => continue,
        };
        let offset = site.offset(id)?;
        let initial = offset == 0.0;
        let last = !initial && offset + length == site.highest_time();
        let target_length;
        if (kind == Kind::Insertion && initial) || (kind == Kind::Deletion && !last) {
            let target = next_element(&site, id)?;
            let Some(target) = target else {
                return Err(variant_error("cannot append None to a Stream"));
            };
            let held = site
                .get_mut(target)
                .ok_or_else(|| variant_error("the element is not in the stream"))?;
            if let StreamElement::Stream(inner) = &mut held.element {
                let kept: Vec<(StreamEvent, Option<usize>)> = inner
                    .events()
                    .iter()
                    .enumerate()
                    .filter(|(_, event)| {
                        !matches!(
                            event.element(),
                            StreamElement::Clef(_) | StreamElement::TimeSignature(_)
                        )
                    })
                    .map(|(index, event)| (event.clone(), Some(index)))
                    .collect();
                inner.replace_events(kept);
            }
            let copy = held.element.clone();
            target_length = copy.quarter_length();
            site.variant_mut(id)?.contents.push(copy);
        } else {
            let target = previous_element(&site, id)?.ok_or_else(|| {
                variant_error("'NoneType' object has no attribute 'getOffsetBySite'")
            })?;
            let held = site
                .get(target)
                .ok_or_else(|| variant_error("the element is not in the stream"))?
                .clone();
            target_length = held.element.quarter_length();
            let variant = site.variant_mut(id)?;
            shift_contents(&mut variant.contents, target_length);
            put(&mut variant.contents, 0.0, held.element.clone());
            let moving = site
                .remove(id)
                .ok_or_else(|| variant_error("Variant not found in stream"))?;
            site.reinsert(moving, held.offset);
        }
        let variant = site.variant_mut(id)?;
        let old = variant.replacement_quarter_length();
        variant.replacement_quarter_length = Some(op_frac(old + target_length));
    }
    let (out, moved) = site.finish(stream.clone());
    *stream = out;
    Ok(moved)
}

/// music21's `_getNextElements`: the first measure, note or rest of the
/// kind the variant holds (or, for a deletion, replaces) at or after where
/// it starts (or ends).
fn next_element(site: &Site, id: usize) -> Result<Option<usize>> {
    let found = replaced(site, id, &[], false, false)?;
    let variant = site.variant(id)?;
    let offset = site.offset(id)?;
    let (class, start) = if variant.length_type() == LengthType::Elongation {
        (
            first_of_kinds(variant.contents.events().iter().map(StreamEvent::element)),
            offset,
        )
    } else {
        (
            first_of_kinds(
                found
                    .iter()
                    .filter_map(|(found_id, _)| site.get(*found_id).map(|held| &held.element)),
            ),
            offset + variant.replacement_quarter_length(),
        )
    };
    let Some(class) = class else {
        return Ok(None);
    };
    Ok(site
        .in_span(start, Some(site.highest_time()), true)
        .into_iter()
        .find(|candidate| {
            site.get(*candidate)
                .is_some_and(|held| is_a(&held.element, class))
        }))
}

/// music21's `_getPreviousElement`: the last measure, note or rest of the
/// kind the variant holds (or replaces) before it.
fn previous_element(site: &Site, id: usize) -> Result<Option<usize>> {
    let found = replaced(site, id, &[], false, false)?;
    let variant = site.variant(id)?;
    let class = if variant.length_type() == LengthType::Elongation {
        first_of_kinds(variant.contents.events().iter().map(StreamEvent::element))
    } else {
        first_of_kinds(
            found
                .iter()
                .filter_map(|(found_id, _)| site.get(*found_id).map(|held| &held.element)),
        )
    };
    let class = class
        .ok_or_else(|| variant_error("Cannot find any Measures, Notes, or Rests in variant"))?;
    let offset = site.offset(id)?;
    Ok(site
        .in_span(0.0, Some(offset), false)
        .into_iter()
        .rfind(|candidate| {
            site.get(*candidate)
                .is_some_and(|held| is_a(&held.element, class))
        }))
}

/// Moves each variant starting inside another's span to start where that
/// one does, a hidden rest put before what it holds, as music21's
/// `makeVariantBlocks` does.
///
/// music21 fails as soon as it has moved one, so this does too, the one
/// moved.
///
/// # Errors
///
/// A variant starting inside another's span, once it is moved.
pub fn make_variant_blocks(stream: &mut Stream) -> Result<()> {
    let mut site = Site::of(stream);
    let mut failed = None;
    'variants: for id in site.variants() {
        let start = site.offset(id)?;
        let end = site.variant(id)?.replacement_quarter_length() + start;
        let conflicting: Vec<usize> = site
            .in_span(start, Some(end), false)
            .into_iter()
            .filter(|other| {
                matches!(
                    site.get(*other).map(|held| &held.element),
                    Some(StreamElement::Variant(_))
                )
            })
            .collect();
        for other in conflicting {
            let other_start = site.offset(other)?;
            if other_start == start {
                continue;
            }
            let shift = op_frac(other_start - start);
            let variant = site.variant_mut(other)?;
            let old = variant.replacement_quarter_length();
            shift_contents(&mut variant.contents, shift);
            let mut rest = Rest::new(Duration::new(shift)?);
            rest.set_hidden(true);
            put(&mut variant.contents, 0.0, StreamElement::Rest(rest));
            variant.replacement_quarter_length = Some(old);
            let moving = site
                .remove(other)
                .ok_or_else(|| variant_error("Variant not found in stream"))?;
            site.reinsert(moving, start);
            failed = Some(variant_error(
                "'StreamIterator' object has no attribute 'append'",
            ));
            break 'variants;
        }
    }
    let (out, _) = site.finish(stream.clone());
    *stream = out;
    failed.map_or(Ok(()), Err)
}

// ------------------------------------------------------------ activation

/// What one variant made real leaves to renumber measures by.
enum Renumber {
    /// A measure taken out, by its number.
    Deleted(IntegerType),
    /// Measures put in after the measure of a number, by their ids.
    Inserted(Option<IntegerType>, Vec<usize>),
}

impl Renumber {
    fn key(&self) -> IntegerType {
        match self {
            Self::Deleted(number) => *number,
            Self::Inserted(prior, _) => prior.unwrap_or(-9999),
        }
    }
}

/// The variant `id` made real, everything it replaces put in a variant of
/// group `default` in its place: music21's `_insertReplacementVariant`.
fn activate_replacement(site: &mut Site, id: usize, match_by_span: bool) -> Result<()> {
    let variant = site.variant(id)?.clone();
    let start = site.offset(id)?;
    let mut removed = Variant::named("default");
    if !match_by_span {
        let mut matched = 0;
        for event in variant.contents.events() {
            // Where music21 looks: the variant's offset and the element's
            // added as Python adds them.
            let exact = Exact::held(start).plus(Exact::held(event.offset()));
            let at = exact.to_float();
            let class = classes(event.element())[0];
            let target = site.held.iter().find(|held| {
                is_a(&held.element, class)
                    && at_offset(held.offset, length_of(&held.element), exact)
            });
            let target = target.map(|held| held.id);
            if let Some(target) = target {
                matched += 1;
                let taken = site
                    .remove(target)
                    .ok_or_else(|| variant_error("the element is not in the stream"))?;
                let mut element = event.element().clone();
                if let Some(number) = measure_number(&taken.element) {
                    set_measure_number(&mut element, number);
                }
                removed.contents.push(taken.element);
                site.insert(at, element);
            }
        }
        if matched > 0 {
            site.remove(id);
            site.insert(start, removed.into());
        }
        return Ok(());
    }
    let mut deleted = VecDeque::new();
    for (target, _) in replaced(site, id, &[], false, false)? {
        let at = op_frac(site.offset(target)? - start);
        let taken = site
            .remove(target)
            .ok_or_else(|| variant_error("the element is not in the stream"))?;
        if let Some(number) = measure_number(&taken.element) {
            deleted.push_back(number);
        }
        put(&mut removed.contents, at, taken.element);
    }
    for event in variant.contents.events() {
        let mut element = event.element().clone();
        if is_measure(&element) {
            set_measure_number(&mut element, deleted.pop_front().unwrap_or(0));
        }
        site.insert(start + event.offset(), element);
    }
    site.remove(id);
    site.insert(start, removed.into());
    Ok(())
}

/// The variant `id`, shorter or longer than what it replaces, made real:
/// music21's `_insertDeletionVariant` and `_insertInsertionVariant`. The
/// answer is the measures taken out that nothing took the number of, and
/// the measures put in that took none, after the highest number before
/// them.
fn activate_changed_length(
    site: &mut Site,
    id: usize,
    insertion: bool,
) -> Result<(Vec<IntegerType>, Renumber)> {
    let variant = site.variant(id)?.clone();
    let start = site.offset(id)?;
    let mut removed = Variant::named("default");
    removed.replacement_quarter_length = Some(variant.contained_highest_time());
    let mut deleted = VecDeque::new();
    for (target, _) in replaced(site, id, &[], false, false)? {
        let at = op_frac(site.offset(target)? - start);
        let taken = site
            .remove(target)
            .ok_or_else(|| variant_error("the element is not in the stream"))?;
        if let Some(number) = measure_number(&taken.element) {
            deleted.push_back(number);
        }
        put(&mut removed.contents, at, taken.element);
    }
    let mut highest = None;
    let mut inserted = Vec::new();
    for event in variant.contents.events() {
        let mut element = event.element().clone();
        let mut new_measure = false;
        if is_measure(&element) {
            match deleted.pop_front() {
                Some(number) => {
                    set_measure_number(&mut element, number);
                    highest = Some(number);
                }
                None => {
                    set_measure_number(&mut element, 0);
                    new_measure = true;
                }
            }
        }
        let new_id = site.insert(start + event.offset(), element);
        if new_measure {
            inserted.push(new_id);
        }
    }
    if insertion && highest.is_none() {
        let mut most = 0;
        for measure in site.in_span(0.0, Some(site.offset(id)?), true) {
            if let Some(number) = site
                .get(measure)
                .and_then(|held| measure_number(&held.element))
                && number > most
            {
                most = number;
            }
        }
        highest = Some(most);
    }
    site.remove(id);
    site.insert(start, removed.into());
    Ok((
        deleted.into_iter().collect(),
        Renumber::Inserted(highest, inserted),
    ))
}

/// How far to move what stands from one offset to the next opening, and
/// what to move only as far as the openings before.
struct Opening<'a> {
    shift: FloatType,
    end: FloatType,
    include_end: bool,
    exempt: &'a Vec<usize>,
    exempt_shift: FloatType,
}

/// Moves elements back over gaps (`remove`) or on to open them, as
/// music21's `_removeOrExpandGaps` does: each gap a start, a length and
/// elements left where they are.
fn shift_gaps(site: &mut Site, gaps: &[(FloatType, FloatType, Vec<usize>)], remove: bool) {
    let duration = site.highest_time();
    let mut sorted: Vec<&(FloatType, FloatType, Vec<usize>)> = gaps.iter().collect();
    sorted.sort_by(|left, right| left.0.total_cmp(&right.0));
    let bounds = |i: usize| -> (FloatType, bool) {
        match sorted.get(i + 1) {
            Some(next) => (next.0, false),
            None => (duration, true),
        }
    };
    if remove {
        let mut shift = 0.0;
        for (i, (start, amount, exempt)) in sorted.iter().enumerate() {
            let (end, include_end) = bounds(i);
            shift += amount;
            for id in site.in_span(start + amount, Some(end), include_end) {
                if exempt.contains(&id) {
                    continue;
                }
                if let Some(held) = site.get_mut(id) {
                    held.offset = op_frac(held.offset - shift);
                }
            }
        }
    } else {
        let mut shift = 0.0;
        // music21 keys them by start, a later one replacing an earlier.
        let mut shifts: Vec<(FloatType, Opening<'_>)> = Vec::new();
        for (i, (start, amount, exempt)) in sorted.iter().enumerate() {
            let (end, include_end) = bounds(i);
            let exempt_shift = shift;
            shift += amount;
            let entry = Opening {
                shift,
                end,
                include_end,
                exempt,
                exempt_shift,
            };
            match shifts.iter_mut().find(|(known, _)| known == start) {
                Some((_, held)) => *held = entry,
                None => shifts.push((*start, entry)),
            }
        }
        shifts.sort_by(|left, right| right.0.total_cmp(&left.0));
        for (offset, opening) in shifts {
            for id in site.in_span(offset, Some(opening.end), opening.include_end) {
                let by = if opening.exempt.contains(&id) {
                    opening.exempt_shift
                } else {
                    opening.shift
                };
                if let Some(held) = site.get_mut(id) {
                    held.offset = op_frac(held.offset + by);
                }
            }
        }
    }
    site.sort();
}

/// music21's `_fixMeasureNumbers`.
fn fix_measure_numbers(site: &mut Site, mut all: Vec<Renumber>) -> Result<()> {
    if all.is_empty() {
        return Ok(());
    }
    all.sort_by_key(Renumber::key);
    let mut old_measures = site.measures();
    let mut new_measures: Vec<usize> = Vec::new();
    let mut cumulative: IntegerType = 0;
    let mut old_corrections: Vec<(IntegerType, IntegerType)> = Vec::new();
    let mut new_corrections: Vec<(IntegerType, IntegerType)> = Vec::new();
    let correct =
        |corrections: &mut Vec<(IntegerType, IntegerType)>, at: IntegerType, by: IntegerType| {
            match corrections.iter_mut().find(|(known, _)| *known == at) {
                Some((_, held)) => *held = by,
                None => corrections.push((at, by)),
            }
        };
    for entry in all {
        match entry {
            Renumber::Inserted(prior, measures) => {
                if measures.is_empty() {
                    continue;
                }
                let prior = prior.ok_or_else(|| {
                    variant_error("unsupported operand type(s) for +: 'NoneType' and 'int'")
                })?;
                cumulative += measures.len() as IntegerType;
                let mut next = prior + 1;
                for id in measures {
                    // music21 takes out nothing for a measure no longer here.
                    old_measures.retain(|known| *known != id);
                    new_measures.push(id);
                    if let Some(held) = site.get_mut(id) {
                        set_measure_number(&mut held.element, next);
                    }
                    next += 1;
                }
                correct(&mut old_corrections, prior + 1, cumulative);
                correct(&mut new_corrections, next, cumulative);
            }
            Renumber::Deleted(number) => {
                cumulative -= 1;
                correct(&mut old_corrections, number + 1, cumulative);
                correct(&mut new_corrections, number + 1, cumulative);
            }
        }
    }
    for (measures, corrections) in [
        (old_measures, old_corrections),
        (new_measures, new_corrections),
    ] {
        let mut corrections = corrections;
        corrections.sort_by_key(|(at, _)| -*at);
        let mut previous: Option<IntegerType> = None;
        for (at, shift) in corrections {
            for &id in &measures {
                let Some(held) = site.get_mut(id) else {
                    continue;
                };
                let Some(number) = measure_number(&held.element) else {
                    continue;
                };
                if previous.is_none_or(|boundary| number < boundary) && number >= at {
                    set_measure_number(&mut held.element, number + shift);
                }
            }
            previous = Some(at);
        }
    }
    Ok(())
}

impl Stream {
    /// A copy of the stream with its variants of `group` (or all of them)
    /// made its reading, as music21's `activateVariants` makes them: what
    /// each replaces goes into a variant of group `default` in its place, a
    /// longer or shorter one moving what follows, and the measures are
    /// numbered again. With `match_by_span`, a variant replaces everything
    /// of its classes within its span; without, each of its elements the
    /// first of its class where it starts.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    /// use music21_rs::variant::merge_variants;
    ///
    /// let first = from_tiny_notation("4/4 c1 d1 e1")?;
    /// let second = from_tiny_notation("4/4 c1 f1 e1")?;
    /// let merged = merge_variants(&first, &second, "second")?;
    /// let made = merged.activate_variants(Some("second"), true)?;
    /// assert_eq!(made.pitches()[1].name(), "F");
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A variant that cannot say what it replaces, or measures put in after
    /// none to be numbered from, where music21 fails.
    pub fn activate_variants(&self, group: Option<&str>, match_by_span: bool) -> Result<Stream> {
        let mut site = Site::of(self);
        let (mut elongations, mut deletions) = (Vec::new(), Vec::new());
        for id in site.variants() {
            let variant = site.variant(id)?;
            if group.is_some_and(|group| !variant.groups.iter().any(|known| known == group)) {
                continue;
            }
            match variant.length_type() {
                LengthType::Elongation => elongations.push(id),
                LengthType::Deletion => deletions.push(id),
                LengthType::Replacement => activate_replacement(&mut site, id, match_by_span)?,
            }
        }
        let mut deleted: Vec<Renumber> = Vec::new();
        let mut inserted: Vec<Renumber> = Vec::new();
        let mut gaps = Vec::new();
        for id in deletions {
            let variant = site.variant(id)?;
            let difference =
                variant.replacement_quarter_length() - variant.contained_highest_time();
            let start = site.offset(id)? + variant.contained_highest_time();
            let (left, renumber) = activate_changed_length(&mut site, id, false)?;
            gaps.push((start, difference, Vec::new()));
            deleted.extend(left.into_iter().map(Renumber::Deleted));
            inserted.push(renumber);
        }
        shift_gaps(&mut site, &gaps, true);
        let mut openings = Vec::new();
        for &id in &elongations {
            let variant = site.variant(id)?;
            let difference =
                variant.replacement_quarter_length() - variant.contained_highest_time();
            let start = site.offset(id)? + variant.replacement_quarter_length();
            openings.push((start, -difference, vec![id]));
        }
        shift_gaps(&mut site, &openings, false);
        for id in elongations {
            let (left, renumber) = activate_changed_length(&mut site, id, true)?;
            inserted.push(renumber);
            deleted.extend(left.into_iter().map(Renumber::Deleted));
        }
        deleted.extend(inserted);
        fix_measure_numbers(&mut site, deleted)?;
        Ok(site.finish(self.clone()).0)
    }

    /// A copy of a score with, for each of `variant_groups` whose variants
    /// replace something in the part at `part` among its parts, a part
    /// added showing only those variants: the part's notes and rests made
    /// hidden rests and its variants of that group made its reading, as
    /// music21's `showVariantAsOssialikePart` does.
    ///
    /// # Errors
    ///
    /// No part at `part`, or as [`Stream::activate_variants`].
    pub fn show_variant_as_ossialike_part(
        &self,
        part: usize,
        variant_groups: &[&str],
    ) -> Result<Stream> {
        let mut out = self.clone();
        let source = self
            .parts()
            .get(part)
            .map(|found| (*found).clone())
            .ok_or_else(|| variant_error("Could not find the part in the score"))?;
        for group in variant_groups {
            let mut expressed = false;
            // Each element kept with the order it went in: a rest put in
            // place of a note after everything already there.
            let mut held: Vec<(usize, StreamEvent, Option<usize>)> = Vec::new();
            let mut later = source.events().len();
            for (index, event) in source.events().iter().enumerate() {
                match event.element() {
                    StreamElement::Variant(variant) => {
                        if !variant.groups.iter().any(|known| known == group)
                            || matches!(
                                variant.length_type(),
                                LengthType::Elongation | LengthType::Deletion
                            )
                        {
                            continue;
                        }
                        expressed = true;
                        held.push((index, event.clone(), Some(index)));
                    }
                    element if is_general_note(element) => {
                        let mut rest = Rest::new(Duration::new(element.quarter_length())?);
                        rest.set_hidden(true);
                        held.push((
                            later,
                            StreamEvent::new(event.offset(), StreamElement::Rest(rest)),
                            None,
                        ));
                        later += 1;
                    }
                    StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                        let length = highest_time(measure);
                        let mut emptied = (**measure).clone();
                        let kept: Vec<(StreamEvent, Option<usize>)> = emptied
                            .events()
                            .iter()
                            .enumerate()
                            .filter(|(_, inner)| !is_general_note(inner.element()))
                            .map(|(at, inner)| (inner.clone(), Some(at)))
                            .collect();
                        emptied.replace_events(kept);
                        let mut rest = Rest::new(Duration::new(length)?);
                        rest.set_hidden(true);
                        put(&mut emptied, 0.0, StreamElement::Rest(rest));
                        held.push((
                            index,
                            StreamEvent::new(
                                event.offset(),
                                StreamElement::Stream(Box::new(emptied)),
                            ),
                            Some(index),
                        ));
                    }
                    _ => held.push((index, event.clone(), Some(index))),
                }
            }
            held.sort_by(|left, right| {
                crate::makenotation::event_order(&left.1, &right.1).then(left.0.cmp(&right.0))
            });
            let mut new_part = source.clone();
            new_part.replace_events(
                held.into_iter()
                    .map(|(_, event, origin)| (event, origin))
                    .collect(),
            );
            let new_part = new_part.activate_variants(Some(group), true)?;
            if expressed {
                out.insert_sorted(vec![StreamEvent::new(
                    0.0,
                    StreamElement::Stream(Box::new(new_part)),
                )]);
            }
        }
        Ok(out)
    }
}
