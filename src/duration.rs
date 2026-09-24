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
#[must_use]
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
#[must_use]
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
    /// The written values, once something has said what they are.
    ///
    /// `None` is the ordinary case: the values are read off the length by
    /// [`Duration::components`]. `Some` is a list a caller built a value at a
    /// time, cut, consolidated or unlinked from the length, and it is then
    /// what the length is worked out from rather than the other way round.
    #[cfg_attr(feature = "serde", serde(default))]
    written: Option<Vec<DurationTuple>>,
    /// Dots written above dots, once set: music21's `dotGroups`.
    #[cfg_attr(feature = "serde", serde(default))]
    dot_groups: Option<Vec<u32>>,
    /// Whether the written values and the sounding length have been told
    /// apart, so that neither is worked out from the other.
    #[cfg_attr(feature = "serde", serde(default))]
    unlinked: bool,
}

/// One written value of a duration and how long it lasts: music21's
/// `DurationTuple`.
///
/// The length is kept beside the value because the two can disagree. A grace
/// note is written as an eighth and lasts no time, and a piece cut out of a
/// longer value may have a length no single note value reaches, which
/// music21 calls `inexpressible` and which has no type here.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct DurationTuple {
    duration_type: Option<DurationType>,
    dots: u32,
    quarter_length: FloatType,
}

impl DurationTuple {
    /// A note value with its dots, lasting as long as that is written.
    pub fn new(duration_type: DurationType, dots: u32) -> Self {
        Self {
            duration_type: Some(duration_type),
            dots,
            quarter_length: duration_type.quarter_length_with_dots(dots),
        }
    }

    /// The single written value a length comes to: music21's
    /// `durationTupleFromQuarterLength`. Nought is the `zero` value, and a
    /// length no dotted note value has is one with no type.
    pub fn from_quarter_length(quarter_length: FloatType) -> Self {
        if quarter_length == 0.0 {
            return Self::new(DurationType::Zero, 0);
        }
        match exact_type_and_dots(quarter_length) {
            Some((duration_type, dots)) => Self::new(duration_type, dots),
            None => Self {
                duration_type: None,
                dots: 0,
                quarter_length,
            },
        }
    }

    /// The same written value lasting no time, which is how a grace note
    /// keeps what it is written as.
    pub fn silenced(self) -> Self {
        Self {
            quarter_length: 0.0,
            ..self
        }
    }

    /// The note value, or `None` for music21's `inexpressible`.
    pub fn duration_type(&self) -> Option<DurationType> {
        self.duration_type
    }

    /// The augmentation dots on the value.
    pub fn dots(&self) -> u32 {
        self.dots
    }

    /// How long the value lasts, in quarter lengths.
    pub fn quarter_length(&self) -> FloatType {
        self.quarter_length
    }
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

/// music21's `defaults.limitOffsetDenominator`: the largest denominator it
/// will read a length as a fraction with.
const DENOMINATOR_LIMIT: i128 = 65535;

/// Reduces a fraction to its lowest terms.
fn reduce(numerator: &mut i128, denominator: &mut i128) {
    let divisor = num::integer::gcd(*numerator, *denominator);
    if divisor > 1 {
        *numerator /= divisor;
        *denominator /= divisor;
    }
}

/// The fraction closest to a value with a denominator no larger than the
/// limit, as Python's `Fraction.limit_denominator` finds it.
pub(crate) fn limited_fraction(value: FloatType, max_denominator: i128) -> Option<(i128, i128)> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    // The exact fraction the float stands for, as `Fraction.from_float` reads
    // it: a float is a binary fraction, so doubling reaches a whole number.
    let mut exact = value;
    let mut denominator: i128 = 1;
    for _ in 0..64 {
        if exact.fract() == 0.0 {
            break;
        }
        exact *= 2.0;
        denominator = denominator.checked_mul(2)?;
    }
    let mut numerator = exact as i128;
    if exact.fract() != 0.0 {
        return None;
    }
    if denominator <= max_denominator {
        reduce(&mut numerator, &mut denominator);
        return Some((numerator, denominator));
    }
    // Python's own walk up the Stern-Brocot tree.
    let (mut p0, mut q0, mut p1, mut q1) = (0i128, 1i128, 1i128, 0i128);
    let (mut n, mut d) = (numerator, denominator);
    loop {
        let a = n / d;
        let q2 = q0 + a * q1;
        if q2 > max_denominator {
            break;
        }
        (p0, q0, p1, q1) = (p1, q1, p0 + a * p1, q2);
        let next = n - a * d;
        n = d;
        d = next;
        if d == 0 {
            break;
        }
    }
    if q1 == 0 {
        return None;
    }
    let k = (max_denominator - q0) / q1;
    let (bound_numerator, bound_denominator) = (p0 + k * p1, q0 + k * q1);
    let value_as =
        |numerator: i128, denominator: i128| numerator as FloatType / denominator as FloatType;
    let one = (value_as(bound_numerator, bound_denominator) - value).abs();
    let two = (value_as(p1, q1) - value).abs();
    let (mut numerator, mut denominator) = if one <= two {
        (bound_numerator, bound_denominator)
    } else {
        (p1, q1)
    };
    reduce(&mut numerator, &mut denominator);
    Some((numerator, denominator))
}

/// How close a length has to be to a tuplet's to be read as one, relative to
/// the length itself.
///
/// music21 snaps a quarter length onto a limited-denominator fraction before
/// it looks, which is why `0.333333` is a triplet eighth there; this is the
/// same latitude for a crate that keeps the length as a float.
const TUPLET_TOLERANCE: FloatType = 1e-5;

/// A length written as a tuplet: `actual` notes in the time of `normal`,
/// each side optionally counted in a written value.
///
/// This is the naming half of music21's `Tuplet`. A [`Duration`] here is a
/// quarter length, so a tuplet is something a length is *read as* rather
/// than something a duration carries — what it is for is being able to say
/// that two thirds of a quarter is a quarter triplet.
///
/// Either written value may be unsaid, as music21's may: a bare triplet is
/// three in the time of two of nothing in particular, and wherever a length
/// is needed an eighth is assumed, which is music21's basic triplet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Tuplet {
    actual: u32,
    normal: u32,
    /// The written value the `actual` notes are written as, with its dots.
    duration_actual: Option<(DurationType, u32)>,
    /// The written value the `normal` count is counted in, which is not
    /// always the one the `actual` count is written as: three quarters in the
    /// time of one half is the same ratio as three in the time of two
    /// quarters, and music21 keeps both spellings.
    duration_normal: Option<(DurationType, u32)>,
}

/// The value music21 counts a side in when nothing says: an eighth.
const ASSUMED_VALUE: (DurationType, u32) = (DurationType::Eighth, 0);

impl Tuplet {
    /// A tuplet of `actual` notes of a written value in the time of
    /// `normal` of them.
    pub fn new(actual: u32, normal: u32, duration_type: DurationType, dots: u32) -> Self {
        Self {
            actual,
            normal,
            duration_actual: Some((duration_type, dots)),
            duration_normal: Some((duration_type, dots)),
        }
    }

    /// `actual` notes in the time of `normal`, with neither side counted in
    /// a written value: music21's `Tuplet(actual, normal)`.
    ///
    /// ```
    /// use music21_rs::Tuplet;
    ///
    /// let triplet = Tuplet::ratio(3, 2);
    /// assert_eq!(triplet.duration_actual(), None);
    /// assert_eq!(triplet.total_tuplet_length(), 1.0);
    /// ```
    pub fn ratio(actual: u32, normal: u32) -> Self {
        Self {
            actual,
            normal,
            duration_actual: None,
            duration_normal: None,
        }
    }

    /// The same tuplet counting the `normal` side in a different written
    /// value: music21's `durationNormal` apart from its `durationActual`.
    pub fn with_normal(mut self, duration_type: DurationType, dots: u32) -> Self {
        self.duration_normal = Some((duration_type, dots));
        self
    }

    /// The written value the `actual` notes are written as, with its dots,
    /// where one is said: music21's `durationActual`.
    pub fn duration_actual(&self) -> Option<(DurationType, u32)> {
        self.duration_actual
    }

    /// The written value the `normal` count is counted in, with its dots,
    /// where one is said: music21's `durationNormal`.
    pub fn duration_normal(&self) -> Option<(DurationType, u32)> {
        self.duration_normal
    }

    /// Says, or unsays, the value the `actual` notes are written as.
    pub fn set_duration_actual(&mut self, value: Option<(DurationType, u32)>) {
        self.duration_actual = value;
    }

    /// Says, or unsays, the value the `normal` count is counted in.
    pub fn set_duration_normal(&mut self, value: Option<(DurationType, u32)>) {
        self.duration_normal = value;
    }

    /// The `actual` count beside the value it is written in: music21's
    /// `tupletActual`.
    pub fn tuplet_actual(&self) -> (u32, Option<(DurationType, u32)>) {
        (self.actual, self.duration_actual)
    }

    /// The `normal` count beside the value it is counted in: music21's
    /// `tupletNormal`.
    pub fn tuplet_normal(&self) -> (u32, Option<(DurationType, u32)>) {
        (self.normal, self.duration_normal)
    }

    /// Writes both sides of the tuplet in one value: music21's
    /// `setDurationType`.
    pub fn set_duration_type(&mut self, duration_type: DurationType, dots: u32) {
        self.duration_actual = Some((duration_type, dots));
        self.duration_normal = Some((duration_type, dots));
    }

    /// Changes how many notes are played in the time of how many: music21's
    /// `setRatio`.
    pub fn set_ratio(&mut self, actual: u32, normal: u32) {
        self.actual = actual;
        self.normal = normal;
    }

    /// The same tuplet with both its written values scaled by `amount`:
    /// music21's `augmentOrDiminish`. The ratio stays as it is.
    ///
    /// ```
    /// use music21_rs::{DurationType, Tuplet};
    ///
    /// let mut sextuplet = Tuplet::ratio(6, 2);
    /// sextuplet.set_duration_type(DurationType::Eighth, 0);
    /// let halved = sextuplet.augmented(0.5)?;
    /// assert_eq!(halved.duration_actual(), Some((DurationType::Sixteenth, 0)));
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// An `amount` that is not greater than nought, or a scaled value no
    /// dotted note value writes.
    pub fn augmented(&self, amount: FloatType) -> Result<Self> {
        if amount.is_nan() || amount <= 0.0 {
            return Err(Error::Value(
                "amountToScale must be greater than zero".to_string(),
            ));
        }
        let scaled = |value: Option<(DurationType, u32)>| -> Result<Option<(DurationType, u32)>> {
            value
                .map(|(duration_type, dots)| {
                    let length = duration_type.quarter_length_with_dots(dots) * amount;
                    exact_type_and_dots(length).ok_or_else(|| {
                        Error::Duration(format!("no written value lasts {length} quarter lengths"))
                    })
                })
                .transpose()
        };
        Ok(Self {
            duration_actual: scaled(self.duration_actual)?,
            duration_normal: scaled(self.duration_normal)?,
            ..*self
        })
    }

    /// How long the whole tuplet lasts: music21's `totalTupletLength`, the
    /// `normal` count of the value it is counted in.
    pub fn total_tuplet_length(&self) -> FloatType {
        let (duration_type, dots) = self.duration_normal.unwrap_or(ASSUMED_VALUE);
        FloatType::from(self.normal) * duration_type.quarter_length_with_dots(dots)
    }

    /// How many notes are played: music21's `numberNotesActual`.
    pub fn actual(&self) -> u32 {
        self.actual
    }

    /// How many notes they are played in the time of: `numberNotesNormal`.
    pub fn normal(&self) -> u32 {
        self.normal
    }

    /// The note value each one is written as, an eighth where none is said.
    pub fn duration_type(&self) -> DurationType {
        self.duration_actual.unwrap_or(ASSUMED_VALUE).0
    }

    /// How many augmentation dots that written value carries.
    pub fn dots(&self) -> u32 {
        self.duration_actual.unwrap_or(ASSUMED_VALUE).1
    }

    /// The written value the `normal` count is counted in, an eighth where
    /// none is said.
    pub fn normal_duration_type(&self) -> DurationType {
        self.duration_normal.unwrap_or(ASSUMED_VALUE).0
    }

    /// The dots on that value.
    pub fn normal_dots(&self) -> u32 {
        self.duration_normal.unwrap_or(ASSUMED_VALUE).1
    }

    /// What the written length is multiplied by inside the tuplet:
    /// music21's `tupletMultiplier`, how long the `normal` side lasts over
    /// how long the `actual` notes would last unaltered.
    ///
    /// ```
    /// use music21_rs::{DurationType, FractionType, Tuplet};
    ///
    /// assert_eq!(Tuplet::ratio(3, 2).multiplier(), FractionType::new(2, 3));
    /// // Three quarters in the time of one half.
    /// let written =
    ///     Tuplet::new(3, 1, DurationType::Quarter, 0).with_normal(DurationType::Half, 0);
    /// assert_eq!(written.multiplier(), FractionType::new(2, 3));
    /// ```
    pub fn multiplier(&self) -> FractionType {
        let (actual_type, actual_dots) = self.duration_actual.unwrap_or(ASSUMED_VALUE);
        let unaltered =
            FloatType::from(self.actual) * actual_type.quarter_length_with_dots(actual_dots);
        let (numerator, denominator) =
            limited_fraction(self.total_tuplet_length() / unaltered, 65535).unwrap_or((0, 1));
        FractionType::new(
            IntegerType::try_from(numerator).unwrap_or(IntegerType::MAX),
            IntegerType::try_from(denominator).unwrap_or(IntegerType::MAX),
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

        Ok(Self::of_length(quarter_length))
    }

    /// A length with nothing said about how it is written.
    fn of_length(quarter_length: FloatType) -> Self {
        Self {
            quarter_length,
            tuplets: None,
            written: None,
            dot_groups: None,
            unlinked: false,
        }
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
        Self::of_length(duration_type.quarter_length())
    }

    /// Creates a duration from a note-value type carrying augmentation dots.
    pub fn from_type_with_dots(duration_type: DurationType, dots: u32) -> Self {
        Self::of_length(duration_type.quarter_length_with_dots(dots))
    }

    /// Returns the note-value type and dot count that together make exactly
    /// this length, such as a half note with one dot for `3.0`. `None` for a
    /// length no single dotted note value has, such as a tuplet or a tie.
    ///
    /// Where the written values have been said, it is the one value there
    /// is, whatever the length: an unlinked duration written as a half is a
    /// half however long it sounds.
    pub fn type_and_dots(&self) -> Option<(DurationType, u32)> {
        match self.written.as_deref() {
            Some([one]) => one
                .duration_type
                .map(|duration_type| (duration_type, one.dots)),
            Some(_) => None,
            None => exact_type_and_dots(self.quarter_length),
        }
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

    /// The tuplet this length is written as, if any: the first of
    /// [`quarter_length_to_tuplet`].
    ///
    /// A length that is already a plain written value is that value rather
    /// than a tuplet of some other one, so a quarter answers `None`: music21
    /// tries the exact match before it tries any ratio, and a plain quarter
    /// is a quarter even though it is also two thirds of a dotted quarter in
    /// a triplet.
    pub fn tuplet(&self) -> Option<Tuplet> {
        if self.type_and_dots().is_some() {
            return None;
        }
        quarter_length_to_tuplet(self.quarter_length, 1)
            .into_iter()
            .next()
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
            None => convert(self.quarter_length, true).1.into_iter().collect(),
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
        // Folded from a positive nought: Rust sums floats from `-0.0`.
        self.written_values()
            .iter()
            .fold(0.0, |total, value| total + value.quarter_length)
    }

    /// Says what tuplets this length is written inside, keeping the written
    /// values and changing the sounding length to match: music21's `tuplets`
    /// setter.
    pub fn set_tuplets(&mut self, tuplets: Vec<Tuplet>) {
        let written = self.quarter_length_no_tuplets();
        self.tuplets = Some(tuplets);
        if self.unlinked {
            return;
        }
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
    /// This is the first half of [`quarter_conversion`], read over the
    /// written length. A length whose tuplets a caller has set is read as
    /// the tie those tuplets leave, without looking for a tuplet of its own.
    ///
    /// Where the written values have been said these are they, less any
    /// with no type; [`Duration::written_values`] keeps those too.
    pub fn components(&self) -> Vec<(DurationType, u32)> {
        match &self.written {
            Some(written) => written
                .iter()
                .filter_map(|value| Some((value.duration_type?, value.dots)))
                .collect(),
            None => convert(self.written_quarter_length(), self.tuplets.is_none()).0,
        }
    }

    /// The written values with the length of each: music21's `components`
    /// as the `DurationTuple`s they are there.
    ///
    /// A length no tie of note values reaches is one value with no type,
    /// as it is upstream, and nought is no values at all.
    pub fn written_values(&self) -> Vec<DurationTuple> {
        if let Some(written) = &self.written {
            return written.clone();
        }
        let components = self.components();
        if components.is_empty() && self.quarter_length != 0.0 {
            return vec![DurationTuple::from_quarter_length(
                self.written_quarter_length(),
            )];
        }
        components
            .into_iter()
            .map(|(duration_type, dots)| DurationTuple::new(duration_type, dots))
            .collect()
    }

    /// Makes these the written values, and the length what they come to
    /// inside the tuplets and dot groups the duration already has.
    fn set_written_values(&mut self, written: Vec<DurationTuple>) {
        // Read the tuplets off the length while it still says them.
        self.tuplets = Some(self.tuplets());
        self.written = Some(written);
        self.relength();
    }

    /// music21's `_updateQuarterLength`: the length the written values come
    /// to. An unlinked duration keeps the length it was given.
    fn relength(&mut self) {
        if self.unlinked {
            return;
        }
        let groups: FloatType = self
            .dot_groups
            .iter()
            .flatten()
            .filter(|dots| **dots != 0)
            .map(|dots| 2.0 - FloatType::powi(0.5, *dots as i32))
            .product();
        self.quarter_length = self.quarter_length_no_tuplets()
            * float_from_fraction(self.aggregate_tuplet_multiplier())
            * groups;
    }

    /// One written value for the whole length, losing how it had been
    /// written: music21's `consolidate`.
    ///
    /// The value is the one the written total comes to, before any tuplet,
    /// and has no type where no single note value does. A duration already
    /// written as one value is left alone.
    ///
    /// ```
    /// use music21_rs::{Duration, DurationType};
    ///
    /// let mut duration = Duration::quarter();
    /// duration.add_duration_tuple(DurationType::Half, 0);
    /// duration.add_duration_tuple(DurationType::Quarter, 0);
    /// assert_eq!(duration.components().len(), 3);
    /// duration.consolidate();
    /// assert_eq!(duration.components(), [(DurationType::Whole, 0)]);
    /// ```
    pub fn consolidate(&mut self) {
        if self.written_values().len() == 1 {
            return;
        }
        let total = self.quarter_length_no_tuplets();
        self.set_written_values(vec![DurationTuple::from_quarter_length(total)]);
    }

    /// Cuts the written value sounding at a position in two, leaving the
    /// whole length as it was: music21's `sliceComponentAtPosition`.
    ///
    /// The position is counted in the written values. One that falls on the
    /// join between two values, or at either end, cuts nothing and is an
    /// error.
    pub fn slice_component_at_position(&mut self, position: FloatType) -> Result<()> {
        let written = self.written_values();
        let mut start = 0.0;
        for (index, value) in written.iter().enumerate() {
            let end = start + value.quarter_length;
            if position > start && position < end {
                let mut sliced = written[..index].to_vec();
                sliced.push(DurationTuple::from_quarter_length(position - start));
                sliced.push(DurationTuple::from_quarter_length(end - position));
                sliced.extend_from_slice(&written[index + 1..]);
                self.set_written_values(sliced);
                return Ok(());
            }
            start = end;
        }
        Err(Error::Duration(
            "no slice is possible at this quarter position".to_string(),
        ))
    }

    /// Writes the duration with dots above its dots: music21's `dotGroups`
    /// setter. Each group lengthens the value as dots do, so a half under
    /// `[1, 1]` lasts four and a half quarters. The plain dots come off the
    /// written values, since the groups now say how the note is dotted.
    pub fn set_dot_groups(&mut self, groups: Vec<u32>) {
        let undotted = self
            .written_values()
            .into_iter()
            .map(|value| match value.duration_type {
                Some(duration_type) => DurationTuple::new(duration_type, 0),
                None => value,
            })
            .collect();
        self.dot_groups = Some(groups);
        self.set_written_values(undotted);
    }

    /// The same length written as a tie of singly dotted values, for a
    /// reader with no dot groups: music21's `splitDotGroups`.
    ///
    /// Each group after the first ties on every value so far at the next
    /// note value down, so a half under `[1, 1]` is a dotted half tied to a
    /// dotted quarter. It is an error for a duration that is not one
    /// written value.
    pub fn split_dot_groups(&self) -> Result<Duration> {
        let (duration_type, _) = self.type_and_dots().ok_or_else(|| {
            Error::Duration("only a single written value has dot groups to split".to_string())
        })?;
        let groups = self.dot_groups();
        let mut values = vec![DurationTuple::new(
            duration_type,
            groups.first().copied().unwrap_or(0),
        )];
        for _ in 1..groups.len() {
            let smaller: Vec<DurationTuple> = values
                .iter()
                .map(|value| {
                    let kind = value.duration_type.unwrap_or(duration_type);
                    DurationTuple::new(kind.next_smaller().unwrap_or(kind), value.dots)
                })
                .collect();
            values.extend(smaller);
        }
        let mut split = self.clone();
        split.dot_groups = None;
        split.set_written_values(values);
        Ok(split)
    }

    /// Whether the written values and the sounding length move together:
    /// music21's `linked`, true unless a caller has said otherwise.
    pub fn linked(&self) -> bool {
        !self.unlinked
    }

    /// Links or unlinks the written values and the length: music21's
    /// `linked` setter.
    ///
    /// Unlinking keeps the values the duration is written as today, and
    /// [`Duration::set_quarter_length`] then changes how long it sounds
    /// without touching them. Linking again keeps the length and reads the
    /// values off it afresh.
    pub fn set_linked(&mut self, linked: bool) {
        if !linked && !self.unlinked {
            self.tuplets = Some(self.tuplets());
            self.written = Some(self.written_values());
        } else if linked && self.unlinked {
            self.written = None;
            self.tuplets = None;
            self.dot_groups = None;
        }
        self.unlinked = !linked;
    }

    /// The same written values sounding for no time, unlinked from their
    /// length: the duration of a grace note, music21's `getGraceDuration`.
    ///
    /// A duration with no written values is written as an eighth, since a
    /// grace note has to be written as something. The tuplets are dropped,
    /// as they are upstream.
    ///
    /// ```
    /// use music21_rs::{Duration, DurationType};
    ///
    /// let grace = Duration::new(1.25)?.grace_duration();
    /// assert_eq!(grace.quarter_length(), 0.0);
    /// assert!(!grace.linked());
    /// assert_eq!(
    ///     grace.components(),
    ///     [(DurationType::Quarter, 0), (DurationType::Sixteenth, 0)]
    /// );
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn grace_duration(&self) -> Duration {
        let mut written: Vec<DurationTuple> = self
            .written_values()
            .into_iter()
            .map(DurationTuple::silenced)
            .collect();
        if written.is_empty() {
            written.push(DurationTuple::new(DurationType::Eighth, 0).silenced());
        }
        Duration {
            quarter_length: 0.0,
            tuplets: Some(Vec::new()),
            written: Some(written),
            dot_groups: None,
            unlinked: true,
        }
    }

    /// Whether the length needs more than one written value, tied: music21's
    /// `isComplex`.
    pub fn is_complex(&self) -> bool {
        self.components().len() > 1
    }

    /// The dots on the written value, as the one dot group the crate keeps:
    /// music21's `dotGroups`, which can also write one length twice over
    /// where a mensural notation asks for it.
    pub fn dot_groups(&self) -> Vec<u32> {
        match &self.dot_groups {
            Some(groups) => groups.clone(),
            None => vec![self.dots()],
        }
    }

    /// Takes the length away: music21's `clear`, which empties the written
    /// values so the duration sounds for no time. The tuplets it was told
    /// stay.
    pub fn clear(&mut self) {
        self.tuplets = Some(self.tuplets());
        self.written = Some(Vec::new());
        self.dot_groups = None;
        self.quarter_length = 0.0;
    }

    /// Ties one more written value onto the end: music21's
    /// `addDurationTuple`, which lengthens the duration by that value inside
    /// whatever tuplets it is written in.
    ///
    /// The values are kept as they were added from then on, so a quarter
    /// with a half and a quarter tied on is three values and not a whole
    /// until it is [consolidated](Duration::consolidate).
    pub fn add_duration_tuple(&mut self, duration_type: DurationType, dots: u32) {
        let mut written = self.written_values();
        written.push(DurationTuple::new(duration_type, dots));
        self.set_written_values(written);
    }

    /// Which written value is sounding at a position within the duration:
    /// music21's `componentIndexAtQtrPosition`. The positions are counted
    /// in the written values, before any tuplet scales them, and the start
    /// and the very end answer the first and the last value.
    pub fn component_index_at_qtr_position(&self, position: FloatType) -> Result<usize> {
        let components = self.components();
        if components.is_empty() {
            return Err(Error::Duration(
                "Need components to run getComponentIndexAtQtrPosition".to_string(),
            ));
        }
        let total = self.quarter_length_no_tuplets();
        if position.is_nan() || position < 0.0 {
            return Err(Error::Value(
                "position is before the start of the duration".to_string(),
            ));
        }
        if position > total {
            return Err(Error::Value(
                "position is after the end of the duration".to_string(),
            ));
        }
        if position == total {
            return Ok(components.len() - 1);
        }
        let mut reached = 0.0;
        for (index, (duration_type, dots)) in components.iter().enumerate() {
            reached += duration_type.quarter_length_with_dots(*dots);
            if reached > position {
                return Ok(index);
            }
        }
        Ok(components.len() - 1)
    }

    /// Where a written value starts within the duration, counted in the
    /// written values: music21's `componentStartTime`. An index past the
    /// values is an error.
    pub fn component_start_time(&self, index: usize) -> Result<FloatType> {
        let components = self.components();
        if index >= components.len() {
            return Err(Error::Duration(format!(
                "invalid component index value {index} submitted; value must be an integer between 0 and {}",
                components.len().saturating_sub(1)
            )));
        }
        Ok(components[..index]
            .iter()
            .map(|(duration_type, dots)| duration_type.quarter_length_with_dots(*dots))
            .sum())
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
    ///
    /// The written values are read off the new length, unless the duration
    /// is [unlinked](Duration::set_linked) and keeps the ones it has.
    pub fn set_quarter_length(&mut self, quarter_length: FloatType) -> Result<()> {
        let replacement = Self::new(quarter_length)?;
        if self.unlinked {
            self.quarter_length = replacement.quarter_length;
        } else {
            *self = replacement;
        }
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

/// The tuplets a length can be written as, shortest note value first:
/// music21's `quarterLengthToTuplet`, stopping after `max_to_return` of
/// them.
///
/// The search walks the note values from shortest to longest and, for
/// each, tries every tuplet numerator and every count of them that fits.
/// Order is what decides the answer: two thirds of a quarter matches a
/// quarter in a triplet before it matches anything longer, which is why
/// music21 calls it a quarter triplet and not two eighth triplets. Dotted
/// tuplets are found; nested ones and four in the time of three are not,
/// the latter being a dotted note.
pub fn quarter_length_to_tuplet(quarter_length: FloatType, max_to_return: usize) -> Vec<Tuplet> {
    let mut found = Vec::new();
    if quarter_length.is_nan() || quarter_length <= 0.0 || max_to_return == 0 {
        return found;
    }
    let mut values = DurationType::ALL;
    values.sort_by(|left, right| {
        left.quarter_length()
            .partial_cmp(&right.quarter_length())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let tolerance = quarter_length * TUPLET_TOLERANCE;
    for duration_type in values {
        for actual in TUPLET_NUMERATORS {
            for normal in 1..actual {
                for dots in TUPLET_DOTS {
                    let candidate = duration_type.quarter_length_with_dots(dots)
                        * FloatType::from(normal)
                        / FloatType::from(actual);
                    if (candidate - quarter_length).abs() <= tolerance {
                        found.push(Tuplet::new(actual, normal, duration_type, dots));
                        break;
                    }
                }
            }
            if found.len() >= max_to_return {
                return found;
            }
        }
    }
    found
}

/// The one enormous tuplet music21 falls back on for a length no tie of
/// written values reaches: its `quarterLengthToNonPowerOf2Tuplet`.
///
/// Any length can be written as a single note inside a strange enough
/// tuplet — 53/25 of a quarter is a whole note in a tuplet of a hundred
/// in the time of fifty-three — and music21 tries that before it calls a
/// length inexpressible. The answer is the tuplet together with the note
/// value and dots written inside it, or nothing for a length that defeats
/// even that.
pub fn quarter_length_to_non_power_of_2_tuplet(
    quarter_length: FloatType,
) -> Option<(Tuplet, DurationType, u32)> {
    if quarter_length.is_nan() || quarter_length <= 0.0 {
        return None;
    }
    let (original_actual, original_normal) =
        limited_fraction(1.0 / quarter_length, DENOMINATOR_LIMIT)?;
    let (mut actual, mut normal) = (original_actual, original_normal);
    // Between one and two, which is where a tuplet ratio belongs.
    while actual < normal {
        actual *= 2;
        reduce(&mut actual, &mut normal);
    }
    while actual > normal * 2 {
        normal *= 2;
        reduce(&mut actual, &mut normal);
    }
    let (written, _) = quarter_length_to_closest_type(quarter_length / normal as FloatType).ok()?;
    // What is written inside the tuplet, which is the ratio the normalising
    // undid.
    let inside = (actual as FloatType / normal as FloatType)
        / (original_actual as FloatType / original_normal as FloatType);
    let (kind, dots) = exact_type_and_dots(inside)?;
    Some((
        Tuplet::new(
            u32::try_from(actual).ok()?,
            u32::try_from(normal).ok()?,
            written,
            0,
        ),
        kind,
        dots,
    ))
}

/// The written note values a length is made of and the tuplet, if any, they
/// are written inside: music21's `quarterConversion`.
///
/// One value for a plain or dotted note, one for a tuplet (the written
/// value, which the tuplet's ratio then scales), and several for a length
/// that can only be written as a tie — a quarter tied to a sixteenth for
/// five sixteenths. Empty for a length that runs off the end of the note
/// values, which is music21's `inexpressible`, and for a length of nought.
///
/// The tie is found greedily, largest value first, as music21 finds it:
/// take the largest note that fits, and look for a single dotted value
/// covering what is left before taking another bite. A length no tie of
/// eight values reaches is written as one note inside the tuplet
/// [`quarter_length_to_non_power_of_2_tuplet`] finds.
pub fn quarter_conversion(quarter_length: FloatType) -> (Vec<(DurationType, u32)>, Option<Tuplet>) {
    convert(quarter_length, true)
}

/// [`quarter_conversion`], with the search for an ordinary tuplet turned off
/// where a caller has already said which tuplets the length is written in.
fn convert(
    written: FloatType,
    look_for_tuplet: bool,
) -> (Vec<(DurationType, u32)>, Option<Tuplet>) {
    // Zero is written as nothing at all, which is what music21's empty
    // `components` says; `type_and_dots` would call it a `zero` note.
    if written == 0.0 {
        return (Vec::new(), None);
    }
    if let Some(value) = exact_type_and_dots(written) {
        return (vec![value], None);
    }
    // Shorter than the shortest note value, or longer than a tie of the
    // longest can reach: music21 calls both *inexpressible*, and asks this
    // before it looks for a tuplet.
    let Ok((largest, _)) = quarter_length_to_closest_type(written) else {
        return (Vec::new(), None);
    };
    if largest.next_larger().is_none() {
        return (Vec::new(), None);
    }
    if look_for_tuplet && let Some(tuplet) = quarter_length_to_tuplet(written, 1).into_iter().next()
    {
        return (vec![(tuplet.duration_type(), tuplet.dots())], Some(tuplet));
    }
    let mut components = vec![(largest, 0)];
    let mut remainder = written - largest.quarter_length();
    for _ in 0..MAX_TIED_COMPONENTS {
        if let Some(rest) = exact_type_and_dots(remainder) {
            components.push(rest);
            return (components, None);
        }
        let Ok((next, _)) = quarter_length_to_closest_type(remainder) else {
            break;
        };
        remainder -= next.quarter_length();
        components.push((next, 0));
    }
    match quarter_length_to_non_power_of_2_tuplet(written) {
        Some((tuplet, kind, dots)) => (vec![(kind, dots)], Some(tuplet)),
        None => (Vec::new(), None),
    }
}

/// The largest denominator [`mixed_numeral`] will write. music21 allows any
/// up to 65535, but it prints the quarter length of a written note, and a
/// note nobody can write does not want a fraction with a four-digit
/// denominator in its name.
const MAX_MIXED_NUMERAL_DENOMINATOR: u32 = 1024;

/// How close a fraction has to be to be written as one, relative to the
/// value.
const MIXED_NUMERAL_TOLERANCE: FloatType = 1e-6;

/// Writes a quarter length the way music21's `mixedNumeral` does: `"2"`,
/// `"1/2"`, `"1 3/4"`.
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
    fn a_type_is_found_from_its_undotted_length_alone() {
        assert_eq!(
            DurationType::from_quarter_length(2.0),
            Some(DurationType::Half)
        );
        assert_eq!(
            DurationType::from_quarter_length(0.125),
            Some(DurationType::ThirtySecond)
        );
        assert_eq!(DurationType::from_quarter_length(3.0), None);
        assert_eq!(super::mixed_numeral(2.0 / 3.0), "2/3");
        assert_eq!(super::mixed_numeral(1.0 / 3.0 + 1.0), "1 1/3");
    }

    #[test]
    fn a_duration_is_cleared_and_lengthened_a_value_at_a_time() {
        let mut duration = Duration::new(1.5).unwrap();
        assert_eq!(duration.dot_groups(), [1]);
        duration.add_duration_tuple(DurationType::Eighth, 0);
        assert_eq!(duration.quarter_length(), 2.0);
        duration.clear();
        assert_eq!(duration.quarter_length(), 0.0);
        assert_eq!(duration.dot_groups(), [0]);
        // Inside a triplet the value added is scaled as the rest is.
        let mut triplet = Duration::new(2.0 / 3.0).unwrap();
        triplet.add_duration_tuple(DurationType::Quarter, 0);
        assert!((triplet.quarter_length() - 4.0 / 3.0).abs() < 1e-9);
    }

    /// Every expectation here was read off music21.
    #[test]
    fn written_values_are_cut_consolidated_split_and_unlinked_as_music21_does() {
        let typed = |duration: &Duration| -> Vec<(Option<DurationType>, u32, FloatType)> {
            duration
                .written_values()
                .iter()
                .map(|value| (value.duration_type(), value.dots(), value.quarter_length()))
                .collect()
        };

        // A cut leaves the length alone, and a piece may have no type.
        let mut tied = Duration::new(1.25).unwrap();
        tied.slice_component_at_position(0.5).unwrap();
        assert_eq!(
            tied.components(),
            [
                (DurationType::Eighth, 0),
                (DurationType::Eighth, 0),
                (DurationType::Sixteenth, 0)
            ]
        );
        assert_eq!(tied.quarter_length(), 1.25);
        let mut dotted = Duration::new(3.0).unwrap();
        dotted.slice_component_at_position(1.25).unwrap();
        assert_eq!(
            typed(&dotted),
            [(None, 0, 1.25), (Some(DurationType::Quarter), 2, 1.75)]
        );
        assert!(
            Duration::new(1.25)
                .unwrap()
                .slice_component_at_position(1.0)
                .is_err()
        );

        // Consolidating keeps the length and may lose the type.
        let mut long = Duration::quarter();
        long.add_duration_tuple(DurationType::Half, 0);
        long.add_duration_tuple(DurationType::Half, 0);
        assert_eq!(long.components().len(), 3);
        assert_eq!(long.duration_type(), None);
        long.consolidate();
        assert_eq!(typed(&long), [(None, 0, 5.0)]);
        assert_eq!(long.quarter_length(), 5.0);
        let mut triplets = Duration::new(1.0 / 3.0).unwrap();
        triplets.add_duration_tuple(DurationType::Eighth, 0);
        triplets.consolidate();
        assert_eq!(triplets.components(), [(DurationType::Quarter, 0)]);
        assert!((triplets.quarter_length() - 2.0 / 3.0).abs() < 1e-9);

        // Dot groups multiply, and split into a tie of dotted values.
        let mut half = Duration::half();
        half.set_dot_groups(vec![1, 1]);
        assert_eq!(half.quarter_length(), 4.5);
        assert_eq!(half.components(), [(DurationType::Half, 0)]);
        let split = half.split_dot_groups().unwrap();
        assert_eq!(
            split.components(),
            [(DurationType::Half, 1), (DurationType::Quarter, 1)]
        );
        assert_eq!(split.quarter_length(), 4.5);
        half.set_dot_groups(vec![1, 1, 1]);
        assert_eq!(half.quarter_length(), 6.75);
        assert_eq!(
            half.split_dot_groups().unwrap().components(),
            [
                (DurationType::Half, 1),
                (DurationType::Quarter, 1),
                (DurationType::Quarter, 1),
                (DurationType::Eighth, 1)
            ]
        );
        assert_eq!(
            Duration::new(1.5)
                .unwrap()
                .split_dot_groups()
                .unwrap()
                .components(),
            [(DurationType::Quarter, 1)]
        );

        // Unlinked, the length and the written value go their own ways;
        // linked again, the value is read off the length.
        let mut unlinked = Duration::quarter();
        assert!(unlinked.linked());
        unlinked.set_linked(false);
        unlinked.set_quarter_length(3.0).unwrap();
        assert_eq!(unlinked.duration_type(), Some(DurationType::Quarter));
        assert_eq!(unlinked.quarter_length(), 3.0);
        unlinked.set_linked(true);
        assert_eq!(unlinked.type_and_dots(), Some((DurationType::Half, 1)));
        assert_eq!(unlinked.quarter_length(), 3.0);

        // A grace note keeps what it is written as and sounds nothing.
        let grace = Duration::new(1.25).unwrap().grace_duration();
        assert_eq!(
            typed(&grace),
            [
                (Some(DurationType::Quarter), 0, 0.0),
                (Some(DurationType::Sixteenth), 0, 0.0)
            ]
        );
        assert!(!grace.linked());
        assert_eq!(grace.quarter_length(), 0.0);
        let bare = Duration::new(0.0).unwrap().grace_duration();
        assert_eq!(bare.duration_type(), Some(DurationType::Eighth));
        let appoggiatura = Duration::new(1.0 / 3.0).unwrap().grace_duration();
        assert_eq!(appoggiatura.components(), [(DurationType::Eighth, 0)]);
        assert!(appoggiatura.tuplets().is_empty());
    }

    /// music21's own examples, over a tie the crate reads off the length.
    #[test]
    fn a_position_within_a_tie_names_the_value_sounding_there() {
        let tied = Duration::new(2.5).unwrap();
        assert_eq!(tied.components().len(), 2);
        assert_eq!(tied.component_index_at_qtr_position(0.0).unwrap(), 0);
        assert_eq!(tied.component_index_at_qtr_position(1.5).unwrap(), 0);
        assert_eq!(tied.component_index_at_qtr_position(2.0).unwrap(), 1);
        assert_eq!(tied.component_index_at_qtr_position(2.5).unwrap(), 1);
        assert!(tied.component_index_at_qtr_position(3.0).is_err());
        assert!(tied.component_index_at_qtr_position(-1.0).is_err());
        assert!(
            Duration::new(0.0)
                .unwrap()
                .component_index_at_qtr_position(0.0)
                .is_err()
        );
        assert_eq!(tied.component_start_time(0).unwrap(), 0.0);
        assert_eq!(tied.component_start_time(1).unwrap(), 2.0);
        assert!(tied.component_start_time(2).is_err());
    }

    #[test]
    fn a_tuplet_reads_and_writes_both_of_its_sides() {
        let mut tuplet = Tuplet::new(3, 2, DurationType::Eighth, 0);
        assert_eq!(tuplet.duration_actual(), Some((DurationType::Eighth, 0)));
        assert_eq!(tuplet.duration_normal(), Some((DurationType::Eighth, 0)));
        assert_eq!(tuplet.tuplet_actual(), (3, Some((DurationType::Eighth, 0))));
        assert_eq!(tuplet.tuplet_normal(), (2, Some((DurationType::Eighth, 0))));
        tuplet.set_duration_type(DurationType::Quarter, 1);
        assert_eq!(tuplet.duration_actual(), Some((DurationType::Quarter, 1)));
        assert_eq!(tuplet.normal_duration_type(), DurationType::Quarter);
        assert_eq!(tuplet.normal_dots(), 1);
        tuplet.set_ratio(5, 4);
        assert_eq!((tuplet.actual(), tuplet.normal()), (5, 4));
        assert_eq!(tuplet.multiplier(), FractionType::new(4, 5));
    }

    /// Every answer here is music21's, for a tuplet saying no written value
    /// and for ones whose two sides are counted in different values.
    #[test]
    fn a_tuplet_may_leave_its_written_values_unsaid() {
        let mut triplet = Tuplet::ratio(3, 2);
        assert_eq!(triplet.duration_actual(), None);
        assert_eq!(triplet.total_tuplet_length(), 1.0);
        assert_eq!(triplet.multiplier(), FractionType::new(2, 3));
        triplet.set_duration_type(DurationType::Half, 1);
        assert_eq!(triplet.total_tuplet_length(), 6.0);

        let mut three_in_one = Tuplet::ratio(3, 1);
        three_in_one.set_duration_actual(Some((DurationType::Quarter, 0)));
        three_in_one.set_duration_normal(Some((DurationType::Half, 0)));
        assert_eq!(three_in_one.multiplier(), FractionType::new(2, 3));
        assert_eq!(three_in_one.total_tuplet_length(), 2.0);

        let mut six = Tuplet::ratio(6, 2);
        six.set_duration_type(DurationType::Eighth, 0);
        let halved = six.augmented(0.5).unwrap();
        assert_eq!(halved.duration_actual(), Some((DurationType::Sixteenth, 0)));
        assert_eq!(halved.multiplier(), FractionType::new(1, 3));
        assert!(six.augmented(-1.0).is_err());

        let mut five = Tuplet::ratio(5, 2);
        five.set_duration_type(DurationType::Half, 0);
        five.set_duration_normal(Some((DurationType::Whole, 0)));
        assert_eq!(five.total_tuplet_length(), 8.0);
    }

    /// music21's own examples for `quarterLengthToTuplet`,
    /// `quarterLengthToNonPowerOf2Tuplet` and `quarterConversion`.
    #[test]
    fn a_length_is_read_as_the_tuplets_and_ties_music21_reads() {
        use super::{
            Duration, DurationType, quarter_conversion, quarter_length_to_non_power_of_2_tuplet,
            quarter_length_to_tuplet,
        };

        let names = |tuplets: Vec<Tuplet>| -> Vec<String> {
            tuplets
                .iter()
                .map(|tuplet| {
                    format!(
                        "{}/{}/{}",
                        tuplet.actual(),
                        tuplet.normal(),
                        tuplet.duration_type().music21_name()
                    )
                })
                .collect()
        };
        assert_eq!(
            names(quarter_length_to_tuplet(0.333_333_33, 4)),
            ["3/2/eighth", "3/1/quarter"]
        );
        assert_eq!(
            names(quarter_length_to_tuplet(0.20, 4)),
            ["5/4/16th", "5/2/eighth", "5/1/quarter"]
        );
        assert_eq!(
            names(quarter_length_to_tuplet(0.333_333_3, 1)),
            ["3/2/eighth"]
        );
        // A plain quarter is also two dotted quarters in the time of three,
        // which is why `Duration::tuplet` asks for the exact value first.
        let plain = quarter_length_to_tuplet(1.0, 1);
        assert_eq!(names(plain.clone()), ["3/2/quarter"]);
        assert_eq!(plain[0].dots(), 1);
        assert!(quarter_length_to_tuplet(0.0, 4).is_empty());

        let (tuplet, kind, dots) = quarter_length_to_non_power_of_2_tuplet(7.0).unwrap();
        assert_eq!(names(vec![tuplet]), ["8/7/quarter"]);
        assert_eq!((kind, dots), (DurationType::Breve, 0));
        let (tuplet, kind, _) = quarter_length_to_non_power_of_2_tuplet(7.0 / 3.0).unwrap();
        assert_eq!(names(vec![tuplet]), ["12/7/16th"]);
        assert_eq!(kind, DurationType::Whole);
        assert!(quarter_length_to_non_power_of_2_tuplet(0.0).is_none());

        let (components, tuplet) = quarter_conversion(2.5);
        assert_eq!(
            components,
            [(DurationType::Half, 0), (DurationType::Eighth, 0)]
        );
        assert!(tuplet.is_none());
        let (components, tuplet) = quarter_conversion(2.0 / 3.0);
        assert_eq!(components, [(DurationType::Quarter, 0)]);
        assert_eq!(names(tuplet.into_iter().collect()), ["3/2/quarter"]);
        let (components, tuplet) = quarter_conversion(3.75);
        assert_eq!(components, [(DurationType::Half, 3)]);
        assert!(tuplet.is_none());
        assert_eq!(quarter_conversion(99.0), (Vec::new(), None));
        assert_eq!(quarter_conversion(0.0), (Vec::new(), None));

        assert!(Duration::new(2.5).unwrap().is_complex());
        assert!(!Duration::new(3.0).unwrap().is_complex());
        assert!(!Duration::new(2.0 / 3.0).unwrap().is_complex());
    }

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
        // A quarter over three eighths, as music21 reads it: the multiplier
        // is what each eighth is scaled by, not the ratio of the counts.
        assert_eq!(across.multiplier(), FractionType::new(2i32, 3i32));
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
