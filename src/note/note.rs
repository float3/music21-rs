use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{
    generalnote::GeneralNoteTrait,
    notrest::{NotRest, NotRestTrait},
};

pub(crate) struct Note {
    notrest: NotRest,
}

impl Note {
    pub(crate) fn new() -> Self {
        Self {
            notrest: NotRest::new(),
        }
    }
}

pub(crate) trait NoteTrait: NotRestTrait {}

impl NoteTrait for Note {}

impl NotRestTrait for Note {}

impl GeneralNoteTrait for Note {}

impl ProtoM21ObjectTrait for Note {}

impl Music21ObjectTrait for Note {}
