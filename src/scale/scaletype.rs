//! The named scales music21 exposes as `ConcreteScale` subclasses.
//!
//! Each scale is a sequence of step intervals walked from the tonic (upward,
//! except for the one descending step Rag Marwa's network contains),
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

pub use super::realized::Scale;

/// How a scale respells pitches that would otherwise pile up accidentals.
///
/// Mirrors music21's `IntervalNetwork.pitchSimplification`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Simplification {
    /// Spell literally, however many accidentals that takes.
    Exact,
    /// Cap at one accidental, respelling anything beyond it.
    MaxAccidental,
    /// Respell to the most common spelling of the pitch class.
    MostCommon,
}

/// The step intervals the scale tables use, parsed once.
///
/// A scale given by its notes rather than by a name can step by anything, so
/// this is a fast path and not the whole world: a step it does not hold is
/// parsed on the spot.
static STEP_INTERVALS: LazyLock<HashMap<&'static str, Interval>> = LazyLock::new(|| {
    ["m2", "M2", "a2", "m3", "M3", "-M2"]
        .into_iter()
        .map(|name| {
            let interval =
                Interval::from_name(name).expect("scale step intervals are valid interval names");
            (name, interval)
        })
        .collect()
});

pub(super) fn step_interval(name: &str) -> Result<Interval> {
    match STEP_INTERVALS.get(name) {
        Some(interval) => Ok(interval.clone()),
        None => Interval::from_name(name),
    }
}

/// Which set of solfège syllables [`Scale::solfeg`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SolfegVariant {
    /// music21's own table.
    Music21,
    /// The Humdrum spellings, which differ in `my`, `so` and `ty`.
    Humdrum,
}

/// music21's `_solfegSyllables`: for each of the seven degrees, the syllable
/// at alterations of -2, -1, 0, +1 and +2.
pub const SOLFEG_SYLLABLES: [[&str; 5]; 7] = [
    ["def", "de", "do", "di", "dis"],
    ["raf", "ra", "re", "ri", "ris"],
    ["mef", "me", "mi", "mis", "mish"],
    ["fef", "fe", "fa", "fi", "fis"],
    ["sef", "se", "sol", "si", "sis"],
    ["lef", "le", "la", "li", "lis"],
    ["tef", "te", "ti", "tis", "tish"],
];

/// music21's `_humdrumSolfegSyllables`, laid out like [`SOLFEG_SYLLABLES`].
pub const HUMDRUM_SOLFEG_SYLLABLES: [[&str; 5]; 7] = [
    ["def", "de", "do", "di", "dis"],
    ["raf", "ra", "re", "ri", "ris"],
    ["mef", "me", "mi", "my", "mish"],
    ["fef", "fe", "fa", "fi", "fis"],
    ["sef", "se", "so", "si", "sis"],
    ["lef", "le", "la", "li", "lis"],
    ["tef", "te", "ti", "ty", "tish"],
];

/// A named scale from music21's scale module.
///
/// Ordered as music21 defines them: the seven church modes, their plagal
/// counterparts, the altered minors, then the symmetrical and non-Western
/// scales.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[must_use]
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
    /// Rag Marwa, as a seven-step ascending scale.
    ///
    /// Not monotonic: music21's ascending network dips back down a major
    /// second from the sixth degree before rising a minor third to the octave,
    /// so the realized pitches repeat a note and briefly descend.
    RagMarwa,
}

impl ScaleType {
    /// Every scale type, in declaration order.
    pub const ALL: [ScaleType; 20] = [
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
        Self::RagMarwa,
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
            Self::RagMarwa => "RagMarwa",
        }
    }

    /// The words music21 puts after the tonic when it names a scale:
    /// `"C major"`, `"C harmonic minor"`, `"C Rag Asawari"`.
    ///
    /// The capitalisation is upstream's and is not consistent — the modes are
    /// lower case, the others are not — which is why this is a table rather
    /// than something derived from the class name.
    pub fn music21_descriptive_name(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Dorian => "dorian",
            Self::Phrygian => "phrygian",
            Self::Lydian => "lydian",
            Self::Mixolydian => "mixolydian",
            Self::Locrian => "locrian",
            Self::Hypodorian => "hypodorian",
            Self::Hypophrygian => "hypophrygian",
            Self::Hypolydian => "hypolydian",
            Self::Hypomixolydian => "hypomixolydian",
            Self::Hypolocrian => "hypolocrian",
            Self::Hypoaeolian => "hypoaeolian",
            Self::HarmonicMinor => "harmonic minor",
            Self::MelodicMinor => "melodic minor",
            Self::Chromatic => "Chromatic",
            Self::WholeTone => "Whole tone",
            Self::Octatonic => "Octatonic",
            Self::RagAsawari => "Rag Asawari",
            Self::RagMarwa => "Rag Marwa",
        }
    }

    /// The scale type music21 calls by this class name, such as
    /// `"MajorScale"`.
    pub fn from_music21_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|scale_type| scale_type.music21_name() == name)
    }

    /// Which degree of the realized scale is its final: music21's
    /// `tonicDegree`.
    ///
    /// A plagal mode — the six that music21 prefixes `Hypo` — runs from a
    /// fourth below its final to a fifth above it, so the note the music
    /// comes to rest on is its fourth degree and not its first. Every other
    /// scale here starts on its own tonic.
    pub fn tonic_degree(self) -> usize {
        if self.is_plagal() { 4 } else { 1 }
    }

    /// Which degree is the reciting tone: music21's `dominantDegree`.
    ///
    /// A fifth above the final in the authentic modes. The plagal ones took
    /// theirs a third above the authentic dominant and then moved it off any
    /// B, which is why hypophrygian and hypomixolydian differ from the rest.
    pub fn dominant_degree(self) -> usize {
        match self {
            Self::Hypophrygian | Self::Hypomixolydian => 7,
            _ if self.is_plagal() => 6,
            _ => 5,
        }
    }

    /// The steps walked from the pitch the scale is *realized* from, which
    /// for a plagal mode is not its final.
    ///
    /// `steps` is the collection written from the final, which is
    /// how every one of these scales is named. A plagal mode is realized
    /// from a fourth below that, so its walk starts three steps earlier in
    /// the same cycle — the rotation that puts the final at the degree
    /// [`Self::tonic_degree`] names.
    pub fn realization_steps(self) -> Vec<&'static str> {
        let steps = self.steps();
        let count = steps.len();
        let start = (count + 1 - self.tonic_degree()) % count;
        steps[start..]
            .iter()
            .chain(&steps[..start])
            .copied()
            .collect()
    }

    /// The step a scale carries on past its own octave, where it has one.
    ///
    /// Rag Marwa is the only one here: its network keeps going a semitone
    /// above the terminus, so the collection realized from `C` closes on the
    /// octave and then sounds the `D-` above it. music21 calls it "a pitch
    /// beyond the terminus" and gives it whenever the realization is not
    /// bounded by a range.
    pub fn beyond_terminus(self) -> Option<&'static str> {
        match self {
            Self::RagMarwa => Some("m2"),
            _ => None,
        }
    }

    /// The collection this scale uses coming down, where that is not the
    /// one it uses going up.
    ///
    /// The melodic minor is the familiar case — it raises its sixth and
    /// seventh degrees ascending and lets them fall descending, which is the
    /// natural minor — and Rag Asawari's avaroha is that same collection.
    /// Every other scale here descends through the notes it ascends
    /// through, and answers `None`.
    pub fn descending_form(self) -> Option<Self> {
        match self {
            Self::MelodicMinor | Self::RagAsawari => Some(Self::Minor),
            _ => None,
        }
    }

    /// The degrees the notes of this scale's ascent stand on, where they are
    /// not simply counted off one to a note.
    ///
    /// Rag Asawari's aroha leaves out the third and the seventh, so its five
    /// notes stand on the first, second, fourth, fifth and sixth degrees. It
    /// has no third going up at all — which is what music21 answers when
    /// asked for one — and its fourth degree is its third note.
    pub fn ascending_degrees(self) -> Option<&'static [u8]> {
        match self {
            Self::RagAsawari => Some(&[1, 2, 4, 5, 6]),
            _ => None,
        }
    }

    /// The steps this scale is walked by coming down, where coming down is
    /// not simply the ascending pattern read backwards and no other named
    /// scale is that pattern.
    ///
    /// Rag Marwa's avarohana is the one: from its tonic it rises a semitone
    /// to the flat second *above* the octave and falls from there, which is
    /// music21's network edge from the high terminus upward before anything
    /// descends. Written as a rising list from the tonic it closes by falling
    /// back onto it, so the flat second is both the second degree and the
    /// seventh — which is what music21 answers when asked which degree a
    /// `D-` is coming down.
    pub fn descending_steps(self) -> Option<&'static [&'static str]> {
        match self {
            Self::RagMarwa => Some(&["m2", "A2", "M2", "m3", "M2", "d3", "-m2"]),
            _ => None,
        }
    }

    /// Whether this is a plagal mode, whose range sits below its final.
    pub fn is_plagal(self) -> bool {
        matches!(
            self,
            Self::Hypodorian
                | Self::Hypophrygian
                | Self::Hypolydian
                | Self::Hypomixolydian
                | Self::Hypolocrian
                | Self::Hypoaeolian
        )
    }

    /// Returns the step intervals walked from the tonic.
    ///
    /// Almost every scale ascends throughout; Rag Marwa is the exception, and
    /// carries one descending step (`-M2`), matching its network edge.
    ///
    /// These are music21's `IntervalNetwork` edges, not the intervals between
    /// the pitches it finally reports — the two differ wherever simplification
    /// respells a degree.
    pub(super) fn steps(self) -> &'static [&'static str] {
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
            // The sixth step is music21's `M-2` edge: a descending major
            // second inside an otherwise ascending network.
            Self::RagMarwa => &["m2", "a2", "M2", "m3", "M2", "-M2", "m3"],
        }
    }

    /// Returns how this scale respells pitches, matching music21's network.
    pub(super) fn simplification(self) -> Simplification {
        match self {
            Self::WholeTone | Self::Octatonic | Self::RagMarwa => Simplification::MaxAccidental,
            Self::Chromatic | Self::RagAsawari => Simplification::MostCommon,
            _ => Simplification::Exact,
        }
    }

    /// Returns the number of distinct degrees in one octave.
    pub fn degree_count(self) -> usize {
        self.steps().len()
    }
}

/// The tonics music21's `IntervalNetwork.find` tries, in its order. Ties in
/// the ranking fall back to this order reversed.
pub(super) const SCALE_STARTS: [&str; 15] = [
    "C", "C#", "D-", "D", "D#", "E-", "E", "F", "F#", "G", "G#", "A", "B-", "B", "C-",
];

impl ScaleType {
    /// Ranks every candidate tonic by how many of `pitches` fall in this
    /// scale type built on it, best first, as music21's `deriveRanked` does.
    /// Pitches are compared by pitch class and duplicates count separately.
    pub fn derive_ranked(
        self,
        pitches: &[Pitch],
        limit: Option<usize>,
    ) -> Result<Vec<(usize, Scale)>> {
        self.derive_ranked_by(pitches, limit, DegreeComparison::PitchClass)
    }

    /// The same, saying how a pitch is matched against a scale degree.
    ///
    /// By pitch class an `E#` is in C major, since the scale has an `F`; by
    /// name it is not. music21 offers both, and its `deriveRanked` defaults
    /// to the first.
    pub fn derive_ranked_by(
        self,
        pitches: &[Pitch],
        limit: Option<usize>,
        comparison: DegreeComparison,
    ) -> Result<Vec<(usize, Scale)>> {
        let targets = pitches
            .iter()
            .map(|pitch| comparison.key(pitch))
            .collect::<Vec<_>>();
        let mut ranked = Vec::with_capacity(SCALE_STARTS.len());
        for start in SCALE_STARTS {
            let scale = Scale::new(self, Pitch::from_name(start)?);
            let degrees = scale
                .pitches()?
                .iter()
                .map(|pitch| comparison.key(pitch))
                .collect::<Vec<_>>();
            let matched = targets
                .iter()
                .filter(|target| degrees.contains(target))
                .count();
            ranked.push((matched, scale));
        }
        ranked.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.tonic().ps().total_cmp(&right.1.tonic().ps()))
        });
        ranked.reverse();
        if let Some(limit) = limit {
            ranked.truncate(limit);
        }
        Ok(ranked)
    }

    /// Returns this scale type on the tonic that fits `pitches` best.
    pub fn derive(self, pitches: &[Pitch]) -> Result<Scale> {
        self.derive_ranked(pitches, Some(1))?
            .pop()
            .map(|(_, scale)| scale)
            .ok_or_else(|| crate::error::Error::Scale("no candidate tonics".to_string()))
    }

    /// Returns every tonic on which this scale type contains all of `pitches`,
    /// best-ranked first.
    pub fn derive_all(self, pitches: &[Pitch]) -> Result<Vec<Scale>> {
        Ok(self
            .derive_ranked(pitches, None)?
            .into_iter()
            .filter(|(matched, _)| *matched == pitches.len())
            .map(|(_, scale)| scale)
            .collect())
    }
}

/// How many octaves of a scale [`Scale::pitches_between`] will walk, which
/// is the whole of music21's pitch space and then some.
pub(super) const MAX_RANGE_OCTAVES: usize = 12;

/// How a pitch is matched against a scale degree: music21's
/// `comparisonAttribute`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DegreeComparison {
    /// By sounding note, so `E#` matches the `F` of C major.
    PitchClass,
    /// By written note, so it does not.
    Name,
    /// By letter alone, so an `E-` matches the `E` of C major.
    Step,
}

impl DegreeComparison {
    /// What two pitches have to share to count as the same degree.
    pub(super) fn key(self, pitch: &Pitch) -> String {
        match self {
            Self::PitchClass => pitch.pitch_class().number().to_string(),
            Self::Name => pitch.name(),
            Self::Step => pitch.name().chars().take(1).collect(),
        }
    }
}

/// Transposes one scale step, applying the scale's simplification.
pub(super) fn advance(
    pitch: &Pitch,
    interval: &Interval,
    simplification: Simplification,
) -> Result<Pitch> {
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
    use crate::defaults::{FloatType, IntegerType};
    #[test]
    fn degrees_are_found_by_pitch_class_name_or_step() {
        use super::{DegreeComparison, Scale, ScaleType};
        use crate::Pitch;

        let major = Scale::new(ScaleType::Major, Pitch::from_name("C").unwrap());
        let e_sharp = Pitch::from_name("E#").unwrap();
        assert_eq!(
            major
                .degree_of_by(&e_sharp, DegreeComparison::PitchClass)
                .unwrap(),
            Some(4)
        );
        assert_eq!(
            major
                .degree_of_by(&e_sharp, DegreeComparison::Name)
                .unwrap(),
            None
        );
        assert_eq!(
            major
                .degree_of_by(&e_sharp, DegreeComparison::Step)
                .unwrap(),
            Some(3)
        );
        let e_flat = Pitch::from_name("E-").unwrap();
        let (degree, accidental) = major.degree_and_accidental_of(&e_flat).unwrap();
        assert_eq!(degree, 3);
        assert_eq!(accidental.as_ref().map(|a| a.name()), Some("flat"));
        let whole_tone = Scale::new(ScaleType::WholeTone, Pitch::from_name("C").unwrap());
        assert!(
            whole_tone
                .degree_and_accidental_of(&Pitch::from_name("B").unwrap())
                .is_err()
        );

        // Rag Marwa names its A twice, as the fifth degree and the seventh.
        let marwa = Scale::new(ScaleType::RagMarwa, Pitch::from_name("C4").unwrap());
        assert_eq!(
            marwa
                .degrees_of_by(&Pitch::from_name("A").unwrap(), DegreeComparison::Name)
                .unwrap(),
            [5, 7]
        );
    }

    #[test]
    fn a_scale_moves_to_its_relatives_parallels_and_a_new_tonic() {
        use super::{Scale, ScaleType};
        use crate::Pitch;

        let tonic = |scale: &Scale| scale.tonic().name();
        let c_minor = Scale::new(ScaleType::Minor, Pitch::from_name("C").unwrap());
        assert_eq!(tonic(&c_minor.parallel_major()), "C");
        assert_eq!(c_minor.parallel_major().scale_type(), ScaleType::Major);
        assert_eq!(tonic(&c_minor.relative_major().unwrap()), "E-");
        let c_major = Scale::new(ScaleType::Major, Pitch::from_name("C").unwrap());
        assert_eq!(tonic(&c_major.relative_minor().unwrap()), "A");
        assert_eq!(c_major.parallel_minor().scale_type(), ScaleType::Minor);
        let mut moved = c_major.clone();
        moved.set_tonic(Pitch::from_name("D").unwrap());
        assert_eq!(tonic(&moved), "D");
        assert_eq!(moved.pitches().unwrap()[1].name(), "E");
        assert!(c_major.octave_duplicating());
        assert!(c_major.is_realizable());
        assert_eq!(
            ScaleType::from_music21_name("MajorScale"),
            Some(ScaleType::Major)
        );
        assert_eq!(ScaleType::from_music21_name("NoSuchScale"), None);
    }

    #[test]
    fn a_scale_walks_down_as_well_as_up() {
        use super::{Scale, ScaleType};
        use crate::Pitch;

        let names = |pitches: Vec<Pitch>| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };
        let major = Scale::new(ScaleType::Major, Pitch::from_name("C4").unwrap());
        assert_eq!(
            names(major.pitches_descending().unwrap()),
            ["C5", "B4", "A4", "G4", "F4", "E4", "D4", "C4"]
        );
        let low = Pitch::from_name("C3").unwrap();
        let high = Pitch::from_name("C5").unwrap();
        assert_eq!(
            names(
                major
                    .pitches_between_descending(&Pitch::from_name("G4").unwrap(), &high)
                    .unwrap()
            ),
            ["C5", "B4", "A4", "G4"]
        );
        assert_eq!(
            names(
                major
                    .pitches_from_scale_degrees_between(&[1, 5], &low, &high)
                    .unwrap()
            ),
            ["C3", "G3", "C4", "G4", "C5"]
        );

        // A pitch off the scale comes onto it first: below C#4 in C major
        // is B3 from the lower neighbour and C4 from the upper.
        let c_sharp = Pitch::from_name("C#4").unwrap();
        assert_eq!(
            major
                .next_pitch_below(&c_sharp, 1)
                .unwrap()
                .name_with_octave(),
            "C4"
        );
        assert_eq!(
            major
                .next_pitch_beside(&c_sharp, -1, true)
                .unwrap()
                .name_with_octave(),
            "B3"
        );
        assert_eq!(
            major
                .next_pitch_beside(&c_sharp, -1, false)
                .unwrap()
                .name_with_octave(),
            "C4"
        );
        assert_eq!(
            major
                .next_pitch_beside(&c_sharp, 1, false)
                .unwrap()
                .name_with_octave(),
            "E4"
        );

        // Rag Marwa stands D- in two places, as the note above its tonic and
        // the one it passes through coming down from the octave, and each
        // has its own neighbour.
        let marwa = Scale::new(ScaleType::RagMarwa, Pitch::from_name("C4").unwrap());
        let d_flat = Pitch::from_name("D-4").unwrap();
        assert_eq!(marwa.places_of(&d_flat).unwrap(), 1);
        assert_eq!(
            marwa
                .next_pitch_above_from(&d_flat, 1, 0)
                .unwrap()
                .name_with_octave(),
            "E4"
        );
        assert_eq!(
            marwa
                .next_pitch_below_from(&Pitch::from_name("C5").unwrap(), 1, 0)
                .unwrap()
                .name_with_octave(),
            "A4"
        );
    }

    #[test]
    fn a_custom_scale_ranks_the_tonics_that_fit_a_set_of_pitches() {
        use super::{DegreeComparison, Scale};
        use crate::Pitch;

        let pitches: Vec<Pitch> = ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"]
            .iter()
            .map(|name| Pitch::from_name(*name).unwrap())
            .collect();
        let custom = Scale::from_pitches(&pitches).unwrap();
        assert!(custom.is_custom());
        let targets: Vec<Pitch> = ["G4", "B4", "D5"]
            .iter()
            .map(|name| Pitch::from_name(*name).unwrap())
            .collect();
        let ranked = custom
            .derive_ranked_by(&targets, Some(2), DegreeComparison::PitchClass)
            .unwrap();
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].0, 3);
        assert!(ranked[0].1.is_custom());
    }

    /// A scale carrying its own tuning retunes the notes it names, in their
    /// own octaves and spellings, and leaves the notes it does not name
    /// where they are.
    #[test]
    fn a_stream_is_tuned_onto_a_scale() {
        use super::Scale;
        use crate::stream::StreamElement;
        use crate::{Chord, Note, Pitch, Stream};

        let tuned: Vec<Pitch> = [
            ("C4", 0.0),
            ("D4", 20.0),
            ("E4", 0.0),
            ("F4", 0.0),
            ("G4", 0.0),
            ("A4", -15.0),
            ("B4", 0.0),
            ("C5", 0.0),
        ]
        .iter()
        .map(|(name, cents)| {
            let mut pitch = Pitch::from_name(*name).unwrap();
            pitch.set_microtone_cents(*cents).unwrap();
            pitch
        })
        .collect();
        let scale = Scale::from_pitches(&tuned).unwrap();
        let mut stream = Stream::new();
        for name in ["D5", "F#4", "A3", "B#4", "G-5"] {
            stream.push(Note::from_pitch(Pitch::from_name(name).unwrap()));
        }
        stream.push(Chord::new("C5 E5 G5").unwrap());
        scale.tune(&mut stream).unwrap();
        let sounding: Vec<(String, FloatType)> = stream
            .events()
            .iter()
            .flat_map(|event| match event.element() {
                StreamElement::Note(note) => {
                    vec![(note.pitch().name_with_octave(), note.pitch().ps())]
                }
                StreamElement::Chord(chord) => chord
                    .pitches()
                    .iter()
                    .map(|pitch| (pitch.name_with_octave(), pitch.ps()))
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        let expected: Vec<(String, FloatType)> = [
            ("D5", 74.2),
            ("F#4", 66.0),
            ("A3", 56.85),
            ("B#4", 72.0),
            ("G-5", 78.0),
            ("C5", 72.0),
            ("E5", 76.0),
            ("G5", 79.0),
        ]
        .iter()
        .map(|(name, ps)| (name.to_string(), *ps))
        .collect();
        assert_eq!(sounding.len(), expected.len());
        for ((name, ps), (wanted_name, wanted_ps)) in sounding.iter().zip(&expected) {
            assert_eq!(name, wanted_name);
            assert!(
                (ps - wanted_ps).abs() < 1e-9,
                "{name} sounds at {ps}, not {wanted_ps}"
            );
        }
    }

    /// music21's own `romanNumeral` example, and the Scala reading of a
    /// major scale.
    #[test]
    fn a_scale_carries_numerals_and_writes_itself_as_scala() {
        use super::{Scale, ScaleType};
        use crate::Pitch;
        use crate::tuningsystem::scala::ScalaDegree;

        let scale = Scale::new(ScaleType::Major, Pitch::from_name("A-4").unwrap());
        let tonic = scale.roman_numeral(1).unwrap();
        assert_eq!(tonic.to_chord().unwrap().root().unwrap().to_string(), "A-4");
        let dominant = scale.roman_numeral(5).unwrap();
        assert_eq!(
            dominant.to_chord().unwrap().root().unwrap().to_string(),
            "E-5"
        );
        assert_eq!(dominant.figure_and_key(), "V in A- major");
        assert!(scale.roman_numeral(8).is_err());

        let scala = scale.scala_data().unwrap();
        assert_eq!(scala.len(), 7);
        assert!(matches!(scala.degrees()[0], ScalaDegree::Ratio(_)));
        let cents: Vec<i64> = scala
            .degrees()
            .iter()
            .map(|degree| degree.cents().round() as i64)
            .collect();
        assert_eq!(cents, [0, 200, 400, 500, 700, 900, 1100]);
        assert_eq!(scala.period().cents().round() as i64, 1200);
        assert_eq!(scala.description(), "A- major");
    }

    #[test]
    fn a_scale_can_be_given_by_its_notes() {
        // music21's own example: a scale of four notes, which repeats at the
        // octave like any other.
        let given: Vec<Pitch> = ["C4", "E-4", "G-4", "A4"]
            .iter()
            .map(|name| Pitch::from_name(*name).expect("valid pitch"))
            .collect();
        let scale = Scale::from_pitches(&given).expect("a scale");
        assert!(scale.is_custom());
        assert_eq!(scale.degree_count(), 4);
        let realized: Vec<String> = scale
            .pitches()
            .expect("realizes")
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(realized, ["C4", "E-4", "G-4", "A4", "C5"]);
        assert_eq!(scale.pitch_at_degree(4).unwrap().name_with_octave(), "A4");
        // The fifth degree of a four-note scale is the first again, in the
        // octave the scale stands in — music21 reads a degree within the one
        // octave its network holds.
        assert_eq!(scale.pitch_at_degree(5).unwrap().name_with_octave(), "C4");
        assert_eq!(
            scale.degree_of(&Pitch::from_name("G-").unwrap()).unwrap(),
            Some(3)
        );

        // And it walks a range the way a named scale does.
        let range: Vec<String> = scale
            .pitches_between(
                &Pitch::from_name("E-5").unwrap(),
                &Pitch::from_name("C6").unwrap(),
            )
            .expect("a range")
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(range, ["E-5", "G-5", "A5", "C6"]);

        assert!(Scale::from_pitches(&[]).is_err());
    }
    #[test]
    fn a_range_starts_at_the_first_scale_pitch_inside_it() {
        // music21's own example: C major from E-flat 5 to G-flat 7 starts on
        // E5, the first scale pitch that is not below the bottom.
        let scale = Scale::new(ScaleType::Major, Pitch::from_name("C").unwrap());
        let range = scale
            .pitches_between(
                &Pitch::from_name("E-5").unwrap(),
                &Pitch::from_name("G-7").unwrap(),
            )
            .unwrap();
        let names: Vec<String> = range.iter().map(Pitch::name_with_octave).take(4).collect();
        assert_eq!(names, ["E5", "F5", "G5", "A5"]);
        assert_eq!(range.last().unwrap().name_with_octave(), "F7");

        // A range of exactly one octave has both ends in it.
        let octave = scale
            .pitches_between(
                &Pitch::from_name("C3").unwrap(),
                &Pitch::from_name("C4").unwrap(),
            )
            .unwrap();
        let names: Vec<String> = octave.iter().map(Pitch::name_with_octave).collect();
        assert_eq!(names, ["C3", "D3", "E3", "F3", "G3", "A3", "B3", "C4"]);

        // Asked the other way round, the same range comes back descending,
        // which is what music21 does.
        let descending: Vec<String> = scale
            .pitches_between(
                &Pitch::from_name("C5").unwrap(),
                &Pitch::from_name("C4").unwrap(),
            )
            .unwrap()
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(descending, ["C5", "B4", "A4", "G4", "F4", "E4", "D4", "C4"]);
    }

    #[test]
    fn scale_helpers_match_music21() {
        let c_major = Scale::new(ScaleType::Major, Pitch::from_name("C4").unwrap());
        let pitch = |name: &str| Pitch::from_name(name).unwrap();
        let names = |pitches: &[Pitch]| -> Vec<String> {
            pitches.iter().map(Pitch::name_with_octave).collect()
        };

        assert_eq!(c_major.degree_count(), 7);
        assert_eq!(
            Scale::new(ScaleType::Chromatic, pitch("C4")).degree_count(),
            12
        );
        assert_eq!(
            c_major
                .transpose(&Interval::from_name("P5").unwrap())
                .unwrap()
                .tonic()
                .name_with_octave(),
            "G4"
        );
        assert_eq!(
            names(&c_major.chord().unwrap().pitches()),
            ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"]
        );
        assert_eq!(
            names(
                &c_major
                    .pitches_from_scale_degrees(&[1, 3, 5, 8, 10])
                    .unwrap()
            ),
            ["C4", "E4", "G4", "C5"]
        );
        assert_eq!(
            c_major.interval_between_degrees(1, 5).unwrap().short_name(),
            "P5"
        );
        assert_eq!(
            c_major.interval_between_degrees(3, 7).unwrap().short_name(),
            "P5"
        );
        assert_eq!(
            c_major
                .interval_between_degrees(5, 2)
                .unwrap()
                .directed_name(),
            "P-4"
        );
        assert_eq!(
            c_major.interval_between_degrees(2, 9).unwrap().short_name(),
            "P1"
        );

        assert!(c_major.is_next(&pitch("D4"), &pitch("C4"), 1).unwrap());
        assert!(c_major.is_next(&pitch("D5"), &pitch("C4"), 1).unwrap());
        assert!(!c_major.is_next(&pitch("E4"), &pitch("C4"), 1).unwrap());
        assert!(c_major.is_next(&pitch("E4"), &pitch("C4"), 2).unwrap());

        let (matched, unmatched) = c_major
            .match_pitches(&["C4", "E4", "G-4", "B-5", "A"].map(pitch))
            .unwrap();
        assert_eq!(names(&matched), ["C4", "E4", "A4"]);
        assert_eq!(names(&unmatched), ["G-4", "B-5"]);

        assert_eq!(
            names(
                &c_major
                    .find_missing(&["C4", "E4", "G4"].map(pitch))
                    .unwrap()
            ),
            ["D4", "F4", "A4", "B4"]
        );
        let a_minor = Scale::new(ScaleType::Minor, pitch("A"));
        assert_eq!(
            names(
                &a_minor
                    .find_missing(&["A", "B", "C", "E"].map(pitch))
                    .unwrap()
            ),
            ["D5", "F5", "G5"]
        );
        assert!(
            c_major
                .find_missing(&["C4", "D4", "E4", "F4", "G4", "A4", "B4"].map(pitch))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn solfeg_matches_music21() {
        let c_major = Scale::new(ScaleType::Major, Pitch::from_name("C4").unwrap());
        let cases = [
            ("C4", "do", "do", "do"),
            ("C#4", "di", "di", "do"),
            ("D-4", "ra", "ra", "re"),
            ("E-4", "me", "me", "mi"),
            ("F#4", "fi", "fi", "fa"),
            ("G-4", "se", "se", "sol"),
            ("G4", "sol", "so", "sol"),
            ("A-4", "le", "le", "la"),
            ("B-4", "te", "te", "ti"),
            ("C-4", "de", "de", "do"),
            ("B#4", "tis", "ty", "ti"),
            ("F##4", "fis", "fis", "fa"),
            ("E#4", "mis", "my", "mi"),
            ("D##4", "ris", "ris", "re"),
        ];
        for (name, music21, humdrum, plain) in cases {
            let pitch = Pitch::from_name(name).unwrap();
            assert_eq!(
                c_major
                    .solfeg(&pitch, SolfegVariant::Music21, true)
                    .unwrap(),
                music21,
                "{name}"
            );
            assert_eq!(
                c_major
                    .solfeg(&pitch, SolfegVariant::Humdrum, true)
                    .unwrap(),
                humdrum,
                "{name}"
            );
            assert_eq!(
                c_major
                    .solfeg(&pitch, SolfegVariant::Music21, false)
                    .unwrap(),
                plain,
                "{name}"
            );
        }
        let chromatic = Scale::new(ScaleType::Chromatic, Pitch::from_name("C4").unwrap());
        assert_eq!(
            chromatic
                .solfeg(
                    &Pitch::from_name("D4").unwrap(),
                    SolfegVariant::Music21,
                    true
                )
                .unwrap(),
            "mi"
        );
        assert!(
            chromatic
                .solfeg(
                    &Pitch::from_name("B4").unwrap(),
                    SolfegVariant::Music21,
                    true
                )
                .is_err()
        );
        assert!(
            c_major
                .solfeg(
                    &Pitch::from_name("C###4").unwrap(),
                    SolfegVariant::Music21,
                    true
                )
                .is_err()
        );
    }
    use super::*;

    #[test]
    fn degrees_and_neighbouring_pitches_match_music21() {
        let scale = Scale::new(ScaleType::Major, Pitch::from_name("G4").unwrap());
        let degree = |name: &str| scale.degree_of(&Pitch::from_name(name).unwrap()).unwrap();
        assert_eq!(degree("G4"), Some(1));
        assert_eq!(degree("A4"), Some(2));
        assert_eq!(degree("F#5"), Some(7));
        assert_eq!(degree("B"), Some(3));
        assert_eq!(degree("B-4"), None);
        assert_eq!(degree("E3"), Some(6));
        assert_eq!(
            scale
                .degree_of_pitch_class(&Pitch::from_name("G-4").unwrap())
                .unwrap(),
            Some(7)
        );

        let cases = [
            ("G4", true, 1, "A4"),
            ("G4", false, 1, "F#4"),
            ("F#4", true, 1, "G4"),
            ("B4", true, 3, "E5"),
            ("G4", false, 2, "E4"),
            ("E-4", true, 1, "E4"),
            ("E-4", false, 1, "D4"),
            ("B-4", false, 1, "A4"),
            ("G4", true, 8, "A5"),
            ("D5", true, 7, "D6"),
            ("G", true, 1, "A"),
            ("F#5", true, 1, "G5"),
            ("C3", false, 4, "F#2"),
        ];
        for (origin, ascending, steps, expected) in cases {
            let origin_pitch = Pitch::from_name(origin).unwrap();
            let next = if ascending {
                scale.next_pitch_above(&origin_pitch, steps)
            } else {
                scale.next_pitch_below(&origin_pitch, steps)
            };
            assert_eq!(
                next.unwrap().name_with_octave(),
                expected,
                "{origin} {ascending} {steps}"
            );
        }
        assert!(
            scale
                .next_pitch_above(&Pitch::from_name("G4").unwrap(), 0)
                .is_err()
        );
    }

    #[test]
    fn derivation_matches_music21() {
        let pitches = |names: &[&str]| {
            names
                .iter()
                .map(|name| Pitch::from_name(*name).unwrap())
                .collect::<Vec<_>>()
        };
        type Row = (
            &'static [&'static str],
            ScaleType,
            &'static str,
            [(&'static str, usize); 4],
            &'static [&'static str],
        );
        let cases: [Row; 12] = [
            (
                &["C", "E", "G"],
                ScaleType::Major,
                "G",
                [("G", 3), ("F", 3), ("C", 3), ("B-", 2)],
                &["G", "F", "C"],
            ),
            (
                &["C", "E", "G"],
                ScaleType::Minor,
                "A",
                [("A", 3), ("E", 3), ("D", 3), ("B", 2)],
                &["A", "E", "D"],
            ),
            (
                &["C", "E", "G"],
                ScaleType::Dorian,
                "A",
                [("A", 3), ("G", 3), ("D", 3), ("B-", 2)],
                &["A", "G", "D"],
            ),
            (
                &["F#", "A", "C#", "E"],
                ScaleType::Major,
                "A",
                [("A", 4), ("E", 4), ("D", 4), ("B", 3)],
                &["A", "E", "D"],
            ),
            (
                &["F#", "A", "C#", "E"],
                ScaleType::Minor,
                "B",
                [("B", 4), ("F#", 4), ("D-", 4), ("C#", 4)],
                &["B", "F#", "D-", "C#", "C-"],
            ),
            (
                &["G#", "B", "D", "F"],
                ScaleType::HarmonicMinor,
                "A",
                [("A", 4), ("F#", 4), ("E-", 4), ("D#", 4)],
                &["A", "F#", "E-", "D#", "C"],
            ),
            (
                &["B-", "D", "F", "A-"],
                ScaleType::Major,
                "E-",
                [("E-", 4), ("D#", 4), ("B-", 3), ("G#", 3)],
                &["E-", "D#"],
            ),
            (
                &["B-", "D", "F", "A-"],
                ScaleType::Dorian,
                "F",
                [("F", 4), ("B-", 3), ("G#", 3), ("G", 3)],
                &["F"],
            ),
            (
                &["C", "D", "E", "F#", "G", "A", "B"],
                ScaleType::Major,
                "G",
                [("G", 7), ("D", 6), ("C", 6), ("A", 5)],
                &["G"],
            ),
            (
                &["C", "D", "E", "F#", "G", "A", "B"],
                ScaleType::Minor,
                "E",
                [("E", 7), ("B", 6), ("A", 6), ("C-", 6)],
                &["E"],
            ),
            (
                &["C", "C#", "D"],
                ScaleType::Major,
                "B-",
                [("B-", 2), ("A", 2), ("G#", 2), ("G", 2)],
                &[],
            ),
            (
                &["E-", "G", "B-"],
                ScaleType::Major,
                "B-",
                [("B-", 3), ("G#", 3), ("E-", 3), ("D#", 3)],
                &["B-", "G#", "E-", "D#"],
            ),
        ];
        for (names, scale_type, best, ranked, all) in cases {
            let input = pitches(names);
            assert_eq!(
                scale_type.derive(&input).unwrap().tonic().name(),
                best,
                "{scale_type:?} {names:?}"
            );
            let top = scale_type
                .derive_ranked(&input, Some(4))
                .unwrap()
                .into_iter()
                .map(|(matched, scale)| (scale.tonic().name(), matched))
                .collect::<Vec<_>>();
            let expected = ranked
                .iter()
                .map(|(tonic, matched)| (tonic.to_string(), *matched))
                .collect::<Vec<_>>();
            assert_eq!(top, expected, "{scale_type:?} {names:?}");
            let complete = scale_type
                .derive_all(&input)
                .unwrap()
                .iter()
                .map(|scale| scale.tonic().name())
                .collect::<Vec<_>>();
            assert_eq!(complete, all, "{scale_type:?} {names:?}");
        }
    }

    #[test]
    fn degree_with_accidental_matches_music21() {
        let scale = Scale::new(ScaleType::Major, Pitch::from_name("C4").unwrap());
        let cases = [
            ("C4", 1, None),
            ("D4", 2, None),
            ("E-4", 3, Some("flat")),
            ("F#4", 4, Some("sharp")),
            ("G", 5, None),
            ("A#", 6, Some("sharp")),
            ("B-", 7, Some("flat")),
            ("D--4", 2, Some("double-flat")),
            ("C#4", 1, Some("sharp")),
            ("E#4", 3, Some("sharp")),
        ];
        for (name, degree, accidental) in cases {
            let (found, found_accidental) = scale
                .degree_and_accidental_of(&Pitch::from_name(name).unwrap())
                .unwrap();
            assert_eq!(found, degree, "{name}");
            assert_eq!(
                found_accidental
                    .as_ref()
                    .map(|accidental| accidental.name()),
                accidental,
                "{name}"
            );
        }
    }

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
            let mut plagal_names = names(plagal, "C4");
            plagal_names.sort();
            plagal_names.dedup();
            let mut authentic_names = names(authentic, "C4");
            authentic_names.sort();
            authentic_names.dedup();
            assert_eq!(
                plagal_names, authentic_names,
                "{plagal:?} should share {authentic:?}'s pitch collection"
            );
        }
    }

    #[test]
    fn a_plagal_mode_is_realized_a_fourth_below_its_final() {
        // music21's own example: the hypodorian on D runs A3 to A4, and the
        // note it comes to rest on is its fourth degree.
        let scale = Scale::new(
            ScaleType::Hypodorian,
            Pitch::from_name("d").expect("valid tonic"),
        );
        let realized: Vec<String> = scale
            .pitches()
            .expect("scale realizes")
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(
            realized,
            ["A3", "B3", "C4", "D4", "E4", "F4", "G4", "A4"],
            "a plagal mode starts a fourth below its final"
        );
        assert_eq!(scale.final_pitch().unwrap().name_with_octave(), "D4");
        assert_eq!(scale.dominant().unwrap().name_with_octave(), "F4");

        // An authentic mode starts on its own final, and its dominant is the
        // fifth degree.
        let dorian = Scale::new(
            ScaleType::Dorian,
            Pitch::from_name("d").expect("valid tonic"),
        );
        assert_eq!(dorian.final_pitch().unwrap().name_with_octave(), "D4");
        assert_eq!(dorian.dominant().unwrap().name_with_octave(), "A4");
    }

    #[test]
    fn a_leading_tone_is_a_semitone_below_the_final() {
        // In a minor scale that is not the seventh degree the scale has.
        let minor = Scale::new(
            ScaleType::Minor,
            Pitch::from_name("c").expect("valid tonic"),
        );
        assert_eq!(minor.pitch_at_degree(7).unwrap().name(), "B-");
        assert_eq!(minor.leading_tone().unwrap().name(), "B");
        // In a major scale it already is.
        let major = Scale::new(
            ScaleType::Major,
            Pitch::from_name("C").expect("valid tonic"),
        );
        assert_eq!(major.leading_tone().unwrap().name_with_octave(), "B4");
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
    fn rag_marwa_dips_below_its_sixth_degree() {
        // music21's ascending network steps down a M2 from B before closing on
        // C, so A appears twice and the line is not monotonic — and then it
        // carries on a semitone past the octave, which is the one scale here
        // that ends above where it closed.
        assert_eq!(
            names(ScaleType::RagMarwa, "C4"),
            ["C", "D-", "E", "F#", "A", "B", "A", "C", "D-"]
        );
        assert_eq!(
            names(ScaleType::RagMarwa, "E-4"),
            ["E-", "F-", "G", "A", "C", "D", "C", "E-", "F-"]
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
    fn a_note_a_scale_stands_twice_has_two_notes_below_it() {
        // Rag Marwa comes down through the flat second twice over: once
        // above the octave and once above the tonic. music21 reads `D-2` as
        // either, and the note below it is `B1` or `C2` accordingly.
        let marwa = Scale::new(ScaleType::RagMarwa, Pitch::from_name("C4").unwrap()).descending();
        let from = Pitch::from_name("D-2").unwrap();
        assert_eq!(marwa.places_of(&from).unwrap(), 2);
        let below: Vec<String> = (0..2)
            .map(|place| {
                marwa
                    .next_pitch_below_from(&from, 1, place)
                    .unwrap()
                    .name_with_octave()
            })
            .collect();
        assert_eq!(below, ["B1", "C2"]);

        // A note the scale stands only once is the same whichever place is
        // asked for, so a caller that always asks for the first is right.
        let major = Scale::new(ScaleType::Major, Pitch::from_name("C4").unwrap());
        let d = Pitch::from_name("D4").unwrap();
        assert_eq!(major.places_of(&d).unwrap(), 1);
        assert_eq!(
            major
                .next_pitch_below_from(&d, 1, 7)
                .unwrap()
                .name_with_octave(),
            "C4"
        );
    }

    #[test]
    fn degree_lookup_matches_the_realized_pitches() {
        for scale_type in ScaleType::ALL {
            let scale = Scale::new(scale_type, Pitch::from_name("E-4").unwrap());
            let pitches = scale.pitches().unwrap();
            // Every degree but the closing octave, which is the first degree
            // again as far as a degree lookup is concerned. Asked by the
            // scale's own degrees, since Rag Asawari's are not its positions.
            let degrees = scale
                .named_degrees()
                .unwrap_or_else(|| (1..=scale.degree_count() as IntegerType).collect());
            for (expected, degree) in pitches.iter().zip(degrees) {
                let actual = scale.pitch_at_degree(degree).unwrap();
                assert_eq!(
                    actual.name_with_octave(),
                    expected.name_with_octave(),
                    "{scale_type:?} degree {degree}"
                );
                // Read back the other way: the degree the note is found on
                // stands on that note. Not necessarily the degree asked for
                // — Rag Marwa's A is both its fifth and its seventh.
                let found = scale.degree_of(expected).unwrap().unwrap();
                assert_eq!(
                    scale
                        .pitch_at_degree(found as IntegerType)
                        .unwrap()
                        .name_with_octave(),
                    expected.name_with_octave(),
                    "{scale_type:?} degree {degree} read back as {found}"
                );
            }
            // A degree the scale does not have is nothing, rather than the
            // note that would stand there had the degrees been counted off.
            if let Some(named) = scale.named_degrees() {
                for degree in 1..=*named.last().unwrap() {
                    assert_eq!(
                        scale.pitch_on_degree(degree).unwrap().is_some(),
                        named.contains(&degree),
                        "{scale_type:?} degree {degree}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_degree_outside_the_scale_counts_round_it() {
        // music21's own answers for `scale.PhrygianScale('g').pitchFromDegree`.
        let scale = Scale::new(ScaleType::Phrygian, Pitch::from_name("G4").unwrap());
        for (degree, expected) in [
            (-1, "E-5"),
            (0, "F5"),
            (1, "G4"),
            (7, "F5"),
            (8, "G4"),
            (11, "C5"),
            (15, "G4"),
        ] {
            assert_eq!(
                scale.pitch_at_degree(degree).unwrap().name_with_octave(),
                expected,
                "degree {degree}"
            );
        }
    }

    #[test]
    fn every_scale_realizes_on_every_common_tonic() {
        for scale_type in ScaleType::ALL {
            for tonic in [
                "C4", "G4", "D4", "A4", "E4", "B4", "F#4", "F4", "B-4", "E-4", "A-4",
            ] {
                let scale = Scale::new(scale_type, Pitch::from_name(tonic).unwrap());
                let pitches = scale.pitches().expect("scale realizes");
                let beyond = usize::from(scale_type.beyond_terminus().is_some());
                assert_eq!(
                    pitches.len(),
                    scale_type.degree_count() + 1 + beyond,
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
