use fraction::{
    GenericFraction::{self, Infinity},
    Sign,
};
use num::{Float, Integer, Unsigned};
use num_traits::{FromPrimitive, One, Signed, Zero};

pub(crate) trait FractionPow<I, F, U> {
    fn pow(&self, exp: U) -> Self;
    fn powi(&self, exp: I) -> Self;
    fn powf(&self, exp: F) -> Self;
}

impl<T, I, F, U> FractionPow<I, F, U> for GenericFraction<T>
where
    T: Integer
        + Clone
        + PartialEq
        + One
        + Zero
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
    I: Signed + Integer + Copy + FromPrimitive + From<i32>,
    U: Unsigned + Integer + Copy + FromPrimitive + From<u32>,
    F: Float + Copy + FromPrimitive,
{
    fn pow(&self, exp: U) -> Self {
        match *self {
            GenericFraction::Rational(_, ref ratio) => {
                let mut exp_val = exp;
                let mut result = GenericFraction::new(T::one(), T::one());
                let two = U::from(2);
                let zero = U::from(0);
                let one = U::from(1);
                let mut base = self.clone();
                while exp_val > zero {
                    if exp_val % two == one {
                        result *= base.clone();
                    }
                    base = base.clone() * base;
                    exp_val = exp_val / two;
                }
                result
            }
            GenericFraction::NaN => GenericFraction::NaN,
            Infinity(sign) => Infinity(sign),
        }
    }

    fn powi(&self, exp: I) -> Self {
        match *self {
            GenericFraction::Rational(_, ref ratio) => {
                // For negative exponent, invert the fraction.
                let base = if exp < I::zero() {
                    GenericFraction::new(ratio.denom().clone(), ratio.numer().clone())
                } else {
                    self.clone()
                };

                let mut exp_abs = if exp < I::zero() { -exp } else { exp };
                let mut result = GenericFraction::new(T::one(), T::one());
                let two = I::from(2);
                let zero = I::zero();
                let one = I::from(1);
                let mut base_pow = base;
                while exp_abs > zero {
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

    fn powf(&self, exp: F) -> Self {
        match *self {
            GenericFraction::Rational(_, _) => {
                if exp.fract() == F::zero() {
                    if let Some(exp_i32) = exp.to_i32() {
                        if let Some(exp_i) = I::from_i32(exp_i32) {
                            <Self as FractionPow<I, F, U>>::powi(self, exp_i)
                        } else {
                            GenericFraction::NaN
                        }
                    } else {
                        GenericFraction::NaN
                    }
                } else {
                    GenericFraction::NaN
                }
            }
            GenericFraction::Infinity(sign) => {
                let zero = F::zero();
                if exp > zero {
                    if exp.fract() == zero {
                        if let Some(exp_i32) = exp.to_i32() {
                            let new_sign = if sign == Sign::Minus && exp_i32 % 2 == 0 {
                                Sign::Plus
                            } else {
                                sign
                            };
                            Infinity(new_sign)
                        } else {
                            GenericFraction::NaN
                        }
                    } else {
                        GenericFraction::NaN
                    }
                } else if exp < zero {
                    GenericFraction::new(T::zero(), T::one())
                } else {
                    GenericFraction::new(T::one(), T::one())
                }
            }
            GenericFraction::NaN => GenericFraction::NaN,
        }
    }
}

#[cfg(test)]
mod tests {
    use fraction::Fraction;

    use super::*;
    use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};

    #[test]
    fn test_pow_unsigned() {
        let frac: GenericFraction<i32> = GenericFraction::new(2, 3);
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &frac,
                0 as UnsignedIntegerType
            ),
            GenericFraction::new(1, 1)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &frac,
                1 as UnsignedIntegerType
            ),
            GenericFraction::new(2, 3)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &frac,
                3 as UnsignedIntegerType
            ),
            GenericFraction::new(8, 27)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &frac,
                4 as UnsignedIntegerType
            ),
            GenericFraction::new(16, 81)
        );
    }

    #[test]
    fn test_pow_signed() {
        let frac: GenericFraction<i32> = GenericFraction::new(3, 4);
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, 0),
            GenericFraction::new(1, 1)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, 2),
            GenericFraction::new(9, 16)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, -2),
            GenericFraction::new(16, 9)
        );
    }

    #[test]
    fn test_pow_float() {
        let frac: GenericFraction<i32> = GenericFraction::new(3, 5);
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&frac, 0.0),
            GenericFraction::new(1, 1)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&frac, 2.0),
            GenericFraction::new(9, 25)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&frac, -1.0),
            GenericFraction::new(5, 3)
        );
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&frac, 1.5);
        match result {
            GenericFraction::NaN => {}
            _ => panic!("Expected NaN for non-integer exponent, got {result:?}"),
        }
    }

    #[test]
    fn test_nan_behavior() {
        let nan_frac: GenericFraction<i32> = GenericFraction::NaN;
        assert!(matches!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &nan_frac,
                5 as UnsignedIntegerType
            ),
            GenericFraction::NaN
        ));
        assert!(matches!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&nan_frac, 3),
            GenericFraction::NaN
        ));
        assert!(matches!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&nan_frac, 2.0),
            GenericFraction::NaN
        ));
    }

    #[test]
    fn test_infinity_behavior() {
        let pos_inf: GenericFraction<i32> = Infinity(Sign::Plus);
        let neg_inf: GenericFraction<i32> = Infinity(Sign::Minus);

        // For pow and powi, Infinity returns Infinity with the same sign.
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &pos_inf,
                3 as UnsignedIntegerType
            ),
            pos_inf
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&pos_inf, -2),
            pos_inf
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(
                &neg_inf,
                2 as UnsignedIntegerType
            ),
            neg_inf
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&neg_inf, -3),
            neg_inf
        );

        // For powf:
        // Exponent > 0 returns Infinity.
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&pos_inf, 2.0),
            pos_inf
        );
        // Exponent < 0 returns 0 (i.e. 0/1).
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&pos_inf, -2.0),
            GenericFraction::new(0, 1)
        );
        // Exponent == 0 returns 1 (i.e. 1/1).
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&pos_inf, 0.0),
            GenericFraction::new(1, 1)
        );

        // Repeat for negative infinity.
        // According to current behavior, neg_inf.powf(2.0) yields pos_inf.
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&neg_inf, 2.0),
            pos_inf
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&neg_inf, -2.0),
            GenericFraction::new(0, 1)
        );
        assert_eq!(
            FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powf(&neg_inf, 0.0),
            GenericFraction::new(1, 1)
        );
    }

    #[test]
    fn test_method_pow_positive_exponent() {
        let frac: GenericFraction<UnsignedIntegerType> = GenericFraction::new(2u32, 3u32);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(&frac, 3);
        let expected = GenericFraction::new(8u32, 27u32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn test_method_pow_negative_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, -2);
        let expected = GenericFraction::new(9, 4);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of -2 should be 9/4"
        );
    }

    #[test]
    fn test_method_pow_zero_exponent() {
        let frac: GenericFraction<UnsignedIntegerType> = GenericFraction::new(2u32, 3u32);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(&frac, 0);
        let expected = GenericFraction::one();
        assert_eq!(
            result, expected,
            "Any fraction raised to the power of 0 should be 1/1"
        );
    }

    #[test]
    fn test_method_pow_one_exponent() {
        let frac: GenericFraction<UnsignedIntegerType> = GenericFraction::new(5u32, 7u32);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(&frac, 1);
        // Should return itself.
        assert_eq!(
            result, frac,
            "Any fraction raised to the power of 1 should be itself"
        );
    }

    #[test]
    fn test_method_pow_negative_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(4, 5);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, -1);
        let expected = GenericFraction::new(5, 4);
        assert_eq!(
            result, expected,
            "4/5 raised to the power of -1 should be 5/4"
        );
    }

    #[test]
    fn test_method_pow_large_exponent() {
        let frac: GenericFraction<UnsignedIntegerType> = GenericFraction::new(2u32, 3u32);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(&frac, 10);
        let expected = GenericFraction::new(1024u32, 59049u32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 10 should be 1024/59049"
        );
    }

    #[test]
    fn test_method_pow_negative_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, -3);
        let expected = GenericFraction::new(27, 8);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of -3 should be 27/8"
        );
    }

    // ========================================================
    // Zero Numerator Cases
    // ========================================================

    #[test]
    fn test_zero_numerator_positive_exponent() {
        let frac: GenericFraction<UnsignedIntegerType> = GenericFraction::new(0u32, 5u32);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::pow(&frac, 3);
        let expected = GenericFraction::new(0u32, 1u32);
        assert_eq!(
            result, expected,
            "0/5 raised to the power of 3 should be 0/1"
        );
    }

    #[test]
    fn test_zero_numerator_negative_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0, 5);
        let result = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(&frac, -2);
        // Dividing by zero in the reciprocal should result in Infinity.
        let expected = Infinity(Sign::Plus);
        assert_eq!(
            result, expected,
            "0/5 raised to the power of -2 should be Infinity(Plus)"
        );
    }

    // ========================================================
    // Underlying Type Tests
    // ========================================================

    #[test]
    fn test_underlying_type_i32() {
        let frac: GenericFraction<i32> = GenericFraction::new(2, 3);
        let result = FractionPow::<i32, FloatType, UnsignedIntegerType>::powi(&frac, 3);
        let expected = GenericFraction::new(8, 27);
        assert_eq!(
            result, expected,
            "GenericFraction<i32>: 2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn test_underlying_type_i64() {
        let frac: GenericFraction<i64> = GenericFraction::new(2, 3);
        let result = FractionPow::<i64, FloatType, UnsignedIntegerType>::powi(&frac, 3);
        let expected = GenericFraction::new(8, 27);
        assert_eq!(
            result, expected,
            "GenericFraction<i64>: 2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn test_underlying_type_u64() {
        // Assuming that `Fraction` is defined as GenericFraction<u64> or an alias.
        let frac: Fraction = Fraction::new(2u64, 3u64);
        let result = FractionPow::<i64, FloatType, u64>::pow(&frac, 3u64);
        let expected = Fraction::new(8u32, 27u32);
        assert_eq!(
            result, expected,
            "Fraction (u64): 2/3 raised to the power of 3 should be 8/27"
        );
    }
}
