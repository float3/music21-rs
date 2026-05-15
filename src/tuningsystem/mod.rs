use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Default octave size for twelve-tone systems.
pub const OCTAVE_SIZE: UnsignedIntegerType = 12;

/// Frequency of middle C in hertz.
pub const C4: FloatType = 261.6256;
/// Frequency of C0 in hertz.
pub const C0: FloatType = C4 / 16.0;
/// Frequency of C-1 in hertz.
pub const CN1: FloatType = C4 / 32.0;

/// Frequency of A4 in hertz.
pub const A4: FloatType = 440.0;
/// Frequency of A0 in hertz.
pub const A0: FloatType = A4 / 16.0;
/// Frequency of A-1 in hertz.
pub const AN1: FloatType = A4 / 32.0;

/// Degree labels for a twelve-tone chromatic octave.
pub const TWELVE_TONE_NAMES: [&str; 12] = [
    "C", "C#/Db", "D", "D#/Eb", "E", "F", "F#/Gb", "G", "G#/Ab", "A", "A#/Bb", "B",
];

/// Degree labels for a whole-tone octave.
pub const WHOLE_TONE_NAMES: [&str; 6] = ["C", "D", "E", "F#/Gb", "G#/Ab", "A#/Bb"];

/// The common twelve-tone tuning systems useful for comparing pitch frequencies.
pub const COMMON_TWELVE_TONE_TUNING_SYSTEMS: [TuningSystem; 4] = [
    TuningSystem::EqualTemperament {
        octave_size: OCTAVE_SIZE,
    },
    TuningSystem::JustIntonation,
    TuningSystem::PythagoreanTuning,
    TuningSystem::FiveLimit,
];

/// All built-in tuning systems in canonical display order.
pub const ALL_TUNING_SYSTEMS: [TuningSystem; 17] = [
    TuningSystem::EqualTemperament {
        octave_size: OCTAVE_SIZE,
    },
    TuningSystem::WholeTone,
    TuningSystem::QuarterTone,
    TuningSystem::JustIntonation,
    TuningSystem::RecursiveJustIntonation,
    TuningSystem::TwelveTetRootedJust,
    TuningSystem::JustIntonation24,
    TuningSystem::PythagoreanTuning,
    TuningSystem::FiveLimit,
    TuningSystem::ElevenLimit,
    TuningSystem::FortyThreeTone,
    TuningSystem::Javanese,
    TuningSystem::Thai,
    TuningSystem::Indian,
    TuningSystem::IndianAlt,
    TuningSystem::Indian22,
    TuningSystem::IndianFull,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A ratio-like value used by tuning tables.
pub struct Fraction {
    /// Numerator for a rational ratio, or exponent numerator when `base` is set.
    pub numerator: UnsignedIntegerType,
    /// Denominator for a rational ratio, or exponent denominator when `base` is set.
    pub denominator: UnsignedIntegerType,
    /// Exponential base. A value of `0` means use `numerator / denominator`.
    pub base: UnsignedIntegerType,
}

impl Fraction {
    /// Creates a rational fraction.
    pub const fn new(numerator: UnsignedIntegerType, denominator: UnsignedIntegerType) -> Self {
        Self::new_with_base(numerator, denominator, 0)
    }

    /// Creates a fraction with an optional exponential base.
    pub const fn new_with_base(
        numerator: UnsignedIntegerType,
        denominator: UnsignedIntegerType,
        base: UnsignedIntegerType,
    ) -> Self {
        Self {
            numerator,
            denominator,
            base,
        }
    }

    /// Returns the numerator.
    pub const fn numerator(&self) -> UnsignedIntegerType {
        self.numerator
    }

    /// Returns the denominator.
    pub const fn denominator(&self) -> UnsignedIntegerType {
        self.denominator
    }

    /// Returns the exponential base, or `0` for rational ratios.
    pub const fn base(&self) -> UnsignedIntegerType {
        self.base
    }

    /// Converts this value into a floating-point ratio.
    pub fn ratio(self) -> FloatType {
        self.into()
    }

    /// Returns a compact music-friendly display label.
    pub fn label(self) -> String {
        self.to_string()
    }

    /// Returns this fraction shifted upward by `octaves`.
    pub fn with_octaves(mut self, octaves: UnsignedIntegerType) -> Self {
        if octaves == 0 {
            return self;
        }

        if self.base == 0 {
            let multiplier = (2 as UnsignedIntegerType)
                .checked_pow(octaves)
                .expect("octave multiplier exceeds u32 range");
            self.numerator = self
                .numerator
                .checked_mul(multiplier)
                .expect("fraction numerator exceeds u32 range");
        } else {
            let octave_offset = self
                .denominator
                .checked_mul(octaves)
                .expect("fraction octave offset exceeds u32 range");
            self.numerator = self
                .numerator
                .checked_add(octave_offset)
                .expect("fraction numerator exceeds u32 range");
        }

        self
    }
}

impl From<Fraction> for FloatType {
    fn from(frac: Fraction) -> Self {
        if frac.base == 0 {
            frac.numerator as FloatType / frac.denominator as FloatType
        } else {
            (frac.base as FloatType)
                .powf(frac.numerator as FloatType / frac.denominator as FloatType)
        }
    }
}

impl Display for Fraction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.base == 0 {
            if self.denominator == 1 {
                write!(f, "{}", self.numerator)
            } else {
                write!(f, "{}/{}", self.numerator, self.denominator)
            }
        } else if self.numerator == 0 {
            write!(f, "1")
        } else {
            write!(f, "{}^({}/{})", self.base, self.numerator, self.denominator)
        }
    }
}

impl From<(UnsignedIntegerType, UnsignedIntegerType)> for Fraction {
    fn from(frac: (UnsignedIntegerType, UnsignedIntegerType)) -> Self {
        Self::new(frac.0, frac.1)
    }
}

impl
    From<(
        UnsignedIntegerType,
        UnsignedIntegerType,
        UnsignedIntegerType,
    )> for Fraction
{
    fn from(
        frac: (
            UnsignedIntegerType,
            UnsignedIntegerType,
            UnsignedIntegerType,
        ),
    ) -> Self {
        Self::new_with_base(frac.0, frac.1, frac.2)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Supported tuning systems and ratio tables.
pub enum TuningSystem {
    /// Equal temperament with a configurable octave size.
    EqualTemperament {
        /// Number of equal divisions in each octave.
        octave_size: UnsignedIntegerType,
    },
    /// Six-tone equal temperament.
    WholeTone,
    /// Twenty-four-tone equal temperament.
    QuarterTone,

    /// Twelve-tone just intonation table.
    JustIntonation,
    /// Twenty-four-tone just intonation table.
    JustIntonation24,
    /// Twelve-tone Pythagorean tuning table.
    PythagoreanTuning,

    /// Twelve-tone five-limit table.
    FiveLimit,
    /// Twenty-nine-tone eleven-limit table.
    ElevenLimit,

    /// Forty-three-tone ratio table.
    FortyThreeTone,

    /// Twelve-tone recursive just intonation table.
    RecursiveJustIntonation,
    /// Twelve-tone just-intervals table rooted on twelve-tone equal temperament.
    TwelveTetRootedJust,

    // Ethnic scales.
    /// Five-tone Javanese equal-temperament approximation.
    Javanese,
    /// Seven-tone Thai equal-temperament approximation.
    Thai,
    /// Seven-tone Indian scale table.
    Indian,
    /// Alternate seven-tone Indian scale table.
    IndianAlt,
    /// Twenty-two-tone Indian scale table.
    Indian22,
    /// Full twenty-two-tone Indian scale table.
    IndianFull,
}

impl TuningSystem {
    /// Returns the canonical identifier used by [`FromStr`].
    pub fn id(self) -> &'static str {
        match self {
            Self::EqualTemperament { .. } => "EqualTemperament",
            Self::WholeTone => "WholeTone",
            Self::QuarterTone => "QuarterTone",
            Self::JustIntonation => "JustIntonation",
            Self::JustIntonation24 => "JustIntonation24",
            Self::PythagoreanTuning => "PythagoreanTuning",
            Self::FiveLimit => "FiveLimit",
            Self::ElevenLimit => "ElevenLimit",
            Self::FortyThreeTone => "FortyThreeTone",
            Self::RecursiveJustIntonation => "RecursiveJustIntonation",
            Self::TwelveTetRootedJust => "TwelveTetRootedJust",
            Self::Javanese => "Javanese",
            Self::Thai => "Thai",
            Self::Indian => "Indian",
            Self::IndianAlt => "IndianAlt",
            Self::Indian22 => "Indian22",
            Self::IndianFull => "IndianFull",
        }
    }

    /// Returns a compact display name for this tuning system.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::EqualTemperament { .. } => "Equal temperament",
            Self::WholeTone => "Whole tone",
            Self::QuarterTone => "Quarter tone",
            Self::JustIntonation => "Just intonation",
            Self::JustIntonation24 => "Just intonation 24",
            Self::PythagoreanTuning => "Pythagorean",
            Self::FiveLimit => "Five-limit",
            Self::ElevenLimit => "Eleven-limit",
            Self::FortyThreeTone => "Forty-three tone",
            Self::RecursiveJustIntonation => "Recursive just intonation",
            Self::TwelveTetRootedJust => "12-TET-rooted just intervals",
            Self::Javanese => "Javanese",
            Self::Thai => "Thai",
            Self::Indian => "Indian",
            Self::IndianAlt => "Indian alternate",
            Self::Indian22 => "Indian 22",
            Self::IndianFull => "Indian full",
        }
    }

    /// Returns a short description of this tuning system.
    pub fn description(self) -> &'static str {
        match self {
            Self::EqualTemperament { .. } => "Twelve equal divisions of the octave.",
            Self::WholeTone => "Six equal whole-tone steps per octave.",
            Self::QuarterTone => "Twenty-four equal quarter-tone steps per octave.",
            Self::JustIntonation => "A twelve-tone just-intonation ratio table.",
            Self::JustIntonation24 => "A twenty-four-tone just-intonation ratio table.",
            Self::PythagoreanTuning => "A twelve-tone tuning table built from pure fifths.",
            Self::FiveLimit => "A twelve-tone table using five-limit just ratios.",
            Self::ElevenLimit => "A twenty-nine-tone table using eleven-limit ratios.",
            Self::FortyThreeTone => "A forty-three-tone ratio table.",
            Self::RecursiveJustIntonation => {
                "Chord-contextual recursive just intonation using a twelve-tone table."
            }
            Self::TwelveTetRootedJust => {
                "A twelve-tone just-intonation interval table rooted on twelve-tone equal temperament."
            }
            Self::Javanese => "A five-tone Javanese equal-temperament approximation.",
            Self::Thai => "A seven-tone Thai equal-temperament approximation.",
            Self::Indian => "A seven-tone Indian scale ratio table.",
            Self::IndianAlt => "An alternate seven-tone Indian scale ratio table.",
            Self::Indian22 => "A twenty-two-tone Indian scale ratio table.",
            Self::IndianFull => "The full twenty-two-tone Indian scale table.",
        }
    }

    /// Returns the frequency ratio for a degree index.
    pub fn ratio(self, index: usize) -> FloatType {
        get_ratio(self, index, None)
    }

    /// Returns the table fraction for a degree index.
    pub fn fraction(self, index: usize) -> Fraction {
        get_fraction(self, index, None)
    }

    /// Returns a display label for a degree index.
    pub fn label(self, index: UnsignedIntegerType) -> String {
        get_label(self, index, None)
    }

    /// Returns the octave number containing a degree index.
    pub fn octave(self, index: UnsignedIntegerType) -> UnsignedIntegerType {
        index / self.octave_size()
    }

    /// Returns the frequency in hertz for a degree index.
    pub fn frequency(self, index: UnsignedIntegerType) -> FloatType {
        get_frequency(self, index, None)
    }

    /// Returns the frequency in hertz for a fractional degree index.
    pub fn frequency_at(self, index: FloatType) -> FloatType {
        get_frequency_at(self, index, None)
    }

    /// Returns the frequency in hertz for `index` in a recursive root context.
    ///
    /// The `root_index` is first tuned from the global C-based table, then the
    /// distance from `root_index` to `index` is tuned from the same table again.
    /// For equal temperament this is identical to [`Self::frequency_at`]. For
    /// ratio-table systems such as [`Self::JustIntonation`], the result is a
    /// chord-contextual frequency.
    pub fn recursive_frequency_at(self, root_index: FloatType, index: FloatType) -> FloatType {
        get_recursive_frequency_at(self, root_index, index, None)
    }

    /// Returns cents offset from equal temperament for a degree index.
    pub fn cents(self, index: UnsignedIntegerType) -> FloatType {
        get_cents(self, index, None)
    }

    /// Returns cents offset from equal temperament for a fractional degree index.
    pub fn cents_at(self, index: FloatType) -> FloatType {
        get_cents_at(self, index, None)
    }

    /// Returns cents offset from equal temperament in a recursive root context.
    pub fn recursive_cents_at(self, root_index: FloatType, index: FloatType) -> FloatType {
        get_recursive_cents_at(self, root_index, index, None)
    }

    /// Returns the number of degrees in one octave for this tuning system.
    pub fn octave_size(self) -> UnsignedIntegerType {
        match self {
            Self::EqualTemperament { octave_size } => octave_size,
            Self::WholeTone => 6,
            Self::QuarterTone | Self::JustIntonation24 => 24,
            Self::FortyThreeTone => 43,
            Self::ElevenLimit => 29,
            Self::Javanese => 5,
            Self::Thai | Self::Indian | Self::IndianAlt => 7,
            Self::Indian22 | Self::IndianFull => 22,
            Self::JustIntonation
            | Self::PythagoreanTuning
            | Self::FiveLimit
            | Self::RecursiveJustIntonation
            | Self::TwelveTetRootedJust => OCTAVE_SIZE,
        }
    }

    fn ratio_table(self) -> Option<&'static [Fraction]> {
        match self {
            Self::JustIntonation => Some(&JUST_INTONATION),
            Self::JustIntonation24 => Some(&JUST_INTONATION_24),
            Self::PythagoreanTuning => Some(&PYTHAGOREAN_TUNING),
            Self::FiveLimit => Some(&FIVE_LIMIT),
            Self::ElevenLimit => Some(&ELEVEN_LIMIT),
            Self::FortyThreeTone => Some(&FORTY_THREE_TONE),
            Self::RecursiveJustIntonation => Some(&JUST_INTONATION),
            Self::TwelveTetRootedJust => Some(&JUST_INTONATION),
            Self::Javanese => Some(&JAVANESE),
            Self::Thai => Some(&THAI),
            Self::Indian => Some(&INDIAN_SCALE),
            Self::IndianAlt => Some(&INDIA_SCALE_ALT),
            Self::Indian22 | Self::IndianFull => Some(&INDIAN_SCALE_22),
            Self::EqualTemperament { .. } | Self::WholeTone | Self::QuarterTone => None,
        }
    }

    fn degree_label(self, index: UnsignedIntegerType, octave_size: UnsignedIntegerType) -> String {
        if octave_size == 0 {
            return default_degree_label(OCTAVE_SIZE, index);
        }

        let degree = index % octave_size;
        match self {
            Self::WholeTone if octave_size == 6 => WHOLE_TONE_NAMES[degree as usize].to_string(),
            Self::Indian | Self::IndianAlt if octave_size == 7 => {
                INDIAN_SCALE_NAMES[degree as usize].to_string()
            }
            _ => default_degree_label(octave_size, index),
        }
    }
}

impl Display for TuningSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

impl FromStr for TuningSystem {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "EqualTemperament" => Ok(Self::EqualTemperament {
                octave_size: OCTAVE_SIZE,
            }),
            "WholeTone" => Ok(Self::WholeTone),
            "QuarterTone" => Ok(Self::QuarterTone),
            "JustIntonation" => Ok(Self::JustIntonation),
            "JustIntonation24" => Ok(Self::JustIntonation24),
            "PythagoreanTuning" => Ok(Self::PythagoreanTuning),
            "FiveLimit" => Ok(Self::FiveLimit),
            "ElevenLimit" => Ok(Self::ElevenLimit),
            "FortyThreeTone" => Ok(Self::FortyThreeTone),
            "RecursiveJustIntonation" => Ok(Self::RecursiveJustIntonation),
            "TwelveTetRootedJust" => Ok(Self::TwelveTetRootedJust),
            "StepMethod" => Ok(Self::RecursiveJustIntonation),
            "Javanese" => Ok(Self::Javanese),
            "Thai" => Ok(Self::Thai),
            "Indian" => Ok(Self::Indian),
            "IndianAlt" => Ok(Self::IndianAlt),
            "Indian22" => Ok(Self::Indian22),
            "IndianFull" => Ok(Self::IndianFull),
            _ => Err(Error::TuningSystem(format!("unknown tuning system {s:?}"))),
        }
    }
}

/// Creates an equal-temperament fraction for `tone` within `octave_size`.
pub fn equal_temperament(tone: UnsignedIntegerType, octave_size: UnsignedIntegerType) -> Fraction {
    Fraction::new_with_base(tone, octave_size, 2)
}

/// Creates a twelve-tone equal-temperament fraction.
pub fn equal_temperament_12(tone: UnsignedIntegerType) -> Fraction {
    equal_temperament(tone, 12)
}

/// Creates an equal-temperament fraction using [`OCTAVE_SIZE`].
pub fn equal_temperament_default(tone: UnsignedIntegerType) -> Fraction {
    equal_temperament(tone, OCTAVE_SIZE)
}

/// Returns the frequency ratio for a tuning-system degree.
pub fn get_ratio(
    tuning_system: TuningSystem,
    index: usize,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    get_fraction(tuning_system, index, size).into()
}

/// Returns the fraction for a tuning-system degree.
///
/// The optional `size` overrides the tuning system's octave size for
/// equal-temperament-style systems.
pub fn get_fraction(
    tuning_system: TuningSystem,
    index: usize,
    size: Option<UnsignedIntegerType>,
) -> Fraction {
    match tuning_system {
        TuningSystem::EqualTemperament { octave_size } => equal_temperament(
            index_to_unsigned_integer(index),
            size.unwrap_or(octave_size),
        ),
        TuningSystem::WholeTone => {
            equal_temperament(index_to_unsigned_integer(index), size.unwrap_or(6))
        }
        TuningSystem::QuarterTone => {
            equal_temperament(index_to_unsigned_integer(index), size.unwrap_or(24))
        }
        TuningSystem::RecursiveJustIntonation | TuningSystem::TwelveTetRootedJust => {
            get_fraction_from_table(tuning_system, index)
        }
        _ => get_fraction_from_table(tuning_system, index),
    }
}

/// Returns a display label for a tuning-system degree.
///
/// The optional `size` overrides the tuning system's octave size for label
/// calculation.
pub fn get_label(
    tuning_system: TuningSystem,
    index: UnsignedIntegerType,
    size: Option<UnsignedIntegerType>,
) -> String {
    let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
    assert!(octave_size > 0, "octave_size must be greater than zero");
    degree_name_with_octave(
        &tuning_system.degree_label(index, octave_size),
        index / octave_size,
    )
}

/// Returns the frequency in hertz for a tuning-system degree.
///
/// The optional `size` overrides the tuning system's octave size for
/// equal-temperament-style systems.
pub fn get_frequency(
    tuning_system: TuningSystem,
    index: UnsignedIntegerType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    get_frequency_at(tuning_system, FloatType::from(index), size)
}

/// Returns the frequency in hertz for a fractional tuning-system degree.
///
/// Integer degrees use the tuning system table exactly. Fractional degrees are
/// interpolated by equal-temperament distance within the same octave.
pub fn get_frequency_at(
    tuning_system: TuningSystem,
    index: FloatType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    CN1 * get_ratio_at(tuning_system, index, size)
}

/// Returns the recursive frequency in hertz for a fractional tuning degree.
///
/// This treats `root_index` as a local tonic: the root is tuned from the global
/// C-based table, and the note's distance above or below that root is tuned
/// using the same system again.
pub fn get_recursive_frequency_at(
    tuning_system: TuningSystem,
    root_index: FloatType,
    index: FloatType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    assert!(root_index.is_finite(), "root degree index must be finite");
    assert!(index.is_finite(), "degree index must be finite");
    if tuning_system == TuningSystem::TwelveTetRootedJust {
        let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
        get_frequency_at(
            TuningSystem::EqualTemperament { octave_size },
            root_index,
            size,
        ) * get_ratio_at(TuningSystem::JustIntonation, index - root_index, size)
    } else {
        CN1 * get_ratio_at(tuning_system, root_index, size)
            * get_ratio_at(tuning_system, index - root_index, size)
    }
}

fn get_ratio_at(
    tuning_system: TuningSystem,
    index: FloatType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    assert!(index.is_finite(), "degree index must be finite");
    let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
    assert!(octave_size > 0, "octave_size must be greater than zero");

    if tuning_system.ratio_table().is_none() {
        return (2.0 as FloatType).powf(index / FloatType::from(octave_size));
    }

    let base_index = index.floor() as IntegerType;
    let fractional_degree = index - FloatType::from(base_index);
    get_ratio_at_integer_index(tuning_system, base_index)
        * (2.0 as FloatType).powf(fractional_degree / FloatType::from(octave_size))
}

/// Returns cents offset from equal temperament for a tuning-system degree.
///
/// The optional `size` overrides the tuning system's octave size for the
/// equal-temperament comparison.
pub fn get_cents(
    tuning_system: TuningSystem,
    index: UnsignedIntegerType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    get_cents_at(tuning_system, FloatType::from(index), size)
}

/// Returns cents offset from equal temperament for a fractional degree index.
///
/// The optional `size` overrides the octave size of the equal-temperament
/// comparison.
pub fn get_cents_at(
    tuning_system: TuningSystem,
    index: FloatType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
    assert!(octave_size > 0, "octave_size must be greater than zero");
    let reference_freq = get_frequency_at(
        TuningSystem::EqualTemperament { octave_size },
        index,
        Some(octave_size),
    );
    let comparison_freq = get_frequency_at(tuning_system, index, size);
    1200.0 * (comparison_freq / reference_freq).log2()
}

/// Returns cents offset from equal temperament in a recursive root context.
pub fn get_recursive_cents_at(
    tuning_system: TuningSystem,
    root_index: FloatType,
    index: FloatType,
    size: Option<UnsignedIntegerType>,
) -> FloatType {
    let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
    assert!(octave_size > 0, "octave_size must be greater than zero");
    let reference_freq = get_frequency_at(
        TuningSystem::EqualTemperament { octave_size },
        index,
        Some(octave_size),
    );
    let comparison_freq = get_recursive_frequency_at(tuning_system, root_index, index, size);
    1200.0 * (comparison_freq / reference_freq).log2()
}

fn get_fraction_from_table(tuning_system: TuningSystem, index: usize) -> Fraction {
    let table = tuning_system
        .ratio_table()
        .expect("tuning system does not have a ratio table");
    let len = table.len();
    let octaves = (index / len) as UnsignedIntegerType;
    table[index % len].with_octaves(octaves)
}

fn get_ratio_at_integer_index(tuning_system: TuningSystem, index: IntegerType) -> FloatType {
    let table = tuning_system
        .ratio_table()
        .expect("tuning system does not have a ratio table");
    let len = IntegerType::try_from(table.len()).expect("ratio table length exceeds i32 range");
    let octave = index.div_euclid(len);
    let degree = index.rem_euclid(len) as usize;
    table[degree].ratio() * (2.0 as FloatType).powi(octave)
}

fn index_to_unsigned_integer(index: usize) -> UnsignedIntegerType {
    UnsignedIntegerType::try_from(index).expect("tone index exceeds u32 range")
}

fn default_degree_label(octave_size: UnsignedIntegerType, index: UnsignedIntegerType) -> String {
    if octave_size == OCTAVE_SIZE {
        TWELVE_TONE_NAMES[(index % OCTAVE_SIZE) as usize].to_string()
    } else {
        format!("T{}", index % octave_size)
    }
}

fn degree_name_with_octave(degree_label: &str, octave: UnsignedIntegerType) -> String {
    let adjusted_octave = i64::from(octave) - 1;
    let generic_degree_label = degree_label
        .strip_prefix('T')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()));

    if generic_degree_label {
        return if adjusted_octave < 0 {
            format!("{degree_label}ON{}", -adjusted_octave)
        } else {
            format!("{degree_label}O{adjusted_octave}")
        };
    }

    if adjusted_octave < 0 {
        format!("{degree_label}N{}", -adjusted_octave)
    } else {
        format!("{degree_label}{adjusted_octave}")
    }
}

/// Twelve-tone just intonation ratios.
pub const JUST_INTONATION: [Fraction; 12] = [
    Fraction::new(1, 1),
    Fraction::new(17, 16),
    Fraction::new(9, 8),
    Fraction::new(19, 16),
    Fraction::new(5, 4),
    Fraction::new(4, 3),
    Fraction::new(45, 32),
    Fraction::new(3, 2),
    Fraction::new(51, 32),
    Fraction::new(27, 16),
    Fraction::new(57, 32),
    Fraction::new(15, 8),
];

/// Twenty-four-tone just intonation ratios.
pub const JUST_INTONATION_24: [Fraction; 24] = [
    Fraction::new(1, 1),
    Fraction::new(33, 32),
    Fraction::new(17, 16),
    Fraction::new(35, 32),
    Fraction::new(9, 8),
    Fraction::new(37, 32),
    Fraction::new(19, 16),
    Fraction::new(39, 32),
    Fraction::new(5, 4),
    Fraction::new(41, 32),
    Fraction::new(4, 3),
    Fraction::new(11, 8),
    Fraction::new(45, 32),
    Fraction::new(93, 64),
    Fraction::new(3, 2),
    Fraction::new(99, 64),
    Fraction::new(51, 32),
    Fraction::new(105, 64),
    Fraction::new(27, 16),
    Fraction::new(111, 64),
    Fraction::new(57, 32),
    Fraction::new(117, 64),
    Fraction::new(15, 8),
    Fraction::new(31, 16),
];

/// Twelve-tone Pythagorean tuning ratios.
pub const PYTHAGOREAN_TUNING: [Fraction; 12] = [
    Fraction::new(1, 1),
    Fraction::new(256, 243),
    Fraction::new(9, 8),
    Fraction::new(32, 27),
    Fraction::new(81, 64),
    Fraction::new(4, 3),
    Fraction::new(729, 512),
    Fraction::new(3, 2),
    Fraction::new(128, 81),
    Fraction::new(27, 16),
    Fraction::new(16, 9),
    Fraction::new(243, 128),
];

/// Twelve-tone five-limit tuning ratios.
pub const FIVE_LIMIT: [Fraction; 12] = [
    Fraction::new(1, 1),
    Fraction::new(16, 15),
    Fraction::new(9, 8),
    Fraction::new(6, 5),
    Fraction::new(5, 4),
    Fraction::new(4, 3),
    Fraction::new(64, 45),
    Fraction::new(3, 2),
    Fraction::new(8, 5),
    Fraction::new(5, 3),
    Fraction::new(16, 9),
    Fraction::new(15, 8),
];

/// Twenty-nine-tone eleven-limit tuning ratios.
pub const ELEVEN_LIMIT: [Fraction; 29] = [
    Fraction::new(1, 1),
    Fraction::new(12, 11),
    Fraction::new(11, 10),
    Fraction::new(10, 9),
    Fraction::new(9, 8),
    Fraction::new(8, 7),
    Fraction::new(7, 6),
    Fraction::new(6, 5),
    Fraction::new(11, 9),
    Fraction::new(5, 4),
    Fraction::new(14, 11),
    Fraction::new(9, 7),
    Fraction::new(4, 3),
    Fraction::new(11, 8),
    Fraction::new(7, 5),
    Fraction::new(10, 7),
    Fraction::new(16, 11),
    Fraction::new(3, 2),
    Fraction::new(14, 9),
    Fraction::new(11, 7),
    Fraction::new(8, 5),
    Fraction::new(18, 11),
    Fraction::new(5, 3),
    Fraction::new(12, 7),
    Fraction::new(7, 4),
    Fraction::new(16, 9),
    Fraction::new(9, 5),
    Fraction::new(20, 11),
    Fraction::new(11, 6),
];

/// Forty-three-tone tuning ratios.
pub const FORTY_THREE_TONE: [Fraction; 43] = [
    Fraction::new(1, 1),
    Fraction::new(81, 80),
    Fraction::new(33, 32),
    Fraction::new(21, 20),
    Fraction::new(16, 15),
    Fraction::new(12, 11),
    Fraction::new(11, 10),
    Fraction::new(10, 9),
    Fraction::new(9, 8),
    Fraction::new(8, 7),
    Fraction::new(7, 6),
    Fraction::new(32, 27),
    Fraction::new(6, 5),
    Fraction::new(11, 9),
    Fraction::new(5, 4),
    Fraction::new(14, 11),
    Fraction::new(9, 7),
    Fraction::new(21, 16),
    Fraction::new(4, 3),
    Fraction::new(27, 20),
    Fraction::new(11, 8),
    Fraction::new(7, 5),
    Fraction::new(10, 7),
    Fraction::new(16, 11),
    Fraction::new(40, 27),
    Fraction::new(3, 2),
    Fraction::new(23, 21),
    Fraction::new(14, 9),
    Fraction::new(11, 7),
    Fraction::new(8, 5),
    Fraction::new(18, 11),
    Fraction::new(5, 3),
    Fraction::new(27, 16),
    Fraction::new(12, 7),
    Fraction::new(7, 4),
    Fraction::new(16, 8),
    Fraction::new(9, 5),
    Fraction::new(20, 11),
    Fraction::new(11, 6),
    Fraction::new(15, 8),
    Fraction::new(40, 21),
    Fraction::new(64, 33),
    Fraction::new(160, 81),
];

/// Backwards-compatible alias for [`FORTY_THREE_TONE`].
pub const FORTYTHREE_TONE: [Fraction; 43] = FORTY_THREE_TONE;

/// Five-tone Javanese equal-temperament approximation.
pub const JAVANESE: [Fraction; 5] = [
    Fraction::new_with_base(0, 5, 2),
    Fraction::new_with_base(1, 5, 2),
    Fraction::new_with_base(2, 5, 2),
    Fraction::new_with_base(3, 5, 2),
    Fraction::new_with_base(4, 5, 2),
];

/// Seven-tone Thai equal-temperament approximation.
pub const THAI: [Fraction; 7] = [
    Fraction::new_with_base(0, 7, 2),
    Fraction::new_with_base(1, 7, 2),
    Fraction::new_with_base(2, 7, 2),
    Fraction::new_with_base(3, 7, 2),
    Fraction::new_with_base(4, 7, 2),
    Fraction::new_with_base(5, 7, 2),
    Fraction::new_with_base(6, 7, 2),
];

/// Degree labels for the seven-tone Indian scale.
pub const INDIAN_SCALE_NAMES: [&str; 7] = ["Sa", "Re", "Ga", "Ma", "Pa", "Dha", "Ni"];

/// Seven-tone Indian scale ratios.
pub const INDIAN_SCALE: [Fraction; 7] = [
    Fraction::new(1, 1),
    Fraction::new(9, 8),
    Fraction::new(5, 4),
    Fraction::new(4, 3),
    Fraction::new(3, 2),
    Fraction::new(5, 3),
    Fraction::new(15, 8),
];

/// Alternate seven-tone Indian scale ratios.
pub const INDIA_SCALE_ALT: [Fraction; 7] = [
    Fraction::new(1, 1),
    Fraction::new(9, 8),
    Fraction::new(5, 4),
    Fraction::new(4, 3),
    Fraction::new(3, 2),
    Fraction::new(27, 16),
    Fraction::new(15, 8),
];

/// Twenty-two-tone Indian scale ratios.
pub const INDIAN_SCALE_22: [Fraction; 22] = [
    Fraction::new(1, 1),
    Fraction::new(256, 243),
    Fraction::new(16, 15),
    Fraction::new(10, 9),
    Fraction::new(9, 8),
    Fraction::new(32, 27),
    Fraction::new(6, 5),
    Fraction::new(5, 4),
    Fraction::new(81, 64),
    Fraction::new(4, 3),
    Fraction::new(27, 20),
    Fraction::new(45, 32),
    Fraction::new(729, 512),
    Fraction::new(3, 2),
    Fraction::new(128, 81),
    Fraction::new(8, 5),
    Fraction::new(5, 3),
    Fraction::new(27, 16),
    Fraction::new(16, 9),
    Fraction::new(9, 5),
    Fraction::new(15, 8),
    Fraction::new(243, 128),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_temperament_degree_helpers_work_without_tone_objects() {
        assert_eq!(
            TuningSystem::EqualTemperament { octave_size: 12 }.label(0),
            "CN1"
        );
        assert_eq!(
            TuningSystem::EqualTemperament { octave_size: 12 }.octave(0),
            0
        );
        assert_eq!(
            TuningSystem::EqualTemperament { octave_size: 12 }.frequency(0),
            8.1758
        );

        assert_eq!(
            TuningSystem::EqualTemperament { octave_size: 12 }.label(69),
            "A4"
        );
        assert_eq!(
            TuningSystem::EqualTemperament { octave_size: 12 }.octave(69),
            5
        );
        assert!(
            (TuningSystem::EqualTemperament { octave_size: 12 }.frequency(69) - 440.0).abs()
                < 0.0001
        );
    }

    #[test]
    fn fractional_frequency_helpers_support_pitch_space_values() {
        let equal = TuningSystem::EqualTemperament {
            octave_size: OCTAVE_SIZE,
        };
        assert!((equal.frequency_at(69.0) - A4).abs() < 0.0001);
        assert!((equal.frequency_at(60.0) - C4).abs() < 0.0001);
        assert!((TuningSystem::FiveLimit.frequency_at(64.0) - (C4 * 5.0 / 4.0)).abs() < 0.0001);
        assert!(
            (TuningSystem::PythagoreanTuning.frequency_at(67.0) - (C4 * 3.0 / 2.0)).abs() < 0.0001
        );
        assert!(TuningSystem::FiveLimit.cents_at(64.0) < -13.0);
    }

    #[test]
    fn recursive_frequency_helpers_apply_table_from_local_root() {
        let just = TuningSystem::JustIntonation;
        let fixed_g_sharp = just.frequency_at(68.0);
        let recursive_g_sharp = just.recursive_frequency_at(64.0, 68.0);

        assert!((recursive_g_sharp - (C4 * 25.0 / 16.0)).abs() < 0.0001);
        assert!((1200.0 * (recursive_g_sharp / fixed_g_sharp).log2() + 34.2827).abs() < 0.001);
        assert!((just.recursive_frequency_at(65.0, 60.0) - C4).abs() < 0.0001);
    }

    #[test]
    fn recursive_equal_temperament_matches_fixed_equal_temperament() {
        let equal = TuningSystem::EqualTemperament {
            octave_size: OCTAVE_SIZE,
        };

        for (root, index) in [(60.0, 67.0), (64.0, 68.0), (65.0, 60.0)] {
            assert!(
                (equal.recursive_frequency_at(root, index) - equal.frequency_at(index)).abs()
                    < 1e-10
            );
            assert!(equal.recursive_cents_at(root, index).abs() < 1e-10);
        }
    }

    #[test]
    fn ratio_helpers_cover_octaves() {
        let two_one: FloatType = Fraction::new(2, 1).into();
        assert_eq!(get_ratio(TuningSystem::JustIntonation, 12, None), two_one);
        assert_eq!(get_ratio(TuningSystem::JustIntonation24, 24, None), two_one);
        assert_eq!(
            get_ratio(
                TuningSystem::EqualTemperament {
                    octave_size: OCTAVE_SIZE,
                },
                12,
                None,
            ),
            two_one
        );
    }

    #[test]
    fn fraction_helpers_cover_rational_and_exponential_forms() {
        let rational = Fraction::from((3, 2));
        assert_eq!(rational.numerator(), 3);
        assert_eq!(rational.denominator(), 2);
        assert_eq!(rational.base(), 0);
        assert_eq!(rational.ratio(), 1.5);
        assert_eq!(rational.label(), "3/2");
        assert_eq!(rational.with_octaves(2), Fraction::new(12, 2));

        let exponential = Fraction::from((7, 12, 2));
        assert_eq!(exponential.label(), "2^(7/12)");
        assert_eq!(
            exponential.with_octaves(1),
            Fraction::new_with_base(19, 12, 2)
        );
        assert!((exponential.ratio() - 2.0_f64.powf(7.0 / 12.0)).abs() < 1e-12);
    }

    #[test]
    fn free_tuning_helpers_accept_size_overrides() {
        let system = TuningSystem::EqualTemperament { octave_size: 12 };
        assert_eq!(equal_temperament_12(12), Fraction::new_with_base(12, 12, 2));
        assert_eq!(
            equal_temperament_default(3),
            Fraction::new_with_base(3, OCTAVE_SIZE, 2)
        );
        assert_eq!(
            get_fraction(system, 6, Some(24)),
            Fraction::new_with_base(6, 24, 2)
        );
        assert_eq!(get_label(system, 24, Some(24)), "T0O0");
        assert!((get_frequency(system, 12, Some(24)) - CN1 * 2.0_f64.sqrt()).abs() < 1e-10);
        assert_eq!(get_cents(system, 12, Some(24)), 0.0);
    }

    #[test]
    fn current_tuning_system_variants_return_ratios() {
        assert_eq!(TuningSystem::WholeTone.ratio(6), 2.0);
        assert_eq!(TuningSystem::QuarterTone.ratio(24), 2.0);
        assert_eq!(TuningSystem::PythagoreanTuning.ratio(7), 1.5);
        assert_eq!(TuningSystem::Indian22.ratio(22), 2.0);
    }

    #[test]
    fn table_ratios_shift_by_real_octaves() {
        assert_eq!(TuningSystem::JustIntonation.ratio(19), 3.0);
        assert_eq!(TuningSystem::FortyThreeTone.ratio(68), 3.0);
        assert_eq!(TuningSystem::Indian.ratio(8), 2.25);
    }

    #[test]
    fn non_twelve_tone_systems_keep_system_octaves_and_labels() {
        assert_eq!(TuningSystem::WholeTone.label(1), "DN1");
        assert_eq!(TuningSystem::WholeTone.octave_size(), 6);
        assert!(
            (TuningSystem::WholeTone.ratio(1) - (2.0 as FloatType).powf(1.0 / 6.0)).abs() < 1e-12
        );

        assert_eq!(TuningSystem::QuarterTone.label(13), "T13ON1");
        assert_eq!(TuningSystem::QuarterTone.octave_size(), 24);
        assert!(
            (TuningSystem::QuarterTone.ratio(13) - (2.0 as FloatType).powf(13.0 / 24.0)).abs()
                < 1e-12
        );

        assert_eq!(TuningSystem::Thai.label(7), "T0O0");
        assert_eq!(TuningSystem::Thai.octave_size(), 7);
        assert_eq!(TuningSystem::Thai.ratio(7), 2.0);

        assert_eq!(TuningSystem::Indian.label(8), "Re0");
        assert_eq!(TuningSystem::Indian.octave_size(), 7);
        assert_eq!(TuningSystem::Indian.ratio(8), 2.25);

        assert_eq!(TuningSystem::FortyThreeTone.label(68), "T25O0");
        assert_eq!(TuningSystem::FortyThreeTone.octave_size(), 43);
        assert_eq!(TuningSystem::FortyThreeTone.ratio(68), 3.0);
    }

    #[test]
    fn tuning_system_display_and_parse_are_canonical() {
        let system = TuningSystem::FiveLimit;
        assert_eq!(system.id(), "FiveLimit");
        assert_eq!(system.to_string(), "FiveLimit");
        assert_eq!("FiveLimit".parse::<TuningSystem>().unwrap(), system);

        let err = "not-a-system".parse::<TuningSystem>().unwrap_err();
        assert_eq!(
            err,
            Error::TuningSystem("unknown tuning system \"not-a-system\"".to_string())
        );
    }

    #[test]
    fn tuning_system_display_names_cover_variants() {
        for system in ALL_TUNING_SYSTEMS {
            assert!(!system.id().is_empty());
            assert!(!system.display_name().is_empty());
            assert!(!system.description().is_empty());
            assert!(system.octave_size() > 0);
            assert_eq!(system.to_string(), system.id());
        }
    }

    #[test]
    fn twelve_tone_systems_keep_chromatic_ratios_ascending() {
        for system in ALL_TUNING_SYSTEMS
            .into_iter()
            .filter(|system| system.octave_size() == OCTAVE_SIZE)
        {
            let mut previous = system.ratio(0);
            for degree in 1..=OCTAVE_SIZE {
                let ratio = system.ratio(degree as usize);
                assert!(
                    ratio > previous,
                    "{} degree {degree} ratio {ratio} should be higher than {previous}",
                    system.id()
                );
                previous = ratio;
            }
        }
    }
}
