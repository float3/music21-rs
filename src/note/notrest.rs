use super::generalnote::{GeneralNote, GeneralNoteTrait};
use crate::{base::Music21ObjectTrait, defaults::IntegerType, prebase::ProtoM21ObjectTrait};

#[derive(Clone, Debug)]
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

pub(crate) trait IntoNotRests {
    fn into(self) -> Vec<NotRest>;
}

impl IntoNotRests for Vec<NotRest> {
    fn into(self) -> Vec<NotRest> {
        self
    }
}

impl IntoNotRests for String {
    fn into(self) -> Vec<NotRest> {
        todo!()
    }
}

impl IntoNotRests for Vec<String> {
    fn into(self) -> Vec<NotRest> {
        todo!()
    }
}

impl IntoNotRests for Vec<IntegerType> {
    fn into(self) -> Vec<NotRest> {
        todo!()
    }
}
