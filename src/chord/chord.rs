use crate::{
    base::Music21ObjectTrait,
    note::{generalnote::GeneralNoteTrait, note::Note, notrest::NotRestTrait},
    pitch::pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

use super::chordbase::{ChordBase, ChordBaseTrait};

pub struct Chord {
    chordbase: ChordBase,
}

impl Chord {
    pub fn new() -> Self {
        Self {
            chordbase: ChordBase::new(),
        }
    }

    pub fn pitched_common_name(&self) -> String {
        todo!()
    }
}

pub(crate) trait ChordTrait {}

impl ChordTrait for Chord {}

impl ChordBaseTrait for Chord {}

impl NotRestTrait for Chord {}

impl GeneralNoteTrait for Chord {}

impl Music21ObjectTrait for Chord {}

impl ProtoM21ObjectTrait for Chord {}
