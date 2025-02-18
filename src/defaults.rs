use crate::stepname::StepName;
use fraction::GenericFraction;

// #[cfg(feature = "halfprecision")]
// mod nested {
//     pub(crate) type IntegerType = i32;
//     pub(crate) type UnsignedIntegerType = u32;
//     pub(crate) type FloatType = f32;
// }

// #[cfg(not(feature = "halfprecision"))]
// mod nested {
pub(crate) type IntegerType = i64;
pub(crate) type UnsignedIntegerType = u64;
pub(crate) type FloatType = f64;
// }

// pub(crate) use nested::{FloatType, IntegerType, UnsignedIntegerType};

pub(crate) type FractionType = GenericFraction<IntegerType>;

pub(crate) type Octave = Option<IntegerType>;

/// CD track level precision
pub(crate) const LIMIT_OFFSET_DENOMINATOR: UnsignedIntegerType = 65535;

pub(crate) const PITCH_STEP: StepName = StepName::C;
pub(crate) const PITCH_OCTAVE: UnsignedIntegerType = 4;

pub(crate) const TWELFTH_ROOT_OF_TWO: FloatType = 1.059_463_1; // 2.0 ** (1 / 12)
pub(crate) const PITCH_SPACE_SIGNIFICANT_DIGITS: UnsignedIntegerType = 6;
