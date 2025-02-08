use crate::{
    base::{Music21Object, Music21ObjectTrait},
    prebase::ProtoM21ObjectTrait,
};

pub(crate) mod concretescale;
pub(crate) mod diatonicscale;

pub(crate) struct Scale {
    music21object: Music21Object,
}

pub(crate) trait ScaleTrait: Music21ObjectTrait {}

impl ScaleTrait for Scale {}

impl Music21ObjectTrait for Scale {}

impl ProtoM21ObjectTrait for Scale {}
