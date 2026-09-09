//! Integer powers of a `GenericFraction`.
//!
//! `fraction` carries no `pow` of its own, and `num_traits::Pow` cannot be
//! implemented for `GenericFraction` here because both are foreign, so
//! raising a fraction to a power needs a local extension trait.

use fraction::GenericFraction::{self, Infinity};
use fraction::Sign;
use num::Integer;
use num_traits::{One, Zero};

use crate::defaults::IntegerType;

/// Raising a fraction to an integer power.
pub(crate) trait FractionPow {
    /// `self` raised to `exp`, by squaring.
    ///
    /// A negative exponent inverts the fraction first, so the reciprocal of
    /// nought is infinity, as it is everywhere else in `fraction`.
    ///
    /// Infinity follows `f64::powi`: an exponent of nought gives one, a
    /// negative exponent gives nought, and a negative infinity keeps its sign
    /// only for odd exponents. NaN deliberately does not — it stays NaN even
    /// at an exponent of nought, where `f64` gives one, because NaN absorbs
    /// every other operation in `fraction` and this is no place to make it
    /// the exception.
    ///
    /// # Overflow
    ///
    /// The multiplications are the backing type's, so a result too large for
    /// it overflows: over [`IntegerType`], `(3/2)^19` is the last power that
    /// fits.
    #[must_use]
    fn powi(&self, exp: IntegerType) -> Self;
}

impl<T> FractionPow for GenericFraction<T>
where
    T: Integer
        + Clone
        + PartialEq
        + One
        + Zero
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
{
    fn powi(&self, exp: IntegerType) -> Self {
        match *self {
            GenericFraction::Rational(_, ref ratio) => {
                let base = if exp < 0 {
                    GenericFraction::new(ratio.denom().clone(), ratio.numer().clone())
                } else {
                    self.clone()
                };

                // `unsigned_abs`, because negating `IntegerType::MIN` is the
                // one exponent that would overflow before any multiplying.
                let mut remaining = exp.unsigned_abs();
                let mut result = GenericFraction::new(T::one(), T::one());
                let mut squared = base;

                while remaining > 0 {
                    if remaining % 2 == 1 {
                        result *= squared.clone();
                    }
                    remaining /= 2;
                    // Squaring after the last bit is consumed produces a value
                    // nothing reads, and near the backing type's ceiling it is
                    // what overflows: `(3/2).powi(18)` fits an `i32` with room
                    // to spare while the thirty-second power does not.
                    if remaining > 0 {
                        squared = squared.clone() * squared;
                    }
                }
                result
            }
            GenericFraction::NaN => GenericFraction::NaN,
            Infinity(sign) => {
                if exp == 0 {
                    GenericFraction::new(T::one(), T::one())
                } else if exp < 0 {
                    GenericFraction::new(T::zero(), T::one())
                } else if matches!(sign, Sign::Minus) && exp % 2 != 0 {
                    Infinity(Sign::Minus)
                } else {
                    Infinity(Sign::Plus)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powi_supports_positive_zero_and_negative_exponents() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(3, 4);

        assert_eq!(frac.powi(0), GenericFraction::new(1, 1));
        assert_eq!(frac.powi(2), GenericFraction::new(9, 16));
        assert_eq!(frac.powi(-2), GenericFraction::new(16, 9));
    }

    /// `interval::pythagorean_ratio` walks up to eighteen fifths before it
    /// gives up, so every power to eighteen has to be reachable without
    /// overflowing `IntegerType`.
    #[test]
    fn powi_reaches_the_largest_power_its_caller_asks_for() {
        let fifth: GenericFraction<IntegerType> = GenericFraction::new(3, 2);

        assert_eq!(fifth.powi(16), GenericFraction::new(43_046_721, 65_536));
        assert_eq!(fifth.powi(18), GenericFraction::new(387_420_489, 262_144));
        assert_eq!(fifth.powi(19), GenericFraction::new(1_162_261_467, 524_288));
    }

    #[test]
    fn powi_leaves_nan_alone() {
        let nan: GenericFraction<IntegerType> = GenericFraction::NaN;

        assert!(matches!(nan.powi(3), GenericFraction::NaN));
        // `f64::NAN.powi(0)` is one; NaN absorbs everything else in
        // `fraction`, so here it absorbs this too.
        assert!(matches!(nan.powi(0), GenericFraction::NaN));
    }

    /// The answers `f64::powi` gives for the same exponents.
    #[test]
    fn powi_of_an_infinity_follows_the_float() {
        let plus: GenericFraction<IntegerType> = Infinity(Sign::Plus);
        let minus: GenericFraction<IntegerType> = Infinity(Sign::Minus);

        assert_eq!(plus.powi(0), GenericFraction::new(1, 1));
        assert_eq!(minus.powi(0), GenericFraction::new(1, 1));

        assert_eq!(plus.powi(-2), GenericFraction::new(0, 1));
        assert_eq!(minus.powi(-3), GenericFraction::new(0, 1));

        assert_eq!(plus.powi(2), Infinity(Sign::Plus));
        assert_eq!(minus.powi(2), Infinity(Sign::Plus));
        assert_eq!(minus.powi(3), Infinity(Sign::Minus));
    }

    #[test]
    fn powi_handles_zero_numerator_negative_exponents() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0, 5);

        assert_eq!(frac.powi(-2), Infinity(Sign::Plus));
    }

    #[test]
    fn powi_supports_other_integer_backing_types() {
        let frac: GenericFraction<i64> = GenericFraction::new(2, 3);

        assert_eq!(frac.powi(3), GenericFraction::new(8, 27));
    }

    #[test]
    fn powi_still_supports_fraction_alias_backing_type() {
        let frac: fraction::Fraction = fraction::Fraction::new(2u64, 3u64);

        assert_eq!(frac.powi(3), fraction::Fraction::new(8u32, 27u32));
    }
}
