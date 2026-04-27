use crate::{
    base::Music21ObjectTrait,
    defaults::{IntegerType, UnsignedIntegerType},
    exception::{Exception, ExceptionResult},
    note::Note,
    pitch::Pitch,
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
            self.specifier.semitones_above_perfect()?
        } else {
            self.specifier.semitones_above_major()?
        };

        let mut semitones: IntegerType =
            ((octave_offset * 12 + semitones_start) as IntegerType) + semitones_adjust;

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

    pub(crate) fn nice_name(&self) -> String {
        format!(
            "{} {}",
            self.specifier.nice_name(),
            self.generic.nice_name()
        )
    }

    pub(crate) fn semi_simple_nice_name(&self) -> String {
        format!(
            "{} {}",
            self.specifier.nice_name(),
            self.generic.semi_simple_nice_name()
        )
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
            "Invalid diatonic interval: {in}"
        ))),
    }
}

impl IntervalBaseTrait for DiatonicInterval {
    fn transpose_note(self, note1: Note) -> ExceptionResult<Note> {
        let interval =
            super::Interval::from_diatonic_and_chromatic(self.clone(), self.get_chromatic()?)?;
        interval.transpose_note(note1)
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        let interval =
            super::Interval::from_diatonic_and_chromatic(self.clone(), self.get_chromatic()?)?;
        interval.transpose_pitch(&pitch1, false, Some(4))
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        let transposed = self.transpose_pitch(pitch1.clone())?;
        *pitch1 = transposed;
        Ok(())
    }

    fn reverse(self) -> ExceptionResult<Self>
    where
        Self: Sized,
    {
        if self.generic.undirected() == 1 {
            Ok(Self::new(
                self.specifier.inversion(),
                &GenericInterval::from_int(1)?,
            ))
        } else {
            Ok(Self::new(self.specifier, &self.generic.reverse()?))
        }
    }
}

impl Music21ObjectTrait for DiatonicInterval {}

impl ProtoM21ObjectTrait for DiatonicInterval {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diatonic_get_chromatic_major_third() {
        let generic = GenericInterval::from_int(3).unwrap();
        let diatonic = DiatonicInterval::new(Specifier::Major, &generic);
        assert_eq!(diatonic.get_chromatic().unwrap().semitones, 4);
    }

    #[test]
    fn diatonic_reverse_unison_inverts_specifier() {
        let generic = GenericInterval::from_int(1).unwrap();
        let diatonic = DiatonicInterval::new(Specifier::Augmented, &generic);
        let reversed = diatonic.reverse().unwrap();
        assert_eq!(reversed.get_chromatic().unwrap().semitones, -1);
    }
}
