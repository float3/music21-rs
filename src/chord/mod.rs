/// Guitar tuning and fingering helpers.
pub mod guitar;
pub(crate) mod root;
pub mod tables;

use crate::common::numbertools::ORDINALS;
use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::duration::Duration;
use crate::error::Error;
use crate::error::Result;
use crate::interval::{Interval, PitchOrNote};
use crate::key::Key;
use crate::key::keysignature::KeySignature;
use crate::notation::{Beams, Lyric, Notehead, StemDirection, Tie};
use crate::note::{IntoNote, Note};
use crate::pitch::{Pitch, PitchClass, PitchClassSpecifier};
use crate::volume::Volume;

pub use guitar::{GuitarFingering, GuitarStringFingering, GuitarTuning, GuitarTuningString};

use num::integer::{gcd, lcm};
use std::fmt::{Display, Formatter};
use std::ops::Index;
use std::str::FromStr;
use std::sync::LazyLock;

mod names;
mod notation;
mod quality;
mod resolution;
mod setclass;

pub use names::KnownChordType;
pub use quality::TriadQuality;
pub use resolution::ChordResolutionSuggestion;
pub use setclass::{ChordTableAddress, format_vector_string};

use quality::*;
use resolution::*;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A collection of notes analyzed as one vertical sonority.
///
/// `Chord` accepts several note-like inputs, including whitespace-separated
/// pitch names, slices of pitches or notes, MIDI pitch numbers, vectors, and
/// `None` for an empty chord.
#[must_use]
pub struct Chord {
    notes: Vec<Note>,
    duration: Option<Duration>,
    /// A volume for the chord as a whole, used when its notes carry none.
    #[cfg_attr(feature = "serde", serde(default))]
    volume: Option<Volume>,
    /// A colour for the chord as a whole, used when its notes carry none.
    #[cfg_attr(feature = "serde", serde(default))]
    color: Option<String>,
    /// The notation the chord carries in its own right, apart from its
    /// notes': music21 keeps these on `NotRest`, which a chord is, and a
    /// chord's are read independently of the notes inside it.
    #[cfg_attr(feature = "serde", serde(default))]
    notehead: Notehead,
    #[cfg_attr(feature = "serde", serde(default))]
    notehead_fill: Option<bool>,
    #[cfg_attr(feature = "serde", serde(default))]
    notehead_parenthesis: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    stem_direction: StemDirection,
    /// The beams joining the chord's flags to its neighbours'.
    #[cfg_attr(feature = "serde", serde(default))]
    beams: Beams,
    #[cfg_attr(feature = "serde", serde(skip))]
    from_integer_pitches: bool,
    /// A root the caller decided on, which wins over the one the pitches
    /// imply: music21's overridden root, for chords spelled oddly or with
    /// added notes.
    #[cfg_attr(feature = "serde", serde(default))]
    root_override: Option<Pitch>,
    /// A bass the caller decided on, which wins over the lowest pitch.
    #[cfg_attr(feature = "serde", serde(default))]
    bass_override: Option<Pitch>,
}

use crate::interval::constants::PERFECT_FIFTH_UP as PERFECT_FIFTH;

impl Index<usize> for Chord {
    type Output = Note;

    fn index(&self, index: usize) -> &Self::Output {
        &self.notes[index]
    }
}

impl IntoIterator for Chord {
    type Item = Note;
    type IntoIter = std::vec::IntoIter<Note>;

    fn into_iter(self) -> Self::IntoIter {
        self.notes.into_iter()
    }
}

impl<'a> IntoIterator for &'a Chord {
    type Item = &'a Note;
    type IntoIter = std::slice::Iter<'a, Note>;

    fn into_iter(self) -> Self::IntoIter {
        self.notes.iter()
    }
}

impl<'a> IntoIterator for &'a mut Chord {
    type Item = &'a mut Note;
    type IntoIter = std::slice::IterMut<'a, Note>;

    fn into_iter(self) -> Self::IntoIter {
        self.notes.iter_mut()
    }
}

impl FromStr for Chord {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Chord {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<String> for Chord {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[Pitch]> for Chord {
    type Error = Error;

    fn try_from(value: &[Pitch]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[Note]> for Chord {
    type Error = Error;

    fn try_from(value: &[Note]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[IntegerType]> for Chord {
    type Error = Error;

    fn try_from(value: &[IntegerType]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[&str]> for Chord {
    type Error = Error;

    fn try_from(value: &[&str]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&[String]> for Chord {
    type Error = Error;

    fn try_from(value: &[String]) -> Result<Self> {
        Self::new(value)
    }
}

impl Display for Chord {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pitched_common_name())
    }
}

impl Chord {
    /// Builds a chord from any supported note collection.
    ///
    /// Empty inputs are valid: pass `""`, an empty vector or slice, or
    /// `Option::<&str>::None` to construct an empty chord.
    pub fn new<T>(notes: T) -> Result<Self>
    where
        T: IntoNotes,
    {
        Ok(Self {
            notes: notes.try_into_notes()?.into_iter().collect(),
            duration: None,
            from_integer_pitches: T::FROM_INTEGER_PITCHES,
            volume: None,
            color: None,
            notehead: Notehead::default(),
            notehead_fill: None,
            notehead_parenthesis: false,
            stem_direction: StemDirection::default(),
            beams: Beams::default(),
            root_override: None,
            bass_override: None,
        })
    }

    /// Builds an empty chord.
    pub fn empty() -> Self {
        Self {
            notes: Vec::new(),
            duration: None,
            from_integer_pitches: false,
            volume: None,
            color: None,
            notehead: Notehead::default(),
            notehead_fill: None,
            notehead_parenthesis: false,
            stem_direction: StemDirection::default(),
            beams: Beams::default(),
            root_override: None,
            bass_override: None,
        }
    }

    /// Returns a suggested standard-tuning guitar fingering.
    ///
    /// The fingering is a compact voicing on six-string guitar in
    /// E2-A2-D3-G3-B3-E4 tuning. It prefers shapes that cover all chord pitches,
    /// place the
    /// root in the bass when possible, avoid internal muted strings, and stay
    /// within a small fret span.
    pub fn guitar_fingering(&self) -> Option<GuitarFingering> {
        guitar::suggested_guitar_fingering(self)
    }

    /// Returns a suggested guitar fingering for the supplied tuning.
    ///
    /// The tuning strings must be ordered from low to high. Fingering generation
    /// uses exact pitch spaces, so both the chord pitches and open-string
    /// octaves affect the result.
    pub fn guitar_fingering_with_tuning(&self, tuning: &GuitarTuning) -> Option<GuitarFingering> {
        guitar::suggested_guitar_fingering_with_tuning(self, tuning)
    }

    /// Returns the distinct pitch classes in ascending order.
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.ordered_pitch_classes()
    }

    /// Maps this chord's pitch classes to a reduced integer polyrhythm ratio.
    ///
    /// Pitch classes are measured from the inferred root when possible, or
    /// from the lowest pitch class otherwise. Each semitone offset is mapped
    /// to a compact just-intonation ratio and reduced to whole-number
    /// components.
    pub fn polyrhythm_components(&self) -> Vec<UnsignedIntegerType> {
        let pitch_classes = self.ordered_pitch_classes();
        if pitch_classes.is_empty() {
            return vec![1];
        }

        let root_pc = self
            .find_root_pitch()
            .map(root::pitch_class)
            .filter(|root_pc| pitch_classes.contains(root_pc))
            .unwrap_or(pitch_classes[0]);
        let mut offsets = pitch_classes
            .iter()
            .map(|pc| (*pc + 12 - root_pc) % 12)
            .collect::<Vec<_>>();
        offsets.sort_unstable();

        let ratios = offsets
            .into_iter()
            .map(Self::just_ratio_for_semitone)
            .collect::<Vec<_>>();
        let common_denominator = ratios
            .iter()
            .fold(1, |acc, (_, denominator)| lcm(acc, *denominator));
        let integers = ratios
            .iter()
            .map(|(numerator, denominator)| numerator * (common_denominator / denominator))
            .collect::<Vec<_>>();
        let divisor = integers.iter().copied().reduce(gcd).unwrap_or(1).max(1);

        integers.into_iter().map(|value| value / divisor).collect()
    }

    /// Returns [`Self::polyrhythm_components`] formatted as `a:b:c`.
    pub fn polyrhythm_ratio_string(&self) -> String {
        self.polyrhythm_components()
            .into_iter()
            .map(|component| component.to_string())
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Adds pitches or notes to the end of the chord, as music21's
    /// `Chord.add` does. The chord is not re-sorted: the new notes sit after
    /// the ones already there.
    pub fn add<T>(&mut self, notes: T) -> Result<()>
    where
        T: IntoNotes,
    {
        self.notes.extend(notes.try_into_notes()?);
        Ok(())
    }

    /// Removes the first note whose pitch equals this one, as music21's
    /// `Chord.remove` does, and errors when the chord has no such pitch.
    pub fn remove(&mut self, pitch: &Pitch) -> Result<()> {
        let found = self.notes.iter().position(|note| &note.pitch == pitch);
        match found {
            Some(index) => {
                let _ = self.notes.remove(index);
                Ok(())
            }
            None => Err(Error::Chord("Chord.remove(x), x not in chord".to_string())),
        }
    }

    /// Removes the first note whose written pitch name matches, as music21's
    /// `Chord.remove` does with a string.
    pub fn remove_named(&mut self, name_with_octave: &str) -> Result<()> {
        let found = self
            .notes
            .iter()
            .position(|note| note.pitch.name_with_octave() == name_with_octave);
        match found {
            Some(index) => {
                let _ = self.notes.remove(index);
                Ok(())
            }
            None => Err(Error::Chord("Chord.remove(x), x not in chord".to_string())),
        }
    }

    /// Returns cloned pitches for every note in the chord, in input order.
    pub fn pitches(&self) -> Vec<Pitch> {
        self.notes.iter().map(|note| note.pitch.clone()).collect()
    }

    /// Borrows the pitches in input order.
    ///
    /// [`Chord::pitches`] is music21's accessor and clones each pitch; an
    /// `Accidental` owns two `String`s, so that is two allocations a pitch a
    /// caller that only reads them does not need.
    pub fn iter_pitches(&self) -> impl Iterator<Item = &Pitch> + '_ {
        self.notes.iter().map(|note| &note.pitch)
    }

    /// Returns the notes in input order.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// The notes in input order, for editing in place. This is how a note's
    /// notation is changed through the chord: `chord.notes_mut()[1]
    /// .set_notehead(Notehead::Diamond)`.
    pub fn notes_mut(&mut self) -> &mut [Note] {
        &mut self.notes
    }

    /// How many notes the chord holds: what `len(chord)` answers upstream.
    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Whether the chord holds no notes at all.
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// The first note whose pitch equals this one: music21's per-note
    /// accessors take a pitch this way.
    pub fn note_for_pitch(&self, pitch: &Pitch) -> Option<&Note> {
        self.notes.iter().find(|note| &note.pitch == pitch)
    }

    /// The first note whose pitch equals this one, for editing in place.
    pub fn note_for_pitch_mut(&mut self, pitch: &Pitch) -> Option<&mut Note> {
        self.notes.iter_mut().find(|note| &note.pitch == pitch)
    }

    /// Returns the chord duration when one has been assigned.
    pub fn duration(&self) -> Option<&Duration> {
        self.duration.as_ref()
    }

    /// Assigns a duration to the chord.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = Some(duration);
    }

    /// Returns a copy of this chord with the supplied duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.set_duration(duration);
        self
    }

    /// Returns the inversion number the way music21 finds it: root position
    /// is `0`, and the number climbs with the chord step the bass sits on,
    /// so a bass on the seventh is `3`. `None` only for an empty chord.
    pub fn inversion(&self) -> Option<u8> {
        let bass_to_root = diatonic_steps_above(self.bass()?, self.root()?);
        Some([0, 3, 6, 2, 5, 1, 4][usize::from(bass_to_root - 1)])
    }

    /// The inversion the way [`Self::inversion`] counts it, but measured from a
    /// root the caller has decided on rather than the one the chord infers.
    pub(crate) fn inversion_with_root(&self, root: &Pitch) -> Option<u8> {
        let bass_to_root = diatonic_steps_above(self.bass()?, root);
        Some([0, 3, 6, 2, 5, 1, 4][usize::from(bass_to_root - 1)])
    }

    /// Returns a human-readable inversion label.
    ///
    /// Returns `None` whenever [`Self::inversion`] returns `None`.
    /// The figured-bass number music21's `inversionName` answers: `53`, `6`
    /// and `64` for a triad, `7`, `65`, `43` and `42` for a seventh. `None`
    /// when the chord has no inversion, and an error when it is neither a
    /// triad nor a seventh, as music21 raises there. For the words, see
    /// [`Self::inversion_text`].
    pub fn inversion_name(&self) -> Result<Option<IntegerType>> {
        let Some(inversion) = self.inversion() else {
            return Ok(None);
        };
        let inversion = usize::from(inversion);
        if self.is_seventh() || self.seventh().is_some() {
            return [7, 65, 43, 42]
                .get(inversion)
                .copied()
                .map(Some)
                .ok_or_else(|| {
                    Error::Chord(format!("Not a normal inversion for a seventh: {inversion}"))
                });
        }
        if self.is_triad() {
            return [53, 6, 64]
                .get(inversion)
                .copied()
                .map(Some)
                .ok_or_else(|| {
                    Error::Chord(format!("Not a normal inversion for a triad: {inversion}"))
                });
        }
        Err(Error::Chord(
            "Not a triad or Seventh, cannot determine inversion.".to_string(),
        ))
    }

    /// Returns a copy with simplified enharmonic spellings.
    ///
    /// This mirrors music21's explicit enharmonic simplification workflow:
    /// construction stays side-effect free, and callers can request simpler
    /// spellings with an optional key-signature context.
    pub fn simplify_enharmonics(&self, key_context: Option<KeySignature>) -> Result<Self> {
        let mut chord = self.clone();
        chord.simplify_enharmonics_in_place(key_context)?;
        Ok(chord)
    }

    /// Simplifies this chord's pitch spellings in place.
    pub fn simplify_enharmonics_in_place(
        &mut self,
        key_context: Option<KeySignature>,
    ) -> Result<()> {
        match crate::pitch::simplify_multiple_enharmonics(&self.pitches(), None, key_context) {
            Ok(pitches) => {
                for (i, pitch) in pitches.iter().enumerate() {
                    if let Some(note) = self.notes.get_mut(i) {
                        note.pitch = pitch.clone();
                    }
                }
                Ok(())
            }
            Err(err) => Err(Error::Chord(format!(
                "simplifying multiple enharmonics failed because of {err}"
            ))),
        }
    }

    /// Returns the root, found the way music21's `Chord.root` finds it.
    pub fn root(&self) -> Option<&Pitch> {
        self.root_override
            .as_ref()
            .or_else(|| self.find_root_pitch())
    }

    /// The root the pitches imply, ignoring any override.
    pub fn found_root(&self) -> Option<&Pitch> {
        self.find_root_pitch()
    }

    /// Fixes the root the chord reports, or clears the override with `None`:
    /// music21's `root(newroot)`. The pitch need not be in the chord, which
    /// is the point of it for oddly spelled or added-note chords.
    pub fn set_root(&mut self, root: Option<Pitch>) {
        self.root_override = root;
    }

    /// Returns the lowest pitch, or the bass a caller fixed with
    /// [`Self::set_bass`].
    pub fn bass(&self) -> Option<&Pitch> {
        self.bass_override.as_ref().or_else(|| self.bass_pitch())
    }

    /// The lowest pitch, ignoring any override.
    pub fn found_bass(&self) -> Option<&Pitch> {
        self.bass_pitch()
    }

    /// The bass a caller fixed with [`Self::set_bass`], and nothing when
    /// none was.
    pub fn overridden_bass(&self) -> Option<&Pitch> {
        self.bass_override.as_ref()
    }

    /// The root a caller fixed with [`Self::set_root`], and nothing when
    /// none was.
    pub fn overridden_root(&self) -> Option<&Pitch> {
        self.root_override.as_ref()
    }

    /// Fixes the bass the chord reports, or clears the override with `None`:
    /// music21's `bass(newbass)`. A pitch the chord does not already carry is
    /// added below the others, as music21 adds it.
    pub fn set_bass(&mut self, bass: Option<Pitch>) {
        let Some(bass) = bass else {
            self.bass_override = None;
            return;
        };
        let known = self
            .notes
            .iter()
            .any(|note| note.pitch.name_with_octave() == bass.name_with_octave());
        if !known {
            self.notes.insert(0, Note::from_pitch(bass.clone()));
        }
        self.bass_override = Some(bass);
    }

    /// Rearranges the chord so it stands in the given inversion, raising the
    /// bass by octaves until it does: music21's `inversion(newInversion)`.
    /// An inversion the chord cannot reach is an error.
    pub fn set_inversion(&mut self, inversion: u8) -> Result<()> {
        self.bass_override = None;
        let mut runs = self.notes.len() + 2;
        while self.inversion() != Some(inversion) {
            if runs == 0 {
                return Err(Error::Chord(
                    "Could not invert chord: inversion may not exist".to_string(),
                ));
            }
            runs -= 1;
            let highest_ps = self
                .notes
                .iter()
                .map(|note| note.pitch.ps())
                .fold(FloatType::NEG_INFINITY, FloatType::max);
            let Some(bass_name) = self.bass().map(Pitch::name_with_octave) else {
                return Err(Error::Chord(
                    "Could not invert chord: inversion may not exist".to_string(),
                ));
            };
            let Some(index) = self
                .notes
                .iter()
                .position(|note| note.pitch.name_with_octave() == bass_name)
            else {
                return Err(Error::Chord(
                    "Could not invert chord: inversion may not exist".to_string(),
                ));
            };
            while self.notes[index].pitch.ps() < highest_ps {
                let octave = self.notes[index].pitch.implicit_octave();
                self.notes[index].pitch.octave_setter(Some(octave + 1));
            }
        }
        *self = self.sort_ascending();
        Ok(())
    }

    /// Returns a copy with every pitch brought within an octave above the
    /// bass, duplicates removed and the notes sorted, as music21's
    /// `closedPosition` does. `force_octave` moves the bass to that octave
    /// first, carrying the rest of the chord with it.
    pub fn closed_position(
        &self,
        force_octave: Option<IntegerType>,
        leave_redundant_pitches: bool,
    ) -> Self {
        let mut chord = self.clone();
        let Some(bass_index) = chord.bass_index() else {
            return chord;
        };
        let implicit_octave = crate::defaults::PITCH_OCTAVE as IntegerType;
        if let Some(force_octave) = force_octave {
            let bass_octave = chord.notes[bass_index]
                .pitch
                .octave()
                .unwrap_or(implicit_octave);
            let shift = force_octave - bass_octave;
            for note in &mut chord.notes {
                let octave = note.pitch.octave().unwrap_or(implicit_octave);
                note.pitch.octave_setter(Some(octave + shift));
            }
        }
        let bass_ps = chord.notes[bass_index].pitch.ps();
        let bass_number = root::diatonic_note_number(&chord.notes[bass_index].pitch);
        for note in &mut chord.notes {
            let mut octave = note.pitch.octave().unwrap_or(implicit_octave);
            note.pitch.octave_setter(Some(octave));
            while note.pitch.ps() >= bass_ps + 12.0 {
                octave -= 1;
                note.pitch.octave_setter(Some(octave));
            }
            if root::diatonic_note_number(&note.pitch) < bass_number {
                note.pitch.octave_setter(Some(octave + 1));
            }
        }
        if !leave_redundant_pitches {
            chord.retain_first_by(spelling_and_octave);
        }
        chord.sort_ascending_in_place();
        chord
    }

    /// The chord with duplicate pitches removed, and the pitches that went:
    /// music21's `removeRedundantPitches`, which hands back what it dropped.
    pub fn remove_redundant_pitches_reporting(&self) -> (Self, Vec<Pitch>) {
        self.reduced_reporting(spelling_and_octave)
    }

    /// The chord with pitches of the same name removed, and the ones that
    /// went: music21's `removeRedundantPitchNames`.
    pub fn remove_redundant_pitch_names_reporting(&self) -> (Self, Vec<Pitch>) {
        self.reduced_reporting(Pitch::name)
    }

    /// The chord with pitches of the same pitch class removed, and the ones
    /// that went: music21's `removeRedundantPitchClasses`.
    pub fn remove_redundant_pitch_classes_reporting(&self) -> (Self, Vec<Pitch>) {
        self.reduced_reporting(root::pitch_class)
    }

    fn reduced_reporting<K: PartialEq>(&self, key: impl Fn(&Pitch) -> K) -> (Self, Vec<Pitch>) {
        let mut kept = self.clone();
        let mut seen: Vec<K> = Vec::with_capacity(self.notes.len());
        let mut removed = Vec::new();
        kept.notes.retain(|note| {
            let candidate = key(&note.pitch);
            if seen.contains(&candidate) {
                removed.push(note.pitch.clone());
                false
            } else {
                seen.push(candidate);
                true
            }
        });
        (kept, removed)
    }

    /// Returns a copy keeping the first of every pitch that appears more than
    /// once with the same name and octave. `B-1` (B-flat, octave 1) and `B`
    /// in octave -1 print alike but are different pitches, and so are `C`
    /// with no octave of its own and `C4`.
    pub fn remove_redundant_pitches(&self) -> Self {
        let mut chord = self.clone();
        chord.retain_first_by(spelling_and_octave);
        chord
    }

    /// Returns a copy keeping the first of every pitch name, regardless of
    /// octave.
    pub fn remove_redundant_pitch_names(&self) -> Self {
        let mut chord = self.clone();
        chord.retain_first_by(Pitch::name);
        chord
    }

    /// Returns a copy keeping the first of every pitch class, so `C#` and
    /// `D-` count as one.
    pub fn remove_redundant_pitch_classes(&self) -> Self {
        let mut chord = self.clone();
        chord.retain_first_by(root::pitch_class);
        chord
    }

    /// Returns a copy sorted by staff position and then pitch space, so
    /// `F##` sorts below `G-`.
    pub fn sort_ascending(&self) -> Self {
        let mut chord = self.clone();
        chord.sort_ascending_in_place();
        chord
    }

    pub(crate) fn unique_pitch_names(&self) -> std::collections::BTreeSet<String> {
        self.pitch_refs().map(Pitch::name).collect()
    }

    fn bass_index(&self) -> Option<usize> {
        self.notes
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                left.pitch
                    .ps()
                    .partial_cmp(&right.pitch.ps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)
    }

    fn retain_first_by<K: PartialEq>(&mut self, key: impl Fn(&Pitch) -> K) {
        let mut seen: Vec<K> = Vec::with_capacity(self.notes.len());
        self.notes.retain(|note| {
            let candidate = key(&note.pitch);
            if seen.contains(&candidate) {
                false
            } else {
                seen.push(candidate);
                true
            }
        });
    }

    fn sort_ascending_in_place(&mut self) {
        self.notes.sort_by(|left, right| {
            root::diatonic_note_number(&left.pitch)
                .cmp(&root::diatonic_note_number(&right.pitch))
                .then_with(|| {
                    left.pitch
                        .ps()
                        .partial_cmp(&right.pitch.ps())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });
    }

    /// Returns a copy with every note transposed by the interval.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        let mut chord = self.clone();
        for note in &mut chord.notes {
            note.pitch = interval.transpose_pitch(&note.pitch)?;
        }
        // A root or bass a caller fixed moves with the chord, as music21
        // moves it: a chord symbol carries its root as an override, and one
        // transposed up a semitone is a chord on the note above.
        if let Some(root) = &chord.root_override {
            chord.root_override = Some(interval.transpose_pitch(root)?);
        }
        if let Some(bass) = &chord.bass_override {
            chord.bass_override = Some(interval.transpose_pitch(bass)?);
        }
        Ok(chord)
    }

    /// Returns each pitch's degree in `scale` with the accidental that
    /// separates it from the scale's spelling, as music21's `scaleDegrees`
    /// does: `C E- G` in C major is `(1, None), (3, flat), (5, None)`. A pitch
    /// whose letter the scale lacks reports `(None, None)`.
    pub fn scale_degrees(
        &self,
        scale: &crate::scale::Scale,
    ) -> Result<Vec<(Option<usize>, Option<crate::pitch::Accidental>)>> {
        self.pitch_refs()
            .map(|pitch| match scale.degree_and_accidental_of(pitch) {
                Ok((degree, accidental)) => Ok((Some(degree), accidental)),
                Err(Error::Scale(_)) => Ok((None, None)),
                Err(error) => Err(error),
            })
            .collect()
    }

    fn bass_pitch(&self) -> Option<&Pitch> {
        root::bass_pitch(self.pitch_refs())
    }

    fn find_root_pitch(&self) -> Option<&Pitch> {
        root::find_root_pitch(self.pitch_refs())
    }

    fn pitch_refs(&self) -> impl Iterator<Item = &Pitch> {
        self.notes.iter().map(|note| &note.pitch)
    }

    fn just_ratio_for_semitone(offset: u8) -> (UnsignedIntegerType, UnsignedIntegerType) {
        const RATIOS: [(UnsignedIntegerType, UnsignedIntegerType); 12] = [
            (1, 1),
            (16, 15),
            (9, 8),
            (6, 5),
            (5, 4),
            (4, 3),
            (7, 5),
            (3, 2),
            (25, 16),
            (5, 3),
            (7, 4),
            (15, 8),
        ];
        RATIOS[offset as usize % 12]
    }
}

/// Tries to convert a supported chord input into notes.
///
/// Implementations are provided for strings, slices, vectors, other chords,
/// integer pitch inputs, and `Option<T>`. `None` converts to an empty note list.
/// String and integer inputs can fail while constructing pitches or simplifying
/// enharmonics, so this trait stays explicitly fallible.
pub trait IntoNotes {
    /// Whether this input should be treated as integer-derived pitches.
    const FROM_INTEGER_PITCHES: bool = false;

    /// Iterator-like collection returned by the conversion.
    type Notes: IntoIterator<Item = Note>;

    /// Converts the input into notes.
    fn try_into_notes(self) -> Result<Self::Notes>;
}

impl<T> IntoNotes for Option<T>
where
    T: IntoNotes,
{
    const FROM_INTEGER_PITCHES: bool = T::FROM_INTEGER_PITCHES;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        match self {
            Some(notes) => Ok(notes.try_into_notes()?.into_iter().collect()),
            None => Ok(Vec::new()),
        }
    }
}

impl<T> IntoNotes for Vec<T>
where
    T: IntoNote,
{
    const FROM_INTEGER_PITCHES: bool = T::FROM_INTEGER_PITCH;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut notes = self
            .into_iter()
            .map(IntoNote::try_into_note)
            .collect::<Result<Vec<_>>>()?;
        if Self::FROM_INTEGER_PITCHES {
            simplify_integer_notes(&mut notes)?;
        }
        Ok(notes)
    }
}

/// What `removeRedundantPitches` compares: the spelling and the octave as
/// music21 keeps them, not the printed `nameWithOctave`, which reads the same
/// for `B-1` and B in octave -1 and for `C` with and without an octave.
fn spelling_and_octave(pitch: &Pitch) -> (String, crate::defaults::Octave) {
    (pitch.name(), pitch.octave())
}

fn simplify_integer_notes(notes: &mut [Note]) -> Result<()> {
    if notes.is_empty() {
        return Ok(());
    }

    let pitches = notes
        .iter()
        .map(|note| note.pitch.clone())
        .collect::<Vec<_>>();
    for (note, pitch) in notes
        .iter_mut()
        .zip(crate::pitch::simplify_multiple_enharmonics(
            &pitches, None, None,
        )?)
    {
        note.pitch = pitch;
    }

    Ok(())
}

impl IntoNotes for &[Pitch] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        Ok(self.iter().cloned().map(Note::from_pitch).collect())
    }
}

impl IntoNotes for &[Note] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        Ok(self.to_vec())
    }
}

impl IntoNotes for &[Chord] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        Ok(self.iter().flat_map(|chord| chord.notes.clone()).collect())
    }
}

impl IntoNotes for &[String] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        self.iter()
            .map(|name| Note::from_name(name.as_str()))
            .collect::<Result<Vec<_>>>()
    }
}

impl IntoNotes for String {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        if self.trim().is_empty() {
            Ok(Vec::new())
        } else if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .as_slice()
                .try_into_notes()
        } else {
            Ok(vec![Note::from_name(self)?])
        }
    }
}

impl IntoNotes for &[&str] {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut vec = vec![];
        for str in self {
            vec.append(&mut str.try_into_notes()?);
        }
        Ok(vec)
    }
}

impl IntoNotes for &str {
    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        if self.trim().is_empty() {
            Ok(Vec::new())
        } else if self.contains(char::is_whitespace) {
            self.split_whitespace()
                .collect::<Vec<&str>>()
                .try_into_notes()
        } else {
            Ok(vec![Note::from_name(self)?])
        }
    }
}

impl IntoNotes for &[IntegerType] {
    const FROM_INTEGER_PITCHES: bool = true;

    type Notes = Vec<Note>;

    fn try_into_notes(self) -> Result<Self::Notes> {
        let mut notes = self
            .iter()
            .map(|number| Note::from_number(*number as FloatType))
            .collect::<Result<Vec<_>>>()?;
        simplify_integer_notes(&mut notes)?;
        Ok(notes)
    }
}

impl Chord {
    /// Returns music21's `inversionText`: `Root Position`, `First Inversion`
    /// and so on, or `Unknown Position` for an empty chord.
    pub fn inversion_text(&self) -> String {
        match self.inversion() {
            Some(0) => "Root Position".to_string(),
            Some(inversion) => format!("{} Inversion", ORDINALS[usize::from(inversion)]),
            None => "Unknown Position".to_string(),
        }
    }

    /// Returns the chord in closed position with every repeated step raised
    /// an octave, so an eight-note cluster spreads into a scale: music21's
    /// `semiClosedPosition`.
    pub fn semi_closed_position(
        &self,
        force_octave: Option<IntegerType>,
        leave_redundant_pitches: bool,
    ) -> Self {
        let mut chord = self.closed_position(force_octave, leave_redundant_pitches);
        let implicit_octave = crate::defaults::PITCH_OCTAVE as IntegerType;
        let mut remaining: Vec<usize> = (0..chord.notes.len()).collect();
        while !remaining.is_empty() {
            let mut used_steps = Vec::new();
            let mut still_clashing = Vec::new();
            for index in remaining {
                let pitch = &mut chord.notes[index].pitch;
                let step = root::diatonic_note_number(pitch).rem_euclid(7);
                if used_steps.contains(&step) {
                    let octave = pitch.octave().unwrap_or(implicit_octave) + 1;
                    pitch.octave_setter(Some(octave));
                    still_clashing.push(index);
                } else {
                    used_steps.push(step);
                }
            }
            remaining = still_clashing;
        }
        chord.sort_ascending_in_place();
        chord
    }

    /// Returns a copy sorted by pitch space alone, so enharmonic pairs keep
    /// their input order: music21's `sortChromaticAscending`.
    pub fn sort_chromatic_ascending(&self) -> Self {
        let mut chord = self.clone();
        chord.notes.sort_by(|left, right| {
            left.pitch
                .ps()
                .partial_cmp(&right.pitch.ps())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chord
    }

    /// Returns a copy sorted by staff position and then pitch space, so
    /// `B#3` sorts below `C4`: music21's `sortDiatonicAscending`, which is
    /// also what [`Self::sort_ascending`] does.
    pub fn sort_diatonic_ascending(&self) -> Self {
        self.sort_ascending()
    }

    /// Returns a copy sorted by frequency: music21's `sortFrequencyAscending`.
    pub fn sort_frequency_ascending(&self) -> Self {
        let mut chord = self.clone();
        chord.notes.sort_by(|left, right| {
            left.pitch
                .frequency_hz()
                .partial_cmp(&right.pitch.frequency_hz())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chord
    }

    /// Returns the pitch names in input order, without octaves.
    pub fn pitch_names(&self) -> Vec<String> {
        self.notes.iter().map(|note| note.pitch.name()).collect()
    }

    /// The pitch class of every note, in the order the chord holds them and
    /// with repeats kept: music21's `pitchClasses`. For the sorted, distinct
    /// list see [`Self::pitch_classes`].
    pub fn note_pitch_classes(&self) -> Vec<u8> {
        self.notes
            .iter()
            .map(|note| root::pitch_class(&note.pitch))
            .collect()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_chord_indexes_and_iterates_over_its_notes() {
        let chord = Chord::new("C4 E4 G4").unwrap();

        assert_eq!(chord.len(), 3);
        assert!(!chord.is_empty());
        assert_eq!(chord[1].pitch.name_with_octave(), "E4");

        let borrowed: Vec<String> = (&chord)
            .into_iter()
            .map(|note| note.pitch.name_with_octave())
            .collect();
        assert_eq!(borrowed, vec!["C4", "E4", "G4"]);
        assert_eq!(chord.into_iter().count(), 3);
    }

    #[test]
    fn forte_and_interval_vector_constructors_match_music21() {
        let names = |chord: &Chord| chord.pitch_names();
        assert_eq!(
            names(&Chord::from_forte_class("3-11").unwrap()),
            ["C", "E-", "G"]
        );
        assert_eq!(
            names(&Chord::from_forte_class("3-11B").unwrap()),
            ["C", "E", "G"]
        );
        assert_eq!(
            names(&Chord::from_forte_class("3-11a").unwrap()),
            ["C", "E-", "G"]
        );
        assert_eq!(
            names(&Chord::from_forte_address(4, 27, Some(-1)).unwrap()),
            ["C", "E-", "G-", "A-"]
        );
        assert_eq!(
            Chord::from_forte_class("3-11").unwrap().prime_form_string(),
            "<037>"
        );
        assert!(Chord::from_forte_class("311").is_err());
        assert!(Chord::from_forte_class("3-99").is_err());
        assert_eq!(
            Chord::from_forte_class("4-z15")
                .unwrap()
                .forte_class()
                .as_deref(),
            Some("4-15A")
        );
        assert_eq!(
            Chord::from_forte_class("4-Z15").unwrap().pitch_names(),
            Chord::from_forte_class("4-15").unwrap().pitch_names()
        );

        assert_eq!(
            names(&Chord::from_interval_vector(&[0, 0, 1, 1, 1, 0], false).unwrap()),
            ["C", "E-", "G"]
        );
        assert_eq!(
            names(&Chord::from_interval_vector(&[1, 1, 1, 1, 1, 1], false).unwrap()),
            ["C", "C#", "E", "F#"]
        );
        assert_eq!(
            names(&Chord::from_interval_vector(&[1, 1, 1, 1, 1, 1], true).unwrap()),
            ["C", "D-", "E-", "G"]
        );
        assert!(Chord::from_interval_vector(&[9, 9, 9, 9, 9, 9], false).is_none());
    }

    #[test]
    fn geometric_normal_form_matches_music21() {
        let cases: [(&str, &[u8]); 9] = [
            ("C4 E4 G4", &[0, 3, 8]),
            ("E4 G4 C5", &[0, 3, 8]),
            ("C4 D-4 E4 G-4", &[0, 1, 4, 6]),
            ("C4 E-4 G-4 A4", &[0, 3, 6, 9]),
            ("C4", &[0]),
            ("C4 C5", &[0]),
            ("B3 C4 E4", &[0, 1, 5]),
            ("F#4 A4 C5 E-5", &[0, 3, 6, 9]),
            ("C4 D4 E4 F4 G4 A4 B4", &[0, 1, 3, 5, 6, 8, 10]),
        ];
        for (notes, expected) in cases {
            assert_eq!(
                Chord::new(notes).unwrap().geometric_normal_form(),
                expected,
                "{notes}"
            );
        }
        assert!(Chord::empty().geometric_normal_form().is_empty());
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn set_class_strings_and_flags_match_music21() {
        let cases: [(&str, &str, &str, u8, &str, usize, bool, bool, &str, bool); 14] = [
            (
                "C4 E4 G4",
                "<001110>",
                "<047>",
                11,
                "3-11B",
                3,
                true,
                false,
                "Root Position",
                false,
            ),
            (
                "C4 E-4 G-4 B--4",
                "<004002>",
                "<0369>",
                28,
                "4-28",
                4,
                false,
                false,
                "Root Position",
                true,
            ),
            (
                "C4 E4 G4 B-4",
                "<012111>",
                "<47A0>",
                27,
                "4-27B",
                4,
                true,
                false,
                "Root Position",
                false,
            ),
            (
                "C3 G3 E4 C5",
                "<001110>",
                "<047>",
                11,
                "3-11B",
                4,
                true,
                false,
                "Root Position",
                false,
            ),
            (
                "E-4 G4 C5",
                "<001110>",
                "<037>",
                11,
                "3-11A",
                3,
                false,
                false,
                "First Inversion",
                false,
            ),
            (
                "C4 D-4 E4 G-4",
                "<111111>",
                "<0146>",
                15,
                "4-15A",
                4,
                false,
                true,
                "Root Position",
                false,
            ),
            (
                "C4 D-4 E-4 G4",
                "<111111>",
                "<0137>",
                29,
                "4-29A",
                4,
                false,
                true,
                "Root Position",
                false,
            ),
            (
                "C4 C#4 D4 E4 F#4 G4 A4 B4",
                "<465472>",
                "<B0124679>",
                23,
                "8-23",
                8,
                false,
                false,
                "Root Position",
                false,
            ),
            (
                "G3 C4 E4",
                "<001110>",
                "<047>",
                11,
                "3-11B",
                3,
                true,
                false,
                "Second Inversion",
                false,
            ),
            (
                "B#3 C4 E4 G-4 F#4",
                "<010101>",
                "<046>",
                8,
                "3-8B",
                5,
                true,
                false,
                "Third Inversion",
                false,
            ),
            (
                "C4 E-4 G-4 A4",
                "<004002>",
                "<0369>",
                28,
                "4-28",
                4,
                false,
                false,
                "First Inversion",
                true,
            ),
            (
                "C4",
                "<000000>",
                "<0>",
                1,
                "1-1",
                1,
                false,
                false,
                "Root Position",
                false,
            ),
            (
                "C4 F4 G4",
                "<010020>",
                "<570>",
                9,
                "3-9",
                3,
                false,
                false,
                "Second Inversion",
                false,
            ),
            (
                "D4 F4 A-4 C-5",
                "<004002>",
                "<258B>",
                28,
                "4-28",
                4,
                false,
                false,
                "Root Position",
                true,
            ),
        ];
        for (
            notes,
            vector,
            normal_order,
            forte_number,
            forte_tn,
            cardinality,
            prime_inversion,
            z_relation,
            inversion_text,
            false_diminished,
        ) in cases
        {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(chord.interval_vector_string(), vector, "{notes}");
            assert_eq!(chord.normal_order_string(), normal_order, "{notes}");
            assert_eq!(chord.forte_class_number(), Some(forte_number), "{notes}");
            assert_eq!(chord.forte_class_tn().as_deref(), Some(forte_tn), "{notes}");
            assert_eq!(chord.multiset_cardinality(), cardinality, "{notes}");
            assert_eq!(chord.is_prime_form_inversion(), prime_inversion, "{notes}");
            assert_eq!(chord.has_z_relation(), z_relation, "{notes}");
            assert_eq!(chord.inversion_text(), inversion_text, "{notes}");
            assert_eq!(
                chord.is_false_diminished_seventh(),
                false_diminished,
                "{notes}"
            );
        }

        let empty = Chord::empty();
        assert_eq!(empty.interval_vector_string(), "<000000>");
        assert_eq!(empty.normal_order_string(), "<>");
        assert_eq!(empty.forte_class_number(), None);
        assert_eq!(empty.multiset_cardinality(), 0);
        assert!(!empty.has_z_relation());
        assert_eq!(empty.inversion_text(), "Unknown Position");
    }

    #[test]
    fn z_relations_pair_up_like_music21() {
        let z15 = Chord::new("C4 D-4 E4 G-4").unwrap();
        let z29 = Chord::new("C4 D-4 E-4 G4").unwrap();
        let triad = Chord::new("C E G").unwrap();
        assert!(z15.are_z_relations(&z29));
        assert!(z29.are_z_relations(&z15));
        assert!(!z15.are_z_relations(&triad));
        assert!(!triad.are_z_relations(&z15));
    }

    #[test]
    fn interval_from_chord_step_matches_music21() {
        let cases: [(&str, Option<&str>, Option<&str>); 9] = [
            ("C4 E4 G4", Some("M3"), Some("P5")),
            ("C4 E-4 G-4 B--4", Some("m3"), Some("d5")),
            ("C3 G3 E4 C5", Some("M10"), Some("P5")),
            ("E-4 G4 C5", Some("M6"), Some("P4")),
            ("C4 D-4 E4 G-4", Some("M3"), Some("d5")),
            ("C4 E-4 G-4 A4", Some("M6"), Some("A4")),
            ("A2 C#4 E4 G4", Some("M10"), Some("P12")),
            ("C4", None, None),
            ("C4 F4 G4", None, Some("P4")),
        ];
        for (notes, third, fifth) in cases {
            let chord = Chord::new(notes).unwrap();
            let name = |interval: Option<Interval>| interval.map(|interval| interval.short_name());
            assert_eq!(
                name(chord.interval_from_chord_step(3)).as_deref(),
                third,
                "{notes}"
            );
            assert_eq!(
                name(chord.interval_from_chord_step(5)).as_deref(),
                fifth,
                "{notes}"
            );
        }
        assert!(Chord::empty().interval_from_chord_step(3).is_none());
    }

    fn octave_names(chord: &Chord) -> Vec<String> {
        chord
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect()
    }

    #[test]
    fn semi_closed_position_matches_music21() {
        let cases: [(&str, &[&str], &[&str]); 7] = [
            ("C4 E4 G4", &["C4", "E4", "G4"], &["C3", "E3", "G3"]),
            ("C3 G3 E4 C5", &["C3", "E3", "G3"], &["C3", "E3", "G3"]),
            ("E-4 G4 C5", &["E-4", "G4", "C5"], &["E-3", "G3", "C4"]),
            (
                "C4 C#4 D4 E4 F#4 G4 A4 B4",
                &["C4", "D4", "E4", "F#4", "G4", "A4", "B4", "C#5"],
                &["C3", "D3", "E3", "F#3", "G3", "A3", "B3", "C#4"],
            ),
            ("C4 E4 G4 C5 E5", &["C4", "E4", "G4"], &["C3", "E3", "G3"]),
            (
                "B#3 C4 E4 G-4 F#4",
                &["B#3", "C4", "E4", "F#4", "G-4"],
                &["B#3", "C4", "E4", "F#4", "G-4"],
            ),
            (
                "E4 G#4 B4 D5 F5",
                &["E4", "F4", "G#4", "B4", "D5"],
                &["E3", "F3", "G#3", "B3", "D4"],
            ),
        ];
        for (notes, expected, expected_forced) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(
                octave_names(&chord.semi_closed_position(None, false)),
                expected,
                "{notes}"
            );
            assert_eq!(
                octave_names(&chord.semi_closed_position(Some(3), false)),
                expected_forced,
                "{notes} forced to octave 3"
            );
        }
    }

    #[test]
    fn sort_variants_match_music21() {
        let chord = Chord::new("B#3 C4 E4 G-4 F#4").unwrap();
        assert_eq!(
            octave_names(&chord.sort_chromatic_ascending()),
            ["B#3", "C4", "E4", "G-4", "F#4"]
        );
        assert_eq!(
            octave_names(&chord.sort_diatonic_ascending()),
            ["B#3", "C4", "E4", "F#4", "G-4"]
        );
        assert_eq!(
            octave_names(&chord.sort_frequency_ascending()),
            ["B#3", "C4", "E4", "G-4", "F#4"]
        );
        let spread = Chord::new("C5 G3 E4 C3").unwrap();
        assert_eq!(
            octave_names(&spread.sort_chromatic_ascending()),
            ["C3", "G3", "E4", "C5"]
        );
        assert_eq!(
            octave_names(&spread.sort_frequency_ascending()),
            ["C3", "G3", "E4", "C5"]
        );
        assert_eq!(chord.pitch_names(), ["B#", "C", "E", "G-", "F#"]);
    }

    #[test]
    fn full_name_matches_music21() {
        // A chord with no duration of its own reads as a quarter, which is
        // the duration music21 gives every chord by default.
        let chord = Chord::new("C4 E-4 G-4 B--4").unwrap();
        assert_eq!(
            chord.full_name(),
            "Chord {C in octave 4 | E-flat in octave 4 | G-flat in octave 4 | B-double-flat in octave 4} Quarter"
        );
        let quarter = chord.clone().with_duration(Duration::quarter());
        assert_eq!(
            quarter.full_name(),
            "Chord {C in octave 4 | E-flat in octave 4 | G-flat in octave 4 | B-double-flat in octave 4} Quarter"
        );
        let dotted = Chord::new("C4 E4 G4")
            .unwrap()
            .with_duration(Duration::new(1.5).unwrap());
        assert_eq!(
            dotted.full_name(),
            "Chord {C in octave 4 | E in octave 4 | G in octave 4} Dotted Quarter"
        );
    }
    use crate::{Duration, GuitarTuning, Interval, Key, Pitch, chord::Chord, chord::TriadQuality};

    #[test]
    fn a_chord_carries_notation_for_all_its_notes() {
        use crate::notation::{Beams, Notehead, StemDirection};

        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert_eq!(chord.notehead(), Notehead::Normal);
        chord.set_notehead(Notehead::Diamond);
        assert_eq!(chord.notehead(), Notehead::Diamond);
        assert_eq!(chord.notehead_fill(), None);
        chord.set_notehead_fill(Some(false));
        assert_eq!(chord.notehead_fill(), Some(false));
        assert!(!chord.notehead_parenthesis());
        chord.set_notehead_parenthesis(true);
        assert!(chord.notehead_parenthesis());
        chord.set_stem_direction(StemDirection::Down);
        assert_eq!(chord.stem_direction(), StemDirection::Down);
        assert!(chord.beams().is_empty());
        let mut beams = Beams::default();
        beams
            .fill(crate::duration::DurationType::Eighth, None)
            .unwrap();
        chord.set_beams(beams.clone());
        assert_eq!(chord.beams(), &beams);
    }

    #[test]
    fn notes_are_added_and_removed_as_music21_adds_and_removes_them() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        chord.add("B-4").unwrap();
        assert_eq!(chord.pitch_names(), ["C", "E", "G", "B-"]);
        chord.remove(&Pitch::from_name("E4").unwrap()).unwrap();
        assert!(chord.remove(&Pitch::from_name("E4").unwrap()).is_err());
        chord.remove_named("G4").unwrap();
        assert!(chord.remove_named("G4").is_err());
        assert_eq!(chord.pitch_names(), ["C", "B-"]);
        assert_eq!(chord.note_pitch_classes(), [0, 10]);
        assert_eq!(
            Chord::new("E4 C4 G4 C5").unwrap().note_pitch_classes(),
            [4, 0, 7, 0]
        );
    }

    #[test]
    fn an_overridden_bass_or_root_is_kept_apart_from_the_inferred_one() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        let e = Pitch::from_name("E4").unwrap();
        assert_eq!(chord.overridden_bass(), None);
        assert_eq!(chord.found_bass().map(Pitch::name), Some("C".to_string()));
        chord.set_bass(Some(e.clone()));
        assert_eq!(chord.overridden_bass(), Some(&e));
        assert_eq!(chord.bass().map(Pitch::name), Some("E".to_string()));
        assert_eq!(chord.found_bass().map(Pitch::name), Some("C".to_string()));
        chord.set_bass(None);
        assert_eq!(chord.overridden_bass(), None);
        // A bass the chord does not carry is added below the others.
        chord.set_bass(Some(Pitch::from_name("A3").unwrap()));
        assert_eq!(chord.pitch_names(), ["A", "C", "E", "G"]);

        let mut chord = Chord::new("C4 E4 G4").unwrap();
        assert_eq!(chord.overridden_root(), None);
        chord.set_root(Some(e.clone()));
        assert_eq!(chord.overridden_root(), Some(&e));
        assert_eq!(chord.root().map(Pitch::name), Some("E".to_string()));
        assert_eq!(chord.found_root().map(Pitch::name), Some("C".to_string()));
    }

    #[test]
    fn set_inversion_raises_the_bass_until_the_chord_stands_in_it() {
        let mut chord = Chord::new("C4 E4 G4").unwrap();
        chord.set_inversion(1).unwrap();
        assert_eq!(chord.inversion(), Some(1));
        assert_eq!(
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>(),
            ["E4", "G4", "C5"]
        );
        chord.set_inversion(0).unwrap();
        assert_eq!(chord.bass().map(Pitch::name), Some("C".to_string()));
        assert!(chord.set_inversion(5).is_err());
    }

    #[test]
    fn chord_steps_can_be_measured_from_a_root_the_caller_names() {
        let chord = Chord::new("E4 G4 C5").unwrap();
        let c = Pitch::from_name("C4").unwrap();
        let g = Pitch::from_name("G4").unwrap();
        assert_eq!(chord.inversion_from_root(&c), Some(1));
        // A bass a sixth above the root is music21's sixth inversion.
        assert_eq!(chord.inversion_from_root(&g), Some(6));
        assert_eq!(
            chord.chord_step_with_root(3, &c).map(Pitch::name),
            Some("E".to_string())
        );
        assert_eq!(chord.chord_step_with_root(3, &g), None);
        assert_eq!(chord.semitones_from_chord_step_with_root(5, &c), Some(7));
        assert_eq!(chord.semitones_from_chord_step_with_root(3, &g), None);
    }

    #[test]
    fn the_reductions_report_what_they_dropped() {
        let names = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };
        let (kept, dropped) = Chord::new("C4 E4 G4 C4")
            .unwrap()
            .remove_redundant_pitches_reporting();
        assert_eq!(kept.pitch_names(), ["C", "E", "G"]);
        assert_eq!(names(dropped), ["C4"]);
        let (kept, dropped) = Chord::new("C4 E4 G4 C5")
            .unwrap()
            .remove_redundant_pitch_names_reporting();
        assert_eq!(kept.pitch_names(), ["C", "E", "G"]);
        assert_eq!(names(dropped), ["C5"]);
        let (kept, dropped) = Chord::new("C4 E4 G4 B#4")
            .unwrap()
            .remove_redundant_pitch_classes_reporting();
        assert_eq!(kept.pitch_names(), ["C", "E", "G"]);
        assert_eq!(names(dropped), ["B#4"]);
    }

    #[test]
    fn pitches_that_only_print_alike_are_not_redundant() {
        let mut low_b: Pitch = "B".parse().unwrap();
        low_b.octave_setter(Some(-1));
        let mut b_flat: Pitch = "B-".parse().unwrap();
        b_flat.octave_setter(Some(1));
        assert_eq!(low_b.name_with_octave(), b_flat.name_with_octave());
        let chord = Chord::new(vec![low_b, b_flat]).unwrap();
        let (kept, dropped) = chord.remove_redundant_pitches_reporting();
        assert_eq!(kept.notes.len(), 2);
        assert!(dropped.is_empty());
        assert_eq!(chord.remove_redundant_pitches().notes.len(), 2);

        let chord = Chord::new("C C4").unwrap();
        assert_eq!(chord.remove_redundant_pitches().notes.len(), 2);
        assert_eq!(
            Chord::new("C4 C4")
                .unwrap()
                .remove_redundant_pitches()
                .notes
                .len(),
            1
        );
    }

    #[test]
    fn the_chord_table_address_is_a_record_and_an_empty_chord_has_one() {
        let address = Chord::new("C E G").unwrap().chord_tables_address_entry();
        assert_eq!(address.cardinality, 3);
        assert_eq!(address.forte_class, 11);
        assert_eq!(address.inversion, -1);
        assert_eq!(address.pitch_class_original, 0);
        let empty = Chord::new("").unwrap().chord_tables_address_entry();
        assert_eq!(
            (
                empty.cardinality,
                empty.forte_class,
                empty.inversion,
                empty.pitch_class_original
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(super::format_vector_string(&[0, 0, 1, 1, 1, 0]), "<001110>");
    }

    #[test]
    fn a_chord_is_built_from_any_of_the_inputs_try_from_accepts() {
        use crate::note::Note;

        let names = |chord: Chord| chord.pitch_names();
        assert_eq!(names(Chord::try_from("C E G").unwrap()), ["C", "E", "G"]);
        assert_eq!(
            names(Chord::try_from("C E G".to_string()).unwrap()),
            ["C", "E", "G"]
        );
        let pitches = [
            Pitch::from_name("C4").unwrap(),
            Pitch::from_name("E4").unwrap(),
        ];
        assert_eq!(names(Chord::try_from(&pitches[..]).unwrap()), ["C", "E"]);
        let notes = [Note::from_pitch(pitches[0].clone())];
        assert_eq!(names(Chord::try_from(&notes[..]).unwrap()), ["C"]);
        assert_eq!(names(Chord::try_from(&[60, 64][..]).unwrap()), ["C", "E"]);
        assert_eq!(names(Chord::try_from(&["C", "G"][..]).unwrap()), ["C", "G"]);
        let owned = ["D".to_string(), "A".to_string()];
        assert_eq!(names(Chord::try_from(&owned[..]).unwrap()), ["D", "A"]);
    }

    #[test]
    fn set_duration_applies_to_non_empty_chords() {
        // The setter applies whether or not the chord holds notes.
        for input in ["", "C", "C E G", "C E G B-"] {
            let mut chord = Chord::new(input).unwrap();
            chord.set_duration(Duration::whole());
            assert_eq!(
                chord.duration().map(Duration::quarter_length),
                Some(4.0),
                "set_duration on {input:?}"
            );
        }
    }

    struct PredicateCase {
        notes: &'static str,
        quality: TriadQuality,
        flags: [bool; 14],
        third: Option<&'static str>,
        fifth: Option<&'static str>,
        seventh: Option<&'static str>,
        enharmonic: bool,
        repeated_third: bool,
        third_semitones: Option<u8>,
    }

    #[test]
    fn triad_and_seventh_predicates_match_music21() {
        use TriadQuality::*;
        let t = true;
        let f = false;
        let cases = [
            PredicateCase {
                notes: "C E G",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E- G",
                quality: Minor,
                flags: [t, f, t, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E-"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E- G-",
                quality: Diminished,
                flags: [t, f, f, t, f, f, f, f, f, f, f, f, t, f],
                third: Some("E-"),
                fifth: Some("G-"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E G#",
                quality: Augmented,
                flags: [t, f, f, f, t, f, f, f, f, f, f, f, t, f],
                third: Some("E"),
                fifth: Some("G#"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C4 E4 G4 B-4",
                quality: Major,
                flags: [f, f, f, f, f, t, t, f, f, f, f, f, t, t],
                third: Some("E4"),
                fifth: Some("G4"),
                seventh: Some("B-4"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E- G- B--",
                quality: Diminished,
                flags: [f, f, f, f, f, t, f, f, t, f, f, f, t, t],
                third: Some("E-"),
                fifth: Some("G-"),
                seventh: Some("B--"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E- G- B-",
                quality: Diminished,
                flags: [f, f, f, f, f, t, f, t, f, f, f, f, t, t],
                third: Some("E-"),
                fifth: Some("G-"),
                seventh: Some("B-"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C E G B",
                quality: Major,
                flags: [f, f, f, f, f, t, f, f, f, f, f, f, t, t],
                third: Some("E"),
                fifth: Some("G"),
                seventh: Some("B"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "E G C",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "G C E",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E",
                quality: Major,
                flags: [f, f, f, f, f, f, f, f, f, t, t, f, f, f],
                third: Some("E"),
                fifth: None,
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E-",
                quality: Minor,
                flags: [f, f, f, f, f, f, f, f, f, t, f, t, f, f],
                third: Some("E-"),
                fifth: None,
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C G",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, t, f, f, f, f],
                third: None,
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C F",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                third: None,
                fifth: Some("C"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C4 F4",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                third: None,
                fifth: Some("C4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C E G C5",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C E E- G",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, t, f],
                third: Some("E"),
                fifth: Some("G"),
                seventh: None,
                enharmonic: f,
                repeated_third: t,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "B# E G",
                quality: Other,
                flags: [t, f, f, f, f, f, f, f, f, f, f, f, t, f],
                third: Some("G"),
                fifth: Some("B#"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C F# G",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                third: None,
                fifth: Some("C"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "C E G B- D",
                quality: Major,
                flags: [f, f, f, f, f, f, f, f, f, f, f, f, t, t],
                third: Some("E"),
                fifth: Some("G"),
                seventh: Some("B-"),
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C",
                quality: Other,
                flags: [f, f, f, f, f, f, f, f, f, t, f, f, f, f],
                third: None,
                fifth: None,
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: None,
            },
            PredicateCase {
                notes: "E-4 G4 B-4",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("G4"),
                fifth: Some("B-4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C#4 E4 G4",
                quality: Diminished,
                flags: [t, f, f, t, f, f, f, f, f, f, f, f, t, f],
                third: Some("E4"),
                fifth: Some("G4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(3),
            },
            PredicateCase {
                notes: "C4 E4 G4 E5",
                quality: Major,
                flags: [t, t, f, f, f, f, f, f, f, t, f, f, t, f],
                third: Some("E4"),
                fifth: Some("G4"),
                seventh: None,
                enharmonic: f,
                repeated_third: f,
                third_semitones: Some(4),
            },
            PredicateCase {
                notes: "C#4 D-4 E4",
                quality: Minor,
                flags: [f, f, f, f, f, f, f, f, f, f, f, t, f, f],
                third: Some("E4"),
                fifth: None,
                seventh: None,
                enharmonic: t,
                repeated_third: f,
                third_semitones: Some(3),
            },
        ];
        for case in cases {
            let chord = Chord::new(case.notes).unwrap();
            let notes = case.notes;
            let name = |pitch: Option<&Pitch>| pitch.map(Pitch::name_with_octave);
            assert_eq!(chord.quality(), case.quality, "{notes} quality");
            let actual = [
                chord.is_triad(),
                chord.is_major_triad(),
                chord.is_minor_triad(),
                chord.is_diminished_triad(),
                chord.is_augmented_triad(),
                chord.is_seventh(),
                chord.is_dominant_seventh(),
                chord.is_half_diminished_seventh(),
                chord.is_diminished_seventh(),
                chord.is_consonant(),
                chord.is_incomplete_major_triad(),
                chord.is_incomplete_minor_triad(),
                chord.contains_triad(),
                chord.contains_seventh(),
            ];
            assert_eq!(actual, case.flags, "{notes} predicates");
            assert_eq!(name(chord.third()).as_deref(), case.third, "{notes} third");
            assert_eq!(name(chord.fifth()).as_deref(), case.fifth, "{notes} fifth");
            assert_eq!(
                name(chord.seventh()).as_deref(),
                case.seventh,
                "{notes} seventh"
            );
            assert_eq!(
                chord.has_any_enharmonic_spelled_pitches(),
                case.enharmonic,
                "{notes} enharmonic"
            );
            assert_eq!(
                chord.has_repeated_chord_step(3),
                case.repeated_third,
                "{notes} repeated third"
            );
            assert_eq!(
                chord.semitones_from_chord_step(3),
                case.third_semitones,
                "{notes} third semitones"
            );
        }

        let empty = Chord::empty();
        assert_eq!(empty.quality(), TriadQuality::Other);
        assert!(!empty.is_triad());
        assert!(!empty.is_consonant());
        assert!(empty.third().is_none());
        assert!(!empty.contains_triad());
        assert_eq!(TriadQuality::Diminished.to_string(), "diminished");
    }

    #[test]
    fn consonance_of_dyads_follows_closed_position() {
        assert!(Chord::new("C4 C5 E5").unwrap().is_consonant());
        assert!(!Chord::new("C4 F4 C5").unwrap().is_consonant());
        assert!(Chord::new("F4 C5").unwrap().is_consonant());
        assert!(!Chord::new("C4 G3").unwrap().is_consonant());
    }

    #[test]
    fn closed_position_matches_music21() {
        let names = |chord: Chord| {
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>()
        };
        let cases = [
            ("C#4 G5 E6", None, vec!["C#4", "E4", "G4"]),
            ("C#4 G5 E6", Some(2), vec!["C#2", "E2", "G2"]),
            ("C#4 G5 E6", Some(6), vec!["C#6", "E6", "G6"]),
            ("C#4 F4 C5 F5", None, vec!["C#4", "F4", "C5"]),
            ("A B", None, vec!["A4", "B4"]),
            ("C4 B#7", None, vec!["C4", "B#4"]),
            ("E4 C5 G5", None, vec!["E4", "G4", "C5"]),
            (
                "C3 C#3 E-3 E3 E#3 G3",
                None,
                vec!["C3", "C#3", "E-3", "E3", "E#3", "G3"],
            ),
            ("G4 C4 E4", Some(5), vec!["C5", "E5", "G5"]),
            ("C4 E4 G4 C5 E5", None, vec!["C4", "E4", "G4"]),
            ("C#4 D-4 E4", None, vec!["C#4", "D-4", "E4"]),
        ];
        for (notes, force_octave, expected) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(
                names(chord.closed_position(force_octave, false)),
                expected,
                "{notes}"
            );
        }
        assert!(
            Chord::empty()
                .closed_position(None, false)
                .notes()
                .is_empty()
        );
        assert_eq!(
            names(
                Chord::new("C4 E4 C4 E5")
                    .unwrap()
                    .remove_redundant_pitches()
            ),
            vec!["C4", "E4", "E5"]
        );
        assert_eq!(
            names(
                Chord::new("C4 E4 C5 E5")
                    .unwrap()
                    .remove_redundant_pitch_names()
            ),
            vec!["C4", "E4"]
        );
        assert_eq!(
            names(
                Chord::new("C#4 D-4 E4")
                    .unwrap()
                    .remove_redundant_pitch_classes()
            ),
            vec!["C#4", "E4"]
        );
        assert_eq!(
            names(Chord::new("G-4 F##4 E4").unwrap().sort_ascending()),
            vec!["E4", "F##4", "G-4"]
        );
    }

    #[test]
    fn inversions_match_music21() {
        let cases = [
            ("C E G", 0, "C", "C"),
            ("E G C", 0, "C", "C"),
            ("G C E", 0, "C", "C"),
            ("C4 E4 G4 B-4", 0, "C4", "C4"),
            ("E4 G4 B-4 C5", 1, "C5", "E4"),
            ("B-3 C4 E4 G4", 3, "C4", "B-3"),
            ("G3 C4 E4 B-4", 2, "C4", "G3"),
            ("A-4 C5 F#5", 1, "F#5", "A-4"),
            ("C5 F#5 A-5", 2, "F#5", "C5"),
            ("F#4 A-4 C5", 0, "F#4", "F#4"),
            ("C F G", 2, "F", "C"),
            ("C4 G4 E5", 0, "C4", "C4"),
            ("C", 0, "C", "C"),
            ("G C", 0, "C", "C"),
            ("E C", 0, "C", "C"),
            ("F A C E", 2, "F", "C"),
            ("E4 C5 G5", 1, "C5", "E4"),
            ("C E G A", 1, "A", "C"),
            ("C F# G", 2, "F#", "C"),
            ("D4 F#4 A4 C5 E5", 0, "D4", "D4"),
            ("A-3 C4 E-4 F#4", 1, "F#4", "A-3"),
            ("C4 A-4 E-5 F#5", 2, "F#5", "C4"),
            ("E3 G3 B-3 D-4", 0, "E3", "E3"),
        ];
        for (notes, inversion, root, bass) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(chord.inversion(), Some(inversion), "{notes} inversion");
            assert_eq!(
                chord.root().map(Pitch::name_with_octave).as_deref(),
                Some(root),
                "{notes} root"
            );
            assert_eq!(
                chord.bass().map(Pitch::name_with_octave).as_deref(),
                Some(bass),
                "{notes} bass"
            );
        }
        assert_eq!(Chord::empty().inversion(), None);
        assert_eq!(
            Chord::new("B#3 C4 E4")
                .unwrap()
                .bass()
                .unwrap()
                .name_with_octave(),
            "B#3"
        );
    }

    #[test]
    fn augmented_sixths_and_set_class_helpers_match_music21() {
        struct Case {
            notes: &'static str,
            flags: [bool; 11],
            prime_form: &'static str,
            forte_tni: &'static str,
            cardinality: usize,
            ordered: &'static str,
        }
        let t = true;
        let f = false;
        let cases = [
            Case {
                notes: "A-4 C5 F#5",
                flags: [t, t, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "A-4 C5 D5 F#5",
                flags: [t, f, t, f, f, t, f, f, t, f, f],
                prime_form: "<0268>",
                forte_tni: "4-25",
                cardinality: 4,
                ordered: "<0268>",
            },
            Case {
                notes: "A-4 C5 E-5 F#5",
                flags: [t, f, f, t, f, t, f, f, f, f, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<0368>",
            },
            Case {
                notes: "A-4 C5 D#5 F#5",
                flags: [t, f, f, f, t, t, f, f, f, f, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<0368>",
            },
            Case {
                notes: "C4 E4 G4",
                flags: [f, f, f, f, f, f, f, f, f, t, t],
                prime_form: "<037>",
                forte_tni: "3-11",
                cardinality: 3,
                ordered: "<047>",
            },
            Case {
                notes: "F#4 A-4 C5",
                flags: [f, f, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "C5 F#5 A-5",
                flags: [f, f, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "A-4 F#5 C6",
                flags: [t, t, f, f, f, t, t, f, f, f, f],
                prime_form: "<026>",
                forte_tni: "3-8",
                cardinality: 3,
                ordered: "<068>",
            },
            Case {
                notes: "A-4 C5 E-5 F#5 A-5",
                flags: [t, f, f, t, f, t, f, f, f, f, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<0368>",
            },
            Case {
                notes: "C E G B- D",
                flags: [f, f, f, f, f, f, f, t, f, f, f],
                prime_form: "<02469>",
                forte_tni: "5-34",
                cardinality: 5,
                ordered: "<0247A>",
            },
            Case {
                notes: "C E G B D F",
                flags: [f, f, f, f, f, f, f, f, f, f, f],
                prime_form: "<013568>",
                forte_tni: "6-25",
                cardinality: 6,
                ordered: "<02457B>",
            },
            Case {
                notes: "C E G#",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<048>",
                forte_tni: "3-12",
                cardinality: 3,
                ordered: "<048>",
            },
            Case {
                notes: "C E- G- B--",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<0369>",
                forte_tni: "4-28",
                cardinality: 4,
                ordered: "<0369>",
            },
            Case {
                notes: "C D E F# G# A#",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<02468A>",
                forte_tni: "6-35",
                cardinality: 6,
                ordered: "<02468A>",
            },
            Case {
                notes: "C C# D D# E F F# G G# A A# B",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<0123456789AB>",
                forte_tni: "12-1",
                cardinality: 12,
                ordered: "<0123456789AB>",
            },
            Case {
                notes: "C",
                flags: [f, f, f, f, f, f, f, f, f, f, f],
                prime_form: "<0>",
                forte_tni: "1-1",
                cardinality: 1,
                ordered: "<0>",
            },
            Case {
                notes: "B- D F A-",
                flags: [f, f, f, f, f, f, f, f, f, t, f],
                prime_form: "<0258>",
                forte_tni: "4-27",
                cardinality: 4,
                ordered: "<258A>",
            },
            Case {
                notes: "C F#",
                flags: [f, f, f, f, f, f, f, f, t, f, f],
                prime_form: "<06>",
                forte_tni: "2-6",
                cardinality: 2,
                ordered: "<06>",
            },
        ];
        for case in cases {
            let chord = Chord::new(case.notes).unwrap();
            let notes = case.notes;
            let actual = [
                chord.is_augmented_sixth(false),
                chord.is_italian_augmented_sixth(false, false),
                chord.is_french_augmented_sixth(false),
                chord.is_german_augmented_sixth(false),
                chord.is_swiss_augmented_sixth(false),
                chord.is_augmented_sixth(true),
                chord.is_italian_augmented_sixth(true, false),
                chord.is_ninth(),
                chord.is_transpositionally_symmetrical(false),
                chord.can_be_dominant_v(),
                chord.can_be_tonic(),
            ];
            assert_eq!(actual, case.flags, "{notes}");
            assert_eq!(
                chord.prime_form_string(),
                case.prime_form,
                "{notes} prime form"
            );
            assert_eq!(
                chord.forte_class_tni().as_deref(),
                Some(case.forte_tni),
                "{notes} forte tni"
            );
            assert_eq!(chord.pitch_class_cardinality(), case.cardinality, "{notes}");
            assert_eq!(
                chord.ordered_pitch_classes_string(),
                case.ordered,
                "{notes}"
            );
        }

        assert!(
            Chord::new("C")
                .unwrap()
                .is_transpositionally_symmetrical(true)
        );
        assert!(
            !Chord::new("C D-")
                .unwrap()
                .is_transpositionally_symmetrical(false)
        );
        assert!(
            Chord::new("A-4 C5 D5 F#5")
                .unwrap()
                .is_transpositionally_symmetrical(false)
        );
        assert!(
            !Chord::new("A-4 C5 D5 F#5")
                .unwrap()
                .is_transpositionally_symmetrical(true)
        );
        assert!(
            Chord::new("C C# D D# E F F# G G# A A# B")
                .unwrap()
                .has_any_repeated_diatonic_note()
        );
        assert!(
            !Chord::new("C E G")
                .unwrap()
                .has_any_repeated_diatonic_note()
        );

        let empty = Chord::empty();
        assert!(empty.is_transpositionally_symmetrical(false));
        assert!(empty.prime_form().is_empty());
        assert_eq!(empty.prime_form_string(), "<>");
        assert_eq!(empty.forte_class_tni(), None);
        assert_eq!(empty.pitch_class_cardinality(), 0);
        assert_eq!(empty.ordered_pitch_classes_string(), "<>");
    }

    #[test]
    fn transpose_moves_every_note() {
        let names = |chord: Chord| {
            chord
                .pitches()
                .iter()
                .map(Pitch::name_with_octave)
                .collect::<Vec<_>>()
        };
        let chord = Chord::new("C4 E4 G4").unwrap();
        let up = |name: &str| {
            chord
                .transpose(&Interval::from_name(name).unwrap())
                .unwrap()
        };
        assert_eq!(names(up("M2")), vec!["D4", "F#4", "A4"]);
        assert_eq!(names(up("-m3")), vec!["A3", "C#4", "E4"]);
        assert_eq!(
            names(
                chord
                    .transpose(&Interval::from_semitones(3).unwrap())
                    .unwrap()
            ),
            vec!["E-4", "G4", "B-4"]
        );
        let bare = Chord::new("C E G").unwrap();
        assert_eq!(
            names(bare.transpose(&Interval::from_name("P5").unwrap()).unwrap()),
            vec!["G", "B", "D"]
        );
    }

    #[test]
    fn scale_degrees_match_music21() {
        let cases = [
            (
                "C",
                "C E G",
                vec![(Some(1), None), (Some(3), None), (Some(5), None)],
            ),
            (
                "C",
                "C E- G",
                vec![(Some(1), None), (Some(3), Some("flat")), (Some(5), None)],
            ),
            (
                "F",
                "F A C E",
                vec![
                    (Some(1), None),
                    (Some(3), None),
                    (Some(5), None),
                    (Some(7), None),
                ],
            ),
            (
                "a",
                "G# B D",
                vec![(Some(7), Some("sharp")), (Some(2), None), (Some(4), None)],
            ),
            (
                "B-",
                "E- G B-",
                vec![(Some(4), None), (Some(6), None), (Some(1), None)],
            ),
            (
                "D",
                "C# E G B-",
                vec![
                    (Some(7), None),
                    (Some(2), None),
                    (Some(4), None),
                    (Some(6), Some("flat")),
                ],
            ),
            (
                "C",
                "C E G B- D-",
                vec![
                    (Some(1), None),
                    (Some(3), None),
                    (Some(5), None),
                    (Some(7), Some("flat")),
                    (Some(2), Some("flat")),
                ],
            ),
        ];
        for (key, notes, expected) in cases {
            let scale = Key::from_tonic(key).unwrap().as_scale().unwrap();
            let degrees = Chord::new(notes)
                .unwrap()
                .scale_degrees(&scale)
                .unwrap()
                .into_iter()
                .map(|(degree, accidental)| (degree, accidental.map(|a| a.name().to_string())))
                .collect::<Vec<_>>();
            let expected = expected
                .into_iter()
                .map(|(degree, accidental): (Option<usize>, Option<&str>)| {
                    (degree, accidental.map(str::to_string))
                })
                .collect::<Vec<_>>();
            assert_eq!(degrees, expected, "{key} {notes}");
        }
    }

    #[test]
    fn pitched_common_names_match_the_music21_reference() {
        let cases = [
            ("C E G", "C-major triad"),
            ("C E- G", "C-minor triad"),
            ("C E G B-", "C-dominant seventh chord"),
            ("C E G B", "C-major seventh chord"),
            ("C E- G B-", "C-minor seventh chord"),
            ("C E- G- B-", "C-half-diminished seventh chord"),
            ("C E- G- B--", "C-diminished seventh chord"),
            ("C E G B- D", "C-dominant-ninth"),
            ("C E G B D", "C-major-ninth chord"),
            ("C E- G B- D", "C-minor-ninth chord"),
            ("G2 B2 D3 F3", "G-dominant seventh chord"),
            ("B2 D3 F3 A3", "B-half-diminished seventh chord"),
        ];
        for (notes, expected) in cases {
            let chord = Chord::new(notes).unwrap();
            assert_eq!(chord.pitched_common_name(), expected, "{notes}");
        }

        let integers: &[crate::IntegerType] = &[1, 2, 3, 4, 5, 10];
        let chord = Chord::new(integers).unwrap();
        assert_eq!(chord.pitched_common_name(), "forte class 6-36B above C#");
    }

    #[test]
    fn c_e_g_pitchedcommonname() {
        let chord = Chord::new("C E G");

        assert!(chord.is_ok());

        assert_eq!(chord.unwrap().pitched_common_name(), "C-major triad");
    }

    #[test]
    fn new_accepts_empty_inputs() {
        assert_eq!(Chord::new("").unwrap().pitched_common_name(), "empty chord");
        assert_eq!(
            Chord::new(Vec::<Pitch>::new())
                .unwrap()
                .pitched_common_name(),
            "empty chord"
        );
        assert_eq!(
            Chord::new(Option::<&str>::None)
                .unwrap()
                .pitched_common_name(),
            "empty chord"
        );
    }

    #[test]
    fn pitched_common_names_returns_aliases() {
        let chord = Chord::new("C E G#").unwrap();
        assert_eq!(
            chord.pitched_common_names(),
            vec![
                "C-augmented triad".to_string(),
                "C-equal 3-part octave division".to_string()
            ]
        );
    }

    #[test]
    fn chord_symbols_return_symbol_names() {
        let major_seventh = Chord::new("C E G B").unwrap();
        let petrushka = Chord::new("C4 D4 Eb4 F#4 Ab4 A4").unwrap();
        let slash_chord = Chord::new("F4 C5 D5 E-5").unwrap();

        assert_eq!(major_seventh.chord_symbol().as_deref(), Some("Cmaj7"));
        assert_eq!(
            petrushka.chord_symbol().as_deref(),
            Some("Ddom7dim5/CaddA,E-")
        );
        assert_eq!(slash_chord.chord_symbol().as_deref(), None);
    }

    #[test]
    fn chord_symbols_with_root_accept_pitch_names() {
        let chord = Chord::new("G3 C4 E4").unwrap();

        assert_eq!(
            chord.chord_symbol_with_root("C").unwrap().as_deref(),
            Some("C/G")
        );
        assert_eq!(
            chord.chord_symbol_with_root(0).unwrap().as_deref(),
            Some("C/G")
        );
    }

    #[test]
    fn guitar_fingering_covers_common_chord_tones() {
        let chord = Chord::new("C E G").unwrap();
        let fingering = chord.guitar_fingering().unwrap();

        assert_eq!(fingering.strings.len(), 6);
        // A voicing sounds chord *tones*, in whatever octave falls under the
        // hand; it is not required to reproduce the written octaves.
        assert_eq!(fingering.covered_pitch_classes, vec![0, 4, 7]);
        assert!(fingering.omitted_pitch_classes.is_empty());
        assert!(
            fingering.covered_pitch_spaces.len() >= 3,
            "expected a full voicing, got {:?}",
            fingering.covered_pitch_spaces
        );
        assert!(
            fingering
                .strings
                .iter()
                .filter(|string| string.fret.is_some_and(|fret| fret > 0))
                .all(|string| string
                    .finger
                    .is_some_and(|finger| (1..=4).contains(&finger)))
        );
    }

    #[test]
    fn guitar_fingering_still_returns_large_pitch_sets() {
        let chord = Chord::new("C D E F G A B").unwrap();
        let fingering = chord.guitar_fingering().unwrap();

        assert_eq!(fingering.strings.len(), 6);
        assert!(!fingering.covered_pitch_classes.is_empty());
        assert!(!fingering.omitted_pitch_classes.is_empty());
    }

    #[test]
    fn guitar_fingering_uses_supplied_tuning_and_octaves() {
        let chord = Chord::new("D3 A3 D4").unwrap();
        let tuning = GuitarTuning::new(["D2", "A2", "D3", "G3", "A3", "D4"]).unwrap();
        let fingering = chord.guitar_fingering_with_tuning(&tuning).unwrap();

        assert_eq!(fingering.strings.len(), 6);
        assert_eq!(fingering.strings[0].string_name, "D2");
        assert_eq!(fingering.covered_pitch_classes, vec![2, 9]);
        assert!(fingering.omitted_pitch_classes.is_empty());
    }

    /// Renders a fingering as the `x 3 2 0 1 0` notation guitarists read.
    fn shape(notes: &str) -> String {
        Chord::new(notes)
            .unwrap()
            .guitar_fingering()
            .unwrap()
            .strings
            .iter()
            .map(|string| match string.fret {
                None => "x".to_string(),
                Some(fret) => fret.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn guitar_fingering_finds_the_standard_open_chords() {
        // The shapes any player would name for these chords. Before voicings
        // were matched by pitch class these all came back as `x x x n n n`.
        assert_eq!(shape("C E G"), "x 3 2 0 1 0");
        assert_eq!(shape("A C# E"), "x 0 2 2 2 0");
        assert_eq!(shape("E G# B"), "0 2 2 1 0 0");
        assert_eq!(shape("D F# A"), "x x 0 2 3 2");
        assert_eq!(shape("A C E"), "x 0 2 2 1 0");
        assert_eq!(shape("E G B"), "0 2 2 0 0 0");
        assert_eq!(shape("D F A"), "x x 0 2 3 1");
        assert_eq!(shape("G B D F"), "3 2 0 0 0 1");
        assert_eq!(shape("C E G B"), "x 3 2 0 0 0");
        assert_eq!(shape("A C E G"), "x 0 2 0 1 0");
    }

    #[test]
    fn guitar_fingering_keeps_every_chord_tone() {
        // Omitting a chord tone costs a voicing far more than omitting a
        // written octave, so a seventh chord keeps its seventh.
        for notes in ["G B D F", "C E G B-", "A C E G", "C E G B", "B D F"] {
            let fingering = Chord::new(notes).unwrap().guitar_fingering().unwrap();
            assert!(
                fingering.omitted_pitch_classes.is_empty(),
                "{notes} dropped {:?}",
                fingering.omitted_pitch_classes
            );
        }
    }

    #[test]
    fn guitar_fingering_puts_the_root_in_the_bass_for_open_chords() {
        for (notes, root) in [("C E G", 0), ("G B D", 7), ("E G# B", 4), ("A C E", 9)] {
            let fingering = Chord::new(notes).unwrap().guitar_fingering().unwrap();
            let bass = fingering
                .strings
                .iter()
                .find_map(|string| string.fret.and(string.pitch_class))
                .expect("a sounding string");
            assert_eq!(bass, root, "{notes} should sound its root lowest");
        }
    }

    #[test]
    fn guitar_tuning_rejects_empty_tunings() {
        assert!(GuitarTuning::new(Vec::<&str>::new()).is_err());
    }

    #[test]
    fn dyad_names_follow_music21_interval_rules() {
        let pcs = [0, 1];
        let integer_chord = Chord::new(pcs.as_slice()).unwrap();
        assert_eq!(integer_chord.common_name(), "Minor Second");
        assert_eq!(integer_chord.pitched_common_name(), "Minor Second above C");

        let spelled_chord = Chord::new("C C#").unwrap();
        assert_eq!(spelled_chord.common_name(), "Augmented Unison");
        assert_eq!(
            spelled_chord.pitched_common_name(),
            "Augmented Unison above C"
        );

        let octave = Chord::new("D3 D4").unwrap();
        assert_eq!(octave.common_name(), "Perfect Octave");
        assert_eq!(octave.pitched_common_name(), "Perfect Octave above D");

        let compound = Chord::new("E-3 C5 C6").unwrap();
        assert_eq!(compound.common_name(), "Major Sixth with octave doublings");
        assert_eq!(
            compound.pitched_common_name(),
            "Major Sixth with octave doublings above Eb"
        );
    }

    #[test]
    fn chord_metadata_methods_have_forte_and_inversion() {
        let chord = Chord::new("C E G").unwrap();
        assert_eq!(chord.root_pitch_name().as_deref(), Some("C"));
        assert_eq!(chord.bass_pitch_name().as_deref(), Some("C"));
        assert_eq!(chord.inversion(), Some(0));
        assert_eq!(chord.inversion_name().unwrap(), Some(53));
        assert_eq!(chord.inversion_text(), "Root Position");
        assert_eq!(chord.forte_class().as_deref(), Some("3-11B"));
        assert_eq!(chord.interval_class_vector(), Some(vec![0, 0, 1, 1, 1, 0]));
        assert!(chord.invariance_vector().is_some());
        assert_eq!(chord.z_relation(), None);
        assert!(
            chord
                .common_names()
                .iter()
                .any(|name| name == "major triad")
        );
    }

    #[test]
    fn chord_simplifies_enharmonics_explicitly() {
        let chord = Chord::new("D# F## A#").unwrap();
        let simplified = chord.simplify_enharmonics(None).unwrap();
        assert_eq!(chord.pitches()[0].name(), "D#");
        assert_eq!(simplified.pitches().len(), chord.pitches().len());

        let mut in_place = chord.clone();
        in_place.simplify_enharmonics_in_place(None).unwrap();
        assert_eq!(
            simplified
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name_with_octave())
                .collect::<Vec<_>>(),
            in_place
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name_with_octave())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn chord_maps_to_reduced_polyrhythm_components() {
        let major = Chord::new("C E G").unwrap();
        assert_eq!(major.polyrhythm_components(), vec![4, 5, 6]);
        assert_eq!(major.polyrhythm_ratio_string(), "4:5:6");

        let empty = Chord::empty();
        assert_eq!(empty.polyrhythm_ratio_string(), "1");
    }

    #[test]
    fn new_rejects_invalid_pitch_inputs() {
        assert!(Chord::new("C nope G").is_err());
    }

    #[test]
    fn chord_supports_rust_conversion_traits() {
        let parsed: Chord = "C E G".parse().unwrap();
        assert_eq!(parsed.to_string(), "C-major triad");
        assert_eq!(parsed.notes().len(), 3);

        let from_str = Chord::try_from("C E G").unwrap();
        assert_eq!(from_str.pitched_common_name(), "C-major triad");

        let midi = [60, 64, 67];
        let from_slice = Chord::try_from(midi.as_slice()).unwrap();
        assert_eq!(from_slice.pitched_common_name(), "C-major triad");
    }

    #[test]
    fn known_chord_types_include_music21_table_names() {
        let known = Chord::known_chord_types();
        assert_eq!(known.len(), 351);
        assert!(
            known
                .iter()
                .any(|entry| entry.common_names.iter().any(|name| name == "major triad"))
        );
        assert!(known.iter().any(|entry| {
            entry
                .common_names
                .iter()
                .any(|name| name == "dominant seventh chord")
        }));
    }

    #[test]
    fn chord_first_inversion_detected() {
        let chord = Chord::new("E3 G3 C4").unwrap();
        assert_eq!(chord.inversion(), Some(1));
        assert_eq!(chord.inversion_name().unwrap(), Some(6));
        assert_eq!(chord.inversion_text(), "First Inversion");
        assert_eq!(
            Chord::new("C E G B-").unwrap().inversion_name().unwrap(),
            Some(7)
        );
        // A chord that is neither a triad nor carries a seventh has no
        // figured-bass number, which music21 reports by raising.
        assert!(Chord::new("C D E").unwrap().inversion_name().is_err());
    }

    #[test]
    fn dominant_seventh_resolves_to_tonic() {
        let chord = Chord::new("G3 B3 D4 F4").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "C-major triad");
    }

    #[test]
    fn resolution_chords_stay_near_source_register() {
        let chord = Chord::new("G2 B2 D3 F3").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();
        let names = resolution
            .pitches()
            .into_iter()
            .map(|pitch| pitch.name_with_octave())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["C3", "E3", "G3"]);
    }

    #[test]
    fn resolution_suggestions_infer_contexts() {
        let chord = Chord::new("G3 B3 D4 F4").unwrap();
        let suggestions = chord.resolution_suggestions().unwrap();

        assert!(suggestions.iter().any(|suggestion| {
            suggestion.key_context == "dominant resolution to C major"
                && suggestion.chord.pitched_common_name() == "C-major triad"
        }));
        assert!(suggestions.iter().any(|suggestion| {
            suggestion.key_context == "dominant resolution to C minor"
                && suggestion.chord.pitched_common_name() == "C-minor triad"
        }));
    }

    #[test]
    fn resolution_suggestions_stay_near_source_register() {
        let chord = Chord::new("G2 B2 D3 F3").unwrap();
        let suggestions = chord.resolution_suggestions().unwrap();
        let c_major = suggestions
            .iter()
            .find(|suggestion| suggestion.key_context == "dominant resolution to C major")
            .unwrap();
        let names = c_major
            .chord
            .pitches()
            .into_iter()
            .map(|pitch| pitch.name_with_octave())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["C3", "E3", "G3"]);
    }

    #[test]
    fn resolution_suggestions_can_use_explicit_key_context() {
        let secondary_dominant = Chord::new("D3 F#3 A3 C4").unwrap();
        let c_major = Key::from_tonic_mode("C", Some("major")).unwrap();
        let suggestions = secondary_dominant
            .resolution_suggestions_in_key(&c_major)
            .unwrap();

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].key_context, "dominant resolution in C major");
        assert_eq!(suggestions[0].chord.pitched_common_name(), "G-major triad");
    }

    #[test]
    fn dominant_seventh_resolves_to_minor_tonic() {
        let chord = Chord::new("G3 B3 D4 F4").unwrap();
        let resolution = chord.resolution_chord("C", Some("minor")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "C-minor triad");
    }

    #[test]
    fn secondary_dominant_resolves_to_diatonic_target() {
        let chord = Chord::new("D3 F#3 A3 C4").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "G-major triad");
    }

    #[test]
    fn dominant_extensions_resolve_to_tonic() {
        let dominant_ninth = Chord::new("G2 B2 D3 F3 A3").unwrap();
        let dominant_eleventh = Chord::new("G2 B2 D3 F3 A3 C4").unwrap();
        let dominant_thirteenth = Chord::new("G2 B2 D3 F3 A3 C4 E4").unwrap();

        for chord in [dominant_ninth, dominant_eleventh, dominant_thirteenth] {
            let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();
            assert_eq!(resolution.pitched_common_name(), "C-major triad");
        }
    }

    #[test]
    fn leading_tone_sevenths_resolve_by_semitone() {
        let fully_diminished = Chord::new("B3 D4 F4 A-4").unwrap();
        let half_diminished = Chord::new("B3 D4 F4 A4").unwrap();

        assert_eq!(
            fully_diminished
                .resolution_chord("C", Some("major"))
                .unwrap()
                .unwrap()
                .pitched_common_name(),
            "C-major triad"
        );
        assert_eq!(
            half_diminished
                .resolution_chord("C", Some("major"))
                .unwrap()
                .unwrap()
                .pitched_common_name(),
            "C-major triad"
        );
    }

    #[test]
    fn leading_tone_diminished_triad_resolves_by_semitone() {
        let chord = Chord::new("B3 D4 F4").unwrap();
        let resolution = chord.resolution_chord("C", Some("major")).unwrap().unwrap();

        assert_eq!(resolution.pitched_common_name(), "C-major triad");
    }

    #[test]
    fn contextual_augmented_sixth_resolves_to_dominant() {
        let german_augmented_sixth = Chord::new("A-3 C4 E-4 F#4").unwrap();
        let resolution = german_augmented_sixth
            .resolution_chord("C", Some("major"))
            .unwrap()
            .unwrap();

        assert_eq!(resolution.pitched_common_name(), "G-major triad");
    }

    #[test]
    fn unsupported_resolution_returns_none() {
        let tonic = Chord::new("C E G").unwrap();
        assert!(
            tonic
                .resolution_chord("C", Some("major"))
                .unwrap()
                .is_none()
        );
    }
}
