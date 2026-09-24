//! One bass note and its figures, and every way of voicing them: music21's
//! `figuredBass.segment`.
//!
//! A [`Segment`] knows the notes its bass and figures stand for, and every
//! pitch at or above the bass that spells one of them. A possibility is one
//! pitch from those for each upper part and the bass itself for the lowest;
//! the segment lists every possibility, keeps those its rules accept, and --
//! given the segment that follows -- every pair of possibilities that may
//! move from one to the other, resolving a dominant seventh, a diminished
//! seventh or an augmented sixth by the rules those chords resolve by.

use crate::chord::Chord;
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::figuredbass::Notation;
use crate::figuredbass::possibility::{self, ItalianSixth};
use crate::figuredbass::resolution::{self, AugmentedSixth, ChordInfo};
use crate::figuredbass::rules::Rules;
use crate::figuredbass::scale::FiguredBassScale;
use crate::interval::Interval;
use crate::pitch::Pitch;
use crate::scale::{Scale, ScaleType};

/// A voicing: a pitch for each part, highest part first, the bass last.
pub type Possibility = Vec<Pitch>;

/// A voicing in one segment beside a voicing in the next.
pub type PossibilityPair = (Possibility, Possibility);

/// The pairs of voicings two segments may move between, and why they were
/// found as they were.
#[derive(Clone, Debug, PartialEq)]
pub struct Consecutive {
    /// Every pair, in the order music21 lists them.
    pub pairs: Vec<PossibilityPair>,
    /// Where a chord that resolves by a rule of its own found no rule to
    /// resolve to the next chord by, and the pairs were found as between any
    /// two chords: music21's warning, word for word.
    pub fallback: Option<String>,
}

impl Consecutive {
    fn resolved(pairs: Vec<PossibilityPair>) -> Self {
        Self {
            pairs,
            fallback: None,
        }
    }
}

/// A bass note, its figures, and the voicings they allow.
#[derive(Clone, Debug)]
#[must_use]
pub struct Segment {
    bass: Pitch,
    quarter_length: FloatType,
    num_parts: usize,
    max_pitch: Pitch,
    pitch_names: Vec<String>,
    pitches_above_bass: Vec<Pitch>,
    chord: Chord,
    overlaid: bool,
    /// The rules its voicings are held to.
    pub rules: Rules,
}

impl Segment {
    /// The segment of `bass` under `notation`, read in `scale`, voiced in
    /// `num_parts` parts no higher than `max_pitch`: music21's `Segment`.
    ///
    /// # Errors
    ///
    /// A bass or figures the scale cannot read, or a maximum pitch with no
    /// octave.
    pub fn new(
        bass: Pitch,
        quarter_length: FloatType,
        notation: &Notation,
        scale: &FiguredBassScale,
        rules: Rules,
        num_parts: usize,
        max_pitch: Pitch,
    ) -> Result<Self> {
        let names = scale.pitch_names(&bass, notation)?;
        Self::from_pitch_names(bass, quarter_length, names, rules, num_parts, max_pitch)
    }

    /// The segment of `bass` spelling the notes named `pitch_names`, as
    /// music21 builds one given `listOfPitches` rather than figures.
    ///
    /// # Errors
    ///
    /// A pitch name that is none, or a maximum pitch with no octave.
    pub fn from_pitch_names(
        bass: Pitch,
        quarter_length: FloatType,
        pitch_names: Vec<String>,
        rules: Rules,
        num_parts: usize,
        max_pitch: Pitch,
    ) -> Result<Self> {
        let pitches_above_bass = pitches(&pitch_names, &bass, &max_pitch)?;
        let chord = Chord::new(pitches_above_bass.as_slice())?;
        Ok(Self {
            bass,
            quarter_length,
            num_parts,
            max_pitch,
            pitch_names,
            pitches_above_bass,
            chord,
            overlaid: false,
            rules,
        })
    }

    /// The same segment, with the parts its rules hold to a pitch sung at
    /// that pitch in every voicing: music21's `OverlaidSegment`.
    pub fn overlaid(mut self) -> Self {
        self.overlaid = true;
        self
    }

    /// The bass.
    pub fn bass(&self) -> &Pitch {
        &self.bass
    }

    /// How long the bass lasts, in quarter notes.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }

    /// How many parts sing it, the bass among them.
    pub fn num_parts(&self) -> usize {
        self.num_parts
    }

    /// The highest pitch any part may sing.
    pub fn max_pitch(&self) -> &Pitch {
        &self.max_pitch
    }

    /// The names of the notes the bass and figures stand for, the bass's
    /// first: music21's `pitchNamesInChord`.
    pub fn pitch_names(&self) -> &[String] {
        &self.pitch_names
    }

    /// Every pitch from the bass up to the highest allowed that spells one
    /// of those notes, lowest first: music21's `allPitchesAboveBass`.
    pub fn pitches_above_bass(&self) -> &[Pitch] {
        &self.pitches_above_bass
    }

    /// Those pitches as one chord: music21's `segmentChord`.
    pub fn chord(&self) -> &Chord {
        &self.chord
    }

    /// The pitches each part may sing, highest part first: the pitches
    /// above the bass for each upper part, and the bass for the lowest, with
    /// the parts an overlaid segment holds to a pitch held to it.
    fn choices(&self) -> Result<Vec<Vec<Pitch>>> {
        let upper = self.num_parts.saturating_sub(1);
        let mut choices: Vec<Vec<Pitch>> = vec![self.pitches_above_bass.clone(); upper];
        choices.push(vec![Pitch::from_name(self.bass.name_with_octave())?]);
        if self.overlaid {
            for (part, pitch) in &self.rules.part_pitch_limits {
                let slot = part.checked_sub(1).and_then(|index| choices.get_mut(index));
                let Some(slot) = slot else {
                    return Err(Error::FiguredBass(format!(
                        "a segment of {} parts has no part {part}",
                        self.num_parts
                    )));
                };
                *slot = vec![Pitch::from_name(pitch.name_with_octave())?];
            }
        }
        Ok(choices)
    }

    /// Every voicing, acceptable or not, in music21's order: the highest
    /// part changes slowest. music21's `allSinglePossibilities`.
    pub fn all_single_possibilities(&self) -> Result<Vec<Possibility>> {
        Ok(product(&self.choices()?))
    }

    /// The voicings its rules accept: music21's
    /// `allCorrectSinglePossibilities`.
    ///
    /// The voicings are walked as indices into what each part may sing, and
    /// held to the rules over each pitch's place and name worked out once, so
    /// only the voicings kept are ever built: a seventh chord has thousands
    /// to look at and a handful to keep.
    pub fn all_correct_single_possibilities(&self) -> Result<Vec<Possibility>> {
        let choices = self.choices()?;
        if choices.iter().any(Vec::is_empty) {
            return Ok(Vec::new());
        }
        let rules = &self.rules;
        // Which of the chord's notes each pitch spells, as bits: a voicing is
        // complete when its pitches' bits cover every note.
        let wanted: Option<u64> = if rules.forbid_incomplete_possibilities {
            if self.pitch_names.len() > 64 {
                return Ok(product(&choices)
                    .into_iter()
                    .filter(|voicing| self.is_correct_single(voicing))
                    .collect());
            }
            Some(
                self.pitch_names
                    .iter()
                    .enumerate()
                    .fold(0, |bits, (index, _)| bits | 1 << index),
            )
        } else {
            None
        };
        let spaces: Vec<Vec<FloatType>> = choices
            .iter()
            .map(|part| part.iter().map(Pitch::ps).collect())
            .collect();
        let spelled: Vec<Vec<u64>> = choices
            .iter()
            .map(|part| {
                part.iter()
                    .map(|pitch| {
                        self.pitch_names
                            .iter()
                            .enumerate()
                            .filter(|(_, name)| pitch.is_named(name))
                            .fold(0, |bits, (index, _)| bits | 1 << index)
                    })
                    .collect()
            })
            .collect();
        let limit = rules
            .upper_parts_max_semitone_separation
            .map(FloatType::from);
        let upper = choices.len() - 1;
        let mut indices = vec![0; choices.len()];
        let mut places = vec![0.0; choices.len()];
        let mut kept = Vec::new();
        loop {
            for (part, &index) in indices.iter().enumerate() {
                places[part] = spaces[part][index];
            }
            let complete = wanted.is_none_or(|wanted| {
                indices
                    .iter()
                    .enumerate()
                    .fold(0, |bits, (part, &index)| bits | spelled[part][index])
                    & wanted
                    == wanted
            });
            let within = limit.is_none_or(|limit| {
                (0..upper).all(|high| {
                    (high + 1..upper).all(|low| (places[high] - places[low]).abs() <= limit)
                })
            });
            let crossing = rules.forbid_voice_crossing
                && (0..places.len())
                    .any(|high| (high + 1..places.len()).any(|low| places[high] < places[low]));
            if complete && within && !crossing {
                kept.push(
                    indices
                        .iter()
                        .enumerate()
                        .map(|(part, &index)| choices[part][index].clone())
                        .collect(),
                );
            }
            let mut position = choices.len();
            loop {
                if position == 0 {
                    return Ok(kept);
                }
                position -= 1;
                indices[position] += 1;
                if indices[position] < choices[position].len() {
                    break;
                }
                indices[position] = 0;
            }
        }
    }

    /// Whether the rules accept `voicing` on its own.
    fn is_correct_single(&self, voicing: &[Pitch]) -> bool {
        let rules = &self.rules;
        !(rules.forbid_incomplete_possibilities
            && possibility::is_incomplete(voicing, &self.pitch_names))
            && possibility::upper_parts_within_limit(
                voicing,
                rules.upper_parts_max_semitone_separation,
            )
            && !(rules.forbid_voice_crossing && possibility::voice_crossing(voicing))
    }

    /// Whether the rules accept moving from `from` to `to`, a voicing of the
    /// segment after this one.
    fn is_correct_consecutive(
        &self,
        from: &[Pitch],
        to: &[Pitch],
        italian: Option<&ItalianSixth>,
    ) -> Result<bool> {
        let rules = &self.rules;
        Ok(
            possibility::parts_same(from, to, Some(&rules.parts_to_check))?
                && !(rules.upper_parts_remain_same && !possibility::upper_parts_same(from, to)?)
                && !(rules.forbid_voice_overlap && possibility::voice_overlap(from, to)?)
                && possibility::part_movements_within_limits(
                    from,
                    to,
                    &rules.part_movement_limits,
                )?
                && !(rules.forbid_parallel_fifths && possibility::parallel_fifths(from, to)?)
                && !(rules.forbid_parallel_octaves && possibility::parallel_octaves(from, to)?)
                && !(rules.forbid_hidden_fifths && possibility::hidden_fifths(from, to)?)
                && !(rules.forbid_hidden_octaves && possibility::hidden_octaves(from, to)?)
                && match italian {
                    Some(sixth) => possibility::could_be_italian_a6_resolution(
                        from,
                        to,
                        Some(sixth),
                        rules.restrict_doublings_in_italian_a6_resolution,
                    )?,
                    None => true,
                },
        )
    }

    /// The Italian sixth an Italian-sixth resolution is judged by, where the
    /// rules resolve augmented sixths and this chord is one.
    fn italian_sixth(&self) -> Result<Option<ItalianSixth>> {
        if self.rules.resolve_augmented_sixth_properly
            && self.chord.is_italian_augmented_sixth(false, false)
        {
            Ok(Some(ItalianSixth::from_chord(&self.chord)?))
        } else {
            Ok(None)
        }
    }

    /// Every pair of voicings `next` may follow this segment by, resolving
    /// this chord by its own rule where the rules say to and it is a chord
    /// with one: music21's `allCorrectConsecutivePossibilities`.
    ///
    /// `next` is taken mutably because music21 lets a dominant seventh
    /// resolving to its tonic leave that tonic incomplete, which it does by
    /// switching the tonic segment's rule off.
    ///
    /// # Errors
    ///
    /// Two segments of different numbers of parts or different highest
    /// pitches, or a resolution that fails.
    pub fn all_correct_consecutive_possibilities(&self, next: &mut Segment) -> Result<Consecutive> {
        if self.num_parts != next.num_parts {
            return Err(Error::FiguredBass(
                "Two segments with unequal numParts cannot be compared.".to_string(),
            ));
        }
        if self.max_pitch != next.max_pitch {
            return Err(Error::FiguredBass(
                "Two segments with unequal maxPitch cannot be compared.".to_string(),
            ));
        }
        let rules = &self.rules;
        if rules.resolve_dominant_seventh_properly && self.chord.is_dominant_seventh() {
            return self.resolve_dominant_seventh_segment(next);
        }
        if rules.resolve_diminished_seventh_properly && self.chord.is_diminished_seventh() {
            return self.resolve_diminished_seventh_segment(next, rules.doubled_root_in_dim7);
        }
        if rules.resolve_augmented_sixth_properly && self.chord.is_augmented_sixth(false) {
            return self.resolve_augmented_sixth_segment(next);
        }
        Ok(Consecutive::resolved(self.resolve_ordinary(next)?))
    }

    /// The pairs between two segments where no chord needs a resolution of
    /// its own: every acceptable voicing of this beside every acceptable
    /// voicing of `next`, kept where the rules accept the move.
    fn resolve_ordinary(&self, next: &Segment) -> Result<Vec<PossibilityPair>> {
        let italian = self.italian_sixth()?;
        let from = self.all_correct_single_possibilities()?;
        let to = next.all_correct_single_possibilities()?;
        let mut pairs = Vec::new();
        for a in &from {
            for b in &to {
                if self.is_correct_consecutive(a, b, italian.as_ref())? {
                    pairs.push((a.clone(), b.clone()));
                }
            }
        }
        Ok(pairs)
    }

    /// Resolves each acceptable voicing by `resolve`, keeping the
    /// resolutions no part of which sings above `next`'s highest pitch, and
    /// holding them to the rules the rules say to.
    fn resolve_special(
        &self,
        next: &Segment,
        resolve: impl Fn(&[Pitch]) -> Result<Vec<Pitch>>,
    ) -> Result<Vec<PossibilityPair>> {
        let italian = self.italian_sixth()?;
        let mut pairs = Vec::new();
        for voicing in self.all_correct_single_possibilities()? {
            let resolved = resolve(&voicing)?;
            if !possibility::pitches_within_limit(&resolved, &next.max_pitch) {
                continue;
            }
            if self.rules.apply_consecutive_possib_rules_to_resolution
                && !self.is_correct_consecutive(&voicing, &resolved, italian.as_ref())?
            {
                continue;
            }
            if self.rules.apply_single_possib_rules_to_resolution
                && !next.is_correct_single(&resolved)
            {
                continue;
            }
            pairs.push((voicing, resolved));
        }
        Ok(pairs)
    }

    /// The first special resolution whose condition holds, or the ordinary
    /// one with `warning` where none does.
    fn resolve_first(
        &self,
        next: &Segment,
        candidates: Vec<(bool, SpecialResolution<'_>)>,
        warning: &str,
    ) -> Result<Consecutive> {
        match candidates.into_iter().find(|(applies, _)| *applies) {
            Some((_, resolve)) => Ok(Consecutive::resolved(self.resolve_special(next, resolve)?)),
            None => Ok(Consecutive {
                pairs: self.resolve_ordinary(next)?,
                fallback: Some(warning.to_string()),
            }),
        }
    }

    /// Resolves a dominant seventh to `next`: to the tonic, major or minor,
    /// and from root position also to either submediant or subdominant.
    /// music21's `resolveDominantSeventhSegment`.
    ///
    /// # Errors
    ///
    /// A segment whose chord is no dominant seventh.
    pub fn resolve_dominant_seventh_segment(&self, next: &mut Segment) -> Result<Consecutive> {
        let dominant = &self.chord;
        if !dominant.is_dominant_seventh() {
            return Err(Error::FiguredBass(
                "Dominant seventh resolution: Not a dominant seventh Segment.".to_string(),
            ));
        }
        let info = ChordInfo::of(dominant);
        let dominant_scale = ScaleType::Major.derive(dominant.pitches().as_slice())?;
        let minor_scale = dominant_scale.parallel_minor();
        let tonic = dominant_scale.tonic().name();
        let subdominant = dominant_scale.pitch_at_degree(4)?.name();
        let major_submediant = dominant_scale.pitch_at_degree(6)?.name();
        let minor_submediant = minor_scale.pitch_at_degree(6)?.name();

        let resolving = next.chord.clone();
        let second_inversion = dominant.inversion() == Some(2);
        let v43_to_i6 = second_inversion && resolving.inversion() == Some(1);
        let root = resolving.root().map(Pitch::name).unwrap_or_default();
        let major = resolving.is_major_triad();
        let minor = resolving.is_minor_triad();
        if dominant.inversion() == Some(0) && root == tonic && (major || minor) {
            next.rules.forbid_incomplete_possibilities = false;
        }
        let info = &info;
        let candidates: Vec<(bool, SpecialResolution<'_>)> = vec![
            (
                root == tonic && major,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::dominant_seventh_to_major_tonic(voicing, v43_to_i6, Some(info))
                }),
            ),
            (
                root == tonic && minor,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::dominant_seventh_to_minor_tonic(voicing, v43_to_i6, Some(info))
                }),
            ),
            (
                root == major_submediant && minor && !second_inversion,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::dominant_seventh_to_minor_submediant(voicing, Some(info))
                }),
            ),
            (
                root == minor_submediant && major && !second_inversion,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::dominant_seventh_to_major_submediant(voicing, Some(info))
                }),
            ),
            (
                root == subdominant && major && !second_inversion,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::dominant_seventh_to_major_subdominant(voicing, Some(info))
                }),
            ),
            (
                root == subdominant && minor && !second_inversion,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::dominant_seventh_to_minor_subdominant(voicing, Some(info))
                }),
            ),
        ];
        self.resolve_first(
            next,
            candidates,
            "Dominant seventh resolution: No proper resolution available. Executing ordinary resolution.",
        )
    }

    /// Resolves a diminished seventh to `next`: to the tonic, major or minor,
    /// or to the subdominant. With `doubled_root`, a resolution to the tonic
    /// doubles its root, unless the inversions decide it.
    /// music21's `resolveDiminishedSeventhSegment`.
    ///
    /// # Errors
    ///
    /// A segment whose chord is no fully diminished seventh.
    pub fn resolve_diminished_seventh_segment(
        &self,
        next: &Segment,
        doubled_root: bool,
    ) -> Result<Consecutive> {
        let diminished = &self.chord;
        if !diminished.is_diminished_seventh() {
            return Err(Error::FiguredBass(
                "Diminished seventh resolution: Not a diminished seventh Segment.".to_string(),
            ));
        }
        let info = ChordInfo::of(diminished);
        let root = diminished
            .root()
            .ok_or_else(|| Error::FiguredBass("a diminished seventh with no root".to_string()))?;
        let placeholder = Pitch::from_name("C")?;
        let scale = Scale::new(ScaleType::HarmonicMinor, placeholder).derive_by_degree(7, root)?;
        let tonic = scale.tonic().name();
        let subdominant = scale.pitch_at_degree(4)?.name();

        let resolving = &next.chord;
        let mut doubled_root = doubled_root;
        if diminished.inversion() == Some(1) {
            match resolving.inversion() {
                Some(0) => doubled_root = true,
                Some(1) => doubled_root = false,
                _ => {}
            }
        }
        let resolving_root = resolving.root().map(Pitch::name).unwrap_or_default();
        let major = resolving.is_major_triad();
        let minor = resolving.is_minor_triad();
        let info = &info;
        let candidates: Vec<(bool, SpecialResolution<'_>)> = vec![
            (
                resolving_root == tonic && major,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::diminished_seventh_to_major_tonic(voicing, doubled_root, Some(info))
                }),
            ),
            (
                resolving_root == tonic && minor,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::diminished_seventh_to_minor_tonic(voicing, doubled_root, Some(info))
                }),
            ),
            (
                resolving_root == subdominant && major,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::diminished_seventh_to_major_subdominant(voicing, Some(info))
                }),
            ),
            (
                resolving_root == subdominant && minor,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::diminished_seventh_to_minor_subdominant(voicing, Some(info))
                }),
            ),
        ];
        self.resolve_first(
            next,
            candidates,
            "Diminished seventh resolution: No proper resolution available. Executing ordinary resolution.",
        )
    }

    /// Resolves an augmented sixth to `next`: to the tonic in second
    /// inversion, major or minor, or to the dominant. An Italian sixth, and
    /// a kind music21 has no rule for, resolve as any chord does.
    /// music21's `resolveAugmentedSixthSegment`.
    ///
    /// # Errors
    ///
    /// A segment whose chord is no augmented sixth.
    pub fn resolve_augmented_sixth_segment(&self, next: &Segment) -> Result<Consecutive> {
        let sixth = &self.chord;
        if !sixth.is_augmented_sixth(false) {
            return Err(Error::FiguredBass(
                "Augmented sixth resolution: Not an augmented sixth Segment.".to_string(),
            ));
        }
        if sixth.is_italian_augmented_sixth(false, false) {
            return Ok(Consecutive::resolved(self.resolve_ordinary(next)?));
        }
        let kind = if sixth.is_french_augmented_sixth(false) {
            AugmentedSixth::French
        } else if sixth.is_german_augmented_sixth(false) {
            AugmentedSixth::German
        } else if sixth.is_swiss_augmented_sixth(false) {
            AugmentedSixth::Swiss
        } else {
            return Ok(Consecutive {
                pairs: self.resolve_ordinary(next)?,
                fallback: Some(
                    "Augmented sixth resolution: Augmented sixth type not supported. Executing ordinary resolution."
                        .to_string(),
                ),
            });
        };
        let bass = sixth
            .bass()
            .ok_or_else(|| Error::FiguredBass("an augmented sixth with no bass".to_string()))?;
        let tonic = bass.transpose(&Interval::from_name("M3")?)?;
        let dominant = Scale::new(ScaleType::Major, tonic.clone())
            .pitch_at_degree(5)?
            .name();
        let tonic = tonic.name();
        let info = ChordInfo::of(sixth);

        let resolving = &next.chord;
        let root = resolving.root().map(Pitch::name).unwrap_or_default();
        let resolving_bass = resolving.bass().map(Pitch::name).unwrap_or_default();
        let second_inversion = resolving.inversion() == Some(2);
        let major = resolving.is_major_triad();
        let minor = resolving.is_minor_triad();
        let info = &info;
        let candidates: Vec<(bool, SpecialResolution<'_>)> = vec![
            (
                second_inversion && root == tonic && major,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::augmented_sixth_to_major_tonic(voicing, Some(kind), Some(info))
                }),
            ),
            (
                second_inversion && root == tonic && minor,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::augmented_sixth_to_minor_tonic(voicing, Some(kind), Some(info))
                }),
            ),
            (
                dominant == resolving_bass && major,
                Box::new(move |voicing: &[Pitch]| {
                    resolution::augmented_sixth_to_dominant(voicing, Some(kind), Some(info))
                }),
            ),
        ];
        self.resolve_first(
            next,
            candidates,
            "Augmented sixth resolution: No proper resolution available. Executing ordinary resolution.",
        )
    }
}

/// One way of resolving a voicing.
type SpecialResolution<'a> = Box<dyn Fn(&[Pitch]) -> Result<Vec<Pitch>> + 'a>;

/// Every choice of one item from each list, the first list changing
/// slowest, as `itertools.product` gives them.
fn product(choices: &[Vec<Pitch>]) -> Vec<Possibility> {
    if choices.iter().any(Vec::is_empty) {
        return Vec::new();
    }
    let mut indices = vec![0; choices.len()];
    let mut all = Vec::new();
    loop {
        all.push(
            indices
                .iter()
                .zip(choices)
                .map(|(&index, choice)| choice[index].clone())
                .collect(),
        );
        let mut position = choices.len();
        loop {
            if position == 0 {
                return all;
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < choices[position].len() {
                break;
            }
            indices[position] = 0;
        }
    }
}

/// Every pitch named in `pitch_names`, in every octave from `bass` up to
/// `max_pitch`, both included, lowest first: music21's `segment.getPitches`.
///
/// # Errors
///
/// A maximum pitch with no octave, or a name that is no pitch.
pub fn pitches<S: AsRef<str>>(
    pitch_names: &[S],
    bass: &Pitch,
    max_pitch: &Pitch,
) -> Result<Vec<Pitch>> {
    let Some(top_octave) = max_pitch.octave() else {
        return Err(Error::Value(
            "maxPitch must not have an implicit octave".to_string(),
        ));
    };
    let mut found = Vec::new();
    for name in pitch_names {
        for octave in 0..=top_octave {
            let pitch = Pitch::from_name(format!("{}{octave}", name.as_ref()))?;
            if pitch.ps() >= bass.ps() && pitch.ps() <= max_pitch.ps() {
                found.push(pitch);
            }
        }
    }
    found.sort_by(|a, b| a.ps().total_cmp(&b.ps()));
    Ok(found)
}
