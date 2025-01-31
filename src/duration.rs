use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

#[derive(Clone)]
pub(crate) struct Duration {
    proto: ProtoM21Object,
    mixin: SlottedObjectMixin,
}

impl ProtoM21ObjectTrait for Duration {}

impl SlottedObjectMixinTrait for Duration {}
