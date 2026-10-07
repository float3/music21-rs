//! Parts cut into overlapping stretches of text, and every stretch of some
//! scores compared with every other: music21's `search.segment`.

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    stream::{Stream, StreamElement},
};

use super::{Searched, Translation, difflib, recursed, translate_stream_to_string_no_rhythm};

/// A part cut into stretches of its translated notes, and the measures
/// each stretch runs from and to: one entry of music21's score index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segments {
    /// Each stretch of text.
    pub segments: Vec<String>,
    /// The measure of each stretch's first and last character.
    pub measures: Vec<(Option<IntegerType>, Option<IntegerType>)>,
}

/// The notes of a stream in the order music21's `recurse().notes.stream()`
/// leaves them: by offset in the stream, then by class, grace notes first,
/// then as the walk met them -- so the notes of two voices interleave.
fn sorted_notes(stream: &Stream) -> Vec<Searched<'_>> {
    let mut notes: Vec<Searched<'_>> = recursed(stream)
        .into_iter()
        .filter(|found| {
            matches!(
                found.element,
                StreamElement::Note(_)
                    | StreamElement::Chord(_)
                    | StreamElement::Unpitched(_)
                    | StreamElement::PercussionChord(_)
                    | StreamElement::ChordSymbol(_)
            )
        })
        .collect();
    let grace = |element: &StreamElement| {
        element
            .duration()
            .is_some_and(crate::duration::Duration::is_grace)
    };
    notes.sort_by(|left, right| {
        left.offset
            .total_cmp(&right.offset)
            .then_with(|| {
                left.element
                    .class_sort_order()
                    .cmp(&right.element.class_sort_order())
            })
            .then_with(|| grace(right.element).cmp(&grace(left.element)))
    });
    notes
}

/// A part's notes, written by `translate`, cut into stretches
/// `segment_length` characters long, each starting `segment_length -
/// overlap` after the one before, the last as long as what is left:
/// music21's `translateMonophonicPartToSegments`. Each stretch's measures
/// are read off the translation's measures at its first and last
/// character, as music21 reads them, which is a note's measure only where
/// the translation writes one character a note, as
/// [`translate_stream_to_string_no_rhythm`], music21's default, does.
///
/// music21's `jitter`, which moves each start by a random amount, is not
/// offered.
///
/// # Errors
///
/// An overlap as long as the stretches, or a translation with fewer
/// measures than characters, which music21 cannot index either.
pub fn translate_monophonic_part_to_segments(
    part: &Stream,
    segment_length: usize,
    overlap: usize,
    translate: fn(&[Searched<'_>]) -> Translation,
) -> Result<Segments> {
    let step = segment_length
        .checked_sub(overlap)
        .filter(|step| *step > 0)
        .ok_or_else(|| Error::Search("the overlap must be shorter than a segment".to_string()))?;
    let notes = sorted_notes(part);
    let translation = translate(&notes);
    let text: Vec<char> = translation.text.chars().collect();
    let total = text.len();
    let count = total.div_ceil(step);
    let measure_at = |index: usize| -> Result<Option<IntegerType>> {
        translation
            .measures
            .get(index)
            .copied()
            .ok_or_else(|| Error::Search("list index out of range".to_string()))
    };
    let mut segments = Segments {
        segments: Vec::with_capacity(count),
        measures: Vec::with_capacity(count),
    };
    for number in 0..count {
        let start = (number * step).min(total - 1);
        let end = (start + segment_length).min(total);
        segments.segments.push(text[start..end].iter().collect());
        segments
            .measures
            .push((measure_at(start)?, measure_at(end - 1)?));
    }
    Ok(segments)
}

/// Each part of a score cut into segments, by music21's defaults --
/// thirty characters a segment, twelve overlapping, pitches alone:
/// music21's `indexScoreParts`.
///
/// # Errors
///
/// As [`translate_monophonic_part_to_segments`].
pub fn index_score_parts(score: &Stream) -> Result<Vec<Segments>> {
    index_score_parts_with(score, 30, 12, translate_stream_to_string_no_rhythm)
}

/// The same, cut as asked.
///
/// # Errors
///
/// As [`translate_monophonic_part_to_segments`].
pub fn index_score_parts_with(
    score: &Stream,
    segment_length: usize,
    overlap: usize,
    translate: fn(&[Searched<'_>]) -> Translation,
) -> Result<Vec<Segments>> {
    score
        .parts()
        .into_iter()
        .map(|part| translate_monophonic_part_to_segments(part, segment_length, overlap, translate))
        .collect()
}

/// Where a segment comes from in an index of scores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentAt {
    /// The score's name in the index.
    pub score: String,
    /// The part, counted from nought.
    pub part: usize,
    /// The segment of the part, counted from nought.
    pub segment: usize,
    /// The measures the segment runs from and to.
    pub measures: (Option<IntegerType>, Option<IntegerType>),
}

/// How alike two segments are: one row of music21's `scoreSimilarity`.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentSimilarity {
    /// The segment compared.
    pub this: SegmentAt,
    /// The segment it is compared with.
    pub that: SegmentAt,
    /// Python's `difflib` ratio of the two.
    pub ratio: crate::defaults::FloatType,
}

/// Every segment of an index of named scores at least `minimum_length`
/// characters long, compared with every such segment of each score after
/// its own: music21's `scoreSimilarity`, by `difflib`. A score's segments
/// are not compared with each other, as music21 compares none. With `include_reverse`, each comparison comes again with
/// its two segments the other way round.
pub fn score_similarity(
    index: &[(String, Vec<Segments>)],
    minimum_length: usize,
    include_reverse: bool,
) -> Vec<SegmentSimilarity> {
    let long_enough = |segment: &str| segment.chars().count() >= minimum_length;
    let at = |score: &str, part: usize, segment: usize, segments: &Segments| SegmentAt {
        score: score.to_string(),
        part,
        segment,
        measures: segments.measures[segment],
    };
    let mut rows = Vec::new();
    for (this_number, (this_name, this_score)) in index.iter().enumerate() {
        for (this_part, this_segments) in this_score.iter().enumerate() {
            for (this_segment, this_text) in this_segments.segments.iter().enumerate() {
                if !long_enough(this_text) {
                    continue;
                }
                let this_chars: Vec<char> = this_text.chars().collect();
                let this_at = at(this_name, this_part, this_segment, this_segments);
                for (that_name, that_score) in &index[this_number + 1..] {
                    for (that_part, that_segments) in that_score.iter().enumerate() {
                        for (that_segment, that_text) in that_segments.segments.iter().enumerate() {
                            if !long_enough(that_text) {
                                continue;
                            }
                            let that_chars: Vec<char> = that_text.chars().collect();
                            // music21 sets this segment as difflib's second
                            // sequence and each other as its first.
                            let ratio = difflib::ratio(&that_chars, &this_chars);
                            let that_at = at(that_name, that_part, that_segment, that_segments);
                            rows.push(SegmentSimilarity {
                                this: this_at.clone(),
                                that: that_at.clone(),
                                ratio,
                            });
                            if include_reverse {
                                rows.push(SegmentSimilarity {
                                    this: that_at,
                                    that: this_at.clone(),
                                    ratio,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn a_part_is_cut_into_overlapping_segments() -> Result<()> {
        // music21: translateMonophonicPartToSegments of the same line, with
        // segmentLengths=4 and overlap=2.
        let line = from_tiny_notation("4/4 c4 d e f g a b c' d' e' f' g'")?;
        let cut = translate_monophonic_part_to_segments(
            &line,
            4,
            2,
            translate_stream_to_string_no_rhythm,
        )?;
        assert_eq!(cut.segments, ["<>@A", "@ACE", "CEGH", "GHJL", "JLMO", "MO"]);
        assert_eq!(
            cut.measures,
            [
                (Some(1), Some(1)),
                (Some(1), Some(2)),
                (Some(2), Some(2)),
                (Some(2), Some(3)),
                (Some(3), Some(3)),
                (Some(3), Some(3)),
            ]
        );
        assert!(
            translate_monophonic_part_to_segments(
                &line,
                4,
                4,
                translate_stream_to_string_no_rhythm
            )
            .is_err()
        );
        Ok(())
    }
}
