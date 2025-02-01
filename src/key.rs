use crate::{
    base::{Music21Object, Music21ObjectTrait},
    defaults::IntegerType,
    prebase::ProtoM21ObjectTrait,
};

pub(crate) struct KeySignature {
    music21object: Music21Object,
    _sharps: IntegerType,
}

impl KeySignature {
    pub(crate) fn new(sharps: IntegerType) -> Self {
        Self {
            music21object: Music21Object::new(),
            _sharps: sharps,
        }
    }
}

pub(crate) trait KeySignatureTrait: Music21ObjectTrait {}

impl KeySignatureTrait for KeySignature {}

impl Music21ObjectTrait for KeySignature {}

impl ProtoM21ObjectTrait for KeySignature {}
