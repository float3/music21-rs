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

/// The historical twelve-tone rows music21 ships, in music21's order.
pub const HISTORICAL_ROWS: [HistoricalRow; 71] = [
    HistoricalRow {
        name: "WebernOp29",
        composer: "Webern",
        opus: Some("Op. 29"),
        title: "Cantata I",
        pitch_classes: [3, 11, 2, 1, 5, 4, 7, 6, 10, 9, 0, 8],
    },
    HistoricalRow {
        name: "WebernOp28",
        composer: "Webern",
        opus: Some("Op. 28"),
        title: "String Quartet",
        pitch_classes: [1, 0, 3, 2, 6, 7, 4, 5, 9, 8, 11, 10],
    },
    HistoricalRow {
        name: "SchoenbergOp24Mvmt5",
        composer: "Schoenberg",
        opus: Some("Op. 24"),
        title: "Serenade, Mvt. 5, \"Tanzscene\"",
        pitch_classes: [9, 10, 0, 3, 4, 6, 5, 7, 8, 11, 1, 2],
    },
    HistoricalRow {
        name: "SchoenbergOp24Mvmt4",
        composer: "Schoenberg",
        opus: Some("Op. 24"),
        title: "Serenade, Mvt. 4, \"Sonett\"",
        pitch_classes: [4, 2, 3, 11, 0, 1, 8, 6, 9, 5, 7, 10],
    },
    HistoricalRow {
        name: "SchoenbergJakobsleiter",
        composer: "Schoenberg",
        opus: None,
        title: "Die Jakobsleiter",
        pitch_classes: [1, 2, 5, 4, 8, 7, 0, 3, 11, 10, 6, 9],
    },
    HistoricalRow {
        name: "SchoenbergOp27No4",
        composer: "Schoenberg",
        opus: Some("Op. 27 No. 4"),
        title: "Four Pieces for Mixed Chorus, No. 4",
        pitch_classes: [1, 3, 10, 6, 8, 4, 11, 0, 2, 9, 5, 7],
    },
    HistoricalRow {
        name: "WebernOp23",
        composer: "Webern",
        opus: Some("Op. 23"),
        title: "Three Songs",
        pitch_classes: [8, 3, 7, 4, 10, 6, 2, 5, 1, 0, 9, 11],
    },
    HistoricalRow {
        name: "BergLuluActIIScene1",
        composer: "Berg",
        opus: Some("Lulu, Act II, Scene 1"),
        title: "Perm. (Every 5th Note Of Transposed Primary Row)",
        pitch_classes: [10, 7, 1, 0, 9, 2, 4, 11, 5, 8, 3, 6],
    },
    HistoricalRow {
        name: "SchoenbergOp27No1",
        composer: "Schoenberg",
        opus: Some("Op. 27 No. 1"),
        title: "Four Pieces for Mixed Chorus, No. 1",
        pitch_classes: [6, 5, 2, 8, 7, 1, 3, 4, 10, 9, 11, 0],
    },
    HistoricalRow {
        name: "BergLuluActIScene20",
        composer: "Berg",
        opus: Some("Lulu, Act I , Scene XX"),
        title: "Perm. (Every 7th Note Of Transposed Primary Row)",
        pitch_classes: [10, 6, 3, 8, 5, 11, 4, 2, 9, 0, 1, 7],
    },
    HistoricalRow {
        name: "SchoenbergOp27No3",
        composer: "Schoenberg",
        opus: Some("Op. 27 No. 3"),
        title: "Four Pieces for Mixed Chorus, No. 3",
        pitch_classes: [7, 6, 2, 4, 5, 3, 11, 0, 8, 10, 9, 1],
    },
    HistoricalRow {
        name: "SchoenbergOp27No2",
        composer: "Schoenberg",
        opus: Some("Op. 27 No. 2"),
        title: "Four Pieces for Mixed Chorus, No. 2",
        pitch_classes: [0, 11, 4, 10, 2, 8, 3, 7, 6, 5, 9, 1],
    },
    HistoricalRow {
        name: "SchoenbergFragPiano",
        composer: "Schoenberg",
        opus: None,
        title: "Fragment For Piano",
        pitch_classes: [6, 9, 0, 7, 1, 2, 8, 11, 5, 10, 4, 3],
    },
    HistoricalRow {
        name: "SchoenbergOp50B",
        composer: "Schoenberg",
        opus: Some("Op. 50B"),
        title: "De Profundis",
        pitch_classes: [3, 9, 8, 4, 2, 10, 7, 11, 0, 6, 5, 1],
    },
    HistoricalRow {
        name: "SchoenbergOp50C",
        composer: "Schoenberg",
        opus: Some("Op. 50C"),
        title: "Modern Psalms, The First Psalm",
        pitch_classes: [4, 3, 0, 8, 11, 7, 5, 9, 6, 10, 1, 2],
    },
    HistoricalRow {
        name: "SchoenbergOp50A",
        composer: "Schoenberg",
        opus: Some("Op. 50A"),
        title: "Three Times A Thousand Years",
        pitch_classes: [7, 9, 6, 4, 5, 11, 10, 2, 0, 1, 3, 8],
    },
    HistoricalRow {
        name: "SchoenbergMosesAron",
        composer: "Schoenberg",
        opus: None,
        title: "Moses And Aron",
        pitch_classes: [9, 10, 4, 2, 3, 1, 7, 5, 6, 8, 11, 0],
    },
    HistoricalRow {
        name: "WebernOp25",
        composer: "Webern",
        opus: Some("Op. 25"),
        title: "Three Songs",
        pitch_classes: [7, 4, 3, 6, 1, 5, 2, 11, 10, 0, 9, 8],
    },
    HistoricalRow {
        name: "SchoenbergOp23No5",
        composer: "Schoenberg",
        opus: Some("Op. 23, No. 5"),
        title: "Five Piano Pieces",
        pitch_classes: [1, 9, 11, 7, 8, 6, 10, 2, 4, 3, 0, 5],
    },
    HistoricalRow {
        name: "SchoenbergOp28No1",
        composer: "Schoenberg",
        opus: Some("Op. 28 No. 1"),
        title: "Three Satires for Mixed Chorus, No. 1",
        pitch_classes: [0, 4, 7, 1, 9, 11, 5, 3, 2, 6, 8, 10],
    },
    HistoricalRow {
        name: "SchoenbergOp28No3",
        composer: "Schoenberg",
        opus: Some("Op. 28 No. 3"),
        title: "Three Satires for Mixed Chorus, No. 3",
        pitch_classes: [5, 6, 4, 8, 2, 10, 7, 9, 3, 11, 1, 0],
    },
    HistoricalRow {
        name: "WebernOp21",
        composer: "Webern",
        opus: Some("Op. 21"),
        title: "Chamber Symphony",
        pitch_classes: [5, 8, 7, 6, 10, 9, 3, 4, 0, 1, 2, 11],
    },
    HistoricalRow {
        name: "SchoenbergIsraelExists",
        composer: "Schoenberg",
        opus: None,
        title: "Israel Exists Again",
        pitch_classes: [0, 3, 4, 9, 11, 5, 2, 1, 10, 8, 6, 7],
    },
    HistoricalRow {
        name: "SchoenbergOp35No2",
        composer: "Schoenberg",
        opus: Some("Op. 35"),
        title: "Six Pieces for Male Chorus, No. 2",
        pitch_classes: [6, 9, 7, 1, 0, 2, 5, 11, 10, 3, 4, 8],
    },
    HistoricalRow {
        name: "SchoenbergOp35No3",
        composer: "Schoenberg",
        opus: Some("Op. 35"),
        title: "Six Pieces for Male Chorus, No. 3",
        pitch_classes: [3, 6, 7, 8, 5, 0, 9, 10, 4, 11, 2, 1],
    },
    HistoricalRow {
        name: "SchoenbergOp35No1",
        composer: "Schoenberg",
        opus: Some("Op. 35"),
        title: "Six Pieces for Male Chorus, No. 1",
        pitch_classes: [2, 11, 3, 5, 4, 1, 8, 10, 9, 6, 0, 7],
    },
    HistoricalRow {
        name: "SchoenbergOp48No1",
        composer: "Schoenberg",
        opus: Some("Op. 48"),
        title: "Three Songs, No. 1, \"Sommermud\"",
        pitch_classes: [1, 2, 0, 6, 3, 5, 4, 10, 11, 7, 9, 8],
    },
    HistoricalRow {
        name: "SchoenbergOp35No5",
        composer: "Schoenberg",
        opus: Some("Op. 35"),
        title: "Six Pieces for Male Chorus, No. 5",
        pitch_classes: [1, 7, 10, 2, 3, 11, 8, 4, 0, 6, 5, 9],
    },
    HistoricalRow {
        name: "SchoenbergOp29",
        composer: "Schoenberg",
        opus: Some("Op. 29"),
        title: "Suite",
        pitch_classes: [3, 7, 6, 10, 2, 11, 0, 9, 8, 4, 5, 1],
    },
    HistoricalRow {
        name: "BergLyricSuitePerm",
        composer: "Berg",
        opus: None,
        title: "Lyric Suite, Last Mvt. Permutation",
        pitch_classes: [5, 6, 10, 4, 1, 9, 2, 8, 7, 3, 0, 11],
    },
    HistoricalRow {
        name: "WebernOp20",
        composer: "Webern",
        opus: Some("Op. 20"),
        title: "String Trio",
        pitch_classes: [8, 7, 2, 1, 6, 5, 9, 10, 3, 4, 0, 11],
    },
    HistoricalRow {
        name: "SchoenbergOp46",
        composer: "Schoenberg",
        opus: Some("Op. 46"),
        title: "A Survivor From Warsaw",
        pitch_classes: [6, 7, 0, 8, 4, 3, 11, 10, 5, 9, 1, 2],
    },
    HistoricalRow {
        name: "SchoenbergFragOrganSonata",
        composer: "Schoenberg",
        opus: None,
        title: "Fragment of Sonata For Organ",
        pitch_classes: [1, 7, 11, 3, 9, 2, 8, 6, 10, 5, 0, 4],
    },
    HistoricalRow {
        name: "SchoenbergOp44",
        composer: "Schoenberg",
        opus: Some("Op. 44"),
        title: "Prelude To A Suite From \"Genesis\"",
        pitch_classes: [10, 6, 2, 5, 4, 0, 11, 8, 1, 3, 9, 7],
    },
    HistoricalRow {
        name: "SchoenbergOp45",
        composer: "Schoenberg",
        opus: Some("Op. 45"),
        title: "String Trio",
        pitch_classes: [2, 10, 3, 9, 4, 1, 11, 8, 6, 7, 5, 0],
    },
    HistoricalRow {
        name: "SchoenbergOp33A",
        composer: "Schoenberg",
        opus: Some("Op. 33A"),
        title: "Two Piano Pieces, No. 1",
        pitch_classes: [10, 5, 0, 11, 9, 6, 1, 3, 7, 8, 2, 4],
    },
    HistoricalRow {
        name: "SchoenbergOp25",
        composer: "Schoenberg",
        opus: Some("Op.25"),
        title: "Suite for Piano",
        pitch_classes: [4, 5, 7, 1, 6, 3, 8, 2, 11, 0, 9, 10],
    },
    HistoricalRow {
        name: "SchoenbergOp26",
        composer: "Schoenberg",
        opus: Some("Op. 26"),
        title: "Wind Quintet",
        pitch_classes: [3, 7, 9, 11, 1, 0, 10, 2, 4, 6, 8, 5],
    },
    HistoricalRow {
        name: "SchoenbergOp33B",
        composer: "Schoenberg",
        opus: Some("Op. 33B"),
        title: "Two Piano Pieces, No. 2",
        pitch_classes: [11, 1, 5, 3, 9, 8, 6, 10, 7, 4, 0, 2],
    },
    HistoricalRow {
        name: "BergViolinConcerto",
        composer: "Berg",
        opus: None,
        title: "Concerto For Violin And Orchestra",
        pitch_classes: [7, 10, 2, 6, 9, 0, 4, 8, 11, 1, 3, 5],
    },
    HistoricalRow {
        name: "WebernOp22",
        composer: "Webern",
        opus: Some("Op. 22"),
        title: "Quartet For Violin, Clarinet, Tenor Sax, And Piano",
        pitch_classes: [6, 3, 2, 5, 4, 8, 9, 10, 11, 1, 7, 0],
    },
    HistoricalRow {
        name: "BergLulu",
        composer: "Berg",
        opus: None,
        title: "Lulu: Primary Row",
        pitch_classes: [0, 4, 5, 2, 7, 9, 6, 8, 11, 10, 3, 1],
    },
    HistoricalRow {
        name: "WebernOp30",
        composer: "Webern",
        opus: Some("Op. 30"),
        title: "Variations For Orchestra",
        pitch_classes: [9, 10, 1, 0, 11, 2, 3, 6, 5, 4, 7, 8],
    },
    HistoricalRow {
        name: "WebernOp31",
        composer: "Webern",
        opus: Some("Op. 31"),
        title: "Cantata II",
        pitch_classes: [6, 9, 5, 4, 8, 3, 7, 11, 10, 2, 1, 0],
    },
    HistoricalRow {
        name: "WebernOpNo17No1",
        composer: "Webern",
        opus: Some("Op. 17, No. 1"),
        title: "\"Armer Sunder, Du\"",
        pitch_classes: [11, 10, 5, 6, 3, 4, 7, 8, 9, 0, 1, 2],
    },
    HistoricalRow {
        name: "WebernOp24",
        composer: "Webern",
        opus: Some("Op. 24"),
        title: "Concerto For Nine Instruments",
        pitch_classes: [11, 10, 2, 3, 7, 6, 8, 4, 5, 0, 1, 9],
    },
    HistoricalRow {
        name: "SchoenbergOp48No2",
        composer: "Schoenberg",
        opus: Some("Op. 48"),
        title: "Three Songs, No. 2, \"Tot\"",
        pitch_classes: [2, 3, 9, 1, 10, 4, 8, 7, 0, 11, 5, 6],
    },
    HistoricalRow {
        name: "WebernOp27",
        composer: "Webern",
        opus: Some("Op. 27"),
        title: "Variations For Piano",
        pitch_classes: [3, 11, 10, 2, 1, 0, 6, 4, 7, 5, 9, 8],
    },
    HistoricalRow {
        name: "SchoenbergOp47",
        composer: "Schoenberg",
        opus: Some("Op. 47"),
        title: "Fantasy For Violin And Piano",
        pitch_classes: [10, 9, 1, 11, 5, 7, 3, 4, 0, 2, 8, 6],
    },
    HistoricalRow {
        name: "WebernOp19No2",
        composer: "Webern",
        opus: Some("Op. 19, No. 2"),
        title: "\"Ziehn Die Schafe\"",
        pitch_classes: [8, 4, 9, 6, 7, 0, 11, 5, 3, 2, 10, 1],
    },
    HistoricalRow {
        name: "WebernOp19No1",
        composer: "Webern",
        opus: Some("Op. 19, No. 1"),
        title: "\"Weiss Wie Lilien\"",
        pitch_classes: [7, 10, 6, 5, 3, 9, 8, 1, 2, 11, 4, 0],
    },
    HistoricalRow {
        name: "WebernOp26",
        composer: "Webern",
        opus: Some("Op. 26"),
        title: "Das Augenlicht",
        pitch_classes: [8, 10, 9, 0, 11, 3, 4, 1, 5, 2, 6, 7],
    },
    HistoricalRow {
        name: "SchoenbergFragPianoPhantasia",
        composer: "Schoenberg",
        opus: None,
        title: "Fragment of Phantasia For Piano",
        pitch_classes: [1, 5, 3, 6, 4, 8, 0, 11, 2, 9, 10, 7],
    },
    HistoricalRow {
        name: "BergDerWein",
        composer: "Berg",
        opus: None,
        title: "Der Wein",
        pitch_classes: [2, 4, 5, 7, 9, 10, 1, 6, 8, 0, 11, 3],
    },
    HistoricalRow {
        name: "BergWozzeckPassacaglia",
        composer: "Berg",
        opus: None,
        title: "Wozzeck, Act I, Scene 4 \"Passacaglia\"",
        pitch_classes: [3, 11, 7, 1, 0, 6, 4, 10, 9, 5, 8, 2],
    },
    HistoricalRow {
        name: "WebernOp18No1",
        composer: "Webern",
        opus: Some("Op. 18, No. 1"),
        title: "\"Schatzerl Klein\"",
        pitch_classes: [0, 11, 5, 8, 10, 9, 3, 4, 1, 7, 2, 6],
    },
    HistoricalRow {
        name: "WebernOp18No2",
        composer: "Webern",
        opus: Some("Op. 18, No. 2"),
        title: "\"Erlosung\"",
        pitch_classes: [6, 9, 5, 8, 4, 7, 3, 11, 2, 10, 1, 0],
    },
    HistoricalRow {
        name: "WebernOp18No3",
        composer: "Webern",
        opus: Some("Op. 18, No. 3"),
        title: "\"Ave, Regina Coelorum\"",
        pitch_classes: [4, 3, 7, 6, 5, 11, 10, 2, 1, 0, 9, 8],
    },
    HistoricalRow {
        name: "SchoenbergOp42",
        composer: "Schoenberg",
        opus: Some("Op. 42"),
        title: "Concerto For Piano And Orchestra",
        pitch_classes: [3, 10, 2, 5, 4, 0, 6, 8, 1, 9, 11, 7],
    },
    HistoricalRow {
        name: "SchoenbergOp48No3",
        composer: "Schoenberg",
        opus: Some("Op. 48"),
        title: "Three Songs, No, 3, \"Madchenlied\"",
        pitch_classes: [1, 7, 9, 11, 3, 5, 10, 6, 4, 0, 8, 2],
    },
    HistoricalRow {
        name: "SchoenbergOp37",
        composer: "Schoenberg",
        opus: Some("Op. 37"),
        title: "Fourth String Quartet",
        pitch_classes: [2, 1, 9, 10, 5, 3, 4, 0, 8, 7, 6, 11],
    },
    HistoricalRow {
        name: "SchoenbergOp36",
        composer: "Schoenberg",
        opus: Some("Op. 36"),
        title: "Concerto for Violin and Orchestra",
        pitch_classes: [9, 10, 3, 11, 4, 6, 0, 1, 7, 8, 2, 5],
    },
    HistoricalRow {
        name: "SchoenbergOp34",
        composer: "Schoenberg",
        opus: Some("Op. 34"),
        title: "Accompaniment to a Film Scene",
        pitch_classes: [3, 6, 2, 4, 1, 0, 9, 11, 10, 8, 5, 7],
    },
    HistoricalRow {
        name: "BergChamberConcerto",
        composer: "Berg",
        opus: None,
        title: "Chamber Concerto",
        pitch_classes: [11, 7, 5, 9, 2, 3, 6, 8, 0, 1, 4, 10],
    },
    HistoricalRow {
        name: "SchoenbergOp32",
        composer: "Schoenberg",
        opus: Some("Op. 32"),
        title: "Von Heute Auf Morgen",
        pitch_classes: [2, 3, 9, 1, 11, 5, 8, 7, 4, 0, 10, 6],
    },
    HistoricalRow {
        name: "SchoenbergOp31",
        composer: "Schoenberg",
        opus: Some("Op. 31"),
        title: "Variations for Orchestra",
        pitch_classes: [10, 4, 6, 3, 5, 9, 2, 1, 7, 8, 11, 0],
    },
    HistoricalRow {
        name: "SchoenbergOp30",
        composer: "Schoenberg",
        opus: Some("Op. 30"),
        title: "Third String Quartet",
        pitch_classes: [7, 4, 3, 9, 0, 5, 6, 11, 10, 1, 8, 2],
    },
    HistoricalRow {
        name: "BergLyricSuite",
        composer: "Berg",
        opus: None,
        title: "Lyric Suite Primary Row",
        pitch_classes: [5, 4, 0, 9, 7, 2, 8, 1, 3, 6, 10, 11],
    },
    HistoricalRow {
        name: "SchoenbergOp41",
        composer: "Schoenberg",
        opus: Some("Op. 41"),
        title: "Ode To Napoleon",
        pitch_classes: [1, 0, 4, 5, 9, 8, 3, 2, 6, 7, 11, 10],
    },
    HistoricalRow {
        name: "WebernOp17No3",
        composer: "Webern",
        opus: Some("Op. 17, No. 3"),
        title: "\"Heiland, Unsere Missetaten...\"",
        pitch_classes: [8, 5, 4, 3, 7, 6, 0, 1, 2, 11, 10, 9],
    },
    HistoricalRow {
        name: "WebernOp17No2",
        composer: "Webern",
        opus: Some("Op. 17, No. 2"),
        title: "\"Liebste Jungfrau\"",
        pitch_classes: [1, 0, 11, 7, 8, 2, 3, 6, 5, 4, 9, 10],
    },
];

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

/// John Link's catalogue of all-interval rows containing the all-trichord
/// hexachord, as the untransformed interval strings music21 matches against.
const LINK_CHORDS: [LinkChord; 238] = [
    LinkChord {
        intervals: "125634T97E8",
        special_intervals: "25634",
        classification: 1,
    },
    LinkChord {
        intervals: "134E78526T9",
        special_intervals: "134E7",
        classification: 2,
    },
    LinkChord {
        intervals: "134E79T6258",
        special_intervals: "134E7",
        classification: 3,
    },
    LinkChord {
        intervals: "134E79T6258",
        special_intervals: "E79T6",
        classification: 3,
    },
    LinkChord {
        intervals: "1367T89E254",
        special_intervals: "89E25",
        classification: 4,
    },
    LinkChord {
        intervals: "137E542896T",
        special_intervals: "7E542",
        classification: 5,
    },
    LinkChord {
        intervals: "137E982456T",
        special_intervals: "7E982",
        classification: 6,
    },
    LinkChord {
        intervals: "142965837ET",
        special_intervals: "29658",
        classification: 7,
    },
    LinkChord {
        intervals: "142973856ET",
        special_intervals: "856ET",
        classification: 8,
    },
    LinkChord {
        intervals: "1429738E65T",
        special_intervals: "8E65T",
        classification: 9,
    },
    LinkChord {
        intervals: "14297TE6853",
        special_intervals: "E6853",
        classification: 10,
    },
    LinkChord {
        intervals: "145638E729T",
        special_intervals: "8E729",
        classification: 11,
    },
    LinkChord {
        intervals: "1456T729E83",
        special_intervals: "729E8",
        classification: 12,
    },
    LinkChord {
        intervals: "1456T982E73",
        special_intervals: "982E7",
        classification: 13,
    },
    LinkChord {
        intervals: "145927E836T",
        special_intervals: "927E8",
        classification: 14,
    },
    LinkChord {
        intervals: "14598E63T72",
        special_intervals: "8E63T",
        classification: 15,
    },
    LinkChord {
        intervals: "14689T7E253",
        special_intervals: "7E253",
        classification: 16,
    },
    LinkChord {
        intervals: "1469E27T853",
        special_intervals: "7T853",
        classification: 17,
    },
    LinkChord {
        intervals: "149278356ET",
        special_intervals: "78356",
        classification: 18,
    },
    LinkChord {
        intervals: "1492783E65T",
        special_intervals: "783E6",
        classification: 19,
    },
    LinkChord {
        intervals: "1496258T73E",
        special_intervals: "58T73",
        classification: 20,
    },
    LinkChord {
        intervals: "1496E358T72",
        special_intervals: "358T7",
        classification: 21,
    },
    LinkChord {
        intervals: "14972836E5T",
        special_intervals: "2836E",
        classification: 22,
    },
    LinkChord {
        intervals: "14972E6385T",
        special_intervals: "2E638",
        classification: 23,
    },
    LinkChord {
        intervals: "1497T853E62",
        special_intervals: "7T853",
        classification: 24,
    },
    LinkChord {
        intervals: "14E379T6528",
        special_intervals: "79T65",
        classification: 25,
    },
    LinkChord {
        intervals: "14E6T783529",
        special_intervals: "78352",
        classification: 26,
    },
    LinkChord {
        intervals: "172E6853T94",
        special_intervals: "E6853",
        classification: 27,
    },
    LinkChord {
        intervals: "17356ET8294",
        special_intervals: "56ET8",
        classification: 28,
    },
    LinkChord {
        intervals: "1738T542E69",
        special_intervals: "42E69",
        classification: 29,
    },
    LinkChord {
        intervals: "173E65T8294",
        special_intervals: "E65T8",
        classification: 30,
    },
    LinkChord {
        intervals: "1763T4952E8",
        special_intervals: "4952E",
        classification: 31,
    },
    LinkChord {
        intervals: "176852E34T9",
        special_intervals: "52E34",
        classification: 32,
    },
    LinkChord {
        intervals: "179236E8T54",
        special_intervals: "236E8",
        classification: 33,
    },
    LinkChord {
        intervals: "179236E8T54",
        special_intervals: "36E8T",
        classification: 33,
    },
    LinkChord {
        intervals: "17923E685T4",
        special_intervals: "3E685",
        classification: 34,
    },
    LinkChord {
        intervals: "179245T8E63",
        special_intervals: "T8E63",
        classification: 35,
    },
    LinkChord {
        intervals: "17924T586E3",
        special_intervals: "586E3",
        classification: 36,
    },
    LinkChord {
        intervals: "179T65234E8",
        special_intervals: "79T65",
        classification: 37,
    },
    LinkChord {
        intervals: "179T8E63254",
        special_intervals: "T8E63",
        classification: 38,
    },
    LinkChord {
        intervals: "179T8E63254",
        special_intervals: "8E632",
        classification: 38,
    },
    LinkChord {
        intervals: "1825E43796T",
        special_intervals: "25E43",
        classification: 39,
    },
    LinkChord {
        intervals: "1825E43796T",
        special_intervals: "5E437",
        classification: 39,
    },
    LinkChord {
        intervals: "1825E79T364",
        special_intervals: "25E79",
        classification: 40,
    },
    LinkChord {
        intervals: "1825E79T364",
        special_intervals: "5E79T",
        classification: 40,
    },
    LinkChord {
        intervals: "1852E43769T",
        special_intervals: "E4376",
        classification: 41,
    },
    LinkChord {
        intervals: "1852E79463T",
        special_intervals: "E7946",
        classification: 42,
    },
    LinkChord {
        intervals: "185629TE743",
        special_intervals: "9TE74",
        classification: 43,
    },
    LinkChord {
        intervals: "18563479TE2",
        special_intervals: "479TE",
        classification: 44,
    },
    LinkChord {
        intervals: "18563E7T492",
        special_intervals: "E7T49",
        classification: 45,
    },
    LinkChord {
        intervals: "18734E5296T",
        special_intervals: "734E5",
        classification: 46,
    },
    LinkChord {
        intervals: "18734E5296T",
        special_intervals: "34E52",
        classification: 46,
    },
    LinkChord {
        intervals: "187E259436T",
        special_intervals: "87E25",
        classification: 47,
    },
    LinkChord {
        intervals: "187E259436T",
        special_intervals: "E2594",
        classification: 47,
    },
    LinkChord {
        intervals: "18T352E7964",
        special_intervals: "352E7",
        classification: 48,
    },
    LinkChord {
        intervals: "18T497E3562",
        special_intervals: "T497E",
        classification: 49,
    },
    LinkChord {
        intervals: "18T97E52364",
        special_intervals: "T97E5",
        classification: 50,
    },
    LinkChord {
        intervals: "18T97E52364",
        special_intervals: "97E52",
        classification: 50,
    },
    LinkChord {
        intervals: "18E63T79452",
        special_intervals: "8E63T",
        classification: 51,
    },
    LinkChord {
        intervals: "18E63T79452",
        special_intervals: "T7945",
        classification: 51,
    },
    LinkChord {
        intervals: "19476538TE2",
        special_intervals: "76538",
        classification: 52,
    },
    LinkChord {
        intervals: "1947T8536E2",
        special_intervals: "7T853",
        classification: 53,
    },
    LinkChord {
        intervals: "1954763T8E2",
        special_intervals: "4763T",
        classification: 54,
    },
    LinkChord {
        intervals: "19742538E6T",
        special_intervals: "97425",
        classification: 55,
    },
    LinkChord {
        intervals: "197425T6E83",
        special_intervals: "97425",
        classification: 56,
    },
    LinkChord {
        intervals: "1974T8E6352",
        special_intervals: "T8E63",
        classification: 57,
    },
    LinkChord {
        intervals: "197T8532E64",
        special_intervals: "7T853",
        classification: 58,
    },
    LinkChord {
        intervals: "1T5E7928364",
        special_intervals: "5E792",
        classification: 59,
    },
    LinkChord {
        intervals: "1T63E874259",
        special_intervals: "74259",
        classification: 60,
    },
    LinkChord {
        intervals: "1T6E3852479",
        special_intervals: "52479",
        classification: 61,
    },
    LinkChord {
        intervals: "1T8352E4679",
        special_intervals: "8352E",
        classification: 62,
    },
    LinkChord {
        intervals: "1T974253E68",
        special_intervals: "97425",
        classification: 63,
    },
    LinkChord {
        intervals: "214367TE985",
        special_intervals: "4367T",
        classification: 64,
    },
    LinkChord {
        intervals: "214376598ET",
        special_intervals: "43765",
        classification: 65,
    },
    LinkChord {
        intervals: "214376598ET",
        special_intervals: "76598",
        classification: 65,
    },
    LinkChord {
        intervals: "2143E86597T",
        special_intervals: "E8659",
        classification: 66,
    },
    LinkChord {
        intervals: "2149586E37T",
        special_intervals: "586E3",
        classification: 67,
    },
    LinkChord {
        intervals: "214976538ET",
        special_intervals: "49765",
        classification: 68,
    },
    LinkChord {
        intervals: "214976538ET",
        special_intervals: "76538",
        classification: 68,
    },
    LinkChord {
        intervals: "216734ET985",
        special_intervals: "6734E",
        classification: 69,
    },
    LinkChord {
        intervals: "216743E9T85",
        special_intervals: "21674",
        classification: 70,
    },
    LinkChord {
        intervals: "217634TE985",
        special_intervals: "7634T",
        classification: 71,
    },
    LinkChord {
        intervals: "21T8E956347",
        special_intervals: "1T8E9",
        classification: 72,
    },
    LinkChord {
        intervals: "21T96583E47",
        special_intervals: "T9658",
        classification: 73,
    },
    LinkChord {
        intervals: "234T1596E87",
        special_intervals: "34T15",
        classification: 74,
    },
    LinkChord {
        intervals: "235189E647T",
        special_intervals: "5189E",
        classification: 75,
    },
    LinkChord {
        intervals: "235189E647T",
        special_intervals: "189E6",
        classification: 75,
    },
    LinkChord {
        intervals: "23689E7T145",
        special_intervals: "689E7",
        classification: 76,
    },
    LinkChord {
        intervals: "236E981547T",
        special_intervals: "6E981",
        classification: 77,
    },
    LinkChord {
        intervals: "236E981547T",
        special_intervals: "E9815",
        classification: 77,
    },
    LinkChord {
        intervals: "23T514697E8",
        special_intervals: "3T514",
        classification: 78,
    },
    LinkChord {
        intervals: "2513647ET98",
        special_intervals: "47ET9",
        classification: 79,
    },
    LinkChord {
        intervals: "25189TE7463",
        special_intervals: "25189",
        classification: 80,
    },
    LinkChord {
        intervals: "25189TE7463",
        special_intervals: "9TE74",
        classification: 80,
    },
    LinkChord {
        intervals: "2546E981T73",
        special_intervals: "6E981",
        classification: 81,
    },
    LinkChord {
        intervals: "25691T8E473",
        special_intervals: "91T8E",
        classification: 82,
    },
    LinkChord {
        intervals: "2569E8T1437",
        special_intervals: "9E8T1",
        classification: 83,
    },
    LinkChord {
        intervals: "258T73E6149",
        special_intervals: "58T73",
        classification: 84,
    },
    LinkChord {
        intervals: "258T9614E37",
        special_intervals: "T9614",
        classification: 85,
    },
    LinkChord {
        intervals: "25T31496E87",
        special_intervals: "5T314",
        classification: 86,
    },
    LinkChord {
        intervals: "25T89E61437",
        special_intervals: "89E61",
        classification: 87,
    },
    LinkChord {
        intervals: "2618T497E35",
        special_intervals: "T497E",
        classification: 88,
    },
    LinkChord {
        intervals: "26347ET9185",
        special_intervals: "47ET9",
        classification: 89,
    },
    LinkChord {
        intervals: "263891T7E45",
        special_intervals: "891T7",
        classification: 90,
    },
    LinkChord {
        intervals: "263891T7E45",
        special_intervals: "1T7E4",
        classification: 90,
    },
    LinkChord {
        intervals: "2653E718T49",
        special_intervals: "E718T",
        classification: 91,
    },
    LinkChord {
        intervals: "2654T1783E9",
        special_intervals: "T1783",
        classification: 92,
    },
    LinkChord {
        intervals: "2654T1783E9",
        special_intervals: "1783E",
        classification: 92,
    },
    LinkChord {
        intervals: "2654E3871T9",
        special_intervals: "E3871",
        classification: 93,
    },
    LinkChord {
        intervals: "2654E3871T9",
        special_intervals: "3871T",
        classification: 93,
    },
    LinkChord {
        intervals: "2654E7T1983",
        special_intervals: "4E7T1",
        classification: 94,
    },
    LinkChord {
        intervals: "2654E7T1983",
        special_intervals: "7T198",
        classification: 94,
    },
    LinkChord {
        intervals: "265819TE743",
        special_intervals: "9TE74",
        classification: 95,
    },
    LinkChord {
        intervals: "2659E8T1347",
        special_intervals: "9E8T1",
        classification: 96,
    },
    LinkChord {
        intervals: "267431T8E95",
        special_intervals: "1T8E9",
        classification: 97,
    },
    LinkChord {
        intervals: "2694T817E35",
        special_intervals: "T817E",
        classification: 98,
    },
    LinkChord {
        intervals: "269T1783E45",
        special_intervals: "T1783",
        classification: 99,
    },
    LinkChord {
        intervals: "269T1783E45",
        special_intervals: "1783E",
        classification: 99,
    },
    LinkChord {
        intervals: "269E3871T45",
        special_intervals: "E3871",
        classification: 100,
    },
    LinkChord {
        intervals: "269E3871T45",
        special_intervals: "3871T",
        classification: 100,
    },
    LinkChord {
        intervals: "26E451T7389",
        special_intervals: "451T7",
        classification: 101,
    },
    LinkChord {
        intervals: "26E451T7389",
        special_intervals: "1T738",
        classification: 101,
    },
    LinkChord {
        intervals: "26E459817T3",
        special_intervals: "59817",
        classification: 102,
    },
    LinkChord {
        intervals: "26E459817T3",
        special_intervals: "9817T",
        classification: 102,
    },
    LinkChord {
        intervals: "26E4T718953",
        special_intervals: "T7189",
        classification: 103,
    },
    LinkChord {
        intervals: "26E4T718953",
        special_intervals: "71895",
        classification: 103,
    },
    LinkChord {
        intervals: "26E873T5149",
        special_intervals: "3T514",
        classification: 104,
    },
    LinkChord {
        intervals: "26E95134T87",
        special_intervals: "5134T",
        classification: 105,
    },
    LinkChord {
        intervals: "26E95178T43",
        special_intervals: "5178T",
        classification: 106,
    },
    LinkChord {
        intervals: "274316E985T",
        special_intervals: "4316E",
        classification: 107,
    },
    LinkChord {
        intervals: "274316E985T",
        special_intervals: "16E98",
        classification: 107,
    },
    LinkChord {
        intervals: "2743E86591T",
        special_intervals: "E8659",
        classification: 108,
    },
    LinkChord {
        intervals: "274916E385T",
        special_intervals: "4916E",
        classification: 109,
    },
    LinkChord {
        intervals: "274916E385T",
        special_intervals: "16E38",
        classification: 109,
    },
    LinkChord {
        intervals: "2749586E31T",
        special_intervals: "586E3",
        classification: 110,
    },
    LinkChord {
        intervals: "276134ET985",
        special_intervals: "6134E",
        classification: 111,
    },
    LinkChord {
        intervals: "276143E9T85",
        special_intervals: "27614",
        classification: 112,
    },
    LinkChord {
        intervals: "27E34169T85",
        special_intervals: "4169T",
        classification: 113,
    },
    LinkChord {
        intervals: "2965387E41T",
        special_intervals: "65387",
        classification: 114,
    },
    LinkChord {
        intervals: "2965387E41T",
        special_intervals: "5387E",
        classification: 114,
    },
    LinkChord {
        intervals: "296E387T145",
        special_intervals: "6E387",
        classification: 115,
    },
    LinkChord {
        intervals: "29TE7436185",
        special_intervals: "9TE74",
        classification: 116,
    },
    LinkChord {
        intervals: "29TE7463158",
        special_intervals: "9TE74",
        classification: 117,
    },
    LinkChord {
        intervals: "29E7835641T",
        special_intervals: "E7835",
        classification: 118,
    },
    LinkChord {
        intervals: "29E7835641T",
        special_intervals: "78356",
        classification: 118,
    },
    LinkChord {
        intervals: "2E431T96587",
        special_intervals: "E431T",
        classification: 119,
    },
    LinkChord {
        intervals: "2E431T96587",
        special_intervals: "T9658",
        classification: 119,
    },
    LinkChord {
        intervals: "2E465387T19",
        special_intervals: "65387",
        classification: 120,
    },
    LinkChord {
        intervals: "2E637T85419",
        special_intervals: "37T85",
        classification: 121,
    },
    LinkChord {
        intervals: "2E783T51469",
        special_intervals: "2E783",
        classification: 122,
    },
    LinkChord {
        intervals: "2E783T51469",
        special_intervals: "3T514",
        classification: 122,
    },
    LinkChord {
        intervals: "2E796415T38",
        special_intervals: "415T3",
        classification: 123,
    },
    LinkChord {
        intervals: "2E8T1956743",
        special_intervals: "E8T19",
        classification: 124,
    },
    LinkChord {
        intervals: "2E9658T4137",
        special_intervals: "9658T",
        classification: 125,
    },
    LinkChord {
        intervals: "3142E8956T7",
        special_intervals: "3142E",
        classification: 126,
    },
    LinkChord {
        intervals: "3142ET79685",
        special_intervals: "3142E",
        classification: 127,
    },
    LinkChord {
        intervals: "31456T972E8",
        special_intervals: "56T97",
        classification: 128,
    },
    LinkChord {
        intervals: "314672E9T85",
        special_intervals: "31467",
        classification: 129,
    },
    LinkChord {
        intervals: "3152689TE74",
        special_intervals: "52689",
        classification: 130,
    },
    LinkChord {
        intervals: "3152689TE74",
        special_intervals: "9TE74",
        classification: 130,
    },
    LinkChord {
        intervals: "3152E9T8764",
        special_intervals: "152E9",
        classification: 131,
    },
    LinkChord {
        intervals: "3158629TE74",
        special_intervals: "58629",
        classification: 132,
    },
    LinkChord {
        intervals: "3158629TE74",
        special_intervals: "9TE74",
        classification: 132,
    },
    LinkChord {
        intervals: "316452E98T7",
        special_intervals: "52E98",
        classification: 133,
    },
    LinkChord {
        intervals: "317ET562894",
        special_intervals: "56289",
        classification: 134,
    },
    LinkChord {
        intervals: "317ET926854",
        special_intervals: "92685",
        classification: 135,
    },
    LinkChord {
        intervals: "317ET986254",
        special_intervals: "98625",
        classification: 136,
    },
    LinkChord {
        intervals: "319765T42E8",
        special_intervals: "765T4",
        classification: 137,
    },
    LinkChord {
        intervals: "3198265TE74",
        special_intervals: "98265",
        classification: 138,
    },
    LinkChord {
        intervals: "31T524796E8",
        special_intervals: "52479",
        classification: 139,
    },
    LinkChord {
        intervals: "31T567942E8",
        special_intervals: "56794",
        classification: 140,
    },
    LinkChord {
        intervals: "31T7E294685",
        special_intervals: "31T7E",
        classification: 141,
    },
    LinkChord {
        intervals: "325E79T8164",
        special_intervals: "25E79",
        classification: 142,
    },
    LinkChord {
        intervals: "325E79T8164",
        special_intervals: "5E79T",
        classification: 142,
    },
    LinkChord {
        intervals: "3265981TE74",
        special_intervals: "65981",
        classification: 143,
    },
    LinkChord {
        intervals: "329E71T4568",
        special_intervals: "29E71",
        classification: 144,
    },
    LinkChord {
        intervals: "329E71T4568",
        special_intervals: "9E71T",
        classification: 144,
    },
    LinkChord {
        intervals: "32E981T6547",
        special_intervals: "2E981",
        classification: 145,
    },
    LinkChord {
        intervals: "347621T8E95",
        special_intervals: "1T8E9",
        classification: 146,
    },
    LinkChord {
        intervals: "3479TE26185",
        special_intervals: "479TE",
        classification: 147,
    },
    LinkChord {
        intervals: "34T9E712568",
        special_intervals: "T9E71",
        classification: 148,
    },
    LinkChord {
        intervals: "35146T927E8",
        special_intervals: "146T9",
        classification: 149,
    },
    LinkChord {
        intervals: "35146T927E8",
        special_intervals: "927E8",
        classification: 149,
    },
    LinkChord {
        intervals: "351T64927E8",
        special_intervals: "1T649",
        classification: 150,
    },
    LinkChord {
        intervals: "351T64927E8",
        special_intervals: "927E8",
        classification: 150,
    },
    LinkChord {
        intervals: "351T7924E68",
        special_intervals: "51T79",
        classification: 151,
    },
    LinkChord {
        intervals: "35216E98T74",
        special_intervals: "16E98",
        classification: 152,
    },
    LinkChord {
        intervals: "3521T8E9674",
        special_intervals: "1T8E9",
        classification: 153,
    },
    LinkChord {
        intervals: "3581629ET74",
        special_intervals: "1629E",
        classification: 154,
    },
    LinkChord {
        intervals: "3594T6127E8",
        special_intervals: "94T61",
        classification: 155,
    },
    LinkChord {
        intervals: "359E6128T74",
        special_intervals: "E6128",
        classification: 156,
    },
    LinkChord {
        intervals: "35E7216T498",
        special_intervals: "16T49",
        classification: 157,
    },
    LinkChord {
        intervals: "35E72946T18",
        special_intervals: "946T1",
        classification: 158,
    },
    LinkChord {
        intervals: "35E729T6418",
        special_intervals: "9T641",
        classification: 159,
    },
    LinkChord {
        intervals: "3625189TE74",
        special_intervals: "25189",
        classification: 160,
    },
    LinkChord {
        intervals: "3625189TE74",
        special_intervals: "9TE74",
        classification: 160,
    },
    LinkChord {
        intervals: "36524ET8917",
        special_intervals: "36524",
        classification: 161,
    },
    LinkChord {
        intervals: "3674218T9E5",
        special_intervals: "36742",
        classification: 162,
    },
    LinkChord {
        intervals: "36T154927E8",
        special_intervals: "T1549",
        classification: 163,
    },
    LinkChord {
        intervals: "36T154927E8",
        special_intervals: "927E8",
        classification: 163,
    },
    LinkChord {
        intervals: "3764128T9E5",
        special_intervals: "37641",
        classification: 164,
    },
    LinkChord {
        intervals: "38297E5T164",
        special_intervals: "297E5",
        classification: 165,
    },
    LinkChord {
        intervals: "38E729T6145",
        special_intervals: "8E729",
        classification: 166,
    },
    LinkChord {
        intervals: "3T17E924568",
        special_intervals: "T17E9",
        classification: 167,
    },
    LinkChord {
        intervals: "3T17E924568",
        special_intervals: "17E92",
        classification: 167,
    },
    LinkChord {
        intervals: "3T4952E8617",
        special_intervals: "4952E",
        classification: 168,
    },
    LinkChord {
        intervals: "3T6194527E8",
        special_intervals: "61945",
        classification: 169,
    },
    LinkChord {
        intervals: "3T62E815497",
        special_intervals: "15497",
        classification: 170,
    },
    LinkChord {
        intervals: "3T97E528164",
        special_intervals: "T97E5",
        classification: 171,
    },
    LinkChord {
        intervals: "3T97E528164",
        special_intervals: "97E52",
        classification: 171,
    },
    LinkChord {
        intervals: "3E2418596T7",
        special_intervals: "3E241",
        classification: 172,
    },
    LinkChord {
        intervals: "3E7T4926185",
        special_intervals: "E7T49",
        classification: 173,
    },
    LinkChord {
        intervals: "416352E7T98",
        special_intervals: "352E7",
        classification: 174,
    },
    LinkChord {
        intervals: "41T629E7835",
        special_intervals: "629E7",
        classification: 175,
    },
    LinkChord {
        intervals: "41T629E7835",
        special_intervals: "E7835",
        classification: 175,
    },
    LinkChord {
        intervals: "4328T56E917",
        special_intervals: "8T56E",
        classification: 176,
    },
    LinkChord {
        intervals: "4328TE65917",
        special_intervals: "8TE65",
        classification: 177,
    },
    LinkChord {
        intervals: "43T9E865217",
        special_intervals: "9E865",
        classification: 178,
    },
    LinkChord {
        intervals: "463152E9T87",
        special_intervals: "152E9",
        classification: 179,
    },
    LinkChord {
        intervals: "46529E8T137",
        special_intervals: "9E8T1",
        classification: 180,
    },
    LinkChord {
        intervals: "46731T8E925",
        special_intervals: "1T8E9",
        classification: 181,
    },
    LinkChord {
        intervals: "4692E513T87",
        special_intervals: "2E513",
        classification: 182,
    },
    LinkChord {
        intervals: "4692E513T87",
        special_intervals: "E513T",
        classification: 182,
    },
    LinkChord {
        intervals: "46982315ET7",
        special_intervals: "2315E",
        classification: 183,
    },
    LinkChord {
        intervals: "469T315E287",
        special_intervals: "T315E",
        classification: 184,
    },
    LinkChord {
        intervals: "469T315E287",
        special_intervals: "315E2",
        classification: 184,
    },
    LinkChord {
        intervals: "4769E251T38",
        special_intervals: "9E251",
        classification: 185,
    },
    LinkChord {
        intervals: "4783T1629E5",
        special_intervals: "1629E",
        classification: 186,
    },
    LinkChord {
        intervals: "47T198236E5",
        special_intervals: "7T198",
        classification: 187,
    },
    LinkChord {
        intervals: "47E928361T5",
        special_intervals: "8361T",
        classification: 188,
    },
    LinkChord {
        intervals: "4T1629E3785",
        special_intervals: "1629E",
        classification: 189,
    },
    LinkChord {
        intervals: "4TE86592317",
        special_intervals: "E8659",
        classification: 190,
    },
    LinkChord {
        intervals: "4E2538T1697",
        special_intervals: "E2538",
        classification: 191,
    },
    LinkChord {
        intervals: "4E29658T317",
        special_intervals: "29658",
        classification: 192,
    },
    LinkChord {
        intervals: "4E29658T317",
        special_intervals: "9658T",
        classification: 192,
    },
    LinkChord {
        intervals: "4ET85692317",
        special_intervals: "T8569",
        classification: 193,
    },
    LinkChord {
        intervals: "4ET85692317",
        special_intervals: "85692",
        classification: 193,
    },
    LinkChord {
        intervals: "5896T142E37",
        special_intervals: "142E3",
        classification: 194,
    },
];

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
