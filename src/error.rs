use std::convert::Infallible;

use thiserror::Error as ThisError;

/// Result type returned by fallible `music21-rs` operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error variants produced by the crate's theory helpers.
#[derive(Clone, Debug, Eq, PartialEq, ThisError)]
#[non_exhaustive]
pub enum Error {
    /// Error associated with a generic music21-style object.
    #[error("Music21Object error: {0}")]
    Music21Object(String),
    /// Error associated with chord construction or analysis.
    #[error("Chord error: {0}")]
    Chord(String),
    /// Error associated with pitch construction, spelling or conversion.
    #[error("Pitch error: {0}")]
    Pitch(String),
    /// Error associated with microtone construction or conversion.
    #[error("Microtone error: {0}")]
    Microtone(String),
    /// Error associated with accidental parsing or conversion.
    #[error("Accidental error: {0}")]
    Accidental(String),
    /// Error associated with generated chord-table lookup data.
    #[error("ChordTables error: {0}")]
    ChordTables(String),
    /// Error associated with interval construction or conversion.
    #[error("Interval error: {0}")]
    Interval(String),
    /// Error associated with step-name parsing or conversion.
    #[error("StepName error: {0}")]
    StepName(String),
    /// Error associated with numeric pitch-class parsing.
    #[error("PitchClass error: {0}")]
    PitchClass(String),
    /// Error associated with ordinal-name parsing.
    #[error("Ordinal error: {0}")]
    Ordinal(String),
    /// Error associated with polyrhythm construction or timing.
    #[error("Polyrhythm error: {0}")]
    Polyrhythm(String),
    /// Error associated with tuning-system parsing or lookup.
    #[error("TuningSystem error: {0}")]
    TuningSystem(String),
    /// Error associated with MIDI import or export, or a MIDI value out of
    /// range.
    #[error("Midi error: {0}")]
    Midi(Box<FormatError>),
    /// Error associated with analysis helpers.
    #[error("Analysis error: {0}")]
    Analysis(String),
    /// Error associated with time-signature parsing or beat lookup.
    #[error("Meter error: {0}")]
    Meter(String),
    /// Error associated with Xenakis sieve parsing or evaluation.
    #[error("Sieve error: {0}")]
    Sieve(String),
    /// Error associated with a score's metadata: a date relevance none of
    /// music21's date classes takes, or a name that is not a metadata name.
    #[error("Metadata error: {0}")]
    Metadata(String),
    /// Error associated with duration values.
    #[error("Duration error: {0}")]
    Duration(String),
    /// Error associated with keys and key signatures.
    #[error("Key error: {0}")]
    Key(String),
    /// Error associated with scale degrees and realization.
    #[error("Scale error: {0}")]
    Scale(String),
    /// Error associated with tempo marks.
    #[error("Tempo error: {0}")]
    Tempo(String),
    /// Error associated with instruments and the names scores give them.
    #[error("Instrument error: {0}")]
    Instrument(String),
    /// Error associated with figured bass and its realization.
    #[error("Figured bass error: {0}")]
    FiguredBass(String),
    /// ABC text that cannot be read.
    #[error("ABC error: {0}")]
    Abc(Box<FormatError>),
    /// Humdrum that cannot be read.
    #[error("Humdrum error: {0}")]
    Humdrum(Box<FormatError>),
    /// A document that is not well-formed XML.
    #[error("XML error: {0}")]
    Xml(Box<FormatError>),
    /// MEI that cannot be read.
    #[error("MEI error: {0}")]
    Mei(Box<FormatError>),
    /// RomanText that cannot be read.
    #[error("RomanText error: {0}")]
    RomanText(Box<FormatError>),
    /// Capella's CapXML that cannot be read.
    #[error("Capella error: {0}")]
    Capella(Box<FormatError>),
    /// MuseData that cannot be read.
    #[error("MuseData error: {0}")]
    MuseData(Box<FormatError>),
    /// NoteWorthy Composer text that cannot be read.
    #[error("NoteWorthy error: {0}")]
    Noteworthy(Box<FormatError>),
    /// A MuseScore file that cannot be read, or MuseScore itself could not be
    /// run or could not convert a file.
    #[error("MuseScore error: {0}")]
    MuseScore(Box<FormatError>),
    /// Error associated with ornaments and other expressions.
    #[error("Expression error: {0}")]
    Expression(String),
    /// Error associated with clefs.
    #[error("Clef error: {0}")]
    Clef(String),
    /// Error associated with tone rows and serial transformations.
    #[error("Serial error: {0}")]
    Serial(String),
    /// Error associated with ties, noteheads, stems and lyrics.
    #[error("Notation error: {0}")]
    Notation(String),
    /// Error associated with note volumes.
    #[error("Volume error: {0}")]
    Volume(String),
    /// A stream that cannot be changed as asked: music21's
    /// `StreamException`.
    #[error("Stream error: {0}")]
    Stream(String),

    /// A feature that cannot be read out of a piece, or a normalized feature
    /// whose values are all nought: music21's `FeatureException`.
    #[error("Feature error: {0}")]
    Feature(String),
    /// Text that cannot be worked on as asked: an article looked for in a
    /// language music21 knows none of.
    #[error("Text error: {0}")]
    Text(String),
    /// A search that cannot be made as asked: music21's `SearchException`
    /// and `LyricSearcherException`.
    #[error("Search error: {0}")]
    Search(String),
    /// A fretboard that cannot say what it sounds: one not tuned a pitch a
    /// string, or a note on a string it has not got.
    #[error("Tablature error: {0}")]
    Tablature(String),
    /// A timespan tree that cannot answer as asked: music21's
    /// `TreeException`.
    #[error("Tree error: {0}")]
    Tree(String),
    /// A score the OMR correctors cannot work on: parts of different
    /// lengths, where music21 runs out of measures in one of them, or a
    /// measure with no time signature in force.
    #[error("OMR error: {0}")]
    Omr(String),
    /// Variants that cannot be merged, placed or made real as asked:
    /// music21's `VariantException`, and the errors music21 meets on the
    /// way.
    #[error("Variant error: {0}")]
    Variant(String),
    /// A MusicXML document that cannot be read, or a score that cannot be
    /// written as one.
    #[cfg(feature = "musicxml")]
    #[error("MusicXML export error: {0}")]
    MusicXml(Box<FormatError>),
    /// A value the caller gave that nothing musical could be read from.
    ///
    /// music21 keeps this apart from its own exceptions — an octave written
    /// before a pitch name, or a name that is not a string at all, is a
    /// `ValueError` there and not a `PitchException` — and callers catch the
    /// two separately, so the crate keeps them apart too.
    #[error("Value error: {0}")]
    Value(String),
    /// Another error, placed where a reader met it: a pitch an ABC note
    /// cannot spell, at the tune's line and measure. It reads as the error
    /// itself; [`Error::unplaced`] hands that error back to match on.
    #[error("{0}")]
    Placed(Box<PlacedError>),
}

/// An error that is no format's own, and where in its document a reader
/// met it.
///
/// ```
/// use music21::error::{Error, Position};
///
/// let error = music21::tinynotation::from_tiny_notation("4/4 c4 e99999999999999999999");
/// let error = error.unwrap_err();
/// assert!(error.to_string().starts_with("Duration error: Cannot parse token"));
/// assert!(matches!(error.unplaced(), Error::Duration(_)));
/// assert_eq!(error.position(), Some(&Position::line(1).with_column(8)));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct PlacedError {
    /// What went wrong.
    pub error: Error,
    /// Where.
    pub position: Position,
}

impl std::fmt::Display for PlacedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

/// A document of one format that cannot be read or written: what kind of
/// trouble it is, where the reader stood, and the reader's own words.
///
/// Its `Display` is the message alone; the [`Error`] variant holding it
/// names the format, and holds it boxed, so that the crate's every
/// `Result` is no wider for it.
///
/// ```
/// use music21::error::{FormatErrorKind, Position};
///
/// let error = music21::mei::from_mei("<mei><music").unwrap_err();
/// let read = error.format_error().unwrap();
/// assert_eq!(read.kind, FormatErrorKind::Malformed);
/// assert_eq!(error.position(), Some(&Position::line(1).with_column(12)));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct FormatError {
    /// What kind of trouble it is.
    pub kind: FormatErrorKind,
    /// Where in the document the reader was, where it knows.
    pub position: Option<Position>,
    /// What the reader or writer says went wrong.
    pub message: String,
}

impl FormatError {
    /// An error of `kind` saying `message`, at no position yet.
    pub fn new(kind: FormatErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            position: None,
            message: message.into(),
        }
    }

    /// The same error, at `position`.
    #[must_use]
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// What kind of trouble a [`FormatError`] is.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum FormatErrorKind {
    /// The document breaks its format's syntax: XML that is not
    /// well-formed, a token or a number that cannot be read, a file that
    /// ends early.
    Malformed,
    /// The document reads, but says something that cannot be: a value out
    /// of range, a reference to nothing, two things that contradict.
    Invalid,
    /// Something the reader or the writer does not handle, or that the
    /// format has no way to say.
    Unsupported,
    /// Past a limit the crate sets on what it reads, so that a hostile
    /// file cannot exhaust memory or time: elements nested past
    /// `MAX_DEPTH`, more than `MAX_MEASURES` measures, more than sixteen
    /// dots.
    TooLarge,
    /// What was asked for is not in the document: a tune's reference
    /// number, a part.
    NotFound,
    /// A program the crate runs could not be run, or failed: MuseScore.
    External,
}

/// Where in a document a reader was when it stopped: whichever of these
/// the reader knows. Lines and columns count from one, a column in
/// characters.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub struct Position {
    /// The line of the document.
    pub line: Option<usize>,
    /// The column of that line.
    pub column: Option<usize>,
    /// The measure, by the number the document gives it: MusicXML's are
    /// text, `"12"` or `"X1"`.
    pub measure: Option<String>,
    /// The part, by the id the document gives it.
    pub part: Option<String>,
    /// A MIDI file's track, counted from nought.
    pub track: Option<usize>,
    /// A MIDI track's tick, from its start.
    pub tick: Option<u64>,
}

impl Position {
    /// A line of the document.
    pub fn line(line: usize) -> Self {
        Self {
            line: Some(line),
            ..Self::default()
        }
    }

    /// A measure, by the number the document gives it.
    pub fn measure(number: impl Into<String>) -> Self {
        Self {
            measure: Some(number.into()),
            ..Self::default()
        }
    }

    /// A MIDI file's track, counted from nought.
    pub fn track(index: usize) -> Self {
        Self {
            track: Some(index),
            ..Self::default()
        }
    }

    /// The line and column, counted from one in characters, of the byte
    /// `at` in `text`.
    pub(crate) fn in_text(text: &str, at: usize) -> Self {
        let before = &text[..at];
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        let column = before[line_start..].chars().count() + 1;
        Self::line(line).with_column(column)
    }

    /// The same position, at a column of its line.
    #[must_use]
    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    /// The same position, in a part.
    #[must_use]
    pub fn in_part(mut self, part: impl Into<String>) -> Self {
        self.part = Some(part.into());
        self
    }

    /// The same position, at a tick of its track.
    #[must_use]
    pub fn at_tick(mut self, tick: u64) -> Self {
        self.tick = Some(tick);
        self
    }

    /// Fills what this position does not say from `outer`, which a reader
    /// knew further out: a track around a tick.
    fn fill_from(&mut self, outer: Position) {
        self.line = self.line.or(outer.line);
        self.column = self.column.or(outer.column);
        self.measure = self.measure.take().or(outer.measure);
        self.part = self.part.take().or(outer.part);
        self.track = self.track.or(outer.track);
        self.tick = self.tick.or(outer.tick);
    }
}

impl std::fmt::Display for Position {
    /// `line 3, column 7`, `part P1, measure 12`, `track 2, tick 960`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut said = Vec::new();
        if let Some(part) = &self.part {
            said.push(format!("part {part}"));
        }
        if let Some(measure) = &self.measure {
            said.push(format!("measure {measure}"));
        }
        if let Some(track) = self.track {
            said.push(format!("track {track}"));
        }
        if let Some(tick) = self.tick {
            said.push(format!("tick {tick}"));
        }
        if let Some(line) = self.line {
            said.push(format!("line {line}"));
        }
        if let Some(column) = self.column {
            said.push(format!("column {column}"));
        }
        f.write_str(&said.join(", "))
    }
}

/// The crate's constructor of each format's error, of kind
/// [`FormatErrorKind::Invalid`] until a reader says otherwise.
macro_rules! format_constructors {
    ($($(#[$attribute:meta])* $name:ident => $variant:ident,)*) => {
        impl Error {
            $(
                $(#[$attribute])*
                pub(crate) fn $name(message: impl Into<String>) -> Self {
                    Self::$variant(Box::new(FormatError::new(FormatErrorKind::Invalid, message)))
                }
            )*
        }
    };
}

format_constructors! {
    midi => Midi,
    abc => Abc,
    humdrum => Humdrum,
    xml => Xml,
    mei => Mei,
    roman_text => RomanText,
    capella => Capella,
    muse_data => MuseData,
    noteworthy => Noteworthy,
    muse_score => MuseScore,
    #[cfg(feature = "musicxml")]
    music_xml => MusicXml,
}

impl Error {
    /// The error itself, without the place a reader gave it: what to match
    /// on to ask which kind of error it is.
    pub fn unplaced(&self) -> &Error {
        match self {
            Self::Placed(placed) => &placed.error,
            _ => self,
        }
    }

    /// The document error this is, where it is a format's: which kind of
    /// trouble, where, and the reader's words.
    pub fn format_error(&self) -> Option<&FormatError> {
        match self.unplaced() {
            Self::Midi(error)
            | Self::Abc(error)
            | Self::Humdrum(error)
            | Self::Xml(error)
            | Self::Mei(error)
            | Self::RomanText(error)
            | Self::Capella(error)
            | Self::MuseData(error)
            | Self::Noteworthy(error)
            | Self::MuseScore(error) => Some(error.as_ref()),
            #[cfg(feature = "musicxml")]
            Self::MusicXml(error) => Some(error.as_ref()),
            _ => None,
        }
    }

    fn format_error_mut(&mut self) -> Option<&mut FormatError> {
        match self {
            Self::Midi(error)
            | Self::Abc(error)
            | Self::Humdrum(error)
            | Self::Xml(error)
            | Self::Mei(error)
            | Self::RomanText(error)
            | Self::Capella(error)
            | Self::MuseData(error)
            | Self::Noteworthy(error)
            | Self::MuseScore(error) => Some(error.as_mut()),
            #[cfg(feature = "musicxml")]
            Self::MusicXml(error) => Some(error.as_mut()),
            _ => None,
        }
    }

    /// Where in its document the error was met, where the reader knows.
    pub fn position(&self) -> Option<&Position> {
        if let Self::Placed(placed) = self {
            return Some(&placed.position);
        }
        self.format_error()?.position.as_ref()
    }

    /// Where this error says it is, to change.
    fn position_mut(&mut self) -> Option<&mut Position> {
        if let Self::Placed(placed) = self {
            return Some(&mut placed.position);
        }
        self.format_error_mut()?.position.as_mut()
    }

    /// The same error, of `kind` where it is a format's.
    #[must_use]
    pub(crate) fn of_kind(mut self, kind: FormatErrorKind) -> Self {
        if let Some(error) = self.format_error_mut() {
            error.kind = kind;
        }
        self
    }

    /// The same error, its line counted `before` lines further down: a
    /// reader that read one piece of a document places what went wrong in
    /// the whole of it.
    #[must_use]
    pub(crate) fn lines_down(mut self, before: usize) -> Self {
        let line = self
            .position_mut()
            .and_then(|position| position.line.as_mut());
        if let Some(line) = line {
            *line += before;
        }
        self
    }

    /// The same error, placed at what `position` says where it does not
    /// already say it: a reader's loop over measures or tracks places what
    /// went wrong inside one. An error that is no format's own is held in
    /// [`Error::Placed`].
    #[must_use]
    pub(crate) fn placed(mut self, position: impl FnOnce() -> Position) -> Self {
        if let Some(known) = self.position_mut() {
            known.fill_from(position());
            return self;
        }
        if let Some(error) = self.format_error_mut() {
            error.position = Some(position());
            return self;
        }
        Self::Placed(Box::new(PlacedError {
            error: self,
            position: position(),
        }))
    }
}

impl From<Infallible> for Error {
    fn from(value: Infallible) -> Self {
        match value {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as StdError;

    #[test]
    fn test_display_music21object() {
        let err = Error::Music21Object("error message".to_string());
        assert_eq!(format!("{err}"), "Music21Object error: error message");
    }

    #[test]
    fn test_display_chord() {
        let err = Error::Chord("chord error".to_string());
        assert_eq!(format!("{err}"), "Chord error: chord error");
    }

    #[test]
    fn test_display_pitch() {
        let err = Error::Pitch("pitch error".to_string());
        assert_eq!(format!("{err}"), "Pitch error: pitch error");
    }

    #[test]
    fn test_display_microtone() {
        let err = Error::Microtone("microtone error".to_string());
        assert_eq!(format!("{err}"), "Microtone error: microtone error");
    }

    #[test]
    fn test_display_accidental() {
        let err = Error::Accidental("accidental error".to_string());
        assert_eq!(format!("{err}"), "Accidental error: accidental error");
    }

    #[test]
    fn test_display_chordtables() {
        let err = Error::ChordTables("chordtables error".to_string());
        assert_eq!(format!("{err}"), "ChordTables error: chordtables error");
    }

    #[test]
    fn test_display_interval() {
        let err = Error::Interval("interval error".to_string());
        assert_eq!(format!("{err}"), "Interval error: interval error");
    }

    #[test]
    fn test_display_stepname() {
        let err = Error::StepName("step name error".to_string());
        assert_eq!(format!("{err}"), "StepName error: step name error");
    }

    #[test]
    fn test_display_ordinal() {
        let err = Error::Ordinal("ordinal error".to_string());
        assert_eq!(format!("{err}"), "Ordinal error: ordinal error");
    }

    #[test]
    fn test_display_polyrhythm() {
        let err = Error::Polyrhythm("polyrhythm error".to_string());
        assert_eq!(format!("{err}"), "Polyrhythm error: polyrhythm error");
    }

    #[test]
    fn test_display_tuningsystem() {
        let err = Error::TuningSystem("tuning system error".to_string());
        assert_eq!(format!("{err}"), "TuningSystem error: tuning system error");
    }

    #[test]
    fn a_format_error_shows_as_its_message_under_its_format() {
        let error = Error::abc("cannot read the meter \"x\"").placed(|| Position::line(3));
        assert_eq!(error.to_string(), "ABC error: cannot read the meter \"x\"");
        assert_eq!(error.position(), Some(&Position::line(3)));
        assert_eq!(error.format_error().unwrap().kind, FormatErrorKind::Invalid);
        assert!(Error::Pitch("pitch".to_string()).format_error().is_none());
    }

    #[test]
    fn a_position_said_inside_keeps_what_it_says() {
        let error = Error::midi("a tempo of no time to the quarter")
            .placed(|| Position::default().at_tick(96))
            .placed(|| Position::track(2).at_tick(0))
            .of_kind(FormatErrorKind::Malformed);
        let position = error.position().unwrap();
        assert_eq!(position, &Position::track(2).at_tick(96));
        assert_eq!(position.to_string(), "track 2, tick 96");
        assert_eq!(
            Position::measure("12").in_part("P1").to_string(),
            "part P1, measure 12"
        );
        assert_eq!(
            Position::line(4).with_column(2).to_string(),
            "line 4, column 2"
        );
        let shifted = Error::humdrum("x")
            .placed(|| Position::line(2))
            .lines_down(3);
        assert_eq!(shifted.position(), Some(&Position::line(5)));
    }

    #[test]
    fn an_error_of_no_format_is_placed_around_itself() {
        let error = Error::Pitch("cannot spell it".to_string())
            .placed(|| Position::line(4))
            .placed(|| Position::measure("2").with_column(9))
            .lines_down(1);
        assert_eq!(error.to_string(), "Pitch error: cannot spell it");
        assert_eq!(
            error.unplaced(),
            &Error::Pitch("cannot spell it".to_string())
        );
        let mut wanted = Position::line(5).with_column(9);
        wanted.measure = Some("2".to_string());
        assert_eq!(error.position(), Some(&wanted));
        assert!(error.format_error().is_none());

        let read = Error::abc("x").placed(|| Position::line(1));
        assert_eq!(read.unplaced(), &read);
    }

    #[test]
    fn test_source_none() {
        let errors = [
            Error::Music21Object("music21".to_string()),
            Error::Chord("chord".to_string()),
            Error::Pitch("pitch".to_string()),
            Error::Microtone("microtone".to_string()),
            Error::Accidental("accidental".to_string()),
            Error::ChordTables("chordtables".to_string()),
            Error::Interval("interval".to_string()),
            Error::StepName("step".to_string()),
            Error::PitchClass("pitch class".to_string()),
            Error::Ordinal("ordinal".to_string()),
            Error::Polyrhythm("polyrhythm".to_string()),
            Error::TuningSystem("tuning system".to_string()),
            Error::midi("midi"),
            Error::Analysis("analysis".to_string()),
        ];

        for err in errors.iter() {
            // Ensure that source() returns None for each Error.
            assert!(
                err.source().is_none(),
                "Expected None for source() in {err:?}"
            );
        }
    }

    #[test]
    fn test_all_errors_display() {
        let cases = [
            (
                Error::Music21Object("music21".to_string()),
                "Music21Object error: music21",
            ),
            (Error::Chord("chord".to_string()), "Chord error: chord"),
            (Error::Pitch("pitch".to_string()), "Pitch error: pitch"),
            (
                Error::Microtone("microtone".to_string()),
                "Microtone error: microtone",
            ),
            (
                Error::Accidental("accidental".to_string()),
                "Accidental error: accidental",
            ),
            (
                Error::ChordTables("chordtables".to_string()),
                "ChordTables error: chordtables",
            ),
            (
                Error::Interval("interval".to_string()),
                "Interval error: interval",
            ),
            (Error::StepName("step".to_string()), "StepName error: step"),
            (
                Error::PitchClass("pitchclass".to_string()),
                "PitchClass error: pitchclass",
            ),
            (
                Error::Ordinal("ordinal".to_string()),
                "Ordinal error: ordinal",
            ),
            (
                Error::Polyrhythm("polyrhythm".to_string()),
                "Polyrhythm error: polyrhythm",
            ),
            (
                Error::TuningSystem("tuning system".to_string()),
                "TuningSystem error: tuning system",
            ),
            (Error::midi("midi"), "Midi error: midi"),
            (
                Error::Analysis("analysis".to_string()),
                "Analysis error: analysis",
            ),
            (Error::Serial("serial".to_string()), "Serial error: serial"),
        ];

        for (err, expected) in cases.iter() {
            assert_eq!(format!("{err}"), *expected);
            assert!(err.source().is_none());
        }
    }
}
