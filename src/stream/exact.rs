//! Offsets as music21 holds them, exactly, for the comparisons music21
//! makes between a `Fraction` and a float.

use crate::defaults::FloatType;
use crate::makenotation::op_frac;

/// A number as music21 holds it, exactly: an offset snapped to a fraction
/// is a `Fraction` unless it is a binary fraction, which is a float.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Exact {
    numerator: i128,
    denominator: i128,
    /// Whether music21 holds it as a `Fraction` rather than a float.
    fraction: bool,
}

impl Exact {
    /// A float's exact value.
    pub(crate) fn of_float(value: FloatType) -> Self {
        if value == 0.0 || !value.is_finite() {
            return Self {
                numerator: 0,
                denominator: 1,
                fraction: false,
            };
        }
        let bits = value.to_bits();
        let negative = bits >> 63 == 1;
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let mantissa = if exponent == 0 {
            (bits & 0xf_ffff_ffff_ffff) << 1
        } else {
            (bits & 0xf_ffff_ffff_ffff) | 0x10_0000_0000_0000
        };
        let mut numerator = mantissa as i128;
        let mut power = exponent.max(1) - 1075;
        while numerator % 2 == 0 && power < 0 {
            numerator /= 2;
            power += 1;
        }
        let (numerator, denominator) = if power >= 0 {
            (numerator << power.min(60), 1)
        } else {
            (numerator, 1_i128 << (-power).min(100))
        };
        Self {
            numerator: if negative { -numerator } else { numerator },
            denominator,
            fraction: false,
        }
    }

    /// An offset as music21 keeps it: snapped by `opFrac`.
    pub(crate) fn held(value: FloatType) -> Self {
        let snapped = op_frac(value);
        match crate::duration::limited_fraction(snapped.abs(), 65535) {
            Some((numerator, denominator))
                if denominator & (denominator - 1) != 0
                    && (numerator as FloatType / denominator as FloatType - snapped.abs())
                        .abs()
                        < 1e-9 =>
            {
                Self {
                    numerator: if snapped < 0.0 { -numerator } else { numerator },
                    denominator,
                    fraction: true,
                }
            }
            _ => Self::of_float(snapped),
        }
    }

    /// A Python `int`, which adds to a `Fraction` exactly.
    pub(crate) fn integer(value: i64) -> Self {
        Self {
            numerator: i128::from(value),
            denominator: 1,
            fraction: true,
        }
    }

    /// Two numbers music21 holds added as Python adds them: exactly when
    /// both are fractions, and as floats otherwise.
    pub(crate) fn plus(self, other: Self) -> Self {
        if self.fraction && other.fraction {
            Self {
                numerator: self.numerator * other.denominator + other.numerator * self.denominator,
                denominator: self.denominator * other.denominator,
                fraction: true,
            }
        } else {
            Self::of_float(self.to_float() + other.to_float())
        }
    }

    pub(crate) fn to_float(self) -> FloatType {
        self.numerator as FloatType / self.denominator as FloatType
    }

    pub(crate) fn cmp(self, other: Self) -> std::cmp::Ordering {
        (self.numerator * other.denominator).cmp(&(other.numerator * self.denominator))
    }
}

/// music21's `getElementsByOffset` at one offset, given as music21 holds
/// it: an element there, or of no length ending there. The offset is
/// snapped as the start of the span but is its end as given, so a float
/// short of the fraction an element stands at finds nothing.
pub(crate) fn at_offset(offset: FloatType, length: FloatType, at: Exact) -> bool {
    let start = Exact::held(at.to_float());
    let held = Exact::held(offset);
    if held.cmp(at).is_gt() {
        return false;
    }
    if Exact::held(offset + length).cmp(start).is_lt() {
        return false;
    }
    if length == 0.0 {
        return true;
    }
    !held.cmp(start).is_lt()
}
