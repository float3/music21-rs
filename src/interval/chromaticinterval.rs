use crate::{
    base::Music21ObjectTrait, defaults::UnsignedIntegerType, prebase::ProtoM21ObjectTrait,
};

use super::{diatonicinterval::DiatonicInterval, intervalbase::IntervalBase, IntervalBaseTrait};

#[derive(Clone, Debug)]
pub(crate) struct ChromaticInterval {
    intervalbase: IntervalBase,
    pub(crate) semitones: UnsignedIntegerType,
}

impl ChromaticInterval {
    pub(crate) fn new(semitones: UnsignedIntegerType) -> Self {
        Self {
            intervalbase: IntervalBase::new(),
            semitones,
        }
    }

    pub(crate) fn get_diatonic(&self) -> DiatonicInterval {
        todo!()
    }
}

impl IntervalBaseTrait for ChromaticInterval {
    fn reverse(self) -> crate::exception::ExceptionResult<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl Music21ObjectTrait for ChromaticInterval {}

impl ProtoM21ObjectTrait for ChromaticInterval {}
