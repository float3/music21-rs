use crate::{defaults::IntegerType, error::Error};

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum PitchClassString {
    a,
    A,
    t,
    T,
    b,
    B,
    e,
    E,
}

impl PitchClassString {
    pub(crate) fn to_number(self) -> IntegerType {
        match self {
            PitchClassString::a
            | PitchClassString::A
            | PitchClassString::t
            | PitchClassString::T => 10,
            PitchClassString::b
            | PitchClassString::B
            | PitchClassString::e
            | PitchClassString::E => 11,
        }
    }
}

impl From<PitchClassString> for IntegerType {
    fn from(val: PitchClassString) -> Self {
        val.to_number()
    }
}

impl TryFrom<char> for PitchClassString {
    type Error = crate::error::Error;

    fn try_from(c: char) -> Result<Self, Self::Error> {
        match c {
            'a' => Ok(PitchClassString::a),
            'A' => Ok(PitchClassString::A),
            't' => Ok(PitchClassString::t),
            'T' => Ok(PitchClassString::T),
            'b' => Ok(PitchClassString::b),
            'B' => Ok(PitchClassString::B),
            'e' => Ok(PitchClassString::e),
            'E' => Ok(PitchClassString::E),
            _ => Err(Error::PitchClassString(format!("Invalid pitch class: {c}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_try_from_char() {
        assert_eq!(
            PitchClassString::try_from('a').unwrap(),
            PitchClassString::a
        );
        assert_eq!(
            PitchClassString::try_from('A').unwrap(),
            PitchClassString::A
        );
        assert_eq!(
            PitchClassString::try_from('t').unwrap(),
            PitchClassString::t
        );
        assert_eq!(
            PitchClassString::try_from('T').unwrap(),
            PitchClassString::T
        );
        assert_eq!(
            PitchClassString::try_from('b').unwrap(),
            PitchClassString::b
        );
        assert_eq!(
            PitchClassString::try_from('B').unwrap(),
            PitchClassString::B
        );
        assert_eq!(
            PitchClassString::try_from('e').unwrap(),
            PitchClassString::e
        );
        assert_eq!(
            PitchClassString::try_from('E').unwrap(),
            PitchClassString::E
        );

        // Verify error for an invalid character.
        assert!(PitchClassString::try_from('X').is_err());
    }
}
