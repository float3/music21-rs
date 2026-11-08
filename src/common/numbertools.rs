//! music21's `common.numberTools`: quarter lengths kept exact, numbers
//! rounded and written the way music21 writes them, Roman numerals and
//! ordinals.

use std::convert::TryFrom;
use std::fmt::{self, Display};
use std::ops::Range;
use std::sync::LazyLock;

use num::bigint::BigInt;
use num::rational::BigRational;
use num::{FromPrimitive, Integer, One, Signed, ToPrimitive, Zero};
use num_traits::Float;

use crate::defaults::{FloatType, UnsignedIntegerType};
use crate::error::{Error, Result};

macro_rules! define_ordinals {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident => ($display:expr, $lower:expr)),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        #[repr(u8)]
        $vis enum $name {
            $($variant),*
        }

        impl TryFrom<UnsignedIntegerType> for $name {
            type Error = crate::error::Error;
            fn try_from(value: UnsignedIntegerType) -> Result<Self, Self::Error> {
                const VARIANTS: &[$name] = &[$($name::$variant),*];
                let idx = value as usize;
                if idx < VARIANTS.len() {
                    Ok(VARIANTS[idx])
                } else {
                    Err(Error::Ordinal(format!(
                        "Invalid {} value: {}",
                        stringify!($name),
                        value
                    )))
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = crate::error::Error;
            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value.to_lowercase().as_str() {
                    $(
                        $lower => Ok($name::$variant),
                    )*
                    _ => Err(Error::Ordinal(format!(
                        "Invalid {} string: {}",
                        stringify!($name),
                        value
                    ))),
                }
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let s = match self {
                    $(
                        $name::$variant => $display,
                    )*
                };
                write!(f, "{}", s)
            }
        }
    };
}

// Define the musical ordinals (override a few names)
define_ordinals! {
    pub(crate) enum MusicOrdinals {
        Zeroth      => ("Zeroth", "zeroth"),
        Unison      => ("Unison", "unison"),          // instead of First
        Second      => ("Second", "second"),
        Third       => ("Third", "third"),
        Fourth      => ("Fourth", "fourth"),
        Fifth       => ("Fifth", "fifth"),
        Sixth       => ("Sixth", "sixth"),
        Seventh     => ("Seventh", "seventh"),
        Octave      => ("Octave", "octave"),           // instead of Eighth
        Ninth       => ("Ninth", "ninth"),
        Tenth       => ("Tenth", "tenth"),
        Eleventh    => ("Eleventh", "eleventh"),
        Twelfth     => ("Twelfth", "twelfth"),
        Thirteenth  => ("Thirteenth", "thirteenth"),
        Fourteenth  => ("Fourteenth", "fourteenth"),
        DoubleOctave=> ("Double-octave", "double-octave"), // instead of Fifteenth
        Sixteenth   => ("Sixteenth", "sixteenth"),
        Seventeenth => ("Seventeenth", "seventeenth"),
        Eighteenth  => ("Eighteenth", "eighteenth"),
        Nineteenth  => ("Nineteenth", "nineteenth"),
        Twentieth   => ("Twentieth", "twentieth"),
        TwentyFirst => ("Twenty-first", "twenty-first"),
        TripleOctave=> ("Triple-octave", "triple-octave"), // instead of Twenty-second
    }
}

/// music21's plain `ordinals` list, as far as any chord inversion can reach.
pub(crate) const ORDINALS: [&str; 8] = [
    "Zeroth", "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh",
];

pub(crate) static MUSICAL_ORDINAL_STRINGS: LazyLock<[String; 23]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let val: UnsignedIntegerType = i as UnsignedIntegerType;
        let ordinal = MusicOrdinals::try_from(val).unwrap();
        format!("{ordinal}")
    })
});

/// The same names lowered, for the case-insensitive search an interval name
/// does over all twenty-three of them.
pub(crate) static MUSICAL_ORDINAL_STRINGS_LOWER: LazyLock<[String; 23]> =
    LazyLock::new(|| std::array::from_fn(|i| MUSICAL_ORDINAL_STRINGS[i].to_ascii_lowercase()));

/// music21's `defaults.limitOffsetDenominator`: the largest denominator
/// [`op_frac`] keeps a quarter length as a fraction over.
pub const DENOM_LIMIT: u64 = 65535;

/// music21's `ordinals`: the names of the numbers from nought to
/// twenty-two, in English.
pub const ORDINAL_NAMES: [&str; 23] = [
    "Zeroth",
    "First",
    "Second",
    "Third",
    "Fourth",
    "Fifth",
    "Sixth",
    "Seventh",
    "Eighth",
    "Ninth",
    "Tenth",
    "Eleventh",
    "Twelfth",
    "Thirteenth",
    "Fourteenth",
    "Fifteenth",
    "Sixteenth",
    "Seventeenth",
    "Eighteenth",
    "Nineteenth",
    "Twentieth",
    "Twenty-first",
    "Twenty-second",
];

/// music21's `musicOrdinals`: [`ORDINAL_NAMES`] with an interval's own
/// names for the first, the eighth, the fifteenth and the twenty-second.
pub const MUSIC_ORDINAL_NAMES: [&str; 23] = [
    "Zeroth",
    "Unison",
    "Second",
    "Third",
    "Fourth",
    "Fifth",
    "Sixth",
    "Seventh",
    "Octave",
    "Ninth",
    "Tenth",
    "Eleventh",
    "Twelfth",
    "Thirteenth",
    "Fourteenth",
    "Double-octave",
    "Sixteenth",
    "Seventeenth",
    "Eighteenth",
    "Nineteenth",
    "Twentieth",
    "Twenty-first",
    "Triple-octave",
];

/// A quarter length or an offset as music21 keeps one: a float where a
/// float says it exactly, a fraction where none does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OffsetQL {
    /// A value a float holds exactly.
    Float(FloatType),
    /// A value in lowest terms whose denominator is not a power of two.
    Fraction {
        /// The numerator, which carries the sign.
        numerator: i64,
        /// The denominator, always positive.
        denominator: i64,
    },
}

impl OffsetQL {
    /// The value as the nearest float.
    pub fn to_f64(self) -> FloatType {
        match self {
            Self::Float(value) => value,
            Self::Fraction {
                numerator,
                denominator,
            } => numerator as FloatType / denominator as FloatType,
        }
    }
}

/// A whole number or a float, which is what music21's `numToIntOrFloat`
/// and `roundToHalfInteger` give back.
#[derive(Clone, Debug, PartialEq)]
pub enum IntOrFloat {
    /// A whole number, of any size.
    Int(BigInt),
    /// Anything else.
    Float(FloatType),
}

/// Python's `math.isclose(a, b, abs_tol=abs_tol)`, with its relative
/// tolerance of one in a billion.
///
/// ```
/// use music21::common::numbertools::is_close;
///
/// assert!(is_close(0.1 + 0.2, 0.3, 0.0));
/// assert!(!is_close(1.0, 1.1, 0.01));
/// ```
pub fn is_close(a: FloatType, b: FloatType, abs_tol: FloatType) -> bool {
    if a == b {
        return true;
    }
    if a.is_infinite() || b.is_infinite() {
        return false;
    }
    let difference = (b - a).abs();
    difference <= (1e-9 * b).abs() || difference <= (1e-9 * a).abs() || difference <= abs_tol
}

/// A float written as Python's `repr` writes it: the shortest digits that
/// read back as the same float, `1.0` for a whole number, and an exponent
/// below a ten-thousandth or from 1e16 up.
///
/// ```
/// use music21::common::numbertools::float_repr;
///
/// assert_eq!(float_repr(1.0), "1.0");
/// assert_eq!(float_repr(0.125), "0.125");
/// assert_eq!(float_repr(1e16), "1e+16");
/// assert_eq!(float_repr(0.00001), "1e-05");
/// ```
pub fn float_repr(value: FloatType) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    if (-4..16).contains(&exponent) {
        let point = exponent + 1;
        if point <= 0 {
            let zeros = "0".repeat(point.unsigned_abs() as usize);
            return format!("{sign}0.{zeros}{digits}");
        }
        let point = point as usize;
        if digits.len() <= point {
            let zeros = "0".repeat(point - digits.len());
            return format!("{sign}{digits}{zeros}.0");
        }
        return format!("{sign}{}.{}", &digits[..point], &digits[point..]);
    }
    let rest = if digits.len() > 1 {
        format!(".{}", &digits[1..])
    } else {
        String::new()
    };
    let exponent_sign = if exponent < 0 { '-' } else { '+' };
    format!(
        "{sign}{}{rest}e{exponent_sign}{:02}",
        &digits[..1],
        exponent.unsigned_abs()
    )
}

/// Python's `round(value, digits)` for a float: the value written out to
/// that many decimal places, correctly rounded, and read back.
pub(crate) fn round_digits(value: FloatType, digits: usize) -> FloatType {
    if !value.is_finite() {
        return value;
    }
    format!("{value:.digits$}").parse().unwrap_or(value)
}

/// Python's `divmod` for floats: the floored quotient, and the remainder
/// with the sign of the divisor.
pub(crate) fn float_divmod(x: FloatType, y: FloatType) -> (FloatType, FloatType) {
    let mut modulo = x % y;
    let mut division = (x - modulo) / y;
    if modulo != 0.0 {
        if (y < 0.0) != (modulo < 0.0) {
            modulo += y;
            division -= 1.0;
        }
    } else {
        modulo = 0.0f64.copysign(y);
    }
    if division == 0.0 {
        return (0.0f64.copysign(x / y), modulo);
    }
    let mut floored = division.floor();
    if division - floored > 0.5 {
        floored += 1.0;
    }
    (floored, modulo)
}

/// The exact fraction a float stands for, in lowest terms, as Python's
/// `float.as_integer_ratio` gives it.
///
/// ```
/// use music21::common::numbertools::float_ratio;
/// use num::BigInt;
///
/// assert_eq!(float_ratio(0.75), Some((BigInt::from(3), BigInt::from(4))));
/// assert_eq!(float_ratio(f64::NAN), None);
/// ```
pub fn float_ratio(value: FloatType) -> Option<(BigInt, BigInt)> {
    if !value.is_finite() {
        return None;
    }
    let (mantissa, exponent, sign) = Float::integer_decode(value);
    if mantissa == 0 {
        return Some((BigInt::zero(), BigInt::one()));
    }
    let zeros = mantissa.trailing_zeros();
    let mantissa = BigInt::from(mantissa >> zeros) * BigInt::from(sign);
    let exponent = i32::from(exponent) + zeros as i32;
    if exponent >= 0 {
        return Some((mantissa << exponent as usize, BigInt::one()));
    }
    Some((mantissa, BigInt::one() << exponent.unsigned_abs() as usize))
}

/// The fraction nearest `numerator / denominator` whose denominator is at
/// most `limit`: Python's `Fraction.limit_denominator`, which music21's
/// `_preFracLimitDenominator` copies. A fraction already within the limit
/// comes back as it is.
///
/// ```
/// use music21::common::numbertools::limit_denominator;
/// use num::BigInt;
///
/// let third = limit_denominator(&100_001.into(), &300_001.into(), &65535.into())?;
/// assert_eq!(third, (BigInt::from(1), BigInt::from(3)));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A limit below one, which no denominator meets.
pub fn limit_denominator(
    numerator: &BigInt,
    denominator: &BigInt,
    limit: &BigInt,
) -> Result<(BigInt, BigInt)> {
    if *limit < BigInt::one() {
        return Err(Error::Value(
            "max_denominator should be at least 1".to_string(),
        ));
    }
    if denominator <= limit {
        return Ok((numerator.clone(), denominator.clone()));
    }
    let (mut p0, mut q0, mut p1, mut q1) =
        (BigInt::zero(), BigInt::one(), BigInt::one(), BigInt::zero());
    let (mut n, mut d) = (numerator.clone(), denominator.clone());
    loop {
        // Only a fraction not in lowest terms runs out before the limit.
        if d.is_zero() {
            return Ok((p1, q1));
        }
        let a = n.div_floor(&d);
        let q2 = &q0 + &a * &q1;
        if q2 > *limit {
            break;
        }
        let p2 = &p0 + &a * &p1;
        (p0, q0, p1, q1) = (p1, q1, p2, q2);
        let next = &n - &a * &d;
        n = d;
        d = next;
    }
    let k = (limit - &q0).div_floor(&q1);
    let far_n = &p0 + &k * &p1;
    let far_d = &q0 + &k * &q1;
    let far_off = (&far_n * denominator - numerator * &far_d).abs();
    let near_off = (&p1 * denominator - numerator * &q1).abs();
    if far_off * (denominator * &q1) - near_off * (denominator * &far_d) >= BigInt::zero() {
        return Ok((p1, q1));
    }
    Ok((far_n, far_d))
}

fn is_power_of_two(value: &BigInt) -> bool {
    value.is_positive() && (value & (value - BigInt::one())).is_zero()
}

/// music21's `opFrac` for a float: the float itself where its exact
/// fraction has a denominator up to [`DENOM_LIMIT`], and otherwise the
/// nearest fraction within the limit, written as a float again where its
/// denominator is a power of two.
///
/// It is what keeps three triplet eighths summing to exactly one beat.
///
/// ```
/// use music21::common::numbertools::{op_frac, OffsetQL};
///
/// assert_eq!(op_frac(0.25)?, OffsetQL::Float(0.25));
/// assert_eq!(
///     op_frac(1.0 / 3.0)?,
///     OffsetQL::Fraction { numerator: 1, denominator: 3 }
/// );
/// assert_eq!(op_frac(0.99999999842)?, OffsetQL::Float(1.0));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A value that is not finite, which has no fraction.
pub fn op_frac(value: FloatType) -> Result<OffsetQL> {
    let (numerator, denominator) = float_ratio(value).ok_or_else(|| {
        let which = if value.is_nan() { "NaN" } else { "Infinity" };
        Error::Value(format!("cannot convert {which} to integer ratio"))
    })?;
    let limit = BigInt::from(DENOM_LIMIT);
    if denominator <= limit {
        return Ok(OffsetQL::Float(value));
    }
    let (numerator, denominator) = limit_denominator(&numerator, &denominator, &limit)?;
    // Within the limit, a numerator is far inside an `i64`: a float with
    // bits below 2^-16 is under 2^37.
    let numerator = numerator.to_i64().unwrap_or(i64::MAX);
    let denominator = denominator.to_i64().unwrap_or(1);
    if denominator & (denominator - 1) == 0 {
        return Ok(OffsetQL::Float(
            numerator as FloatType / denominator as FloatType,
        ));
    }
    Ok(OffsetQL::Fraction {
        numerator,
        denominator,
    })
}

/// Whether `denominator` is a power of two, which is when music21's
/// `opFrac` turns a fraction into a float.
pub fn is_binary_denominator(denominator: &BigInt) -> bool {
    is_power_of_two(denominator)
}

/// A value close enough to a whole number, within a millionth, as that
/// whole number, and anything else as it is: music21's `numToIntOrFloat`.
///
/// ```
/// use music21::common::numbertools::{num_to_int_or_float, IntOrFloat};
///
/// assert_eq!(num_to_int_or_float(0.999999999)?, IntOrFloat::Int(1.into()));
/// assert_eq!(num_to_int_or_float(1.5)?, IntOrFloat::Float(1.5));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A value that is not finite, which rounds to no whole number.
pub fn num_to_int_or_float(value: FloatType) -> Result<IntOrFloat> {
    let rounded = whole(value.round_ties_even())?;
    let back = rounded.to_f64().unwrap_or(FloatType::INFINITY);
    if is_close(back, value, 1e-6) {
        return Ok(IntOrFloat::Int(rounded));
    }
    Ok(IntOrFloat::Float(value))
}

/// A whole float as a whole number, refused as Python's `int` refuses it.
fn whole(value: FloatType) -> Result<BigInt> {
    if value.is_nan() {
        return Err(Error::Value(
            "cannot convert float NaN to integer".to_string(),
        ));
    }
    BigInt::from_f64(value.trunc())
        .ok_or_else(|| Error::Value("cannot convert float infinity to integer".to_string()))
}

/// A fraction as Python's `str` writes one: `3/7`, or `3` over one.
fn fraction_text(numerator: &BigInt, denominator: &BigInt) -> String {
    if denominator.is_one() {
        return numerator.to_string();
    }
    format!("{numerator}/{denominator}")
}

fn mixed_text(quotient: &BigInt, remainder: &BigRational) -> String {
    let fraction = fraction_text(remainder.numer(), remainder.denom());
    if !quotient.is_zero() {
        if remainder.is_zero() {
            return quotient.to_string();
        }
        return format!("{quotient} {fraction}");
    }
    if !remainder.is_zero() {
        return fraction;
    }
    "0".to_string()
}

/// A number as a whole number and a fraction, `1 1/3`, with the fraction's
/// denominator limited to `limit`: music21's `mixedNumeral` for a float.
///
/// ```
/// use music21::common::numbertools::mixed_numeral;
///
/// assert_eq!(mixed_numeral(1.333333, 65535)?, "1 1/3");
/// assert_eq!(mixed_numeral(-0.333333, 65535)?, "-1/3");
/// assert_eq!(mixed_numeral(2.0000001, 65535)?, "2");
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A value that is not finite, or a limit below one.
pub fn mixed_numeral(value: FloatType, limit: u64) -> Result<String> {
    let (mut quotient, remainder) = float_divmod(value, 1.0);
    let (numerator, denominator) = float_ratio(remainder)
        .ok_or_else(|| Error::Value("cannot convert NaN to integer ratio".to_string()))?;
    let (numerator, denominator) =
        limit_denominator(&numerator, &denominator, &BigInt::from(limit))?;
    let mut fraction = BigRational::new(numerator, denominator);
    if quotient < -1.0 {
        quotient += 1.0;
        fraction = BigRational::one() - fraction;
    } else if quotient == -1.0 {
        quotient = 0.0;
        fraction -= BigRational::one();
    }
    Ok(mixed_text(&whole(quotient)?, &fraction))
}

/// [`mixed_numeral`] for an exact fraction, whose denominator is not
/// limited: `-4 3/7` for -31/7.
///
/// ```
/// use music21::common::numbertools::mixed_numeral_fraction;
///
/// assert_eq!(mixed_numeral_fraction(&31.into(), &7.into())?, "4 3/7");
/// assert_eq!(mixed_numeral_fraction(&(-4).into(), &5.into())?, "-4/5");
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A denominator of nought, or a fraction too large for a float, which
/// music21 passes through on the way.
pub fn mixed_numeral_fraction(numerator: &BigInt, denominator: &BigInt) -> Result<String> {
    if denominator.is_zero() {
        return Err(Error::Value("Fraction(_, 0)".to_string()));
    }
    let value = BigRational::new(numerator.clone(), denominator.clone());
    let quotient = whole(value.to_f64().unwrap_or(FloatType::NAN))?;
    let mut remainder = &value - BigRational::from_integer(quotient.clone());
    if quotient.is_negative() {
        remainder = -remainder;
    }
    Ok(mixed_text(&quotient, &remainder))
}

/// A number rounded to the nearest half, a quarter rounding up, as
/// music21's `roundToHalfInteger` does: a whole number where it lands on
/// one, and a float on a half.
///
/// ```
/// use music21::common::numbertools::{round_to_half_integer, IntOrFloat};
///
/// assert_eq!(round_to_half_integer(1.35)?, IntOrFloat::Float(1.5));
/// assert_eq!(round_to_half_integer(1.75)?, IntOrFloat::Int(2.into()));
/// assert_eq!(round_to_half_integer(-0.25)?, IntOrFloat::Int(0.into()));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A value that is not finite.
pub fn round_to_half_integer(value: FloatType) -> Result<IntOrFloat> {
    let (floor, fraction) = float_divmod(value, 1.0);
    let floor = whole(floor)?;
    if fraction < 0.25 {
        return Ok(IntOrFloat::Int(floor));
    }
    if fraction < 0.75 {
        let floor = floor.to_f64().unwrap_or(FloatType::INFINITY);
        return Ok(IntOrFloat::Float(floor + 0.5));
    }
    Ok(IntOrFloat::Int(floor + 1))
}

/// The third or sixth a value is within `grain` of, as [`op_frac`] gives
/// it, or nothing for a value near neither: music21's
/// `addFloatPrecision`, which leaves such a value alone.
///
/// ```
/// use music21::common::numbertools::{add_float_precision, OffsetQL};
///
/// assert_eq!(
///     add_float_precision(0.33, 1e-2),
///     Some(OffsetQL::Fraction { numerator: 1, denominator: 3 })
/// );
/// assert_eq!(add_float_precision(0.2, 1e-2), None);
/// ```
pub fn add_float_precision(value: FloatType, grain: FloatType) -> Option<OffsetQL> {
    [1.0 / 3.0, 2.0 / 3.0, 1.0 / 6.0, 5.0 / 6.0]
        .into_iter()
        .find(|candidate| is_close(value, *candidate, grain))
        .and_then(|candidate| op_frac(candidate).ok())
}

/// A float written with `max_digits` decimals and its trailing zeros
/// dropped, one decimal always kept: music21's `strTrimFloat`.
///
/// ```
/// use music21::common::numbertools::str_trim_float;
///
/// assert_eq!(str_trim_float(42.3333333333, 4)?, "42.3333");
/// assert_eq!(str_trim_float(6.66666666666666, 2)?, "6.67");
/// assert_eq!(str_trim_float(2.0, 4)?, "2.0");
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// No decimals asked for, or a value that is not finite: neither is
/// written with a decimal point, which music21 looks for.
pub fn str_trim_float(value: FloatType, max_digits: usize) -> Result<String> {
    let written = format!("{value:.max_digits$}");
    let point = written
        .find('.')
        .ok_or_else(|| Error::Value("substring not found".to_string()))?;
    let bytes = written.as_bytes();
    let mut length = written.len();
    for index in (point + 2..written.len()).rev() {
        if bytes[index] != b'0' {
            break;
        }
        length -= 1;
    }
    Ok(written[..length].to_string())
}

/// The multiple of a unit nearest a value, as [`nearest_multiple`] finds
/// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearestMultiple {
    /// The multiple itself.
    pub multiple: FloatType,
    /// How far the value is from it, to seven decimals.
    pub error: FloatType,
    /// The value less the multiple, to seven decimals.
    pub signed_error: FloatType,
}

/// The multiple of `unit` nearest a value that is not negative, how far
/// off it is and which way: music21's `nearestMultiple`. Half way rounds
/// down.
///
/// ```
/// use music21::common::numbertools::nearest_multiple;
///
/// let found = nearest_multiple(0.4, 0.25)?;
/// assert_eq!(found.multiple, 0.5);
/// assert_eq!(found.signed_error, -0.1);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A negative value, or one that falls between no two multiples.
pub fn nearest_multiple(value: FloatType, unit: FloatType) -> Result<NearestMultiple> {
    if value < 0.0 {
        return Err(Error::Value(format!(
            "n ({}) is less than zero. Thus, cannot find the nearest multiple for a value \
             less than the unit, {}",
            float_repr(value),
            float_repr(unit)
        )));
    }
    let multiple = (value / unit).floor();
    let low = unit * multiple;
    let high = unit * (multiple + 1.0);
    if low >= value && value >= high {
        return Err(Error::Value(format!(
            "cannot place n between multiples: {}, {}",
            float_repr(low),
            float_repr(high)
        )));
    }
    if low <= value && value <= low + unit / 2.0 {
        let error = round_digits(value - low, 7);
        return Ok(NearestMultiple {
            multiple: low,
            error,
            signed_error: error,
        });
    }
    Ok(NearestMultiple {
        multiple: high,
        error: round_digits(high - value, 7),
        signed_error: round_digits(value - high, 7),
    })
}

/// music21's `_DOT_LOOKUP`: what one to eight dots multiply a length by.
const DOT_MULTIPLIERS: [FloatType; 9] = [
    1.0, 1.5, 1.75, 1.875, 1.9375, 1.96875, 1.984375, 1.9921875, 1.99609375,
];

/// What `dots` dots multiply a note's length by: music21's
/// `dotMultiplier`. It is always exact, since its denominator is a power
/// of two.
///
/// ```
/// use music21::common::numbertools::dot_multiplier;
///
/// assert_eq!(dot_multiplier(0), 1.0);
/// assert_eq!(dot_multiplier(2), 1.75);
/// ```
pub fn dot_multiplier(dots: u32) -> FloatType {
    if let Some(found) = DOT_MULTIPLIERS.get(dots as usize) {
        return *found;
    }
    let dots = FloatType::from(dots);
    (2.0f64.powf(dots + 1.0) - 1.0) / 2.0f64.powf(dots)
}

/// The tuplet ratio, in lowest terms, that a simple decimal stands for:
/// music21's `decimalToTuplet`. Below one, the denominator is the larger.
///
/// ```
/// use music21::common::numbertools::decimal_to_tuplet;
///
/// assert_eq!(decimal_to_tuplet(1.25)?, (5, 4));
/// assert_eq!(decimal_to_tuplet(0.8)?, (4, 5));
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A value not above nought, or one no ratio with a denominator under a
/// thousand comes within a ten-millionth of.
pub fn decimal_to_tuplet(value: FloatType) -> Result<(i128, i128)> {
    if value <= 0.0 {
        return Err(Error::Value("number must be greater than zero".to_string()));
    }
    let flipped = value < 1.0;
    let value = if flipped { 1.0 / value } else { value };
    let multiplier = value.trunc();
    let working = value / multiplier;
    let (numerator, denominator) =
        simple_fraction(working).ok_or_else(|| Error::Value("No such luck".to_string()))?;
    let scaled = FloatType::from(numerator) * multiplier;
    let divisor = num::integer::gcd(scaled as i128, i128::from(denominator));
    let numerator = (scaled / divisor as FloatType) as i128;
    let denominator = (FloatType::from(denominator) / divisor as FloatType) as i128;
    if flipped {
        return Ok((denominator, numerator));
    }
    Ok((numerator, denominator))
}

/// The first `j / i`, `i` under a thousand and `j` from `i` to under twice
/// it, within a ten-millionth of `value`.
fn simple_fraction(value: FloatType) -> Option<(u32, u32)> {
    for index in 1..1000u32 {
        for numerator in index..index * 2 {
            if is_close(
                value,
                FloatType::from(numerator) / FloatType::from(index),
                1e-7,
            ) {
                return Some((numerator, index));
            }
        }
    }
    None
}

/// Each value as its share of their sum: music21's
/// `unitNormalizeProportion`.
///
/// ```
/// use music21::common::numbertools::unit_normalize_proportion;
///
/// assert_eq!(unit_normalize_proportion(&[1.0, 1.0, 2.0])?, [0.25, 0.25, 0.5]);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A negative value. Values summing to nought give no finite shares.
pub fn unit_normalize_proportion(values: &[FloatType]) -> Result<Vec<FloatType>> {
    let mut sum = 0.0;
    for value in values {
        if *value < 0.0 {
            return Err(Error::Value("value members must be positive".to_string()));
        }
        sum += value;
    }
    Ok(values.iter().map(|value| value / sum).collect())
}

/// The span of the unit interval each value's share of their sum takes,
/// one after another, the last ending at exactly one: music21's
/// `unitBoundaryProportion`.
///
/// ```
/// use music21::common::numbertools::unit_boundary_proportion;
///
/// assert_eq!(
///     unit_boundary_proportion(&[1.0, 1.0, 2.0])?,
///     [(0.0, 0.25), (0.25, 0.5), (0.5, 1.0)]
/// );
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A negative value.
pub fn unit_boundary_proportion(values: &[FloatType]) -> Result<Vec<(FloatType, FloatType)>> {
    let shares = unit_normalize_proportion(values)?;
    let mut bounds = Vec::with_capacity(shares.len());
    let mut sum = 0.0;
    for (index, share) in shares.iter().enumerate() {
        if index == shares.len() - 1 {
            bounds.push((sum, 1.0));
            break;
        }
        bounds.push((sum, sum + share));
        sum += share;
    }
    Ok(bounds)
}

/// Which of the weighted choices a draw `draw` from the unit interval
/// lands on, as music21's `weightedSelection` picks one: the last where it
/// lands on none.
///
/// ```
/// use music21::common::numbertools::weighted_index;
///
/// assert_eq!(weighted_index(&[1.0, 3.0], 0.2)?, 0);
/// assert_eq!(weighted_index(&[1.0, 3.0], 0.3)?, 1);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A negative weight.
pub fn weighted_index(weights: &[FloatType], draw: FloatType) -> Result<usize> {
    let bounds = unit_boundary_proportion(weights)?;
    let found = bounds
        .iter()
        .position(|(low, high)| *low <= draw && draw < *high);
    Ok(found.unwrap_or(bounds.len().saturating_sub(1)))
}

/// The greatest length that goes a whole number of times, to within
/// `grain`, into every one of `values`: music21's `approximateGCD`, which
/// tries the smallest value, then each value divided by one to sixteen.
///
/// ```
/// use music21::common::numbertools::approximate_gcd;
///
/// assert_eq!(approximate_gcd(&[2.5, 10.0, 0.25], 1e-4)?, 0.25);
/// assert_eq!(approximate_gcd(&[1.5, 5.0, 2.0, 7.0], 1e-4)?, 0.5);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// No values, or no length that goes into them all. The smallest value
/// being nought gives no finite answer.
pub fn approximate_gcd(values: &[FloatType], grain: FloatType) -> Result<FloatType> {
    let lowest = values
        .iter()
        .copied()
        .reduce(FloatType::min)
        .ok_or_else(|| Error::Value("min() iterable argument is empty".to_string()))?;
    let divides = values.iter().all(|value| {
        let adjusted = value / lowest;
        is_close(adjusted - adjusted.trunc(), 0.0, grain)
    });
    if divides {
        return Ok(lowest);
    }
    let divisions: Vec<Vec<FloatType>> = values
        .iter()
        .map(|value| (1..=16).map(|d| value / FloatType::from(d)).collect())
        .collect();
    divisions
        .iter()
        .flatten()
        .copied()
        .filter(|candidate| {
            divisions
                .iter()
                .all(|row| row.iter().any(|x| is_close(*x, *candidate, grain)))
        })
        .reduce(FloatType::max)
        .ok_or_else(|| Error::Value("cannot find a common divisor".to_string()))
}

/// Whether each value is one more than the one before: music21's
/// `contiguousList`, which wants them sorted first. An empty list is
/// contiguous here; music21 cannot look at its first value.
///
/// ```
/// use music21::common::numbertools::contiguous_list;
///
/// assert!(contiguous_list(&[3, 4, 5, 6]));
/// assert!(!contiguous_list(&[3, 4, 5, 6, 8]));
/// ```
pub fn contiguous_list<T>(values: &[T]) -> bool
where
    T: Copy + PartialEq + std::ops::Add<Output = T> + One,
{
    values.windows(2).all(|pair| pair[1] == pair[0] + T::one())
}

/// The runs of sorted values each one more than the one before, as ranges
/// of their positions: what music21's `groupContiguousIntegers` groups.
///
/// ```
/// use music21::common::numbertools::contiguous_runs;
///
/// assert_eq!(contiguous_runs(&[3, 4, 6, 7, 20]), [0..2, 2..4, 4..5]);
/// ```
pub fn contiguous_runs<T>(sorted: &[T]) -> Vec<Range<usize>>
where
    T: Copy + PartialEq + std::ops::Add<Output = T> + One,
{
    let mut runs = Vec::new();
    let mut start = 0;
    for index in 1..=sorted.len() {
        let ends = index == sorted.len() || sorted[index] != sorted[index - 1] + T::one();
        if ends {
            runs.push(start..index);
            start = index;
        }
    }
    runs
}

/// A Roman numeral, upper or lower case, read as a number: music21's
/// `fromRoman`. The medieval `IIII` and `IC` are read too, unless
/// `strict_modern` refuses a subtraction of less than a tenth.
///
/// ```
/// use music21::common::numbertools::from_roman;
///
/// assert_eq!(from_roman("MCDLXXXIX", false)?, 1489);
/// assert_eq!(from_roman("ic", false)?, 99);
/// assert!(from_roman("ic", true).is_err());
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A letter that is not a numeral, or a subtraction no Roman wrote.
pub fn from_roman(text: &str, strict_modern: bool) -> Result<i64> {
    let upper = text.to_uppercase();
    let value_of = |letter: char| match letter {
        'M' => Some(1000),
        'D' => Some(500),
        'C' => Some(100),
        'L' => Some(50),
        'X' => Some(10),
        'V' => Some(5),
        'I' => Some(1),
        _ => None,
    };
    let values: Vec<i64> = upper
        .chars()
        .map(value_of)
        .collect::<Option<_>>()
        .ok_or_else(|| Error::Value(format!("value is not a valid roman numeral: {upper}")))?;
    let mut sum = 0;
    for (index, value) in values.iter().enumerate() {
        let mut value = *value;
        if let Some(next) = values.get(index + 1).copied() {
            if next > value && [1, 10, 100].contains(&value) {
                if strict_modern && next >= value * 10 {
                    return Err(Error::Value(format!(
                        "input contains an invalid subtraction element (modern interpretation): \
                         {text}"
                    )));
                }
                value = -value;
            } else if next > value {
                return Err(Error::Value(format!(
                    "input contains an invalid subtraction element: {text}"
                )));
            }
        }
        sum += value;
    }
    Ok(sum)
}

/// A number from 1 to 3999 as a Roman numeral: music21's `toRoman`.
///
/// ```
/// use music21::common::numbertools::to_roman;
///
/// assert_eq!(to_roman(1999)?, "MCMXCIX");
/// assert!(to_roman(0).is_err());
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// A number outside that range.
pub fn to_roman(number: i64) -> Result<String> {
    if !(1..4000).contains(&number) {
        return Err(Error::Value(
            "Argument must be between 1 and 3999".to_string(),
        ));
    }
    const NUMERALS: [(i64, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut rest = number;
    let mut written = String::new();
    for (value, numeral) in NUMERALS {
        let count = rest / value;
        written.push_str(&numeral.repeat(count as usize));
        rest -= value * count;
    }
    Ok(written)
}

/// The letters written after a number to make it an ordinal, `st`, `nd`,
/// `rd` or `th`, with an `s` after all but `st` for `plural`: music21's
/// `ordinalAbbreviation`.
///
/// ```
/// use music21::common::numbertools::ordinal_abbreviation;
///
/// assert_eq!(ordinal_abbreviation(3, false), "rd");
/// assert_eq!(ordinal_abbreviation(12, false), "th");
/// assert_eq!(ordinal_abbreviation(255, true), "ths");
/// ```
pub fn ordinal_abbreviation(value: i64, plural: bool) -> &'static str {
    let suffix = if (11..=13).contains(&value.rem_euclid(100)) {
        "th"
    } else {
        match value.rem_euclid(10) {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    if !plural || suffix == "st" {
        return suffix;
    }
    match suffix {
        "nd" => "nds",
        "rd" => "rds",
        _ => "ths",
    }
}

/// music21's `ordinalsToNumbers`, in the order it fills it: each ordinal
/// name, lowered and not, and abbreviated, `1st`, and each interval name
/// that differs from it, mapped to its number.
pub fn ordinals_to_numbers() -> Vec<(String, usize)> {
    let mut entries = Vec::new();
    for (index, name) in ORDINAL_NAMES.iter().enumerate() {
        entries.push(((*name).to_string(), index));
        entries.push((name.to_lowercase(), index));
        let abbreviation = ordinal_abbreviation(index as i64, false);
        entries.push((format!("{index}{abbreviation}"), index));
        let music = MUSIC_ORDINAL_NAMES[index];
        if music != *name {
            entries.push((music.to_string(), index));
            entries.push((music.to_lowercase(), index));
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn floats_past_the_limit_snap_to_a_fraction() {
        assert_eq!(
            op_frac(0.123456789).unwrap(),
            OffsetQL::Fraction {
                numerator: 10,
                denominator: 81
            }
        );
        assert_eq!(op_frac(0.000001).unwrap(), OffsetQL::Float(0.0));
        assert_eq!(op_frac(-1.0 / 3.0).unwrap().to_f64(), -1.0 / 3.0);
        assert!(op_frac(f64::NAN).is_err());
        assert!(op_frac(f64::INFINITY).is_err());
    }

    #[test]
    fn music21s_own_limit_matches_pythons() {
        let limited = limit_denominator(&100_001.into(), &300_001.into(), &65535.into()).unwrap();
        assert_eq!(limited, (BigInt::from(1), BigInt::from(3)));
        let (n, d) = float_ratio(1e-300).unwrap();
        let limited = limit_denominator(&n, &d, &65535.into()).unwrap();
        assert_eq!(limited, (BigInt::from(0), BigInt::from(1)));
    }

    #[test]
    fn mixed_numerals_written_as_music21_writes_them() {
        assert_eq!(mixed_numeral(-1.333333, 65535).unwrap(), "-1 1/3");
        assert_eq!(mixed_numeral(0.0, 65535).unwrap(), "0");
        assert_eq!(mixed_numeral(-0.0, 65535).unwrap(), "0");
        assert_eq!(
            mixed_numeral(2.0000001, 10_000_000).unwrap(),
            "2 1/10000000"
        );
        let fraction = |n: i64, d: i64| mixed_numeral_fraction(&n.into(), &d.into()).unwrap();
        assert_eq!(fraction(-31, 7), "-4 3/7");
        assert_eq!(fraction(-1, 5), "-1/5");
        assert_eq!(fraction(1, 5), "1/5");
    }

    #[test]
    fn floats_written_as_python_writes_them() {
        assert_eq!(float_repr(-0.5), "-0.5");
        assert_eq!(float_repr(23404.0), "23404.0");
        assert_eq!(float_repr(1e22), "1e+22");
        assert_eq!(float_repr(1.5e-7), "1.5e-07");
        assert_eq!(float_repr(0.0001), "0.0001");
        assert_eq!(float_repr(123456789012345.6), "123456789012345.6");
        assert_eq!(round_digits(0.35 - 0.25, 7), 0.1);
    }

    #[test]
    fn halves_round_up_on_the_boundaries() {
        let int = |value: i64| IntOrFloat::Int(value.into());
        assert_eq!(round_to_half_integer(1.2).unwrap(), int(1));
        assert_eq!(round_to_half_integer(0.25).unwrap(), IntOrFloat::Float(0.5));
        assert_eq!(round_to_half_integer(0.75).unwrap(), int(1));
        assert_eq!(
            round_to_half_integer(-0.26).unwrap(),
            IntOrFloat::Float(-0.5)
        );
        assert!(round_to_half_integer(f64::INFINITY).is_err());
    }

    #[test]
    fn nearest_multiples_and_their_errors() {
        let found = nearest_multiple(23404.0 - 0.0625, 0.125).unwrap();
        assert_eq!(
            (found.multiple, found.error, found.signed_error),
            (23403.875, 0.0625, 0.0625)
        );
        let error = nearest_multiple(-0.5, 0.125).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Value error: n (-0.5) is less than zero. Thus, cannot find the nearest multiple \
             for a value less than the unit, 0.125"
        );
    }

    #[test]
    fn numbers_and_strings_of_music21s_examples() {
        assert_eq!(str_trim_float(-5.0, 4).unwrap(), "-5.0");
        assert!(str_trim_float(1.0, 0).is_err());
        assert_eq!(decimal_to_tuplet(1.5).unwrap(), (3, 2));
        assert!(decimal_to_tuplet(-0.02).is_err());
        assert_eq!(dot_multiplier(3), 1.875);
        assert_eq!(dot_multiplier(10), 2047.0 / 1024.0);
        assert_eq!(approximate_gcd(&[2.0, 5.0, 10.0], 1e-4).unwrap(), 1.0);
        let third = approximate_gcd(&[5.0 / 3.0, 2.0 / 3.0, 5.0 / 6.0, 3.0 / 6.0], 1e-4).unwrap();
        assert_eq!(str_trim_float(third, 4).unwrap(), "0.1667");
        assert_eq!(
            num_to_int_or_float(1.00003).unwrap(),
            IntOrFloat::Float(1.00003)
        );
        assert!(unit_normalize_proportion(&[0.0, -2.0, -8.0]).is_err());
        assert_eq!(weighted_index(&[1.0, 0.0], 0.99).unwrap(), 0);
        assert_eq!(contiguous_runs(&[3, 3, 4]), [0..1, 1..3]);
        assert_eq!(contiguous_runs::<i64>(&[]), Vec::<Range<usize>>::new());
    }

    #[test]
    fn roman_numerals_both_ways() {
        assert_eq!(from_roman("vii", false).unwrap(), 7);
        assert_eq!(from_roman("MCCCCLXXXIX", false).unwrap(), 1489);
        assert_eq!(
            from_roman("vx", false).unwrap_err().to_string(),
            "Value error: input contains an invalid subtraction element: vx"
        );
        assert_eq!(
            from_roman("ab", false).unwrap_err().to_string(),
            "Value error: value is not a valid roman numeral: AB"
        );
        assert_eq!(to_roman(7).unwrap(), "VII");
        assert!(to_roman(4000).is_err());
    }

    #[test]
    fn ordinals_map_to_their_numbers() {
        let entries = ordinals_to_numbers();
        let find = |name: &str| entries.iter().find(|(key, _)| key == name).map(|(_, n)| *n);
        assert_eq!(find("unison"), Some(1));
        assert_eq!(find("1st"), Some(1));
        assert_eq!(find("Octave"), Some(8));
        assert_eq!(find("8th"), Some(8));
        assert_eq!(find("22nd"), Some(22));
        assert_eq!(
            &entries[..3],
            &[
                ("Zeroth".to_string(), 0),
                ("zeroth".to_string(), 0),
                ("0th".to_string(), 0)
            ]
        );
    }

    #[test]
    fn test_music_ordinal_try_from_integer_valid() {
        let cases = [
            (0, MusicOrdinals::Zeroth),
            (1, MusicOrdinals::Unison),
            (2, MusicOrdinals::Second),
            (8, MusicOrdinals::Octave),
            (15, MusicOrdinals::DoubleOctave),
            (22, MusicOrdinals::TripleOctave),
        ];
        for (input, expected) in cases {
            let ordinal = MusicOrdinals::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_music_ordinal_try_from_integer_invalid() {
        let result = MusicOrdinals::try_from(23);
        assert!(result.is_err());
        if let Err(Error::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid MusicOrdinals value: 23"));
        } else {
            panic!("Expected Error::Ordinal error");
        }
    }

    #[test]
    fn test_music_ordinal_try_from_str_valid() {
        let cases = [
            ("zeroth", MusicOrdinals::Zeroth),
            ("Unison", MusicOrdinals::Unison),
            ("SECOND", MusicOrdinals::Second),
            ("octave", MusicOrdinals::Octave),
            ("double-octave", MusicOrdinals::DoubleOctave),
            ("triple-octave", MusicOrdinals::TripleOctave),
        ];
        for (input, expected) in cases {
            let ordinal = MusicOrdinals::try_from(input).unwrap();
            assert_eq!(ordinal, expected);
        }
    }

    #[test]
    fn test_music_ordinal_try_from_str_invalid() {
        let result = MusicOrdinals::try_from("invalid");
        assert!(result.is_err());
        if let Err(Error::Ordinal(msg)) = result {
            assert!(msg.contains("Invalid MusicOrdinals string: invalid"));
        } else {
            panic!("Expected Error::Ordinal error");
        }
    }

    #[test]
    fn test_music_ordinal_display() {
        let ordinal = MusicOrdinals::TwentyFirst;
        assert_eq!(format!("{ordinal}"), "Twenty-first");
    }

    #[test]
    fn test_static_arrays() {
        let musical_strings: Vec<_> = MUSICAL_ORDINAL_STRINGS.iter().cloned().collect();
        assert_eq!(musical_strings[0], "Zeroth");
        assert_eq!(musical_strings[1], "Unison");
        assert_eq!(musical_strings[8], "Octave");
        assert_eq!(musical_strings[15], "Double-octave");
        assert_eq!(musical_strings[22], "Triple-octave");
    }
}
