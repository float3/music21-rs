use std::convert::TryFrom;
use std::fmt::{self, Display};

use crate::defaults::IntegerType;
use crate::exception::Exception;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Ordinal {
    Zeroth = 0,
    First = 1,
    Second = 2,
    Third = 3,
    Fourth = 4,
    Fifth = 5,
    Sixth = 6,
    Seventh = 7,
    Eighth = 8,
    Ninth = 9,
    Tenth = 10,
    Eleventh = 11,
    Twelfth = 12,
    Thirteenth = 13,
    Fourteenth = 14,
    Fifteenth = 15,
    Sixteenth = 16,
    Seventeenth = 17,
    Eighteenth = 18,
    Nineteenth = 19,
    Twentieth = 20,
    TwentyFirst = 21,
    TwentySecond = 22,
}

impl TryFrom<IntegerType> for Ordinal {
    type Error = Exception;

    fn try_from(value: IntegerType) -> Result<Self, Self::Error> {
        const ORDINALS: [Ordinal; 23] = [
            Ordinal::Zeroth,
            Ordinal::First,
            Ordinal::Second,
            Ordinal::Third,
            Ordinal::Fourth,
            Ordinal::Fifth,
            Ordinal::Sixth,
            Ordinal::Seventh,
            Ordinal::Eighth,
            Ordinal::Ninth,
            Ordinal::Tenth,
            Ordinal::Eleventh,
            Ordinal::Twelfth,
            Ordinal::Thirteenth,
            Ordinal::Fourteenth,
            Ordinal::Fifteenth,
            Ordinal::Sixteenth,
            Ordinal::Seventeenth,
            Ordinal::Eighteenth,
            Ordinal::Nineteenth,
            Ordinal::Twentieth,
            Ordinal::TwentyFirst,
            Ordinal::TwentySecond,
        ];
        let idx = value as usize;
        if idx < ORDINALS.len() {
            Ok(ORDINALS[idx])
        } else {
            Err(Exception::Ordinal(format!(
                "Invalid ordinal value: {}",
                value
            )))
        }
    }
}

impl TryFrom<&str> for Ordinal {
    type Error = Exception;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "zeroth" => Ok(Ordinal::Zeroth),
            "first" => Ok(Ordinal::First),
            "second" => Ok(Ordinal::Second),
            "third" => Ok(Ordinal::Third),
            "fourth" => Ok(Ordinal::Fourth),
            "fifth" => Ok(Ordinal::Fifth),
            "sixth" => Ok(Ordinal::Sixth),
            "seventh" => Ok(Ordinal::Seventh),
            "eighth" => Ok(Ordinal::Eighth),
            "ninth" => Ok(Ordinal::Ninth),
            "tenth" => Ok(Ordinal::Tenth),
            "eleventh" => Ok(Ordinal::Eleventh),
            "twelfth" => Ok(Ordinal::Twelfth),
            "thirteenth" => Ok(Ordinal::Thirteenth),
            "fourteenth" => Ok(Ordinal::Fourteenth),
            "fifteenth" => Ok(Ordinal::Fifteenth),
            "sixteenth" => Ok(Ordinal::Sixteenth),
            "seventeenth" => Ok(Ordinal::Seventeenth),
            "eighteenth" => Ok(Ordinal::Eighteenth),
            "nineteenth" => Ok(Ordinal::Nineteenth),
            "twentieth" => Ok(Ordinal::Twentieth),
            "twenty-first" => Ok(Ordinal::TwentyFirst),
            "twenty-second" => Ok(Ordinal::TwentySecond),
            _ => Err(Exception::Ordinal(format!(
                "Invalid ordinal string: {}",
                value
            ))),
        }
    }
}

impl Display for Ordinal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Ordinal::Zeroth => "zeroth",
            Ordinal::First => "first",
            Ordinal::Second => "second",
            Ordinal::Third => "third",
            Ordinal::Fourth => "fourth",
            Ordinal::Fifth => "fifth",
            Ordinal::Sixth => "sixth",
            Ordinal::Seventh => "seventh",
            Ordinal::Eighth => "eighth",
            Ordinal::Ninth => "ninth",
            Ordinal::Tenth => "tenth",
            Ordinal::Eleventh => "eleventh",
            Ordinal::Twelfth => "twelfth",
            Ordinal::Thirteenth => "thirteenth",
            Ordinal::Fourteenth => "fourteenth",
            Ordinal::Fifteenth => "fifteenth",
            Ordinal::Sixteenth => "sixteenth",
            Ordinal::Seventeenth => "seventeenth",
            Ordinal::Eighteenth => "eighteenth",
            Ordinal::Nineteenth => "nineteenth",
            Ordinal::Twentieth => "twentieth",
            Ordinal::TwentyFirst => "twenty-first",
            Ordinal::TwentySecond => "twenty-second",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn test_try_from_integer_valid() {
        let cases = [
            (0, Ordinal::Zeroth),
            (1, Ordinal::First),
            (2, Ordinal::Second),
            (22, Ordinal::TwentySecond),
        ];
        for (input, expected) in cases {
            let ordinal = Ordinal::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_try_from_integer_invalid() {
        let result = Ordinal::try_from(23);
        assert!(result.is_err());
        if let Err(Exception::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid ordinal value: 23"));
        } else {
            panic!("Expected Exception::Ordinal error");
        }
    }

    #[test]
    fn test_try_from_str_valid() {
        let cases = [
            ("zeroth", Ordinal::Zeroth),
            ("First", Ordinal::First),
            ("SECOND", Ordinal::Second),
            ("twenty-first", Ordinal::TwentyFirst),
            ("Twenty-Second", Ordinal::TwentySecond),
        ];
        for (input, expected) in cases {
            let ordinal = Ordinal::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_try_from_str_invalid() {
        let result = Ordinal::try_from("invalid");
        assert!(result.is_err());
        if let Err(Exception::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid ordinal string: invalid"));
        } else {
            panic!("Expected Exception::Ordinal error");
        }
    }

    #[test]
    fn test_display() {
        let ordinal = Ordinal::TwentyFirst;
        assert_eq!(format!("{}", ordinal), "twenty-first");
    }
}
