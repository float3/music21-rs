use keysignature::{KeySignature, KeySignatureTrait, pitch_to_sharps};

use crate::{
    base::Music21ObjectTrait, chord::Chord, defaults::IntegerType, error::Result,
    pitch::Pitch, prebase::ProtoM21ObjectTrait, scale::diatonicscale::DiatonicScale,
};

pub(crate) mod keysignature;

#[derive(Clone, Debug)]
pub(crate) struct Key {
    tonic_pitch: Pitch,
    mode: String,
    sharps: IntegerType,
}

impl Key {
    pub(crate) fn new(tonic_pitch: Pitch, mode: &str, sharps: IntegerType) -> Self {
        Self {
            tonic_pitch,
            mode: mode.to_string(),
            sharps,
        }
    }

    pub(crate) fn from_tonic_mode(tonic: &str, mode: Option<&str>) -> Result<Self> {
        let tonic_pitch = Pitch::new(
            Some(tonic.to_string()),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )?;

        let resolved_mode = match mode {
            Some(mode) => mode.to_lowercase(),
            None => {
                if tonic.chars().all(|ch| !ch.is_ascii_uppercase()) {
                    "minor".to_string()
                } else {
                    "major".to_string()
                }
            }
        };

        let sharps = pitch_to_sharps(&tonic_pitch, Some(&resolved_mode))?;
        Ok(Self::new(tonic_pitch, &resolved_mode, sharps))
    }

    pub(crate) fn tonic(&self) -> Pitch {
        self.tonic_pitch.clone()
    }

    pub(crate) fn mode(&self) -> &str {
        &self.mode
    }

    pub(crate) fn sharps(&self) -> IntegerType {
        self.sharps
    }

    pub(crate) fn key_signature(&self) -> KeySignature {
        KeySignature::new(self.sharps)
    }

    pub(crate) fn scale(&self) -> DiatonicScale {
        DiatonicScale::new(self.tonic_pitch.clone(), self.sharps, &self.mode)
    }

    pub(crate) fn pitch_from_degree(&self, degree: usize) -> Result<Pitch> {
        self.scale().pitch_from_degree(degree)
    }

    pub(crate) fn pitches(&self) -> Result<Vec<Pitch>> {
        self.scale().pitches()
    }

    pub(crate) fn triad_from_degree(&self, degree: usize) -> Result<Chord> {
        self.scale().triad_from_degree(degree)
    }

    pub(crate) fn seventh_chord_from_degree(&self, degree: usize) -> Result<Chord> {
        self.scale().seventh_chord_from_degree(degree)
    }

    pub(crate) fn harmonized_triads(&self) -> Result<Vec<Chord>> {
        (1..=7)
            .map(|degree| self.triad_from_degree(degree))
            .collect()
    }

    pub(crate) fn harmonized_sevenths(&self) -> Result<Vec<Chord>> {
        (1..=7)
            .map(|degree| self.seventh_chord_from_degree(degree))
            .collect()
    }

    pub(crate) fn relative(&self) -> Result<Self> {
        match self.mode.as_str() {
            "major" => self.key_signature().try_as_key(Some("minor"), None),
            "minor" => self.key_signature().try_as_key(Some("major"), None),
            _ => Ok(self.clone()),
        }
    }

    pub(crate) fn parallel(&self) -> Result<Self> {
        match self.mode.as_str() {
            "major" => Self::from_tonic_mode(&self.tonic_pitch.name(), Some("minor")),
            "minor" => Self::from_tonic_mode(&self.tonic_pitch.name(), Some("major")),
            _ => Ok(self.clone()),
        }
    }
}

impl KeySignatureTrait for Key {}
impl Music21ObjectTrait for Key {}
impl ProtoM21ObjectTrait for Key {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::keysignature::pitch_name_to_sharps;

    #[test]
    fn key_from_tonic_mode() {
        let c_major = Key::from_tonic_mode("C", Some("major")).unwrap();
        assert_eq!(c_major.sharps(), 0);
        let g_major = Key::from_tonic_mode("G", Some("major")).unwrap();
        assert_eq!(g_major.sharps(), 1);
        let a_minor = Key::from_tonic_mode("A", Some("minor")).unwrap();
        assert_eq!(a_minor.sharps(), 0);
        let e_phrygian = Key::from_tonic_mode("E", Some("phrygian")).unwrap();
        assert_eq!(e_phrygian.sharps(), 0);
    }

    #[test]
    fn key_scale_degree_and_chords() {
        let d_major = Key::from_tonic_mode("D", Some("major")).unwrap();
        assert_eq!(
            d_major.pitch_from_degree(7).unwrap().name_with_octave(),
            "C#5"
        );
        assert_eq!(
            d_major.triad_from_degree(1).unwrap().pitched_common_name(),
            "D-major triad"
        );
        assert_eq!(
            d_major
                .seventh_chord_from_degree(5)
                .unwrap()
                .pitched_common_name(),
            "A-dominant seventh chord"
        );
    }

    #[test]
    fn key_harmonized_triads() {
        let c_major = Key::from_tonic_mode("C", Some("major")).unwrap();
        let triads = c_major.harmonized_triads().unwrap();
        assert_eq!(triads.len(), 7);
        assert_eq!(triads[0].pitched_common_name(), "C-major triad");
        assert_eq!(triads[4].pitched_common_name(), "G-major triad");
    }

    #[test]
    fn key_relative_and_parallel() {
        let c_major = Key::from_tonic_mode("C", Some("major")).unwrap();
        let relative = c_major.relative().unwrap();
        assert_eq!(relative.mode(), "minor");
        assert_eq!(relative.tonic().name(), "A");

        let parallel = c_major.parallel().unwrap();
        assert_eq!(parallel.mode(), "minor");
        assert_eq!(parallel.tonic().name(), "C");
    }

    #[test]
    fn pitch_name_to_sharps_modes() {
        assert_eq!(pitch_name_to_sharps("C", Some("major")).unwrap(), 0);
        assert_eq!(pitch_name_to_sharps("E", Some("minor")).unwrap(), 1);
        assert_eq!(pitch_name_to_sharps("D", Some("dorian")).unwrap(), 0);
        assert_eq!(pitch_name_to_sharps("A", Some("mixolydian")).unwrap(), 2);
    }
}
