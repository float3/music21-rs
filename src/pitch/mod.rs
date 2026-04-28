pub(crate) mod accidental;
pub(crate) mod microtone;
pub(crate) mod pitchclass;
pub(crate) mod pitchclassstring;

use crate::defaults::FloatType;
use crate::defaults::IntegerType;
use crate::defaults::Octave;
use crate::defaults::PITCH_OCTAVE;
use crate::defaults::PITCH_SPACE_SIGNIFICANT_DIGITS;
use crate::defaults::PITCH_STEP;
use crate::defaults::UnsignedIntegerType;
use crate::exception::Exception;
use crate::exception::ExceptionResult;
use crate::interval::Interval;
use crate::interval::IntervalArgument;
use crate::interval::PitchOrNote;
use crate::interval::intervalstring::IntervalString;
use crate::key::keysignature::KeySignature;
use crate::note::Note;
use crate::prebase::ProtoM21Object;
use crate::prebase::ProtoM21ObjectTrait;
use crate::stepname::StepName;

use accidental::Accidental;
use accidental::IntoAccidental;
use microtone::IntoCentShift;
use microtone::Microtone;
use pitchclass::convert_ps_to_oct;

use itertools::Itertools;
use num::Num;
use num_traits::ToPrimitive;
use ordered_float::OrderedFloat;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

// TODO: rework this, don't use a HashMap for two possible inputs, either figure
// out what the -d2 and d2 intervals are beforehand or caculate them and store
// them each in a static
static TRANSPOSITIONAL_INTERVALS: LazyLock<Mutex<HashMap<IntervalString, Interval>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PitchName {
    Name(String),
    Number(f64),
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

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PitchAccidental {
    Name(String),
    Alter(f64),
}

impl From<&str> for PitchAccidental {
    fn from(value: &str) -> Self {
        Self::Name(value.to_string())
    }
}

impl From<String> for PitchAccidental {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}

impl From<i8> for PitchAccidental {
    fn from(value: i8) -> Self {
        Self::Alter(value as FloatType)
    }
}

impl From<IntegerType> for PitchAccidental {
    fn from(value: IntegerType) -> Self {
        Self::Alter(value as FloatType)
    }
}

impl From<FloatType> for PitchAccidental {
    fn from(value: FloatType) -> Self {
        Self::Alter(value)
    }
}

impl Display for PitchAccidental {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => write!(f, "{name}"),
            Self::Alter(alter) => write!(f, "{alter}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PitchMicrotone {
    Cents(f64),
    Text(String),
}

impl From<&str> for PitchMicrotone {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for PitchMicrotone {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<IntegerType> for PitchMicrotone {
    fn from(value: IntegerType) -> Self {
        Self::Cents(value as FloatType)
    }
}

impl From<FloatType> for PitchMicrotone {
    fn from(value: FloatType) -> Self {
        Self::Cents(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PitchClassSpecifier {
    Number(f64),
    String(String),
}

impl PitchClassSpecifier {
    fn to_number(&self) -> ExceptionResult<f64> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::String(value) => {
                let value = value.trim();
                match value {
                    "a" | "A" | "t" | "T" => Ok(10.0),
                    "b" | "B" | "e" | "E" => Ok(11.0),
                    _ => value
                        .parse::<IntegerType>()
                        .map(|value| value as FloatType)
                        .map_err(|err| {
                            Exception::PitchClass(format!(
                                "cannot parse pitch class {value:?}: {err}"
                            ))
                        }),
                }
            }
        }
    }
}

impl From<IntegerType> for PitchClassSpecifier {
    fn from(value: IntegerType) -> Self {
        Self::Number(value as FloatType)
    }
}

impl From<u8> for PitchClassSpecifier {
    fn from(value: u8) -> Self {
        Self::Number(value as FloatType)
    }
}

impl From<FloatType> for PitchClassSpecifier {
    fn from(value: FloatType) -> Self {
        Self::Number(value)
    }
}

impl From<char> for PitchClassSpecifier {
    fn from(value: char) -> Self {
        Self::String(value.to_string())
    }
}

impl From<&str> for PitchClassSpecifier {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for PitchClassSpecifier {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PitchOptions {
    pub name: Option<PitchName>,
    pub step: Option<char>,
    pub octave: Option<i32>,
    pub accidental: Option<PitchAccidental>,
    pub microtone: Option<PitchMicrotone>,
    pub pitch_class: Option<PitchClassSpecifier>,
    pub midi: Option<i32>,
    pub ps: Option<f64>,
    pub fundamental: Option<Pitch>,
}

impl PitchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<PitchName>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn step(mut self, step: char) -> Self {
        self.step = Some(step);
        self
    }

    pub fn octave(mut self, octave: i32) -> Self {
        self.octave = Some(octave);
        self
    }

    pub fn accidental(mut self, accidental: impl Into<PitchAccidental>) -> Self {
        self.accidental = Some(accidental.into());
        self
    }

    pub fn microtone(mut self, microtone: impl Into<PitchMicrotone>) -> Self {
        self.microtone = Some(microtone.into());
        self
    }

    pub fn pitch_class(mut self, pitch_class: impl Into<PitchClassSpecifier>) -> Self {
        self.pitch_class = Some(pitch_class.into());
        self
    }

    pub fn midi(mut self, midi: i32) -> Self {
        self.midi = Some(midi);
        self
    }

    pub fn ps(mut self, ps: f64) -> Self {
        self.ps = Some(ps);
        self
    }

    pub fn fundamental(mut self, fundamental: Pitch) -> Self {
        self.fundamental = Some(fundamental);
        self
    }

    pub fn build(self) -> ExceptionResult<Pitch> {
        Pitch::from_options(self)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pitch {
    proto: ProtoM21Object,
    _step: StepName,
    _octave: Octave,
    _overriden_freq440: Option<FloatType>,
    _accidental: Accidental,
    _microtone: Option<Microtone>,
    #[cfg_attr(feature = "serde", serde(skip))]
    _client: Option<Arc<Note>>,
    spelling_is_infered: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    fundamental: Option<Arc<Pitch>>,
}

impl PartialEq for Pitch {
    fn eq(&self, other: &Self) -> bool {
        self._step == other._step
            && self._octave == other._octave
            && self._accidental == other._accidental
            && self._microtone == other._microtone
    }
}

impl Pitch {
    pub fn from_options(options: PitchOptions) -> ExceptionResult<Self> {
        let step = options.step.map(StepName::try_from).transpose()?;

        Self::new(
            options.name,
            step,
            options.octave,
            options.accidental,
            options.microtone,
            options.pitch_class,
            options.midi,
            options.ps,
            options.fundamental,
        )
    }

    pub fn builder() -> PitchOptions {
        PitchOptions::new()
    }

    pub fn from_name(name: impl Into<String>) -> ExceptionResult<Self> {
        Self::new(
            Some(name.into()),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn from_number(number: f64) -> ExceptionResult<Self> {
        Self::new(
            Some(PitchName::Number(number)),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn from_step(step: char) -> ExceptionResult<Self> {
        Self::new(
            Option::<String>::None,
            Some(StepName::try_from(step)?),
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn from_name_and_octave(name: impl Into<String>, octave: i32) -> ExceptionResult<Self> {
        PitchOptions::new().name(name.into()).octave(octave).build()
    }

    pub fn from_pitch_class(pitch_class: impl Into<PitchClassSpecifier>) -> ExceptionResult<Self> {
        PitchOptions::new().pitch_class(pitch_class).build()
    }

    pub fn from_midi(midi: i32) -> ExceptionResult<Self> {
        Self::new(
            Option::<String>::None,
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            Some(midi),
            None,
            None,
        )
    }

    pub fn from_pitch_space(ps: f64) -> ExceptionResult<Self> {
        Self::new(
            Option::<String>::None,
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            Some(ps),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new<T, U, V>(
        name: Option<T>,
        step: Option<StepName>,
        octave: Octave,
        accidental: Option<U>,
        microtone: Option<V>,
        pitch_class: Option<PitchClassSpecifier>,
        midi: Option<IntegerType>,
        ps: Option<FloatType>,
        fundamental: Option<Pitch>,
    ) -> ExceptionResult<Self>
    where
        T: IntoPitchName,
        U: IntoAccidental,
        V: IntoCentShift,
    {
        let has_explicit_step = step.is_some();
        let has_explicit_octave = octave.is_some();
        let has_explicit_accidental = accidental.is_some();
        let has_explicit_microtone = microtone.is_some();

        // --- Step 1: Parse parameters ---
        let mut self_name = None;
        let mut self_step = PITCH_STEP;
        let mut self_accidental: Option<Accidental> = None;
        let mut self_microtone: Option<Microtone> = None;
        let mut self_spelling_is_inferred = false;
        let mut self_octave = None;
        let self_pitch_class = pitch_class;
        let self_fundamental = fundamental;
        let self_midi = midi;
        let self_ps = ps;

        if let Some(name) = name {
            let x = name.into_name();
            self_name = x.name;
            if let Some(step) = x.step {
                self_step = step;
            }
            if let Some(accidental) = x.accidental {
                self_accidental = Some(accidental);
            }
            if let Some(inferred) = x.spelling_is_inferred {
                self_spelling_is_inferred = inferred;
            }
            self_octave = x.octave;
        } else if let Some(s) = step {
            self_step = s;
        }

        if let Some(oct) = octave {
            self_octave = Some(oct);
        }

        if let Some(acc) = accidental {
            self_accidental = Some(if acc.is_accidental() {
                acc.accidental()
            } else {
                Accidental::new(acc)?
            });
        } else if self_accidental.is_none() {
            self_accidental = Some(Accidental::new("natural")?);
        }

        if let Some(mt) = microtone {
            self_microtone = Some(if mt.is_microtone() {
                mt.microtone()
            } else {
                mt.into_microtone()?
            });
        }

        // --- Step 2: Construct Pitch with initial values ---
        let mut pitch = Pitch {
            proto: ProtoM21Object::new(),
            _step: self_step,
            _overriden_freq440: None,
            _accidental: self_accidental.clone().unwrap(),
            _microtone: self_microtone,
            _octave: self_octave,
            _client: None,
            spelling_is_infered: self_spelling_is_inferred,
            fundamental: None,
        };

        // --- Step 3: Call setters in proper order ---
        if let Some(ref n) = self_name {
            pitch.name_setter(n)?;
        }

        if has_explicit_step || self_name.is_none() {
            pitch.step_setter(self_step);
        }

        if has_explicit_octave || self_name.is_none() {
            pitch.octave_setter(self_octave);
        }

        if has_explicit_accidental || self_name.is_none() {
            pitch.accidental_setter(pitch._accidental.clone());
        }
        if has_explicit_microtone {
            let Some(mt) = pitch._microtone.clone() else {
                return Err(Exception::Pitch(
                    "microtone was expected but missing".to_string(),
                ));
            };
            pitch.microtone_setter(mt.clone());
        }
        if let Some(pc) = self_pitch_class {
            pitch.pitch_class_setter(pc)?;
        }
        if let Some(f) = self_fundamental {
            pitch.fundamental_setter(f);
        }
        if let Some(m) = self_midi {
            pitch.midi_setter(m);
        }
        if let Some(p) = self_ps {
            pitch.ps_setter(p);
        }

        Ok(pitch)
    }

    pub fn name_with_octave(&self) -> String {
        match self._octave {
            Some(octave) => format!("{}{}", self.name(), octave),
            None => self.name(),
        }
    }

    pub fn name(&self) -> String {
        format!("{:?}{}", self._step, self._accidental.modifier())
    }

    fn name_setter(&mut self, usr_str: &str) -> ExceptionResult<()> {
        let usr_str = usr_str.trim();

        let digit_index = usr_str
            .char_indices()
            .find(|&(_, c)| c.is_ascii_digit())
            .map(|(i, _)| i);

        let (pitch_part, octave_part) = if let Some(i) = digit_index {
            if i == 0 {
                return Err(Exception::Pitch(format!(
                    "Cannot have octave given before pitch name in {usr_str:?}"
                )));
            }
            (&usr_str[..i], &usr_str[i..])
        } else {
            (usr_str, "")
        };

        // Process the pitch part.
        let mut pitch_chars = pitch_part.chars();
        let step = pitch_chars.next().ok_or(Exception::Pitch(format!(
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
                .map_err(|_| Exception::Pitch(format!("Cannot parse {octave_part:?} to octave")))?;
            self.octave_setter(Some(octave));
        }

        Ok(())
    }

    pub fn alter(&self) -> FloatType {
        let mut post = 0.0;

        post += self._accidental._alter;

        if let Some(microtone) = &self._microtone {
            post += microtone.alter();
        }

        post
    }

    pub(crate) fn octave_setter(&mut self, octave: Octave) {
        self._octave = octave;
        self.inform_client()
    }

    fn get_all_common_enharmonics(
        &mut self,
        alter_limit: FloatType,
    ) -> ExceptionResult<Vec<Pitch>> {
        let mut post = Vec::new();

        let simplified = self.clone().simplify_enharmonic(false)?;
        if simplified.name() != self.name() {
            post.push(simplified);
        }

        let mut higher = self.clone();
        while let Ok(next) = higher.get_higher_enharmonic() {
            if next._accidental._alter.abs() > alter_limit {
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
            if next._accidental._alter.abs() > alter_limit {
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

    fn inform_client(&self) {
        if let Some(ref client) = self._client {
            client.pitch_changed();
        }
    }

    pub(crate) fn transpose(&self, clone: Interval) -> Pitch {
        let mut p = clone
            .clone()
            .transpose_pitch(self, false, Some(4))
            .unwrap_or_else(|_| self.clone());

        if !clone.implicit_diatonic {
            p.spelling_is_infered = self.spelling_is_infered;
        }
        if p.spelling_is_infered {
            let _ = p.simplify_enharmonic_in_place(true);
        }

        p
    }

    pub fn ps(&self) -> FloatType {
        let octave = self._octave.unwrap_or(PITCH_OCTAVE as IntegerType);
        ((octave + 1) * 12) as FloatType + self._step.step_ref() as FloatType + self.alter()
    }

    fn step_setter(&mut self, step_name: StepName) {
        self._step = step_name;
        self.spelling_is_infered = true;
        self.inform_client();
    }

    fn accidental_setter(&mut self, value: Accidental) {
        self._accidental = value;
        self.inform_client();
    }

    fn microtone_setter(&mut self, mt: Microtone) {
        self._microtone = Some(mt);
        self.inform_client();
    }

    fn pitch_class_setter(&mut self, pc: PitchClassSpecifier) -> ExceptionResult<()> {
        self.pitch_class_value_setter(pc.to_number()?);
        Ok(())
    }

    fn pitch_class_value_setter(&mut self, pc: FloatType) {
        let (step, accidental, _microtone, _harmonic_shift) = convert_ps_to_step(pc);
        self._step = step;
        self._accidental = accidental;
        self.spelling_is_infered = true;
        self.inform_client();
    }

    fn fundamental_setter(&mut self, f: Pitch) {
        self.fundamental = Some(Arc::new(f));
        self.inform_client();
    }

    fn midi_setter(&mut self, m: IntegerType) {
        self.ps_setter(normalize_midi(m) as FloatType);
    }

    fn ps_setter(&mut self, p: FloatType) {
        let (step, accidental, microtone, octave_shift) = convert_ps_to_step(p);
        self._step = step;
        self._accidental = accidental;
        if microtone.alter() == 0.0 {
            self._microtone = None;
        } else {
            self._microtone = Some(microtone);
        }

        let octave = convert_ps_to_oct(p) + octave_shift;
        self._octave = Some(octave);
        self.spelling_is_infered = true;
        self.inform_client();
    }

    fn simplify_enharmonic(&mut self, most_common: bool) -> ExceptionResult<Pitch> {
        const EXCLUDED_NAMES: [&str; 4] = ["E#", "B#", "C-", "F-"];
        if self._accidental._alter.abs().partial_cmp(&2.0) != Some(Ordering::Less)
            || EXCLUDED_NAMES.contains(&self.name().as_str())
        {
            // by resetting the pitch space value, we get a simpler enharmonic spelling
            let save_octave = self._octave;
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

        Ok(self.clone())
    }

    fn simplify_enharmonic_in_place(&mut self, most_common: bool) -> ExceptionResult<()> {
        *self = self.simplify_enharmonic(most_common)?;
        Ok(())
    }

    fn get_higher_enharmonic(&self) -> ExceptionResult<Pitch> {
        self._get_enharmonic_helper(true)
    }

    fn get_higher_enharmonic_in_place(&mut self) -> ExceptionResult<()> {
        self._get_enharmonic_helper_in_place(true)
    }

    fn get_lower_enharmonic(&self) -> ExceptionResult<Pitch> {
        self._get_enharmonic_helper(false)
    }

    fn get_lower_enharmonic_in_place(&mut self) -> ExceptionResult<()> {
        self._get_enharmonic_helper_in_place(false)
    }

    fn _get_enharmonic_helper(&self, up: bool) -> ExceptionResult<Pitch> {
        let interval_string = match up {
            true => IntervalString::Up,
            false => IntervalString::Down,
        };

        let mut dict = match TRANSPOSITIONAL_INTERVALS.lock() {
            Ok(dict) => dict,
            Err(err) => err.into_inner(),
        };

        let interval: Interval = match dict.get(&interval_string) {
            None => {
                let interval =
                    Interval::new(IntervalArgument::Str(interval_string.clone().string()))?;
                dict.insert(interval_string.clone(), interval.clone());
                interval
            }
            Some(interval) => interval.to_owned(),
        };

        let octave_stored = self._octave;

        let mut p = interval.transpose_pitch(self, false, None)?;
        if octave_stored.is_none() {
            p.octave_setter(None);
        }
        Ok(p)
    }

    fn _get_enharmonic_helper_in_place(&mut self, up: bool) -> ExceptionResult<()> {
        *self = self._get_enharmonic_helper(up)?;
        Ok(())
    }

    /// Returns the stored octave.
    ///
    /// Returns `None` when the pitch was created without an explicit octave,
    /// such as `Pitch::from_name("C")`. In calculations, octave-less pitches
    /// use the library default octave.
    pub fn octave(&self) -> Octave {
        self._octave
    }

    pub(crate) fn step(&self) -> StepName {
        self._step
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

impl ProtoM21ObjectTrait for Pitch {}

pub(crate) struct PitchParameteres {
    pub(crate) name: Option<String>,
    pub(crate) step: Option<StepName>,
    pub(crate) accidental: Option<Accidental>,
    pub(crate) spelling_is_inferred: Option<bool>,
    pub(crate) octave: Octave,
}

pub(crate) trait IntoPitchName {
    fn into_name(self) -> PitchParameteres;
}

impl IntoPitchName for Pitch {
    fn into_name(self) -> PitchParameteres {
        self.name_with_octave().into_name()
    }
}

impl IntoPitchName for PitchName {
    fn into_name(self) -> PitchParameteres {
        match self {
            PitchName::Name(name) => name.into_name(),
            PitchName::Number(number) => number.into_name(),
        }
    }
}

impl IntoPitchName for IntegerType {
    fn into_name(self) -> PitchParameteres {
        let (step_name, accidental, _, _) = convert_ps_to_step(self);

        let octave = if self >= 12 {
            Some(self / 12 - 1)
        } else {
            None
        };

        PitchParameteres {
            name: None,
            step: Some(step_name),
            accidental: Some(accidental),
            spelling_is_inferred: Some(true),
            octave,
        }
    }
}

impl IntoPitchName for FloatType {
    fn into_name(self) -> PitchParameteres {
        let (step_name, accidental, _, _) = convert_ps_to_step(self);

        let octave = if self >= 12.0 {
            Some((self / 12.0) as IntegerType - 1)
        } else {
            None
        };

        PitchParameteres {
            name: None,
            step: Some(step_name),
            accidental: Some(accidental),
            spelling_is_inferred: Some(true),
            octave,
        }
    }
}

impl IntoPitchName for String {
    fn into_name(self) -> PitchParameteres {
        PitchParameteres {
            name: Some(self),
            step: None,
            accidental: None,
            spelling_is_inferred: None,
            octave: None,
        }
    }
}

impl IntoAccidental for PitchAccidental {
    fn accidental_args(self, allow_non_standard_values: bool) -> Option<(String, FloatType)> {
        match self {
            PitchAccidental::Name(name) => name.accidental_args(allow_non_standard_values),
            PitchAccidental::Alter(alter) => alter.accidental_args(allow_non_standard_values),
        }
    }

    fn is_accidental(&self) -> bool {
        false
    }

    fn into_accidental(self) -> ExceptionResult<Accidental> {
        Accidental::new(self)
    }

    fn accidental(self) -> Accidental {
        panic!("call into_accidental instead")
    }
}

impl IntoCentShift for PitchMicrotone {
    fn into_cent_shift(self) -> FloatType {
        match self {
            PitchMicrotone::Cents(cents) => cents,
            PitchMicrotone::Text(text) => text.into_cent_shift(),
        }
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::new(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("call into_microtone instead")
    }
}

impl IntoPitchName for &str {
    fn into_name(self) -> PitchParameteres {
        PitchParameteres {
            name: Some(self.to_string()),
            step: None,
            accidental: None,
            spelling_is_inferred: None,
            octave: None,
        }
    }
}

fn convert_ps_to_step<T: Num + ToPrimitive>(
    ps: T,
) -> (StepName, Accidental, Microtone, IntegerType) {
    const NATURAL_PCS: [IntegerType; 7] = [0, 2, 4, 5, 7, 9, 11];

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

    let mut octave_shift = 0;
    let (pc_name, accidental_alter) = if alter == 1.0 && matches!(pc, 4 | 11) {
        if pc == 11 {
            octave_shift = 1;
        }
        ((pc + 1).rem_euclid(12), 0.0)
    } else if NATURAL_PCS.contains(&pc) {
        (pc, alter)
    } else if [0, 5, 7].contains(&(pc - 1)) && alter >= 1.0 {
        (pc + 1, alter - 1.0)
    } else if [0, 5, 7].contains(&(pc - 1)) || ([11, 4].contains(&(pc + 1)) && alter <= -1.0) {
        (pc - 1, 1.0 + alter)
    } else if [11, 4].contains(&(pc + 1)) {
        (pc + 1, -1.0 + alter)
    } else {
        panic!("cannot match condition for pitch class: {pc}");
    };

    let step = StepName::ref_to_step(pc_name.rem_euclid(12))
        .unwrap_or_else(|err| panic!("pitch class should map to a step: {err}"));
    let accidental = Accidental::new(accidental_alter)
        .unwrap_or_else(|err| panic!("accidental conversion should not fail: {err}"));
    let microtone = Microtone::new(Some(micro * 100.0), None)
        .unwrap_or_else(|err| panic!("microtone conversion should not fail: {err}"));

    (step, accidental, microtone, octave_shift)
}

fn round_to_digits(value: FloatType, digits: UnsignedIntegerType) -> FloatType {
    let factor = (10 as FloatType).powi(digits as i32);
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

type CriterionFunction = fn(&[Pitch]) -> ExceptionResult<FloatType>;

pub(crate) fn simplify_multiple_enharmonics(
    pitches: &[Pitch],
    criterion: Option<CriterionFunction>,
    key_context: Option<KeySignature>,
) -> ExceptionResult<Vec<Pitch>> {
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
        new_p.spelling_is_infered = old_p.spelling_is_infered;
    }

    if remove_first {
        simplified_pitches.remove(0);
    }

    Ok(simplified_pitches)
}

fn brute_force_enharmonics_search(
    old_pitches: &mut [Pitch],
    score_func: CriterionFunction,
) -> ExceptionResult<Vec<Pitch>> {
    let all_possible_pitches: ExceptionResult<Vec<Vec<Pitch>>> = old_pitches[1..]
        .iter_mut()
        .map(|p| -> ExceptionResult<Vec<Pitch>> {
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
) -> ExceptionResult<Vec<Pitch>> {
    let mut new_pitches = vec![];

    if let Some(first) = old_pitches.first() {
        new_pitches.push(first.clone());
    } else {
        return Err(Exception::Pitch(
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
            .ok_or_else(|| Exception::Pitch("candidates list is unexpectedly empty".to_string()))?;
        new_pitches.push(best_candidate.clone());
    }
    Ok(new_pitches)
}

fn default_dissonance_score(pitches: &[Pitch]) -> ExceptionResult<FloatType> {
    dissonance_score(pitches, true, true, true)
}

fn dissonance_score(
    pitches: &[Pitch],
    small_pythagorean_ratio: bool,
    accidental_penalty: bool,
    triad_award: bool,
) -> ExceptionResult<FloatType> {
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

fn pythagorean_denominator_log(interval: &Interval) -> ExceptionResult<FloatType> {
    let start_pitch = Pitch::new(
        Some("C1".to_string()),
        None,
        None,
        Option::<i8>::None,
        Option::<IntegerType>::None,
        None,
        None,
        None,
        None,
    )?;
    let end_pitch = interval
        .clone()
        .transpose_pitch(&start_pitch, false, Some(4))?;

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

    Ok(denominator_twos as FloatType * 2.0_f64.ln()
        + denominator_threes as FloatType * 3.0_f64.ln())
}

fn convert_harmonic_to_cents(_harmonic_shift: IntegerType) -> IntegerType {
    let mut value = _harmonic_shift as FloatType;
    if value < 0.0 {
        value = 1.0 / value.abs();
    }
    (1200.0 * value.log2()).round() as IntegerType
}

#[cfg(test)]
mod tests {
    use crate::defaults::IntegerType;
    use crate::interval::{Interval, IntervalArgument};

    use super::{Pitch, convert_harmonic_to_cents, simplify_multiple_enharmonics};

    #[test]
    fn simplify_multiple_enharmonics_test() {
        let more_than_five = vec![
            Pitch::new(
                Some(0),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(1),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(2),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(3),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(4),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(5),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(12),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            Pitch::new(
                Some(13),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
        ];

        let _x = simplify_multiple_enharmonics(&more_than_five, None, None);
        let _less_than_five = [
            Pitch::new(
                Some(0),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(1),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(2),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(12),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
            Pitch::new(
                Some(13),
                None,
                None,
                Option::<i8>::None,
                Option::<IntegerType>::None,
                None,
                None,
                None,
                None,
            ),
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
        let c4 = Pitch::new(
            Some("C4".to_string()),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let m3 = Interval::new(IntervalArgument::Str("m3".to_string())).unwrap();
        let out = c4.transpose(m3);
        assert_eq!(out.name_with_octave(), "E-4");
    }

    #[test]
    fn test_higher_enharmonic_helper() {
        let c_sharp = Pitch::new(
            Some("C#3".to_string()),
            None,
            None,
            Option::<i8>::None,
            Option::<IntegerType>::None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let out = c_sharp.get_higher_enharmonic().unwrap();
        assert_eq!(out.name_with_octave(), "D-3");
    }
}
