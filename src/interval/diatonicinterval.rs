use crate::{base::Music21ObjectTrait, prebase::ProtoM21ObjectTrait};

use super::{
    chromaticinterval::ChromaticInterval, specifier::Specifier, GenericInterval, IntervalBaseTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct DiatonicInterval {
    pub(crate) generic: GenericInterval,
    speicifier: Specifier,
}

impl DiatonicInterval {
    pub(crate) fn get_chromatic(&self) -> ChromaticInterval {
        todo!()
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
