//! The named scales music21 exposes as `ConcreteScale` subclasses.
//!
//! Each scale is a sequence of step intervals walked upward from the tonic,
//! matching the edges of music21's `IntervalNetwork`, plus the pitch
//! simplification that network applies. Both together are needed: the steps
//! alone give the right pitch classes but the wrong spelling for scales that
//! run out of reasonable accidentals, which is why a C whole-tone scale ends
//! `A#` rather than `B-` but a B whole-tone scale does not end `A##`.

use crate::error::Result;
use crate::interval::Interval;
use crate::pitch::Pitch;

use std::collections::HashMap;
use std::sync::LazyLock;

/// How a scale respells pitches that would otherwise pile up accidentals.
///
/// Mirrors music21's `IntervalNetwork.pitchSimplification`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Simplification {
    /// Spell literally, however many accidentals that takes.
    Exact,
    /// Cap at one accidental, respelling anything beyond it.
    MaxAccidental,
    /// Respell to the most common spelling of the pitch class.
    MostCommon,
}

/// Distinct step intervals used by the scale tables, parsed once.
static STEP_INTERVALS: LazyLock<HashMap<&'static str, Interval>> = LazyLock::new(|| {
    ["m2", "M2", "a2", "m3", "M3"]
        .into_iter()
        .map(|name| {
            let interval =
                Interval::from_name(name).expect("scale step intervals are valid interval names");
            (name, interval)
        })
        .collect()
});

fn step_interval(name: &str) -> &'static Interval {
    STEP_INTERVALS
        .get(name)
        .expect("scale tables only use intervals listed in STEP_INTERVALS")
}

/// A named scale from music21's scale module.
///
/// Ordered as music21 defines them: the seven church modes, their plagal
/// counterparts, the altered minors, then the symmetrical and non-Western
/// scales.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ScaleType {
    /// Major (Ionian).
    Major,
    /// Natural minor (Aeolian).
    Minor,
    /// Dorian mode.
    Dorian,
    /// Phrygian mode.
    Phrygian,
    /// Lydian mode.
    Lydian,
    /// Mixolydian mode.
    Mixolydian,
    /// Locrian mode.
    Locrian,
    /// Hypodorian mode. Shares Dorian's pitches; the ambitus differs.
    Hypodorian,
    /// Hypophrygian mode. Shares Phrygian's pitches.
    Hypophrygian,
    /// Hypolydian mode. Shares Lydian's pitches.
    Hypolydian,
    /// Hypomixolydian mode. Shares Mixolydian's pitches.
    Hypomixolydian,
    /// Hypolocrian mode. Shares Locrian's pitches.
    Hypolocrian,
    /// Hypoaeolian mode. Shares natural minor's pitches.
    Hypoaeolian,
    /// Harmonic minor, with a raised seventh.
    HarmonicMinor,
    /// Ascending melodic minor.
    MelodicMinor,
    /// Twelve-tone chromatic scale.
    Chromatic,
    /// Six-tone whole-tone scale.
    WholeTone,
    /// Eight-tone octatonic scale, alternating tone and semitone.
    Octatonic,
    /// Rag Asawari, as a five-tone ascending scale.
    RagAsawari,
}

impl ScaleType {
    /// Every scale type, in declaration order.
    pub const ALL: [ScaleType; 19] = [
        Self::Major,
        Self::Minor,
        Self::Dorian,
        Self::Phrygian,
        Self::Lydian,
        Self::Mixolydian,
        Self::Locrian,
        Self::Hypodorian,
        Self::Hypophrygian,
        Self::Hypolydian,
        Self::Hypomixolydian,
        Self::Hypolocrian,
        Self::Hypoaeolian,
        Self::HarmonicMinor,
        Self::MelodicMinor,
        Self::Chromatic,
        Self::WholeTone,
        Self::Octatonic,
        Self::RagAsawari,
    ];

    /// Returns the music21 class name for this scale.
    pub fn music21_name(self) -> &'static str {
        match self {
            Self::Major => "MajorScale",
            Self::Minor => "MinorScale",
            Self::Dorian => "DorianScale",
            Self::Phrygian => "PhrygianScale",
            Self::Lydian => "LydianScale",
            Self::Mixolydian => "MixolydianScale",
            Self::Locrian => "LocrianScale",
            Self::Hypodorian => "HypodorianScale",
            Self::Hypophrygian => "HypophrygianScale",
            Self::Hypolydian => "HypolydianScale",
            Self::Hypomixolydian => "HypomixolydianScale",
            Self::Hypolocrian => "HypolocrianScale",
            Self::Hypoaeolian => "HypoaeolianScale",
            Self::HarmonicMinor => "HarmonicMinorScale",
            Self::MelodicMinor => "MelodicMinorScale",
            Self::Chromatic => "ChromaticScale",
            Self::WholeTone => "WholeToneScale",
            Self::Octatonic => "OctatonicScale",
            Self::RagAsawari => "RagAsawari",
        }
    }

    /// Returns the step intervals walked upward from the tonic.
    ///
    /// These are music21's `IntervalNetwork` edges, not the intervals between
    /// the pitches it finally reports — the two differ wherever simplification
    /// respells a degree.
    fn steps(self) -> &'static [&'static str] {
        match self {
            Self::Major => &["M2", "M2", "m2", "M2", "M2", "M2", "m2"],
            Self::Minor | Self::Hypoaeolian => &["M2", "m2", "M2", "M2", "m2", "M2", "M2"],
            Self::Dorian => &["M2", "m2", "M2", "M2", "M2", "m2", "M2"],
            Self::Phrygian => &["m2", "M2", "M2", "M2", "m2", "M2", "M2"],
            Self::Lydian => &["M2", "M2", "M2", "m2", "M2", "M2", "m2"],
            Self::Mixolydian => &["M2", "M2", "m2", "M2", "M2", "m2", "M2"],
            Self::Locrian => &["m2", "M2", "M2", "m2", "M2", "M2", "M2"],
            Self::Hypodorian => &["M2", "m2", "M2", "M2", "M2", "m2", "M2"],
            Self::Hypophrygian => &["m2", "M2", "M2", "M2", "m2", "M2", "M2"],
            Self::Hypolydian => &["M2", "M2", "M2", "m2", "M2", "M2", "m2"],
            Self::Hypomixolydian => &["M2", "M2", "m2", "M2", "M2", "m2", "M2"],
            Self::Hypolocrian => &["m2", "M2", "M2", "m2", "M2", "M2", "M2"],
            Self::HarmonicMinor => &["M2", "m2", "M2", "M2", "m2", "a2", "m2"],
            Self::MelodicMinor => &["M2", "m2", "M2", "M2", "M2", "M2", "m2"],
            Self::Chromatic => &["m2"; 12],
            Self::WholeTone => &["M2"; 6],
            Self::Octatonic => &["M2", "m2", "M2", "m2", "M2", "m2", "M2", "m2"],
            Self::RagAsawari => &["M2", "m3", "M2", "m2", "M3"],
        }
    }

    /// Returns how this scale respells pitches, matching music21's network.
    fn simplification(self) -> Simplification {
        match self {
            Self::WholeTone | Self::Octatonic => Simplification::MaxAccidental,
            Self::Chromatic | Self::RagAsawari => Simplification::MostCommon,
            _ => Simplification::Exact,
        }
    }

    /// Returns the number of distinct degrees in one octave.
    pub fn degree_count(self) -> usize {
        self.steps().len()
    }
}

/// A named scale realized from a tonic pitch.
///
/// ```
/// use music21_rs::{Pitch, Scale, ScaleType};
///
/// let scale = Scale::new(ScaleType::Octatonic, Pitch::from_name("C4")?);
/// let names: Vec<String> = scale.pitches()?.iter().map(|p| p.name()).collect();
///
/// assert_eq!(names, ["C", "D", "E-", "F", "G-", "A-", "A", "B", "C"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Scale {
    scale_type: ScaleType,
    tonic: Pitch,
}

impl Scale {
    /// Builds a scale of the given type on a tonic.
    pub fn new(scale_type: ScaleType, tonic: Pitch) -> Self {
        Self { scale_type, tonic }
    }

    /// Returns the scale type.
    pub fn scale_type(&self) -> ScaleType {
        self.scale_type
    }

    /// Returns the tonic pitch.
    pub fn tonic(&self) -> &Pitch {
        &self.tonic
    }

    /// Returns the pitches of one octave, from the tonic through its octave.
    ///
    /// The result has `degree_count() + 1` entries, since the closing octave is
    /// included the way music21's `getPitches` includes it.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        let simplification = self.scale_type.simplification();
        let mut pitches = Vec::with_capacity(self.scale_type.degree_count() + 1);
        pitches.push(self.tonic.clone());

        let mut current = self.tonic.clone();
        for step in self.scale_type.steps() {
            current = advance(&current, step, simplification)?;
            pitches.push(current.clone());
        }
        Ok(pitches)
    }

    /// Returns the pitch at a one-based scale degree.
    ///
    /// Degree 1 is the tonic and `degree_count() + 1` is the octave above it.
    /// Degrees beyond that continue into higher octaves.
    pub fn pitch_at_degree(&self, degree: usize) -> Result<Pitch> {
        if degree == 0 {
            return Err(crate::error::Error::Ordinal(
                "scale degree must be >= 1".to_string(),
            ));
        }

        let simplification = self.scale_type.simplification();
        let steps = self.scale_type.steps();
        let mut current = self.tonic.clone();
        for index in 0..(degree - 1) {
            current = advance(&current, steps[index % steps.len()], simplification)?;
        }
        Ok(current)
    }
}

/// Transposes one scale step, applying the scale's simplification.
fn advance(pitch: &Pitch, step: &str, simplification: Simplification) -> Result<Pitch> {
    let interval = step_interval(step);
    match simplification {
        Simplification::MaxAccidental => {
            interval.transpose_pitch_with_options(pitch, false, Some(1))
        }
        Simplification::Exact => interval.transpose_pitch_with_options(pitch, false, None),
        Simplification::MostCommon => {
            let mut transposed = interval.transpose_pitch_with_options(pitch, false, None)?;
            if transposed.accidental().alter() != 0.0 {
                transposed.simplify_enharmonic_in_place(true)?;
            }
            Ok(transposed)
        }
    }
}

/// Returns the maximum accidental count used by a scale, for tests.
#[cfg(test)]
fn max_alter(pitches: &[Pitch]) -> crate::defaults::IntegerType {
    pitches
        .iter()
        .map(|pitch| pitch.accidental().alter().abs() as crate::defaults::IntegerType)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(scale_type: ScaleType, tonic: &str) -> Vec<String> {
        Scale::new(scale_type, Pitch::from_name(tonic).expect("valid tonic"))
            .pitches()
            .expect("scale realizes")
            .iter()
            .map(|pitch| pitch.name())
            .collect()
    }

    #[test]
    fn realizes_the_church_modes_on_c() {
        assert_eq!(
            names(ScaleType::Major, "C4"),
            ["C", "D", "E", "F", "G", "A", "B", "C"]
        );
        assert_eq!(
            names(ScaleType::Minor, "C4"),
            ["C", "D", "E-", "F", "G", "A-", "B-", "C"]
        );
        assert_eq!(
            names(ScaleType::Dorian, "C4"),
            ["C", "D", "E-", "F", "G", "A", "B-", "C"]
        );
        assert_eq!(
            names(ScaleType::Phrygian, "C4"),
            ["C", "D-", "E-", "F", "G", "A-", "B-", "C"]
        );
        assert_eq!(
            names(ScaleType::Lydian, "C4"),
            ["C", "D", "E", "F#", "G", "A", "B", "C"]
        );
        assert_eq!(
            names(ScaleType::Mixolydian, "C4"),
            ["C", "D", "E", "F", "G", "A", "B-", "C"]
        );
        assert_eq!(
            names(ScaleType::Locrian, "C4"),
            ["C", "D-", "E-", "F", "G-", "A-", "B-", "C"]
        );
    }

    #[test]
    fn realizes_the_altered_minors() {
        assert_eq!(
            names(ScaleType::HarmonicMinor, "C4"),
            ["C", "D", "E-", "F", "G", "A-", "B", "C"]
        );
        assert_eq!(
            names(ScaleType::MelodicMinor, "C4"),
            ["C", "D", "E-", "F", "G", "A", "B", "C"]
        );
    }

    #[test]
    fn plagal_modes_share_their_authentic_pitches() {
        for (plagal, authentic) in [
            (ScaleType::Hypodorian, ScaleType::Dorian),
            (ScaleType::Hypophrygian, ScaleType::Phrygian),
            (ScaleType::Hypolydian, ScaleType::Lydian),
            (ScaleType::Hypomixolydian, ScaleType::Mixolydian),
            (ScaleType::Hypolocrian, ScaleType::Locrian),
            (ScaleType::Hypoaeolian, ScaleType::Minor),
        ] {
            assert_eq!(
                names(plagal, "C4"),
                names(authentic, "C4"),
                "{plagal:?} should share {authentic:?}'s pitch collection"
            );
        }
    }

    #[test]
    fn whole_tone_spells_upward_until_accidentals_run_out() {
        // From C the scale can stay on sharps and closes on B#, not C.
        assert_eq!(
            names(ScaleType::WholeTone, "C4"),
            ["C", "D", "E", "F#", "G#", "A#", "B#"]
        );
        // From B a literal spelling would need G##, so music21 respells.
        assert_eq!(
            names(ScaleType::WholeTone, "B4"),
            ["B", "C#", "D#", "E#", "G", "A", "B"]
        );
    }

    #[test]
    fn octatonic_alternates_tone_and_semitone() {
        assert_eq!(
            names(ScaleType::Octatonic, "C4"),
            ["C", "D", "E-", "F", "G-", "A-", "A", "B", "C"]
        );
        assert_eq!(
            names(ScaleType::Octatonic, "G4"),
            ["G", "A", "B-", "C", "D-", "E-", "F-", "G-", "G"]
        );
    }

    #[test]
    fn chromatic_uses_the_most_common_spelling() {
        assert_eq!(
            names(ScaleType::Chromatic, "C4"),
            [
                "C", "C#", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B", "C"
            ]
        );
    }

    #[test]
    fn rag_asawari_is_pentatonic() {
        assert_eq!(
            names(ScaleType::RagAsawari, "C4"),
            ["C", "D", "F", "G", "A-", "C"]
        );
    }

    #[test]
    fn simplifying_scales_never_exceed_their_accidental_budget() {
        for scale_type in [ScaleType::WholeTone, ScaleType::Octatonic] {
            for tonic in ["C4", "G4", "D4", "A4", "E4", "B4", "F#4", "E-4", "G-4"] {
                let pitches = Scale::new(scale_type, Pitch::from_name(tonic).unwrap())
                    .pitches()
                    .unwrap();
                assert!(
                    max_alter(&pitches) <= 1,
                    "{scale_type:?} on {tonic} exceeded one accidental"
                );
            }
        }
    }

    #[test]
    fn degree_lookup_matches_the_realized_pitches() {
        for scale_type in ScaleType::ALL {
            let scale = Scale::new(scale_type, Pitch::from_name("E-4").unwrap());
            let pitches = scale.pitches().unwrap();
            for (index, expected) in pitches.iter().enumerate() {
                let actual = scale.pitch_at_degree(index + 1).unwrap();
                assert_eq!(
                    actual.name_with_octave(),
                    expected.name_with_octave(),
                    "{scale_type:?} degree {}",
                    index + 1
                );
            }
        }
    }

    #[test]
    fn degree_zero_is_rejected() {
        let scale = Scale::new(ScaleType::Major, Pitch::from_name("C4").unwrap());
        assert!(scale.pitch_at_degree(0).is_err());
    }

    #[test]
    fn every_scale_realizes_on_every_common_tonic() {
        for scale_type in ScaleType::ALL {
            for tonic in [
                "C4", "G4", "D4", "A4", "E4", "B4", "F#4", "F4", "B-4", "E-4", "A-4",
            ] {
                let scale = Scale::new(scale_type, Pitch::from_name(tonic).unwrap());
                let pitches = scale.pitches().expect("scale realizes");
                assert_eq!(
                    pitches.len(),
                    scale_type.degree_count() + 1,
                    "{scale_type:?} on {tonic}"
                );
            }
        }
    }

    #[test]
    fn music21_names_are_distinct() {
        let mut names: Vec<&str> = ScaleType::ALL.iter().map(|s| s.music21_name()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "music21 names must be unique");
    }
}
