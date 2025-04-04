use crate::{
    base::Music21ObjectTrait,
    defaults::{IntegerType, UnsignedIntegerType},
    exception::{Exception, ExceptionResult},
    prebase::ProtoM21ObjectTrait,
};

use super::{
    GenericInterval, IntervalBaseTrait, chromaticinterval::ChromaticInterval, direction::Direction,
    specifier::Specifier,
};

#[derive(Clone, Debug)]
pub(crate) struct DiatonicInterval {
    pub(crate) generic: GenericInterval,
    specifier: Specifier,
}

impl DiatonicInterval {
    pub(crate) fn get_chromatic(&self) -> ExceptionResult<ChromaticInterval> {
        let octave_offset = (self.generic.staff_distance().abs() / 7) as UnsignedIntegerType;
        let semitones_start =
            semitones_generic(self.generic.simple_undirected() as UnsignedIntegerType)?;

        let semitones_adjust = if self.generic.is_perfectable() {
            self.specifier.semitones_above_perfect()
        } else {
            self.specifier.semitones_above_major()
        };

        let mut semitones: IntegerType =
            ((octave_offset * 12) + semitones_start + semitones_adjust) as IntegerType;

        if self.generic.direction() == Direction::Descending {
            semitones *= -1;
        }

        Ok(ChromaticInterval::new(semitones))
    }

    pub(crate) fn new(specifier: Specifier, generic: &GenericInterval) -> Self {
        Self {
            generic: generic.clone(),
            specifier,
        }
    }
}

fn semitones_generic(r#in: UnsignedIntegerType) -> ExceptionResult<UnsignedIntegerType> {
    match r#in {
        1 => Ok(0),
        2 => Ok(2),
        3 => Ok(4),
        4 => Ok(5),
        5 => Ok(7),
        6 => Ok(9),
        7 => Ok(11),
        _ => Err(Exception::Interval(format!(
            "Invalid diatonic interval: {}",
            r#in
        ))),
    }
}

impl IntervalBaseTrait for DiatonicInterval {
    fn transpose_note(
        self,
        note1: crate::note::Note,
    ) -> crate::exception::ExceptionResult<crate::note::Note> {
        todo!()
    }

    fn transpose_pitch(
        self,
        pitch1: crate::pitch::Pitch,
    ) -> crate::exception::ExceptionResult<crate::pitch::Pitch> {
        todo!()
    }

    fn transpose_pitch_in_place(
        self,
        pitch1: &mut crate::pitch::Pitch,
    ) -> crate::exception::ExceptionResult<()> {
        todo!()
    }

    fn reverse(self) -> crate::exception::ExceptionResult<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl Music21ObjectTrait for DiatonicInterval {}

impl ProtoM21ObjectTrait for DiatonicInterval {}
