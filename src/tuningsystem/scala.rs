//! Runtime parsing of Scala `.scl` scale files.
//!
//! Scala files are the de facto interchange format for microtonal scales, and
//! music21 ships an archive of several thousand of them. Unlike the fixed
//! [`TuningSystem`](super::TuningSystem) tables, a scale loaded here is owned
//! data decided at runtime, and its degrees may be either exact integer ratios
//! or cents.
//!
//! ```
//! use music21_rs::ScalaScale;
//!
//! let scale: ScalaScale = "! example.scl\n\
//!                          !\n\
//!                          A perfect fifth and an octave\n\
//!                          2\n\
//!                          !\n\
//!                          3/2\n\
//!                          2/1\n".parse()?;
//!
//! assert_eq!(scale.description(), "A perfect fifth and an octave");
//! assert_eq!(scale.len(), 2);
//! assert_eq!(scale.ratio_at(1), 1.5);
//! // Degree 2 is the first degree of the next period.
//! assert_eq!(scale.ratio_at(2), 2.0);
//! # Ok::<(), music21_rs::Error>(())
//! ```

use super::Fraction;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Cents in one octave, used to convert between cents and frequency ratios.
const CENTS_PER_OCTAVE: FloatType = 1200.0;

/// A single degree of a [`ScalaScale`].
///
/// Scala files express each degree either as an exact ratio (`3/2`) or as a
/// cents value (`701.955`). The distinction is preserved rather than collapsed,
/// because a ratio carries exactness that cents cannot express.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ScalaDegree {
    /// An exact integer ratio, such as `3/2`.
    Ratio(Fraction),
    /// A cents value above the scale root, such as `701.955`.
    Cents(FloatType),
}

impl ScalaDegree {
    /// Returns this degree as a frequency ratio above the scale root.
    pub fn ratio(self) -> FloatType {
        match self {
            Self::Ratio(fraction) => fraction.ratio(),
            Self::Cents(cents) => (2.0 as FloatType).powf(cents / CENTS_PER_OCTAVE),
        }
    }

    /// Returns this degree in cents above the scale root.
    pub fn cents(self) -> FloatType {
        match self {
            Self::Ratio(fraction) => CENTS_PER_OCTAVE * fraction.ratio().log2(),
            Self::Cents(cents) => cents,
        }
    }

    /// Returns the exact ratio, when this degree was written as one.
    pub fn as_fraction(self) -> Option<Fraction> {
        match self {
            Self::Ratio(fraction) => Some(fraction),
            Self::Cents(_) => None,
        }
    }

    /// Parses one Scala note line.
    fn parse(token: &str) -> Result<Self> {
        // Anything after the value on a note line is a comment. A `!` opens one
        // without needing whitespace before it, as in dyadic53tone9div.scl's
        // `2957/2048!Gb`.
        let token = token.split('!').next().unwrap_or_default();
        let token = token.split_whitespace().next().unwrap_or_default();
        if token.is_empty() {
            return Err(Error::TuningSystem("empty scala degree".to_string()));
        }

        if token.contains('.') {
            let cents: FloatType = token
                .parse()
                .map_err(|_| Error::TuningSystem(format!("invalid scala cents value {token:?}")))?;
            if !cents.is_finite() {
                return Err(Error::TuningSystem(format!(
                    "scala cents value {token:?} is not finite"
                )));
            }
            return Ok(Self::Cents(cents));
        }

        let (numerator, denominator) = match token.split_once('/') {
            Some((numerator, denominator)) => (numerator.trim(), denominator.trim()),
            None => (token, "1"),
        };

        if let (Ok(numerator), Ok(denominator)) = (
            numerator.parse::<UnsignedIntegerType>(),
            denominator.parse::<UnsignedIntegerType>(),
        ) && numerator != 0
            && denominator != 0
        {
            return Ok(Self::Ratio(Fraction::new(numerator, denominator)));
        }

        // Four archive scales quote ratios far wider than the u32 `Fraction`
        // can hold, such as atomschis.scl's
        // 156348578434374084375/147573952589676412928. Rather than reject the
        // whole file, keep the degree as cents - `as_fraction` then reports
        // that it is not exact.
        let (numerator, denominator) = (
            parse_ratio_term(numerator, token, "numerator")?,
            parse_ratio_term(denominator, token, "denominator")?,
        );
        if numerator <= 0.0 || denominator <= 0.0 {
            return Err(Error::TuningSystem(format!(
                "scala ratio {token:?} must be positive"
            )));
        }

        Ok(Self::Cents(
            CENTS_PER_OCTAVE * (numerator / denominator).log2(),
        ))
    }
}

/// Parses one side of a ratio that did not fit `Fraction`'s integer range.
fn parse_ratio_term(term: &str, token: &str, side: &str) -> Result<FloatType> {
    if term.is_empty() || !term.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::TuningSystem(format!(
            "invalid scala ratio {side} in {token:?}"
        )));
    }
    term.parse()
        .map_err(|_| Error::TuningSystem(format!("invalid scala ratio {side} in {token:?}")))
}

impl Display for ScalaDegree {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ratio(fraction) => write!(f, "{fraction}"),
            Self::Cents(cents) => write!(f, "{cents}"),
        }
    }
}

/// A scale parsed from a Scala `.scl` file.
///
/// The stored degrees follow this crate's convention rather than Scala's: the
/// implicit `1/1` unison is made explicit at index 0, and the final entry of
/// the file — the interval the scale repeats at — is held separately as the
/// [period](Self::period) instead of being a degree. So a 12-note file yields
/// 12 degrees plus a period, and `degrees()[0]` is always `1/1`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScalaScale {
    description: String,
    degrees: Vec<ScalaDegree>,
    period: ScalaDegree,
}

impl ScalaScale {
    /// Parses the contents of a `.scl` file.
    ///
    /// Both ratio and cents degrees are accepted. Lines beginning with `!` are
    /// comments; the first non-comment line is the description, the second is
    /// the degree count, and the rest are degrees.
    pub fn parse(contents: &str) -> Result<Self> {
        let lines: Vec<&str> = contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with('!'))
            .collect();

        let count_line = lines
            .get(1)
            .ok_or_else(|| Error::TuningSystem("scala file has no degree count".to_string()))?;
        let count: usize = count_line
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .parse()
            .map_err(|_| {
                Error::TuningSystem(format!("invalid scala degree count {count_line:?}"))
            })?;
        if count == 0 {
            return Err(Error::TuningSystem(
                "scala file declares zero degrees".to_string(),
            ));
        }

        let entries: Vec<&str> = lines
            .iter()
            .skip(2)
            .filter(|line| !line.is_empty())
            .take(count)
            .copied()
            .collect();
        if entries.len() != count {
            return Err(Error::TuningSystem(format!(
                "scala file declares {count} degrees but lists {}",
                entries.len()
            )));
        }

        let mut parsed = Vec::with_capacity(count);
        for entry in entries {
            parsed.push(ScalaDegree::parse(entry)?);
        }

        // Scala's last entry is the repeat interval, not a degree of the scale.
        let period = parsed.pop().expect("count is non-zero");
        let mut degrees = vec![ScalaDegree::Ratio(Fraction::new(1, 1))];
        degrees.append(&mut parsed);

        Ok(Self {
            description: lines.first().unwrap_or(&"").trim().to_string(),
            degrees,
            period,
        })
    }

    /// Parses the raw bytes of a `.scl` file, replacing invalid UTF-8.
    ///
    /// Much of the Scala archive predates UTF-8 and carries Latin-1 bytes in
    /// its description lines; 73 of the ~3900 files music21 ships are not valid
    /// UTF-8. Degree lines are always ASCII, so a lossy conversion affects only
    /// the description text.
    ///
    /// This crate does no file IO of its own, so read the file yourself:
    ///
    /// ```no_run
    /// use music21_rs::ScalaScale;
    ///
    /// let bytes = std::fs::read("partch_43.scl")?;
    /// let scale = ScalaScale::parse_bytes(&bytes)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self> {
        Self::parse(&String::from_utf8_lossy(bytes))
    }

    /// Returns the scale's description line.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the degrees of one period, starting with `1/1`.
    pub fn degrees(&self) -> &[ScalaDegree] {
        &self.degrees
    }

    /// Returns the interval the scale repeats at.
    ///
    /// This is usually an octave, but need not be — Bohlen-Pierce repeats at
    /// `3/1`, and non-octave scales are common in the Scala archive.
    pub fn period(&self) -> ScalaDegree {
        self.period
    }

    /// Returns the number of degrees in one period.
    pub fn len(&self) -> usize {
        self.degrees.len()
    }

    /// Returns whether the scale has no degrees.
    ///
    /// Always `false` for a parsed scale, since a zero-degree file is rejected.
    pub fn is_empty(&self) -> bool {
        self.degrees.is_empty()
    }

    /// Returns the frequency ratio above the root for a degree index.
    ///
    /// Indices outside one period wrap, shifting by the [period](Self::period)
    /// for each wrap. Negative indices run below the root.
    pub fn ratio_at(&self, index: IntegerType) -> FloatType {
        let len = self.degrees.len() as IntegerType;
        let periods = index.div_euclid(len);
        let degree = index.rem_euclid(len) as usize;
        self.degrees[degree].ratio() * self.period.ratio().powi(periods)
    }

    /// Returns the cents above the root for a degree index.
    ///
    /// Note this is absolute distance from the scale root, unlike
    /// [`TuningSystem::cents_at`](super::TuningSystem::cents_at), which reports
    /// deviation from equal temperament.
    pub fn cents_above_root(&self, index: IntegerType) -> FloatType {
        CENTS_PER_OCTAVE * self.ratio_at(index).log2()
    }

    /// Returns the frequency in hertz for a degree index, given a root pitch.
    ///
    /// A Scala scale fixes no absolute pitch, so the root is supplied by the
    /// caller.
    pub fn frequency_at(&self, root_hz: FloatType, index: IntegerType) -> FloatType {
        root_hz * self.ratio_at(index)
    }
}

impl FromStr for ScalaScale {
    type Err = Error;

    fn from_str(contents: &str) -> Result<Self> {
        Self::parse(contents)
    }
}

impl Display for ScalaScale {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} degrees)", self.description, self.degrees.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIFTH_AND_OCTAVE: &str = "! example.scl\n!\nA fifth and an octave\n 2\n!\n 3/2\n 2/1\n";

    #[test]
    fn parses_ratios_into_the_crate_convention() {
        let scale = ScalaScale::parse(FIFTH_AND_OCTAVE).unwrap();
        assert_eq!(scale.description(), "A fifth and an octave");
        assert_eq!(scale.len(), 2);
        assert_eq!(
            scale.degrees(),
            &[
                ScalaDegree::Ratio(Fraction::new(1, 1)),
                ScalaDegree::Ratio(Fraction::new(3, 2)),
            ]
        );
        assert_eq!(scale.period(), ScalaDegree::Ratio(Fraction::new(2, 1)));
    }

    #[test]
    fn wraps_indices_by_the_period_in_both_directions() {
        let scale = ScalaScale::parse(FIFTH_AND_OCTAVE).unwrap();
        assert_eq!(scale.ratio_at(0), 1.0);
        assert_eq!(scale.ratio_at(1), 1.5);
        assert_eq!(scale.ratio_at(2), 2.0);
        assert_eq!(scale.ratio_at(3), 3.0);
        assert_eq!(scale.ratio_at(-2), 0.5);
        assert_eq!(scale.ratio_at(-1), 0.75);
    }

    #[test]
    fn honours_a_non_octave_period() {
        // Bohlen-Pierce repeats at a tritave rather than an octave.
        let scale = ScalaScale::parse("Tritave\n 2\n 5/3\n 3/1\n").unwrap();
        assert_eq!(scale.period(), ScalaDegree::Ratio(Fraction::new(3, 1)));
        assert_eq!(scale.ratio_at(2), 3.0);
        assert_eq!(scale.ratio_at(4), 9.0);
    }

    #[test]
    fn accepts_cents_degrees() {
        let scale = ScalaScale::parse("Cents\n 2\n 701.955\n 1200.0\n").unwrap();
        assert_eq!(scale.degrees()[1], ScalaDegree::Cents(701.955));
        assert!((scale.ratio_at(1) - 1.5).abs() < 1e-6);
        assert!((scale.ratio_at(2) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn mixes_ratio_and_cents_degrees() {
        let scale = ScalaScale::parse("Mixed\n 3\n 100.0\n 3/2\n 2/1\n").unwrap();
        assert!(matches!(scale.degrees()[1], ScalaDegree::Cents(_)));
        assert!(matches!(scale.degrees()[2], ScalaDegree::Ratio(_)));
        assert!((scale.cents_above_root(2) - 701.955).abs() < 1e-3);
    }

    #[test]
    fn treats_a_bare_integer_as_a_whole_ratio() {
        let scale = ScalaScale::parse("Integers\n 2\n 3\n 4\n").unwrap();
        assert_eq!(scale.degrees()[1], ScalaDegree::Ratio(Fraction::new(3, 1)));
        assert_eq!(scale.period(), ScalaDegree::Ratio(Fraction::new(4, 1)));
    }

    #[test]
    fn ignores_trailing_comments_on_degree_lines() {
        let scale = ScalaScale::parse("Commented\n 2\n 3/2 the fifth\n 2/1 octave\n").unwrap();
        assert_eq!(scale.ratio_at(1), 1.5);
    }

    #[test]
    fn ignores_a_bang_comment_with_no_leading_space() {
        // dyadic53tone9div.scl writes degrees as `2957/2048!Gb`.
        let scale = ScalaScale::parse("Bang\n 2\n 3/2!the fifth\n 2/1!octave\n").unwrap();
        assert_eq!(scale.ratio_at(1), 1.5);
        assert_eq!(scale.period(), ScalaDegree::Ratio(Fraction::new(2, 1)));
    }

    #[test]
    fn keeps_an_empty_description_line() {
        let scale = ScalaScale::parse("\n 1\n 2/1\n").unwrap();
        assert_eq!(scale.description(), "");
        assert_eq!(scale.len(), 1);
    }

    #[test]
    fn computes_frequencies_from_a_caller_supplied_root() {
        let scale = ScalaScale::parse(FIFTH_AND_OCTAVE).unwrap();
        assert!((scale.frequency_at(440.0, 1) - 660.0).abs() < 1e-9);
        assert!((scale.frequency_at(440.0, 2) - 880.0).abs() < 1e-9);
    }

    #[test]
    fn falls_back_to_cents_for_ratios_too_wide_for_u32() {
        // First degree of atomschis.scl, whose numerator and denominator are
        // both far past u32.
        let scale = ScalaScale::parse(
            "Atom Schisma\n 2\n 156348578434374084375/147573952589676412928\n 2/1\n",
        )
        .unwrap();

        let degree = scale.degrees()[1];
        assert!(degree.as_fraction().is_none(), "not exactly representable");
        assert!(
            (degree.cents() - 99.993_599_6).abs() < 1e-6,
            "{}",
            degree.cents()
        );
    }

    #[test]
    fn parses_latin1_description_bytes_lossily() {
        // 0xE9 is Latin-1 "e-acute" and is not valid UTF-8 on its own.
        let bytes = b"Caf\xe9 scale\n 1\n 2/1\n";
        let scale = ScalaScale::parse_bytes(bytes).unwrap();
        assert!(scale.description().starts_with("Caf"));
        assert_eq!(scale.len(), 1);
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",                           // nothing at all
            "Only a description\n",       // no count
            "Bad count\n not-a-number\n", // unparseable count
            "Too few\n 4\n 3/2\n 2/1\n",  // count exceeds the listed degrees
            "Zero\n 0\n",                 // no degrees
            "Bad ratio\n 1\n 3/0\n",      // zero denominator
            "Bad ratio\n 1\n 1/2/3\n",    // not a ratio
        ] {
            assert!(
                ScalaScale::parse(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn round_trips_through_from_str() {
        let scale: ScalaScale = FIFTH_AND_OCTAVE.parse().unwrap();
        assert_eq!(scale.len(), 2);
    }
}
