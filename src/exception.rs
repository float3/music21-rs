use std::error::Error;
use std::fmt;

pub(crate) type ExceptionResult<T> = Result<T, Exception>;

#[derive(Debug)]
pub enum Exception {
    Music21Object(String),
    Chord(String),
    Pitch(String),
    Microtone(String),
    Accidental(String),
    ChordTables(String),
    Interval(String),
    StepName(String),
    PitchClass(String),
    PitchClassString(String),
    Ordinal(String),
    Polyrhythm(String),
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exception::Music21Object(msg) => write!(f, "Music21Object error: {msg}"),
            Exception::Chord(msg) => write!(f, "Chord error: {msg}"),
            Exception::Pitch(msg) => write!(f, "Pitch error: {msg}"),
            Exception::Microtone(msg) => write!(f, "Microtone error: {msg}"),
            Exception::Accidental(msg) => write!(f, "Accidental error: {msg}"),
            Exception::ChordTables(msg) => write!(f, "ChordTables error: {msg}"),
            Exception::Interval(msg) => write!(f, "Interval error: {msg}"),
            Exception::StepName(msg) => write!(f, "StepName error: {msg}"),
            Exception::PitchClass(msg) => write!(f, "PitchClass error: {msg}"),
            Exception::PitchClassString(msg) => write!(f, "PitchClassString error: {msg}"),
            Exception::Ordinal(msg) => write!(f, "Ordinal error: {msg}"),
            Exception::Polyrhythm(msg) => write!(f, "Polyrhythm {msg}"),
        }
    }
}

impl Error for Exception {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_display_music21object() {
        let err = Exception::Music21Object("error message".to_string());
        assert_eq!(format!("{err}"), "Music21Object error: error message");
    }

    #[test]
    fn test_display_chord() {
        let err = Exception::Chord("chord error".to_string());
        assert_eq!(format!("{err}"), "Chord error: chord error");
    }

    #[test]
    fn test_display_pitch() {
        let err = Exception::Pitch("pitch error".to_string());
        assert_eq!(format!("{err}"), "Pitch error: pitch error");
    }

    #[test]
    fn test_display_microtone() {
        let err = Exception::Microtone("microtone error".to_string());
        assert_eq!(format!("{err}"), "Microtone error: microtone error");
    }

    #[test]
    fn test_display_accidental() {
        let err = Exception::Accidental("accidental error".to_string());
        assert_eq!(format!("{err}"), "Accidental error: accidental error");
    }

    #[test]
    fn test_display_chordtables() {
        let err = Exception::ChordTables("chordtables error".to_string());
        assert_eq!(format!("{err}"), "ChordTables error: chordtables error");
    }

    #[test]
    fn test_display_interval() {
        let err = Exception::Interval("interval error".to_string());
        assert_eq!(format!("{err}"), "Interval error: interval error");
    }

    #[test]
    fn test_display_stepname() {
        let err = Exception::StepName("step name error".to_string());
        assert_eq!(format!("{err}"), "StepName error: step name error");
    }

    #[test]
    fn test_display_pitchclassstring() {
        let err = Exception::PitchClassString("pitch class error".to_string());
        assert_eq!(
            format!("{err}"),
            "PitchClassString error: pitch class error"
        );
    }

    #[test]
    fn test_display_ordinal() {
        let err = Exception::Ordinal("ordinal error".to_string());
        assert_eq!(format!("{err}"), "Ordinal error: ordinal error");
    }

    #[test]
    fn test_source_none() {
        let exceptions = [
            Exception::Music21Object("music21".to_string()),
            Exception::Chord("chord".to_string()),
            Exception::Pitch("pitch".to_string()),
            Exception::Microtone("microtone".to_string()),
            Exception::Accidental("accidental".to_string()),
            Exception::ChordTables("chordtables".to_string()),
            Exception::Interval("interval".to_string()),
            Exception::StepName("step".to_string()),
            Exception::PitchClassString("pitch class".to_string()),
            Exception::Ordinal("ordinal".to_string()),
        ];

        for err in exceptions.iter() {
            // Ensure that source() returns None for each exception.
            assert!(
                err.source().is_none(),
                "Expected None for source() in {err:?}"
            );
        }
    }

    #[test]
    fn test_all_exceptions_display() {
        let cases = [
            (
                Exception::Music21Object("music21".to_string()),
                "Music21Object error: music21",
            ),
            (Exception::Chord("chord".to_string()), "Chord error: chord"),
            (Exception::Pitch("pitch".to_string()), "Pitch error: pitch"),
            (
                Exception::Microtone("microtone".to_string()),
                "Microtone error: microtone",
            ),
            (
                Exception::Accidental("accidental".to_string()),
                "Accidental error: accidental",
            ),
            (
                Exception::ChordTables("chordtables".to_string()),
                "ChordTables error: chordtables",
            ),
            (
                Exception::Interval("interval".to_string()),
                "Interval error: interval",
            ),
            (
                Exception::StepName("step".to_string()),
                "StepName error: step",
            ),
            (
                Exception::PitchClassString("pitchclass".to_string()),
                "PitchClassString error: pitchclass",
            ),
            (
                Exception::Ordinal("ordinal".to_string()),
                "Ordinal error: ordinal",
            ),
        ];

        for (err, expected) in cases.iter() {
            assert_eq!(format!("{err}"), *expected);
            assert!(err.source().is_none());
        }
    }
}
