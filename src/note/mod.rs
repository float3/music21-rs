pub(crate) mod generalnote;
pub(crate) mod notrest;

use crate::{
    base::Music21ObjectTrait, defaults::IntegerType, exceptions::ExceptionResult, pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

use generalnote::GeneralNoteTrait;
use notrest::{NotRest, NotRestTrait};

#[derive(Clone, Debug)]
pub(crate) struct Note {
    notrest: NotRest,
    pub(crate) _pitch: Pitch,
}

impl Note {
    pub(crate) fn new<T>(
        pitch: Option<T>,
        duration: Option<crate::duration::Duration>,
        name: Option<String>,
        name_with_octave: Option<String>,
    ) -> ExceptionResult<Self>
    where
        T: IntoPitch,
    {
        let _pitch = match pitch {
            Some(pitch) => pitch.into_pitch(),
            None => Ok({
                let name = match name_with_octave {
                    Some(name_with_octave) => name_with_octave,
                    None => match name {
                        Some(name) => name,
                        None => "C4".to_string(),
                    },
                };

                Pitch::new(
                    Some(name),
                    None,
                    None,
                    Option::<i8>::None,
                    Option::<IntegerType>::None,
                )?
            }),
        }?;

        Ok(Self {
            notrest: NotRest::new(duration),
            _pitch,
        })
    }

    pub(crate) fn get_super(&self) -> &NotRest {
        &self.notrest
    }

    pub(crate) fn pitch_changed(&self) {
        todo!()
    }
}

pub(crate) trait NoteTrait: NotRestTrait {}

impl NoteTrait for Note {}

impl NotRestTrait for Note {}

impl GeneralNoteTrait for Note {
    fn duration(&self) -> &Option<crate::duration::Duration> {
        self.notrest.duration()
    }
}

impl ProtoM21ObjectTrait for Note {}

impl Music21ObjectTrait for Note {}

pub(crate) trait IntoPitch {
    fn into_pitch(self) -> ExceptionResult<Pitch>;
}

impl IntoPitch for Pitch {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Ok(self.clone())
    }
}

impl IntoPitch for String {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
        )
    }
}

impl IntoPitch for &str {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
        )
    }
}

impl IntoPitch for IntegerType {
    fn into_pitch(self) -> ExceptionResult<Pitch> {
        Pitch::new(
            Some(self),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
        )
    }
}
