use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

use super::{FloatType, IntegerType};

#[derive(Clone, Debug)]
pub(crate) struct Microtone {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
    _cent_shift: FloatType,
    pub(crate) _alter: FloatType,
}

impl Microtone {
    pub(crate) fn new<T>(cents_or_string: Option<T>, harmonic_shift: Option<IntegerType>) -> Self
    where
        T: IntoMicrotone,
    {
        Self {
            proto: ProtoM21Object::new(),
            slottedobjectmixin: SlottedObjectMixin::new(),
            _alter: todo!(),
            _cent_shift: todo!(),
        }
    }
}

impl ProtoM21ObjectTrait for Microtone {}

impl SlottedObjectMixinTrait for Microtone {}

pub(crate) trait IntoMicrotone {
    fn into_microtone(self) -> Microtone;
}

impl IntoMicrotone for String {
    fn into_microtone(self) -> Microtone {
        todo!()
    }
}

impl IntoMicrotone for &str {
    fn into_microtone(self) -> Microtone {
        todo!()
    }
}

impl IntoMicrotone for IntegerType {
    fn into_microtone(self) -> Microtone {
        todo!()
    }
}

impl IntoMicrotone for FloatType {
    fn into_microtone(self) -> Microtone {
        todo!()
    }
}

impl IntoMicrotone for Microtone {
    fn into_microtone(self) -> Microtone {
        self
    }
}
