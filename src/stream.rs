//! Music on a timeline: music21's `stream.Stream` and the subclasses it
//! nests inside one another.
//!
//! A [`Stream`] is a list of things at quarter-length offsets, and one of
//! those things may be another stream: a score holds parts, a part holds
//! measures, a measure holds voices and notes. [`Stream::flatten`] dissolves
//! that nesting into one timeline the way music21's does, adding each
//! stream's own offset to its contents'.
//!
//! **What is deliberately not here is music21's *sites*.** There an object
//! can sit in several streams at once and carry a different offset in each,
//! which is why it needs a back-reference to every stream holding it and why
//! `getContextByClass` has a graph to walk. A stream here *owns* what it
//! holds, so the nesting is a tree; asking what key or metre is in force at
//! an offset is then a backwards look through the flattened timeline rather
//! than a search through a graph. That is the same answer for music that is
//! written once, which is all a stream built here can be.

use crate::{
    chord::Chord, defaults::FloatType, duration::Duration, error::Result, interval::Interval,
    key::KeySignature, meter::TimeSignature, note::Note, pitch::Pitch, rest::Rest,
    tempo::MetronomeMark,
};

/// Which of music21's `Stream` subclasses a stream stands for.
///
/// music21 makes each of these a class of its own; they carry no behaviour
/// that a tag does not, and a tag is what the filters here need.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StreamKind {
    /// A stream with no particular role: music21's `Stream`.
    #[default]
    Stream,
    /// One independent line inside a measure.
    Voice,
    /// One bar.
    Measure,
    /// One instrument's line through the piece.
    Part,
    /// One staff of a part written on several.
    PartStaff,
    /// A whole piece, holding parts.
    Score,
    /// A collection of scores.
    Opus,
}

impl StreamKind {
    /// Every kind, in music21's order of containment.
    pub const ALL: [StreamKind; 7] = [
        StreamKind::Stream,
        StreamKind::Voice,
        StreamKind::Measure,
        StreamKind::Part,
        StreamKind::PartStaff,
        StreamKind::Score,
        StreamKind::Opus,
    ];

    /// music21's class name for this kind.
    pub fn as_str(self) -> &'static str {
        match self {
            StreamKind::Stream => "Stream",
            StreamKind::Voice => "Voice",
            StreamKind::Measure => "Measure",
            StreamKind::Part => "Part",
            StreamKind::PartStaff => "PartStaff",
            StreamKind::Score => "Score",
            StreamKind::Opus => "Opus",
        }
    }
}

impl std::fmt::Display for StreamKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A musical object that can live on a timeline.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StreamElement {
    /// A single pitched note.
    Note(Note),
    /// A chord containing one or more notes.
    Chord(Chord),
    /// A silent rest.
    Rest(Rest),
    /// A stream nested inside this one: a part in a score, a measure in a
    /// part. Boxed, since a stream holds these by value.
    Stream(Box<Stream>),
    /// The key signature in force from here on.
    KeySignature(KeySignature),
    /// The metre in force from here on.
    TimeSignature(TimeSignature),
    /// The tempo in force from here on.
    MetronomeMark(MetronomeMark),
}

impl StreamElement {
    /// Returns the assigned duration, if present.
    ///
    /// The marks that only say what is in force from a point — a key, a
    /// metre, a tempo — have none, as they take no time in music21 either.
    pub fn duration(&self) -> Option<&Duration> {
        match self {
            Self::Note(note) => note.duration(),
            Self::Chord(chord) => chord.duration(),
            Self::Rest(rest) => Some(rest.duration()),
            Self::Stream(_)
            | Self::KeySignature(_)
            | Self::TimeSignature(_)
            | Self::MetronomeMark(_) => None,
        }
    }

    /// Returns the duration in quarter lengths.
    ///
    /// A nested stream is as long as its own contents; a mark takes no time;
    /// anything else with no duration of its own defaults to a quarter, as
    /// music21's does.
    pub fn quarter_length(&self) -> FloatType {
        match self {
            Self::Stream(stream) => stream.end_offset(),
            Self::KeySignature(_) | Self::TimeSignature(_) | Self::MetronomeMark(_) => 0.0,
            _ => self
                .duration()
                .map(Duration::quarter_length)
                .unwrap_or_else(|| Duration::default().quarter_length()),
        }
    }

    /// Returns all pitches contained by this element, a nested stream's
    /// included.
    pub fn pitches(&self) -> Vec<Pitch> {
        match self {
            Self::Note(note) => vec![note.pitch().clone()],
            Self::Chord(chord) => chord.pitches(),
            Self::Stream(stream) => stream.pitches(),
            Self::Rest(_)
            | Self::KeySignature(_)
            | Self::TimeSignature(_)
            | Self::MetronomeMark(_) => Vec::new(),
        }
    }

    /// The stream this element is, when it is one.
    pub fn as_stream(&self) -> Option<&Stream> {
        match self {
            Self::Stream(stream) => Some(stream),
            _ => None,
        }
    }

    /// Whether this element sounds — music21's `.notes`, which is notes and
    /// chords but not rests.
    pub fn is_note_or_chord(&self) -> bool {
        matches!(self, Self::Note(_) | Self::Chord(_))
    }

    /// Returns a transposed copy.
    ///
    /// A key signature moves with the music, as music21's does; a metre and
    /// a tempo do not depend on pitch and are left alone.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        match self {
            Self::Note(note) => {
                let mut out = note.clone();
                out.set_pitch(interval.transpose_pitch(note.pitch())?);
                Ok(Self::Note(out))
            }
            Self::Chord(chord) => Ok(Self::Chord(chord.transpose(interval)?)),
            Self::Rest(rest) => Ok(Self::Rest(rest.clone())),
            Self::Stream(stream) => Ok(Self::Stream(Box::new(stream.transpose(interval)?))),
            Self::KeySignature(key) => Ok(Self::KeySignature(key.transpose(interval)?)),
            Self::TimeSignature(meter) => Ok(Self::TimeSignature(meter.clone())),
            Self::MetronomeMark(mark) => Ok(Self::MetronomeMark(mark.clone())),
        }
    }
}

impl From<Note> for StreamElement {
    fn from(value: Note) -> Self {
        Self::Note(value)
    }
}

impl From<Chord> for StreamElement {
    fn from(value: Chord) -> Self {
        Self::Chord(value)
    }
}

impl From<Rest> for StreamElement {
    fn from(value: Rest) -> Self {
        Self::Rest(value)
    }
}

impl From<Stream> for StreamElement {
    fn from(value: Stream) -> Self {
        Self::Stream(Box::new(value))
    }
}

impl From<KeySignature> for StreamElement {
    fn from(value: KeySignature) -> Self {
        Self::KeySignature(value)
    }
}

impl From<TimeSignature> for StreamElement {
    fn from(value: TimeSignature) -> Self {
        Self::TimeSignature(value)
    }
}

impl From<MetronomeMark> for StreamElement {
    fn from(value: MetronomeMark) -> Self {
        Self::MetronomeMark(value)
    }
}

/// A timestamped stream item.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StreamEvent {
    offset: FloatType,
    element: StreamElement,
}

impl StreamEvent {
    /// Creates an event at an offset measured in quarter lengths.
    pub fn new(offset: FloatType, element: impl Into<StreamElement>) -> Self {
        Self {
            offset,
            element: element.into(),
        }
    }

    /// Returns the offset in quarter lengths.
    pub fn offset(&self) -> FloatType {
        self.offset
    }

    /// Returns the stream element.
    pub fn element_mut(&mut self) -> &mut StreamElement {
        &mut self.element
    }

    /// The element itself.
    pub fn element(&self) -> &StreamElement {
        &self.element
    }

    /// The offset just past this element.
    pub fn end_offset(&self) -> FloatType {
        self.offset + self.element.quarter_length()
    }
}

/// An ordered stream of music, which may hold other streams.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Stream {
    #[cfg_attr(feature = "serde", serde(default))]
    kind: StreamKind,
    events: Vec<StreamEvent>,
}

impl Stream {
    /// Creates an empty stream.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty stream standing for one of music21's subclasses.
    pub fn with_kind(kind: StreamKind) -> Self {
        Self {
            kind,
            events: Vec::new(),
        }
    }

    /// Which of music21's `Stream` subclasses this stands for.
    pub fn kind(&self) -> StreamKind {
        self.kind
    }

    /// Sets which subclass this stands for.
    pub fn set_kind(&mut self, kind: StreamKind) {
        self.kind = kind;
    }

    /// Creates a stream from events, sorted by offset.
    pub fn from_events(events: impl IntoIterator<Item = StreamEvent>) -> Self {
        let mut stream = Self {
            kind: StreamKind::Stream,
            events: events.into_iter().collect(),
        };
        stream.sort_events();
        stream
    }

    /// Inserts an element at a quarter-length offset.
    pub fn insert(&mut self, offset: FloatType, element: impl Into<StreamElement>) {
        self.events.push(StreamEvent::new(offset, element));
        self.sort_events();
    }

    /// Appends an element after the current end of the stream.
    pub fn push(&mut self, element: impl Into<StreamElement>) {
        let element = element.into();
        let offset = self.end_offset();
        self.events.push(StreamEvent::new(offset, element));
    }

    /// Returns immutable events in offset order.
    pub fn events_mut(&mut self) -> &mut [StreamEvent] {
        &mut self.events
    }

    /// The events in offset order.
    pub fn events(&self) -> &[StreamEvent] {
        &self.events
    }

    /// Iterates over events in offset order.
    pub fn iter(&self) -> impl Iterator<Item = &StreamEvent> {
        self.events.iter()
    }

    /// How many events this stream holds directly, not counting what any
    /// nested stream holds.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether this stream holds nothing at all.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// One timeline with the nesting dissolved: music21's `flatten`.
    ///
    /// Each nested stream's offset is added to its contents', and the
    /// streams themselves are gone; everything else comes through in offset
    /// order.
    pub fn flatten(&self) -> Self {
        let mut flattened = Self::with_kind(self.kind);
        self.flatten_into(0.0, &mut flattened.events);
        flattened.sort_events();
        flattened
    }

    fn flatten_into(&self, base: FloatType, out: &mut Vec<StreamEvent>) {
        for event in &self.events {
            let offset = base + event.offset;
            match &event.element {
                StreamElement::Stream(stream) => stream.flatten_into(offset, out),
                element => out.push(StreamEvent::new(offset, element.clone())),
            }
        }
    }

    /// Every element at its offset from this stream's start, nested streams
    /// themselves included: music21's `recurse`.
    pub fn recurse(&self) -> Vec<(FloatType, &StreamElement)> {
        let mut out = Vec::new();
        self.recurse_into(0.0, &mut out);
        out
    }

    fn recurse_into<'a>(&'a self, base: FloatType, out: &mut Vec<(FloatType, &'a StreamElement)>) {
        for event in &self.events {
            let offset = base + event.offset;
            out.push((offset, &event.element));
            if let StreamElement::Stream(stream) = &event.element {
                stream.recurse_into(offset, out);
            }
        }
    }

    /// The streams of one kind held directly by this one: music21's `.parts`
    /// on a score, `.measures` on a part, `.voices` on a measure.
    pub fn streams_of_kind(&self, kind: StreamKind) -> Vec<&Stream> {
        self.events
            .iter()
            .filter_map(|event| event.element.as_stream())
            .filter(|stream| stream.kind == kind)
            .collect()
    }

    /// The parts this stream holds.
    pub fn parts(&self) -> Vec<&Stream> {
        self.streams_of_kind(StreamKind::Part)
    }

    /// The measures this stream holds.
    pub fn measures(&self) -> Vec<&Stream> {
        self.streams_of_kind(StreamKind::Measure)
    }

    /// The voices this stream holds.
    pub fn voices(&self) -> Vec<&Stream> {
        self.streams_of_kind(StreamKind::Voice)
    }

    /// Every note and chord on the flattened timeline: music21's
    /// `flatten().notes`, which leaves rests out.
    pub fn notes(&self) -> Vec<(FloatType, StreamElement)> {
        self.flatten()
            .events
            .into_iter()
            .filter(|event| event.element.is_note_or_chord())
            .map(|event| (event.offset, event.element))
            .collect()
    }

    /// Returns the maximum event end offset, a nested stream's own length
    /// included.
    pub fn end_offset(&self) -> FloatType {
        self.events
            .iter()
            .map(StreamEvent::end_offset)
            .fold(0.0, FloatType::max)
    }

    /// Returns all pitches in timeline order, from nested streams too.
    pub fn pitches(&self) -> Vec<Pitch> {
        self.flatten()
            .events
            .iter()
            .flat_map(|event| event.element.pitches())
            .collect()
    }

    /// The key signature in force at an offset: the last one written at or
    /// before it, anywhere in the nesting.
    ///
    /// This is what music21 answers with `getContextByClass(KeySignature)`,
    /// found by looking back along the flattened timeline rather than by
    /// walking the sites an object belongs to.
    pub fn key_signature_at(&self, offset: FloatType) -> Option<KeySignature> {
        self.in_force_at(offset, |element| match element {
            StreamElement::KeySignature(key) => Some(key.clone()),
            _ => None,
        })
    }

    /// The metre in force at an offset, found the same way.
    pub fn time_signature_at(&self, offset: FloatType) -> Option<TimeSignature> {
        self.in_force_at(offset, |element| match element {
            StreamElement::TimeSignature(meter) => Some(meter.clone()),
            _ => None,
        })
    }

    /// The tempo in force at an offset, found the same way.
    pub fn metronome_mark_at(&self, offset: FloatType) -> Option<MetronomeMark> {
        self.in_force_at(offset, |element| match element {
            StreamElement::MetronomeMark(mark) => Some(mark.clone()),
            _ => None,
        })
    }

    /// The last thing of one sort written at or before an offset.
    fn in_force_at<T>(
        &self,
        offset: FloatType,
        read: impl Fn(&StreamElement) -> Option<T>,
    ) -> Option<T> {
        self.flatten()
            .events
            .iter()
            .take_while(|event| event.offset <= offset)
            .filter_map(|event| read(&event.element))
            .last()
    }

    /// Returns a transposed copy, nesting and all.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        let events = self
            .events
            .iter()
            .map(|event| {
                Ok(StreamEvent::new(
                    event.offset,
                    event.element.transpose(interval)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut out = Self::with_kind(self.kind);
        out.events = events;
        out.sort_events();
        Ok(out)
    }

    fn sort_events(&mut self) {
        self.events.sort_by(|left, right| {
            left.offset
                .partial_cmp(&right.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

impl<'a> IntoIterator for &'a Stream {
    type Item = &'a StreamEvent;
    type IntoIter = std::slice::Iter<'a, StreamEvent>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_stream_says_what_kind_it_is_and_walks_its_events() {
        use super::{Stream, StreamElement, StreamEvent, StreamKind};
        use crate::note::Note;
        use crate::pitch::Pitch;
        use crate::tempo::MetronomeMark;

        assert_eq!(StreamKind::Voice.as_str(), "Voice");
        assert_eq!(StreamKind::Voice.to_string(), "Voice");
        let mut stream = Stream::new();
        assert!(stream.is_empty());
        stream.set_kind(StreamKind::Voice);
        assert_eq!(stream.kind(), StreamKind::Voice);
        assert!(matches!(
            StreamElement::from(MetronomeMark::new(120.0)),
            StreamElement::MetronomeMark(_)
        ));

        let note = Note::from_pitch(Pitch::from_name("C4").unwrap());
        let rebuilt = Stream::from_events([StreamEvent::new(0.0, note)]);
        assert_eq!(rebuilt.iter().count(), 1);
        assert!(!rebuilt.is_empty());
        assert!(rebuilt.voices().is_empty());
        let mut outer = Stream::new();
        outer.push(stream);
        assert_eq!(outer.voices().len(), 1);
    }

    use super::*;

    #[test]
    fn a_stream_iterates_by_reference() {
        let mut stream = Stream::new();
        stream.push(Note::from_name("C4").unwrap());
        stream.push(Note::from_name("E4").unwrap());

        let offsets: Vec<FloatType> = (&stream).into_iter().map(StreamEvent::offset).collect();
        assert_eq!(offsets, vec![0.0, 1.0]);
        assert_eq!((&stream).into_iter().count(), stream.len());
    }

    #[test]
    fn stream_push_uses_durations() {
        let mut stream = Stream::new();
        stream.push(
            Note::from_name("C4")
                .unwrap()
                .with_duration(Duration::half()),
        );
        stream.push(Rest::from_quarter_length(0.5).unwrap());
        assert_eq!(stream.events()[0].offset(), 0.0);
        assert_eq!(stream.events()[1].offset(), 2.0);
        assert_eq!(stream.end_offset(), 2.5);
    }

    #[test]
    fn stream_transposes_notes_and_chords() {
        let mut stream = Stream::new();
        stream.push(Note::from_name("C4").unwrap());
        stream.push(Chord::new("E4 G4").unwrap());
        let out = stream
            .transpose(&Interval::from_name("M2").unwrap())
            .unwrap();
        let names = out
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["D4", "F#4", "A4"]);
    }

    /// A score of one part of two measures, which is the shape music21 puts
    /// almost everything in.
    fn two_measure_score() -> Stream {
        let mut first = Stream::with_kind(StreamKind::Measure);
        first.insert(0.0, KeySignature::new(2));
        first.insert(0.0, TimeSignature::new(4, 4).unwrap());
        first.push(Note::from_name("D4").unwrap());
        first.push(Note::from_name("E4").unwrap());
        first.push(Note::from_name("F#4").unwrap());
        first.push(Note::from_name("G4").unwrap());

        let mut second = Stream::with_kind(StreamKind::Measure);
        second.insert(0.0, KeySignature::new(-1));
        second.push(Note::from_name("A4").unwrap());
        second.push(Note::from_name("B-4").unwrap());

        let mut part = Stream::with_kind(StreamKind::Part);
        part.push(first);
        part.push(second);

        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(part);
        score
    }

    #[test]
    fn a_nested_stream_flattens_onto_one_timeline() {
        let score = two_measure_score();
        assert_eq!(score.kind(), StreamKind::Score);
        assert_eq!(score.len(), 1, "a score holds its one part, not its notes");
        assert_eq!(score.parts().len(), 1);
        assert_eq!(score.parts()[0].measures().len(), 2);

        // the second measure starts a bar in, so its notes do too
        let notes = score.notes();
        let offsets: Vec<FloatType> = notes.iter().map(|(offset, _)| *offset).collect();
        assert_eq!(offsets, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(score.end_offset(), 6.0);

        let names: Vec<String> = score
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(names, ["D4", "E4", "F#4", "G4", "A4", "B-4"]);

        // recurse sees the streams themselves; flatten does not
        assert_eq!(score.recurse().len(), 1 + 2 + 6 + 3);
        assert_eq!(score.flatten().len(), 6 + 3);
    }

    #[test]
    fn the_key_in_force_is_the_last_one_written_before_it() {
        let score = two_measure_score();
        assert_eq!(score.key_signature_at(0.0).unwrap().sharps(), Some(2));
        assert_eq!(score.key_signature_at(3.5).unwrap().sharps(), Some(2));
        // the second measure changes it
        assert_eq!(score.key_signature_at(4.0).unwrap().sharps(), Some(-1));
        assert_eq!(score.key_signature_at(100.0).unwrap().sharps(), Some(-1));
        assert_eq!(
            score.time_signature_at(5.0).unwrap().ratio_string(),
            "4/4",
            "a metre stays in force across the bar that follows it"
        );
        assert!(score.metronome_mark_at(0.0).is_none());

        // nothing is in force before the first mark
        let mut late = Stream::new();
        late.insert(2.0, KeySignature::new(3));
        assert!(late.key_signature_at(1.0).is_none());
        assert_eq!(late.key_signature_at(2.0).unwrap().sharps(), Some(3));
    }

    #[test]
    fn transposing_a_score_moves_its_key_signatures_too() {
        let score = two_measure_score();
        let up = score
            .transpose(&Interval::from_name("M2").unwrap())
            .unwrap();
        assert_eq!(up.key_signature_at(0.0).unwrap().sharps(), Some(4));
        assert_eq!(up.key_signature_at(4.0).unwrap().sharps(), Some(1));
        let names: Vec<String> = up.pitches().iter().map(Pitch::name_with_octave).collect();
        assert_eq!(names, ["E4", "F#4", "G#4", "A4", "B4", "C5"]);
        assert_eq!(up.kind(), StreamKind::Score);
    }
}
