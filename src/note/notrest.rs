use super::generalnote::GeneralNote;
use super::generalnote::GeneralNoteTrait;

use crate::duration::Duration;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct NotRest {
    general_note: GeneralNote,
}

impl NotRest {
    pub(crate) fn new(duration: Option<Duration>) -> Self {
        Self {
            general_note: GeneralNote::new(duration),
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
