mod display;
mod enharmonic;
mod harmonics;
mod names;

pub use display::AccidentalDisplayOptions;
pub use enharmonic::{CriterionFunction, dissonance_score, simplify_multiple_enharmonics};
pub use names::{CHROMATIC_PITCH_CLASS_NAMES, pitch_class_name};

use harmonics::*;

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
#[must_use]
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

        if let Some(PitchName::Number(number)) = &name
            && !number.is_finite()
        {
            return Err(Error::Pitch(format!(
                "a pitch-space number must be finite, got {number}"
            )));
        }
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
            if !ps.is_finite() {
                return Err(Error::Pitch(format!(
                    "a pitch-space number must be finite, got {ps}"
                )));
            }
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
                    return Err(Error::Value(format!(
                        "Cannot have octave given before pitch name in '{usr_str}'."
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

    /// music21's `spellingIsInferred`: whether the crate chose the spelling
    /// rather than being told it. A pitch built from a number, a MIDI value,
    /// a pitch class or a frequency has an inferred spelling, and only such
    /// a pitch is respelled by a transposition.
    pub fn spelling_is_inferred(&self) -> bool {
        self.spelling_is_inferred
    }

    /// Says whether the spelling was chosen or given.
    pub fn set_spelling_is_inferred(&mut self, inferred: bool) {
        self.spelling_is_inferred = inferred;
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

    /// Returns the octave, or music21's default of 4 when none is set.
    ///
    /// This is what music21's `.octave` answers, which is always an `int`.
    /// [`Self::octave`] keeps the `Option`, which is music21's own `_octave`
    /// and says strictly more; [`Self::octave_is_implicit`] tells the two
    /// apart.
    pub fn implicit_octave(&self) -> IntegerType {
        self.octave.unwrap_or(PITCH_OCTAVE as IntegerType)
    }

    /// Whether this pitch was never given an octave, so it stands for its
    /// pitch class in any octave: music21's `octaveIsImplicit`.
    ///
    /// Such a pitch prints without an octave number and reports the default
    /// octave from [`Self::implicit_octave`].
    #[must_use]
    pub fn octave_is_implicit(&self) -> bool {
        self.octave.is_none()
    }

    /// Makes the octave implicit or explicit: the setter of music21's
    /// `octaveIsImplicit`.
    ///
    /// Making it explicit puts the pitch in the default octave, as music21
    /// does; making it implicit takes the octave away. Setting it to what it
    /// already is does nothing.
    pub fn set_octave_is_implicit(&mut self, implicit: bool) {
        if implicit == self.octave.is_none() {
            return;
        }
        self.octave = if implicit {
            None
        } else {
            Some(PITCH_OCTAVE as IntegerType)
        };
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

    /// Builds a pitch from a frequency in hertz, spelled in twelve-tone equal
    /// temperament at A4 = 440 with any remainder as a microtone.
    pub fn from_frequency(hertz: FloatType) -> Result<Self> {
        if !hertz.is_finite() || hertz <= 0.0 {
            return Err(Error::Pitch(format!(
                "frequency must be a finite number greater than zero, got {hertz}"
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

        // A quarter of a semitone exactly is *not* a quarter tone: music21
        // writes `F` plus twenty-five cents rather than an F half-sharp
        // twenty-five cents flat, and the bound it draws is exclusive.
        let alter = if round_to_digits(micro, 1) == 0.5 || (0.25 < micro && micro < 0.75) {
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

#[cfg(test)]
mod tests {
    /// A number that is not a number spells no pitch. Left to the conversion
    /// it became a C, because the cast that reads a step off pitch space
    /// answers nought for anything it cannot represent.
    #[test]
    fn a_pitch_space_number_that_is_not_finite_is_refused() {
        use crate::pitch::Pitch;

        for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Pitch::from_pitch_space(number).is_err());
            assert!(Pitch::from_number(number).is_err());
            assert!(Pitch::builder().ps(number).build().is_err());
        }
        assert_eq!(Pitch::from_pitch_space(61.0).unwrap().name(), "C#");
    }

    #[test]
    fn a_frequency_that_is_not_finite_is_refused() {
        use crate::pitch::Pitch;

        assert!(Pitch::from_frequency(f64::INFINITY).is_err());
        assert!(Pitch::from_frequency(f64::NAN).is_err());
        assert!(Pitch::from_frequency(0.0).is_err());
        assert_eq!(
            Pitch::from_frequency(440.0).unwrap().name_with_octave(),
            "A4"
        );
    }

    #[test]
    fn a_pitch_is_built_from_a_step_a_number_or_an_owned_name() {
        use crate::pitch::{Pitch, PitchName, pitch_class_name};

        assert_eq!(Pitch::from_step('d').unwrap().name(), "D");
        assert!(Pitch::from_step('h').is_err());
        assert_eq!(
            Pitch::try_from("E-4".to_string())
                .unwrap()
                .name_with_octave(),
            "E-4"
        );
        assert!(matches!(PitchName::from(60), PitchName::Number(n) if n == 60.0));
        assert!(matches!(PitchName::from(61.5), PitchName::Number(n) if n == 61.5));
        assert_eq!(pitch_class_name(1), "D-");
        assert_eq!(pitch_class_name(13), "D-");
        assert_eq!(pitch_class_name(10), "B-");
    }

    #[test]
    fn spelling_is_inferred_for_a_pitch_built_from_a_number_and_can_be_unsaid() {
        use crate::pitch::Pitch;

        let mut pitch = Pitch::from_midi(61).unwrap();
        assert!(pitch.spelling_is_inferred());
        pitch.set_spelling_is_inferred(false);
        assert!(!pitch.spelling_is_inferred());
        assert!(!Pitch::from_name("C#").unwrap().spelling_is_inferred());

        let mut raised = Pitch::from_name("C#4").unwrap();
        raised.get_higher_enharmonic_in_place().unwrap();
        assert_eq!(raised.name_with_octave(), "D-4");
    }

    #[test]
    fn a_pitch_knows_whether_a_key_signature_already_writes_it() {
        use crate::pitch::Pitch;

        let altered = [
            Pitch::from_name("F#").unwrap(),
            Pitch::from_name("C#").unwrap(),
        ];
        assert!(
            Pitch::from_name("F#4")
                .unwrap()
                .name_in_key_signature(&altered)
        );
        assert!(
            !Pitch::from_name("F4")
                .unwrap()
                .name_in_key_signature(&altered)
        );
        assert!(
            !Pitch::from_name("F-4")
                .unwrap()
                .name_in_key_signature(&altered)
        );
        assert!(
            Pitch::from_name("F4")
                .unwrap()
                .step_in_key_signature(&altered)
        );
        assert!(
            !Pitch::from_name("G4")
                .unwrap()
                .step_in_key_signature(&altered)
        );
    }

    /// music21's `updateAccidentalDisplay` on its simplest shapes: a repeat
    /// of a written accidental in the bar is not written again, a natural
    /// after an accidental in the bar is a caution, and a note the key
    /// signature already alters needs nothing.
    #[test]
    fn accidental_display_follows_the_pitches_before_it() {
        use crate::pitch::{AccidentalDisplayOptions, Pitch};

        let first = Pitch::from_name("F#4").unwrap();
        let mut repeat = Pitch::from_name("F#4").unwrap();
        let past = [first.clone()];
        repeat.update_accidental_display(&AccidentalDisplayOptions {
            pitch_past: &past,
            ..AccidentalDisplayOptions::default()
        });
        assert_eq!(repeat.accidental().display_status(), Some(false));

        let mut natural = Pitch::from_name("F4").unwrap();
        natural.update_accidental_display(&AccidentalDisplayOptions {
            pitch_past: &past,
            ..AccidentalDisplayOptions::default()
        });
        assert!(natural.has_accidental());
        assert_eq!(natural.accidental().display_status(), Some(true));

        let mut in_key = Pitch::from_name("F#4").unwrap();
        in_key.update_accidental_display(&AccidentalDisplayOptions {
            altered_pitches: &[Pitch::from_name("F#").unwrap()],
            ..AccidentalDisplayOptions::default()
        });
        assert_eq!(in_key.accidental().display_status(), Some(false));

        let mut fresh = Pitch::from_name("B-4").unwrap();
        fresh.update_accidental_display(&AccidentalDisplayOptions::default());
        assert_eq!(fresh.accidental().display_status(), Some(true));
    }

    /// music21's `.octave` always answers a number, and `.octaveIsImplicit`
    /// says whether one was ever given.
    #[test]
    fn an_octave_is_implicit_until_one_is_given() {
        use crate::pitch::Pitch;

        let mut anywhere = Pitch::from_name("G#").unwrap();
        assert!(anywhere.octave_is_implicit());
        assert_eq!(anywhere.octave(), None);
        assert_eq!(anywhere.implicit_octave(), 4);

        let somewhere = Pitch::from_name("E-6").unwrap();
        assert!(!somewhere.octave_is_implicit());
        assert_eq!(somewhere.octave(), Some(6));
        assert_eq!(somewhere.implicit_octave(), 6);

        // Making it explicit puts the pitch in the default octave; making it
        // implicit again takes the octave away.
        anywhere.set_octave_is_implicit(false);
        assert!(!anywhere.octave_is_implicit());
        assert_eq!(anywhere.octave(), Some(4));
        anywhere.set_octave_is_implicit(true);
        assert_eq!(anywhere.octave(), None);

        // Setting it to what it already is leaves the octave alone.
        let mut high = Pitch::from_name("C7").unwrap();
        high.set_octave_is_implicit(false);
        assert_eq!(high.octave(), Some(7));
    }

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
