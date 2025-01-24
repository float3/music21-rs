use std::fmt::Display;

use crate::defaults::IntegerType;

#[derive(Debug, Copy, Clone)]
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
    type Error = ();

    fn try_from(value: IntegerType) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Ordinal::Zeroth),
            1 => Ok(Ordinal::First),
            2 => Ok(Ordinal::Second),
            3 => Ok(Ordinal::Third),
            4 => Ok(Ordinal::Fourth),
            5 => Ok(Ordinal::Fifth),
            6 => Ok(Ordinal::Sixth),
            7 => Ok(Ordinal::Seventh),
            8 => Ok(Ordinal::Eighth),
            9 => Ok(Ordinal::Ninth),
            10 => Ok(Ordinal::Tenth),
            11 => Ok(Ordinal::Eleventh),
            12 => Ok(Ordinal::Twelfth),
            13 => Ok(Ordinal::Thirteenth),
            14 => Ok(Ordinal::Fourteenth),
            15 => Ok(Ordinal::Fifteenth),
            16 => Ok(Ordinal::Sixteenth),
            17 => Ok(Ordinal::Seventeenth),
            18 => Ok(Ordinal::Eighteenth),
            19 => Ok(Ordinal::Nineteenth),
            20 => Ok(Ordinal::Twentieth),
            21 => Ok(Ordinal::TwentyFirst),
            22 => Ok(Ordinal::TwentySecond),
            _ => Err(()),
        }
    }
}

impl TryFrom<&str> for Ordinal {
    type Error = ();

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
            _ => Err(()),
        }
    }
}

impl Display for Ordinal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let x: String = {
            let this = &self;
            match this {
                Ordinal::Zeroth => "zeroth".to_string(),
                Ordinal::First => "first".to_string(),
                Ordinal::Second => "second".to_string(),
                Ordinal::Third => "third".to_string(),
                Ordinal::Fourth => "fourth".to_string(),
                Ordinal::Fifth => "fifth".to_string(),
                Ordinal::Sixth => "sixth".to_string(),
                Ordinal::Seventh => "seventh".to_string(),
                Ordinal::Eighth => "eighth".to_string(),
                Ordinal::Ninth => "ninth".to_string(),
                Ordinal::Tenth => "tenth".to_string(),
                Ordinal::Eleventh => "eleventh".to_string(),
                Ordinal::Twelfth => "twelfth".to_string(),
                Ordinal::Thirteenth => "thirteenth".to_string(),
                Ordinal::Fourteenth => "fourteenth".to_string(),
                Ordinal::Fifteenth => "fifteenth".to_string(),
                Ordinal::Sixteenth => "sixteenth".to_string(),
                Ordinal::Seventeenth => "seventeenth".to_string(),
                Ordinal::Eighteenth => "eighteenth".to_string(),
                Ordinal::Nineteenth => "nineteenth".to_string(),
                Ordinal::Twentieth => "twentieth".to_string(),
                Ordinal::TwentyFirst => "twenty-first".to_string(),
                Ordinal::TwentySecond => "twenty-second".to_string(),
            }
        };
        write!(f, "{}", x)
    }
}
