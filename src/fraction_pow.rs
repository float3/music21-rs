use crate::defaults::IntegerType;

use fraction::GenericFraction;
use num::Integer;
use num_traits::{One, Pow, Zero};
use std::cmp::Ordering;

pub(crate) trait FractionPow {
    type Output;

    fn pow(self, exponent: IntegerType) -> Self::Output;
}

impl<T> FractionPow for GenericFraction<T>
where
    T: Pow<u32, Output = T> + Clone + PartialEq + Zero + One + Integer,
{
    type Output = GenericFraction<T>;

    fn pow(self, exponent: IntegerType) -> GenericFraction<T> {
        let (numer, denom) = match exponent.cmp(&0) {
            Ordering::Greater => (self.numer(), self.denom()),
            Ordering::Less => (self.denom(), self.numer()),
            Ordering::Equal => return GenericFraction::one(),
        };

        let numer = match numer {
            Some(n) => n.clone().pow(exponent.abs() as u32),
            None => {
                eprintln!("Error: numerator is None");
                return GenericFraction::zero();
            }
        };

        let denom = match denom {
            Some(d) => d.clone().pow(exponent.abs() as u32),
            None => {
                eprintln!("Error: denominator is None");
                return GenericFraction::zero();
            }
        };

        if exponent.is_negative() {
            GenericFraction::new(denom, numer)
        } else {
            GenericFraction::new(numer, denom)
        }
    }
}

#[cfg(test)]
mod tests {
    use fraction::{Fraction, GenericFraction::Infinity, Sign::Plus};

    use super::*;

    #[test]
    fn test_pow_positive_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8, 27);
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

    fn fraction_u32() {
        let frac: GenericFraction<u32> = GenericFraction::new(2u32, 3u32);
        let result = frac.pow(3);
        let expected = GenericFraction::new(8u32, 27u32);
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
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = frac.pow(-2);
        let expected = GenericFraction::new(9, 4);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of -2 should be 9/4"
        );
    }

    #[test]
    fn test_pow_zero_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = frac.pow(0);
        let expected = GenericFraction::one();
        assert_eq!(
            result, expected,
            "Any fraction raised to the power of 0 should be 1/1"
        );
    }

    #[test]
    fn test_pow_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(5, 7);
        let result = frac.pow(1);
        let expected = frac.clone();
        assert_eq!(
            result, expected,
            "Any fraction raised to the power of 1 should be itself"
        );
    }

    #[test]
    fn test_pow_negative_one_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(4, 5);
        let result = frac.pow(-1);
        let expected = GenericFraction::new(5, 4);
        assert_eq!(
            result, expected,
            "4/5 raised to the power of -1 should be 5/4"
        );
    }

    #[test]
    fn test_pow_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = frac.pow(10);
        let expected = GenericFraction::new(1024, 59049);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of 10 should be 1024/59049"
        );
    }

    #[test]
    fn test_pow_negative_large_exponent() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(2, 3);
        let result = frac.pow(-3);
        let expected = GenericFraction::new(27, 8);
        assert_eq!(
            result, expected,
            "2/3 raised to the power of -3 should be 27/8"
        );
    }

    #[test]
    fn test_pow_zero_fraction() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0, 5);
        let result = frac.pow(3);
        let expected = GenericFraction::new(0, 1);
        assert_eq!(
            result, expected,
            "0/5 raised to the power of 3 should be 0/1"
        );
    }

    #[test]
    fn test_pow_negative_exponent_zero_fraction() {
        let frac: GenericFraction<IntegerType> = GenericFraction::new(0, 5);
        let result = frac.pow(-2);
        let expected = Infinity(Plus); // Adjusted expectation
        assert_eq!(
            result, expected,
            "0/5 raised to the power of -2 should be Infinity(Plus)"
        );
    }
}
