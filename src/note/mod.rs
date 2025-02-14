pub(crate) mod generalnote;
pub(crate) mod notrest;

use crate::base::Music21ObjectTrait;
use crate::defaults::IntegerType;
use crate::duration::Duration;
use crate::exception::ExceptionResult;
use crate::pitch::Pitch;
use crate::prebase::ProtoM21ObjectTrait;

use generalnote::GeneralNoteTrait;
use notrest::{NotRest, NotRestTrait};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Note {
    notrest: NotRest,
    pub(crate) _pitch: Pitch,
}

impl Note {
    pub(crate) fn new<T>(
        pitch: Option<T>,
        duration: Option<Duration>,
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
                    None,
                    None,
                    None,
                    None,
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
    fn duration(&self) -> &Option<Duration> {
        self.notrest.duration()
    }

    fn set_duration(&self, duration: &Duration) {
        todo!()
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
            None,
            None,
            None,
            None,
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
            None,
            None,
            None,
            None,
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
            None,
            None,
            None,
            None,
        )
    }
}
