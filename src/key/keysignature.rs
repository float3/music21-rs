use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    pitch::{Accidental, Pitch},
    scale::{FIFTHS_ORDER_SHARP, Scale, ScaleType},
};

use super::Key;

use crate::interval::constants::{
    PERFECT_FIFTH_DOWN, PERFECT_FIFTH_UP, PERFECT_FIFTH_UP as PERFECT_FIFTH, PERFECT_FOURTH_UP,
    PERFECT_FOURTH_UP as PERFECT_FOURTH,
};

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

    // A mode nobody has a signature alteration for leaves the signature
    // where the major key put it, which is what music21 does: `C
    // hypomixolydian` is written with no sharps and no flats.
    if let Some(offset) = mode.and_then(mode_sharps_alter) {
        sharps += offset;
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
#[cfg_attr(feature = "serde", serde(from = "SavedKeySignature"))]
/// A key signature represented by the number of sharps.
///
/// Flats are represented as negative sharps, so B-flat major has `-2`.
/// A non-traditional signature, `E-` and `G#` together, is flagged as one
/// and lists its pitches; it keeps a count as well, as music21's does
/// since 14acfa0f2.
#[must_use]
pub struct KeySignature {
    sharps: IntegerType,
    /// music21's `isNonTraditional`, set on its own.
    #[cfg_attr(feature = "serde", serde(default))]
    non_traditional: bool,
    /// Pitches written over the ones the count implies: music21's
    /// `_alteredPitches`, which a traditional signature may carry too.
    altered: Option<Vec<Pitch>>,
    accidentals_apply_only_to_octave: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    color: Option<String>,
}

/// A key signature as it was saved before the count was always a count:
/// a missing count was a non-traditional signature.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct SavedKeySignature {
    sharps: Option<IntegerType>,
    #[serde(default)]
    non_traditional: bool,
    altered: Option<Vec<Pitch>>,
    accidentals_apply_only_to_octave: bool,
    #[serde(default)]
    color: Option<String>,
}

#[cfg(feature = "serde")]
impl From<SavedKeySignature> for KeySignature {
    fn from(saved: SavedKeySignature) -> Self {
        Self {
            sharps: saved.sharps.unwrap_or(0),
            non_traditional: saved.non_traditional || saved.sharps.is_none(),
            altered: saved.altered,
            accidentals_apply_only_to_octave: saved.accidentals_apply_only_to_octave,
            color: saved.color,
        }
    }
}

impl PartialEq for KeySignature {
    /// music21's `equalityAttributes`: the count and the flag.
    fn eq(&self, other: &Self) -> bool {
        self.sharps == other.sharps && self.non_traditional == other.non_traditional
    }
}

impl KeySignature {
    /// Creates a key signature from a sharp count.
    pub fn new(sharps: IntegerType) -> Self {
        Self {
            sharps,
            non_traditional: false,
            altered: None,
            accidentals_apply_only_to_octave: false,
            color: None,
        }
    }

    /// The colour the signature is drawn in, if one was said.
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// Sets the colour the signature is drawn in.
    pub fn set_color(&mut self, color: Option<String>) {
        self.color = color;
    }

    /// Creates a non-traditional key signature from the pitches it alters:
    /// `E-` and `G#` together, say, which no count of sharps can express.
    /// Its count is nought.
    pub fn from_altered_pitches(pitches: Vec<Pitch>) -> Self {
        let mut signature = Self::new(0);
        signature.non_traditional = true;
        signature.altered = Some(pitches);
        signature
    }

    /// Returns the number of sharps, with flats as negative values.
    pub fn sharps(&self) -> IntegerType {
        self.sharps
    }

    /// Replaces the sharp count, leaving the flag and any pitches written
    /// over it as they are, as music21's `sharps` setter does.
    pub fn set_sharps(&mut self, sharps: IntegerType) {
        self.sharps = sharps;
    }

    /// Writes pitches over the ones the count implies, as music21's
    /// `alteredPitches` setter does. It does not make the signature
    /// non-traditional; [`Self::set_non_traditional`] does that.
    pub fn set_altered_pitches(&mut self, pitches: Vec<Pitch>) {
        self.altered = Some(pitches);
    }

    /// Whether the signature is a list of altered pitches rather than a
    /// count of sharps or flats: music21's `isNonTraditional`.
    pub fn is_non_traditional(&self) -> bool {
        self.non_traditional
    }

    /// Flags the signature non-traditional or traditional, its count and
    /// pitches left as they are.
    pub fn set_non_traditional(&mut self, non_traditional: bool) {
        self.non_traditional = non_traditional;
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

    /// The description music21 prints after `of`: `3 sharps`, `1 flat`,
    /// `no sharps or flats`, or `pitches: [E-, G#]`.
    pub fn description(&self) -> String {
        if self.non_traditional {
            let names: Vec<String> = self
                .altered
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(Pitch::to_string)
                .collect();
            return format!("pitches: [{}]", names.join(", "));
        }
        match self.sharps {
            0 => "no sharps or flats".to_string(),
            1 => "1 sharp".to_string(),
            -1 => "1 flat".to_string(),
            n if n > 1 => format!("{n} sharps"),
            n => format!("{} flats", -n),
        }
    }

    /// Returns the pitches this signature alters, in circle-of-fifths order
    /// and without octaves: `F# C# G#` for three sharps, `B- E- A-` for three
    /// flats; a non-traditional signature answers the pitches it was given.
    pub fn altered_pitches(&self) -> Result<Vec<Pitch>> {
        if let Some(altered) = &self.altered {
            return Ok(altered.clone());
        }
        if self.non_traditional {
            return Ok(Vec::new());
        }
        let sharps = self.sharps;
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
            .and_then(|pitch| pitch.written_accidental().cloned()))
    }

    /// Returns the signature of the major key this one's major tonic moves
    /// to by the interval, drawn in the colour this one is.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        let tonic = self.try_as_key(Some("major"), None)?.tonic();
        let mut transposed = interval.transpose_pitch(&tonic)?;
        if interval.implicit_diatonic && pitch_to_sharps(&transposed, None)?.abs() > 6 {
            transposed = transposed.get_enharmonic()?;
        }
        // music21 transposes the object, so only the count moves: its colour,
        // its flag and any pitches written over it stay.
        let mut moved = self.clone();
        moved.sharps = pitch_to_sharps(&transposed, None)?;
        Ok(moved)
    }

    /// Transposes a pitch spelled in C into this key signature the way
    /// music21's `transposePitchFromC` does: up a fifth per sharp, or up a
    /// fourth per flat, keeping the pitch's own octave, so `E4` in two sharps
    /// is `F#4` and in three flats `G4`.
    pub fn transpose_pitch_from_c(&self, pitch: &Pitch) -> Result<Pitch> {
        let sharps = self.sharps;
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
        let our_sharps = self.sharps;

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

    /// The flag turns on and off again and the count stays through both.
    #[test]
    fn a_signature_can_be_made_non_traditional_and_back() {
        let mut signature = KeySignature::new(3);
        assert!(!signature.is_non_traditional());

        signature.set_non_traditional(true);
        assert!(signature.is_non_traditional());
        assert_eq!(signature.sharps(), 3);

        signature.set_non_traditional(false);
        assert!(!signature.is_non_traditional());
        assert_eq!(signature.sharps(), 3);
    }

    fn pitches(names: &[&str]) -> Vec<Pitch> {
        names
            .iter()
            .map(|name| Pitch::from_name(*name).unwrap())
            .collect()
    }

    /// music21 14acfa0f2: `sharps` is always a count, and `isNonTraditional`
    /// is a flag of its own.
    #[test]
    fn a_pitch_list_makes_a_non_traditional_signature() {
        let unusual = KeySignature::from_altered_pitches(pitches(&["Eb", "G#"]));
        assert!(unusual.is_non_traditional());
        assert_eq!(unusual.sharps(), 0);
        assert_eq!(unusual.to_string(), "KeySignature of pitches: [Eb, G#]");
        assert_eq!(
            unusual.accidental_by_step('G').unwrap().unwrap().name(),
            "sharp"
        );
        assert_eq!(unusual.accidental_by_step('A').unwrap(), None);
        // Its count still names a key and still transposes.
        assert_eq!(
            unusual
                .try_as_key(Some("major"), None)
                .unwrap()
                .tonic()
                .name(),
            "C"
        );
        assert_eq!(
            unusual
                .transpose_pitch_from_c(&Pitch::from_name("C4").unwrap())
                .unwrap()
                .name_with_octave(),
            "C4"
        );
    }

    /// Each of the three is set on its own, as music21's attributes are.
    #[test]
    fn sharps_flag_and_pitches_are_set_apart() {
        // Pitches written over a traditional signature leave it traditional.
        let mut signature = KeySignature::new(2);
        signature.set_altered_pitches(pitches(&["Eb"]));
        assert!(!signature.is_non_traditional());
        assert_eq!(signature.sharps(), 2);
        assert_eq!(signature.to_string(), "KeySignature of 2 sharps");
        assert_eq!(signature.altered_pitches().unwrap(), pitches(&["Eb"]));
        signature.set_sharps(3);
        assert_eq!(signature.altered_pitches().unwrap(), pitches(&["Eb"]));

        // The flag keeps the count; the count keeps the flag.
        let mut flagged = KeySignature::new(2);
        flagged.set_non_traditional(true);
        assert_eq!(flagged.sharps(), 2);
        assert_eq!(flagged.to_string(), "KeySignature of pitches: []");
        assert_eq!(flagged.try_as_key(None, None).unwrap().tonic().name(), "D");
        flagged.set_sharps(3);
        assert!(flagged.is_non_traditional());
        flagged.set_non_traditional(false);
        assert_eq!(flagged.to_string(), "KeySignature of 3 sharps");

        assert!(!signature.accidentals_apply_only_to_octave());
        signature.set_accidentals_apply_only_to_octave(true);
        assert!(signature.accidentals_apply_only_to_octave());
        assert_eq!(
            KeySignature::new(0).to_string(),
            "KeySignature of no sharps or flats"
        );
        assert_eq!(KeySignature::new(-4).description(), "4 flats");
    }

    /// A transposed signature keeps its flag and its pitches; only the count
    /// moves.
    #[test]
    fn transposing_moves_only_the_count() {
        let mut unusual = KeySignature::new(2);
        unusual.set_non_traditional(true);
        unusual.set_altered_pitches(pitches(&["B-", "F#"]));
        let moved = unusual
            .transpose(&Interval::from_name("M2").unwrap())
            .unwrap();
        assert_eq!(moved.sharps(), 4);
        assert!(moved.is_non_traditional());
        assert_eq!(moved.altered_pitches().unwrap(), pitches(&["B-", "F#"]));
    }

    /// A non-traditional signature is not the C-major one it shares a count
    /// with.
    #[test]
    fn the_flag_counts_in_equality() {
        let mut flagged = KeySignature::new(2);
        flagged.set_non_traditional(true);
        assert_ne!(flagged, KeySignature::new(2));
        assert_eq!(KeySignature::new(2), KeySignature::new(2));
    }

    #[test]
    fn traditional_altered_pitches_carry_explicit_accidentals() {
        let three_sharps = KeySignature::new(3);
        let altered = three_sharps.altered_pitches().unwrap();
        assert!(
            altered
                .iter()
                .all(|pitch| pitch.written_accidental().is_some())
        );
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
    fn a_signature_names_a_key_in_a_mode_or_from_a_tonic() {
        let two_sharps = KeySignature::new(2);
        assert_eq!(two_sharps.as_key("major").tonic().name(), "D");
        assert_eq!(two_sharps.as_key("minor").tonic().name(), "B");
        let from_tonic = two_sharps.try_as_key(None, Some("b")).unwrap();
        assert_eq!(
            (from_tonic.tonic().name(), from_tonic.mode()),
            ("B".to_string(), "aeolian")
        );
        assert_eq!(
            two_sharps.try_as_key(None, None).unwrap().tonic().name(),
            "D"
        );
        // A mode given beside a tonic wins, and the tonic is not consulted.
        assert_eq!(
            two_sharps
                .try_as_key(Some("major"), Some("C"))
                .unwrap()
                .tonic()
                .name(),
            "D"
        );
        let altered = KeySignature::from_altered_pitches(vec![Pitch::from_name("F#").unwrap()]);
        assert_eq!(altered.as_key("major").tonic().name(), "C");
        assert_eq!(
            pitch_to_sharps(&Pitch::from_name("D").unwrap(), Some("major")).unwrap(),
            2
        );
        assert_eq!(
            pitch_to_sharps(&Pitch::from_name("D").unwrap(), Some("minor")).unwrap(),
            -1
        );
        // A mode with no signature alteration leaves the major key's count.
        assert_eq!(
            pitch_to_sharps(&Pitch::from_name("D").unwrap(), Some("nonsense")).unwrap(),
            2
        );
    }

    #[test]
    fn transpose_pitch_from_c_matches_music21() {
        let cases: [(i32, [&str; 5]); 7] = [
            (0, ["C4", "E4", "Bb3", "F#5", "G"]),
            (1, ["G4", "B4", "F3", "C#5", "D"]),
            (2, ["D4", "F#4", "C3", "G#5", "A"]),
            (-1, ["F4", "A4", "Eb3", "B5", "C"]),
            (-3, ["Eb4", "G4", "Db3", "A5", "Bb"]),
            (7, ["C#4", "E#4", "B3", "F##5", "G#"]),
            (-7, ["Cb4", "Eb4", "Bbb3", "F5", "Gb"]),
        ];
        for (sharps, expected) in cases {
            let signature = KeySignature::new(sharps);
            let actual = ["C4", "E4", "Bb3", "F#5", "G"].map(|name| {
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
                &["Bb", "Eb", "Ab", "Db", "Gb", "Cb", "Fb"],
                [Some("flat"), Some("flat"), Some("flat")],
                [-6, -8, -12],
                "Cb",
                "Ab",
            ),
            (
                -6,
                &["Bb", "Eb", "Ab", "Db", "Gb", "Cb"],
                [None, Some("flat"), Some("flat")],
                [-5, -7, -11],
                "Gb",
                "Eb",
            ),
            (
                -5,
                &["Bb", "Eb", "Ab", "Db", "Gb"],
                [None, Some("flat"), None],
                [-4, -6, -10],
                "Db",
                "Bb",
            ),
            (
                -4,
                &["Bb", "Eb", "Ab", "Db"],
                [None, Some("flat"), None],
                [-3, -5, -9],
                "Ab",
                "F",
            ),
            (
                -3,
                &["Bb", "Eb", "Ab"],
                [None, Some("flat"), None],
                [-2, -4, -8],
                "Eb",
                "C",
            ),
            (
                -2,
                &["Bb", "Eb"],
                [None, Some("flat"), None],
                [-1, -3, -7],
                "Bb",
                "G",
            ),
            (
                -1,
                &["Bb"],
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
                assert_eq!(moved.sharps(), expected, "{sharps} by {name}");
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
        assert_eq!(b_flat.name(), "Bb");
        assert_eq!(sharps_to_pitch(-7).unwrap().name(), "Cb");
        assert_eq!(sharps_to_pitch(7).unwrap().name(), "C#");
        assert_eq!(KeySignature::new(-7).as_key("major").tonic().name(), "Cb");
    }
}
