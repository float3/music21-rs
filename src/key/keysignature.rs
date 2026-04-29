use crate::{
    base::{Music21Object, Music21ObjectTrait},
    defaults::IntegerType,
    error::{Error, Result},
    interval::{Interval, IntervalArgument},
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
    scale::FIFTHS_ORDER_SHARP,
};

use super::Key;

const MODE_SHARPS_ALTER: [(&str, IntegerType); 9] = [
    ("major", 0),
    ("ionian", 0),
    ("minor", -3),
    ("aeolian", -3),
    ("dorian", -2),
    ("phrygian", -4),
    ("lydian", 1),
    ("mixolydian", -1),
    ("locrian", -5),
];

fn canonical_mode_for_offset(offset: IntegerType) -> Option<&'static str> {
    match offset {
        0 => Some("ionian"),
        -1 => Some("mixolydian"),
        -2 => Some("dorian"),
        -3 => Some("aeolian"),
        -4 => Some("phrygian"),
        -5 => Some("locrian"),
        1 => Some("lydian"),
        _ => None,
    }
}

pub(crate) fn mode_sharps_alter(mode: &str) -> Option<IntegerType> {
    MODE_SHARPS_ALTER
        .iter()
        .find_map(|(name, value)| (*name == mode.to_lowercase()).then_some(*value))
}

pub(crate) fn sharps_to_pitch(sharp_count: IntegerType) -> Result<Pitch> {
    if sharp_count == 0 {
        return Pitch::new(
            Some("C".to_string()),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        );
    }

    let mut pitch = Pitch::new(
        Some("C".to_string()),
        None,
        None,
        Option::<i8>::None,
        Option::<IntegerType>::None,
        None,
        None,
        None,
        None,
    )?;
    pitch.octave_setter(None);

    let interval = if sharp_count > 0 {
        Interval::new(IntervalArgument::Str("P5".to_string()))?
    } else {
        Interval::new(IntervalArgument::Str("P-5".to_string()))?
    };

    for _ in 0..sharp_count.abs() {
        pitch = pitch.transpose(interval.clone());
        pitch.octave_setter(None);
    }
    Ok(pitch)
}

pub(crate) fn pitch_to_sharps(
    pitch_value: &Pitch,
    mode: Option<&str>,
) -> Result<IntegerType> {
    let step_index = FIFTHS_ORDER_SHARP
        .iter()
        .position(|step| *step == pitch_value.step())
        .ok_or_else(|| Error::StepName("cannot map step to circle of fifths".to_string()))?;

    let mut sharps = step_index as IntegerType - 1;
    let accidental_alter = pitch_value.alter().round() as IntegerType;
    sharps += 7 * accidental_alter;

    if let Some(mode) = mode {
        let Some(mode_offset) = mode_sharps_alter(mode) else {
            return Err(Error::Ordinal(format!("unknown mode {mode}")));
        };
        sharps += mode_offset;
    }

    Ok(sharps)
}

pub(crate) fn pitch_name_to_sharps(
    pitch_name: &str,
    mode: Option<&str>,
) -> Result<IntegerType> {
    let pitch = Pitch::new(
        Some(pitch_name.to_string()),
        None,
        None,
        Option::<i8>::None,
        Option::<IntegerType>::None,
        None,
        None,
        None,
        None,
    )?;
    pitch_to_sharps(&pitch, mode)
}

#[derive(Clone, Debug)]
pub(crate) struct KeySignature {
    music21object: Music21Object,
    sharps: IntegerType,
}

impl KeySignature {
    pub(crate) fn new(sharps: IntegerType) -> Self {
        Self {
            music21object: Music21Object::new(),
            sharps,
        }
    }

    pub(crate) fn sharps(&self) -> IntegerType {
        self.sharps
    }

    pub(crate) fn as_key(&self, mode: &str) -> Key {
        self.try_as_key(Some(mode), None).unwrap_or_else(|_| {
            Key::new(
                Pitch::new(
                    Some("C".to_string()),
                    None,
                    None,
                    Option::<i8>::None,
                    Option::<IntegerType>::None,
                    None,
                    None,
                    None,
                    None,
                )
                .expect("C is valid pitch"),
                "major",
                0,
            )
        })
    }

    pub(crate) fn try_as_key(
        &self,
        mode: Option<&str>,
        tonic: Option<&str>,
    ) -> Result<Key> {
        let our_sharps = self.sharps;

        let resolved_mode = if mode.is_none() && tonic.is_none() {
            "major".to_string()
        } else if mode.is_none() && tonic.is_some() {
            let tonic_name = tonic.expect("checked is_some above");
            let major_sharps = pitch_name_to_sharps(tonic_name, None)?;
            canonical_mode_for_offset(our_sharps - major_sharps)
                .ok_or_else(|| {
                    Error::Ordinal(format!(
                        "Could not solve mode from sharps={} and tonic={}",
                        self.sharps, tonic_name
                    ))
                })?
                .to_string()
        } else {
            mode.expect("checked is_some above").to_lowercase()
        };

        let sharp_alteration_from_major = mode_sharps_alter(&resolved_mode)
            .ok_or_else(|| Error::Ordinal(format!("Mode {resolved_mode} is unknown")))?;

        let tonic_pitch = sharps_to_pitch(our_sharps - sharp_alteration_from_major)?;
        Ok(Key::new(tonic_pitch, &resolved_mode, our_sharps))
    }
}

pub(crate) trait KeySignatureTrait: Music21ObjectTrait {}

impl KeySignatureTrait for KeySignature {}

impl Music21ObjectTrait for KeySignature {}

impl ProtoM21ObjectTrait for KeySignature {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysignature_as_key_major_minor() {
        let ks = KeySignature::new(2);
        assert_eq!(ks.as_key("major").tonic().name(), "D");
        assert_eq!(ks.as_key("minor").tonic().name(), "B");
    }

    #[test]
    fn keysignature_mode_inference_from_tonic() {
        let ks = KeySignature::new(0);
        let key = ks.try_as_key(None, Some("D")).unwrap();
        assert_eq!(key.mode(), "dorian");
        assert_eq!(key.tonic().name(), "D");
    }

    #[test]
    fn sharps_to_pitch_roundtrip() {
        let f_sharp = sharps_to_pitch(6).unwrap();
        assert_eq!(f_sharp.name(), "F#");
        let b_flat = sharps_to_pitch(-2).unwrap();
        assert_eq!(b_flat.name(), "B-");
    }
}
