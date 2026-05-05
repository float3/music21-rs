use fraction::GenericFraction::{self, Infinity};
use num::Integer;
use num_traits::{One, Signed, Zero};

pub(crate) trait FractionPow<I> {
    fn powi(&self, exp: I) -> Self;
}

impl<T, I> FractionPow<I> for GenericFraction<T>
where
    T: Integer
        + Clone
        + PartialEq
        + One
        + Zero
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
    I: Signed + Integer + Copy + From<crate::defaults::IntegerType>,
{
    fn powi(&self, exp: I) -> Self {
        match *self {
            GenericFraction::Rational(_, ref ratio) => {
                let base = if exp < I::zero() {
                    GenericFraction::new(ratio.denom().clone(), ratio.numer().clone())
                } else {
                    self.clone()
                };

                let mut exp_abs = if exp < I::zero() { -exp } else { exp };
                let mut result = GenericFraction::new(T::one(), T::one());
                let two = I::from(2);
                let one = I::from(1);
                let mut base_pow = base;

                while exp_abs > I::zero() {
                    if exp_abs % two == one {
                        result *= base_pow.clone();
                    }
                    base_pow = base_pow.clone() * base_pow;
                    exp_abs = exp_abs / two;
                }
                result
            }
            GenericFraction::NaN => GenericFraction::NaN,
            Infinity(sign) => Infinity(sign),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defaults::{FloatType, IntegerType};

    #[test]
    fn powi_supports_positive_zero_and_negative_exponents() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(3, 4);
        assert_eq!(
            FractionPow::<IntegerType>::powi(&frac, 0),
            GenericFraction::new(1, 1)
        );
        assert_eq!(
            FractionPow::<IntegerType>::powi(&frac, 2),
            GenericFraction::new(9, 16)
        );
        assert_eq!(
            FractionPow::<IntegerType>::powi(&frac, -2),
            GenericFraction::new(16, 9)
        );
    }

    #[test]
    fn powi_preserves_nan_and_infinity() {
        let nan_frac: GenericFraction<IntegerType> = GenericFraction::NaN;
        assert!(matches!(
            FractionPow::<IntegerType>::powi(&nan_frac, 3),
            GenericFraction::NaN
        ));

        let pos_inf: GenericFraction<IntegerType> = Infinity(fraction::Sign::Plus);
        assert_eq!(FractionPow::<IntegerType>::powi(&pos_inf, -2), pos_inf);
    }

    #[test]
    fn powi_handles_zero_numerator_negative_exponents() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0, 5);
        assert_eq!(
            FractionPow::<IntegerType>::powi(&frac, -2),
            Infinity(fraction::Sign::Plus)
        );
    }

    #[test]
    fn powi_supports_other_integer_backing_types() {
        let frac: GenericFraction<i64> = GenericFraction::new(2, 3);
        assert_eq!(
            FractionPow::<i64>::powi(&frac, 3),
            GenericFraction::new(8, 27)
        );
    }

    #[test]
    fn powi_still_supports_fraction_alias_backing_type() {
        let frac: fraction::Fraction = fraction::Fraction::new(2u64, 3u64);
        assert_eq!(
            FractionPow::<i64>::powi(&frac, 3),
            fraction::Fraction::new(8u32, 27u32)
        );
    }

    #[test]
    fn powi_type_parameter_stays_compatible_with_float_alias_users() {
        let _: FloatType = 0.0;
    }
}
