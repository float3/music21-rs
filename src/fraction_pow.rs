use fraction::GenericFraction::{self, Infinity};
use num::Integer;
use num_traits::{FromPrimitive, One, Signed, Zero};

pub(crate) trait FractionPow<E> {
    fn pow(&self, exp: E) -> Self;
}

impl<T, E> FractionPow<E> for GenericFraction<T>
where
    T: Integer
        + Clone
        + PartialEq
        + One
        + Zero
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>,
    E: Signed + Integer + Copy + FromPrimitive + From<i32>,
{
    fn pow(&self, exp: E) -> Self {
        match *self {
            GenericFraction::Rational(sign, ref ratio) => {
                let base = if exp < E::zero() {
                    GenericFraction::new(ratio.denom().clone(), ratio.numer().clone())
                } else {
                    self.clone()
                };

                let mut exp_abs = if exp < E::zero() { -exp } else { exp };
                let mut result = GenericFraction::new(T::one(), T::one());
                let two = E::from(2);

                let mut base_pow = base;
                while exp_abs > E::zero() {
                    if exp_abs % two == E::one() {
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
    use fraction::{Fraction, GenericFraction::Infinity, Sign::Plus};

    use crate::defaults::IntegerType;

    use super::*;

    #[test]
    fn test_pow_positive_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2i32, 3i32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8i32, 27i32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn fraction_i64() {
        let frac: GenericFraction<i64> = GenericFraction::new(2, 3);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8, 27);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn fraction_i32() {
        let frac: GenericFraction<i32> = GenericFraction::new(2i32, 3i32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8i32, 27i32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn fraction() {
        let frac: Fraction = Fraction::new(2u64, 3u64);
        let result = frac.pow(3);
        let expected = Fraction::new(8u64, 27u64);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn test_pow_negative_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2i32, 3i32);
        let result = frac.pow(-2);
        let expected = GenericFraction::new(9i32, 4i32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of -2 should be 9/4"
        );
    }

    #[test]
    fn test_pow_zero_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2i32, 3i32);
        let result = frac.pow(0);
        let expected = GenericFraction::one();
        assert_eq!(
            result, expected,
            "Any fraction raised to the power of 0 should be 1/1"
        );
    }

    #[test]
    fn test_pow_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(5i32, 7i32);
        let result = frac.pow(1);
        let expected = frac;
        assert_eq!(
            result, expected,
            "Any fraction raised to the power of 1 should be itself"
        );
    }

    #[test]
    fn test_pow_negative_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(4i32, 5i32);
        let result = frac.pow(-1);
        let expected = GenericFraction::new(5i32, 4i32);
        assert_eq!(
            result, expected,
            "4/5 raised to the power of -1 should be 5/4"
        );
    }

    #[test]
    fn test_pow_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2i32, 3i32);
        let result = frac.pow(10);
        let expected = GenericFraction::new(1024i32, 59049i32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 10 should be 1024/59049"
        );
    }

    #[test]
    fn test_pow_negative_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2i32, 3i32);
        let result = frac.pow(-3);
        let expected = GenericFraction::new(27i32, 8i32);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of -3 should be 27/8"
        );
    }

    #[test]
    fn test_pow_zero_fraction() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0i32, 5i32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(0i32, 1i32);
        assert_eq!(
            result, expected,
            "0/5 raised to the power of 3 should be 0/1"
        );
    }

    #[test]
    fn test_pow_negative_exponent_zero_fraction() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0i32, 5i32);
        let result = frac.pow(-2);
        let expected = Infinity(Plus); // Adjusted expectation
        assert_eq!(
            result, expected,
            "0/5 raised to the power of -2 should be Infinity(Plus)"
        );
    }
}
