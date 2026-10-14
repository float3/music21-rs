//! How long a stream lasts in seconds, and which tempo holds where:
//! music21's `Stream.seconds` and `metronomeMarkBoundaries`.
//!
//! Both read the tempi the stream holds. music21's `seconds` also searches
//! the stream's context for a tempo when the stream states none from its
//! start; a crate stream knows nothing outside itself, so the tempo in
//! force is given to it.

use super::{Stream, StreamElement, StreamEvent};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::tempo::MetronomeMark;

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
        let flat = self.flatten();
        let highest = flat.end_offset();
        let lowest = flat.events().first().map_or(0.0, StreamEvent::offset);
        let mut marks: Vec<(FloatType, MetronomeMark)> = Vec::new();
        for event in flat.events() {
            let previous = marks.last().map(|(_, mark)| mark.clone());
            let Some(sounding) = sounding_mark(event.element(), previous.as_ref())? else {
                continue;
            };
            marks.push((event.offset(), sounding));
        }
        let default = || MetronomeMark::new(120.0);
        let mut boundaries = Vec::new();
        match marks.as_slice() {
            [] => boundaries.push((lowest, highest, default())),
            [(offset, mark)] => {
                if *offset > lowest {
                    boundaries.push((lowest, *offset, default()));
                    boundaries.push((*offset, highest, mark.clone()));
                } else {
                    boundaries.push((lowest, highest, mark.clone()));
                }
            }
            _ => {
                if marks[0].0 > lowest {
                    boundaries.push((lowest, marks[0].0, default()));
                }
                boundaries.push((marks[0].0, marks[1].0, marks[0].1.clone()));
                for index in 1..marks.len() {
                    let end = marks.get(index + 1).map_or(highest, |(offset, _)| *offset);
                    boundaries.push((marks[index].0, end, marks[index].1.clone()));
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

    #[test]
    fn a_stream_with_no_tempo_is_counted_at_a_quarter_at_120() {
        let boundaries = four_quarters()
            .metronome_mark_boundaries()
            .expect("boundaries");
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].2.number(), Some(120.0));
    }
}
