use crate::prebase::{ProtoM21Object, ProtoM21ObjectTrait};

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Music21Object {
    proto: ProtoM21Object,
}

impl Music21Object {
    pub(crate) fn new() -> Self {
        Self {
            proto: ProtoM21Object::new(),
        }
    }

    pub(crate) fn get_super(&self) -> ProtoM21Object {
        self.proto.clone()
    }
}

pub(crate) trait Music21ObjectTrait: ProtoM21ObjectTrait {}

impl Music21ObjectTrait for Music21Object {}

impl ProtoM21ObjectTrait for Music21Object {}
