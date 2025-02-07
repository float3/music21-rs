use crate::{
    defaults::IntegerType,
    exceptions::{Exception, ExceptionResult},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum StepName {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

pub(crate) type StepType = IntegerType;

impl StepName {
    pub(crate) fn dnn_offset_to_step(n: StepType) -> ExceptionResult<Self> {
        match n {
            0 => Ok(Self::C),
            1 => Ok(Self::D),
            2 => Ok(Self::E),
            3 => Ok(Self::F),
            4 => Ok(Self::G),
            5 => Ok(Self::A),
            6 => Ok(Self::B),
            _ => Err(Exception::StepName(format!(
                "dnn offset doesn't match step: {}",
                n
            ))),
        }
    }

    pub(crate) fn step_to_dnn_offset(&self) -> StepType {
        match self {
            StepName::C => 1,
            StepName::D => 2,
            StepName::E => 3,
            StepName::F => 4,
            StepName::G => 5,
            StepName::A => 6,
            StepName::B => 7,
        }
    }

    pub(crate) fn step_ref(&self) -> StepType {
        match self {
            StepName::C => 0,
            StepName::D => 2,
            StepName::E => 4,
            StepName::F => 5,
            StepName::G => 7,
            StepName::A => 9,
            StepName::B => 11,
        }
    }

    pub(crate) fn ref_to_step(n: StepType) -> ExceptionResult<Self> {
        match n {
            0 => Ok(StepName::C),
            2 => Ok(StepName::D),
            4 => Ok(StepName::E),
            5 => Ok(StepName::F),
            7 => Ok(StepName::G),
            9 => Ok(StepName::A),
            11 => Ok(StepName::B),
            _ => Err(Exception::StepName(format!(
                "ref doesn't match any step: {}",
                n
            ))),
        }
    }
}
