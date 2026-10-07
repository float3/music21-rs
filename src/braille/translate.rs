//! Scores, parts and measures in braille: music21's `braille.translate`.

use super::basic::word_to_braille;
use super::lookup;
use super::segment::{BrailleGrandSegment, Hand, SegmentOptions, find_segments};
use crate::{
    error::{Error, Result},
    metadata::Metadata,
    stream::{Stream, StreamElement, StreamKind},
};

/// How music21's braille translation is asked to write: its keyword
/// arguments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrailleOptions {
    /// Whether the stream is written as it stands, or first given the
    /// notation it leaves unsaid as music21's `makeNotation` gives it.
    pub in_place: bool,
    /// Whether each segment is written as music21's English listing of its
    /// groupings instead of braille.
    pub debug: bool,
    /// The options each segment is written with.
    pub segment: SegmentOptions,
}

/// A score, part or measure in braille, as music21's `objectToBraille`
/// writes one: a score part by part, two piano staves side by side, a part
/// segment by segment.
///
/// # Errors
///
/// A stream of another kind, or music that music21 cannot write.
pub fn stream_to_braille(stream: &Stream, options: &BrailleOptions) -> Result<String> {
    match stream.kind() {
        StreamKind::Part | StreamKind::PartStaff => part_to_braille(stream, options),
        StreamKind::Measure => measure_to_braille(stream, options),
        StreamKind::Score => score_to_braille(stream, options),
        StreamKind::Opus => {
            let scores: Vec<String> = stream
                .events()
                .iter()
                .filter_map(|event| event.element().as_stream())
                .filter(|inner| inner.kind() == StreamKind::Score)
                .map(|score| score_to_braille(score, options))
                .collect::<Result<_>>()?;
            Ok(scores.join("\n\n"))
        }
        _ => {
            let staves: Vec<&Stream> = stream
                .events()
                .iter()
                .filter_map(|event| event.element().as_stream())
                .filter(|inner| inner.kind() == StreamKind::PartStaff)
                .collect();
            if staves.len() == 2 {
                score_to_braille(stream, options)
            } else {
                Err(Error::Notation(
                    "Stream cannot be translated to Braille.".to_string(),
                ))
            }
        }
    }
}

/// A score in braille: its metadata, then each part, a piano's two staves
/// together: music21's `scoreToBraille`.
///
/// # Errors
///
/// Music music21 cannot write.
pub fn score_to_braille(score: &Stream, options: &BrailleOptions) -> Result<String> {
    let mut lines = Vec::new();
    if let Some(metadata) = score.metadata() {
        lines.push(metadata_to_string(metadata, !options.debug)?);
    }
    let mut pending: Option<Stream> = None;
    let mut first_leaf = 0;
    for event in score.events() {
        let leaves = match event.element() {
            StreamElement::Stream(inner) => inner.leaves().len(),
            _ => 1,
        };
        let start = first_leaf;
        first_leaf += leaves;
        let Some(part) = event.element().as_stream() else {
            continue;
        };
        let part = &with_own_spanners(score, part, start);
        match part.kind() {
            StreamKind::PartStaff => {
                if let Some(upper) = pending.take() {
                    lines.push(keyboard_parts_to_braille(&upper, part, options)?);
                } else {
                    pending = Some(part.clone());
                }
            }
            StreamKind::Part => {
                if let Some(alone) = pending.take() {
                    lines.push(part_to_braille(&alone, options)?);
                }
                lines.push(part_to_braille(part, options)?);
            }
            _ => {}
        }
    }
    if let Some(alone) = pending {
        lines.push(part_to_braille(&alone, options)?);
    }
    Ok(lines.join("\n"))
}

/// A part of a score with the spanners music21 keeps in it: those of the
/// score that start and end among the part's elements, braille reading a
/// slur by its ends alone. music21 keeps every spanner of a part written on
/// several staves on its first staff, so a later staff has none. `start` is
/// where the part's leaves start among the score's.
fn with_own_spanners(score: &Stream, part: &Stream, start: usize) -> Stream {
    let mut part = part.clone();
    let later_staff = part.kind() == StreamKind::PartStaff
        && part
            .id()
            .and_then(|id| id.rsplit_once("-Staff"))
            .is_some_and(|(_, number)| number != "1");
    if later_staff {
        return part;
    }
    let end = start + part.leaves().len();
    let inside = |place: Option<&Option<usize>>| {
        place.is_some_and(|place| place.is_some_and(|place| (start..end).contains(&place)))
    };
    for spanner in score.spanners() {
        let places = spanner.spanned();
        if inside(places.first()) && inside(places.last()) {
            let mut own = spanner.clone();
            let moved: Vec<Option<usize>> =
                (0..end).map(|place| place.checked_sub(start)).collect();
            own.move_places(&moved);
            part.add_spanner(own);
        }
    }
    part
}

/// A score's metadata, a line each sorted, as `Name: value` or its braille:
/// music21's `metadataToString`.
///
/// # Errors
///
/// A character braille has no sign for.
pub fn metadata_to_string(metadata: &Metadata, braille: bool) -> Result<String> {
    let mut lines = Vec::new();
    for (unique_name, value) in metadata.all() {
        if unique_name == "software" {
            continue;
        }
        let Some(namespace) = crate::metadata::namespace_name(unique_name) else {
            continue;
        };
        if namespace.starts_with("m21FileInfo:") {
            continue;
        }
        let words = split_camel_case(unique_name);
        let mut line = format!("{}: {}", title_case(&words.join(" ")), value.text());
        if braille {
            let written: Vec<String> = line
                .split_whitespace()
                .map(|word| word_to_braille(word, false))
                .collect::<Result<_>>()?;
            line = written.join(&lookup::alphabet(' ').unwrap_or_default());
        }
        lines.push(line);
    }
    lines.sort();
    Ok(lines.join("\n"))
}

/// The words of a camel-cased name, as music21 finds them with
/// `([A-Z]*[a-z]+)`.
fn split_camel_case(name: &str) -> Vec<String> {
    let characters: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let start = index;
        let mut upper_end = index;
        while upper_end < characters.len() && characters[upper_end].is_ascii_uppercase() {
            upper_end += 1;
        }
        let mut lower_end = upper_end;
        while lower_end < characters.len() && characters[lower_end].is_ascii_lowercase() {
            lower_end += 1;
        }
        if lower_end > upper_end {
            words.push(characters[start..lower_end].iter().collect());
            index = lower_end;
        } else {
            index = start + 1;
        }
    }
    words
}

/// Text in title case as Python's `str.title` writes it: each run of
/// letters capitalised.
fn title_case(text: &str) -> String {
    let mut out = String::new();
    let mut previous_letter = false;
    for character in text.chars() {
        if character.is_alphabetic() {
            if previous_letter {
                out.extend(character.to_lowercase());
            } else {
                out.extend(character.to_uppercase());
            }
            previous_letter = true;
        } else {
            out.push(character);
            previous_letter = false;
        }
    }
    out
}

/// A copy of a part with the notation it leaves unsaid made, as music21's
/// `makeNotation(cautionaryNotImmediateRepeat=False)` makes it.
fn notated(part: &Stream, options: &BrailleOptions) -> Result<Stream> {
    let mut part = part.clone();
    if !options.in_place {
        crate::makenotation::make_part_notation_keeping_spanners(&mut part, false)?;
    }
    Ok(part)
}

/// A part in braille, segment by segment: music21's `partToBraille`.
///
/// # Errors
///
/// Music music21 cannot write.
pub fn part_to_braille(part: &Stream, options: &BrailleOptions) -> Result<String> {
    let part = notated(part, options)?;
    let mut segments = find_segments(&part, None, options.segment);
    let mut written = Vec::new();
    for segment in &mut segments {
        let braille = segment.transcribe()?;
        written.push(if options.debug {
            segment.english()
        } else {
            braille
        });
    }
    Ok(written.join("\n"))
}

/// A measure in braille, as the one measure of a part with no heading or
/// measure number unless asked: music21's `measureToBraille`.
///
/// # Errors
///
/// Music music21 cannot write.
pub fn measure_to_braille(measure: &Stream, options: &BrailleOptions) -> Result<String> {
    let mut measure = measure.clone();
    if !options.in_place {
        crate::makenotation::make_part_notation_keeping_spanners(&mut measure, false)?;
    }
    let mut part = Stream::with_kind(StreamKind::Part);
    part.push(measure);
    part_to_braille(
        &part,
        &BrailleOptions {
            in_place: true,
            ..*options
        },
    )
}

/// A piano's two staves in braille, side by side measure by measure:
/// music21's `keyboardPartsToBraille`.
///
/// # Errors
///
/// Music music21 cannot write.
pub fn keyboard_parts_to_braille(
    upper: &Stream,
    lower: &Stream,
    options: &BrailleOptions,
) -> Result<String> {
    let upper = notated(upper, options)?;
    let lower = notated(lower, options)?;
    let right = find_segments(&upper, Some(Hand::Right), options.segment);
    let left = find_segments(&lower, Some(Hand::Left), options.segment);
    let mut written = Vec::new();
    for (right, left) in right.into_iter().zip(left) {
        let mut grand = BrailleGrandSegment {
            groupings: right.groupings.into_iter().chain(left.groupings).collect(),
            line_length: options.segment.max_line_length,
            right_contexts: right.contexts,
            left_contexts: left.contexts,
        };
        let braille = grand.transcribe()?;
        written.push(if options.debug {
            grand.english()
        } else {
            braille
        });
    }
    Ok(written.join("\n"))
}

/// An element that is not a stream, in braille as the one element of a
/// measure: music21's `objectToBraille` given one.
///
/// # Errors
///
/// Music music21 cannot write.
pub fn element_to_braille(element: StreamElement, options: &BrailleOptions) -> Result<String> {
    if element.as_stream().is_some() {
        return Err(Error::Notation(
            "Stream cannot be translated to Braille.".to_string(),
        ));
    }
    let mut measure = Stream::with_kind(StreamKind::Measure);
    measure.push(element);
    let mut part = Stream::with_kind(StreamKind::Part);
    part.push(measure);
    let mut segment = options.segment;
    segment.show_first_measure_number = false;
    segment.show_heading = false;
    part_to_braille(
        &part,
        &BrailleOptions {
            in_place: true,
            segment,
            ..*options
        },
    )
}
