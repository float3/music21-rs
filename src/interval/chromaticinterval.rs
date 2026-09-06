//! The chromatic half of an interval: a signed semitone count, music21's
//! `interval.ChromaticInterval`.

use std::fmt;

use crate::{
    defaults::{FloatType, IntegerType},
    error::Result,
    pitch::Pitch,
};

use super::{diatonicinterval::DiatonicInterval, direction::Direction};

/// A signed number of semitones.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChromaticInterval {
    pub(crate) semitones: IntegerType,
}

impl ChromaticInterval {
    /// A chromatic interval of `semitones`, negative for descending.
    pub fn new(semitones: IntegerType) -> Self {
        Self { semitones }
    }

    /// The signed semitone count.
    pub fn semitones(&self) -> IntegerType {
        self.semitones
    }

    /// The signed semitone count: music21's `directed`.
    pub fn directed(&self) -> IntegerType {
        self.semitones
    }

    /// The semitone count without its sign.
    pub fn undirected(&self) -> IntegerType {
        self.semitones.abs()
    }

    /// Ascending, descending, or oblique for no semitones at all.
    pub fn direction(&self) -> Direction {
        match self.semitones.signum() {
            1 => Direction::Ascending,
            -1 => Direction::Descending,
            _ => Direction::Oblique,
        }
    }

    /// The size in cents, signed.
    pub fn cents(&self) -> FloatType {
        FloatType::from(self.semitones) * 100.0
    }

    /// The semitones reduced to a pitch class, `0` to `11`, so a descending
    /// major third is `8`.
    pub fn mod12(&self) -> IntegerType {
        self.semitones.rem_euclid(12)
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
        self.undirected() % 12
    }

    /// The interval class, `0` to `6`: the smaller of the pitch-class
    /// distance and its complement.
    pub fn interval_class(&self) -> IntegerType {
        let mod12 = self.mod12();
        if mod12 > 6 { 12 - mod12 } else { mod12 }
    }

    /// Whether the interval is one semitone either way.
    pub fn is_chromatic_step(&self) -> bool {
        self.undirected() == 1
    }

    /// The same as [`Self::is_chromatic_step`], as music21 defines `isStep`
    /// on a chromatic interval.
    pub fn is_step(&self) -> bool {
        self.is_chromatic_step()
    }

    /// The same interval in the other direction.
    pub fn reverse(&self) -> Self {
        Self::new(-self.semitones)
    }

    /// The simplest diatonic spelling of the semitone count, as music21's
    /// `getDiatonic` reads it: six semitones are a diminished fifth, one a
    /// minor second.
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
        p_out.set_ps(pitch.ps() + self.semitones as FloatType);
        if pitch.octave().is_none() {
            p_out.octave_setter(None);
        }
        Ok(p_out)
    }
}

impl fmt::Display for ChromaticInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.semitones)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::from_name(name).expect("valid pitch")
    }

    #[test]
    fn chromatic_get_diatonic_roundtrip() {
        let chromatic = ChromaticInterval::new(6);
        let diatonic = chromatic.get_diatonic();
        let roundtrip = diatonic.get_chromatic().unwrap();
        assert_eq!(roundtrip.semitones, 6);
    }

    #[test]
    fn chromatic_transpose_pitch() {
        let c4 = pitch("C4");
        let out = ChromaticInterval::new(7).transpose_pitch(&c4).unwrap();
        assert_eq!(out.name_with_octave(), "G4");
    }

    #[test]
    fn chromatic_reverse() {
        let reversed = ChromaticInterval::new(-4).reverse();
        assert_eq!(reversed.semitones, 4);
    }

    #[test]
    fn chromatic_measures_match_music21() {
        let descending_third = ChromaticInterval::new(-4);
        assert_eq!(descending_third.directed(), -4);
        assert_eq!(descending_third.undirected(), 4);
        assert_eq!(descending_third.direction(), Direction::Descending);
        assert_eq!(descending_third.mod12(), 8);
        assert_eq!(descending_third.simple_directed(), -4);
        assert_eq!(descending_third.simple_undirected(), 4);
        assert_eq!(descending_third.interval_class(), 4);
        assert_eq!(descending_third.cents(), -400.0);
        assert!(ChromaticInterval::new(1).is_chromatic_step());
        assert!(!ChromaticInterval::new(13).is_step());
        assert_eq!(ChromaticInterval::new(0).direction(), Direction::Oblique);
        assert_eq!(ChromaticInterval::new(14).simple_undirected(), 2);
        assert_eq!(ChromaticInterval::new(-4).to_string(), "-4");
    }
}
