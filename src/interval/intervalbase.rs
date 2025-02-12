use crate::{
    base::{Music21Object, Music21ObjectTrait},
    exception::ExceptionResult,
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct IntervalBase {
    music21object: Music21Object,
}

impl IntervalBase {
    pub(crate) fn new() -> Self {
        Self {
            music21object: Music21Object::new(),
        }
    }
}

pub(crate) trait IntervalBaseTrait: Music21ObjectTrait {
    fn reverse(self) -> ExceptionResult<Self>
    where
        Self: Sized;
}

impl IntervalBaseTrait for IntervalBase {
    fn reverse(self) -> ExceptionResult<Self> {
        panic!("interval base doesn't know how to do this")
    }
}

impl Music21ObjectTrait for IntervalBase {}

impl ProtoM21ObjectTrait for IntervalBase {}
