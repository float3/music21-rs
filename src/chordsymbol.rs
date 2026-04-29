use std::str::FromStr;

use crate::{
    chord::Chord,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    interval::Interval,
    pitch::{Pitch, pitch_class_name},
};
use std::collections::BTreeSet;

/// Tertian quality parsed from a chord symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChordQuality {
    /// Major triad or major-family sonority.
    Major,
    /// Minor triad or minor-family sonority.
    Minor,
    /// Dominant seventh-family sonority.
    Dominant,
    /// Diminished triad or diminished-family sonority.
    Diminished,
    /// Augmented triad sonority.
    Augmented,
    /// Half-diminished seventh-family sonority.
    HalfDiminished,
    /// Suspended-second sonority.
    Suspended2,
    /// Suspended-fourth sonority.
    Suspended4,
    /// Power-chord sonority containing a root and fifth.
    Power,
}

/// A chord-symbol alteration such as `b5` or `#11`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordAlteration {
    degree: u8,
    semitones: IntegerType,
}

impl ChordAlteration {
    /// Creates an alteration for a scale degree and semitone displacement.
    pub fn new(degree: u8, semitones: IntegerType) -> Self {
        Self { degree, semitones }
    }

    /// Returns the altered or added chord degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the semitone displacement from the unaltered degree.
    pub fn semitones(&self) -> IntegerType {
        self.semitones
    }
}

/// Parsed chord symbol.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordSymbol {
    figure: String,
    root: Pitch,
    bass: Option<Pitch>,
    quality: ChordQuality,
    extensions: Vec<u8>,
    alterations: Vec<ChordAlteration>,
    #[cfg_attr(feature = "serde", serde(default))]
    omissions: Vec<u8>,
    #[cfg_attr(feature = "serde", serde(default))]
    additions: Vec<ChordAlteration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SymbolCandidate {
    figure: String,
    score: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SymbolAnalysis {
    suffix: String,
    core_intervals: BTreeSet<u8>,
    omissions: Vec<u8>,
    alteration_count: usize,
    rank: usize,
}

#[derive(Clone, Copy, Debug)]
struct SymbolContext {
    has_major_third: bool,
    has_minor_third: bool,
    has_perfect_fifth: bool,
    has_flat_fifth: bool,
}

impl ChordSymbol {
    /// Parses a chord symbol such as `"Cmaj7"`, `"F#m7b5"`, or `"Bb7#11"`.
    pub fn parse(figure: impl Into<String>) -> Result<Self> {
        let figure = figure.into();
        let trimmed = figure.trim();
        if trimmed.is_empty() {
            return Err(Error::Chord("chord symbol cannot be empty".to_string()));
        }

        let (body, bass) = match trimmed.split_once('/') {
            Some((body, bass)) => (body, Some(parse_pitch_only(bass)?)),
            None => (trimmed, None),
        };
        let (root_name, suffix) = parse_pitch_prefix(body)?;
        let root = Pitch::from_name(root_name)?;
        let suffix_without_additions = strip_addition_groups(suffix);
        let additions = parse_additions(suffix);
        let omissions = parse_omissions(suffix);
        let alterations = parse_alterations(&suffix_without_additions);
        let extensions = parse_extensions(&suffix_without_additions, &alterations);
        let quality = parse_quality(&suffix_without_additions, &alterations);

        Ok(Self {
            figure: trimmed.to_string(),
            root,
            bass,
            quality,
            extensions,
            alterations,
            omissions,
            additions,
        })
    }

    /// Returns the original chord-symbol figure.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the root pitch.
    pub fn root(&self) -> &Pitch {
        &self.root
    }

    /// Returns the slash bass pitch, if one was supplied.
    pub fn bass(&self) -> Option<&Pitch> {
        self.bass.as_ref()
    }

    /// Returns the parsed chord quality.
    pub fn quality(&self) -> ChordQuality {
        self.quality
    }

    /// Returns parsed extension degrees.
    pub fn extensions(&self) -> &[u8] {
        &self.extensions
    }

    /// Returns parsed alterations.
    pub fn alterations(&self) -> &[ChordAlteration] {
        &self.alterations
    }

    /// Returns degrees omitted with `no...` or `omit...` markers.
    pub fn omissions(&self) -> &[u8] {
        &self.omissions
    }

    /// Returns parsed added tones from `add(...)` groups.
    pub fn additions(&self) -> &[ChordAlteration] {
        &self.additions
    }

    /// Realizes the chord symbol as a [`Chord`].
    pub fn to_chord(&self) -> Result<Chord> {
        let mut interval_names = self.base_intervals();

        for extension in [6, 9, 11, 13] {
            if self.extensions.contains(&extension)
                && !self.alterations.iter().any(|alt| alt.degree == extension)
            {
                interval_names.push(default_extension_interval(extension));
            }
        }

        for alteration in &self.alterations {
            if alteration.degree == 5 && matches!(self.quality, ChordQuality::HalfDiminished) {
                continue;
            }
            if alteration.degree == 5 {
                continue;
            }
            interval_names.push(altered_interval(alteration)?);
        }

        for addition in &self.additions {
            interval_names.push(added_interval(addition)?);
        }

        interval_names.sort_unstable_by_key(|name| interval_sort_key(name));
        interval_names.dedup();

        let mut pitches = interval_names
            .into_iter()
            .map(|name| Interval::from_name(name)?.transpose_pitch(&self.root))
            .collect::<Result<Vec<_>>>()?;

        if let Some(bass) = &self.bass {
            if let Some(index) = pitches.iter().position(|pitch| pitch.name() == bass.name()) {
                let bass = pitches.remove(index);
                pitches.insert(0, bass);
            } else {
                pitches.insert(0, bass.clone());
            }
        }

        Chord::new(pitches.as_slice())
    }

    fn base_intervals(&self) -> Vec<&'static str> {
        let altered_fifth = self
            .alterations
            .iter()
            .find(|alteration| alteration.degree == 5)
            .and_then(|alteration| match alteration.semitones {
                -1 => Some("d5"),
                1 => Some("a5"),
                _ => None,
            });

        let fifth = altered_fifth.unwrap_or("P5");
        let has_seventh = self
            .extensions
            .iter()
            .any(|degree| matches!(degree, 7 | 9 | 11 | 13));

        let intervals = match self.quality {
            ChordQuality::Major => {
                if has_seventh {
                    vec![(1, "P1"), (3, "M3"), (5, fifth), (7, "M7")]
                } else {
                    vec![(1, "P1"), (3, "M3"), (5, fifth)]
                }
            }
            ChordQuality::Minor => {
                if has_seventh {
                    vec![(1, "P1"), (3, "m3"), (5, fifth), (7, "m7")]
                } else {
                    vec![(1, "P1"), (3, "m3"), (5, fifth)]
                }
            }
            ChordQuality::Dominant => vec![(1, "P1"), (3, "M3"), (5, fifth), (7, "m7")],
            ChordQuality::Diminished => {
                if has_seventh {
                    vec![(1, "P1"), (3, "m3"), (5, "d5"), (7, "d7")]
                } else {
                    vec![(1, "P1"), (3, "m3"), (5, "d5")]
                }
            }
            ChordQuality::Augmented => vec![(1, "P1"), (3, "M3"), (5, "a5")],
            ChordQuality::HalfDiminished => vec![(1, "P1"), (3, "m3"), (5, "d5"), (7, "m7")],
            ChordQuality::Suspended2 => vec![(1, "P1"), (2, "M2"), (5, fifth)],
            ChordQuality::Suspended4 => vec![(1, "P1"), (4, "P4"), (5, fifth)],
            ChordQuality::Power => vec![(1, "P1"), (5, fifth)],
        };

        intervals
            .into_iter()
            .filter_map(|(degree, interval)| {
                (!self.omissions.contains(&degree)).then_some(interval)
            })
            .collect()
    }
}

/// Returns ranked chord-symbol names for a chord.
///
/// This complements music21-style common names with compact symbolic spellings
/// such as `Cmaj7`, `F#m7b5`, or `D7b9#11/C`. Dense pitch-class sets
/// return no symbols when every candidate would overfit contradictory
/// extensions.
pub(crate) fn chord_symbol_spellings(chord: &Chord) -> Vec<String> {
    chord_symbol_spellings_for_root(chord, None)
}

pub(crate) fn chord_symbol_spellings_with_root(chord: &Chord, root: u8) -> Vec<String> {
    chord_symbol_spellings_for_root(chord, Some(root % 12))
}

fn chord_symbol_spellings_for_root(chord: &Chord, explicit_root: Option<u8>) -> Vec<String> {
    let pitches = chord.pitches();
    if pitches.iter().any(|pitch| {
        let ps = pitch.ps();
        (ps - ps.round()).abs() > FloatType::EPSILON
    }) {
        return Vec::new();
    }

    let pitch_classes = chord.pitch_classes();
    if pitch_classes.is_empty() {
        return Vec::new();
    }

    let pitch_class_set = pitch_classes.iter().copied().collect::<BTreeSet<_>>();
    let first_pitch_class = pitches.first().map(pitch_class);
    let bass_pitch = pitches.iter().min_by(|left, right| {
        left.ps()
            .partial_cmp(&right.ps())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let bass_pitch_class = bass_pitch.map(pitch_class);
    let bass_name = bass_pitch.map(|pitch| pitch.name().replace('-', "b"));
    let root_pitch_class = chord
        .root_pitch_name()
        .and_then(|name| Pitch::from_name(name).ok())
        .map(|pitch| pitch_class(&pitch));
    let root_candidates = if let Some(root) = explicit_root {
        if !pitch_class_set.contains(&root) {
            return Vec::new();
        }
        vec![root]
    } else {
        let mut root_candidates = Vec::new();

        if let Some(root) = root_pitch_class
            && pitch_class_set.contains(&root)
        {
            root_candidates.push(root);
        }
        if let Some(first) = first_pitch_class
            && pitch_class_set.contains(&first)
            && !root_candidates.contains(&first)
        {
            root_candidates.push(first);
        }
        for pitch_class in &pitch_classes {
            if !root_candidates.contains(pitch_class) {
                root_candidates.push(*pitch_class);
            }
        }

        root_candidates
    };

    let mut candidates = Vec::new();
    for root in root_candidates {
        let root_name = root_name_for_pitch_class(chord, root);
        let intervals = pitch_class_set
            .iter()
            .map(|pc| (*pc + 12 - root) % 12)
            .collect::<BTreeSet<_>>();
        if !intervals.contains(&0) {
            continue;
        }

        for analysis in symbol_analyses_for_intervals(&intervals) {
            let context = SymbolContext::from_intervals(&analysis.core_intervals);
            let additions = intervals
                .iter()
                .copied()
                .filter(|interval| !analysis.core_intervals.contains(interval))
                .filter_map(|interval| addition_label(interval, context))
                .collect::<Vec<_>>();

            if additions.len() != intervals.len() - analysis.core_intervals.len() {
                continue;
            }
            if is_overfit_dense_symbol(&intervals, &additions, &analysis) {
                continue;
            }

            let slash_bass = bass_name
                .as_deref()
                .filter(|_| explicit_root.is_none() && Some(root) != bass_pitch_class);
            let figure = symbol_figure(
                &root_name,
                &analysis.suffix,
                &analysis.omissions,
                &additions,
                slash_bass,
            );
            let root_bonus = if explicit_root.is_some() {
                0
            } else {
                usize::from(Some(root) != root_pitch_class) * 2
                    + usize::from(Some(root) != first_pitch_class)
            };
            let score = additions.len() * 16
                + analysis.omissions.len() * 8
                + analysis.alteration_count * 2
                + analysis.rank
                + root_bonus;
            candidates.push(SymbolCandidate { figure, score });
        }
    }

    candidates.sort_by(|left, right| {
        left.score
            .cmp(&right.score)
            .then_with(|| left.figure.len().cmp(&right.figure.len()))
            .then_with(|| left.figure.cmp(&right.figure))
    });

    let mut names = Vec::new();
    for candidate in candidates {
        if !names.contains(&candidate.figure) {
            names.push(candidate.figure);
        }
        if names.len() >= 16 {
            break;
        }
    }
    names
}

fn symbol_analyses_for_intervals(intervals: &BTreeSet<u8>) -> Vec<SymbolAnalysis> {
    let mut analyses = Vec::new();

    push_major_analyses(intervals, &mut analyses);
    push_dominant_analyses(intervals, &mut analyses);
    push_minor_analyses(intervals, &mut analyses);
    push_diminished_analyses(intervals, &mut analyses);
    push_half_diminished_analyses(intervals, &mut analyses);
    push_augmented_analyses(intervals, &mut analyses);
    push_suspended_analyses(intervals, &mut analyses);
    push_power_analysis(intervals, &mut analyses);
    push_root_analysis(intervals, &mut analyses);

    analyses
}

fn push_major_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if !intervals.contains(&4) || !intervals.contains(&7) {
        return;
    }

    if intervals.contains(&11) {
        let mut suffix = "maj7".to_string();
        let mut core = vec![0, 4, 7, 11];
        add_highest_natural_extension(intervals, &mut suffix, &mut core, "maj");
        push_analysis(analyses, suffix, &core, Vec::new(), 0, 0);
    } else if !intervals.contains(&10) {
        if intervals.contains(&9) {
            push_analysis(analyses, "6", &[0, 4, 7, 9], Vec::new(), 0, 1);
        } else {
            push_analysis(analyses, "", &[0, 4, 7], Vec::new(), 0, 0);
        }
    }
}

fn push_dominant_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if !intervals.contains(&4) || !intervals.contains(&10) {
        return;
    }

    let mut suffix = "7".to_string();
    let mut core = vec![0, 4, 10];
    let mut omissions = Vec::new();
    let mut alteration_count = 0;

    add_highest_natural_extension(intervals, &mut suffix, &mut core, "");

    if intervals.contains(&7) {
        core.push(7);
    } else if intervals.contains(&6) {
        core.push(6);
        suffix.push_str("b5");
        alteration_count += 1;
    } else if intervals.contains(&8) {
        core.push(8);
        suffix.push_str("#5");
        alteration_count += 1;
    } else {
        omissions.push(5);
    }

    alteration_count += add_dominant_alterations(intervals, &mut suffix, &mut core);

    push_analysis(analyses, suffix, &core, omissions, alteration_count, 0);
}

fn push_minor_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if !intervals.contains(&3) {
        return;
    }

    if intervals.contains(&10) {
        let mut suffix = "m7".to_string();
        let mut core = vec![0, 3, 10];
        let omissions = if intervals.contains(&7) {
            core.push(7);
            Vec::new()
        } else {
            vec![5]
        };
        add_highest_natural_extension(intervals, &mut suffix, &mut core, "m");
        push_analysis(analyses, suffix, &core, omissions, 0, 0);
    } else if intervals.contains(&11) && intervals.contains(&7) {
        push_analysis(analyses, "m(maj7)", &[0, 3, 7, 11], Vec::new(), 0, 1);
    } else if intervals.contains(&7) {
        if intervals.contains(&9) {
            push_analysis(analyses, "m6", &[0, 3, 7, 9], Vec::new(), 0, 1);
        } else {
            push_analysis(analyses, "m", &[0, 3, 7], Vec::new(), 0, 0);
        }
    }
}

fn push_diminished_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if !intervals.contains(&3) || !intervals.contains(&6) {
        return;
    }

    if intervals.contains(&9) {
        if intervals.contains(&2) {
            push_analysis(analyses, "dim9", &[0, 2, 3, 6, 9], Vec::new(), 0, 0);
        } else {
            push_analysis(analyses, "dim7", &[0, 3, 6, 9], Vec::new(), 0, 0);
        }
    } else {
        push_analysis(analyses, "dim", &[0, 3, 6], Vec::new(), 0, 0);
    }
}

fn push_half_diminished_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if !intervals.contains(&3) || !intervals.contains(&6) || !intervals.contains(&10) {
        return;
    }

    if intervals.contains(&2) {
        push_analysis(analyses, "m9b5", &[0, 2, 3, 6, 10], Vec::new(), 0, 1);
    } else {
        push_analysis(analyses, "m7b5", &[0, 3, 6, 10], Vec::new(), 0, 0);
    }
}

fn push_augmented_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if !intervals.contains(&4) || !intervals.contains(&8) {
        return;
    }

    if intervals.contains(&10) {
        push_analysis(analyses, "7#5", &[0, 4, 8, 10], Vec::new(), 1, 0);
    } else if intervals.contains(&11) {
        push_analysis(analyses, "maj7#5", &[0, 4, 8, 11], Vec::new(), 1, 0);
    } else {
        push_analysis(analyses, "aug", &[0, 4, 8], Vec::new(), 0, 0);
    }
}

fn push_suspended_analyses(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if intervals.contains(&3) || intervals.contains(&4) || !intervals.contains(&7) {
        return;
    }

    if intervals.contains(&2) {
        push_analysis(analyses, "sus2", &[0, 2, 7], Vec::new(), 0, 1);
    }
    if intervals.contains(&5) {
        push_analysis(analyses, "sus4", &[0, 5, 7], Vec::new(), 0, 1);
    }
}

fn push_power_analysis(intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    if intervals.contains(&7) {
        push_analysis(analyses, "5", &[0, 7], Vec::new(), 0, 3);
    }
}

fn push_root_analysis(_intervals: &BTreeSet<u8>, analyses: &mut Vec<SymbolAnalysis>) {
    push_analysis(analyses, "", &[0], Vec::new(), 0, 9);
}

fn push_analysis(
    analyses: &mut Vec<SymbolAnalysis>,
    suffix: impl Into<String>,
    core_intervals: &[u8],
    omissions: Vec<u8>,
    alteration_count: usize,
    rank: usize,
) {
    analyses.push(SymbolAnalysis {
        suffix: suffix.into(),
        core_intervals: core_intervals.iter().copied().collect(),
        omissions,
        alteration_count,
        rank,
    });
}

fn add_highest_natural_extension(
    intervals: &BTreeSet<u8>,
    suffix: &mut String,
    core: &mut Vec<u8>,
    family_prefix: &str,
) {
    if intervals.contains(&9) {
        *suffix = format!("{family_prefix}13");
        core.push(9);
    } else if intervals.contains(&5) {
        *suffix = format!("{family_prefix}11");
        core.push(5);
    } else if intervals.contains(&2) {
        *suffix = format!("{family_prefix}9");
        core.push(2);
    }
}

fn add_dominant_alterations(
    intervals: &BTreeSet<u8>,
    suffix: &mut String,
    core: &mut Vec<u8>,
) -> usize {
    let mut count = 0;
    for (interval, label) in [(1, "b9"), (3, "#9")] {
        if intervals.contains(&interval) {
            suffix.push_str(label);
            core.push(interval);
            count += 1;
        }
    }

    if intervals.contains(&7) && intervals.contains(&6) {
        suffix.push_str("#11");
        core.push(6);
        count += 1;
    }
    if intervals.contains(&7) && intervals.contains(&8) {
        suffix.push_str("b13");
        core.push(8);
        count += 1;
    }

    count
}

fn is_overfit_dense_symbol(
    intervals: &BTreeSet<u8>,
    additions: &[&'static str],
    analysis: &SymbolAnalysis,
) -> bool {
    intervals.len() >= 8
        && additions.len() + analysis.alteration_count + analysis.omissions.len() >= 3
}

fn symbol_figure(
    root_name: &str,
    suffix: &str,
    omissions: &[u8],
    additions: &[&'static str],
    bass_name: Option<&str>,
) -> String {
    let mut figure = format!("{root_name}{suffix}");
    if !omissions.is_empty() {
        figure.push_str("(no");
        figure.push_str(
            &omissions
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",no"),
        );
        figure.push(')');
    }
    if !additions.is_empty() {
        figure.push_str(" add(");
        figure.push_str(&additions.join(","));
        figure.push(')');
    }
    if let Some(bass_name) = bass_name {
        figure.push('/');
        figure.push_str(bass_name);
    }
    figure
}

impl SymbolContext {
    fn from_intervals(intervals: &BTreeSet<u8>) -> Self {
        Self {
            has_major_third: intervals.contains(&4),
            has_minor_third: intervals.contains(&3),
            has_perfect_fifth: intervals.contains(&7),
            has_flat_fifth: intervals.contains(&6),
        }
    }
}

fn addition_label(interval: u8, context: SymbolContext) -> Option<&'static str> {
    match interval {
        1 => Some("b9"),
        2 => Some("9"),
        3 if context.has_major_third => Some("#9"),
        3 if !context.has_minor_third => Some("b3"),
        4 if !context.has_major_third => Some("3"),
        5 => Some("11"),
        6 if context.has_perfect_fifth => Some("#11"),
        6 if !context.has_flat_fifth => Some("b5"),
        7 if !context.has_perfect_fifth => Some("5"),
        8 if context.has_perfect_fifth => Some("b13"),
        8 => Some("#5"),
        9 => Some("13"),
        10 => Some("b7"),
        11 => Some("7"),
        _ => None,
    }
}

fn root_name_for_pitch_class(chord: &Chord, target: u8) -> String {
    chord
        .pitches()
        .iter()
        .find(|pitch| pitch_class(pitch) == target)
        .map(|pitch| pitch.name())
        .unwrap_or_else(|| pitch_class_name(target).to_string())
        .replace('-', "b")
}

fn pitch_class(pitch: &Pitch) -> u8 {
    (pitch.ps().round() as IntegerType).rem_euclid(12) as u8
}

impl FromStr for ChordSymbol {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for ChordSymbol {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<String> for ChordSymbol {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(value)
    }
}

fn parse_quality(suffix: &str, alterations: &[ChordAlteration]) -> ChordQuality {
    let lower = suffix.to_ascii_lowercase();
    let has_flat_five = alterations
        .iter()
        .any(|alteration| alteration.degree == 5 && alteration.semitones == -1);

    if lower.contains("sus2") {
        ChordQuality::Suspended2
    } else if lower.contains("sus") {
        ChordQuality::Suspended4
    } else if lower.starts_with("maj") || suffix.starts_with('M') {
        ChordQuality::Major
    } else if lower.starts_with("min") || lower.starts_with('m') {
        if has_flat_five && lower.contains('7') {
            ChordQuality::HalfDiminished
        } else {
            ChordQuality::Minor
        }
    } else if lower.starts_with("dim") || lower.starts_with('o') {
        ChordQuality::Diminished
    } else if lower.starts_with("aug") || lower.starts_with('+') {
        ChordQuality::Augmented
    } else if lower.starts_with('5') {
        ChordQuality::Power
    } else if lower.starts_with('7')
        || lower.starts_with('9')
        || lower.starts_with("11")
        || lower.starts_with("13")
    {
        ChordQuality::Dominant
    } else {
        ChordQuality::Major
    }
}

fn parse_extensions(suffix: &str, alterations: &[ChordAlteration]) -> Vec<u8> {
    let mut extensions = Vec::new();
    let bytes = suffix.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if byte.is_ascii_digit()
            && idx
                .checked_sub(1)
                .is_none_or(|prev| !matches!(bytes[prev] as char, '#' | 'b' | '-'))
        {
            let start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if let Ok(degree) = suffix[start..idx].parse::<u8>()
                && matches!(degree, 6 | 7 | 9 | 11 | 13)
                && !extensions.contains(&degree)
            {
                extensions.push(degree);
            }
        } else {
            idx += 1;
        }
    }

    for alteration in alterations {
        if alteration.degree > 5 && !extensions.contains(&alteration.degree) {
            extensions.push(alteration.degree);
        }
    }

    extensions.sort_unstable();
    extensions
}

fn strip_addition_groups(suffix: &str) -> String {
    let lower = suffix.to_ascii_lowercase();
    let mut stripped = String::with_capacity(suffix.len());
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("add(") {
        let start = cursor + relative_start;
        let content_start = start + "add(".len();
        let Some(relative_end) = suffix[content_start..].find(')') else {
            break;
        };

        stripped.push_str(&suffix[cursor..start]);
        cursor = content_start + relative_end + 1;
    }

    stripped.push_str(&suffix[cursor..]);
    stripped
}

fn parse_additions(suffix: &str) -> Vec<ChordAlteration> {
    let lower = suffix.to_ascii_lowercase();
    let mut additions = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = lower[cursor..].find("add(") {
        let content_start = cursor + relative_start + "add(".len();
        let Some(relative_end) = suffix[content_start..].find(')') else {
            break;
        };
        let content_end = content_start + relative_end;

        for token in
            suffix[content_start..content_end].split(|ch: char| ch == ',' || ch.is_whitespace())
        {
            if let Some(addition) = parse_addition_token(token) {
                additions.push(addition);
            }
        }

        cursor = content_end + 1;
    }

    additions
}

fn parse_omissions(suffix: &str) -> Vec<u8> {
    let lower = suffix.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut omissions = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let marker_len = if bytes[cursor..].starts_with(b"omit") {
            4
        } else if bytes[cursor..].starts_with(b"no") {
            2
        } else {
            cursor += 1;
            continue;
        };

        cursor += marker_len;
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'(')
        {
            cursor += 1;
        }

        let degree_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if degree_start == cursor {
            continue;
        }

        if let Ok(degree) = std::str::from_utf8(&bytes[degree_start..cursor])
            .unwrap_or_default()
            .parse::<u8>()
            && !omissions.contains(&degree)
        {
            omissions.push(degree);
        }
    }

    omissions
}

fn parse_addition_token(token: &str) -> Option<ChordAlteration> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    let (semitones, degree) = match token.as_bytes()[0] as char {
        '#' => (1, &token[1..]),
        'b' | '-' => (-1, &token[1..]),
        _ => (0, token),
    };

    degree
        .parse::<u8>()
        .ok()
        .map(|degree| ChordAlteration::new(degree, semitones))
}

fn parse_alterations(suffix: &str) -> Vec<ChordAlteration> {
    let bytes = suffix.as_bytes();
    let mut alterations = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let semitones = match bytes[idx] as char {
            '#' => 1,
            'b' | '-' => -1,
            _ => {
                idx += 1;
                continue;
            }
        };
        idx += 1;
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if start == idx {
            continue;
        }
        if let Ok(degree) = suffix[start..idx].parse::<u8>() {
            alterations.push(ChordAlteration::new(degree, semitones));
        }
    }
    alterations
}

fn parse_pitch_only(value: &str) -> Result<Pitch> {
    let (name, rest) = parse_pitch_prefix(value)?;
    if !rest.is_empty() {
        return Err(Error::Chord(format!("invalid slash bass {value:?}")));
    }
    Pitch::from_name(name)
}

fn parse_pitch_prefix(value: &str) -> Result<(String, &str)> {
    let mut chars = value.char_indices();
    let Some((_, first)) = chars.next() else {
        return Err(Error::Chord("missing pitch name".to_string()));
    };

    if !matches!(first.to_ascii_uppercase(), 'A'..='G') {
        return Err(Error::Chord(format!("invalid pitch name in {value:?}")));
    }

    let mut end = first.len_utf8();
    let mut name = first.to_ascii_uppercase().to_string();
    for (idx, ch) in chars {
        match ch {
            '#' => {
                name.push('#');
                end = idx + ch.len_utf8();
            }
            'b' | '-' => {
                name.push('-');
                end = idx + ch.len_utf8();
            }
            _ => break,
        }
    }

    Ok((name, &value[end..]))
}

fn default_extension_interval(degree: u8) -> &'static str {
    match degree {
        6 => "M6",
        9 => "M9",
        11 => "P11",
        13 => "M13",
        _ => "P1",
    }
}

fn altered_interval(alteration: &ChordAlteration) -> Result<&'static str> {
    match (alteration.degree, alteration.semitones) {
        (5, -1) => Ok("d5"),
        (5, 1) => Ok("a5"),
        (9, -1) => Ok("m9"),
        (9, 1) => Ok("a9"),
        (11, 1) => Ok("a11"),
        (13, -1) => Ok("m13"),
        (13, 1) => Ok("a13"),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol alteration {alteration:?}"
        ))),
    }
}

fn added_interval(addition: &ChordAlteration) -> Result<&'static str> {
    let degree = match addition.degree {
        2 => 9,
        4 => 11,
        6 => 13,
        degree => degree,
    };

    match (degree, addition.semitones) {
        (3, -1) => Ok("m3"),
        (3, 0) => Ok("M3"),
        (3, 1) => Ok("a3"),
        (5, -1) => Ok("d5"),
        (5, 0) => Ok("P5"),
        (5, 1) => Ok("a5"),
        (7, -1) => Ok("m7"),
        (7, 0) => Ok("M7"),
        (9, -1) => Ok("m9"),
        (9, 0) => Ok("M9"),
        (9, 1) => Ok("a9"),
        (11, -1) => Ok("d11"),
        (11, 0) => Ok("P11"),
        (11, 1) => Ok("a11"),
        (13, -1) => Ok("m13"),
        (13, 0) => Ok("M13"),
        (13, 1) => Ok("a13"),
        _ => Err(Error::Chord(format!(
            "unsupported chord-symbol added tone {addition:?}"
        ))),
    }
}

fn interval_sort_key(name: &str) -> u8 {
    name.chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u8>()
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_major_seventh_symbol() {
        let symbol: ChordSymbol = "Cmaj7".parse().unwrap();
        assert_eq!(symbol.root().name(), "C");
        assert_eq!(symbol.quality(), ChordQuality::Major);
        assert_eq!(symbol.extensions(), &[7]);
        assert_eq!(
            symbol.to_chord().unwrap().pitched_common_name(),
            "C-major seventh chord"
        );
    }

    #[test]
    fn parses_half_diminished_symbol() {
        let symbol = ChordSymbol::parse("F#m7b5").unwrap();
        assert_eq!(symbol.root().name(), "F#");
        assert_eq!(symbol.quality(), ChordQuality::HalfDiminished);
        assert_eq!(
            symbol.to_chord().unwrap().pitched_common_name(),
            "F#-half-diminished seventh chord"
        );
    }

    #[test]
    fn parses_dominant_altered_symbol() {
        let symbol = ChordSymbol::parse("Bb7#11").unwrap();
        assert_eq!(symbol.root().name(), "B-");
        assert_eq!(symbol.quality(), ChordQuality::Dominant);
        assert_eq!(symbol.extensions(), &[7, 11]);
        assert_eq!(symbol.alterations()[0], ChordAlteration::new(11, 1));
        let names = symbol
            .to_chord()
            .unwrap()
            .pitches()
            .iter()
            .map(Pitch::name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["B-", "D", "F", "A-", "E"]);
    }

    #[test]
    fn parses_added_tones_without_changing_the_base_chord() {
        let symbol = ChordSymbol::parse("Cdim9 add(#5)").unwrap();
        assert_eq!(symbol.extensions(), &[9]);
        assert_eq!(symbol.additions(), &[ChordAlteration::new(5, 1)]);
        assert_eq!(
            symbol.to_chord().unwrap().pitch_classes(),
            vec![0, 2, 3, 6, 8, 9]
        );
    }

    #[test]
    fn parses_altered_dominant_with_slash_bass() {
        let symbol = ChordSymbol::parse("D7b9#11/C").unwrap();
        assert_eq!(symbol.root().name(), "D");
        assert_eq!(symbol.bass().map(Pitch::name).as_deref(), Some("C"));
        assert_eq!(symbol.quality(), ChordQuality::Dominant);
        assert_eq!(symbol.extensions(), &[7, 9, 11]);
        assert_eq!(
            symbol.alterations(),
            &[ChordAlteration::new(9, -1), ChordAlteration::new(11, 1)]
        );
        assert_eq!(
            symbol.to_chord().unwrap().pitch_classes(),
            vec![0, 2, 3, 6, 8, 9]
        );
    }

    #[test]
    fn generates_petrushka_chord_symbol_name() {
        let chord = Chord::new("C4 D4 Eb4 F#4 Ab4 A4").unwrap();
        let names = chord_symbol_spellings(&chord);

        assert_eq!(names.first().map(String::as_str), Some("D7b9#11/C"));
        assert!(names.iter().any(|name| name == "D7b9#11/C"));
    }

    #[test]
    fn generates_common_chord_symbols() {
        let major_seventh = Chord::new("C E G B").unwrap();
        let dominant_ninth = Chord::new("C E G B- D").unwrap();

        assert_eq!(
            chord_symbol_spellings(&major_seventh)
                .first()
                .map(String::as_str),
            Some("Cmaj7")
        );
        assert_eq!(
            chord_symbol_spellings(&dominant_ninth)
                .first()
                .map(String::as_str),
            Some("C9")
        );
    }

    #[test]
    fn generated_symbols_use_inferred_root_and_slash_bass() {
        let chord = Chord::new("F4 C5 D5 E-5").unwrap();
        let names = chord_symbol_spellings(&chord);

        assert_eq!(
            names.first().map(String::as_str),
            Some("Dm7(no5) add(b9)/F")
        );
        assert_eq!(
            ChordSymbol::parse("Dm7(no5) add(b9)/F")
                .unwrap()
                .to_chord()
                .unwrap()
                .pitches()
                .first()
                .map(Pitch::name)
                .as_deref(),
            Some("F")
        );
        assert_eq!(
            ChordSymbol::parse("Dm7(no5) add(b9)/F")
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_classes()
                .into_iter()
                .collect::<BTreeSet<_>>(),
            [0, 2, 3, 5].into_iter().collect()
        );
    }

    #[test]
    fn generates_chord_symbol_names_with_explicit_root() {
        let major_sixth = Chord::new("C4 A4").unwrap();
        let tritone = Chord::new("C4 F#4").unwrap();
        let major_second = Chord::new("C4 D4").unwrap();
        let incomplete_dominant = Chord::new("C4 E-4 F4").unwrap();

        assert_eq!(
            chord_symbol_spellings_with_root(&major_sixth, 0)
                .first()
                .map(String::as_str),
            Some("C add(13)")
        );
        assert_eq!(
            chord_symbol_spellings_with_root(&tritone, 0)
                .first()
                .map(String::as_str),
            Some("C add(b5)")
        );
        assert_eq!(
            chord_symbol_spellings_with_root(&major_second, 0)
                .first()
                .map(String::as_str),
            Some("C add(9)")
        );
        assert_eq!(
            chord_symbol_spellings_with_root(&incomplete_dominant, 0)
                .first()
                .map(String::as_str),
            Some("C add(b3,11)")
        );
    }

    #[test]
    fn dense_non_tertian_sets_do_not_overfit_chord_symbols() {
        let chord = Chord::new("C4 D-4 E-4 E4 F#4 G4 A-4 A4").unwrap();

        assert!(chord_symbol_spellings(&chord).is_empty());
    }
}
