//! Moments of symmetry — the scales a single generator makes.
//!
//! Stack one interval inside a period, fold everything back into that period,
//! and for certain numbers of notes the result comes out with exactly two step
//! sizes. That is a *moment of symmetry*, and the diatonic scale is the famous
//! one: seven fifths folded into an octave give five whole tones and two
//! semitones, `5L 2s`. Stop at five notes instead and it is the pentatonic,
//! `2L 3s`; carry on to twelve and every step is the same, which is where the
//! piano comes from.
//!
//! The pattern and the tuning are separate things here. [`Mos`] is the pattern
//! alone — how many large steps and how many small — and says nothing about
//! how wide either is. [`MosScale`] is a generator and a period in cents with
//! a note count, and answers in cents; asking it for its [`MosScale::pattern`]
//! is what connects the two.

use crate::defaults::{FloatType, UnsignedIntegerType};
use crate::error::{Error, Result};

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// How near two steps must be, in cents, to count as the same size.
///
/// Generous next to the floating-point error of folding a generator into a
/// period, and far under any step distinction a scale actually makes.
const STEP_TOLERANCE: FloatType = 1e-6;

/// The width of the octave, in cents, which is the usual period.
pub const OCTAVE_CENTS: FloatType = 1200.0;

/// How many large and small steps a moment of symmetry has.
///
/// This is the pattern with no tuning attached — `5L 2s` is the diatonic
/// scale whether its whole tone is 200 cents or 193, and `2L 5s` is mavila's
/// anti-diatonic, where the pattern is the same shape but the two step sizes
/// have traded places.
///
/// ```
/// use music21_rs::tuningsystem::Mos;
///
/// let diatonic: Mos = "5L 2s".parse()?;
/// assert_eq!(diatonic.notes(), 7);
/// assert_eq!(diatonic.brightest_word(), "LLLsLLs");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Mos {
    large: UnsignedIntegerType,
    small: UnsignedIntegerType,
}

impl Mos {
    /// Builds a pattern of `large` large steps and `small` small ones.
    ///
    /// Errors when neither kind is present, since a scale with no steps at all
    /// is not a scale.
    pub fn new(large: UnsignedIntegerType, small: UnsignedIntegerType) -> Result<Self> {
        if large == 0 && small == 0 {
            return Err(Error::TuningSystem(
                "a moment of symmetry needs at least one step".to_owned(),
            ));
        }
        Ok(Self { large, small })
    }

    /// How many large steps the pattern has.
    #[must_use]
    pub fn large(self) -> UnsignedIntegerType {
        self.large
    }

    /// How many small steps the pattern has.
    #[must_use]
    pub fn small(self) -> UnsignedIntegerType {
        self.small
    }

    /// How many notes there are to a period, which is every step counted.
    #[must_use]
    pub fn notes(self) -> UnsignedIntegerType {
        self.large + self.small
    }

    /// The pattern with its two step sizes traded.
    ///
    /// `5L 2s` is the diatonic scale and `2L 5s` is mavila's anti-diatonic:
    /// the same seven notes generated the same way, with the fifth flat enough
    /// that what was the large step became the small one.
    pub fn inverted(self) -> Self {
        Self {
            large: self.small,
            small: self.large,
        }
    }

    /// The brightest mode's step word, `L` for a large step and `s` for a small.
    ///
    /// Brightest is the mode whose large steps come earliest, which for `5L 2s`
    /// is Lydian — `LLLsLLs`, the diatonic scale from `F` with no accidentals.
    #[must_use]
    pub fn brightest_word(self) -> String {
        let notes = self.notes();
        // The upper mechanical word of slope large/notes, which is the mode
        // with its large steps as early as they will go.
        (0..notes)
            .map(|position| {
                let before = ceiling_ratio(position * self.large, notes);
                let after = ceiling_ratio((position + 1) * self.large, notes);
                if after > before { 'L' } else { 's' }
            })
            .collect()
    }
}

/// `numerator / denominator`, rounded up, in whole numbers.
fn ceiling_ratio(numerator: UnsignedIntegerType, denominator: UnsignedIntegerType) -> u64 {
    let numerator = u64::from(numerator);
    let denominator = u64::from(denominator);
    numerator.div_ceil(denominator)
}

impl Display for Mos {
    /// Writes the pattern the way the literature does, `5L 2s`.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}L {}s", self.large, self.small)
    }
}

impl FromStr for Mos {
    type Err = Error;

    /// Reads `5L 2s`, with the space optional.
    fn from_str(text: &str) -> Result<Self> {
        let malformed =
            || Error::TuningSystem(format!("{text} is not a pattern; a pattern reads 5L 2s"));
        let (large, rest) = text
            .trim()
            .split_once('L')
            .ok_or_else(malformed)
            .map(|(large, rest)| (large.trim(), rest.trim()))?;
        let small = rest.strip_suffix('s').ok_or_else(malformed)?.trim();
        Self::new(
            large.parse().map_err(|_| malformed())?,
            small.parse().map_err(|_| malformed())?,
        )
    }
}

/// A generator and a period, folded into a scale of so many notes.
///
/// The generator and period are cents, so nothing here assumes an octave:
/// Bohlen-Pierce generates inside a period of `1901.955` cents and works the
/// same way.
///
/// ```
/// use music21_rs::tuningsystem::{MosScale, OCTAVE_CENTS};
///
/// // Seven fifths folded into an octave are the diatonic scale. Stacked
/// // upwards from the tonic they land in its brightest mode, Lydian.
/// let diatonic = MosScale::new(701.955, OCTAVE_CENTS, 7)?;
/// assert_eq!(diatonic.pattern()?.to_string(), "5L 2s");
/// assert_eq!(diatonic.word()?, "LLLsLLs");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct MosScale {
    generator: FloatType,
    period: FloatType,
    notes: UnsignedIntegerType,
}

impl MosScale {
    /// Builds a scale of `notes` notes from a generator inside a period.
    ///
    /// The generator is folded into the period, so handing over a fifth of
    /// `701.955` and a fifth of `1901.955` build the same scale. Errors on a
    /// period that is not positive, on a generator that folds to nothing, and
    /// on a scale of no notes.
    pub fn new(
        generator: FloatType,
        period: FloatType,
        notes: UnsignedIntegerType,
    ) -> Result<Self> {
        if !period.is_finite() || period <= 0.0 {
            return Err(Error::TuningSystem(format!(
                "{period} cents is not a period a scale can be generated in"
            )));
        }
        if !generator.is_finite() {
            return Err(Error::TuningSystem(
                "a generator has to be a real number of cents".to_owned(),
            ));
        }
        if notes == 0 {
            return Err(Error::TuningSystem(
                "a scale of no notes is not a scale".to_owned(),
            ));
        }
        let folded = generator.rem_euclid(period);
        if folded < STEP_TOLERANCE || period - folded < STEP_TOLERANCE {
            return Err(Error::TuningSystem(format!(
                "a generator of {generator} cents folds to the period itself and generates nothing"
            )));
        }
        Ok(Self {
            generator: folded,
            period,
            notes,
        })
    }

    /// Builds a scale whose generator is `steps` of an equal division.
    ///
    /// This is the wiki's `3\22` — three steps of 22 equal divisions of the
    /// period — which is how a generator is usually named once a temperament
    /// has been pinned to an equal temperament that supports it.
    pub fn from_equal_division(
        steps: UnsignedIntegerType,
        divisions: UnsignedIntegerType,
        period: FloatType,
        notes: UnsignedIntegerType,
    ) -> Result<Self> {
        if divisions == 0 {
            return Err(Error::TuningSystem(
                "a period cannot be divided into no steps".to_owned(),
            ));
        }
        let generator = period * FloatType::from(steps) / FloatType::from(divisions);
        Self::new(generator, period, notes)
    }

    /// The generator, in cents, folded into the period.
    #[must_use]
    pub fn generator(self) -> FloatType {
        self.generator
    }

    /// The period, in cents.
    #[must_use]
    pub fn period(self) -> FloatType {
        self.period
    }

    /// How many notes there are to a period.
    #[must_use]
    pub fn notes(self) -> UnsignedIntegerType {
        self.notes
    }

    /// The scale's degrees in cents, rising from nought inside one period.
    #[must_use]
    pub fn degrees(self) -> Vec<FloatType> {
        let mut degrees = (0..self.notes)
            .map(|index| (FloatType::from(index) * self.generator).rem_euclid(self.period))
            .collect::<Vec<_>>();
        degrees.sort_by(|left, right| left.partial_cmp(right).expect("cents are real numbers"));
        degrees
    }

    /// The width of each step in cents, from the tonic round to the period.
    #[must_use]
    pub fn steps(self) -> Vec<FloatType> {
        let degrees = self.degrees();
        degrees
            .iter()
            .zip(degrees.iter().skip(1))
            .map(|(lower, higher)| higher - lower)
            .chain(std::iter::once(
                self.period - degrees.last().copied().unwrap_or(0.0),
            ))
            .collect()
    }

    /// The distinct step widths, largest first.
    fn step_sizes(self) -> Vec<FloatType> {
        let mut sizes: Vec<FloatType> = Vec::new();
        for step in self.steps() {
            if !sizes
                .iter()
                .any(|known| (known - step).abs() < STEP_TOLERANCE)
            {
                sizes.push(step);
            }
        }
        sizes.sort_by(|left, right| right.partial_cmp(left).expect("cents are real numbers"));
        sizes
    }

    /// Whether the scale really is a moment of symmetry — two step sizes, no more.
    #[must_use]
    pub fn is_moment_of_symmetry(self) -> bool {
        self.step_sizes().len() == 2
    }

    /// Whether every step came out the same width, which is an equal division.
    #[must_use]
    pub fn is_equal(self) -> bool {
        self.step_sizes().len() == 1
    }

    /// How many large and small steps the scale has.
    ///
    /// Errors when the scale is not a moment of symmetry at all: either every
    /// step is the same width, which is an equal division, or there are three
    /// widths or more, which is the note count between two moments.
    pub fn pattern(self) -> Result<Mos> {
        let sizes = self.step_sizes();
        let [large, small] = sizes[..] else {
            return Err(Error::TuningSystem(format!(
                "{} notes of a generator of {:.3} cents give {} step sizes, not two",
                self.notes,
                self.generator,
                sizes.len()
            )));
        };
        let count = |width: FloatType| {
            self.steps()
                .iter()
                .filter(|step| (*step - width).abs() < STEP_TOLERANCE)
                .count() as UnsignedIntegerType
        };
        Mos::new(count(large), count(small))
    }

    /// The scale's own step word, from its tonic rather than from its brightest mode.
    ///
    /// Errors where [`MosScale::pattern`] does.
    pub fn word(self) -> Result<String> {
        let sizes = self.step_sizes();
        let [large, _] = sizes[..] else {
            let _ = self.pattern()?;
            unreachable!("pattern rejects anything but two step sizes");
        };
        Ok(self
            .steps()
            .iter()
            .map(|step| {
                if (step - large).abs() < STEP_TOLERANCE {
                    'L'
                } else {
                    's'
                }
            })
            .collect())
    }
}

/// Every note count up to `most` at which a generator makes a moment of symmetry.
///
/// This is the list a temperament page gives under "MOS scales": for the fifth
/// it is 2, 3, 4, 5, 7, 12, 17… — the pentatonic and the diatonic among them.
/// A count where every step comes out the same width is left out, since an
/// equal division is not a moment of symmetry.
pub fn moment_of_symmetry_sizes(
    generator: FloatType,
    period: FloatType,
    most: UnsignedIntegerType,
) -> Result<Vec<UnsignedIntegerType>> {
    // Built once so a bad generator or period is an error rather than a silence.
    let _ = MosScale::new(generator, period, 1)?;
    Ok((2..=most)
        .filter(|&notes| {
            MosScale::new(generator, period, notes).is_ok_and(MosScale::is_moment_of_symmetry)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_is_read_from_its_name() {
        let diatonic: Mos = "5L 2s".parse().unwrap();
        assert_eq!(diatonic.notes(), 7);
        assert_eq!("2L5s".parse::<Mos>().unwrap().notes(), 7);
        assert!("5L".parse::<Mos>().is_err());
        assert!("xL 2s".parse::<Mos>().is_err());
    }

    #[test]
    fn the_diatonic_scale_is_five_large_steps_and_two_small() {
        let diatonic = MosScale::new(701.955, OCTAVE_CENTS, 7).expect("a fifth");
        assert!(diatonic.is_moment_of_symmetry());
        assert_eq!(diatonic.pattern().expect("a moment").to_string(), "5L 2s");
        // Stacked upwards from the tonic the fifths land in Lydian, the mode
        // with its large steps as early as they go.
        assert_eq!(diatonic.word().expect("a moment"), "LLLsLLs");
        assert_eq!(diatonic.degrees().len(), 7);
        let steps = diatonic.steps();
        assert!((steps[0] - 203.910).abs() < 1e-3, "{steps:?}");
        assert!((steps[3] - 90.225).abs() < 1e-3, "{steps:?}");
        assert!((steps.iter().sum::<FloatType>() - OCTAVE_CENTS).abs() < 1e-9);
    }

    #[test]
    fn the_pentatonic_is_the_moment_before_the_diatonic() {
        let pentatonic = MosScale::new(701.955, OCTAVE_CENTS, 5).expect("a fifth");
        assert_eq!(pentatonic.pattern().expect("a moment").to_string(), "2L 3s");
    }

    #[test]
    fn twelve_fifths_come_out_equal_and_so_are_not_a_moment() {
        let chromatic = MosScale::new(700.0, OCTAVE_CENTS, 12).expect("a fifth");
        assert!(chromatic.is_equal());
        assert!(!chromatic.is_moment_of_symmetry());
        assert!(chromatic.pattern().is_err());
    }

    #[test]
    fn a_count_between_two_moments_has_three_step_sizes() {
        let six = MosScale::new(701.955, OCTAVE_CENTS, 6).expect("a fifth");
        assert!(!six.is_moment_of_symmetry());
        assert!(six.pattern().is_err());
    }

    /// Porcupine, mavila and blackwood, each read off the equal temperament
    /// the wiki names their generator in.
    #[test]
    fn named_temperaments_give_the_patterns_they_are_listed_with() {
        for (steps, divisions, notes, pattern) in [
            (3, 22, 7, "1L 6s"),  // porcupine[7], generator 3\22
            (3, 22, 8, "7L 1s"),  // porcupine[8]
            (9, 16, 7, "2L 5s"),  // mavila[7], the anti-diatonic, generator 9\16
            (9, 16, 9, "7L 2s"),  // mavila[9]
            (13, 23, 7, "2L 5s"), // mavila again, its fifth read in 23edo
            (7, 12, 7, "5L 2s"),  // the diatonic scale in 12edo
            (7, 12, 5, "2L 3s"),  // the pentatonic in 12edo
            (18, 31, 7, "5L 2s"), // meantone's fifth in 31edo
            // A generator that is not a fifth at all says so rather than
            // being forced into the pattern of one.
            (5, 17, 7, "3L 4s"),
        ] {
            let scale = MosScale::from_equal_division(steps, divisions, OCTAVE_CENTS, notes)
                .expect("a generator");
            assert_eq!(
                scale.pattern().expect("a moment").to_string(),
                pattern,
                "{steps}\\{divisions} at {notes} notes"
            );
        }
    }

    #[test]
    fn a_pattern_writes_and_reads_the_way_it_is_named() {
        let diatonic = Mos::new(5, 2).expect("a pattern");
        assert_eq!(diatonic.to_string(), "5L 2s");
        assert_eq!("5L 2s".parse::<Mos>().expect("a pattern"), diatonic);
        assert_eq!("5L2s".parse::<Mos>().expect("a pattern"), diatonic);
        assert_eq!(diatonic.notes(), 7);
        assert_eq!(diatonic.inverted().to_string(), "2L 5s");
        assert!("5L".parse::<Mos>().is_err());
        assert!("five L 2s".parse::<Mos>().is_err());
        assert!(Mos::new(0, 0).is_err());
    }

    /// The brightest mode of the diatonic is Lydian, which is where the word starts.
    #[test]
    fn the_brightest_mode_puts_its_large_steps_first() {
        for (pattern, word) in [
            ("5L 2s", "LLLsLLs"),
            ("2L 5s", "LssLsss"),
            ("1L 6s", "Lssssss"),
            ("7L 1s", "LLLLLLLs"),
            ("2L 3s", "LsLss"),
            ("3L 4s", "LsLsLss"),
        ] {
            let mos: Mos = pattern.parse().expect("a pattern");
            assert_eq!(mos.brightest_word(), word, "{pattern}");
            assert_eq!(mos.brightest_word().len(), mos.notes() as usize);
        }
    }

    #[test]
    fn the_moments_of_a_fifth_are_the_ones_the_wiki_lists() {
        // Four fifths give three step sizes, so four notes is not among them —
        // the moments are the two familiar scales and the counts around them.
        let sizes = moment_of_symmetry_sizes(701.955, OCTAVE_CENTS, 12).expect("a fifth");
        assert_eq!(sizes, vec![2, 3, 5, 7, 12]);
    }

    /// Nothing here assumes an octave: Bohlen-Pierce generates in a twelfth.
    #[test]
    fn a_period_need_not_be_an_octave() {
        let tritave = 1200.0 * 3.0_f64.log2();
        let lambda = MosScale::from_equal_division(3, 13, tritave, 9).expect("a generator");
        assert_eq!(lambda.pattern().expect("a moment").to_string(), "4L 5s");
        assert!((lambda.period() - 1901.955).abs() < 1e-3);
        assert!((lambda.steps().iter().sum::<FloatType>() - tritave).abs() < 1e-9);
    }

    #[test]
    fn a_generator_that_folds_to_the_period_generates_nothing() {
        assert!(MosScale::new(1200.0, OCTAVE_CENTS, 7).is_err());
        assert!(MosScale::new(0.0, OCTAVE_CENTS, 7).is_err());
        assert!(MosScale::new(701.955, 0.0, 7).is_err());
        assert!(MosScale::new(701.955, OCTAVE_CENTS, 0).is_err());
        assert!(MosScale::from_equal_division(3, 0, OCTAVE_CENTS, 7).is_err());
        assert!(moment_of_symmetry_sizes(0.0, OCTAVE_CENTS, 12).is_err());
    }

    #[test]
    fn a_generator_is_folded_into_its_period() {
        let fifth = MosScale::new(701.955, OCTAVE_CENTS, 7).expect("a fifth");
        let twelfth = MosScale::new(1901.955, OCTAVE_CENTS, 7).expect("a twelfth");
        assert!((fifth.generator() - twelfth.generator()).abs() < 1e-9);
        assert_eq!(
            fifth.pattern().expect("a moment"),
            twelfth.pattern().expect("a moment")
        );
    }
}
