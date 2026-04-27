use crate::{
    base::{Music21Object, Music21ObjectTrait},
    exception::{Exception, ExceptionResult},
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
        Err(Exception::Interval(
            "IntervalBase cannot transpose a note directly".to_string(),
        ))
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        Err(Exception::Interval(
            "IntervalBase cannot transpose a pitch directly".to_string(),
        ))
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        Err(Exception::Interval(
            "IntervalBase cannot transpose a pitch in place directly".to_string(),
        ))
    }

    fn reverse(self) -> ExceptionResult<Self> {
        Err(Exception::Interval(
            "IntervalBase cannot reverse directly".to_string(),
        ))
    }
}

impl Music21ObjectTrait for IntervalBase {}

impl ProtoM21ObjectTrait for IntervalBase {}
