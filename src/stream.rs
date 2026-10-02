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
//!
//! Where an element sits is asked for, not stored: [`Stream::placed`]
//! answers each element with its offset, its measure offset and the key,
//! metre and tempo in force, as one [`Placed`](crate::stream::Placed) value. music21's author
//! names keeping objects and sites apart this way as what he would do
//! again.

mod placed;

pub use placed::Placed;

use crate::{
    bar::{Barline, Ending},
    chord::Chord,
    chordsymbol::ChordSymbol,
    clef::Clef,
    defaults::{FloatType, IntegerType},
    duration::Duration,
    dynamics::Dynamic,
    error::Result,
    expressions::TextExpression,
    instrument::Instrument,
    interval::Interval,
    key::{Key, KeySignature},
    metadata::Metadata,
    meter::TimeSignature,
    note::Note,
    percussion::{PercussionChord, Unpitched},
    pitch::Pitch,
    repeat::RepeatExpression,
    rest::Rest,
    spanner::Spanner,
    tempo::{MetronomeMark, TempoText},
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
    /// The clef in force from here on.
    Clef(Clef),
    /// The instrument playing from here on. Boxed: an instrument is large.
    Instrument(Box<Instrument>),
    /// The key signature in force from here on.
    KeySignature(KeySignature),
    /// The key in force from here on: a key signature that also says its
    /// tonic and mode, as music21's `Key` is a `KeySignature`.
    Key(Key),
    /// The metre in force from here on.
    TimeSignature(TimeSignature),
    /// The tempo in force from here on.
    MetronomeMark(MetronomeMark),
    /// A tempo said in words, `Grave` or `Allegro`.
    TempoText(TempoText),
    /// Words written over or under the staff.
    TextExpression(TextExpression),
    /// A sign or words saying where to go next, *Fine* or *D.C.*
    RepeatExpression(RepeatExpression),
    /// The dynamic in force from here on.
    Dynamic(Dynamic),
    /// A chord named over the music. It holds for the time it has been
    /// given and takes none until then.
    ChordSymbol(ChordSymbol),
    /// A stroke with no pitch.
    Unpitched(Unpitched),
    /// Several strokes at once.
    PercussionChord(PercussionChord),
}

impl StreamElement {
    /// Returns the assigned duration, if present.
    ///
    /// The marks that only say what is in force from a point — a key, a
    /// metre, a tempo, a dynamic — have none, as they take no time in
    /// music21 either. A chord symbol has one, of no length until it is
    /// given the time it holds.
    pub fn duration(&self) -> Option<&Duration> {
        match self {
            Self::Note(note) => note.duration(),
            Self::Chord(chord) => chord.duration(),
            Self::Unpitched(stroke) => stroke.duration(),
            Self::PercussionChord(chord) => chord.duration(),
            Self::Rest(rest) => Some(rest.duration()),
            Self::ChordSymbol(symbol) => Some(symbol.duration()),
            Self::Stream(_)
            | Self::Clef(_)
            | Self::Instrument(_)
            | Self::KeySignature(_)
            | Self::Key(_)
            | Self::TimeSignature(_)
            | Self::MetronomeMark(_)
            | Self::TempoText(_)
            | Self::TextExpression(_)
            | Self::RepeatExpression(_)
            | Self::Dynamic(_) => None,
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
            Self::Clef(_)
            | Self::Instrument(_)
            | Self::KeySignature(_)
            | Self::Key(_)
            | Self::TimeSignature(_)
            | Self::MetronomeMark(_)
            | Self::TempoText(_)
            | Self::TextExpression(_)
            | Self::RepeatExpression(_)
            | Self::Dynamic(_) => 0.0,
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
            Self::PercussionChord(chord) => chord.pitches(),
            Self::Unpitched(_)
            | Self::Rest(_)
            | Self::Clef(_)
            | Self::Instrument(_)
            | Self::KeySignature(_)
            | Self::Key(_)
            | Self::TimeSignature(_)
            | Self::MetronomeMark(_)
            | Self::TempoText(_)
            | Self::TextExpression(_)
            | Self::RepeatExpression(_)
            | Self::Dynamic(_)
            | Self::ChordSymbol(_) => Vec::new(),
        }
    }

    /// The stream this element is, when it is one.
    pub fn as_stream(&self) -> Option<&Stream> {
        match self {
            Self::Stream(stream) => Some(stream),
            _ => None,
        }
    }

    /// music21's `classSortOrder`: what comes first among the things standing
    /// at one offset.
    pub(crate) fn class_sort_order(&self) -> i32 {
        match self {
            Self::TextExpression(_) => -30,
            Self::Instrument(_) => -25,
            Self::Stream(stream) if stream.kind() == StreamKind::Voice => 5,
            Self::Stream(_) => -20,
            Self::Clef(_) => 0,
            Self::MetronomeMark(_) | Self::TempoText(_) => 1,
            Self::KeySignature(_) | Self::Key(_) => 2,
            Self::TimeSignature(_) => 4,
            Self::Dynamic(_) => 10,
            Self::ChordSymbol(_) => 19,
            _ => 20,
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
            // A stroke has no pitch to move, and a percussion part is not
            // transposed with the music.
            Self::Unpitched(stroke) => Ok(Self::Unpitched(stroke.clone())),
            Self::PercussionChord(chord) => Ok(Self::PercussionChord(chord.clone())),
            Self::Stream(stream) => Ok(Self::Stream(Box::new(stream.transpose(interval)?))),
            Self::Clef(clef) => Ok(Self::Clef(clef.clone())),
            Self::Instrument(instrument) => Ok(Self::Instrument(instrument.clone())),
            Self::KeySignature(key) => Ok(Self::KeySignature(key.transpose(interval)?)),
            Self::Key(key) => Ok(Self::Key(key.transpose(interval)?)),
            Self::TimeSignature(meter) => Ok(Self::TimeSignature(meter.clone())),
            Self::MetronomeMark(mark) => Ok(Self::MetronomeMark(mark.clone())),
            Self::TempoText(text) => Ok(Self::TempoText(text.clone())),
            Self::TextExpression(text) => Ok(Self::TextExpression(text.clone())),
            Self::RepeatExpression(mark) => Ok(Self::RepeatExpression(mark.clone())),
            Self::Dynamic(dynamic) => Ok(Self::Dynamic(dynamic.clone())),
            Self::ChordSymbol(symbol) => Ok(Self::ChordSymbol(symbol.transpose(interval)?)),
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

impl From<Clef> for StreamElement {
    fn from(value: Clef) -> Self {
        Self::Clef(value)
    }
}

impl From<Instrument> for StreamElement {
    fn from(value: Instrument) -> Self {
        Self::Instrument(Box::new(value))
    }
}

impl From<Key> for StreamElement {
    fn from(value: Key) -> Self {
        Self::Key(value)
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

impl From<Dynamic> for StreamElement {
    fn from(value: Dynamic) -> Self {
        Self::Dynamic(value)
    }
}

impl From<ChordSymbol> for StreamElement {
    fn from(value: ChordSymbol) -> Self {
        Self::ChordSymbol(value)
    }
}

impl From<TextExpression> for StreamElement {
    fn from(value: TextExpression) -> Self {
        Self::TextExpression(value)
    }
}

impl From<Unpitched> for StreamElement {
    fn from(value: Unpitched) -> Self {
        Self::Unpitched(value)
    }
}

impl From<PercussionChord> for StreamElement {
    fn from(value: PercussionChord) -> Self {
        Self::PercussionChord(value)
    }
}

impl From<RepeatExpression> for StreamElement {
    fn from(value: RepeatExpression) -> Self {
        Self::RepeatExpression(value)
    }
}

impl From<TempoText> for StreamElement {
    fn from(value: TempoText) -> Self {
        Self::TempoText(value)
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
    #[cfg_attr(feature = "serde", serde(default))]
    labels: Labels,
}

/// How a staff group's barlines are drawn: music21's `barTogether`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BarTogether {
    /// Barlines run through every staff of the group.
    Yes,
    /// Each staff is barred on its own.
    No,
    /// Barlines run between the staves but not through them.
    Mensurstrich,
}

impl BarTogether {
    /// The group barline MusicXML writes for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Mensurstrich => "Mensurstrich",
        }
    }
}

/// Parts of a score bracketed together: music21's `layout.StaffGroup`,
/// which holds the parts it spans.
///
/// The parts are named by where they stand among the score's parts, first
/// to last, in the order the group was given them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StaffGroup {
    parts: Vec<usize>,
    name: Option<String>,
    abbreviation: Option<String>,
    symbol: Option<String>,
    bar_together: Option<BarTogether>,
    name_hidden: bool,
}

impl StaffGroup {
    /// A group of the parts at these positions, barred together and drawn
    /// with no symbol, as music21's starts out.
    pub fn new(parts: Vec<usize>) -> Self {
        Self {
            parts,
            name: None,
            abbreviation: None,
            symbol: None,
            bar_together: Some(BarTogether::Yes),
            name_hidden: false,
        }
    }

    /// The positions of the parts the group spans.
    pub fn parts(&self) -> &[usize] {
        &self.parts
    }

    /// The group's name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Names the group, or clears the name.
    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    /// The group's short name.
    pub fn abbreviation(&self) -> Option<&str> {
        self.abbreviation.as_deref()
    }

    /// Sets the short name, or clears it.
    pub fn set_abbreviation(&mut self, abbreviation: Option<String>) {
        self.abbreviation = abbreviation;
    }

    /// What the group is drawn with: `bracket`, `brace`, `line` or `square`.
    pub fn symbol(&self) -> Option<&str> {
        self.symbol.as_deref()
    }

    /// Sets what the group is drawn with. As in music21, `none` in any case
    /// clears it, and anything but the four symbols is refused.
    pub fn set_symbol(&mut self, symbol: Option<&str>) -> Result<()> {
        self.symbol = match symbol.map(str::to_lowercase) {
            None => None,
            Some(symbol) if symbol == "none" => None,
            Some(symbol) if ["brace", "line", "bracket", "square"].contains(&symbol.as_str()) => {
                Some(symbol)
            }
            Some(symbol) => {
                return Err(crate::error::Error::Value(format!(
                    "the symbol value {symbol} is not acceptable"
                )));
            }
        };
        Ok(())
    }

    /// How the group's barlines are drawn, where that has been said.
    pub fn bar_together(&self) -> Option<BarTogether> {
        self.bar_together
    }

    /// Says how the group's barlines are drawn.
    pub fn set_bar_together(&mut self, bar_together: Option<BarTogether>) {
        self.bar_together = bar_together;
    }

    /// Whether the name is left unprinted: music21's `hideObjectOnPrint`.
    pub fn name_hidden(&self) -> bool {
        self.name_hidden
    }

    /// Says whether the name is left unprinted.
    pub fn set_name_hidden(&mut self, hidden: bool) {
        self.name_hidden = hidden;
    }
}

/// What a stream is called and numbered, as distinct from what it holds.
///
/// music21 spreads these over its subclasses: a measure has a number, a part
/// a name. They are kept together here, and a stream of a kind that has no
/// use for one simply leaves it unset.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Labels {
    number: IntegerType,
    number_suffix: Option<String>,
    number_hidden: bool,
    id: Option<String>,
    name: Option<String>,
    abbreviation: Option<String>,
    name_hidden: bool,
    abbreviation_hidden: bool,
    /// Boxed: most streams carry none.
    metadata: Option<Box<Metadata>>,
    staff_groups: Vec<StaffGroup>,
    left_barline: Option<Barline>,
    right_barline: Option<Barline>,
    spanners: Vec<Spanner>,
    ending: Option<Ending>,
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
            ..Self::default()
        }
    }

    /// A measure's number: music21's `Measure.number`, which is nought until
    /// something numbers it.
    pub fn number(&self) -> IntegerType {
        self.labels.number
    }

    /// Numbers the measure.
    pub fn set_number(&mut self, number: IntegerType) {
        self.labels.number = number;
    }

    /// What follows the number, as the `a` of a measure numbered `12a`:
    /// music21's `numberSuffix`.
    pub fn number_suffix(&self) -> Option<&str> {
        self.labels.number_suffix.as_deref()
    }

    /// Sets what follows the number, or clears it.
    pub fn set_number_suffix(&mut self, suffix: Option<String>) {
        self.labels.number_suffix = suffix;
    }

    /// The number with its suffix, `12a`: music21's
    /// `measureNumberWithSuffix`.
    pub fn number_with_suffix(&self) -> String {
        format!(
            "{}{}",
            self.labels.number,
            self.labels.number_suffix.as_deref().unwrap_or("")
        )
    }

    /// Whether the measure's number is left unshown, as a pickup bar's is:
    /// music21's `showNumber` set to `NEVER`.
    pub fn number_hidden(&self) -> bool {
        self.labels.number_hidden
    }

    /// Says whether the measure's number is left unshown.
    pub fn set_number_hidden(&mut self, hidden: bool) {
        self.labels.number_hidden = hidden;
    }

    /// The name the stream is known by, as a part's `P1`: music21's `id`
    /// where one was given rather than taken from a memory address.
    pub fn id(&self) -> Option<&str> {
        self.labels.id.as_deref()
    }

    /// Sets the stream's id, or clears it.
    pub fn set_id(&mut self, id: Option<String>) {
        self.labels.id = id;
    }

    /// A part's name, as printed beside its first system: music21's
    /// `partName`.
    pub fn name(&self) -> Option<&str> {
        self.labels.name.as_deref()
    }

    /// Names the part, or clears the name.
    pub fn set_name(&mut self, name: Option<String>) {
        self.labels.name = name;
    }

    /// A part's short name, as printed beside the systems after the first:
    /// music21's `partAbbreviation`.
    pub fn abbreviation(&self) -> Option<&str> {
        self.labels.abbreviation.as_deref()
    }

    /// Sets the part's short name, or clears it.
    pub fn set_abbreviation(&mut self, abbreviation: Option<String>) {
        self.labels.abbreviation = abbreviation;
    }

    /// Whether the part's name is left unprinted: music21's
    /// `style.printPartName` set to false.
    pub fn name_hidden(&self) -> bool {
        self.labels.name_hidden
    }

    /// Says whether the part's name is left unprinted.
    pub fn set_name_hidden(&mut self, hidden: bool) {
        self.labels.name_hidden = hidden;
    }

    /// Whether the part's short name is left unprinted: music21's
    /// `style.printPartAbbreviation` set to false.
    pub fn abbreviation_hidden(&self) -> bool {
        self.labels.abbreviation_hidden
    }

    /// Says whether the part's short name is left unprinted.
    pub fn set_abbreviation_hidden(&mut self, hidden: bool) {
        self.labels.abbreviation_hidden = hidden;
    }

    /// The title, composer and the rest a score carries: music21's
    /// `metadata`.
    pub fn metadata(&self) -> Option<&Metadata> {
        self.labels.metadata.as_deref()
    }

    /// Sets the metadata, or clears it.
    pub fn set_metadata(&mut self, metadata: Option<Metadata>) {
        self.labels.metadata = metadata.map(Box::new);
    }

    /// The groups a score brackets its parts in, in the order they were
    /// added.
    pub fn staff_groups(&self) -> &[StaffGroup] {
        &self.labels.staff_groups
    }

    /// Brackets some of the score's parts together.
    pub fn add_staff_group(&mut self, group: StaffGroup) {
        self.labels.staff_groups.push(group);
    }

    /// The barline a measure starts with, where it is not an ordinary one:
    /// music21's `leftBarline`.
    pub fn left_barline(&self) -> Option<&Barline> {
        self.labels.left_barline.as_ref()
    }

    /// Sets the barline a measure starts with, or clears it.
    pub fn set_left_barline(&mut self, barline: Option<Barline>) {
        self.labels.left_barline = barline;
    }

    /// The barline a measure ends with, where it is not an ordinary one:
    /// music21's `rightBarline`.
    pub fn right_barline(&self) -> Option<&Barline> {
        self.labels.right_barline.as_ref()
    }

    /// Sets the barline a measure ends with, or clears it.
    pub fn set_right_barline(&mut self, barline: Option<Barline>) {
        self.labels.right_barline = barline;
    }

    /// The alternative ending a measure is part of, where it is in one.
    pub fn ending(&self) -> Option<&Ending> {
        self.labels.ending.as_ref()
    }

    /// Puts the measure in an alternative ending, or takes it out.
    pub fn set_ending(&mut self, ending: Option<Ending>) {
        self.labels.ending = ending;
    }

    /// The slurs and other spanners this stream holds, each naming what it
    /// joins by position in this stream's [`Stream::leaves`].
    pub fn spanners(&self) -> &[Spanner] {
        &self.labels.spanners
    }

    /// Adds a spanner, naming what it joins by position in this stream's
    /// [`Stream::leaves`].
    pub fn add_spanner(&mut self, spanner: Spanner) {
        self.labels.spanners.push(spanner);
    }

    /// Which of music21's `Stream` subclasses this stands for.
    pub fn kind(&self) -> StreamKind {
        self.kind
    }

    /// Sets which subclass this stands for.
    pub fn set_kind(&mut self, kind: StreamKind) {
        self.kind = kind;
    }

    /// This stream holding other events, in the order given: its kind, name
    /// and numbers kept, its spanners dropped, since the places they name
    /// are gone.
    pub(crate) fn with_events(&self, events: Vec<StreamEvent>) -> Self {
        let mut labels = self.labels.clone();
        labels.spanners.clear();
        Self {
            kind: self.kind,
            events,
            labels,
        }
    }

    /// Creates a stream from events, sorted by offset.
    pub fn from_events(events: impl IntoIterator<Item = StreamEvent>) -> Self {
        let mut stream = Self {
            events: events.into_iter().collect(),
            ..Self::default()
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

    /// Every element that is not itself a stream, at its offset from this
    /// stream's start, in the order [`Stream::for_each_mut`] hands them over.
    ///
    /// An element's index in this list is its *position*, which is how the
    /// walks answering about particular elements say which they mean:
    /// [`crate::volume::dynamics_in_force`],
    /// [`crate::chordsymbol::chord_symbol_durations`],
    /// [`crate::voiceleading::voice_leading_quartet_positions`] and
    /// [`crate::voiceleading::verticality_positions_at`]. A caller keeping
    /// something of its own beside each element, as a binding keeps the
    /// object it was handed, finds it again by the same index.
    pub fn leaves(&self) -> Vec<(FloatType, &StreamElement)> {
        self.leaves_under_top()
            .into_iter()
            .map(|(_, offset, element)| (offset, element))
            .collect()
    }

    /// [`Stream::leaves`], each with the index of the event of this stream it
    /// came out of: itself for an element held directly, the nested stream
    /// holding it otherwise.
    pub(crate) fn leaves_under_top(&self) -> Vec<(usize, FloatType, &StreamElement)> {
        let mut out = Vec::new();
        for (top, event) in self.events.iter().enumerate() {
            match &event.element {
                StreamElement::Stream(stream) => {
                    let mut inner = Vec::new();
                    stream.leaves_into(event.offset, &mut inner);
                    out.extend(
                        inner
                            .into_iter()
                            .map(|(offset, element)| (top, offset, element)),
                    );
                }
                element => out.push((top, event.offset, element)),
            }
        }
        out
    }

    fn leaves_into<'a>(&'a self, base: FloatType, out: &mut Vec<(FloatType, &'a StreamElement)>) {
        for event in &self.events {
            let offset = base + event.offset;
            match &event.element {
                StreamElement::Stream(stream) => stream.leaves_into(offset, out),
                element => out.push((offset, element)),
            }
        }
    }

    /// Hands every element to a closure with its offset from this stream's
    /// start, nested streams' contents included, so that it may be changed
    /// where it sits.
    ///
    /// [`Stream::flatten`] and [`Stream::recurse`] hand back copies and
    /// references; this is the walk for an edit that has to land in the
    /// score itself. Things at one offset come in the same order `flatten`
    /// gives them. A nested stream is walked into rather than handed over.
    pub fn for_each_mut(&mut self, visit: &mut impl FnMut(FloatType, &mut StreamElement)) {
        self.for_each_mut_from(0.0, visit);
    }

    fn for_each_mut_from(
        &mut self,
        base: FloatType,
        visit: &mut impl FnMut(FloatType, &mut StreamElement),
    ) {
        for event in &mut self.events {
            let offset = base + event.offset;
            match &mut event.element {
                StreamElement::Stream(stream) => stream.for_each_mut_from(offset, visit),
                element => visit(offset, element),
            }
        }
    }

    /// Keeps the leaves `keep` accepts, handed each with its position in
    /// [`Stream::leaves`] to change as it likes, and drops the rest. Every
    /// spanner, here or in a nested stream, is moved to where what it names
    /// now stands, and names nothing for an element dropped.
    pub(crate) fn retain_leaves(
        &mut self,
        keep: &mut impl FnMut(usize, &mut StreamElement) -> bool,
    ) {
        let mut seen = 0;
        self.retain_leaves_from(&mut seen, keep);
    }

    /// Adds events to this stream's own and puts them all in the order
    /// music21 sorts a stream in, an added event after one already here that
    /// sorts alike. Every spanner of this stream is moved to where what it
    /// names now stands.
    pub(crate) fn insert_sorted(&mut self, added: Vec<StreamEvent>) {
        // Each event held here with where its leaves started; nothing for
        // one added.
        let mut first = 0;
        let mut held: Vec<(StreamEvent, Option<usize>)> = Vec::new();
        for event in std::mem::take(&mut self.events) {
            let count = leaf_count(&event.element);
            held.push((event, Some(first)));
            first += count;
        }
        held.extend(added.into_iter().map(|event| (event, None)));
        held.sort_by(|left, right| crate::makenotation::music21_order(&left.0, &right.0));
        let mut moved: Vec<Option<usize>> = vec![None; first];
        let mut placed = 0;
        for (event, start) in &held {
            let count = leaf_count(&event.element);
            if let Some(start) = start {
                for offset in 0..count {
                    moved[start + offset] = Some(placed + offset);
                }
            }
            placed += count;
        }
        self.events = held.into_iter().map(|(event, _)| event).collect();
        for spanner in &mut self.labels.spanners {
            spanner.move_places(&moved);
        }
    }

    /// [`Stream::retain_leaves`] below the `seen` leaves already walked; the
    /// answer is where each of this stream's leaves went, by old position.
    fn retain_leaves_from(
        &mut self,
        seen: &mut usize,
        keep: &mut impl FnMut(usize, &mut StreamElement) -> bool,
    ) -> Vec<Option<usize>> {
        let mut moved: Vec<Option<usize>> = Vec::new();
        let mut kept = 0;
        self.events.retain_mut(|event| match &mut event.element {
            StreamElement::Stream(inner) => {
                let inner_moved = inner.retain_leaves_from(seen, keep);
                let count = inner_moved.iter().flatten().count();
                moved.extend(
                    inner_moved
                        .into_iter()
                        .map(|place| place.map(|place| place + kept)),
                );
                kept += count;
                true
            }
            element => {
                let position = *seen;
                *seen += 1;
                if keep(position, element) {
                    moved.push(Some(kept));
                    kept += 1;
                    true
                } else {
                    moved.push(None);
                    false
                }
            }
        });
        for spanner in &mut self.labels.spanners {
            spanner.move_places(&moved);
        }
        moved
    }

    /// Whether this stream holds parts, or streams standing in for them:
    /// music21's `hasPartLikeStreams`. A part says so; a measure or a voice,
    /// or a stream starting anywhere but the beginning, says not; any other
    /// stream holding measures or notes and rests says so, unless a later
    /// one says not.
    pub fn has_part_like_streams(&self) -> bool {
        let mut part_like = false;
        for event in &self.events {
            let Some(stream) = event.element.as_stream() else {
                continue;
            };
            match stream.kind {
                StreamKind::Part => return true,
                StreamKind::Measure | StreamKind::Voice => return false,
                _ if event.offset != 0.0 => return false,
                _ => {}
            }
            let holds_music = stream.events.iter().any(|inner| {
                matches!(
                    inner.element,
                    StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::Rest(_)
                ) || inner
                    .element
                    .as_stream()
                    .is_some_and(|nested| nested.kind == StreamKind::Measure)
            });
            if holds_music {
                part_like = true;
            }
        }
        part_like
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

    /// The parts this stream holds, a staff of a part written on several
    /// included: music21's `.parts`, which a `PartStaff` is one of.
    pub fn parts(&self) -> Vec<&Stream> {
        self.events
            .iter()
            .filter_map(|event| event.element.as_stream())
            .filter(|stream| matches!(stream.kind, StreamKind::Part | StreamKind::PartStaff))
            .collect()
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
            StreamElement::Key(key) => Some(key.key_signature()),
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
        out.labels = self.labels.clone();
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

/// How many leaves an element is: those of a nested stream, or itself.
fn leaf_count(element: &StreamElement) -> usize {
    match element {
        StreamElement::Stream(inner) => inner.leaves().len(),
        _ => 1,
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
    fn a_stream_has_part_like_streams_as_music21_decides() {
        use super::{Stream, StreamKind};
        use crate::note::Note;

        // music21's own examples: a part, a measure, one generic stream at
        // the start, and a second one appended after it.
        let four_notes = |kind: StreamKind| {
            let mut stream = Stream::with_kind(kind);
            for _ in 0..4 {
                stream.push(Note::from_name("C4").unwrap());
            }
            stream
        };
        let mut score = Stream::with_kind(StreamKind::Score);
        assert!(!score.has_part_like_streams());
        score.insert(0.0, four_notes(StreamKind::Part));
        assert!(score.has_part_like_streams());

        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(four_notes(StreamKind::Measure));
        assert!(!score.has_part_like_streams());

        let mut score = Stream::with_kind(StreamKind::Score);
        score.push(four_notes(StreamKind::Stream));
        assert!(score.has_part_like_streams());
        score.push(four_notes(StreamKind::Stream));
        assert!(!score.has_part_like_streams());
    }

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
        second.push(Note::from_name("Bb4").unwrap());

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
        assert_eq!(names, ["D4", "E4", "F#4", "G4", "A4", "Bb4"]);

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
