use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{intervalbase::IntervalBase, IntegerType, IntervalBaseTrait};

#[derive(Clone, Debug)]
pub(crate) struct ChromaticInterval {
    intervalbase: IntervalBase,
    pub(crate) semitones: IntegerType,
}

impl ChromaticInterval {
    pub(crate) fn new(semitones: IntegerType) -> Self {
        todo!()
    }
}

impl IntervalBaseTrait for ChromaticInterval {}

impl Music21ObjectTrait for ChromaticInterval {}

impl ProtoM21ObjectTrait for ChromaticInterval {}
