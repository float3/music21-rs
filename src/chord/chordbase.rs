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
    pub(crate) fn new<T>(notes: Option<T>, duration: Option<Duration>) -> Self
    where
        T: IntoNotRests,
    {
        let mut x = Self {
            notrest: NotRest::new(duration.clone()),
            _notes: vec![],
            _overrides: HashMap::new(),
        };

        x.add_core_or_init(notes, &duration);
        x
    }

    fn add_core_or_init<T>(&mut self, notes: Option<T>, duration: &Option<Duration>)
    where
        T: IntoNotRests,
    {
        todo!()
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

    fn into_not_rests(self, duration: Option<Duration>) -> Self::T;
}

impl IntoNotRests for String {
    type T = Vec<NotRest>;
    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests(duration)
        } else {
            vec![Note::new(Some(self.clone()), duration, None, None)
                .get_super()
                .clone()]
        }
    }
}

impl IntoNotRests for &[String] {
    type T = Vec<NotRest>;
    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        self.iter()
            .map(|s| {
                Note::new(Some(s.to_string()), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect()
    }
}

impl IntoNotRests for &str {
    type T = Vec<NotRest>;
    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .into_not_rests(duration)
        } else {
            vec![Note::new(Some(self), duration.clone(), None, None)
                .get_super()
                .clone()]
        }
    }
}

impl IntoNotRests for &[&str] {
    type T = Vec<NotRest>;
    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        self.iter()
            .map(|s| {
                Note::new(Some(*s), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect()
    }
}

impl IntoNotRests for &[Pitch] {
    type T = Vec<NotRest>;
    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        self.iter()
            .map(|p| {
                Note::new(Some(p.clone()), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect()
    }
}

impl IntoNotRests for &[ChordBase] {
    type T = Vec<NotRest>;

    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        self.iter()
            .flat_map(|chord_base| chord_base._notes.clone())
            .collect()
    }
}

impl IntoNotRests for &[NotRest] {
    type T = Vec<NotRest>;

    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        self.to_vec()
    }
}

impl IntoNotRests for &[IntegerType] {
    type T = Vec<NotRest>;
    fn into_not_rests(self, duration: Option<Duration>) -> Self::T {
        self.iter()
            .map(|i| {
                Note::new(Some(*i), duration.clone(), None, None)
                    .get_super()
                    .clone()
            })
            .collect()
    }
}

// pub(crate) trait IntoNotRest {
//     fn into_not_rest(&self) -> NotRest;
// }

// impl<T> IntoNotRests for T
// where
//     T: IntoNotRest,
// {
//     type T = Vec<NotRest>;

//     fn into_not_rests(self, duration: Duration) -> Self::T {
//         vec![self.into_not_rest()]
//     }
// }
