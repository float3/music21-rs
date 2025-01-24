pub(crate) mod chordbase;
pub(crate) mod tables;

use chordbase::{ChordBase, ChordBaseTrait};

use crate::{
    base::Music21ObjectTrait,
    note::{
        generalnote::GeneralNoteTrait,
        notrest::{IntoNotRests, NotRestTrait},
        {IntoNotes, Note},
    },
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub struct Chord {
    chordbase: ChordBase,
    _notes: Vec<Note>,
}

impl Chord {
    pub fn new<T>(notes: Option<T>) -> Self
    where
        T: IntoNotes + IntoNotRests + Clone,
    {
        Self {
            chordbase: ChordBase::new(notes.clone().map(IntoNotRests::into)),
            _notes: notes.map_or_else(Vec::new, IntoNotes::into),
        }
    }

    pub(crate) fn get_super(&self) -> ChordBase {
        self.chordbase.clone()
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

impl IntoNotes for Vec<Chord> {
    fn into(self) -> Vec<Note> {
        self.into_iter().flat_map(|chord| chord._notes).collect()
    }
}
