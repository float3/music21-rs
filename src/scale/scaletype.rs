//! The named scales music21 exposes as `ConcreteScale` subclasses.
//!
//! Each scale is a sequence of step intervals walked from the tonic (upward,
//! except for the one descending step Rag Marwa's network contains),
//! matching the edges of music21's `IntervalNetwork`, plus the pitch
//! simplification that network applies. Both together are needed: the steps
//! alone give the right pitch classes but the wrong spelling for scales that
//! run out of reasonable accidentals, which is why a C whole-tone scale ends
//! `A#` rather than `B-` but a B whole-tone scale does not end `A##`.

use crate::chord::{Chord, root};
use crate::defaults::{FloatType, IntegerType};
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

fn step_interval(name: &str) -> Result<Interval> {
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
    /// [`Self::steps`] is the collection written from the final, which is
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
            // The sixth step is music21's `M-2` edge: a descending major
            // second inside an otherwise ascending network.
            Self::RagMarwa => &["m2", "a2", "M2", "m3", "M2", "-M2", "m3"],
        }
    }

    /// Returns how this scale respells pitches, matching music21's network.
    fn simplification(self) -> Simplification {
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
const SCALE_STARTS: [&str; 15] = [
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
const MAX_RANGE_OCTAVES: usize = 12;

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
    fn key(self, pitch: &Pitch) -> String {
        match self {
            Self::PitchClass => pitch.pitch_class().number().to_string(),
            Self::Name => pitch.name(),
            Self::Step => pitch.name().chars().take(1).collect(),
        }
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
    /// The steps of a scale nobody has a name for.
    ///
    /// music21's `ConcreteScale(pitches=[...])` is a scale given by its
    /// notes rather than by a name, and it behaves as any other scale does —
    /// it realizes, it has degrees, it can be matched against. `None` is the
    /// ordinary case, where the steps come from the named type.
    ///
    /// They are intervals and not names: a step between microtonal pitches
    /// has no name to be written and read back through.
    #[cfg_attr(feature = "serde", serde(default))]
    custom_steps: Option<Vec<Interval>>,
}

impl Scale {
    /// Builds a scale of the given type on a tonic.
    pub fn new(scale_type: ScaleType, tonic: Pitch) -> Self {
        Self {
            scale_type,
            tonic,
            custom_steps: None,
        }
    }

    /// A scale given by the notes of one octave of it rather than by a name:
    /// music21's `ConcreteScale(pitches=[...])`.
    ///
    /// The first pitch is the tonic, the steps are the intervals between
    /// neighbours, and the scale closes back on the octave, so the notes
    /// repeat an octave higher as any scale's do.
    pub fn from_pitches(pitches: &[Pitch]) -> Result<Self> {
        let pitches = rising_octaves(pitches);
        let pitches = pitches.as_slice();
        let Some(tonic) = pitches.first() else {
            return Err(crate::error::Error::Scale(
                "a scale needs at least one pitch".to_string(),
            ));
        };
        let mut steps = Vec::with_capacity(pitches.len());
        for pair in pitches.windows(2) {
            steps.push(Interval::between_pitches(&pair[0], &pair[1])?);
        }
        // The closing step back to the tonic, so the collection repeats.
        // Notes that already close on it need none — and they may close two
        // octaves up rather than one, which is a pattern two octaves long and
        // not a scale that folds back on itself.
        let last = pitches.last().unwrap_or(tonic);
        let span = last.ps() - tonic.ps();
        if span.rem_euclid(12.0) != 0.0 {
            let octaves = (span / 12.0).floor() + 1.0;
            let closing = tonic.transpose(&Interval::from_semitones(
                (octaves * 12.0) as crate::defaults::IntegerType,
            )?)?;
            steps.push(Interval::between_pitches(last, &closing)?);
        }
        Ok(Self {
            scale_type: ScaleType::Major,
            tonic: tonic.clone(),
            custom_steps: Some(steps),
        })
    }

    /// This scale as it sounds coming down, which for most is itself.
    pub fn descending(&self) -> Scale {
        match self.custom_steps {
            Some(_) => self.clone(),
            None => match self.scale_type.descending_form() {
                Some(scale_type) => Scale::new(scale_type, self.tonic.clone()),
                None => self.clone(),
            },
        }
    }

    /// The scale coming down: highest note first, through the collection it
    /// uses descending.
    pub fn pitches_descending(&self) -> Result<Vec<Pitch>> {
        let mut pitches = self.descending().pitches()?;
        pitches.reverse();
        Ok(pitches)
    }

    /// A range of the scale coming down, highest note first.
    pub fn pitches_between_descending(
        &self,
        minimum: &Pitch,
        maximum: &Pitch,
    ) -> Result<Vec<Pitch>> {
        let mut pitches = self.descending().pitches_between(minimum, maximum)?;
        pitches.reverse();
        Ok(pitches)
    }

    /// Whether this scale was given by its notes rather than by a name.
    pub fn is_custom(&self) -> bool {
        self.custom_steps.is_some()
    }

    /// Moves the scale to a new tonic, keeping its pattern of steps.
    pub fn set_tonic(&mut self, tonic: Pitch) {
        self.tonic = tonic;
    }

    /// The scales of *this* pattern that contain the most of `pitches`, best
    /// first: music21's `deriveRanked` on the scale rather than on the type,
    /// which is what a scale given by its notes has to use.
    pub fn derive_ranked_by(
        &self,
        pitches: &[Pitch],
        limit: Option<usize>,
        comparison: DegreeComparison,
    ) -> Result<Vec<(usize, Scale)>> {
        if !self.is_custom() {
            return self.scale_type.derive_ranked_by(pitches, limit, comparison);
        }
        let targets: Vec<String> = pitches.iter().map(|p| comparison.key(p)).collect();
        let mut ranked = Vec::with_capacity(SCALE_STARTS.len());
        for start in SCALE_STARTS {
            let mut candidate = self.clone();
            candidate.set_tonic(Pitch::from_name(start)?);
            let degrees: Vec<String> = candidate
                .pitches()?
                .iter()
                .map(|p| comparison.key(p))
                .collect();
            let matched = targets
                .iter()
                .filter(|target| degrees.contains(target))
                .count();
            ranked.push((matched, candidate));
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

    /// The steps walked from where the scale is realized.
    fn walk(&self) -> Result<Vec<Interval>> {
        match &self.custom_steps {
            Some(steps) => Ok(steps.clone()),
            None => self
                .scale_type
                .realization_steps()
                .into_iter()
                .map(step_interval)
                .collect(),
        }
    }

    /// Returns the number of distinct degrees: music21's `getDegreeMaxUnique`,
    /// seven for a major scale and twelve for the chromatic.
    pub fn degree_count(&self) -> usize {
        match &self.custom_steps {
            Some(steps) => steps.len(),
            None => self.scale_type.degree_count(),
        }
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
    ///
    /// A tonic with no octave is realized in octave 4, which is what music21
    /// does: the scale on a bare `G` runs `G4 A4 B-4 C5 …`. The tonic itself
    /// keeps its own spelling — `Scale::tonic` still has no octave — because
    /// the octave belongs to the realization and not to the scale.
    pub fn pitches(&self) -> Result<Vec<Pitch>> {
        let simplification = self.scale_type.simplification();
        let start = self.realization_start()?;
        let mut pitches = Vec::with_capacity(self.scale_type.degree_count() + 1);
        pitches.push(start.clone());

        let mut current = start;
        for step in self.walk()? {
            current = advance(&current, &step, simplification)?;
            pitches.push(current.clone());
        }
        if self.custom_steps.is_none()
            && let Some(beyond) = self.scale_type.beyond_terminus()
        {
            pitches.push(advance(&current, &step_interval(beyond)?, simplification)?);
        }
        Ok(pitches)
    }

    /// The pitch the scale is realized from, which is its final except in a
    /// plagal mode, where the range starts below it.
    ///
    /// How far below is the scale's own business rather than a flat fourth:
    /// the walk goes back down the last steps of the collection, so the
    /// hypolocrian on C starts on `G-` and not on `G`, its fifth degree
    /// being diminished.
    fn realization_start(&self) -> Result<Pitch> {
        let mut start = self.realized_tonic();
        if self.custom_steps.is_some() {
            return Ok(start);
        }
        let steps = self.scale_type.steps();
        for step in steps
            .iter()
            .rev()
            .take(self.scale_type.tonic_degree().saturating_sub(1))
        {
            start = start.transpose(&step_interval(step)?.reversed()?)?;
        }
        Ok(start)
    }

    /// The tonic as the scale sounds it: music21's `getTonic`, which is the
    /// tonic in octave 4 when it was given without one.
    pub fn realized_tonic(&self) -> Pitch {
        let mut tonic = self.tonic.clone();
        if tonic.octave().is_none() {
            tonic.octave_setter(Some(crate::defaults::PITCH_OCTAVE as IntegerType));
        }
        tonic
    }

    /// The major scale written with the same key signature as this one:
    /// music21's `getRelativeMajor`.
    ///
    /// A mode is written with the signature of the major scale it is a
    /// rotation of, so D dorian is C major and E minor is G major. Only the
    /// seven-note modes have one.
    pub fn relative_major(&self) -> Result<Scale> {
        self.relative(ScaleType::Major)
    }

    /// The minor scale written with the same key signature: music21's
    /// `getRelativeMinor`.
    pub fn relative_minor(&self) -> Result<Scale> {
        self.relative(ScaleType::Minor)
    }

    /// The major scale on the same tonic: music21's `getParallelMajor`.
    pub fn parallel_major(&self) -> Scale {
        Scale::new(ScaleType::Major, self.tonic.clone())
    }

    /// The minor scale on the same tonic: music21's `getParallelMinor`.
    pub fn parallel_minor(&self) -> Scale {
        Scale::new(ScaleType::Minor, self.tonic.clone())
    }

    /// The scale of the wanted type carrying this one's key signature.
    ///
    /// It stands at or above this scale, in the same octave where that is
    /// possible: the relative major of A minor on `A4` is C major on `C5`,
    /// because a `C4` would sound below the scale it came from.
    fn relative(&self, wanted: ScaleType) -> Result<Scale> {
        let mode = self.scale_type.music21_descriptive_name();
        let sharps = crate::key::pitch_to_sharps(&self.tonic, Some(mode))?;
        let key = crate::key::KeySignature::new(sharps)
            .try_as_key(Some(wanted.music21_descriptive_name()), None)?;
        let here = self.realized_tonic();
        let mut tonic = key.tonic();
        tonic.set_octave(here.octave());
        if tonic.ps() < here.ps() {
            tonic.set_octave(tonic.octave().map(|octave| octave + 1));
        }
        Ok(Scale::new(wanted, tonic))
    }

    /// Returns the pitch at a one-based scale degree.
    ///
    /// Degree 1 is the tonic. Every other degree is read within the one
    /// octave the scale is realized in, so the eighth degree is the tonic
    /// again and not the octave above it, and the zeroth and the negative
    /// degrees count back round from the top. That is music21's
    /// `pitchFromDegree`, which asks its interval network for the node the
    /// degree names and gets one of the nodes it has.
    pub fn pitch_at_degree(&self, degree: IntegerType) -> Result<Pitch> {
        let simplification = self.scale_type.simplification();
        let steps = self.walk()?;
        let count = self.degree_count().max(1) as IntegerType;
        let wrapped = (degree - 1).rem_euclid(count);
        let mut current = self.realization_start()?;
        for index in 0..wrapped as usize {
            current = advance(&current, &steps[index % steps.len()], simplification)?;
        }
        Ok(current)
    }
    /// Returns every pitch of the scale from `minimum` up to `maximum`,
    /// inclusive: music21's `getPitches` given a range.
    ///
    /// The scale is realized from the tonic in whatever octave puts it at or
    /// below the bottom of the range, then walked upward, so asking a C major
    /// scale for `E-5` to `G-7` starts at `E5` — the first scale pitch that
    /// is not below the bottom — and not at a respelled `E-5`.
    pub fn pitches_between(&self, minimum: &Pitch, maximum: &Pitch) -> Result<Vec<Pitch>> {
        // Asked the other way round, music21 walks down instead: the same
        // pitches, highest first.
        if maximum.ps() < minimum.ps() {
            let mut descending = self.pitches_between(maximum, minimum)?;
            descending.reverse();
            return Ok(descending);
        }
        let lowest = minimum.ps();
        let highest = maximum.ps();
        let simplification = self.scale_type.simplification();
        let steps = self.walk()?;
        // Down whole periods until the start is at or below the range. A
        // period is usually the octave, but a scale given by its notes may
        // take two to come back to where it began, and dropping by one would
        // start the pattern halfway through itself.
        let period = self.period_in_octaves(&steps);
        let mut current = self.realization_start()?;
        while current.ps() > lowest {
            let octave = current.octave().unwrap_or(0);
            current.octave_setter(Some(octave - period));
        }
        let mut pitches = Vec::new();
        // Two octaves of headroom past the range, so a scale whose degrees
        // are not evenly spaced still reaches the top of it.
        let limit = steps.len() * (MAX_RANGE_OCTAVES + 2) + 1;
        for index in 0..limit {
            let sounding = current.ps();
            if sounding > highest {
                break;
            }
            if sounding >= lowest {
                pitches.push(current.clone());
            }
            current = advance(&current, &steps[index % steps.len()], simplification)?;
        }
        Ok(pitches)
    }

    /// Whether the pattern can be walked at all.
    ///
    /// A collection given by its notes may rise and fall back to where it
    /// began — `A4 B4 C4 D4 E4 F4 G4 A4` does — and a pattern that goes
    /// nowhere cannot be realized over a range, however many times it is
    /// walked. music21 says so as well, out of the network it walks.
    pub fn is_realizable(&self) -> bool {
        match &self.custom_steps {
            Some(steps) => steps.iter().map(Interval::semitones).sum::<FloatType>() > 0.0,
            None => true,
        }
    }

    /// Whether the pattern repeats at the octave: music21's
    /// `octaveDuplicating`. Every named scale does, and one given by its
    /// notes need not — a collection spanning two octaves before it comes
    /// back to its tonic is a pattern two octaves long.
    pub fn octave_duplicating(&self) -> bool {
        match &self.custom_steps {
            Some(steps) => self.period_in_octaves(steps) == 1,
            None => true,
        }
    }

    /// How many octaves the pattern takes to come back to where it began,
    /// which is one for every scale that has a name and may be more for one
    /// given by its notes.
    fn period_in_octaves(&self, steps: &[Interval]) -> IntegerType {
        let semitones: FloatType = steps.iter().map(Interval::semitones).sum();
        ((semitones / 12.0).round() as IntegerType).max(1)
    }

    /// The note the scale comes to rest on, as it sounds: music21's
    /// `getTonic`, which is the fourth degree of a plagal mode.
    pub fn final_pitch(&self) -> Result<Pitch> {
        self.pitch_at_degree(self.scale_type.tonic_degree() as IntegerType)
    }

    /// The reciting tone: music21's `getDominant`.
    pub fn dominant(&self) -> Result<Pitch> {
        self.pitch_at_degree(self.scale_type.dominant_degree() as IntegerType)
    }

    /// The seventh degree raised or lowered to sit a semitone below the
    /// final: music21's `getLeadingTone`, which in a minor scale is not the
    /// seventh degree the scale itself has.
    pub fn leading_tone(&self) -> Result<Pitch> {
        let seventh = self.pitch_at_degree(7)?;
        let tonic = self.final_pitch()?;
        let distance = seventh.midi() - tonic.midi();
        if distance == 11 {
            return Ok(seventh);
        }
        let alter = seventh.accidental().alter() + FloatType::from(11 - distance);
        let mut raised = seventh.clone();
        raised.set_accidental(Some(crate::pitch::Accidental::new(alter)?));
        Ok(raised)
    }

    /// Returns the scale of the same type on which `pitch` is the given degree:
    /// music21's `deriveByDegree`, so the major scale with `E` as its fifth is
    /// A major. The pitch keeps its spelling; a pitch without an octave is
    /// read in octave 4, as music21 reads it, so the new tonic has one.
    pub fn derive_by_degree(&self, degree: usize, pitch: &Pitch) -> Result<Scale> {
        let implicit_octave = Some(crate::defaults::PITCH_OCTAVE as IntegerType);
        let mut tonic = self.tonic.clone();
        if tonic.octave().is_none() {
            tonic.octave_setter(implicit_octave);
        }
        let degree_pitch =
            Scale::new(self.scale_type, tonic.clone()).pitch_at_degree(degree as IntegerType)?;
        let up_to_degree = Interval::between_pitches(&tonic, &degree_pitch)?;
        let mut reference = pitch.clone();
        if reference.octave().is_none() {
            reference.octave_setter(implicit_octave);
        }
        let new_tonic = reference.transpose(&up_to_degree.reversed()?)?;
        Ok(Scale::new(self.scale_type, new_tonic))
    }

    /// Returns the same scale type on the tonic transposed by `interval`.
    pub fn transpose(&self, interval: &Interval) -> Result<Scale> {
        Ok(Scale::new(self.scale_type, self.tonic.transpose(interval)?))
    }

    /// Returns one octave of the scale as a chord, tonic through octave:
    /// music21's `getChord`.
    pub fn chord(&self) -> Result<Chord> {
        Chord::new(self.pitches()?.as_slice())
    }

    /// Returns the pitches at the given degrees within one octave of the
    /// tonic: music21's `pitchesFromScaleDegrees`, which realizes tonic
    /// through octave once and so silently drops a degree beyond the octave.
    pub fn pitches_from_scale_degrees(&self, degrees: &[usize]) -> Result<Vec<Pitch>> {
        let octave = self.pitches()?;
        // The realization closes on the tonic an octave up, and that closing
        // pitch is degree one again — music21 asks the whole realization
        // which of its pitches stand on the degrees wanted, so the first
        // degree of A minor answers with both `A3` and `A4`.
        let count = octave.len().saturating_sub(1).max(1);
        Ok(octave
            .into_iter()
            .enumerate()
            .filter(|(index, _)| degrees.contains(&(index % count + 1)))
            .map(|(_, pitch)| pitch)
            .collect())
    }

    /// Every pitch of the named degrees between two pitches: music21's
    /// `pitchesFromScaleDegrees` given a range, so the third and seventh of
    /// C major from `c2` to `c6` are `D2 G2 D3 G3 D4 G4 D5 G5`.
    pub fn pitches_from_scale_degrees_between(
        &self,
        degrees: &[usize],
        minimum: &Pitch,
        maximum: &Pitch,
    ) -> Result<Vec<Pitch>> {
        let wanted: Vec<String> = self
            .pitches_from_scale_degrees(degrees)?
            .iter()
            .map(Pitch::name)
            .collect();
        Ok(self
            .pitches_between(minimum, maximum)?
            .into_iter()
            .filter(|pitch| wanted.contains(&pitch.name()))
            .collect())
    }

    /// Returns the interval from one degree to another, both folded into the
    /// first octave the way music21's `pitchFromDegree` folds them, so degree
    /// 9 of a seven-note scale is degree 2 and the interval from 2 to 9 is a
    /// unison.
    pub fn interval_between_degrees(&self, start: usize, end: usize) -> Result<Interval> {
        Interval::between_pitches(
            &self.pitch_at_degree(start as IntegerType)?,
            &self.pitch_at_degree(end as IntegerType)?,
        )
    }

    /// Returns whether `other` is the scale pitch `steps` degrees above
    /// `origin`, compared by name so the octave does not matter: music21's
    /// `isNext`.
    pub fn is_next(&self, other: &Pitch, origin: &Pitch, steps: usize) -> Result<bool> {
        Ok(self.next_pitch_above(origin, steps)?.name() == other.name())
    }

    /// Splits pitches into those whose names the scale contains and those it
    /// does not: music21's `match`. The matched list carries the scale's own
    /// pitches, realized from the tonic in octave 4 when it has none, and
    /// the unmatched list carries the pitches as given.
    pub fn match_pitches(&self, pitches: &[Pitch]) -> Result<(Vec<Pitch>, Vec<Pitch>)> {
        self.match_pitches_by(pitches, DegreeComparison::Name)
    }

    /// The same, saying how a pitch is matched against a degree.
    ///
    /// Both lists hold the pitches as given rather than the scale's own —
    /// music21 hands its targets straight back — except that one with no
    /// octave is heard in octave 4, since that is where the scale sounds.
    pub fn match_pitches_by(
        &self,
        pitches: &[Pitch],
        comparison: DegreeComparison,
    ) -> Result<(Vec<Pitch>, Vec<Pitch>)> {
        let realized = self.realized_in_implicit_octave()?;
        let degrees: Vec<String> = realized.iter().map(|p| comparison.key(p)).collect();
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for pitch in pitches {
            let mut heard = pitch.clone();
            if heard.octave().is_none() {
                heard.octave_setter(Some(crate::defaults::PITCH_OCTAVE as IntegerType));
            }
            if degrees.contains(&comparison.key(&heard)) {
                matched.push(heard);
            } else {
                unmatched.push(heard);
            }
        }
        Ok((matched, unmatched))
    }

    /// Returns the scale pitches, tonic through octave, whose pitch classes
    /// none of `pitches` has: music21's `findMissing`, so C major against
    /// `C E G` is `D4 F4 A4 B4`.
    pub fn find_missing(&self, pitches: &[Pitch]) -> Result<Vec<Pitch>> {
        let present: Vec<u8> = pitches.iter().map(root::pitch_class).collect();
        Ok(self
            .realized_in_implicit_octave()?
            .into_iter()
            .filter(|candidate| !present.contains(&root::pitch_class(candidate)))
            .collect())
    }

    /// Returns the solfège syllable for a pitch, `do` through `ti` with the
    /// chromatic inflections (`di`, `ra`, …): music21's `solfeg`. Without
    /// `chromatic` the plain syllable of the degree is returned whatever the
    /// accidental. Errors for degrees past seven and alterations past a
    /// double sharp or flat.
    pub fn solfeg(&self, pitch: &Pitch, variant: SolfegVariant, chromatic: bool) -> Result<String> {
        let (degree, accidental) = self.degree_and_accidental_of(pitch)?;
        if degree > 7 {
            return Err(crate::error::Error::Scale(
                "Cannot call solfeg on non-7-degree scales".to_string(),
            ));
        }
        let table = match variant {
            SolfegVariant::Music21 => &SOLFEG_SYLLABLES,
            SolfegVariant::Humdrum => &HUMDRUM_SOLFEG_SYLLABLES,
        };
        let alter = if chromatic {
            accidental.map_or(0, |accidental| accidental.alter() as IntegerType)
        } else {
            0
        };
        let column = usize::try_from(alter + 2)
            .ok()
            .filter(|column| *column < 5)
            .ok_or_else(|| {
                crate::error::Error::Scale(format!(
                    "no solfeg syllable for an alteration of {alter}"
                ))
            })?;
        Ok(table[degree - 1][column].to_string())
    }

    fn realized_in_implicit_octave(&self) -> Result<Vec<Pitch>> {
        self.pitches()
    }

    /// Returns the one-based degree matching a pitch under the given
    /// comparison, or `None` when the scale does not have it.
    pub fn degree_of_by(
        &self,
        pitch: &Pitch,
        comparison: DegreeComparison,
    ) -> Result<Option<usize>> {
        let wanted = comparison.key(pitch);
        Ok(self
            .scale_pitches()?
            .iter()
            .position(|candidate| comparison.key(candidate) == wanted)
            .map(|index| index + 1))
    }

    /// Returns the one-based degree whose pitch name matches, ignoring octave,
    /// or `None` when the pitch is not in the scale.
    pub fn degree_of(&self, pitch: &Pitch) -> Result<Option<usize>> {
        let name = pitch.name();
        Ok(self
            .scale_pitches()?
            .iter()
            .position(|candidate| candidate.name() == name)
            .map(|index| index + 1))
    }

    /// Returns the one-based degree whose pitch class matches, so `F-` finds
    /// the `E` of C major.
    pub fn degree_of_pitch_class(&self, pitch: &Pitch) -> Result<Option<usize>> {
        let pitch_class = pitch.pitch_class().number();
        Ok(self
            .scale_pitches()?
            .iter()
            .position(|candidate| candidate.pitch_class().number() == pitch_class)
            .map(|index| index + 1))
    }

    /// Returns the scale pitch `steps` degrees above `origin`. A pitch outside
    /// the scale first moves to the nearest scale pitch above it.
    pub fn next_pitch_above(&self, origin: &Pitch, steps: usize) -> Result<Pitch> {
        self.pitch_steps_from(origin, steps as IntegerType, None)
    }

    /// The same, told which side of a pitch outside the scale to start from:
    /// music21's `getNeighbor`, where naming a side means stepping the whole
    /// way from the neighbour on it rather than counting the move onto the
    /// scale as one of the steps.
    pub fn next_pitch_beside(
        &self,
        origin: &Pitch,
        steps: IntegerType,
        below: bool,
    ) -> Result<Pitch> {
        self.pitch_steps_from(origin, steps, Some(below))
    }

    /// Returns the scale pitch `steps` degrees below `origin`. A pitch outside
    /// the scale first moves to the nearest scale pitch below it.
    pub fn next_pitch_below(&self, origin: &Pitch, steps: usize) -> Result<Pitch> {
        self.pitch_steps_from(origin, -(steps as IntegerType), None)
    }

    /// Returns the degree a pitch sits on together with the accidental that
    /// separates it from the scale's own spelling of that degree, so `E-` in
    /// C major is degree three with a flat. Errors when no degree shares the
    /// pitch's letter.
    pub fn degree_and_accidental_of(
        &self,
        pitch: &Pitch,
    ) -> Result<(usize, Option<crate::pitch::Accidental>)> {
        if let Some(degree) = self.degree_of(pitch)? {
            return Ok((degree, None));
        }
        let pitches = self.scale_pitches()?;
        let index = pitches
            .iter()
            .position(|candidate| candidate.step() == pitch.step())
            .ok_or_else(|| {
                crate::error::Error::Scale(format!(
                    "cannot get any scale degree for {pitch} in {self:?}"
                ))
            })?;
        let difference = pitch.accidental().alter() - pitches[index].accidental().alter();
        let accidental = if difference == 0.0 {
            None
        } else {
            Some(crate::pitch::Accidental::new(difference)?)
        };
        Ok((index + 1, accidental))
    }

    fn scale_pitches(&self) -> Result<Vec<Pitch>> {
        let mut pitches = self.pitches()?;
        pitches.truncate(self.scale_type.degree_count());
        Ok(pitches)
    }

    fn pitch_steps_from(
        &self,
        origin: &Pitch,
        steps: IntegerType,
        neighbour_below: Option<bool>,
    ) -> Result<Pitch> {
        if steps == 0 {
            return Err(crate::error::Error::Scale(
                "step size must be at least 1".to_string(),
            ));
        }
        let pitches = self.scale_pitches()?;
        let count = pitches.len() as IntegerType;
        let origin_ps = origin.ps();
        let base_shift = ((origin_ps - self.tonic.ps()) / 12.0).floor() as IntegerType;
        let candidates = (base_shift - 1..=base_shift + 1)
            .flat_map(|shift| {
                pitches.iter().enumerate().map(move |(index, pitch)| {
                    (pitch.ps() + 12.0 * shift as FloatType, index, shift)
                })
            })
            .collect::<Vec<_>>();
        let ascending = steps > 0;
        let name = origin.name();
        let (index, shift, remaining) = match candidates
            .iter()
            .find(|(ps, index, _)| pitches[*index].name() == name && (ps - origin_ps).abs() < 1e-9)
        {
            Some(&(_, index, shift)) => (index, shift, steps),
            None => {
                // Which side of the pitch to come onto the scale at: the way
                // the move is going unless a caller has said otherwise, and
                // then the move onto the scale counts as one of the steps.
                let take_below = neighbour_below.unwrap_or(!ascending);
                let neighbour = if take_below {
                    candidates
                        .iter()
                        .filter(|(ps, _, _)| *ps < origin_ps)
                        .max_by(|left, right| left.0.total_cmp(&right.0))
                } else {
                    candidates
                        .iter()
                        .filter(|(ps, _, _)| *ps > origin_ps)
                        .min_by(|left, right| left.0.total_cmp(&right.0))
                };
                let &(_, index, shift) = neighbour.ok_or_else(|| {
                    crate::error::Error::Scale(format!("no scale pitch beside {origin}"))
                })?;
                let remaining = if neighbour_below.is_some() {
                    steps
                } else {
                    steps - steps.signum()
                };
                (index, shift, remaining)
            }
        };
        let total = index as IntegerType + remaining;
        let mut pitch = pitches[total.rem_euclid(count) as usize].clone();
        let octave_shift = shift + total.div_euclid(count);
        let octave = origin.octave().map(|_| {
            pitch
                .octave()
                .unwrap_or(crate::defaults::PITCH_OCTAVE as IntegerType)
                + octave_shift
        });
        pitch.octave_setter(octave);
        Ok(pitch)
    }
}

/// A collection's notes with the octaves a caller left out filled in so that
/// the collection rises: music21's `fixDefaultOctaveForPitchList`.
///
/// `A B C D E F G# A` is a scale on A, not a collection that climbs a tone
/// and then falls a seventh, and a caller who names no octaves means the
/// first. Notes that carry an octave are left exactly as they are.
fn rising_octaves(pitches: &[Pitch]) -> Vec<Pitch> {
    let mut risen: Vec<Pitch> = Vec::with_capacity(pitches.len());
    let mut last_ps = 0.0;
    let mut last_octave =
        pitches
            .first()
            .map_or(crate::defaults::PITCH_OCTAVE as IntegerType, |pitch| {
                pitch
                    .octave()
                    .unwrap_or(crate::defaults::PITCH_OCTAVE as IntegerType)
            });
    for pitch in pitches {
        let mut pitch = pitch.clone();
        if pitch.octave().is_none() {
            if last_ps > pitch.ps() {
                pitch.octave_setter(Some(last_octave));
            }
            while last_ps > pitch.ps() {
                last_octave += 1;
                pitch.octave_setter(Some(last_octave));
            }
        }
        last_ps = pitch.ps();
        risen.push(pitch);
    }
    risen
}

/// Transposes one scale step, applying the scale's simplification.
fn advance(pitch: &Pitch, interval: &Interval, simplification: Simplification) -> Result<Pitch> {
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
    fn degree_lookup_matches_the_realized_pitches() {
        for scale_type in ScaleType::ALL {
            let scale = Scale::new(scale_type, Pitch::from_name("E-4").unwrap());
            let pitches = scale.pitches().unwrap();
            // Every degree but the closing octave, which is the first degree
            // again as far as a degree lookup is concerned.
            for (index, expected) in pitches.iter().take(scale.degree_count()).enumerate() {
                let actual = scale.pitch_at_degree(index as IntegerType + 1).unwrap();
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
