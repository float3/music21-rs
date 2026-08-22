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

use std::collections::BTreeMap;
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

        // Scala also writes equal-tempered steps as `n\m`, meaning `n` steps of
        // `m`-EDO. `1\41` is one step of 41-EDO, or 29.268 cents.
        if let Some((steps, divisions)) = token.split_once('\\') {
            let steps: FloatType = steps.trim().parse().map_err(|_| {
                Error::TuningSystem(format!("invalid scala step count in {token:?}"))
            })?;
            let divisions: FloatType = divisions.trim().parse().map_err(|_| {
                Error::TuningSystem(format!("invalid scala division count in {token:?}"))
            })?;
            if divisions == 0.0 {
                return Err(Error::TuningSystem(format!(
                    "scala degree {token:?} divides the octave into zero steps"
                )));
            }
            return Ok(Self::Cents(CENTS_PER_OCTAVE * steps / divisions));
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
    /// Builds a scale from already-split parts.
    ///
    /// Used by the bundled archive, whose degrees were separated from the file
    /// structure when the archive was generated.
    #[cfg(feature = "scala-archive")]
    pub(crate) fn from_parts(
        description: String,
        degrees: Vec<ScalaDegree>,
        period: ScalaDegree,
    ) -> Self {
        Self {
            description,
            degrees,
            period,
        }
    }

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
        // A file may legitimately declare zero degrees, in which case there is
        // neither a degree list nor a repeat interval to read.
        let (mut degrees, period) = match parsed.pop() {
            Some(period) => (vec![ScalaDegree::Ratio(Fraction::new(1, 1))], period),
            None => (Vec::new(), ScalaDegree::Ratio(Fraction::new(1, 1))),
        };
        degrees.append(&mut parsed);

        Ok(Self {
            description: lines.first().unwrap_or(&"").trim().to_string(),
            degrees,
            period,
        })
    }

    /// Parses the raw bytes of a `.scl` file.
    ///
    /// The Scala format is defined as latin-1 (ISO-8859-1), which music21's
    /// `scale.scala` module states explicitly, and 73 of the ~3900 files it
    /// ships are not valid UTF-8. Bytes are therefore decoded as latin-1 rather
    /// than as UTF-8, so accented characters in description lines survive
    /// instead of becoming replacement characters. Degree lines are ASCII in
    /// either reading, so numbers are unaffected.
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
        // In latin-1 every byte is its own code point, so this cannot fail.
        let text: String = bytes.iter().map(|&byte| byte as char).collect();
        Self::parse(&text)
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
    /// A Scala file may declare zero degrees — the archive's `xxx.scl` does,
    /// and music21 reads it as a scale with no pitches.
    pub fn is_empty(&self) -> bool {
        self.degrees.is_empty()
    }

    /// Returns the frequency ratio above the root for a degree index.
    ///
    /// Indices outside one period wrap, shifting by the [period](Self::period)
    /// for each wrap. Negative indices run below the root.
    ///
    /// A scale with no degrees has nothing to wrap through, so every index
    /// returns the root ratio of `1.0`.
    pub fn ratio_at(&self, index: IntegerType) -> FloatType {
        if self.degrees.is_empty() {
            return 1.0;
        }
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

/// A searchable collection of Scala scales, keyed by file name.
///
/// This is the runtime counterpart to music21's `scale.scala` module, whose
/// `search` and `getPaths` helpers index the Scala scale archive. The archive
/// itself is **not** bundled with this crate — see the note below — so the
/// caller supplies the files and this type indexes whatever it is given, which
/// also keeps the crate free of file IO and usable from wasm.
///
/// ```
/// use music21_rs::ScalaArchive;
///
/// let mut archive = ScalaArchive::new();
/// archive.insert("mbira_banda.scl", b"Mbira Banda\n 1\n 2/1\n")?;
/// archive.insert("slendro5_2.scl", b"Slendro\n 1\n 2/1\n")?;
///
/// assert_eq!(archive.search("mbira"), ["mbira_banda.scl"]);
/// assert!(archive.get("slendro5_2.scl").is_some());
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScalaArchive {
    scales: BTreeMap<String, ScalaScale>,
}

impl ScalaArchive {
    /// Creates an empty archive.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses and indexes one `.scl` file under the given name.
    ///
    /// The name is the file name as music21 refers to it, such as
    /// `"partch_43.scl"`. Returns the scale it replaced, if any.
    pub fn insert(
        &mut self,
        file_name: impl Into<String>,
        bytes: &[u8],
    ) -> Result<Option<ScalaScale>> {
        let file_name = file_name.into();
        let scale = ScalaScale::parse_bytes(bytes)
            .map_err(|error| Error::TuningSystem(format!("{file_name}: {error}")))?;
        Ok(self.scales.insert(file_name, scale))
    }

    /// Indexes an already-parsed scale under the given name.
    pub fn insert_scale(
        &mut self,
        file_name: impl Into<String>,
        scale: ScalaScale,
    ) -> Option<ScalaScale> {
        self.scales.insert(file_name.into(), scale)
    }

    /// Returns the scale stored under a file name.
    pub fn get(&self, file_name: &str) -> Option<&ScalaScale> {
        self.scales.get(file_name)
    }

    /// Returns every indexed file name, in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.scales.keys().map(String::as_str)
    }

    /// Returns every indexed scale with its file name, in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ScalaScale)> {
        self.scales
            .iter()
            .map(|(name, scale)| (name.as_str(), scale))
    }

    /// Returns the number of indexed scales.
    pub fn len(&self) -> usize {
        self.scales.len()
    }

    /// Returns whether the archive is empty.
    pub fn is_empty(&self) -> bool {
        self.scales.is_empty()
    }

    /// Builds an archive from the `.scl` files bundled with the crate.
    ///
    /// Only available under the non-default `scala-archive` feature, which adds
    /// roughly a megabyte of scale data to the build. Without it, supply the
    /// files yourself with [`ScalaArchive::insert`].
    ///
    /// Use [`ScalaArchive::bundled_with_failures`] to see any scale that fails
    /// to load rather than silently dropping it.
    #[cfg(feature = "scala-archive")]
    pub fn bundled() -> Self {
        Self::bundled_with_failures().0
    }

    /// Builds an archive from the bundled files, reporting the ones that fail.
    ///
    /// The failures are a fixed property of the bundled data, so the returned
    /// list is the same on every call.
    #[cfg(feature = "scala-archive")]
    pub fn bundled_with_failures() -> (Self, Vec<(&'static str, Error)>) {
        let mut archive = Self::new();
        let mut failures = Vec::new();
        for (file_name, description, degrees, period) in crate::tuningsystem::scala_bundled::SCALES
        {
            match build_bundled(description, degrees, period) {
                Ok(scale) => {
                    archive.insert_scale(file_name, scale);
                }
                Err(error) => failures.push((file_name, error)),
            }
        }
        (archive, failures)
    }

    /// Returns the number of `.scl` files bundled with the crate.
    ///
    /// Counts files without parsing them, so this includes the one that
    /// [`ScalaArchive::bundled`] skips.
    #[cfg(feature = "scala-archive")]
    pub fn bundled_len() -> usize {
        crate::tuningsystem::scala_bundled::SCALES.len()
    }

    /// Finds file names matching a search string, as music21's
    /// `scale.scala.search` does.
    ///
    /// Spaces in the target are ignored, an exact file-name match is preferred,
    /// and the remaining matches are substring hits against the name with its
    /// extension dropped and against a form with `_` and `-` removed. Results
    /// are sorted.
    pub fn search(&self, target: &str) -> Vec<&str> {
        let target = target.replace(' ', "").to_lowercase();
        let mut matches = Vec::new();

        for name in self.scales.keys() {
            if name.to_lowercase() == target {
                matches.push(name.as_str());
            }
        }

        for name in self.scales.keys() {
            if matches.contains(&name.as_str()) {
                continue;
            }
            let stem = name.strip_suffix(".scl").unwrap_or(name).to_lowercase();
            let squashed = stem.replace(['_', '-'], "");
            if stem.contains(&target) || squashed.contains(&target) {
                matches.push(name.as_str());
            }
        }

        matches.sort_unstable();
        matches
    }
}

/// Rebuilds a [`ScalaScale`] from the tokens emitted into `scala_bundled`.
///
/// The generator already split description, degrees and period, so this only
/// has to parse each degree token — no Scala file structure is involved.
#[cfg(feature = "scala-archive")]
fn build_bundled(description: &str, degrees: &[&str], period: &str) -> Result<ScalaScale> {
    let degrees = degrees
        .iter()
        .map(|token| ScalaDegree::parse(token))
        .collect::<Result<Vec<_>>>()?;
    Ok(ScalaScale::from_parts(
        description.to_string(),
        degrees,
        ScalaDegree::parse(period)?,
    ))
}

impl Extend<(String, ScalaScale)> for ScalaArchive {
    fn extend<T: IntoIterator<Item = (String, ScalaScale)>>(&mut self, iter: T) {
        self.scales.extend(iter);
    }
}

impl FromIterator<(String, ScalaScale)> for ScalaArchive {
    fn from_iter<T: IntoIterator<Item = (String, ScalaScale)>>(iter: T) -> Self {
        Self {
            scales: iter.into_iter().collect(),
        }
    }
}

#[cfg(all(test, feature = "scala-archive"))]
mod bundled_tests {
    use super::ScalaArchive;

    #[test]
    fn the_whole_bundled_archive_parses() {
        let (archive, failures) = ScalaArchive::bundled_with_failures();
        let names: Vec<&str> = failures.iter().map(|(name, _)| *name).collect();
        // Every scale in the bundle parses. `sparschuh-stanhope.scl` used to
        // fail here for writing a degree as `697//441`; upstream fixed it in
        // cuthbertLab/music21#2003. `xxx.scl` declares zero degrees, which is
        // legal Scala and which music21 accepts too.
        assert_eq!(names, [] as [&str; 0]);
        assert_eq!(archive.len(), ScalaArchive::bundled_len());
        // 3932 from music21 plus 62 vendored into data/scala_extra.
        assert_eq!(ScalaArchive::bundled_len(), 3994);
    }

    #[test]
    fn the_bundled_archive_carries_the_scales_the_tuning_tables_cite() {
        let archive = ScalaArchive::bundled();
        for name in [
            "partch_43.scl",
            "partch_29.scl",
            "werck3.scl",
            "vallotti.scl",
            "meanquar.scl",
            "ptolemy.scl",
            "pyth_12.scl",
            "kirnberger3.scl",
            "rameau.scl",
            "young2.scl",
            "carlos_harm.scl",
            "riley_albion.scl",
            "indian.scl",
            "indian-sagrama.scl",
        ] {
            assert!(
                archive.get(name).is_some(),
                "{name} missing from the bundle"
            );
        }
    }

    #[test]
    fn bundled_file_names_are_sorted_and_unique() {
        let names: Vec<&str> = crate::tuningsystem::scala_bundled::SCALES
            .iter()
            .map(|(name, _, _, _)| *name)
            .collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted, "bundled file list must be sorted and unique");
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn accepts_a_scale_declaring_zero_degrees() {
        // music21 reads the archive's xxx.scl as a scale with no pitches, so
        // the count line being `0` is legal rather than malformed.
        let scale = ScalaScale::parse(
            "! xxx.scl
!
Saved scale from Scala
 0
!
",
        )
        .expect("a zero-degree file parses");
        assert!(scale.is_empty());
        assert_eq!(scale.len(), 0);
        assert_eq!(scale.description(), "Saved scale from Scala");
        // Nothing to wrap through, so every index is the root.
        assert_eq!(scale.ratio_at(0), 1.0);
        assert_eq!(scale.ratio_at(7), 1.0);
        assert_eq!(scale.ratio_at(-3), 1.0);
    }

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
    fn reads_equal_tempered_step_notation() {
        // `n\\m` is n steps of m-EDO, which several archives use for EDO files.
        let scale = ScalaScale::parse("41-EDO\n 2\n 1\\41\n 41\\41\n").unwrap();
        assert!((scale.degrees()[1].cents() - 1200.0 / 41.0).abs() < 1e-9);
        assert!((scale.period().cents() - 1200.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_a_zero_division_step() {
        assert!(ScalaScale::parse("Bad\n 1\\0\n").is_err());
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",                           // nothing at all
            "Only a description\n",       // no count
            "Bad count\n not-a-number\n", // unparseable count
            "Too few\n 4\n 3/2\n 2/1\n",  // count exceeds the listed degrees
            "Bad ratio\n 1\n 3/0\n",      // zero denominator
            "Bad ratio\n 1\n 1/2/3\n",    // not a ratio
        ] {
            assert!(
                ScalaScale::parse(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    fn archive_of(names: &[&str]) -> ScalaArchive {
        let mut archive = ScalaArchive::new();
        for name in names {
            archive
                .insert(*name, b"A scale\n 1\n 2/1\n")
                .expect("fixture scale parses");
        }
        archive
    }

    #[test]
    fn archive_indexes_and_retrieves_by_file_name() {
        let archive = archive_of(&["partch_43.scl", "slendro5_2.scl"]);
        assert_eq!(archive.len(), 2);
        assert!(archive.get("partch_43.scl").is_some());
        assert!(archive.get("missing.scl").is_none());
        assert_eq!(
            archive.names().collect::<Vec<_>>(),
            ["partch_43.scl", "slendro5_2.scl"]
        );
    }

    #[test]
    fn archive_search_matches_music21_semantics() {
        let archive = archive_of(&[
            "mbira_banda.scl",
            "mbira_banda2.scl",
            "mbira_zimb.scl",
            "slendro5_2.scl",
            "partch_43.scl",
        ]);

        // Substring hit against the stem.
        assert_eq!(
            archive.search("mbira"),
            ["mbira_banda.scl", "mbira_banda2.scl", "mbira_zimb.scl"]
        );
        // Spaces in the target are ignored.
        assert_eq!(
            archive.search("mbira banda"),
            ["mbira_banda.scl", "mbira_banda2.scl"]
        );
        // Underscores and hyphens are ignored on the indexed side.
        assert_eq!(
            archive.search("mbirabanda"),
            ["mbira_banda.scl", "mbira_banda2.scl"]
        );
        // Matching is case-insensitive.
        assert_eq!(archive.search("PARTCH"), ["partch_43.scl"]);
        // An exact file name matches.
        assert_eq!(archive.search("slendro5_2.scl"), ["slendro5_2.scl"]);
        assert!(archive.search("nothing-here").is_empty());
    }

    #[test]
    fn archive_reports_the_offending_file_on_a_parse_error() {
        let mut archive = ScalaArchive::new();
        let error = archive
            .insert("broken.scl", b"Broken\n not-a-number\n")
            .expect_err("should reject");
        assert!(error.to_string().contains("broken.scl"), "{error}");
    }

    #[test]
    fn decodes_latin1_description_bytes() {
        // 0xE9 is latin-1 "e-acute"; the Scala format is defined as latin-1.
        let scale = ScalaScale::parse_bytes(b"Caf\xe9\n 1\n 2/1\n").unwrap();
        assert_eq!(scale.description(), "Caf\u{e9}");
    }
    #[test]
    fn round_trips_through_from_str() {
        let scale: ScalaScale = FIFTH_AND_OCTAVE.parse().unwrap();
        assert_eq!(scale.len(), 2);
    }
}
