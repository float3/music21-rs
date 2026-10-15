//! What a stream holds and where: music21's questions of a stream's layout
//! -- its gaps and overlaps, what sounds when, its measures by offset.

use super::span::OffsetSpan;
use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::defaults::FloatType;
use crate::makenotation::op_frac;
use crate::rest::Rest;

/// One element of a stream with where it starts and ends and the voice it
/// is in: music21's `OffsetMap`.
#[derive(Clone, Debug)]
pub struct OffsetMapEntry {
    /// The element.
    pub element: StreamElement,
    /// Where it starts.
    pub offset: FloatType,
    /// Where it ends.
    pub end_time: FloatType,
    /// The voice it is in, by its place among the stream's voices, or
    /// nothing for an element outside them.
    pub voice: Option<usize>,
}

impl Stream {
    /// Where the last element this stream holds itself starts, nought for
    /// an empty stream: music21's `highestOffset`.
    pub fn highest_offset(&self) -> FloatType {
        self.events
            .iter()
            .map(StreamEvent::offset)
            .fold(None, |most: Option<FloatType>, offset| {
                Some(most.map_or(offset, |most| most.max(offset)))
            })
            .unwrap_or(0.0)
    }

    /// Where the first element this stream holds itself starts, nought for
    /// an empty stream: music21's `lowestOffset`.
    pub fn lowest_offset(&self) -> FloatType {
        self.events
            .iter()
            .map(StreamEvent::offset)
            .fold(None, |least: Option<FloatType>, offset| {
                Some(least.map_or(offset, |least| least.min(offset)))
            })
            .unwrap_or(0.0)
    }

    /// Whether this stream holds a measure itself: music21's `hasMeasures`.
    pub fn has_measures(&self) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure))
    }

    /// Whether this stream holds a voice itself: music21's `hasVoices`.
    pub fn has_voices(&self) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice))
    }

    /// Whether the stream is notation as music21 writes it out: a measure
    /// or a voice; a part of measures; a score or other stream of parts, or
    /// an opus of scores, whose every stream holds measures; and nothing
    /// holding notes or rests loose: music21's `isWellFormedNotation`.
    pub fn is_well_formed_notation(&self) -> bool {
        let all_hold_measures = |stream: &Stream| {
            stream
                .events
                .iter()
                .filter_map(|event| event.element().as_stream())
                .all(Stream::has_measures)
        };
        match self.kind() {
            StreamKind::Measure | StreamKind::Voice => true,
            _ if self
                .events
                .iter()
                .any(|event| event.element().is_instance("GeneralNote")) =>
            {
                false
            }
            StreamKind::Part | StreamKind::PartStaff => self.has_measures(),
            StreamKind::Opus => self
                .streams_of_kind(StreamKind::Score)
                .into_iter()
                .all(all_hold_measures),
            _ if self.has_part_like_streams() => all_hold_measures(self),
            _ => false,
        }
    }

    /// The stretches of this stream, between its own elements, where
    /// nothing sounds, each as a rest, or nothing where there are none:
    /// music21's `findGaps`.
    ///
    /// ```
    /// use music21_rs::{Duration, Note, Stream};
    ///
    /// let mut line = Stream::new();
    /// line.insert(0.0, Note::from_name("C4")?.with_duration(Duration::quarter()));
    /// line.insert(2.0, Note::from_name("D4")?.with_duration(Duration::quarter()));
    /// let gaps = line.find_gaps().expect("a gap");
    /// assert_eq!(gaps.events()[0].offset(), 1.0);
    /// assert_eq!(gaps.events()[0].element().quarter_length(), 1.0);
    /// assert!(!line.is_gapless());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn find_gaps(&self) -> Option<Stream> {
        let mut gaps = Stream::with_kind(self.kind());
        let mut highest: FloatType = 0.0;
        for event in &self.events {
            let offset = event.offset();
            if offset > highest {
                let length = op_frac(offset - highest);
                if let Ok(duration) = crate::duration::Duration::new(length) {
                    gaps.insert_sorted(vec![StreamEvent::new(
                        highest,
                        StreamElement::Rest(Rest::new(duration)),
                    )]);
                }
            }
            highest = op_frac(highest.max(offset + event.element().quarter_length()));
        }
        (!gaps.is_empty()).then_some(gaps)
    }

    /// Whether this stream has no gaps: music21's `isGapless`.
    pub fn is_gapless(&self) -> bool {
        self.find_gaps().is_none()
    }

    /// The flattened stream, each element with where it starts and ends.
    fn spans(&self) -> (Stream, Vec<(FloatType, FloatType)>) {
        let flat = self.flatten();
        let spans = flat
            .events
            .iter()
            .map(|event| {
                let offset = event.offset();
                (offset, op_frac(offset + event.element().quarter_length()))
            })
            .collect();
        (flat, spans)
    }

    /// music21's `_findLayering`: for each element of the flattened stream,
    /// the others it overlaps, found looking on from it until the first that
    /// does not.
    fn layering(spans: &[(FloatType, FloatType)]) -> Vec<Vec<usize>> {
        let overlap = |a: (FloatType, FloatType), b: (FloatType, FloatType)| {
            let (first, second) = if (b.0, b.1) < (a.0, a.1) {
                (b, a)
            } else {
                (a, b)
            };
            second.0 < first.1
        };
        let mut map = vec![Vec::new(); spans.len()];
        for i in 0..spans.len() {
            for j in (i + 1)..spans.len() {
                if overlap(spans[i], spans[j]) {
                    map[i].push(j);
                    map[j].push(i);
                } else {
                    break;
                }
            }
        }
        for list in &mut map {
            list.sort_unstable();
        }
        map
    }

    /// The elements of the flattened stream that sound over one another,
    /// gathered by the offset where each overlapping group starts:
    /// music21's `getOverlaps`. Only elements found overlapping looking on
    /// from one until the first that does not are gathered, as music21
    /// gathers them.
    ///
    /// ```
    /// use music21_rs::{Duration, Note, Stream};
    ///
    /// let mut line = Stream::new();
    /// line.insert(0.0, Note::from_name("C4")?.with_duration(Duration::half()));
    /// line.insert(1.0, Note::from_name("E4")?.with_duration(Duration::quarter()));
    /// line.insert(3.0, Note::from_name("G4")?.with_duration(Duration::quarter()));
    /// let overlaps = line.overlaps();
    /// assert_eq!(overlaps.len(), 1);
    /// assert_eq!(overlaps[0].0, 0.0);
    /// assert_eq!(overlaps[0].1.len(), 2);
    /// assert!(!line.is_sequence());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn overlaps(&self) -> Vec<(FloatType, Vec<StreamEvent>)> {
        let flat = self.flatten();
        self.overlap_groups()
            .into_iter()
            .map(|(offset, members)| {
                (
                    offset,
                    members
                        .into_iter()
                        .map(|index| flat.events[index].clone())
                        .collect(),
                )
            })
            .collect()
    }

    /// [`Stream::overlaps`], each group's members by their position among
    /// [`Stream::leaves`] rather than as values.
    ///
    /// music21 hands back the elements themselves, so a caller keeping an
    /// object per element -- a binding keeping what it read the stream
    /// from -- finds each again by that position.
    ///
    /// ```
    /// use music21_rs::{Duration, Note, Stream, StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// let mut measure = Stream::with_kind(StreamKind::Measure);
    /// measure.insert(0.0, Note::from_name("C4")?.with_duration(Duration::half()));
    /// measure.insert(1.0, Note::from_name("E4")?);
    /// part.insert(0.0, measure);
    ///
    /// // The two notes overlap, and are the part's first two leaves. The
    /// // one overlapped is listed before the one looking on from itself,
    /// // which is the order music21's own dictionary takes them in.
    /// let found = part.overlap_positions();
    /// assert_eq!(found.len(), 1);
    /// assert_eq!(found[0], (0.0, vec![1, 0]));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn overlap_positions(&self) -> Vec<(FloatType, Vec<usize>)> {
        let leaves = self.leaves();
        // The flattened stream is the leaves in `flatten`'s order, so the
        // nth element of it is the leaf that order puts nth.
        let order = Stream::flatten_order(&leaves);
        self.overlap_groups()
            .into_iter()
            .map(|(offset, members)| {
                (
                    offset,
                    members.into_iter().map(|index| order[index]).collect(),
                )
            })
            .collect()
    }

    /// The groups [`Stream::overlaps`] finds, each holding indices into the
    /// flattened stream, in the order music21's dictionary takes them.
    fn overlap_groups(&self) -> Vec<(FloatType, Vec<usize>)> {
        let (flat, spans) = self.spans();
        let map = Self::layering(&spans);
        // Each group by its offset, holding indices of the flattened stream,
        // in the order music21's dictionary takes them.
        let mut groups: Vec<(FloatType, Vec<usize>)> = Vec::new();
        let held_at = |groups: &[(FloatType, Vec<usize>)], index: usize| {
            groups
                .iter()
                .find(|(_, members)| members.contains(&index))
                .map(|(offset, _)| *offset)
        };
        let push = |groups: &mut Vec<(FloatType, Vec<usize>)>, offset: FloatType, index: usize| {
            match groups.iter_mut().find(|(known, _)| *known == offset) {
                Some((_, members)) => members.push(index),
                None => groups.push((offset, vec![index])),
            }
        };
        for (index, indices) in map.iter().enumerate() {
            if indices.is_empty() {
                continue;
            }
            let source = flat.events[index].offset();
            let mut destination = None;
            for &other in indices {
                let mut store = true;
                if let Some(offset) = held_at(&groups, other) {
                    store = false;
                    destination = Some(offset);
                }
                let at = *destination.get_or_insert(source);
                if store {
                    push(&mut groups, at, other);
                }
            }
            if held_at(&groups, index).is_none() {
                let at = destination.unwrap_or(source);
                push(&mut groups, at, index);
            }
        }
        groups
    }

    /// Whether no element of the flattened stream overlaps another, as
    /// [`Stream::overlaps`] finds them: music21's `isSequence`.
    pub fn is_sequence(&self) -> bool {
        let (_, spans) = self.spans();
        Self::layering(&spans).iter().all(Vec::is_empty)
    }

    /// The offsets where something in this stream and something in
    /// `other` both start, each stream's own elements: music21's
    /// `simultaneousAttacks`.
    pub fn simultaneous_attacks(&self, other: &Stream) -> Vec<FloatType> {
        let mut offsets: Vec<FloatType> = Vec::new();
        for event in &self.events {
            if !offsets.contains(&event.offset()) {
                offsets.push(event.offset());
            }
        }
        let mut shared: Vec<FloatType> = offsets
            .into_iter()
            .filter(|offset| other.events.iter().any(|event| event.offset() == *offset))
            .collect();
        shared.sort_by(FloatType::total_cmp);
        shared
    }

    /// The element of this stream sounding where `element` starts, at
    /// `offset`: the one there, or of those there the first of `element`'s
    /// class, or else the first; nothing where none is: music21's
    /// `playingWhenAttacked`.
    pub fn playing_when_attacked(
        &self,
        element: &StreamElement,
        offset: FloatType,
    ) -> Option<&StreamEvent> {
        let mut span = OffsetSpan::at(offset);
        span.must_begin_in_span = false;
        let there: Vec<&StreamEvent> = self
            .events
            .iter()
            .filter(|event| span.takes_at(event.offset(), event.element().quarter_length()))
            .collect();
        match there.as_slice() {
            [] => None,
            [only] => Some(*only),
            many => {
                let class = element.music21_classes()[0];
                many.iter()
                    .find(|event| event.element().is_instance(class))
                    .or_else(|| many.first())
                    .copied()
            }
        }
    }

    /// Every element of this stream sounding while `element`, at `offset`,
    /// sounds, each at its offset from `offset`: music21's
    /// `allPlayingWhileSounding`.
    pub fn all_playing_while_sounding(&self, element: &StreamElement, offset: FloatType) -> Stream {
        let end = offset + element.quarter_length();
        let span = if end != offset {
            OffsetSpan {
                must_begin_in_span: false,
                include_end_boundary: false,
                include_elements_that_end_at_start: false,
                ..OffsetSpan::between(offset, end)
            }
        } else {
            OffsetSpan {
                must_begin_in_span: false,
                ..OffsetSpan::at(offset)
            }
        };
        let mut out = Stream::with_kind(self.kind());
        out.events = self
            .events
            .iter()
            .filter(|event| span.takes_at(event.offset(), event.element().quarter_length()))
            .map(|event| {
                StreamEvent::new(op_frac(event.offset() - offset), event.element().clone())
            })
            .collect();
        out
    }

    /// Each element of this stream with where it starts and ends, those of
    /// its voices flattened and labelled with the voice, barlines left out:
    /// music21's `offsetMap`.
    pub fn offset_map(&self) -> Vec<OffsetMapEntry> {
        let entry = |event: &StreamEvent, voice: Option<usize>| OffsetMapEntry {
            element: event.element().clone(),
            offset: event.offset(),
            end_time: op_frac(event.offset() + event.element().quarter_length()),
            voice,
        };
        let keep = |event: &&StreamEvent| !matches!(event.element(), StreamElement::Barline(_));
        if !self.has_voices() {
            return self
                .events
                .iter()
                .filter(keep)
                .map(|event| entry(event, None))
                .collect();
        }
        let mut out: Vec<OffsetMapEntry> = self
            .events
            .iter()
            .filter(|event| !matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Voice))
            .filter(keep)
            .map(|event| entry(event, None))
            .collect();
        for (index, voice) in self.voices().into_iter().enumerate() {
            let flat = voice.flatten();
            out.extend(
                flat.events
                    .iter()
                    .filter(keep)
                    .map(|event| entry(event, Some(index))),
            );
        }
        out
    }

    /// The measures by offset, in order: a score's those of every part at
    /// each offset, and another stream's its own: music21's
    /// `measureOffsetMap`.
    pub fn measure_offset_map(&self) -> Vec<(FloatType, Vec<&Stream>)> {
        let mut map: Vec<(FloatType, Vec<&Stream>)> = Vec::new();
        let parts = self.parts();
        let holders: Vec<&Stream> = if self.kind() == StreamKind::Score && !parts.is_empty() {
            parts
        } else {
            vec![self]
        };
        for holder in holders {
            for event in &holder.events {
                let StreamElement::Stream(measure) = event.element() else {
                    continue;
                };
                if measure.kind() != StreamKind::Measure {
                    continue;
                }
                match map.iter_mut().find(|(offset, _)| *offset == event.offset()) {
                    Some((_, measures)) => measures.push(measure),
                    None => map.push((event.offset(), vec![measure])),
                }
            }
        }
        map.sort_by(|left, right| left.0.total_cmp(&right.0));
        map
    }
}
