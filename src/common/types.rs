use crate::defaults::{FloatType, FractionType, IntegerType};

use super::enums::offsetspecial::OffsetSpecial;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OffsetQL {
    Float(FloatType),
    Fraction(FractionType),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OffsetQLSpecial {
    Float(FloatType),
    Fraction(FractionType),
    OffsetSpecial(OffsetSpecial),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum OffsetQLIn {
    Int(IntegerType),
    Float(FloatType),
    Fraction(FractionType),
}
