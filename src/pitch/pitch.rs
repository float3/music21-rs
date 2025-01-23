use crate::{
    common::objects::slottedobjectmixin::SlottedObjectMixinTrait,
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

pub(crate) struct Pitch {
    proto: ProtoM21Object,
}

impl Pitch {
    pub(crate) fn new() -> Self {
        Self {
            proto: ProtoM21Object::new(),
        }
    }
}

impl ProtoM21ObjectTrait for Pitch {}

impl SlottedObjectMixinTrait for Pitch {}
