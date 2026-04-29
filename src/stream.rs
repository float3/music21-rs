use crate::{
    chord::Chord, defaults::FloatType, duration::Duration, error::Result, interval::Interval,
    note::Note, pitch::Pitch, rest::Rest,
};

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
}

impl StreamElement {
    /// Returns the assigned duration, if present.
    pub fn duration(&self) -> Option<&Duration> {
        match self {
            Self::Note(note) => note.duration(),
            Self::Chord(chord) => chord.duration(),
            Self::Rest(rest) => Some(rest.duration()),
        }
    }

    /// Returns the duration in quarter lengths, defaulting to `1.0`.
    pub fn quarter_length(&self) -> FloatType {
        self.duration()
            .map(Duration::quarter_length)
            .unwrap_or_else(|| Duration::default().quarter_length())
    }

    /// Returns all pitches contained by this element.
    pub fn pitches(&self) -> Vec<Pitch> {
        match self {
            Self::Note(note) => vec![note.pitch().clone()],
            Self::Chord(chord) => chord.pitches(),
            Self::Rest(_) => Vec::new(),
        }
    }

    /// Returns a transposed copy.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        match self {
            Self::Note(note) => {
                let mut out = note.clone();
                out._pitch = interval.transpose_pitch(note.pitch())?;
                Ok(Self::Note(out))
            }
            Self::Chord(chord) => {
                let pitches = chord
                    .pitches()
                    .iter()
                    .map(|pitch| interval.transpose_pitch(pitch))
                    .collect::<Result<Vec<_>>>()?;
                let mut out = Chord::new(pitches.as_slice())?;
                if let Some(duration) = chord.duration() {
                    out.set_duration(duration.clone());
                }
                Ok(Self::Chord(out))
            }
            Self::Rest(rest) => Ok(Self::Rest(rest.clone())),
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
    pub fn element(&self) -> &StreamElement {
        &self.element
    }
}

/// A small ordered stream of notes, chords, and rests.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stream {
    events: Vec<StreamEvent>,
}

impl Stream {
    /// Creates an empty stream.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a stream from events, sorted by offset.
    pub fn from_events(events: impl IntoIterator<Item = StreamEvent>) -> Self {
        let mut stream = Self {
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
    pub fn events(&self) -> &[StreamEvent] {
        &self.events
    }

    /// Iterates over events in offset order.
    pub fn iter(&self) -> impl Iterator<Item = &StreamEvent> {
        self.events.iter()
    }

    /// Returns a sorted clone of the stream.
    pub fn flatten(&self) -> Self {
        Self::from_events(self.events.clone())
    }

    /// Returns the maximum event end offset.
    pub fn end_offset(&self) -> FloatType {
        self.events
            .iter()
            .map(|event| event.offset + event.element.quarter_length())
            .fold(0.0, FloatType::max)
    }

    /// Returns all pitches in timeline order.
    pub fn pitches(&self) -> Vec<Pitch> {
        self.events
            .iter()
            .flat_map(|event| event.element.pitches())
            .collect()
    }

    /// Returns a transposed copy.
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
        Ok(Self::from_events(events))
    }

    fn sort_events(&mut self) {
        self.events.sort_by(|left, right| {
            left.offset
                .partial_cmp(&right.offset)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
