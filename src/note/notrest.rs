use super::generalnote::{GeneralNote, GeneralNoteTrait};
use crate::{base::Music21ObjectTrait, duration::Duration, prebase::ProtoM21ObjectTrait};

#[derive(Clone, Debug)]
pub(crate) struct NotRest {
    general_note: GeneralNote,
    // pub(crate) _chord_attached: Option<Arc<ChordBase>>,
}

impl NotRest {
    pub(crate) fn new(duration: Option<Duration>) -> Self {
        Self {
            general_note: GeneralNote::new(duration),
            // _chord_attached: None,
        }
    }
}

pub(crate) trait NotRestTrait: GeneralNoteTrait {}

impl NotRestTrait for NotRest {}

impl GeneralNoteTrait for NotRest {
    fn duration(&self) -> &Option<Duration> {
        self.general_note.duration()
    }
}

impl Music21ObjectTrait for NotRest {}

impl ProtoM21ObjectTrait for NotRest {}
