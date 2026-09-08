//! Monzos and vals, the two vectors regular temperament theory is written in.
//!
//! A *monzo* is an interval written as the exponents of the primes that
//! multiply out to its ratio, so the syntonic comma `81/80` is
//! `2^-4 * 3^4 * 5^-1` and reads `[-4 4 -1⟩`. A *val* is the other side of the
//! same coin: a map from those primes to how many steps of some tuning each
//! one is worth, so twelve-tone equal temperament is `⟨12 19 28]` — twelve
//! steps to the octave, nineteen to the twelfth, twenty-eight to the
//! seventeenth. Pairing the two says how wide an interval comes out in a
//! tuning, and a val that maps a comma to nothing is a tuning that *tempers it
//! out*: `⟨12 19 28]` sends `[-4 4 -1⟩` to zero, which is why four fifths and
//! a major third are the same note on a piano.

use crate::defaults::{FloatType, FractionType, IntegerType};
use crate::error::{Error, Result};

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// The primes a [`Monzo`] or [`Val`] is written over, in order.
///
/// The vectors are positional — an entry means the prime standing at its
/// index — so this list is what fixes what a monzo says. Twenty-five primes
/// reach the 97-limit, well past anything regular temperament practice uses.
pub const PRIMES: [IntegerType; 25] = [
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
];

/// The index of `prime` in [`PRIMES`], or `None` if it is not one of them.
fn prime_index(prime: IntegerType) -> Option<usize> {
    PRIMES.iter().position(|&candidate| candidate == prime)
}

/// How many entries a monzo or val needs to reach `limit`.
fn entries_for_limit(limit: IntegerType) -> Result<usize> {
    prime_index(limit).map(|index| index + 1).ok_or_else(|| {
        Error::TuningSystem(format!(
            "{limit} is not a prime this module carries; the largest is {}",
            PRIMES[PRIMES.len() - 1]
        ))
    })
}

/// Trailing zeros say nothing, so a monzo is stored without them.
fn trimmed(mut entries: Vec<IntegerType>) -> Vec<IntegerType> {
    while entries.last() == Some(&0) {
        let _ = entries.pop();
    }
    entries
}

/// Parses the numbers between a vector's brackets.
fn parse_entries(body: &str) -> Result<Vec<IntegerType>> {
    body.split([' ', ',', '\t'])
        .filter(|piece| !piece.is_empty())
        .map(|piece| {
            piece.parse::<IntegerType>().map_err(|_| {
                Error::TuningSystem(format!("{piece} is not a whole number in a monzo or val"))
            })
        })
        .collect()
}

/// How many times `prime` divides `value`.
fn factor_out(mut value: IntegerType, prime: IntegerType) -> IntegerType {
    let mut count = 0;
    while value % prime == 0 {
        value /= prime;
        count += 1;
    }
    count
}

/// What is left of `value` once every prime in [`PRIMES`] is divided out.
fn strip_primes(mut value: IntegerType) -> IntegerType {
    for &prime in &PRIMES {
        while value % prime == 0 {
            value /= prime;
        }
    }
    value
}

/// An interval written as the exponents of the primes making up its ratio.
///
/// The entries are positional over [`PRIMES`], so `[-4 4 -1⟩` is
/// `2^-4 * 3^4 * 5^-1`, the syntonic comma. Multiplying two intervals adds
/// their monzos, which is the whole reason for writing intervals this way.
///
/// ```
/// use music21_rs::tuningsystem::Monzo;
///
/// let comma = Monzo::from_ratio(81, 80)?;
/// assert_eq!(comma.exponents(), [-4, 4, -1]);
/// assert_eq!(comma.limit(), Some(5));
/// assert_eq!(comma.to_string(), "[-4 4 -1⟩");
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Monzo {
    exponents: Vec<IntegerType>,
}

impl Monzo {
    /// Builds a monzo from prime exponents, lowest prime first.
    pub fn new(exponents: impl Into<Vec<IntegerType>>) -> Self {
        Self {
            exponents: trimmed(exponents.into()),
        }
    }

    /// The unison, whose every exponent is nought.
    pub fn unison() -> Self {
        Self::default()
    }

    /// Factors a ratio into prime exponents.
    ///
    /// Errors on a ratio that is not positive, or one carrying a prime factor
    /// larger than [`PRIMES`] reaches.
    pub fn from_ratio(numerator: IntegerType, denominator: IntegerType) -> Result<Self> {
        if numerator <= 0 || denominator <= 0 {
            return Err(Error::TuningSystem(format!(
                "{numerator}/{denominator} is not a positive ratio"
            )));
        }
        if strip_primes(numerator) * strip_primes(denominator) != 1 {
            return Err(Error::TuningSystem(format!(
                "{numerator}/{denominator} has a prime factor past the {}-limit",
                PRIMES[PRIMES.len() - 1]
            )));
        }
        let exponents = PRIMES
            .iter()
            .map(|&prime| factor_out(numerator, prime) - factor_out(denominator, prime))
            .collect::<Vec<_>>();
        Ok(Self::new(exponents))
    }

    /// Factors a [`FractionType`] into prime exponents.
    pub fn from_fraction(ratio: FractionType) -> Result<Self> {
        match (ratio.numer(), ratio.denom()) {
            (Some(&numerator), Some(&denominator)) => Self::from_ratio(numerator, denominator),
            _ => Err(Error::TuningSystem(
                "an infinite or undefined ratio has no monzo".to_owned(),
            )),
        }
    }

    /// The prime exponents, lowest prime first, without trailing zeros.
    #[must_use]
    pub fn exponents(&self) -> &[IntegerType] {
        &self.exponents
    }

    /// The exponent of `prime`, which is nought for a prime not written.
    ///
    /// Errors only for a number that is not a prime this module carries.
    pub fn exponent_of(&self, prime: IntegerType) -> Result<IntegerType> {
        let index = prime_index(prime).ok_or_else(|| {
            Error::TuningSystem(format!("{prime} is not a prime this module carries"))
        })?;
        Ok(self.at(index))
    }

    /// The largest prime the monzo actually uses, or `None` for the unison.
    #[must_use]
    pub fn limit(&self) -> Option<IntegerType> {
        self.exponents
            .iter()
            .rposition(|&exponent| exponent != 0)
            .map(|index| PRIMES[index])
    }

    /// Whether every exponent is nought.
    #[must_use]
    pub fn is_unison(&self) -> bool {
        self.exponents.is_empty()
    }

    /// The ratio the exponents multiply out to.
    ///
    /// Errors when the ratio does not fit an [`IntegerType`], which a stack of
    /// any size will reach — `[-4 4 -1⟩` is `81/80`, but twelve of them are not
    /// a fraction of two `i32`s.
    pub fn ratio(&self) -> Result<FractionType> {
        let mut numerator: IntegerType = 1;
        let mut denominator: IntegerType = 1;
        for (index, &exponent) in self.exponents.iter().enumerate() {
            let prime = PRIMES[index];
            let magnitude = exponent.unsigned_abs();
            let power = prime.checked_pow(magnitude).ok_or_else(|| {
                Error::TuningSystem(format!("{prime}^{magnitude} does not fit the ratio type"))
            })?;
            let side = if exponent >= 0 {
                &mut numerator
            } else {
                &mut denominator
            };
            *side = side
                .checked_mul(power)
                .ok_or_else(|| Error::TuningSystem("the ratio does not fit its type".to_owned()))?;
        }
        Ok(FractionType::new(numerator, denominator))
    }

    /// How wide the interval is, in cents.
    #[must_use]
    pub fn cents(&self) -> FloatType {
        1200.0
            * self
                .exponents
                .iter()
                .enumerate()
                .map(|(index, &exponent)| {
                    FloatType::from(exponent) * FloatType::from(PRIMES[index]).log2()
                })
                .sum::<FloatType>()
    }

    /// The interval reached by sounding both, which adds the exponents.
    pub fn multiply(&self, other: &Self) -> Self {
        let width = self.exponents.len().max(other.exponents.len());
        Self::new(
            (0..width)
                .map(|index| self.at(index) + other.at(index))
                .collect::<Vec<_>>(),
        )
    }

    /// The interval left by taking `other` off this one.
    pub fn divide(&self, other: &Self) -> Self {
        self.multiply(&other.inverse())
    }

    /// The same interval measured the other way.
    pub fn inverse(&self) -> Self {
        Self::new(
            self.exponents
                .iter()
                .map(|exponent| -exponent)
                .collect::<Vec<_>>(),
        )
    }

    /// The interval stacked `count` times.
    pub fn pow(&self, count: IntegerType) -> Self {
        Self::new(
            self.exponents
                .iter()
                .map(|exponent| exponent * count)
                .collect::<Vec<_>>(),
        )
    }

    /// The exponent at `index`, nought past the end.
    fn at(&self, index: usize) -> IntegerType {
        self.exponents.get(index).copied().unwrap_or(0)
    }
}

impl Display for Monzo {
    /// Writes the monzo in the wiki's own notation, `[-4 4 -1⟩`.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for (position, exponent) in self.exponents.iter().enumerate() {
            if position > 0 {
                write!(f, " ")?;
            }
            write!(f, "{exponent}")?;
        }
        write!(f, "⟩")
    }
}

impl FromStr for Monzo {
    type Err = Error;

    /// Reads `[-4 4 -1⟩`, and the ASCII spelling `|-4 4 -1>` beside it.
    fn from_str(text: &str) -> Result<Self> {
        let text = text.trim();
        let body = text
            .strip_prefix('[')
            .or_else(|| text.strip_prefix('|'))
            .and_then(|rest| rest.strip_suffix('⟩').or_else(|| rest.strip_suffix('>')))
            .ok_or_else(|| {
                Error::TuningSystem(format!("{text} is not a monzo; a monzo reads [-4 4 -1⟩"))
            })?;
        Ok(Self::new(parse_entries(body)?))
    }
}

/// How many steps of a tuning each prime is worth.
///
/// `⟨12 19 28]` is twelve-tone equal temperament read to the 5-limit: twelve
/// steps to the octave, nineteen to the perfect twelfth, twenty-eight to the
/// major seventeenth. Applying it to a [`Monzo`] says how many steps that
/// interval comes to, and a comma it sends to nothing is a comma the tuning
/// tempers out.
///
/// ```
/// use music21_rs::tuningsystem::{Monzo, Val};
///
/// let twelve = Val::patent(12, 5)?;
/// assert_eq!(twelve.entries(), [12, 19, 28]);
/// assert_eq!(twelve.map(&Monzo::from_ratio(3, 2)?), 7);
/// assert!(twelve.tempers_out(&Monzo::from_ratio(81, 80)?));
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Val {
    entries: Vec<IntegerType>,
}

impl Val {
    /// Builds a val from step counts, lowest prime first.
    ///
    /// Trailing zeros are kept, unlike a monzo's: a val saying the seventh
    /// harmonic is worth no steps at all has said something about it, while a
    /// val that stops before it has not.
    pub fn new(entries: impl Into<Vec<IntegerType>>) -> Self {
        Self {
            entries: entries.into(),
        }
    }

    /// The patent val of `divisions` equal steps, read up to `limit`.
    ///
    /// Each prime is mapped to the nearest whole number of steps, which is what
    /// *patent* means: the obvious reading, before anyone chooses a warped one.
    /// Errors for a limit that is not a prime this module carries.
    pub fn patent(divisions: IntegerType, limit: IntegerType) -> Result<Self> {
        let width = entries_for_limit(limit)?;
        Ok(Self::new(
            PRIMES[..width]
                .iter()
                .map(|&prime| {
                    (FloatType::from(divisions) * FloatType::from(prime).log2()).round()
                        as IntegerType
                })
                .collect::<Vec<_>>(),
        ))
    }

    /// The step counts, lowest prime first.
    #[must_use]
    pub fn entries(&self) -> &[IntegerType] {
        &self.entries
    }

    /// How many steps the val makes `interval` come to.
    ///
    /// A prime the val does not reach is read as nought steps, so ask
    /// [`Val::reaches`] first where that would be a lie rather than an answer.
    #[must_use]
    pub fn map(&self, interval: &Monzo) -> IntegerType {
        interval
            .exponents()
            .iter()
            .enumerate()
            .map(|(index, &exponent)| exponent * self.entries.get(index).copied().unwrap_or(0))
            .sum()
    }

    /// Whether the val says anything about every prime `interval` uses.
    #[must_use]
    pub fn reaches(&self, interval: &Monzo) -> bool {
        interval.exponents().len() <= self.entries.len()
    }

    /// Whether the tuning tempers `comma` out — maps it to no steps at all.
    #[must_use]
    pub fn tempers_out(&self, comma: &Monzo) -> bool {
        self.reaches(comma) && self.map(comma) == 0
    }

    /// How many steps the val divides the octave into.
    ///
    /// That is its first entry, since the first prime is two.
    #[must_use]
    pub fn divisions(&self) -> IntegerType {
        self.entries.first().copied().unwrap_or(0)
    }

    /// The largest prime the val reaches, or `None` for an empty one.
    #[must_use]
    pub fn limit(&self) -> Option<IntegerType> {
        self.entries
            .len()
            .checked_sub(1)
            .and_then(|index| PRIMES.get(index).copied())
    }

    /// How far off `interval` sounds in this tuning, in cents.
    ///
    /// Positive is sharp of just. The step size comes from the val's own
    /// octave, so a val mapping the octave to nothing has no answer and this is
    /// an error.
    pub fn error_cents(&self, interval: &Monzo) -> Result<FloatType> {
        let divisions = self.divisions();
        if divisions == 0 {
            return Err(Error::TuningSystem(
                "a val mapping the octave to no steps has no step size".to_owned(),
            ));
        }
        let step = 1200.0 / FloatType::from(divisions);
        Ok(FloatType::from(self.map(interval)) * step - interval.cents())
    }
}

impl Display for Val {
    /// Writes the val in the wiki's own notation, `⟨12 19 28]`.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "⟨")?;
        for (position, entry) in self.entries.iter().enumerate() {
            if position > 0 {
                write!(f, " ")?;
            }
            write!(f, "{entry}")?;
        }
        write!(f, "]")
    }
}

impl FromStr for Val {
    type Err = Error;

    /// Reads `⟨12 19 28]`, and the ASCII spelling `<12 19 28]` beside it.
    fn from_str(text: &str) -> Result<Self> {
        let text = text.trim();
        let body = text
            .strip_prefix('⟨')
            .or_else(|| text.strip_prefix('<'))
            .and_then(|rest| rest.strip_suffix(']'))
            .ok_or_else(|| {
                Error::TuningSystem(format!("{text} is not a val; a val reads ⟨12 19 28]"))
            })?;
        Ok(Self::new(parse_entries(body)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ratio the wiki writes a monzo for, factored and written back.
    #[test]
    fn monzos_factor_the_commas_they_are_named_for() {
        for (numerator, denominator, exponents) in [
            (81, 80, vec![-4, 4, -1]),         // syntonic comma
            (531_441, 524_288, vec![-19, 12]), // Pythagorean comma
            (128, 125, vec![7, 0, -3]),        // diesis
            (2048, 2025, vec![11, -4, -2]),    // diaschisma
            (64, 63, vec![6, -2, 0, -1]),      // septimal comma
            (225, 224, vec![-5, 2, 2, -1]),    // marvel comma
            (2, 1, vec![1]),                   // the octave itself
            (3, 2, vec![-1, 1]),               // the fifth
            (5, 4, vec![-2, 0, 1]),            // the just major third
        ] {
            let monzo = Monzo::from_ratio(numerator, denominator).expect("a factorable ratio");
            assert_eq!(monzo.exponents(), exponents, "{numerator}/{denominator}");
            assert_eq!(
                monzo.ratio().expect("a ratio that fits"),
                FractionType::new(numerator, denominator),
                "{numerator}/{denominator} did not come back"
            );
        }
    }

    #[test]
    fn a_monzos_cents_is_the_width_of_its_ratio() {
        let comma = Monzo::from_ratio(81, 80).expect("the syntonic comma");
        assert!((comma.cents() - 21.506_29).abs() < 1e-5);
        let fifth = Monzo::from_ratio(3, 2).expect("the fifth");
        assert!((fifth.cents() - 701.955_00).abs() < 1e-5);
        assert_eq!(Monzo::unison().cents(), 0.0);
    }

    #[test]
    fn stacking_intervals_adds_their_monzos() {
        let fifth = Monzo::from_ratio(3, 2).expect("the fifth");
        let octave = Monzo::from_ratio(2, 1).expect("the octave");
        // Four fifths less two octaves and a just third is the syntonic comma.
        let third = Monzo::from_ratio(5, 4).expect("the third");
        let comma = fifth.pow(4).divide(&octave.pow(2)).divide(&third);
        assert_eq!(comma, Monzo::from_ratio(81, 80).expect("the comma"));
        // Twelve fifths less seven octaves is the Pythagorean comma.
        let pythagorean = fifth.pow(12).divide(&octave.pow(7));
        assert_eq!(
            pythagorean,
            Monzo::from_ratio(531_441, 524_288).expect("the comma")
        );
    }

    #[test]
    fn a_monzo_writes_and_reads_the_wikis_notation() {
        let comma = Monzo::from_ratio(81, 80).expect("the syntonic comma");
        assert_eq!(comma.to_string(), "[-4 4 -1⟩");
        assert_eq!("[-4 4 -1⟩".parse::<Monzo>().expect("a monzo"), comma);
        assert_eq!("|-4 4 -1>".parse::<Monzo>().expect("a monzo"), comma);
        assert_eq!("[-4, 4, -1⟩".parse::<Monzo>().expect("a monzo"), comma);
        assert_eq!(Monzo::unison().to_string(), "[⟩");
        assert!("-4 4 -1".parse::<Monzo>().is_err());
        assert!("[-4 x -1⟩".parse::<Monzo>().is_err());
    }

    #[test]
    fn a_ratio_past_the_table_or_below_zero_has_no_monzo() {
        assert!(Monzo::from_ratio(101, 100).is_err());
        assert!(Monzo::from_ratio(-3, 2).is_err());
        assert!(Monzo::from_ratio(1, 0).is_err());
        assert!(Monzo::from_ratio(97, 89).is_ok());
    }

    /// The patent vals the wiki lists for the equal temperaments it uses.
    #[test]
    fn patent_vals_match_the_ones_the_wiki_lists() {
        for (divisions, limit, entries) in [
            (5, 5, vec![5, 8, 12]),
            (7, 5, vec![7, 11, 16]),
            (12, 5, vec![12, 19, 28]),
            (12, 7, vec![12, 19, 28, 34]),
            (19, 5, vec![19, 30, 44]),
            (22, 5, vec![22, 35, 51]),
            (31, 11, vec![31, 49, 72, 87, 107]),
            (41, 5, vec![41, 65, 95]),
            (53, 5, vec![53, 84, 123]),
            (72, 11, vec![72, 114, 167, 202, 249]),
        ] {
            let val = Val::patent(divisions, limit).expect("a prime limit");
            assert_eq!(
                val.entries(),
                entries,
                "{divisions}edo to the {limit}-limit"
            );
            assert_eq!(val.divisions(), divisions);
            assert_eq!(val.limit(), Some(limit));
        }
    }

    #[test]
    fn a_temperament_is_the_commas_its_val_sends_to_nothing() {
        let syntonic = Monzo::from_ratio(81, 80).expect("the syntonic comma");
        let pythagorean = Monzo::from_ratio(531_441, 524_288).expect("the Pythagorean comma");

        // Meantone tempers the syntonic comma out; 12, 19 and 31 are meantone.
        for divisions in [12, 19, 31] {
            let val = Val::patent(divisions, 5).expect("the 5-limit");
            assert!(val.tempers_out(&syntonic), "{divisions}edo is meantone");
        }
        // 22edo deliberately is not, which is the whole point of it.
        let twenty_two = Val::patent(22, 5).expect("the 5-limit");
        assert!(!twenty_two.tempers_out(&syntonic));

        // Only 12 of those tempers the Pythagorean comma out.
        let twelve = Val::patent(12, 5).expect("the 5-limit");
        assert!(twelve.tempers_out(&pythagorean));
        assert!(
            !Val::patent(19, 5)
                .expect("the 5-limit")
                .tempers_out(&pythagorean)
        );
    }

    #[test]
    fn a_val_says_nothing_about_a_prime_it_does_not_reach() {
        let five_limit = Val::patent(12, 5).expect("the 5-limit");
        let septimal = Monzo::from_ratio(7, 4).expect("the harmonic seventh");
        assert!(!five_limit.reaches(&septimal));
        assert!(!five_limit.tempers_out(&Monzo::from_ratio(64, 63).expect("the comma")));
        assert!(Val::patent(12, 7).expect("the 7-limit").reaches(&septimal));
    }

    #[test]
    fn error_cents_says_how_far_off_a_tuning_sounds() {
        let fifth = Monzo::from_ratio(3, 2).expect("the fifth");
        let twelve = Val::patent(12, 5).expect("the 5-limit");
        // 12edo's fifth is a bit under two cents flat of just.
        let error = twelve.error_cents(&fifth).expect("a step size");
        assert!((error + 1.955).abs() < 1e-3, "{error}");
        // Its major third is fourteen cents sharp, which is the famous one.
        let third = Monzo::from_ratio(5, 4).expect("the third");
        let error = twelve.error_cents(&third).expect("a step size");
        assert!((error - 13.686).abs() < 1e-3, "{error}");
        assert!(Val::new([0, 0]).error_cents(&fifth).is_err());
    }

    #[test]
    fn a_val_writes_and_reads_the_wikis_notation() {
        let twelve = Val::patent(12, 5).expect("the 5-limit");
        assert_eq!(twelve.to_string(), "⟨12 19 28]");
        assert_eq!("⟨12 19 28]".parse::<Val>().expect("a val"), twelve);
        assert_eq!("<12 19 28]".parse::<Val>().expect("a val"), twelve);
        assert!("12 19 28".parse::<Val>().is_err());
    }

    #[test]
    fn a_limit_that_is_not_a_prime_is_refused() {
        assert!(Val::patent(12, 9).is_err());
        assert!(Val::patent(12, 101).is_err());
        assert!(
            Monzo::from_ratio(9, 8)
                .expect("a ratio")
                .exponent_of(9)
                .is_err()
        );
    }
}
