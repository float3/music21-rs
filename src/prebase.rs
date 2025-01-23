#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug)]
pub(crate) struct ProtoM21Object {}

impl ProtoM21Object {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

pub(crate) trait ProtoM21ObjectTrait {}

impl ProtoM21ObjectTrait for ProtoM21Object {}
