//! The rules a possibility is held to: music21's `figuredBass.possibility`.
//!
//! A possibility is one way of voicing a figured-bass chord: a pitch for each
//! part, from the highest part down to the bass, which is last. Unlike a
//! chord's notes, the order says which part sings which pitch, and parts may
//! cross, so the highest pitch need not be first.
//!
//! The rules come in two kinds. Those over one possibility say whether a
//! voicing is acceptable on its own; those over two say whether moving from
//! one voicing to the next is.

use std::sync::LazyLock;

use crate::chord::Chord;
use crate::defaults::{FloatType, IntegerType};
use crate::error::{Error, Result};
use crate::interval::Interval;
use crate::pitch::Pitch;
use crate::voiceleading::VoiceLeadingQuartet;

/// The highest pitch a part may reach unless told otherwise: `B5`.
pub static DEFAULT_MAX_PITCH: LazyLock<Pitch> =
    LazyLock::new(|| Pitch::from_name("B5").expect("B5 is a pitch name"));

/// Whether any part sings higher than a part above it.
///
/// ```
/// use music21_rs::Pitch;
/// use music21_rs::figuredbass::possibility::voice_crossing;
///
/// let pitches = |names: &[&str]| -> Vec<Pitch> {
///     names.iter().map(|name| Pitch::from_name(*name).unwrap()).collect()
/// };
/// assert!(!voice_crossing(&pitches(&["G5", "C5", "E4", "C4"])));
/// assert!(voice_crossing(&pitches(&["C5", "G5", "E4", "C4"])));
/// ```
pub fn voice_crossing(possibility: &[Pitch]) -> bool {
    possibility.iter().enumerate().any(|(index, higher)| {
        possibility[index + 1..]
            .iter()
            .any(|lower| higher.ps() < lower.ps())
    })
}

/// Whether the possibility leaves out any of `pitch_names`, the names of
/// the notes its chord has to sound.
pub fn is_incomplete<S: AsRef<str>>(possibility: &[Pitch], pitch_names: &[S]) -> bool {
    let contained: Vec<String> = possibility.iter().map(Pitch::name).collect();
    pitch_names
        .iter()
        .any(|name| !contained.iter().any(|held| held == name.as_ref()))
}

/// Whether every two of the upper parts -- all but the bass -- lie within
/// `max_semitone_separation` of each other; with no limit, they always do.
/// music21's default is twelve, the span a hand can play.
pub fn upper_parts_within_limit(
    possibility: &[Pitch],
    max_semitone_separation: Option<IntegerType>,
) -> bool {
    let Some(limit) = max_semitone_separation else {
        return true;
    };
    let upper = &possibility[..possibility.len().saturating_sub(1)];
    upper.iter().enumerate().all(|(index, higher)| {
        upper[index + 1..]
            .iter()
            .all(|lower| (higher.ps() - lower.ps()).abs() <= FloatType::from(limit))
    })
}

/// Whether no part sings above `max_pitch`, which is
/// [`DEFAULT_MAX_PITCH`] unless a caller says otherwise.
pub fn pitches_within_limit(possibility: &[Pitch], max_pitch: &Pitch) -> bool {
    possibility.iter().all(|pitch| pitch.ps() <= max_pitch.ps())
}

/// Whether each part named in `part_pitch_limits` sings exactly the pitch
/// it is held to. Parts are numbered from one, the highest.
///
/// # Errors
///
/// A part number the possibility has no part for.
pub fn limit_part_to_pitch(
    possibility: &[Pitch],
    part_pitch_limits: &[(usize, Pitch)],
) -> Result<bool> {
    for (part, pitch) in part_pitch_limits {
        if possibility_part(possibility, *part)? != pitch {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The pitch part `part` sings, counting the highest part as one.
fn possibility_part(possibility: &[Pitch], part: usize) -> Result<&Pitch> {
    part.checked_sub(1)
        .and_then(|index| possibility.get(index))
        .ok_or_else(|| {
            Error::FiguredBass(format!(
                "a possibility of {} parts has no part {part}",
                possibility.len()
            ))
        })
}

/// Whether the pitch-space distance between `a` and `b` is a whole number of
/// `semitones` past some octaves.
fn apart_by(a: &Pitch, b: &Pitch, semitones: FloatType) -> bool {
    (a.ps() - b.ps()).abs() % 12.0 == semitones
}

/// Whether any two parts move in parallel fifths from `a` to `b`.
///
/// # Errors
///
/// Two possibilities with different numbers of parts.
pub fn parallel_fifths(a: &[Pitch], b: &[Pitch]) -> Result<bool> {
    parallel(a, b, 7.0, VoiceLeadingQuartet::parallel_fifth)
}

/// Whether any two parts move in parallel octaves from `a` to `b`.
///
/// # Errors
///
/// Two possibilities with different numbers of parts.
pub fn parallel_octaves(a: &[Pitch], b: &[Pitch]) -> Result<bool> {
    parallel(a, b, 0.0, VoiceLeadingQuartet::parallel_octave)
}

fn parallel(
    a: &[Pitch],
    b: &[Pitch],
    semitones: FloatType,
    rule: fn(&VoiceLeadingQuartet) -> bool,
) -> Result<bool> {
    let pairs = part_pairs(a, b)?;
    for (index, (higher_a, higher_b)) in pairs.iter().enumerate() {
        for (lower_a, lower_b) in &pairs[index + 1..] {
            if !apart_by(higher_a, lower_a, semitones) || !apart_by(higher_b, lower_b, semitones) {
                continue;
            }
            let quartet = VoiceLeadingQuartet::new(
                (*lower_a).clone(),
                (*lower_b).clone(),
                (*higher_a).clone(),
                (*higher_b).clone(),
            )?;
            if rule(&quartet) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether the outer parts, the highest and the bass, move by similar motion
/// into a fifth from `a` to `b`.
///
/// # Errors
///
/// Two possibilities with different numbers of parts, or none.
pub fn hidden_fifths(a: &[Pitch], b: &[Pitch]) -> Result<bool> {
    hidden(a, b, 7.0, VoiceLeadingQuartet::hidden_fifth)
}

/// Whether the outer parts, the highest and the bass, move by similar motion
/// into an octave from `a` to `b`.
///
/// # Errors
///
/// Two possibilities with different numbers of parts, or none.
pub fn hidden_octaves(a: &[Pitch], b: &[Pitch]) -> Result<bool> {
    hidden(a, b, 0.0, VoiceLeadingQuartet::hidden_octave)
}

fn hidden(
    a: &[Pitch],
    b: &[Pitch],
    semitones: FloatType,
    rule: fn(&VoiceLeadingQuartet) -> bool,
) -> Result<bool> {
    let pairs = part_pairs(a, b)?;
    let (Some((highest_a, highest_b)), Some((lowest_a, lowest_b))) = (pairs.first(), pairs.last())
    else {
        return Err(Error::FiguredBass(
            "a possibility with no parts has no outer parts".to_string(),
        ));
    };
    if !apart_by(highest_b, lowest_b, semitones) {
        return Ok(false);
    }
    let quartet = VoiceLeadingQuartet::new(
        (*lowest_a).clone(),
        (*lowest_b).clone(),
        (*highest_a).clone(),
        (*highest_b).clone(),
    )?;
    Ok(rule(&quartet))
}

/// Whether moving from `a` to `b` takes any part past where a part above it
/// was, or below where a part beneath it was.
///
/// # Errors
///
/// Two possibilities with different numbers of parts.
pub fn voice_overlap(a: &[Pitch], b: &[Pitch]) -> Result<bool> {
    let pairs = part_pairs(a, b)?;
    Ok(pairs
        .iter()
        .enumerate()
        .any(|(index, (higher_a, higher_b))| {
            pairs[index + 1..].iter().any(|(lower_a, lower_b)| {
                lower_b.ps() > higher_a.ps() || higher_b.ps() < lower_a.ps()
            })
        }))
}

/// Whether each part named in `limits` moves no further than the semitones
/// beside it from `a` to `b`. Parts are numbered from one, the highest.
///
/// # Errors
///
/// A part number either possibility has no part for.
pub fn part_movements_within_limits(
    a: &[Pitch],
    b: &[Pitch],
    limits: &[(usize, IntegerType)],
) -> Result<bool> {
    for (part, limit) in limits {
        let from = possibility_part(a, *part)?;
        let to = possibility_part(b, *part)?;
        if (to.ps() - from.ps()).abs() > FloatType::from(*limit) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether every part but the bass sings the same pitch in `a` and `b`.
///
/// # Errors
///
/// Two possibilities with different numbers of parts.
pub fn upper_parts_same(a: &[Pitch], b: &[Pitch]) -> Result<bool> {
    let pairs = part_pairs(a, b)?;
    Ok(pairs[..pairs.len().saturating_sub(1)]
        .iter()
        .all(|(from, to)| from == to))
}

/// Whether each part named in `parts` sings the same pitch in `a` and `b`;
/// with no parts named, they always do. Parts are numbered from one, the
/// highest.
///
/// # Errors
///
/// Two possibilities with different numbers of parts, or a part number they
/// have no part for.
pub fn parts_same(a: &[Pitch], b: &[Pitch], parts: Option<&[usize]>) -> Result<bool> {
    let Some(parts) = parts else {
        return Ok(true);
    };
    part_pairs(a, b)?;
    for part in parts {
        if possibility_part(a, *part)? != possibility_part(b, *part)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The notes of an Italian augmented sixth an Italian-sixth resolution is
/// judged by: its bass, root, third and fifth.
#[derive(Clone, Debug, PartialEq)]
pub struct ItalianSixth {
    /// The chord's bass.
    pub bass: Pitch,
    /// The chord's root.
    pub root: Pitch,
    /// The chord's third.
    pub third: Pitch,
    /// The chord's fifth.
    pub fifth: Pitch,
}

impl ItalianSixth {
    /// The bass, root, third and fifth of the Italian augmented sixth
    /// `possibility` spells.
    ///
    /// # Errors
    ///
    /// A possibility that spells no Italian augmented sixth.
    pub fn of(possibility: &[Pitch]) -> Result<Self> {
        Self::from_chord(&Chord::new(possibility)?)
    }

    /// The bass, root, third and fifth of `chord`, an Italian augmented
    /// sixth.
    ///
    /// # Errors
    ///
    /// A chord that is no Italian augmented sixth.
    pub fn from_chord(chord: &Chord) -> Result<Self> {
        let refused =
            || Error::FiguredBass("possibA does not spell out an It+6 chord.".to_string());
        if !chord.is_italian_augmented_sixth(false, false) {
            return Err(refused());
        }
        let (Some(bass), Some(root), Some(third), Some(fifth)) = (
            chord.bass(),
            chord.root(),
            chord.chord_step(3),
            chord.chord_step(5),
        ) else {
            return Err(refused());
        };
        Ok(Self {
            bass: bass.clone(),
            root: root.clone(),
            third: third.clone(),
            fifth: fifth.clone(),
        })
    }
}

/// Whether moving from `a`, an Italian augmented sixth, to `b` resolves it:
/// the bass falls a semitone, the root rises one, the third falls one and the
/// fifth moves by a third or a second. With `restrict_doublings`, the root
/// may resolve only once and the third not at all.
///
/// `sixth` is the chord's notes where the caller has them already, and is
/// read off `a` where not.
///
/// # Errors
///
/// An `a` that spells no Italian augmented sixth when `sixth` is not given.
pub fn could_be_italian_a6_resolution(
    a: &[Pitch],
    b: &[Pitch],
    sixth: Option<&ItalianSixth>,
    restrict_doublings: bool,
) -> Result<bool> {
    let read;
    let sixth = match sixth {
        Some(sixth) => sixth,
        None => {
            read = ItalianSixth::of(a)?;
            &read
        }
    };
    const FIFTH_MOVES: [&str; 4] = ["M3", "m3", "M2", "m-2"];
    let mut root_resolved = false;
    for (from, to) in a.iter().zip(b) {
        if from.name() == sixth.fifth.name() {
            if from == to {
                continue;
            }
            if (from.ps() - to.ps()).abs() > 4.0 {
                return Ok(false);
            }
            let moved = Interval::between_pitches(from, to)?.directed_simple_name();
            if !FIFTH_MOVES.contains(&moved.as_str()) {
                return Ok(false);
            }
        } else if from.name() == sixth.bass.name() && from == &sixth.bass {
            if from.ps() - to.ps() != 1.0
                || Interval::between_pitches(from, to)?.directed_name() != "m-2"
            {
                return Ok(false);
            }
        } else if from.name() == sixth.root.name() {
            if root_resolved && restrict_doublings {
                return Ok(false);
            }
            if to.ps() - from.ps() != 1.0
                || Interval::between_pitches(from, to)?.directed_name() != "m2"
            {
                return Ok(false);
            }
            root_resolved = true;
        } else if from.name() == sixth.third.name() {
            if restrict_doublings {
                return Ok(false);
            }
            if from.ps() - to.ps() != 1.0
                || Interval::between_pitches(from, to)?.directed_name() != "m-2"
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Each part's pitch in `a` beside its pitch in `b`.
///
/// # Errors
///
/// Two possibilities with different numbers of parts.
pub fn part_pairs<'a>(a: &'a [Pitch], b: &'a [Pitch]) -> Result<Vec<(&'a Pitch, &'a Pitch)>> {
    if a.len() != b.len() {
        return Err(Error::FiguredBass(format!(
            "possibilities of {} and {} parts cannot be paired",
            a.len(),
            b.len()
        )));
    }
    Ok(a.iter().zip(b).collect())
}
