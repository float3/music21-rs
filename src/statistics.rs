//! Python's `statistics` module where music21 leans on it: each answer the
//! exact one, rounded once.

use num::{BigUint, ToPrimitive};

use crate::defaults::FloatType;

/// Some values as whole numbers once scaled by a power of two, with the
/// scale: every interval a semitone or a quarter tone wide, every pitch on
/// a key. Scaling by a power of two leaves the rounding where it was.
fn scaled(values: &[FloatType]) -> Option<(FloatType, Vec<i128>)> {
    (0..=40).find_map(|power| {
        let scale = (2.0 as FloatType).powi(power);
        values
            .iter()
            .map(|value| {
                let whole = value * scale;
                (whole.fract() == 0.0 && whole.abs() < 2e15).then_some(whole as i128)
            })
            .collect::<Option<Vec<i128>>>()
            .map(|scaled| (scale, scaled))
    })
}

/// The mean of some values, the exact answer rounded once: Python's
/// `statistics.mean`. Nought for no values.
pub(crate) fn mean(values: &[FloatType]) -> FloatType {
    if values.is_empty() {
        return 0.0;
    }
    match scaled(values) {
        Some((scale, scaled)) => {
            scaled.iter().sum::<i128>() as FloatType / values.len() as FloatType / scale
        }
        None => values.iter().sum::<FloatType>() / values.len() as FloatType,
    }
}

/// The sample standard deviation and the mean of some values, each the
/// exact answer rounded once, as Python's `statistics.stdev` and `mean`
/// give them. Values that are whole numbers once scaled by a power of two
/// -- every interval a semitone or a quarter tone wide -- are summed as
/// integers; scaling by a power of two leaves the rounding where it was.
pub(crate) fn deviation_and_mean(values: &[FloatType]) -> (FloatType, FloatType) {
    let count = values.len() as i128;
    let Some((scale, scaled)) = scaled(values) else {
        let mean = values.iter().sum::<FloatType>() / count as FloatType;
        let squares: FloatType = values.iter().map(|value| (value - mean).powi(2)).sum();
        return ((squares / (count - 1) as FloatType).sqrt(), mean);
    };
    let sum: i128 = scaled.iter().sum();
    let squares: i128 = scaled.iter().map(|value| value * value).sum();
    // The variance is (n * sum of squares - sum^2) / (n * (n - 1)).
    let numerator = (count * squares - sum * sum) as u128;
    let denominator = (count * (count - 1)) as u128;
    let deviation = square_root_of_fraction(numerator, denominator);
    let mean = sum as FloatType / count as FloatType;
    (deviation / scale, mean / scale)
}

/// The square root of a fraction, correctly rounded: Python's
/// `statistics._float_sqrt_of_frac`, which takes an integer square root
/// rounded to odd at twice a double's precision and lets the conversion to
/// a double round it once.
fn square_root_of_fraction(numerator: u128, denominator: u128) -> FloatType {
    if numerator == 0 {
        return 0.0;
    }
    const WIDTH: i64 = 2 * 53 + 3;
    let n = BigUint::from(numerator);
    let m = BigUint::from(denominator);
    let shift = (n.bits() as i64 - m.bits() as i64 - WIDTH).div_euclid(2);
    let rounded_to_odd = |n: &BigUint, m: &BigUint| -> BigUint {
        let root = (n / m).sqrt();
        let exact = &root * &root * m == *n;
        if exact {
            root
        } else {
            root | BigUint::from(1u8)
        }
    };
    if shift >= 0 {
        let root = rounded_to_odd(&n, &(m << (2 * shift) as usize));
        let root = root
            .to_u64()
            .map_or(FloatType::INFINITY, |root| root as FloatType);
        root * (2.0 as FloatType).powi(shift as i32)
    } else {
        let root = rounded_to_odd(&(n << (-2 * shift) as usize), &m);
        let root = root
            .to_u64()
            .map_or(FloatType::INFINITY, |root| root as FloatType);
        root / (2.0 as FloatType).powi((-shift) as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_root_is_rounded_once() {
        assert_eq!(square_root_of_fraction(2, 1), (2.0 as FloatType).sqrt());
        assert_eq!(square_root_of_fraction(9, 4), 1.5);
        assert_eq!(square_root_of_fraction(0, 7), 0.0);
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
    }

    #[test]
    fn half_steps_scale_without_moving_the_rounding() {
        let (whole, _) = deviation_and_mean(&[1.0, 2.0, 4.0]);
        let (half, mean) = deviation_and_mean(&[0.5, 1.0, 2.0]);
        assert_eq!(half * 2.0, whole);
        assert_eq!(mean, 3.5 / 3.0);
    }
}
