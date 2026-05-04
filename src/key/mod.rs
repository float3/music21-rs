pub use keysignature::{KeySignature, pitch_name_to_sharps, pitch_to_sharps, sharps_to_pitch};

use crate::{
    chord::Chord,
    defaults::IntegerType,
    error::{Error, Result},
    pitch::Pitch,
    scale::diatonicscale::DiatonicScale,
};
use std::str::FromStr;

/// Key-signature conversion and spelling helpers.
pub mod keysignature;

#[derive(Clone, Debug)]
/// A tonal key with a tonic pitch and mode.
pub struct Key {
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

    /// Builds a key from a tonic and mode.
    ///
    /// Pass a mode string such as `"major"`, `"minor"`, `"dorian"`, or
    /// `None::<&str>` to infer major/minor from tonic case.
    pub fn from_tonic_mode<'a, M>(tonic: &str, mode: M) -> Result<Self>
    where
        M: Into<Option<&'a str>>,
    {
        let tonic_pitch = Pitch::new(
            Some(tonic.to_string()),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )?;

        let mode = mode.into();
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

    /// Builds a key and infers major/minor from tonic case.
    pub fn from_tonic(tonic: &str) -> Result<Self> {
        Self::from_tonic_mode(tonic, None::<&str>)
    }

    /// Returns a cloned tonic pitch.
    pub fn tonic(&self) -> Pitch {
        self.tonic_pitch.clone()
    }

    /// Returns a borrowed tonic pitch.
    pub fn tonic_pitch(&self) -> &Pitch {
        &self.tonic_pitch
    }

    /// Returns the key mode.
    pub fn mode(&self) -> &str {
        &self.mode
    }

    /// Returns the number of sharps in the key signature.
    pub fn sharps(&self) -> IntegerType {
        self.sharps
    }

    /// Returns the matching key signature.
    pub fn key_signature(&self) -> KeySignature {
        KeySignature::new(self.sharps)
    }

    /// Returns the diatonic scale for this key.
    pub fn scale(&self) -> DiatonicScale {
        DiatonicScale::new(self.tonic_pitch.clone(), self.sharps, &self.mode)
    }

    /// Returns the pitch at a one-based scale degree.
    pub fn pitch_from_degree(&self, degree: usize) -> Result<Pitch> {
        self.scale().pitch_from_degree(degree)
    }

    /// Returns scale pitches from degree 1 through the octave.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        self.scale().pitches()
    }

    /// Builds the diatonic triad on a one-based degree.
    pub fn triad_from_degree(&self, degree: usize) -> Result<Chord> {
        self.scale().triad_from_degree(degree)
    }

    /// Builds the diatonic seventh chord on a one-based degree.
    pub fn seventh_chord_from_degree(&self, degree: usize) -> Result<Chord> {
        self.scale().seventh_chord_from_degree(degree)
    }

    /// Returns all seven diatonic triads.
    pub fn harmonized_triads(&self) -> Result<Vec<Chord>> {
        (1..=7)
            .map(|degree| self.triad_from_degree(degree))
            .collect()
    }

    /// Returns all seven diatonic seventh chords.
    pub fn harmonized_sevenths(&self) -> Result<Vec<Chord>> {
        (1..=7)
            .map(|degree| self.seventh_chord_from_degree(degree))
            .collect()
    }

    /// Returns the relative major or minor key when applicable.
    pub fn relative(&self) -> Result<Self> {
        match self.mode.as_str() {
            "major" => self.key_signature().try_as_key(Some("minor"), None),
            "minor" => self.key_signature().try_as_key(Some("major"), None),
            _ => Ok(self.clone()),
        }
    }

    /// Returns the parallel major or minor key when applicable.
    pub fn parallel(&self) -> Result<Self> {
        match self.mode.as_str() {
            "major" => Self::from_tonic_mode(&self.tonic_pitch.name(), "minor"),
            "minor" => Self::from_tonic_mode(&self.tonic_pitch.name(), "major"),
            _ => Ok(self.clone()),
        }
    }
}

impl FromStr for Key {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        parse_key(value)
    }
}

impl TryFrom<&str> for Key {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        value.parse()
    }
}

impl TryFrom<String> for Key {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        value.parse()
    }
}

fn parse_key(value: &str) -> Result<Key> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(Error::Analysis("key cannot be empty".to_string()));
    }

    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        [tonic] => {
            if let Some((tonic, mode)) = split_compact_key_token(tonic) {
                Key::from_tonic_mode(tonic, Some(mode.as_str()))
            } else {
                Key::from_tonic(tonic)
            }
        }
        [tonic, mode] => {
            let mode = canonical_key_mode(mode);
            Key::from_tonic_mode(tonic, Some(mode.as_str()))
        }
        _ => Err(Error::Analysis(format!(
            "invalid key {value:?}; use a tonic and optional mode, such as \"C\", \"C major\", or \"Am\""
        ))),
    }
}

fn split_compact_key_token(token: &str) -> Option<(&str, String)> {
    let lower = token.to_ascii_lowercase();
    for suffix in ["major", "minor", "maj", "min", "m"] {
        if lower.ends_with(suffix) && lower.len() > suffix.len() {
            let tonic_end = token.len() - suffix.len();
            return Some((&token[..tonic_end], canonical_key_mode(suffix)));
        }
    }
    None
}

fn canonical_key_mode(mode: &str) -> String {
    match mode.to_ascii_lowercase().as_str() {
        "maj" | "major" => "major".to_string(),
        "m" | "min" | "minor" => "minor".to_string(),
        other => other.to_string(),
    }
}

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
    fn key_from_string_accepts_common_notation() {
        let c_major: Key = "C major".parse().unwrap();
        assert_eq!(c_major.tonic().name(), "C");
        assert_eq!(c_major.mode(), "major");

        let a_minor: Key = "Am".parse().unwrap();
        assert_eq!(a_minor.tonic().name(), "A");
        assert_eq!(a_minor.mode(), "minor");

        let b_flat_minor: Key = "Bb minor".parse().unwrap();
        assert_eq!(b_flat_minor.tonic().name(), "B-");
        assert_eq!(b_flat_minor.mode(), "minor");
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
