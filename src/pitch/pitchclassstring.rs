use crate::{defaults::IntegerType, exception::Exception};

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
    pub(crate) fn as_char(&self) -> char {
        match self {
            PitchClassString::a => 'a',
            PitchClassString::A => 'A',
            PitchClassString::t => 't',
            PitchClassString::T => 'T',
            PitchClassString::b => 'b',
            PitchClassString::B => 'B',
            PitchClassString::e => 'e',
            PitchClassString::E => 'E',
        }
    }

    pub(crate) fn to_uppercase(self) -> Self {
        match self {
            PitchClassString::a | PitchClassString::A => PitchClassString::A,
            PitchClassString::t | PitchClassString::T => PitchClassString::T,
            PitchClassString::b | PitchClassString::B => PitchClassString::B,
            PitchClassString::e | PitchClassString::E => PitchClassString::E,
        }
    }

    pub(crate) fn to_lowercase(self) -> Self {
        match self {
            PitchClassString::a | PitchClassString::A => PitchClassString::a,
            PitchClassString::t | PitchClassString::T => PitchClassString::t,
            PitchClassString::b | PitchClassString::B => PitchClassString::b,
            PitchClassString::e | PitchClassString::E => PitchClassString::e,
        }
    }

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
    type Error = Exception;

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
            _ => Err(Exception::PitchClassString(format!(
                "Invalid pitch class: {}",
                c
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_as_char() {
        assert_eq!(PitchClassString::a.as_char(), 'a');
        assert_eq!(PitchClassString::A.as_char(), 'A');
        assert_eq!(PitchClassString::t.as_char(), 't');
        assert_eq!(PitchClassString::T.as_char(), 'T');
        assert_eq!(PitchClassString::b.as_char(), 'b');
        assert_eq!(PitchClassString::B.as_char(), 'B');
        assert_eq!(PitchClassString::e.as_char(), 'e');
        assert_eq!(PitchClassString::E.as_char(), 'E');
    }

    #[test]
    fn test_to_uppercase() {
        assert_eq!(PitchClassString::a.to_uppercase(), PitchClassString::A);
        assert_eq!(PitchClassString::A.to_uppercase(), PitchClassString::A);
        assert_eq!(PitchClassString::t.to_uppercase(), PitchClassString::T);
        assert_eq!(PitchClassString::T.to_uppercase(), PitchClassString::T);
        assert_eq!(PitchClassString::b.to_uppercase(), PitchClassString::B);
        assert_eq!(PitchClassString::B.to_uppercase(), PitchClassString::B);
        assert_eq!(PitchClassString::e.to_uppercase(), PitchClassString::E);
        assert_eq!(PitchClassString::E.to_uppercase(), PitchClassString::E);
    }

    #[test]
    fn test_to_lowercase() {
        assert_eq!(PitchClassString::a.to_lowercase(), PitchClassString::a);
        assert_eq!(PitchClassString::A.to_lowercase(), PitchClassString::a);
        assert_eq!(PitchClassString::t.to_lowercase(), PitchClassString::t);
        assert_eq!(PitchClassString::T.to_lowercase(), PitchClassString::t);
        assert_eq!(PitchClassString::b.to_lowercase(), PitchClassString::b);
        assert_eq!(PitchClassString::B.to_lowercase(), PitchClassString::b);
        assert_eq!(PitchClassString::e.to_lowercase(), PitchClassString::e);
        assert_eq!(PitchClassString::E.to_lowercase(), PitchClassString::e);
    }

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
