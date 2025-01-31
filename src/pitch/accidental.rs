use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

use super::FloatType;

#[derive(Clone, Debug)]
pub(crate) struct Accidental {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
    pub(crate) _alter: FloatType,
}

impl Accidental {
    pub(crate) fn new() -> Self {
        Self {
            proto: ProtoM21Object::new(),
            slottedobjectmixin: SlottedObjectMixin::new(),
            _alter: todo!(),
        }
    }
}

impl ProtoM21ObjectTrait for Accidental {}

impl SlottedObjectMixinTrait for Accidental {}
