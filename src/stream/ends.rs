//! The barline a stream ends with, and how many lines its staff has:
//! music21's `Stream.finalBarline` and `staffLines`.

use super::{Stream, StreamElement, StreamKind};
use crate::bar::Barline;
use crate::layout::Layout;

impl Stream {
    /// The barline this stream ends with: music21's `finalBarline` for a
    /// stream that is not part-like.
    ///
    /// It is the right barline of the last measure the stream holds, or a
    /// measure's own right barline, and nothing for a stream holding no
    /// measures at all. music21 answers a *list* for a score or anything
    /// else holding part-like streams, which is [`Self::final_barlines`].
    ///
    /// ```
    /// use music21_rs::bar::{Barline, BarlineType};
    /// use music21_rs::{Stream, StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// for bar_type in [BarlineType::Double, BarlineType::Final] {
    ///     let mut measure = Stream::with_kind(StreamKind::Measure);
    ///     measure.set_right_barline(Some(Barline::new(bar_type)));
    ///     part.push(measure);
    /// }
    /// assert_eq!(
    ///     part.final_barline().map(Barline::bar_type),
    ///     Some(BarlineType::Final)
    /// );
    ///
    /// // A stream holding no measures has none, as music21 answers.
    /// assert!(Stream::new().final_barline().is_none());
    /// ```
    pub fn final_barline(&self) -> Option<&Barline> {
        if self.has_measures() {
            return self.measures().last().and_then(|last| last.right_barline());
        }
        // music21 reads a stream's own right barline only where it has one
        // to read, which is a measure.
        if self.kind() == StreamKind::Measure {
            return self.right_barline();
        }
        None
    }

    /// The barline each part-like stream ends with: music21's
    /// `finalBarline` for a score, which answers one barline per part.
    ///
    /// A stream holding no part-like streams answers nothing, where
    /// [`Self::final_barline`] answers for the stream itself.
    pub fn final_barlines(&self) -> Vec<Option<&Barline>> {
        if !self.has_part_like_streams() {
            return Vec::new();
        }
        // music21 asks every nested stream, not only the parts.
        self.events
            .iter()
            .filter_map(|event| event.element().as_stream())
            .map(Stream::final_barline)
            .collect()
    }

    /// Writes the barline the stream ends with: music21's `finalBarline`
    /// setter.
    ///
    /// On a stream holding part-like streams every part takes it, as
    /// music21's setter gives each part the one barline it was handed. On
    /// any other stream it goes on the last measure, or on a measure
    /// itself; a stream with no measures to carry it is left alone, as
    /// music21 leaves it.
    pub fn set_final_barline(&mut self, barline: Option<Barline>) {
        if self.has_part_like_streams() {
            for event in &mut self.events {
                if let StreamElement::Stream(part) = event.element_mut() {
                    part.set_final_barline(barline.clone());
                }
            }
            return;
        }
        if self.has_measures() {
            let last = self
                .events
                .iter_mut()
                .rev()
                .find_map(|event| match event.element_mut() {
                    StreamElement::Stream(stream) if stream.kind() == StreamKind::Measure => {
                        Some(stream)
                    }
                    _ => None,
                });
            if let Some(last) = last {
                last.set_right_barline(barline);
            }
            return;
        }
        if self.kind() == StreamKind::Measure {
            self.set_right_barline(barline);
        }
    }

    /// How many lines this stream's staff has: music21's `staffLines`.
    ///
    /// It is read from the first staff layout standing at the start of the
    /// stream, however deep, that says a number of lines; five where none
    /// does, which is the ordinary staff. A layout standing anywhere after
    /// the start says nothing about it, as in music21.
    ///
    /// ```
    /// use music21_rs::layout::{Layout, StaffLayout};
    /// use music21_rs::{Stream, StreamElement, StreamKind};
    ///
    /// let mut measure = Stream::with_kind(StreamKind::Measure);
    /// assert_eq!(measure.staff_lines(), 5);
    ///
    /// let staff = StaffLayout {
    ///     staff_lines: Some(1),
    ///     ..StaffLayout::default()
    /// };
    /// measure.insert(0.0, StreamElement::Layout(Layout::Staff(staff)));
    /// assert_eq!(measure.staff_lines(), 1);
    ///
    /// // A part reads the layout of the measure it starts with.
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// part.push(measure);
    /// assert_eq!(part.staff_lines(), 1);
    /// ```
    pub fn staff_lines(&self) -> i64 {
        // music21 reads the staff layouts alone, so anything else
        // standing later does not stop the search; the first layout that
        // does stops it.
        let layouts = self
            .recurse()
            .into_iter()
            .filter_map(|(offset, element)| match element {
                StreamElement::Layout(Layout::Staff(staff)) => Some((offset, staff)),
                _ => None,
            });
        for (offset, staff) in layouts {
            if offset > 0.0 {
                break;
            }
            if let Some(lines) = staff.staff_lines {
                return lines;
            }
        }
        DEFAULT_STAFF_LINES
    }
}

/// The lines an ordinary staff has, which music21 answers for a stream
/// whose layout says nothing.
const DEFAULT_STAFF_LINES: i64 = 5;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar::BarlineType;
    use crate::duration::Duration;
    use crate::layout::StaffLayout;
    use crate::note::Note;

    fn part_ending(bar_types: &[BarlineType]) -> Stream {
        let mut part = Stream::with_kind(StreamKind::Part);
        for bar_type in bar_types {
            let mut measure = Stream::with_kind(StreamKind::Measure);
            measure.set_right_barline(Some(Barline::new(*bar_type)));
            part.push(measure);
        }
        part
    }

    #[test]
    fn the_last_measures_right_barline_is_the_final_one() {
        let part = part_ending(&[BarlineType::Double, BarlineType::Final]);
        assert_eq!(
            part.final_barline().map(Barline::bar_type),
            Some(BarlineType::Final)
        );
        // A measure answers its own.
        let measure = part.measures()[0];
        assert_eq!(
            measure.final_barline().map(Barline::bar_type),
            Some(BarlineType::Double)
        );
        // A stream holding neither measures nor a barline of its own has
        // none, where music21 refused before v6.3.
        let mut loose = Stream::with_kind(StreamKind::Part);
        loose.insert(0.0, Note::from_name("C4").expect("a note name"));
        assert!(loose.final_barline().is_none());
    }

    #[test]
    fn a_score_answers_one_barline_for_each_part() {
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part_ending(&[BarlineType::Final]));
        score.push(part_ending(&[BarlineType::Double, BarlineType::Regular]));
        let types: Vec<Option<BarlineType>> = score
            .final_barlines()
            .into_iter()
            .map(|barline| barline.map(Barline::bar_type))
            .collect();
        assert_eq!(
            types,
            [Some(BarlineType::Final), Some(BarlineType::Regular)]
        );
        // The score itself holds no measures, so it has no one barline.
        assert!(score.final_barline().is_none());
        // A part answers no list, having no part-like streams.
        assert!(score.final_barlines()[0].is_some());
        assert!(
            part_ending(&[BarlineType::Final])
                .final_barlines()
                .is_empty()
        );
    }

    #[test]
    fn writing_the_final_barline_reaches_every_part() {
        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part_ending(&[BarlineType::Regular, BarlineType::Regular]));
        score.push(part_ending(&[BarlineType::Regular]));
        score.set_final_barline(Some(Barline::new(BarlineType::None)));
        let types: Vec<Option<BarlineType>> = score
            .final_barlines()
            .into_iter()
            .map(|barline| barline.map(Barline::bar_type))
            .collect();
        assert_eq!(types, [Some(BarlineType::None), Some(BarlineType::None)]);
        // The first part's earlier measure is untouched.
        assert_eq!(
            score.parts()[0].measures()[0]
                .right_barline()
                .map(Barline::bar_type),
            Some(BarlineType::Regular)
        );
    }

    /// Each answer here is music21's own for the same arrangement.
    #[test]
    fn a_staff_says_how_many_lines_it_has_from_its_start() {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        assert_eq!(measure.staff_lines(), 5);

        let staff = StaffLayout {
            staff_lines: Some(1),
            ..StaffLayout::default()
        };
        measure.insert(0.0, StreamElement::Layout(Layout::Staff(staff.clone())));
        assert_eq!(measure.staff_lines(), 1);

        // A part reads the layout of the measure it starts with.
        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(measure.clone());
        assert_eq!(part.staff_lines(), 1);

        // A layout in a measure that starts after the stream does says
        // nothing about it.
        let mut later = Stream::with_kind(StreamKind::Part);
        let mut first = Stream::with_kind(StreamKind::Measure);
        first.insert(
            0.0,
            Note::from_name("C4")
                .expect("a note name")
                .with_duration(Duration::whole()),
        );
        later.push(first);
        later.push(measure.clone());
        assert_eq!(later.staff_lines(), 5);

        // A measure holding nothing takes no time, so the one after it
        // starts where it does and is read after all.
        let mut empty_first = Stream::with_kind(StreamKind::Part);
        empty_first.push(Stream::with_kind(StreamKind::Measure));
        empty_first.push(measure);
        assert_eq!(empty_first.staff_lines(), 1);
    }
}
