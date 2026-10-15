//! How long a stream lasts in seconds, and which tempo holds where:
//! music21's `Stream.seconds` and `metronomeMarkBoundaries`.
//!
//! Both read the tempi the stream holds. music21's `seconds` also searches
//! the stream's context for a tempo when the stream states none from its
//! start; a crate stream knows nothing outside itself, so the tempo in
//! force is given to it.

use super::{Stream, StreamElement};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::tempo::MetronomeMark;

/// One stretch of a stream and the tempo sounding through it: a boundary
/// of music21's `metronomeMarkBoundaries`.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct TempoSpan {
    /// Where the stretch begins.
    pub start: FloatType,
    /// Where it ends, which is where the next tempo stands, or the end of
    /// the stream for the last.
    pub end: FloatType,
    /// The tempo sounding through it.
    pub mark: MetronomeMark,
    /// The position among [`Stream::leaves`] of the mark the stream holds,
    /// where the tempo is one of its own: nothing for the quarter at 120
    /// standing in for a stream that states none, and nothing for what a
    /// tempo word or a metric modulation means.
    pub position: Option<usize>,
}

impl Stream {
    /// The stretches of the stream each tempo holds, start, end and the
    /// mark sounding there: music21's `metronomeMarkBoundaries`.
    ///
    /// The stream is read flattened, so a tempo written on a part is found
    /// from the score. Where it states no tempo, or none from its start, a
    /// quarter at 120 holds.
    ///
    /// # Errors
    ///
    /// A metric modulation whose new tempo cannot be worked out.
    pub fn metronome_mark_boundaries(&self) -> Result<Vec<(FloatType, FloatType, MetronomeMark)>> {
        Ok(self
            .metronome_mark_positions()?
            .into_iter()
            .map(|boundary| (boundary.start, boundary.end, boundary.mark))
            .collect())
    }

    /// [`Stream::metronome_mark_boundaries`], each stretch also saying
    /// which element the tempo was read off, by its position among
    /// [`Stream::leaves`].
    ///
    /// music21 hands back the mark objects the stream holds, so a caller
    /// keeping something of its own beside each element -- a binding
    /// keeping the object it was handed -- finds the mark again by that
    /// position. A stretch standing under no mark of the stream's carries
    /// none: the quarter at 120 standing in for a stream that states no
    /// tempo, and the mark a tempo word or a metric modulation *means*,
    /// which music21 builds afresh as well.
    ///
    /// # Errors
    ///
    /// A metric modulation whose new tempo cannot be worked out.
    pub fn metronome_mark_positions(&self) -> Result<Vec<TempoSpan>> {
        let leaves = self.leaves();
        // The leaves as `flatten` orders them: the stream is read
        // flattened, so a tempo written on a part is found from the score.
        let order = Stream::flatten_order(&leaves);
        let highest = crate::makenotation::op_frac(
            leaves
                .iter()
                .map(|(offset, element)| offset + element.quarter_length())
                .fold(0.0, FloatType::max),
        );
        let lowest = order.first().map_or(0.0, |place| leaves[*place].0);
        let mut marks: Vec<(FloatType, MetronomeMark, Option<usize>)> = Vec::new();
        for place in order {
            let (offset, element) = leaves[place];
            let previous = marks.last().map(|(_, mark, _)| mark.clone());
            let Some(sounding) = sounding_mark(element, previous.as_ref())? else {
                continue;
            };
            // Only a mark of the stream's own is the object music21 hands
            // back; what a tempo word or a modulation means is built here.
            let held = matches!(element, StreamElement::MetronomeMark(_)).then_some(place);
            marks.push((offset, sounding, held));
        }
        let default = |start, end| TempoSpan {
            start,
            end,
            mark: MetronomeMark::new(120.0),
            position: None,
        };
        let span = |(offset, mark, position): &(FloatType, MetronomeMark, Option<usize>),
                    end: FloatType| TempoSpan {
            start: *offset,
            end,
            mark: mark.clone(),
            position: *position,
        };
        let mut boundaries = Vec::new();
        match marks.as_slice() {
            [] => boundaries.push(default(lowest, highest)),
            [only] => {
                if only.0 > lowest {
                    boundaries.push(default(lowest, only.0));
                }
                boundaries.push(span(only, highest));
            }
            _ => {
                if marks[0].0 > lowest {
                    boundaries.push(default(lowest, marks[0].0));
                }
                boundaries.push(span(&marks[0], marks[1].0));
                for index in 1..marks.len() {
                    let end = marks.get(index + 1).map_or(highest, |next| next.0);
                    boundaries.push(span(&marks[index], end));
                }
            }
        }
        Ok(boundaries)
    }

    /// How long the stream lasts in seconds: music21's `seconds`.
    ///
    /// Only the tempi the stream holds *itself* count, as music21's do --
    /// a tempo inside a part of a score is not one of the score's own --
    /// and each holds until the next, the last to the end of the stream.
    /// Where the stream states no tempo, or none from its start,
    /// `in_force` stands for what music21 searches its context for, and is
    /// read as a mark standing at nought, which is where music21 puts what
    /// it finds. What music21's own search finds turns on sort order: a
    /// measure standing *after* a tempo in its part finds it, one standing
    /// at the same offset does not, a stream sorting before the marks at
    /// its offset.
    ///
    /// With no tempo anywhere, a stream holding nothing that lasts is
    /// nought seconds long, and one holding something is `NaN`, which is
    /// what music21 answers rather than refusing.
    ///
    /// ```
    /// use music21_rs::{Duration, MetronomeMark, Note, Stream, StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// part.insert(0.0, Note::from_name("C4")?.with_duration(Duration::whole()));
    /// // A quarter at 60 makes the whole note four seconds.
    /// assert_eq!(part.seconds(Some(&MetronomeMark::new(60.0)))?, 4.0);
    ///
    /// // A tempo the stream holds itself is used from where it stands.
    /// part.insert(0.0, MetronomeMark::new(120.0));
    /// assert_eq!(part.seconds(None)?, 2.0);
    ///
    /// // With no tempo at all, something that lasts answers NaN.
    /// let mut bare = Stream::new();
    /// assert_eq!(bare.seconds(None)?, 0.0);
    /// bare.insert(0.0, Note::from_name("C4")?.with_duration(Duration::half()));
    /// assert!(bare.seconds(None)?.is_nan());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A tempo that says no number, or a metric modulation whose new tempo
    /// cannot be worked out.
    pub fn seconds(&self, in_force: Option<&MetronomeMark>) -> Result<FloatType> {
        let highest = self.end_offset();
        let mut marks: Vec<(FloatType, MetronomeMark)> = Vec::new();
        for event in self.events() {
            let previous = marks.last().map(|(_, mark)| mark.clone());
            let Some(sounding) = sounding_mark(event.element(), previous.as_ref())? else {
                continue;
            };
            marks.push((event.offset(), sounding));
        }
        if marks.first().is_none_or(|(offset, _)| *offset > 0.0) {
            let Some(in_force) = in_force else {
                // music21 answers nought for a stream with nothing in it
                // that lasts, and NaN rather than refusing otherwise.
                return Ok(if highest == 0.0 { 0.0 } else { FloatType::NAN });
            };
            marks.insert(0, (0.0, in_force.clone()));
        }
        if let [(_, only)] = marks.as_slice() {
            // With one tempo music21 counts the whole stream at it, from
            // nought rather than from where the tempo stands.
            return only.quarter_length_to_seconds(highest);
        }
        let mut total = 0.0;
        for (at, (offset, mark)) in marks.iter().enumerate() {
            let end = marks.get(at + 1).map_or(highest, |(next, _)| *next);
            total += mark.quarter_length_to_seconds(end - offset)?;
        }
        Ok(total)
    }
}

/// The tempo one element sounds, or nothing where the element is not a
/// tempo at all: music21's `getSoundingMetronomeMark`.
///
/// A metric modulation's new tempo is worked out from the tempo before it
/// where it says no number of its own, which music21 has filled in by the
/// time it reads the modulation.
///
/// # Errors
///
/// A metric modulation whose new tempo cannot be worked out.
fn sounding_mark(
    element: &StreamElement,
    previous: Option<&MetronomeMark>,
) -> Result<Option<MetronomeMark>> {
    let sounding = match element {
        StreamElement::MetronomeMark(mark) => mark.clone(),
        StreamElement::TempoText(text) => text.metronome_mark(),
        StreamElement::MetricModulation(modulation) => {
            let mut modulation = (**modulation).clone();
            if modulation
                .new_metronome()
                .is_some_and(|mark| mark.number().is_none())
            {
                modulation.update_from(previous);
            }
            modulation
                .new_metronome()
                .cloned()
                .ok_or_else(|| Error::Tempo("a metric modulation with no new tempo".to_string()))?
        }
        _ => return Ok(None),
    };
    Ok(Some(sounding))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::note::Note;
    use crate::stream::StreamKind;

    fn four_quarters() -> Stream {
        let mut part = Stream::with_kind(StreamKind::Part);
        for place in 0..4 {
            let note = Note::from_name("C4")
                .expect("a note name")
                .with_duration(Duration::new(1.0).expect("a quarter"));
            part.insert(FloatType::from(place), note);
        }
        part
    }

    #[test]
    fn a_tempo_the_stream_holds_counts_from_where_it_stands() {
        let mut part = four_quarters();
        part.insert(0.0, MetronomeMark::new(60.0));
        assert_eq!(part.seconds(None).expect("seconds"), 4.0);
        // The second tempo holds from offset 2 to the end.
        part.insert(2.0, MetronomeMark::new(120.0));
        assert_eq!(part.seconds(None).expect("seconds"), 3.0);
    }

    /// music21 prepends the tempo it finds in context at offset nought, so
    /// each answer here is music21's own for the same stream with the mark
    /// standing at nought: 2.0 and 2.5.
    #[test]
    fn a_tempo_in_force_stands_for_what_music21_finds_in_context() {
        let part = four_quarters();
        assert_eq!(
            part.seconds(Some(&MetronomeMark::new(120.0)))
                .expect("seconds"),
            2.0
        );
        // A tempo from offset 1 leaves the first quarter to the one in
        // force, as music21 does by inserting it at nought.
        let mut later = four_quarters();
        later.insert(1.0, MetronomeMark::new(120.0));
        assert_eq!(
            later
                .seconds(Some(&MetronomeMark::new(60.0)))
                .expect("seconds"),
            2.5
        );
    }

    #[test]
    fn nothing_that_lasts_is_nought_seconds_and_anything_else_is_not_a_number() {
        let empty = Stream::new();
        assert_eq!(empty.seconds(None).expect("seconds"), 0.0);
        assert!(four_quarters().seconds(None).expect("seconds").is_nan());
    }

    /// The positions are how a caller holding an object per element finds
    /// the very mark music21 would have handed back.
    #[test]
    fn a_boundary_says_which_mark_of_the_streams_own_sounds_through_it() {
        let mut part = four_quarters();
        part.insert(2.0, MetronomeMark::new(90.0));
        let spans = part.metronome_mark_positions().expect("boundaries");
        assert_eq!(spans.len(), 2);

        // The quarter at 120 standing in for the tempo the stream says
        // nothing of up to offset 2 is nobody's mark.
        assert_eq!((spans[0].start, spans[0].end), (0.0, 2.0));
        assert_eq!(spans[0].mark.number(), Some(120.0));
        assert_eq!(spans[0].position, None);

        assert_eq!((spans[1].start, spans[1].end), (2.0, 4.0));
        let leaves = part.leaves();
        let position = spans[1].position.expect("the mark the stream holds");
        assert!(matches!(
            leaves[position].1,
            StreamElement::MetronomeMark(mark) if mark.number() == Some(90.0)
        ));
    }

    /// A tempo word *means* a tempo rather than holding one, as a metric
    /// modulation does, so there is no object to point at.
    #[test]
    fn the_mark_a_tempo_word_means_is_nobodys() {
        let mut part = four_quarters();
        part.insert(0.0, crate::tempo::TempoText::new("adagio"));
        let spans = part.metronome_mark_positions().expect("boundaries");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].mark.number(), Some(56.0));
        assert_eq!(spans[0].position, None);
    }

    #[test]
    fn a_stream_with_no_tempo_is_counted_at_a_quarter_at_120() {
        let boundaries = four_quarters()
            .metronome_mark_boundaries()
            .expect("boundaries");
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].2.number(), Some(120.0));
    }
}
