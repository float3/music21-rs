use crate::{
    chord::Chord,
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    key::Key,
    pitch::Pitch,
};

/// A parsed Roman numeral in a key.
#[derive(Clone, Debug)]
pub struct RomanNumeral {
    figure: String,
    key: Key,
    degree: u8,
    inversion: u8,
    seventh: bool,
    quality: RomanQuality,
    secondary: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RomanQuality {
    Major,
    Minor,
    Diminished,
    Augmented,
}

impl RomanNumeral {
    /// Parses a Roman numeral figure in a key.
    pub fn new(figure: impl Into<String>, key: Key) -> Result<Self> {
        let figure = figure.into();
        let trimmed = figure.trim();
        if trimmed.is_empty() {
            return Err(Error::Chord("roman numeral cannot be empty".to_string()));
        }

        let (primary, secondary) = match trimmed.split_once('/') {
            Some((primary, secondary)) => (primary, Some(secondary.to_string())),
            None => (trimmed, None),
        };

        let (roman, suffix) = split_roman_prefix(primary)?;
        let degree = roman_degree(roman)?;
        let quality = roman_quality(roman, suffix);
        let inversion = parse_inversion(suffix);
        let seventh = suffix.contains('7')
            || suffix.contains("65")
            || suffix.contains("43")
            || suffix.contains("42");

        Ok(Self {
            figure: trimmed.to_string(),
            key,
            degree,
            inversion,
            seventh,
            quality,
            secondary,
        })
    }

    /// Returns the original figure.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the one-based scale degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the inversion number, where root position is `0`.
    pub fn inversion(&self) -> u8 {
        self.inversion
    }

    /// Returns the secondary/applied target figure, if any.
    pub fn secondary(&self) -> Option<&str> {
        self.secondary.as_deref()
    }

    /// Returns the key context.
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Realizes the Roman numeral as a chord.
    pub fn to_chord(&self) -> Result<Chord> {
        let effective_key = self.effective_key()?;
        let root = effective_key.pitch_from_degree(self.degree as usize)?;
        let mut pitches = self
            .interval_names()
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&root))
            .collect::<Result<Vec<_>>>()?;

        for _ in 0..self.inversion.min(pitches.len().saturating_sub(1) as u8) {
            let pitch = pitches.remove(0);
            let transposed = Interval::from_name("P8")?.transpose_pitch(&pitch)?;
            pitches.push(transposed);
        }

        Chord::new(pitches.as_slice())
    }

    /// Performs a compact root/quality Roman-numeral analysis in a key.
    pub fn analyze(chord: &Chord, key: Key) -> Result<Option<Self>> {
        let Some(root_name) = chord.root_pitch_name() else {
            return Ok(None);
        };
        let root = Pitch::from_name(normalize_pitch_name(&root_name))?;
        let root_pc = pitch_class(&root);

        for degree in 1..=7 {
            let degree_pitch = key.pitch_from_degree(degree)?;
            if pitch_class(&degree_pitch) != root_pc {
                continue;
            }

            let common_name = chord.common_name();
            let base = degree_to_roman(degree as u8);
            let figure = if common_name.contains("diminished") {
                format!("{}o", base.to_ascii_lowercase())
            } else if common_name.contains("minor") && !common_name.contains("major") {
                base.to_ascii_lowercase()
            } else {
                base.to_string()
            };

            let figure = if common_name.contains("seventh") {
                format!("{figure}7")
            } else {
                figure
            };

            return Self::new(figure, key).map(Some);
        }

        Ok(None)
    }

    fn effective_key(&self) -> Result<Key> {
        let Some(secondary) = &self.secondary else {
            return Ok(self.key.clone());
        };

        let (roman, _) = split_roman_prefix(secondary)?;
        let degree = roman_degree(roman)?;
        let tonic = self.key.pitch_from_degree(degree as usize)?;
        let mode = if roman.chars().next().is_some_and(char::is_uppercase) {
            "major"
        } else {
            "minor"
        };
        Key::from_tonic_mode(&tonic.name(), mode)
    }

    fn interval_names(&self) -> Vec<&'static str> {
        match (self.quality, self.seventh) {
            (RomanQuality::Major, false) => vec!["P1", "M3", "P5"],
            (RomanQuality::Major, true) => vec!["P1", "M3", "P5", "m7"],
            (RomanQuality::Minor, false) => vec!["P1", "m3", "P5"],
            (RomanQuality::Minor, true) => vec!["P1", "m3", "P5", "m7"],
            (RomanQuality::Diminished, false) => vec!["P1", "m3", "d5"],
            (RomanQuality::Diminished, true) => vec!["P1", "m3", "d5", "d7"],
            (RomanQuality::Augmented, false) => vec!["P1", "M3", "a5"],
            (RomanQuality::Augmented, true) => vec!["P1", "M3", "a5", "m7"],
        }
    }
}

/// Performs a compact root/quality Roman-numeral analysis in a key.
pub fn analyze_chord(chord: &Chord, key: Key) -> Result<Option<RomanNumeral>> {
    RomanNumeral::analyze(chord, key)
}

fn split_roman_prefix(value: &str) -> Result<(&str, &str)> {
    let end = value
        .char_indices()
        .find_map(|(idx, ch)| (!matches!(ch, 'I' | 'V' | 'X' | 'i' | 'v' | 'x')).then_some(idx))
        .unwrap_or(value.len());

    if end == 0 {
        return Err(Error::Chord(format!("missing roman numeral in {value:?}")));
    }

    Ok((&value[..end], &value[end..]))
}

fn roman_degree(roman: &str) -> Result<u8> {
    match roman.to_ascii_uppercase().as_str() {
        "I" => Ok(1),
        "II" => Ok(2),
        "III" => Ok(3),
        "IV" => Ok(4),
        "V" => Ok(5),
        "VI" => Ok(6),
        "VII" => Ok(7),
        _ => Err(Error::Chord(format!("unsupported roman numeral {roman:?}"))),
    }
}

fn roman_quality(roman: &str, suffix: &str) -> RomanQuality {
    let lower = suffix.to_ascii_lowercase();
    if lower.contains('o') || lower.contains("dim") {
        RomanQuality::Diminished
    } else if lower.contains('+') || lower.contains("aug") {
        RomanQuality::Augmented
    } else if roman.chars().next().is_some_and(char::is_lowercase) {
        RomanQuality::Minor
    } else {
        RomanQuality::Major
    }
}

fn parse_inversion(suffix: &str) -> u8 {
    if suffix.contains("64") || suffix.contains("43") {
        2
    } else if suffix.contains("65") || suffix.contains('6') {
        1
    } else if suffix.contains("42") {
        3
    } else {
        0
    }
}

fn degree_to_roman(degree: u8) -> &'static str {
    match degree {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        7 => "VII",
        _ => "I",
    }
}

fn normalize_pitch_name(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut normalized = first.to_string();
    for ch in chars {
        if ch == 'b' {
            normalized.push('-');
        } else {
            normalized.push(ch);
        }
    }
    normalized
}

fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_dominant_resolves_to_chord() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let rn = RomanNumeral::new("V7/V", key).unwrap();
        assert_eq!(rn.degree(), 5);
        assert_eq!(rn.secondary(), Some("V"));
        assert_eq!(
            rn.to_chord().unwrap().pitched_common_name(),
            "D-dominant seventh chord"
        );
    }

    #[test]
    fn analyzes_chord_in_key() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let chord = Chord::new("G B D F").unwrap();
        let rn = RomanNumeral::analyze(&chord, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "V7");
    }

    #[test]
    fn roman_numerals_parse_inversions_and_qualities() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let first_inversion = RomanNumeral::new("I6", key.clone()).unwrap();
        assert_eq!(first_inversion.inversion(), 1);
        assert_eq!(
            first_inversion
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G", "C"]
        );

        let diminished = RomanNumeral::new("viio7", key.clone()).unwrap();
        assert_eq!(diminished.degree(), 7);
        assert!(
            diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("diminished")
        );

        let augmented = RomanNumeral::new("III+", key).unwrap();
        assert_eq!(
            augmented
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G#", "B#"]
        );
    }

    #[test]
    fn roman_numerals_report_invalid_figures_and_empty_analysis() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        assert!(RomanNumeral::new("", key.clone()).is_err());
        assert!(RomanNumeral::new("Q", key.clone()).is_err());
        assert!(
            analyze_chord(&Chord::empty().unwrap(), key)
                .unwrap()
                .is_none()
        );
    }
}
