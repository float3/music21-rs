//! Notes on the strings and frets of a fretted instrument: music21's
//! `tablature`.
//!
//! ```
//! use music21_rs::tablature::{FretBoard, FretNote};
//!
//! let board = FretBoard::guitar(vec![FretNote::new(Some(1), Some(2), Some(2))], 4);
//! let pitches = board.pitches()?;
//! assert_eq!(pitches[5].as_ref().map(|pitch| pitch.name_with_octave()), Some("F#4".to_string()));
//! # Ok::<(), music21_rs::Error>(())
//! ```

use std::fmt;

use crate::{
    chordsymbol::ChordSymbol,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    pitch::{Pitch, microtone::ordinal_suffix},
};

/// A finger on a string at a fret: music21's `FretNote`. Strings count
/// from the highest-sounding, one, as guitarists count them.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FretNote {
    /// The string.
    pub string: Option<IntegerType>,
    /// The fret, nought for the open string.
    pub fret: Option<IntegerType>,
    /// The finger.
    pub fingering: Option<IntegerType>,
    /// Whether the finger is shown: music21's `displayFingerNumber`.
    pub display_finger_number: bool,
}

impl FretNote {
    /// A note on a string at a fret, played by a finger, the finger shown.
    pub fn new(
        string: Option<IntegerType>,
        fret: Option<IntegerType>,
        fingering: Option<IntegerType>,
    ) -> Self {
        Self {
            string,
            fret,
            fingering,
            display_finger_number: true,
        }
    }
}

/// Written as music21 writes the note inside its representation: `6th
/// string, 133rd fret, 2nd finger`, leaving out what is not given.
impl fmt::Display for FretNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = [
            (self.string, "string"),
            (self.fret, "fret"),
            (self.fingering, "finger"),
        ]
        .into_iter()
        .filter_map(|(value, what)| {
            value.map(|value| format!("{value}{} {what}", ordinal_suffix(value)))
        })
        .collect();
        f.write_str(&parts.join(", "))
    }
}

/// A fretted instrument's strings, the notes stopped on them, and their
/// open pitches: music21's `FretBoard`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FretBoard {
    /// How many strings there are.
    pub strings: usize,
    /// The notes stopped, in no particular order.
    pub fret_notes: Vec<FretNote>,
    /// How many frets a diagram shows.
    pub display_frets: usize,
    /// Each string's open pitch, lowest string first.
    pub tuning: Vec<Pitch>,
}

fn tuned(names: &[&str]) -> Vec<Pitch> {
    names
        .iter()
        .map(|name| Pitch::from_name(*name).expect("a tuning names pitches"))
        .collect()
}

impl FretBoard {
    /// A board of so many strings with these notes, not yet tuned.
    pub fn new(strings: usize, fret_notes: Vec<FretNote>, display_frets: usize) -> Self {
        Self {
            strings,
            fret_notes,
            display_frets,
            tuning: Vec::new(),
        }
    }

    /// A guitar's six strings, E2 A2 D3 G3 B3 E4: music21's
    /// `GuitarFretBoard`.
    pub fn guitar(fret_notes: Vec<FretNote>, display_frets: usize) -> Self {
        Self {
            tuning: tuned(&["E2", "A2", "D3", "G3", "B3", "E4"]),
            ..Self::new(6, fret_notes, display_frets)
        }
    }

    /// A ukulele's four strings, re-entrant G4 C4 E4 A4: music21's
    /// `UkeleleFretBoard`.
    pub fn ukulele(fret_notes: Vec<FretNote>, display_frets: usize) -> Self {
        Self {
            tuning: tuned(&["G4", "C4", "E4", "A4"]),
            ..Self::new(4, fret_notes, display_frets)
        }
    }

    /// A bass guitar's four strings, E1 A1 D2 G2: music21's
    /// `BassGuitarFretBoard`.
    pub fn bass_guitar(fret_notes: Vec<FretNote>, display_frets: usize) -> Self {
        Self {
            tuning: tuned(&["E1", "A1", "D2", "G2"]),
            ..Self::new(4, fret_notes, display_frets)
        }
    }

    /// A mandolin's four courses, G3 D4 A4 E5: music21's
    /// `MandolinFretBoard`.
    pub fn mandolin(fret_notes: Vec<FretNote>, display_frets: usize) -> Self {
        Self {
            tuning: tuned(&["G3", "D4", "A4", "E5"]),
            ..Self::new(4, fret_notes, display_frets)
        }
    }

    /// The note on a string, the first if several are: music21's
    /// `getFretNoteByString`.
    pub fn fret_note_by_string(&self, string: IntegerType) -> Option<&FretNote> {
        self.fret_notes
            .iter()
            .find(|note| note.string == Some(string))
    }

    /// The note on each string that has one, lowest-sounding string first:
    /// music21's `fretNotesLowestFirst`.
    pub fn fret_notes_lowest_first(&self) -> Vec<&FretNote> {
        (1..=self.strings as IntegerType)
            .rev()
            .filter_map(|string| self.fret_note_by_string(string))
            .collect()
    }

    /// The pitch each string sounds, lowest string first, nothing for a
    /// string with no note: music21's `getPitches`. A note's pitch is its
    /// string's open pitch raised by its fret, spelled as music21 spells a
    /// pitch made from a number. Strings are found as music21 finds them,
    /// counting back from the highest, so a note naming no string is read
    /// as on the lowest.
    ///
    /// # Errors
    ///
    /// A board whose tuning is not one pitch a string, or a note on a
    /// string the board has not got.
    pub fn pitches(&self) -> Result<Vec<Option<Pitch>>> {
        if self.tuning.len() != self.strings {
            return Err(Error::Tablature(format!(
                "Tuning must be set first, tuned for {} notes, on a {} string instrument",
                self.tuning.len(),
                self.strings
            )));
        }
        let mut pitches: Vec<Option<Pitch>> = vec![None; self.strings];
        for note in &self.fret_notes {
            // music21 indexes the strings with `-string`, as Python indexes a
            // list: string nought is the first entry and string one the
            // last.
            let python_index = -i64::from(note.string.unwrap_or(0));
            let index = if python_index < 0 {
                python_index + self.strings as i64
            } else {
                python_index
            };
            let index = usize::try_from(index)
                .ok()
                .filter(|index| *index < self.strings)
                .ok_or_else(|| Error::Tablature("list index out of range".to_string()))?;
            let open = self.tuning[index].ps();
            let sounding = open + note.fret.unwrap_or(0) as FloatType;
            pitches[index] = Some(Pitch::from_number(sounding)?);
        }
        Ok(pitches)
    }
}

/// The fret a diagram starts at, and where its number is written: music21's
/// `FirstFret`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirstFret {
    /// The fret.
    pub fret: IntegerType,
    /// Where its number goes, `right` by default.
    pub location: String,
}

impl FirstFret {
    /// A first fret with its number on the right.
    pub fn new(fret: IntegerType) -> Self {
        Self {
            fret,
            location: "right".to_string(),
        }
    }
}

/// A chord symbol with a fret diagram: music21's `ChordWithFretBoard`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChordWithFretBoard {
    /// The symbol.
    pub chord_symbol: ChordSymbol,
    /// The diagram.
    pub fret_board: FretBoard,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fret_notes_are_written_and_ordered_as_music21_does() {
        // music21's own tests of tablature.
        assert_eq!(
            FretNote::new(Some(6), Some(133), None).to_string(),
            "6th string, 133rd fret"
        );
        assert_eq!(FretNote::new(None, None, None).string, None);
        let board = FretBoard::new(
            6,
            vec![
                FretNote::new(Some(1), Some(2), Some(2)),
                FretNote::new(Some(2), Some(1), Some(1)),
            ],
            4,
        );
        let strings: Vec<Option<IntegerType>> = board
            .fret_notes_lowest_first()
            .iter()
            .map(|note| note.string)
            .collect();
        assert_eq!(strings, [Some(2), Some(1)]);
        assert!(board.pitches().is_err());
    }

    #[test]
    fn a_board_sounds_its_strings_raised_by_their_frets() -> Result<()> {
        // music21's GuitarFretBoard doctest: an A major chord.
        let board = FretBoard::guitar(
            vec![
                FretNote::new(Some(3), Some(2), None),
                FretNote::new(Some(2), Some(2), None),
                FretNote::new(Some(4), Some(2), None),
                FretNote::new(Some(5), Some(0), None),
            ],
            4,
        );
        let names: Vec<Option<String>> = board
            .pitches()?
            .iter()
            .map(|pitch| pitch.as_ref().map(Pitch::name_with_octave))
            .collect();
        assert_eq!(
            names,
            [
                None,
                Some("A2".to_string()),
                Some("E3".to_string()),
                Some("A3".to_string()),
                Some("C#4".to_string()),
                None
            ]
        );
        Ok(())
    }
}
