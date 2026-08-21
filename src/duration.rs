use crate::{
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
};

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// A note-value name, as music21's `duration.typeToDuration` defines them.
///
/// Each type is a power-of-two multiple of a quarter note, from the
/// `duplex-maxima` (sixteen whole notes) down to the `2048th`, plus the `zero`
/// length music21 uses for grace notes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DurationType {
    /// Duplex maxima, sixteen whole notes.
    DuplexMaxima,
    /// Maxima, eight whole notes.
    Maxima,
    /// Longa, four whole notes.
    Longa,
    /// Breve, or double whole note.
    Breve,
    /// Whole note.
    Whole,
    /// Half note.
    Half,
    /// Quarter note.
    Quarter,
    /// Eighth note.
    Eighth,
    /// Sixteenth note.
    Sixteenth,
    /// Thirty-second note.
    ThirtySecond,
    /// Sixty-fourth note.
    SixtyFourth,
    /// Hundred-twenty-eighth note.
    HundredTwentyEighth,
    /// Two-hundred-fifty-sixth note.
    TwoHundredFiftySixth,
    /// Five-hundred-twelfth note.
    FiveHundredTwelfth,
    /// Ten-twenty-fourth note.
    TenTwentyFourth,
    /// Twenty-forty-eighth note.
    TwentyFortyEighth,
    /// A grace-note duration of no length.
    Zero,
}

impl DurationType {
    /// Every duration type, longest first, matching music21's ordering.
    pub const ALL: [DurationType; 17] = [
        Self::DuplexMaxima,
        Self::Maxima,
        Self::Longa,
        Self::Breve,
        Self::Whole,
        Self::Half,
        Self::Quarter,
        Self::Eighth,
        Self::Sixteenth,
        Self::ThirtySecond,
        Self::SixtyFourth,
        Self::HundredTwentyEighth,
        Self::TwoHundredFiftySixth,
        Self::FiveHundredTwelfth,
        Self::TenTwentyFourth,
        Self::TwentyFortyEighth,
        Self::Zero,
    ];

    /// Returns the music21 type name, such as `"whole"` or `"16th"`.
    pub fn music21_name(self) -> &'static str {
        match self {
            Self::DuplexMaxima => "duplex-maxima",
            Self::Maxima => "maxima",
            Self::Longa => "longa",
            Self::Breve => "breve",
            Self::Whole => "whole",
            Self::Half => "half",
            Self::Quarter => "quarter",
            Self::Eighth => "eighth",
            Self::Sixteenth => "16th",
            Self::ThirtySecond => "32nd",
            Self::SixtyFourth => "64th",
            Self::HundredTwentyEighth => "128th",
            Self::TwoHundredFiftySixth => "256th",
            Self::FiveHundredTwelfth => "512th",
            Self::TenTwentyFourth => "1024th",
            Self::TwentyFortyEighth => "2048th",
            Self::Zero => "zero",
        }
    }

    /// Returns the length of one undotted note of this type, in quarter lengths.
    pub fn quarter_length(self) -> FloatType {
        match self {
            Self::DuplexMaxima => 64.0,
            Self::Maxima => 32.0,
            Self::Longa => 16.0,
            Self::Breve => 8.0,
            Self::Whole => 4.0,
            Self::Half => 2.0,
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::Sixteenth => 0.25,
            Self::ThirtySecond => 0.125,
            Self::SixtyFourth => 0.0625,
            Self::HundredTwentyEighth => 0.03125,
            Self::TwoHundredFiftySixth => 0.015625,
            Self::FiveHundredTwelfth => 0.0078125,
            Self::TenTwentyFourth => 0.00390625,
            Self::TwentyFortyEighth => 0.001953125,
            Self::Zero => 0.0,
        }
    }

    /// Parses a music21 type name.
    pub fn from_music21_name(name: &str) -> Option<Self> {
        match name {
            "duplex-maxima" => Some(Self::DuplexMaxima),
            "maxima" => Some(Self::Maxima),
            "longa" => Some(Self::Longa),
            "breve" => Some(Self::Breve),
            "whole" => Some(Self::Whole),
            "half" => Some(Self::Half),
            "quarter" => Some(Self::Quarter),
            "eighth" => Some(Self::Eighth),
            "16th" => Some(Self::Sixteenth),
            "32nd" => Some(Self::ThirtySecond),
            "64th" => Some(Self::SixtyFourth),
            "128th" => Some(Self::HundredTwentyEighth),
            "256th" => Some(Self::TwoHundredFiftySixth),
            "512th" => Some(Self::FiveHundredTwelfth),
            "1024th" => Some(Self::TenTwentyFourth),
            "2048th" => Some(Self::TwentyFortyEighth),
            "zero" => Some(Self::Zero),
            _ => None,
        }
    }

    /// Returns the type whose undotted length is exactly `quarter_length`.
    pub fn from_quarter_length(quarter_length: FloatType) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.quarter_length() == quarter_length)
    }

    /// Returns the length of this type carrying `dots` augmentation dots.
    ///
    /// Each dot adds half of what came before, so a dotted half is `3.0` and a
    /// double-dotted half is `3.5`.
    pub fn quarter_length_with_dots(self, dots: u32) -> FloatType {
        self.quarter_length() * (2.0 - (0.5 as FloatType).powi(dots as i32))
    }
}

impl Display for DurationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.music21_name())
    }
}

impl FromStr for DurationType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_music21_name(value)
            .ok_or_else(|| Error::Ordinal(format!("unknown duration type {value:?}")))
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Rhythmic duration measured in quarter lengths.
///
/// A quarter note has a quarter length of `1.0`; an eighth note is `0.5`;
/// a whole note is `4.0`.
pub struct Duration {
    quarter_length: FloatType,
}

impl Duration {
    /// Creates a duration from a quarter-length value.
    pub fn new(quarter_length: FloatType) -> Result<Self> {
        if !quarter_length.is_finite() || quarter_length < 0.0 {
            return Err(Error::Ordinal(format!(
                "duration quarter length must be finite and non-negative, got {quarter_length}"
            )));
        }

        Ok(Self { quarter_length })
    }

    /// Returns a quarter-note duration.
    pub fn quarter() -> Self {
        Self::default()
    }

    /// Returns a half-note duration.
    pub fn half() -> Self {
        Self::new(2.0).expect("constant duration is valid")
    }

    /// Returns a whole-note duration.
    pub fn whole() -> Self {
        Self::new(4.0).expect("constant duration is valid")
    }

    /// Returns an eighth-note duration.
    pub fn eighth() -> Self {
        Self::new(0.5).expect("constant duration is valid")
    }

    /// Creates a duration from a note-value type.
    pub fn from_type(duration_type: DurationType) -> Self {
        Self {
            quarter_length: duration_type.quarter_length(),
        }
    }

    /// Creates a duration from a note-value type carrying augmentation dots.
    pub fn from_type_with_dots(duration_type: DurationType, dots: u32) -> Self {
        Self {
            quarter_length: duration_type.quarter_length_with_dots(dots),
        }
    }

    /// Returns the note-value type whose undotted length this duration is.
    ///
    /// Returns `None` for a length that is not a plain note value, such as a
    /// dotted or tuplet duration.
    pub fn duration_type(&self) -> Option<DurationType> {
        DurationType::from_quarter_length(self.quarter_length)
    }

    /// Returns the duration in quarter lengths.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }

    /// Updates the duration in quarter lengths.
    pub fn set_quarter_length(&mut self, quarter_length: FloatType) -> Result<()> {
        *self = Self::new(quarter_length)?;
        Ok(())
    }
}

impl Default for Duration {
    fn default() -> Self {
        Self {
            quarter_length: 1.0,
        }
    }
}

impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        self.quarter_length == other.quarter_length
    }
}

impl TryFrom<FloatType> for Duration {
    type Error = Error;

    fn try_from(value: FloatType) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<IntegerType> for Duration {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::new(value as FloatType)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// music21's `duration.typeToDuration`, verbatim.
    const MUSIC21_TYPE_TO_DURATION: [(&str, FloatType); 17] = [
        ("duplex-maxima", 64.0),
        ("maxima", 32.0),
        ("longa", 16.0),
        ("breve", 8.0),
        ("whole", 4.0),
        ("half", 2.0),
        ("quarter", 1.0),
        ("eighth", 0.5),
        ("16th", 0.25),
        ("32nd", 0.125),
        ("64th", 0.0625),
        ("128th", 0.03125),
        ("256th", 0.015625),
        ("512th", 0.0078125),
        ("1024th", 0.00390625),
        ("2048th", 0.001953125),
        ("zero", 0.0),
    ];

    #[test]
    fn duration_types_match_music21s_table() {
        assert_eq!(DurationType::ALL.len(), MUSIC21_TYPE_TO_DURATION.len());
        for (duration_type, (name, quarter_length)) in
            DurationType::ALL.into_iter().zip(MUSIC21_TYPE_TO_DURATION)
        {
            assert_eq!(duration_type.music21_name(), name);
            assert_eq!(duration_type.quarter_length(), quarter_length, "{name}");
            assert_eq!(DurationType::from_music21_name(name), Some(duration_type));
        }
    }

    #[test]
    fn duration_types_round_trip_through_their_names() {
        for duration_type in DurationType::ALL {
            let name = duration_type.music21_name();
            assert_eq!(name.parse::<DurationType>().unwrap(), duration_type);
            assert_eq!(duration_type.to_string(), name);
        }
        assert!("not-a-duration".parse::<DurationType>().is_err());
    }

    #[test]
    fn each_type_is_half_the_one_before_it() {
        // `zero` is the exception and is excluded.
        let ordered = &DurationType::ALL[..DurationType::ALL.len() - 1];
        for pair in ordered.windows(2) {
            assert_eq!(
                pair[1].quarter_length() * 2.0,
                pair[0].quarter_length(),
                "{} should be half of {}",
                pair[1],
                pair[0]
            );
        }
    }

    #[test]
    fn dots_add_half_of_what_came_before() {
        assert_eq!(DurationType::Half.quarter_length_with_dots(0), 2.0);
        assert_eq!(DurationType::Half.quarter_length_with_dots(1), 3.0);
        assert_eq!(DurationType::Half.quarter_length_with_dots(2), 3.5);
        assert_eq!(DurationType::Half.quarter_length_with_dots(3), 3.75);
        assert_eq!(DurationType::Quarter.quarter_length_with_dots(1), 1.5);
    }

    #[test]
    fn durations_convert_to_and_from_note_values() {
        assert_eq!(
            Duration::from_type(DurationType::Whole).quarter_length(),
            4.0
        );
        assert_eq!(
            Duration::from_type(DurationType::Whole).duration_type(),
            Some(DurationType::Whole)
        );
        assert_eq!(
            Duration::from_type_with_dots(DurationType::Half, 1).quarter_length(),
            3.0
        );
        // A dotted value is not itself a note value.
        assert_eq!(
            Duration::from_type_with_dots(DurationType::Half, 1).duration_type(),
            None
        );
        // Nor is a triplet eighth.
        assert_eq!(Duration::new(1.0 / 3.0).unwrap().duration_type(), None);
    }

    #[test]
    fn the_named_helpers_agree_with_their_types() {
        assert_eq!(
            Duration::quarter(),
            Duration::from_type(DurationType::Quarter)
        );
        assert_eq!(Duration::half(), Duration::from_type(DurationType::Half));
        assert_eq!(Duration::whole(), Duration::from_type(DurationType::Whole));
        assert_eq!(
            Duration::eighth(),
            Duration::from_type(DurationType::Eighth)
        );
    }

    #[test]
    fn duration_tracks_quarter_lengths() {
        assert_eq!(Duration::quarter().quarter_length(), 1.0);
        assert_eq!(Duration::half().quarter_length(), 2.0);
        assert_eq!(Duration::whole().quarter_length(), 4.0);
        assert_eq!(Duration::eighth().quarter_length(), 0.5);
    }

    #[test]
    fn duration_rejects_invalid_values() {
        assert!(Duration::new(-1.0).is_err());
        assert!(Duration::new(FloatType::INFINITY).is_err());
    }

    #[test]
    fn duration_supports_conversions_and_updates() {
        let mut duration = Duration::try_from(3 as IntegerType).unwrap();
        assert_eq!(duration.quarter_length(), 3.0);

        duration.set_quarter_length(1.5).unwrap();
        assert_eq!(duration, Duration::try_from(1.5).unwrap());
        assert!(duration.set_quarter_length(FloatType::NAN).is_err());
    }
}
