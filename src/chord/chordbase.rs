use super::IntegerType;
use super::Pitch;

use crate::base::Music21ObjectTrait;
use crate::duration::Duration;
use crate::exception::ExceptionResult;
use crate::note::Note;
use crate::note::generalnote::GeneralNoteTrait;
use crate::note::notrest::NotRest;
use crate::note::notrest::NotRestTrait;
use crate::prebase::ProtoM21ObjectTrait;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct ChordBase {
    notrest: NotRest,
    #[cfg_attr(feature = "serde", serde(skip))]
    _notes: Arc<Mutex<Vec<NotRest>>>,
    _overrides: HashMap<String, String>,
}

impl ChordBase {
    pub(crate) fn new<T>(
        notes: Option<T>,
        duration: &Option<Duration>,
    ) -> ExceptionResult<Arc<Self>>
    where
        T: IntoNotRests,
    {
        let chord = Arc::new(Self {
            notrest: NotRest::new(duration.clone()),
            _notes: Arc::new(Mutex::new(vec![])),
            _overrides: HashMap::new(),
        });

        Self::add_core_or_init(Arc::clone(&chord), notes, duration)?;
        Ok(chord)
    }

    fn add_core_or_init<T>(
        chord: Arc<Self>,
        notes: Option<T>,
        duration: &Option<Duration>,
    ) -> ExceptionResult<Option<Duration>>
    where
        T: IntoNotRests,
    {
        let mut quick_duration = false;
        let mut duration_ref: &Option<Duration> = match duration {
            Some(_) => duration,
            None => {
                quick_duration = true;
                &chord.duration().clone()
            }
        };

        if let Some(notes) = notes {
            let (use_duration, _self_duration, notrests) =
                notes.into_not_rests(duration_ref, quick_duration)?;
            duration_ref = use_duration;
            notrests.into_iter().for_each(|mut n| {
                n._chord_attached = Some(Arc::clone(&chord));
                chord._notes.lock().unwrap().push(n);
            });
        }

        Ok(duration_ref.clone())
    }
}

pub(crate) trait ChordBaseTrait {}

impl ChordBaseTrait for ChordBase {}

impl NotRestTrait for ChordBase {}

impl GeneralNoteTrait for ChordBase {
    fn duration(&self) -> &Option<Duration> {
        self.notrest.duration()
    }

    fn set_duration(&self, duration: &Duration) {
        todo!()
    }
}

impl Music21ObjectTrait for ChordBase {}

impl ProtoM21ObjectTrait for ChordBase {}

pub(crate) trait IntoNotRests {
    type T: IntoIterator<Item = NotRest>;

    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(
        &Option<Duration>, //useDuration
        Option<Duration>,  //self.duration
        Self::T,
    )>;
}

impl IntoNotRests for String {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        if self.contains(char::is_whitespace) {
            // Split into whitespace parts and delegate to &[&str]
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests(duration, quick_duration)
        } else {
            let note = Note::new(Some(self), duration.clone(), None, None)?
                .get_super()
                .clone();
            Ok((duration, duration.clone(), vec![note]))
        }
    }
}

impl IntoNotRests for &str {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests(duration, quick_duration)
        } else {
            let note = Note::new(Some(self), duration.clone(), None, None)?
                .get_super()
                .clone();
            Ok((duration, duration.clone(), vec![note]))
        }
    }
}

impl IntoNotRests for &[String] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        let notes = self
            .iter()
            .map(|s| {
                Ok(
                    Note::new(Some(s.to_string()), duration.clone(), None, None)?
                        .get_super()
                        .clone(),
                )
            })
            .collect::<ExceptionResult<Vec<_>>>()?;
        Ok((duration, duration.clone(), notes))
    }
}

impl IntoNotRests for &[&str] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        let notes = self
            .iter()
            .map(|s| {
                Ok(Note::new(Some(*s), duration.clone(), None, None)?
                    .get_super()
                    .clone())
            })
            .collect::<ExceptionResult<Vec<_>>>()?;
        Ok((duration, duration.clone(), notes))
    }
}

impl IntoNotRests for &[Pitch] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        let notes = self
            .iter()
            .map(|p| {
                Ok(Note::new(Some(p.clone()), duration.clone(), None, None)?
                    .get_super()
                    .clone())
            })
            .collect::<ExceptionResult<Vec<_>>>()?;
        Ok((duration, duration.clone(), notes))
    }
}

impl IntoNotRests for &[ChordBase] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        if quick_duration {
            // Here we obtain self_duration from the first chord.
            let self_duration = self[0].duration().clone();
            let notes: Self::T = self
                .iter()
                .flat_map(|chord_base| chord_base._notes.lock().unwrap().clone())
                .collect();
            Ok((&None, self_duration, notes))
        } else {
            let notes: Self::T = self
                .iter()
                .flat_map(|chord_base| chord_base._notes.lock().unwrap().clone())
                .collect();
            Ok((duration, None, notes))
        }
    }
}

impl IntoNotRests for &[NotRest] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        Ok((duration, duration.clone(), self.to_vec()))
    }
}

impl IntoNotRests for &[IntegerType] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> ExceptionResult<(&Option<Duration>, Option<Duration>, Self::T)> {
        let notes = self
            .iter()
            .map(|i| {
                Ok(Note::new(Some(*i), duration.clone(), None, None)?
                    .get_super()
                    .clone())
            })
            .collect::<ExceptionResult<Vec<_>>>()?;
        Ok((duration, duration.clone(), notes))
    }
}
