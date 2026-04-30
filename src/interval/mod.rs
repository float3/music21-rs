pub(crate) mod chromaticinterval;
pub(crate) mod diatonicinterval;
pub(crate) mod direction;
pub(crate) mod genericinterval;
pub(crate) mod intervalbase;
pub(crate) mod intervalstring;
pub(crate) mod specifier;

use chromaticinterval::ChromaticInterval;
use diatonicinterval::DiatonicInterval;
use genericinterval::GenericInterval;
use intervalbase::IntervalBaseTrait;
use regex::Regex;
use specifier::Specifier;

use std::str::FromStr;
use std::sync::Mutex;
use std::{cmp::Ordering, collections::HashMap, sync::LazyLock};

use crate::base::Music21ObjectTrait;

use crate::common::numbertools::MUSICAL_ORDINAL_STRINGS;
use crate::common::stringtools::get_num_from_str;
use crate::defaults::UnsignedIntegerType;
use crate::error::{Error, Result};
use crate::prebase::ProtoM21ObjectTrait;
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

pub(crate) enum IntervalArgument {
    Str(String),
    Int(IntegerType),
    Pitch(Pitch),
    Note(Note),
}

static PYTHAGOREAN_CACHE: LazyLock<Mutex<HashMap<String, (Pitch, FractionType)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn extract_pitch(arg: PitchOrNote) -> Pitch {
    match arg {
        PitchOrNote::Pitch(pitch) => pitch,
        PitchOrNote::Note(note) => note._pitch,
    }
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

    pub(crate) fn new(arg: IntervalArgument) -> Result<Interval> {
        match arg {
            IntervalArgument::Str(str) => {
                let name = str;
                let (diatonic_new, chromatic_new, inferred) = _string_to_diatonic_chromatic(name)?;
                Ok(Self {
                    implicit_diatonic: inferred,
                    diatonic: diatonic_new,
                    chromatic: chromatic_new,
                    pitch_start: None,
                    pitch_end: None,
                })
            }
            IntervalArgument::Int(int) => {
                let chromatic = ChromaticInterval::new(int);
                let diatonic = chromatic.get_diatonic();

                Ok(Self {
                    implicit_diatonic: true,
                    diatonic,
                    chromatic,
                    pitch_start: None,
                    pitch_end: None,
                })
            }
            IntervalArgument::Pitch(_pitch) => Err(Error::Interval(
                "Constructing Interval from a single Pitch is not supported".to_string(),
            )),
            IntervalArgument::Note(_note) => Err(Error::Interval(
                "Constructing Interval from a single Note is not supported".to_string(),
            )),
        }
    }

    /// Parses an interval name such as `"M3"`, `"P5"`, or `"-m6"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        Self::new(IntervalArgument::Str(name.into()))
    }

    /// Creates an implicit diatonic interval from a chromatic semitone count.
    pub fn from_semitones(semitones: IntegerType) -> Result<Self> {
        Self::new(IntervalArgument::Int(semitones))
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
        self.clone().reverse()
    }

    /// Transposes a pitch by this interval.
    pub fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        self.clone()
            .transpose_pitch_with_options(pitch, false, Some(4))
    }

    /// Transposes a note by this interval.
    pub fn transpose_note(&self, note: &Note) -> Result<Note> {
        let mut out = note.clone();
        out._pitch = self.transpose_pitch(&note._pitch)?;
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
        self,
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
            return self.chromatic.transpose_pitch(p.clone());
        }

        let use_implicit_octave = p.octave().is_none();
        let old_dnn = p.step().step_to_dnn_offset() + (7 * p.octave().unwrap_or(4));
        let new_dnn = old_dnn + self.diatonic.generic.staff_distance();

        let new_octave = (new_dnn - 1).div_euclid(7);
        let step_number = (new_dnn - 1).rem_euclid(7);
        let new_step = crate::stepname::StepName::try_from((step_number + 1) as u8)?;

        let step_char = format!("{new_step:?}");
        let mut pitch2 = Pitch::new(
            Some(format!("{step_char}{new_octave}")),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )?;

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
                pitch2 = Pitch::new(
                    Some(format!("{step_char}{accidental_modifier}{new_octave}")),
                    None,
                    None,
                    Option::<IntegerType>::None,
                    Option::<IntegerType>::None,
                    None,
                    None,
                    None,
                    None,
                )?;
            }
        }

        if use_implicit_octave {
            pitch2.octave_setter(None);
        }
        Ok(pitch2)
    }

    pub(crate) fn transpose_pitch_in_place(
        &self,
        arg: &mut Pitch,
        reverse: bool,
        max_accidental: Option<IntegerType>,
    ) -> Result<()> {
        *arg = self
            .clone()
            .transpose_pitch_with_options(arg, reverse, max_accidental)?;
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

fn _string_to_diatonic_chromatic(
    mut value: String,
) -> Result<(DiatonicInterval, ChromaticInterval, bool)> {
    let mut inferred = false;
    let mut dir_scale = 1;

    // Check for '-' and remove them:
    if value.contains('-') {
        value = value.replace('-', "");
        dir_scale = -1;
    }
    // Remove directional words:
    {
        let descending_re = Regex::new(r"(?i)descending\s*").unwrap();
        if descending_re.is_match(&value) {
            value = descending_re.replace_all(&value, "").to_string();
            dir_scale = -1;
        } else {
            let ascending_re = Regex::new(r"(?i)ascending\s*").unwrap();
            if ascending_re.is_match(&value) {
                value = ascending_re.replace_all(&value, "").to_string();
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
        if value.to_lowercase().contains(&ordinal.to_lowercase()) {
            let pattern = format!(r"(?i)\s*{}\s*", regex::escape(ordinal));
            let re = Regex::new(&pattern).unwrap();
            value = re.replace_all(&value, i.to_string().as_str()).to_string();
        }
    }

    // Extract number and remaining spec:
    let (found, remain) = get_num_from_str(&value, "0123456789");
    let generic_number: IntegerType = found
        .parse::<IntegerType>()
        .expect("Failed to parse number")
        * dir_scale;
    let spec = Specifier::parse(remain);

    let g_interval = GenericInterval::from_int(generic_number)?;
    let d_interval = g_interval.get_diatonic(spec);
    let c_interval = d_interval.get_chromatic()?;
    Ok((d_interval, c_interval, inferred))
}

impl IntervalBaseTrait for Interval {
    fn reverse(self) -> Result<Self>
    where
        Self: Sized,
    {
        if let (Some(start), Some(end)) = (self.pitch_start, self.pitch_end) {
            Interval::between(PitchOrNote::Pitch(end), PitchOrNote::Pitch(start))
        } else {
            Interval::from_diatonic_and_chromatic(
                self.diatonic.reverse()?,
                self.chromatic.reverse()?,
            )
        }
    }

    fn transpose_note(self, note1: Note) -> Result<Note> {
        let mut cloned = note1.clone();
        cloned._pitch =
            Interval::transpose_pitch_with_options(self, &note1._pitch, false, Some(4))?;
        Ok(cloned)
    }

    fn transpose_pitch(self, pitch1: Pitch) -> Result<Pitch> {
        Interval::transpose_pitch_with_options(self, &pitch1, false, Some(4))
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> Result<()> {
        *pitch1 = Interval::transpose_pitch_with_options(self, pitch1, false, Some(4))?;
        Ok(())
    }
}

impl Music21ObjectTrait for Interval {}

impl ProtoM21ObjectTrait for Interval {}

pub(crate) fn interval_to_pythagorean_ratio(interval: Interval) -> Result<FractionType> {
    let start_pitch = Pitch::new(
        Some("C1".to_string()),
        None,
        None,
        Option::<IntegerType>::None,
        Option::<IntegerType>::None,
        None,
        None,
        None,
        None,
    )?;

    let end_pitch_wanted =
        interval
            .clone()
            .transpose_pitch_with_options(&start_pitch, false, Some(4))?;

    let mut cache = match PYTHAGOREAN_CACHE.lock() {
        Ok(cache) => cache,
        Err(poisoned) => poisoned.into_inner(),
    };

    if let Some((cached_pitch, cached_ratio)) = cache.get(&end_pitch_wanted.name()).cloned() {
        let octaves = (end_pitch_wanted.ps() - cached_pitch.ps()) / 12.0;
        let octave_multiplier = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
            &FractionType::new(2 as IntegerType, 1 as IntegerType),
            octaves as IntegerType,
        );
        return Ok(cached_ratio * octave_multiplier);
    }

    let mut end_pitch_up = start_pitch.clone();
    let mut end_pitch_down = start_pitch.clone();
    let mut found: Option<(Pitch, FractionType)> = None;
    let fifth_up = Interval::new(IntervalArgument::Str("P5".to_string()))?;
    let fifth_down = Interval::new(IntervalArgument::Str("-P5".to_string()))?;

    for counter in 0..37 {
        if end_pitch_up.name() == end_pitch_wanted.name() {
            if counter > 18 {
                return Err(Error::Interval(format!(
                    "pythagorean ratio for {} exceeds integer range",
                    end_pitch_wanted.name()
                )));
            }
            found = Some((
                end_pitch_up.clone(),
                FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
                    &FractionType::new(3i32, 2i32),
                    counter,
                ),
            ));
            break;
        } else if end_pitch_down.name() == end_pitch_wanted.name() {
            if counter > 18 {
                return Err(Error::Interval(format!(
                    "pythagorean ratio for {} exceeds integer range",
                    end_pitch_wanted.name()
                )));
            }
            found = Some((
                end_pitch_down.clone(),
                FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
                    &FractionType::new(2i32, 3i32),
                    counter,
                ),
            ));
            break;
        } else {
            end_pitch_up =
                fifth_up
                    .clone()
                    .transpose_pitch_with_options(&end_pitch_up, false, Some(4))?;
            end_pitch_down =
                fifth_down
                    .clone()
                    .transpose_pitch_with_options(&end_pitch_down, false, Some(4))?;
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

    cache.insert(
        end_pitch_wanted.name().clone(),
        (found_pitch.clone(), found_ratio),
    );

    let octaves = (end_pitch_wanted.ps() - found_pitch.ps()) / 12.0;
    let octave_multiplier = FractionPow::<IntegerType, FloatType, UnsignedIntegerType>::powi(
        &FractionType::new(2i32, 1i32),
        octaves as IntegerType,
    );

    Ok(found_ratio * octave_multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(name: &str) -> Pitch {
        Pitch::new(
            Some(name.to_string()),
            None,
            None,
            Option::<IntegerType>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
        .expect("valid pitch")
    }

    #[test]
    fn interval_from_string_has_expected_chromatic() {
        let interval = Interval::new(IntervalArgument::Str("M3".to_string())).unwrap();
        assert_eq!(interval.chromatic.semitones, 4);
        assert!(!interval.implicit_diatonic);
    }

    #[test]
    fn interval_from_int_is_implicit_diatonic() {
        let interval = Interval::new(IntervalArgument::Int(1)).unwrap();
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
        let m3 = Interval::new(IntervalArgument::Str("m3".to_string())).unwrap();
        let out = m3.transpose_pitch(c4).unwrap();
        assert_eq!(out.name_with_octave(), "E-4");
    }

    #[test]
    fn interval_inverts_oblique_unison() {
        let unison = Interval::from_name("P1").unwrap();
        let inverted = unison.inversion().unwrap();

        assert_eq!(inverted.semitones(), 0);
        assert_eq!(inverted.generic_number(), 1);
    }

    #[test]
    fn interval_single_pitch_constructor_is_rejected() {
        let result = Interval::new(IntervalArgument::Pitch(pitch("C4")));
        assert!(result.is_err());
    }
}
