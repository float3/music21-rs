use super::{IntegerType, Pitch};
use crate::{
    base::Music21ObjectTrait,
    duration::Duration,
    note::{
        generalnote::GeneralNoteTrait,
        notrest::{NotRest, NotRestTrait},
        Note,
    },
    prebase::ProtoM21ObjectTrait,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) struct ChordBase {
    notrest: NotRest,
    _notes: Vec<NotRest>,
    _overrides: HashMap<String, String>,
}

impl ChordBase {
    pub(crate) fn new<T>(notes: Option<T>, duration: &Option<Duration>) -> Self
    where
        T: IntoNotRests,
    {
        let mut x = Self {
            notrest: NotRest::new(duration.clone()),
            _notes: vec![],
            _overrides: HashMap::new(),
        };

        x.add_core_or_init(notes, duration);
        x
    }

    fn add_core_or_init<T>(
        &mut self,
        notes: Option<T>,
        duration: &Option<Duration>,
    ) -> Option<Duration>
    where
        T: IntoNotRests,
    {
        let mut quick_duration = false;

        let mut duration: &Option<Duration> = match duration {
            Some(_) => duration,
            None => {
                quick_duration = true;
                &self.duration().clone()
            }
        };

        if let Some(notes) = notes {
            let (use_duration, self_duration, notrests) =
                notes.into_not_rests(duration, quick_duration);
            duration = use_duration;
            notrests.into_iter().for_each(|n| {
                // n._chord_attached = Some(Arc::new(self));
                self._notes.push(n);
            })
        }

        duration.clone()
    }
}

pub(crate) trait ChordBaseTrait {}

impl ChordBaseTrait for ChordBase {}

impl NotRestTrait for ChordBase {}

impl GeneralNoteTrait for ChordBase {
    fn duration(&self) -> &Option<Duration> {
        self.notrest.duration()
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
    ) -> (
        &Option<Duration>, //useDuration
        Option<Duration>,  //self.duration
        Self::T,
    );
}

impl IntoNotRests for String {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        if self.contains(char::is_whitespace) {
            // Split into whitespace parts and delegate to &[&str]
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests(duration, quick_duration)
        } else {
            let note = Note::new(Some(self), duration.clone(), None, None)
                .get_super()
                .clone();
            (duration, duration.clone(), vec![note])
        }
    }
}

impl IntoNotRests for &str {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests(duration, quick_duration)
        } else {
            let note = Note::new(Some(self), duration.clone(), None, None)
                .get_super()
                .clone();
            (duration, duration.clone(), vec![note])
        }
    }
}

impl IntoNotRests for &[String] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        let notes = self
            .iter()
            .map(|s| {
                Note::new(Some(s.to_string()), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect();
        (duration, duration.clone(), notes)
    }
}

impl IntoNotRests for &[&str] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        let notes = self
            .iter()
            .map(|s| {
                Note::new(Some(*s), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect();
        (duration, duration.clone(), notes)
    }
}

impl IntoNotRests for &[Pitch] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        let notes = self
            .iter()
            .map(|p| {
                Note::new(Some(p.clone()), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect();
        (duration, duration.clone(), notes)
    }
}

impl IntoNotRests for &[ChordBase] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        if quick_duration {
            // Here we obtain self_duration from the first chord.
            let self_duration = self[0].duration().clone();
            let notes: Self::T = self
                .iter()
                .flat_map(|chord_base| chord_base._notes.clone())
                .collect();
            (&None, self_duration, notes)
        } else {
            let notes: Self::T = self
                .iter()
                .flat_map(|chord_base| chord_base._notes.clone())
                .collect();
            (duration, None, notes)
        }
    }
}

impl IntoNotRests for &[NotRest] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        (duration, duration.clone(), self.to_vec())
    }
}

impl IntoNotRests for &[IntegerType] {
    type T = Vec<NotRest>;
    fn into_not_rests(
        self,
        duration: &Option<Duration>,
        quick_duration: bool,
    ) -> (&Option<Duration>, Option<Duration>, Self::T) {
        let notes = self
            .iter()
            .map(|i| {
                Note::new(Some(*i), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect();
        (duration, duration.clone(), notes)
    }
}
