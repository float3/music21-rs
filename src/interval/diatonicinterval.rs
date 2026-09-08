//! A generic interval with a quality: music21's `interval.DiatonicInterval`.

use std::fmt;

use crate::{
    defaults::{FloatType, IntegerType, UnsignedIntegerType},
    error::{Error, Result},
    pitch::Pitch,
};

use super::{
    GenericInterval, chromaticinterval::ChromaticInterval, direction::Direction,
    specifier::Specifier,
};

/// A quality and a generic interval together: `M3`, `-P5`, `dd7`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct DiatonicInterval {
    pub(crate) generic: GenericInterval,
    pub(crate) specifier: Specifier,
}

impl PartialEq for DiatonicInterval {
    /// Two diatonic intervals are equal when their generic value, specifier
    /// and direction agree, as in music21.
    fn eq(&self, other: &Self) -> bool {
        self.generic.value() == other.generic.value()
            && self.specifier == other.specifier
            && self.direction() == other.direction()
    }
}

impl Eq for DiatonicInterval {}

impl DiatonicInterval {
    /// Pairs a quality with a generic interval without checking that the
    /// pair exists; see [`Self::try_new`] for the checked form.
    pub fn new(specifier: Specifier, generic: &GenericInterval) -> Self {
        Self {
            generic: generic.clone(),
            specifier,
        }
    }

    /// Pairs a quality with a generic interval, rejecting the combinations
    /// music21 rejects: major and minor on a perfectable interval, perfect
    /// on the others, and a descending perfect unison.
    pub fn try_new(specifier: Specifier, generic: GenericInterval) -> Result<Self> {
        let perfectable = generic.is_perfectable();
        let mismatched = match specifier {
            Specifier::Major | Specifier::Minor => perfectable,
            Specifier::Perfect => !perfectable,
            _ => false,
        };
        if mismatched {
            return Err(Error::Interval(format!(
                "Cannot create a '{} {}'",
                specifier.nice_name(),
                generic.nice_name()
            )));
        }
        if generic.value() == -1 && specifier == Specifier::Perfect {
            return Err(Error::Interval(
                "There is no such thing as a descending Perfect Unison".to_owned(),
            ));
        }
        Ok(Self { generic, specifier })
    }

    /// Parses a diatonic interval from a name such as `M3`, `-P5` or
    /// `Major Third`.
    pub fn from_name(name: &str) -> Result<Self> {
        let (diatonic, _, _) = super::parse_interval_name(name.to_string())?;
        Self::try_new(diatonic.specifier, diatonic.generic)
    }

    /// The quality.
    pub fn specifier(&self) -> Specifier {
        self.specifier
    }

    /// The generic interval.
    pub fn generic(&self) -> &GenericInterval {
        &self.generic
    }

    /// The short name without direction, `M3`, `P5`, `dd7`.
    pub fn name(&self) -> String {
        format!("{}{}", self.specifier.prefix(), self.generic.undirected())
    }

    /// The spelled-out name, `Major Third`.
    pub fn nice_name(&self) -> String {
        format!(
            "{} {}",
            self.specifier.nice_name(),
            self.generic.nice_name()
        )
    }

    /// The short name with the direction, `M-3` for a descending third,
    /// which is where music21 puts the hyphen.
    pub fn directed_name(&self) -> String {
        format!("{}{}", self.specifier.prefix(), self.generic.directed())
    }

    /// The spelled-out name with the direction, `Descending Major Third`.
    pub fn directed_nice_name(&self) -> String {
        format!("{}{}", self.direction_prefix(), self.nice_name())
    }

    /// The short name of the simple interval, so `M10` is `M3`.
    pub fn simple_name(&self) -> String {
        format!(
            "{}{}",
            self.specifier.prefix(),
            self.generic.simple_undirected()
        )
    }

    /// The short name of the semi-simple interval, so `P15` is `P8`.
    pub fn semi_simple_name(&self) -> String {
        format!(
            "{}{}",
            self.specifier.prefix(),
            self.generic.semi_simple_undirected()
        )
    }

    /// The spelled-out name of the simple interval.
    pub fn simple_nice_name(&self) -> String {
        format!(
            "{} {}",
            self.specifier.nice_name(),
            self.generic.simple_nice_name()
        )
    }

    /// The spelled-out name of the semi-simple interval.
    pub fn semi_simple_nice_name(&self) -> String {
        format!(
            "{} {}",
            self.specifier.nice_name(),
            self.generic.semi_simple_nice_name()
        )
    }

    /// The short directed name of the simple interval, `M-3` for a
    /// descending tenth.
    pub fn directed_simple_name(&self) -> String {
        format!(
            "{}{}",
            self.specifier.prefix(),
            self.generic.simple_directed()
        )
    }

    /// The short directed name of the semi-simple interval.
    pub fn directed_semi_simple_name(&self) -> String {
        format!(
            "{}{}",
            self.specifier.prefix(),
            self.generic.semi_simple_directed()
        )
    }

    /// The spelled-out directed name of the simple interval.
    pub fn directed_simple_nice_name(&self) -> String {
        format!("{}{}", self.direction_prefix(), self.simple_nice_name())
    }

    /// The spelled-out directed name of the semi-simple interval.
    pub fn directed_semi_simple_nice_name(&self) -> String {
        format!(
            "{}{}",
            self.direction_prefix(),
            self.semi_simple_nice_name()
        )
    }

    /// The quality's spelled-out name alone, `Major`.
    pub fn specific_name(&self) -> String {
        self.specifier.nice_name()
    }

    /// Ascending, descending, or oblique. A unison takes its direction from
    /// the quality: an augmented unison ascends and a diminished one
    /// descends, as music21 reads them.
    pub fn direction(&self) -> Direction {
        if self.generic.undirected() != 1 {
            return self.generic.direction();
        }
        match self.specifier {
            Specifier::Diminished
            | Specifier::DoubleDiminished
            | Specifier::TripleDiminished
            | Specifier::QuadrupleDiminished => Direction::Descending,
            Specifier::Augmented
            | Specifier::DoubleAugmented
            | Specifier::TripleAugmented
            | Specifier::QuadrupleAugmented => Direction::Ascending,
            Specifier::Perfect | Specifier::Major | Specifier::Minor => Direction::Oblique,
        }
    }

    /// The size in cents, signed, from the chromatic equivalent.
    pub fn cents(&self) -> Result<FloatType> {
        Ok(self.get_chromatic()?.cents())
    }

    /// The name of the simple interval measured upward, so a descending
    /// major third is `m6`.
    pub fn mod7(&self) -> String {
        if self.direction() == Direction::Descending {
            self.mod7_inversion()
        } else {
            self.simple_name()
        }
    }

    /// The name of the inversion of the simple interval: `M3` becomes `m6`.
    pub fn mod7_inversion(&self) -> String {
        format!(
            "{}{}",
            self.specifier.inversion().prefix(),
            self.generic.mod7_inversion()
        )
    }

    /// Whether the interval is a second in either direction.
    pub fn is_step(&self) -> bool {
        self.generic.is_step()
    }

    /// The same as [`Self::is_step`].
    pub fn is_diatonic_step(&self) -> bool {
        self.generic.is_diatonic_step()
    }

    /// Whether the interval is larger than a second.
    pub fn is_skip(&self) -> bool {
        self.generic.is_skip()
    }

    /// Whether the generic interval takes perfect qualities.
    pub fn is_perfectable(&self) -> bool {
        self.generic.is_perfectable()
    }

    /// The semitone count the quality and generic interval add up to.
    pub fn get_chromatic(&self) -> Result<ChromaticInterval> {
        let octave_offset = (self.generic.staff_distance().abs() / 7) as UnsignedIntegerType;
        let semitones_start =
            semitones_generic(self.generic.simple_undirected() as UnsignedIntegerType)?;
        let semitones_adjust = if self.generic.is_perfectable() {
            self.specifier.semitones_above_perfect()?
        } else {
            self.specifier.semitones_above_major()?
        };
        let mut semitones: IntegerType =
            ((octave_offset * 12 + semitones_start) as IntegerType) + semitones_adjust;
        if self.generic.direction() == Direction::Descending {
            semitones *= -1;
        }
        Ok(ChromaticInterval::from_int(semitones))
    }

    /// The same interval in the other direction. A unison keeps its
    /// generic value and inverts its quality instead, since an augmented
    /// unison downward is a diminished one.
    pub fn reverse(&self) -> Self {
        if self.generic.undirected() == 1 {
            Self::new(self.specifier.inversion(), &self.generic)
        } else {
            Self::new(self.specifier, &self.generic.reverse())
        }
    }

    /// Moves a pitch by the interval, spelling the result by the quality.
    pub fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        let interval =
            super::Interval::from_diatonic_and_chromatic(self.clone(), self.get_chromatic()?)?;
        interval.transpose_pitch_with_options(pitch, false, Some(4))
    }

    fn direction_prefix(&self) -> &'static str {
        match self.direction() {
            Direction::Descending => "Descending ",
            Direction::Ascending => "Ascending ",
            Direction::Oblique => "",
        }
    }
}

impl fmt::Display for DiatonicInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

fn semitones_generic(r#in: UnsignedIntegerType) -> Result<UnsignedIntegerType> {
    match r#in {
        1 => Ok(0),
        2 => Ok(2),
        3 => Ok(4),
        4 => Ok(5),
        5 => Ok(7),
        6 => Ok(9),
        7 => Ok(11),
        _ => Err(Error::Interval(format!("Invalid diatonic interval: {in}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diatonic_get_chromatic_major_third() {
        let generic = GenericInterval::from_int(3).unwrap();
        let diatonic = DiatonicInterval::new(Specifier::Major, &generic);
        assert_eq!(diatonic.get_chromatic().unwrap().semitones, 4.0);
    }

    #[test]
    fn diatonic_reverse_unison_inverts_specifier() {
        let generic = GenericInterval::from_int(1).unwrap();
        let diatonic = DiatonicInterval::new(Specifier::Augmented, &generic);
        let reversed = diatonic.reverse();
        assert_eq!(reversed.get_chromatic().unwrap().semitones, -1.0);
    }

    #[test]
    fn diatonic_names_match_music21() {
        let descending_tenth =
            DiatonicInterval::new(Specifier::Major, &GenericInterval::from_int(-10).unwrap());
        assert_eq!(descending_tenth.name(), "M10");
        assert_eq!(descending_tenth.directed_name(), "M-10");
        assert_eq!(descending_tenth.nice_name(), "Major Tenth");
        assert_eq!(
            descending_tenth.directed_nice_name(),
            "Descending Major Tenth"
        );
        assert_eq!(descending_tenth.simple_name(), "M3");
        assert_eq!(descending_tenth.directed_simple_name(), "M-3");
        assert_eq!(descending_tenth.simple_nice_name(), "Major Third");
        assert_eq!(
            descending_tenth.directed_simple_nice_name(),
            "Descending Major Third"
        );
        assert_eq!(descending_tenth.mod7(), "m6");
        assert_eq!(descending_tenth.mod7_inversion(), "m6");
        assert_eq!(descending_tenth.specific_name(), "Major");
        assert_eq!(descending_tenth.cents().unwrap(), -1600.0);
        assert_eq!(descending_tenth.to_string(), "M10");
        let fifteenth =
            DiatonicInterval::new(Specifier::Perfect, &GenericInterval::from_int(15).unwrap());
        assert_eq!(fifteenth.semi_simple_name(), "P8");
        assert_eq!(fifteenth.directed_semi_simple_name(), "P8");
        assert_eq!(fifteenth.semi_simple_nice_name(), "Perfect Octave");
        assert_eq!(
            fifteenth.directed_semi_simple_nice_name(),
            "Ascending Perfect Octave"
        );
    }

    #[test]
    fn diatonic_unison_direction_comes_from_the_quality() {
        let unison = GenericInterval::from_int(1).unwrap();
        assert_eq!(
            DiatonicInterval::new(Specifier::Augmented, &unison).direction(),
            Direction::Ascending
        );
        assert_eq!(
            DiatonicInterval::new(Specifier::Diminished, &unison).direction(),
            Direction::Descending
        );
        assert_eq!(
            DiatonicInterval::new(Specifier::Perfect, &unison).direction(),
            Direction::Oblique
        );
        assert_eq!(
            DiatonicInterval::new(Specifier::Diminished, &unison).directed_nice_name(),
            "Descending Diminished Unison"
        );
    }

    #[test]
    fn diatonic_try_new_rejects_impossible_pairs() {
        let fifth = GenericInterval::from_int(5).unwrap();
        let err = DiatonicInterval::try_new(Specifier::Major, fifth).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Interval error: Cannot create a 'Major Fifth'"
        );
        let third = GenericInterval::from_int(3).unwrap();
        assert!(DiatonicInterval::try_new(Specifier::Perfect, third).is_err());
        let down_unison = GenericInterval::from_int(-1).unwrap();
        assert!(DiatonicInterval::try_new(Specifier::Perfect, down_unison).is_err());
        assert!(
            DiatonicInterval::try_new(Specifier::Augmented, GenericInterval::from_int(-1).unwrap())
                .is_ok()
        );
        let from_name = DiatonicInterval::from_name("M-3").unwrap();
        assert_eq!(from_name.directed_name(), "M-3");
        assert_eq!(
            DiatonicInterval::from_name("Major Third").unwrap(),
            DiatonicInterval::from_name("M3").unwrap()
        );
    }
}
