pub(crate) mod accidental;
pub(crate) mod microtone;
pub(crate) mod test;

use crate::{
    common::objects::slottedobjectmixin::SlottedObjectMixinTrait,
    note::{
        notrest::{IntoNotRests, NotRest},
        {IntoNotes, Note},
    },
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

pub(crate) struct Pitch {
    proto: ProtoM21Object,
}

impl Pitch {
    pub(crate) fn new() -> Self {
        Self {
            proto: ProtoM21Object::new(),
        }
    }
}

impl ProtoM21ObjectTrait for Pitch {}

impl SlottedObjectMixinTrait for Pitch {}

impl IntoNotes for Vec<Pitch> {
    fn into(self) -> Vec<Note> {
        todo!()
    }
}

impl IntoNotRests for Vec<Pitch> {
    fn into(self) -> Vec<NotRest> {
        todo!()
    }
}
