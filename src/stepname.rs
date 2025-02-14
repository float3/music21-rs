use crate::{
    defaults::IntegerType,
    exception::{Exception, ExceptionResult},
};
use std::convert::TryFrom;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub(crate) enum StepName {
    C = 1,
    D = 2,
    E = 3,
    F = 4,
    G = 5,
    A = 6,
    B = 7,
}

pub(crate) type StepType = IntegerType;

impl StepName {
    pub(crate) fn dnn_offset_to_step(n: StepType) -> ExceptionResult<Self> {
        Self::try_from((n + 1) as u8)
            .map_err(|_| Exception::StepName(format!("dnn offset doesn't match step: {}", n)))
    }

    pub(crate) fn step_to_dnn_offset(&self) -> StepType {
        *self as StepType
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

impl TryFrom<u8> for StepName {
    type Error = Exception;

    fn try_from(value: u8) -> ExceptionResult<Self> {
        match value {
            1 => Ok(StepName::C),
            2 => Ok(StepName::D),
            3 => Ok(StepName::E),
            4 => Ok(StepName::F),
            5 => Ok(StepName::G),
            6 => Ok(StepName::A),
            7 => Ok(StepName::B),
            _ => Err(Exception::StepName(format!(
                "Invalid value for StepName: {}",
                value
            ))),
        }
    }
}

impl TryFrom<char> for StepName {
    type Error = Exception;

    fn try_from(value: char) -> ExceptionResult<Self> {
        match value.to_ascii_uppercase() {
            'A' => Ok(StepName::A),
            'B' => Ok(StepName::B),
            'C' => Ok(StepName::C),
            'D' => Ok(StepName::D),
            'E' => Ok(StepName::E),
            'F' => Ok(StepName::F),
            'G' => Ok(StepName::G),
            _ => Err(Exception::StepName(format!(
                "cannot make StepName out of {}",
                value
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_dnn_offset_to_step() {
        // Valid offsets: 0..=6 map to C, D, E, F, G, A, B respectively.
        assert_eq!(StepName::dnn_offset_to_step(0).unwrap(), StepName::C);
        assert_eq!(StepName::dnn_offset_to_step(1).unwrap(), StepName::D);
        assert_eq!(StepName::dnn_offset_to_step(2).unwrap(), StepName::E);
        assert_eq!(StepName::dnn_offset_to_step(3).unwrap(), StepName::F);
        assert_eq!(StepName::dnn_offset_to_step(4).unwrap(), StepName::G);
        assert_eq!(StepName::dnn_offset_to_step(5).unwrap(), StepName::A);
        assert_eq!(StepName::dnn_offset_to_step(6).unwrap(), StepName::B);

        // Invalid offset.
        let err = StepName::dnn_offset_to_step(7).unwrap_err();
        if let Exception::StepName(msg) = err {
            assert!(msg.contains("dnn offset doesn't match step"));
        } else {
            panic!("Unexpected error variant");
        }
    }

    #[test]
    fn test_step_to_dnn_offset() {
        assert_eq!(StepName::C.step_to_dnn_offset(), 1);
        assert_eq!(StepName::D.step_to_dnn_offset(), 2);
        assert_eq!(StepName::E.step_to_dnn_offset(), 3);
        assert_eq!(StepName::F.step_to_dnn_offset(), 4);
        assert_eq!(StepName::G.step_to_dnn_offset(), 5);
        assert_eq!(StepName::A.step_to_dnn_offset(), 6);
        assert_eq!(StepName::B.step_to_dnn_offset(), 7);
    }

    #[test]
    fn test_step_ref() {
        assert_eq!(StepName::C.step_ref(), 0);
        assert_eq!(StepName::D.step_ref(), 2);
        assert_eq!(StepName::E.step_ref(), 4);
        assert_eq!(StepName::F.step_ref(), 5);
        assert_eq!(StepName::G.step_ref(), 7);
        assert_eq!(StepName::A.step_ref(), 9);
        assert_eq!(StepName::B.step_ref(), 11);
    }

    #[test]
    fn test_ref_to_step() {
        assert_eq!(StepName::ref_to_step(0).unwrap(), StepName::C);
        assert_eq!(StepName::ref_to_step(2).unwrap(), StepName::D);
        assert_eq!(StepName::ref_to_step(4).unwrap(), StepName::E);
        assert_eq!(StepName::ref_to_step(5).unwrap(), StepName::F);
        assert_eq!(StepName::ref_to_step(7).unwrap(), StepName::G);
        assert_eq!(StepName::ref_to_step(9).unwrap(), StepName::A);
        assert_eq!(StepName::ref_to_step(11).unwrap(), StepName::B);

        let err = StepName::ref_to_step(1).unwrap_err();
        if let Exception::StepName(msg) = err {
            assert!(msg.contains("ref doesn't match any step"));
        } else {
            panic!("Unexpected error variant");
        }
    }

    #[test]
    fn test_try_from_char() {
        assert_eq!(StepName::try_from('C').unwrap(), StepName::C);
        assert_eq!(StepName::try_from('d').unwrap(), StepName::D);
        assert_eq!(StepName::try_from('E').unwrap(), StepName::E);
        assert_eq!(StepName::try_from('f').unwrap(), StepName::F);
        assert_eq!(StepName::try_from('G').unwrap(), StepName::G);
        assert_eq!(StepName::try_from('a').unwrap(), StepName::A);
        assert_eq!(StepName::try_from('B').unwrap(), StepName::B);

        let err = StepName::try_from('H').unwrap_err();
        if let Exception::StepName(msg) = err {
            assert!(msg.contains("cannot make StepName out of"));
        } else {
            panic!("Unexpected error variant");
        }
    }
}
