//! The intervals this crate walks by, parsed once.
//!
//! Six modules each parsed their own copy of the same few names. They are
//! the same intervals, and a name that would not parse is a bug in this file
//! rather than in each of them.

use std::sync::LazyLock;

use super::Interval;

/// A perfect unison.
pub(crate) static PERFECT_UNISON: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P1").expect("P1 is a valid interval"));

/// A perfect fourth upwards.
pub(crate) static PERFECT_FOURTH_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P4").expect("P4 is a valid interval"));

/// A perfect fifth upwards.
pub(crate) static PERFECT_FIFTH_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));

/// A perfect fifth downwards.
pub(crate) static PERFECT_FIFTH_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("-P5").expect("-P5 is a valid interval"));

/// A perfect octave upwards.
pub(crate) static PERFECT_OCTAVE: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P8").expect("P8 is a valid interval"));

/// A diminished second upwards, which is how a pitch is respelled.
pub(crate) static DIMINISHED_SECOND_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("d2").expect("d2 is a valid interval"));

/// The same interval downwards.
pub(crate) static DIMINISHED_SECOND_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("-d2").expect("-d2 is a valid interval"));

#[cfg(test)]
mod tests {
    /// `keysignature` spelled its descending fifth `P-5` and `interval`
    /// spelled it `-P5`. One constant now stands for both, which holds only
    /// while the two spellings mean one interval.
    #[test]
    fn the_two_spellings_of_a_descending_fifth_agree() {
        use super::PERFECT_FIFTH_DOWN;
        use crate::interval::Interval;

        assert_eq!(
            Interval::from_name("P-5").expect("P-5 parses"),
            *PERFECT_FIFTH_DOWN
        );
    }
}
