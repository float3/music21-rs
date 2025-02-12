use crate::{base::Music21ObjectTrait, exception::ExceptionResult, prebase::ProtoM21ObjectTrait};

use super::{
    diatonicinterval::DiatonicInterval, direction::Direction, intervalbase::IntervalBase,
    specifier::Specifier, IntegerType, IntervalBaseTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct GenericInterval {
    pub(crate) intervalbase: IntervalBase,
    _value: IntegerType,
}

impl GenericInterval {
    pub(crate) fn simple_directed(&self) -> IntegerType {
        todo!()
    }

    ///default value is "Unison"
    pub(crate) fn from_string(value: String) -> ExceptionResult<Self> {
        let mut slf = Self {
            intervalbase: IntervalBase::new(),
            _value: 1,
        };

        slf.value_setter(convert_generic_string(value))?;

        Ok(slf)
    }

    pub(crate) fn from_int(value: IntegerType) -> ExceptionResult<Self> {
        let mut slf = Self {
            intervalbase: IntervalBase::new(),
            _value: 1,
        };

        slf.value_setter(convert_generic(value))?;

        Ok(slf)
    }

    fn undirected(&self) -> IntegerType {
        todo!()
    }

    fn direction(&self) -> IntegerType {
        todo!()
    }

    fn value_setter(&mut self, value: IntegerType) -> ExceptionResult<()> {
        todo!()
    }

    pub(crate) fn get_diatonic(&self, spec_name: Specifier) -> DiatonicInterval {
        todo!()
    }
}

fn convert_generic_string(value: String) -> i64 {
    todo!()
}

fn convert_generic(value: IntegerType) -> IntegerType {
    let post = value;
    let direction_scalar = Direction::Ascending;
    post * direction_scalar as i64
}

impl IntervalBaseTrait for GenericInterval {
    fn reverse(self) -> crate::exception::ExceptionResult<Self>
    where
        Self: Sized,
    {
        if self.undirected() == 1 {
            GenericInterval::from_int(1)
        } else {
            GenericInterval::from_int(self.undirected() * -self.direction())
        }
    }
}

impl Music21ObjectTrait for GenericInterval {}

impl ProtoM21ObjectTrait for GenericInterval {}
