//! The chromatic half of an interval: a signed semitone count, music21's
//! `interval.ChromaticInterval`.

use std::fmt;

use crate::{
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    pitch::Pitch,
};

use super::{diatonicinterval::DiatonicInterval, direction::Direction};

/// How far from a whole number a semitone count may drift before it stops
/// counting as that whole number, standing in for music21's
/// `if semitones == int(semitones)`.
const WHOLE_SEMITONE_TOLERANCE: FloatType = 1e-9;

/// A signed number of semitones, fractional for microtonal intervals.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct ChromaticInterval {
    pub(crate) semitones: FloatType,
}

impl ChromaticInterval {
    /// A chromatic interval of `semitones`, negative for descending and
    /// fractional for a microtonal one: a quarter tone is `0.5`.
    ///
    /// A count that is not finite spells no interval. It used to spell a
    /// perfect unison: what reads an interval off a count casts it first, and
    /// a cast answers nought for anything it cannot represent.
    pub fn new(semitones: FloatType) -> Result<Self> {
        if !semitones.is_finite() {
            return Err(Error::Interval(format!(
                "a semitone count must be finite, got {semitones}"
            )));
        }
        let rounded = semitones.round();
        let semitones = if (semitones - rounded).abs() < WHOLE_SEMITONE_TOLERANCE {
            rounded
        } else {
            semitones
        };
        Ok(Self { semitones })
    }

    /// A chromatic interval of a whole number of semitones.
    pub fn from_int(semitones: IntegerType) -> Self {
        Self {
            semitones: FloatType::from(semitones),
        }
    }

    /// The signed semitone count.
    pub fn semitones(&self) -> FloatType {
        self.semitones
    }

    /// The signed semitone count: music21's `directed`.
    pub fn directed(&self) -> FloatType {
        self.semitones
    }

    /// The semitone count without its sign.
    pub fn undirected(&self) -> FloatType {
        self.semitones.abs()
    }

    /// The semitone count rounded to a whole number, which is how music21
    /// reads a microtonal interval wherever it needs a diatonic spelling or a
    /// pitch class.
    pub fn whole_semitones(&self) -> IntegerType {
        self.semitones.round() as IntegerType
    }

    /// Ascending, descending, or oblique for no semitones at all.
    pub fn direction(&self) -> Direction {
        if self.semitones > 0.0 {
            Direction::Ascending
        } else if self.semitones < 0.0 {
            Direction::Descending
        } else {
            Direction::Oblique
        }
    }

    /// The size in cents, signed, so a quarter tone is `50.0`.
    pub fn cents(&self) -> FloatType {
        (self.semitones * 100.0 * 100_000.0).round() / 100_000.0
    }

    /// The semitones reduced to a pitch class, `0` to `11`, so a descending
    /// major third is `8`.
    pub fn mod12(&self) -> IntegerType {
        self.whole_semitones().rem_euclid(12)
    }

    /// The semitones within one octave, keeping the sign.
    pub fn simple_directed(&self) -> IntegerType {
        if self.direction() == Direction::Descending {
            -self.simple_undirected()
        } else {
            self.simple_undirected()
        }
    }

    /// The semitones within one octave, without the sign.
    pub fn simple_undirected(&self) -> IntegerType {
        self.whole_semitones().abs() % 12
    }

    /// The interval class, `0` to `6`: the smaller of the pitch-class
    /// distance and its complement.
    pub fn interval_class(&self) -> IntegerType {
        let mod12 = self.mod12();
        if mod12 > 6 { 12 - mod12 } else { mod12 }
    }

    /// Whether the interval is one semitone either way.
    pub fn is_chromatic_step(&self) -> bool {
        self.undirected() == 1.0
    }

    /// The same as [`Self::is_chromatic_step`], as music21 defines `isStep`
    /// on a chromatic interval.
    pub fn is_step(&self) -> bool {
        self.is_chromatic_step()
    }

    /// The same interval in the other direction.
    pub fn reverse(&self) -> Self {
        Self {
            semitones: -self.semitones,
        }
    }

    /// The simplest diatonic spelling of the semitone count, as music21's
    /// `getDiatonic` reads it: six semitones are a diminished fifth, one a
    /// minor second, and a microtonal count is rounded to the nearest of
    /// those.
    pub fn get_diatonic(&self) -> DiatonicInterval {
        let (specifier, generic) = super::convert_semitone_to_specifier_generic(self.semitones);
        let generic = super::GenericInterval::from_int(generic)
            .unwrap_or_else(|_| super::GenericInterval::from_int(1).expect("P1 is valid"));
        DiatonicInterval::new(specifier, &generic)
    }

    /// Moves a pitch by the semitone count, respelling it from its new pitch
    /// space the way music21's chromatic `transposePitch` does. A pitch
    /// without an octave keeps having none.
    pub fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        let mut p_out = pitch.clone();
        p_out.set_ps(pitch.ps() + self.semitones);
        if pitch.octave().is_none() {
            p_out.octave_setter(None);
        }
        Ok(p_out)
    }
}

impl fmt::Display for ChromaticInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.semitones == self.semitones.round() {
            write!(f, "{}", self.whole_semitones())
        } else {
            write!(f, "{}", self.semitones)
        }
    }
}

#[cfg(test)]
mod tests {
    /// A count that is not a number is not a distance. Left to the cast that
    /// reads an interval off it, every one of these came back a unison.
    #[test]
    fn a_semitone_count_that_is_not_finite_is_refused() {
        use super::ChromaticInterval;
        use crate::interval::notes_to_chromatic;
        use crate::pitch::Pitch;

        for count in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(ChromaticInterval::new(count).is_err());
        }
        assert_eq!(ChromaticInterval::new(7.0).unwrap().semitones(), 7.0);

        let c4 = Pitch::from_name("C4").unwrap();
        let g4 = Pitch::from_name("G4").unwrap();
        assert_eq!(notes_to_chromatic(&c4, &g4).unwrap().semitones(), 7.0);
    }

    use super::*;

    #[test]
    fn a_chromatic_interval_names_its_diatonic_reading() {
        assert_eq!(ChromaticInterval::from_int(4).get_diatonic().name(), "M3");
        assert_eq!(ChromaticInterval::from_int(6).get_diatonic().name(), "d5");
        assert_eq!(
            ChromaticInterval::from_int(-7)
                .get_diatonic()
                .directed_name(),
            "P-5"
        );
    }

    fn pitch(name: &str) -> Pitch {
        Pitch::from_name(name).expect("valid pitch")
    }

    #[test]
    fn chromatic_get_diatonic_roundtrip() {
        let chromatic = ChromaticInterval::new(6.0).unwrap();
        let diatonic = chromatic.get_diatonic();
        let roundtrip = diatonic.get_chromatic().unwrap();
        assert_eq!(roundtrip.semitones, 6.0);
    }

    #[test]
    fn chromatic_transpose_pitch() {
        let c4 = pitch("C4");
        let out = ChromaticInterval::new(7.0)
            .unwrap()
            .transpose_pitch(&c4)
            .unwrap();
        assert_eq!(out.name_with_octave(), "G4");
    }

    #[test]
    fn chromatic_reverse() {
        let reversed = ChromaticInterval::new(-4.0).unwrap().reverse();
        assert_eq!(reversed.semitones, 4.0);
    }

    #[test]
    fn chromatic_measures_match_music21() {
        let descending_third = ChromaticInterval::new(-4.0).unwrap();
        assert_eq!(descending_third.directed(), -4.0);
        assert_eq!(descending_third.undirected(), 4.0);
        assert_eq!(descending_third.direction(), Direction::Descending);
        assert_eq!(descending_third.mod12(), 8);
        assert_eq!(descending_third.simple_directed(), -4);
        assert_eq!(descending_third.simple_undirected(), 4);
        assert_eq!(descending_third.interval_class(), 4);
        assert_eq!(descending_third.cents(), -400.0);
        assert!(ChromaticInterval::new(1.0).unwrap().is_chromatic_step());
        assert!(!ChromaticInterval::new(13.0).unwrap().is_step());
        assert_eq!(
            ChromaticInterval::new(0.0).unwrap().direction(),
            Direction::Oblique
        );
        assert_eq!(ChromaticInterval::new(14.0).unwrap().simple_undirected(), 2);
        assert_eq!(ChromaticInterval::new(-4.0).unwrap().to_string(), "-4");
    }

    #[test]
    fn chromatic_keeps_a_fractional_semitone_count() {
        let quarter_tone = ChromaticInterval::new(0.5).unwrap();
        assert_eq!(quarter_tone.semitones(), 0.5);
        assert_eq!(quarter_tone.cents(), 50.0);
        assert_eq!(quarter_tone.direction(), Direction::Ascending);
        assert_eq!(quarter_tone.get_diatonic().name(), "P1");
        assert_eq!(ChromaticInterval::new(0.1).unwrap().cents(), 10.0);
        assert_eq!(
            ChromaticInterval::new(0.4).unwrap().get_diatonic().name(),
            "P1"
        );
        assert_eq!(
            ChromaticInterval::new(0.6).unwrap().get_diatonic().name(),
            "m2"
        );
        assert_eq!(
            ChromaticInterval::new(-0.5).unwrap().direction(),
            Direction::Descending
        );
        assert_eq!(quarter_tone.to_string(), "0.5");
    }

    #[test]
    fn chromatic_rounds_a_fractional_count_where_music21_rounds() {
        let three_quarters = ChromaticInterval::new(2.75).unwrap();
        assert_eq!(three_quarters.whole_semitones(), 3);
        assert_eq!(three_quarters.mod12(), 3);
        assert_eq!(three_quarters.simple_undirected(), 3);
        assert_eq!(three_quarters.interval_class(), 3);
        assert!(!three_quarters.is_chromatic_step());
    }

    #[test]
    fn chromatic_transposes_a_pitch_by_a_quarter_tone() {
        let out = ChromaticInterval::new(0.5)
            .unwrap()
            .transpose_pitch(&pitch("C4"))
            .unwrap();
        assert_eq!(out.ps(), 60.5);
    }
}
