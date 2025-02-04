use std::sync::Arc;

use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    display::{DisplayLocation, DisplaySize, DisplayStyle, DisplayType},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

use super::{FloatType, IntegerType, Pitch};

#[derive(Clone, Debug)]
pub(crate) struct Accidental {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
    _display_type: DisplayType,
    _display_status: Option<bool>,
    display_style: DisplayStyle,
    display_size: DisplaySize,
    display_location: DisplayLocation,
    _client: Option<Arc<Pitch>>,
    _name: String,
    _modifier: String,
    pub(crate) _alter: FloatType,
}

impl Accidental {
    pub(crate) fn new<T>(specifier: T) -> Self
    where
        T: IntoAccidental,
    {
        let mut acci = Self {
            proto: ProtoM21Object::new(),
            slottedobjectmixin: SlottedObjectMixin::new(),
            _display_type: DisplayType::Normal,
            _display_status: None,
            display_style: DisplayStyle::Normal,
            display_size: DisplaySize::Full,
            display_location: DisplayLocation::Normal,
            _client: None,
            _name: "".to_string(),
            _modifier: "".to_string(),
            _alter: 0.0,
        };

        acci.set(specifier);
        acci
    }

    fn set<T>(&mut self, specifier: T)
    where
        T: IntoAccidental,
    {
        todo!()
    }
}

impl ProtoM21ObjectTrait for Accidental {}

impl SlottedObjectMixinTrait for Accidental {}

pub(crate) trait IntoAccidental {
    fn into_accidental(self) -> Accidental;
}

impl IntoAccidental for IntegerType {
    fn into_accidental(self) -> Accidental {
        todo!()
    }
}

impl IntoAccidental for FloatType {
    fn into_accidental(self) -> Accidental {
        todo!()
    }
}

impl IntoAccidental for String {
    fn into_accidental(self) -> Accidental {
        todo!()
    }
}

impl IntoAccidental for &str {
    fn into_accidental(self) -> Accidental {
        todo!()
    }
}

impl IntoAccidental for Accidental {
    fn into_accidental(self) -> Accidental {
        self
    }
}
