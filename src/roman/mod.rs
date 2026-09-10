use crate::{
    chord::{Chord, root::pitch_class},
    chordsymbol::{ChordQuality, ChordSymbol},
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    figuredbass::{Figure, Notation},
    interval::Interval,
    key::Key,
    pitch::{Accidental, Pitch},
};
use std::fmt;

mod analysis;
mod figure;
mod realize;

pub use analysis::{
    analyze_chord, analyze_chord_with_root, correct_rn_alteration_for_minor,
    correct_suffix_for_chord_quality, figure_tuples, identify_as_tonic_or_dominant,
    roman_inversion_name,
};
pub(crate) use figure::degree_to_roman;
pub use figure::{
    adjust_minor_vi_and_vii_by_quality, bass_scale_degree_from_notation, expand_shorthand,
    parse_numeral_alone, secondary_key, split_roman_accidental_prefix, split_roman_prefix,
    split_secondary, take_added_steps, take_bracketed_alterations, take_omitted_steps,
};
pub use realize::match_pitches_to_quality;

use figure::*;
use realize::*;

/// How a figure on the sixth or seventh degree of a minor key decides
/// between the natural and the raised degree: music21's `Minor67Default`.
///
/// Minor is two scales at once, and a `vi` might mean either of two chords.
/// music21 lets a caller say which reading to use, and the readings differ
/// in what they do with an accidental the figure already carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Minor67Default {
    /// Read it from the chord the figure asks for: a minor triad on the
    /// sixth or a diminished chord on the seventh needs the raised degree,
    /// and a major one needs the natural. This is music21's default and the
    /// reading most scores assume.
    #[default]
    Quality,
    /// Always the natural degree, whatever the figure asks for. A sharp
    /// written in front still raises it.
    Flat,
    /// Always the raised degree. A flat written in front still lowers it.
    Sharp,
    /// As `Quality`, but an accidental already written in front is read as a
    /// caution rather than as a further change — so `#vi` and `vi` are the
    /// same chord, and so are `bVI` and `VI`.
    Cautionary,
}

/// A parsed Roman numeral in a key.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct RomanNumeral {
    figure: String,
    key: Key,
    degree: u8,
    accidental: i8,
    /// The alteration as it was written in front of the numeral, before the
    /// minor sixth and seventh were read against the chord asked for.
    ///
    /// music21 keeps the two apart: `vi` in C minor reports `#vi` as its
    /// numeral, because its root is a raised sixth degree, while the figure
    /// it was written with carries no accidental at all.
    written_accidental: i8,
    inversion: u8,
    seventh: bool,
    quality: RomanQuality,
    /// The quality the figure states, as music21 reads it off the symbol in
    /// front of the inversion digits or off the case of the numeral. It is
    /// what the notes read from the scale are respelled to.
    implied_quality: ImpliedQuality,
    /// The figured-bass column the digits stand for, with the omissions,
    /// additions and alterations written in brackets beside them.
    figures: FiguredBass,
    secondary: Option<String>,
    kind: RomanKind,
    /// How the sixth and seventh degrees of a minor key are read.
    sixth_minor: Minor67Default,
    seventh_minor: Minor67Default,
    /// The collection the figure is read over, when it is not a key at all.
    ///
    /// music21 takes a `ConcreteScale` wherever it takes a key, and reads
    /// every degree off that instead — which is how a numeral means
    /// something in a collection no key signature can write, such as the
    /// octatonic. The key is still carried, as the major of the same tonic,
    /// because everything that asks a numeral for its key expects one.
    scale: Option<crate::scale::Scale>,
    /// Whether an upper-case numeral means a major chord and a lower-case
    /// one a minor chord.
    ///
    /// music21's `caseMatters`. The older figured-bass tradition writes every
    /// numeral in upper case and lets the key say what the chord is, and a
    /// numeral read that way states no quality at all: its notes are whatever
    /// the scale spells, and the sixth and seventh of a minor key are left
    /// where the key signature put them.
    case_matters: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum RomanKind {
    Diatonic,
    AugmentedSixth(AugmentedSixthKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum AugmentedSixthKind {
    Italian,
    French,
    German,
    Swiss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum RomanQuality {
    Major,
    Minor,
    Diminished,
    HalfDiminished,
    Augmented,
}

impl AugmentedSixthKind {
    /// The augmented sixth a figure names, if it names one.
    ///
    /// music21 reads these as the country's name, an optional `+`, and
    /// whatever inversion figure follows — so `It`, `It+`, `It6`, `It+6` and
    /// `Ger6/5` are all augmented sixths, and the inversion is read off
    /// afterwards.
    fn from_figure(figure: &str) -> Option<Self> {
        let figure = figure.trim();
        let kind = augmented_sixth_prefix(figure)?;
        let name = match kind {
            Self::Italian => "It",
            Self::French => "Fr",
            Self::German => "Ger",
            Self::Swiss => "Sw",
        };
        let rest = figure[name.len()..].trim_start_matches('+');
        unslash_inversion(rest)
            .chars()
            .all(|written| written.is_ascii_digit())
            .then_some(kind)
    }

    fn from_common_name(name: &str) -> Option<Self> {
        if name.contains("Italian augmented sixth chord") {
            Some(Self::Italian)
        } else if name.contains("French augmented sixth chord") {
            Some(Self::French)
        } else if name.contains("German augmented sixth chord") {
            Some(Self::German)
        } else if name.contains("Swiss augmented sixth chord") {
            Some(Self::Swiss)
        } else {
            None
        }
    }

    /// The inversion figure music21 reads one of these in when the figure
    /// names no digits: a German sixth written `Ger` is `Ger65`.
    fn default_inversion(self) -> &'static str {
        match self {
            Self::Italian => "6",
            Self::French | Self::Swiss => "43",
            Self::German => "65",
        }
    }

    /// The digits an augmented-sixth figure writes, once its name and the
    /// `+` are off it. music21's `_parseRNAloneAmidstAug6`: a figure naming
    /// no digits takes the usual inversion, and one naming a plain `6` takes
    /// it too, since `Fr6` is how `Fr43` is usually written.
    fn written_figure(self, figure: &str) -> String {
        let rest = figure
            .trim()
            .trim_start_matches(['I', 't', 'G', 'e', 'r', 'F', 'S', 'w'])
            .trim_start_matches('+');
        // A figure written `6/5` is the same as `65`.
        let rest = unslash_inversion(rest);
        let rest = rest.as_str();
        let leading_digit = rest
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_digit());
        if !leading_digit {
            return format!("{}{rest}", self.default_inversion());
        }
        if self != Self::Italian
            && rest.starts_with('6')
            && !rest[1..].starts_with(|c: char| c.is_ascii_digit())
        {
            return format!("{}{}", self.default_inversion(), &rest[1..]);
        }
        rest.to_string()
    }

    /// The scale degree music21 reads one of these on: the fourth for the
    /// Italian and German sixths, the second for the French and Swiss.
    fn degree(self) -> u8 {
        match self {
            Self::Italian | Self::German => 4,
            Self::French | Self::Swiss => 2,
        }
    }

    /// The alteration music21 records in front of it, in semitones. It is
    /// written down and not applied — the sharp that makes the chord is the
    /// bracketed one.
    fn written_alteration(self) -> i8 {
        match self {
            Self::French => 0,
            _ => 1,
        }
    }

    /// The alterations that make the chord augmented: music21's
    /// `bracketedAlterations`, a sharp on the root for every kind but the
    /// French, and one on the third for the French and the Swiss.
    fn bracketed_alterations(self) -> Vec<(i8, u8)> {
        let mut alterations = Vec::new();
        if self != Self::French {
            alterations.push((1, 1));
        }
        if matches!(self, Self::French | Self::Swiss) {
            alterations.push((1, 3));
        }
        alterations
    }

    fn figure(self) -> &'static str {
        match self {
            Self::Italian => "It+6",
            Self::French => "Fr+6",
            Self::German => "Ger+6",
            Self::Swiss => "Sw+6",
        }
    }
}

impl RomanNumeral {
    /// Parses a Roman numeral figure in a key.
    ///
    /// Supports ordinary figures such as `V7/V` and augmented-sixth figures
    /// such as `It+6`, `Fr+6`, `Ger+6`, and `Sw+6`.
    pub fn new(figure: impl Into<String>, key: Key) -> Result<Self> {
        Self::with_minor_defaults(
            figure,
            key,
            Minor67Default::default(),
            Minor67Default::default(),
        )
    }

    /// The same, saying how the sixth and seventh degrees of a minor key are
    /// to be read: music21's `sixthMinor` and `seventhMinor`.
    pub fn with_minor_defaults(
        figure: impl Into<String>,
        key: Key,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
    ) -> Result<Self> {
        Self::with_options(figure, key, sixth_minor, seventh_minor, true)
    }

    /// The same again, saying whether the case of the numeral states the
    /// chord's quality: music21's `caseMatters`.
    pub fn with_options(
        figure: impl Into<String>,
        key: Key,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
        case_matters: bool,
    ) -> Result<Self> {
        Self::over_scale(figure, key, None, sixth_minor, seventh_minor, case_matters)
    }

    /// The same again over a scale that is not a key: music21's numerals
    /// read against a `ConcreteScale`.
    ///
    /// The key is still needed — a numeral reports one, and a secondary
    /// numeral establishes one — so pass the major key of the scale's tonic,
    /// which is what music21 falls back on.
    pub fn over_scale(
        figure: impl Into<String>,
        key: Key,
        scale: Option<crate::scale::Scale>,
        sixth_minor: Minor67Default,
        seventh_minor: Minor67Default,
        case_matters: bool,
    ) -> Result<Self> {
        let figure = figure.into();
        let written = fold_figure_symbols(figure.trim());
        let trimmed = written.as_str();
        if trimmed.is_empty() {
            return Err(Error::Chord("roman numeral cannot be empty".to_string()));
        }
        validate_figure(trimmed)?;

        // The applied part comes off first, so that `Ger6/vi` is a German
        // sixth read in the key its `vi` establishes — but a slash inside
        // the inversion figure is not an applied part at all, and `Ger6/5`
        // is the German sixth in the position it is usually written in.
        let (aug6_primary, aug6_secondary) = if AugmentedSixthKind::from_figure(trimmed).is_some() {
            (trimmed, None)
        } else {
            split_secondary(trimmed)
        };
        if let Some(kind) = AugmentedSixthKind::from_figure(aug6_primary) {
            // music21's `_parseRNAloneAmidstAug6`: an augmented sixth is a
            // figured-bass column over an altered degree, read in the
            // parallel minor. The alteration in front of it is *recorded*
            // and not applied — what makes the chord augmented is the
            // bracketed sharp, which the ordinary path puts on afterwards.
            let column = kind.written_figure(aug6_primary);
            return Ok(Self {
                // The figure is kept as written, since music21 names these
                // several ways and reports back the one it was given.
                figure: written.clone(),
                key,
                degree: kind.degree(),
                accidental: 0,
                written_accidental: kind.written_alteration(),
                inversion: parse_inversion(&column),
                seventh: suffix_has_seventh(&column),
                quality: RomanQuality::from(ImpliedQuality::Unstated),
                implied_quality: ImpliedQuality::Unstated,
                figures: FiguredBass {
                    column: Notation::parse(&expand_shorthand(&column).join(","))?,
                    written: column,
                    bracketed: kind.bracketed_alterations(),
                    ..FiguredBass::default()
                },
                secondary: aug6_secondary,
                kind: RomanKind::AugmentedSixth(kind),
                sixth_minor,
                seventh_minor,
                case_matters,
                scale,
            });
        }

        let (primary, secondary) = (aug6_primary, aug6_secondary);

        // music21 writes a few chords by name rather than by numeral: the
        // Neapolitan and the cadential six-four. Each is read as the figure
        // it stands for, while the numeral keeps the name it was given — and
        // read in the key it actually sounds in, so the `Cad64` of `Cad64/V`
        // in C minor is the major tonic of G and not the minor one of C.
        let reading = match &secondary {
            Some(secondary) => {
                secondary_key(&key, secondary, sixth_minor, seventh_minor, case_matters)?
            }
            None => key.clone(),
        };
        let mut working = named_figure(primary, &reading);

        // The brackets come off before anything reads the digits, so a
        // `[no3]` cannot be mistaken for a diminished mark.
        let omitted = take_omitted_steps(&mut working);
        let added = take_added_steps(&mut working);
        let bracketed = take_bracketed_alterations(&mut working);

        let (accidental, rest) = split_roman_accidental_prefix(&working);
        let (roman, suffix) = split_roman_prefix(rest)?;
        let degree = roman_degree(roman)?;
        let (implied_quality, mut column) =
            implied_quality_from_string(roman, suffix, case_matters);
        let inversion = parse_inversion(&column);
        let seventh = suffix_has_seventh(&column);

        let mut numeral = Self {
            figure: written.clone(),
            key,
            degree,
            accidental,
            written_accidental: accidental,
            inversion,
            seventh,
            quality: RomanQuality::from(implied_quality),
            implied_quality,
            figures: FiguredBass::default(),
            secondary,
            kind: RomanKind::Diatonic,
            sixth_minor,
            seventh_minor,
            case_matters,
            scale,
        };
        numeral.raise_minor_sixth_and_seventh(&mut column)?;
        let written = column.clone();
        // This crate writes an addition as `add(13)` where music21 writes a
        // bracket, and it is not part of the figured-bass column: an added
        // thirteenth is a note beside the chord, not a figure over the bass.
        let column = strip_roman_addition_groups(&column);
        numeral.figures = FiguredBass {
            column: Notation::parse(&expand_shorthand(&column).join(","))?,
            written,
            omitted,
            added,
            bracketed,
        };
        Ok(numeral)
    }

    /// The alterations written in square brackets, as the semitones each
    /// moves its chord step by and the step it moves.
    pub fn bracketed_alterations(&self) -> &[(i8, u8)] {
        &self.figures.bracketed
    }

    /// The chord steps the figure leaves out, as `[no3]`.
    pub fn omitted_steps(&self) -> &[u8] {
        &self.figures.omitted
    }

    /// The notes the figure puts in beside the chord, as `[add4]`: the
    /// alteration in semitones and how far above the root each stands.
    pub fn added_steps(&self) -> &[(i8, u8)] {
        &self.figures.added
    }

    /// The scale the figure is read over, where it is not a key at all.
    ///
    /// A numeral read this way spells its chord where the scale stands, so
    /// nothing downstream has to place it.
    pub fn scale(&self) -> Option<&crate::scale::Scale> {
        self.scale.as_ref()
    }

    /// The quality the figure states, which is what the notes read off the
    /// scale are respelled to.
    pub fn implied_quality(&self) -> ImpliedQuality {
        self.implied_quality
    }

    /// The numbers of the figured-bass column the figure's digits stand for,
    /// written high to low and expanded out of music21's shorthand.
    pub fn figure_numbers(&self) -> Vec<u8> {
        self.figures.numbers()
    }

    /// Whether the case of the numeral states the chord's quality.
    pub fn case_matters(&self) -> bool {
        self.case_matters
    }

    /// How this numeral reads the sixth degree of a minor key.
    pub fn sixth_minor(&self) -> Minor67Default {
        self.sixth_minor
    }

    /// How it reads the seventh.
    pub fn seventh_minor(&self) -> Minor67Default {
        self.seventh_minor
    }

    /// Returns the original figure.
    /// The digits written under the numeral, as music21's `figuresWritten`:
    /// the figure with the numeral, its accidental and its quality symbol
    /// taken off it, and nothing expanded.
    pub fn figures_written(&self) -> &str {
        &self.figures.written
    }

    /// The figured-bass column the numeral's digits stand for, expanded out
    /// of the shorthand they were written in: music21's `figuresNotationObj`.
    pub fn figures_notation(&self) -> &Notation {
        &self.figures.column
    }

    /// The figure the numeral was written with, as given: music21's `figure`.
    pub fn figure(&self) -> &str {
        &self.figure
    }

    /// Returns the one-based scale degree.
    pub fn degree(&self) -> u8 {
        self.degree
    }

    /// Returns the chromatic alteration of the scale degree in semitones.
    ///
    /// Negative values are flats and positive values are sharps, so `bII`
    /// returns `-1` and `#iv` returns `1`.
    pub fn written_accidental(&self) -> i8 {
        self.written_accidental
    }

    /// The alteration the numeral reports, once the sixth and seventh
    /// degrees of a minor key have been read against the chord asked for.
    pub fn accidental(&self) -> i8 {
        self.accidental
    }

    /// Returns the inversion number, where root position is `0`.
    pub fn inversion(&self) -> u8 {
        self.inversion
    }

    /// Returns the secondary/applied target figure, if any.
    pub fn secondary(&self) -> Option<&str> {
        self.secondary.as_deref()
    }

    /// Returns the key context.
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// The numeral with its front alteration and nothing else: music21's
    /// `romanNumeral`, so `bII6` is `bII` and `V65/V` is `V`. An augmented
    /// sixth answers its nationality, `It`, `Fr`, `Ger` or `Sw`.
    pub fn roman_numeral(&self) -> String {
        let numeral = self.roman_numeral_alone();
        if matches!(self.kind, RomanKind::AugmentedSixth(_)) {
            return numeral;
        }
        let prefix = match self.accidental {
            0 => String::new(),
            sharps if sharps > 0 => "#".repeat(sharps.unsigned_abs() as usize),
            flats => "b".repeat(flats.unsigned_abs() as usize),
        };
        format!("{prefix}{numeral}")
    }

    /// The numeral with nothing written in front of it: music21's
    /// `romanNumeralAlone`, so `bVII65/V` is `VII`.
    pub fn roman_numeral_alone(&self) -> String {
        if let RomanKind::AugmentedSixth(kind) = self.kind {
            return kind.figure().trim_end_matches(['+', '6']).to_string();
        }
        let primary = self.figure.split('/').next().unwrap_or_default();
        // A chord written by name is read as the figure it stands for
        // first, so the numeral alone of `N6` is `II` and of `Cad64` is the
        // tonic — which is what music21 answers.
        let reading = self.effective_key().unwrap_or_else(|_| self.key.clone());
        let named = named_figure(primary, &reading);
        let (_, unaltered) = split_roman_accidental_prefix(&named);
        unaltered
            .chars()
            .take_while(|c| matches!(c, 'i' | 'v' | 'x' | 'I' | 'V' | 'X'))
            .collect()
    }

    /// The figure and its key together: music21's `figureAndKey`,
    /// `bII6 in a minor`.
    pub fn figure_and_key(&self) -> String {
        format!(
            "{} in {} {}",
            self.figure,
            self.key.tonic_pitch_name_with_case(),
            self.key.mode()
        )
    }

    /// The scale degree with the alteration in front of it, as sharps
    /// (positive) or flats (negative): music21's `scaleDegreeWithAlteration`.
    pub fn scale_degree_with_alteration(&self) -> (u8, i8) {
        (self.degree, self.accidental)
    }

    /// How strongly the figure implies its function, from music21's
    /// `functionalityScores` table: `I` is 100, `V7` 80, an unknown figure 0.
    /// A secondary figure multiplies the scores of its halves.
    /// An augmented sixth is looked up by music21's spelling, `It6` rather
    /// than the `It+6` this crate writes.
    pub fn functionality_score(&self) -> u8 {
        if self.secondary.is_some() {
            let score = self.figure.split('/').fold(100.0, |score, part| {
                score * f64::from(functionality_score_of(part).unwrap_or(0)) / 100.0
            });
            return score as u8;
        }
        functionality_score_of(&self.figure.replace('+', "")).unwrap_or(0)
    }

    /// Whether this is a Neapolitan chord: a major triad on the flattened
    /// second degree, in first inversion unless `require_first_inversion` is
    /// off.
    pub fn is_neapolitan(&self, require_first_inversion: bool) -> bool {
        self.degree == 2
            && self.accidental == -1
            && self.quality == RomanQuality::Major
            && (!require_first_inversion || self.inversion == 1)
    }

    /// Whether the chord is borrowed from the parallel mode: music21's
    /// `isMixture`, so `iv` and `bVI` in a major key are mixture and `IV` in a
    /// minor key is. With `evaluate_secondary` a secondary figure is judged by
    /// the numeral after the slash.
    pub fn is_mixture(&self, evaluate_secondary: bool) -> Result<bool> {
        if evaluate_secondary && let Some(secondary) = &self.secondary {
            return RomanNumeral::new(secondary.clone(), self.key.clone())?.is_mixture(true);
        }
        if self.kind != RomanKind::Diatonic || !(1..=7).contains(&self.degree) {
            return Ok(false);
        }
        // music21 asks the *chord* for its quality here, and a chord answers
        // for its triad — so a half-diminished seventh reads as diminished,
        // and the seventh above the triad is what the degree-seven rule below
        // then asks about separately.
        let quality = match self.quality {
            RomanQuality::Diminished | RomanQuality::HalfDiminished => "diminished",
            RomanQuality::Minor => "minor",
            RomanQuality::Major => "major",
            RomanQuality::Augmented => return Ok(false),
        };
        let front = match self.accidental {
            0 => "natural",
            1 => "sharp",
            -1 => "flat",
            _ => "other",
        };
        let entry = (self.degree, quality, front);
        Ok(match self.key.mode() {
            "major" => {
                MAJOR_KEY_MIXTURES.contains(&entry)
                    || (self.degree == 7
                        && self.seventh
                        && self.quality == RomanQuality::Diminished)
            }
            "minor" => {
                MINOR_KEY_MIXTURES.contains(&entry)
                    || (self.degree == 7
                        && self.seventh
                        && self.quality == RomanQuality::HalfDiminished)
            }
            _ => false,
        })
    }

    /// The same figure in the key transposed by `interval`.
    pub fn transpose(&self, interval: &Interval) -> Result<Self> {
        RomanNumeral::new(self.figure.clone(), self.key.transpose(interval)?)
    }

    /// The key the figure is actually read in: the key it was given, or the
    /// one a secondary numeral establishes — the `V` of `V/V` in G major is
    /// read in D major.
    pub fn effective_key_of(&self) -> Result<Key> {
        self.effective_key()
    }

    fn effective_key(&self) -> Result<Key> {
        let Some(secondary) = &self.secondary else {
            return Ok(self.key.clone());
        };

        secondary_key(
            &self.key,
            secondary,
            self.sixth_minor,
            self.seventh_minor,
            self.case_matters,
        )
    }
}

impl fmt::Display for RomanNumeral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.figure())
    }
}

/// music21's `functionalityScores`: how strongly a figure implies its
/// harmonic function, on a hundred-point scale, in music21's order.
pub const FUNCTIONALITY_SCORES: [(&str, u8); 48] = [
    ("I", 100),
    ("i", 90),
    ("V7", 80),
    ("V", 70),
    ("V65", 68),
    ("I6", 65),
    ("V6", 63),
    ("V43", 61),
    ("I64", 60),
    ("IV", 59),
    ("i6", 58),
    ("viio7", 57),
    ("V42", 55),
    ("viio65", 53),
    ("viio6", 52),
    ("#viio65", 51),
    ("ii", 50),
    ("#viio6", 49),
    ("ii65", 48),
    ("ii43", 47),
    ("ii42", 46),
    ("IV6", 45),
    ("ii6", 43),
    ("VI", 42),
    ("#VI", 41),
    ("vi", 40),
    ("viio", 39),
    ("#viio", 38),
    ("iio", 37),
    ("iio42", 36),
    ("bII6", 35),
    ("It6", 34),
    ("Ger65", 33),
    ("iio43", 32),
    ("iio65", 31),
    ("Fr43", 30),
    ("#vio", 28),
    ("#vio6", 27),
    ("III", 22),
    ("Sw43", 21),
    ("v", 20),
    ("VII", 19),
    ("VII7", 18),
    ("IV65", 17),
    ("IV7", 16),
    ("iii", 15),
    ("iii6", 12),
    ("vi6", 10),
];

fn functionality_score_of(figure: &str) -> Option<u8> {
    FUNCTIONALITY_SCORES
        .iter()
        .find(|(known, _)| *known == figure)
        .map(|(_, score)| *score)
}

const MAJOR_KEY_MIXTURES: [(u8, &str, &str); 7] = [
    (1, "minor", "natural"),
    (2, "diminished", "natural"),
    (3, "major", "flat"),
    (4, "minor", "natural"),
    (5, "minor", "natural"),
    (6, "major", "flat"),
    (7, "major", "flat"),
];

const MINOR_KEY_MIXTURES: [(u8, &str, &str); 5] = [
    (1, "major", "natural"),
    (2, "minor", "natural"),
    (3, "minor", "sharp"),
    (4, "major", "natural"),
    (6, "minor", "sharp"),
];

/// Reads a chord as the tonic or the dominant of a key, with its inversion:
/// A pitch as a figure over a reference pitch: the scale step it stands
/// above the reference, how far it is altered from the key's own spelling of
/// that degree, and the accidental written in front of a figure for it.
/// music21's `FigureTuple`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct FigureTuple {
    /// The generic step above the reference pitch, `1` to `7`.
    pub deg_from_ref_pitch: u8,
    /// Semitones away from the key's own spelling of the degree.
    pub alter: FloatType,
    /// The accidental written in front of a figure for it: `#`, `b`, `##`
    /// and so on, or nothing.
    pub prefix: String,
}

/// A [`FigureTuple`] beside the pitch it describes: music21's
/// `PitchFigureTuple`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct PitchFigureTuple {
    /// The figure.
    pub figure: FigureTuple,
    /// The pitch it was read from.
    pub pitch: Pitch,
}

/// The figured-bass column a roman numeral's digits stand for, and what the
/// brackets beside them say.
///
/// This is music21's `figuredBass.notation.Notation` reduced to what a
/// numeral needs — the numbers above the bass and their accidentals, already
/// expanded out of the shorthand a figure is usually written in — together
/// with the omissions, additions and alterations music21 parses out of the
/// figure before the column is read.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct FiguredBass {
    /// The column itself, expanded out of the shorthand it was written in.
    column: Notation,
    /// The digits as the figure wrote them, before the shorthand was
    /// expanded and after everything that is not a digit has been read off:
    /// `V65` writes `65`, `I7#5b3` writes `7#5b3`, and `V` writes nothing.
    written: String,
    /// Chord steps the figure asks to be left out, as `[no3]`.
    omitted: Vec<u8>,
    /// Notes the figure asks to be added, as `[add4]`: the alteration in
    /// semitones and how far above the *root* the note stands.
    added: Vec<(i8, u8)>,
    /// Alterations written in square brackets, as `[#7]`: the alteration in
    /// semitones and the chord step it applies to.
    bracketed: Vec<(i8, u8)>,
}

impl FiguredBass {
    /// The numbers of the column, high to low.
    fn numbers(&self) -> Vec<u8> {
        self.column
            .numbers()
            .iter()
            .filter_map(|number| number.map(|number| number as u8))
            .collect()
    }

    /// The figures of the column, high to low.
    fn figures(&self) -> &[Figure] {
        self.column.figures()
    }

    /// Whether an accidental was written beside a given number, which is what
    /// stops the implied quality from correcting the note back.
    fn alters(&self, number: u8) -> bool {
        self.column.figures().iter().any(|figure| {
            figure.number() == Some(IntegerType::from(number))
                && figure
                    .modifier()
                    .accidental()
                    .is_some_and(|accidental| accidental.alter() != 0.0)
        })
    }
}

/// The quality a figure says its chord has, whatever the scale spells.
///
/// music21's `impliedQuality`: the symbol in front of the inversion digits,
/// or — when there is none — the case the numeral was written in. What it is
/// for is respelling the third, the fifth and the seventh once they have been
/// read off the scale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImpliedQuality {
    /// No quality was implied, and the scale's own spelling stands.
    #[default]
    Unstated,
    /// A major triad.
    Major,
    /// A minor triad.
    Minor,
    /// A diminished triad, or a fully diminished seventh.
    Diminished,
    /// A half-diminished seventh: a diminished triad under a minor seventh.
    HalfDiminished,
    /// An augmented triad.
    Augmented,
    /// A major triad under a minor seventh.
    DominantSeventh,
}

impl From<ImpliedQuality> for RomanQuality {
    /// The quality a numeral reports, which is the stated one with the two
    /// readings that only say how a seventh is spelled folded onto major.
    fn from(quality: ImpliedQuality) -> Self {
        match quality {
            ImpliedQuality::Minor => Self::Minor,
            ImpliedQuality::Diminished => Self::Diminished,
            ImpliedQuality::HalfDiminished => Self::HalfDiminished,
            ImpliedQuality::Augmented => Self::Augmented,
            ImpliedQuality::Unstated | ImpliedQuality::Major | ImpliedQuality::DominantSeventh => {
                Self::Major
            }
        }
    }
}

impl ImpliedQuality {
    /// The quality music21 names in a string, as it writes the names.
    pub fn from_name(name: &str) -> Self {
        match name {
            "major" => Self::Major,
            "minor" => Self::Minor,
            "diminished" => Self::Diminished,
            "half-diminished" => Self::HalfDiminished,
            "augmented" => Self::Augmented,
            "minor-seventh" | "dominant-seventh" => Self::DominantSeventh,
            _ => Self::Unstated,
        }
    }

    /// The name music21 writes it under, which is the empty string for a
    /// quality nobody stated.
    pub fn name(self) -> &'static str {
        match self {
            Self::Unstated => "",
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Diminished => "diminished",
            Self::HalfDiminished => "half-diminished",
            Self::Augmented => "augmented",
            Self::DominantSeventh => "dominant-seventh",
        }
    }

    /// How many semitones the third, the fifth and — where the quality says
    /// so — the seventh stand above the root.
    pub fn correct_semitones(self) -> &'static [u8] {
        match self {
            Self::Unstated => &[],
            Self::Major => &[4, 7],
            Self::Minor => &[3, 7],
            Self::Diminished => &[3, 6, 9],
            Self::HalfDiminished => &[3, 6, 10],
            Self::Augmented => &[4, 8],
            Self::DominantSeventh => &[4, 7, 10],
        }
    }
}

/// The numeral a figure opens with, and everything it implies.
///
/// This is what music21's `_parseRNAloneAmidstAug6` reads: usually just the
/// roman letters and the scale degree they name, but an augmented sixth is
/// written by nationality rather than by numeral and carries its own degree,
/// its own alteration and the accidentals that make it augmented — and it is
/// always read in the minor of the key it is written in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumeralAlone {
    /// The numeral as written, or the augmented sixth's name.
    pub numeral: String,
    /// What is left of the figure once the numeral is off it.
    pub rest: String,
    /// The scale degree the numeral stands on.
    pub degree: u8,
    /// The alteration in front of it, in semitones, where the numeral itself
    /// implies one.
    pub alteration: i8,
    /// Whether the figure has to be read in the parallel minor, as every
    /// augmented sixth is.
    pub minor: bool,
    /// The alterations that make an augmented sixth augmented.
    pub bracketed: Vec<(i8, u8)>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_numeral_reports_how_it_was_read() {
        use super::{
            ImpliedQuality, Minor67Default, NumeralAlone, RomanNumeral, analyze_chord_with_root,
            parse_numeral_alone, secondary_key,
        };
        use crate::{Chord, Key, Pitch};

        let key = Key::from_tonic("C").unwrap();
        let numeral = RomanNumeral::new("V7[no3][add4][#5]/ii", key.clone()).unwrap();
        assert_eq!(numeral.key().tonic().name(), "C");
        assert!(numeral.scale().is_none());
        assert!(numeral.case_matters());
        assert_eq!(numeral.sixth_minor(), Minor67Default::Quality);
        assert_eq!(numeral.seventh_minor(), Minor67Default::Quality);
        assert_eq!(numeral.implied_quality(), ImpliedQuality::Major);
        assert_eq!(numeral.written_accidental(), 0);
        assert_eq!(numeral.omitted_steps(), [3]);
        assert_eq!(numeral.added_steps(), [(0, 4)]);
        assert_eq!(numeral.bracketed_alterations(), [(1, 5)]);
        // A bare `7` expands to the seventh, fifth and third.
        assert_eq!(numeral.figures_notation().numbers().len(), 3);
        assert_eq!(numeral.effective_key_of().unwrap().tonic().name(), "D");
        assert_eq!(numeral.roman_numeral_alone(), "V");
        assert_eq!(numeral.to_string(), "V7[no3][add4][#5]/ii");
        assert_eq!(numeral.figure_and_key(), "V7[no3][add4][#5]/ii in C major");

        assert_eq!(
            ImpliedQuality::from_name("half-diminished").name(),
            "half-diminished"
        );
        assert_eq!(
            ImpliedQuality::from_name("nonsense"),
            ImpliedQuality::Unstated
        );
        assert_eq!(ImpliedQuality::Unstated.name(), "");

        let alone: NumeralAlone = parse_numeral_alone("VI6").unwrap();
        assert_eq!((alone.numeral.as_str(), alone.rest.as_str()), ("VI", "6"));
        assert_eq!((alone.degree, alone.alteration, alone.minor), (6, 0, false));
        let german = parse_numeral_alone("Ger").unwrap();
        assert_eq!(
            (german.numeral.as_str(), german.rest.as_str()),
            ("Ger", "65")
        );
        assert!(german.minor);
        assert!(parse_numeral_alone("").is_err());

        let dominant_of_dominant = secondary_key(
            &key,
            "V",
            Minor67Default::Quality,
            Minor67Default::Quality,
            true,
        )
        .unwrap();
        assert_eq!(dominant_of_dominant.tonic().name(), "G");
        assert_eq!(dominant_of_dominant.mode(), "major");
        let of_supertonic = secondary_key(
            &key,
            "ii",
            Minor67Default::Quality,
            Minor67Default::Quality,
            true,
        )
        .unwrap();
        assert_eq!(of_supertonic.mode(), "minor");

        let chord = Chord::new("E4 G4 C5").unwrap();
        let root = Pitch::from_name("C4").unwrap();
        let analyzed = analyze_chord_with_root(&chord, key, &root)
            .unwrap()
            .unwrap();
        assert_eq!(analyzed.figure(), "I6");
    }

    /// music21's own examples for `FigureTuple.fromPitchAndReference`,
    /// `figureTuples`, `correctRNAlterationForMinor` and
    /// `correctSuffixForChordQuality`.
    #[test]
    fn figures_are_read_above_a_reference_pitch_and_corrected_for_minor() {
        use super::{
            FigureTuple, correct_rn_alteration_for_minor, correct_suffix_for_chord_quality,
            figure_tuples,
        };
        use crate::{Chord, Key, Pitch};

        let c_major = Key::from_tonic("C").unwrap();
        let c_minor = Key::from_tonic("c").unwrap();
        let figure = |name: &str, key: &Key, reference: &str| {
            FigureTuple::from_pitch_and_reference(
                &Pitch::from_name(name).unwrap(),
                key,
                &Pitch::from_name(reference).unwrap(),
            )
            .unwrap()
        };
        let read = |figure: FigureTuple| (figure.deg_from_ref_pitch, figure.alter, figure.prefix);
        assert_eq!(
            read(figure("A-3", &c_major, "F#2")),
            (3, -1.0, "b".to_string())
        );
        assert_eq!(
            read(figure("E--4", &c_minor, "C3")),
            (3, -1.0, "b".to_string())
        );
        assert_eq!(read(figure("E-4", &c_minor, "C3")), (3, 0.0, String::new()));
        assert_eq!(
            read(figure("E#4", &c_minor, "C3")),
            (3, 2.0, "##".to_string())
        );
        assert_eq!(
            read(figure("A4", &c_minor, "C3")),
            (6, 1.0, "#".to_string())
        );
        assert_eq!(
            read(figure("B5", &c_minor, "C3")),
            (7, 1.0, "#".to_string())
        );

        let tuples = figure_tuples(&Chord::new("F#2 D3 A-3 C#4").unwrap(), &c_minor).unwrap();
        let read_all: Vec<(u8, FloatType, String)> = tuples
            .iter()
            .map(|tuple| read(tuple.figure.clone()))
            .collect();
        assert_eq!(
            read_all,
            [
                (1, 1.0, "#".to_string()),
                (6, 0.0, String::new()),
                (3, 0.0, String::new()),
                (5, 1.0, "#".to_string()),
            ]
        );
        assert_eq!(tuples[2].pitch.name_with_octave(), "A-3");
        assert!(
            figure_tuples(&Chord::new("").unwrap(), &c_minor)
                .unwrap()
                .is_empty()
        );

        let sixth = |alter: FloatType, prefix: &str| FigureTuple {
            deg_from_ref_pitch: 6,
            alter,
            prefix: prefix.to_string(),
        };
        assert_eq!(
            read(correct_rn_alteration_for_minor(
                &sixth(-1.0, ""),
                &c_minor,
                false
            )),
            (6, -1.0, "b".to_string())
        );
        assert_eq!(
            read(correct_rn_alteration_for_minor(
                &sixth(0.0, ""),
                &c_minor,
                false
            )),
            (6, 0.0, "b".to_string())
        );
        let raised_seventh = figure("B5", &c_minor, "C3");
        assert_eq!(
            read(correct_rn_alteration_for_minor(
                &raised_seventh,
                &c_minor,
                false
            )),
            (7, 0.0, String::new())
        );
        assert_eq!(
            read(correct_rn_alteration_for_minor(
                &raised_seventh,
                &c_minor,
                true
            )),
            (7, 1.0, "#".to_string())
        );
        assert_eq!(
            read(correct_rn_alteration_for_minor(
                &sixth(2.0, "##"),
                &c_minor,
                false
            )),
            (6, 1.0, "#".to_string())
        );
        let in_major = sixth(-1.0, "b");
        assert_eq!(
            correct_rn_alteration_for_minor(&in_major, &c_major, false),
            in_major
        );
        let fourth = FigureTuple {
            deg_from_ref_pitch: 4,
            alter: -1.0,
            prefix: "b".to_string(),
        };
        assert_eq!(
            correct_rn_alteration_for_minor(&fourth, &c_minor, false),
            fourth
        );

        assert_eq!(
            correct_suffix_for_chord_quality(&Chord::new("E3 C4 G4").unwrap(), "6"),
            "6"
        );
        assert_eq!(
            correct_suffix_for_chord_quality(&Chord::new("E3 C4 G-4").unwrap(), "6"),
            "o6"
        );
        assert_eq!(
            correct_suffix_for_chord_quality(&Chord::new("C4 E4 G#4").unwrap(), ""),
            "+"
        );
        assert_eq!(
            correct_suffix_for_chord_quality(&Chord::new("B3 D4 F4 A4").unwrap(), "7"),
            "ø7"
        );
        assert_eq!(
            correct_suffix_for_chord_quality(&Chord::new("B3 D4 F4 A-4").unwrap(), "o7"),
            "o7"
        );
    }

    /// music21's own examples: in C minor a minor `vi` is raised and a
    /// major `VI` is not, and in a major key nothing moves.
    #[test]
    fn the_sixth_and_seventh_of_a_minor_key_are_raised_by_quality() {
        use super::{ImpliedQuality, Minor67Default, adjust_minor_vi_and_vii_by_quality};
        use crate::Key;

        let minor = Key::from_tonic("c").unwrap();
        let major = Key::from_tonic("C").unwrap();
        let adjust = |key: &Key, reading, quality, accidental| {
            adjust_minor_vi_and_vii_by_quality(key, reading, quality, accidental)
        };
        assert_eq!(
            adjust(&minor, Minor67Default::Quality, ImpliedQuality::Minor, 0),
            1
        );
        assert_eq!(
            adjust(&minor, Minor67Default::Quality, ImpliedQuality::Major, 0),
            0
        );
        assert_eq!(
            adjust(&major, Minor67Default::Quality, ImpliedQuality::Minor, 0),
            0
        );
        assert_eq!(
            adjust(&minor, Minor67Default::Flat, ImpliedQuality::Diminished, 0),
            0
        );
        assert_eq!(
            adjust(&minor, Minor67Default::Sharp, ImpliedQuality::Major, 0),
            1
        );
        // A caution already written says nothing more; a flat cancels the
        // raise and the two meet at the natural degree.
        assert_eq!(
            adjust(&minor, Minor67Default::Cautionary, ImpliedQuality::Minor, 1),
            1
        );
        assert_eq!(
            adjust(
                &minor,
                Minor67Default::Cautionary,
                ImpliedQuality::Major,
                -1
            ),
            0
        );
        assert_eq!(
            adjust(&minor, Minor67Default::Cautionary, ImpliedQuality::Minor, 0),
            1
        );
    }

    #[test]
    fn a_numeral_reports_the_digits_it_was_written_with() {
        let major = Key::from_tonic("C").unwrap();
        for (figure, written) in [
            ("V65", "65"),
            ("V", ""),
            ("I7#5b3", "7#5b3"),
            ("viio6", "6"),
            ("vii\u{f8}7", "7"),
        ] {
            let numeral = RomanNumeral::new(figure, major.clone()).unwrap();
            assert_eq!(numeral.figures_written(), written, "{figure}");
        }
        let minor = Key::from_tonic("c").unwrap();
        for (figure, written) in [("Fr43", "43"), ("Fr+6", "43"), ("It+6", "6"), ("Ger", "65")] {
            let numeral = RomanNumeral::new(figure, minor.clone()).unwrap();
            assert_eq!(numeral.figures_written(), written, "{figure}");
        }
    }

    #[test]
    fn a_minor_key_raises_its_sixth_and_seventh_where_the_figure_asks() {
        let minor = Key::from_tonic_mode("a", Some("minor")).unwrap();
        // The diminished seventh on the leading note is the raised seventh
        // degree: G#, not G with three flats hung off it.
        let leading = RomanNumeral::new("viio7", minor.clone()).unwrap();
        assert_eq!(
            leading.to_chord().unwrap().pitch_names(),
            ["G#", "B", "D", "F"]
        );
        assert_eq!(leading.roman_numeral(), "#vii");
        assert_eq!(leading.roman_numeral_alone(), "vii");
        assert_eq!(leading.scale_degree_with_alteration(), (7, 1));

        // A minor triad on the sixth degree is the raised one too.
        assert_eq!(
            RomanNumeral::new("vi", minor.clone())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["F#", "A", "C#"]
        );
        // The natural degrees are what the upper-case figures ask for.
        assert_eq!(
            RomanNumeral::new("VI", minor.clone())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["F", "A", "C"]
        );
        // And a major key is left alone entirely.
        let major = Key::from_tonic_mode("C", Some("major")).unwrap();
        assert_eq!(
            RomanNumeral::new("vi", major)
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names(),
            ["A", "C", "E"]
        );
    }

    #[test]
    fn the_four_readings_of_a_minor_sixth_and_seventh() {
        let minor = Key::from_tonic_mode("c", Some("minor")).unwrap();
        let read = |figure: &str, sixth: Minor67Default| {
            RomanNumeral::with_minor_defaults(figure, minor.clone(), sixth, sixth)
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names()
                .join(" ")
        };

        // By quality, which is the default: the chord the figure asks for
        // says which sixth it is built on.
        assert_eq!(read("vi", Minor67Default::Quality), "A C E");
        assert_eq!(read("VI", Minor67Default::Quality), "A- C E-");

        // Flat is always the natural degree, whatever the figure asks for,
        // and a sharp written in front still raises it.
        assert_eq!(read("vi", Minor67Default::Flat), "A- C- E-");
        assert_eq!(read("#vi", Minor67Default::Flat), "A C E");

        // Sharp is always the raised one, and a flat still lowers it.
        assert_eq!(read("VI", Minor67Default::Sharp), "A C# E");
        assert_eq!(read("bVI", Minor67Default::Sharp), "A- C E-");

        // Cautionary reads the quality, but an accidental already written is
        // a caution rather than a further change, so `#vi` is `vi` and
        // `bVI` is `VI`.
        assert_eq!(read("#vi", Minor67Default::Cautionary), "A C E");
        assert_eq!(read("vi", Minor67Default::Cautionary), "A C E");
        assert_eq!(read("bVI", Minor67Default::Cautionary), "A- C E-");
        assert_eq!(read("VI", Minor67Default::Cautionary), "A- C E-");
    }

    #[test]
    fn the_neapolitan_can_be_written_by_name() {
        // music21 writes the flattened second degree as `N`, and in first
        // inversion — the way it is nearly always used — as `N6`.
        let key = Key::from_tonic_mode("c#", Some("minor")).unwrap();
        let named = RomanNumeral::new("N6", key.clone()).unwrap();
        assert_eq!(named.figure(), "N6");
        assert_eq!(named.degree(), 2);
        assert_eq!(named.accidental(), -1);
        assert_eq!(named.inversion(), 1);
        assert!(named.is_neapolitan(true));

        let spelled = RomanNumeral::new("bII6", key.clone()).unwrap();
        assert_eq!(
            named.to_chord().unwrap().pitch_names(),
            spelled.to_chord().unwrap().pitch_names()
        );

        // `N53` is the same chord in root position.
        let root_position = RomanNumeral::new("N53", key).unwrap();
        assert_eq!(root_position.inversion(), 0);
    }

    #[test]
    fn inversion_names_and_tonic_dominant_reading_match_music21() {
        let chord = |notes: &str| Chord::new(notes).unwrap();
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), None), "");
        assert_eq!(roman_inversion_name(&chord("E4 G4 C5"), None), "6");
        assert_eq!(roman_inversion_name(&chord("G3 C4 E4"), None), "64");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4 B-4"), None), "7");
        assert_eq!(roman_inversion_name(&chord("E4 G4 B-4 C5"), None), "65");
        assert_eq!(roman_inversion_name(&chord("G3 B-3 C4 E4"), None), "43");
        assert_eq!(roman_inversion_name(&chord("B-3 C4 E4 G4"), None), "42");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), Some(2)), "64");
        assert_eq!(roman_inversion_name(&chord("C4 E4 G4"), Some(3)), "");
        assert_eq!(roman_inversion_name(&Chord::empty(), None), "");

        let cases = [
            ("C4 E4 G4", "C", Some("I")),
            ("E4 G4 C5", "C", Some("I6")),
            ("G3 B3 D4 F4", "C", Some("V7")),
            ("D4 F4 A4", "C", Some("V43")),
            ("A3 C4 E4", "a", Some("i")),
            ("E3 G#3 B3", "a", Some("V")),
            ("C4 E4", "C", Some("I")),
            ("G4 D5", "C", Some("V")),
            ("B3 D4", "C", Some("V")),
            ("F#4 A4", "C", None),
        ];
        for (notes, key, expected) in cases {
            let reading =
                identify_as_tonic_or_dominant(&chord(notes), &Key::from_tonic(key).unwrap())
                    .unwrap();
            assert_eq!(reading.as_deref(), expected, "{notes} in {key}");
        }
    }

    #[test]
    #[allow(clippy::type_complexity)]
    fn numeral_names_scores_and_mixture_match_music21() {
        let cases: [(&str, &str, &str, &str, (u8, i8), u8, bool, bool, bool); 18] = [
            (
                "V7",
                "C",
                "V",
                "V7 in C major",
                (5, 0),
                80,
                false,
                false,
                false,
            ),
            (
                "bII6",
                "C",
                "bII",
                "bII6 in C major",
                (2, -1),
                35,
                true,
                true,
                false,
            ),
            (
                "bII",
                "C",
                "bII",
                "bII in C major",
                (2, -1),
                0,
                false,
                true,
                false,
            ),
            (
                "bII6",
                "a",
                "bII",
                "bII6 in a minor",
                (2, -1),
                35,
                true,
                true,
                false,
            ),
            (
                "viio7",
                "C",
                "vii",
                "viio7 in C major",
                (7, 0),
                57,
                false,
                false,
                true,
            ),
            (
                "I",
                "C",
                "I",
                "I in C major",
                (1, 0),
                100,
                false,
                false,
                false,
            ),
            (
                "i",
                "a",
                "i",
                "i in a minor",
                (1, 0),
                90,
                false,
                false,
                false,
            ),
            (
                "V65/V",
                "C",
                "V",
                "V65/V in C major",
                (5, 0),
                47,
                false,
                false,
                false,
            ),
            (
                "iv",
                "C",
                "iv",
                "iv in C major",
                (4, 0),
                0,
                false,
                false,
                true,
            ),
            (
                "bVI",
                "C",
                "bVI",
                "bVI in C major",
                (6, -1),
                0,
                false,
                false,
                true,
            ),
            (
                "I",
                "a",
                "I",
                "I in a minor",
                (1, 0),
                100,
                false,
                false,
                true,
            ),
            (
                "III",
                "a",
                "III",
                "III in a minor",
                (3, 0),
                22,
                false,
                false,
                false,
            ),
            (
                "#iii",
                "a",
                "#iii",
                "#iii in a minor",
                (3, 1),
                0,
                false,
                false,
                true,
            ),
            (
                "ii",
                "C",
                "ii",
                "ii in C major",
                (2, 0),
                50,
                false,
                false,
                false,
            ),
            (
                "iio",
                "C",
                "ii",
                "iio in C major",
                (2, 0),
                37,
                false,
                false,
                true,
            ),
            (
                "IV",
                "a",
                "IV",
                "IV in a minor",
                (4, 0),
                59,
                false,
                false,
                true,
            ),
            (
                "bVII",
                "C",
                "bVII",
                "bVII in C major",
                (7, -1),
                0,
                false,
                false,
                true,
            ),
            (
                "v",
                "C",
                "v",
                "v in C major",
                (5, 0),
                20,
                false,
                false,
                true,
            ),
        ];
        for (figure, key, numeral, figure_and_key, degree, score, neapolitan, loose, mixture) in
            cases
        {
            let rn = RomanNumeral::new(figure, Key::from_tonic(key).unwrap()).unwrap();
            assert_eq!(rn.roman_numeral(), numeral, "{figure} in {key}");
            assert_eq!(rn.figure_and_key(), figure_and_key, "{figure} in {key}");
            assert_eq!(
                rn.scale_degree_with_alteration(),
                degree,
                "{figure} in {key}"
            );
            assert_eq!(rn.functionality_score(), score, "{figure} in {key}");
            assert_eq!(rn.is_neapolitan(true), neapolitan, "{figure} in {key}");
            assert_eq!(rn.is_neapolitan(false), loose, "{figure} in {key}");
            assert_eq!(rn.is_mixture(false).unwrap(), mixture, "{figure} in {key}");
            assert_eq!(rn.is_mixture(true).unwrap(), mixture, "{figure} in {key}");
        }
        // A half-diminished seventh on the leading note is mixture in a minor
        // key and not in a major one, which only works because its quality
        // reads as its triad's — diminished — the way music21's chord does.
        for (key, mixture) in [("a", true), ("A", false)] {
            let half = RomanNumeral::new("viiø7", Key::from_tonic(key).unwrap()).unwrap();
            assert_eq!(half.is_mixture(false).unwrap(), mixture, "viiø7 in {key}");
        }

        let italian = RomanNumeral::new("It6", Key::from_tonic("C").unwrap()).unwrap();
        assert_eq!(italian.roman_numeral(), "It");
        assert_eq!(italian.functionality_score(), 34);
        assert!(!italian.is_mixture(false).unwrap());
        let transposed = RomanNumeral::new("bII6", Key::from_tonic("a").unwrap())
            .unwrap()
            .transpose(&Interval::from_name("M2").unwrap())
            .unwrap();
        assert_eq!(transposed.figure_and_key(), "bII6 in b minor");
    }

    #[test]
    fn functionality_scores_table_has_no_duplicates() {
        let mut figures: Vec<&str> = FUNCTIONALITY_SCORES.iter().map(|(f, _)| *f).collect();
        figures.sort_unstable();
        figures.dedup();
        assert_eq!(figures.len(), FUNCTIONALITY_SCORES.len());
    }
    use super::*;

    #[test]
    fn secondary_dominant_resolves_to_chord() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let rn = RomanNumeral::new("V7/V", key).unwrap();
        assert_eq!(rn.degree(), 5);
        assert_eq!(rn.secondary(), Some("V"));
        assert_eq!(
            rn.to_chord().unwrap().pitched_common_name(),
            "D-dominant seventh chord"
        );
    }

    #[test]
    fn analyzes_chord_in_key() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let chord = Chord::new("G B D F").unwrap();
        let rn = RomanNumeral::analyze(&chord, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "V7");
    }

    #[test]
    fn analyzes_accidentals_inversions_and_half_diminished_quality() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        let neapolitan = Chord::new("D- F A-").unwrap();
        let rn = RomanNumeral::analyze(&neapolitan, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "bII");
        assert_eq!(rn.degree(), 2);
        assert_eq!(rn.accidental(), -1);

        let first_inversion = Chord::new("E4 G4 C5").unwrap();
        let rn = RomanNumeral::analyze(&first_inversion, key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I6");

        let leading_tone = Chord::new("B D F A").unwrap();
        let rn = RomanNumeral::analyze(&leading_tone, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "vii\u{00f8}7");
    }

    #[test]
    fn analyzes_with_explicit_root_for_browser_style_sets() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let chord = Chord::new("C E G").unwrap();
        let rn = RomanNumeral::analyze_with_root(&chord, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I");

        let seventh = Chord::new("C E G B-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&seventh, key, &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "I7");
    }

    #[test]
    fn analyzes_augmented_sixth_chords_functionally() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let root = Pitch::from_name("C").unwrap();
        let french = Chord::new("C D F# A-").unwrap();
        let rn = RomanNumeral::analyze_with_root(&french, key.clone(), &root)
            .unwrap()
            .unwrap();
        assert_eq!(rn.figure(), "Fr+6");

        let german = Chord::new("A- C E- F#").unwrap();
        let rn = RomanNumeral::analyze(&german, key).unwrap().unwrap();
        assert_eq!(rn.figure(), "Ger+6");
    }

    #[test]
    fn figured_bass_shorthands_give_music21_inversions() {
        // Captured from music21's RomanNumeral(...).inversion(). `V642` holds
        // `64` inside it and is a third inversion all the same.
        let key = Key::from_tonic_mode("C", "major").unwrap();
        for (figure, expected) in [
            ("V", 0),
            ("V6", 1),
            ("V64", 2),
            ("V7", 0),
            ("V65", 1),
            ("V43", 2),
            ("V42", 3),
            ("V642", 3),
            ("V653", 1),
            ("V643", 2),
            ("V63", 1),
            ("V53", 0),
            ("V9", 0),
        ] {
            let numeral = RomanNumeral::new(figure, key.clone())
                .unwrap_or_else(|err| panic!("{figure} should parse: {err}"));
            assert_eq!(numeral.inversion(), expected, "{figure}");
        }
    }

    #[test]
    fn a_figure_expands_into_a_figured_bass_column() {
        // music21's shorthand: a bare `7` is a seventh chord in root
        // position, and `43` is the same chord over its fifth.
        let numbers = |figure: &str| {
            RomanNumeral::new(figure, Key::from_tonic("C").unwrap())
                .unwrap()
                .figure_numbers()
        };
        assert_eq!(numbers("V"), vec![5, 3]);
        assert_eq!(numbers("V7"), vec![7, 5, 3]);
        assert_eq!(numbers("V65"), vec![6, 5, 3]);
        assert_eq!(numbers("V43"), vec![6, 4, 3]);
        assert_eq!(numbers("V9"), vec![9, 7, 5, 3]);
        // A column written out is left as written, alterations and all.
        assert_eq!(numbers("V7#5b3"), vec![7, 5, 3]);
    }

    #[test]
    fn a_column_says_which_degree_is_in_the_bass() {
        // A sixth and a third over the bass is a triad in first inversion, so
        // the bass of a `V6` is the seventh degree.
        assert_eq!(bass_scale_degree_from_notation(5, &[6, 3]).unwrap(), 7);
        assert_eq!(bass_scale_degree_from_notation(1, &[5, 3]).unwrap(), 1);
        assert_eq!(bass_scale_degree_from_notation(2, &[6, 5, 3]).unwrap(), 4);
        // A column that implies no root leaves the degree where it was.
        assert_eq!(bass_scale_degree_from_notation(5, &[5, 4]).unwrap(), 5);
    }

    #[test]
    fn a_figure_alters_one_note_rather_than_naming_another_chord() {
        let names = |figure: &str, key: &str| {
            RomanNumeral::new(figure, Key::from_tonic(key).unwrap())
                .unwrap()
                .to_chord()
                .unwrap()
                .pitch_names()
        };
        // Writing the fifth out says the column is `7,b5` and nothing else,
        // so the third music21 would have implied is gone with it.
        assert_eq!(names("V7b5", "C"), ["G", "D-", "F"]);
        assert_eq!(names("V[no3]", "F"), ["C", "G"]);
        assert_eq!(names("I[add4][no3]", "C"), ["C", "F", "G"]);
        // The sharp of `i#7` raises the flattened seventh of a minor key to a
        // natural, rather than spelling a `B#`.
        assert_eq!(names("i#7", "c"), ["C", "E-", "G", "B"]);
        assert_eq!(names("i#7", "C"), ["C", "E-", "G", "B#"]);
    }

    #[test]
    fn a_numeral_can_be_read_over_a_scale_that_is_not_a_key() {
        // music21 reads a numeral against any concrete scale, and an
        // octatonic one has eight degrees rather than seven.
        let scale = crate::scale::Scale::new(
            crate::scale::ScaleType::Octatonic,
            Pitch::from_name("C2").unwrap(),
        );
        let numeral = RomanNumeral::over_scale(
            "I9",
            Key::from_tonic("C").unwrap(),
            Some(scale),
            Minor67Default::Quality,
            Minor67Default::Quality,
            false,
        )
        .unwrap();
        let pitches: Vec<String> = numeral
            .to_chord()
            .unwrap()
            .pitches()
            .iter()
            .map(Pitch::name_with_octave)
            .collect();
        assert_eq!(pitches, ["C2", "E-2", "G-2", "A2", "C3"]);
    }

    #[test]
    fn roman_numerals_parse_inversions_and_qualities() {
        let key = Key::from_tonic_mode("C", "major").unwrap();
        let first_inversion = RomanNumeral::new("I6", key.clone()).unwrap();
        assert_eq!(first_inversion.inversion(), 1);
        assert_eq!(
            first_inversion
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G", "C"]
        );

        let diminished = RomanNumeral::new("viio7", key.clone()).unwrap();
        assert_eq!(diminished.degree(), 7);
        assert!(
            diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("diminished")
        );

        let half_diminished = RomanNumeral::new("vii\u{00f8}7", key.clone()).unwrap();
        assert_eq!(half_diminished.degree(), 7);
        assert_eq!(half_diminished.accidental(), 0);
        assert!(
            half_diminished
                .to_chord()
                .unwrap()
                .common_name()
                .contains("half-diminished")
        );

        let borrowed = RomanNumeral::new("bII", key.clone()).unwrap();
        assert_eq!(borrowed.degree(), 2);
        assert_eq!(borrowed.accidental(), -1);

        let added_thirteenth = RomanNumeral::new("I add(13)", key.clone()).unwrap();
        assert_eq!(added_thirteenth.inversion(), 0);
        assert_eq!(
            added_thirteenth.to_chord().unwrap().common_name(),
            "major triad"
        );

        let augmented = RomanNumeral::new("III+", key).unwrap();
        assert_eq!(
            augmented
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["E", "G#", "B#"]
        );
    }

    #[test]
    fn roman_numerals_parse_augmented_sixth_figures() {
        let key = Key::from_tonic_mode("C", "minor").unwrap();
        let french = RomanNumeral::new("Fr+6", key).unwrap();
        // music21 reads a French sixth on the second degree, with nothing
        // written in front of it; the sharp that makes it augmented is the
        // bracketed one on its third.
        assert_eq!(french.degree(), 2);
        assert_eq!(french.accidental(), 0);
        assert_eq!(
            french
                .to_chord()
                .unwrap()
                .pitches()
                .into_iter()
                .map(|pitch| pitch.name())
                .collect::<Vec<_>>(),
            vec!["A-", "C", "D", "F#"]
        );
    }

    #[test]
    fn roman_numerals_report_invalid_figures_and_empty_analysis() {
        let key = Key::from_tonic_mode("C", "major").unwrap();

        assert!(RomanNumeral::new("", key.clone()).is_err());
        assert!(RomanNumeral::new("Q", key.clone()).is_err());
        assert!(analyze_chord(&Chord::empty(), key).unwrap().is_none());
    }
}
