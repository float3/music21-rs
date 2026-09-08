//! The quality of an interval: music21's `interval.Specifier`.

use std::fmt;
use std::str::FromStr;

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
};

/// An interval quality, numbered as music21 numbers its `Specifier` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub enum Specifier {
    /// `P`
    Perfect = 1,
    /// `M`
    Major = 2,
    /// `m`
    Minor = 3,
    /// `A`
    Augmented = 4,
    /// `d`
    Diminished = 5,
    /// `AA`
    DoubleAugmented = 6,
    /// `dd`
    DoubleDiminished = 7,
    /// `AAA`
    TripleAugmented = 8,
    /// `ddd`
    TripleDiminished = 9,
    /// `AAAA`
    QuadrupleAugmented = 10,
    /// `dddd`
    QuadrupleDiminished = 11,
}

impl Specifier {
    /// Every specifier, in music21's numbering order.
    pub const ALL: [Specifier; 11] = [
        Specifier::Perfect,
        Specifier::Major,
        Specifier::Minor,
        Specifier::Augmented,
        Specifier::Diminished,
        Specifier::DoubleAugmented,
        Specifier::DoubleDiminished,
        Specifier::TripleAugmented,
        Specifier::TripleDiminished,
        Specifier::QuadrupleAugmented,
        Specifier::QuadrupleDiminished,
    ];

    /// music21's number for the specifier, `1` for perfect through `11`.
    pub fn value(self) -> IntegerType {
        self as IntegerType
    }

    /// The specifier with music21's number, if there is one.
    pub fn from_value(value: IntegerType) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|specifier| specifier.value() == value)
            .ok_or_else(|| Error::Interval(format!("{value} is not a valid Specifier")))
    }

    /// Returns a human-friendly name for the specifier, music21's `niceName`.
    pub fn nice_name(&self) -> String {
        match self {
            Specifier::Perfect => "Perfect".to_string(),
            Specifier::Major => "Major".to_string(),
            Specifier::Minor => "Minor".to_string(),
            Specifier::Augmented => "Augmented".to_string(),
            Specifier::Diminished => "Diminished".to_string(),
            Specifier::DoubleAugmented => "Doubly-Augmented".to_string(),
            Specifier::DoubleDiminished => "Doubly-Diminished".to_string(),
            Specifier::TripleAugmented => "Triply-Augmented".to_string(),
            Specifier::TripleDiminished => "Triply-Diminished".to_string(),
            Specifier::QuadrupleAugmented => "Quadruply-Augmented".to_string(),
            Specifier::QuadrupleDiminished => "Quadruply-Diminished".to_string(),
        }
    }

    /// Parses a specifier the way music21's `parseSpecifier` does: a prefix
    /// such as `"P"`, `"m"` or `"AA"`, or a spelled-out name such as
    /// `"Perfect"` or `"Doubly-Augmented"`.
    ///
    /// Case is significant only for `m`/`M`: minor and major are the one pair
    /// where both spellings already denote different intervals. Every other
    /// specifier accepts either case, which is exactly the distinction music21
    /// draws — `a2`/`A2`, `d5`/`D5`, `p5`/`P5` and the doubled and tripled
    /// forms all parse there, while `m3` and `M3` stay distinct. Lowercasing
    /// the input wholesale would silently turn every major third minor.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "M" => return Ok(Specifier::Major),
            "m" => return Ok(Specifier::Minor),
            _ => {}
        }
        let lower = name.to_ascii_lowercase();
        if let Some(found) = Self::ALL.into_iter().find(|specifier| {
            specifier.prefix().to_ascii_lowercase() == lower
                || specifier.nice_name().to_ascii_lowercase() == lower
        }) {
            return Ok(found);
        }
        match lower.as_str() {
            "double augmented" => Ok(Specifier::DoubleAugmented),
            "double diminished" => Ok(Specifier::DoubleDiminished),
            "triple augmented" => Ok(Specifier::TripleAugmented),
            "triple diminished" => Ok(Specifier::TripleDiminished),
            "quadruple augmented" => Ok(Specifier::QuadrupleAugmented),
            "quadruple diminished" => Ok(Specifier::QuadrupleDiminished),
            _ => Err(Error::Interval(format!(
                "Cannot find a match for value: '{name}'"
            ))),
        }
    }

    pub(crate) fn parse(remain: &str) -> Result<Self> {
        Self::from_name(remain)
    }

    /// Returns the prefix music21 writes before the interval number, such as
    /// `"P"`, `"m"` or `"AA"`.
    pub fn prefix(self) -> &'static str {
        match self {
            Specifier::Perfect => "P",
            Specifier::Major => "M",
            Specifier::Minor => "m",
            Specifier::Augmented => "A",
            Specifier::Diminished => "d",
            Specifier::DoubleAugmented => "AA",
            Specifier::DoubleDiminished => "dd",
            Specifier::TripleAugmented => "AAA",
            Specifier::TripleDiminished => "ddd",
            Specifier::QuadrupleAugmented => "AAAA",
            Specifier::QuadrupleDiminished => "dddd",
        }
    }

    /// The specifier of the inverted interval: major becomes minor,
    /// augmented becomes diminished, perfect stays perfect.
    pub fn inversion(&self) -> Self {
        match self {
            Specifier::Perfect => Specifier::Perfect,
            Specifier::Major => Specifier::Minor,
            Specifier::Minor => Specifier::Major,
            Specifier::Augmented => Specifier::Diminished,
            Specifier::Diminished => Specifier::Augmented,
            Specifier::DoubleAugmented => Specifier::DoubleDiminished,
            Specifier::DoubleDiminished => Specifier::DoubleAugmented,
            Specifier::TripleAugmented => Specifier::TripleDiminished,
            Specifier::TripleDiminished => Specifier::TripleAugmented,
            Specifier::QuadrupleAugmented => Specifier::QuadrupleDiminished,
            Specifier::QuadrupleDiminished => Specifier::QuadrupleAugmented,
        }
    }

    /// How many semitones above perfect the specifier lies. Major and minor
    /// cannot be compared to perfect and are an error, as in music21.
    pub fn semitones_above_perfect(&self) -> Result<IntegerType> {
        match self {
            Specifier::Perfect => Ok(0),
            Specifier::Augmented => Ok(1),
            Specifier::DoubleAugmented => Ok(2),
            Specifier::TripleAugmented => Ok(3),
            Specifier::QuadrupleAugmented => Ok(4),
            Specifier::Diminished => Ok(-1),
            Specifier::DoubleDiminished => Ok(-2),
            Specifier::TripleDiminished => Ok(-3),
            Specifier::QuadrupleDiminished => Ok(-4),
            _ => Err(Error::Interval(format!(
                "<Specifier.{}> cannot be compared to Perfect",
                self.python_name()
            ))),
        }
    }

    /// How many semitones above major the specifier lies. Perfect cannot be
    /// compared to major and is an error, as in music21.
    pub fn semitones_above_major(&self) -> Result<IntegerType> {
        match self {
            Specifier::Major => Ok(0),
            Specifier::Minor => Ok(-1),
            Specifier::Augmented => Ok(1),
            Specifier::DoubleAugmented => Ok(2),
            Specifier::TripleAugmented => Ok(3),
            Specifier::QuadrupleAugmented => Ok(4),
            Specifier::Diminished => Ok(-2),
            Specifier::DoubleDiminished => Ok(-3),
            Specifier::TripleDiminished => Ok(-4),
            Specifier::QuadrupleDiminished => Ok(-5),
            _ => Err(Error::Interval(format!(
                "<Specifier.{}> cannot be compared to Major",
                self.python_name()
            ))),
        }
    }

    /// music21's enum member name, `PERFECT`, `DBLAUG`, `QUADDIM`.
    pub fn python_name(self) -> &'static str {
        match self {
            Specifier::Perfect => "PERFECT",
            Specifier::Major => "MAJOR",
            Specifier::Minor => "MINOR",
            Specifier::Augmented => "AUGMENTED",
            Specifier::Diminished => "DIMINISHED",
            Specifier::DoubleAugmented => "DBLAUG",
            Specifier::DoubleDiminished => "DBLDIM",
            Specifier::TripleAugmented => "TRPAUG",
            Specifier::TripleDiminished => "TRPDIM",
            Specifier::QuadrupleAugmented => "QUADAUG",
            Specifier::QuadrupleDiminished => "QUADDIM",
        }
    }
}

impl fmt::Display for Specifier {
    /// The prefix, as `str(Specifier.PERFECT)` is `P` in music21.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.prefix())
    }
}

impl FromStr for Specifier {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        Self::from_name(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specifier_nice_name() {
        assert_eq!(Specifier::Perfect.nice_name(), "Perfect");
        assert_eq!(Specifier::Major.nice_name(), "Major");
        assert_eq!(Specifier::Minor.nice_name(), "Minor");
        assert_eq!(Specifier::Augmented.nice_name(), "Augmented");
        assert_eq!(Specifier::Diminished.nice_name(), "Diminished");
        assert_eq!(Specifier::DoubleAugmented.nice_name(), "Doubly-Augmented");
        assert_eq!(Specifier::DoubleDiminished.nice_name(), "Doubly-Diminished");
        assert_eq!(Specifier::TripleAugmented.nice_name(), "Triply-Augmented");
        assert_eq!(Specifier::TripleDiminished.nice_name(), "Triply-Diminished");
        assert_eq!(
            Specifier::QuadrupleAugmented.nice_name(),
            "Quadruply-Augmented"
        );
        assert_eq!(
            Specifier::QuadrupleDiminished.nice_name(),
            "Quadruply-Diminished"
        );
    }

    #[test]
    fn test_specifier_semitones_above_perfect() {
        assert_eq!(Specifier::Perfect.semitones_above_perfect().unwrap(), 0);
        assert_eq!(Specifier::Augmented.semitones_above_perfect().unwrap(), 1);
        assert_eq!(
            Specifier::DoubleDiminished
                .semitones_above_perfect()
                .unwrap(),
            -2
        );
        assert!(Specifier::Major.semitones_above_perfect().is_err());
    }

    #[test]
    fn test_specifier_semitones_above_major() {
        assert_eq!(Specifier::Major.semitones_above_major().unwrap(), 0);
        assert_eq!(Specifier::Minor.semitones_above_major().unwrap(), -1);
        assert_eq!(Specifier::Diminished.semitones_above_major().unwrap(), -2);
        assert!(Specifier::Perfect.semitones_above_major().is_err());
    }

    #[test]
    fn names_numbers_and_prefixes_round_trip() {
        for specifier in Specifier::ALL {
            assert_eq!(Specifier::from_value(specifier.value()).unwrap(), specifier);
            assert_eq!(Specifier::from_name(specifier.prefix()).unwrap(), specifier);
            assert_eq!(
                Specifier::from_name(&specifier.nice_name()).unwrap(),
                specifier
            );
            assert_eq!(specifier.to_string(), specifier.prefix());
        }
        assert_eq!(Specifier::from_name("perfect").unwrap(), Specifier::Perfect);
        assert_eq!(
            Specifier::from_name("dd").unwrap(),
            Specifier::DoubleDiminished
        );
        assert_eq!("M".parse::<Specifier>().unwrap(), Specifier::Major);
        assert!(Specifier::from_name("xyz").is_err());
        assert!(Specifier::from_value(12).is_err());
    }
}
