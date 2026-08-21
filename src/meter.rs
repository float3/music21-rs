//! Time signatures, ported from music21's `meter` package.
//!
//! [`TimeSignature`] covers the part of music21's meter handling that is a pure
//! function of the numerator and denominator: how long a bar is, how many beats
//! it carries, how long each beat is, and how that beat subdivides.
//!
//! music21 derives all of this from a `MeterSequence` partition tree that also
//! drives beaming, display sequences and accent weighting. Only the partition
//! *result* is ported here — the tree's other consumers have no counterpart in
//! this crate yet, and building the tree to read one number back off it would be
//! the transliterated machinery the repository guidance warns against. The
//! partition rule itself is music21's `_setDefaultBeatPartitions`, verified
//! against upstream by the `meter_parity` fixture.

use crate::defaults::{FloatType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::{Error, Result};

/// Names music21 gives a partition count, indexed by the count itself.
///
/// Index 0 is music21's `Empty`, which a valid time signature never reaches.
const BEAT_COUNT_NAMES: [&str; 9] = [
    "Empty",
    "Single",
    "Duple",
    "Triple",
    "Quadruple",
    "Quintuple",
    "Sextuple",
    "Septuple",
    "Octuple",
];

/// How the beat of a meter subdivides.
///
/// Mirrors music21's `beatDivisionCountName`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BeatDivision {
    /// A single-beat meter, which music21 reports as `Other` rather than as a
    /// division — there is no lower level to divide.
    Other,
    /// Beats divide in two.
    Simple,
    /// Beats divide in three.
    Compound,
}

impl BeatDivision {
    /// Returns music21's name for this division.
    pub fn music21_name(self) -> &'static str {
        match self {
            Self::Other => "Other",
            Self::Simple => "Simple",
            Self::Compound => "Compound",
        }
    }

    /// Returns the number of divisions in one beat.
    ///
    /// Matches music21's `beatDivisionCount`, which reports `1` rather than
    /// raising for a single-beat meter.
    pub fn count(self) -> UnsignedIntegerType {
        match self {
            Self::Other => 1,
            Self::Simple => 2,
            Self::Compound => 3,
        }
    }
}

/// A time signature, such as `4/4` or `6/8`.
///
/// ```
/// use music21_rs::TimeSignature;
///
/// let six_eight = TimeSignature::from_ratio_string("6/8")?;
/// assert_eq!(six_eight.beat_count(), 2);
/// assert_eq!(six_eight.beat_quarter_length(), 1.5);
/// assert_eq!(six_eight.classification(), "Compound Duple");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeSignature {
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
}

impl Default for TimeSignature {
    /// Returns `4/4`, matching music21's default `TimeSignature()`.
    fn default() -> Self {
        Self::common()
    }
}

impl TimeSignature {
    /// Creates a time signature from a numerator and denominator.
    ///
    /// Both must be non-zero. The denominator need not be a power of two —
    /// music21 accepts irrational meters such as `4/3`, and so does this.
    pub fn new(numerator: UnsignedIntegerType, denominator: UnsignedIntegerType) -> Result<Self> {
        if numerator == 0 {
            return Err(Error::Meter(
                "time signature numerator must be non-zero".to_string(),
            ));
        }
        if denominator == 0 {
            return Err(Error::Meter(
                "time signature denominator must be non-zero".to_string(),
            ));
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// Parses a `"numerator/denominator"` string such as `"6/8"`.
    pub fn from_ratio_string(ratio: &str) -> Result<Self> {
        let (numerator, denominator) = ratio.split_once('/').ok_or_else(|| {
            Error::Meter(format!(
                "time signature {ratio:?} is not `numerator/denominator`"
            ))
        })?;
        let parse = |part: &str, label: &str| {
            part.trim().parse::<UnsignedIntegerType>().map_err(|_| {
                Error::Meter(format!("cannot read a {label} from {part:?} in {ratio:?}"))
            })
        };
        Self::new(
            parse(numerator, "numerator")?,
            parse(denominator, "denominator")?,
        )
    }

    /// Returns common time, `4/4`.
    pub fn common() -> Self {
        Self {
            numerator: 4,
            denominator: 4,
        }
    }

    /// Returns cut time, `2/2`.
    pub fn cut() -> Self {
        Self {
            numerator: 2,
            denominator: 2,
        }
    }

    /// Returns the numerator.
    pub fn numerator(self) -> UnsignedIntegerType {
        self.numerator
    }

    /// Returns the denominator.
    pub fn denominator(self) -> UnsignedIntegerType {
        self.denominator
    }

    /// Returns the `"numerator/denominator"` spelling.
    pub fn ratio_string(self) -> String {
        format!("{}/{}", self.numerator, self.denominator)
    }

    /// Returns the length of one bar in quarter lengths.
    pub fn bar_quarter_length(self) -> FloatType {
        FloatType::from(self.numerator) * 4.0 / FloatType::from(self.denominator)
    }

    /// Returns the length of one bar as a [`Duration`].
    pub fn bar_duration(self) -> Duration {
        Duration::new(self.bar_quarter_length())
            .expect("a non-zero numerator and denominator give a positive finite bar length")
    }

    /// Returns how many beats one bar carries.
    ///
    /// This is music21's `beatCount`, which follows the numerator rather than
    /// the denominator — except at `3`, where `3/4` is three beats but `3/8` is
    /// one.
    pub fn beat_count(self) -> UnsignedIntegerType {
        match self.numerator {
            1 => 1,
            2 => 2,
            // music21 treats 3 as a single beat once the denominator is short
            // enough that the bar reads as one compound unit.
            3 if self.denominator > 4 => 1,
            3 => 3,
            4 => 4,
            6 => 2,
            9 => 3,
            12 => 4,
            numerator if numerator >= 15 && numerator.is_multiple_of(3) => numerator / 3,
            numerator => numerator,
        }
    }

    /// Returns music21's name for the beat count, such as `"Duple"`.
    ///
    /// Counts above eight are spelled as `"<n>-uple"`, as music21 does.
    pub fn beat_count_name(self) -> String {
        let count = self.beat_count();
        BEAT_COUNT_NAMES
            .get(count as usize)
            .map_or_else(|| format!("{count}-uple"), |name| (*name).to_string())
    }

    /// Returns the length of one beat in quarter lengths.
    ///
    /// Every meter this type can express has a uniform beat, so unlike
    /// music21's `beatDuration` this never fails. music21 only reports a
    /// non-uniform beat for a hand-partitioned `MeterSequence`, which has no
    /// counterpart here.
    pub fn beat_quarter_length(self) -> FloatType {
        self.bar_quarter_length() / FloatType::from(self.beat_count())
    }

    /// Returns the length of one beat as a [`Duration`].
    pub fn beat_duration(self) -> Duration {
        Duration::new(self.beat_quarter_length())
            .expect("a positive bar length divided by a positive beat count stays positive")
    }

    /// Returns how the beat subdivides.
    pub fn beat_division(self) -> BeatDivision {
        if self.beat_count() == 1 {
            BeatDivision::Other
        } else if matches!(self.numerator, 6 | 9 | 12)
            || (self.numerator >= 15 && self.numerator.is_multiple_of(3))
        {
            BeatDivision::Compound
        } else {
            BeatDivision::Simple
        }
    }

    /// Returns the number of divisions in one beat.
    pub fn beat_division_count(self) -> UnsignedIntegerType {
        self.beat_division().count()
    }

    /// Returns `true` when beats divide in three.
    pub fn is_compound(self) -> bool {
        self.beat_division() == BeatDivision::Compound
    }

    /// Returns music21's `classification`, such as `"Compound Duple"`.
    pub fn classification(self) -> String {
        format!(
            "{} {}",
            self.beat_division().music21_name(),
            self.beat_count_name()
        )
    }

    /// Returns the quarter-length offset of each beat within one bar.
    pub fn beat_offsets(self) -> Vec<FloatType> {
        let beat = self.beat_quarter_length();
        (0..self.beat_count())
            .map(|index| FloatType::from(index) * beat)
            .collect()
    }

    /// Returns the one-based beat containing `offset` quarter lengths into a bar.
    ///
    /// Matches music21's `getBeat`. Offsets at or beyond the end of the bar are
    /// rejected rather than wrapping.
    pub fn beat_at_offset(self, offset: FloatType) -> Result<UnsignedIntegerType> {
        if !offset.is_finite() || offset < 0.0 || offset >= self.bar_quarter_length() {
            return Err(Error::Meter(format!(
                "offset {offset} is outside a {} bar of {} quarter lengths",
                self.ratio_string(),
                self.bar_quarter_length()
            )));
        }
        let beat = (offset / self.beat_quarter_length()).floor();
        Ok(beat as UnsignedIntegerType + 1)
    }
}

impl std::fmt::Display for TimeSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.ratio_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(ratio: &str) -> TimeSignature {
        TimeSignature::from_ratio_string(ratio).expect("valid time signature")
    }

    #[test]
    fn common_and_cut_time_match_their_ratios() {
        assert_eq!(TimeSignature::common().ratio_string(), "4/4");
        assert_eq!(TimeSignature::cut().ratio_string(), "2/2");
        assert_eq!(TimeSignature::default(), TimeSignature::common());
    }

    #[test]
    fn bar_and_beat_lengths_follow_the_ratio() {
        assert_eq!(ts("4/4").bar_quarter_length(), 4.0);
        assert_eq!(ts("5/16").bar_quarter_length(), 1.25);
        assert_eq!(ts("3/8").bar_quarter_length(), 1.5);
        assert_eq!(ts("6/8").beat_quarter_length(), 1.5);
        assert_eq!(ts("4/4").beat_quarter_length(), 1.0);
        assert_eq!(ts("2/2").beat_duration().quarter_length(), 2.0);
    }

    #[test]
    fn compound_meters_beat_in_threes() {
        for (ratio, beats, division) in [
            ("6/8", 2, BeatDivision::Compound),
            ("9/8", 3, BeatDivision::Compound),
            ("12/8", 4, BeatDivision::Compound),
            ("15/8", 5, BeatDivision::Compound),
            ("18/8", 6, BeatDivision::Compound),
            ("24/8", 8, BeatDivision::Compound),
        ] {
            assert_eq!(ts(ratio).beat_count(), beats, "{ratio}");
            assert_eq!(ts(ratio).beat_division(), division, "{ratio}");
            assert!(ts(ratio).is_compound(), "{ratio}");
        }
    }

    #[test]
    fn three_is_the_one_denominator_sensitive_numerator() {
        // 3/2 and 3/4 read as three beats; 3/8 and shorter read as one.
        assert_eq!(ts("3/2").beat_count(), 3);
        assert_eq!(ts("3/4").beat_count(), 3);
        assert_eq!(ts("3/8").beat_count(), 1);
        assert_eq!(ts("3/16").beat_count(), 1);
        assert_eq!(ts("3/32").beat_count(), 1);
        // Every other numerator ignores the denominator entirely.
        for denominator in [2, 4, 8, 16] {
            assert_eq!(TimeSignature::new(6, denominator).unwrap().beat_count(), 2);
            assert_eq!(TimeSignature::new(5, denominator).unwrap().beat_count(), 5);
        }
    }

    #[test]
    fn classification_joins_division_and_count() {
        assert_eq!(ts("4/4").classification(), "Simple Quadruple");
        assert_eq!(ts("6/8").classification(), "Compound Duple");
        assert_eq!(ts("3/8").classification(), "Other Single");
        assert_eq!(ts("5/4").classification(), "Simple Quintuple");
        assert_eq!(ts("13/8").classification(), "Simple 13-uple");
        assert_eq!(ts("21/16").classification(), "Compound Septuple");
    }

    #[test]
    fn beat_offsets_partition_the_bar() {
        assert_eq!(ts("4/4").beat_offsets(), [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(ts("6/8").beat_offsets(), [0.0, 1.5]);
        assert_eq!(ts("5/8").beat_offsets(), [0.0, 0.5, 1.0, 1.5, 2.0]);
    }

    #[test]
    fn beat_at_offset_is_one_based_and_bounded() {
        assert_eq!(ts("4/4").beat_at_offset(1.5).unwrap(), 2);
        assert_eq!(ts("6/8").beat_at_offset(1.5).unwrap(), 2);
        assert_eq!(ts("5/8").beat_at_offset(1.5).unwrap(), 4);
        assert_eq!(ts("4/4").beat_at_offset(0.0).unwrap(), 1);
        assert!(ts("4/4").beat_at_offset(4.0).is_err());
        assert!(ts("4/4").beat_at_offset(-0.5).is_err());
        assert!(ts("4/4").beat_at_offset(FloatType::NAN).is_err());
    }

    #[test]
    fn irrational_denominators_are_accepted_as_music21_accepts_them() {
        let four_three = ts("4/3");
        assert!((four_three.bar_quarter_length() - 16.0 / 3.0).abs() < 1e-12);
        assert_eq!(four_three.beat_count(), 4);
    }

    #[test]
    fn malformed_ratios_error_instead_of_panicking() {
        for ratio in [
            "", "4", "4/", "/4", "4/4/4", "x/4", "4/x", "0/4", "4/0", "-1/4",
        ] {
            assert!(
                TimeSignature::from_ratio_string(ratio).is_err(),
                "{ratio:?} should not parse"
            );
        }
    }

    #[test]
    fn display_is_the_ratio_string() {
        assert_eq!(ts("7/8").to_string(), "7/8");
    }
}
