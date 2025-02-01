use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{IntegerType, IntervalBaseTrait};

#[derive(Clone, Debug)]
pub(crate) struct GenericInterval {}
impl GenericInterval {
    pub(crate) fn simple_directed(&self) -> IntegerType {
        todo!()
    }
}

impl IntervalBaseTrait for GenericInterval {}

impl Music21ObjectTrait for GenericInterval {}

impl ProtoM21ObjectTrait for GenericInterval {}
