use crate::{
    base::Music21ObjectTrait,
    defaults::{FloatType, IntegerType},
    error::Result,
    note::Note,
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

use super::{IntervalBaseTrait, diatonicinterval::DiatonicInterval, intervalbase::IntervalBase};

#[derive(Clone, Debug)]
pub(crate) struct ChromaticInterval {
    intervalbase: IntervalBase,
    pub(crate) semitones: IntegerType,
}

impl ChromaticInterval {
    pub(crate) fn new(semitones: IntegerType) -> Self {
        Self {
            intervalbase: IntervalBase::new(),
            semitones,
        }
    }

    pub(crate) fn get_diatonic(&self) -> DiatonicInterval {
        let (specifier, generic) = super::convert_semitone_to_specifier_generic(self.semitones);
        let generic = super::GenericInterval::from_int(generic)
            .unwrap_or_else(|_| super::GenericInterval::from_int(1).expect("P1 is valid"));
        DiatonicInterval::new(specifier, &generic)
    }
}

impl IntervalBaseTrait for ChromaticInterval {
    fn transpose_note(self, note1: Note) -> Result<Note> {
        let mut cloned = note1.clone();
        cloned._pitch = self.transpose_pitch(note1._pitch)?;
        Ok(cloned)
    }

    fn transpose_pitch(self, pitch1: Pitch) -> Result<Pitch> {
        let mut p_out = Pitch::new(
            Some((pitch1.ps() + self.semitones as FloatType).round() as IntegerType),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )?;
        if pitch1.octave().is_none() {
            p_out.octave_setter(None);
        }
        Ok(p_out)
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> Result<()> {
        *pitch1 = self.transpose_pitch(pitch1.clone())?;
        Ok(())
    }

    fn reverse(self) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self::new(-self.semitones))
    }
}

impl Music21ObjectTrait for ChromaticInterval {}

impl ProtoM21ObjectTrait for ChromaticInterval {}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::new(
            Some(name.to_string()),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
        .expect("valid pitch")
    }

    #[test]
    fn chromatic_get_diatonic_roundtrip() {
        let chromatic = ChromaticInterval::new(6);
        let diatonic = chromatic.get_diatonic();
        let roundtrip = diatonic.get_chromatic().unwrap();
        assert_eq!(roundtrip.semitones, 6);
    }

    #[test]
    fn chromatic_transpose_pitch() {
        let c4 = pitch("C4");
        let out = ChromaticInterval::new(7).transpose_pitch(c4).unwrap();
        assert_eq!(out.name_with_octave(), "G4");
    }

    #[test]
    fn chromatic_reverse() {
        let reversed = ChromaticInterval::new(-4).reverse().unwrap();
        assert_eq!(reversed.semitones, 4);
    }
}
