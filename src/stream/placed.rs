//! An element together with where it stands: music21's sites, answered
//! as a composite rather than stored on the element.
//!
//! music21 keeps on every object a back-reference to each stream holding
//! it, so that `getContextByClass` can walk up to the key or metre in
//! force; that is why its objects are deep-copied so often. Its author
//! would do it differently: "store objects and sites separately and return
//! them as a separate composite object" ("Music21's Mistakes", point 2).
//! [`Stream::placed`] is that composite. The element stays a plain value in
//! the tree; where it sits, and what is in force there, is worked out from
//! the tree when asked.
//!
//! ```text
//!   Score ─┬─ Part 1 ─┬─ Measure 1: KeySignature(-1), C4 …
//!          │          └─ Measure 2: D4 …   ─► Placed { D4, offset 4.0,
//!          │                                     measure offset 0.0,
//!          │                                     key: one flat }
//!          └─ Part 2 ─── Measure 1: E4 …   ─► Placed { E4, key: none }
//! ```

use super::{Stream, StreamElement, StreamKind};
use crate::{
    Duration, Result, defaults::FloatType, key::KeySignature, meter::TimeSignature,
    tempo::MetronomeMark,
};

/// Which marks a leaf reads: those of its own part, and those held by the
/// stream asked, which stand over every part.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Shared,
    Part(usize),
}

impl Scope {
    fn reaches(self, leaf: Scope) -> bool {
        self == Scope::Shared || self == leaf
    }
}

/// One element of a stream with its place in it: its offset, its offset in
/// the measure holding it, and the key signature, metre and tempo in force
/// where it stands.
#[derive(Clone, Debug)]
pub struct Placed<'a> {
    element: &'a StreamElement,
    offset: FloatType,
    measure_offset: Option<FloatType>,
    /// Where the element stands in the bar before its meter is applied:
    /// music21's `_getMeasureOffset`, the offset in its measure plus the
    /// measure's padding, or its offset in the stream holding it.
    bar_position: FloatType,
    key_signature: Option<&'a KeySignature>,
    time_signature: Option<&'a TimeSignature>,
    /// The meter's own place in its bar, its padding left out, as music21's
    /// `_getMeasureOffset(includeMeasurePadding=False)` reads it.
    time_signature_position: FloatType,
    metronome_mark: Option<&'a MetronomeMark>,
}

impl<'a> Placed<'a> {
    /// The element itself.
    pub fn element(&self) -> &'a StreamElement {
        self.element
    }

    /// The offset from the start of the stream asked.
    pub fn offset(&self) -> FloatType {
        self.offset
    }

    /// The offset from the start of the measure holding the element, or
    /// `None` outside any measure.
    pub fn measure_offset(&self) -> Option<FloatType> {
        self.measure_offset
    }

    /// The key signature in force where the element stands.
    pub fn key_signature(&self) -> Option<&'a KeySignature> {
        self.key_signature
    }

    /// The metre in force where the element stands.
    pub fn time_signature(&self) -> Option<&'a TimeSignature> {
        self.time_signature
    }

    /// The tempo in force where the element stands.
    pub fn metronome_mark(&self) -> Option<&'a MetronomeMark> {
        self.metronome_mark
    }

    /// Where the element falls in the bar of the meter in force: music21's
    /// `getMeasureOffsetOrMeterModulusOffset`. Inside a measure that is its
    /// place in it, padding included; outside one, the meter's bars are
    /// counted on from where the meter stands. `None` with no meter.
    ///
    /// ```text
    ///   3/4 at 0, no measures:  note at 4.0  ->  bar offset 1.0
    ///   4/4 pickup, padding 3:  note at 0.0  ->  bar offset 3.0
    /// ```
    pub fn bar_offset(&self) -> Option<FloatType> {
        let meter = self.time_signature?;
        Some(meter.offset_in_bar(self.bar_position, self.time_signature_position))
    }

    /// The beat the element falls on, counted from 1 with the progress
    /// through it as a fraction: music21's `beat`. `None` with no meter,
    /// where music21 answers `nan`.
    ///
    /// # Errors
    ///
    /// An offset the meter cannot place, where music21 raises.
    pub fn beat(&self) -> Result<Option<FloatType>> {
        self.read_bar(TimeSignature::beat_proportion)
    }

    /// [`Placed::beat`] written as music21's `beatStr` writes it: `2 1/2`.
    ///
    /// # Errors
    ///
    /// As [`Placed::beat`].
    pub fn beat_string(&self) -> Result<Option<String>> {
        self.read_bar(TimeSignature::beat_proportion_string)
    }

    /// How long the beat holding the element is: music21's `beatDuration`.
    /// `None` with no meter, where music21 answers a duration of nothing.
    ///
    /// # Errors
    ///
    /// As [`Placed::beat`].
    pub fn beat_duration(&self) -> Result<Option<Duration>> {
        self.read_bar(TimeSignature::beat_duration_at)
    }

    /// The metrical accent of the element's place: music21's
    /// `beatStrength`, which weighs a place that starts no partition at
    /// half the smallest weight. `None` with no meter.
    ///
    /// # Errors
    ///
    /// As [`Placed::beat`].
    pub fn beat_strength(&self) -> Result<Option<FloatType>> {
        self.read_bar(|meter, offset| meter.accent_weight_with(offset, true, false))
    }

    /// One reading of the meter in force at the element's bar offset.
    fn read_bar<T>(
        &self,
        read: impl Fn(&TimeSignature, FloatType) -> Result<T>,
    ) -> Result<Option<T>> {
        let (Some(meter), Some(offset)) = (self.time_signature, self.bar_offset()) else {
            return Ok(None);
        };
        read(meter, offset).map(Some)
    }
}

/// A leaf as the walk finds it, before its context is looked up.
struct Leaf<'a> {
    element: &'a StreamElement,
    offset: FloatType,
    measure_start: Option<FloatType>,
    /// The offset in the stream directly holding it.
    site_offset: FloatType,
    /// The padding of the measure holding it, nothing outside one.
    padding: FloatType,
    scope: Scope,
}

impl Leaf<'_> {
    /// music21's `_getMeasureOffset`: the place in the measure, with or
    /// without the measure's padding, or the place in the stream holding
    /// it outside a measure.
    fn bar_position(&self, padded: bool) -> FloatType {
        let Some(start) = self.measure_start else {
            return self.site_offset;
        };
        let padding = if padded { self.padding } else { 0.0 };
        crate::makenotation::op_frac(self.offset - start + padding)
    }
}

impl Stream {
    /// Every element that is not itself a stream, in [`Stream::leaves`]
    /// order, each with where it stands and what is in force there.
    ///
    /// A mark is in force from its offset on, the last one at or before an
    /// element winning. In a stream holding parts, each part reads its own
    /// marks and those the stream holds directly, never another part's,
    /// which is music21's `getContextByClass` for music written once.
    ///
    /// ```
    /// use music21_rs::{KeySignature, Note, Stream, StreamElement, stream::StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// part.insert(0.0, KeySignature::new(-1));
    /// part.insert(0.0, Note::from_name("B-4")?);
    /// let placed = part.placed();
    /// let note = placed
    ///     .iter()
    ///     .find(|placed| matches!(placed.element(), StreamElement::Note(_)))
    ///     .unwrap();
    /// assert_eq!(note.key_signature().map(KeySignature::sharps), Some(-1));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn placed(&self) -> Vec<Placed<'_>> {
        let parts = self.has_part_like_streams();
        let mut leaves = Vec::new();
        for (index, event) in self.events.iter().enumerate() {
            let StreamElement::Stream(stream) = &event.element else {
                leaves.push(Leaf {
                    element: &event.element,
                    offset: event.offset,
                    measure_start: self.measure_start(0.0),
                    site_offset: event.offset,
                    padding: self.measure_padding(),
                    scope: Scope::Shared,
                });
                continue;
            };

            let scope = if parts {
                Scope::Part(index)
            } else {
                Scope::Shared
            };
            let measure_start = self.measure_start(0.0);
            stream.gather(
                event.offset,
                measure_start,
                self.measure_padding(),
                scope,
                &mut leaves,
            );
        }

        leaves
            .iter()
            .map(|leaf| {
                let meter = in_force(&leaves, leaf, |element| match element {
                    StreamElement::TimeSignature(meter) => Some(meter),
                    _ => None,
                });
                Placed {
                    element: leaf.element,
                    offset: leaf.offset,
                    measure_offset: leaf.measure_start.map(|start| leaf.offset - start),
                    bar_position: leaf.bar_position(true),
                    key_signature: in_force(&leaves, leaf, |element| match element {
                        StreamElement::KeySignature(key) => Some(key),
                        _ => None,
                    })
                    .map(|(_, key)| key),
                    time_signature: meter.map(|(_, meter)| meter),
                    time_signature_position: meter
                        .map_or(0.0, |(mark, _)| mark.bar_position(false)),
                    metronome_mark: in_force(&leaves, leaf, |element| match element {
                        StreamElement::MetronomeMark(mark) => Some(mark),
                        _ => None,
                    })
                    .map(|(_, mark)| mark),
                }
            })
            .collect()
    }

    /// Where a measure starts, when this stream is one starting at `base`.
    fn measure_start(&self, base: FloatType) -> Option<FloatType> {
        (self.kind == StreamKind::Measure).then_some(base)
    }

    /// A measure's padding, nothing for any other stream.
    fn measure_padding(&self) -> FloatType {
        match self.kind {
            StreamKind::Measure => self.padding_left(),
            _ => 0.0,
        }
    }

    fn gather<'a>(
        &'a self,
        base: FloatType,
        measure_start: Option<FloatType>,
        padding: FloatType,
        scope: Scope,
        out: &mut Vec<Leaf<'a>>,
    ) {
        // The innermost measure is the one whose place counts.
        let (measure_start, padding) = match self.measure_start(base) {
            Some(start) => (Some(start), self.measure_padding()),
            None => (measure_start, padding),
        };
        for event in &self.events {
            let offset = base + event.offset;
            if let StreamElement::Stream(stream) = &event.element {
                stream.gather(offset, measure_start, padding, scope, out);
                continue;
            }

            out.push(Leaf {
                element: &event.element,
                offset,
                measure_start,
                site_offset: event.offset,
                padding,
                scope,
            });
        }
    }
}

/// The last mark of one sort at or before a leaf that the leaf's scope
/// reaches; of two at one offset, the later-listed. The mark's own leaf
/// comes with it, for where it stands.
fn in_force<'l, 'a, T>(
    leaves: &'l [Leaf<'a>],
    leaf: &Leaf<'a>,
    read: impl Fn(&'a StreamElement) -> Option<&'a T>,
) -> Option<(&'l Leaf<'a>, &'a T)> {
    let mut found: Option<(&'l Leaf<'a>, &'a T)> = None;
    for mark in leaves {
        if mark.offset > leaf.offset || !mark.scope.reaches(leaf.scope) {
            continue;
        }
        let Some(value) = read(mark.element) else {
            continue;
        };
        if found.is_none_or(|(held, _)| mark.offset >= held.offset) {
            found = Some((mark, value));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::Placed;
    use crate::{
        Duration, KeySignature, Note, Stream, StreamElement, meter::TimeSignature,
        stream::StreamKind,
    };

    fn measure(notes: &[&str]) -> Stream {
        let mut measure = Stream::with_kind(StreamKind::Measure);
        for name in notes {
            measure.push(Note::from_name(name).unwrap());
        }
        measure
    }

    /// A part's key reaches its later measures but not the other part.
    #[test]
    fn each_part_reads_its_own_marks() {
        let mut first = measure(&["C4", "D4", "E4", "F4"]);
        first.insert(0.0, KeySignature::new(-1));
        let mut flute = Stream::with_kind(StreamKind::Part);
        flute.insert(0.0, first);
        flute.insert(4.0, measure(&["G4"]));
        let mut oboe = Stream::with_kind(StreamKind::Part);
        oboe.insert(0.0, measure(&["A4"]));
        let mut score = Stream::with_kind(StreamKind::Score);
        score.insert(0.0, TimeSignature::new(4, 4).unwrap());
        score.insert(0.0, flute);
        score.insert(0.0, oboe);

        let notes: Vec<_> = score
            .placed()
            .into_iter()
            .filter(|placed| matches!(placed.element(), StreamElement::Note(_)))
            .collect();
        let sharps = |index: usize| notes[index].key_signature().map(KeySignature::sharps);

        assert_eq!(notes.len(), 6);
        assert_eq!(sharps(4), Some(-1));
        assert_eq!(notes[4].offset(), 4.0);
        assert_eq!(notes[4].measure_offset(), Some(0.0));
        assert_eq!(notes[3].measure_offset(), Some(3.0));
        assert_eq!(sharps(5), None);
        assert!(notes.iter().all(|placed| placed.time_signature().is_some()));
    }

    /// The composite lists what `leaves` lists, in the same order.
    #[test]
    fn placed_follows_leaves() {
        let mut part = Stream::with_kind(StreamKind::Part);
        part.insert(0.0, measure(&["C4", "D4"]));
        part.insert(2.0, measure(&["E4"]));
        let offsets: Vec<_> = part.placed().iter().map(|placed| placed.offset()).collect();
        let leaves: Vec<_> = part.leaves().iter().map(|(offset, _)| *offset).collect();
        assert_eq!(offsets, leaves);
    }

    /// `count` notes of one length, in a stream of `kind` under `meter`.
    fn repeated(kind: StreamKind, meter: &str, length: f64, count: usize) -> Stream {
        let mut stream = Stream::with_kind(kind);
        stream.insert(0.0, TimeSignature::from_ratio_string(meter).unwrap());
        for _ in 0..count {
            let mut note = Note::from_name("C4").unwrap();
            note.set_duration(Duration::new(length).unwrap());
            stream.push(note);
        }
        stream
    }

    fn notes_of(stream: &Stream) -> Vec<Placed<'_>> {
        stream
            .placed()
            .into_iter()
            .filter(|placed| matches!(placed.element(), StreamElement::Note(_)))
            .collect()
    }

    /// music21's `beat` and `beatStrength` examples, read off a measure.
    #[test]
    fn a_measure_counts_its_beats() {
        let three = repeated(StreamKind::Measure, "3/4", 0.5, 6);
        let beats: Vec<_> = notes_of(&three)
            .iter()
            .map(|placed| placed.beat().unwrap().unwrap())
            .collect();
        assert_eq!(beats, [1.0, 1.5, 2.0, 2.5, 3.0, 3.5]);
        let strengths: Vec<_> = notes_of(&three)
            .iter()
            .map(|placed| placed.beat_strength().unwrap().unwrap())
            .collect();
        assert_eq!(strengths, [1.0, 0.25, 0.5, 0.25, 0.5, 0.25]);

        let six = repeated(StreamKind::Measure, "6/8", 0.5, 6);
        let written: Vec<_> = notes_of(&six)
            .iter()
            .map(|placed| placed.beat_string().unwrap().unwrap())
            .collect();
        assert_eq!(written, ["1", "1 1/3", "1 2/3", "2", "2 1/3", "2 2/3"]);
        let strengths: Vec<_> = notes_of(&six)
            .iter()
            .map(|placed| placed.beat_strength().unwrap().unwrap())
            .collect();
        assert_eq!(strengths, [1.0, 0.25, 0.25, 0.5, 0.25, 0.25]);
    }

    /// Outside a measure the meter wraps, as if its bars went on.
    #[test]
    fn a_stream_without_measures_wraps_at_the_bar() {
        let three = repeated(StreamKind::Stream, "3/4", 1.0, 8);
        let beats: Vec<_> = notes_of(&three)
            .iter()
            .map(|placed| placed.beat().unwrap().unwrap())
            .collect();
        assert_eq!(beats, [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0]);

        let additive = repeated(StreamKind::Stream, "2/4+3/4", 1.0, 8);
        let lengths: Vec<_> = notes_of(&additive)
            .iter()
            .map(|placed| placed.beat_duration().unwrap().unwrap().quarter_length())
            .collect();
        assert_eq!(lengths, [2.0, 2.0, 3.0, 3.0, 3.0, 2.0, 2.0, 3.0]);

        // A second meter counts from where it stands.
        let mut changing = repeated(StreamKind::Stream, "2/2", 1.0, 12);
        changing.insert(4.0, TimeSignature::from_ratio_string("3/4").unwrap());
        let strengths: Vec<_> = notes_of(&changing)
            .iter()
            .map(|placed| placed.beat_strength().unwrap().unwrap())
            .collect();
        assert_eq!(
            strengths,
            [1.0, 0.25, 0.5, 0.25, 1.0, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 0.5]
        );
    }

    /// A pickup's padding counts toward where its notes fall in the bar.
    #[test]
    fn a_pickup_counts_from_its_padding() {
        let mut pickup = repeated(StreamKind::Measure, "4/4", 1.0, 1);
        pickup.set_padding_left(3.0);
        let notes = notes_of(&pickup);
        assert_eq!(notes[0].bar_offset(), Some(3.0));
        assert_eq!(notes[0].beat().unwrap(), Some(4.0));
    }

    /// With no meter in force there is no beat to tell.
    #[test]
    fn no_meter_no_beat() {
        let mut bare = Stream::new();
        bare.push(Note::from_name("E4").unwrap());
        let notes = notes_of(&bare);
        assert_eq!(notes[0].bar_offset(), None);
        assert_eq!(notes[0].beat().unwrap(), None);
        assert_eq!(notes[0].beat_strength().unwrap(), None);
    }
}
