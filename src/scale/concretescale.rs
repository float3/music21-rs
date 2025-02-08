use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{Scale, ScaleTrait};

pub(crate) struct ConcreteScale {
    scale: Scale,
}

pub(crate) trait ConcreteScaleTrait: ScaleTrait {}

impl ConcreteScaleTrait for ConcreteScale {}

impl ScaleTrait for ConcreteScale {}

impl Music21ObjectTrait for ConcreteScale {}

impl ProtoM21ObjectTrait for ConcreteScale {}
