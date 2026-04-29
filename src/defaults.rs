use crate::stepname::StepName;
use fraction::GenericFraction;

/// Signed integer type used by music21-rs APIs.
pub type IntegerType = i32;
/// Unsigned integer type used by count-like music21-rs APIs.
pub type UnsignedIntegerType = u32;
/// Floating-point type used for pitch space, quarter lengths, and frequencies.
pub type FloatType = f64;

/// Fraction type used by ratio and interval helpers.
pub type FractionType = GenericFraction<IntegerType>;

/// Optional octave number, matching music21's absent-octave representation.
pub type Octave = Option<IntegerType>;

/// CD track level precision
pub(crate) const LIMIT_OFFSET_DENOMINATOR: UnsignedIntegerType = 65535;

pub(crate) const PITCH_STEP: StepName = StepName::C;
pub(crate) const PITCH_OCTAVE: UnsignedIntegerType = 4;

pub(crate) const TWELFTH_ROOT_OF_TWO: FloatType = 1.059_463_1; // 2.0 ** (1 / 12)
pub(crate) const PITCH_SPACE_SIGNIFICANT_DIGITS: UnsignedIntegerType = 6;
