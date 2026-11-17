//! music21's `meter.tools`: reading meters written as text, and the ways
//! music21 conventionally divides a meter.
//!
//! A meter is written `3/8`, `slow 6/8`, `3/8+2/8` or `3+2/8`, the last
//! sharing one denominator among the numerators before it. The division
//! options are what [`super::MeterTerminal::partition_options`] chooses
//! among, in music21's order of preference.

use num::BigInt;
use num::integer::Integer;

use crate::common::numbertools::{float_ratio, limit_denominator};
use crate::common::stringtools::get_num_from_str;
use crate::defaults::{FloatType, UnsignedIntegerType};

/// The denominators music21 will write a meter in, in order: music21's
/// `validDenominators`.
pub const VALID_DENOMINATORS: [UnsignedIntegerType; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

/// How a meter's beat is felt, from the word written before it: music21's
/// `MeterDivision`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum MeterDivision {
    /// `fast 6/8`: the beat is the dotted quarter.
    Fast,
    /// `slow 6/8`: the beat is the eighth.
    Slow,
    /// No word: the meter's own reading.
    #[default]
    None,
}

impl MeterDivision {
    /// The member's name in music21's enum: `FAST`, `SLOW` or `NONE`.
    #[must_use]
    pub fn music21_name(self) -> &'static str {
        match self {
            Self::Fast => "FAST",
            Self::Slow => "SLOW",
            Self::None => "NONE",
        }
    }
}

/// A meter read from text: music21's `MeterTerminalTuple`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SlashMeter {
    /// The number of units.
    pub numerator: UnsignedIntegerType,
    /// The unit, as a note value: `8` for eighths.
    pub denominator: UnsignedIntegerType,
    /// The word written before it, if any.
    pub division: MeterDivision,
}

/// Why a meter written as text could not be read, each the exception
/// music21 raises, with its message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlashError {
    /// No `n/d` in it, or a numerator with no denominator after it:
    /// music21's `MeterException`.
    Meter(String),
    /// A part with a slash that is not `n/d`: music21's
    /// `TimeSignatureException`.
    TimeSignature(String),
    /// A part with no slash that is not a number: music21's plain
    /// `Music21Exception`.
    Unreadable(String),
}

impl std::fmt::Display for SlashError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Meter(message) | Self::TimeSignature(message) | Self::Unreadable(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for SlashError {}

/// `n/d` at the start of `text`, both runs of ASCII digits, as music21's
/// `(\d+)/(\d+)` matches it.
fn leading_ratio(text: &str) -> Option<(UnsignedIntegerType, UnsignedIntegerType)> {
    let (numerator, rest) = text.split_at(text.find(|c: char| !c.is_ascii_digit())?);
    let rest = rest.strip_prefix('/')?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let denominator = &rest[..end];
    if numerator.is_empty() || denominator.is_empty() {
        return None;
    }
    Some((numerator.parse().ok()?, denominator.parse().ok()?))
}

/// One meter read from text, with the word before it that says how its
/// beat is felt: music21's `slashToTuple`.
///
/// ```
/// use music21::meter::tools::{MeterDivision, slash_to_tuple};
///
/// let read = slash_to_tuple("slow 6/8").unwrap();
/// assert_eq!((read.numerator, read.denominator), (6, 8));
/// assert_eq!(read.division, MeterDivision::Slow);
/// ```
///
/// # Errors
///
/// [`SlashError::Meter`] where there is no `n/d` to read.
pub fn slash_to_tuple(value: &str) -> Result<SlashMeter, SlashError> {
    let (numbers, words) = get_num_from_str(value, "0123456789/.");
    let words = words.to_lowercase();
    let division = if words.contains("slow") {
        MeterDivision::Slow
    } else if words.contains("fast") {
        MeterDivision::Fast
    } else {
        MeterDivision::None
    };
    let Some((numerator, denominator)) = leading_ratio(numbers.trim()) else {
        return Err(SlashError::Meter(format!(
            "slashToTuple() cannot find two part fraction for {value}"
        )));
    };
    Ok(SlashMeter {
        numerator,
        denominator,
        division,
    })
}

/// A compound meter as the ratio of each of its parts, leaving out a part
/// that is not one: music21's `slashCompoundToFraction`.
///
/// ```
/// use music21::meter::tools::slash_compound_to_fraction;
///
/// assert_eq!(slash_compound_to_fraction("3/8+2/4"), [(3, 8), (2, 4)]);
/// ```
#[must_use]
pub fn slash_compound_to_fraction(value: &str) -> Vec<(UnsignedIntegerType, UnsignedIntegerType)> {
    value
        .trim()
        .split('+')
        .filter_map(|part| slash_to_tuple(part).ok())
        .map(|read| (read.numerator, read.denominator))
        .collect()
}

/// A meter written with or without a denominator on each part as the ratio
/// of each part, and whether any numerator borrowed the next denominator,
/// as in `3+2/8`: music21's `slashMixedToFraction`.
///
/// ```
/// use music21::meter::tools::slash_mixed_to_fraction;
///
/// let (parts, summed) = slash_mixed_to_fraction("3+2/8+3/4").unwrap();
/// assert_eq!(parts, [(3, 8), (2, 8), (3, 4)]);
/// assert!(summed);
/// ```
///
/// # Errors
///
/// [`SlashError::TimeSignature`] for a part with a slash that is no ratio,
/// [`SlashError::Unreadable`] for a bare numerator that is no number, and
/// [`SlashError::Meter`] for a numerator with no denominator after it.
pub fn slash_mixed_to_fraction(
    value: &str,
) -> Result<(Vec<(UnsignedIntegerType, UnsignedIntegerType)>, bool), SlashError> {
    let mut read: Vec<(UnsignedIntegerType, Option<UnsignedIntegerType>)> = Vec::new();
    let mut summed = false;
    for part in value.trim().split('+') {
        if part.contains('/') {
            let ratio = slash_to_tuple(part).map_err(|_| {
                SlashError::TimeSignature(format!("Cannot create time signature from \"{value}\""))
            })?;
            read.push((ratio.numerator, Some(ratio.denominator)));
            continue;
        }
        let numerator = part.trim().parse().map_err(|_| {
            SlashError::Unreadable(format!(
                "Cannot parse this file -- this error often comes up if the musicxml \
                 pickled file is out of date after a change in musicxml/__init__.py . \
                 Clear your temp directory of .p and .p.gz files and try again. \
                 Time Signature: {value} "
            ))
        })?;
        read.push((numerator, None));
        summed = true;
    }

    let mut parts = Vec::with_capacity(read.len());
    for (index, (numerator, denominator)) in read.iter().enumerate() {
        // A bare numerator takes the first denominator after it.
        let denominator = denominator
            .or_else(|| read[index + 1..].iter().find_map(|(_, later)| *later))
            .ok_or_else(|| {
                SlashError::Meter(format!("cannot match denominator to numerator in: {value}"))
            })?;
        parts.push((*numerator, denominator));
    }
    Ok((parts, summed))
}

/// Ratios as a meter writes them, numerators run together where neighbours
/// share a denominator: music21's `fractionToSlashMixed`.
///
/// ```
/// use music21::meter::tools::fraction_to_slash_mixed;
///
/// let mixed = fraction_to_slash_mixed(&[(3, 8), (2, 8), (3, 4)]);
/// assert_eq!(mixed, [("3+2".to_string(), 8), ("3".to_string(), 4)]);
/// ```
#[must_use]
pub fn fraction_to_slash_mixed(
    fractions: &[(UnsignedIntegerType, UnsignedIntegerType)],
) -> Vec<(String, UnsignedIntegerType)> {
    let mut runs: Vec<(Vec<String>, UnsignedIntegerType)> = Vec::new();
    for (numerator, denominator) in fractions {
        match runs.last_mut() {
            Some((numerators, shared)) if shared == denominator => {
                numerators.push(numerator.to_string());
            }
            _ => runs.push((vec![numerator.to_string()], *denominator)),
        }
    }
    runs.into_iter()
        .map(|(numerators, denominator)| (numerators.join("+"), denominator))
        .collect()
}

/// The sum of ratios, not reduced, so that `3/8 + 3/8` is `6/8`: music21's
/// `fractionSum`. Unlike denominators meet at their least common multiple,
/// and nothing at all sums to `0/1`.
///
/// ```
/// use music21::meter::tools::fraction_sum;
/// use num::BigInt;
///
/// let big = |pairs: &[(i32, i32)]| -> Vec<(BigInt, BigInt)> {
///     pairs.iter().map(|(n, d)| (BigInt::from(*n), BigInt::from(*d))).collect()
/// };
/// assert_eq!(fraction_sum(&big(&[(3, 8), (3, 8)])), (6.into(), 8.into()));
/// assert_eq!(fraction_sum(&big(&[(1, 6), (2, 3)])), (5.into(), 6.into()));
/// ```
#[must_use]
pub fn fraction_sum(fractions: &[(BigInt, BigInt)]) -> (BigInt, BigInt) {
    let Some((_, first)) = fractions.first() else {
        return (BigInt::from(0), BigInt::from(1));
    };
    if fractions
        .iter()
        .all(|(_, denominator)| denominator == first)
    {
        let numerator = fractions.iter().map(|(numerator, _)| numerator).sum();
        return (numerator, first.clone());
    }
    let common = fractions
        .iter()
        .fold(BigInt::from(1), |common, (_, denominator)| {
            common.lcm(denominator)
        });
    let numerator = fractions
        .iter()
        .map(|(numerator, denominator)| numerator * (&common / denominator))
        .sum();
    (numerator, common)
}

/// The fraction with a denominator of at most sixteen nearest a proportion:
/// music21's `proportionToFraction`. `None` for a value that is not finite.
///
/// ```
/// use music21::meter::tools::proportion_to_fraction;
/// use num::BigInt;
///
/// let third = proportion_to_fraction(0.333).unwrap();
/// assert_eq!(third, (BigInt::from(1), BigInt::from(3)));
/// ```
#[must_use]
pub fn proportion_to_fraction(value: FloatType) -> Option<(BigInt, BigInt)> {
    let (numerator, denominator) = float_ratio(value)?;
    limit_denominator(&numerator, &denominator, &BigInt::from(16)).ok()
}

/// The same ratio in shorter notes, while the denominator allows: music21's
/// `divisionOptionsFractionsUpward`.
#[must_use]
pub fn fractions_upward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<String> {
    let largest = VALID_DENOMINATORS[VALID_DENOMINATORS.len() - 1];
    let mut out = Vec::new();
    if denominator >= largest {
        return out;
    }
    let (mut numerator, mut denominator) = (numerator * 2, denominator * 2);
    while denominator <= largest {
        out.push(format!("{numerator}/{denominator}"));
        numerator *= 2;
        denominator *= 2;
    }
    out
}

/// The same ratio in longer notes, while it stays whole: music21's
/// `divisionOptionsFractionsDownward`.
#[must_use]
pub fn fractions_downward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<String> {
    let smallest = VALID_DENOMINATORS[0];
    let mut out = Vec::new();
    if denominator <= smallest || !numerator.is_multiple_of(2) {
        return out;
    }
    let (mut numerator, mut denominator) = (numerator / 2, denominator / 2);
    loop {
        out.push(format!("{numerator}/{denominator}"));
        if !numerator.is_multiple_of(2) || denominator <= smallest {
            break;
        }
        numerator /= 2;
        denominator /= 2;
    }
    out
}

/// One unit at a time, then twice as many half as long: music21's
/// `divisionOptionsAdditiveMultiplesUpward`, which stops at sixteen parts or
/// at the numerator where that is larger.
#[must_use]
pub fn additive_multiples_upward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if numerator <= 1 {
        return out;
    }
    let largest = VALID_DENOMINATORS[VALID_DENOMINATORS.len() - 1];
    let limit = if numerator > 16 { numerator } else { 16 };
    let (mut denominator, mut count) = (denominator, numerator);
    while denominator <= largest && count <= limit {
        out.push(vec![format!("1/{denominator}"); count as usize]);
        denominator *= 2;
        count *= 2;
    }
    out
}

/// The bar halved, and halved again while it stays even: music21's
/// `divisionOptionsAdditiveMultiplesEvenDivision`. A `4/4` bar reads
/// `1/2+1/2`, written in the note value a half of it actually is.
#[must_use]
pub fn additive_multiples_halved(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if !numerator.is_multiple_of(2) || denominator < 2 {
        return out;
    }
    let (mut count, mut denominator) = (numerator / 2, denominator / 2);
    while denominator >= 1 && count > 1 {
        out.push(vec![format!("1/{denominator}"); count as usize]);
        if !count.is_multiple_of(2) || denominator == 1 {
            break;
        }
        denominator /= 2;
        count /= 2;
    }
    out
}

/// Even groupings in the bar's own note value: music21's
/// `divisionOptionsAdditiveMultiples`, which reads `6/4` as `3/4+3/4`.
#[must_use]
pub fn additive_multiples(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    if numerator <= 3 || !numerator.is_multiple_of(2) {
        return out;
    }
    let mut divisor = 2;
    while numerator.is_multiple_of(divisor) {
        let count = numerator / divisor;
        if count <= 1 {
            break;
        }
        out.push(vec![format!("{count}/{denominator}"); divisor as usize]);
        divisor *= 2;
    }
    out
}

/// A single unit split into smaller ones: music21's
/// `divisionOptionsAdditiveMultiplesDownward`, which only ever applies where
/// the numerator is one.
#[must_use]
pub fn additive_multiples_downward(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let largest = VALID_DENOMINATORS[VALID_DENOMINATORS.len() - 1];
    let mut out = Vec::new();
    if denominator >= largest || numerator != 1 {
        return out;
    }
    let (mut count, mut denominator) = (2usize, denominator * 2);
    while denominator <= largest {
        out.push(vec![format!("{numerator}/{denominator}"); count]);
        denominator *= 2;
        count *= 2;
    }
    out
}

/// The ways music21 divides a meter, in the order it prefers them, each
/// once: music21's `divisionOptionsAlgo`.
///
/// The order is the behaviour, not decoration:
/// [`super::MeterTerminal::partitioned_by_count`] takes the first option of the right
/// length, so these pieces are composed in the same sequence music21
/// composes them in.
#[must_use]
pub fn division_options_algo(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    let mut options: Vec<Vec<String>> = Vec::new();

    // Compound meters divide into threes first: 6/8 is two dotted beats.
    if numerator > 3 && numerator.is_multiple_of(3) {
        options.push(vec![format!("3/{denominator}"); (numerator / 3) as usize]);
    }
    // The odd meters music21 keeps a grouping for.
    let groupings: &[&[UnsignedIntegerType]] = match numerator {
        5 => &[&[2, 3], &[3, 2]],
        7 => &[&[2, 2, 3], &[3, 2, 2], &[2, 3, 2]],
        10 => &[&[2, 2, 3, 3]],
        _ => &[],
    };
    for grouping in groupings {
        options.push(
            grouping
                .iter()
                .map(|count| format!("{count}/{denominator}"))
                .collect(),
        );
    }
    options.extend(additive_multiples_upward(numerator, denominator));
    options.extend(additive_multiples_halved(numerator, denominator));
    options.push(vec![format!("{numerator}/{denominator}")]);
    options.extend(additive_multiples(numerator, denominator));
    options.extend(additive_multiples_downward(numerator, denominator));
    for written in fractions_downward(numerator, denominator) {
        options.push(vec![written]);
    }
    for written in fractions_upward(numerator, denominator) {
        options.push(vec![written]);
    }
    // music21 hands each option out once.
    let mut seen: Vec<Vec<String>> = Vec::new();
    options.retain(|option| {
        if seen.contains(option) {
            return false;
        }
        seen.push(option.clone());
        true
    });
    options
}

/// The divisions music21 keeps by hand because no rule produces them: only
/// the two extra readings of a five.
#[must_use]
pub fn division_options_preset(
    numerator: UnsignedIntegerType,
    denominator: UnsignedIntegerType,
) -> Vec<Vec<String>> {
    if numerator != 5 {
        return Vec::new();
    }
    vec![
        vec![
            format!("2/{denominator}"),
            format!("2/{denominator}"),
            format!("1/{denominator}"),
        ],
        vec![
            format!("2/{denominator}"),
            format!("1/{denominator}"),
            format!("2/{denominator}"),
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(options: &[Vec<String>]) -> Vec<String> {
        options.iter().map(|option| option.join(",")).collect()
    }

    #[test]
    fn a_meter_is_read_with_its_word() {
        let read = slash_to_tuple("7/32").unwrap();
        assert_eq!((read.numerator, read.denominator), (7, 32));
        assert_eq!(read.division, MeterDivision::None);
        assert_eq!(
            slash_to_tuple("Fast 6/8").unwrap().division,
            MeterDivision::Fast
        );
        assert_eq!(
            slash_to_tuple("3.0/4.0").unwrap_err(),
            SlashError::Meter("slashToTuple() cannot find two part fraction for 3.0/4.0".into())
        );
    }

    #[test]
    fn a_mixed_meter_fails_as_music21_fails() {
        let (parts, summed) = slash_mixed_to_fraction("3+2+5/8+3/4+2+1+4/16").unwrap();
        assert_eq!(
            parts,
            [(3, 8), (2, 8), (5, 8), (3, 4), (2, 16), (1, 16), (4, 16)]
        );
        assert!(summed);
        assert_eq!(
            slash_mixed_to_fraction("4/4").unwrap(),
            (vec![(4, 4)], false)
        );
        assert_eq!(
            slash_mixed_to_fraction("3+2+5/8+3/4+2+1+4").unwrap_err(),
            SlashError::Meter("cannot match denominator to numerator in: 3+2+5/8+3/4+2+1+4".into())
        );
        assert_eq!(
            slash_mixed_to_fraction("3.0/4.0").unwrap_err(),
            SlashError::TimeSignature("Cannot create time signature from \"3.0/4.0\"".into())
        );
        assert!(matches!(
            slash_mixed_to_fraction("x+3/4"),
            Err(SlashError::Unreadable(_))
        ));
    }

    #[test]
    fn fractions_sum_without_reducing() {
        let big = |pairs: &[(i32, i32)]| -> Vec<(BigInt, BigInt)> {
            pairs
                .iter()
                .map(|(n, d)| (BigInt::from(*n), BigInt::from(*d)))
                .collect()
        };
        let sum = |pairs: &[(i32, i32)]| fraction_sum(&big(pairs));
        assert_eq!(
            sum(&[(1, 13), (2, 17)]),
            (BigInt::from(43), BigInt::from(221))
        );
        assert_eq!(sum(&[(3, 4), (1, 2)]), (BigInt::from(5), BigInt::from(4)));
        assert_eq!(sum(&[]), (BigInt::from(0), BigInt::from(1)));
        assert_eq!(
            proportion_to_fraction(0.83333),
            Some((BigInt::from(5), BigInt::from(6)))
        );
        assert_eq!(proportion_to_fraction(FloatType::NAN), None);
    }

    #[test]
    fn the_options_are_music21s() {
        assert_eq!(
            strings(&division_options_algo(12, 8)),
            [
                "3/8,3/8,3/8,3/8",
                "1/8,1/8,1/8,1/8,1/8,1/8,1/8,1/8,1/8,1/8,1/8,1/8",
                "1/4,1/4,1/4,1/4,1/4,1/4",
                "1/2,1/2,1/2",
                "12/8",
                "6/8,6/8",
                "6/4",
                "3/2",
                "24/16",
                "48/32",
                "96/64",
                "192/128",
            ]
        );
        assert_eq!(
            strings(&division_options_algo(3, 128)),
            ["1/128,1/128,1/128", "3/128"]
        );
        assert_eq!(fractions_downward(12, 16), ["6/8", "3/4"]);
        assert_eq!(strings(&additive_multiples(6, 4)), ["3/4,3/4"]);
        assert!(additive_multiples_halved(3, 4).is_empty());
    }
}
