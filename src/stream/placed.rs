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
use crate::{defaults::FloatType, key::KeySignature, meter::TimeSignature, tempo::MetronomeMark};

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
    key_signature: Option<&'a KeySignature>,
    time_signature: Option<&'a TimeSignature>,
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
}

/// A leaf as the walk finds it, before its context is looked up.
struct Leaf<'a> {
    element: &'a StreamElement,
    offset: FloatType,
    measure_start: Option<FloatType>,
    scope: Scope,
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
    /// assert_eq!(note.key_signature().and_then(KeySignature::sharps), Some(-1));
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
            stream.gather(event.offset, measure_start, scope, &mut leaves);
        }

        leaves
            .iter()
            .map(|leaf| Placed {
                element: leaf.element,
                offset: leaf.offset,
                measure_offset: leaf.measure_start.map(|start| leaf.offset - start),
                key_signature: in_force(&leaves, leaf, |element| match element {
                    StreamElement::KeySignature(key) => Some(key),
                    _ => None,
                }),
                time_signature: in_force(&leaves, leaf, |element| match element {
                    StreamElement::TimeSignature(meter) => Some(meter),
                    _ => None,
                }),
                metronome_mark: in_force(&leaves, leaf, |element| match element {
                    StreamElement::MetronomeMark(mark) => Some(mark),
                    _ => None,
                }),
            })
            .collect()
    }

    /// Where a measure starts, when this stream is one starting at `base`.
    fn measure_start(&self, base: FloatType) -> Option<FloatType> {
        (self.kind == StreamKind::Measure).then_some(base)
    }

    fn gather<'a>(
        &'a self,
        base: FloatType,
        measure_start: Option<FloatType>,
        scope: Scope,
        out: &mut Vec<Leaf<'a>>,
    ) {
        let measure_start = self.measure_start(base).or(measure_start);
        for event in &self.events {
            let offset = base + event.offset;
            if let StreamElement::Stream(stream) = &event.element {
                stream.gather(offset, measure_start, scope, out);
                continue;
            }

            out.push(Leaf {
                element: &event.element,
                offset,
                measure_start,
                scope,
            });
        }
    }
}

/// The last mark of one sort at or before a leaf that the leaf's scope
/// reaches; of two at one offset, the later-listed.
fn in_force<'a, T>(
    leaves: &[Leaf<'a>],
    leaf: &Leaf<'a>,
    read: impl Fn(&'a StreamElement) -> Option<&'a T>,
) -> Option<&'a T> {
    let mut found: Option<(FloatType, &'a T)> = None;
    for mark in leaves {
        if mark.offset > leaf.offset || !mark.scope.reaches(leaf.scope) {
            continue;
        }
        let Some(value) = read(mark.element) else {
            continue;
        };
        if found.is_none_or(|(offset, _)| mark.offset >= offset) {
            found = Some((mark.offset, value));
        }
    }
    found.map(|(_, value)| value)
}

#[cfg(test)]
mod tests {
    use crate::{
        KeySignature, Note, Stream, StreamElement, meter::TimeSignature, stream::StreamKind,
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
        let sharps = |index: usize| notes[index].key_signature().and_then(KeySignature::sharps);

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
}
