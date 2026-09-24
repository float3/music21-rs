//! The best spelling of a run of pitches: music21's `analysis.enharmonics`.
//!
//! Every pitch may be written as itself or as any common enharmonic with at
//! most one sharp or flat. Each combination is scored by three penalties and
//! the lowest score wins, the first of equals in the order the combinations
//! are listed:
//!
//! ```text
//!   aug/dim     (augmented + diminished steps + 1) * aug_dim penalty
//! + alteration  (sharps + flats + 1)               * alteration penalty
//! + mixture     min(sharps, flats)                 * mixture penalty
//! ```
//!
//! A penalty that is off scores 1 rather than 0, as music21's does.

use itertools::Itertools;

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    pitch::Pitch,
};

/// The alteration count a spelling is allowed besides the one it came in.
const ALTER_LIMIT: IntegerType = 1;

/// What a penalty that is switched off scores.
const PENALTY_OFF: IntegerType = 1;

/// The places of the natural steps in Hewlett's base-40 system, before the
/// three places below each natural that its flats take:
/// `C-- = 1, C = 3, D = 9, ... B## = 40`.
const BASE40_STEPS: [(char, IntegerType); 7] = [
    ('C', 3),
    ('D', 9),
    ('E', 15),
    ('F', 20),
    ('G', 26),
    ('A', 32),
    ('B', 38),
];

/// The accidentals base 40 has room for, with the places each moves a step.
const BASE40_MODIFIERS: [(&str, IntegerType); 5] =
    [("--", -2), ("-", -1), ("", 0), ("#", 1), ("##", 2)];

/// The number of places in one base-40 octave.
const BASE40_OCTAVE: IntegerType = 40;

/// music21's `base40IntervalTable`: the interval each distance between two
/// base-40 places spells, within an octave. A distance with no interval
/// reads as `ddd`.
const BASE40_INTERVALS: [(IntegerType, &str); 25] = [
    (0, "P1"),
    (1, "A1"),
    (4, "d2"),
    (5, "m2"),
    (6, "M2"),
    (7, "A2"),
    (10, "d3"),
    (11, "m3"),
    (12, "M3"),
    (13, "A3"),
    (16, "d4"),
    (17, "P4"),
    (18, "A4"),
    (22, "d5"),
    (23, "P5"),
    (24, "A5"),
    (27, "d6"),
    (28, "m6"),
    (29, "M6"),
    (30, "A6"),
    (33, "d7"),
    (34, "m7"),
    (35, "M7"),
    (36, "A7"),
    (39, "d8"),
];

/// What a distance base 40 has no interval for is read as.
const BASE40_UNKNOWN: &str = "ddd";

/// Which weighting of the three penalties to score a spelling by.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EnharmonicRules {
    /// music21's `EnharmonicScoreRules`: mixing sharps and flats costs
    /// nothing.
    #[default]
    Melodic,
    /// music21's `ChordEnharmonicScoreRules`: mixing sharps and flats costs
    /// two for each of the rarer kind.
    Chordal,
}

impl EnharmonicRules {
    /// What each sharp or flat costs.
    fn alteration_penalty(self) -> IntegerType {
        4
    }

    /// What each augmented or diminished step costs.
    fn aug_dim_penalty(self) -> IntegerType {
        2
    }

    /// What each accidental of the rarer kind costs, if anything.
    fn mixture_penalty(self) -> Option<IntegerType> {
        match self {
            Self::Melodic => None,
            Self::Chordal => Some(2),
        }
    }

    /// music21's `getAlterationScore`: one more than the sharps and flats in
    /// `spelling`, times the alteration penalty.
    pub fn alteration_score(self, spelling: &[Pitch]) -> IntegerType {
        let (flats, sharps) = accidental_counts(spelling);
        (flats + sharps + 1) * self.alteration_penalty()
    }

    /// music21's `getMixSharpFlatsScore`: the count of the rarer of sharps
    /// and flats in `spelling`, times the mixture penalty.
    pub fn mixture_score(self, spelling: &[Pitch]) -> IntegerType {
        let Some(penalty) = self.mixture_penalty() else {
            return PENALTY_OFF;
        };

        let (flats, sharps) = accidental_counts(spelling);
        flats.min(sharps) * penalty
    }

    /// music21's `getAugDimScore`: one more than the augmented and
    /// diminished steps between neighbours in `spelling`, times the
    /// aug/dim penalty. `E- G#` is one, an augmented third.
    ///
    /// # Errors
    ///
    /// A neighbour spelled with an accidental base 40 has no place for,
    /// such as a triple sharp or a microtone's step.
    pub fn aug_dim_score(self, spelling: &[Pitch]) -> Result<IntegerType> {
        let mut odd_steps = 0;
        for pair in spelling.windows(2) {
            let low = base40(&pair[0])?;
            let high = base40(&pair[1])?;

            // music21 counts the letters of the interval names joined up,
            // so `ddd` counts three.
            let name = base40_interval((high - low).rem_euclid(BASE40_OCTAVE));
            odd_steps += name.chars().filter(|c| matches!(c, 'A' | 'd')).count() as IntegerType;
        }

        Ok((odd_steps + 1) * self.aug_dim_penalty())
    }

    /// The whole score of `spelling`; lower is better.
    fn score(self, spelling: &[Pitch]) -> Result<IntegerType> {
        Ok(self.aug_dim_score(spelling)?
            + self.alteration_score(spelling)
            + self.mixture_score(spelling))
    }
}

/// The best spelling of `pitches` under `rules`: music21's
/// `EnharmonicSimplifier.bestPitches`. Octaves move with a respelling, so
/// `B#3` may come back as `C4`.
///
/// ```
/// use music21_rs::{Pitch, analysis::enharmonics::{EnharmonicRules, best_spelling}};
///
/// let pitches: Vec<Pitch> = ["D--", "E", "F##"].iter().map(|n| n.parse()).collect::<Result<_, _>>()?;
/// let best = best_spelling(&pitches, EnharmonicRules::Melodic)?;
/// let names: Vec<String> = best.iter().map(Pitch::name).collect();
/// assert_eq!(names, ["C", "E", "G"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
///
/// # Errors
///
/// No pitches, which music21 refuses too, or a spelling base 40 has no
/// place for.
pub fn best_spelling(pitches: &[Pitch], rules: EnharmonicRules) -> Result<Vec<Pitch>> {
    if pitches.is_empty() {
        return Err(Error::Analysis(
            "an enharmonic spelling needs at least one pitch".to_string(),
        ));
    }

    // Each pitch as written first, then its common enharmonics.
    let spellings = pitches.iter().map(|pitch| {
        let mut options = vec![pitch.clone()];
        options.extend(pitch.all_common_enharmonics(ALTER_LIMIT));
        options
    });

    // The first strictly lowest score wins, as music21's `<` keeps it.
    let mut best: Option<(IntegerType, Vec<Pitch>)> = None;
    for spelling in spellings.multi_cartesian_product() {
        let score = rules.score(&spelling)?;
        if best.as_ref().is_some_and(|(lowest, _)| score >= *lowest) {
            continue;
        }
        best = Some((score, spelling));
    }

    Ok(best.map(|(_, spelling)| spelling).unwrap_or_default())
}

/// The flats and the sharps written in `spelling`'s names.
fn accidental_counts(spelling: &[Pitch]) -> (IntegerType, IntegerType) {
    spelling.iter().fold((0, 0), |(flats, sharps), pitch| {
        let name = pitch.name();
        let count = |mark: char| name.chars().filter(|c| *c == mark).count() as IntegerType;
        (flats + count('-'), sharps + count('#'))
    })
}

/// `pitch`'s place in Hewlett's base 40, octave aside.
fn base40(pitch: &Pitch) -> Result<IntegerType> {
    let name = pitch.name();
    let mut chars = name.chars();
    let step = chars.next();
    let modifier = chars.as_str();

    let natural = BASE40_STEPS
        .iter()
        .find(|(letter, _)| Some(*letter) == step);
    let shift = BASE40_MODIFIERS
        .iter()
        .find(|(written, _)| *written == modifier);
    let (Some((_, natural)), Some((_, shift))) = (natural, shift) else {
        return Err(Error::Analysis(format!("{name} has no place in base 40")));
    };

    Ok(natural + shift)
}

/// The interval base 40 spells for a distance within an octave.
fn base40_interval(distance: IntegerType) -> &'static str {
    BASE40_INTERVALS
        .iter()
        .find(|(known, _)| *known == distance)
        .map_or(BASE40_UNKNOWN, |(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitches(names: &[&str]) -> Vec<Pitch> {
        names.iter().map(|name| name.parse().unwrap()).collect()
    }

    fn names(pitches: &[Pitch]) -> Vec<String> {
        pitches.iter().map(Pitch::name_with_octave).collect()
    }

    #[test]
    fn a_plain_spelling_stays() {
        let best = best_spelling(&pitches(&["C", "D", "E"]), EnharmonicRules::Melodic).unwrap();
        assert_eq!(names(&best), ["C", "D", "E"]);
    }

    #[test]
    fn double_accidentals_are_simplified() {
        let best = best_spelling(&pitches(&["D--", "E", "F##"]), EnharmonicRules::Melodic).unwrap();
        assert_eq!(names(&best), ["C", "E", "G"]);
    }

    #[test]
    fn an_octave_moves_with_its_respelling() {
        let best =
            best_spelling(&pitches(&["B#3", "F-4", "G4"]), EnharmonicRules::Melodic).unwrap();
        assert_eq!(names(&best), ["C4", "E4", "G4"]);
    }

    #[test]
    fn no_pitches_are_refused() {
        assert!(best_spelling(&[], EnharmonicRules::Melodic).is_err());
    }

    #[test]
    fn scores_add_up_as_music21_adds_them() {
        // C E G# has no odd step; C E- G# has one, the augmented third E- G#.
        let spelling = pitches(&["C", "E", "G#"]);
        assert_eq!(EnharmonicRules::Melodic.alteration_score(&spelling), 8);
        assert_eq!(
            EnharmonicRules::Melodic.aug_dim_score(&spelling).unwrap(),
            2
        );
        let augmented = pitches(&["C", "E-", "G#"]);
        assert_eq!(
            EnharmonicRules::Melodic.aug_dim_score(&augmented).unwrap(),
            4
        );
        assert_eq!(
            EnharmonicRules::Melodic.mixture_score(&spelling),
            PENALTY_OFF
        );

        let mixed = pitches(&["C#", "E-", "G#"]);
        assert_eq!(EnharmonicRules::Chordal.mixture_score(&mixed), 2);
    }

    #[test]
    fn a_distance_base40_cannot_spell_reads_as_three_odd_steps() {
        // C to E##: 17 - 3 = 14 places, which base 40 has no name for.
        let odd = pitches(&["C", "E##"]);
        assert_eq!(EnharmonicRules::Melodic.aug_dim_score(&odd).unwrap(), 8);
    }

    #[test]
    fn a_triple_sharp_has_no_base40_place() {
        let spelling = pitches(&["C###", "E"]);
        assert!(EnharmonicRules::Melodic.aug_dim_score(&spelling).is_err());
    }
}
