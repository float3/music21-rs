//! Equal divisions of any interval, not only of the octave.
//!
//! [`crate::tuningsystem::TuningSystem::EqualTemperament`] divides an octave,
//! and is octave-repeating all the way down — its degree count *is* a count
//! per octave, and its ratios double from one octave to the next. Plenty of
//! tunings do not repeat at the octave at all: Bohlen-Pierce divides a twelfth
//! into thirteen and never sounds an octave, and Carlos's Alpha, Beta and
//! Gamma repeat at nothing in particular. This is the type for those.
//!
//! The period is kept as cents so that any interval can be one, and the ratio
//! it came from is kept beside it so the division can still name itself the
//! way the literature does — `13edt` rather than `13 equal steps of 1901.955
//! cents`.
//!
//! ```
//! use music21_rs::tuningsystem::EqualDivision;
//!
//! let bohlen_pierce = EqualDivision::tritave(13)?;
//! assert_eq!(bohlen_pierce.to_string(), "13edt");
//! assert!((bohlen_pierce.step_cents() - 146.304).abs() < 1e-3);
//! // Its ninth degree is nowhere near an octave, which is the point of it.
//! assert!((bohlen_pierce.cents_at(8) - 1170.4).abs() < 0.1);
//! # Ok::<(), music21_rs::Error>(())
//! ```

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};
use crate::tuningsystem::monzo::{Monzo, PRIMES, Val};
use crate::tuningsystem::mos::{MosScale, OCTAVE_CENTS};

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// The tritave, `3/1`, in cents — Bohlen-Pierce's period.
pub const TRITAVE_CENTS: FloatType = 1_901.955_000_865_388_7;

/// An equal division of an arbitrary interval.
///
/// `12edo` is twelve equal divisions of the octave, `13edt` thirteen of the
/// tritave. Nothing here assumes the period is an octave, so a degree past the
/// period simply carries on into the next one — which for Bohlen-Pierce means
/// the pitches never line up with an octave at all.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct EqualDivision {
    divisions: UnsignedIntegerType,
    period_cents: FloatType,
    period_ratio: Option<(IntegerType, IntegerType)>,
}

impl EqualDivision {
    /// Divides a period given in cents.
    ///
    /// Errors on a period that is not a positive real number, or on no
    /// divisions at all.
    pub fn new(divisions: UnsignedIntegerType, period_cents: FloatType) -> Result<Self> {
        if divisions == 0 {
            return Err(Error::TuningSystem(
                "a period cannot be divided into no steps".to_owned(),
            ));
        }
        if !period_cents.is_finite() || period_cents <= 0.0 {
            return Err(Error::TuningSystem(format!(
                "{period_cents} cents is not an interval that can be divided"
            )));
        }
        Ok(Self {
            divisions,
            period_cents,
            period_ratio: None,
        })
    }

    /// Divides the interval `numerator/denominator`.
    ///
    /// The ratio is remembered, so the division names itself after it.
    pub fn of_ratio(
        divisions: UnsignedIntegerType,
        numerator: IntegerType,
        denominator: IntegerType,
    ) -> Result<Self> {
        if numerator <= 0 || denominator <= 0 || numerator == denominator {
            return Err(Error::TuningSystem(format!(
                "{numerator}/{denominator} is not an interval that can be divided"
            )));
        }
        let ratio = FloatType::from(numerator) / FloatType::from(denominator);
        let mut division = Self::new(divisions, OCTAVE_CENTS * ratio.log2())?;
        division.period_ratio = Some((numerator, denominator));
        Ok(division)
    }

    /// Divides the octave — an EDO, and what everyone means by default.
    pub fn octave(divisions: UnsignedIntegerType) -> Result<Self> {
        Self::of_ratio(divisions, 2, 1)
    }

    /// Divides the tritave, `3/1` — an EDT, which is Bohlen-Pierce's period.
    pub fn tritave(divisions: UnsignedIntegerType) -> Result<Self> {
        Self::of_ratio(divisions, 3, 1)
    }

    /// How many steps the period is divided into.
    #[must_use]
    pub fn divisions(&self) -> UnsignedIntegerType {
        self.divisions
    }

    /// The period, in cents.
    #[must_use]
    pub fn period_cents(&self) -> FloatType {
        self.period_cents
    }

    /// The ratio the period was given as, where it was given as one.
    #[must_use]
    pub fn period_ratio(&self) -> Option<(IntegerType, IntegerType)> {
        self.period_ratio
    }

    /// Whether the period is an octave, which is what makes this an ordinary EDO.
    #[must_use]
    pub fn repeats_at_the_octave(&self) -> bool {
        (self.period_cents - OCTAVE_CENTS).abs() < 1e-9
    }

    /// One step, in cents.
    #[must_use]
    pub fn step_cents(&self) -> FloatType {
        self.period_cents / FloatType::from(self.divisions)
    }

    /// How far above the tonic `degree` steps sit, in cents.
    ///
    /// A degree past the period carries on into the next one, and a negative
    /// degree goes below the tonic.
    #[must_use]
    pub fn cents_at(&self, degree: IntegerType) -> FloatType {
        FloatType::from(degree) * self.step_cents()
    }

    /// The frequency ratio of `degree` steps above the tonic.
    #[must_use]
    pub fn ratio_at(&self, degree: IntegerType) -> FloatType {
        (2.0 as FloatType).powf(self.cents_at(degree) / OCTAVE_CENTS)
    }

    /// Every degree of one period, in cents, from nought up to but not including the period.
    #[must_use]
    pub fn degrees(&self) -> Vec<FloatType> {
        (0..self.divisions)
            .map(|degree| FloatType::from(degree) * self.step_cents())
            .collect()
    }

    /// The degree nearest `cents` above the tonic.
    #[must_use]
    pub fn nearest_degree(&self, cents: FloatType) -> IntegerType {
        (cents / self.step_cents()).round() as IntegerType
    }

    /// How far the nearest degree sits from `cents`; positive is sharp.
    #[must_use]
    pub fn error_at_cents(&self, cents: FloatType) -> FloatType {
        self.cents_at(self.nearest_degree(cents)) - cents
    }

    /// The degree nearest `interval`, and how far off it sounds in cents.
    ///
    /// This is how well the division does a just interval, which for an EDO is
    /// the whole question of whether it is worth using.
    #[must_use]
    pub fn approximation_of(&self, interval: &Monzo) -> (IntegerType, FloatType) {
        let cents = interval.cents();
        (self.nearest_degree(cents), self.error_at_cents(cents))
    }

    /// The patent val of this division, read up to `limit`.
    ///
    /// Each prime is mapped to the nearest whole number of *steps*, which for
    /// an octave division is the familiar patent val. For a period that is not
    /// an octave this is the generalized reading, and the octave itself then
    /// gets a mapping like any other prime rather than being the period.
    /// Errors for a limit that is not a prime the module carries.
    pub fn patent_val(&self, limit: IntegerType) -> Result<Val> {
        let width = PRIMES
            .iter()
            .position(|&prime| prime == limit)
            .map(|index| index + 1)
            .ok_or_else(|| {
                Error::TuningSystem(format!("{limit} is not a prime this module carries"))
            })?;
        Ok(Val::new(
            PRIMES[..width]
                .iter()
                .map(|&prime| self.nearest_degree(OCTAVE_CENTS * FloatType::from(prime).log2()))
                .collect::<Vec<_>>(),
        ))
    }

    /// The scale of `notes` notes made by stacking `steps` of this division.
    ///
    /// This is the wiki's `3\22` — three steps of twenty-two — handed to the
    /// moment-of-symmetry machinery.
    pub fn mos(&self, steps: UnsignedIntegerType, notes: UnsignedIntegerType) -> Result<MosScale> {
        MosScale::from_equal_division(steps, self.divisions, self.period_cents, notes)
    }
}

impl Display for EqualDivision {
    /// Names the division the way the literature does: `12edo`, `13edt`, `9ed3/2`.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.divisions)?;
        match self.period_ratio {
            Some((2, 1)) => write!(f, "edo"),
            Some((3, 1)) => write!(f, "edt"),
            Some((numerator, 1)) => write!(f, "ed{numerator}"),
            Some((numerator, denominator)) => write!(f, "ed{numerator}/{denominator}"),
            None => write!(f, "ed{:.4}c", self.period_cents),
        }
    }
}

impl FromStr for EqualDivision {
    type Err = Error;

    /// Reads `12edo`, `13edt`, `13ed3` and `9ed3/2`.
    fn from_str(text: &str) -> Result<Self> {
        let text = text.trim();
        let malformed =
            || Error::TuningSystem(format!("{text} is not an equal division; one reads 13edt"));
        let (count, period) = text.split_once("ed").ok_or_else(malformed)?;
        let divisions = count.parse().map_err(|_| malformed())?;
        match period {
            "o" => Self::octave(divisions),
            "t" => Self::tritave(divisions),
            _ => {
                let (numerator, denominator) = match period.split_once('/') {
                    Some((numerator, denominator)) => (numerator, denominator),
                    None => (period, "1"),
                };
                Self::of_ratio(
                    divisions,
                    numerator.parse().map_err(|_| malformed())?,
                    denominator.parse().map_err(|_| malformed())?,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_octave_division_is_the_equal_temperament_everyone_means() {
        let twelve = EqualDivision::octave(12).expect("an octave");
        assert_eq!(twelve.to_string(), "12edo");
        assert!(twelve.repeats_at_the_octave());
        assert!((twelve.step_cents() - 100.0).abs() < 1e-9);
        assert!((twelve.cents_at(7) - 700.0).abs() < 1e-9);
        assert!((twelve.ratio_at(12) - 2.0).abs() < 1e-9);
        assert_eq!(twelve.degrees().len(), 12);
        // The patent val of an octave division is the familiar one.
        assert_eq!(
            twelve.patent_val(5).expect("the 5-limit"),
            Val::new([12, 19, 28])
        );
    }

    #[test]
    fn bohlen_pierce_divides_a_twelfth_and_never_reaches_an_octave() {
        let bohlen_pierce = EqualDivision::tritave(13).expect("a tritave");
        assert_eq!(bohlen_pierce.to_string(), "13edt");
        assert!(!bohlen_pierce.repeats_at_the_octave());
        assert!((bohlen_pierce.period_cents() - TRITAVE_CENTS).abs() < 1e-6);
        assert!((bohlen_pierce.step_cents() - 146.3042).abs() < 1e-4);
        // No degree lands on an octave: the nearest is eight steps, and it is
        // nearly thirty cents flat.
        let octave = Monzo::from_ratio(2, 1).expect("the octave");
        let (degree, error) = bohlen_pierce.approximation_of(&octave);
        assert_eq!(degree, 8);
        assert!((error + 29.6).abs() < 0.1, "{error}");
        // Its own period, though, is exact.
        let tritave = Monzo::from_ratio(3, 1).expect("the tritave");
        assert!(bohlen_pierce.approximation_of(&tritave).1.abs() < 1e-6);
    }

    /// Carlos Alpha, Beta and Gamma repeat at nothing in particular, which is
    /// exactly why they cannot be a `TuningSystem` variant.
    #[test]
    fn the_carlos_scales_have_periods_that_are_not_intervals_at_all() {
        for (name, period, divisions, step) in [
            ("alpha", 1404.0, 9, 156.0),
            ("beta", 1403.6, 11, 127.6),
            ("gamma", 1228.465, 20, 61.42),
        ] {
            let division = EqualDivision::new(divisions, period).expect("a period");
            assert!(!division.repeats_at_the_octave(), "{name}");
            assert!((division.step_cents() - step).abs() < 0.01, "{name}");
            assert_eq!(division.period_ratio(), None, "{name}");
            // With no ratio to name it after, it says its period in cents.
            assert!(division.to_string().ends_with('c'), "{name}");
        }
    }

    #[test]
    fn how_well_a_division_does_the_just_intervals_is_the_whole_question() {
        let twelve = EqualDivision::octave(12).expect("an octave");
        let fifth = Monzo::from_ratio(3, 2).expect("the fifth");
        let third = Monzo::from_ratio(5, 4).expect("the third");
        assert_eq!(twelve.approximation_of(&fifth).0, 7);
        assert!((twelve.approximation_of(&fifth).1 + 1.955).abs() < 1e-3);
        // The famously sharp third.
        assert!((twelve.approximation_of(&third).1 - 13.686).abs() < 1e-3);

        // 31edo was built to do the third better, and does.
        let thirty_one = EqualDivision::octave(31).expect("an octave");
        assert!(thirty_one.approximation_of(&third).1.abs() < 1.0);
        // 53edo does nearly everything in the 5-limit.
        let fifty_three = EqualDivision::octave(53).expect("an octave");
        assert!(fifty_three.approximation_of(&fifth).1.abs() < 0.1);
        assert!(fifty_three.approximation_of(&third).1.abs() < 1.5);
    }

    #[test]
    fn a_division_hands_its_steps_to_the_moment_of_symmetry_machinery() {
        let twenty_two = EqualDivision::octave(22).expect("an octave");
        assert_eq!(
            twenty_two
                .mos(3, 7)
                .expect("a generator")
                .pattern()
                .expect("a moment")
                .to_string(),
            "1L 6s"
        );
        let bohlen_pierce = EqualDivision::tritave(13).expect("a tritave");
        assert_eq!(
            bohlen_pierce
                .mos(3, 9)
                .expect("a generator")
                .pattern()
                .expect("a moment")
                .to_string(),
            "4L 5s"
        );
    }

    #[test]
    fn a_division_writes_and_reads_the_way_it_is_named() {
        for name in ["12edo", "13edt", "9ed3/2", "5ed5"] {
            let division: EqualDivision = name.parse().expect("a division");
            assert_eq!(division.to_string(), name);
        }
        // `13ed3` is the tritave written the long way, and names itself back
        // the short way.
        assert_eq!(
            "13ed3"
                .parse::<EqualDivision>()
                .expect("a division")
                .to_string(),
            "13edt"
        );
        assert!("edo".parse::<EqualDivision>().is_err());
        assert!("12".parse::<EqualDivision>().is_err());
        assert!("12edx".parse::<EqualDivision>().is_err());
        assert!("0edo".parse::<EqualDivision>().is_err());
    }

    #[test]
    fn a_period_that_is_not_an_interval_is_refused() {
        assert!(EqualDivision::new(0, OCTAVE_CENTS).is_err());
        assert!(EqualDivision::new(12, 0.0).is_err());
        assert!(EqualDivision::new(12, -1200.0).is_err());
        assert!(EqualDivision::of_ratio(12, 1, 1).is_err());
        assert!(EqualDivision::of_ratio(12, -2, 1).is_err());
        assert!(
            EqualDivision::octave(12)
                .expect("an octave")
                .patent_val(9)
                .is_err()
        );
    }
}
