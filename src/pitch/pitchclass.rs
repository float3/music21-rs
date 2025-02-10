use crate::{
    defaults::{FloatType, IntegerType, PITCH_SPACE_SIGNIFICANT_DIGITS},
    exception::{Exception, ExceptionResult},
};

use super::pitchclassstring::PitchClassString;

#[allow(clippy::enum_variant_names)]
pub(crate) enum PitchClass {
    Integer(IntegerType),
    Float(FloatType),
    PitchClassString(PitchClassString),
    String(String),
}

impl TryFrom<PitchClass> for FloatType {
    type Error = Exception;

    fn try_from(val: PitchClass) -> ExceptionResult<Self> {
        match val {
            PitchClass::Integer(i) => Ok(i as FloatType),
            PitchClass::Float(f) => Ok(f),
            PitchClass::PitchClassString(s) => Ok(s.to_number() as FloatType),
            PitchClass::String(s) => s
                .parse::<FloatType>()
                .or_else(|_| s.parse::<IntegerType>().map(|i| i as FloatType))
                .map_err(|err| Exception::PitchClass(format!("error occurred: {:?}", err.kind()))),
        }
    }
}

pub(crate) fn convert_pitch_class_to_str(pc: i32) -> String {
    // Mimic Python's modulo: always a non-negative remainder.
    let pc = pc.rem_euclid(12);
    format!("{:X}", pc)
}

pub(crate) fn convert_ps_to_oct(ps: FloatType) -> IntegerType {
    let factor = (10 as FloatType).powi(PITCH_SPACE_SIGNIFICANT_DIGITS as i32);
    let ps_rounded = (ps * factor).round() / factor;
    (ps_rounded / 12.0).floor() as IntegerType - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive() {
        assert_eq!(convert_pitch_class_to_str(3), "3");
        assert_eq!(convert_pitch_class_to_str(10), "A");
    }

    #[test]
    fn test_wraparound() {
        assert_eq!(convert_pitch_class_to_str(12), "0");
        assert_eq!(convert_pitch_class_to_str(13), "1");
    }

    #[test]
    fn test_negative() {
        // In Python: -1 % 12 == 11, so expect "B"
        assert_eq!(convert_pitch_class_to_str(-1), "B");
    }
}
