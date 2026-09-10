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
    /// Error associated with MIDI import or export.
    #[error("Midi error: {0}")]
    Midi(String),
    /// Error associated with analysis helpers.
    #[error("Analysis error: {0}")]
    Analysis(String),
    /// Error associated with time-signature parsing or beat lookup.
    #[error("Meter error: {0}")]
    Meter(String),
    /// Error associated with Xenakis sieve parsing or evaluation.
    #[error("Sieve error: {0}")]
    Sieve(String),
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
    /// Error associated with tone rows and serial transformations.
    #[error("Serial error: {0}")]
    Serial(String),
    /// Error associated with ties, noteheads, stems and lyrics.
    #[error("Notation error: {0}")]
    Notation(String),
    /// Error associated with note volumes.
    #[error("Volume error: {0}")]
    Volume(String),
    /// Error associated with Harte chord labels and degrees.
    #[error("Harte error: {0}")]
    Harte(String),
    /// A value the caller gave that nothing musical could be read from.
    ///
    /// music21 keeps this apart from its own exceptions — an octave written
    /// before a pitch name, or a name that is not a string at all, is a
    /// `ValueError` there and not a `PitchException` — and callers catch the
    /// two separately, so the crate keeps them apart too.
    #[error("Value error: {0}")]
    Value(String),
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
            (Error::Serial("serial".to_string()), "Serial error: serial"),
        ];

        for (err, expected) in cases.iter() {
            assert_eq!(format!("{err}"), *expected);
            assert!(err.source().is_none());
        }
    }
}
