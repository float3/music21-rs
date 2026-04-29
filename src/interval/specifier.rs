use crate::{
    defaults::IntegerType,
    error::{Error, Result},
};

#[derive(Clone, Debug)]
pub(crate) enum Specifier {
    Perfect = 1,
    Major = 2,
    Minor = 3,
    Augmented = 4,
    Diminished = 5,
    DoubleAugmented = 6,
    DoubleDiminished = 7,
    TripleAugmented = 8,
    TripleDiminished = 9,
    QuadrupleAugmented = 10,
    QuadrupleDiminished = 11,
}

impl Specifier {
    /// Returns a human-friendly name for the specifier.
    pub(crate) fn nice_name(&self) -> String {
        match self {
            Specifier::Perfect => "Perfect".to_string(),
            Specifier::Major => "Major".to_string(),
            Specifier::Minor => "Minor".to_string(),
            Specifier::Augmented => "Augmented".to_string(),
            Specifier::Diminished => "Diminished".to_string(),
            Specifier::DoubleAugmented => "Double Augmented".to_string(),
            Specifier::DoubleDiminished => "Double Diminished".to_string(),
            Specifier::TripleAugmented => "Triple Augmented".to_string(),
            Specifier::TripleDiminished => "Triple Diminished".to_string(),
            Specifier::QuadrupleAugmented => "Quadruple Augmented".to_string(),
            Specifier::QuadrupleDiminished => "Quadruple Diminished".to_string(),
        }
    }

    pub(crate) fn parse(remain: String) -> Self {
        match remain.as_str() {
            "Perfect" | "p" | "P" => Specifier::Perfect,
            "Major" | "M" => Specifier::Major,
            "Minor" | "m" => Specifier::Minor,
            "Augmented" | "a" => Specifier::Augmented,
            "Diminished" | "d" => Specifier::Diminished,
            "Double Augmented" | "aa" => Specifier::DoubleAugmented,
            "Double Diminished" | "dd" => Specifier::DoubleDiminished,
            "Triple Augmented" | "aaa" => Specifier::TripleAugmented,
            "Triple Diminished" | "ddd" => Specifier::TripleDiminished,
            "Quadruple Augmented" | "aaaa" => Specifier::QuadrupleAugmented,
            "Quadruple Diminished" | "dddd" => Specifier::QuadrupleDiminished,
            val => panic!("Invalid specifier: {val}"),
        }
    }

    pub(crate) fn inversion(&self) -> Self {
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

    pub(crate) fn semitones_above_perfect(&self) -> Result<IntegerType> {
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
                "{self:?} cannot be compared to Perfect"
            ))),
        }
    }

    pub(crate) fn semitones_above_major(&self) -> Result<IntegerType> {
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
                "{self:?} cannot be compared to Major"
            ))),
        }
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
        assert_eq!(Specifier::DoubleAugmented.nice_name(), "Double Augmented");
        assert_eq!(Specifier::DoubleDiminished.nice_name(), "Double Diminished");
        assert_eq!(Specifier::TripleAugmented.nice_name(), "Triple Augmented");
        assert_eq!(Specifier::TripleDiminished.nice_name(), "Triple Diminished");
        assert_eq!(
            Specifier::QuadrupleAugmented.nice_name(),
            "Quadruple Augmented"
        );
        assert_eq!(
            Specifier::QuadrupleDiminished.nice_name(),
            "Quadruple Diminished"
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
}
