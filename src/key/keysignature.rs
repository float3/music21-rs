use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    pitch::{Accidental, Pitch},
    scale::{FIFTHS_ORDER_SHARP, Scale, ScaleType},
};

use super::Key;
use std::sync::LazyLock;

static PERFECT_FIFTH_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));
static PERFECT_FOURTH_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P4").expect("P4 is a valid interval"));

static PERFECT_FIFTH: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));
static PERFECT_FOURTH: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P4").expect("P4 is a valid interval"));
static PERFECT_FIFTH_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P-5").expect("P-5 is a valid interval"));

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

/// Returns the circle-of-fifths sharp-count offset for a mode name.
pub fn mode_sharps_alter(mode: &str) -> Option<IntegerType> {
    MODE_SHARPS_ALTER
        .iter()
        .find_map(|(name, value)| (*name == mode.to_lowercase()).then_some(*value))
}

/// Returns the major-key tonic pitch for a key-signature sharp count.
pub fn sharps_to_pitch(sharp_count: IntegerType) -> Result<Pitch> {
    if sharp_count == 0 {
        return Pitch::from_name("C");
    }

    let mut pitch = Pitch::from_name("C")?;
    pitch.octave_setter(None);

    let interval = if sharp_count > 0 {
        &*PERFECT_FIFTH
    } else {
        &*PERFECT_FIFTH_DOWN
    };

    for _ in 0..sharp_count.abs() {
        pitch = interval.transpose_pitch(&pitch)?;
        pitch.octave_setter(None);
    }
    Ok(pitch)
}

/// Returns the key-signature sharp count for a tonic pitch and optional mode.
pub fn pitch_to_sharps(pitch_value: &Pitch, mode: Option<&str>) -> Result<IntegerType> {
    let step_index = FIFTHS_ORDER_SHARP
        .iter()
        .position(|step| *step == pitch_value.step())
        .ok_or_else(|| Error::StepName("cannot map step to circle of fifths".to_string()))?;

    let mut sharps = step_index as IntegerType - 1;
    if !pitch_value.accidental().is_twelve_tone() {
        return Err(Error::Key(
            "Cannot determine sharps for quarter-tone keys! silly!".to_string(),
        ));
    }
    sharps += 7 * pitch_value.accidental().alter() as IntegerType;

    if let Some(mode) = mode {
        let Some(mode_offset) = mode_sharps_alter(mode) else {
            return Err(Error::Key(format!("unknown mode {mode}")));
        };
        sharps += mode_offset;
    }

    Ok(sharps)
}

/// Returns the key-signature sharp count for a tonic pitch name and optional mode.
pub fn pitch_name_to_sharps(pitch_name: &str, mode: Option<&str>) -> Result<IntegerType> {
    let pitch = Pitch::from_name(pitch_name)?;
    pitch_to_sharps(&pitch, mode)
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A key signature represented by the number of sharps.
///
/// Flats are represented as negative sharps, so B-flat major has `-2`.
pub struct KeySignature {
    sharps: Option<IntegerType>,
    altered: Option<Vec<Pitch>>,
    accidentals_apply_only_to_octave: bool,
}

impl KeySignature {
    /// Creates a key signature from a sharp count.
    pub fn new(sharps: IntegerType) -> Self {
        Self {
            sharps: Some(sharps),
            altered: None,
            accidentals_apply_only_to_octave: false,
        }
    }

    /// Creates a non-traditional key signature from the pitches it alters,
    /// as music21 does when `alteredPitches` is assigned: `E-` and `G#`
    /// together, say, which no count of sharps can express. Such a signature
    /// has no sharp count and no key.
    pub fn from_altered_pitches(pitches: Vec<Pitch>) -> Self {
        Self {
            sharps: None,
            altered: Some(pitches),
            accidentals_apply_only_to_octave: false,
        }
    }

    /// Returns the number of sharps, with flats as negative values, or
    /// `None` for a non-traditional signature.
    pub fn sharps(&self) -> Option<IntegerType> {
        self.sharps
    }

    /// Replaces the sharp count, turning a non-traditional signature back
    /// into a traditional one.
    pub fn set_sharps(&mut self, sharps: IntegerType) {
        self.sharps = Some(sharps);
        self.altered = None;
    }

    /// Replaces the altered pitches, turning the signature non-traditional.
    pub fn set_altered_pitches(&mut self, pitches: Vec<Pitch>) {
        self.sharps = None;
        self.altered = Some(pitches);
    }

    /// Whether the signature is a list of altered pitches rather than a
    /// count of sharps or flats.
    pub fn is_non_traditional(&self) -> bool {
        self.sharps.is_none()
    }

    /// Whether the altered pitches of a non-traditional signature carry
    /// octaves that limit where they apply, as `F#4` altering only that
    /// octave. music21 records the flag; applying it is a stream's job.
    pub fn accidentals_apply_only_to_octave(&self) -> bool {
        self.accidentals_apply_only_to_octave
    }

    /// Sets whether the altered pitches apply only in their own octave.
    pub fn set_accidentals_apply_only_to_octave(&mut self, value: bool) {
        self.accidentals_apply_only_to_octave = value;
    }

    fn sharps_or_error(&self) -> Result<IntegerType> {
        self.sharps.ok_or_else(|| {
            Error::Key(
                "a non-traditional key signature has no count of sharps or flats".to_string(),
            )
        })
    }

    /// The description music21 prints after `of`: `3 sharps`, `1 flat`,
    /// `no sharps or flats`, or `pitches: [E-, G#]`.
    pub fn description(&self) -> String {
        match self.sharps {
            None => {
                let names: Vec<String> = self
                    .altered
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(Pitch::to_string)
                    .collect();
                format!("pitches: [{}]", names.join(", "))
            }
            Some(0) => "no sharps or flats".to_string(),
            Some(1) => "1 sharp".to_string(),
            Some(-1) => "1 flat".to_string(),
            Some(n) if n > 1 => format!("{n} sharps"),
            Some(n) => format!("{} flats", -n),
        }
    }

    /// Returns the pitches this signature alters, in circle-of-fifths order
    /// and without octaves: `F# C# G#` for three sharps, `B- E- A-` for three
    /// flats; a non-traditional signature answers the pitches it was given.
    pub fn altered_pitches(&self) -> Result<Vec<Pitch>> {
        if let Some(altered) = &self.altered {
            return Ok(altered.clone());
        }
        let sharps = self.sharps_or_error()?;
        let (start, interval) = if sharps > 0 {
            ("B", &*PERFECT_FIFTH)
        } else {
            ("F", &*PERFECT_FOURTH)
        };
        let mut current = Pitch::from_name(start)?;
        let mut altered = Vec::with_capacity(sharps.unsigned_abs() as usize);
        for _ in 0..sharps.abs() {
            current = interval.transpose_pitch(&current)?;
            current.octave_setter(None);
            altered.push(current.clone());
        }
        Ok(altered)
    }

    /// Returns the accidental this signature puts on a step letter, if any.
    pub fn accidental_by_step(&self, step: char) -> Result<Option<Accidental>> {
        let step = crate::stepname::StepName::try_from(step)?;
        Ok(self
            .altered_pitches()?
            .into_iter()
            .rev()
            .find(|pitch| pitch.step() == step)
            .and_then(|pitch| pitch.explicit_accidental().cloned()))
    }

    /// Returns the signature of the major key this one's major tonic moves
    /// to by the interval.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        let tonic = self.try_as_key(Some("major"), None)?.tonic();
        let mut transposed = interval.transpose_pitch(&tonic)?;
        if interval.implicit_diatonic && pitch_to_sharps(&transposed, None)?.abs() > 6 {
            transposed = transposed.get_enharmonic()?;
        }
        Ok(Self::new(pitch_to_sharps(&transposed, None)?))
    }

    /// Transposes a pitch spelled in C into this key signature the way
    /// music21's `transposePitchFromC` does: up a fifth per sharp, or up a
    /// fourth per flat, keeping the pitch's own octave, so `E4` in two sharps
    /// is `F#4` and in three flats `G4`.
    pub fn transpose_pitch_from_c(&self, pitch: &Pitch) -> Result<Pitch> {
        let sharps = self.sharps_or_error()?;
        if sharps == 0 {
            return Ok(pitch.clone());
        }
        let step = if sharps < 0 {
            &*PERFECT_FOURTH_UP
        } else {
            &*PERFECT_FIFTH_UP
        };
        let mut transposed = pitch.clone();
        for _ in 0..sharps.unsigned_abs() {
            transposed = step.transpose_pitch_with_options(&transposed, false, None)?;
        }
        transposed.octave_setter(pitch.octave());
        Ok(transposed)
    }

    /// Returns the major or minor scale this signature implies.
    pub fn scale(&self, mode: &str) -> Result<Scale> {
        let scale_type = match mode {
            "major" => ScaleType::Major,
            "minor" => ScaleType::Minor,
            other => {
                return Err(Error::Key(format!(
                    "no mapping to a scale exists for this mode yet: {other}"
                )));
            }
        };
        Ok(Scale::new(
            scale_type,
            self.try_as_key(Some(mode), None)?.tonic(),
        ))
    }

    /// Converts this signature to a key in the given mode.
    pub fn as_key(&self, mode: &str) -> Key {
        self.try_as_key(Some(mode), None).unwrap_or_else(|_| {
            Key::new(Pitch::from_name("C").expect("C is valid pitch"), "major", 0)
        })
    }

    /// Converts this signature to a key, optionally inferring mode from tonic.
    pub fn try_as_key(&self, mode: Option<&str>, tonic: Option<&str>) -> Result<Key> {
        let our_sharps = self.sharps_or_error()?;

        let resolved_mode = if mode.is_none() && tonic.is_none() {
            "major".to_string()
        } else if mode.is_none() && tonic.is_some() {
            let tonic_name = tonic.expect("checked is_some above");
            let major_sharps = pitch_name_to_sharps(tonic_name, None)?;
            canonical_mode_for_offset(our_sharps - major_sharps)
                .ok_or_else(|| {
                    Error::Key(format!(
                        "Could not solve for mode from sharps={our_sharps}, tonic={tonic_name}"
                    ))
                })?
                .to_string()
        } else {
            mode.expect("checked is_some above").to_lowercase()
        };

        let sharp_alteration_from_major = mode_sharps_alter(&resolved_mode)
            .ok_or_else(|| Error::Key(format!("Mode {resolved_mode} is unknown")))?;

        let tonic_pitch = sharps_to_pitch(our_sharps - sharp_alteration_from_major)?;
        Ok(Key::new(tonic_pitch, &resolved_mode, our_sharps))
    }
}

impl std::fmt::Display for KeySignature {
    /// `KeySignature of 3 sharps`, `KeySignature of pitches: [E-, G#]`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeySignature of {}", self.description())
    }
}

#[cfg(test)]
mod non_traditional_tests {
    use super::*;

    fn pitches(names: &[&str]) -> Vec<Pitch> {
        names
            .iter()
            .map(|name| Pitch::from_name(*name).unwrap())
            .collect()
    }

    #[test]
    fn a_pitch_list_makes_a_non_traditional_signature() {
        let unusual = KeySignature::from_altered_pitches(pitches(&["E-", "G#"]));
        assert!(unusual.is_non_traditional());
        assert_eq!(unusual.sharps(), None);
        assert_eq!(unusual.to_string(), "KeySignature of pitches: [E-, G#]");
        assert_eq!(
            unusual.accidental_by_step('G').unwrap().unwrap().name(),
            "sharp"
        );
        assert_eq!(unusual.accidental_by_step('A').unwrap(), None);
        assert!(unusual.try_as_key(Some("major"), None).is_err());
        assert!(
            unusual
                .transpose_pitch_from_c(&Pitch::from_name("C4").unwrap())
                .is_err()
        );
    }

    #[test]
    fn setting_pitches_and_sharps_switches_kinds() {
        let mut signature = KeySignature::new(2);
        assert_eq!(signature.to_string(), "KeySignature of 2 sharps");
        signature.set_altered_pitches(pitches(&["F#4"]));
        assert!(signature.is_non_traditional());
        assert_eq!(signature.to_string(), "KeySignature of pitches: [F#4]");
        assert!(!signature.accidentals_apply_only_to_octave());
        signature.set_accidentals_apply_only_to_octave(true);
        assert!(signature.accidentals_apply_only_to_octave());
        signature.set_sharps(-1);
        assert!(!signature.is_non_traditional());
        assert_eq!(signature.to_string(), "KeySignature of 1 flat");
        assert_eq!(
            KeySignature::new(0).to_string(),
            "KeySignature of no sharps or flats"
        );
        assert_eq!(KeySignature::new(-4).description(), "4 flats");
    }

    #[test]
    fn traditional_altered_pitches_carry_explicit_accidentals() {
        let three_sharps = KeySignature::new(3);
        let altered = three_sharps.altered_pitches().unwrap();
        assert!(altered.iter().all(Pitch::has_accidental));
        assert_eq!(
            three_sharps
                .accidental_by_step('F')
                .unwrap()
                .unwrap()
                .name(),
            "sharp"
        );
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn transpose_pitch_from_c_matches_music21() {
        let cases: [(i32, [&str; 5]); 7] = [
            (0, ["C4", "E4", "B-3", "F#5", "G"]),
            (1, ["G4", "B4", "F3", "C#5", "D"]),
            (2, ["D4", "F#4", "C3", "G#5", "A"]),
            (-1, ["F4", "A4", "E-3", "B5", "C"]),
            (-3, ["E-4", "G4", "D-3", "A5", "B-"]),
            (7, ["C#4", "E#4", "B3", "F##5", "G#"]),
            (-7, ["C-4", "E-4", "B--3", "F5", "G-"]),
        ];
        for (sharps, expected) in cases {
            let signature = KeySignature::new(sharps);
            let actual = ["C4", "E4", "B-3", "F#5", "G"].map(|name| {
                signature
                    .transpose_pitch_from_c(&Pitch::from_name(name).unwrap())
                    .unwrap()
                    .name_with_octave()
            });
            assert_eq!(actual, expected, "{sharps} sharps");
        }
    }
    use super::*;

    #[test]
    fn altered_pitches_and_transpositions_match_music21() {
        type Row = (
            i32,
            &'static [&'static str],
            [Option<&'static str>; 3],
            [i32; 3],
            &'static str,
            &'static str,
        );
        let cases: [Row; 15] = [
            (
                -7,
                &["B-", "E-", "A-", "D-", "G-", "C-", "F-"],
                [Some("flat"), Some("flat"), Some("flat")],
                [-6, -8, -12],
                "C-",
                "A-",
            ),
            (
                -6,
                &["B-", "E-", "A-", "D-", "G-", "C-"],
                [None, Some("flat"), Some("flat")],
                [-5, -7, -11],
                "G-",
                "E-",
            ),
            (
                -5,
                &["B-", "E-", "A-", "D-", "G-"],
                [None, Some("flat"), None],
                [-4, -6, -10],
                "D-",
                "B-",
            ),
            (
                -4,
                &["B-", "E-", "A-", "D-"],
                [None, Some("flat"), None],
                [-3, -5, -9],
                "A-",
                "F",
            ),
            (
                -3,
                &["B-", "E-", "A-"],
                [None, Some("flat"), None],
                [-2, -4, -8],
                "E-",
                "C",
            ),
            (
                -2,
                &["B-", "E-"],
                [None, Some("flat"), None],
                [-1, -3, -7],
                "B-",
                "G",
            ),
            (
                -1,
                &["B-"],
                [None, Some("flat"), None],
                [0, -2, -6],
                "F",
                "D",
            ),
            (0, &[], [None, None, None], [1, -1, -5], "C", "A"),
            (
                1,
                &["F#"],
                [Some("sharp"), None, None],
                [2, 0, -4],
                "G",
                "E",
            ),
            (
                2,
                &["F#", "C#"],
                [Some("sharp"), None, Some("sharp")],
                [3, 1, -3],
                "D",
                "B",
            ),
            (
                3,
                &["F#", "C#", "G#"],
                [Some("sharp"), None, Some("sharp")],
                [4, 2, -2],
                "A",
                "F#",
            ),
            (
                4,
                &["F#", "C#", "G#", "D#"],
                [Some("sharp"), None, Some("sharp")],
                [5, 3, -1],
                "E",
                "C#",
            ),
            (
                5,
                &["F#", "C#", "G#", "D#", "A#"],
                [Some("sharp"), None, Some("sharp")],
                [6, 4, 0],
                "B",
                "G#",
            ),
            (
                6,
                &["F#", "C#", "G#", "D#", "A#", "E#"],
                [Some("sharp"), None, Some("sharp")],
                [7, 5, 1],
                "F#",
                "D#",
            ),
            (
                7,
                &["F#", "C#", "G#", "D#", "A#", "E#", "B#"],
                [Some("sharp"), Some("sharp"), Some("sharp")],
                [8, 6, 2],
                "C#",
                "A#",
            ),
        ];
        for (sharps, altered, by_step, transposed, major, minor) in cases {
            let signature = KeySignature::new(sharps);
            let names = signature
                .altered_pitches()
                .unwrap()
                .iter()
                .map(Pitch::name)
                .collect::<Vec<_>>();
            assert_eq!(names, altered, "{sharps} altered");
            for (step, expected) in ['F', 'B', 'C'].into_iter().zip(by_step) {
                let accidental = signature.accidental_by_step(step).unwrap();
                assert_eq!(
                    accidental.as_ref().map(Accidental::name),
                    expected,
                    "{sharps} {step}"
                );
            }
            for (name, expected) in ["P5", "-P5", "m2"].into_iter().zip(transposed) {
                let moved = signature
                    .transpose(&Interval::from_name(name).unwrap())
                    .unwrap();
                assert_eq!(moved.sharps(), Some(expected), "{sharps} by {name}");
            }
            assert_eq!(signature.scale("major").unwrap().tonic().name(), major);
            assert_eq!(signature.scale("minor").unwrap().tonic().name(), minor);
        }
        assert!(KeySignature::new(0).scale("dorian").is_err());
        assert!(KeySignature::new(0).accidental_by_step('H').is_err());
    }
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
        assert_eq!(sharps_to_pitch(-7).unwrap().name(), "C-");
        assert_eq!(sharps_to_pitch(7).unwrap().name(), "C#");
        assert_eq!(KeySignature::new(-7).as_key("major").tonic().name(), "C-");
    }
}
