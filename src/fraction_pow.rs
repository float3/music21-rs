use fraction::GenericFraction;
use num::Integer;
use num_traits::{FromPrimitive, One, Signed, Zero};

pub trait FractionPow<E> {
    type Output;
    fn pow(&self, exp: E) -> Self::Output;
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
    E: Signed + Integer + Copy + FromPrimitive,
{
    type Output = Option<Self>;
    fn pow(&self, exp: E) -> Self::Output {
        let base = if exp < E::zero() {
            let numer = self.numer()?;
            if *numer == T::zero() {
                return None;
            }
            let denom = self.denom()?;
            GenericFraction::new(denom.clone(), numer.clone())
        } else {
            self.clone()
        };

        let mut exp_abs = if exp < E::zero() { -exp } else { exp };
        let mut result = GenericFraction::new(T::one(), T::one());
        let two = E::from_i32(2)?;

        let mut base_pow = base;
        while exp_abs > E::zero() {
            if exp_abs % two == E::one() {
                result = result * base_pow.clone();
            }
            base_pow = base_pow.clone() * base_pow;
            exp_abs = exp_abs / two;
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use fraction::{Fraction, GenericFraction::Infinity, Sign::Plus};

    use crate::defaults::IntegerType;

    use super::*;

    #[test]
    fn test_pow_positive_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8u32, 27u32);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn fraction_i64() {
        let frac: GenericFraction<i64> = GenericFraction::new(2, 3);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8, 27);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn fraction_u32() {
        let frac: GenericFraction<u32> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8u32, 27u32);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn fraction() {
        let frac: Fraction = Fraction::new(2u64, 3u64);
        let result = frac.pow(3);
        let expected = Fraction::new(8u64, 27u64);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of 3 should be 8/27"
        );
    }

    #[test]
    fn test_pow_negative_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(-2);
        let expected = GenericFraction::new(9u32, 4u32);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of -2 should be 9/4"
        );
    }

    #[test]
    fn test_pow_zero_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(0);
        let expected = GenericFraction::one();
        assert_eq!(
            result,
            Some(expected),
            "Any fraction raised to the power of 0 should be 1/1"
        );
    }

    #[test]
    fn test_pow_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(5u32, 7u32);
        let result = frac.pow(1);
        let expected = frac;
        assert_eq!(
            result,
            Some(expected),
            "Any fraction raised to the power of 1 should be itself"
        );
    }

    #[test]
    fn test_pow_negative_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(4u32, 5u32);
        let result = frac.pow(-1);
        let expected = GenericFraction::new(5u32, 4u32);
        assert_eq!(
            result,
            Some(expected),
            "4/5 raised to the power of -1 should be 5/4"
        );
    }

    #[test]
    fn test_pow_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(10);
        let expected = GenericFraction::new(1024u32, 59049u32);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of 10 should be 1024/59049"
        );
    }

    #[test]
    fn test_pow_negative_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(-3);
        let expected = GenericFraction::new(27u32, 8u32);
        assert_eq!(
            result,
            Some(expected),
            "2/3 raised to the power of -3 should be 27/8"
        );
    }

    #[test]
    fn test_pow_zero_fraction() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0u32, 5u32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(0u32, 1u32);
        assert_eq!(
            result,
            Some(expected),
            "0/5 raised to the power of 3 should be 0/1"
        );
    }

    #[test]
    fn test_pow_negative_exponent_zero_fraction() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0u32, 5u32);
        let result = frac.pow(-2);
        let expected = Infinity(Plus); // Adjusted expectation
        assert_eq!(
            result,
            Some(expected),
            "0/5 raised to the power of -2 should be Infinity(Plus)"
        );
    }
}
