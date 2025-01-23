use super::{
    generalnote::GeneralNoteTrait,
    notrest::{IntoNotRests, NotRest, NotRestTrait},
};
use crate::{base::Music21ObjectTrait, defaults::IntegerType, prebase::ProtoM21ObjectTrait};

#[derive(Clone, Debug)]
pub struct Note {
    notrest: NotRest,
}

impl Note {
    pub(crate) fn new() -> Self {
        Self {
            notrest: NotRest::new(),
        }
    }

    pub(crate) fn get_super(&self) -> NotRest {
        self.notrest.clone()
    }
}

pub(crate) trait NoteTrait: NotRestTrait {}

impl NoteTrait for Note {}

impl NotRestTrait for Note {}

impl GeneralNoteTrait for Note {}

impl ProtoM21ObjectTrait for Note {}

impl Music21ObjectTrait for Note {}

pub(crate) trait IntoNotes {
    fn into(self) -> Vec<Note>;
}

impl IntoNotes for Vec<Note> {
    fn into(self) -> Vec<Note> {
        self
    }
}

impl IntoNotRests for Vec<Note> {
    fn into(self) -> Vec<NotRest> {
        self.into_iter().map(|note| note.get_super()).collect()
    }
}

impl IntoNotes for String {
    fn into(self) -> Vec<Note> {
        todo!()
    }
}

impl IntoNotes for Vec<String> {
    fn into(self) -> Vec<Note> {
        todo!()
    }
}

impl IntoNotes for Vec<IntegerType> {
    fn into(self) -> Vec<Note> {
        todo!()
    }
}
