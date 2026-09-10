//! Tone rows and twelve-tone serial transformations, a port of music21's
//! `serial` module.
//!
//! A [`ToneRow`] is an ordered sequence of pitch classes. It need not have
//! twelve members and need not be a permutation of the aggregate; the methods
//! that only make sense for a true twelve-tone row (`is_all_interval`,
//! `link_classification`, `are_combinatorial`) return an error otherwise,
//! where music21 raises `SerialException`.
//!
//! music21 separates `ToneRow`, `TwelveToneRow` and `HistoricalTwelveToneRow`
//! as stream subclasses. Nothing here dispatches on that distinction, so there
//! is one row type, and the historical rows are a table of [`HistoricalRow`]
//! entries that build a `ToneRow` on demand.

use std::fmt;
use std::ops::Index;
use std::str::FromStr;

use crate::{
    chord::root,
    defaults::{IntegerType, UnsignedIntegerType},
    error::{Error, Result},
    pitch::{Pitch, pitchclass::convert_pitch_class_to_str},
};

mod tables;

pub use tables::HISTORICAL_ROWS;
use tables::LINK_CHORDS;

/// A serial transformation of a tone row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transformation {
    /// The row itself, transposed. music21 writes this `P` in the
    /// zero-centered convention and `T` in the original-centered one.
    Prime,
    /// The row inverted about its first pitch class.
    Inversion,
    /// The row backwards.
    Retrograde,
    /// The inversion backwards.
    RetrogradeInversion,
}

impl Transformation {
    /// The four transformations in music21's order.
    pub const ALL: [Transformation; 4] = [
        Transformation::Prime,
        Transformation::Inversion,
        Transformation::Retrograde,
        Transformation::RetrogradeInversion,
    ];

    /// music21's label under the given convention: `P`/`T`, `I`, `R`, `RI`.
    pub fn label(self, convention: TransformationConvention) -> &'static str {
        match (self, convention) {
            (Transformation::Prime, TransformationConvention::ZeroCentered) => "P",
            (Transformation::Prime, TransformationConvention::OriginalCentered) => "T",
            (Transformation::Inversion, _) => "I",
            (Transformation::Retrograde, _) => "R",
            (Transformation::RetrogradeInversion, _) => "RI",
        }
    }

    /// Parses music21's label. `P` and `T` both mean [`Transformation::Prime`].
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "P" | "T" => Ok(Transformation::Prime),
            "I" => Ok(Transformation::Inversion),
            "R" => Ok(Transformation::Retrograde),
            "RI" => Ok(Transformation::RetrogradeInversion),
            other => Err(Error::Serial(format!(
                "Invalid transformation type: {other}"
            ))),
        }
    }
}

impl fmt::Display for Transformation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label(TransformationConvention::ZeroCentered))
    }
}

impl FromStr for Transformation {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        Transformation::from_name(name)
    }
}

/// What the index of a transformation refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TransformationConvention {
    /// `P(n)` and `I(n)` start on pitch class `n`; `R(n)` and `RI(n)` end on
    /// it. This is the common convention and music21's default.
    ZeroCentered,
    /// `T(n)` transposes the original row up `n` semitones, and `I(n)`,
    /// `R(n)` and `RI(n)` transform the row in place and then transpose the
    /// result by `n`.
    OriginalCentered,
}

impl TransformationConvention {
    /// Parses music21's convention argument, `"zero"` or `"original"`.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "zero" => Ok(TransformationConvention::ZeroCentered),
            "original" => Ok(TransformationConvention::OriginalCentered),
            _ => Err(Error::Serial(
                "Invalid convention - choose 'zero' or 'original'.".to_string(),
            )),
        }
    }
}

/// A transformation and its index, as music21 reports one: `("I", 8)`.
pub type IndexedTransformation = (Transformation, u8);

/// An ordered sequence of pitch classes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct ToneRow {
    pitch_classes: Vec<u8>,
}

fn wrap(value: IntegerType) -> u8 {
    value.rem_euclid(12) as u8
}

impl ToneRow {
    /// Builds a row from pitch-class integers, which are reduced modulo 12 the
    /// way music21's `pcToToneRow` does, so `[-6, 19, 128]` is `[6, 7, 8]`.
    pub fn new(pitch_classes: impl IntoIterator<Item = IntegerType>) -> Self {
        Self {
            pitch_classes: pitch_classes.into_iter().map(wrap).collect(),
        }
    }

    /// Builds a row from the pitch classes of a sequence of pitches.
    pub fn from_pitches<'a>(pitches: impl IntoIterator<Item = &'a Pitch>) -> Self {
        Self {
            pitch_classes: pitches.into_iter().map(root::pitch_class).collect(),
        }
    }

    /// The pitch classes in order.
    pub fn pitch_classes(&self) -> &[u8] {
        &self.pitch_classes
    }

    /// The number of pitch classes in the row.
    pub fn len(&self) -> usize {
        self.pitch_classes.len()
    }

    /// Whether the row has no pitch classes.
    pub fn is_empty(&self) -> bool {
        self.pitch_classes.is_empty()
    }

    /// The row as octave-less pitches, spelled the way music21 spells a pitch
    /// built from a pitch class (`C#`, `E-`, `F#`, `G#`, `B-`).
    pub fn pitches(&self) -> Vec<Pitch> {
        self.pitch_classes
            .iter()
            .map(|&pc| {
                Pitch::from_pitch_class(IntegerType::from(pc))
                    .expect("a pitch class below twelve always spells")
            })
            .collect()
    }

    /// The note names of the row, music21's `noteNames`.
    pub fn note_names(&self) -> Vec<String> {
        self.pitches().iter().map(Pitch::name).collect()
    }

    /// Whether the row is a permutation of all twelve pitch classes.
    pub fn is_twelve_tone_row(&self) -> bool {
        self.pitch_classes.len() == 12 && (0..12u8).all(|pc| self.pitch_classes.contains(&pc))
    }

    /// The row as a twelve-tone row: music21's `makeTwelveToneRow`. There is
    /// one row type here, so this is the row itself, and a row that is not a
    /// permutation of the twelve pitch classes is an error rather than a row
    /// whose twelve-tone questions fail one by one.
    pub fn make_twelve_tone_row(&self) -> Result<ToneRow> {
        if !self.is_twelve_tone_row() {
            return Err(Error::Serial(
                "A twelve-tone row must contain each pitch class exactly once".to_string(),
            ));
        }
        Ok(self.clone())
    }

    /// Whether two rows have the same pitch classes in the same order.
    pub fn is_same_row(&self, other: &ToneRow) -> bool {
        self == other
    }

    /// The intervals between consecutive pitch classes as one character each,
    /// with `T` for ten and `E` for eleven: music21's `getIntervalsAsString`.
    pub fn intervals_as_string(&self) -> String {
        self.pitch_classes
            .windows(2)
            .map(|pair| {
                let interval = wrap(IntegerType::from(pair[1]) - IntegerType::from(pair[0]));
                match interval {
                    10 => 'T',
                    11 => 'E',
                    digit => char::from(b'0' + digit),
                }
            })
            .collect()
    }

    /// Transforms the row in the zero-centered convention, where `P(n)` and
    /// `I(n)` start on pitch class `n` and `R(n)` and `RI(n)` end on it.
    pub fn zero_centered_transformation(
        &self,
        transformation: Transformation,
        index: IntegerType,
    ) -> ToneRow {
        let Some(&first) = self.pitch_classes.first() else {
            return ToneRow::default();
        };
        let first = IntegerType::from(first);
        let forward = self.pitch_classes.iter().map(|&pc| IntegerType::from(pc));
        let backward = self
            .pitch_classes
            .iter()
            .rev()
            .map(|&pc| IntegerType::from(pc));
        let transformed: Vec<IntegerType> = match transformation {
            Transformation::Prime => forward.map(|pc| pc - first + index).collect(),
            Transformation::Inversion => forward.map(|pc| index + first - pc).collect(),
            Transformation::Retrograde => backward.map(|pc| index + pc - first).collect(),
            Transformation::RetrogradeInversion => backward.map(|pc| index - pc + first).collect(),
        };
        ToneRow::new(transformed)
    }

    /// Transforms the row in the original-centered convention, where `T(n)`
    /// transposes the row up `n` semitones and the other transformations act
    /// in place before transposing by `n`.
    pub fn original_centered_transformation(
        &self,
        transformation: Transformation,
        index: IntegerType,
    ) -> ToneRow {
        let Some(&first) = self.pitch_classes.first() else {
            return ToneRow::default();
        };
        self.zero_centered_transformation(transformation, IntegerType::from(first) + index)
    }

    /// Transforms the row under either convention.
    pub fn transformation(
        &self,
        convention: TransformationConvention,
        transformation: Transformation,
        index: IntegerType,
    ) -> ToneRow {
        match convention {
            TransformationConvention::ZeroCentered => {
                self.zero_centered_transformation(transformation, index)
            }
            TransformationConvention::OriginalCentered => {
                self.original_centered_transformation(transformation, index)
            }
        }
    }

    /// The zero-centered transformations that take this row to `other`, in
    /// music21's order `P`, `I`, `R`, `RI`.
    pub fn find_zero_centered_transformations(
        &self,
        other: &ToneRow,
    ) -> Vec<IndexedTransformation> {
        let (Some(&first), Some(&last)) = (other.pitch_classes.first(), other.pitch_classes.last())
        else {
            return Vec::new();
        };
        if self.len() != other.len() {
            return Vec::new();
        }
        let candidates = [
            (Transformation::Prime, first),
            (Transformation::Inversion, first),
            (Transformation::Retrograde, last),
            (Transformation::RetrogradeInversion, last),
        ];
        candidates
            .into_iter()
            .filter(|&(transformation, index)| {
                self.zero_centered_transformation(transformation, IntegerType::from(index))
                    == *other
            })
            .collect()
    }

    /// The original-centered transformations that take this row to `other`,
    /// in music21's order `T`, `I`, `R`, `RI`.
    pub fn find_original_centered_transformations(
        &self,
        other: &ToneRow,
    ) -> Vec<IndexedTransformation> {
        let (Some(&old_first), Some(&old_last), Some(&new_first)) = (
            self.pitch_classes.first(),
            self.pitch_classes.last(),
            other.pitch_classes.first(),
        ) else {
            return Vec::new();
        };
        if self.len() != other.len() {
            return Vec::new();
        }
        let (old_first, old_last, new_first) = (
            IntegerType::from(old_first),
            IntegerType::from(old_last),
            IntegerType::from(new_first),
        );
        let transposition = wrap(new_first - old_first);
        let retrograde = wrap(new_first - old_last);
        let retrograde_inversion = wrap(new_first - 2 * old_first + old_last);
        let candidates = [
            (Transformation::Prime, transposition),
            (Transformation::Inversion, transposition),
            (Transformation::Retrograde, retrograde),
            (Transformation::RetrogradeInversion, retrograde_inversion),
        ];
        candidates
            .into_iter()
            .filter(|&(transformation, index)| {
                self.original_centered_transformation(transformation, IntegerType::from(index))
                    == *other
            })
            .collect()
    }

    /// The transformations that take this row to `other` under either
    /// convention.
    pub fn find_transformations(
        &self,
        convention: TransformationConvention,
        other: &ToneRow,
    ) -> Vec<IndexedTransformation> {
        match convention {
            TransformationConvention::ZeroCentered => {
                self.find_zero_centered_transformations(other)
            }
            TransformationConvention::OriginalCentered => {
                self.find_original_centered_transformations(other)
            }
        }
    }

    /// The row's matrix: every transposition of the row, ordered so that the
    /// leading diagonal is zero and each column reads the inversion.
    pub fn matrix(&self) -> TwelveToneMatrix {
        let rows = self
            .pitch_classes
            .iter()
            .map(|&pc| {
                let transposition = wrap(12 - IntegerType::from(pc));
                ToneRow::new(
                    self.pitch_classes
                        .iter()
                        .map(|&x| IntegerType::from(x) + IntegerType::from(transposition)),
                )
            })
            .collect();
        TwelveToneMatrix { rows }
    }

    /// The historical rows identical to this one.
    pub fn find_historical(&self) -> Vec<&'static HistoricalRow> {
        HISTORICAL_ROWS
            .iter()
            .filter(|historical| historical.pitch_classes == self.pitch_classes.as_slice())
            .collect()
    }

    /// The historical rows of which this row is a transformation, each with
    /// the transformations taking the historical row to this one.
    pub fn find_transformed_historical(
        &self,
        convention: TransformationConvention,
    ) -> Vec<(&'static HistoricalRow, Vec<IndexedTransformation>)> {
        HISTORICAL_ROWS
            .iter()
            .filter_map(|historical| {
                let transformations = historical.row().find_transformations(convention, self);
                (!transformations.is_empty()).then_some((historical, transformations))
            })
            .collect()
    }

    fn require_twelve_tone(&self, purpose: &str) -> Result<()> {
        if self.is_twelve_tone_row() {
            Ok(())
        } else {
            Err(Error::Serial(format!(
                "{purpose} must be a twelve-tone row."
            )))
        }
    }

    /// Whether every interval class from one to eleven occurs between
    /// consecutive members of the row.
    ///
    /// Errors unless the row is a twelve-tone row.
    pub fn is_all_interval(&self) -> Result<bool> {
        self.require_twelve_tone("An all-interval row")?;
        let intervals = self.intervals_as_string();
        Ok("123456789TE"
            .chars()
            .all(|interval| intervals.contains(interval)))
    }

    /// The Link chord classification of the row, if it is one: an
    /// all-interval row containing a voicing of the all-trichord hexachord
    /// `[0, 1, 2, 4, 7, 8]`, numbered as in John Link's catalogue.
    ///
    /// Errors unless the row is a twelve-tone row.
    pub fn link_classification(&self) -> Result<Option<LinkClassification>> {
        self.require_twelve_tone("A Link Chord")?;
        let forms = [
            self.clone(),
            self.zero_centered_transformation(Transformation::Inversion, 0),
            self.zero_centered_transformation(Transformation::Retrograde, 0),
            self.zero_centered_transformation(Transformation::RetrogradeInversion, 0),
        ];
        let mut number = None;
        let mut special_intervals = Vec::new();
        for form in &forms {
            let intervals = form.intervals_as_string();
            for link in LINK_CHORDS
                .iter()
                .filter(|link| link.intervals == intervals)
            {
                number = Some(link.classification);
                special_intervals.push(link.special_intervals);
            }
        }
        Ok(number.map(|number| LinkClassification {
            number,
            special_intervals,
        }))
    }

    /// Whether the row is a Link chord. Errors unless it is a twelve-tone row.
    pub fn is_link_chord(&self) -> Result<bool> {
        Ok(self.link_classification()?.is_some())
    }

    /// Whether two zero-centered transformations of the row are
    /// hexachordally combinatorial: their first hexachords together form the
    /// aggregate.
    ///
    /// Errors unless the row is a twelve-tone row.
    pub fn are_combinatorial(
        &self,
        first: Transformation,
        first_index: IntegerType,
        second: Transformation,
        second_index: IntegerType,
    ) -> Result<bool> {
        self.require_twelve_tone("Combinatoriality applies only to twelve-tone rows; this")?;
        let first = self.zero_centered_transformation(first, first_index);
        let second = self.zero_centered_transformation(second, second_index);
        let combined = first.pitch_classes[..6]
            .iter()
            .chain(&second.pitch_classes[..6])
            .map(|&pc| IntegerType::from(pc));
        Ok(ToneRow::new(combined).is_twelve_tone_row())
    }
}

impl Index<usize> for ToneRow {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.pitch_classes[index]
    }
}

impl<'a> IntoIterator for &'a ToneRow {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.pitch_classes.iter()
    }
}

impl fmt::Display for ToneRow {
    /// The pitch classes as single characters, `A` and `B` for ten and eleven.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &pc in &self.pitch_classes {
            f.write_str(&convert_pitch_class_to_str(IntegerType::from(pc)))?;
        }
        Ok(())
    }
}

impl From<Vec<u8>> for ToneRow {
    fn from(pitch_classes: Vec<u8>) -> Self {
        ToneRow::new(pitch_classes.into_iter().map(IntegerType::from))
    }
}

impl<const N: usize> From<[u8; N]> for ToneRow {
    fn from(pitch_classes: [u8; N]) -> Self {
        ToneRow::new(pitch_classes.into_iter().map(IntegerType::from))
    }
}

/// The Link chord number of a row and the interval sets that voice the
/// all-trichord hexachord within it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkClassification {
    /// The classification number, 1 to 194.
    pub number: UnsignedIntegerType,
    /// The five-interval strings within the row (or a transformation of it)
    /// that voice the all-trichord hexachord.
    pub special_intervals: Vec<&'static str>,
}

/// The transpositions of a row laid out as a matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct TwelveToneMatrix {
    rows: Vec<ToneRow>,
}

impl TwelveToneMatrix {
    /// The rows of the matrix, top to bottom.
    pub fn rows(&self) -> &[ToneRow] {
        &self.rows
    }

    /// One row of the matrix.
    pub fn row(&self, index: usize) -> Option<&ToneRow> {
        self.rows.get(index)
    }
}

impl fmt::Display for TwelveToneMatrix {
    /// music21's layout: each pitch class right-aligned in three columns,
    /// with `A` and `B` for ten and eleven.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, row) in self.rows.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            for &pc in row.pitch_classes() {
                write!(
                    f,
                    "{:>3}",
                    convert_pitch_class_to_str(IntegerType::from(pc))
                )?;
            }
        }
        Ok(())
    }
}

/// music21's `rowToMatrix`: the matrix of a pitch-class list as text, with
/// the pitch classes written in decimal rather than as `A` and `B`.
pub fn row_to_matrix(pitch_classes: &[IntegerType]) -> String {
    ToneRow::new(pitch_classes.iter().copied())
        .matrix()
        .rows()
        .iter()
        .map(|row| {
            row.pitch_classes()
                .iter()
                .map(|pc| format!("{pc:>3}"))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A twelve-tone row from the historical literature, with the attributes
/// music21 stores for it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct HistoricalRow {
    /// music21's key for the row, such as `SchoenbergOp37`.
    pub name: &'static str,
    /// The composer's surname.
    pub composer: &'static str,
    /// The opus, where the work has one.
    pub opus: Option<&'static str>,
    /// The title of the work, or of the row within it.
    pub title: &'static str,
    /// The row's pitch classes.
    pub pitch_classes: [u8; 12],
}

impl HistoricalRow {
    /// The row as a [`ToneRow`].
    pub fn row(&self) -> ToneRow {
        ToneRow::from(self.pitch_classes)
    }
}

/// Looks a historical row up by music21's name for it. The pre-v6 names with a
/// `Row` prefix are accepted too.
pub fn historical_row_by_name(name: &str) -> Result<&'static HistoricalRow> {
    let name = name.strip_prefix("Row").unwrap_or(name);
    HISTORICAL_ROWS
        .iter()
        .find(|row| row.name == name)
        .ok_or_else(|| Error::Serial("No historical row with given name found".to_string()))
}

struct LinkChord {
    intervals: &'static str,
    special_intervals: &'static str,
    classification: UnsignedIntegerType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_permutation_of_the_twelve_classes_makes_a_twelve_tone_row() {
        assert!(ToneRow::new(0..11).make_twelve_tone_row().is_err());
        let row = ToneRow::new(0..12).make_twelve_tone_row().unwrap();
        assert_eq!(row.pitch_classes().len(), 12);
    }

    #[test]
    fn a_row_indexes_and_iterates_over_its_pitch_classes() {
        let row = ToneRow::new([0, 13, -1]);

        assert_eq!(row[1], 1);
        let collected: Vec<u8> = (&row).into_iter().copied().collect();
        assert_eq!(collected, vec![0, 1, 11]);
    }

    fn row(pitch_classes: impl IntoIterator<Item = IntegerType>) -> ToneRow {
        ToneRow::new(pitch_classes)
    }

    fn chromatic() -> ToneRow {
        row(0..12)
    }

    fn historical(name: &str) -> ToneRow {
        historical_row_by_name(name).unwrap().row()
    }

    fn labelled(
        transformations: &[IndexedTransformation],
        convention: TransformationConvention,
    ) -> Vec<(&'static str, u8)> {
        transformations
            .iter()
            .map(|&(transformation, index)| (transformation.label(convention), index))
            .collect()
    }

    #[test]
    fn pitch_classes_are_reduced_modulo_twelve() {
        let quintuple = row((0..12).map(|i| 5 * i));
        assert_eq!(
            quintuple.pitch_classes(),
            &[0, 5, 10, 3, 8, 1, 6, 11, 4, 9, 2, 7]
        );
        assert_eq!(quintuple.to_string(), "05A3816B4927");
        assert_eq!(row([-6, 19, 128]).pitch_classes(), &[6, 7, 8]);
        assert_eq!(row([-6, 19, 128]).note_names(), ["F#", "G", "G#"]);
    }

    #[test]
    fn note_names_spell_pitch_classes_as_music21_does() {
        assert_eq!(
            chromatic().note_names(),
            [
                "C", "C#", "D", "E-", "E", "F", "F#", "G", "G#", "A", "B-", "B"
            ]
        );
    }

    #[test]
    fn twelve_tone_row_needs_all_twelve_pitch_classes_once() {
        assert!(chromatic().is_twelve_tone_row());
        assert!(!row([0, 4, 8]).is_twelve_tone_row());
        assert!(!row([3; 12]).is_twelve_tone_row());
        assert!(!ToneRow::default().is_twelve_tone_row());
    }

    #[test]
    fn same_row_compares_pitch_classes_in_order() {
        assert!(row([6, 7, 8]).is_same_row(&row([-6, 19, 128])));
        assert!(!row([6, 7, 8]).is_same_row(&row([6, 7, -8])));
        assert!(!row([6, 7, 8]).is_same_row(&row([6, 7])));
    }

    #[test]
    fn intervals_as_string_uses_t_and_e() {
        assert_eq!(row([0]).intervals_as_string(), "");
        assert_eq!(ToneRow::default().intervals_as_string(), "");
        assert_eq!(row((0..12).rev()).intervals_as_string(), "EEEEEEEEEEE");
        assert_eq!(
            historical("BergLyricSuite").intervals_as_string(),
            "E89T7652341"
        );
    }

    #[test]
    fn zero_centered_transformations_match_music21() {
        assert_eq!(
            chromatic()
                .zero_centered_transformation(Transformation::Prime, 3)
                .pitch_classes(),
            &[3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 1, 2]
        );
        assert_eq!(
            chromatic()
                .zero_centered_transformation(Transformation::Inversion, 6)
                .pitch_classes(),
            &[6, 5, 4, 3, 2, 1, 0, 11, 10, 9, 8, 7]
        );
        let schoenberg = historical("SchoenbergOp26");
        assert_eq!(
            schoenberg.pitch_classes(),
            &[3, 7, 9, 11, 1, 0, 10, 2, 4, 6, 8, 5]
        );
        assert_eq!(
            schoenberg
                .zero_centered_transformation(Transformation::Retrograde, 8)
                .pitch_classes(),
            &[10, 1, 11, 9, 7, 3, 5, 6, 4, 2, 0, 8]
        );
        assert_eq!(
            schoenberg
                .zero_centered_transformation(Transformation::RetrogradeInversion, 9)
                .note_names(),
            [
                "G", "E", "F#", "G#", "B-", "D", "C", "B", "C#", "E-", "F", "A"
            ]
        );
    }

    #[test]
    fn original_centered_transformations_match_music21() {
        assert_eq!(
            chromatic()
                .original_centered_transformation(Transformation::Prime, 3)
                .pitch_classes(),
            &[3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 1, 2]
        );
        assert_eq!(
            chromatic()
                .original_centered_transformation(Transformation::Inversion, 6)
                .pitch_classes(),
            &[6, 5, 4, 3, 2, 1, 0, 11, 10, 9, 8, 7]
        );
        let schoenberg = historical("SchoenbergOp26");
        assert_eq!(
            schoenberg
                .original_centered_transformation(Transformation::Retrograde, 8)
                .pitch_classes(),
            &[1, 4, 2, 0, 10, 6, 8, 9, 7, 5, 3, 11]
        );
        assert_eq!(
            schoenberg
                .original_centered_transformation(Transformation::RetrogradeInversion, 9)
                .note_names(),
            [
                "B-", "G", "A", "B", "C#", "F", "E-", "D", "E", "F#", "G#", "C"
            ]
        );
    }

    #[test]
    fn transforming_an_empty_row_gives_an_empty_row() {
        let empty = ToneRow::default();
        assert!(
            empty
                .zero_centered_transformation(Transformation::Retrograde, 3)
                .is_empty()
        );
        assert!(
            empty
                .original_centered_transformation(Transformation::Inversion, 3)
                .is_empty()
        );
        assert!(empty.find_zero_centered_transformations(&empty).is_empty());
        assert!(
            empty
                .find_original_centered_transformations(&empty)
                .is_empty()
        );
    }

    #[test]
    fn finds_zero_centered_transformations_between_rows() {
        let zero = TransformationConvention::ZeroCentered;
        let rising = row([2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 1]);
        let falling = row([8, 7, 6, 5, 4, 3, 2, 1, 0, 11, 10, 9]);
        assert_eq!(
            labelled(&rising.find_zero_centered_transformations(&falling), zero),
            [("I", 8), ("R", 9)]
        );
        let op25 = historical("SchoenbergOp25");
        let op26 = historical("SchoenbergOp26");
        assert!(op25.find_zero_centered_transformations(&op26).is_empty());
        let ri8 = op26.zero_centered_transformation(Transformation::RetrogradeInversion, 8);
        assert_eq!(
            labelled(&op26.find_zero_centered_transformations(&ri8), zero),
            [("RI", 8)]
        );
        assert!(
            op26.find_zero_centered_transformations(&row([0]))
                .is_empty()
        );
    }

    #[test]
    fn finds_original_centered_transformations_between_rows() {
        let original = TransformationConvention::OriginalCentered;
        let rising = row([2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 1]);
        let falling = row([8, 7, 6, 5, 4, 3, 2, 1, 0, 11, 10, 9]);
        assert_eq!(
            labelled(
                &rising.find_original_centered_transformations(&falling),
                original
            ),
            [("I", 6), ("R", 7)]
        );
        let op25 = historical("SchoenbergOp25");
        let op26 = historical("SchoenbergOp26");
        assert!(
            op25.find_original_centered_transformations(&op26)
                .is_empty()
        );
        let ri8 = op26.original_centered_transformation(Transformation::RetrogradeInversion, 8);
        assert_eq!(
            labelled(&op26.find_original_centered_transformations(&ri8), original),
            [("RI", 8)]
        );
        assert_eq!(
            labelled(&rising.find_transformations(original, &falling), original),
            [("I", 6), ("R", 7)]
        );
    }

    #[test]
    fn every_transformation_is_found_again() {
        let source = historical("WebernOp27");
        for convention in [
            TransformationConvention::ZeroCentered,
            TransformationConvention::OriginalCentered,
        ] {
            for transformation in Transformation::ALL {
                for index in 0..12 {
                    let target = source.transformation(convention, transformation, index);
                    let found = source.find_transformations(convention, &target);
                    assert!(
                        found.contains(&(transformation, index as u8)),
                        "{transformation:?} {index} under {convention:?} not found in {found:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn matrix_matches_music21_layout() {
        let matrix = row([0, 2, 11, 7, 8, 3, 9, 1, 4, 10, 6, 5]).matrix();
        let expected = [
            "  0  2  B  7  8  3  9  1  4  A  6  5",
            "  A  0  9  5  6  1  7  B  2  8  4  3",
            "  1  3  0  8  9  4  A  2  5  B  7  6",
            "  5  7  4  0  1  8  2  6  9  3  B  A",
            "  4  6  3  B  0  7  1  5  8  2  A  9",
            "  9  B  8  4  5  0  6  A  1  7  3  2",
            "  3  5  2  A  B  6  0  4  7  1  9  8",
            "  B  1  A  6  7  2  8  0  3  9  5  4",
            "  8  A  7  3  4  B  5  9  0  6  2  1",
            "  2  4  1  9  A  5  B  3  6  0  8  7",
            "  6  8  5  1  2  9  3  7  A  4  0  B",
            "  7  9  6  2  3  A  4  8  B  5  1  0",
        ]
        .join(
            "
",
        );
        assert_eq!(matrix.to_string(), expected);
        assert_eq!(matrix.rows().len(), 12);
        assert_eq!(
            matrix.row(0).unwrap().pitch_classes(),
            &[0, 2, 11, 7, 8, 3, 9, 1, 4, 10, 6, 5]
        );
        assert!(matrix.row(12).is_none());

        let op37 = historical("SchoenbergOp37").matrix();
        assert_eq!(
            op37.row(0).unwrap().note_names(),
            [
                "C", "B", "G", "G#", "E-", "C#", "D", "B-", "F#", "F", "E", "A"
            ]
        );
        assert!(
            op37.to_string()
                .starts_with("  0  B  7  8  3  1  2  A  6  5  4  9\n")
        );
    }

    #[test]
    fn row_to_matrix_writes_decimal_pitch_classes() {
        let text = row_to_matrix(&[0, 2, 11, 7, 8, 3, 9, 1, 4, 10, 6, 5]);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 12);
        assert_eq!(lines[0], "  0  2 11  7  8  3  9  1  4 10  6  5");
        assert_eq!(lines[1], " 10  0  9  5  6  1  7 11  2  8  4  3");
        assert_eq!(lines[11], "  7  9  6  2  3 10  4  8 11  5  1  0");
    }

    #[test]
    fn historical_rows_are_found_by_name_with_or_without_the_old_prefix() {
        let webern = historical_row_by_name("WebernOp29").unwrap();
        assert_eq!(webern.composer, "Webern");
        assert_eq!(webern.opus, Some("Op. 29"));
        assert_eq!(webern.title, "Cantata I");
        assert_eq!(webern.pitch_classes, [3, 11, 2, 1, 5, 4, 7, 6, 10, 9, 0, 8]);
        assert_eq!(historical_row_by_name("RowWebernOp29").unwrap(), webern);
        assert_eq!(
            historical_row_by_name("SchoenbergJakobsleiter")
                .unwrap()
                .opus,
            None
        );
        assert!(matches!(
            historical_row_by_name("Nope"),
            Err(Error::Serial(_))
        ));
    }

    #[test]
    fn every_historical_row_is_a_twelve_tone_row_with_a_unique_name() {
        let mut names: Vec<&str> = HISTORICAL_ROWS.iter().map(|row| row.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), HISTORICAL_ROWS.len());
        for historical in &HISTORICAL_ROWS {
            assert!(
                historical.row().is_twelve_tone_row(),
                "{} is not a twelve-tone row",
                historical.name
            );
        }
    }

    #[test]
    fn finds_historical_rows_and_their_transformations() {
        let names = |rows: Vec<&HistoricalRow>| -> Vec<&str> {
            rows.into_iter().map(|row| row.name).collect()
        };
        assert_eq!(
            names(row([2, 3, 9, 1, 11, 5, 8, 7, 4, 0, 10, 6]).find_historical()),
            ["SchoenbergOp32"]
        );
        assert!(chromatic().find_historical().is_empty());

        let transformed = row([5, 9, 11, 3, 6, 7, 4, 10, 0, 8, 2, 1]);
        let original =
            transformed.find_transformed_historical(TransformationConvention::OriginalCentered);
        assert_eq!(original.len(), 1);
        assert_eq!(original[0].0.name, "SchoenbergOp32");
        assert_eq!(
            labelled(&original[0].1, TransformationConvention::OriginalCentered),
            [("R", 11)]
        );
        let zero = transformed.find_transformed_historical(TransformationConvention::ZeroCentered);
        assert_eq!(zero[0].0.name, "SchoenbergOp32");
        assert_eq!(
            labelled(&zero[0].1, TransformationConvention::ZeroCentered),
            [("R", 1)]
        );
    }

    #[test]
    fn all_interval_rows() {
        assert!(!chromatic().is_all_interval().unwrap());
        let berg = historical("BergLyricSuite");
        assert_eq!(
            berg.pitch_classes(),
            &[5, 4, 0, 9, 7, 2, 8, 1, 3, 6, 10, 11]
        );
        assert!(berg.is_all_interval().unwrap());
        assert!(matches!(
            row([0, 4, 8]).is_all_interval(),
            Err(Error::Serial(_))
        ));
    }

    #[test]
    fn link_chords_match_music21() {
        let berg = historical("BergLyricSuite");
        assert_eq!(berg.link_classification().unwrap(), None);
        assert!(!berg.is_link_chord().unwrap());

        let link = row([0, 3, 8, 2, 10, 11, 9, 4, 1, 5, 7, 6]);
        assert_eq!(
            link.link_classification().unwrap(),
            Some(LinkClassification {
                number: 62,
                special_intervals: vec!["8352E"],
            })
        );
        assert!(link.is_link_chord().unwrap());

        let double = row([0, 1, 8, 5, 7, 10, 4, 3, 11, 9, 2, 6]);
        assert_eq!(
            double.link_classification().unwrap(),
            Some(LinkClassification {
                number: 33,
                special_intervals: vec!["236E8", "36E8T"],
            })
        );
        assert!(matches!(
            row([0, 4, 8]).is_link_chord(),
            Err(Error::Serial(_))
        ));
    }

    #[test]
    fn every_link_chord_interval_string_classifies_as_itself() {
        for link in &LINK_CHORDS {
            let mut pitch_classes = vec![0];
            for interval in link.intervals.chars() {
                let step = match interval {
                    'T' => 10,
                    'E' => 11,
                    digit => IntegerType::from(digit.to_digit(10).unwrap() as u8),
                };
                pitch_classes.push(pitch_classes.last().unwrap() + step);
            }
            let classified = row(pitch_classes).link_classification().unwrap().unwrap();
            assert_eq!(classified.number, link.classification, "{}", link.intervals);
            assert!(
                classified
                    .special_intervals
                    .contains(&link.special_intervals)
            );
        }
    }

    #[test]
    fn combinatoriality_matches_music21() {
        let moses = historical("SchoenbergMosesAron");
        assert_eq!(
            moses.pitch_classes(),
            &[9, 10, 4, 2, 3, 1, 7, 5, 6, 8, 11, 0]
        );
        let (p, i, r, ri) = (
            Transformation::Prime,
            Transformation::Inversion,
            Transformation::Retrograde,
            Transformation::RetrogradeInversion,
        );
        assert!(moses.are_combinatorial(p, 0, i, 3).unwrap());
        assert!(moses.are_combinatorial(p, 1, i, 4).unwrap());
        assert!(moses.are_combinatorial(r, 1, ri, 4).unwrap());
        assert!(!moses.are_combinatorial(r, 6, ri, 4).unwrap());
        assert!(matches!(
            row([0, 4, 8]).are_combinatorial(p, 0, i, 3),
            Err(Error::Serial(_))
        ));
    }

    #[test]
    fn transformation_labels_round_trip() {
        for transformation in Transformation::ALL {
            for convention in [
                TransformationConvention::ZeroCentered,
                TransformationConvention::OriginalCentered,
            ] {
                let label = transformation.label(convention);
                assert_eq!(Transformation::from_name(label).unwrap(), transformation);
                assert_eq!(label.parse::<Transformation>().unwrap(), transformation);
            }
        }
        assert_eq!(Transformation::Prime.to_string(), "P");
        assert!(matches!(
            Transformation::from_name("X"),
            Err(Error::Serial(_))
        ));
        assert_eq!(
            TransformationConvention::from_name("zero").unwrap(),
            TransformationConvention::ZeroCentered
        );
        assert!(TransformationConvention::from_name("sideways").is_err());
    }

    #[test]
    fn rows_build_from_pitches_and_arrays() {
        let pitches: Vec<Pitch> = ["C4", "F#3", "B-5"]
            .iter()
            .map(|&name| Pitch::from_name(name).unwrap())
            .collect();
        assert_eq!(ToneRow::from_pitches(&pitches).pitch_classes(), &[0, 6, 10]);
        assert_eq!(ToneRow::from([0u8, 6, 10]).pitch_classes(), &[0, 6, 10]);
        assert_eq!(ToneRow::from(vec![0u8, 6, 10]).len(), 3);
        assert_eq!(
            ToneRow::from([0u8, 6, 10]).pitches()[2].name_with_octave(),
            "B-"
        );
    }
}
