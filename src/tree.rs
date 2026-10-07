//! A stream's elements as spans of time, and the moments where what sounds
//! changes: music21's `tree` package, its timespan trees and
//! verticalities.
//!
//! [`as_timespans`] reads every element of a stream, nested ones included,
//! as an [`ElementTimespan`] from where it starts to where it ends, with the
//! streams it stands in; a [`TimespanTree`] answers which start, stop or
//! sound at an offset; and a [`Verticality`] is one such offset, with the
//! pitches sounding there.
//!
//! ```
//! use music21_rs::tinynotation::from_tiny_notation;
//! use music21_rs::tree::{Flatten, as_timespans};
//!
//! let line = from_tiny_notation("4/4 c2 e4 g")?;
//! let tree = as_timespans(&line, Flatten::Flat, None);
//! let third = tree.verticality_at(3.0);
//! assert_eq!(third.start_timespans().len(), 1);
//! assert_eq!(tree.maximum_overlap(), 3);
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::{
    chord::Chord,
    defaults::{FloatType, IntegerType},
    error::Result,
    makenotation::op_frac,
    pitch::Pitch,
    stream::{Stream, StreamElement, StreamKind},
};

/// How [`as_timespans`] reads the streams a stream holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Flatten {
    /// Their elements only: music21's `flatten=True`.
    #[default]
    Flat,
    /// Their elements and the streams themselves: music21's
    /// `flatten='semiFlat'`.
    SemiFlat,
}

/// One element of a stream, from where it starts to where it ends:
/// music21's `ElementTimespan`, and its `PitchedTimespan` where the element
/// is a note-like one or a stream.
#[derive(Clone, Debug)]
pub struct ElementTimespan<'a> {
    element: &'a StreamElement,
    offset: FloatType,
    end_time: FloatType,
    parent_offset: FloatType,
    parent_end_time: FloatType,
    parentage: Vec<&'a Stream>,
    measure: Option<IntegerType>,
    pitched: bool,
}

impl<'a> ElementTimespan<'a> {
    /// The element.
    pub fn element(&self) -> &'a StreamElement {
        self.element
    }

    /// Where it starts in the stream read.
    pub fn offset(&self) -> FloatType {
        self.offset
    }

    /// Where it ends: music21's `endTime`.
    pub fn end_time(&self) -> FloatType {
        self.end_time
    }

    /// How long it lasts.
    pub fn quarter_length(&self) -> FloatType {
        self.end_time - self.offset
    }

    /// Where the stream holding it starts: music21's `parentOffset`.
    pub fn parent_offset(&self) -> FloatType {
        self.parent_offset
    }

    /// Where the stream holding it ends: music21's `parentEndTime`.
    pub fn parent_end_time(&self) -> FloatType {
        self.parent_end_time
    }

    /// The streams it stands in, the one holding it first and the stream
    /// read last: music21's `parentage`.
    pub fn parentage(&self) -> &[&'a Stream] {
        &self.parentage
    }

    /// The part it is in, if any: music21's `part`.
    pub fn part(&self) -> Option<&'a Stream> {
        self.parentage
            .iter()
            .copied()
            .find(|stream| matches!(stream.kind(), StreamKind::Part | StreamKind::PartStaff))
    }

    /// The number of the measure it is in, if it is in one: music21's
    /// `measureNumber`.
    pub fn measure_number(&self) -> Option<IntegerType> {
        self.measure
    }

    /// Whether it is music21's `PitchedTimespan`: the element a note, chord,
    /// chord symbol, unpitched stroke, percussion chord or stream.
    pub fn is_pitched(&self) -> bool {
        self.pitched
    }

    /// The pitches the element sounds, none for one that is not pitched:
    /// music21's `pitches`.
    pub fn pitches(&self) -> Vec<Pitch> {
        if !self.pitched {
            return Vec::new();
        }
        match self.element {
            StreamElement::ChordSymbol(symbol) => symbol.pitches().unwrap_or_default(),
            element => element.pitches(),
        }
    }
}

/// Whether music21 makes a `PitchedTimespan` of an element: a `NotRest` or
/// a stream.
fn is_pitched(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::Stream(_)
    )
}

/// The timespans of a stream in time, each answer in music21's order: by
/// where they start, then where they end, then as the stream holds them:
/// music21's `TimespanTree`.
#[derive(Clone, Debug)]
pub struct TimespanTree<'a> {
    timespans: Vec<ElementTimespan<'a>>,
}

/// Every element of a stream, nested ones included, as a timespan:
/// music21's `asTimespans`. `filter` keeps only the elements it accepts, as
/// music21's `classList` does; the streams are walked into either way.
pub fn as_timespans<'a>(
    stream: &'a Stream,
    flatten: Flatten,
    filter: Option<fn(&StreamElement) -> bool>,
) -> TimespanTree<'a> {
    let mut timespans = Vec::new();
    let measure = (stream.kind() == StreamKind::Measure).then(|| stream.number());
    walk(
        stream,
        0.0,
        &mut vec![stream],
        measure,
        flatten,
        filter,
        &mut timespans,
    );
    timespans.sort_by(|a: &ElementTimespan<'_>, b: &ElementTimespan<'_>| {
        a.offset
            .total_cmp(&b.offset)
            .then_with(|| a.end_time.total_cmp(&b.end_time))
    });
    TimespanTree { timespans }
}

fn walk<'a>(
    stream: &'a Stream,
    initial: FloatType,
    parentage: &mut Vec<&'a Stream>,
    measure: Option<IntegerType>,
    flatten: Flatten,
    filter: Option<fn(&StreamElement) -> bool>,
    out: &mut Vec<ElementTimespan<'a>>,
) {
    let parent_end_time = op_frac(initial + stream.end_offset());
    let reversed: Vec<&'a Stream> = parentage.iter().rev().copied().collect();
    for event in stream.events() {
        let offset = op_frac(event.offset() + initial);
        let element = event.element();
        // A measure's own number is its measure number, as music21 reads it.
        let mut own_measure = measure;
        if let StreamElement::Stream(inner) = element {
            parentage.push(inner);
            let inner_measure = if inner.kind() == StreamKind::Measure {
                Some(inner.number())
            } else {
                measure
            };
            walk(
                inner,
                offset,
                parentage,
                inner_measure,
                flatten,
                filter,
                out,
            );
            parentage.pop();
            own_measure = inner_measure;
            if flatten == Flatten::Flat {
                continue;
            }
        }
        if filter.is_some_and(|accepts| !accepts(element)) {
            continue;
        }
        out.push(ElementTimespan {
            element,
            offset,
            end_time: op_frac(offset + element.quarter_length()),
            parent_offset: initial,
            parent_end_time,
            parentage: reversed.clone(),
            measure: own_measure,
            pitched: is_pitched(element),
        });
    }
}

/// music21's `Pitch.__le__`: lower in pitch space, or the same pitch
/// spelled the same.
fn at_most(a: &Pitch, b: &Pitch) -> bool {
    a.ps() < b.ps() || a == b
}

impl<'a> TimespanTree<'a> {
    /// Every timespan, in order.
    pub fn timespans(&self) -> &[ElementTimespan<'a>] {
        &self.timespans
    }

    /// How many timespans there are.
    pub fn len(&self) -> usize {
        self.timespans.len()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.timespans.is_empty()
    }

    /// Where the earliest timespan starts: music21's `lowestPosition`.
    pub fn lowest_position(&self) -> Option<FloatType> {
        self.timespans.first().map(|span| span.offset)
    }

    /// Where the latest starts: music21's `highestPosition`.
    pub fn highest_position(&self) -> Option<FloatType> {
        self.timespans.last().map(|span| span.offset)
    }

    /// Where the last to end ends: music21's `endTime`.
    pub fn end_time(&self) -> Option<FloatType> {
        self.timespans
            .iter()
            .map(|span| span.end_time)
            .reduce(FloatType::max)
    }

    /// Each offset a timespan starts at, once, in order: music21's
    /// `allOffsets`.
    pub fn all_offsets(&self) -> Vec<FloatType> {
        let mut offsets: Vec<FloatType> = self.timespans.iter().map(|span| span.offset).collect();
        offsets.dedup();
        offsets
    }

    /// Each offset a timespan starts or ends at, once, in order: music21's
    /// `allTimePoints`.
    pub fn all_time_points(&self) -> Vec<FloatType> {
        let mut points: Vec<FloatType> = self
            .timespans
            .iter()
            .flat_map(|span| [span.offset, span.end_time])
            .collect();
        points.sort_by(FloatType::total_cmp);
        points.dedup();
        points
    }

    /// The first offset after this one a timespan starts at: music21's
    /// `getPositionAfter`.
    pub fn position_after(&self, offset: FloatType) -> Option<FloatType> {
        self.timespans
            .iter()
            .map(|span| span.offset)
            .find(|start| *start > offset)
    }

    /// The last offset before this one a timespan starts at: music21's
    /// `getPositionBefore`.
    pub fn position_before(&self, offset: FloatType) -> Option<FloatType> {
        self.timespans
            .iter()
            .rev()
            .map(|span| span.offset)
            .find(|start| *start < offset)
    }

    /// The timespans starting at an offset: music21's `elementsStartingAt`.
    pub fn starting_at(&self, offset: FloatType) -> Vec<&ElementTimespan<'a>> {
        self.timespans
            .iter()
            .filter(|span| span.offset == offset)
            .collect()
    }

    /// The timespans ending at an offset: music21's `elementsStoppingAt`.
    pub fn stopping_at(&self, offset: FloatType) -> Vec<&ElementTimespan<'a>> {
        self.timespans
            .iter()
            .filter(|span| span.end_time == offset)
            .collect()
    }

    /// The timespans sounding across an offset, started before it and
    /// ending after: music21's `elementsOverlappingOffset`.
    pub fn overlapping(&self, offset: FloatType) -> Vec<&ElementTimespan<'a>> {
        self.timespans
            .iter()
            .filter(|span| span.offset < offset && offset < span.end_time)
            .collect()
    }

    /// The offsets a timespan starts at, or with `include_stop_points` also
    /// ends at, where another sounds across: music21's
    /// `overlapTimePoints`.
    pub fn overlap_time_points(&self, include_stop_points: bool) -> Vec<FloatType> {
        let points = if include_stop_points {
            self.all_time_points()
        } else {
            self.all_offsets()
        };
        points
            .into_iter()
            .filter(|point| !self.overlapping(*point).is_empty())
            .collect()
    }

    /// What starts, stops and sounds across an offset: music21's
    /// `getVerticalityAt`.
    pub fn verticality_at(&self, offset: FloatType) -> Verticality<'_, 'a> {
        Verticality {
            tree: self,
            offset,
            start_timespans: self.starting_at(offset),
            overlap_timespans: self.overlapping(offset),
            stop_timespans: self.stopping_at(offset),
        }
    }

    /// The verticality at an offset where something starts there, or the
    /// one before: music21's `getVerticalityAtOrBefore`.
    pub fn verticality_at_or_before(&self, offset: FloatType) -> Option<Verticality<'_, 'a>> {
        let verticality = self.verticality_at(offset);
        if verticality.start_timespans.is_empty() {
            verticality.previous()
        } else {
            Some(verticality)
        }
    }

    /// The verticality at each offset a timespan starts at, in order:
    /// music21's `iterateVerticalities`.
    pub fn verticalities(&self) -> Vec<Verticality<'_, 'a>> {
        self.all_offsets()
            .into_iter()
            .map(|offset| self.verticality_at(offset))
            .collect()
    }

    /// The most timespans starting at or sounding across any one offset a
    /// timespan starts at, nought for an empty tree: music21's
    /// `maximumOverlap`.
    pub fn maximum_overlap(&self) -> usize {
        self.verticalities()
            .iter()
            .map(|verticality| {
                verticality.start_timespans.len() + verticality.overlap_timespans.len()
            })
            .max()
            .unwrap_or(0)
    }

    /// The next timespan after this one, starting at a later offset, in the
    /// same part: music21's `findNextPitchedTimespanInSameStreamByClass`.
    pub fn next_in_same_part(
        &self,
        timespan: &ElementTimespan<'a>,
    ) -> Option<&ElementTimespan<'a>> {
        let part = timespan.part();
        let mut offset = timespan.offset;
        while let Some(next) = self.position_after(offset) {
            if let Some(found) = self
                .starting_at(next)
                .into_iter()
                .find(|span| same_stream(span.part(), part))
            {
                return Some(found);
            }
            offset = next;
        }
        None
    }

    /// The timespan before this one, starting at an earlier offset, in the
    /// same part: music21's `findPreviousPitchedTimespanInSameStreamByClass`.
    pub fn previous_in_same_part(
        &self,
        timespan: &ElementTimespan<'a>,
    ) -> Option<&ElementTimespan<'a>> {
        let part = timespan.part();
        let mut offset = timespan.offset;
        while let Some(previous) = self.position_before(offset) {
            if let Some(found) = self
                .starting_at(previous)
                .into_iter()
                .find(|span| same_stream(span.part(), part))
            {
                return Some(found);
            }
            offset = previous;
        }
        None
    }
}

/// Whether two parts are the same stream, or both are none.
fn same_stream(a: Option<&Stream>, b: Option<&Stream>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// What starts, stops and sounds across one offset of a [`TimespanTree`]:
/// music21's `Verticality`.
#[derive(Clone, Debug)]
pub struct Verticality<'t, 'a> {
    tree: &'t TimespanTree<'a>,
    offset: FloatType,
    start_timespans: Vec<&'t ElementTimespan<'a>>,
    overlap_timespans: Vec<&'t ElementTimespan<'a>>,
    stop_timespans: Vec<&'t ElementTimespan<'a>>,
}

impl<'t, 'a> Verticality<'t, 'a> {
    /// The offset.
    pub fn offset(&self) -> FloatType {
        self.offset
    }

    /// The timespans starting here: music21's `startTimespans`.
    pub fn start_timespans(&self) -> &[&'t ElementTimespan<'a>] {
        &self.start_timespans
    }

    /// The timespans sounding across: music21's `overlapTimespans`.
    pub fn overlap_timespans(&self) -> &[&'t ElementTimespan<'a>] {
        &self.overlap_timespans
    }

    /// The timespans ending here: music21's `stopTimespans`.
    pub fn stop_timespans(&self) -> &[&'t ElementTimespan<'a>] {
        &self.stop_timespans
    }

    /// Those starting, then those sounding across: music21's
    /// `startAndOverlapTimespans`.
    pub fn start_and_overlap_timespans(&self) -> Vec<&'t ElementTimespan<'a>> {
        self.start_timespans
            .iter()
            .chain(&self.overlap_timespans)
            .copied()
            .collect()
    }

    /// Each pitch sounding, each name and octave once, lowest first:
    /// music21's `pitchSet`, ordered as its `toChord` orders it.
    pub fn pitch_set(&self) -> Vec<Pitch> {
        let mut seen: Vec<String> = Vec::new();
        let mut pitches: Vec<Pitch> = Vec::new();
        for span in self.start_and_overlap_timespans() {
            for pitch in span.pitches() {
                let name = pitch.name_with_octave();
                if !seen.contains(&name) {
                    seen.push(name);
                    pitches.push(pitch);
                }
            }
        }
        pitches.sort_by(|a, b| a.ps().total_cmp(&b.ps()));
        pitches
    }

    /// Each pitch class sounding, once, in order: music21's
    /// `pitchClassSet`, by its classes.
    pub fn pitch_class_set(&self) -> Vec<u8> {
        let mut classes: Vec<u8> = self
            .pitch_set()
            .iter()
            .map(|pitch| (pitch.ps().round_ties_even() as i64).rem_euclid(12) as u8)
            .collect();
        classes.sort_unstable();
        classes.dedup();
        classes
    }

    /// A chord of the pitches sounding, lowest first: music21's `toChord`.
    ///
    /// # Errors
    ///
    /// None in practice; a chord of no pitches is an empty chord.
    pub fn to_chord(&self) -> Result<Chord> {
        Chord::new(self.pitch_set().as_slice())
    }

    /// The timespan sounding the lowest pitch, the last of those sounding it
    /// alike: music21's `bassTimespan`.
    pub fn bass_timespan(&self) -> Option<&'t ElementTimespan<'a>> {
        let mut lowest: Option<(Pitch, &'t ElementTimespan<'a>)> = None;
        for span in self.start_and_overlap_timespans() {
            let mut pitches = span.pitches();
            if pitches.is_empty() {
                continue;
            }
            pitches.sort_by(|a, b| a.ps().total_cmp(&b.ps()));
            let pitch = pitches.swap_remove(0);
            match &lowest {
                None => lowest = Some((pitch, span)),
                Some((overall, _)) if at_most(&pitch, overall) => lowest = Some((pitch, span)),
                Some(_) => {}
            }
        }
        lowest.map(|(_, span)| span)
    }

    /// The measure of the first timespan starting here: music21's
    /// `measureNumber`.
    pub fn measure_number(&self) -> Option<IntegerType> {
        self.start_timespans.first().and_then(|span| span.measure)
    }

    /// The next offset a timespan starts at: music21's `nextStartOffset`.
    pub fn next_start_offset(&self) -> Option<FloatType> {
        self.tree.position_after(self.offset)
    }

    /// The verticality there: music21's `nextVerticality`.
    pub fn next(&self) -> Option<Verticality<'t, 'a>> {
        self.next_start_offset()
            .map(|offset| self.tree.verticality_at(offset))
    }

    /// The verticality at the last offset before this one a timespan starts
    /// at: music21's `previousVerticality`.
    pub fn previous(&self) -> Option<Verticality<'t, 'a>> {
        self.tree
            .position_before(self.offset)
            .map(|offset| self.tree.verticality_at(offset))
    }

    /// How long until the next start, or the end of the tree after the
    /// last: music21's `timeToNextEvent`.
    pub fn time_to_next_event(&self) -> Option<FloatType> {
        let next = self.next_start_offset().or_else(|| self.tree.end_time())?;
        Some(op_frac(next - self.offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tinynotation::from_tiny_notation;

    #[test]
    fn a_line_is_read_as_timespans_and_verticalities() -> Result<()> {
        let line = from_tiny_notation("4/4 c2 e4 g")?;
        let tree = as_timespans(&line, Flatten::Flat, None);
        let notes: Vec<(FloatType, FloatType)> = tree
            .timespans()
            .iter()
            .filter(|span| matches!(span.element(), StreamElement::Note(_)))
            .map(|span| (span.offset(), span.end_time()))
            .collect();
        assert_eq!(notes, [(0.0, 2.0), (2.0, 3.0), (3.0, 4.0)]);
        let at_one = tree.verticality_at(1.0);
        assert!(at_one.start_timespans().is_empty());
        assert_eq!(at_one.overlap_timespans().len(), 1);
        assert_eq!(at_one.pitch_class_set(), [0]);
        assert_eq!(
            tree.verticality_at(2.0).next().map(|next| next.offset()),
            Some(3.0)
        );
        assert_eq!(tree.verticality_at(3.0).time_to_next_event(), Some(1.0));
        Ok(())
    }
}
