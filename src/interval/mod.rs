pub(crate) mod chromaticinterval;
pub(crate) mod diatonicinterval;
pub(crate) mod direction;
pub(crate) mod genericinterval;
pub(crate) mod intervalbase;
pub(crate) mod specifier;

use chromaticinterval::ChromaticInterval;
use diatonicinterval::DiatonicInterval;
use genericinterval::GenericInterval;
use intervalbase::IntervalBaseTrait;
use specifier::Specifier;

use std::str::FromStr;
use std::{cmp::Ordering, sync::LazyLock};

use crate::common::numbertools::MUSICAL_ORDINAL_STRINGS;
use crate::common::stringtools::get_num_from_str;
use crate::error::{Error, Result};
use crate::{
    defaults::{FloatType, FractionType, IntegerType},
    fraction_pow::FractionPow,
    note::Note,
    pitch::Pitch,
};

/// Direction of a directed interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IntervalDirection {
    /// The end pitch is lower than the start pitch.
    Descending = -1,
    /// The interval is an oblique unison.
    Oblique = 0,
    /// The end pitch is higher than the start pitch.
    Ascending = 1,
}

impl IntervalDirection {
    /// Returns `-1`, `0`, or `1` for descending, oblique, or ascending.
    pub fn as_int(self) -> IntegerType {
        self as IntegerType
    }

    /// Returns a display label for the direction.
    pub fn name(self) -> &'static str {
        match self {
            Self::Descending => "Descending",
            Self::Oblique => "Oblique",
            Self::Ascending => "Ascending",
        }
    }
}

fn public_direction(value: direction::Direction) -> IntervalDirection {
    match value {
        direction::Direction::Descending => IntervalDirection::Descending,
        direction::Direction::Oblique => IntervalDirection::Oblique,
        direction::Direction::Ascending => IntervalDirection::Ascending,
    }
}

#[derive(Clone, Debug)]
/// A directed musical interval with diatonic spelling and chromatic size.
pub struct Interval {
    pub(crate) implicit_diatonic: bool,
    pub(crate) diatonic: DiatonicInterval,
    pub(crate) chromatic: ChromaticInterval,
    pitch_start: Option<Pitch>,
    pitch_end: Option<Pitch>,
}

pub(crate) enum PitchOrNote {
    Pitch(Pitch),
    Note(Note),
}

/// The pure fifths the Pythagorean walk steps by, parsed once rather than
/// re-parsed from "P5"/"-P5" on every call.
static PERFECT_FIFTH_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));
static PERFECT_FIFTH_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("-P5").expect("-P5 is a valid interval"));

fn extract_pitch(arg: PitchOrNote) -> Pitch {
    match arg {
        PitchOrNote::Pitch(pitch) => pitch,
        PitchOrNote::Note(note) => note.pitch,
    }
}

fn strip_direction_word(value: &str, word: &str) -> (String, bool) {
    replace_case_insensitive(value, word, "", false, true)
}

fn replace_music_ordinal(value: &str, ordinal: &str, replacement: &str) -> (String, bool) {
    replace_case_insensitive(value, ordinal, replacement, true, true)
}

fn replace_case_insensitive(
    value: &str,
    needle: &str,
    replacement: &str,
    consume_leading_whitespace: bool,
    consume_trailing_whitespace: bool,
) -> (String, bool) {
    let needle_lower = needle.to_ascii_lowercase();
    let value_lower = value.to_ascii_lowercase();
    let mut output = String::with_capacity(value.len());
    let mut pos = 0;
    let mut replaced = false;

    while let Some(relative_start) = value_lower[pos..].find(&needle_lower) {
        let match_start = pos + relative_start;
        let match_end = match_start + needle.len();
        let mut copy_end = match_start;
        let mut next_pos = match_end;

        if consume_leading_whitespace {
            while copy_end > pos {
                let Some(ch) = value[pos..copy_end].chars().next_back() else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                copy_end -= ch.len_utf8();
            }
        }

        if consume_trailing_whitespace {
            while next_pos < value.len() {
                let Some(ch) = value[next_pos..].chars().next() else {
                    break;
                };
                if !ch.is_whitespace() {
                    break;
                }
                next_pos += ch.len_utf8();
            }
        }

        output.push_str(&value[pos..copy_end]);
        output.push_str(replacement);
        pos = next_pos;
        replaced = true;
    }

    if !replaced {
        return (value.to_string(), false);
    }

    output.push_str(&value[pos..]);
    (output, true)
}

fn convert_staff_distance_to_interval(staff_dist: IntegerType) -> IntegerType {
    match staff_dist.cmp(&0) {
        Ordering::Equal => 1,
        Ordering::Greater => staff_dist + 1,
        Ordering::Less => staff_dist - 1,
    }
}

fn notes_to_generic(p1: &Pitch, p2: &Pitch) -> Result<GenericInterval> {
    let dnn1 = p1.step().step_to_dnn_offset() + (7 * p1.octave().unwrap_or(4));
    let dnn2 = p2.step().step_to_dnn_offset() + (7 * p2.octave().unwrap_or(4));
    let staff_dist = dnn2 - dnn1;
    GenericInterval::from_int(convert_staff_distance_to_interval(staff_dist))
}

fn notes_to_chromatic(p1: &Pitch, p2: &Pitch) -> ChromaticInterval {
    ChromaticInterval::new((p2.ps() - p1.ps()).round() as IntegerType)
}

fn specifier_from_generic_chromatic(
    g_int: &GenericInterval,
    c_int: &ChromaticInterval,
) -> Result<Specifier> {
    let note_vals: [IntegerType; 7] = [0, 2, 4, 5, 7, 9, 11];
    let normal_semis = note_vals[(g_int.simple_undirected() - 1) as usize]
        + 12 * g_int.simple_steps_and_octaves().1;

    let c_direction = match c_int.semitones.cmp(&0) {
        Ordering::Equal => direction::Direction::Oblique,
        Ordering::Less => direction::Direction::Descending,
        Ordering::Greater => direction::Direction::Ascending,
    };

    let these_semis = if g_int.direction() != c_direction
        && g_int.direction() != direction::Direction::Oblique
        && c_direction != direction::Direction::Oblique
    {
        -c_int.semitones.abs()
    } else if g_int.undirected() == 1 {
        c_int.semitones
    } else {
        c_int.semitones.abs()
    };

    let diff = these_semis - normal_semis;

    if g_int.is_perfectable() {
        match diff {
            0 => Ok(Specifier::Perfect),
            1 => Ok(Specifier::Augmented),
            2 => Ok(Specifier::DoubleAugmented),
            3 => Ok(Specifier::TripleAugmented),
            4 => Ok(Specifier::QuadrupleAugmented),
            -1 => Ok(Specifier::Diminished),
            -2 => Ok(Specifier::DoubleDiminished),
            -3 => Ok(Specifier::TripleDiminished),
            -4 => Ok(Specifier::QuadrupleDiminished),
            _ => Err(Error::Interval(format!(
                "cannot get specifier from perfectable diff {diff}"
            ))),
        }
    } else {
        match diff {
            0 => Ok(Specifier::Major),
            -1 => Ok(Specifier::Minor),
            1 => Ok(Specifier::Augmented),
            2 => Ok(Specifier::DoubleAugmented),
            3 => Ok(Specifier::TripleAugmented),
            4 => Ok(Specifier::QuadrupleAugmented),
            -2 => Ok(Specifier::Diminished),
            -3 => Ok(Specifier::DoubleDiminished),
            -4 => Ok(Specifier::TripleDiminished),
            -5 => Ok(Specifier::QuadrupleDiminished),
            _ => Err(Error::Interval(format!(
                "cannot get specifier from major diff {diff}"
            ))),
        }
    }
}

fn intervals_to_diatonic(
    g_int: &GenericInterval,
    c_int: &ChromaticInterval,
) -> Result<DiatonicInterval> {
    let specifier = specifier_from_generic_chromatic(g_int, c_int)?;
    Ok(DiatonicInterval::new(specifier, g_int))
}

pub(crate) fn convert_semitone_to_specifier_generic(
    count: IntegerType,
) -> (Specifier, IntegerType) {
    let dir_scale = if count < 0 { -1 } else { 1 };
    let size = count.abs() % 12;
    let octave = count.abs() / 12;
    let (spec, generic) = match size {
        0 => (Specifier::Perfect, 1),
        1 => (Specifier::Minor, 2),
        2 => (Specifier::Major, 2),
        3 => (Specifier::Minor, 3),
        4 => (Specifier::Major, 3),
        5 => (Specifier::Perfect, 4),
        6 => (Specifier::Diminished, 5),
        7 => (Specifier::Perfect, 5),
        8 => (Specifier::Minor, 6),
        9 => (Specifier::Major, 6),
        10 => (Specifier::Minor, 7),
        _ => (Specifier::Major, 7),
    };
    (spec, (generic + octave * 7) * dir_scale)
}

impl Interval {
    pub(crate) fn between(start: PitchOrNote, end: PitchOrNote) -> Result<Self> {
        let start_pitch = extract_pitch(start);
        let end_pitch = extract_pitch(end);
        let generic = notes_to_generic(&start_pitch, &end_pitch)?;
        let chromatic = notes_to_chromatic(&start_pitch, &end_pitch);
        let diatonic = intervals_to_diatonic(&generic, &chromatic)?;

        Ok(Self {
            implicit_diatonic: false,
            diatonic,
            chromatic,
            pitch_start: Some(start_pitch),
            pitch_end: Some(end_pitch),
        })
    }

    pub(crate) fn from_diatonic_and_chromatic(
        diatonic: DiatonicInterval,
        chromatic: ChromaticInterval,
    ) -> Result<Interval> {
        Ok(Self {
            implicit_diatonic: false,
            diatonic,
            chromatic,
            pitch_start: None,
            pitch_end: None,
        })
    }

    /// Parses an interval name such as `"M3"`, `"P5"`, or `"-m6"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        let (diatonic, chromatic, inferred) = parse_interval_name(name.into())?;
        Ok(Self {
            implicit_diatonic: inferred,
            diatonic,
            chromatic,
            pitch_start: None,
            pitch_end: None,
        })
    }

    /// Creates an implicit diatonic interval from a chromatic semitone count.
    pub fn from_semitones(semitones: IntegerType) -> Result<Self> {
        let chromatic = ChromaticInterval::new(semitones);
        let diatonic = chromatic.get_diatonic();
        Ok(Self {
            implicit_diatonic: true,
            diatonic,
            chromatic,
            pitch_start: None,
            pitch_end: None,
        })
    }

    /// Returns the directed interval from `start` to `end`.
    pub fn between_pitches(start: &Pitch, end: &Pitch) -> Result<Self> {
        Self::between(
            PitchOrNote::Pitch(start.clone()),
            PitchOrNote::Pitch(end.clone()),
        )
    }

    /// Returns the directed interval from `start` to `end`.
    pub fn between_notes(start: &Note, end: &Note) -> Result<Self> {
        Self::between(
            PitchOrNote::Note(start.clone()),
            PitchOrNote::Note(end.clone()),
        )
    }

    /// Returns the directed chromatic size in semitones.
    pub fn semitones(&self) -> IntegerType {
        self.chromatic.semitones
    }

    /// Returns the directed interval direction.
    pub fn direction(&self) -> IntervalDirection {
        public_direction(self.generic().direction())
    }

    /// Returns the human-readable interval name, such as `"Major Third"`.
    pub fn name(&self) -> String {
        self.nice_name()
    }

    /// Returns the simple or compound generic interval number.
    pub fn generic_number(&self) -> IntegerType {
        self.generic().simple_directed()
    }

    /// Returns `true` when the interval was inferred from semitones only.
    pub fn is_implicit_diatonic(&self) -> bool {
        self.implicit_diatonic
    }

    /// Returns the complementary interval inversion.
    pub fn inversion(&self) -> Result<Self> {
        let direction = match self.direction() {
            IntervalDirection::Oblique => 1,
            direction => direction.as_int(),
        };
        let simple = self.generic().simple_undirected();
        let inverted_generic = if simple == 1 { 1 } else { 9 - simple };
        let generic = GenericInterval::from_int(inverted_generic * direction)?;
        let diatonic = DiatonicInterval::new(self.diatonic.specifier.inversion(), &generic);
        let chromatic = diatonic.get_chromatic()?;
        Self::from_diatonic_and_chromatic(diatonic, chromatic)
    }

    /// Returns the same interval in the opposite direction.
    pub fn reversed(&self) -> Result<Self> {
        self.reverse()
    }

    /// Returns the Pythagorean tuning ratio for this interval.
    ///
    /// The ratio is expressed as a rational fraction built from pure fifths,
    /// matching the helper music21 uses for enharmonic scoring.
    pub fn pythagorean_ratio(&self) -> Result<FractionType> {
        interval_to_pythagorean_ratio(self)
    }

    /// Transposes a pitch by this interval.
    pub fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        self.transpose_pitch_with_options(pitch, false, Some(4))
    }

    /// Transposes a note by this interval.
    pub fn transpose_note(&self, note: &Note) -> Result<Note> {
        let mut out = note.clone();
        out.pitch = self.transpose_pitch(&note.pitch)?;
        Ok(out)
    }

    pub(crate) fn generic(&self) -> &GenericInterval {
        &self.diatonic.generic
    }

    pub(crate) fn nice_name(&self) -> String {
        self.diatonic.nice_name()
    }

    pub(crate) fn semi_simple_nice_name(&self) -> String {
        self.diatonic.semi_simple_nice_name()
    }

    /// reverse default is false
    /// maxAccidental default is 4
    pub(crate) fn transpose_pitch_with_options(
        &self,
        p: &Pitch,
        reverse: bool,
        max_accidental: Option<IntegerType>,
    ) -> Result<Pitch> {
        if reverse {
            return self
                .reverse()?
                .transpose_pitch_with_options(p, false, Some(4));
        }
        let max_accidental = max_accidental.unwrap_or(4);

        if self.implicit_diatonic {
            return self.chromatic.transpose_pitch(p);
        }

        let use_implicit_octave = p.octave().is_none();
        let old_dnn = p.step().step_to_dnn_offset() + (7 * p.octave().unwrap_or(4));
        let new_dnn = old_dnn + self.diatonic.generic.staff_distance();

        let new_octave = (new_dnn - 1).div_euclid(7);
        let step_number = (new_dnn - 1).rem_euclid(7);
        let new_step = crate::stepname::StepName::try_from((step_number + 1) as u8)?;

        let step_char = new_step.as_char();
        let mut pitch2 = Pitch::from_name(format!("{step_char}{new_octave}"))?;

        let mut half_steps_to_fix = self.chromatic.semitones as FloatType - (pitch2.ps() - p.ps());
        while half_steps_to_fix >= 12.0 {
            half_steps_to_fix -= 12.0;
            pitch2.octave_setter(Some(pitch2.octave().unwrap_or(4) - 1));
        }
        while half_steps_to_fix <= -12.0 {
            half_steps_to_fix += 12.0;
            pitch2.octave_setter(Some(pitch2.octave().unwrap_or(4) + 1));
        }

        let rounded_fix = half_steps_to_fix.round() as IntegerType;
        if half_steps_to_fix != 0.0 {
            if rounded_fix.abs() > max_accidental {
                pitch2.set_ps(pitch2.ps() + half_steps_to_fix);
            } else {
                let accidental = crate::pitch::accidental::Accidental::new(rounded_fix as i8)?;
                let accidental_modifier = accidental.modifier().to_string();
                pitch2 = Pitch::from_name(format!("{step_char}{accidental_modifier}{new_octave}"))?;
            }
        }

        if use_implicit_octave {
            pitch2.octave_setter(None);
        }
        Ok(pitch2)
    }

    /// Transposes a pitch in place by this interval.
    pub fn transpose_pitch_in_place(&self, pitch: &mut Pitch) -> Result<()> {
        *pitch = self.transpose_pitch(pitch)?;
        Ok(())
    }
}

impl FromStr for Interval {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<&str> for Interval {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<String> for Interval {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<IntegerType> for Interval {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::from_semitones(value)
    }
}

fn parse_interval_name(mut value: String) -> Result<(DiatonicInterval, ChromaticInterval, bool)> {
    let mut inferred = false;
    let mut dir_scale = 1;

    // Check for '-' and remove them:
    if value.contains('-') {
        value = value.replace('-', "");
        dir_scale = -1;
    }
    // Remove directional words:
    {
        let (without_descending, found_descending) = strip_direction_word(&value, "descending");
        if found_descending {
            value = without_descending;
            dir_scale = -1;
        } else {
            let (without_ascending, found_ascending) = strip_direction_word(&value, "ascending");
            if found_ascending {
                value = without_ascending;
            }
        }
    }
    let value_lower = value.to_lowercase();

    // Handle whole/half abbreviations:
    if value_lower == "w" || value_lower == "whole" || value_lower == "tone" {
        value = "M2".to_string();
        inferred = true;
    } else if value_lower == "h" || value_lower == "half" || value_lower == "semitone" {
        value = "m2".to_string();
        inferred = true;
    }

    // Replace any music ordinal in the string with its index.
    for (i, ordinal) in MUSICAL_ORDINAL_STRINGS.iter().enumerate() {
        let replacement = i.to_string();
        let (next_value, replaced) = replace_music_ordinal(&value, ordinal, &replacement);
        if replaced {
            value = next_value;
        }
    }

    // Extract number and remaining spec:
    let (found, remain) = get_num_from_str(&value, "0123456789");
    let generic_number: IntegerType = found
        .parse::<IntegerType>()
        .map_err(|_| Error::Interval(format!("cannot read an interval number from {value:?}")))?
        * dir_scale;
    let spec = Specifier::parse(&remain)?;

    let g_interval = GenericInterval::from_int(generic_number)?;
    let d_interval = g_interval.get_diatonic(spec);
    let c_interval = d_interval.get_chromatic()?;
    Ok((d_interval, c_interval, inferred))
}

impl IntervalBaseTrait for Interval {
    fn reverse(&self) -> Result<Self>
    where
        Self: Sized,
    {
        if let (Some(start), Some(end)) = (&self.pitch_start, &self.pitch_end) {
            Interval::between(
                PitchOrNote::Pitch(end.clone()),
                PitchOrNote::Pitch(start.clone()),
            )
        } else {
            Interval::from_diatonic_and_chromatic(
                self.diatonic.reverse()?,
                self.chromatic.reverse()?,
            )
        }
    }

    fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        Interval::transpose_pitch(self, pitch)
    }
}

pub(crate) fn interval_to_pythagorean_ratio(interval: &Interval) -> Result<FractionType> {
    let start_pitch = Pitch::from_name("C1")?;

    let end_pitch_wanted = interval.transpose_pitch_with_options(&start_pitch, false, Some(4))?;

    let wanted_name = end_pitch_wanted.name();

    let mut end_pitch_up = start_pitch.clone();
    let mut end_pitch_down = start_pitch.clone();
    let mut found: Option<(Pitch, FractionType)> = None;
    let fifth_up: &Interval = &PERFECT_FIFTH_UP;
    let fifth_down: &Interval = &PERFECT_FIFTH_DOWN;

    for counter in 0..37 {
        if end_pitch_up.name() == wanted_name {
            if counter > 18 {
                return Err(Error::Interval(format!(
                    "pythagorean ratio for {wanted_name} exceeds integer range"
                )));
            }
            found = Some((
                end_pitch_up.clone(),
                FractionPow::<IntegerType>::powi(&FractionType::new(3i32, 2i32), counter),
            ));
            break;
        } else if end_pitch_down.name() == wanted_name {
            if counter > 18 {
                return Err(Error::Interval(format!(
                    "pythagorean ratio for {wanted_name} exceeds integer range"
                )));
            }
            found = Some((
                end_pitch_down.clone(),
                FractionPow::<IntegerType>::powi(&FractionType::new(2i32, 3i32), counter),
            ));
            break;
        } else {
            end_pitch_up = fifth_up.transpose_pitch_with_options(&end_pitch_up, false, Some(4))?;
            end_pitch_down =
                fifth_down.transpose_pitch_with_options(&end_pitch_down, false, Some(4))?;
        }
    }

    let (found_pitch, found_ratio) = match found {
        Some(val) => val,
        None => {
            return Err(Error::Interval(format!(
                "Could not find a pythagorean ratio for {interval:?}"
            )));
        }
    };

    let octaves = (end_pitch_wanted.ps() - found_pitch.ps()) / 12.0;
    let octave_multiplier =
        FractionPow::<IntegerType>::powi(&FractionType::new(2i32, 1i32), octaves as IntegerType);

    Ok(found_ratio * octave_multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::from_name(name).expect("valid pitch")
    }

    #[test]
    fn malformed_interval_names_error_instead_of_panicking() {
        // Regression: the generic number was pulled out of the string with
        // `.expect("Failed to parse number")`, so any name with no digits in it
        // panicked out of a Result-returning public API.
        for bad in ["", "X", "perfect", "?!", "MM"] {
            assert!(
                Interval::from_name(bad).is_err(),
                "Interval::from_name({bad:?}) should be an error"
            );
        }
    }

    #[test]
    fn interval_from_string_has_expected_chromatic() {
        let interval = Interval::from_name("M3").unwrap();
        assert_eq!(interval.chromatic.semitones, 4);
        assert!(!interval.implicit_diatonic);
    }

    #[test]
    fn interval_parser_accepts_direction_words_and_ordinals() {
        let descending = Interval::from_name("Descending Perfect Twelfth").unwrap();
        assert_eq!(descending.semitones(), -19);
        assert_eq!(descending.generic_number(), -5);

        let ascending = Interval::from_name("ascending Major Second").unwrap();
        assert_eq!(ascending.semitones(), 2);
        assert_eq!(ascending.generic_number(), 2);

        let major_third = Interval::from_name("Major Third").unwrap();
        assert_eq!(major_third.semitones(), 4);
        assert_eq!(major_third.generic_number(), 3);
    }

    #[test]
    fn interval_from_int_is_implicit_diatonic() {
        let interval = Interval::from_semitones(1).unwrap();
        assert!(interval.implicit_diatonic);
        assert_eq!(interval.chromatic.semitones, 1);
    }

    #[test]
    fn interval_between_pitches() {
        let c4 = pitch("C4");
        let g4 = pitch("G4");
        let interval = Interval::between(PitchOrNote::Pitch(c4), PitchOrNote::Pitch(g4)).unwrap();
        assert_eq!(interval.chromatic.semitones, 7);
        assert_eq!(interval.generic().staff_distance(), 4);
    }

    #[test]
    fn interval_transpose_pitch() {
        let c4 = pitch("C4");
        let m3 = Interval::from_name("m3").unwrap();
        let out = m3.transpose_pitch(&c4).unwrap();
        assert_eq!(out.name_with_octave(), "E-4");
    }

    #[test]
    fn interval_transpose_pitch_in_place() {
        let mut c4 = pitch("C4");
        Interval::from_name("M2")
            .unwrap()
            .transpose_pitch_in_place(&mut c4)
            .unwrap();
        assert_eq!(c4.name_with_octave(), "D4");
    }

    #[test]
    fn interval_pythagorean_ratio() {
        let ratio = Interval::from_name("P5")
            .unwrap()
            .pythagorean_ratio()
            .unwrap();
        assert_eq!(ratio, FractionType::new(3, 2));
    }

    #[test]
    fn interval_inverts_oblique_unison() {
        let unison = Interval::from_name("P1").unwrap();
        let inverted = unison.inversion().unwrap();

        assert_eq!(inverted.semitones(), 0);
        assert_eq!(inverted.generic_number(), 1);
    }
    #[test]
    fn specifier_case_matters_only_for_major_versus_minor() {
        // Verified against music21: it accepts either case for every specifier
        // letter, and m/M is the sole pair where case changes the interval.
        for (lower, upper) in [
            ("p5", "P5"),
            ("a2", "A2"),
            ("d5", "D5"),
            ("aa2", "AA2"),
            ("dd5", "DD5"),
            ("aaa2", "AAA2"),
            ("ddd5", "DDD5"),
        ] {
            let a = Interval::from_name(lower).expect("lowercase parses");
            let b = Interval::from_name(upper).expect("uppercase parses");
            assert_eq!(a.semitones(), b.semitones(), "{lower} vs {upper}");
            assert_eq!(a.name(), b.name(), "{lower} vs {upper}");
        }

        // The carve-out: these must stay different.
        let minor = Interval::from_name("m3").expect("m3 parses");
        let major = Interval::from_name("M3").expect("M3 parses");
        assert_eq!(minor.semitones(), 3);
        assert_eq!(major.semitones(), 4);
    }

    #[test]
    fn an_unknown_specifier_errors_instead_of_panicking() {
        for name in ["Q5", "x3", "5", "zz2"] {
            assert!(
                Interval::from_name(name).is_err(),
                "{name:?} should be rejected, not panic"
            );
        }
    }

    #[test]
    fn a_hyphen_anywhere_makes_an_interval_name_descending() {
        // Verified against music21: it parses every form below identically,
        // so these assertions pin parity rather than a local accident. The
        // prefix form is this crate's convention; "M-2" is where music21's
        // directedName puts the hyphen.
        for name in ["-M2", "M-2"] {
            let interval = Interval::from_name(name).expect("descending name parses");
            assert_eq!(interval.semitones(), -2, "{name}");
            assert_eq!(interval.generic_number(), -2, "{name}");
        }

        // Count and position are both irrelevant: hyphens do not cancel, so
        // repeating one leaves the interval descending rather than flipping it
        // back. music21 agrees on both spellings.
        for name in ["--M2", "-M-2"] {
            let interval = Interval::from_name(name).expect("repeated hyphen parses");
            assert_eq!(interval.semitones(), -2, "{name}");
        }

        // A specifier other than major is unaffected by where the hyphen sits.
        assert_eq!(
            Interval::from_name("d-5").expect("d-5 parses").semitones(),
            -6
        );
    }
}
