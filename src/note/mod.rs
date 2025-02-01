pub(crate) mod generalnote;
pub mod notrest;

use generalnote::GeneralNoteTrait;
use notrest::{NotRest, NotRestTrait};
use num::Num;

use crate::{
    base::Music21ObjectTrait,
    pitch::{IntoPitchName, Pitch},
    prebase::ProtoM21ObjectTrait,
};

#[derive(Clone, Debug)]
pub struct Note {
    notrest: NotRest,
    pub(crate) _pitch: Pitch,
}

impl Note {
    pub(crate) fn new<T>(pitch: Option<T>) -> Self
    where
        T: IntoPitch,
    {
        Self {
            notrest: NotRest::new(),
            _pitch: todo!(),
        }
    }

    pub(crate) fn get_super(&self) -> NotRest {
        self.notrest.clone()
    }

    pub(crate) fn pitchChanged(&self) {
        todo!()
    }
}

pub(crate) trait NoteTrait: NotRestTrait {}

impl NoteTrait for Note {}

impl NotRestTrait for Note {}

impl GeneralNoteTrait for Note {}

impl ProtoM21ObjectTrait for Note {}

impl Music21ObjectTrait for Note {}

pub trait IntoPitch {
    fn into_pitch(&self) -> Pitch;
}

impl IntoPitch for Pitch {
    fn into_pitch(&self) -> Pitch {
        self.clone()
    }
}

impl<T: Num + IntoPitchName + Clone> IntoPitch for T {
    fn into_pitch(&self) -> Pitch {
        Pitch::new(Some(self.clone()))
    }
}
