use crate::{
    base::Music21ObjectTrait, defaults::IntegerType, exception::ExceptionResult, note::Note,
    pitch::Pitch, prebase::ProtoM21ObjectTrait,
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
        todo!()
    }
}

impl IntervalBaseTrait for ChromaticInterval {
    fn transpose_note(self, note1: Note) -> ExceptionResult<Note> {
        todo!()
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        todo!()
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        todo!()
    }

    fn reverse(self) -> ExceptionResult<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl Music21ObjectTrait for ChromaticInterval {}

impl ProtoM21ObjectTrait for ChromaticInterval {}
