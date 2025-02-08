use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{
    concretescale::{ConcreteScale, ConcreteScaleTrait},
    ScaleTrait,
};

pub(crate) struct Diatonicscale {
    scale: ConcreteScale,
}

pub(crate) trait DiatonicscaleTrait: ConcreteScaleTrait {}

impl DiatonicscaleTrait for Diatonicscale {}

impl ConcreteScaleTrait for Diatonicscale {}

impl ScaleTrait for Diatonicscale {}

impl Music21ObjectTrait for Diatonicscale {}

impl ProtoM21ObjectTrait for Diatonicscale {}
