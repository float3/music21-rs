//! What kind of chord it is: the chord steps above the root, the triad
//! and seventh predicates, the augmented sixths and the quality they add
//! up to.

use super::*;

impl Chord {
    /// Returns the first pitch lying at the given chord step above the root,
    /// so `3` is the third and `7` the seventh. Steps of eight and above are
    /// folded down by an octave, so `9` finds a second.
    pub fn chord_step(&self, step: u8) -> Option<&Pitch> {
        self.chord_step_from(step, self.root()?)
    }

    /// Returns the third above the root, if the chord has one.
    pub fn third(&self) -> Option<&Pitch> {
        self.chord_step(3)
    }

    /// Returns the fifth above the root, if the chord has one.
    pub fn fifth(&self) -> Option<&Pitch> {
        self.chord_step(5)
    }

    /// Returns the seventh above the root, if the chord has one.
    pub fn seventh(&self) -> Option<&Pitch> {
        self.chord_step(7)
    }

    /// Returns the semitones from the root to the given chord step, within an
    /// octave, if the chord has that step.
    pub fn semitones_from_chord_step(&self, step: u8) -> Option<u8> {
        let root = self.root()?;
        let pitch = self.chord_step_from(step, root)?;
        Some(semitones_above(root, pitch))
    }

    /// Returns whether the chord has the given step spelled two different
    /// ways, such as both `E` and `E-` above `C`.
    pub fn has_repeated_chord_step(&self, step: u8) -> bool {
        let Some(root) = self.root() else {
            return false;
        };
        let step = fold_chord_step(step);
        let Some(first) = self
            .chord_step_from(step, root)
            .map(|pitch| semitones_above(root, pitch))
        else {
            return false;
        };
        self.pitch_refs().any(|pitch| {
            diatonic_steps_above(root, pitch) == step && semitones_above(root, pitch) != first
        })
    }

    /// Returns whether two pitches share a pitch class under different names,
    /// such as `C#` and `D-`.
    pub fn has_any_enharmonic_spelled_pitches(&self) -> bool {
        self.pitch_class_set().len() != self.unique_pitch_names().len()
    }

    /// Returns whether the chord is exactly three distinct pitch names with a
    /// third and a fifth above the root, of any quality.
    pub fn is_triad(&self) -> bool {
        self.unique_pitch_names().len() == 3 && self.third().is_some() && self.fifth().is_some()
    }

    /// Returns whether the chord is exactly four distinct pitch names with a
    /// third, fifth and seventh above the root, of any quality.
    pub fn is_seventh(&self) -> bool {
        self.unique_pitch_names().len() == 4
            && self.third().is_some()
            && self.fifth().is_some()
            && self.seventh().is_some()
    }

    /// Returns whether the chord is a correctly spelled major triad.
    pub fn is_major_triad(&self) -> bool {
        self.is_triad_of_type((3, 11, -1), 4, 7)
    }

    /// Returns whether the chord is a correctly spelled minor triad.
    pub fn is_minor_triad(&self) -> bool {
        self.is_triad_of_type((3, 11, 1), 3, 7)
    }

    /// Returns whether the chord is a correctly spelled diminished triad.
    pub fn is_diminished_triad(&self) -> bool {
        self.is_triad_of_type((3, 10, 0), 3, 6)
    }

    /// Returns whether the chord is a correctly spelled augmented triad.
    pub fn is_augmented_triad(&self) -> bool {
        self.is_triad_of_type((3, 12, 0), 4, 8)
    }

    /// Returns whether the chord is a seventh chord whose pitches all lie at
    /// the given semitone offsets above the root.
    pub fn is_seventh_of_type(&self, semitones: &[u8]) -> bool {
        if !self.is_seventh() {
            return false;
        }
        let Some(root) = self.root() else {
            return false;
        };
        self.pitch_refs()
            .all(|pitch| semitones.contains(&semitones_above(root, pitch)))
    }

    /// Returns whether the chord is a dominant seventh: a major triad with a
    /// minor seventh.
    pub fn is_dominant_seventh(&self) -> bool {
        self.is_seventh_of_type(&[0, 4, 7, 10])
    }

    /// Returns whether the chord is a half-diminished seventh.
    pub fn is_half_diminished_seventh(&self) -> bool {
        self.is_seventh_of_type(&[0, 3, 6, 10])
    }

    /// Returns whether the chord is a fully diminished seventh.
    pub fn is_diminished_seventh(&self) -> bool {
        self.is_seventh_of_type(&[0, 3, 6, 9])
    }

    /// Returns whether the chord is only a root and a major third above it.
    pub fn is_incomplete_major_triad(&self) -> bool {
        self.is_incomplete_triad_of_type((2, 4), 4)
    }

    /// Returns whether the chord is only a root and a minor third above it.
    pub fn is_incomplete_minor_triad(&self) -> bool {
        self.is_incomplete_triad_of_type((2, 3), 3)
    }

    /// Returns whether the chord has a third and a fifth above its root. A
    /// dominant seventh is not a triad but contains one.
    pub fn contains_triad(&self) -> bool {
        self.third().is_some() && self.fifth().is_some()
    }

    /// Returns whether the chord contains a triad and a seventh above its root.
    pub fn contains_seventh(&self) -> bool {
        self.contains_triad() && self.seventh().is_some()
    }

    /// Returns the quality of the triad above the root, following music21's
    /// `Chord.quality`: incomplete triads still count, and a chord with a
    /// repeated or missing chord step is [`TriadQuality::Other`].
    pub fn quality(&self) -> TriadQuality {
        let Some(third) = self.semitones_from_chord_step(3) else {
            return TriadQuality::Other;
        };
        if self.has_repeated_chord_step(1) || self.has_repeated_chord_step(3) {
            return TriadQuality::Other;
        }
        let Some(fifth) = self.semitones_from_chord_step(5) else {
            return match third {
                4 => TriadQuality::Major,
                3 => TriadQuality::Minor,
                _ => TriadQuality::Other,
            };
        };
        if self.has_repeated_chord_step(5) {
            return TriadQuality::Other;
        }
        match (third, fifth) {
            (4, 7) => TriadQuality::Major,
            (3, 7) => TriadQuality::Minor,
            (4, 8) => TriadQuality::Augmented,
            (3, 6) => TriadQuality::Diminished,
            _ => TriadQuality::Other,
        }
    }

    /// Returns whether the chord is consonant in the common-practice sense:
    /// one pitch name, two whose closed-position interval is consonant, or a
    /// major or minor triad not in second inversion.
    pub fn is_consonant(&self) -> bool {
        let distinct = self.remove_redundant_pitch_names();
        match distinct.notes.len() {
            1 => true,
            2 => {
                let closed = self.closed_position(None, false).remove_redundant_pitches();
                Interval::between_pitches(&closed.notes[0].pitch, &closed.notes[1].pitch)
                    .is_ok_and(|interval| interval.is_consonant())
            }
            3 => (self.is_major_triad() || self.is_minor_triad()) && self.inversion() != Some(2),
            _ => false,
        }
    }

    /// The first pitch at the given chord step above a root the caller
    /// decided on, rather than the one the chord infers: music21's
    /// `getChordStep(step, testRoot)`.
    pub fn chord_step_with_root(&self, step: u8, root: &Pitch) -> Option<&Pitch> {
        self.chord_step_from(step, root)
    }

    /// The semitone distance from a caller-supplied root to the pitch at the
    /// given chord step: music21's `semitonesFromChordStep(step, testRoot)`.
    pub fn semitones_from_chord_step_with_root(&self, step: u8, root: &Pitch) -> Option<u8> {
        let pitch = self.chord_step_from(step, root)?;
        Some(semitones_above(root, pitch))
    }

    /// The inversion measured from a root the caller decided on: music21's
    /// `inversion(testRoot=...)`.
    pub fn inversion_from_root(&self, root: &Pitch) -> Option<u8> {
        self.inversion_with_root(root)
    }

    pub(crate) fn chord_step_from(&self, step: u8, root: &Pitch) -> Option<&Pitch> {
        let step = fold_chord_step(step);
        self.pitch_refs()
            .find(|pitch| diatonic_steps_above(root, pitch) == step)
    }

    pub(super) fn is_triad_of_type(
        &self,
        address: (u8, u8, i8),
        third_semitones: u8,
        fifth_semitones: u8,
    ) -> bool {
        if self.forte_address() != Some(address) {
            return false;
        }
        if !self.is_triad() || self.has_any_enharmonic_spelled_pitches() {
            return false;
        }
        let (Some(root), Some(third), Some(fifth)) = (self.root(), self.third(), self.fifth())
        else {
            return false;
        };
        semitones_above(root, third) == third_semitones
            && semitones_above(root, fifth) == fifth_semitones
    }

    pub(super) fn is_incomplete_triad_of_type(
        &self,
        address: (u8, u8),
        third_semitones: u8,
    ) -> bool {
        if self
            .forte_address()
            .is_none_or(|(card, index, _)| (card, index) != address)
        {
            return false;
        }
        let (Some(root), Some(_)) = (self.root(), self.third()) else {
            return false;
        };
        self.pitch_refs()
            .all(|pitch| [0, third_semitones].contains(&semitones_above(root, pitch)))
    }

    /// Returns whether the chord is an Italian, French, German or Swiss
    /// augmented sixth. Each must be in its conventional inversion unless
    /// `permit_any_inversion` is set.
    pub fn is_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        match self.pitch_class_cardinality() {
            3 => self.is_italian_augmented_sixth(permit_any_inversion, false),
            4 => {
                self.is_french_augmented_sixth(permit_any_inversion)
                    || self.is_german_augmented_sixth(permit_any_inversion)
                    || self.is_swiss_augmented_sixth(permit_any_inversion)
            }
            _ => false,
        }
    }

    /// Returns whether the chord is an Italian augmented sixth, such as
    /// `A- C F#`.
    pub fn is_italian_augmented_sixth(
        &self,
        permit_any_inversion: bool,
        restrict_doublings: bool,
    ) -> bool {
        if !self.is_augmented_sixth_of_type(
            (3, 8, 1),
            1,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.italian,
        ) {
            return false;
        }
        if !restrict_doublings {
            return true;
        }
        let (Some(root), Some(third), Some(fifth)) = (self.root(), self.third(), self.fifth())
        else {
            return false;
        };
        self.pitch_refs().all(|pitch| {
            pitch.name() == fifth.name() || std::ptr::eq(pitch, third) || std::ptr::eq(pitch, root)
        })
    }

    /// Returns whether the chord is a French augmented sixth, such as
    /// `A- C D F#`.
    pub fn is_french_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        self.is_augmented_sixth_of_type(
            (4, 25, 0),
            2,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.french,
        )
    }

    /// Returns whether the chord is a German augmented sixth, such as
    /// `A- C E- F#`.
    pub fn is_german_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        self.is_augmented_sixth_of_type(
            (4, 27, -1),
            1,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.german,
        )
    }

    /// Returns whether the chord is a Swiss augmented sixth, such as
    /// `A- C D# F#`.
    pub fn is_swiss_augmented_sixth(&self, permit_any_inversion: bool) -> bool {
        self.is_augmented_sixth_of_type(
            (4, 27, -1),
            2,
            permit_any_inversion,
            &AUGMENTED_SIXTHS.swiss,
        )
    }

    /// Returns whether the chord is five distinct pitch names with a third,
    /// fifth, seventh and ninth above the root.
    pub fn is_ninth(&self) -> bool {
        self.unique_pitch_names().len() == 5
            && self.third().is_some()
            && self.fifth().is_some()
            && self.seventh().is_some()
            && self.chord_step(2).is_some()
    }

    /// Returns whether some transposition other than the octave maps the
    /// pitch-class set onto itself. With `require_intervallic_evenness` only
    /// the evenly spaced sets count, the way Straus defines the property.
    pub fn is_transpositionally_symmetrical(&self, require_intervallic_evenness: bool) -> bool {
        let Some((card, index, _)) = self.forte_address() else {
            return self.notes.is_empty();
        };
        if card == 1 {
            return require_intervallic_evenness;
        }
        const EVEN: [(u8, u8); 5] = [(2, 6), (3, 12), (4, 28), (6, 35), (12, 1)];
        const UNEVEN: [(u8, u8); 10] = [
            (4, 9),
            (4, 25),
            (6, 7),
            (6, 20),
            (6, 30),
            (8, 9),
            (8, 25),
            (8, 28),
            (9, 12),
            (10, 6),
        ];
        EVEN.contains(&(card, index))
            || (!require_intervallic_evenness && UNEVEN.contains(&(card, index)))
    }

    /// Returns whether two pitches share a letter under different
    /// accidentals, such as `E` and `E-`.
    pub fn has_any_repeated_diatonic_note(&self) -> bool {
        let steps = self
            .pitch_refs()
            .map(Pitch::step)
            .collect::<std::collections::BTreeSet<_>>();
        steps.len() != self.unique_pitch_names().len()
    }

    pub(super) fn is_augmented_sixth_of_type(
        &self,
        address: (u8, u8, i8),
        required_inversion: u8,
        permit_any_inversion: bool,
        intervals: &[[Interval; 2]],
    ) -> bool {
        if self.forte_address() != Some(address) || self.has_any_enharmonic_spelled_pitches() {
            return false;
        }
        if !permit_any_inversion && self.inversion() != Some(required_inversion) {
            return false;
        }
        let Some(root) = self.root() else {
            return false;
        };
        let steps = [self.third(), self.fifth(), self.seventh()];
        intervals.iter().zip(steps).all(|(accepted, step)| {
            step.and_then(|pitch| Interval::between_pitches(root, pitch).ok())
                .is_some_and(|interval| {
                    accepted.iter().any(|candidate| {
                        candidate.directed_simple_key() == interval.directed_simple_key()
                    })
                })
        })
    }

    pub(super) fn has_intervals_above_root(&self, intervals: &[u8]) -> bool {
        let Some(root_pitch) = self.find_root_pitch() else {
            return false;
        };
        let root_pc = root::pitch_class(root_pitch);
        let chord_pcs = self.pitch_class_set();
        intervals
            .iter()
            .all(|interval| chord_pcs.contains(&((root_pc + interval) % 12)))
    }

    /// Returns the interval from the root to the first pitch lying on the
    /// given chord step, compound intervals included, so the third of
    /// `C3 G3 E4 C5` is a major tenth. `None` when the chord has no root or
    /// no pitch on that step.
    pub fn interval_from_chord_step(&self, step: u8) -> Option<Interval> {
        let root = self.root()?;
        self.pitch_refs()
            .filter_map(|pitch| Interval::between_pitches(root, pitch).ok())
            .find(|interval| interval.mod7() == IntegerType::from(step))
    }
}

/// The quality of the triad above a chord's root, as music21's
/// `Chord.quality` reports it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TriadQuality {
    /// A major third with a perfect fifth, or a major third alone.
    Major,
    /// A minor third with a perfect fifth, or a minor third alone.
    Minor,
    /// A major third with an augmented fifth.
    Augmented,
    /// A minor third with a diminished fifth.
    Diminished,
    /// Anything else, including a missing third or a repeated chord step.
    Other,
}

impl TriadQuality {
    /// Returns music21's lowercase name for the quality.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Augmented => "augmented",
            Self::Diminished => "diminished",
            Self::Other => "other",
        }
    }
}

impl Display for TriadQuality {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub(super) fn fold_chord_step(step: u8) -> u8 {
    if step >= 8 { step - 7 } else { step }
}

pub(super) fn diatonic_steps_above(root: &Pitch, pitch: &Pitch) -> u8 {
    ((root::step_num(pitch) - root::step_num(root)).rem_euclid(7) + 1) as u8
}

pub(super) fn semitones_above(root: &Pitch, pitch: &Pitch) -> u8 {
    (root::pitch_class(pitch) + 12 - root::pitch_class(root)) % 12
}

pub(super) struct AugmentedSixthIntervals {
    italian: [[Interval; 2]; 2],
    french: [[Interval; 2]; 3],
    german: [[Interval; 2]; 3],
    swiss: [[Interval; 2]; 3],
}

/// The intervals each augmented-sixth type stacks above its root, as music21
/// spells them: the third and fifth (and seventh) may each be written either
/// way up.
pub(super) static AUGMENTED_SIXTHS: LazyLock<AugmentedSixthIntervals> = LazyLock::new(|| {
    let pair = |up: &str, down: &str| {
        [
            Interval::from_name(up).expect("augmented sixth intervals parse"),
            Interval::from_name(down).expect("augmented sixth intervals parse"),
        ]
    };
    AugmentedSixthIntervals {
        italian: [pair("d3", "A-6"), pair("d5", "A-4")],
        french: [pair("M3", "m-6"), pair("d5", "A-4"), pair("m7", "M-2")],
        german: [pair("d3", "A-6"), pair("d5", "A-4"), pair("d7", "A-2")],
        swiss: [pair("m3", "M-6"), pair("dd5", "AA-4"), pair("d7", "A-2")],
    }
});
