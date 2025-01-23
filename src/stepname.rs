use crate::defaults::IntegerType;

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
    pub(crate) fn step_to_dnn_offset_reverse(n: StepType) -> Self {
        match n {
            0 => Self::C,
            1 => Self::D,
            2 => Self::E,
            3 => Self::F,
            4 => Self::G,
            5 => Self::A,
            6 => Self::B,
            _ => panic!(),
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

    pub(crate) fn step_ref_reverse(n: StepType) -> Self {
        match n {
            0 => StepName::C,
            2 => StepName::D,
            4 => StepName::E,
            5 => StepName::F,
            7 => StepName::G,
            9 => StepName::A,
            11 => StepName::B,
            _ => panic!(),
        }
    }
}
