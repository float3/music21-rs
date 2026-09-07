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
use crate::tuningsystem::TuningSystem;

pub use accidental::{Accidental, AccidentalAttribute, AccidentalSpecifier};
pub use microtone::{Microtone, MicrotoneSpecifier};
use pitchclass::convert_ps_to_oct;
pub use pitchclass::{PitchClass, PitchClassSpecifier, convert_pitch_class_to_str};

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
    #[cfg_attr(feature = "serde", serde(default))]
    has_accidental: bool,
    microtone: Option<Microtone>,
    spelling_is_inferred: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    fundamental: Option<Arc<Pitch>>,
}

/// The context music21's `updateAccidentalDisplay` reads: the pitches that
/// came before, in this measure and the previous one, the other pitches
/// sounding at the same time, the key signature's altered pitches, and the
/// cautionary-accidental switches, with music21's defaults.
#[derive(Clone, Debug)]
pub struct AccidentalDisplayOptions<'a> {
    /// Pitches preceding this one in the same measure.
    pub pitch_past: &'a [Pitch],
    /// Pitches preceding this one in the previous measure.
    pub pitch_past_measure: &'a [Pitch],
    /// Other pitches in the same simultaneity.
    pub other_simultaneous_pitches: &'a [Pitch],
    /// The key signature's altered pitches.
    pub altered_pitches: &'a [Pitch],
    /// Whether a past accidental in any octave calls for a cautionary one.
    pub cautionary_pitch_class: bool,
    /// Whether every accidental is shown.
    pub cautionary_all: bool,
    /// Whether an already decided `display_status` is decided again.
    pub override_status: bool,
    /// Whether an altered pitch shows its accidental again unless it
    /// immediately repeats.
    pub cautionary_not_immediate_repeat: bool,
    /// Whether this pitch follows a tie.
    pub last_note_was_tied: bool,
}

impl Default for AccidentalDisplayOptions<'_> {
    fn default() -> Self {
        Self {
            pitch_past: &[],
            pitch_past_measure: &[],
            other_simultaneous_pitches: &[],
            altered_pitches: &[],
            cautionary_pitch_class: true,
            cautionary_all: false,
            override_status: false,
            cautionary_not_immediate_repeat: true,
            last_note_was_tied: false,
        }
    }
}

impl PartialEq for Pitch {
    fn eq(&self, other: &Self) -> bool {
        self.step == other.step
            && self.octave == other.octave
            && self.accidental == other.accidental
            && self.has_accidental == other.has_accidental
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
        let parsed_has_accidental = parsed.accidental.is_some();
        let accidental = match accidental {
            Some(accidental) => Accidental::new(accidental)?,
            None => parsed.accidental.unwrap_or_default(),
        };
        let microtone = match microtone {
            Some(microtone) => Some(Microtone::new(microtone)?),
            None => parsed.microtone,
        };

        let explicit_accidental = has_explicit_accidental.then(|| accidental.clone());
        let mut pitch = Pitch {
            step,
            accidental,
            has_accidental: has_explicit_accidental || parsed_has_accidental,
            microtone,
            octave,
            spelling_is_inferred: parsed.spelling_is_inferred,
            fundamental: None,
        };

        if let Some(name) = &name {
            pitch.name_setter(name)?;
            pitch.spelling_is_inferred = parsed.spelling_is_inferred;
        }
        if explicit_step.is_some() || name.is_none() {
            pitch.step_setter(step);
            pitch.spelling_is_inferred = parsed.spelling_is_inferred;
        }
        if has_explicit_octave || name.is_none() {
            pitch.octave_setter(octave);
        }
        if let Some(accidental) = explicit_accidental {
            pitch.accidental_setter(accidental);
        } else if pitch.spelling_is_inferred {
            pitch.has_accidental = pitch.accidental.alter() != 0.0;
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

        let mut pitch_part = String::with_capacity(usr_str.len());
        let mut octave_part = String::new();
        for character in usr_str.chars() {
            if character.is_ascii_digit() {
                if pitch_part.is_empty() {
                    return Err(Error::Pitch(format!(
                        "Cannot have octave given before pitch name in {usr_str:?}"
                    )));
                }
                octave_part.push(character);
            } else {
                pitch_part.push(character);
            }
        }

        let mut pitch_chars = pitch_part.chars();
        let step = pitch_chars.next().ok_or(Error::Pitch(format!(
            "Cannot make a name out of {pitch_part:?}"
        )))?;
        self.step_setter(StepName::try_from(step)?);

        let accidental_str: String = pitch_chars.collect();
        if accidental_str.is_empty() {
            self.accidental = Accidental::natural();
            self.has_accidental = false;
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

    /// Puts this pitch in an octave, or in none at all: music21's settable
    /// `octave`, which only moves the pitch and does not respell it.
    pub fn set_octave(&mut self, octave: Octave) {
        self.octave = octave;
    }

    pub(crate) fn octave_setter(&mut self, octave: Octave) {
        self.set_octave(octave);
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

    /// Returns this pitch transposed by the interval, as music21's
    /// `Pitch.transpose` does: a pitch whose spelling was inferred from a
    /// number is respelled to its most common enharmonic afterwards, one
    /// spelled explicitly keeps its accidentals.
    pub fn transpose(&self, interval: &Interval) -> Result<Pitch> {
        let mut p = interval.transpose_pitch_with_options(self, false, Some(4))?;

        if !interval.implicit_diatonic {
            p.spelling_is_inferred = self.spelling_is_inferred;
        }
        if p.spelling_is_inferred {
            p.simplify_enharmonic_in_place(true)?;
        }
        if let Some(fundamental) = &self.fundamental {
            p.fundamental_setter(fundamental.transpose(interval)?);
        }

        Ok(p)
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

    /// Returns the MIDI note number the way music21's `midi` reports it: the
    /// pitch space rounded half up, then folded into 0 to 127 by octaves, so
    /// a pitch above the MIDI range reports its highest in-range octave and
    /// one below it its lowest.
    pub fn midi(&self) -> IntegerType {
        normalize_midi((self.pitch_space() + 0.5).floor() as IntegerType)
    }

    /// Returns this pitch's twelve-tone equal-temperament frequency in hertz.
    pub fn frequency_hz(&self) -> FloatType {
        440.0 * (2.0 as FloatType).powf((self.pitch_space() - 69.0) / 12.0)
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
        self.spelling_is_inferred = false;
    }

    /// Moves the pitch to a staff position by diatonic note number, keeping
    /// its accidental and microtone: music21's `diatonicNoteNum` setter.
    pub(crate) fn set_diatonic_note_number(&mut self, dnn: IntegerType) -> Result<()> {
        let (step, octave) = crate::interval::convert_diatonic_number_to_step(dnn);
        self.step_setter(StepName::try_from(step)?);
        self.octave_setter(Some(octave));
        Ok(())
    }

    /// Replaces the accidental, a natural when `None` is given.
    pub(crate) fn set_accidental_or_natural(&mut self, accidental: Option<Accidental>) {
        self.set_accidental(accidental);
    }

    /// Sets or removes the accidental the way music21's `accidental` setter
    /// does: `None` leaves the pitch with no accidental object, which
    /// [`Self::accidental`] still reports as a natural.
    pub fn set_accidental(&mut self, accidental: Option<Accidental>) {
        match accidental {
            Some(accidental) => self.accidental_setter(accidental),
            None => {
                self.accidental = Accidental::natural();
                self.has_accidental = false;
            }
        }
    }

    /// Sets the accidental from a semitone alteration the way music21's
    /// setter does with a float: the part that is a quarter-tone or larger
    /// becomes the accidental and the rest a microtone, so `-1.5` is a flat
    /// with fifty cents down.
    pub fn set_accidental_alter(&mut self, alter: FloatType) -> Result<()> {
        let (alter_shift, cents) = cents_to_alter_and_cents(alter * 100.0);
        self.accidental_setter(Accidental::new(alter_shift)?);
        if cents.abs() > 0.01 {
            self.microtone_setter(Microtone::new(cents)?);
        }
        Ok(())
    }

    /// Whether the pitch carries an accidental object at all. music21 keeps
    /// none on a pitch spelled with a bare letter (`D`) or built from a
    /// number that needs none, and an explicit natural (`Dn`) is one, so
    /// `D` and `Dn` differ here while [`Self::accidental`] answers a natural
    /// for both.
    pub fn has_accidental(&self) -> bool {
        self.has_accidental
    }

    /// The accidental object, if the pitch carries one; see
    /// [`Self::has_accidental`].
    pub fn explicit_accidental(&self) -> Option<&Accidental> {
        self.has_accidental.then_some(&self.accidental)
    }

    /// The accidental object for editing in place, if the pitch carries one.
    pub fn explicit_accidental_mut(&mut self) -> Option<&mut Accidental> {
        self.has_accidental.then_some(&mut self.accidental)
    }

    /// music21's `_nameInKeySignature`: whether one of a key signature's
    /// altered pitches has this pitch's step and the same accidental. A
    /// pitch with no accidental object never matches.
    pub fn name_in_key_signature(&self, altered_pitches: &[Pitch]) -> bool {
        if !self.has_accidental {
            return false;
        }
        altered_pitches.iter().any(|p| {
            p.step == self.step && p.has_accidental && p.accidental.name() == self.accidental.name()
        })
    }

    /// music21's `_stepInKeySignature`: whether a key signature alters this
    /// pitch's step at all, whatever the accidental.
    pub fn step_in_key_signature(&self, altered_pitches: &[Pitch]) -> bool {
        altered_pitches.iter().any(|p| p.step == self.step)
    }

    fn set_display_status_creating(&mut self, status: bool) {
        if !self.has_accidental {
            self.accidental = Accidental::natural();
            self.has_accidental = true;
        }
        self.accidental.set_display_status(Some(status));
    }

    /// Decides whether this pitch's accidental should be shown, given the
    /// pitches before it, and records the answer as the accidental's
    /// `display_status`, adding a natural where a cautionary one is called
    /// for: music21's `updateAccidentalDisplay`, with the same rules for
    /// repeats, octaves, ties, key signatures and simultaneities.
    pub fn update_accidental_display(&mut self, options: &AccidentalDisplayOptions<'_>) {
        let altered = options.altered_pitches;
        let past_all: Vec<&Pitch> = options
            .pitch_past_measure
            .iter()
            .chain(options.pitch_past.iter())
            .collect();
        let mut display_if_no_previous_accidentals = false;

        if !options.override_status
            && self.has_accidental
            && self.accidental.display_status().is_some()
        {
            return;
        }
        if self.has_accidental && self.accidental.display_type() == "never" {
            self.accidental.set_display_status(Some(false));
            return;
        }
        if options.last_note_was_tied {
            if self.has_accidental {
                let even_tied = self.accidental.display_type() == "even-tied";
                self.accidental.set_display_status(Some(even_tied));
            }
            return;
        }
        if options.cautionary_pitch_class
            && options
                .other_simultaneous_pitches
                .iter()
                .any(|p| p.step == self.step && p.pitch_class() != self.pitch_class())
        {
            self.set_display_status_creating(true);
            return;
        }
        if options.cautionary_all
            || (self.has_accidental
                && matches!(self.accidental.display_type(), "even-tied" | "always"))
        {
            self.set_display_status_creating(true);
            return;
        }
        if past_all.is_empty() {
            if self.has_accidental
                && (options.override_status || self.accidental.display_status() != Some(true))
            {
                let status = if self.accidental.name() == "natural" {
                    self.step_in_key_signature(altered)
                } else {
                    !self.name_in_key_signature(altered)
                };
                self.accidental.set_display_status(Some(status));
            } else if self.has_accidental
                && self.accidental.display_status() == Some(true)
                && self.name_in_key_signature(altered)
            {
                self.accidental.set_display_status(Some(false));
            } else if (!self.has_accidental || self.accidental.name() == "natural")
                && self.step_in_key_signature(altered)
            {
                self.set_display_status_creating(true);
            }
            return;
        }
        for past in options.pitch_past.iter().rev() {
            if past.step == self.step && past.octave == self.octave {
                if past.name() != self.name() {
                    self.set_display_status_creating(true);
                    return;
                }
                break;
            }
        }
        let mut set_from_pitch_past = false;
        let out_of_measure_length = options.pitch_past_measure.len();
        let self_name = self.name();
        let self_name_with_octave = self.name_with_octave();
        let name_in_key = self.name_in_key_signature(altered);
        let step_in_key = self.step_in_key_signature(altered);
        let if_absolutely_necessary =
            self.has_accidental && self.accidental.display_type() == "if-absolutely-necessary";
        for i in (0..past_all.len()).rev() {
            let past_in_measure = i >= out_of_measure_length;
            let continuous_repeats_in_measure = past_in_measure
                && past_all[i..]
                    .iter()
                    .all(|p| p.name_with_octave() == self_name_with_octave);
            if !past_in_measure && if_absolutely_necessary {
                break;
            }
            if !past_in_measure
                && self.has_accidental
                && self.accidental.name() != "natural"
                && !name_in_key
            {
                self.accidental.set_display_status(Some(true));
                return;
            }
            let past = past_all[i];
            if past.step != self.step {
                continue;
            }
            let octave_match = self.octave == past.octave;
            let past_acc = past.explicit_accidental();
            let past_name = past_acc.map(Accidental::name);
            let past_status = past_acc.and_then(Accidental::display_status);
            let self_acc_name = self
                .has_accidental
                .then(|| self.accidental.name().to_string());
            let self_status = self
                .has_accidental
                .then(|| self.accidental.display_status())
                .flatten();

            if continuous_repeats_in_measure && past_status == Some(true) {
                if self.has_accidental {
                    self.accidental.set_display_status(Some(false));
                }
                return;
            } else if continuous_repeats_in_measure
                && past_acc.is_some()
                && self_acc_name.is_some()
                && past_name == self_acc_name.as_deref()
            {
                if !name_in_key && (!octave_match || past_status == Some(false)) {
                    display_if_no_previous_accidentals = true;
                    continue;
                }
                self.accidental.set_display_status(Some(false));
                set_from_pitch_past = true;
                break;
            } else if past_name == Some("natural")
                && (self_acc_name.is_none() || self_acc_name.as_deref() == Some("natural"))
            {
                if continuous_repeats_in_measure {
                    if step_in_key && !octave_match {
                        self.set_display_status_creating(true);
                    } else if self.has_accidental {
                        self.accidental.set_display_status(Some(false));
                    }
                } else if step_in_key
                    && (options.cautionary_not_immediate_repeat || !past_in_measure)
                {
                    self.set_display_status_creating(true);
                } else if self.has_accidental {
                    self.accidental.set_display_status(Some(false));
                }
                set_from_pitch_past = true;
                break;
            } else if past_acc.is_some()
                && past.name() != self_name
                && past_name != Some("natural")
                && (self_acc_name.is_none() || self_status == Some(false))
            {
                if !octave_match && !options.cautionary_pitch_class {
                    continue;
                }
                if !octave_match && if_absolutely_necessary {
                    continue;
                }
                self.set_display_status_creating(true);
                set_from_pitch_past = true;
                break;
            } else if self_acc_name.is_some()
                && (((past_acc.is_none() || past_name == Some("natural"))
                    && self_acc_name.as_deref() != Some("natural"))
                    || (past_acc.is_some()
                        && past_name != self_acc_name.as_deref()
                        && (octave_match || !if_absolutely_necessary)))
            {
                self.accidental.set_display_status(Some(true));
                set_from_pitch_past = true;
                break;
            } else if past_acc.is_none() && self_acc_name.is_some() {
                let status = if self_acc_name.as_deref() == Some("natural") {
                    step_in_key
                } else {
                    true
                };
                self.accidental.set_display_status(Some(status));
                set_from_pitch_past = true;
                break;
            } else if !continuous_repeats_in_measure
                && past_acc.is_some()
                && self_acc_name.is_some()
                && past_name == self_acc_name.as_deref()
                && octave_match
            {
                if !options.cautionary_not_immediate_repeat && past_status != Some(false) {
                    self.accidental.set_display_status(Some(false));
                    display_if_no_previous_accidentals = false;
                    set_from_pitch_past = true;
                    break;
                } else if past_status == Some(false) {
                    display_if_no_previous_accidentals = true;
                } else {
                    self.accidental.set_display_status(Some(!name_in_key));
                    return;
                }
            }
        }
        if display_if_no_previous_accidentals {
            if !name_in_key {
                self.set_display_status_creating(true);
            } else if self.has_accidental {
                self.accidental.set_display_status(Some(false));
            }
        } else if !set_from_pitch_past && self.has_accidental {
            let status = if self.accidental.name() == "natural" {
                step_in_key
            } else {
                !name_in_key
            };
            self.accidental.set_display_status(Some(status));
        } else if !set_from_pitch_past && step_in_key {
            self.set_display_status_creating(true);
        }
    }

    fn accidental_setter(&mut self, value: Accidental) {
        self.accidental = value;
        self.has_accidental = true;
    }

    /// Sets the microtone from a cent shift, removing it when the shift is
    /// zero.
    pub fn set_microtone_cents(&mut self, cents: FloatType) -> Result<()> {
        if cents == 0.0 {
            self.microtone = None;
        } else {
            self.microtone = Some(Microtone::new(cents)?);
        }
        Ok(())
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
        self.has_accidental = accidental.alter() != 0.0;
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
        self.has_accidental = accidental.alter() != 0.0;
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

    /// Returns the octave, or music21's default of 4 when none is set.
    pub fn implicit_octave(&self) -> IntegerType {
        self.octave.unwrap_or(PITCH_OCTAVE as IntegerType)
    }

    /// Returns the pitch class as music21's `pitchClassString`, one
    /// character with `A` and `B` for ten and eleven. Like music21's integer
    /// `pitchClass` it rounds a microtone away with Python's round-half-to-even,
    /// so `C` with `+20c` is `0` where [`Self::pitch_class`] would say `0.2`,
    /// and a half-sharp C is `0` even though its MIDI number rounds up to 61.
    pub fn pitch_class_string(&self) -> String {
        crate::pitch::pitchclass::convert_pitch_class_to_str(
            self.ps().round_ties_even() as IntegerType
        )
    }

    /// Returns how many cents the pitch sits from the nearest MIDI note,
    /// rounded to a whole cent: music21's `getCentShiftFromMidi`, so a
    /// half-sharp C reads `-50` because it rounds up to C-sharp.
    pub fn cent_shift_from_midi(&self) -> IntegerType {
        let mut distance = self.ps() - FloatType::from(self.midi());
        while distance < -11.0 {
            distance += 12.0;
        }
        while distance > 11.0 {
            distance -= 12.0;
        }
        (distance * 100.0).round() as IntegerType
    }

    /// Returns the enharmonic music21's `getEnharmonic` picks: sharps respell
    /// upward and flats downward, and a natural goes down for C, D and G and
    /// up for the rest, so C is B-sharp and E is F-flat.
    /// This pitch respelled to agree with a key signature, when the two
    /// disagree about the same sounding note.
    ///
    /// This is the rule music21 applies after transposing by a number of
    /// semitones: a `G-` in a key that writes an `F#` is written `F#`, and
    /// the other way round, because a chromatic step says how far to move
    /// and not how to spell what it lands on. A pitch with no accidental,
    /// or one the signature does not alter, is left as it is.
    pub fn respelled_for(&self, signature: &crate::key::KeySignature) -> Result<Pitch> {
        if !self.has_accidental() {
            return Ok(self.clone());
        }
        let alter = self.accidental().alter();
        for altered in signature.altered_pitches()? {
            if altered.pitch_class() == self.pitch_class() && altered.accidental().alter() != alter
            {
                return self.get_enharmonic();
            }
        }
        Ok(self.clone())
    }

    pub fn get_enharmonic(&self) -> Result<Pitch> {
        let alter = self.accidental.alter();
        let downward = if alter > 0.0 {
            false
        } else if alter < 0.0 {
            true
        } else {
            matches!(self.step.as_char(), 'C' | 'D' | 'G')
        };
        if downward {
            self.get_lower_enharmonic()
        } else {
            self.get_higher_enharmonic()
        }
    }

    /// Returns the pitch with any quarter-tone accidental folded into the
    /// microtone: music21's `convertQuarterTonesToMicrotones`, so a half-sharp
    /// C becomes C with `+50c` and a one-and-a-half-sharp D becomes D-sharp
    /// with `+50c`. Other accidentals are untouched.
    pub fn convert_quarter_tones_to_microtones(&self) -> Result<Pitch> {
        let (alter, shift) = match self.accidental.name() {
            "half-flat" => (0.0, -50.0),
            "half-sharp" => (0.0, 50.0),
            "one-and-a-half-sharp" => (1.0, 50.0),
            "one-and-a-half-flat" => (-1.0, -50.0),
            _ => return Ok(self.clone()),
        };
        let cents = self.microtone.as_ref().map_or(0.0, Microtone::cents);
        let mut pitch = self.clone();
        pitch.accidental_setter(Accidental::new(alter)?);
        pitch.microtone_setter(Microtone::new(cents + shift)?);
        Ok(pitch)
    }

    /// Returns the pitch with its microtone rounded into the nearest
    /// quarter-tone accidental and the remainder kept as a microtone:
    /// music21's `convertMicrotonesToQuarterTones`, so C with `+30c` becomes a
    /// half-sharp C with `-20c` and C with `+150c` becomes C-sharp with `+50c`.
    pub fn convert_microtones_to_quarter_tones(&self) -> Result<Pitch> {
        let cents = self.microtone.as_ref().map_or(0.0, Microtone::cents);
        let (shift, remainder) = cents_to_alter_and_cents(cents);
        let mut pitch = self.clone();
        pitch.accidental_setter(Accidental::new(self.accidental.alter() + shift)?);
        pitch.microtone_setter(Microtone::new(remainder)?);
        Ok(pitch)
    }

    /// Returns which harmonic of `fundamental` this pitch is closest to, and
    /// the fundamental retuned by the difference so that the harmonic lands
    /// exactly here: music21's `harmonicAndFundamentalFromPitch`, so `E5`
    /// over `C2` is the tenth harmonic of `C2` raised 14 cents.
    pub fn harmonic_and_fundamental_from_pitch(&self, fundamental: &Pitch) -> Result<(u32, Pitch)> {
        let (number, cents) = self.harmonic_from_fundamental(fundamental)?;
        let cents = -cents;
        let mut retuned = fundamental.clone();
        match retuned.microtone.as_ref().map(Microtone::cents) {
            Some(existing) => retuned.microtone_setter(Microtone::new(existing + cents)?),
            None if cents != 0.0 => retuned.microtone_setter(Microtone::new(cents)?),
            None => {}
        }
        Ok((number, retuned))
    }

    /// Returns [`Self::harmonic_and_fundamental_from_pitch`] in music21's
    /// notation, `10thH/C2(+14c)`.
    pub fn harmonic_and_fundamental_string_from_pitch(
        &self,
        fundamental: &Pitch,
    ) -> Result<String> {
        let (number, retuned) = self.harmonic_and_fundamental_from_pitch(fundamental)?;
        let suffix = crate::pitch::microtone::ordinal_suffix(number as IntegerType);
        Ok(format!(
            "{number}{suffix}H/{}",
            retuned.name_with_octave_and_microtone()
        ))
    }

    /// Builds a pitch from a frequency in hertz, spelled in twelve-tone equal
    /// temperament at A4 = 440 with any remainder as a microtone.
    pub fn from_frequency(hertz: FloatType) -> Result<Self> {
        if hertz.is_nan() || hertz <= 0.0 {
            return Err(Error::Pitch(format!(
                "frequency must be greater than zero, got {hertz}"
            )));
        }
        let mut pitch = Pitch::default();
        pitch.ps_setter(12.0 * (hertz / 440.0).log2() + 69.0);
        Ok(pitch)
    }

    /// Returns music21's `diatonicNoteNum`: the staff position counting `C0`
    /// as `1`, with the implicit octave standing in when none is set.
    pub fn diatonic_note_number(&self) -> IntegerType {
        let octave = self.octave.unwrap_or(PITCH_OCTAVE as IntegerType);
        self.step.step_to_dnn_offset() + 7 * octave
    }

    /// Returns whether this pitch lies on the twelve-tone grid: no quarter
    /// tone accidental and no microtone.
    pub fn is_twelve_tone(&self) -> bool {
        self.accidental.is_twelve_tone()
            && self
                .microtone
                .as_ref()
                .is_none_or(|microtone| microtone.cents() == 0.0)
    }

    /// Returns whether the two pitches sound the same. Without an octave on
    /// either side only the pitch class is compared.
    pub fn is_enharmonic(&self, other: &Pitch) -> bool {
        if self.octave.is_none() || other.octave.is_none() {
            (other.ps() - self.ps()).rem_euclid(12.0) == 0.0
        } else {
            other.ps() == self.ps()
        }
    }

    /// Returns the other spellings of this pitch with at most `alter_limit`
    /// sharps or flats, simplest first, as music21's `getAllCommonEnharmonics`
    /// lists them.
    pub fn all_common_enharmonics(&self, alter_limit: IntegerType) -> Vec<Pitch> {
        let mut found = Vec::new();
        if let Ok(simplified) = self.simplify_enharmonic(false)
            && simplified.name() != self.name()
        {
            found.push(simplified);
        }
        for upward in [true, false] {
            let mut current = self.clone();
            while let Ok(next) = current.enharmonic_neighbour(upward) {
                if next.accidental().alter().abs() > alter_limit as FloatType
                    || found.contains(&next)
                {
                    break;
                }
                found.push(next.clone());
                current = next;
            }
        }
        found
    }

    /// Returns this pitch moved down by octaves until it is at or below
    /// `target`. With `minimize` it is then raised back to within an octave.
    pub fn transpose_below_target(&self, target: &Pitch, minimize: bool) -> Result<Pitch> {
        let mut pitch = self.octave_bearing_copy("transposeBelowTarget")?;
        while pitch.ps() > target.ps() {
            pitch.shift_octave(-1);
        }
        if minimize {
            while target.ps() - pitch.ps() >= 12.0 {
                pitch.shift_octave(1);
            }
        }
        Ok(pitch)
    }

    /// Returns this pitch moved up by octaves until it is at or above
    /// `target`. With `minimize` it is then lowered back to within an octave.
    pub fn transpose_above_target(&self, target: &Pitch, minimize: bool) -> Result<Pitch> {
        let mut pitch = self.octave_bearing_copy("transposeAboveTarget")?;
        while pitch.ps() < target.ps() {
            pitch.shift_octave(1);
        }
        if minimize {
            while pitch.ps() - target.ps() >= 12.0 {
                pitch.shift_octave(-1);
            }
        }
        Ok(pitch)
    }

    /// Returns the `number`th harmonic of this pitch as a fundamental, spelled
    /// to the nearest twelve-tone pitch with the remainder as a microtone and
    /// this pitch recorded as its fundamental. The first harmonic is the
    /// pitch itself.
    pub fn harmonic(&self, number: u32) -> Result<Pitch> {
        let cent_shift = convert_harmonic_to_cents(number as IntegerType);
        if cent_shift == 0 {
            return Ok(self.clone());
        }
        let mut shifted = self.clone();
        let existing = self.microtone.as_ref().map_or(0.0, Microtone::cents);
        shifted.microtone_setter(Microtone::new(existing + cent_shift as FloatType)?);
        let mut harmonic = Pitch::from_frequency(shifted.frequency_hz())?;
        harmonic.fundamental_setter(self.clone());
        Ok(harmonic)
    }

    /// Returns which harmonic of `fundamental` this pitch is closest to, and
    /// the distance to that harmonic in cents: negative when this pitch lies
    /// above the harmonic, positive when below.
    pub fn harmonic_from_fundamental(&self, fundamental: &Pitch) -> Result<(u32, FloatType)> {
        if self.ps() <= fundamental.ps() {
            return Err(Error::Pitch(format!(
                "cannot find an equivalent harmonic for a fundamental ({fundamental}) that is not above this Pitch ({self})"
            )));
        }
        let mut found = Vec::new();
        for number in 1..32 {
            let candidate = fundamental.harmonic(number)?;
            let above = candidate.ps() > self.ps();
            found.push((number, candidate));
            if above {
                break;
            }
        }
        let (number, gap) = match found.as_slice() {
            [(number, only)] => (*number, only.ps() - self.ps()),
            [.., (lower_number, lower), (higher_number, higher)] => {
                let below = self.ps() - lower.ps();
                let above = higher.ps() - self.ps();
                if below <= above {
                    (*lower_number, -below.abs())
                } else {
                    (*higher_number, above.abs())
                }
            }
            [] => unreachable!("the first harmonic is always collected"),
        };
        Ok((
            number,
            round_to_digits(gap, PITCH_SPACE_SIGNIFICANT_DIGITS) * 100.0,
        ))
    }

    /// Describes this pitch as a harmonic of `fundamental`, or of its own
    /// fundamental when none is given, in music21's notation: `"3rdH(-2c)/C2"`.
    pub fn harmonic_string(&self, fundamental: Option<&Pitch>) -> Result<String> {
        let fundamental = fundamental.or(self.fundamental()).ok_or_else(|| {
            Error::Pitch("no fundamental is defined for this Pitch: provide one".to_string())
        })?;
        let (number, cents) = self.harmonic_from_fundamental(fundamental)?;
        let suffix = crate::pitch::microtone::ordinal_suffix(number as IntegerType);
        let fundamental = fundamental.name_with_octave_and_microtone();
        if cents == 0.0 {
            Ok(format!("{number}{suffix}H/{fundamental}"))
        } else {
            let microtone = Microtone::new(-cents)?;
            Ok(format!("{number}{suffix}H{microtone}/{fundamental}"))
        }
    }

    fn octave_bearing_copy(&self, operation: &str) -> Result<Pitch> {
        if self.octave.is_none() {
            return Err(Error::Pitch(format!(
                "Cannot call {operation} with an octaveless Pitch."
            )));
        }
        Ok(self.clone())
    }

    fn shift_octave(&mut self, octaves: IntegerType) {
        let octave = self.octave.unwrap_or(PITCH_OCTAVE as IntegerType);
        self.octave_setter(Some(octave + octaves));
    }

    /// Returns the German name, where `B` is `H`, `B-` is `B`, sharps add
    /// `is` and flats add `es` or `s`. Errors on a microtonal accidental.
    pub fn german(&self) -> Result<String> {
        let alter = self.whole_alteration("german")?;
        let mut step = self.step.as_char().to_string();
        let mut alter = alter;
        if self.step == StepName::B {
            if alter == -1 {
                alter = 0;
            } else {
                step = "H".to_string();
            }
        }
        Ok(match alter {
            0 => step,
            alter if alter > 0 => step + &"is".repeat(alter as usize),
            alter => {
                let first = if matches!(step.as_str(), "C" | "D" | "F" | "G" | "H") {
                    "es"
                } else {
                    "s"
                };
                step + first + &"es".repeat(alter.unsigned_abs() as usize - 1)
            }
        })
    }

    /// Returns the Italian solfège name, such as `"do diesis"` or
    /// `"si doppio bemolle"`. Errors on a microtonal accidental or more than
    /// four sharps or flats.
    pub fn italian(&self) -> Result<String> {
        let alter = self.whole_alteration("italian")?;
        let solfege = match self.step {
            StepName::C => "do",
            StepName::D => "re",
            StepName::E => "mi",
            StepName::F => "fa",
            StepName::G => "sol",
            StepName::A => "la",
            StepName::B => "si",
        };
        let cardinality = match alter.unsigned_abs() {
            0 => return Ok(solfege.to_string()),
            1 => " ",
            2 => " doppio ",
            3 => " triplo ",
            4 => " quadruplo ",
            _ => {
                return Err(Error::Pitch(format!(
                    "entirely too many accidentals for an Italian name: {self}"
                )));
            }
        };
        let kind = if alter > 0 { "diesis" } else { "bemolle" };
        Ok(format!("{solfege}{cardinality}{kind}"))
    }

    /// Returns the French solfège name, such as `"ré bémol"` or
    /// `"fa double dièse"`. Errors on a microtonal accidental or more than
    /// four sharps or flats.
    pub fn french(&self) -> Result<String> {
        let alter = self.whole_alteration("french")?;
        let solfege = match self.step {
            StepName::D => "ré",
            other => romance_solfege(other),
        };
        let multiplier = match alter.unsigned_abs() {
            0 => return Ok(solfege.to_string()),
            1 => "",
            2 => " double",
            3 => " triple",
            4 => " quadruple",
            _ => {
                return Err(Error::Pitch(format!(
                    "entirely too many accidentals for a French name: {self}"
                )));
            }
        };
        let kind = if alter > 0 { "dièse" } else { "bémol" };
        Ok(format!("{solfege}{multiplier} {kind}"))
    }

    /// Returns the Spanish solfège name, such as `"re bemol"` or
    /// `"fa doble sostenido"`. Errors on a microtonal accidental or more than
    /// four sharps or flats.
    pub fn spanish(&self) -> Result<String> {
        let alter = self.whole_alteration("spanish")?;
        let solfege = romance_solfege(self.step);
        let multiplier = match alter.unsigned_abs() {
            0 => return Ok(solfege.to_string()),
            1 => "",
            2 => " doble",
            3 => " triple",
            4 => " cuádruple",
            _ => {
                return Err(Error::Pitch(format!(
                    "entirely too many accidentals for a Spanish name: {self}"
                )));
            }
        };
        let kind = if alter > 0 { "sostenido" } else { "bemol" };
        Ok(format!("{solfege}{multiplier} {kind}"))
    }

    /// Returns the name with the accidental as a Unicode symbol, such as
    /// `"C♯"` or `"G𝄫"`.
    pub fn unicode_name(&self) -> String {
        if self.accidental.alter() == 0.0 {
            return self.step.as_char().to_string();
        }
        format!("{}{}", self.step.as_char(), self.accidental.unicode())
    }

    /// Returns music21's `fullName`: the step, the accidental's full name, the
    /// octave and any microtone, as in `E-flat in octave 4 (+20c)`.
    pub fn full_name(&self) -> String {
        let mut name = self.step.as_char().to_string();
        if self.accidental.alter() != 0.0 {
            name.push('-');
            name.push_str(self.accidental.full_name());
        }
        if let Some(octave) = self.octave {
            name.push_str(&format!(" in octave {octave}"));
        }
        if let Some(microtone) = &self.microtone
            && microtone.cents() != 0.0
        {
            name.push(' ');
            name.push_str(&microtone.to_string());
        }
        name
    }

    /// Returns the name with its octave and, when it has one that is not
    /// zero, its microtone: music21's `str(Pitch)`, `A4(+20c)`.
    pub fn name_with_octave_and_microtone(&self) -> String {
        match &self.microtone {
            Some(microtone) if microtone.cents() != 0.0 => {
                format!("{}{microtone}", self.name_with_octave())
            }
            _ => self.name_with_octave(),
        }
    }

    /// Returns [`Self::unicode_name`] followed by the octave when one is set.
    pub fn unicode_name_with_octave(&self) -> String {
        match self.octave {
            Some(octave) => format!("{}{octave}", self.unicode_name()),
            None => self.unicode_name(),
        }
    }

    fn whole_alteration(&self, language: &str) -> Result<IntegerType> {
        let alter = self.accidental.alter();
        if alter.fract() != 0.0 {
            return Err(Error::Pitch(match language {
                "german" => {
                    "Es geht nicht \"german\" zu benutzen mit Microtönen.  Schade!".to_string()
                }
                "italian" => "Non si puo usare `italian` con microtoni".to_string(),
                "french" => {
                    "On ne peut pas utiliser les microtones avec \"french.\" Quelle Dommage!"
                        .to_string()
                }
                "spanish" => "Unsupported accidental type.".to_string(),
                other => {
                    format!("{other} names cannot express the microtonal accidental of {self}")
                }
            }));
        }
        Ok(alter as IntegerType)
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
    microtone: Option<Microtone>,
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
                let (step, accidental, microtone, octave_shift) = convert_ps_to_step(number);
                let octave = (number >= 12.0).then(|| convert_ps_to_oct(number) + octave_shift);
                Self {
                    name: None,
                    step: Some(step),
                    accidental: Some(accidental),
                    microtone: (microtone.cents() != 0.0).then_some(microtone),
                    spelling_is_inferred: true,
                    octave,
                }
            }
        }
    }
}

fn romance_solfege(step: StepName) -> &'static str {
    match step {
        StepName::C => "do",
        StepName::D => "re",
        StepName::E => "mi",
        StepName::F => "fa",
        StepName::G => "sol",
        StepName::A => "la",
        StepName::B => "si",
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

/// A scoring function for [`simplify_multiple_enharmonics`]: lower is a
/// simpler spelling.
pub type CriterionFunction = fn(&[Pitch]) -> Result<FloatType>;

/// Respells a set of pitches so that they read as simply as possible
/// together: music21's `simplifyMultipleEnharmonics`. The first pitch is kept
/// as written and each of the others may be swapped for a common enharmonic;
/// the spelling chosen is the one the criterion scores lowest, by default
/// [`dissonance_score`]. Up to four pitches are searched exhaustively, more
/// are settled greedily one at a time, as upstream does. With a key
/// signature the tonic of its major key is placed first as an anchor and
/// removed again afterwards.
pub fn simplify_multiple_enharmonics(
    pitches: &[Pitch],
    criterion: Option<CriterionFunction>,
    key_context: Option<KeySignature>,
) -> Result<Vec<Pitch>> {
    let mut old_pitches: Vec<Pitch> = pitches.to_vec();
    if old_pitches.is_empty() {
        return Ok(Vec::new());
    }

    let criterion: CriterionFunction = criterion.unwrap_or(dissonance_score);

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

/// How awkward a set of pitches reads together: music21's
/// `_dissonanceScore` with all three of its terms on. It averages a penalty
/// for accidentals beyond one sharp or flat, a penalty growing with the
/// denominator of each pair's Pythagorean ratio, and a reward for every
/// third and sixth, so that `C E G` scores below `C F- G`.
pub fn dissonance_score(pitches: &[Pitch]) -> Result<FloatType> {
    weighted_dissonance_score(pitches, true, true, true)
}

fn weighted_dissonance_score(
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
                let interval_semitones = interval.chromatic.whole_semitones() % 12;
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

/// music21's `_convertCentsToAlterAndCents`: how much of a cent shift becomes
/// an accidental, in quarter-tone steps, and what is left as a microtone.
/// Ported as written, including the loop upstream runs for shifts below
/// -150 cents, which adds whole tones until the value passes +100 rather than
/// stopping at zero; nothing here relies on that range.
fn cents_to_alter_and_cents(shift: FloatType) -> (FloatType, FloatType) {
    let mut value = shift;
    let mut alter_add = 0.0;
    if value > 150.0 {
        let increment = (value / 100.0).floor();
        value -= increment * 100.0;
        alter_add += increment;
    } else if value < -150.0 {
        while value < 100.0 {
            value += 100.0;
            alter_add -= 1.0;
        }
    }
    let (alter_shift, cents) = if value < -75.0 {
        (-1.0, value + 100.0)
    } else if value < -25.0 {
        (-0.5, value + 50.0)
    } else if value <= 25.0 {
        (0.0, value)
    } else if value <= 75.0 {
        (0.5, value - 50.0)
    } else {
        (1.0, value - 100.0)
    };
    (alter_shift + alter_add, cents)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_pitch_is_respelled_to_agree_with_the_key_signature() {
        use crate::key::KeySignature;
        // music21's own example: a semitone above F is F# in D major and
        // G- in B-flat minor, because the signature spells that note.
        let f_sharp = Pitch::from_name("F#4").unwrap();
        assert_eq!(
            f_sharp.respelled_for(&KeySignature::new(2)).unwrap().name(),
            "F#"
        );
        assert_eq!(
            f_sharp
                .respelled_for(&KeySignature::new(-5))
                .unwrap()
                .name(),
            "G-"
        );
        // A pitch the signature says nothing about is left alone, and so is
        // one carrying no accidental at all.
        assert_eq!(
            Pitch::from_name("C4")
                .unwrap()
                .respelled_for(&KeySignature::new(-5))
                .unwrap()
                .name(),
            "C"
        );
    }

    #[test]
    fn an_octave_digit_may_sit_anywhere_after_the_step() {
        for (name, expected) in [
            ("e4-", "E-4"),
            ("e-4", "E-4"),
            ("b3-", "B-3"),
            ("g4--", "G--4"),
            ("c#4", "C#4"),
            ("f##2", "F##2"),
            ("d1", "D1"),
        ] {
            assert_eq!(
                Pitch::from_name(name).unwrap().name_with_octave(),
                expected,
                "parsing {name:?}"
            );
        }
        assert_eq!(
            Pitch::from_name("c4#2").unwrap().name_with_octave(),
            "C#42",
            "the digits concatenate wherever they fall, as music21's do"
        );
        assert!(Pitch::from_name("4c").is_err());
    }

    #[test]
    fn options_keep_an_explicit_accidental_and_a_numeric_name_stays_inferred() {
        let sharp = crate::PitchOptions::new()
            .name("C")
            .accidental("#")
            .octave(7)
            .microtone(-30)
            .build()
            .unwrap();
        assert_eq!(sharp.full_name(), "C-sharp in octave 7 (-30c)");
        let double_flat = crate::PitchOptions::new()
            .name("D")
            .accidental(Accidental::new("double-flat").unwrap())
            .build()
            .unwrap();
        assert_eq!(double_flat.name(), "D--");
        let inferred = Pitch::from_number(6.0).unwrap();
        assert_eq!(
            inferred
                .transpose(&Interval::from_name("-m2").unwrap())
                .unwrap()
                .name(),
            "F"
        );
    }

    #[test]
    fn midi_folds_into_range_like_music21() {
        let high = Pitch::from_name("C#10").unwrap();
        assert_eq!(high.midi(), 121);
        assert_eq!(high.cent_shift_from_midi(), 0);
        let half_flat = crate::PitchOptions::new().name("F`10").build().unwrap();
        assert_eq!(half_flat.midi(), 125);
        assert_eq!(
            Pitch::from_name("C#-2")
                .unwrap_or_else(|_| Pitch::from_pitch_space(-11.0).unwrap())
                .midi(),
            1
        );
        assert_eq!(Pitch::from_name("C~4").unwrap().midi(), 61);
        assert_eq!(Pitch::from_name("A4").unwrap().frequency_hz(), 440.0);
    }

    #[test]
    fn harmonics_chain_and_transpose_with_their_fundamental() {
        let a4 = Pitch::from_name("A4").unwrap();
        let seventh = a4.harmonic(7).unwrap();
        assert_eq!(seventh.name_with_octave(), "F#~7");
        assert_eq!(seventh.microtone().unwrap().cents().round(), 19.0);
        let doubled = seventh.harmonic(2).unwrap();
        assert_eq!(doubled.name_with_octave(), "F#~8");
        assert_eq!(doubled.microtone().unwrap().cents().round(), 19.0);
        assert_eq!(doubled.fundamental().unwrap().name_with_octave(), "F#~7");

        let second = a4.harmonic(2).unwrap();
        assert_eq!(second.fundamental().unwrap().name_with_octave(), "A4");
        let up_a_fifth = second
            .transpose(&Interval::from_name("p5").unwrap())
            .unwrap();
        assert_eq!(up_a_fifth.name_with_octave(), "E6");
        assert_eq!(up_a_fifth.fundamental().unwrap().name_with_octave(), "E5");
    }

    #[test]
    fn numbers_and_chromatic_transposition_keep_microtones() {
        let sharp_twenty = Pitch::from_number(60.2).unwrap();
        assert_eq!(sharp_twenty.name_with_octave(), "C4");
        assert_eq!(sharp_twenty.microtone().unwrap().cents().round(), 20.0);
        let harmonic = Pitch::from_name("A4")
            .unwrap()
            .harmonic(7)
            .unwrap()
            .harmonic(2)
            .unwrap();
        let down_two_octaves = harmonic
            .transpose(&Interval::from_semitones(-24).unwrap())
            .unwrap();
        assert_eq!(down_two_octaves.name_with_octave(), "F#~6");
        assert_eq!(down_two_octaves.microtone().unwrap().cents().round(), 19.0);
    }

    #[test]
    fn transposition_reaches_octave_minus_one() {
        let low = Pitch::from_name("D2")
            .unwrap()
            .transpose(&Interval::from_name("m-23").unwrap())
            .unwrap();
        assert_eq!(low.name_with_octave(), "C#-1");
        assert_eq!(low.ps(), 1.0);
    }

    #[test]
    fn simplify_multiple_enharmonics_matches_music21() {
        let names = |names: &[&str]| -> Vec<Pitch> {
            names
                .iter()
                .map(|n| Pitch::from_name(*n).unwrap())
                .collect()
        };
        let spelled = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };
        let cases: [(&[&str], &[&str]); 11] = [
            (&["C#", "D-", "E"], &["C#", "C#", "E"]),
            (&["A#4", "C#5", "E#5"], &["A#4", "C#5", "E#5"]),
            (&["F#", "A#", "C#"], &["F#", "A#", "C#"]),
            (&["G-", "B-", "D-"], &["G-", "B-", "D-"]),
            (&["C", "E-", "G-", "B--"], &["C", "E-", "F#", "A"]),
            (&["B#", "D##", "F##"], &["B#", "E", "G"]),
            (&["C", "E", "G#", "B"], &["C", "E", "G#", "B"]),
            (&["E#", "G##", "B#"], &["E#", "G##", "B#"]),
            (
                &["C#", "E#", "G#", "B", "D#"],
                &["C#", "E#", "G#", "B", "D#"],
            ),
            (
                &["D-", "F", "A-", "C-", "E-", "G-"],
                &["D-", "F", "A-", "C-", "E-", "G-"],
            ),
            (&["C4", "E-4", "F#4"], &["C4", "E-4", "G-4"]),
        ];
        for (input, expected) in cases {
            let simplified = simplify_multiple_enharmonics(&names(input), None, None).unwrap();
            assert_eq!(spelled(simplified), expected, "{input:?}");
        }
        let three_flats = crate::KeySignature::new(-3);
        assert_eq!(
            spelled(
                simplify_multiple_enharmonics(&names(&["C#", "D-", "E"]), None, Some(three_flats))
                    .unwrap()
            ),
            ["D-", "D-", "F-"]
        );
        let six_sharps = crate::KeySignature::new(6);
        assert_eq!(
            spelled(
                simplify_multiple_enharmonics(&names(&["G-", "B-", "D-"]), None, Some(six_sharps))
                    .unwrap()
            ),
            ["F#", "A#", "C#"]
        );
        assert!(
            simplify_multiple_enharmonics(&[], None, None)
                .unwrap()
                .is_empty()
        );
        assert_eq!(super::dissonance_score(&[]).unwrap(), 0.0);
        assert!(
            super::dissonance_score(&names(&["C", "E", "G"])).unwrap()
                < super::dissonance_score(&names(&["C", "F-", "G"])).unwrap()
        );
    }

    #[test]
    fn get_enharmonic_picks_the_direction_music21_does() {
        let cases = [
            ("C#4", "D-4"),
            ("D-4", "C#4"),
            ("C4", "B#3"),
            ("D4", "C##4"),
            ("G4", "F##4"),
            ("E4", "F-4"),
            ("F4", "G--4"),
            ("B4", "C-5"),
            ("A-4", "G#4"),
            ("F##4", "G4"),
            ("C--4", "B-3"),
            ("B#4", "C5"),
            ("G##4", "A4"),
        ];
        for (name, expected) in cases {
            let pitch = Pitch::from_name(name).unwrap();
            assert_eq!(
                pitch.get_enharmonic().unwrap().name_with_octave(),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn cent_shift_from_midi_matches_music21() {
        let cases: [(&str, FloatType, IntegerType, i32, &str); 8] = [
            ("C4", 0.0, 60, 0, "0"),
            ("C4", 20.0, 60, 20, "0"),
            ("C4", -20.0, 60, -20, "0"),
            ("C4", 60.0, 61, -40, "1"),
            ("C~4", 0.0, 61, -50, "0"),
            ("C`4", 0.0, 60, -50, "0"),
            ("C4", -60.0, 59, 40, "B"),
            ("C4", 130.0, 61, 30, "1"),
        ];
        for (name, cents, midi, shift, pitch_class) in cases {
            let pitch = crate::PitchOptions::new()
                .name(name)
                .microtone(cents)
                .build()
                .unwrap();
            assert_eq!(pitch.midi(), midi, "{name} {cents}");
            assert_eq!(pitch.cent_shift_from_midi(), shift, "{name} {cents}");
            assert_eq!(pitch.pitch_class_string(), pitch_class, "{name} {cents}");
            assert_eq!(pitch.implicit_octave(), 4);
        }
        assert_eq!(Pitch::from_name("G").unwrap().implicit_octave(), 4);
        assert_eq!(Pitch::from_name("G2").unwrap().implicit_octave(), 2);
    }

    #[test]
    fn quarter_tones_and_microtones_convert_both_ways() {
        let build = |name: &str, cents: FloatType| {
            crate::PitchOptions::new()
                .name(name)
                .microtone(cents)
                .build()
                .unwrap()
        };
        let describe = |pitch: &Pitch| {
            (
                pitch.name_with_octave(),
                pitch.microtone().map_or(0.0, Microtone::cents),
                pitch.accidental().name().to_string(),
            )
        };

        let to_microtones: [(&str, FloatType, &str, FloatType, &str); 6] = [
            ("C~4", 0.0, "C4", 50.0, "natural"),
            ("C`4", 0.0, "C4", -50.0, "natural"),
            ("D#~4", 0.0, "D#4", 50.0, "sharp"),
            ("E-`4", 0.0, "E-4", -50.0, "flat"),
            ("C~4", 10.0, "C4", 60.0, "natural"),
            ("C#4", 0.0, "C#4", 0.0, "sharp"),
        ];
        for (name, cents, expected_name, expected_cents, accidental) in to_microtones {
            let converted = build(name, cents)
                .convert_quarter_tones_to_microtones()
                .unwrap();
            assert_eq!(
                describe(&converted),
                (
                    expected_name.to_string(),
                    expected_cents,
                    accidental.to_string()
                ),
                "{name} {cents}"
            );
        }

        let to_quarter_tones: [(&str, FloatType, &str, FloatType, &str); 11] = [
            ("C4", 50.0, "C~4", 0.0, "half-sharp"),
            ("C4", -50.0, "C`4", 0.0, "half-flat"),
            ("C4", 30.0, "C~4", -20.0, "half-sharp"),
            ("C4", -30.0, "C`4", 20.0, "half-flat"),
            ("C4", 70.0, "C~4", 20.0, "half-sharp"),
            ("C#4", 50.0, "C#~4", 0.0, "one-and-a-half-sharp"),
            ("C4", 150.0, "C#4", 50.0, "sharp"),
            ("C4", -150.0, "C-4", -50.0, "flat"),
            ("C4", 120.0, "C#4", 20.0, "sharp"),
            ("C-4", -50.0, "C-`4", 0.0, "one-and-a-half-flat"),
            ("C4", 0.0, "C4", 0.0, "natural"),
        ];
        for (name, cents, expected_name, expected_cents, accidental) in to_quarter_tones {
            let converted = build(name, cents)
                .convert_microtones_to_quarter_tones()
                .unwrap();
            assert_eq!(
                describe(&converted),
                (
                    expected_name.to_string(),
                    expected_cents,
                    accidental.to_string()
                ),
                "{name} {cents}"
            );
        }
    }

    #[test]
    fn harmonic_and_fundamental_match_music21() {
        let cases = [
            ("E5", "C2", 10, 14.0, "10thH/C2(+14c)"),
            ("G4", "C2", 6, -2.0, "6thH/C2(-2c)"),
            ("B-4", "C2", 7, 31.0, "7thH/C2(+31c)"),
            ("F#5", "D3", 5, 14.0, "5thH/D3(+14c)"),
            ("C5", "C4", 2, 0.0, "2ndH/C4"),
        ];
        for (name, fundamental, number, cents, text) in cases {
            let pitch = Pitch::from_name(name).unwrap();
            let fundamental = Pitch::from_name(fundamental).unwrap();
            let (harmonic, retuned) = pitch
                .harmonic_and_fundamental_from_pitch(&fundamental)
                .unwrap();
            assert_eq!(harmonic, number, "{name}");
            let retuned_cents = retuned.microtone().map_or(0.0, Microtone::cents);
            assert!(
                (retuned_cents - cents).abs() < 1e-6,
                "{name}: {retuned_cents}"
            );
            assert_eq!(
                pitch
                    .harmonic_and_fundamental_string_from_pitch(&fundamental)
                    .unwrap(),
                text,
                "{name}"
            );
        }
        let c4 = Pitch::from_name("C4").unwrap();
        assert!(c4.harmonic_and_fundamental_from_pitch(&c4).is_err());
    }

    #[test]
    fn full_name_matches_music21() {
        let cases = [
            ("C4", "C in octave 4"),
            ("E-4", "E-flat in octave 4"),
            ("F#", "F-sharp"),
            ("B--3", "B-double-flat in octave 3"),
            ("G##5", "G-double-sharp in octave 5"),
            ("A~4", "A-half-sharp in octave 4"),
            ("C`4", "C-half-flat in octave 4"),
            ("D#~4", "D-one-and-a-half-sharp in octave 4"),
        ];
        for (name, expected) in cases {
            assert_eq!(
                Pitch::from_name(name).unwrap().full_name(),
                expected,
                "{name}"
            );
        }
        let sharp = crate::PitchOptions::new()
            .name("C4")
            .microtone(20)
            .build()
            .unwrap();
        assert_eq!(sharp.full_name(), "C in octave 4 (+20c)");
        let flat = crate::PitchOptions::new()
            .name("E-4")
            .microtone(-33)
            .build()
            .unwrap();
        assert_eq!(flat.full_name(), "E-flat in octave 4 (-33c)");
        let hair = crate::PitchOptions::new()
            .name("E-4")
            .microtone(-0.2)
            .build()
            .unwrap();
        assert_eq!(hair.full_name(), "E-flat in octave 4 (-0c)");
    }
    use crate::defaults::{FloatType, IntegerType};
    use crate::interval::Interval;
    use crate::tuningsystem::TuningSystem;

    use super::{
        Accidental, Microtone, Pitch, convert_harmonic_to_cents, simplify_multiple_enharmonics,
    };

    #[test]
    fn harmonics_match_music21() {
        let cases = [
            ("C2", 1, "C2", None, 65.406),
            ("C2", 2, "C3", None, 130.813),
            ("C2", 3, "G3", Some("(+2c)"), 196.224),
            ("C2", 4, "C4", None, 261.626),
            ("C2", 5, "E4", Some("(-14c)"), 326.973),
            ("C2", 6, "G4", Some("(+2c)"), 392.449),
            ("C2", 7, "A~4", Some("(+19c)"), 457.891),
            ("C2", 8, "C5", None, 523.251),
            ("C2", 9, "D5", Some("(+4c)"), 588.688),
            ("C2", 11, "F~5", Some("(+1c)"), 719.338),
            ("C2", 13, "G#~5", Some("(-9c)"), 850.515),
            ("A2", 3, "E4", Some("(+2c)"), 330.009),
            ("A2", 5, "C#5", Some("(-14c)"), 549.9),
            ("C", 3, "G5", Some("(+2c)"), 784.897),
            ("E-3", 7, "C~6", Some("(+19c)"), 1089.054),
        ];
        for (fundamental, number, name, microtone, hertz) in cases {
            let harmonic = Pitch::from_name(fundamental)
                .unwrap()
                .harmonic(number)
                .unwrap();
            assert_eq!(harmonic.name_with_octave(), name, "{fundamental} {number}");
            assert_eq!(
                harmonic.microtone().map(ToString::to_string).as_deref(),
                microtone,
                "{fundamental} {number}"
            );
            assert!(
                (harmonic.frequency_hz() - hertz).abs() < 5e-4,
                "{fundamental} {number}: {}",
                harmonic.frequency_hz()
            );
            assert_eq!(
                harmonic
                    .fundamental()
                    .map(Pitch::name_with_octave)
                    .as_deref(),
                (number > 1).then_some(fundamental),
                "{fundamental} {number}"
            );
        }
    }

    #[test]
    fn harmonic_from_fundamental_matches_music21() {
        let cases = [
            ("G4", "C2", 6, 2.0, "6thH(-2c)/C2"),
            ("E5", "C2", 10, -14.0, "10thH(+14c)/C2"),
            ("B-4", "C2", 7, -31.0, "7thH(+31c)/C2"),
            ("F#5", "C2", 11, -49.0, "11thH(+49c)/C2"),
            ("C4", "C2", 4, 0.0, "4thH/C2"),
            ("E4", "A2", 3, 2.0, "3rdH(-2c)/A2"),
            ("G#4", "A2", 4, 100.0, "4thH(-100c)/A2"),
            ("A4", "A2", 4, 0.0, "4thH/A2"),
            ("B3", "C2", 4, 100.0, "4thH(-100c)/C2"),
            ("D5", "C2", 9, 4.0, "9thH(-4c)/C2"),
        ];
        for (target, fundamental, number, cents, text) in cases {
            let target_pitch = Pitch::from_name(target).unwrap();
            let fundamental_pitch = Pitch::from_name(fundamental).unwrap();
            let (found, gap) = target_pitch
                .harmonic_from_fundamental(&fundamental_pitch)
                .unwrap();
            assert_eq!(found, number, "{target} over {fundamental}");
            assert!(
                (gap - cents).abs() < 1e-6,
                "{target} over {fundamental}: {gap}"
            );
            assert_eq!(
                target_pitch
                    .harmonic_string(Some(&fundamental_pitch))
                    .unwrap(),
                text
            );
        }
        let c2 = Pitch::from_name("C2").unwrap();
        assert!(
            c2.harmonic_from_fundamental(&Pitch::from_name("C4").unwrap())
                .is_err()
        );
        assert!(c2.harmonic_string(None).is_err());
        assert_eq!(
            c2.harmonic(5).unwrap().harmonic_string(None).unwrap(),
            "5thH/C2"
        );
    }

    #[test]
    fn enharmonic_helpers_match_music21() {
        let names = |pitches: Vec<Pitch>| {
            pitches
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>()
        };
        let cases = [
            (
                "C#4",
                vec!["D-4", "B##3"],
                vec!["D-4"],
                vec!["D-4", "E---4", "B##3"],
            ),
            (
                "E-",
                vec!["F--", "D#"],
                vec!["D#"],
                vec!["F--", "D#", "C###"],
            ),
            ("B#3", vec!["C4"], vec!["C4"], vec!["C4", "A###3"]),
            ("F##4", vec!["G4"], vec!["G4"], vec!["G4", "E###4"]),
            (
                "C4",
                vec!["D--4", "B#3"],
                vec!["B#3"],
                vec!["D--4", "B#3", "A###3"],
            ),
            (
                "G-2",
                vec!["F#2", "E##2"],
                vec!["F#2"],
                vec!["A---2", "F#2", "E##2"],
            ),
            (
                "A4",
                vec!["B--4", "G##4"],
                vec![],
                vec!["B--4", "C---5", "G##4"],
            ),
            (
                "B-4",
                vec!["C--5", "A#4"],
                vec!["A#4"],
                vec!["C--5", "A#4", "G###4"],
            ),
            ("E#5", vec!["F5"], vec!["F5"], vec!["F5", "D###5"]),
            ("D--4", vec!["C4"], vec!["C4"], vec!["C4"]),
        ];
        for (name, limit_two, limit_one, limit_three) in cases {
            let pitch = Pitch::from_name(name).unwrap();
            assert_eq!(names(pitch.all_common_enharmonics(2)), limit_two, "{name}");
            assert_eq!(names(pitch.all_common_enharmonics(1)), limit_one, "{name}");
            assert_eq!(
                names(pitch.all_common_enharmonics(3)),
                limit_three,
                "{name}"
            );
        }

        let enharmonic = |left: &str, right: &str| {
            Pitch::from_name(left)
                .unwrap()
                .is_enharmonic(&Pitch::from_name(right).unwrap())
        };
        assert!(enharmonic("C#4", "D-4"));
        assert!(!enharmonic("C#4", "D-5"));
        assert!(enharmonic("C#", "D-5"));
        assert!(!enharmonic("C#4", "C4"));
        assert!(enharmonic("B#3", "C4"));
        assert!(enharmonic("B#", "C"));
        assert!(enharmonic("C", "C5"));
    }

    #[test]
    fn transposing_to_a_target_matches_music21() {
        let g3 = Pitch::from_name("G3").unwrap();
        let below = [
            ("C4", false, "C3"),
            ("C4", true, "C3"),
            ("C6", false, "C3"),
            ("C6", true, "C3"),
            ("G3", false, "G3"),
            ("C2", false, "C2"),
            ("C2", true, "C3"),
            ("F4", true, "F3"),
            ("G4", true, "G3"),
        ];
        for (source, minimize, expected) in below {
            let moved = Pitch::from_name(source)
                .unwrap()
                .transpose_below_target(&g3, minimize)
                .unwrap();
            assert_eq!(
                moved.name_with_octave(),
                expected,
                "{source} below {minimize}"
            );
        }
        let above = [
            ("C4", false, "C4"),
            ("C1", false, "C4"),
            ("C1", true, "C4"),
            ("C6", false, "C6"),
            ("C6", true, "C4"),
            ("G3", false, "G3"),
            ("F#3", true, "F#4"),
        ];
        for (source, minimize, expected) in above {
            let moved = Pitch::from_name(source)
                .unwrap()
                .transpose_above_target(&g3, minimize)
                .unwrap();
            assert_eq!(
                moved.name_with_octave(),
                expected,
                "{source} above {minimize}"
            );
        }
        assert!(
            Pitch::from_name("C")
                .unwrap()
                .transpose_below_target(&g3, false)
                .is_err()
        );
    }

    #[test]
    fn twelve_tone_and_frequency_construction() {
        assert!(Pitch::from_name("C4").unwrap().is_twelve_tone());
        assert!(Pitch::from_name("C#4").unwrap().is_twelve_tone());
        assert!(!Pitch::from_name("C~4").unwrap().is_twelve_tone());
        assert!(!Pitch::from_name("C`4").unwrap().is_twelve_tone());
        let detuned = Pitch::builder().name("A4").microtone(25).build().unwrap();
        assert!(!detuned.is_twelve_tone());
        assert_eq!(detuned.ps(), 69.25);

        let g4 =
            Pitch::from_frequency(440.0 * (2.0 as FloatType).powf((67.02 - 69.0) / 12.0)).unwrap();
        assert_eq!(g4.name_with_octave(), "G4");
        assert_eq!(g4.microtone().unwrap().to_string(), "(+2c)");
        assert!((g4.ps() - 67.02).abs() < 1e-6);

        let low = Pitch::from_frequency(100.0).unwrap();
        assert_eq!(low.name_with_octave(), "G~2");
        assert_eq!(low.microtone().unwrap().to_string(), "(-15c)");
        assert!(Pitch::from_frequency(0.0).is_err());
        assert_eq!(Pitch::from_name("C4").unwrap().diatonic_note_number(), 29);
        assert_eq!(Pitch::from_name("B#3").unwrap().diatonic_note_number(), 28);
    }

    #[test]
    fn language_names_match_music21() {
        let cases = [
            ("C", "C", "do", "do", "do", "C"),
            ("C#", "Cis", "do diesis", "do dièse", "do sostenido", "C♯"),
            ("D-", "Des", "re bemolle", "ré bémol", "re bemol", "D♭"),
            ("B", "H", "si", "si", "si", "B"),
            ("B-", "B", "si bemolle", "si bémol", "si bemol", "B♭"),
            ("B#", "His", "si diesis", "si dièse", "si sostenido", "B♯"),
            ("E-", "Es", "mi bemolle", "mi bémol", "mi bemol", "E♭"),
            ("A-", "As", "la bemolle", "la bémol", "la bemol", "A♭"),
            (
                "F##",
                "Fisis",
                "fa doppio diesis",
                "fa double dièse",
                "fa doble sostenido",
                "F𝄪",
            ),
            (
                "G--",
                "Geses",
                "sol doppio bemolle",
                "sol double bémol",
                "sol doble bemol",
                "G𝄫",
            ),
            (
                "B--",
                "Heses",
                "si doppio bemolle",
                "si double bémol",
                "si doble bemol",
                "B𝄫",
            ),
            ("F-", "Fes", "fa bemolle", "fa bémol", "fa bemol", "F♭"),
            ("E#", "Eis", "mi diesis", "mi dièse", "mi sostenido", "E♯"),
            (
                "D##",
                "Disis",
                "re doppio diesis",
                "ré double dièse",
                "re doble sostenido",
                "D𝄪",
            ),
            (
                "A####",
                "Aisisisis",
                "la quadruplo diesis",
                "la quadruple dièse",
                "la cuádruple sostenido",
                "A𝄪𝄪",
            ),
            (
                "B----",
                "Heseseses",
                "si quadruplo bemolle",
                "si quadruple bémol",
                "si cuádruple bemol",
                "B𝄫𝄫",
            ),
        ];
        for (name, german, italian, french, spanish, unicode) in cases {
            let pitch = Pitch::from_name(name).unwrap();
            assert_eq!(pitch.german().unwrap(), german, "{name}");
            assert_eq!(pitch.italian().unwrap(), italian, "{name}");
            assert_eq!(pitch.french().unwrap(), french, "{name}");
            assert_eq!(pitch.spanish().unwrap(), spanish, "{name}");
            assert_eq!(pitch.unicode_name(), unicode, "{name}");
        }
        assert_eq!(
            Pitch::from_name("A#4").unwrap().unicode_name_with_octave(),
            "A♯4"
        );
        assert_eq!(Pitch::from_name("E-5").unwrap().german().unwrap(), "Es");
        let quarter_sharp = Pitch::from_name("C~").unwrap();
        assert!(quarter_sharp.german().is_err());
        assert!(quarter_sharp.italian().is_err());
        assert!(quarter_sharp.french().is_err());
        assert!(quarter_sharp.spanish().is_err());
    }

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
        let out = c4.transpose(&m3).unwrap();
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
