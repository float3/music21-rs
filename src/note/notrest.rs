use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::generalnote::{GeneralNote, GeneralNoteTrait};

pub(crate) struct NotRest {
    general_note: GeneralNote,
}

impl NotRest {
    pub(crate) fn new() -> Self {
        Self {
            general_note: GeneralNote::new(),
        }
    }
}

pub(crate) trait NotRestTrait: GeneralNoteTrait {}

impl NotRestTrait for NotRest {}

impl GeneralNoteTrait for NotRest {}

impl Music21ObjectTrait for NotRest {}

impl ProtoM21ObjectTrait for NotRest {}
