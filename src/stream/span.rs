//! The elements standing in a span of offsets, or answering to a class:
//! music21's `getElementsByOffset`, `getElementsByClass`,
//! `getElementsNotOfClass` and `hasElementOfClass`.

use super::exact::Exact;
use super::{Stream, StreamEvent};
use crate::defaults::FloatType;

/// A span of offsets to search, and which elements on its edges the search
/// takes: music21's `getElementsByOffset` and the keywords it reads.
///
/// [`Self::at`] searches one offset and [`Self::between`] a stretch. The
/// four flags start as music21's own defaults, so a span built by either
/// answers what music21 answers with no keywords given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffsetSpan {
    /// Where the span starts: `offsetStart`.
    pub start: FloatType,
    /// Where it ends, or nothing for a span of no length at `start`:
    /// `offsetEnd`.
    pub end: Option<FloatType>,
    /// Whether an element starting exactly where the span ends is taken:
    /// `includeEndBoundary`.
    pub include_end_boundary: bool,
    /// Whether an element sounding past the end of the span is left out:
    /// `mustFinishInSpan`.
    pub must_finish_in_span: bool,
    /// Whether an element starting before the span is left out:
    /// `mustBeginInSpan`.
    pub must_begin_in_span: bool,
    /// Whether an element ending exactly where the span starts is taken:
    /// `includeElementsThatEndAtStart`.
    pub include_elements_that_end_at_start: bool,
}

impl OffsetSpan {
    /// One offset, with music21's defaults: a span of no length, which
    /// takes every element sounding across it and every mark standing on
    /// it.
    pub fn at(offset: FloatType) -> Self {
        Self {
            start: offset,
            end: None,
            include_end_boundary: true,
            must_finish_in_span: false,
            must_begin_in_span: true,
            include_elements_that_end_at_start: true,
        }
    }

    /// A stretch from one offset to another, with music21's defaults.
    pub fn between(start: FloatType, end: FloatType) -> Self {
        Self {
            end: Some(end),
            ..Self::at(start)
        }
    }

    /// Where the span ends, as music21 holds it: snapped with `opFrac`,
    /// and for a span of no length the snapped start itself, so that a
    /// float a hair off the fraction an element stands at still finds it.
    fn end_held(&self) -> Exact {
        Exact::held(self.end.unwrap_or(self.start))
    }

    /// Whether the span has no length, so that every mark standing in it
    /// is taken however long it lasts: music21's `zeroLengthSearch`.
    fn is_one_offset(&self) -> bool {
        self.end.is_none_or(|end| end <= self.start)
    }
}

impl Stream {
    /// The elements standing in a span of offsets: music21's
    /// `getElementsByOffset`.
    ///
    /// Everything starting after the span ends is out, and so is
    /// everything finishing before it starts. What touches an edge is
    /// decided by the span's four flags, and a mark of no length standing
    /// anywhere in a span of no length is always taken.
    ///
    /// **An element is only found where it begins**, because music21's
    /// `mustBeginInSpan` starts true: a half note at nought is not found
    /// at the offset halfway through it, though it is still sounding
    /// there. Setting `must_begin_in_span` false is what reaches back to
    /// it, and is how music21 asks what sounds at a point.
    ///
    /// Offsets are compared as music21 compares them, a snapped fraction
    /// against a float, so an element at a third of a quarter is found by
    /// the float that names it.
    ///
    /// ```
    /// use music21_rs::stream::OffsetSpan;
    /// use music21_rs::{Duration, Note, Stream, StreamKind};
    ///
    /// let mut measure = Stream::with_kind(StreamKind::Measure);
    /// measure.insert(0.0, Note::from_name("C4")?.with_duration(Duration::half()));
    /// measure.insert(2.0, Note::from_name("E4")?.with_duration(Duration::half()));
    ///
    /// // Nothing begins halfway through the first note.
    /// assert!(measure.elements_in_span(&OffsetSpan::at(1.0)).is_empty());
    ///
    /// // What sounds there is asked for by reaching back.
    /// let mut sounding = OffsetSpan::at(1.0);
    /// sounding.must_begin_in_span = false;
    /// let found = measure.elements_in_span(&sounding);
    /// assert_eq!(found.len(), 1);
    /// assert_eq!(found[0].offset(), 0.0);
    ///
    /// // A stretch takes what begins anywhere in it, its end included.
    /// let found = measure.elements_in_span(&OffsetSpan::between(1.0, 2.0));
    /// assert_eq!(found.len(), 1);
    /// assert_eq!(found[0].offset(), 2.0);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn elements_in_span(&self, span: &OffsetSpan) -> Vec<&StreamEvent> {
        let start = Exact::held(span.start);
        let end = span.end_held();
        let mut found = Vec::new();
        for event in &self.events {
            let offset = Exact::held(event.offset());
            if offset.cmp(end) == std::cmp::Ordering::Greater {
                // The stream is in offset order, so nothing later can be
                // in the span either: music21's `stopAfterEnd`.
                break;
            }
            if span.takes(event, offset, start, end) {
                found.push(event);
            }
        }
        found
    }

    /// The elements answering to any of these music21 class names:
    /// music21's `getElementsByClass`.
    ///
    /// ```
    /// use music21_rs::{Note, Stream, StreamKind, TimeSignature};
    ///
    /// let mut measure = Stream::with_kind(StreamKind::Measure);
    /// measure.insert(0.0, TimeSignature::new(4, 4)?);
    /// measure.insert(0.0, Note::from_name("C4")?);
    /// assert_eq!(measure.elements_of_class(&["Note"]).len(), 1);
    /// assert_eq!(measure.elements_of_class(&["GeneralNote"]).len(), 1);
    /// assert!(measure.has_element_of_class(&["TimeSignature"]));
    /// assert_eq!(measure.elements_not_of_class(&["Note"]).len(), 1);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn elements_of_class(&self, classes: &[&str]) -> Vec<&StreamEvent> {
        self.events
            .iter()
            .filter(|event| is_of_class(event, classes))
            .collect()
    }

    /// The elements answering to none of these music21 class names:
    /// music21's `getElementsNotOfClass`.
    pub fn elements_not_of_class(&self, classes: &[&str]) -> Vec<&StreamEvent> {
        self.events
            .iter()
            .filter(|event| !is_of_class(event, classes))
            .collect()
    }

    /// Whether any element answers to one of these music21 class names:
    /// music21's `hasElementOfClass`.
    pub fn has_element_of_class(&self, classes: &[&str]) -> bool {
        self.events.iter().any(|event| is_of_class(event, classes))
    }
}

/// Whether an element answers to one of the class names.
fn is_of_class(event: &StreamEvent, classes: &[&str]) -> bool {
    classes
        .iter()
        .any(|class| event.element().is_instance(class))
}

impl OffsetSpan {
    /// Whether one element standing at an offset is in the span, which is
    /// music21's `isElementOffsetInRange` once the elements past the end
    /// have been left behind.
    fn takes(&self, event: &StreamEvent, offset: Exact, start: Exact, end: Exact) -> bool {
        use std::cmp::Ordering::{Equal, Greater, Less};

        let length = event.element().quarter_length();
        // music21 snaps the sum, not the two numbers it adds.
        let element_end = Exact::held(event.offset() + length);
        if element_end.cmp(start) == Less {
            // It finishes before the span starts.
            return false;
        }
        let no_length = length == 0.0;
        if self.is_one_offset() && no_length {
            return true;
        }
        if self.must_finish_in_span {
            if element_end.cmp(end) == Greater {
                return false;
            }
            if !self.include_end_boundary && element_end.cmp(end) == Equal {
                return false;
            }
        }
        if self.must_begin_in_span {
            if offset.cmp(start) == Less {
                return false;
            }
            if !self.include_end_boundary && offset.cmp(end) == Equal {
                return false;
            }
        } else if !no_length && element_end.cmp(end) == Equal && self.is_one_offset() {
            return false;
        }
        if !self.include_end_boundary && offset.cmp(end) == Equal {
            return false;
        }
        if !self.include_elements_that_end_at_start && element_end.cmp(start) == Equal {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::note::Note;
    use crate::stream::StreamKind;

    fn measure() -> Stream {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        for (offset, name, length) in [(0.0, "C4", 2.0), (2.0, "E4", 1.0), (3.0, "G4", 1.0)] {
            let note = Note::from_name(name)
                .expect("a note name")
                .with_duration(Duration::new(length).expect("a length"));
            measure.insert(offset, note);
        }
        measure
    }

    /// Every expectation below is music21's own answer for the same
    /// measure, read off `getElementsByOffset`.
    #[test]
    fn one_offset_finds_only_what_begins_there() {
        let measure = measure();
        let offsets = |span: &OffsetSpan| -> Vec<FloatType> {
            measure
                .elements_in_span(span)
                .iter()
                .map(|event| event.offset())
                .collect()
        };
        // The note at nought is still sounding at 1, but it does not
        // begin in the span, which music21 asks for by default.
        assert_eq!(offsets(&OffsetSpan::at(1.0)), []);
        let mut sounding = OffsetSpan::at(1.0);
        sounding.must_begin_in_span = false;
        assert_eq!(offsets(&sounding), [0.0]);

        assert_eq!(offsets(&OffsetSpan::at(3.0)), [3.0]);
        assert_eq!(offsets(&OffsetSpan::at(4.0)), []);

        // Reaching back at the offset where the first note ends does not
        // find it: a span of no length refuses an element ending exactly
        // at its end.
        let mut sounding = OffsetSpan::at(2.0);
        sounding.must_begin_in_span = false;
        assert_eq!(offsets(&sounding), [2.0]);
    }

    #[test]
    fn a_stretch_reads_its_edges_as_the_flags_say() {
        let measure = measure();
        let offsets = |span: &OffsetSpan| -> Vec<FloatType> {
            measure
                .elements_in_span(span)
                .iter()
                .map(|event| event.offset())
                .collect()
        };
        assert_eq!(offsets(&OffsetSpan::between(1.0, 3.0)), [2.0, 3.0]);

        let mut no_end = OffsetSpan::between(1.0, 3.0);
        no_end.include_end_boundary = false;
        assert_eq!(offsets(&no_end), [2.0]);

        let mut reaching_back = OffsetSpan::between(1.0, 3.0);
        reaching_back.must_begin_in_span = false;
        assert_eq!(offsets(&reaching_back), [0.0, 2.0, 3.0]);

        let mut inside = OffsetSpan::between(1.0, 3.0);
        inside.must_finish_in_span = true;
        assert_eq!(offsets(&inside), [2.0]);

        assert_eq!(offsets(&OffsetSpan::between(0.0, 4.0)), [0.0, 2.0, 3.0]);

        // Reaching back over a stretch does find an element ending where
        // it starts, until that is refused.
        let mut reaching_back = OffsetSpan::between(2.0, 3.0);
        reaching_back.must_begin_in_span = false;
        assert_eq!(offsets(&reaching_back), [0.0, 2.0, 3.0]);
        reaching_back.include_elements_that_end_at_start = false;
        assert_eq!(offsets(&reaching_back), [2.0, 3.0]);

        let mut no_end = OffsetSpan::between(1.0, 3.0);
        no_end.include_end_boundary = false;
        no_end.must_begin_in_span = false;
        assert_eq!(offsets(&no_end), [0.0, 2.0]);
    }

    #[test]
    fn a_float_a_hair_off_a_fraction_still_finds_the_element() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        let note = Note::from_name("C4")
            .expect("a note name")
            .with_duration(Duration::new(1.0 / 3.0).expect("a third"));
        // Five sixths, where music21 keeps the offset as a fraction.
        measure.insert(1.0 / 3.0 + 0.5, note);
        let found = measure.elements_in_span(&OffsetSpan::at(1.0 / 3.0 + 0.5));
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_class_name_answers_for_the_classes_an_element_stands_in() {
        let measure = measure();
        assert_eq!(measure.elements_of_class(&["Note"]).len(), 3);
        assert_eq!(measure.elements_of_class(&["NotRest", "Rest"]).len(), 3);
        assert!(measure.elements_of_class(&["Clef"]).is_empty());
        assert!(!measure.has_element_of_class(&["Clef"]));
        assert!(measure.has_element_of_class(&["Clef", "Note"]));
        assert_eq!(measure.elements_not_of_class(&["Note"]).len(), 0);
        assert_eq!(measure.elements_not_of_class(&["Clef"]).len(), 3);
    }
}
