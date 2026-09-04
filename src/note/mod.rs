use crate::defaults::{FloatType, IntegerType};
use crate::duration::Duration;
use crate::error::Result;
use crate::pitch::Pitch;

use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A pitched note.
pub struct Note {
    pub(crate) pitch: Pitch,
    duration: Option<Duration>,
}

impl Note {
    /// Builds a note from a pitch name such as `"C#4"` or `"E-"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        Pitch::from_name(name).map(Self::from_pitch)
    }

    /// Builds a note from a pitch-space number, where 60 is middle C.
    pub fn from_number(number: FloatType) -> Result<Self> {
        Pitch::from_number(number).map(Self::from_pitch)
    }

    /// Builds a note from an existing [`Pitch`].
    pub fn from_pitch(pitch: Pitch) -> Self {
        Self {
            pitch,
            duration: None,
        }
    }

    /// Returns the note's pitch.
    pub fn pitch(&self) -> &Pitch {
        &self.pitch
    }

    /// Returns the pitch name without an octave, such as `"C#"` or `"E-"`.
    pub fn pitch_name(&self) -> String {
        self.pitch.name()
    }

    /// Returns the pitch name with an octave when one is set.
    pub fn pitch_name_with_octave(&self) -> String {
        self.pitch.name_with_octave()
    }

    /// Returns the note duration when one has been assigned.
    pub fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }

    /// Assigns a duration to the note.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = Some(duration);
    }

    /// Returns a copy of this note with the supplied duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.set_duration(duration);
        self
    }
}

impl FromStr for Note {
    type Err = crate::error::Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<&str> for Note {
    type Error = crate::error::Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<String> for Note {
    type Error = crate::error::Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_name(value)
    }
}

impl From<Pitch> for Note {
    fn from(value: Pitch) -> Self {
        Self::from_pitch(value)
    }
}

impl From<&Pitch> for Note {
    fn from(value: &Pitch) -> Self {
        Self::from_pitch(value.clone())
    }
}

impl TryFrom<IntegerType> for Note {
    type Error = crate::error::Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::from_number(value as FloatType)
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pitch_name_with_octave())
    }
}

/// Converts a single note-like value into a [`Note`].
///
/// This is useful when constructing vectors or other collections that are
/// later passed to APIs such as `Chord::new`.
pub trait IntoNote {
    /// Whether this value came from an integer pitch class or MIDI-like number.
    const FROM_INTEGER_PITCH: bool = false;

    /// Converts the value into a note.
    fn try_into_note(self) -> Result<Note>;
}

impl IntoNote for Note {
    fn try_into_note(self) -> Result<Note> {
        Ok(self)
    }
}

impl IntoNote for &Note {
    fn try_into_note(self) -> Result<Note> {
        Ok(self.clone())
    }
}

impl IntoNote for Pitch {
    fn try_into_note(self) -> Result<Note> {
        Ok(Note::from_pitch(self))
    }
}

impl IntoNote for &Pitch {
    fn try_into_note(self) -> Result<Note> {
        Ok(Note::from_pitch(self.clone()))
    }
}

impl IntoNote for String {
    fn try_into_note(self) -> Result<Note> {
        Note::from_name(self)
    }
}

impl IntoNote for &String {
    fn try_into_note(self) -> Result<Note> {
        Note::from_name(self.as_str())
    }
}

impl IntoNote for &str {
    fn try_into_note(self) -> Result<Note> {
        Note::from_name(self)
    }
}

impl IntoNote for IntegerType {
    const FROM_INTEGER_PITCH: bool = true;

    fn try_into_note(self) -> Result<Note> {
        Note::from_number(self as FloatType)
    }
}

#[cfg(test)]
mod tests {
    use super::{IntoNote, Note};
    use crate::defaults::IntegerType;
    use crate::pitch::Pitch;

    #[test]
    fn into_note_accepts_note_like_inputs() {
        fn from_integer_pitch<T: IntoNote>() -> bool {
            T::FROM_INTEGER_PITCH
        }

        assert!(!from_integer_pitch::<&str>());
        assert!(from_integer_pitch::<IntegerType>());

        let note = Note::from_name("C4").unwrap();
        assert_eq!(
            note.clone()
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "C4"
        );

        let borrowed_note = Note::from_name("D4").unwrap();
        assert_eq!(
            (&borrowed_note)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "D4"
        );

        let pitch = Pitch::from_name("E4").unwrap();
        assert_eq!(
            pitch.try_into_note().unwrap().pitch_name_with_octave(),
            "E4"
        );

        let borrowed_pitch = Pitch::from_name("F4").unwrap();
        assert_eq!(
            (&borrowed_pitch)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "F4"
        );

        assert_eq!(
            "G4".to_string()
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "G4"
        );

        let owned_name = "A4".to_string();
        assert_eq!(
            (&owned_name)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "A4"
        );

        assert_eq!("B4".try_into_note().unwrap().pitch_name_with_octave(), "B4");

        assert_eq!(
            (60 as IntegerType)
                .try_into_note()
                .unwrap()
                .pitch_name_with_octave(),
            "C4"
        );
    }

    #[test]
    fn note_supports_rust_conversion_traits() {
        let parsed: Note = "C#4".parse().unwrap();
        assert_eq!(parsed.to_string(), "C#4");

        let from_pitch = Note::from(Pitch::from_name("D4").unwrap());
        assert_eq!(from_pitch.pitch_name_with_octave(), "D4");

        let from_integer = Note::try_from(60 as IntegerType).unwrap();
        assert_eq!(from_integer.pitch_name_with_octave(), "C4");
    }
}
