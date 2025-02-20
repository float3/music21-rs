use keysignature::{KeySignature, KeySignatureTrait};

use crate::{
    base::Music21ObjectTrait,
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
    scale::{
        ScaleTrait,
        concretescale::ConcreteScaleTrait,
        diatonicscale::{Diatonicscale, DiatonicscaleTrait},
    },
};

pub(crate) mod keysignature;

pub(crate) struct Key {
    keysignature: KeySignature,
    diatonicscale: Diatonicscale,
}

impl Key {
    pub(crate) fn tonic(&self) -> Pitch {
        todo!()
    }
}

impl DiatonicscaleTrait for Key {}
impl ConcreteScaleTrait for Key {}
impl ScaleTrait for Key {}
impl KeySignatureTrait for Key {}
impl Music21ObjectTrait for Key {}
impl ProtoM21ObjectTrait for Key {}
