use crate::{
    base::Music21ObjectTrait,
    note::{
        generalnote::GeneralNoteTrait,
        notrest::{IntoNotRests, NotRest, NotRestTrait},
    },
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct ChordBase {
    notrest: NotRest,
    _notes: Vec<NotRest>,
}

impl ChordBase {
    pub(crate) fn new<T>(notes: Option<T>) -> Self
    where
        T: IntoNotRests,
    {
        Self {
            notrest: NotRest::new(),
            _notes: notes.map_or_else(Vec::new, IntoNotRests::into),
        }
    }
}

pub(crate) trait ChordBaseTrait {}

impl ChordBaseTrait for ChordBase {}

impl NotRestTrait for ChordBase {}

impl GeneralNoteTrait for ChordBase {}

impl Music21ObjectTrait for ChordBase {}

impl ProtoM21ObjectTrait for ChordBase {}

impl IntoNotRests for Vec<ChordBase> {
    fn into(self) -> Vec<NotRest> {
        self.into_iter().flat_map(|chord| chord._notes).collect()
    }
}
