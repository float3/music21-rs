use super::generalnote::GeneralNote;
use super::generalnote::GeneralNoteTrait;

use crate::chord::chordbase::ChordBase;
use crate::duration::Duration;

use std::sync::Arc;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct NotRest {
    general_note: GeneralNote,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) _chord_attached: Option<Arc<ChordBase>>,
}

impl NotRest {
    pub(crate) fn new(duration: Option<Duration>) -> Self {
        Self {
            general_note: GeneralNote::new(duration),
            _chord_attached: None,
        }
    }
}

impl GeneralNoteTrait for NotRest {
    fn duration(&self) -> &Option<Duration> {
        self.general_note.duration()
    }

    fn set_duration(&mut self, duration: &Duration) {
        self.general_note.set_duration(duration);
    }
}
