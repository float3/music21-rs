use crate::{error::Result, pitch::Pitch};

pub(crate) trait IntervalBaseTrait {
    fn transpose_pitch(self, pitch1: Pitch) -> Result<Pitch>;
    fn reverse(self) -> Result<Self>
    where
        Self: Sized;
}
