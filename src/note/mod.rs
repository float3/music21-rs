pub(crate) mod generalnote;
pub(crate) mod notrest;

use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::error::Result;
use crate::pitch::Pitch;

use generalnote::GeneralNoteTrait;
use notrest::NotRest;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A pitched note.
pub struct Note {
    notrest: NotRest,
    pub(crate) _pitch: Pitch,
    #[cfg_attr(feature = "serde", serde(skip))]
    _cache: Arc<Mutex<HashMap<String, String>>>,
}

impl Note {
    /// Builds a note from a pitch name such as `"C#4"` or `"E-"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        Self::new(Option::<Pitch>::None, None, None, Some(name.into()))
    }

    /// Builds a note from an existing [`Pitch`].
    pub fn from_pitch(pitch: Pitch) -> Result<Self> {
        Self::new(Some(pitch), None, None, None)
    }

    /// Returns the note's pitch.
    pub fn pitch(&self) -> &Pitch {
        &self._pitch
    }

    /// Returns the pitch name without an octave, such as `"C#"` or `"E-"`.
    pub fn pitch_name(&self) -> String {
        self._pitch.name()
    }

    /// Returns the pitch name with an octave when one is set.
    pub fn pitch_name_with_octave(&self) -> String {
        self._pitch.name_with_octave()
    }

    /// Returns the note duration when one has been assigned.
    pub fn duration(&self) -> Option<&Duration> {
        self.notrest.duration().as_ref()
    }

    /// Assigns a duration to the note.
    pub fn set_duration(&mut self, duration: Duration) {
        self.notrest.set_duration(&duration);
    }

    /// Returns a copy of this note with the supplied duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.set_duration(duration);
        self
    }

    pub(crate) fn new<T>(
        pitch: Option<T>,
        duration: Option<Duration>,
        name: Option<String>,
        name_with_octave: Option<String>,
    ) -> Result<Self>
    where
        T: IntoPitch,
    {
        let _pitch = match pitch {
            Some(pitch) => pitch.into_pitch(),
            None => Ok({
                let name = match name_with_octave {
                    Some(name_with_octave) => name_with_octave,
                    None => match name {
                        Some(name) => name,
                        None => "C4".to_string(),
                    },
                };

                Pitch::new(
                    Some(name),
                    None,
                    None,
                    Option::<IntegerType>::None,
                    Option::<IntegerType>::None,
                    None,
                    None,
                    None,
                    None,
                )?
            }),
        }?;

        Ok(Self {
            notrest: NotRest::new(duration),
            _pitch,
            _cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) fn get_super(&self) -> &NotRest {
        &self.notrest
    }

    pub(crate) fn pitch_changed(&self) {
        {
            let mut cache = match self._cache.lock() {
                Ok(cache) => cache,
                Err(err) => err.into_inner(),
            };
            cache.clear();
        }

        if let Some(chord) = &self.notrest._chord_attached {
            chord.clear_cache();
        }
    }

    #[cfg(test)]
    fn insert_cache_value_for_test(&self, key: &str, value: &str) {
        let mut cache = match self._cache.lock() {
            Ok(cache) => cache,
            Err(err) => err.into_inner(),
        };
        cache.insert(key.to_string(), value.to_string());
    }

    #[cfg(test)]
    fn cache_len_for_test(&self) -> usize {
        let cache = match self._cache.lock() {
            Ok(cache) => cache,
            Err(err) => err.into_inner(),
        };
        cache.len()
    }
}

impl GeneralNoteTrait for Note {
    fn duration(&self) -> &Option<Duration> {
        self.notrest.duration()
    }

    fn set_duration(&mut self, duration: &Duration) {
        self.notrest.set_duration(duration);
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

impl TryFrom<Pitch> for Note {
    type Error = crate::error::Error;

    fn try_from(value: Pitch) -> Result<Self> {
        Self::from_pitch(value)
    }
}

impl TryFrom<&Pitch> for Note {
    type Error = crate::error::Error;

    fn try_from(value: &Pitch) -> Result<Self> {
        Self::from_pitch(value.clone())
    }
}

impl TryFrom<IntegerType> for Note {
    type Error = crate::error::Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Note::new(Some(value), None, None, None)
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
        Note::new(Some(self), None, None, None)
    }
}

impl IntoNote for &Pitch {
    fn try_into_note(self) -> Result<Note> {
        Note::new(Some(self.clone()), None, None, None)
    }
}

impl IntoNote for String {
    fn try_into_note(self) -> Result<Note> {
        Note::new(Some(self), None, None, None)
    }
}

impl IntoNote for &String {
    fn try_into_note(self) -> Result<Note> {
        Note::new(Some(self.to_string()), None, None, None)
    }
}

impl IntoNote for &str {
    fn try_into_note(self) -> Result<Note> {
        Note::new(Some(self), None, None, None)
    }
}

impl IntoNote for IntegerType {
    const FROM_INTEGER_PITCH: bool = true;

    fn try_into_note(self) -> Result<Note> {
        Note::new(Some(self), None, None, None)
    }
}

pub(crate) trait IntoPitch {
    fn into_pitch(self) -> Result<Pitch>;
}

impl IntoPitch for Pitch {
    fn into_pitch(self) -> Result<Pitch> {
        Ok(self.clone())
    }
}

impl IntoPitch for String {
    fn into_pitch(self) -> Result<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }
}

impl IntoPitch for &str {
    fn into_pitch(self) -> Result<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }
}

impl IntoPitch for IntegerType {
    fn into_pitch(self) -> Result<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{IntoNote, Note};
    use crate::chord::chordbase::ChordBase;
    use crate::defaults::IntegerType;
    use crate::pitch::Pitch;
    use std::sync::Arc;

    #[test]
    fn pitch_changed_clears_note_cache() {
        let note = Note::new(Some("C4"), None, None, None).unwrap();

        note.insert_cache_value_for_test("pitchName", "C");
        assert_eq!(note.cache_len_for_test(), 1);

        note.pitch_changed();

        assert_eq!(note.cache_len_for_test(), 0);
    }

    #[test]
    fn pitch_changed_clears_attached_chord_cache() {
        let chord = ChordBase::new(Some("C E G"), &None).unwrap();
        chord.insert_cache_value_for_test("analysis", "major triad");
        assert_eq!(chord.cache_len_for_test(), 1);

        let mut note = Note::new(Some("E4"), None, None, None).unwrap();
        note.notrest._chord_attached = Some(Arc::clone(&chord));

        note.pitch_changed();

        assert_eq!(chord.cache_len_for_test(), 0);
    }

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

        let from_pitch = Note::try_from(Pitch::from_name("D4").unwrap()).unwrap();
        assert_eq!(from_pitch.pitch_name_with_octave(), "D4");

        let from_integer = Note::try_from(60 as IntegerType).unwrap();
        assert_eq!(from_integer.pitch_name_with_octave(), "C4");
    }
}
