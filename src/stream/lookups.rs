//! Finding elements by where they stand: music21's `getElementAtOrBefore`,
//! `getElementBeforeOffset`, `getElementAfterElement`,
//! `beatAndMeasureFromOffset`, `getTimeSignatures` and `getInstruments`.

use super::exact::Exact;
use super::{Stream, StreamElement, StreamEvent};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::instrument::Instrument;
use crate::makenotation::{event_order, op_frac};
use crate::meter::TimeSignature;

fn stream_error(message: impl Into<String>) -> Error {
    Error::Stream(message.into())
}

/// Whether an element is of one of `classes`, or of any where none are
/// named.
fn of_classes(element: &StreamElement, classes: &[&str]) -> bool {
    classes.is_empty() || classes.iter().any(|class| element.is_instance(class))
}

/// The time signatures a stream holds directly, with where each stands.
fn time_signatures_in(stream: &Stream) -> impl Iterator<Item = (FloatType, TimeSignature)> + '_ {
    stream
        .events
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::TimeSignature(meter) => Some((event.offset(), meter.clone())),
            _ => None,
        })
}

/// The measure an event is, with where it stands.
fn measure_of(event: &StreamEvent) -> Option<(FloatType, &Stream)> {
    match event.element() {
        StreamElement::Stream(inner) => Some((event.offset(), &**inner)),
        _ => None,
    }
}

impl Stream {
    /// The element this stream holds itself that starts at `offset`, or
    /// nearest before it, of one of `classes` (any where none are named):
    /// music21's `getElementAtOrBefore`. Of several starting at the same
    /// place, the last in the stream's order.
    ///
    /// music21 also looks among a stream's elements stored at its end, which
    /// is where it keeps a measure's right barline; the crate keeps barlines
    /// beside the elements ([`Stream::right_barline`]), so they are not
    /// found here.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("4/4 c4 d4 e2")?.flatten();
    /// let found = line.element_at_or_before(1.5, &["Note"]).unwrap();
    /// assert_eq!(found.offset(), 1.0);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn element_at_or_before(
        &self,
        offset: FloatType,
        classes: &[&str],
    ) -> Option<&StreamEvent> {
        self.nearest_before(offset, classes, false)
    }

    /// The element nearest before `offset`, not at it: music21's
    /// `getElementBeforeOffset`. As [`Stream::element_at_or_before`]
    /// otherwise.
    pub fn element_before_offset(
        &self,
        offset: FloatType,
        classes: &[&str],
    ) -> Option<&StreamEvent> {
        self.nearest_before(offset, classes, true)
    }

    fn nearest_before(
        &self,
        offset: FloatType,
        classes: &[&str],
        before_not_at: bool,
    ) -> Option<&StreamEvent> {
        let offset = op_frac(offset);
        let mut best: Option<(FloatType, &StreamEvent)> = None;
        for event in &self.events {
            if !of_classes(event.element(), classes) {
                continue;
            }
            let span = op_frac(offset - event.offset());
            if span < 0.0 || (span == 0.0 && before_not_at) {
                continue;
            }
            // music21 starts the nearest span at the offset itself, so an
            // element standing before nought is never found.
            let better = match best {
                None => span <= offset,
                Some((nearest, held)) => {
                    span < nearest
                        || (span == nearest && event_order(event, held) != std::cmp::Ordering::Less)
                }
            };
            if better {
                best = Some((span, event));
            }
        }
        best.map(|(_, event)| event)
    }

    /// The element after the one at `index` among this stream's own, of one
    /// of `classes` (any where none are named): music21's
    /// `getElementAfterElement`.
    ///
    /// # Errors
    ///
    /// An index this stream has no element at.
    pub fn element_after(&self, index: usize, classes: &[&str]) -> Result<Option<&StreamEvent>> {
        if index >= self.events.len() {
            return Err(stream_error("cannot find object in this Stream"));
        }
        Ok(self.events[index + 1..]
            .iter()
            .find(|event| of_classes(event.element(), classes)))
    }

    /// Which beat of which measure `offset` falls on: music21's
    /// `beatAndMeasureFromOffset`. The beat counts from one, as a
    /// proportion of the time signature in force at the measure.
    ///
    /// A measure with a number suffix, or numbered nought where
    /// `fix_zeros`, is taken as the second half of the measure before it,
    /// which is answered instead, with the beat counted on from that
    /// measure's last; a measure so numbered that opens the piece is taken
    /// as a pickup, its beats counted from the end of a full bar. A score
    /// answers for its first part with measures.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("3/4 c4 d4 e4 f2.")?;
    /// let (beat, measure) = line.beat_and_measure_from_offset(4.0, true)?;
    /// assert_eq!((beat, measure.number()), (2.0, 2));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A stream with no measures, or none in any part; no measure at or
    /// before `offset`; no time signature in force there; or an offset past
    /// the bar it falls in.
    pub fn beat_and_measure_from_offset(
        &self,
        offset: FloatType,
        fix_zeros: bool,
    ) -> Result<(FloatType, &Stream)> {
        let mut holder = self;
        if !self.has_measures() {
            if !self.has_part_like_streams() {
                return Err(stream_error(
                    "beatAndMeasureFromOffset: could not find any measures!",
                ));
            }
            holder = self
                .events
                .iter()
                .filter_map(|event| event.element().as_stream())
                .find(|inner| inner.has_measures())
                .ok_or_else(|| {
                    stream_error("beatAndMeasureFromOffset: could not find any parts!")
                })?;
        }
        let (measure_offset, measure) = holder
            .element_at_or_before(offset, &["Measure"])
            .and_then(measure_of)
            .ok_or_else(|| stream_error("beatAndMeasureFromOffset: no measure at that offset."))?;
        let meter = holder.time_signature_at(measure_offset).ok_or_else(|| {
            stream_error(
                "beatAndMeasureFromOffset: could not find a time signature for that place.",
            )
        })?;
        let beat = meter
            .beat_proportion(op_frac(offset - measure_offset))
            .map_err(|_| {
                stream_error("beatAndMeasureFromOffset: offset is beyond the end of the piece")
            })?;

        let suffixed = measure
            .number_suffix()
            .is_some_and(|suffix| !suffix.is_empty());
        if !suffixed && !(fix_zeros && measure.number() == 0) {
            return Ok((beat, measure));
        }
        // music21 asks whether the measure before is truthy: whether it
        // holds anything.
        let previous = holder
            .element_before_offset(measure_offset, &["Measure"])
            .and_then(measure_of)
            .filter(|(_, previous)| {
                !previous.events.is_empty() || previous.right_barline().is_some()
            });
        match previous {
            Some((previous_offset, previous)) => {
                let opening = holder.time_signature_at(previous_offset).ok_or_else(|| {
                    stream_error(
                        "beatAndMeasureFromOffset: partial measure found, but could not find a time signature for the preceding measure",
                    )
                })?;
                // Each time signature in the first half counts the stretch up
                // to the next one; one standing where another does replaces it.
                let mut segments = vec![(opening, 0.0)];
                for (start, meter) in time_signatures_in(previous) {
                    if segments.last().is_some_and(|(_, last)| *last == start) {
                        segments.pop();
                    }
                    segments.push((meter, start));
                }
                let filled = op_frac(previous.end_offset());
                let mut pad = Exact::integer(0);
                for (index, (meter, start)) in segments.iter().enumerate() {
                    let end = segments.get(index + 1).map_or(filled, |(_, next)| *next);
                    let length = op_frac(end - start);
                    pad = pad.plus(if length == meter.bar_quarter_length() {
                        Exact::integer(i64::from(meter.beat_count()))
                    } else {
                        Exact::held(meter.beat_proportion(length)?).plus(Exact::integer(-1))
                    });
                }
                let beat = match segments.last() {
                    Some((closing, _))
                        if !time_signatures_in(measure).any(|(start, _)| start == 0.0) =>
                    {
                        closing.beat_proportion(op_frac(offset - measure_offset))?
                    }
                    _ => beat,
                };
                Ok((Exact::held(beat).plus(pad).to_float(), previous))
            }
            None => {
                let pad =
                    Exact::held(meter.beat_proportion(
                        meter.bar_quarter_length() - op_frac(measure.end_offset()),
                    )?)
                    .plus(Exact::integer(-1));
                Ok((Exact::held(beat).plus(pad).to_float(), measure))
            }
        }
    }

    /// The time signatures this stream holds, however deep, each where it
    /// stands from the stream's start: music21's `getTimeSignatures`. With
    /// `return_default`, 4/4 is put first at nought where none stands there.
    ///
    /// music21 also looks outside a stream holding none, through the
    /// streams that hold it; a crate stream knows nothing outside itself.
    ///
    /// # Errors
    ///
    /// None in practice: the default 4/4 is always made.
    pub fn time_signatures(&self, return_default: bool) -> Result<Vec<(FloatType, TimeSignature)>> {
        let mut found: Vec<(FloatType, TimeSignature)> = self
            .flatten()
            .events
            .into_iter()
            .filter_map(|event| match event.element {
                StreamElement::TimeSignature(meter) => Some((event.offset, meter)),
                _ => None,
            })
            .collect();
        if return_default && found.first().is_none_or(|(offset, _)| *offset > 0.0) {
            found.insert(0, (0.0, TimeSignature::new(4, 4)?));
        }
        Ok(found)
    }

    /// The instruments this stream holds, however deep where `recurse`, in
    /// order: music21's `getInstruments`. With `return_default`, an
    /// instrument with an empty part name stands for none found.
    ///
    /// music21 also asks the stream holding this one where it holds none; a
    /// crate stream knows nothing outside itself.
    pub fn instruments(&self, recurse: bool, return_default: bool) -> Vec<Instrument> {
        let held = if recurse {
            self.flatten()
        } else {
            self.clone()
        };
        let mut found: Vec<Instrument> = held
            .events
            .into_iter()
            .filter_map(|event| match event.element {
                StreamElement::Instrument(instrument) => Some(*instrument),
                _ => None,
            })
            .collect();
        if return_default && found.is_empty() {
            let mut instrument = Instrument::new();
            instrument.set_part_name(Some(String::new()));
            found.push(instrument);
        }
        found
    }

    /// The first instrument this stream holds itself, or with `recurse`
    /// however deep: music21's `getInstrument`. With `return_default`, an
    /// instrument with an empty part name where none is.
    pub fn instrument(&self, recurse: bool, return_default: bool) -> Option<Instrument> {
        self.instruments(recurse, return_default).into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::duration::Duration;
    use crate::note::Note;
    use crate::rest::Rest;
    use crate::stream::StreamKind;

    #[test]
    fn the_nearest_before_is_the_last_of_its_offset() -> Result<()> {
        let mut stream = Stream::new();
        stream.insert(0.0, Note::from_name("C4")?);
        stream.insert(1.0, Rest::new(Duration::quarter()));
        stream.insert(1.0, Note::from_name("D4")?);
        let at = stream.element_at_or_before(1.0, &[]).unwrap();
        assert_eq!(at.offset(), 1.0);
        assert!(matches!(at.element(), StreamElement::Note(_)));
        assert!(matches!(
            stream
                .element_at_or_before(1.0, &["Rest"])
                .unwrap()
                .element(),
            StreamElement::Rest(_)
        ));
        assert_eq!(
            stream.element_before_offset(1.0, &[]).unwrap().offset(),
            0.0
        );
        assert!(stream.element_before_offset(0.0, &[]).is_none());
        assert!(stream.element_at_or_before(-1.0, &[]).is_none());
        assert!(matches!(
            stream.element_after(0, &["Note"])?.unwrap().element(),
            StreamElement::Note(_)
        ));
        assert!(stream.element_after(2, &[])?.is_none());
        assert!(stream.element_after(3, &[]).is_err());
        Ok(())
    }

    #[test]
    fn a_pickup_counts_its_beats_from_the_end_of_a_bar() -> Result<()> {
        let mut part = Stream::with_kind(StreamKind::Part);
        let mut pickup = Stream::with_kind(StreamKind::Measure);
        pickup.insert(0.0, TimeSignature::new(4, 4)?);
        pickup.insert(0.0, Note::from_name("G4")?);
        let mut first = Stream::with_kind(StreamKind::Measure);
        first.set_number(1);
        first.insert(0.0, Note::from_name("C5")?.with_duration(Duration::whole()));
        part.insert(0.0, pickup);
        part.insert(1.0, first);
        let (beat, measure) = part.beat_and_measure_from_offset(0.0, true)?;
        assert_eq!((beat, measure.number()), (4.0, 0));
        let (beat, measure) = part.beat_and_measure_from_offset(3.0, true)?;
        assert_eq!((beat, measure.number()), (3.0, 1));
        Ok(())
    }

    #[test]
    fn a_meter_changing_at_a_split_counts_each_half_in_its_own() -> Result<()> {
        let mut part = Stream::with_kind(StreamKind::Part);
        let mut whole = Stream::with_kind(StreamKind::Measure);
        whole.set_number(1);
        whole.insert(0.0, TimeSignature::new(4, 4)?);
        whole.insert(0.0, Note::from_name("C4")?.with_duration(Duration::whole()));
        let mut first_half = Stream::with_kind(StreamKind::Measure);
        first_half.set_number(2);
        first_half.insert(0.0, Note::from_name("C4")?.with_duration(Duration::half()));
        first_half.insert(2.0, TimeSignature::new(12, 16)?);
        let mut second_half = Stream::with_kind(StreamKind::Measure);
        second_half.set_number(2);
        second_half.set_number_suffix(Some("X1".to_string()));
        second_half.insert(
            0.0,
            Note::from_name("C4")?.with_duration(Duration::new(0.75)?),
        );
        second_half.insert(
            0.75,
            Note::from_name("C4")?.with_duration(Duration::new(0.75)?),
        );
        part.insert(0.0, whole);
        part.insert(4.0, first_half);
        part.insert(6.0, second_half);
        let beats = [5.0, 6.0, 6.75]
            .into_iter()
            .map(|offset| Ok(part.beat_and_measure_from_offset(offset, true)?.0))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(beats, [2.0, 3.0, 4.0]);
        Ok(())
    }

    #[test]
    fn a_default_meter_and_instrument_stand_in_for_none() -> Result<()> {
        let stream = Stream::new();
        let meters = stream.time_signatures(true)?;
        assert_eq!(meters.len(), 1);
        assert_eq!(meters[0].1.ratio_string(), "4/4");
        assert!(stream.time_signatures(false)?.is_empty());
        assert_eq!(stream.instrument(true, true).unwrap().part_name(), Some(""));
        assert!(stream.instrument(true, false).is_none());
        Ok(())
    }
}
