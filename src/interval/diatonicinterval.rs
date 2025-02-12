use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{
    chromaticinterval::ChromaticInterval, specifier::Specifier, GenericInterval, IntervalBaseTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct DiatonicInterval {
    pub(crate) generic: GenericInterval,
    speicifier: Specifier,
}

impl DiatonicInterval {
    pub(crate) fn get_chromatic(&self) -> ChromaticInterval {
        todo!()
    }
}

impl IntervalBaseTrait for DiatonicInterval {
    fn reverse(self) -> crate::exception::ExceptionResult<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl Music21ObjectTrait for DiatonicInterval {}

impl ProtoM21ObjectTrait for DiatonicInterval {}
