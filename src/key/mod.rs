pub use keysignature::{
    KeySignature, mode_sharps_alter, pitch_name_to_sharps, pitch_to_sharps, sharps_to_pitch,
};

use crate::{
    chord::Chord,
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
    scale::{Scale, ScaleType, diatonicscale::DiatonicScale},
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
    /// Builds a key from a tonic name and a mode. Without a mode the name
    /// decides, as music21's `Key` reads it: a trailing `m` is minor and a
    /// trailing `M` major (`F#m`, `EM`), otherwise a lower-case name is minor
    /// and an upper-case one major.
    pub fn from_tonic_mode<'a, M>(tonic: &str, mode: M) -> Result<Self>
    where
        M: Into<Option<&'a str>>,
    {
        let mode = mode.into();
        let (tonic, resolved_mode) = match mode {
            Some(mode) => (tonic.to_string(), mode.to_lowercase()),
            None if tonic.contains('m') => (tonic.replace('m', ""), "minor".to_string()),
            None if tonic.contains('M') => (tonic.replace('M', ""), "major".to_string()),
            None if tonic.chars().all(|ch| !ch.is_ascii_uppercase()) => {
                (tonic.to_string(), "minor".to_string())
            }
            None => (tonic.to_string(), "major".to_string()),
        };
        let tonic_pitch = Pitch::from_name(tonic.as_str())?;

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

    /// Returns this key moved by the interval, keeping its mode.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        self.key_signature()
            .transpose(interval)?
            .try_as_key(Some(&self.mode), None)
    }

    /// Returns the [`Scale`] for this key's mode on its tonic. Major and
    /// minor cover ionian and aeolian; the other church modes map to their
    /// scale types, and any other mode is an error.
    pub fn as_scale(&self) -> Result<Scale> {
        let scale_type = match self.mode.as_str() {
            "major" | "ionian" => ScaleType::Major,
            "minor" | "aeolian" => ScaleType::Minor,
            "dorian" => ScaleType::Dorian,
            "phrygian" => ScaleType::Phrygian,
            "lydian" => ScaleType::Lydian,
            "mixolydian" => ScaleType::Mixolydian,
            "locrian" => ScaleType::Locrian,
            other => {
                return Err(Error::Key(format!("no scale type for mode {other}")));
            }
        };
        Ok(Scale::new(scale_type, self.tonic_pitch.clone()))
    }

    /// Returns the tonic name in music21's case convention: upper case for
    /// major, lower case for minor, unchanged for other modes.
    pub fn tonic_pitch_name_with_case(&self) -> String {
        let name = self.tonic_pitch.name();
        match self.mode.as_str() {
            "major" => name.to_uppercase(),
            "minor" => name.to_lowercase(),
            _ => name,
        }
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
    /// Returns the key of the same mode in which `pitch` is the given scale
    /// degree: music21's `deriveByDegree`, so C major with `E` as degree 5 is
    /// A major and A minor with `C` as degree 3 is A minor again.
    pub fn derive_by_degree(&self, degree: usize, pitch: &Pitch) -> Result<Self> {
        let scale = self.as_scale()?.derive_by_degree(degree, pitch)?;
        let tonic = scale.tonic().clone();
        let sharps = pitch_to_sharps(&tonic, Some(&self.mode))?;
        Ok(Self::new(tonic, &self.mode, sharps))
    }

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

/// music21's `convertKeyStringToMusic21KeyString`: a key written with `b`
/// for flat, as in `Bb` or `eb`, rewritten with music21's `-`, so `Bb` is
/// `B-` and `bb` is `b-`. A lone `b` is B, and anything without a trailing
/// `b` is returned as it is.
pub fn convert_key_string_to_music21_key_string(text: &str) -> String {
    if !text.ends_with('b') || text == "b" {
        return text.to_string();
    }
    if text == "bb" {
        return "b-".to_string();
    }
    if text == "Bb" {
        return "B-".to_string();
    }
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return text.to_string();
    };
    let rest: Vec<char> = chars.collect();
    if rest.iter().all(|c| *c == 'b') {
        return format!("{first}{}", "-".repeat(rest.len()));
    }
    text.to_string()
}

#[cfg(test)]
mod tests {

    #[test]
    fn mode_suffixes_and_semitone_transposition_match_music21() {
        let e_major = Key::from_tonic("EM").unwrap();
        assert_eq!(
            (e_major.tonic().name(), e_major.mode()),
            ("E".to_string(), "major")
        );
        let f_sharp_minor = Key::from_tonic("F#m").unwrap();
        assert_eq!(
            (f_sharp_minor.tonic().name(), f_sharp_minor.mode()),
            ("F#".to_string(), "minor")
        );
        let up_a_semitone = Key::from_tonic("e")
            .unwrap()
            .transpose(&Interval::from_semitones(1).unwrap())
            .unwrap();
        assert_eq!(up_a_semitone.tonic_pitch_name_with_case(), "f");
        assert_eq!(up_a_semitone.sharps(), -4);
        let by_name = Key::from_tonic("e")
            .unwrap()
            .transpose(&Interval::from_name("m2").unwrap())
            .unwrap();
        assert_eq!(by_name.tonic_pitch_name_with_case(), "f");
        assert!(matches!(
            pitch_to_sharps(&Pitch::from_name("C~").unwrap(), None),
            Err(crate::Error::Key(_))
        ));
    }

    #[test]
    fn key_strings_convert_like_music21() {
        let cases = [
            ("bb", "b-"),
            ("b", "b"),
            ("B", "B"),
            ("Bb", "B-"),
            ("a", "a"),
            ("f#", "f#"),
            ("F#", "F#"),
            ("eb", "e-"),
            ("e-", "e-"),
            ("Ebb", "E--"),
            ("Abb", "A--"),
        ];
        for (text, expected) in cases {
            assert_eq!(
                convert_key_string_to_music21_key_string(text),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn derive_by_degree_matches_music21() {
        let cases: [(&str, usize, &str, &str, &str); 8] = [
            ("C", 5, "E", "A3", "major"),
            ("C", 1, "F#4", "F#4", "major"),
            ("a", 3, "C", "A3", "minor"),
            ("C", 7, "B-", "C-4", "major"),
            ("C", 4, "B", "F#4", "major"),
            ("D", 2, "C#3", "B2", "major"),
            ("c", 6, "A-4", "C4", "minor"),
            ("C", 5, "G4", "C4", "major"),
        ];
        for (key, degree, pitch, tonic, mode) in cases {
            let derived = Key::from_tonic(key)
                .unwrap()
                .derive_by_degree(degree, &Pitch::from_name(pitch).unwrap())
                .unwrap();
            assert_eq!(
                derived.tonic_pitch().name_with_octave(),
                tonic,
                "{key} {degree} {pitch}"
            );
            assert_eq!(derived.mode(), mode, "{key} {degree} {pitch}");
        }
        assert_eq!(
            Key::from_tonic("C")
                .unwrap()
                .derive_by_degree(5, &Pitch::from_name("E").unwrap())
                .unwrap()
                .sharps(),
            3
        );
    }
    use super::*;
    use crate::key::keysignature::pitch_name_to_sharps;

    #[test]
    fn transposing_a_key_matches_music21() {
        let cases = [
            ("C", "M2", "D", 2, "major"),
            ("c", "P5", "g", -2, "minor"),
            ("F#", "m2", "G", 1, "major"),
            ("B-", "-M3", "G-", -6, "major"),
            ("D", "P8", "D", 2, "major"),
            ("e", "M6", "c#", 4, "minor"),
            ("C", "-m2", "B", 5, "major"),
        ];
        for (key, interval, tonic, sharps, mode) in cases {
            let moved = Key::from_tonic(key)
                .unwrap()
                .transpose(&Interval::from_name(interval).unwrap())
                .unwrap();
            assert_eq!(
                moved.tonic_pitch_name_with_case(),
                tonic,
                "{key} {interval}"
            );
            assert_eq!(moved.sharps(), sharps, "{key} {interval}");
            assert_eq!(moved.mode(), mode, "{key} {interval}");
        }
        let dorian = Key::from_tonic_mode("D", "dorian")
            .unwrap()
            .as_scale()
            .unwrap();
        assert_eq!(dorian.scale_type(), ScaleType::Dorian);
        assert!(
            Key::from_tonic_mode("D", "hypodorian").is_err()
                || Key::from_tonic_mode("D", "hypodorian")
                    .unwrap()
                    .as_scale()
                    .is_err()
        );
    }

    #[test]
    fn tonic_names_carry_mode_case() {
        let cases = [
            ("C", "C", "major"),
            ("c", "c", "minor"),
            ("F#", "F#", "major"),
            ("f#", "f#", "minor"),
            ("B-", "B-", "major"),
            ("e-", "e-", "minor"),
        ];
        for (input, cased, mode) in cases {
            let key: Key = input.parse().unwrap();
            assert_eq!(key.tonic_pitch_name_with_case(), cased, "{input}");
            assert_eq!(key.mode(), mode, "{input}");
        }
        let dorian = Key::from_tonic_mode("D", "dorian").unwrap();
        assert_eq!(dorian.tonic_pitch_name_with_case(), "D");
    }

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
