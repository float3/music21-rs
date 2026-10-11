//! Cutting a stream's notes and rests into shorter ones: music21's
//! `sliceByQuarterLengths`, `sliceByGreatestDivisor`, `sliceAtOffsets`,
//! `sliceByBeat` and `splitByClass`.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::makenotation::{event_order, op_frac, split_element};
use crate::meter::TimeSignature;

fn stream_error(message: impl Into<String>) -> Error {
    Error::Stream(message.into())
}

/// An element cut into pieces of these lengths, which must add up to its
/// own: music21's `splitByQuarterLengths`.
fn split_by_lengths(element: &StreamElement, lengths: &[FloatType]) -> Result<Vec<StreamElement>> {
    if lengths.len() == 1 {
        return Ok(vec![element.clone()]);
    }
    let mut pieces = Vec::with_capacity(lengths.len());
    let mut remain = element.clone();
    for length in &lengths[..lengths.len() - 1] {
        let (piece, rest) = split_element(&remain, *length)?;
        pieces.push(piece);
        remain = rest;
    }
    pieces.push(remain);
    Ok(pieces)
}

/// The sum of some lengths, snapped as music21's `opFrac(sum(...))`.
fn total(lengths: &[FloatType]) -> FloatType {
    op_frac(lengths.iter().sum())
}

/// Python's `math.isclose(a, b, abs_tol=grain)`.
fn is_close(a: FloatType, b: FloatType, grain: FloatType) -> bool {
    (a - b).abs() <= (1e-9 * a.abs().max(b.abs())).max(grain)
}

/// The greatest length that goes a whole number of times into every one of
/// `values`, to within a ten-thousandth: music21's `approximateGCD`, which
/// tries the smallest value and then each value divided by one to sixteen.
///
/// # Errors
///
/// No values, a value of nought, which music21 divides by, or no length
/// that goes into them all.
pub(crate) fn approximate_gcd(values: &[FloatType]) -> Result<FloatType> {
    const GRAIN: FloatType = 0.0001;
    let lowest = values
        .iter()
        .copied()
        .reduce(FloatType::min)
        .ok_or_else(|| stream_error("min() arg is an empty sequence"))?;
    if lowest == 0.0 {
        return Err(stream_error("float division by zero"));
    }
    let whole = values.iter().all(|value| {
        let adjusted = value / lowest;
        is_close(adjusted - adjusted.trunc(), 0.0, GRAIN)
    });
    if whole {
        return Ok(lowest);
    }
    let divisions: Vec<Vec<FloatType>> = values
        .iter()
        .map(|value| (1..=16).map(|d| value / FloatType::from(d)).collect())
        .collect();
    let common = divisions
        .iter()
        .flatten()
        .copied()
        .filter(|candidate| {
            divisions
                .iter()
                .all(|row| row.iter().any(|x| is_close(*x, *candidate, GRAIN)))
        })
        .reduce(FloatType::max);
    common.ok_or_else(|| stream_error("cannot find a common divisor"))
}

/// Where each leaf of a stream went: nowhere moved.
type Moved = Vec<Option<usize>>;

fn unmoved(stream: &Stream) -> Moved {
    (0..stream.leaf_total()).map(Some).collect()
}

/// Where each leaf went after two edits, one after the other.
fn then(first: Moved, next: &[Option<usize>]) -> Moved {
    first
        .into_iter()
        .map(|place| place.and_then(|at| next.get(at).copied().flatten()))
        .collect()
}

/// The direct events of a stream put back in music21's order: each event
/// with the index it stood at, if it did, and the order it went in.
fn reorder(stream: &mut Stream, mut events: Vec<(StreamEvent, Option<usize>, usize)>) -> Moved {
    events.sort_by(|left, right| event_order(&left.0, &right.0).then(left.2.cmp(&right.2)));
    stream.replace_events(
        events
            .into_iter()
            .map(|(event, origin, _)| (event, origin))
            .collect(),
    )
}

fn is_measure(element: &StreamElement) -> bool {
    matches!(element, StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure)
}

fn is_part(element: &StreamElement) -> bool {
    matches!(element, StreamElement::Stream(inner) if matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff))
}

/// Applies `edit` to each nested stream of this stream that `which`
/// accepts, in place, every spanner kept on what it names. The answer is
/// where each leaf of this stream went.
fn each_nested(
    stream: &mut Stream,
    which: impl Fn(&StreamElement) -> bool,
    edit: &mut dyn FnMut(&mut Stream, FloatType) -> Result<Moved>,
) -> Result<Moved> {
    let found: Vec<(usize, FloatType)> = stream
        .events()
        .iter()
        .enumerate()
        .filter(|(_, event)| which(event.element()))
        .map(|(index, event)| (index, event.offset()))
        .collect();
    let mut moved = unmoved(stream);
    for (index, offset) in found {
        let (result, step) = stream.edit_nested(&[index], |inner| match edit(inner, offset) {
            Ok(moved) => (Ok(()), moved),
            Err(error) => (Err(error), unmoved(inner)),
        });
        result?;
        moved = then(moved, &step);
    }
    Ok(moved)
}

impl Stream {
    /// A copy of the stream with each note and rest cut into pieces of
    /// `lengths`, the list repeated as far as it takes to fill the note,
    /// tied: music21's `sliceByQuarterLengths`. A note shorter than the
    /// list is left whole. A stream of measures is cut measure by measure,
    /// and a score part by part.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("4/4 c2 d4 e4")?;
    /// let sliced = line.slice_by_quarter_lengths(&[1.0])?;
    /// assert_eq!(sliced.notes().len(), 4);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A note the list cannot add up to, which music21 refuses.
    pub fn slice_by_quarter_lengths(&self, lengths: &[FloatType]) -> Result<Stream> {
        let mut out = self.clone();
        out.slice_lengths_in_place(lengths)?;
        Ok(out)
    }

    fn slice_lengths_in_place(&mut self, lengths: &[FloatType]) -> Result<Moved> {
        if self.has_measures() {
            return each_nested(self, is_measure, &mut |measure, _| {
                measure.slice_lengths_in_place(lengths)
            });
        }
        if self.has_part_like_streams() {
            return each_nested(self, is_part, &mut |part, _| {
                part.slice_lengths_in_place(lengths)
            });
        }
        let sum = total(lengths);
        let mut events: Vec<(StreamEvent, Option<usize>, usize)> = Vec::new();
        let mut order = self.events().len();
        for (index, event) in self.events().iter().enumerate() {
            let element = event.element();
            let length = element.quarter_length();
            if !element.is_instance("GeneralNote") || sum > length {
                events.push((event.clone(), Some(index), index));
                continue;
            }
            let process: Vec<FloatType> = if sum == length {
                lengths.to_vec()
            } else {
                let mut process = Vec::new();
                let mut at = 0;
                while let Some(next) = lengths.get(at % lengths.len()) {
                    process.push(*next);
                    at += 1;
                    if total(&process) >= length {
                        break;
                    }
                }
                process
            };
            if total(&process) != length {
                return Err(stream_error(format!(
                    "cannot map quarterLength list into element Duration: {}, {}",
                    process.iter().sum::<FloatType>(),
                    length
                )));
            }
            let mut offset = event.offset();
            for piece in split_by_lengths(element, &process)? {
                let piece_length = piece.quarter_length();
                events.push((StreamEvent::new(offset, piece), None, order));
                order += 1;
                offset = op_frac(offset + piece_length);
            }
        }
        Ok(reorder(self, events))
    }

    /// A copy of the stream with each note and rest cut into pieces of the
    /// greatest length that goes into all of them, measure by measure:
    /// music21's `sliceByGreatestDivisor`. A score finds one length for
    /// each measure across its parts.
    ///
    /// # Errors
    ///
    /// A measure with no notes or rests, or with a grace note, which
    /// music21's `approximateGCD` cannot divide; a score whose first part
    /// has more measures than another, or none of its own.
    pub fn slice_by_greatest_divisor(&self) -> Result<Stream> {
        let mut out = self.clone();
        if out.kind() == StreamKind::Score {
            out.slice_score_by_greatest_divisor()?;
        } else {
            out.slice_divisor_in_place()?;
        }
        Ok(out)
    }

    fn unique_lengths(&self, into: &mut Vec<FloatType>) {
        for event in self.events() {
            if event.element().is_instance("GeneralNote") {
                let length = event.element().quarter_length();
                if !into.contains(&length) {
                    into.push(length);
                }
            }
        }
    }

    fn slice_divisor_in_place(&mut self) -> Result<Moved> {
        if self.has_measures() {
            return each_nested(self, is_measure, &mut |measure, _| {
                measure.slice_divisor_in_place()
            });
        }
        let mut lengths = Vec::new();
        self.unique_lengths(&mut lengths);
        let divisor = approximate_gcd(&lengths)?;
        self.slice_lengths_in_place(&[divisor])
    }

    fn slice_score_by_greatest_divisor(&mut self) -> Result<()> {
        let parts: Vec<usize> = self
            .events()
            .iter()
            .enumerate()
            .filter(|(_, event)| is_part(event.element()))
            .map(|(index, _)| index)
            .collect();
        let Some(first) = self.parts().first().map(|part| part.measures().len()) else {
            return Err(stream_error("Cannot sliceByGreatestDivisor without parts"));
        };
        let count = first.max(1);
        let measure_of = |part: &Stream, i: usize| -> Result<Option<usize>> {
            if !part.has_measures() {
                return Ok(None);
            }
            part.events()
                .iter()
                .enumerate()
                .filter(|(_, event)| is_measure(event.element()))
                .nth(i)
                .map(|(index, _)| Some(index))
                .ok_or_else(|| stream_error("index out of range"))
        };
        for i in 0..count {
            let mut lengths = Vec::new();
            for &part_index in &parts {
                let StreamElement::Stream(part) = self.events()[part_index].element() else {
                    continue;
                };
                match measure_of(part, i)? {
                    Some(index) => {
                        if let StreamElement::Stream(measure) = part.events()[index].element() {
                            measure.unique_lengths(&mut lengths);
                        }
                    }
                    None => part.unique_lengths(&mut lengths),
                }
            }
            let divisor = approximate_gcd(&lengths)?;
            for &part_index in &parts {
                let StreamElement::Stream(part) = self.events()[part_index].element() else {
                    continue;
                };
                let path: Vec<usize> = match measure_of(part, i)? {
                    Some(index) => vec![part_index, index],
                    None => vec![part_index],
                };
                let (result, _) = self.edit_nested(&path, |target| {
                    match target.slice_lengths_in_place(&[divisor]) {
                        Ok(moved) => (Ok(()), moved),
                        Err(error) => (Err(error), unmoved(target)),
                    }
                });
                result?;
            }
        }
        Ok(())
    }

    /// A copy of the stream with each note and rest sounding across one of
    /// `offsets` cut there, tied: music21's `sliceAtOffsets`. A stream of
    /// measures is cut measure by measure, each at the offsets that fall in
    /// it, and a score part by part. A note in a voice is cut in its voice,
    /// and the pieces after the first are put in the measure itself, as
    /// music21 puts them.
    ///
    /// # Errors
    ///
    /// A note that cannot be cut at a point.
    pub fn slice_at_offsets(&self, offsets: &[FloatType]) -> Result<Stream> {
        let mut out = self.clone();
        out.slice_offsets_in_place(offsets)?;
        Ok(out)
    }

    fn slice_offsets_in_place(&mut self, offsets: &[FloatType]) -> Result<Moved> {
        if self.has_measures() {
            return each_nested(self, is_measure, &mut |measure, at| {
                let local: Vec<FloatType> = offsets.iter().map(|offset| offset - at).collect();
                measure.slice_offsets_in_place(&local)
            });
        }
        if self.has_part_like_streams() {
            return each_nested(
                self,
                |element| matches!(element, StreamElement::Stream(_)),
                &mut |part, at| {
                    let local: Vec<FloatType> = offsets.iter().map(|offset| offset - at).collect();
                    part.slice_offsets_in_place(&local)
                },
            );
        }
        let offsets: Vec<FloatType> = offsets.iter().map(|offset| op_frac(*offset)).collect();
        let cuts = |start: FloatType, end: FloatType| -> Vec<FloatType> {
            let (start, end) = (op_frac(start), op_frac(end));
            offsets
                .iter()
                .copied()
                .filter(|offset| start < *offset && *offset < end)
                .collect()
        };
        // Cuts each element in turn: the first piece in its place, the
        // others gathered to be put in this stream.
        let mut added: Vec<StreamEvent> = Vec::new();
        let mut cut_one = |element: &mut StreamElement, start: FloatType| -> Result<()> {
            if !element.is_instance("GeneralNote") {
                return Ok(());
            }
            let points = cuts(start, start + element.quarter_length());
            let mut previous = start;
            let mut current: Option<StreamElement> = None;
            for point in points {
                let target = current.as_ref().unwrap_or(element);
                let (left, right) = split_element(target, op_frac(point - previous))?;
                match current.take() {
                    None => *element = left,
                    Some(_) => {
                        if let Some(last) = added.last_mut() {
                            *last = StreamEvent::new(last.offset(), left);
                        }
                    }
                }
                added.push(StreamEvent::new(point, right.clone()));
                current = Some(right);
                previous = point;
            }
            Ok(())
        };
        let has_voices = self.has_voices();
        let mut events: Vec<StreamEvent> = self.events().to_vec();
        if has_voices {
            for event in events.iter_mut() {
                if !matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice)
                {
                    let start = event.offset();
                    cut_one(event.element_mut(), start)?;
                }
            }
            // music21 reads a voice's elements at their offsets in the
            // voice.
            for event in events.iter_mut() {
                if let StreamElement::Stream(voice) = event.element_mut()
                    && voice.kind() == StreamKind::Voice
                {
                    for inner in voice.events_mut() {
                        let start = inner.offset();
                        cut_one(inner.element_mut(), start)?;
                    }
                }
            }
        } else {
            for event in events.iter_mut() {
                let start = event.offset();
                cut_one(event.element_mut(), start)?;
            }
        }
        let count = events.len();
        let mut held: Vec<(StreamEvent, Option<usize>, usize)> = events
            .into_iter()
            .enumerate()
            .map(|(index, event)| (event, Some(index), index))
            .collect();
        held.extend(
            added
                .into_iter()
                .enumerate()
                .map(|(rank, event)| (event, None, count + rank)),
        );
        Ok(reorder(self, held))
    }

    /// A copy of the stream with each note and rest cut at the beats of
    /// its time signature: music21's `sliceByBeat`. A stream of measures is
    /// cut measure by measure, each at the beats of the time signature in
    /// force there, and a score part by part. A stream with no time
    /// signature is cut at the beats of 4/4.
    ///
    /// # Errors
    ///
    /// A measure or stream holding more than one time signature, which
    /// music21 does not cut.
    pub fn slice_by_beat(&self) -> Result<Stream> {
        let mut out = self.clone();
        let mut meter = None;
        out.slice_beat_in_place(&mut meter)?;
        Ok(out)
    }

    fn slice_beat_in_place(&mut self, meter: &mut Option<TimeSignature>) -> Result<Moved> {
        if self.has_measures() {
            return each_nested(self, is_measure, &mut |measure, _| {
                measure.slice_beat_in_place(meter)
            });
        }
        if self.has_part_like_streams() {
            return each_nested(self, is_part, &mut |part, _| {
                let mut own = None;
                part.slice_beat_in_place(&mut own)
            });
        }
        let own: Vec<TimeSignature> = self
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::TimeSignature(found) => Some(found.clone()),
                _ => None,
            })
            .collect();
        if own.len() > 1 {
            return Err(stream_error(
                "not yet implemented: slice by changing time signatures",
            ));
        }
        if let Some(found) = own.into_iter().next() {
            *meter = Some(found);
        }
        let offsets = match meter {
            Some(found) => found.beat_offsets(),
            None => TimeSignature::new(4, 4)?.beat_offsets(),
        };
        self.slice_offsets_in_place(&offsets)
    }

    /// The elements this stream holds itself in two streams of its kind:
    /// those `keep` accepts and the rest, at the offsets they stand at:
    /// music21's `splitByClass` with no class.
    pub fn split_by(&self, keep: impl Fn(&StreamElement) -> bool) -> (Stream, Stream) {
        let (mut kept, mut rest) = (
            Stream::with_kind(self.kind()),
            Stream::with_kind(self.kind()),
        );
        let (mut ours, mut theirs) = (Vec::new(), Vec::new());
        for event in self.events() {
            if keep(event.element()) {
                ours.push(event.clone());
            } else {
                theirs.push(event.clone());
            }
        }
        kept.insert_sorted(ours);
        rest.insert_sorted(theirs);
        (kept, rest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_greatest_divisor_is_music21s() -> Result<()> {
        // Read off music21's common.approximateGCD.
        assert_eq!(approximate_gcd(&[2.0, 0.5])?, 0.5);
        assert_eq!(approximate_gcd(&[2.0, 1.5, 1.0])?, 0.5);
        assert_eq!(approximate_gcd(&[2.0, 1.5, 1.0, 1.0 / 3.0])?, 1.0 / 6.0);
        assert!(approximate_gcd(&[1.0, 0.0]).is_err());
        assert!(approximate_gcd(&[]).is_err());
        Ok(())
    }
}
