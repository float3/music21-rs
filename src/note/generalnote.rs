use crate::duration::Duration;

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct GeneralNote {
    _duration: Option<Duration>,
}

impl GeneralNote {
    pub(crate) fn new(duration: Option<Duration>) -> Self {
        Self {
            _duration: duration,
        }
    }
}

pub(crate) trait GeneralNoteTrait {
    fn duration(&self) -> &Option<Duration>;
    fn set_duration(&mut self, duration: &Duration);
}

impl GeneralNoteTrait for GeneralNote {
    fn duration(&self) -> &Option<Duration> {
        &self._duration
    }

    fn set_duration(&mut self, duration: &Duration) {
        self._duration = Some(duration.clone());
    }
}
