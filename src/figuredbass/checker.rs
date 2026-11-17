//! Finding where a written-out score breaks the voice-leading rules:
//! music21's `figuredBass.checker`.
//!
//! Where [`super::possibility`] says *whether* a voicing breaks a rule, this
//! says *which parts* do, numbered from one, the highest. A part that is
//! resting holds [`REST_PS`] in its place and is passed over by every rule.
//!
//! The consecutive rules find their candidates by interval alone and leave
//! the deciding to a [`VoiceLeadingQuartet`]; [`candidates`] and
//! [`breaks_rule`] are the two halves, so a caller keeping a table of
//! quartets already decided, as music21 does, asks only for the new ones.

use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::pitch::Pitch;
use crate::voiceleading::VoiceLeadingQuartet;

/// The pitch space a resting part is given in a possibility: music21's
/// `POSSIBILITY_REST_PS`.
pub const REST_PS: FloatType = 0.0;

/// Whether a pitch stands for a resting part.
#[must_use]
pub fn is_rest(pitch: &Pitch) -> bool {
    pitch.ps() == REST_PS
}

/// The rules over two possibilities in a row.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConsecutiveRule {
    /// Two parts a fifth apart moving to another fifth.
    ParallelFifths,
    /// Two parts an octave apart moving to another octave.
    ParallelOctaves,
    /// The outer parts moving in similar motion into a fifth.
    HiddenFifths,
    /// The outer parts moving in similar motion into an octave.
    HiddenOctaves,
}

impl ConsecutiveRule {
    /// Semitones past whole octaves that the parts must stand apart for the
    /// rule to need asking.
    fn semitones(self) -> FloatType {
        match self {
            Self::ParallelFifths | Self::HiddenFifths => 7.0,
            Self::ParallelOctaves | Self::HiddenOctaves => 0.0,
        }
    }

    /// Whether the rule is about the outer parts only.
    fn is_hidden(self) -> bool {
        matches!(self, Self::HiddenFifths | Self::HiddenOctaves)
    }
}

/// Every pair of parts, as part numbers, where a higher part sings below
/// a lower one: music21's `checker.voiceCrossing`.
///
/// ```
/// use music21::Pitch;
/// use music21::figuredbass::checker::voice_crossings;
///
/// let pitches: Vec<Pitch> = ["C5", "G5", "E4"]
///     .iter()
///     .map(|name| Pitch::from_name(*name).unwrap())
///     .collect();
/// assert_eq!(voice_crossings(&pitches), [(1, 2)]);
/// ```
#[must_use]
pub fn voice_crossings(possibility: &[Pitch]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    for (higher_index, higher) in possibility.iter().enumerate() {
        if is_rest(higher) {
            continue;
        }
        for (lower_index, lower) in possibility.iter().enumerate().skip(higher_index + 1) {
            if !is_rest(lower) && higher.ps() < lower.ps() {
                found.push((higher_index + 1, lower_index + 1));
            }
        }
    }
    found
}

/// Whether two pitches stand `semitones` apart past whole octaves.
fn apart_by(a: &Pitch, b: &Pitch, semitones: FloatType) -> bool {
    (a.ps() - b.ps()).abs() % 12.0 == semitones
}

/// The pairs of parts, as part numbers, whose move from `a` to `b` stands
/// at the rule's interval and so needs [`breaks_rule`] to decide it. For
/// a hidden rule that is the outer parts or nothing.
///
/// # Errors
///
/// Two possibilities with different numbers of parts, or none.
pub fn candidates(a: &[Pitch], b: &[Pitch], rule: ConsecutiveRule) -> Result<Vec<(usize, usize)>> {
    if a.len() != b.len() {
        return Err(Error::FiguredBass(format!(
            "possibilities of {} and {} parts cannot be paired",
            a.len(),
            b.len()
        )));
    }
    let semitones = rule.semitones();
    if rule.is_hidden() {
        let (Some(last_a), Some(last_b)) = (a.last(), b.last()) else {
            return Err(Error::FiguredBass(
                "a possibility with no parts has no outer parts".to_string(),
            ));
        };
        let outer = [&a[0], &b[0], last_a, last_b];
        if outer.iter().any(|pitch| is_rest(pitch)) || !apart_by(&b[0], last_b, semitones) {
            return Ok(Vec::new());
        }
        return Ok(vec![(1, b.len())]);
    }

    let mut found = Vec::new();
    for higher in 0..a.len() {
        if is_rest(&a[higher]) || is_rest(&b[higher]) {
            continue;
        }
        for lower in higher + 1..a.len() {
            if is_rest(&a[lower]) || is_rest(&b[lower]) {
                continue;
            }
            if apart_by(&a[higher], &a[lower], semitones)
                && apart_by(&b[higher], &b[lower], semitones)
            {
                found.push((higher + 1, lower + 1));
            }
        }
    }
    Ok(found)
}

/// Whether the lower part moving from `lower.0` to `lower.1` against the
/// higher moving from `higher.0` to `higher.1` breaks the rule, as
/// music21's `VoiceLeadingQuartet` decides it.
///
/// # Errors
///
/// Pitches a quartet cannot be made of.
pub fn breaks_rule(
    lower: (&Pitch, &Pitch),
    higher: (&Pitch, &Pitch),
    rule: ConsecutiveRule,
) -> Result<bool> {
    let quartet = VoiceLeadingQuartet::new(
        lower.0.clone(),
        lower.1.clone(),
        higher.0.clone(),
        higher.1.clone(),
    )?;
    Ok(match rule {
        ConsecutiveRule::ParallelFifths => quartet.parallel_fifth(),
        ConsecutiveRule::ParallelOctaves => quartet.parallel_octave(),
        ConsecutiveRule::HiddenFifths => quartet.hidden_fifth(),
        ConsecutiveRule::HiddenOctaves => quartet.hidden_octave(),
    })
}

/// Every pair of parts, as part numbers, that breaks the rule moving from
/// `a` to `b`: music21's `checker.parallelFifths` and its three siblings.
///
/// ```
/// use music21::Pitch;
/// use music21::figuredbass::checker::{ConsecutiveRule, violations};
///
/// let pitches = |names: &[&str]| -> Vec<Pitch> {
///     names.iter().map(|name| Pitch::from_name(*name).unwrap()).collect()
/// };
/// let a = pitches(&["B4", "G3", "C3"]);
/// let b = pitches(&["A4", "A3", "D3"]);
/// assert_eq!(violations(&a, &b, ConsecutiveRule::ParallelFifths)?, [(2, 3)]);
/// # Ok::<(), music21::Error>(())
/// ```
///
/// # Errors
///
/// Two possibilities with different numbers of parts, or none.
pub fn violations(a: &[Pitch], b: &[Pitch], rule: ConsecutiveRule) -> Result<Vec<(usize, usize)>> {
    let mut found = Vec::new();
    for (higher, lower) in candidates(a, b, rule)? {
        let lower_move = (&a[lower - 1], &b[lower - 1]);
        let higher_move = (&a[higher - 1], &b[higher - 1]);
        if breaks_rule(lower_move, higher_move, rule)? {
            found.push((higher, lower));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitches(names: &[&str]) -> Vec<Pitch> {
        names
            .iter()
            .map(|name| Pitch::from_name(*name).unwrap())
            .collect()
    }

    fn rest() -> Pitch {
        let mut placeholder = Pitch::from_name("C4").unwrap();
        placeholder.set_ps(REST_PS);
        placeholder
    }

    #[test]
    fn the_rules_name_music21s_parts() {
        let a = pitches(&["C4", "G3", "C3"]);
        let b = pitches(&["D4", "A3", "D3"]);
        assert_eq!(
            violations(&a, &b, ConsecutiveRule::ParallelOctaves).unwrap(),
            [(1, 3)]
        );
        assert_eq!(
            violations(&a, &b, ConsecutiveRule::ParallelFifths).unwrap(),
            [(2, 3)]
        );

        let a = pitches(&["E5", "C3"]);
        let b = pitches(&["A5", "D3"]);
        assert_eq!(
            violations(&a, &b, ConsecutiveRule::HiddenFifths).unwrap(),
            [(1, 2)]
        );
        let b = pitches(&["A-5", "D3"]);
        assert!(
            violations(&a, &b, ConsecutiveRule::HiddenFifths)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_resting_part_breaks_no_rule() {
        let crossed = vec![
            pitches(&["C5"])[0].clone(),
            rest(),
            pitches(&["G5"])[0].clone(),
        ];
        assert_eq!(voice_crossings(&crossed), [(1, 3)]);

        let a = vec![rest(), pitches(&["C3"])[0].clone()];
        let b = pitches(&["A5", "D3"]);
        assert!(
            candidates(&a, &b, ConsecutiveRule::HiddenFifths)
                .unwrap()
                .is_empty()
        );
        assert!(candidates(&a, &pitches(&["A5"]), ConsecutiveRule::ParallelFifths).is_err());
    }
}
