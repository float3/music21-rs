use crate::{
    base::{Music21Object, Music21ObjectTrait},
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct GeneralNote {
    music21object: Music21Object,
}

impl GeneralNote {
    pub(crate) fn new() -> Self {
        Self {
            music21object: Music21Object::new(),
        }
    }
}

pub(crate) trait GeneralNoteTrait: Music21ObjectTrait {}

impl GeneralNoteTrait for GeneralNote {}

impl Music21ObjectTrait for GeneralNote {}

impl ProtoM21ObjectTrait for GeneralNote {}
