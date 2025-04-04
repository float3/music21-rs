use crate::{
    base::Music21ObjectTrait,
    exception::{Exception, ExceptionResult},
    note::Note,
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

use super::{
    IntegerType, IntervalBaseTrait, diatonicinterval::DiatonicInterval, direction::Direction,
    intervalbase::IntervalBase, specifier::Specifier,
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

    pub(crate) fn direction(&self) -> Direction {
        todo!()
    }

    fn value_setter(&mut self, value: IntegerType) -> ExceptionResult<()> {
        if value == 0 {
            return Err(Exception::Interval("Interval cannot be zero".to_owned()));
        }
        self._value = value;
        Ok(())
    }

    pub(crate) fn get_diatonic(&self, spec: Specifier) -> DiatonicInterval {
        DiatonicInterval::new(spec, self)
    }

    pub(crate) fn staff_distance(&self) -> IntegerType {
        todo!()
    }

    pub(crate) fn simple_undirected(&self) -> IntegerType {
        todo!()
    }

    pub(crate) fn is_perfectable(&self) -> bool {
        todo!()
    }
}

fn convert_generic_string(value: String) -> IntegerType {
    todo!()
}

fn convert_generic(value: IntegerType) -> IntegerType {
    let post = value;
    let direction_scalar = Direction::Ascending;
    post * direction_scalar as IntegerType
}

impl IntervalBaseTrait for GenericInterval {
    fn transpose_note(self, note1: Note) -> ExceptionResult<Note> {
        todo!()
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        todo!()
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        todo!()
    }

    fn reverse(self) -> ExceptionResult<Self>
    where
        Self: Sized,
    {
        if self.undirected() == 1 {
            GenericInterval::from_int(1)
        } else {
            GenericInterval::from_int(self.undirected() * -self.direction().as_int())
        }
    }
}

impl Music21ObjectTrait for GenericInterval {}

impl ProtoM21ObjectTrait for GenericInterval {}
