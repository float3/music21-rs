use crate::{
    defaults::{FloatType, IntegerType, PITCH_SPACE_SIGNIFICANT_DIGITS},
    error::{Error, Result},
};

use super::pitchclassstring::PitchClassString;
use std::fmt::{Display, Formatter};

/// Input accepted by [`PitchClass::new`] and pitch-class builders.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PitchClassSpecifier {
    /// A numeric pitch class.
    Number(f64),
    /// A string pitch class, including `A`/`T` for 10 and `B`/`E` for 11.
    String(String),
    /// An existing pitch class to clone.
    PitchClass(PitchClass),
}

impl PitchClassSpecifier {
    pub(crate) fn to_number(&self) -> Result<FloatType> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::String(value) => parse_pitch_class_string(value),
            Self::PitchClass(pitch_class) => Ok(pitch_class.number()),
        }
    }
}

impl From<IntegerType> for PitchClassSpecifier {
    fn from(value: IntegerType) -> Self {
        Self::Number(value as FloatType)
    }
}

impl From<u8> for PitchClassSpecifier {
    fn from(value: u8) -> Self {
        Self::Number(value as FloatType)
    }
}

impl From<FloatType> for PitchClassSpecifier {
    fn from(value: FloatType) -> Self {
        Self::Number(value)
    }
}

impl From<char> for PitchClassSpecifier {
    fn from(value: char) -> Self {
        Self::String(value.to_string())
    }
}

impl From<&str> for PitchClassSpecifier {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for PitchClassSpecifier {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<PitchClass> for PitchClassSpecifier {
    fn from(value: PitchClass) -> Self {
        Self::PitchClass(value)
    }
}

impl Display for PitchClassSpecifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(number) => write!(f, "{number}"),
            Self::String(value) => write!(f, "{value}"),
            Self::PitchClass(pitch_class) => write!(f, "{pitch_class}"),
        }
    }
}

/// A normalized pitch-class value.
///
/// Pitch classes wrap into the range `0 <= pc < 12`. Integer pitch classes
/// display using music21's hexadecimal-style spellings: `A` for 10 and `B` for
/// 11.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PitchClass {
    value: FloatType,
}

impl PitchClass {
    /// Creates a normalized pitch class from a number, string, or existing
    /// pitch class.
    pub fn new(specifier: impl Into<PitchClassSpecifier>) -> Result<Self> {
        match specifier.into() {
            PitchClassSpecifier::PitchClass(pitch_class) => Ok(pitch_class),
            specifier => Self::from_number(specifier.to_number()?),
        }
    }

    pub(crate) fn from_number(value: FloatType) -> Result<Self> {
        if !value.is_finite() {
            return Err(Error::PitchClass(format!(
                "pitch class must be finite, got {value}"
            )));
        }

        Ok(Self {
            value: normalize_pitch_class(value),
        })
    }

    /// Returns the normalized numeric pitch class.
    pub fn number(&self) -> FloatType {
        self.value
    }

    /// Returns the integer pitch class if this value is not microtonal.
    pub fn integer(&self) -> Option<IntegerType> {
        if self.value.fract() == 0.0 {
            Some(self.value as IntegerType)
        } else {
            None
        }
    }

    /// Returns the music21-style pitch-class string.
    pub fn string(&self) -> String {
        pitch_class_to_string(self.value)
    }
}

impl Display for PitchClass {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string())
    }
}

pub(crate) fn convert_pitch_class_to_str(pc: i32) -> String {
    // Mimic Python's modulo: always a non-negative remainder.
    let pc = pc.rem_euclid(12);
    format!("{pc:X}")
}

fn pitch_class_to_string(pc: FloatType) -> String {
    let pc = normalize_pitch_class(pc);
    if pc.fract() == 0.0 {
        return convert_pitch_class_to_str(pc as IntegerType);
    }

    trim_float(pc)
}

fn normalize_pitch_class(pc: FloatType) -> FloatType {
    let factor = (10 as FloatType).powi(PITCH_SPACE_SIGNIFICANT_DIGITS as i32);
    let normalized = (pc.rem_euclid(12.0) * factor).round() / factor;
    if normalized == 12.0 { 0.0 } else { normalized }
}

fn parse_pitch_class_string(value: &str) -> Result<FloatType> {
    let value = value.trim();
    if value.chars().count() == 1
        && let Ok(pc_string) = PitchClassString::try_from(value.chars().next().unwrap())
    {
        return Ok(pc_string.to_number() as FloatType);
    }

    value
        .parse::<FloatType>()
        .map_err(|err| Error::PitchClass(format!("cannot parse pitch class {value:?}: {err}")))
}

fn trim_float(value: FloatType) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub(crate) fn convert_ps_to_oct(ps: FloatType) -> IntegerType {
    let factor = (10 as FloatType).powi(PITCH_SPACE_SIGNIFICANT_DIGITS as i32);
    let ps_rounded = (ps * factor).round() / factor;
    (ps_rounded / 12.0).floor() as IntegerType - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive() {
        assert_eq!(convert_pitch_class_to_str(3), "3");
        assert_eq!(convert_pitch_class_to_str(10), "A");
    }

    #[test]
    fn test_wraparound() {
        assert_eq!(convert_pitch_class_to_str(12), "0");
        assert_eq!(convert_pitch_class_to_str(13), "1");
    }

    #[test]
    fn test_negative() {
        // In Python: -1 % 12 == 11, so expect "B"
        assert_eq!(convert_pitch_class_to_str(-1), "B");
    }

    #[test]
    fn pitch_class_normalizes_numeric_values() {
        let pitch_class = PitchClass::new(13).unwrap();
        assert_eq!(pitch_class.number(), 1.0);
        assert_eq!(pitch_class.integer(), Some(1));
        assert_eq!(pitch_class.string(), "1");

        let pitch_class = PitchClass::new(-1).unwrap();
        assert_eq!(pitch_class.number(), 11.0);
        assert_eq!(pitch_class.string(), "B");
    }

    #[test]
    fn pitch_class_accepts_music21_strings() {
        assert_eq!(PitchClass::new("A").unwrap().number(), 10.0);
        assert_eq!(PitchClass::new("t").unwrap().number(), 10.0);
        assert_eq!(PitchClass::new("B").unwrap().number(), 11.0);
        assert_eq!(PitchClass::new("e").unwrap().number(), 11.0);

        let microtonal = PitchClass::new("10.5").unwrap();
        assert_eq!(microtonal.number(), 10.5);
        assert_eq!(microtonal.integer(), None);
        assert_eq!(microtonal.string(), "10.5");
    }

    #[test]
    fn pitch_class_specifier_can_wrap_existing_pitch_class() {
        let pitch_class = PitchClass::new(14).unwrap();
        let clone = PitchClass::new(PitchClassSpecifier::from(pitch_class)).unwrap();
        assert_eq!(clone, pitch_class);
    }
}
