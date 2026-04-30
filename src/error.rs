use std::{convert::Infallible, error, fmt};

/// Result type returned by fallible `music21-rs` operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error variants produced by the crate's theory helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Error associated with a generic music21-style object.
    Music21Object(String),
    /// Error associated with chord construction or analysis.
    Chord(String),
    /// Error associated with pitch construction, spelling or conversion.
    Pitch(String),
    /// Error associated with microtone construction or conversion.
    Microtone(String),
    /// Error associated with accidental parsing or conversion.
    Accidental(String),
    /// Error associated with generated chord-table lookup data.
    ChordTables(String),
    /// Error associated with interval construction or conversion.
    Interval(String),
    /// Error associated with step-name parsing or conversion.
    StepName(String),
    /// Error associated with numeric pitch-class parsing.
    PitchClass(String),
    /// Error associated with pitch-class string parsing.
    PitchClassString(String),
    /// Error associated with ordinal-name parsing.
    Ordinal(String),
    /// Error associated with polyrhythm construction or timing.
    Polyrhythm(String),
    /// Error associated with tuning-system parsing or lookup.
    TuningSystem(String),
    /// Error associated with MIDI import or export.
    Midi(String),
    /// Error associated with analysis helpers.
    Analysis(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Music21Object(msg) => write!(f, "Music21Object error: {msg}"),
            Error::Chord(msg) => write!(f, "Chord error: {msg}"),
            Error::Pitch(msg) => write!(f, "Pitch error: {msg}"),
            Error::Microtone(msg) => write!(f, "Microtone error: {msg}"),
            Error::Accidental(msg) => write!(f, "Accidental error: {msg}"),
            Error::ChordTables(msg) => write!(f, "ChordTables error: {msg}"),
            Error::Interval(msg) => write!(f, "Interval error: {msg}"),
            Error::StepName(msg) => write!(f, "StepName error: {msg}"),
            Error::PitchClass(msg) => write!(f, "PitchClass error: {msg}"),
            Error::PitchClassString(msg) => write!(f, "PitchClassString error: {msg}"),
            Error::Ordinal(msg) => write!(f, "Ordinal error: {msg}"),
            Error::Polyrhythm(msg) => write!(f, "Polyrhythm error: {msg}"),
            Error::TuningSystem(msg) => write!(f, "TuningSystem error: {msg}"),
            Error::Midi(msg) => write!(f, "Midi error: {msg}"),
            Error::Analysis(msg) => write!(f, "Analysis error: {msg}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
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
    fn test_display_pitchclassstring() {
        let err = Error::PitchClassString("pitch class error".to_string());
        assert_eq!(
            format!("{err}"),
            "PitchClassString error: pitch class error"
        );
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
            Error::PitchClassString("pitch class".to_string()),
            Error::Ordinal("ordinal".to_string()),
            Error::Polyrhythm("polyrhythm".to_string()),
            Error::TuningSystem("tuning system".to_string()),
            Error::Midi("midi".to_string()),
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
                Error::PitchClassString("pitchclass".to_string()),
                "PitchClassString error: pitchclass",
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
            (Error::Midi("midi".to_string()), "Midi error: midi"),
            (
                Error::Analysis("analysis".to_string()),
                "Analysis error: analysis",
            ),
        ];

        for (err, expected) in cases.iter() {
            assert_eq!(format!("{err}"), *expected);
            assert!(err.source().is_none());
        }
    }
}
