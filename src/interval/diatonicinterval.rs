use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{specifier::Specifier, GenericInterval, IntervalBaseTrait};

#[derive(Clone, Debug)]
pub(crate) struct DiatonicInterval {
    pub(crate) generic: GenericInterval,
    speicifier: Specifier,
}

impl DiatonicInterval {}

impl IntervalBaseTrait for DiatonicInterval {}

impl Music21ObjectTrait for DiatonicInterval {}

impl ProtoM21ObjectTrait for DiatonicInterval {}
