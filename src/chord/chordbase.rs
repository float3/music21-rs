use crate::{
    base::Music21ObjectTrait,
    note::{
        generalnote::GeneralNoteTrait,
        notrest::{NotRest, NotRestTrait},
    },
    prebase::ProtoM21ObjectTrait,
};

pub(crate) struct ChordBase {
    notrest: NotRest,
}

impl ChordBase {
    pub(crate) fn new() -> Self {
        Self {
            notrest: NotRest::new(),
        }
    }
}

pub(crate) trait ChordBaseTrait {}

impl ChordBaseTrait for ChordBase {}

impl NotRestTrait for ChordBase {}

impl GeneralNoteTrait for ChordBase {}

impl Music21ObjectTrait for ChordBase {}

impl ProtoM21ObjectTrait for ChordBase {}
