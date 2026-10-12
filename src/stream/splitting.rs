//! Cutting a stream in two at a point: music21's
//! `Stream.splitAtQuarterLength`.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::clef::Clef;
use crate::defaults::FloatType;
use crate::error::Result;
use crate::key::KeySignature;
use crate::makenotation::{op_frac, split_element};
use crate::meter::TimeSignature;

/// The clef, key and time signature in force before a measure, which
/// music21 finds by searching the measure's context and
/// [`Stream::split_at_quarter_length`] is given: a crate stream knows
/// nothing outside itself. [`Stream::in_force_before`] reads them off a
/// part.
#[derive(Clone, Debug, Default)]
pub struct InForce {
    /// The clef in force before the measure.
    pub clef: Option<Clef>,
    /// The key signature in force before the measure.
    pub key: Option<KeySignature>,
    /// The time signature in force before the measure.
    pub meter: Option<TimeSignature>,
}

impl Stream {
    /// The clef, key and time signature in force before `offset`: the last
    /// of each this stream holds, however deep, standing before it.
    pub fn in_force_before(&self, offset: FloatType) -> InForce {
        let mut found = InForce::default();
        for event in self.flatten().events() {
            if event.offset() >= offset {
                break;
            }
            match event.element() {
                StreamElement::Clef(clef) => found.clef = Some(clef.clone()),
                StreamElement::KeySignature(key) => found.key = Some(key.clone()),
                StreamElement::Key(key) => found.key = Some(key.key_signature()),
                StreamElement::TimeSignature(meter) => found.meter = Some(meter.clone()),
                _ => {}
            }
        }
        found
    }

    /// The stream cut in two `quarter_length` into it: music21's
    /// `splitAtQuarterLength`. What starts before the point and sounds
    /// past it is cut there, tied, its second piece starting the second
    /// half; what starts at or after the point moves to the second half,
    /// as far from its start. A voice is cut as a stream is.
    ///
    /// The second half of a measure is a measure of its own holding the
    /// time signature of the first (or `in_force`'s where it has none),
    /// and the key and clef in force at the point: the first half's own
    /// last before it, or `in_force`'s. Spanners stay with the first half,
    /// naming nothing for what moved.
    ///
    /// ```
    /// use music21_rs::stream::InForce;
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("4/4 c2 d2")?;
    /// let measure = line.measures()[0].clone();
    /// let (first, second) = measure.split_at_quarter_length(1.0, &InForce::default())?;
    /// assert_eq!(first.notes().len(), 1);
    /// assert_eq!(second.notes().len(), 2);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// An element that cannot be cut at the point.
    pub fn split_at_quarter_length(
        &self,
        quarter_length: FloatType,
        in_force: &InForce,
    ) -> Result<(Stream, Stream)> {
        let quarter_length = op_frac(quarter_length);
        let mut right = Stream::with_kind(self.kind());
        let mut settings: Vec<StreamEvent> = Vec::new();
        if self.kind() == StreamKind::Measure {
            let own = |find: &dyn Fn(&StreamElement) -> bool| -> Option<StreamElement> {
                self.events()
                    .iter()
                    .filter(|event| event.offset() < quarter_length && find(event.element()))
                    .map(|event| event.element().clone())
                    .last()
            };
            let meter = self
                .flatten()
                .events()
                .iter()
                .find_map(|event| match event.element() {
                    StreamElement::TimeSignature(meter) => Some(meter.clone()),
                    _ => None,
                })
                .or_else(|| in_force.meter.clone());
            if let Some(meter) = meter {
                settings.push(StreamEvent::new(0.0, meter));
            }
            let key = own(&|element| {
                matches!(
                    element,
                    StreamElement::KeySignature(_) | StreamElement::Key(_)
                )
            })
            .or_else(|| in_force.key.clone().map(StreamElement::KeySignature));
            if let Some(key) = key {
                settings.push(StreamEvent::new(0.0, key));
            }
            let clef = own(&|element| matches!(element, StreamElement::Clef(_)))
                .or_else(|| in_force.clef.clone().map(StreamElement::Clef));
            if let Some(clef) = clef {
                settings.push(StreamEvent::new(0.0, clef));
            }
        }
        right.insert_sorted(settings);

        let mut left = self.clone();
        let highest = op_frac(self.end_offset());
        if quarter_length > highest {
            return Ok((left, right));
        }
        let span = super::queries::OffsetSpan {
            include_end_boundary: true,
            must_finish_in_span: false,
            include_elements_that_end_at_start: false,
            must_begin_in_span: false,
            ..super::queries::OffsetSpan::new(quarter_length, Some(highest))
        };
        let mut kept: Vec<(StreamEvent, Option<usize>)> = Vec::new();
        let mut moved: Vec<StreamEvent> = Vec::new();
        for (index, event) in self.events().iter().enumerate() {
            let length = event.element().quarter_length();
            if !span.holds(event.offset(), length) {
                kept.push((event.clone(), Some(index)));
                continue;
            }
            if event.offset() < quarter_length {
                let cut = op_frac(quarter_length - event.offset());
                let (first, second) = match event.element() {
                    StreamElement::Stream(inner) => {
                        let (first, second) =
                            inner.split_at_quarter_length(cut, &InForce::default())?;
                        (first.into(), second.into())
                    }
                    element => split_element(element, cut)?,
                };
                kept.push((StreamEvent::new(event.offset(), first), Some(index)));
                moved.push(StreamEvent::new(0.0, second));
            } else {
                moved.push(StreamEvent::new(
                    op_frac(event.offset() - quarter_length),
                    event.element().clone(),
                ));
            }
        }
        left.replace_events(kept);
        if left.right_barline().is_some() && span.holds(highest, 0.0) {
            right.set_right_barline(left.right_barline().cloned());
            left.set_right_barline(None);
        }
        right.insert_sorted(moved);
        Ok((left, right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bar::{Barline, BarlineType};
    use crate::duration::Duration;
    use crate::note::Note;

    #[test]
    fn the_second_half_gets_the_settings_and_the_right_barline() -> Result<()> {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        measure.insert(0.0, Note::from_name("C4")?.with_duration(Duration::whole()));
        measure.set_right_barline(Some(Barline::new(BarlineType::Final)));
        let in_force = InForce {
            clef: Some(Clef::best_for(&[], false)),
            key: Some(KeySignature::new(2)),
            meter: Some(TimeSignature::new(4, 4)?),
        };
        let (left, right) = measure.split_at_quarter_length(1.0, &in_force)?;
        assert!(left.right_barline().is_none());
        assert_eq!(
            right.right_barline().map(Barline::bar_type),
            Some(BarlineType::Final)
        );
        assert!(right.left_barline().is_none());
        let kinds: Vec<&str> = right
            .events()
            .iter()
            .map(|event| match event.element() {
                StreamElement::Clef(_) => "clef",
                StreamElement::KeySignature(_) => "key",
                StreamElement::TimeSignature(_) => "meter",
                StreamElement::Note(_) => "note",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["clef", "key", "meter", "note"]);
        assert_eq!(left.end_offset(), 1.0);
        assert_eq!(right.end_offset(), 3.0);

        // Past the end there is nothing to cut.
        let (whole, empty) = measure.split_at_quarter_length(5.0, &InForce::default())?;
        assert_eq!(whole.end_offset(), 4.0);
        assert!(empty.is_empty());
        Ok(())
    }
}
