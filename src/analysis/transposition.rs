//! How many distinct transpositions a set of pitches has: music21's
//! `analysis.transposition`.
//!
//! A set is transposed up by each of the twelve semitones, and the
//! transpositions are told apart by their normal orders. An augmented
//! triad has four, since every fourth transposition lands on itself:
//!
//! ```text
//!   +0 [0,4,8]  +1 [1,5,9]  +2 [2,6,10]  +3 [3,7,11]  +4 [0,4,8] ...
//! ```

use crate::{
    chord::Chord,
    defaults::IntegerType,
    error::{Error, Result},
    interval::Interval,
    pitch::Pitch,
};

/// The semitones in an octave, and so the transpositions there are.
const SEMITONES: IntegerType = 12;

/// A set of pitches and its transpositions: music21's
/// `TranspositionChecker`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TranspositionChecker {
    pitches: Vec<Pitch>,
}

impl TranspositionChecker {
    /// A checker of `pitches`.
    ///
    /// # Errors
    ///
    /// No pitches, which music21 refuses too.
    pub fn new(pitches: Vec<Pitch>) -> Result<Self> {
        if pitches.is_empty() {
            return Err(Error::Value(
                "Must have at least one element in list".to_string(),
            ));
        }
        Ok(Self { pitches })
    }

    /// The pitches checked.
    pub fn pitches(&self) -> &[Pitch] {
        &self.pitches
    }

    /// The set moved up by each of 0 to 11 semitones, spelled as music21
    /// spells a pitch moved by a number: music21's `getTranspositions`.
    ///
    /// # Errors
    ///
    /// A pitch that cannot be moved.
    pub fn transpositions(&self) -> Result<Vec<Vec<Pitch>>> {
        (0..SEMITONES)
            .map(|semitones| {
                let interval = Interval::from_semitones(semitones)?;
                self.pitches
                    .iter()
                    .map(|pitch| pitch.transpose(&interval))
                    .collect()
            })
            .collect()
    }

    /// The normal order of each transposition: music21's
    /// `listNormalOrders`.
    ///
    /// # Errors
    ///
    /// A pitch that cannot be moved.
    pub fn normal_orders(&self) -> Result<Vec<Vec<u8>>> {
        self.transpositions()?
            .iter()
            .map(|pitches| Ok(Chord::new(pitches.as_slice())?.normal_order()))
            .collect()
    }

    /// The normal orders, each once, in the order first met: music21's
    /// `listDistinctNormalOrders`.
    ///
    /// # Errors
    ///
    /// A pitch that cannot be moved.
    pub fn distinct_normal_orders(&self) -> Result<Vec<Vec<u8>>> {
        let mut distinct: Vec<Vec<u8>> = Vec::new();
        for order in self.normal_orders()? {
            if distinct.contains(&order) {
                continue;
            }
            distinct.push(order);
        }
        Ok(distinct)
    }

    /// How many distinct transpositions the set has: music21's
    /// `numDistinctTranspositions`.
    ///
    /// ```
    /// use music21_rs::{Pitch, analysis::transposition::TranspositionChecker};
    ///
    /// let augmented = ["C4", "E4", "G#4"].iter().map(|n| n.parse()).collect::<Result<_, _>>()?;
    /// assert_eq!(TranspositionChecker::new(augmented)?.distinct_count()?, 4);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// A pitch that cannot be moved.
    pub fn distinct_count(&self) -> Result<usize> {
        Ok(self.distinct_normal_orders()?.len())
    }

    /// A chord of each distinct normal order, spelled as music21 spells a
    /// chord of pitch-class numbers: music21's
    /// `getChordsOfDistinctTranspositions`.
    ///
    /// # Errors
    ///
    /// A pitch that cannot be moved.
    pub fn distinct_chords(&self) -> Result<Vec<Chord>> {
        self.distinct_normal_orders()?
            .into_iter()
            .map(|order| {
                let classes: Vec<IntegerType> = order.into_iter().map(IntegerType::from).collect();
                Chord::new(classes.as_slice())
            })
            .collect()
    }

    /// The pitches of each of those chords: music21's
    /// `getPitchesOfDistinctTranspositions`.
    ///
    /// # Errors
    ///
    /// A pitch that cannot be moved.
    pub fn distinct_pitches(&self) -> Result<Vec<Vec<Pitch>>> {
        Ok(self.distinct_chords()?.iter().map(Chord::pitches).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(names: &[&str]) -> TranspositionChecker {
        TranspositionChecker::new(names.iter().map(|name| name.parse().unwrap()).collect()).unwrap()
    }

    fn names(pitches: &[Pitch]) -> Vec<String> {
        pitches.iter().map(Pitch::name).collect()
    }

    #[test]
    fn a_pitch_moves_through_music21s_spellings() {
        let moved: Vec<String> = checker(&["D#"])
            .transpositions()
            .unwrap()
            .iter()
            .map(|pitches| pitches[0].name())
            .collect();
        assert_eq!(
            moved,
            [
                "E-", "E", "F", "F#", "G", "G#", "A", "B-", "B", "C", "C#", "D"
            ]
        );
    }

    #[test]
    fn an_augmented_triad_has_four_transpositions() {
        let augmented = checker(&["C4", "E4", "G#4"]);
        let orders = augmented.normal_orders().unwrap();
        assert_eq!(orders.len(), 12);
        assert_eq!(orders[4], [0, 4, 8]);
        assert_eq!(
            augmented.distinct_normal_orders().unwrap(),
            [vec![0, 4, 8], vec![1, 5, 9], vec![2, 6, 10], vec![3, 7, 11]]
        );
        assert_eq!(augmented.distinct_count().unwrap(), 4);

        let pitches: Vec<Vec<String>> = augmented
            .distinct_pitches()
            .unwrap()
            .iter()
            .map(|pitches| names(pitches))
            .collect();
        assert_eq!(
            pitches,
            [
                ["C", "E", "G#"],
                ["C#", "F", "A"],
                ["D", "F#", "A#"],
                ["E-", "G", "B"],
            ]
        );
    }

    #[test]
    fn a_diminished_seventh_has_three() {
        let chords = checker(&["C", "E-", "F#", "A"]).distinct_chords().unwrap();
        let spelled: Vec<Vec<String>> = chords.iter().map(|chord| chord.pitch_names()).collect();
        assert_eq!(
            spelled,
            [
                ["C", "E-", "F#", "A"],
                ["C#", "E", "G", "A#"],
                ["D", "F", "G#", "B"],
            ]
        );
    }

    #[test]
    fn no_pitches_are_refused() {
        assert!(TranspositionChecker::new(Vec::new()).is_err());
        assert_eq!(checker(&["D#"]).pitches().len(), 1);
    }
}
