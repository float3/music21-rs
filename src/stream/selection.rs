//! Keeping or taking out a stream's elements: music21's `extractContext`,
//! `removeByClass` and `removeByNotOfClass`.

use super::{Stream, StreamElement, StreamEvent, StreamKind};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use std::cmp::Ordering;

use super::exact::Exact;

/// An empty stream of the same kind, with what music21's `cloneEmpty`
/// keeps: its id and, for a measure, its number and padding, for a part,
/// its name.
fn clone_empty(stream: &Stream) -> Stream {
    let mut out = Stream::with_kind(stream.kind());
    out.labels.id = stream.labels.id.clone();
    out.labels.at_sounding_pitch = stream.labels.at_sounding_pitch;
    match stream.kind() {
        StreamKind::Measure => {
            out.labels.number = stream.labels.number;
            out.labels.number_suffix = stream.labels.number_suffix.clone();
            out.labels.padding_left = stream.labels.padding_left;
            out.labels.padding_right = stream.labels.padding_right;
        }
        StreamKind::Part | StreamKind::PartStaff => {
            out.labels.name = stream.labels.name.clone();
            out.labels.abbreviation = stream.labels.abbreviation.clone();
        }
        _ => {}
    }
    out
}

impl Stream {
    /// A stream of the elements this stream holds itself that start from
    /// `before` quarters before the element at `index` to `after` quarters
    /// after it ends: music21's `extractContext`. The stream keeps its kind
    /// and id; the elements keep their offsets, and spanners theirs.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let line = from_tiny_notation("4/4 c4 d4 e4 f4 g4 a4")?.flatten();
    /// let index = line.events().iter().position(|event| event.offset() == 2.0).unwrap();
    /// let around = line.extract_context(index, 1.0, 1.0)?;
    /// let offsets: Vec<f64> = around.notes().iter().map(|(offset, _)| *offset).collect();
    /// assert_eq!(offsets, [1.0, 2.0, 3.0]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// An index this stream holds no element at.
    pub fn extract_context(
        &self,
        index: usize,
        before: FloatType,
        after: FloatType,
    ) -> Result<Stream> {
        let found = self
            .events
            .get(index)
            .ok_or_else(|| Error::Stream("Could not find the element in the stream".to_string()))?;
        // The bounds are worked out as Python works them out from the
        // numbers music21 holds, and offsets compared with them exactly.
        let at = Exact::held(found.offset());
        let start = at.plus(Exact::of_float(-before));
        let end = at
            .plus(Exact::held(found.element().quarter_length()))
            .plus(Exact::of_float(after));
        let within = |offset: FloatType| {
            let offset = Exact::held(offset);
            start.cmp(offset) != Ordering::Greater && offset.cmp(end) == Ordering::Less
        };
        let mut out = clone_empty(self);
        out.labels.spanners = self.labels.spanners.clone();
        if within(0.0) {
            out.labels.left_barline = self.labels.left_barline.clone();
        }
        if within(self.end_offset()) {
            out.labels.right_barline = self.labels.right_barline.clone();
        }
        let kept: Vec<(StreamEvent, Option<usize>)> = self
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| within(event.offset()))
            .map(|(index, event)| (event.clone(), Some(index)))
            .collect();
        out.events = self.events.clone();
        out.replace_events(kept);
        Ok(out)
    }

    /// Takes out the elements this stream holds itself that are of one of
    /// `classes`, by music21's class names: music21's `removeByClass`. A
    /// spanner names nothing for an element taken out.
    ///
    /// ```
    /// use music21_rs::tinynotation::from_tiny_notation;
    ///
    /// let mut line = from_tiny_notation("4/4 c4 r4 e2")?.flatten();
    /// line.remove_by_class(&["Rest", "TimeSignature"]);
    /// assert!(line.events().iter().all(|event| !event.element().is_instance("Rest")));
    /// assert_eq!(line.notes().len(), 2);
    /// assert!(line.time_signatures(false)?.is_empty());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn remove_by_class(&mut self, classes: &[&str]) {
        self.remove_where(|element| classes.iter().any(|class| element.is_instance(class)));
    }

    /// Takes out the elements this stream holds itself that are of none of
    /// `classes`: music21's `removeByNotOfClass`.
    pub fn remove_by_not_of_class(&mut self, classes: &[&str]) {
        self.remove_where(|element| !classes.iter().any(|class| element.is_instance(class)));
    }

    fn remove_where(&mut self, gone: impl Fn(&StreamElement) -> bool) {
        let barline_gone = |barline: &Option<crate::bar::Barline>| {
            barline
                .as_ref()
                .is_some_and(|barline| gone(&StreamElement::Barline(barline.clone())))
        };
        if barline_gone(&self.labels.left_barline) {
            self.labels.left_barline = None;
        }
        if barline_gone(&self.labels.right_barline) {
            self.labels.right_barline = None;
        }
        let kept: Vec<(StreamEvent, Option<usize>)> = self
            .events
            .iter()
            .enumerate()
            .filter(|(_, event)| !gone(event.element()))
            .map(|(index, event)| (event.clone(), Some(index)))
            .collect();
        self.replace_events(kept);
    }
}
