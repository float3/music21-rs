//! Two-voice voice-leading checks, a port of music21's `VoiceLeadingQuartet`.
//!
//! A quartet is two consecutive notes in an upper voice and two in a lower
//! voice. It classifies how the voices move between them and finds the
//! parallel and hidden perfect intervals that common-practice counterpoint
//! forbids.

use crate::{
    error::Result,
    interval::{Interval, IntervalDirection},
    pitch::Pitch,
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

/// Two consecutive notes in each of two voices. Voice one is the upper voice.
#[derive(Clone, Debug)]
pub struct VoiceLeadingQuartet {
    v1n1: Pitch,
    v1n2: Pitch,
    v2n1: Pitch,
    v2n2: Pitch,
    vertical: [Interval; 2],
    horizontal: [Interval; 2],
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
        required: Option<&Interval>,
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
        required.is_none_or(|required| {
            first.semi_simple_key() == required.semi_simple_key()
                && second.semi_simple_key() == required.semi_simple_key()
        })
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
        self.parallel_motion(Some(interval), true) || self.anti_parallel_motion(Some(interval))
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
