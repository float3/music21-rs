pub(crate) mod generalnote;
pub(crate) mod notrest;

use crate::base::Music21ObjectTrait;
use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::exception::ExceptionResult;
use crate::pitch::Pitch;
use crate::prebase::ProtoM21ObjectTrait;

use generalnote::GeneralNoteTrait;
use notrest::{NotRest, NotRestTrait};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Note {
    notrest: NotRest,
    pub(crate) _pitch: Pitch,
    #[cfg_attr(feature = "serde", serde(skip))]
    _cache: Arc<Mutex<HashMap<String, String>>>,
}

impl Note {
    pub(crate) fn new<T>(
        pitch: Option<T>,
        duration: Option<Duration>,
        name: Option<String>,
        name_with_octave: Option<String>,
    ) -> ExceptionResult<Self>
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
                    Option::<i8>::None,
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

pub(crate) trait NoteTrait: NotRestTrait {}

impl NoteTrait for Note {}

impl NotRestTrait for Note {}

impl GeneralNoteTrait for Note {
    fn duration(&self) -> &Option<Duration> {
        self.notrest.duration()
    }

    fn set_duration(&mut self, duration: &Duration) {
        self.notrest.set_duration(duration);
    }
}

impl ProtoM21ObjectTrait for Note {}

impl Music21ObjectTrait for Note {}

pub(crate) trait IntoPitch {
    fn into_pitch(self) -> ExceptionResult<Pitch>;
}

impl IntoPitch for Pitch {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Ok(self.clone())
    }
}

impl IntoPitch for String {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }
}

impl IntoPitch for &str {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }
}

impl IntoPitch for IntegerType {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<i8>::None,
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
    use super::Note;
    use crate::chord::chordbase::ChordBase;
    use std::sync::Arc;

    #[test]
    fn pitch_changed_clears_note_cache() {
        let note = Note::new(
            Some("C4"),
            None,
            None,
            None,
        )
        .unwrap();

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

        let mut note = Note::new(
            Some("E4"),
            None,
            None,
            None,
        )
        .unwrap();
        note.notrest._chord_attached = Some(Arc::clone(&chord));

        note.pitch_changed();

        assert_eq!(chord.cache_len_for_test(), 0);
    }
}
