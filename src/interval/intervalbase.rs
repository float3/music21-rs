use crate::{
    base::{Music21Object, Music21ObjectTrait},
    exception::ExceptionResult,
    note::Note,
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub(crate) struct IntervalBase {
    music21object: Music21Object,
}

impl IntervalBase {
    pub(crate) fn new() -> Self {
        Self {
            music21object: Music21Object::new(),
        }
    }
}

pub(crate) trait IntervalBaseTrait: Music21ObjectTrait {
    fn transpose_note(self, note1: Note) -> ExceptionResult<Note>;
    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch>;
    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()>;
    fn reverse(self) -> ExceptionResult<Self>
    where
        Self: Sized;
}

impl IntervalBaseTrait for IntervalBase {
    fn transpose_note(self, note1: Note) -> ExceptionResult<Note> {
        todo!()
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        panic!("interval base doesn't know how to do this")
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        panic!("interval base doesn't know how to do this")
    }

    fn reverse(self) -> ExceptionResult<Self> {
        panic!("interval base doesn't know how to do this")
    }
}

impl Music21ObjectTrait for IntervalBase {}

impl ProtoM21ObjectTrait for IntervalBase {}
