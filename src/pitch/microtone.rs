use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

pub(crate) struct Microtone {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
}

impl Microtone {
    pub(crate) fn new() -> Self {
        Self {
            proto: ProtoM21Object::new(),
            slottedobjectmixin: SlottedObjectMixin::new(),
        }
    }
}

impl ProtoM21ObjectTrait for Microtone {}

impl SlottedObjectMixinTrait for Microtone {}
