use crate::{
    common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait},
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    prebase::{ProtoM21Object, ProtoM21ObjectTrait},
};

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Rhythmic duration measured in quarter lengths.
///
/// A quarter note has a quarter length of `1.0`; an eighth note is `0.5`;
/// a whole note is `4.0`.
pub struct Duration {
    proto: ProtoM21Object,
    mixin: SlottedObjectMixin,
    quarter_length: FloatType,
}

impl Duration {
    /// Creates a duration from a quarter-length value.
    pub fn new(quarter_length: FloatType) -> Result<Self> {
        if !quarter_length.is_finite() || quarter_length < 0.0 {
            return Err(Error::Ordinal(format!(
                "duration quarter length must be finite and non-negative, got {quarter_length}"
            )));
        }

        Ok(Self {
            proto: ProtoM21Object::new(),
            mixin: SlottedObjectMixin::new(),
            quarter_length,
        })
    }

    /// Returns a quarter-note duration.
    pub fn quarter() -> Self {
        Self::default()
    }

    /// Returns a half-note duration.
    pub fn half() -> Self {
        Self::new(2.0).expect("constant duration is valid")
    }

    /// Returns a whole-note duration.
    pub fn whole() -> Self {
        Self::new(4.0).expect("constant duration is valid")
    }

    /// Returns an eighth-note duration.
    pub fn eighth() -> Self {
        Self::new(0.5).expect("constant duration is valid")
    }

    /// Returns the duration in quarter lengths.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }

    /// Updates the duration in quarter lengths.
    pub fn set_quarter_length(&mut self, quarter_length: FloatType) -> Result<()> {
        *self = Self::new(quarter_length)?;
        Ok(())
    }
}

impl Default for Duration {
    fn default() -> Self {
        Self {
            proto: ProtoM21Object::new(),
            mixin: SlottedObjectMixin::new(),
            quarter_length: 1.0,
        }
    }
}

impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        self.quarter_length == other.quarter_length
    }
}

impl TryFrom<FloatType> for Duration {
    type Error = Error;

    fn try_from(value: FloatType) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<IntegerType> for Duration {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::new(value as FloatType)
    }
}

impl ProtoM21ObjectTrait for Duration {}

impl SlottedObjectMixinTrait for Duration {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_tracks_quarter_lengths() {
        assert_eq!(Duration::quarter().quarter_length(), 1.0);
        assert_eq!(Duration::half().quarter_length(), 2.0);
        assert_eq!(Duration::whole().quarter_length(), 4.0);
        assert_eq!(Duration::eighth().quarter_length(), 0.5);
    }

    #[test]
    fn duration_rejects_invalid_values() {
        assert!(Duration::new(-1.0).is_err());
        assert!(Duration::new(FloatType::INFINITY).is_err());
    }

    #[test]
    fn duration_supports_conversions_and_updates() {
        let mut duration = Duration::try_from(3 as IntegerType).unwrap();
        assert_eq!(duration.quarter_length(), 3.0);

        duration.set_quarter_length(1.5).unwrap();
        assert_eq!(duration, Duration::try_from(1.5).unwrap());
        assert!(duration.set_quarter_length(FloatType::NAN).is_err());
    }
}
