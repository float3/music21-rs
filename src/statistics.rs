//! Python's `statistics` module where music21 leans on it: each answer the
//! exact one, rounded once.
//!
//! Python reads every float as the fraction it is exactly and rounds only
//! the answer, so these do the same: each value becomes an integer over one
//! shared power of two, everything is summed exactly, and the answer is
//! rounded once to the nearest float.

use num::{BigInt, BigUint, Signed, ToPrimitive, Zero};

use crate::defaults::FloatType;

/// Every value as an integer over one shared power of two, with that
/// power. A float is a whole number times a power of two, so this is
/// exact.
fn exact(values: &[FloatType]) -> (Vec<BigInt>, u64) {
    let parts: Vec<(i64, i64)> = values
        .iter()
        .map(|value| {
            let bits = value.to_bits();
            let sign = if bits >> 63 == 1 { -1 } else { 1 };
            let exponent = ((bits >> 52) & 0x7ff) as i64;
            let fraction = (bits & ((1 << 52) - 1)) as i64;
            if exponent == 0 {
                (sign * fraction, -1074)
            } else {
                (sign * (fraction | (1 << 52)), exponent - 1075)
            }
        })
        .collect();
    let shift = parts
        .iter()
        .map(|(_, exponent)| -exponent)
        .max()
        .unwrap_or(0)
        .max(0);
    let numerators = parts
        .into_iter()
        .map(|(mantissa, exponent)| BigInt::from(mantissa) << ((exponent + shift) as usize))
        .collect();
    (numerators, shift as u64)
}

/// A value times a power of two, exactly where the answer is a normal
/// float.
fn scaled_by_power_of_two(value: FloatType, mut power: i64) -> FloatType {
    let mut scaled = value;
    while power > 0 {
        let step = power.min(1000);
        scaled *= (2.0 as FloatType).powi(step as i32);
        power -= step;
    }
    while power < 0 {
        let step = (-power).min(1000);
        scaled /= (2.0 as FloatType).powi(step as i32);
        power += step;
    }
    scaled
}

/// A fraction as the nearest float: Python's true division of integers.
/// The quotient is taken to a few bits past a float's precision and
/// rounded to odd, so the conversion's own rounding is the only one.
fn fraction_to_float(numerator: &BigInt, denominator: &BigUint) -> FloatType {
    if numerator.is_zero() {
        return 0.0;
    }
    let magnitude = numerator.abs().to_biguint().unwrap_or_default();
    let shift = 55 + denominator.bits() as i64 - magnitude.bits() as i64;
    let (top, bottom) = if shift >= 0 {
        (magnitude << shift as usize, denominator.clone())
    } else {
        (magnitude, denominator << (-shift) as usize)
    };
    let quotient = &top / &bottom;
    let odd = if (&top % &bottom).is_zero() {
        quotient
    } else {
        quotient | BigUint::from(1u8)
    };
    let value = odd
        .to_u64()
        .map_or(FloatType::INFINITY, |odd| odd as FloatType);
    let value = scaled_by_power_of_two(value, -shift);
    if numerator.is_negative() {
        -value
    } else {
        value
    }
}

/// A sum of floats as Python's `sum` makes it, Neumaier's compensated sum,
/// so an average music21 takes of the same numbers is the same number.
pub(crate) fn python_sum(values: &[FloatType]) -> FloatType {
    let mut total: FloatType = 0.0;
    let mut compensation: FloatType = 0.0;
    for &value in values {
        let next = total + value;
        if total.abs() >= value.abs() {
            compensation += (total - next) + value;
        } else {
            compensation += (value - next) + total;
        }
        total = next;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    total
}

/// A float rounded to so many decimal places as Python's `round` rounds
/// one: on its exact value, a tie going to the even digit.
pub(crate) fn python_round(value: FloatType, places: u32) -> FloatType {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let (numerators, shift) = exact(&[value]);
    let scaled = &numerators[0] * BigInt::from(10u64.pow(places));
    let denominator = BigInt::from(1u8) << shift as usize;
    let mut quotient = scaled.abs() / &denominator;
    let twice_remainder = (scaled.abs() % &denominator) * 2;
    if twice_remainder > denominator
        || (twice_remainder == denominator && (&quotient % 2u8) == BigInt::from(1u8))
    {
        quotient += 1;
    }
    let digits = quotient.to_f64().unwrap_or(FloatType::INFINITY) / 10f64.powi(places as i32);
    if value < 0.0 { -digits } else { digits }
}

/// A float as Python's `repr` writes it: the fewest digits that read back
/// as the same float, `1.0` for a whole number, and an exponent, `1e-05` or
/// `1.5e+16`, where the decimal point would sit more than four places
/// before the first digit or sixteen after it.
pub(crate) fn python_repr(value: FloatType) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value == 0.0 {
        return format!("{sign}0.0");
    }
    // Rust's shortest round-trip digits, as `d.ddde±x`.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("a float written with an exponent");
    let exponent: i32 = exponent.parse().expect("a whole exponent");
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let point = exponent + 1;
    let body = if point <= -4 || point > 16 {
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        format!("{first}{fraction}e{exponent_sign}{:02}", exponent.abs())
    } else if point <= 0 {
        format!("0.{}{digits}", "0".repeat(point.unsigned_abs() as usize))
    } else if point as usize >= digits.len() {
        format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
    } else {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{whole}.{fraction}")
    };
    format!("{sign}{body}")
}

/// The mean of some values, the exact answer rounded once: Python's
/// `statistics.mean`. Nought for no values.
pub(crate) fn mean(values: &[FloatType]) -> FloatType {
    if values.is_empty() {
        return 0.0;
    }
    let (numerators, shift) = exact(values);
    let sum: BigInt = numerators.iter().sum();
    fraction_to_float(&sum, &(BigUint::from(values.len()) << shift as usize))
}

/// The sum of the squared deviations from the mean, times the count:
/// `n * sum(x^2) - sum(x)^2`, over the square of the shared power of two.
fn spread(values: &[FloatType]) -> (BigUint, u64) {
    let (numerators, shift) = exact(values);
    let count = BigInt::from(values.len());
    let sum: BigInt = numerators.iter().sum();
    let squares: BigInt = numerators.iter().map(|value| value * value).sum();
    let spread = (count * squares - &sum * &sum)
        .to_biguint()
        .unwrap_or_default();
    (spread, shift)
}

/// The population standard deviation of some values, the exact answer
/// rounded once: Python's `statistics.pstdev`. Nought for no values.
pub(crate) fn pstdev(values: &[FloatType]) -> FloatType {
    if values.is_empty() {
        return 0.0;
    }
    let (spread, shift) = spread(values);
    let count = BigUint::from(values.len());
    square_root_of_fraction(&spread, &((&count * &count) << (2 * shift) as usize))
}

/// The sample standard deviation and the mean of some values, each the
/// exact answer rounded once, as Python's `statistics.stdev` and `mean`
/// give them. The values must be at least two.
pub(crate) fn deviation_and_mean(values: &[FloatType]) -> (FloatType, FloatType) {
    let (spread, shift) = spread(values);
    let count = BigUint::from(values.len());
    let pairs = &count * (&count - BigUint::from(1u8));
    let deviation = square_root_of_fraction(&spread, &(pairs << (2 * shift) as usize));
    (deviation, mean(values))
}

/// The square root of a fraction, correctly rounded: Python's
/// `statistics._float_sqrt_of_frac`, which takes an integer square root
/// rounded to odd at twice a double's precision and lets the conversion to
/// a double round it once.
fn square_root_of_fraction(numerator: &BigUint, denominator: &BigUint) -> FloatType {
    if numerator.is_zero() {
        return 0.0;
    }
    const WIDTH: i64 = 2 * 53 + 3;
    let shift = (numerator.bits() as i64 - denominator.bits() as i64 - WIDTH).div_euclid(2);
    let rounded_to_odd = |n: &BigUint, m: &BigUint| -> BigUint {
        let root = (n / m).sqrt();
        if &root * &root * m == *n {
            root
        } else {
            root | BigUint::from(1u8)
        }
    };
    let root = if shift >= 0 {
        rounded_to_odd(numerator, &(denominator << (2 * shift) as usize))
    } else {
        rounded_to_odd(&(numerator << (-2 * shift) as usize), denominator)
    };
    let root = root
        .to_u64()
        .map_or(FloatType::INFINITY, |root| root as FloatType);
    scaled_by_power_of_two(root, shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_root_is_rounded_once() {
        let root = |n: u32, m: u32| square_root_of_fraction(&BigUint::from(n), &BigUint::from(m));
        assert_eq!(root(2, 1), (2.0 as FloatType).sqrt());
        assert_eq!(root(9, 4), 1.5);
        assert_eq!(root(0, 7), 0.0);
        // Python: statistics.stdev([1, 2, 4]) == 1.5275252316519468
        let (deviation, mean) = deviation_and_mean(&[1.0, 2.0, 4.0]);
        assert_eq!(deviation, 1.527_525_231_651_946_8);
        assert_eq!(mean, 7.0 / 3.0);
    }

    #[test]
    fn a_mean_is_rounded_once() {
        // Python: statistics.mean([60, 62, 65]) == 62.333333333333336
        assert_eq!(mean(&[60.0, 62.0, 65.0]), 62.333_333_333_333_336);
        assert_eq!(mean(&[60.5, 61.0]), 60.75);
        assert_eq!(mean(&[]), 0.0);
        // Python: statistics.mean([0.1, 0.2, 0.3]) == 0.2
        assert_eq!(mean(&[0.1, 0.2, 0.3]), 0.2);
        assert_eq!(mean(&[-1.5, 0.5]), -0.5);
    }

    #[test]
    fn a_population_deviation_is_rounded_once() {
        // Python: statistics.pstdev([0.1, 0.2, 0.4]) == 0.12472191289246472
        assert_eq!(pstdev(&[0.1, 0.2, 0.4]), 0.124_721_912_892_464_72);
        assert_eq!(pstdev(&[0.5, 0.5]), 0.0);
        assert_eq!(pstdev(&[]), 0.0);
    }

    #[test]
    fn a_round_takes_ties_to_even_on_the_exact_value() {
        // Each read off Python's round(x, 8).
        assert_eq!(python_round(0.333_984_375, 8), 0.333_984_38);
        assert_eq!(python_round(2.5e-9, 8), 0.0);
        assert_eq!(python_round(1.000_000_005, 8), 1.0);
        assert_eq!(python_round(0.123_456_785, 8), 0.123_456_78);
        assert_eq!(python_round(7.429_687_5, 8), 7.429_687_5);
        assert_eq!(python_round(-0.333_984_375, 8), -0.333_984_38);
    }

    #[test]
    fn a_float_is_written_as_python_writes_it() {
        // Each read off Python's repr.
        for (value, written) in [
            (1.0, "1.0"),
            (0.5, "0.5"),
            (0.1, "0.1"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (0.000_012_5, "1.25e-05"),
            (123_456.789, "123456.789"),
            (1e16, "1e+16"),
            (1_234_567_890_123_456.0, "1234567890123456.0"),
            (1.5e300, "1.5e+300"),
            (-2.75, "-2.75"),
            (-0.0, "-0.0"),
            (1.0 / 3.0, "0.3333333333333333"),
            (2.0 / 3.0, "0.6666666666666666"),
            (100.0, "100.0"),
        ] {
            assert_eq!(python_repr(value), written);
        }
    }

    #[test]
    fn half_steps_scale_without_moving_the_rounding() {
        let (whole, _) = deviation_and_mean(&[1.0, 2.0, 4.0]);
        let (half, mean) = deviation_and_mean(&[0.5, 1.0, 2.0]);
        assert_eq!(half * 2.0, whole);
        assert_eq!(mean, 3.5 / 3.0);
    }
}
