use fraction::GenericFraction;

use crate::stepname::StepName;

pub(crate) type IntegerType = i64;
pub(crate) type UnsignedIntegerType = u64;
pub(crate) type FloatType = f64;
pub(crate) type FractionType = GenericFraction<IntegerType>;

pub(crate) type Octave = Option<IntegerType>;
pub(crate) const PITCH_STEP: StepName = StepName::C;
pub(crate) const PITCH_OCTAVE: IntegerType = 4;

pub(crate) const TWELFTH_ROOT_OF_TWO: FloatType = 1.0594630943592953; // 2.0 ** (1 / 12)
pub(crate) const PITCH_SPACE_SIGNIFICANT_DIGITS: IntegerType = 6;
