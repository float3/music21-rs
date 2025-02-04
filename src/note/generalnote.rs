use crate::{
    base::{Music21Object, Music21ObjectTrait},
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct GeneralNote {
    music21object: Music21Object,
    _duration: Option<crate::duration::Duration>,
}

impl GeneralNote {
    pub(crate) fn new(duration: Option<crate::duration::Duration>) -> Self {
        Self {
            music21object: Music21Object::new(),
            _duration: duration,
        }
    }
}

pub(crate) trait GeneralNoteTrait: Music21ObjectTrait {
    fn duration(&self) -> &Option<crate::duration::Duration>;
}

impl GeneralNoteTrait for GeneralNote {
    fn duration(&self) -> &Option<crate::duration::Duration> {
        &self._duration
    }
}

impl Music21ObjectTrait for GeneralNote {}

impl ProtoM21ObjectTrait for GeneralNote {}
