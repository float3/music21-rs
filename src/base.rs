use crate::prebase::{ProtoM21Object, ProtoM21ObjectTrait};

#[derive(Debug)]
pub(crate) struct Music21Object {
    proto: ProtoM21Object,
}

impl Music21Object {
    pub(crate) fn new() -> Self {
        Self {
            proto: ProtoM21Object::new(),
        }
    }
}

pub(crate) trait Music21ObjectTrait: ProtoM21ObjectTrait {}

impl Music21ObjectTrait for Music21Object {}

impl ProtoM21ObjectTrait for Music21Object {}
