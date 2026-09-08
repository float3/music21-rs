//! Two-voice voice-leading checks, a port of music21's `VoiceLeadingQuartet`.
//!
//! A quartet is two consecutive notes in an upper voice and two in a lower
//! voice. It classifies how the voices move between them and finds the
//! parallel and hidden perfect intervals that common-practice counterpoint
//! forbids.

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    interval::{Interval, IntervalDirection},
    key::Key,
    pitch::Pitch,
    scale::{Scale, ScaleType},
};
use std::sync::LazyLock;

static PERFECT_UNISON: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P1").expect("P1 is a valid interval"));
static PERFECT_FIFTH: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P5").expect("P5 is a valid interval"));
static PERFECT_OCTAVE: LazyLock<Interval> =
    LazyLock::new(|| Interval::from_name("P8").expect("P8 is a valid interval"));

/// How two voices move relative to each other.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MotionType {
    /// Contrary motion between two spellings of the same simple interval,
    /// such as a fifth opening out to a twelfth. Only reported when asked for.
    AntiParallel,
    /// The voices move in opposite directions.
    Contrary,
    /// Neither voice moves.
    NoMotion,
    /// One voice holds while the other moves.
    Oblique,
    /// The voices move the same way and keep the same generic interval.
    Parallel,
    /// The voices move the same way through different intervals.
    Similar,
}

impl MotionType {
    /// Returns music21's label for the motion.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AntiParallel => "Anti-Parallel",
            Self::Contrary => "Contrary",
            Self::NoMotion => "No Motion",
            Self::Oblique => "Oblique",
            Self::Parallel => "Parallel",
            Self::Similar => "Similar",
        }
    }
}

impl std::fmt::Display for MotionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a caller may ask of a parallel motion.
///
/// music21 takes either: a number is how wide the interval must be, whatever
/// it is spelled — `3` for parallel thirds of any quality — and an interval
/// is the interval itself, spelling and all.
#[derive(Clone, Debug, PartialEq)]
pub enum ParallelRequirement {
    /// However many steps wide, counted inclusively.
    Wide(IntegerType),
    /// This interval exactly.
    Named(Box<Interval>),
}

impl From<Interval> for ParallelRequirement {
    fn from(interval: Interval) -> Self {
        Self::Named(Box::new(interval))
    }
}

impl From<IntegerType> for ParallelRequirement {
    fn from(steps: IntegerType) -> Self {
        Self::Wide(steps)
    }
}

/// Two consecutive notes in each of two voices. Voice one is the upper voice.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VoiceLeadingQuartet {
    v1n1: Pitch,
    v1n2: Pitch,
    v2n1: Pitch,
    v2n2: Pitch,
    vertical: [Interval; 2],
    horizontal: [Interval; 2],
    key: Option<Key>,
}

impl VoiceLeadingQuartet {
    /// Builds a quartet from the first and second pitch of the upper voice,
    /// then the first and second pitch of the lower voice.
    pub fn new(v1n1: Pitch, v1n2: Pitch, v2n1: Pitch, v2n2: Pitch) -> Result<Self> {
        let vertical = [
            Interval::between_pitches(&v1n1, &v2n1)?,
            Interval::between_pitches(&v1n2, &v2n2)?,
        ];
        let horizontal = [
            Interval::between_pitches(&v1n1, &v1n2)?,
            Interval::between_pitches(&v2n1, &v2n2)?,
        ];
        Ok(Self {
            v1n1,
            v1n2,
            v2n1,
            v2n2,
            vertical,
            horizontal,
            key: None,
        })
    }

    /// Builds a quartet from pitch names, in the same order as [`Self::new`].
    pub fn from_names(v1n1: &str, v1n2: &str, v2n1: &str, v2n2: &str) -> Result<Self> {
        Self::new(
            Pitch::from_name(v1n1)?,
            Pitch::from_name(v1n2)?,
            Pitch::from_name(v2n1)?,
            Pitch::from_name(v2n2)?,
        )
    }

    /// Attaches the key the progression is heard in, which
    /// [`Self::is_proper_resolution`] and [`Self::clausula_vera`] consult.
    pub fn with_key(mut self, key: Key) -> Self {
        self.key = Some(key);
        self
    }

    /// The key the progression is heard in, if one was given.
    pub fn key(&self) -> Option<&Key> {
        self.key.as_ref()
    }

    /// Attaches a key, or takes one away.
    pub fn set_key(&mut self, key: Option<Key>) {
        self.key = key;
    }

    /// Whether a dissonant first interval resolves the way voice-leading
    /// rules expect: music21's `isProperResolution`. A fourth wants the upper
    /// voice to fall or stay; a tritone or a minor seventh wants the
    /// leading tone up, the seventh down and the right contrary motion — and
    /// with a key set, only when the notes really are those scale degrees.
    /// Anything else, and no motion at all, is proper.
    pub fn is_proper_resolution(&self) -> Result<bool> {
        if self.no_motion() {
            return Ok(true);
        }
        let (lower_degree_before, lower_degree_after) = match &self.key {
            Some(key) => {
                let scale = key.as_scale()?;
                let mut before = scale.degree_of(&self.v2n1)?;
                let after = scale.degree_of(&self.v2n2)?;
                if key.mode() == "minor" && before.is_none() {
                    before =
                        Scale::new(ScaleType::MelodicMinor, key.tonic()).degree_of(&self.v2n1)?;
                }
                (before, after)
            }
            None => (None, None),
        };
        let keyed = self.key.is_some();
        let first = self.vertical[0].simple_name();
        let second = self.vertical[1].generic().simple_undirected();
        Ok(match first.as_str() {
            "P4" => self.v1n1.ps() >= self.v1n2.ps(),
            "A4" => {
                if keyed && lower_degree_before != Some(4) {
                    true
                } else if keyed && lower_degree_after != Some(3) {
                    false
                } else {
                    self.outward_contrary_motion() && second == 6
                }
            }
            "d5" => {
                if keyed && lower_degree_before != Some(7) {
                    true
                } else if keyed && lower_degree_after != Some(1) {
                    false
                } else {
                    self.inward_contrary_motion() && second == 3
                }
            }
            "m7" => {
                if keyed && lower_degree_before != Some(5) {
                    true
                } else if keyed && lower_degree_after != Some(1) {
                    false
                } else {
                    second == 3
                }
            }
            _ => true,
        })
    }

    /// Whether one voice leaps while the other neither steps nor holds:
    /// music21's `leapNotSetWithStep`. Two thirds in contrary motion are let
    /// through.
    pub fn leap_not_set_with_step(&self) -> bool {
        if self.no_motion() {
            return false;
        }
        let [upper, lower] = &self.horizontal;
        let steps_or_holds =
            |interval: &Interval| interval.is_diatonic_step() || interval.is_unison();
        if upper.generic().undirected() == 3
            && lower.generic().undirected() == 3
            && self.contrary_motion()
        {
            return false;
        }
        if upper.is_skip() {
            !steps_or_holds(lower)
        } else if lower.is_skip() {
            !steps_or_holds(upper)
        } else {
            false
        }
    }

    /// Whether the two voices open the way sixteenth-century counterpoint
    /// opens: music21's `modalOpening`. Errors without a key.
    ///
    /// One of the two harmonic intervals must be a unison or a fifth — the
    /// second may be, to allow for an anacrusis — and the pair must establish
    /// the tonic or the dominant. Which of the two says so is whichever can
    /// be read at all: music21 asks the first, and only falls to the second
    /// when the first says nothing.
    pub fn modal_opening(&self) -> Result<bool> {
        let Some(key) = &self.key else {
            return Err(Error::Analysis(
                "modalOpening requires a key to be set on the VoiceLeadingQuartet".to_string(),
            ));
        };
        let opening = ["P1", "P5"];
        let sounds_open = opening.contains(&self.vertical[0].simple_name().as_str())
            || opening.contains(&self.vertical[1].simple_name().as_str());
        let function_of = |first: &Pitch, second: &Pitch| -> Result<Option<bool>> {
            let chord = crate::chord::Chord::new([first.clone(), second.clone()].as_slice())?;
            Ok(
                crate::roman::identify_as_tonic_or_dominant(&chord, key)?.map(|figure| {
                    figure
                        .chars()
                        .next()
                        .is_some_and(|numeral| matches!(numeral.to_ascii_uppercase(), 'I' | 'V'))
                }),
            )
        };
        let established = match function_of(&self.v1n1, &self.v2n1)? {
            Some(established) => established,
            None => function_of(&self.v1n2, &self.v2n2)?.unwrap_or(false),
        };
        Ok(sounds_open && established)
    }

    /// Whether the two voices close a clausula vera: stepwise contrary
    /// motion, one voice by a semitone and the other by a tone, onto a unison
    /// or octave on the tonic. Errors without a key.
    pub fn clausula_vera(&self) -> Result<bool> {
        let Some(key) = &self.key else {
            return Err(Error::Analysis(
                "clausulaVera requires a key to be set on the VoiceLeadingQuartet".to_string(),
            ));
        };
        let tonic = key.tonic().name();
        let mut horizontal = [
            self.horizontal[0].short_name(),
            self.horizontal[1].short_name(),
        ];
        horizontal.sort_unstable();
        Ok(horizontal == ["M2", "m2"]
            && self.contrary_motion()
            && matches!(self.vertical[1].short_name().as_str(), "P1" | "P8")
            && self.v1n2.name() == tonic
            && self.v2n2.name() == tonic)
    }

    /// The upper voice's first pitch.
    pub fn v1n1(&self) -> &Pitch {
        &self.v1n1
    }

    /// The upper voice's second pitch.
    pub fn v1n2(&self) -> &Pitch {
        &self.v1n2
    }

    /// The lower voice's first pitch.
    pub fn v2n1(&self) -> &Pitch {
        &self.v2n1
    }

    /// The lower voice's second pitch.
    pub fn v2n2(&self) -> &Pitch {
        &self.v2n2
    }

    /// The harmonic intervals from the upper voice to the lower voice, at the
    /// first and second moment.
    pub fn vertical_intervals(&self) -> &[Interval; 2] {
        &self.vertical
    }

    /// The melodic intervals each voice moves through, upper voice first.
    pub fn horizontal_intervals(&self) -> &[Interval; 2] {
        &self.horizontal
    }

    /// Classifies the motion. Anti-parallel motion is reported as contrary
    /// unless `allow_anti_parallel` is set.
    pub fn motion_type(&self, allow_anti_parallel: bool) -> MotionType {
        if self.oblique_motion() {
            MotionType::Oblique
        } else if self.parallel_motion(None, false) {
            MotionType::Parallel
        } else if self.similar_motion() {
            MotionType::Similar
        } else if allow_anti_parallel && self.anti_parallel_motion(None) {
            MotionType::AntiParallel
        } else if self.contrary_motion() {
            MotionType::Contrary
        } else {
            MotionType::NoMotion
        }
    }

    /// Returns whether neither voice moves.
    pub fn no_motion(&self) -> bool {
        self.horizontal.iter().all(Interval::is_perfect_unison)
    }

    /// Returns whether exactly one voice holds its pitch.
    pub fn oblique_motion(&self) -> bool {
        !self.no_motion() && self.horizontal.iter().any(Interval::is_perfect_unison)
    }

    /// Returns whether both voices move in the same direction.
    pub fn similar_motion(&self) -> bool {
        !self.no_motion() && self.horizontal[0].direction() == self.horizontal[1].direction()
    }

    /// Returns whether the voices move in the same direction keeping the same
    /// generic interval. With `required`, the interval must also be that one;
    /// `allow_octave_displacement` accepts a fifth answered by a twelfth.
    pub fn parallel_motion(
        &self,
        required: Option<&ParallelRequirement>,
        allow_octave_displacement: bool,
    ) -> bool {
        let [first, second] = &self.vertical;
        if !self.similar_motion() {
            return false;
        }
        if first.generic().directed() != second.generic().directed() && !allow_octave_displacement {
            return false;
        }
        if first.generic().semi_simple_undirected() != second.generic().semi_simple_undirected() {
            return false;
        }
        match required {
            None => true,
            Some(ParallelRequirement::Wide(steps)) => {
                first.generic().semi_simple_undirected() == *steps
            }
            Some(ParallelRequirement::Named(required)) => {
                first.semi_simple_key() == required.semi_simple_key()
                    && second.semi_simple_key() == required.semi_simple_key()
            }
        }
    }

    /// Returns whether the voices move in opposite directions.
    pub fn contrary_motion(&self) -> bool {
        !self.no_motion()
            && !self.oblique_motion()
            && self.horizontal[0].direction() != self.horizontal[1].direction()
    }

    /// Returns whether the voices move apart.
    pub fn outward_contrary_motion(&self) -> bool {
        self.contrary_motion() && self.horizontal[0].direction() == IntervalDirection::Ascending
    }

    /// Returns whether the voices move towards each other.
    pub fn inward_contrary_motion(&self) -> bool {
        self.contrary_motion() && self.horizontal[0].direction() == IntervalDirection::Descending
    }

    /// Returns whether contrary motion lands on the same simple interval it
    /// left, such as a fifth opening out to a twelfth. With `required`, that
    /// interval must also be the given one.
    pub fn anti_parallel_motion(&self, required: Option<&Interval>) -> bool {
        let [first, second] = &self.vertical;
        self.contrary_motion()
            && first.simple_key() == second.simple_key()
            && required.is_none_or(|required| first.simple_key() == required.simple_key())
    }

    /// Returns whether the voices move in parallel or anti-parallel through
    /// the given interval, in any octave.
    pub fn parallel_interval(&self, interval: &Interval) -> bool {
        let required = ParallelRequirement::Named(Box::new(interval.clone()));
        self.parallel_motion(Some(&required), true) || self.anti_parallel_motion(Some(interval))
    }

    /// Returns whether the voices move in parallel fifths.
    pub fn parallel_fifth(&self) -> bool {
        self.parallel_interval(&PERFECT_FIFTH)
    }

    /// Returns whether the voices move in parallel octaves.
    pub fn parallel_octave(&self) -> bool {
        self.parallel_interval(&PERFECT_OCTAVE)
    }

    /// Returns whether the voices move in parallel unisons.
    pub fn parallel_unison(&self) -> bool {
        self.parallel_interval(&PERFECT_UNISON)
    }

    /// Returns whether the voices move in parallel unisons or octaves.
    pub fn parallel_unison_or_octave(&self) -> bool {
        self.parallel_unison() || self.parallel_octave()
    }

    /// Returns whether similar motion arrives at the given interval without
    /// having started from it.
    pub fn hidden_interval(&self, interval: &Interval) -> bool {
        if self.parallel_motion(None, true) || !self.similar_motion() {
            return false;
        }
        self.vertical[1].simple_key() == interval.simple_key()
    }

    /// Returns whether similar motion arrives at a perfect fifth.
    pub fn hidden_fifth(&self) -> bool {
        self.hidden_interval(&PERFECT_FIFTH)
    }

    /// Returns whether similar motion arrives at a perfect octave.
    pub fn hidden_octave(&self) -> bool {
        self.hidden_interval(&PERFECT_OCTAVE)
    }

    /// Returns whether a voice moves past where the other voice just was.
    pub fn voice_overlap(&self) -> bool {
        self.v1n2.ps() < self.v2n1.ps() || self.v2n2.ps() > self.v1n1.ps()
    }

    /// Returns whether the lower voice is above the upper voice at either
    /// moment.
    pub fn voice_crossing(&self) -> bool {
        self.v1n1.ps() < self.v2n1.ps() || self.v1n2.ps() < self.v2n2.ps()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_modal_opening_needs_a_perfect_interval_and_a_tonic_or_dominant() {
        // music21's own examples.
        let opening = |v1n1, v1n2, v2n1, v2n2, key: &str| {
            VoiceLeadingQuartet::from_names(v1n1, v1n2, v2n1, v2n2)
                .unwrap()
                .with_key(Key::from_tonic(key).unwrap())
                .modal_opening()
                .unwrap()
        };
        assert!(opening("D", "D", "D", "F#", "D"));
        assert!(opening("B", "A", "G#", "A", "A"));
        assert!(opening("A", "A", "F#", "D", "A"));
        assert!(!opening("C#", "C#", "D", "E", "A"));
        assert!(!opening("B", "B", "A", "A", "C"));

        // Without a key there is nothing to be the tonic of.
        assert!(
            VoiceLeadingQuartet::from_names("D", "D", "D", "F#")
                .unwrap()
                .modal_opening()
                .is_err()
        );
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn proper_resolution_leaps_and_clausula_vera_match_music21() {
        let quartet = |a: &str, b: &str, c: &str, d: &str, key: Option<&str>| {
            let built = VoiceLeadingQuartet::from_names(a, b, c, d).unwrap();
            match key {
                Some(key) => built.with_key(Key::from_tonic(key).unwrap()),
                None => built,
            }
        };
        let cases: [(
            &str,
            &str,
            &str,
            &str,
            Option<&str>,
            bool,
            bool,
            Option<bool>,
        ); 20] = [
            ("C4", "B3", "F4", "E4", None, true, false, None),
            ("C4", "B3", "F4", "E4", Some("C"), true, false, Some(false)),
            ("B3", "C4", "F4", "E4", Some("C"), true, false, Some(false)),
            ("B3", "C4", "F4", "E4", None, false, false, None),
            ("F4", "E4", "B3", "C4", Some("C"), true, false, Some(false)),
            ("F4", "E4", "B3", "C4", None, true, false, None),
            ("G3", "C4", "F4", "E4", Some("C"), true, false, Some(false)),
            ("G3", "C4", "F4", "E4", Some("G"), true, false, Some(false)),
            ("C4", "C4", "E4", "E4", Some("C"), true, false, Some(false)),
            ("D4", "F4", "F4", "A4", None, true, true, None),
            ("C4", "F4", "E4", "F4", None, true, false, None),
            ("C4", "E4", "E4", "G4", None, true, true, None),
            ("C4", "E4", "E4", "C4", None, true, false, None),
            ("B3", "C4", "D4", "C4", Some("C"), true, false, Some(true)),
            ("B3", "C4", "D4", "C4", Some("G"), true, false, Some(false)),
            ("B3", "C4", "D4", "C5", Some("C"), true, false, Some(false)),
            ("F4", "E4", "G3", "C4", Some("C"), true, false, Some(false)),
            ("F4", "E4", "B3", "C4", Some("F"), true, false, Some(false)),
            ("D4", "C4", "B3", "C4", Some("C"), true, false, Some(true)),
            ("C4", "D4", "G4", "F4", None, true, false, None),
        ];
        for (a, b, c, d, key, proper, leap, clausula) in cases {
            let vlq = quartet(a, b, c, d, key);
            assert_eq!(
                vlq.is_proper_resolution().unwrap(),
                proper,
                "{a} {b} {c} {d} {key:?}"
            );
            assert_eq!(
                vlq.leap_not_set_with_step(),
                leap,
                "{a} {b} {c} {d} {key:?}"
            );
            match clausula {
                Some(expected) => assert_eq!(
                    vlq.clausula_vera().unwrap(),
                    expected,
                    "{a} {b} {c} {d} {key:?}"
                ),
                None => assert!(vlq.clausula_vera().is_err(), "{a} {b} {c} {d}"),
            }
            assert_eq!(vlq.key().is_some(), key.is_some());
        }
    }
    use super::*;

    struct Expected {
        motion: MotionType,
        with_anti_parallel: MotionType,
        flags: [bool; 15],
    }

    #[test]
    fn quartets_match_music21() {
        use MotionType::*;
        let t = true;
        let f = false;
        let cases = [
            (
                ("C4", "D4", "C3", "D3"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, f, t, f, f, f, f, f],
                },
            ),
            (
                ("C4", "D4", "E3", "F3"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("C4", "G4", "C3", "C3"),
                Expected {
                    motion: Oblique,
                    with_anti_parallel: Oblique,
                    flags: [f, t, f, f, f, f, f, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("C4", "C4", "C3", "C3"),
                Expected {
                    motion: NoMotion,
                    with_anti_parallel: NoMotion,
                    flags: [t, f, f, f, f, f, f, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("C4", "D4", "G3", "F3"),
                Expected {
                    motion: Contrary,
                    with_anti_parallel: Contrary,
                    flags: [f, f, f, f, t, t, f, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("C4", "C5", "F3", "F4"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, t, f, f, f, f, t, f],
                },
            ),
            (
                ("C5", "D5", "C4", "D4"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, f, t, f, f, f, f, f],
                },
            ),
            (
                ("C4", "D4", "G4", "A4"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, t, f, f, f, f, t, t],
                },
            ),
            (
                ("C5", "D5", "G4", "E5"),
                Expected {
                    motion: Similar,
                    with_anti_parallel: Similar,
                    flags: [f, f, t, f, f, f, f, f, f, f, f, f, f, t, t],
                },
            ),
            (
                ("E4", "F4", "C4", "A3"),
                Expected {
                    motion: Contrary,
                    with_anti_parallel: Contrary,
                    flags: [f, f, f, f, t, t, f, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("G4", "F4", "C4", "D4"),
                Expected {
                    motion: Contrary,
                    with_anti_parallel: Contrary,
                    flags: [f, f, f, f, t, f, t, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("C4", "C#4", "C3", "C#3"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, f, t, f, f, f, f, f],
                },
            ),
            (
                ("C4", "B3", "F3", "G3"),
                Expected {
                    motion: Contrary,
                    with_anti_parallel: Contrary,
                    flags: [f, f, f, f, t, f, t, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("D5", "A5", "G3", "D4"),
                Expected {
                    motion: Parallel,
                    with_anti_parallel: Parallel,
                    flags: [f, f, t, t, f, f, f, f, t, f, f, f, f, f, f],
                },
            ),
            (
                ("A4", "B4", "F4", "E4"),
                Expected {
                    motion: Contrary,
                    with_anti_parallel: Contrary,
                    flags: [f, f, f, f, t, t, f, f, f, f, f, f, f, f, f],
                },
            ),
            (
                ("C5", "C5", "E4", "F4"),
                Expected {
                    motion: Oblique,
                    with_anti_parallel: Oblique,
                    flags: [f, t, f, f, f, f, f, f, f, f, f, f, f, f, f],
                },
            ),
        ];
        for ((a, b, c, d), expected) in cases {
            let quartet = VoiceLeadingQuartet::from_names(a, b, c, d).unwrap();
            let label = format!("{a} {b} / {c} {d}");
            assert_eq!(quartet.motion_type(false), expected.motion, "{label}");
            assert_eq!(
                quartet.motion_type(true),
                expected.with_anti_parallel,
                "{label}"
            );
            let actual = [
                quartet.no_motion(),
                quartet.oblique_motion(),
                quartet.similar_motion(),
                quartet.parallel_motion(None, false),
                quartet.contrary_motion(),
                quartet.outward_contrary_motion(),
                quartet.inward_contrary_motion(),
                quartet.anti_parallel_motion(None),
                quartet.parallel_fifth(),
                quartet.parallel_octave(),
                quartet.parallel_unison(),
                quartet.hidden_fifth(),
                quartet.hidden_octave(),
                quartet.voice_overlap(),
                quartet.voice_crossing(),
            ];
            assert_eq!(actual, expected.flags, "{label}");
        }
    }

    #[test]
    fn anti_parallel_fifths_are_contrary_unless_asked_for() {
        let quartet = VoiceLeadingQuartet::from_names("G4", "D5", "C4", "G3").unwrap();
        assert_eq!(quartet.motion_type(false), MotionType::Contrary);
        assert_eq!(quartet.motion_type(true), MotionType::AntiParallel);
        assert!(quartet.parallel_fifth());
        assert_eq!(MotionType::AntiParallel.to_string(), "Anti-Parallel");
    }

    #[test]
    fn hidden_intervals_need_similar_motion_into_a_perfect_interval() {
        let quartet = VoiceLeadingQuartet::from_names("E4", "G4", "C4", "C3").unwrap();
        assert!(!quartet.hidden_fifth());
        let quartet = VoiceLeadingQuartet::from_names("E4", "D5", "C4", "G4").unwrap();
        assert!(quartet.hidden_fifth());
        assert!(!quartet.hidden_octave());
        let quartet = VoiceLeadingQuartet::from_names("E4", "C5", "C4", "C4").unwrap();
        assert!(!quartet.hidden_octave());
    }
}
