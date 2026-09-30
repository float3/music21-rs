//! The scale a figured bass is read in: music21's
//! `figuredBass.realizerScale`.
//!
//! A figure counts scale steps above the bass, so the same `6` means a
//! different note in every key. A [`FiguredBassScale`] is the key it is
//! counted in -- a tonic and one of the five modes music21's realizer knows --
//! and says which notes a bass and its figures stand for.

use std::fmt;
use std::sync::LazyLock;

use crate::defaults::IntegerType;
use crate::error::{Error, Result};
use crate::figuredbass::Notation;
use crate::interval::Interval;
use crate::key::keysignature::{KeySignature, pitch_to_sharps};
use crate::pitch::Pitch;
use crate::scale::{Scale, ScaleType};

/// The modes a figured bass may be realized in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FiguredBassMode {
    /// Major.
    Major,
    /// Natural minor.
    Minor,
    /// Dorian.
    Dorian,
    /// Phrygian.
    Phrygian,
    /// Hypophrygian.
    Hypophrygian,
}

impl FiguredBassMode {
    /// Every mode, in music21's order.
    pub const ALL: [Self; 5] = [
        Self::Major,
        Self::Minor,
        Self::Dorian,
        Self::Phrygian,
        Self::Hypophrygian,
    ];

    /// The mode's name as music21 writes it: `"major"`, `"hypophrygian"`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Dorian => "dorian",
            Self::Phrygian => "phrygian",
            Self::Hypophrygian => "hypophrygian",
        }
    }

    /// The mode music21 names so.
    ///
    /// # Errors
    ///
    /// A name that is none of the five, with music21's own message.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name() == name)
            .ok_or_else(|| Error::FiguredBass(format!("Unsupported scale type-> {name}")))
    }

    fn scale_type(self) -> ScaleType {
        match self {
            Self::Major => ScaleType::Major,
            Self::Minor => ScaleType::Minor,
            Self::Dorian => ScaleType::Dorian,
            Self::Phrygian => ScaleType::Phrygian,
            Self::Hypophrygian => ScaleType::Hypophrygian,
        }
    }
}

impl fmt::Display for FiguredBassMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// The highest pitch [`FiguredBassScale::pitches`] reaches unless told
/// otherwise: `B5`.
static DEFAULT_MAX_PITCH: LazyLock<Pitch> =
    LazyLock::new(|| Pitch::from_name("B5").expect("B5 is a pitch name"));

/// A diminished octave, the span [`FiguredBassScale::sample_pitches`] fills.
static DIMINISHED_OCTAVE: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("d8").expect("d8 is an interval name"));

/// The key a figured bass is counted in: a tonic and a mode.
///
/// ```
/// use music21_rs::Pitch;
/// use music21_rs::figuredbass::Notation;
/// use music21_rs::figuredbass::scale::FiguredBassScale;
///
/// let c_major = FiguredBassScale::default();
/// let first_inversion = Notation::parse("6")?;
/// let names = c_major.pitch_names(&Pitch::from_name("D3")?, &first_inversion)?;
/// assert_eq!(names, ["D", "F", "B"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct FiguredBassScale {
    mode: FiguredBassMode,
    scale: Scale,
    key_signature: KeySignature,
}

impl PartialEq for FiguredBassScale {
    /// Two are equal when they are one mode on one tonic; the key signature
    /// follows from those.
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode && self.scale == other.scale
    }
}

impl Default for FiguredBassScale {
    /// C major, as music21's is when given nothing.
    fn default() -> Self {
        Self::new(
            Pitch::from_name("C").expect("C is a pitch name"),
            FiguredBassMode::Major,
        )
        .expect("C major has a key signature")
    }
}

impl FiguredBassScale {
    /// The scale of `mode` on `tonic`.
    ///
    /// # Errors
    ///
    /// A tonic with no key signature in that mode.
    pub fn new(tonic: Pitch, mode: FiguredBassMode) -> Result<Self> {
        let sharps = pitch_to_sharps(&tonic, Some(mode.name()))?;
        Ok(Self {
            mode,
            scale: Scale::new(mode.scale_type(), tonic),
            key_signature: KeySignature::new(sharps),
        })
    }

    /// The mode it is in.
    pub fn mode(&self) -> FiguredBassMode {
        self.mode
    }

    /// The scale itself: music21's `realizerScale`.
    pub fn scale(&self) -> &Scale {
        &self.scale
    }

    /// The key signature the tonic and mode are written with: music21's
    /// `keySig`.
    pub fn key_signature(&self) -> &KeySignature {
        &self.key_signature
    }

    /// The names of the notes `bass` and `notation` stand for, the bass
    /// first and the rest in the order the figures give them: music21's
    /// `getPitchNames`.
    ///
    /// A bass the scale does not carry is read as the scale's own note on its
    /// letter, spelled as the key signature spells it, which is how a
    /// chromatic bass is still counted from.
    ///
    /// # Errors
    ///
    /// A bass whose letter the scale cannot find, or a figure its modifier
    /// cannot be applied to.
    pub fn pitch_names(&self, bass: &Pitch, notation: &Notation) -> Result<Vec<String>> {
        let degree = match self.scale.degree_of(bass)? {
            Some(degree) => degree,
            None => {
                let mut spelled = bass.clone();
                let from_key = self
                    .key_signature
                    .accidental_by_step(spelled.step().as_char())?;
                if from_key.as_ref() != spelled.written_accidental() {
                    spelled.set_written_accidental(from_key);
                }
                self.scale.degree_of(&spelled)?.ok_or_else(|| {
                    Error::FiguredBass(format!(
                        "{} is on no degree of {} {}",
                        bass.name_with_octave(),
                        self.scale.tonic().name(),
                        self.mode
                    ))
                })?
            }
        };
        let modifiers = notation.modifiers();
        let mut names = Vec::with_capacity(notation.numbers().len() + 1);
        for (number, modifier) in notation.numbers().iter().zip(modifiers) {
            let Some(number) = number else {
                continue;
            };
            let on = (degree as IntegerType + number - 1) % 7;
            let sample = self.scale.pitch_at_degree(on)?;
            names.push(modifier.modify_pitch_name(&sample.name())?);
        }
        names.push(bass.name());
        names.reverse();
        Ok(names)
    }

    /// Every note `bass` and `notation` stand for from the bass up to, but
    /// not including, the octave above it: the closest the chord can be
    /// voiced. music21's `getSamplePitches`.
    ///
    /// # Errors
    ///
    /// As [`FiguredBassScale::pitch_names`].
    pub fn sample_pitches(&self, bass: &Pitch, notation: &Notation) -> Result<Vec<Pitch>> {
        let top = bass.transpose(&DIMINISHED_OCTAVE)?;
        self.pitches(bass, notation, Some(&top))
    }

    /// Every note `bass` and `notation` stand for, in every octave from the
    /// bass up to `max_pitch` (`B5` unless given), both included, lowest
    /// first: music21's `getPitches`.
    ///
    /// # Errors
    ///
    /// As [`FiguredBassScale::pitch_names`].
    pub fn pitches(
        &self,
        bass: &Pitch,
        notation: &Notation,
        max_pitch: Option<&Pitch>,
    ) -> Result<Vec<Pitch>> {
        let max_pitch = max_pitch.unwrap_or(&DEFAULT_MAX_PITCH);
        let names = self.pitch_names(bass, notation)?;
        let mut pitches = Vec::new();
        for name in &names {
            for octave in 0..=max_pitch.implicit_octave() {
                let pitch = Pitch::from_name(format!("{name}{octave}"))?;
                if pitch.ps() >= bass.ps() && pitch.ps() <= max_pitch.ps() {
                    pitches.push(pitch);
                }
            }
        }
        pitches.sort_by(|a, b| a.ps().total_cmp(&b.ps()));
        Ok(pitches)
    }
}
