use crate::defaults::UnsignedIntegerType;

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

    pub(crate) fn semitones_above_perfect(&self) -> UnsignedIntegerType {
        todo!()
    }

    pub(crate) fn semitones_above_major(&self) -> UnsignedIntegerType {
        todo!()
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
}
