use crate::{
    base::{Music21Object, Music21ObjectTrait},
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct IntervalBase {
    music21object: Music21Object,
}

impl IntervalBase {}

pub(crate) trait IntervalBaseTrait: Music21ObjectTrait {}

impl IntervalBaseTrait for IntervalBase {}

impl Music21ObjectTrait for IntervalBase {}

impl ProtoM21ObjectTrait for IntervalBase {}
