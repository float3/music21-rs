//! How loud a note is: music21's `volume.Volume`.
//!
//! A volume is one number held two ways — a MIDI velocity from 0 to 127 and
//! the same value as a scalar from 0 to 1 — plus the rule for reading it
//! against the dynamic marks around it. music21 finds those marks by
//! searching the stream the note sits in; there are no streams here, so
//! [`Volume::realized_with`] takes the dynamic scalar and the articulation
//! shift as arguments instead of going looking for them.

use std::fmt;

use crate::{
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
};

/// The velocity music21 assumes when none was set, as a scalar. It is the
/// `0.5` base level shifted by `0.20866`, which is what makes an unset note
/// sound at a natural mezzo-forte rather than at half volume.
const UNSET_VELOCITY_SHIFT: FloatType = 0.20866;

/// The middle of the dynamic range that scalars move away from.
const BASE_LEVEL: FloatType = 0.5;

/// How loud a note is: music21's `volume.Volume`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Volume {
    velocity_scalar: Option<FloatType>,
    velocity_is_relative: bool,
}

impl Default for Volume {
    /// A volume with no velocity set, which reads as music21's default
    /// loudness rather than as silence.
    fn default() -> Self {
        Self {
            velocity_scalar: None,
            velocity_is_relative: true,
        }
    }
}

impl Volume {
    /// A volume with no velocity of its own.
    pub fn new() -> Self {
        Self::default()
    }

    /// A volume at a MIDI velocity, `0` to `127`, clamped to that range.
    pub fn from_velocity(velocity: IntegerType) -> Self {
        let mut volume = Self::new();
        volume.set_velocity(Some(velocity));
        volume
    }

    /// A volume at a scalar between `0` and `1`, clamped to that range.
    pub fn from_velocity_scalar(scalar: FloatType) -> Result<Self> {
        let mut volume = Self::new();
        volume.set_velocity_scalar(Some(scalar))?;
        Ok(volume)
    }

    /// The MIDI velocity, `0` to `127`, or `None` when none was set.
    pub fn velocity(&self) -> Option<IntegerType> {
        let scalar = self.velocity_scalar?;
        let velocity = (scalar * 127.0).clamp(0.0, 127.0);
        Some(velocity.round_ties_even() as IntegerType)
    }

    /// Sets the MIDI velocity, clamped to `0` through `127`, or clears it.
    pub fn set_velocity(&mut self, velocity: Option<IntegerType>) {
        self.velocity_scalar = velocity.map(|velocity| {
            if velocity <= 0 {
                0.0
            } else if velocity >= 127 {
                1.0
            } else {
                FloatType::from(velocity) / 127.0
            }
        });
    }

    /// The velocity as a scalar between `0` and `1`, or `None` when none was
    /// set.
    pub fn velocity_scalar(&self) -> Option<FloatType> {
        self.velocity_scalar
    }

    /// Sets the velocity from a scalar, clamped to `0` through `1`, or
    /// clears it. A value that is not a number is an error, as in music21.
    pub fn set_velocity_scalar(&mut self, scalar: Option<FloatType>) -> Result<()> {
        match scalar {
            None => self.velocity_scalar = None,
            Some(scalar) if scalar.is_nan() => {
                return Err(Error::Volume(
                    "value provided for velocityScalar must be a number, not NaN".to_string(),
                ));
            }
            Some(scalar) => self.velocity_scalar = Some(scalar.clamp(0.0, 1.0)),
        }
        Ok(())
    }

    /// Whether the velocity shifts the dynamics around it (music21's
    /// default) or fixes the loudness on its own, as a velocity read from a
    /// MIDI file does.
    pub fn velocity_is_relative(&self) -> bool {
        self.velocity_is_relative
    }

    /// Sets whether the velocity is relative to its context.
    pub fn set_velocity_is_relative(&mut self, relative: bool) {
        self.velocity_is_relative = relative;
    }

    /// Whether a velocity was ever set: music21's `hasVolumeInformation` on
    /// the note that owns it.
    pub fn has_velocity_information(&self) -> bool {
        self.velocity_scalar.is_some()
    }

    /// The loudness this volume comes to on its own, between `0` and `1`:
    /// music21's `realized` with no dynamic or articulation context.
    pub fn realized(&self) -> FloatType {
        self.realized_with(None, 0.0, BASE_LEVEL, true)
    }

    /// The loudness this volume comes to against the dynamic in force and
    /// whatever the articulations add: music21's `getRealized`, with the
    /// context search it would do through a stream replaced by its answers.
    ///
    /// A relative velocity doubles the scalar range, so `0.5` leaves the base
    /// level alone and `0.7` raises it; an absolute one decides the answer by
    /// itself. The dynamic scales; an articulation *shifts*, which is why an
    /// accent on a note nobody has marked lifts it rather than doubling it.
    /// `clip` holds the result inside `0` to `1`.
    pub fn realized_with(
        &self,
        dynamic_scalar: Option<FloatType>,
        articulation_shift: FloatType,
        base_level: FloatType,
        clip: bool,
    ) -> FloatType {
        let mut value = base_level;
        match self.velocity_scalar {
            Some(scalar) if !self.velocity_is_relative => value = scalar,
            Some(scalar) => value *= scalar * 2.0,
            None => value += UNSET_VELOCITY_SHIFT,
        }
        if self.velocity_is_relative {
            if let Some(dynamic_scalar) = dynamic_scalar {
                value *= dynamic_scalar * 2.0;
            }
            value += articulation_shift;
        }
        if clip {
            value = value.clamp(0.0, 1.0);
        }
        value
    }

    /// The realized loudness as one of music21's dynamic marks, `ppp`
    /// through `fff`: `dynamics.dynamicStrFromDecimal` of the realized value.
    pub fn realized_dynamic(&self) -> &'static str {
        dynamic_name(self.realized())
    }

    /// The realized loudness written out to two places, which is what
    /// music21's `getRealizedStr` and `cachedRealizedStr` answer.
    pub fn realized_str(&self) -> String {
        rounded_str(self.realized())
    }
}

/// A loudness written out the way Python writes `str(round(value, 2))`: to
/// two places, with the trailing zeros dropped but never the point.
pub fn rounded_str(value: FloatType) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let written = format!("{rounded}");
    if written.contains('.') {
        written
    } else {
        format!("{written}.0")
    }
}

/// The dynamic mark a realized loudness falls under, using music21's
/// `dynamicStrFromDecimal` thresholds.
fn dynamic_name(value: FloatType) -> &'static str {
    match value {
        value if value <= 0.0 => "n",
        value if value < 0.11 => "pppp",
        value if value < 0.16 => "ppp",
        value if value < 0.26 => "pp",
        value if value < 0.36 => "p",
        value if value < 0.5 => "mp",
        value if value < 0.65 => "mf",
        value if value < 0.8 => "f",
        value if value < 0.9 => "ff",
        _ => "fff",
    }
}

impl fmt::Display for Volume {
    /// music21's `repr` body, `realized=0.71`.
    ///
    /// music21 writes `round(self.realized, 2)` into an f-string, so Python's
    /// float formatting drops a trailing zero the way `{:.2}` does not: half
    /// velocity reads `realized=0.5`, not `realized=0.50`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rounded = format!("{:.2}", self.realized());
        while rounded.ends_with('0') && !rounded.ends_with(".0") {
            rounded.pop();
        }
        write!(f, "realized={rounded}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_and_scalar_are_the_same_number() {
        let mut volume = Volume::new();
        assert_eq!(volume.velocity(), None);
        assert_eq!(volume.velocity_scalar(), None);
        assert!(!volume.has_velocity_information());

        volume.set_velocity(Some(64));
        assert_eq!(volume.velocity(), Some(64));
        assert!(volume.has_velocity_information());
        assert!((volume.velocity_scalar().unwrap() - 64.0 / 127.0).abs() < 1e-12);

        volume.set_velocity(Some(200));
        assert_eq!(volume.velocity(), Some(127));
        volume.set_velocity(Some(-5));
        assert_eq!(volume.velocity(), Some(0));
        volume.set_velocity(None);
        assert_eq!(volume.velocity(), None);
    }

    #[test]
    fn scalars_clamp_and_round_trip() {
        let volume = Volume::from_velocity_scalar(0.5).unwrap();
        assert_eq!(volume.velocity(), Some(64));
        assert_eq!(
            Volume::from_velocity_scalar(2.0).unwrap().velocity_scalar(),
            Some(1.0)
        );
        assert_eq!(
            Volume::from_velocity_scalar(-1.0)
                .unwrap()
                .velocity_scalar(),
            Some(0.0)
        );
        assert!(Volume::from_velocity_scalar(FloatType::NAN).is_err());
        assert_eq!(Volume::from_velocity(127).velocity_scalar(), Some(1.0));
    }

    #[test]
    fn realized_matches_music21() {
        // An unset velocity realizes at the shifted base level, which is
        // music21's default loudness for a note nobody has marked.
        let unset = Volume::new();
        assert!((unset.realized() - 0.70866).abs() < 1e-9);
        assert_eq!(unset.realized_dynamic(), "f");
        assert_eq!(unset.realized_str(), "0.71");

        // A relative velocity doubles its scalar against the base level, so
        // half velocity leaves the base level where it is.
        let half = Volume::from_velocity_scalar(0.5).unwrap();
        assert!((half.realized() - 0.5).abs() < 1e-9);
        assert_eq!(half.realized_dynamic(), "mf");
        assert_eq!(half.realized_str(), "0.5");

        // An absolute velocity decides the answer by itself.
        let mut absolute = Volume::from_velocity_scalar(0.5).unwrap();
        absolute.set_velocity_is_relative(false);
        assert!((absolute.realized() - 0.5).abs() < 1e-9);
        let mut loud = Volume::from_velocity_scalar(1.0).unwrap();
        loud.set_velocity_is_relative(false);
        assert!((loud.realized() - 1.0).abs() < 1e-9);

        // Relative velocities scale with the dynamic around them, and clip.
        assert!((half.realized_with(Some(0.5), 0.0, 0.5, true) - 0.5).abs() < 1e-9);
        assert!((half.realized_with(Some(1.0), 0.0, 0.5, true) - 1.0).abs() < 1e-9);
        assert!((half.realized_with(Some(1.0), 0.0, 0.5, false) - 1.0).abs() < 1e-9);
        assert!(loud.realized_with(Some(1.0), 0.0, 0.5, false) > 0.99);

        // An articulation shifts rather than scales, so an accent on a note
        // nobody has marked lifts it a little.
        let unmarked = Volume::new();
        assert!((unmarked.realized_with(None, 0.1, 0.5, true) - 0.80866).abs() < 1e-9);
    }

    #[test]
    fn realized_names_cover_the_dynamic_range() {
        assert_eq!(dynamic_name(0.0), "n");
        assert_eq!(dynamic_name(0.05), "pppp");
        assert_eq!(dynamic_name(0.12), "ppp");
        assert_eq!(dynamic_name(0.2), "pp");
        assert_eq!(dynamic_name(0.3), "p");
        assert_eq!(dynamic_name(0.4), "mp");
        assert_eq!(dynamic_name(0.6), "mf");
        assert_eq!(dynamic_name(0.7), "f");
        assert_eq!(dynamic_name(0.85), "ff");
        assert_eq!(dynamic_name(1.0), "fff");
        assert_eq!(Volume::from_velocity(64).to_string(), "realized=0.5");
        assert_eq!(Volume::new().to_string(), "realized=0.71");
        let mut silent = Volume::new();
        silent.set_velocity(Some(0));
        silent.set_velocity_is_relative(false);
        assert_eq!(silent.to_string(), "realized=0.0");
    }
}
