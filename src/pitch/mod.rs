pub(crate) mod accidental;
pub(crate) mod microtone;
pub(crate) mod pitchclass;

use crate::defaults::FloatType;
use crate::defaults::IntegerType;
use crate::defaults::Octave;
use crate::defaults::PITCH_OCTAVE;
use crate::defaults::PITCH_SPACE_SIGNIFICANT_DIGITS;
use crate::defaults::PITCH_STEP;
use crate::defaults::UnsignedIntegerType;
use crate::error::Error;
use crate::error::Result;
use crate::interval::Interval;
use crate::interval::PitchOrNote;
use crate::key::keysignature::KeySignature;
use crate::stepname::StepName;
use crate::tuningsystem::OCTAVE_SIZE;
use crate::tuningsystem::TuningSystem;

pub use accidental::{Accidental, AccidentalSpecifier};
pub use microtone::{Microtone, MicrotoneSpecifier};
use pitchclass::convert_ps_to_oct;
pub use pitchclass::{PitchClass, PitchClassSpecifier};

use itertools::Itertools;
use num::Num;
use num_traits::ToPrimitive;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::LazyLock;

/// Canonical pitch names for chromatic pitch classes.
pub const CHROMATIC_PITCH_CLASS_NAMES: [&str; 12] = [
    "C", "D-", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];

/// Returns a canonical pitch name for a chromatic pitch class.
pub fn pitch_class_name(pitch_class: u8) -> &'static str {
    CHROMATIC_PITCH_CLASS_NAMES[pitch_class as usize % 12]
}

/// The two intervals enharmonic respelling can ever need: a diminished second
/// up and the same interval down.
static DIMINISHED_SECOND_UP: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("d2").expect("d2 is a valid interval"));
static DIMINISHED_SECOND_DOWN: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("-d2").expect("-d2 is a valid interval"));

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Input accepted as a pitch name or pitch-space number.
pub enum PitchName {
    /// A written pitch name such as `"C#4"` or `"E-"`.
    Name(String),
    /// A pitch-space number, where 60 corresponds to middle C.
    Number(FloatType),
}

impl From<&str> for PitchName {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

impl From<String> for PitchName {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<IntegerType> for PitchName {
    fn from(value: IntegerType) -> Self {
        Self::Number(value as FloatType)
    }
}

impl From<FloatType> for PitchName {
    fn from(value: FloatType) -> Self {
        Self::Number(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Builder options for constructing a [`Pitch`].
pub struct PitchOptions {
    /// Pitch name or pitch-space number.
    pub name: Option<PitchName>,
    /// Diatonic step name.
    pub step: Option<char>,
    /// Octave number.
    pub octave: Octave,
    /// Accidental name or alteration.
    pub accidental: Option<AccidentalSpecifier>,
    /// Microtone cent offset.
    pub microtone: Option<MicrotoneSpecifier>,
    /// Pitch class to realize as a pitch.
    pub pitch_class: Option<PitchClassSpecifier>,
    /// MIDI note number.
    pub midi: Option<IntegerType>,
    /// Pitch-space value.
    pub ps: Option<FloatType>,
    /// Fundamental pitch used for harmonic construction.
    pub fundamental: Option<Pitch>,
}

impl PitchOptions {
    /// Creates an empty pitch builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the pitch name or pitch-space number.
    pub fn name(mut self, name: impl Into<PitchName>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the diatonic step.
    pub fn step(mut self, step: char) -> Self {
        self.step = Some(step);
        self
    }

    /// Sets the octave.
    pub fn octave(mut self, octave: IntegerType) -> Self {
        self.octave = Some(octave);
        self
    }

    /// Sets the accidental.
    pub fn accidental(mut self, accidental: impl Into<AccidentalSpecifier>) -> Self {
        self.accidental = Some(accidental.into());
        self
    }

    /// Sets the microtone.
    pub fn microtone(mut self, microtone: impl Into<MicrotoneSpecifier>) -> Self {
        self.microtone = Some(microtone.into());
        self
    }

    /// Sets the pitch class.
    pub fn pitch_class(mut self, pitch_class: impl Into<PitchClassSpecifier>) -> Self {
        self.pitch_class = Some(pitch_class.into());
        self
    }

    /// Sets the MIDI note number.
    pub fn midi(mut self, midi: IntegerType) -> Self {
        self.midi = Some(midi);
        self
    }

    /// Sets the pitch-space value.
    pub fn ps(mut self, ps: FloatType) -> Self {
        self.ps = Some(ps);
        self
    }

    /// Sets the pitch-space value.
    pub fn pitch_space(mut self, pitch_space: FloatType) -> Self {
        self.ps = Some(pitch_space);
        self
    }

    /// Sets the fundamental pitch.
    pub fn fundamental(mut self, fundamental: Pitch) -> Self {
        self.fundamental = Some(fundamental);
        self
    }

    /// Builds a [`Pitch`] from the collected options.
    pub fn build(self) -> Result<Pitch> {
        Pitch::from_options(self)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A musical pitch with spelling, octave, accidental and optional microtone.
pub struct Pitch {
    step: StepName,
    octave: Octave,
    accidental: Accidental,
    microtone: Option<Microtone>,
    spelling_is_inferred: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    fundamental: Option<Arc<Pitch>>,
}

impl PartialEq for Pitch {
    fn eq(&self, other: &Self) -> bool {
        self.step == other.step
            && self.octave == other.octave
            && self.accidental == other.accidental
            && self.microtone == other.microtone
    }
}

impl FromStr for Pitch {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<&str> for Pitch {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<String> for Pitch {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_name(value)
    }
}

impl TryFrom<&Pitch> for Pitch {
    type Error = Error;

    fn try_from(value: &Pitch) -> Result<Self> {
        Ok(value.clone())
    }
}

impl TryFrom<IntegerType> for Pitch {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::from_midi(value)
    }
}

impl TryFrom<FloatType> for Pitch {
    type Error = Error;

    fn try_from(value: FloatType) -> Result<Self> {
        Self::from_pitch_space(value)
    }
}

impl Display for Pitch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name_with_octave())
    }
}

impl Pitch {
    /// Builds a pitch from [`PitchOptions`].
    ///
    /// This is the port of music21's keyword-argument `Pitch.__init__`: a
    /// `name` wins over an explicit `step`, and `octave`, `accidental`,
    /// `microtone`, `pitch_class`, `midi` and `ps` are applied afterwards
    /// in that order.
    pub fn from_options(options: PitchOptions) -> Result<Self> {
        let PitchOptions {
            name,
            step,
            octave,
            accidental,
            microtone,
            pitch_class,
            midi,
            ps,
            fundamental,
        } = options;
        let explicit_step = step.map(StepName::try_from).transpose()?;
        let has_explicit_octave = octave.is_some();
        let has_explicit_accidental = accidental.is_some();

        let parsed = name.map(PitchParameters::from).unwrap_or_default();
        let name = parsed.name;
        let step = if name.is_some() || parsed.step.is_some() {
            parsed.step
        } else {
            explicit_step
        }
        .unwrap_or(PITCH_STEP);
        let octave = octave.or(parsed.octave);
        let accidental = match accidental {
            Some(accidental) => Accidental::new(accidental)?,
            None => parsed.accidental.unwrap_or_default(),
        };
        let microtone = microtone.map(Microtone::new).transpose()?;

        let mut pitch = Pitch {
            step,
            accidental,
            microtone,
            octave,
            spelling_is_inferred: parsed.spelling_is_inferred,
            fundamental: None,
        };

        if let Some(name) = &name {
            pitch.name_setter(name)?;
        }
        if explicit_step.is_some() || name.is_none() {
            pitch.step_setter(step);
        }
        if has_explicit_octave || name.is_none() {
            pitch.octave_setter(octave);
        }
        if has_explicit_accidental || name.is_none() {
            pitch.accidental_setter(pitch.accidental.clone());
        }
        if let Some(microtone) = pitch.microtone.clone() {
            pitch.microtone_setter(microtone);
        }
        if let Some(pitch_class) = pitch_class {
            pitch.pitch_class_setter(pitch_class)?;
        }
        if let Some(fundamental) = fundamental {
            pitch.fundamental_setter(fundamental);
        }
        if let Some(midi) = midi {
            pitch.midi_setter(midi);
        }
        if let Some(ps) = ps {
            pitch.ps_setter(ps);
        }

        Ok(pitch)
    }

    /// Creates a [`PitchOptions`] builder.
    pub fn builder() -> PitchOptions {
        PitchOptions::new()
    }

    /// Builds a pitch from a name such as `"C#4"` or `"E-"`.
    pub fn from_name(name: impl Into<String>) -> Result<Self> {
        PitchOptions::new().name(name.into()).build()
    }

    /// Builds a pitch from a pitch-space number.
    pub fn from_number(number: FloatType) -> Result<Self> {
        PitchOptions::new().name(PitchName::Number(number)).build()
    }

    /// Builds a pitch from a diatonic step.
    pub fn from_step(step: char) -> Result<Self> {
        PitchOptions::new().step(step).build()
    }

    /// Builds a pitch from a pitch name and explicit octave.
    pub fn from_name_and_octave(name: impl Into<String>, octave: IntegerType) -> Result<Self> {
        PitchOptions::new().name(name.into()).octave(octave).build()
    }

    /// Builds a pitch from a pitch class.
    pub fn from_pitch_class(pitch_class: impl Into<PitchClassSpecifier>) -> Result<Self> {
        PitchOptions::new().pitch_class(pitch_class).build()
    }

    /// Builds a pitch from a MIDI note number.
    pub fn from_midi(midi: IntegerType) -> Result<Self> {
        PitchOptions::new().midi(midi).build()
    }

    /// Builds a pitch from a pitch-space value.
    pub fn from_pitch_space(ps: FloatType) -> Result<Self> {
        PitchOptions::new().ps(ps).build()
    }

    /// Returns the pitch name with the octave suffix when one is set.
    pub fn name_with_octave(&self) -> String {
        match self.octave {
            Some(octave) => format!("{}{}", self.name(), octave),
            None => self.name(),
        }
    }

    /// Returns the pitch name without octave, such as `"F#"` or `"B-"`.
    pub fn name(&self) -> String {
        format!("{}{}", self.step.as_char(), self.accidental.modifier())
    }

    fn name_setter(&mut self, usr_str: &str) -> Result<()> {
        let usr_str = usr_str.trim();

        let digit_index = usr_str
            .char_indices()
            .find(|&(_, c)| c.is_ascii_digit())
            .map(|(i, _)| i);

        let (pitch_part, octave_part) = if let Some(i) = digit_index {
            if i == 0 {
                return Err(Error::Pitch(format!(
                    "Cannot have octave given before pitch name in {usr_str:?}"
                )));
            }
            (&usr_str[..i], &usr_str[i..])
        } else {
            (usr_str, "")
        };

        // Process the pitch part.
        let mut pitch_chars = pitch_part.chars();
        let step = pitch_chars.next().ok_or(Error::Pitch(format!(
            "Cannot make a name out of {pitch_part:?}"
        )))?;
        self.step_setter(StepName::try_from(step)?);

        let accidental_str: String = pitch_chars.collect();
        if accidental_str.is_empty() {
            self.accidental_setter(Accidental::natural());
        } else {
            self.accidental_setter(Accidental::new(accidental_str)?);
        }

        if !octave_part.is_empty() {
            let octave = octave_part
                .parse::<IntegerType>()
                .map_err(|_| Error::Pitch(format!("Cannot parse {octave_part:?} to octave")))?;
            self.octave_setter(Some(octave));
        }

        Ok(())
    }

    /// Returns the total semitone alteration from the natural step.
    pub fn alter(&self) -> FloatType {
        let mut post = 0.0;

        post += self.accidental.alter;

        if let Some(microtone) = &self.microtone {
            post += microtone.alter();
        }

        post
    }

    /// Returns this pitch's accidental object.
    ///
    /// Unlike Python music21, this crate stores an explicit natural accidental
    /// for natural pitches.
    pub fn accidental(&self) -> &Accidental {
        &self.accidental
    }

    /// Returns this pitch's microtone adjustment, when present.
    pub fn microtone(&self) -> Option<&Microtone> {
        self.microtone.as_ref()
    }

    /// Returns this pitch's normalized pitch class.
    pub fn pitch_class(&self) -> PitchClass {
        PitchClass::from_number(self.ps()).unwrap_or_else(|err| {
            panic!("pitch-space value should always map to pitch class: {err}")
        })
    }

    pub(crate) fn octave_setter(&mut self, octave: Octave) {
        self.octave = octave;
    }

    fn get_all_common_enharmonics(&mut self, alter_limit: FloatType) -> Result<Vec<Pitch>> {
        let mut post = Vec::new();

        let simplified = self.clone().simplify_enharmonic(false)?;
        if simplified.name() != self.name() {
            post.push(simplified);
        }

        let mut higher = self.clone();
        while let Ok(next) = higher.get_higher_enharmonic() {
            if next.accidental.alter.abs() > alter_limit {
                break;
            }
            if post.contains(&next) {
                break;
            }
            post.push(next.clone());
            higher = next;
        }

        let mut lower = self.clone();
        while let Ok(next) = lower.get_lower_enharmonic() {
            if next.accidental.alter.abs() > alter_limit {
                break;
            }
            if post.contains(&next) {
                break;
            }
            post.push(next.clone());
            lower = next;
        }

        Ok(post)
    }

    pub(crate) fn transpose(&self, interval: &Interval) -> Pitch {
        let mut p = interval
            .transpose_pitch_with_options(self, false, Some(4))
            .unwrap_or_else(|_| self.clone());

        if !interval.implicit_diatonic {
            p.spelling_is_inferred = self.spelling_is_inferred;
        }
        if p.spelling_is_inferred {
            let _ = p.simplify_enharmonic_in_place(true);
        }

        p
    }

    /// Returns the pitch-space value for this pitch.
    pub fn ps(&self) -> FloatType {
        self.pitch_space()
    }

    /// Returns the pitch-space value for this pitch.
    pub fn pitch_space(&self) -> FloatType {
        let octave = self.octave.unwrap_or(PITCH_OCTAVE as IntegerType);
        ((octave + 1) * 12) as FloatType + self.step.step_ref() as FloatType + self.alter()
    }

    /// Returns the nearest MIDI note number for this pitch.
    pub fn midi(&self) -> IntegerType {
        self.pitch_space().round() as IntegerType
    }

    /// Returns this pitch's twelve-tone equal-temperament frequency in hertz.
    pub fn frequency_hz(&self) -> FloatType {
        self.frequency_hz_in(TuningSystem::EqualTemperament {
            octave_size: OCTAVE_SIZE,
        })
    }

    /// Returns this pitch's frequency in hertz for a supported tuning system.
    ///
    /// The pitch-space value is used as the tuning-system degree index, so this
    /// is most musically meaningful for twelve-tone systems.
    pub fn frequency_hz_in(&self, tuning_system: TuningSystem) -> FloatType {
        tuning_system.frequency_at(self.pitch_space())
    }

    fn step_setter(&mut self, step_name: StepName) {
        self.step = step_name;
        self.spelling_is_inferred = true;
    }

    fn accidental_setter(&mut self, value: Accidental) {
        self.accidental = value;
    }

    fn microtone_setter(&mut self, mt: Microtone) {
        self.microtone = Some(mt);
    }

    fn pitch_class_setter(&mut self, pc: PitchClassSpecifier) -> Result<()> {
        self.pitch_class_value_setter(PitchClass::new(pc)?.number());
        Ok(())
    }

    fn pitch_class_value_setter(&mut self, pc: FloatType) {
        let (step, accidental, _, _) = convert_ps_to_step(pc);
        self.step = step;
        self.accidental = accidental;
        self.spelling_is_inferred = true;
    }

    fn fundamental_setter(&mut self, f: Pitch) {
        self.fundamental = Some(Arc::new(f));
    }

    /// Returns the fundamental this pitch was built against, when one was set.
    pub fn fundamental(&self) -> Option<&Pitch> {
        self.fundamental.as_deref()
    }

    fn midi_setter(&mut self, m: IntegerType) {
        self.ps_setter(normalize_midi(m) as FloatType);
    }

    fn ps_setter(&mut self, p: FloatType) {
        let (step, accidental, microtone, octave_shift) = convert_ps_to_step(p);
        self.step = step;
        self.accidental = accidental;
        if microtone.alter() == 0.0 {
            self.microtone = None;
        } else {
            self.microtone = Some(microtone);
        }

        let octave = convert_ps_to_oct(p) + octave_shift;
        self.octave = Some(octave);
        self.spelling_is_inferred = true;
    }

    /// Returns a simpler enharmonic spelling of this pitch.
    ///
    /// When `most_common` is true, common spellings such as `E-` are preferred
    /// over less common equivalents such as `D#`, following music21's
    /// `Pitch.simplifyEnharmonic` behavior.
    pub fn simplify_enharmonic(&self, most_common: bool) -> Result<Pitch> {
        let mut pitch = self.clone();
        pitch.simplify_enharmonic_in_place(most_common)?;
        Ok(pitch)
    }

    /// Simplifies this pitch's enharmonic spelling in place.
    pub fn simplify_enharmonic_in_place(&mut self, most_common: bool) -> Result<()> {
        const EXCLUDED_NAMES: [&str; 4] = ["E#", "B#", "C-", "F-"];
        if self.accidental.alter.abs().partial_cmp(&2.0) != Some(Ordering::Less)
            || EXCLUDED_NAMES.contains(&self.name().as_str())
        {
            // by resetting the pitch space value, we get a simpler enharmonic spelling
            let save_octave = self.octave;
            self.ps_setter(self.ps());
            if save_octave.is_none() {
                self.octave_setter(None);
            }
        }

        if most_common {
            match self.name().as_str() {
                "D#" => {
                    self.step_setter(StepName::E);
                    self.accidental_setter(Accidental::new("flat")?);
                }
                "A#" => {
                    self.step_setter(StepName::B);
                    self.accidental_setter(Accidental::new("flat")?);
                }
                "G-" => {
                    self.step_setter(StepName::F);
                    self.accidental_setter(Accidental::new("sharp")?);
                }
                "D-" => {
                    self.step_setter(StepName::C);
                    self.accidental_setter(Accidental::new("sharp")?);
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Returns the next higher enharmonic spelling.
    pub fn get_higher_enharmonic(&self) -> Result<Pitch> {
        self.enharmonic_neighbour(true)
    }

    /// Replaces this pitch with its next higher enharmonic spelling.
    pub fn get_higher_enharmonic_in_place(&mut self) -> Result<()> {
        self.enharmonic_neighbour_in_place(true)
    }

    /// Returns the next lower enharmonic spelling.
    pub fn get_lower_enharmonic(&self) -> Result<Pitch> {
        self.enharmonic_neighbour(false)
    }

    /// Replaces this pitch with its next lower enharmonic spelling.
    pub fn get_lower_enharmonic_in_place(&mut self) -> Result<()> {
        self.enharmonic_neighbour_in_place(false)
    }

    fn enharmonic_neighbour(&self, up: bool) -> Result<Pitch> {
        let interval: &Interval = if up {
            &DIMINISHED_SECOND_UP
        } else {
            &DIMINISHED_SECOND_DOWN
        };

        let octave_stored = self.octave;

        let mut p = interval.transpose_pitch_with_options(self, false, None)?;
        if octave_stored.is_none() {
            p.octave_setter(None);
        }
        Ok(p)
    }

    fn enharmonic_neighbour_in_place(&mut self, up: bool) -> Result<()> {
        *self = self.enharmonic_neighbour(up)?;
        Ok(())
    }

    /// Returns the stored octave.
    ///
    /// Returns `None` when the pitch was created without an explicit octave,
    /// such as `Pitch::from_name("C")`. In calculations, octave-less pitches
    /// use the library default octave.
    pub fn octave(&self) -> Octave {
        self.octave
    }

    pub(crate) fn step(&self) -> StepName {
        self.step
    }

    pub(crate) fn set_ps(&mut self, p: FloatType) {
        self.ps_setter(p);
    }
}

impl Default for Pitch {
    fn default() -> Self {
        Self::from_options(PitchOptions::default())
            .expect("default Pitch construction should never fail")
    }
}

#[derive(Default)]
struct PitchParameters {
    name: Option<String>,
    step: Option<StepName>,
    accidental: Option<Accidental>,
    spelling_is_inferred: bool,
    octave: Octave,
}

impl From<PitchName> for PitchParameters {
    fn from(value: PitchName) -> Self {
        match value {
            PitchName::Name(name) => Self {
                name: Some(name),
                ..Self::default()
            },
            PitchName::Number(number) => {
                let (step, accidental, _, _) = convert_ps_to_step(number);
                let octave = (number >= 12.0).then(|| (number / 12.0) as IntegerType - 1);
                Self {
                    name: None,
                    step: Some(step),
                    accidental: Some(accidental),
                    spelling_is_inferred: true,
                    octave,
                }
            }
        }
    }
}

fn convert_ps_to_step<T: Num + ToPrimitive>(
    ps: T,
) -> (StepName, Accidental, Microtone, IntegerType) {
    let ps = ps.to_f64().unwrap_or(0.0);
    let (pc, alter, micro) = if ps.fract() == 0.0 {
        ((ps as IntegerType).rem_euclid(12), 0.0, 0.0)
    } else {
        let ps = round_to_digits(ps, PITCH_SPACE_SIGNIFICANT_DIGITS);
        let pc_real = ps.rem_euclid(12.0);
        let pc = pc_real.floor() as IntegerType;
        let mut micro = pc_real - pc as FloatType;

        let alter = if round_to_digits(micro, 1) == 0.5 || (0.25..0.75).contains(&micro) {
            micro -= 0.5;
            0.5
        } else if (0.75..1.0).contains(&micro) {
            micro -= 1.0;
            1.0
        } else if micro > 0.0 {
            0.0
        } else {
            micro = 0.0;
            0.0
        };

        (pc, alter, micro)
    };

    let octave_shift = IntegerType::from(pc == 11 && alter == 1.0);
    let (pc_name, accidental_alter) = match pc {
        4 | 11 if alter == 1.0 => ((pc + 1).rem_euclid(12), 0.0),
        1 | 6 | 8 if alter >= 1.0 => (pc + 1, alter - 1.0),
        1 | 6 | 8 => (pc - 1, 1.0 + alter),
        3 | 10 if alter <= -1.0 => (pc - 1, 1.0 + alter),
        3 | 10 => (pc + 1, -1.0 + alter),
        _ => (pc, alter),
    };

    let step = StepName::ref_to_step(pc_name.rem_euclid(12))
        .unwrap_or_else(|err| panic!("pitch class should map to a step: {err}"));
    let accidental = Accidental::new(accidental_alter)
        .unwrap_or_else(|err| panic!("accidental conversion should not fail: {err}"));
    let microtone = Microtone::from_cents(micro * 100.0, 1);

    (step, accidental, microtone, octave_shift)
}

fn round_to_digits(value: FloatType, digits: UnsignedIntegerType) -> FloatType {
    let factor = (10 as FloatType).powi(digits as IntegerType);
    (value * factor).round() / factor
}

fn normalize_midi(midi: IntegerType) -> IntegerType {
    if midi > 127 {
        let mut value = (12 * 9) + midi.rem_euclid(12);
        if value < (127 - 12) {
            value += 12;
        }
        value
    } else if midi < 0 {
        midi.rem_euclid(12)
    } else {
        midi
    }
}

type CriterionFunction = fn(&[Pitch]) -> Result<FloatType>;

pub(crate) fn simplify_multiple_enharmonics(
    pitches: &[Pitch],
    criterion: Option<CriterionFunction>,
    key_context: Option<KeySignature>,
) -> Result<Vec<Pitch>> {
    let mut old_pitches: Vec<Pitch> = pitches.to_vec();
    if old_pitches.is_empty() {
        return Ok(Vec::new());
    }

    let criterion: CriterionFunction = criterion.unwrap_or(default_dissonance_score);

    let remove_first: bool = match key_context {
        Some(key) => {
            old_pitches.insert(0, key.as_key("major").tonic());
            true
        }
        None => false,
    };

    let mut simplified_pitches = match old_pitches.len() < 5 {
        true => brute_force_enharmonics_search(&mut old_pitches, criterion)?,
        false => greedy_enharmonics_search(&mut old_pitches, criterion)?,
    };

    for (new_p, old_p) in simplified_pitches.iter_mut().zip(old_pitches) {
        new_p.spelling_is_inferred = old_p.spelling_is_inferred;
    }

    if remove_first {
        simplified_pitches.remove(0);
    }

    Ok(simplified_pitches)
}

fn brute_force_enharmonics_search(
    old_pitches: &mut [Pitch],
    score_func: CriterionFunction,
) -> Result<Vec<Pitch>> {
    let all_possible_pitches: Result<Vec<Vec<Pitch>>> = old_pitches[1..]
        .iter_mut()
        .map(|p| -> Result<Vec<Pitch>> {
            let mut enharmonics = p.get_all_common_enharmonics(2 as FloatType)?;
            enharmonics.insert(0, p.clone());
            Ok(enharmonics)
        })
        .collect();

    let all_pitch_combinations = all_possible_pitches?.into_iter().multi_cartesian_product();

    let mut min_score = FloatType::MAX;
    let mut best_combination: Vec<Pitch> = Vec::new();

    for combination in all_pitch_combinations {
        let mut pitches: Vec<Pitch> = old_pitches[..1].to_vec();
        pitches.extend(combination);
        let score = score_func(&pitches)?;
        if score < min_score {
            min_score = score;
            best_combination = pitches;
        }
    }

    Ok(best_combination)
}

fn greedy_enharmonics_search(
    old_pitches: &mut [Pitch],
    score_func: CriterionFunction,
) -> Result<Vec<Pitch>> {
    let mut new_pitches = vec![];

    if let Some(first) = old_pitches.first() {
        new_pitches.push(first.clone());
    } else {
        return Err(Error::Pitch(
            "can't perform greedy enharmonics search on empty pitches".into(),
        ));
    }

    for old_pitch in old_pitches.iter_mut().skip(1) {
        let mut candidates = vec![old_pitch.clone()];
        candidates.extend(old_pitch.get_all_common_enharmonics(2 as FloatType)?);

        let mut best_candidate = None;
        let mut best_score: Option<OrderedFloat<FloatType>> = None;
        for candidate in candidates.iter() {
            let mut candidate_list = new_pitches.clone();
            candidate_list.push(candidate.clone());
            let score = score_func(&candidate_list)?;
            let score = OrderedFloat(score);
            if best_score.is_none() || score < best_score.unwrap() {
                best_score = Some(score);
                best_candidate = Some(candidate);
            }
        }
        let best_candidate = best_candidate
            .ok_or_else(|| Error::Pitch("candidates list is unexpectedly empty".to_string()))?;
        new_pitches.push(best_candidate.clone());
    }
    Ok(new_pitches)
}

fn default_dissonance_score(pitches: &[Pitch]) -> Result<FloatType> {
    dissonance_score(pitches, true, true, true)
}

fn dissonance_score(
    pitches: &[Pitch],
    small_pythagorean_ratio: bool,
    accidental_penalty: bool,
    triad_award: bool,
) -> Result<FloatType> {
    let mut score_accidentals: FloatType = 0.0;
    let mut score_ratio: FloatType = 0.0;
    let mut score_triad: FloatType = 0.0;

    if pitches.is_empty() {
        return Ok(0.0);
    }

    if accidental_penalty {
        let accidentals = pitches
            .iter()
            .map(|p| p.alter().abs())
            .collect::<Vec<FloatType>>();
        score_accidentals = accidentals
            .iter()
            .map(|a| if *a > 1.0 { *a } else { 0.0 })
            .sum::<FloatType>()
            / pitches.len() as FloatType;
    }

    let mut intervals: Vec<Interval> = vec![];

    if small_pythagorean_ratio | triad_award {
        for (index, p1) in pitches.iter().enumerate() {
            for p2 in pitches.iter().skip(index + 1) {
                let mut p2 = (*p2).clone();
                p2.octave_setter(None);
                let Ok(interval) = Interval::between(
                    PitchOrNote::Pitch(p1.clone()),
                    PitchOrNote::Pitch(p2.clone()),
                ) else {
                    return Ok(FloatType::INFINITY);
                };
                intervals.push(interval);
            }
        }

        if small_pythagorean_ratio {
            for interval in intervals.iter() {
                score_ratio += pythagorean_denominator_log(interval)? * 0.075_853_268_88
            }
            score_ratio /= pitches.len() as FloatType;
        }

        if triad_award {
            intervals.into_iter().for_each(|interval| {
                let simple_directed = interval.generic().simple_directed();
                let interval_semitones = interval.chromatic.semitones % 12;
                if (simple_directed == 3 && (interval_semitones == 3 || interval_semitones == 4))
                    || (simple_directed == 6
                        && (interval_semitones == 8 || interval_semitones == 9))
                {
                    score_triad -= 1.0;
                }
            });
            score_triad /= pitches.len() as FloatType;
        }
    }

    Ok((score_accidentals + score_ratio + score_triad)
        / (small_pythagorean_ratio as IntegerType
            + accidental_penalty as IntegerType
            + triad_award as IntegerType) as FloatType)
}

fn pythagorean_denominator_log(interval: &Interval) -> Result<FloatType> {
    let start_pitch = Pitch::from_name("C1")?;
    let end_pitch = interval.transpose_pitch_with_options(&start_pitch, false, Some(4))?;

    let natural_fifths = match end_pitch.step() {
        StepName::C => 0,
        StepName::D => 2,
        StepName::E => 4,
        StepName::F => -1,
        StepName::G => 1,
        StepName::A => 3,
        StepName::B => 5,
    };
    let fifth_count = natural_fifths + (end_pitch.alter().round() as IntegerType * 7);
    let found_pitch_space = start_pitch.ps() + (7 * fifth_count) as FloatType;
    let octave_adjust = ((end_pitch.ps() - found_pitch_space) / 12.0).round() as IntegerType;

    let mut denominator_twos = if fifth_count > 0 { fifth_count } else { 0 };
    let denominator_threes = if fifth_count < 0 { -fifth_count } else { 0 };
    denominator_twos = (denominator_twos - octave_adjust).max(0);

    Ok(denominator_twos as FloatType * (2.0 as FloatType).ln()
        + denominator_threes as FloatType * (3.0 as FloatType).ln())
}

fn convert_harmonic_to_cents(harmonic_shift: IntegerType) -> IntegerType {
    let mut value = harmonic_shift as FloatType;
    if value < 0.0 {
        value = 1.0 / value.abs();
    }
    (1200.0 * value.log2()).round() as IntegerType
}

#[cfg(test)]
mod tests {
    use crate::defaults::IntegerType;
    use crate::interval::Interval;
    use crate::tuningsystem::TuningSystem;

    use super::{
        Accidental, Microtone, Pitch, convert_harmonic_to_cents, simplify_multiple_enharmonics,
    };

    #[test]
    fn fundamental_round_trips_through_the_builder() {
        let pitch = Pitch::builder()
            .name("E4")
            .fundamental(Pitch::from_name("C2").unwrap())
            .build()
            .unwrap();
        assert_eq!(
            pitch.fundamental().map(Pitch::name_with_octave),
            Some("C2".to_string())
        );
        assert_eq!(Pitch::from_name("E4").unwrap().fundamental(), None);
    }

    #[test]
    fn simplify_multiple_enharmonics_test() {
        let more_than_five = vec![
            Pitch::from_number(0.0).unwrap(),
            Pitch::from_number(1.0).unwrap(),
            Pitch::from_number(2.0).unwrap(),
            Pitch::from_number(3.0).unwrap(),
            Pitch::from_number(4.0).unwrap(),
            Pitch::from_number(5.0).unwrap(),
            Pitch::from_number(12.0).unwrap(),
            Pitch::from_number(13.0).unwrap(),
        ];

        let _x = simplify_multiple_enharmonics(&more_than_five, None, None);
        let _less_than_five = [
            Pitch::from_number(0.0),
            Pitch::from_number(1.0),
            Pitch::from_number(2.0),
            Pitch::from_number(12.0),
            Pitch::from_number(13.0),
        ];
    }

    #[test]
    fn test_convert_harmonic_to_cents_values() {
        assert_eq!(convert_harmonic_to_cents(8), 3600);
        assert_eq!(convert_harmonic_to_cents(5), 2786);
        assert_eq!(convert_harmonic_to_cents(-2), -1200);
    }

    #[test]
    fn test_pitch_transpose_interval() {
        let c4 = Pitch::from_name("C4").unwrap();
        let m3 = Interval::from_name("m3").unwrap();
        let out = c4.transpose(&m3);
        assert_eq!(out.name_with_octave(), "E-4");
    }

    #[test]
    fn test_pitch_frequency_helpers() {
        let a4 = Pitch::from_name("A4").unwrap();
        assert!((a4.frequency_hz() - 440.0).abs() < 0.0001);

        let e4 = Pitch::from_name("E4").unwrap();
        assert!((e4.frequency_hz_in(TuningSystem::FiveLimit) - 327.032).abs() < 0.001);
        assert!(e4.frequency_hz_in(TuningSystem::FiveLimit) < e4.frequency_hz());
    }

    #[test]
    fn pitch_exposes_accidental_object() {
        let custom_accidental = Accidental::new("half-flat").unwrap();
        let pitch = Pitch::builder()
            .step('D')
            .accidental(custom_accidental.clone())
            .octave(4)
            .build()
            .unwrap();

        assert_eq!(pitch.name_with_octave(), "D`4");
        assert_eq!(pitch.accidental(), &custom_accidental);
        assert_eq!(pitch.accidental().name(), "half-flat");
        assert_eq!(pitch.accidental().alter(), -0.5);
    }

    #[test]
    fn pitch_exposes_microtone_object() {
        let microtone = Microtone::new(-25.0).unwrap();
        let pitch = Pitch::builder()
            .name("G#4")
            .microtone(microtone.clone())
            .build()
            .unwrap();

        assert_eq!(pitch.microtone(), Some(&microtone));
        assert_eq!(pitch.microtone().unwrap().to_string(), "(-25c)");
        assert_eq!(pitch.alter(), 0.75);
    }

    #[test]
    fn pitch_supports_rust_conversion_traits() {
        let parsed: Pitch = "C#4".parse().unwrap();
        assert_eq!(parsed.to_string(), "C#4");

        let midi = Pitch::try_from(60 as IntegerType).unwrap();
        assert_eq!(midi.name_with_octave(), "C4");
        assert_eq!(midi.midi(), 60);

        let pitch_space = Pitch::try_from(61.5).unwrap();
        assert_eq!(pitch_space.pitch_space(), 61.5);
        assert_eq!(pitch_space.midi(), 62);

        let built = Pitch::builder().pitch_space(60.0).build().unwrap();
        assert_eq!(built.name_with_octave(), "C4");
    }

    #[test]
    fn pitch_exposes_enharmonic_helpers() {
        let c_sharp = Pitch::from_name("C#3").unwrap();
        let out = c_sharp.get_higher_enharmonic().unwrap();
        assert_eq!(out.name_with_octave(), "D-3");

        let mut d_flat = out;
        d_flat.get_lower_enharmonic_in_place().unwrap();
        assert_eq!(d_flat.name_with_octave(), "C#3");

        let d_sharp = Pitch::from_name("D#4").unwrap();
        assert_eq!(
            d_sharp
                .simplify_enharmonic(true)
                .unwrap()
                .name_with_octave(),
            "E-4"
        );
    }
}
