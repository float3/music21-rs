use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Duration {
    proto: ProtoM21Object,
    mixin: SlottedObjectMixin,
}

impl ProtoM21ObjectTrait for Duration {}

impl SlottedObjectMixinTrait for Duration {}
