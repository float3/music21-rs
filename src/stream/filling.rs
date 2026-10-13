//! Filling a stream from another, stretching what it holds, and emptying
//! it: music21's `Stream.extendDuration`, `mergeElements` and `cloneEmpty`.

use super::Stream;
use super::editing::map_duration;
use crate::defaults::FloatType;
use crate::duration::Duration;
use crate::error::Result;
use crate::makenotation::op_frac;

impl Stream {
    /// A copy of the stream with each element of a class stretched to
    /// where the next one starts: music21's `extendDuration`.
    ///
    /// The last one reaches the end of the stream. Elements of other
    /// classes are left alone and do not break the run, so a dynamic
    /// standing between two notes does not stop the first note reaching
    /// the second.
    ///
    /// Only what the crate gives a length to is stretched: a note, a rest,
    /// a chord or a chord symbol. music21 would stretch any element, since
    /// everything there carries a duration -- `extendDuration(Dynamic)`
    /// gives the dynamic a length -- where the crate keeps the marks that
    /// say what is in force from a point timeless. Asking for one of those
    /// classes leaves the stream as it stands.
    ///
    /// ```
    /// use music21_rs::{Dynamic, Note, Stream};
    ///
    /// let mut line = Stream::new();
    /// for place in [0.0, 10.0, 20.0] {
    ///     line.insert(place, Note::from_name("C4")?);
    /// }
    /// line.insert(15.0, Dynamic::new("ff"));
    ///
    /// let stretched = line.extend_duration(&["GeneralNote"])?;
    /// let lengths: Vec<f64> = stretched
    ///     .notes()
    ///     .iter()
    ///     .map(|(_, note)| note.quarter_length())
    ///     .collect();
    /// // Ten to the next note, ten again past the dynamic, and the last
    /// // keeps the length it came with to the end of the stream.
    /// assert_eq!(lengths, [10.0, 10.0, 1.0]);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A length no written value can be made of.
    pub fn extend_duration(&self, classes: &[&str]) -> Result<Stream> {
        let mut out = self.clone();
        // music21 reads the whole length before stretching anything.
        let total = out.end_offset();
        let places: Vec<usize> = out
            .events()
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                classes
                    .iter()
                    .any(|class| event.element().is_instance(class))
            })
            .map(|(place, _)| place)
            .collect();
        for pair in places.windows(2) {
            let span = op_frac(out.events()[pair[1]].offset() - out.events()[pair[0]].offset());
            out.stretch(pair[0], span)?;
        }
        if let Some(&last) = places.last() {
            let span = op_frac(total - out.events()[last].offset());
            out.stretch(last, span)?;
        }
        Ok(out)
    }

    /// Writes one element's length, giving it one where it has none:
    /// music21's elements all carry a duration, and `extendDuration`
    /// writes to it.
    fn stretch(&mut self, place: usize, quarter_length: FloatType) -> Result<()> {
        let element = self.events[place].element_mut();
        map_duration(element, &mut |_| Duration::new(quarter_length).map(Some))
    }

    /// Takes in everything another stream holds, each at the offset it
    /// stands at there: music21's `mergeElements`.
    ///
    /// With `classes`, only the elements answering to one of those music21
    /// class names come. music21 stores the other stream's own objects in
    /// both streams; a crate stream owns what it holds, so they are
    /// copied.
    ///
    /// ```
    /// use music21_rs::{Note, Stream};
    ///
    /// let mut from = Stream::new();
    /// from.insert(0.0, Note::from_name("F#4")?);
    /// from.insert(1.0, Note::from_name("G4")?);
    ///
    /// let mut into = Stream::new();
    /// into.merge_elements(&from, None);
    /// assert_eq!(into.len(), 2);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn merge_elements(&mut self, other: &Stream, classes: Option<&[&str]>) {
        for event in other.events() {
            let wanted = classes.is_none_or(|classes| {
                classes
                    .iter()
                    .any(|class| event.element().is_instance(class))
            });
            if wanted {
                self.insert(event.offset(), event.element().clone());
            }
        }
    }

    /// A stream like this one holding nothing: music21's `cloneEmpty`.
    ///
    /// It is of the same kind and carries the same id, which is what
    /// music21's `mergeAttributes` copies -- and only where an id was
    /// given, music21 leaving an object's own address behind, which a
    /// crate stream has nothing of. Nothing else comes: not the metadata,
    /// the barlines or the spanners, which
    /// [`Stream::template`](Self::template) keeps.
    ///
    /// ```
    /// use music21_rs::{Note, Stream, StreamKind};
    ///
    /// let mut part = Stream::with_kind(StreamKind::Part);
    /// part.set_id(Some("hi".to_string()));
    /// part.insert(0.0, Note::from_name("C4")?);
    ///
    /// let empty = part.clone_empty();
    /// assert_eq!(empty.kind(), StreamKind::Part);
    /// assert_eq!(empty.id(), Some("hi"));
    /// assert!(empty.is_empty());
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn clone_empty(&self) -> Stream {
        let mut empty = Stream::with_kind(self.kind());
        empty.set_id(self.id().map(str::to_string));
        empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::Dynamic;
    use crate::note::Note;
    use crate::stream::StreamEvent;
    use crate::stream::StreamKind;

    fn three_notes_and_a_dynamic() -> Stream {
        let mut line = Stream::new();
        for place in [0.0, 10.0, 20.0] {
            line.insert(place, Note::from_name("C4").expect("a note name"));
        }
        line.insert(15.0, Dynamic::new("ff"));
        line
    }

    /// The lengths here are music21's own answers for the same stream.
    #[test]
    fn each_element_of_a_class_reaches_the_next_one() {
        let stretched = three_notes_and_a_dynamic()
            .extend_duration(&["GeneralNote"])
            .expect("stretched");
        let lengths: Vec<FloatType> = stretched
            .notes()
            .iter()
            .map(|(_, note)| note.quarter_length())
            .collect();
        assert_eq!(lengths, [10.0, 10.0, 1.0]);
        // The whole stream is as long as it was.
        assert_eq!(stretched.end_offset(), 21.0);
    }

    /// music21 gives the dynamic a length of 6.0 here, the stream's 21
    /// less the 15 it stands at. The crate keeps a dynamic timeless, so
    /// there is nothing to write, and the stream is unchanged.
    #[test]
    fn a_mark_the_crate_keeps_timeless_is_not_stretched() {
        let line = three_notes_and_a_dynamic();
        let stretched = line.extend_duration(&["Dynamic"]).expect("stretched");
        let lengths: Vec<FloatType> = stretched
            .events()
            .iter()
            .map(|event| event.element().quarter_length())
            .collect();
        assert_eq!(lengths, [1.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn a_class_no_element_answers_to_leaves_the_stream_alone() {
        let line = three_notes_and_a_dynamic();
        let stretched = line.extend_duration(&["Clef"]).expect("stretched");
        assert_eq!(stretched.events().len(), line.events().len());
        let lengths: Vec<FloatType> = stretched
            .notes()
            .iter()
            .map(|(_, note)| note.quarter_length())
            .collect();
        assert_eq!(lengths, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn merging_takes_in_what_another_stream_holds_where_it_stands() {
        let from = three_notes_and_a_dynamic();
        let mut into = Stream::new();
        into.merge_elements(&from, None);
        assert_eq!(into.len(), 4);
        let offsets: Vec<FloatType> = into.events().iter().map(StreamEvent::offset).collect();
        assert_eq!(offsets, [0.0, 10.0, 15.0, 20.0]);

        // Only the classes asked for come.
        let mut notes_only = Stream::new();
        notes_only.merge_elements(&from, Some(&["Note"]));
        assert_eq!(notes_only.len(), 3);
        let mut dynamics_only = Stream::new();
        dynamics_only.merge_elements(&from, Some(&["Dynamic"]));
        assert_eq!(dynamics_only.len(), 1);
    }

    #[test]
    fn an_emptied_copy_keeps_its_kind_and_its_id_alone() {
        let mut part = Stream::with_kind(StreamKind::Part);
        part.set_id(Some("hi".to_string()));
        part.set_name(Some("Flute".to_string()));
        part.insert(0.0, Note::from_name("C4").expect("a note name"));

        let empty = part.clone_empty();
        assert_eq!(empty.kind(), StreamKind::Part);
        assert_eq!(empty.id(), Some("hi"));
        assert!(empty.is_empty());
        // music21's mergeAttributes copies the id and the groups, nothing
        // else, so the name does not come.
        assert_eq!(empty.name(), None);
    }
}
