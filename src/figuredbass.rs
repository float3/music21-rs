//! Figured bass: the numbers written under a bass note, and what they mean.
//!
//! A column of figures says how far above the bass each note of the chord
//! stands, and an accidental beside a number says how that note is spelled.
//! Most of a column is left out in practice — a bare `7` means a seventh
//! chord in root position, fifth and third and all — so a column is read in
//! two forms: as written, and expanded to every note it stands for.
//!
//! This is the port of music21's `figuredBass.notation`, and it is what
//! [`crate::roman::RomanNumeral`] reads its own digits with.

use std::fmt;

use crate::{
    defaults::IntegerType,
    error::{Error, Result},
    pitch::{Accidental, Pitch},
};

/// The number a figure carries when it is nothing but an extender line:
/// music21's `EXTENDER_SENTINEL`.
pub const EXTENDER: IntegerType = -1;

/// The shorthand a column is written in, and every note it stands for.
///
/// music21's `shorthandNotation`. A column not listed here is read as it was
/// written.
const SHORTHAND: &[(&[Option<IntegerType>], &[IntegerType])] = &[
    // A column written as nothing at all, or as a bare accidental, is the
    // triad over its bass.
    (&[None], &[5, 3]),
    (&[Some(5)], &[5, 3]),
    (&[Some(6)], &[6, 3]),
    (&[Some(7)], &[7, 5, 3]),
    (&[Some(9)], &[9, 7, 5, 3]),
    (&[Some(11)], &[11, 9, 7, 5, 3]),
    (&[Some(13)], &[13, 11, 9, 7, 5, 3]),
    (&[Some(6), Some(5)], &[6, 5, 3]),
    (&[Some(4), Some(3)], &[6, 4, 3]),
    (&[Some(4), Some(2)], &[6, 4, 2]),
    (&[Some(2)], &[6, 4, 2]),
];

/// The marks figured bass writes that music21's `Accidental` does not know by
/// itself, and what each stands for: music21's `specialModifiers`.
const SPECIAL: &[(&str, &str)] = &[
    ("+", "#"),
    ("/", "-"),
    ("\\", "#"),
    ("b", "-"),
    ("bb", "--"),
    ("bbb", "---"),
    ("bbbb", "-----"),
    ("++", "##"),
    ("+++", "###"),
    ("++++", "####"),
    ("\u{266f}", "#"),
    ("\u{266e}", "n"),
    ("\u{266d}", "-"),
];

/// The accidental written beside a figure: music21's `Modifier`.
///
/// Figured bass has its own marks — a `+` raises and a `/` lowers — so the
/// string it was written with is kept alongside the accidental it means.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Modifier {
    written: Option<String>,
    accidental: Option<Accidental>,
}

impl Modifier {
    /// Reads one, which may be nothing at all: neither `None` nor the empty
    /// string says anything about how the note is spelled.
    pub fn new(written: Option<&str>) -> Result<Self> {
        let Some(mark) = written.filter(|mark| !mark.is_empty()) else {
            return Ok(Self {
                written: written.map(str::to_string),
                accidental: None,
            });
        };
        let accidental = match Accidental::new(mark) {
            Ok(accidental) => accidental,
            Err(_) => {
                let Some((_, spelled)) = SPECIAL.iter().find(|(special, _)| *special == mark)
                else {
                    return Err(Error::Notation(format!(
                        "Figure modifier unsupported in music21: {mark}"
                    )));
                };
                Accidental::new(*spelled)?
            }
        };
        Ok(Self {
            written: Some(mark.to_string()),
            accidental: Some(accidental),
        })
    }

    /// The mark as it was written.
    pub fn written(&self) -> Option<&str> {
        self.written.as_deref()
    }

    /// The accidental it stands for, and nothing where nothing was written.
    pub fn accidental(&self) -> Option<&Accidental> {
        self.accidental.as_ref()
    }

    /// The same note spelled as this modifier asks.
    ///
    /// A written natural replaces whatever was there; anything else is added
    /// to it, so a sharp against a flattened note raises it to a natural.
    pub fn modify(&self, pitch: &Pitch) -> Result<Pitch> {
        let Some(accidental) = &self.accidental else {
            return Ok(pitch.clone());
        };
        let mut modified = pitch.clone();
        let alter = if accidental.alter() == 0.0 || !pitch.has_accidental() {
            accidental.alter()
        } else {
            pitch.accidental().alter() + accidental.alter()
        };
        modified.set_accidental(Some(Accidental::new(alter)?));
        Ok(modified)
    }
}

impl fmt::Display for Modifier {
    /// music21's `_reprInternal`: the mark and the accidental it means.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let written = match &self.written {
            Some(written) => written.as_str(),
            None => "None",
        };
        match &self.accidental {
            Some(accidental) => write!(f, "{written} {}", accidental.name()),
            None => write!(f, "{written} None"),
        }
    }
}

/// One figure of a column: a number above the bass and the accidental written
/// beside it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Figure {
    number: Option<IntegerType>,
    modifier: Modifier,
    extender: bool,
}

impl Figure {
    /// One figure. A number of `None` is a figure written as a bare
    /// accidental, which stands for the third.
    pub fn new(number: Option<IntegerType>, modifier: Modifier, extender: bool) -> Self {
        Self {
            number,
            modifier,
            extender,
        }
    }

    /// How far above the bass the note stands, counted inclusively.
    pub fn number(&self) -> Option<IntegerType> {
        self.number
    }

    /// Puts the figure at another number.
    pub fn set_number(&mut self, number: Option<IntegerType>) {
        self.number = number;
    }

    /// The accidental written beside the number.
    pub fn modifier(&self) -> &Modifier {
        &self.modifier
    }

    /// Whether a line carries the figure on from the note before.
    pub fn has_extender(&self) -> bool {
        self.extender
    }

    /// Whether the figure is nothing but that line: music21 reads a number of
    /// one as no number at all, so an extender on it carries the whole
    /// column forward.
    pub fn is_pure_extender(&self) -> bool {
        self.number == Some(1) && self.extender
    }
}

impl fmt::Display for Figure {
    /// music21's `_reprInternal`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_pure_extender() {
            return write!(f, "pure-extender <Modifier {}>", self.modifier);
        }
        let number = match self.number {
            Some(EXTENDER) => "_".to_string(),
            Some(number) => number.to_string(),
            None => "None".to_string(),
        };
        let extender = if self.extender { "(extender)" } else { "" };
        write!(f, "{number}{extender} <Modifier {}>", self.modifier)
    }
}

/// One column of figured bass, as written and as it stands expanded.
///
/// music21's `figuredBass.notation.Notation`. The figures are given as one
/// string, comma-separated: `'7,5,#3'`, `'6,4'`, `'4+,2'`.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Notation {
    column: String,
    figure_strings: Vec<String>,
    original_numbers: Vec<Option<IntegerType>>,
    original_modifiers: Vec<Option<String>>,
    numbers: Vec<Option<IntegerType>>,
    modifier_strings: Vec<Option<String>>,
    extenders: Vec<bool>,
    figures: Vec<Figure>,
    figures_as_written: Vec<Figure>,
}

impl Notation {
    /// Reads a column.
    pub fn parse(column: &str) -> Result<Self> {
        let mut notation = Self {
            column: column.to_string(),
            ..Self::default()
        };
        notation.read_column()?;
        notation.expand();
        notation.build_figures()?;
        Ok(notation)
    }

    /// music21's `_parseNotationColumn`: every comma-separated figure split
    /// into its number and the mark beside it.
    fn read_column(&mut self) -> Result<()> {
        for written in self.column.split(',') {
            let written = written.trim();
            self.figure_strings.push(written.to_string());
            let digits: String = written
                .chars()
                .filter(|letter| letter.is_ascii_digit() || *letter == '_')
                .collect();
            let marks: String = written
                .chars()
                .filter(|letter| !letter.is_ascii_digit() && *letter != '_')
                .collect();

            let mut number = None;
            let mut extender = false;
            if !digits.is_empty() {
                if digits == "_" {
                    number = Some(EXTENDER);
                    extender = true;
                } else if digits.contains('_') {
                    extender = true;
                    number = digits.trim_matches('_').parse().ok();
                } else {
                    number = digits.parse().ok();
                }
            }
            self.original_numbers.push(number);
            self.original_modifiers
                .push((!marks.is_empty()).then_some(marks));
            self.extenders.push(extender);
        }
        self.numbers = self.original_numbers.clone();
        self.modifier_strings = self.original_modifiers.clone();
        Ok(())
    }

    /// music21's `_translateToLonghand`: a column written in shorthand stands
    /// for every note the shorthand leaves out, and each mark stays with the
    /// number it was written against.
    fn expand(&mut self) {
        let longhand = SHORTHAND
            .iter()
            .find(|(shorthand, _)| *shorthand == self.numbers.as_slice())
            .map(|(_, longhand)| longhand.to_vec());
        let Some(longhand) = longhand else {
            // A figure written as a bare accidental is the third.
            self.numbers = self
                .numbers
                .iter()
                .map(|number| Some(number.unwrap_or(3)))
                .collect();
            return;
        };
        let against: Vec<IntegerType> = self
            .numbers
            .iter()
            .map(|number| number.unwrap_or(3))
            .collect();
        self.modifier_strings = longhand
            .iter()
            .map(|number| {
                against
                    .iter()
                    .position(|written| written == number)
                    .and_then(|index| self.modifier_strings[index].clone())
            })
            .collect();
        self.numbers = longhand.into_iter().map(Some).collect();
    }

    /// music21's `_getFigures`: the numbers and their marks, paired up.
    fn build_figures(&mut self) -> Result<()> {
        for (index, number) in self.numbers.iter().enumerate() {
            let modifier = Modifier::new(self.modifier_strings[index].as_deref())?;
            let extender = self.extenders.get(index).copied().unwrap_or(false);
            self.figures.push(Figure::new(*number, modifier, extender));
        }
        for (index, number) in self.original_numbers.iter().enumerate() {
            let modifier = Modifier::new(self.original_modifiers[index].as_deref())?;
            self.figures_as_written
                .push(Figure::new(*number, modifier, false));
        }
        Ok(())
    }

    /// The column as it was written.
    pub fn column(&self) -> &str {
        &self.column
    }

    /// Each figure of the written column, as a string.
    pub fn figure_strings(&self) -> &[String] {
        &self.figure_strings
    }

    /// The numbers as written, before the shorthand was expanded.
    pub fn original_numbers(&self) -> &[Option<IntegerType>] {
        &self.original_numbers
    }

    /// The marks as written.
    pub fn original_modifiers(&self) -> &[Option<String>] {
        &self.original_modifiers
    }

    /// The numbers of the expanded column.
    pub fn numbers(&self) -> &[Option<IntegerType>] {
        &self.numbers
    }

    /// The marks of the expanded column.
    pub fn modifier_strings(&self) -> &[Option<String>] {
        &self.modifier_strings
    }

    /// Whether any figure carries a line on from the note before.
    pub fn has_extenders(&self) -> bool {
        self.extenders.iter().any(|extender| *extender)
    }

    /// Which figures carry such a line.
    pub fn extenders(&self) -> &[bool] {
        &self.extenders
    }

    /// The figures of the expanded column.
    pub fn figures(&self) -> &[Figure] {
        &self.figures
    }

    /// The figures of the column as it was written.
    pub fn figures_as_written(&self) -> &[Figure] {
        &self.figures_as_written
    }

    /// The accidentals of the expanded column, one per figure.
    pub fn modifiers(&self) -> Vec<&Modifier> {
        self.figures.iter().map(Figure::modifier).collect()
    }
}

impl fmt::Display for Notation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_expands_out_of_its_shorthand() {
        let notation = Notation::parse("4+,2").unwrap();
        assert_eq!(notation.figure_strings(), ["4+", "2"]);
        assert_eq!(notation.original_numbers(), [Some(4), Some(2)]);
        assert_eq!(notation.numbers(), [Some(6), Some(4), Some(2)]);
        assert_eq!(
            notation.modifier_strings(),
            [None, Some("+".to_string()), None]
        );
        assert_eq!(
            notation.figures()[1].to_string(),
            "4 <Modifier + sharp>",
            "the mark stays with the number it was written against"
        );
    }

    #[test]
    fn a_bare_mark_is_the_third() {
        let notation = Notation::parse("-6, -").unwrap();
        assert_eq!(notation.original_numbers(), [Some(6), None]);
        assert_eq!(notation.numbers(), [Some(6), Some(3)]);
        assert_eq!(notation.figures()[1].to_string(), "3 <Modifier - flat>");
    }

    #[test]
    fn figured_bass_reads_its_own_marks() {
        // A `+` raises and a `/` lowers, which no accidental name would.
        assert_eq!(
            Modifier::new(Some("+"))
                .unwrap()
                .accidental()
                .map(|accidental| accidental.name()),
            Some("sharp")
        );
        assert_eq!(
            Modifier::new(Some("/"))
                .unwrap()
                .accidental()
                .map(|accidental| accidental.name()),
            Some("flat")
        );
        assert!(Modifier::new(Some("")).unwrap().accidental().is_none());
        assert!(Modifier::new(None).unwrap().accidental().is_none());
        assert!(Modifier::new(Some("zzz")).is_err());
    }

    #[test]
    fn an_extender_carries_the_figure_on() {
        let notation = Notation::parse("7_").unwrap();
        assert!(notation.has_extenders());
        let pure = Figure::new(Some(1), Modifier::new(Some("#")).unwrap(), true);
        assert!(pure.is_pure_extender());
        assert_eq!(pure.to_string(), "pure-extender <Modifier # sharp>");
    }
}
