use crate::{
    defaults::{FloatType, FractionType, IntegerType},
    error::{Error, Result},
};

use fraction::ToPrimitive;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// A note-value name, as music21's `duration.typeToDuration` defines them.
///
/// Each type is a power-of-two multiple of a quarter note, from the
/// `duplex-maxima` (sixteen whole notes) down to the `2048th`, plus the `zero`
/// length music21 uses for grace notes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DurationType {
    /// Duplex maxima, sixteen whole notes.
    DuplexMaxima,
    /// Maxima, eight whole notes.
    Maxima,
    /// Longa, four whole notes.
    Longa,
    /// Breve, or double whole note.
    Breve,
    /// Whole note.
    Whole,
    /// Half note.
    Half,
    /// Quarter note.
    Quarter,
    /// Eighth note.
    Eighth,
    /// Sixteenth note.
    Sixteenth,
    /// Thirty-second note.
    ThirtySecond,
    /// Sixty-fourth note.
    SixtyFourth,
    /// Hundred-twenty-eighth note.
    HundredTwentyEighth,
    /// Two-hundred-fifty-sixth note.
    TwoHundredFiftySixth,
    /// Five-hundred-twelfth note.
    FiveHundredTwelfth,
    /// Ten-twenty-fourth note.
    TenTwentyFourth,
    /// Twenty-forty-eighth note.
    TwentyFortyEighth,
    /// A grace-note duration of no length.
    Zero,
}

impl DurationType {
    /// Every duration type, longest first, matching music21's ordering.
    pub const ALL: [DurationType; 17] = [
        Self::DuplexMaxima,
        Self::Maxima,
        Self::Longa,
        Self::Breve,
        Self::Whole,
        Self::Half,
        Self::Quarter,
        Self::Eighth,
        Self::Sixteenth,
        Self::ThirtySecond,
        Self::SixtyFourth,
        Self::HundredTwentyEighth,
        Self::TwoHundredFiftySixth,
        Self::FiveHundredTwelfth,
        Self::TenTwentyFourth,
        Self::TwentyFortyEighth,
        Self::Zero,
    ];

    /// Returns the music21 type name, such as `"whole"` or `"16th"`.
    pub fn music21_name(self) -> &'static str {
        match self {
            Self::DuplexMaxima => "duplex-maxima",
            Self::Maxima => "maxima",
            Self::Longa => "longa",
            Self::Breve => "breve",
            Self::Whole => "whole",
            Self::Half => "half",
            Self::Quarter => "quarter",
            Self::Eighth => "eighth",
            Self::Sixteenth => "16th",
            Self::ThirtySecond => "32nd",
            Self::SixtyFourth => "64th",
            Self::HundredTwentyEighth => "128th",
            Self::TwoHundredFiftySixth => "256th",
            Self::FiveHundredTwelfth => "512th",
            Self::TenTwentyFourth => "1024th",
            Self::TwentyFortyEighth => "2048th",
            Self::Zero => "zero",
        }
    }

    /// music21's `ordinal`: the position in the list of note values from the
    /// duplex maxima (`0`) down to the 2048th (`15`). `None` for `Zero`.
    pub fn ordinal(self) -> Option<usize> {
        (self != Self::Zero).then(|| Self::ALL.iter().position(|kind| *kind == self))?
    }

    /// The next longer note value: music21's `nextLargerType`, so a quarter
    /// gives a half. `None` above the duplex maxima and for `Zero`.
    pub fn next_larger(self) -> Option<DurationType> {
        let ordinal = self.ordinal()?;
        Self::ALL.get(ordinal.checked_sub(1)?).copied()
    }

    /// The next shorter note value: music21's `nextSmallerType`, so a quarter
    /// gives an eighth. `None` below the 2048th and for `Zero`.
    pub fn next_smaller(self) -> Option<DurationType> {
        let ordinal = self.ordinal()?;
        Self::ALL
            .get(ordinal + 1)
            .copied()
            .filter(|kind| *kind != Self::Zero)
    }

    /// Returns music21's type number: how many of this note value make a
    /// whole note, so a quarter is `4` and a breve `0.5`. `None` for `Zero`.
    pub fn type_number(self) -> Option<FloatType> {
        (self != Self::Zero).then(|| 4.0 / self.quarter_length())
    }

    fn title(self) -> String {
        let name = self.music21_name();
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            return name.to_string();
        }
        name.split('-')
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Returns the length of one undotted note of this type, in quarter lengths.
    pub fn quarter_length(self) -> FloatType {
        match self {
            Self::DuplexMaxima => 64.0,
            Self::Maxima => 32.0,
            Self::Longa => 16.0,
            Self::Breve => 8.0,
            Self::Whole => 4.0,
            Self::Half => 2.0,
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::Sixteenth => 0.25,
            Self::ThirtySecond => 0.125,
            Self::SixtyFourth => 0.0625,
            Self::HundredTwentyEighth => 0.03125,
            Self::TwoHundredFiftySixth => 0.015625,
            Self::FiveHundredTwelfth => 0.0078125,
            Self::TenTwentyFourth => 0.00390625,
            Self::TwentyFortyEighth => 0.001953125,
            Self::Zero => 0.0,
        }
    }

    /// Parses a music21 type name.
    pub fn from_music21_name(name: &str) -> Option<Self> {
        match name {
            "duplex-maxima" => Some(Self::DuplexMaxima),
            "maxima" => Some(Self::Maxima),
            "longa" => Some(Self::Longa),
            "breve" => Some(Self::Breve),
            "whole" => Some(Self::Whole),
            "half" => Some(Self::Half),
            "quarter" => Some(Self::Quarter),
            "eighth" => Some(Self::Eighth),
            "16th" => Some(Self::Sixteenth),
            "32nd" => Some(Self::ThirtySecond),
            "64th" => Some(Self::SixtyFourth),
            "128th" => Some(Self::HundredTwentyEighth),
            "256th" => Some(Self::TwoHundredFiftySixth),
            "512th" => Some(Self::FiveHundredTwelfth),
            "1024th" => Some(Self::TenTwentyFourth),
            "2048th" => Some(Self::TwentyFortyEighth),
            "zero" => Some(Self::Zero),
            _ => None,
        }
    }

    /// Returns the type whose undotted length is exactly `quarter_length`.
    pub fn from_quarter_length(quarter_length: FloatType) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.quarter_length() == quarter_length)
    }

    /// Returns the length of this type carrying `dots` augmentation dots.
    ///
    /// Each dot adds half of what came before, so a dotted half is `3.0` and a
    /// double-dotted half is `3.5`.
    pub fn quarter_length_with_dots(self, dots: u32) -> FloatType {
        self.quarter_length() * (2.0 - (0.5 as FloatType).powi(dots as i32))
    }
}

impl Display for DurationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.music21_name())
    }
}

impl FromStr for DurationType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::from_music21_name(value)
            .ok_or_else(|| Error::Duration(format!("unknown duration type {value:?}")))
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Rhythmic duration measured in quarter lengths.
///
/// A quarter note has a quarter length of `1.0`; an eighth note is `0.5`;
/// a whole note is `4.0`.
pub struct Duration {
    quarter_length: FloatType,
    /// The tuplets this length is written inside, when a caller has said so.
    ///
    /// `None` is the ordinary case: nobody has said, so the tuplet is read
    /// off the length by [`Duration::tuplet`]. `Some` is what a caller set,
    /// an empty list included — saying "written inside no tuplet at all" is
    /// different from saying nothing, and music21 keeps the difference too.
    #[cfg_attr(feature = "serde", serde(default))]
    tuplets: Option<Vec<Tuplet>>,
}

/// The numerators music21 searches when reading a length as a tuplet: its
/// `defaultTupletNumerators`. Four in the time of three is deliberately
/// absent, since that length is a dotted note.
const TUPLET_NUMERATORS: [u32; 5] = [3, 5, 7, 11, 13];

/// The dot counts music21 allows inside a tuplet: its
/// `POSSIBLE_DOTS_IN_TUPLETS`.
const TUPLET_DOTS: [u32; 2] = [0, 1];

/// How many written values music21 will tie together before it gives up, its
/// `range(8)`.
const MAX_TIED_COMPONENTS: usize = 8;

/// How close a length has to be to a tuplet's to be read as one, relative to
/// the length itself.
///
/// music21 snaps a quarter length onto a limited-denominator fraction before
/// it looks, which is why `0.333333` is a triplet eighth there; this is the
/// same latitude for a crate that keeps the length as a float.
const TUPLET_TOLERANCE: FloatType = 1e-5;

/// A length written as a tuplet: `actual` notes of one written value in the
/// time of `normal` of them.
///
/// This is the naming half of music21's `Tuplet`. A [`Duration`] here is a
/// quarter length, so a tuplet is something a length is *read as* rather
/// than something a duration carries — what it is for is being able to say
/// that two thirds of a quarter is a quarter triplet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tuplet {
    actual: u32,
    normal: u32,
    duration_type: DurationType,
    dots: u32,
    /// The written value the `normal` count is counted in, which is not
    /// always the one the `actual` count is written as: three eighths in the
    /// time of one quarter is the same ratio as three in the time of two
    /// eighths, and music21 keeps both spellings.
    normal_type: DurationType,
    /// The dots on that value.
    normal_dots: u32,
}

impl Tuplet {
    /// A tuplet of `actual` notes of a written value in the time of
    /// `normal` of them.
    pub fn new(actual: u32, normal: u32, duration_type: DurationType, dots: u32) -> Self {
        Self {
            actual,
            normal,
            duration_type,
            dots,
            normal_type: duration_type,
            normal_dots: dots,
        }
    }

    /// The same tuplet counting the `normal` side in a different written
    /// value: music21's `durationNormal` apart from its `durationActual`.
    pub fn with_normal(mut self, duration_type: DurationType, dots: u32) -> Self {
        self.normal_type = duration_type;
        self.normal_dots = dots;
        self
    }

    /// The written value the `normal` count is counted in.
    pub fn normal_duration_type(&self) -> DurationType {
        self.normal_type
    }

    /// The dots on that value.
    pub fn normal_dots(&self) -> u32 {
        self.normal_dots
    }

    /// How long the whole tuplet lasts: music21's `totalTupletLength`, the
    /// `normal` count of the value it is counted in.
    pub fn total_tuplet_length(&self) -> FloatType {
        FloatType::from(self.normal) * self.normal_type.quarter_length_with_dots(self.normal_dots)
    }

    /// How many notes are played: music21's `numberNotesActual`.
    pub fn actual(&self) -> u32 {
        self.actual
    }

    /// How many notes they are played in the time of: `numberNotesNormal`.
    pub fn normal(&self) -> u32 {
        self.normal
    }

    /// The note value each one is written as.
    pub fn duration_type(&self) -> DurationType {
        self.duration_type
    }

    /// How many augmentation dots that written value carries.
    pub fn dots(&self) -> u32 {
        self.dots
    }

    /// What the written length is multiplied by inside the tuplet:
    /// music21's `tupletMultiplier`, `normal / actual`.
    pub fn multiplier(&self) -> FractionType {
        FractionType::new(
            IntegerType::try_from(self.normal).unwrap_or(IntegerType::MAX),
            IntegerType::try_from(self.actual).unwrap_or(IntegerType::MAX),
        )
    }

    /// music21's `Tuplet.fullName`: the familiar name for the ratios that
    /// have one, and `Tuplet of 17/14ths` for the ones that do not.
    pub fn full_name(&self) -> String {
        match (self.actual, self.normal) {
            (3, 2) => "Triplet".to_string(),
            (5, 4 | 2) => "Quintuplet".to_string(),
            (6, 4) => "Sextuplet".to_string(),
            (7, 4) => "Septuplet".to_string(),
            (actual, normal) => format!(
                "Tuplet of {actual}/{normal}{}s",
                ordinal_abbreviation(normal)
            ),
        }
    }
}

/// The `st`, `nd`, `rd` or `th` that follows a number: music21's
/// `ordinalAbbreviation`, which the tuplet ratios without a name use.
fn ordinal_abbreviation(value: u32) -> &'static str {
    if matches!(value % 100, 11..=13) {
        return "th";
    }
    match value % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

/// The word music21 puts in front of a note value for its dots. The mensural
/// values say it differently: an undotted longa is *imperfect* and a dotted
/// one *perfect*.
fn dot_prefix(dots: u32, mensural: bool) -> &'static str {
    match dots {
        0 if mensural => "Imperfect ",
        1 if mensural => "Perfect ",
        0 => "",
        1 => "Dotted ",
        2 => "Double Dotted ",
        3 => "Triple Dotted ",
        _ => "Quadruple Dotted ",
    }
}

impl Duration {
    /// Creates a duration from a quarter-length value.
    pub fn new(quarter_length: FloatType) -> Result<Self> {
        if !quarter_length.is_finite() || quarter_length < 0.0 {
            return Err(Error::Duration(format!(
                "duration quarter length must be finite and non-negative, got {quarter_length}"
            )));
        }

        Ok(Self {
            quarter_length,
            tuplets: None,
        })
    }

    /// Returns a quarter-note duration.
    pub fn quarter() -> Self {
        Self::from_type(DurationType::Quarter)
    }

    /// Returns a half-note duration.
    pub fn half() -> Self {
        Self::from_type(DurationType::Half)
    }

    /// Returns a whole-note duration.
    pub fn whole() -> Self {
        Self::from_type(DurationType::Whole)
    }

    /// Returns an eighth-note duration.
    pub fn eighth() -> Self {
        Self::from_type(DurationType::Eighth)
    }

    /// Creates a duration from a note-value type.
    pub fn from_type(duration_type: DurationType) -> Self {
        Self {
            quarter_length: duration_type.quarter_length(),
            tuplets: None,
        }
    }

    /// Creates a duration from a note-value type carrying augmentation dots.
    pub fn from_type_with_dots(duration_type: DurationType, dots: u32) -> Self {
        Self {
            quarter_length: duration_type.quarter_length_with_dots(dots),
            tuplets: None,
        }
    }

    /// Returns the note-value type and dot count that together make exactly
    /// this length, such as a half note with one dot for `3.0`. `None` for a
    /// length no single dotted note value has, such as a tuplet or a tie.
    pub fn type_and_dots(&self) -> Option<(DurationType, u32)> {
        exact_type_and_dots(self.quarter_length)
    }

    /// Returns the augmentation dots on this duration, or `0` when it is not
    /// a single dotted note value.
    pub fn dots(&self) -> u32 {
        self.type_and_dots().map_or(0, |(_, dots)| dots)
    }

    /// music21's `ordinal` for the duration's type, or `None` where music21
    /// says `complex` or the duration is zero: a duration whose quarter length
    /// is not a single dotted note value.
    pub fn ordinal(&self) -> Option<usize> {
        self.type_and_dots()?.0.ordinal()
    }

    /// Returns the duration scaled by a positive factor: music21's
    /// `augmentOrDiminish`, so a quarter by two is a half.
    pub fn augment_or_diminish(&self, factor: FloatType) -> Result<Duration> {
        if factor.is_nan() || factor <= 0.0 {
            return Err(Error::Duration(
                "amountToScale must be greater than zero".to_string(),
            ));
        }
        Duration::new(self.quarter_length * factor)
    }

    /// The tuplet this length is written as, when it is one: music21's
    /// `quarterLengthToTuplet`, keeping the first match as its `fullName`
    /// does.
    ///
    /// The search walks the note values from shortest to longest and, for
    /// each, tries every tuplet numerator and every count of them that fits.
    /// Order is what decides the answer: two thirds of a quarter matches a
    /// quarter in a triplet before it matches anything longer, which is why
    /// music21 calls it a quarter triplet and not two eighth triplets.
    pub fn tuplet(&self) -> Option<Tuplet> {
        if self.quarter_length <= 0.0 {
            return None;
        }
        // A length that is already a written value is that value, not a
        // tuplet of some other one: music21 tries the exact match before it
        // tries any ratio, and a plain quarter is a quarter even though it
        // is also two thirds of a dotted quarter in a triplet.
        if self.type_and_dots().is_some() {
            return None;
        }
        let mut values = DurationType::ALL;
        values.sort_by(|left, right| {
            left.quarter_length()
                .partial_cmp(&right.quarter_length())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let tolerance = self.quarter_length * TUPLET_TOLERANCE;
        for duration_type in values {
            for actual in TUPLET_NUMERATORS {
                for normal in 1..actual {
                    for dots in TUPLET_DOTS {
                        let candidate = duration_type.quarter_length_with_dots(dots)
                            * FloatType::from(normal)
                            / FloatType::from(actual);
                        if (candidate - self.quarter_length).abs() <= tolerance {
                            return Some(Tuplet::new(actual, normal, duration_type, dots));
                        }
                    }
                }
            }
        }
        None
    }

    /// The tuplets this length is written inside: the ones a caller set, or
    /// the one read off the length when nobody has.
    ///
    /// This is music21's `tuplets`, which is likewise inferred until it is
    /// assigned. Setting it to nothing is not the same as never setting it:
    /// two thirds of a quarter reads as a quarter triplet on its own, and
    /// stays two thirds of a quarter written as no tuplet once told so.
    pub fn tuplets(&self) -> Vec<Tuplet> {
        match &self.tuplets {
            Some(tuplets) => tuplets.clone(),
            None => self.tuplet().into_iter().collect(),
        }
    }

    /// What the written values are multiplied by to give the sounding
    /// length: music21's `aggregateTupletMultiplier`, every tuplet's ratio
    /// multiplied together, so a triplet inside a quintuplet is `8/15`.
    pub fn aggregate_tuplet_multiplier(&self) -> FractionType {
        self.tuplets()
            .iter()
            .map(Tuplet::multiplier)
            .fold(FractionType::from(1), |total, ratio| total * ratio)
    }

    /// The total of the written values, before any tuplet shortens them:
    /// music21's `quarterLengthNoTuplets`.
    pub fn quarter_length_no_tuplets(&self) -> FloatType {
        self.components()
            .into_iter()
            .map(|(duration_type, dots)| duration_type.quarter_length_with_dots(dots))
            .sum()
    }

    /// Says what tuplets this length is written inside, keeping the written
    /// values and changing the sounding length to match: music21's `tuplets`
    /// setter.
    pub fn set_tuplets(&mut self, tuplets: Vec<Tuplet>) {
        let written = self.quarter_length_no_tuplets();
        self.tuplets = Some(tuplets);
        self.quarter_length = written * float_from_fraction(self.aggregate_tuplet_multiplier());
    }

    /// Writes this length inside one more tuplet, shortening it by that
    /// tuplet's ratio: music21's `appendTuplet`.
    pub fn append_tuplet(&mut self, tuplet: Tuplet) {
        let mut tuplets = self.tuplets();
        tuplets.push(tuplet);
        self.set_tuplets(tuplets);
    }

    /// The length of the written values alone, with the tuplets a caller set
    /// divided back out.
    ///
    /// Only the set ones: an inferred tuplet is read *off* this length, so
    /// dividing by it here would be circular. Dividing in binary floating
    /// point rarely lands back on an exact note value, so the result is
    /// snapped onto one when it is within a hair of it.
    fn written_quarter_length(&self) -> FloatType {
        let Some(tuplets) = &self.tuplets else {
            return self.quarter_length;
        };
        if tuplets.is_empty() {
            return self.quarter_length;
        }
        let multiplier = float_from_fraction(self.aggregate_tuplet_multiplier());
        if multiplier == 0.0 {
            return self.quarter_length;
        }
        let written = self.quarter_length / multiplier;
        let tolerance = written.abs() * TUPLET_TOLERANCE;
        DurationType::ALL
            .into_iter()
            .flat_map(|duration_type| {
                (0..=MAX_DOTS).map(move |dots| duration_type.quarter_length_with_dots(dots))
            })
            .find(|candidate| (candidate - written).abs() <= tolerance)
            .unwrap_or(written)
    }

    /// The written note values this length is made of, tied together:
    /// music21's `components`.
    ///
    /// One value for a plain or dotted note, one for a tuplet (the written
    /// value, which the tuplet's ratio then scales), and several for a
    /// length that can only be written as a tie — a quarter tied to a
    /// sixteenth for five sixteenths. Empty for a length that runs off the
    /// end of the note values, which is music21's `inexpressible`.
    ///
    /// The tie is found greedily, largest value first, as music21 finds it:
    /// take the largest note that fits, and look for a single dotted value
    /// covering what is left before taking another bite.
    pub fn components(&self) -> Vec<(DurationType, u32)> {
        let written = self.written_quarter_length();
        // Zero is written as nothing at all, which is what music21's empty
        // `components` says; `type_and_dots` would call it a `zero` note.
        if written == 0.0 {
            return Vec::new();
        }
        if let Some((duration_type, dots)) = exact_type_and_dots(written) {
            return vec![(duration_type, dots)];
        }
        // Shorter than the shortest note value, or longer than a tie of the
        // longest can reach: music21 calls both *inexpressible*, and asks
        // this before it looks for a tuplet.
        let Ok((largest, _)) = quarter_length_to_closest_type(written) else {
            return Vec::new();
        };
        if largest.next_larger().is_none() {
            return Vec::new();
        }
        if self.tuplets.is_none()
            && let Some(tuplet) = self.tuplet()
        {
            return vec![(tuplet.duration_type(), tuplet.dots())];
        }
        let mut components = vec![(largest, 0)];
        let mut remainder = written - largest.quarter_length();
        for _ in 0..MAX_TIED_COMPONENTS {
            if let Some(rest) = Duration::new(remainder)
                .ok()
                .and_then(|duration| duration.type_and_dots())
            {
                components.push(rest);
                return components;
            }
            let Ok((next, _)) = quarter_length_to_closest_type(remainder) else {
                break;
            };
            remainder -= next.quarter_length();
            components.push((next, 0));
        }
        // A length the tie never finished covering is one no notation can
        // write, which is music21's `inexpressible` and not a shorter note.
        Vec::new()
    }

    /// Returns music21's `fullName` for a single written note value, such as
    /// `"Dotted Quarter"`, `"Double Dotted Half"`, `"Imperfect Longa"` or
    /// `"Quarter Triplet (2/3 QL)"`.
    ///
    /// A length that has to be written as a tie names each of its values and
    /// joins them, `"Quarter tied to 16th (1 1/4 total QL)"`. A length no
    /// note value reaches is `"Inexpressible"` and a zero one is
    /// `"Zero Duration (0 total QL)"`, both as music21 names them.
    pub fn full_name(&self) -> String {
        let components = self.components();
        if components.is_empty() {
            return if self.quarter_length == 0.0 {
                // music21 writes a stray second space here, from joining a
                // name that already ends in one; its own docstring for this
                // shows a single space, which is what the whitespace its
                // test runner normalises away comes to.
                "Zero Duration (0 total QL)".to_string()
            } else {
                "Inexpressible".to_string()
            };
        }
        // A tuplet is only asked for once the length has failed to be a
        // plain or dotted note value, which is music21's order: a quarter is
        // a quarter, even though it is also a dotted quarter in a triplet.
        let tuplet = if components.len() == 1 && self.type_and_dots().is_none() {
            self.tuplets().first().copied()
        } else {
            None
        };
        let names: Vec<String> = components
            .iter()
            .map(|(duration_type, dots)| {
                let mensural = matches!(duration_type, DurationType::Longa | DurationType::Maxima);
                let mut name = format!("{}{}", dot_prefix(*dots, mensural), duration_type.title());
                if let Some(tuplet) = &tuplet {
                    name.push(' ');
                    name.push_str(&tuplet.full_name());
                }
                // music21 shows the length itself once the name alone stops
                // saying what it is: past two dots, or inside a tuplet.
                if tuplet.is_some() || *dots >= 3 {
                    name.push_str(&format!(" ({} QL)", mixed_numeral(self.quarter_length)));
                }
                name
            })
            .collect();
        let mut name = names.join(" tied to ");
        if components.len() != 1 {
            name.push_str(&format!(
                " ({} total QL)",
                mixed_numeral(self.quarter_length)
            ));
        }
        name
    }

    /// Returns the note-value type whose undotted length this duration is.
    ///
    /// Returns `None` for a length that is not a plain note value, such as a
    /// dotted or tuplet duration.
    pub fn duration_type(&self) -> Option<DurationType> {
        self.type_and_dots().map(|(duration_type, _)| duration_type)
    }

    /// Returns the duration in quarter lengths.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }

    /// Updates the duration in quarter lengths.
    pub fn set_quarter_length(&mut self, quarter_length: FloatType) -> Result<()> {
        *self = Self::new(quarter_length)?;
        Ok(())
    }
}

const MAX_DOTS: u32 = 4;

/// The note value and dot count whose length is exactly this one, if any.
fn exact_type_and_dots(quarter_length: FloatType) -> Option<(DurationType, u32)> {
    DurationType::ALL.into_iter().find_map(|duration_type| {
        (0..=MAX_DOTS)
            .find(|dots| duration_type.quarter_length_with_dots(*dots) == quarter_length)
            .map(|dots| (duration_type, dots))
    })
}

/// A ratio as a float, which a tuplet's is whenever it meets a quarter
/// length. Zero for the ratio that has no value, which no tuplet has.
fn float_from_fraction(ratio: FractionType) -> FloatType {
    ratio.to_f64().unwrap_or(0.0)
}

/// Returns the note-value type closest to a quarter length, and whether it is
/// exact, as music21's `quarterLengthToClosestType` does: a length between two
/// types reports the longer one, so a triplet quarter is an inexact eighth.
pub fn quarter_length_to_closest_type(quarter_length: FloatType) -> Result<(DurationType, bool)> {
    let too_small = || {
        Error::Duration(format!(
            "cannot return types smaller than 2048th; quarter length was {quarter_length}"
        ))
    };
    if quarter_length.is_nan() || quarter_length <= 0.0 {
        return Err(too_small());
    }
    let note_length = 4.0 / quarter_length;
    if let Some(exact) = DurationType::ALL
        .into_iter()
        .find(|duration_type| duration_type.type_number() == Some(note_length))
    {
        return Ok((exact, true));
    }
    let upper_bound = 8.0 / quarter_length;
    if let Some(closest) = DurationType::ALL.into_iter().find(|duration_type| {
        duration_type
            .type_number()
            .is_some_and(|number| note_length < number && number < upper_bound)
    }) {
        return Ok((closest, false));
    }
    if quarter_length > 128.0 {
        return Ok((DurationType::DuplexMaxima, false));
    }
    Err(too_small())
}

/// Writes a quarter length the way music21's `mixedNumeral` does: `"2"`,
/// `"1/2"`, `"1 3/4"`.
/// The largest denominator [`mixed_numeral`] will write. music21 allows any
/// up to 65535, but it prints the quarter length of a written note, and a
/// note nobody can write does not want a fraction with a four-digit
/// denominator in its name.
const MAX_MIXED_NUMERAL_DENOMINATOR: u32 = 1024;

/// How close a fraction has to be to be written as one, relative to the
/// value.
const MIXED_NUMERAL_TOLERANCE: FloatType = 1e-6;

fn mixed_numeral(value: FloatType) -> String {
    let whole = value.trunc() as IntegerType;
    let remainder = value - value.trunc();
    if remainder == 0.0 {
        return whole.to_string();
    }
    // Ascending denominators, so the simplest fraction that fits wins: a
    // third has to be reachable as well as a quarter, since a tuplet length
    // is not a power of two. The tolerance stands in for the snapping
    // music21 does when it reads a quarter length, which is what makes its
    // own `0.333333` a triplet third rather than a decimal.
    let tolerance = MIXED_NUMERAL_TOLERANCE * value.abs().max(1.0);
    let fractional = (1..=MAX_MIXED_NUMERAL_DENOMINATOR)
        .find_map(|denominator| {
            let numerator = (remainder * FloatType::from(denominator)).round();
            ((remainder - numerator / FloatType::from(denominator)).abs() <= tolerance)
                .then(|| format!("{}/{denominator}", numerator as IntegerType))
        })
        .unwrap_or_else(|| remainder.to_string());
    if whole == 0 {
        fractional
    } else {
        format!("{whole} {fractional}")
    }
}

impl Default for Duration {
    fn default() -> Self {
        Self::quarter()
    }
}

impl PartialEq for Duration {
    fn eq(&self, other: &Self) -> bool {
        self.quarter_length == other.quarter_length
    }
}

impl TryFrom<FloatType> for Duration {
    type Error = Error;

    fn try_from(value: FloatType) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<IntegerType> for Duration {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::new(value as FloatType)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn neighbouring_types_match_music21() {
        assert_eq!(
            DurationType::Quarter.next_larger(),
            Some(DurationType::Half)
        );
        assert_eq!(
            DurationType::Quarter.next_smaller(),
            Some(DurationType::Eighth)
        );
        assert_eq!(DurationType::Whole.next_larger(), Some(DurationType::Breve));
        assert_eq!(DurationType::Breve.next_larger(), Some(DurationType::Longa));
        assert_eq!(
            DurationType::Sixteenth.next_smaller(),
            Some(DurationType::ThirtySecond)
        );
        assert_eq!(DurationType::DuplexMaxima.next_larger(), None);
        assert_eq!(DurationType::ALL[15].next_smaller(), None);
        assert_eq!(DurationType::Zero.next_larger(), None);
        assert_eq!(DurationType::Zero.next_smaller(), None);
    }

    #[test]
    fn ordinal_and_scaling_match_music21() {
        assert_eq!(DurationType::DuplexMaxima.ordinal(), Some(0));
        assert_eq!(DurationType::Quarter.ordinal(), Some(6));
        assert_eq!(DurationType::Sixteenth.ordinal(), Some(8));
        assert_eq!(DurationType::Zero.ordinal(), None);
        assert_eq!(Duration::new(1.5).unwrap().ordinal(), Some(6));
        assert_eq!(Duration::new(2.5).unwrap().ordinal(), None);
        assert_eq!(Duration::new(0.0).unwrap().ordinal(), None);
        assert_eq!(
            Duration::new(1.0)
                .unwrap()
                .augment_or_diminish(2.0)
                .unwrap()
                .quarter_length(),
            2.0
        );
        assert_eq!(
            Duration::new(1.5)
                .unwrap()
                .augment_or_diminish(0.5)
                .unwrap()
                .quarter_length(),
            0.75
        );
        assert!(
            Duration::new(1.0)
                .unwrap()
                .augment_or_diminish(0.0)
                .is_err()
        );
        assert!(
            Duration::new(1.0)
                .unwrap()
                .augment_or_diminish(-1.0)
                .is_err()
        );
    }
    use super::*;

    /// music21's `duration.typeToDuration`, verbatim.
    const MUSIC21_TYPE_TO_DURATION: [(&str, FloatType); 17] = [
        ("duplex-maxima", 64.0),
        ("maxima", 32.0),
        ("longa", 16.0),
        ("breve", 8.0),
        ("whole", 4.0),
        ("half", 2.0),
        ("quarter", 1.0),
        ("eighth", 0.5),
        ("16th", 0.25),
        ("32nd", 0.125),
        ("64th", 0.0625),
        ("128th", 0.03125),
        ("256th", 0.015625),
        ("512th", 0.0078125),
        ("1024th", 0.00390625),
        ("2048th", 0.001953125),
        ("zero", 0.0),
    ];

    #[test]
    fn dots_types_and_full_names_match_music21() {
        let cases = [
            (
                4.0,
                Some((DurationType::Whole, 0)),
                "Whole",
                (DurationType::Whole, true),
            ),
            (
                2.0,
                Some((DurationType::Half, 0)),
                "Half",
                (DurationType::Half, true),
            ),
            (
                1.0,
                Some((DurationType::Quarter, 0)),
                "Quarter",
                (DurationType::Quarter, true),
            ),
            (
                0.5,
                Some((DurationType::Eighth, 0)),
                "Eighth",
                (DurationType::Eighth, true),
            ),
            (
                0.25,
                Some((DurationType::Sixteenth, 0)),
                "16th",
                (DurationType::Sixteenth, true),
            ),
            (
                3.0,
                Some((DurationType::Half, 1)),
                "Dotted Half",
                (DurationType::Half, false),
            ),
            (
                1.5,
                Some((DurationType::Quarter, 1)),
                "Dotted Quarter",
                (DurationType::Quarter, false),
            ),
            (
                0.75,
                Some((DurationType::Eighth, 1)),
                "Dotted Eighth",
                (DurationType::Eighth, false),
            ),
            (
                6.0,
                Some((DurationType::Whole, 1)),
                "Dotted Whole",
                (DurationType::Whole, false),
            ),
            (
                1.75,
                Some((DurationType::Quarter, 2)),
                "Double Dotted Quarter",
                (DurationType::Quarter, false),
            ),
            (
                0.875,
                Some((DurationType::Eighth, 2)),
                "Double Dotted Eighth",
                (DurationType::Eighth, false),
            ),
            (
                7.0,
                Some((DurationType::Whole, 2)),
                "Double Dotted Whole",
                (DurationType::Whole, false),
            ),
            (
                0.375,
                Some((DurationType::Sixteenth, 1)),
                "Dotted 16th",
                (DurationType::Sixteenth, false),
            ),
            (
                3.5,
                Some((DurationType::Half, 2)),
                "Double Dotted Half",
                (DurationType::Half, false),
            ),
            (
                3.75,
                Some((DurationType::Half, 3)),
                "Triple Dotted Half (3 3/4 QL)",
                (DurationType::Half, false),
            ),
            (
                1.875,
                Some((DurationType::Quarter, 3)),
                "Triple Dotted Quarter (1 7/8 QL)",
                (DurationType::Quarter, false),
            ),
            (
                8.0,
                Some((DurationType::Breve, 0)),
                "Breve",
                (DurationType::Breve, true),
            ),
            (
                16.0,
                Some((DurationType::Longa, 0)),
                "Imperfect Longa",
                (DurationType::Longa, true),
            ),
            (
                24.0,
                Some((DurationType::Longa, 1)),
                "Perfect Longa",
                (DurationType::Longa, false),
            ),
        ];
        for (quarter_length, type_and_dots, full_name, closest) in cases {
            let duration = Duration::new(quarter_length).unwrap();
            assert_eq!(duration.type_and_dots(), type_and_dots, "{quarter_length}");
            assert_eq!(duration.full_name(), full_name, "{quarter_length}");
            assert_eq!(
                quarter_length_to_closest_type(quarter_length).unwrap(),
                closest,
                "{quarter_length}"
            );
        }

        let inexact = [
            (2.0 / 3.0, DurationType::Eighth),
            (1.0 / 3.0, DurationType::Sixteenth),
            (4.0 / 3.0, DurationType::Quarter),
            (0.2, DurationType::ThirtySecond),
            (0.4, DurationType::Sixteenth),
            (1.25, DurationType::Quarter),
            (5.0, DurationType::Whole),
            (2.5, DurationType::Half),
        ];
        // None of these is a single written note value. The ones that are a
        // tuplet are named as one; the ones that are a tie of two values are
        // not named at all, since the crate keeps a length and not the
        // components music21 spells them out from.
        let tuplet_names = [
            (2.0 / 3.0, "Quarter Triplet (2/3 QL)"),
            (1.0 / 3.0, "Eighth Triplet (1/3 QL)"),
            (4.0 / 3.0, "Half Triplet (1 1/3 QL)"),
            (0.2, "16th Quintuplet (1/5 QL)"),
            (0.4, "Eighth Quintuplet (2/5 QL)"),
        ];
        // and the ones music21 has to write as two values tied together
        let tied = [
            (1.25, "Quarter tied to 16th (1 1/4 total QL)"),
            (5.0, "Whole tied to Quarter (5 total QL)"),
            (2.5, "Half tied to Eighth (2 1/2 total QL)"),
        ];
        for (quarter_length, closest) in inexact {
            let duration = Duration::new(quarter_length).unwrap();
            assert_eq!(duration.type_and_dots(), None, "{quarter_length}");
            assert_eq!(duration.dots(), 0, "{quarter_length}");
            let tuplet_name = tuplet_names
                .iter()
                .find(|(length, _)| *length == quarter_length);
            let tied_name = tied.iter().find(|(length, _)| *length == quarter_length);
            let expected = tuplet_name.or(tied_name).map(|(_, name)| *name);
            assert_eq!(
                duration.full_name(),
                expected.expect("every inexact length here is named"),
                "{quarter_length}"
            );
            assert_eq!(
                duration.tuplet().is_some(),
                tuplet_name.is_some(),
                "{quarter_length}"
            );
            assert_eq!(
                duration.components().len(),
                if tuplet_name.is_some() { 1 } else { 2 },
                "{quarter_length}"
            );
            assert_eq!(
                quarter_length_to_closest_type(quarter_length).unwrap(),
                (closest, false),
                "{quarter_length}"
            );
        }

        // the two lengths music21 refuses to write at all
        for quarter_length in [0.001, 100.0] {
            let duration = Duration::new(quarter_length).unwrap();
            assert!(duration.components().is_empty(), "{quarter_length}");
            assert_eq!(duration.full_name(), "Inexpressible", "{quarter_length}");
        }
        let zero = Duration::new(0.0).unwrap();
        assert!(zero.components().is_empty());
        assert_eq!(zero.full_name(), "Zero Duration (0 total QL)");

        // music21 snaps a quarter length before it looks, so a truncated
        // third is still a triplet there and here.
        let truncated = Duration::new(0.333_333).unwrap();
        assert_eq!(truncated.full_name(), "Eighth Triplet (1/3 QL)");

        let triplet = Duration::new(2.0 / 3.0).unwrap().tuplet().unwrap();
        assert_eq!((triplet.actual(), triplet.normal()), (3, 2));
        assert_eq!(triplet.duration_type(), DurationType::Quarter);
        assert_eq!(triplet.dots(), 0);
        assert_eq!(triplet.full_name(), "Triplet");
        assert_eq!(triplet.multiplier(), FractionType::new(2i32, 3i32));

        // the ratios music21 has no word for say the ratio instead
        let odd = Tuplet::new(17, 14, DurationType::Quarter, 0);
        assert_eq!(odd.full_name(), "Tuplet of 17/14ths");
        assert_eq!(odd.total_tuplet_length(), 14.0);
        // Three eighths in the time of one quarter is the same ratio written
        // the other way, and lasts exactly as long.
        let across =
            Tuplet::new(3, 1, DurationType::Eighth, 0).with_normal(DurationType::Quarter, 0);
        assert_eq!(across.total_tuplet_length(), 1.0);
        assert_eq!(across.multiplier(), FractionType::new(1i32, 3i32));
        assert_eq!(Duration::new(3.75).unwrap().dots(), 3);
        assert_eq!(
            quarter_length_to_closest_type(200.0).unwrap(),
            (DurationType::DuplexMaxima, false)
        );
        assert!(quarter_length_to_closest_type(0.0).is_err());
        assert!(quarter_length_to_closest_type(0.0001).is_err());
    }

    #[test]
    fn duration_types_match_music21s_table() {
        assert_eq!(DurationType::ALL.len(), MUSIC21_TYPE_TO_DURATION.len());
        for (duration_type, (name, quarter_length)) in
            DurationType::ALL.into_iter().zip(MUSIC21_TYPE_TO_DURATION)
        {
            assert_eq!(duration_type.music21_name(), name);
            assert_eq!(duration_type.quarter_length(), quarter_length, "{name}");
            assert_eq!(DurationType::from_music21_name(name), Some(duration_type));
        }
    }

    #[test]
    fn duration_types_round_trip_through_their_names() {
        for duration_type in DurationType::ALL {
            let name = duration_type.music21_name();
            assert_eq!(name.parse::<DurationType>().unwrap(), duration_type);
            assert_eq!(duration_type.to_string(), name);
        }
        assert!("not-a-duration".parse::<DurationType>().is_err());
    }

    #[test]
    fn each_type_is_half_the_one_before_it() {
        // `zero` is the exception and is excluded.
        let ordered = &DurationType::ALL[..DurationType::ALL.len() - 1];
        for pair in ordered.windows(2) {
            assert_eq!(
                pair[1].quarter_length() * 2.0,
                pair[0].quarter_length(),
                "{} should be half of {}",
                pair[1],
                pair[0]
            );
        }
    }

    #[test]
    fn dots_add_half_of_what_came_before() {
        assert_eq!(DurationType::Half.quarter_length_with_dots(0), 2.0);
        assert_eq!(DurationType::Half.quarter_length_with_dots(1), 3.0);
        assert_eq!(DurationType::Half.quarter_length_with_dots(2), 3.5);
        assert_eq!(DurationType::Half.quarter_length_with_dots(3), 3.75);
        assert_eq!(DurationType::Quarter.quarter_length_with_dots(1), 1.5);
    }

    #[test]
    fn durations_convert_to_and_from_note_values() {
        assert_eq!(
            Duration::from_type(DurationType::Whole).quarter_length(),
            4.0
        );
        assert_eq!(
            Duration::from_type(DurationType::Whole).duration_type(),
            Some(DurationType::Whole)
        );
        assert_eq!(
            Duration::from_type_with_dots(DurationType::Half, 1).quarter_length(),
            3.0
        );
        // A dotted value keeps the type it is dotted from, as music21 reads
        // it: a dotted half is a half with one dot, not a type of its own.
        assert_eq!(
            Duration::from_type_with_dots(DurationType::Half, 1).duration_type(),
            Some(DurationType::Half)
        );
        assert_eq!(
            Duration::from_type_with_dots(DurationType::Half, 1).dots(),
            1
        );
        assert_eq!(
            Duration::new(3.75).unwrap().duration_type(),
            Some(DurationType::Half)
        );
        assert_eq!(Duration::new(3.75).unwrap().dots(), 3);
        // A triplet eighth is no note value at all.
        assert_eq!(Duration::new(1.0 / 3.0).unwrap().duration_type(), None);
    }

    #[test]
    fn the_named_helpers_agree_with_their_types() {
        assert_eq!(
            Duration::quarter(),
            Duration::from_type(DurationType::Quarter)
        );
        assert_eq!(Duration::half(), Duration::from_type(DurationType::Half));
        assert_eq!(Duration::whole(), Duration::from_type(DurationType::Whole));
        assert_eq!(
            Duration::eighth(),
            Duration::from_type(DurationType::Eighth)
        );
    }

    #[test]
    fn duration_tracks_quarter_lengths() {
        assert_eq!(Duration::quarter().quarter_length(), 1.0);
        assert_eq!(Duration::half().quarter_length(), 2.0);
        assert_eq!(Duration::whole().quarter_length(), 4.0);
        assert_eq!(Duration::eighth().quarter_length(), 0.5);
    }

    #[test]
    fn duration_rejects_invalid_values() {
        assert!(Duration::new(-1.0).is_err());
        assert!(Duration::new(FloatType::INFINITY).is_err());
    }

    #[test]
    fn a_plain_note_value_is_not_read_as_a_tuplet() {
        // music21 tries the exact written value before any ratio, so a
        // quarter is a quarter and not two thirds of a dotted quarter.
        assert_eq!(Duration::quarter().tuplet(), None);
        assert_eq!(Duration::new(3.0).unwrap().tuplet(), None);
        assert_eq!(
            Duration::new(1.0 / 3.0).unwrap().tuplet(),
            Some(Tuplet::new(3, 2, DurationType::Eighth, 0))
        );
    }

    #[test]
    fn appending_a_tuplet_shortens_the_length_by_its_ratio() {
        // music21's own `appendTuplet` docstring, exactly.
        let mut duration = Duration::new(1.0).unwrap();
        duration.append_tuplet(Tuplet::new(3, 2, DurationType::Quarter, 0));
        assert!((duration.quarter_length() - 2.0 / 3.0).abs() < 1e-12);
        assert_eq!(duration.quarter_length_no_tuplets(), 1.0);

        duration.append_tuplet(Tuplet::new(5, 4, DurationType::Quarter, 0));
        assert!((duration.quarter_length() - 8.0 / 15.0).abs() < 1e-12);
        assert_eq!(
            duration.aggregate_tuplet_multiplier(),
            FractionType::new(8, 15)
        );
        // The written value is untouched by either tuplet.
        assert_eq!(
            duration.components(),
            vec![(DurationType::Quarter, 0)],
            "the written value stays a quarter inside both tuplets"
        );
    }

    #[test]
    fn saying_a_length_is_in_no_tuplet_is_not_the_same_as_saying_nothing() {
        let inferred = Duration::new(1.0 / 3.0).unwrap();
        assert_eq!(inferred.tuplets().len(), 1);

        let mut told = Duration::new(1.0 / 3.0).unwrap();
        told.set_tuplets(Vec::new());
        assert!(told.tuplets().is_empty());
        // Losing the triplet leaves the written eighth sounding in full.
        assert_eq!(told.quarter_length(), 0.5);
    }

    #[test]
    fn setting_a_length_forgets_the_tuplets_it_was_told() {
        let mut duration = Duration::new(1.0).unwrap();
        duration.append_tuplet(Tuplet::new(3, 2, DurationType::Quarter, 0));
        duration.set_quarter_length(2.0).unwrap();
        assert!(duration.tuplets().is_empty());
        assert_eq!(duration.quarter_length(), 2.0);
    }

    #[test]
    fn duration_supports_conversions_and_updates() {
        let mut duration = Duration::try_from(3 as IntegerType).unwrap();
        assert_eq!(duration.quarter_length(), 3.0);

        duration.set_quarter_length(1.5).unwrap();
        assert_eq!(duration, Duration::try_from(1.5).unwrap());
        assert!(duration.set_quarter_length(FloatType::NAN).is_err());
    }
}
