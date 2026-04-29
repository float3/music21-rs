use crate::{
    base::{Music21Object, Music21ObjectTrait},
    error::{Error, Result},
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
    fn transpose_note(self, note1: Note) -> Result<Note>;
    fn transpose_pitch(self, pitch1: Pitch) -> Result<Pitch>;
    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> Result<()>;
    fn reverse(self) -> Result<Self>
    where
        Self: Sized;
}

impl IntervalBaseTrait for IntervalBase {
    fn transpose_note(self, note1: Note) -> Result<Note> {
        Err(Error::Interval(
            "IntervalBase cannot transpose a note directly".to_string(),
        ))
    }

    fn transpose_pitch(self, pitch1: Pitch) -> Result<Pitch> {
        Err(Error::Interval(
            "IntervalBase cannot transpose a pitch directly".to_string(),
        ))
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> Result<()> {
        Err(Error::Interval(
            "IntervalBase cannot transpose a pitch in place directly".to_string(),
        ))
    }

    fn reverse(self) -> Result<Self> {
        Err(Error::Interval(
            "IntervalBase cannot reverse directly".to_string(),
        ))
    }
}

impl Music21ObjectTrait for IntervalBase {}

impl ProtoM21ObjectTrait for IntervalBase {}
