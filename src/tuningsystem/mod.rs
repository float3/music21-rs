pub mod adaptive;
mod generated;
/// Runtime parsing of Scala `.scl` scale files.
pub mod scala;
#[cfg(feature = "scala-archive")]
pub mod scala_bundled;

pub use generated::*;

use crate::defaults::{FloatType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};
use crate::tuningsystem::adaptive::AdaptiveTuningSystem;

use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Default octave size for twelve-tone systems.
pub const OCTAVE_SIZE: UnsignedIntegerType = 12;

/// Frequency of middle C in hertz.
pub const C4: FloatType = 261.625_565_300_598_6;
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

/// Degree labels for a twelve-tone chromatic octave using sharps.
pub const TWELVE_TONE_NAMES_SHARP: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Degree labels for a twelve-tone chromatic octave using flats.
pub const TWELVE_TONE_NAMES_FLAT: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

/// Degree labels for a whole-tone octave.
pub const WHOLE_TONE_NAMES: [&str; 6] = ["C", "D", "E", "F#/Gb", "G#/Ab", "A#/Bb"];

/// The common twelve-tone tuning systems useful for comparing pitch frequencies.
pub const COMMON_TWELVE_TONE_TUNING_SYSTEMS: [TuningSystem; 4] = [
    TuningSystem::EqualTemperament {
        octave_size: OCTAVE_SIZE,
    },
    TuningSystem::CarlosHarmonic,
    TuningSystem::PythagoreanTuning,
    TuningSystem::FiveLimit,
];

/// The equal divisions of the octave that xenharmonic practice actually uses.
///
/// Any EDO is already expressible as
/// `TuningSystem::EqualTemperament { octave_size: n }`; this names the ones
/// worth reaching for. 19 and 31 support meantone, 22 deliberately does not,
/// 53 gets 5-limit harmony almost exact, and 72 is the usual choice for
/// notating 11-limit music.
pub const COMMON_EQUAL_TEMPERAMENTS: [TuningSystem; 8] = [
    TuningSystem::EqualTemperament { octave_size: 12 },
    TuningSystem::EqualTemperament { octave_size: 19 },
    TuningSystem::EqualTemperament { octave_size: 22 },
    TuningSystem::EqualTemperament { octave_size: 24 },
    TuningSystem::EqualTemperament { octave_size: 31 },
    TuningSystem::EqualTemperament { octave_size: 41 },
    TuningSystem::EqualTemperament { octave_size: 53 },
    TuningSystem::EqualTemperament { octave_size: 72 },
];

/// Historical keyboard temperaments, oldest first.
///
/// These are the well temperaments and meantone tunings that Western keyboard
/// music was actually written for, transcribed from the Scala archive in the
/// `music21` reference submodule. All are twelve-tone.
pub const HISTORICAL_TEMPERAMENTS: [TuningSystem; 14] = [
    TuningSystem::QuarterCommaMeantone,
    TuningSystem::WerckmeisterIII,
    TuningSystem::Rameau,
    TuningSystem::KirnbergerIII,
    TuningSystem::Vallotti,
    TuningSystem::YoungII,
    TuningSystem::ThirdCommaMeantone,
    TuningSystem::SixthCommaMeantone,
    TuningSystem::WerckmeisterIV,
    TuningSystem::WerckmeisterV,
    TuningSystem::KirnbergerI,
    TuningSystem::NeidhardtI,
    TuningSystem::Silbermann,
    TuningSystem::LehmanBach,
];

/// Either a normal tuning system or a context-sensitive adaptive tuning system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AnyTuningSystem {
    Fixed(TuningSystem),
    Adaptive(AdaptiveTuningSystem),
}

impl AnyTuningSystem {
    pub fn frequency_at(
        self,
        context: FloatType,
        index: FloatType,
        size: Option<UnsignedIntegerType>,
    ) -> FloatType {
        match self {
            Self::Fixed(tuning_system) => {
                let _ = context;
                get_frequency_at(tuning_system, index, size)
            }
            Self::Adaptive(adaptive_tuning_system) => {
                adaptive_tuning_system.frequency_at(context, index, size)
            }
        }
    }

    pub fn cents_at(
        self,
        context: FloatType,
        index: FloatType,
        size: Option<UnsignedIntegerType>,
    ) -> FloatType {
        match self {
            Self::Fixed(tuning_system) => {
                let _ = context;
                tuning_system.cents_at(index)
            }
            Self::Adaptive(adaptive_tuning_system) => {
                adaptive_tuning_system.cents_at(context, index, size)
            }
        }
    }

    pub fn is_adaptive(self) -> bool {
        matches!(self, Self::Adaptive(_))
    }
}

impl From<TuningSystem> for AnyTuningSystem {
    fn from(tuning_system: TuningSystem) -> Self {
        Self::Fixed(tuning_system)
    }
}

impl From<AdaptiveTuningSystem> for AnyTuningSystem {
    fn from(adaptive_tuning_system: AdaptiveTuningSystem) -> Self {
        Self::Adaptive(adaptive_tuning_system)
    }
}

/// All built-in tuning systems in canonical display order.
pub const ALL_TUNING_SYSTEMS: [TuningSystem; 28] = [
    TuningSystem::EqualTemperament {
        octave_size: OCTAVE_SIZE,
    },
    TuningSystem::WholeTone,
    TuningSystem::QuarterTone,
    TuningSystem::CarlosHarmonic,
    TuningSystem::CarlosHarmonic24,
    TuningSystem::PythagoreanTuning,
    TuningSystem::FiveLimit,
    TuningSystem::ElevenLimit,
    TuningSystem::FortyThreeTone,
    TuningSystem::Javanese,
    TuningSystem::Thai,
    TuningSystem::PtolemyIntenseDiatonic,
    TuningSystem::IndianAlt,
    TuningSystem::Indian22,
    TuningSystem::QuarterCommaMeantone,
    TuningSystem::WerckmeisterIII,
    TuningSystem::Rameau,
    TuningSystem::KirnbergerIII,
    TuningSystem::Vallotti,
    TuningSystem::YoungII,
    TuningSystem::ThirdCommaMeantone,
    TuningSystem::SixthCommaMeantone,
    TuningSystem::WerckmeisterIV,
    TuningSystem::WerckmeisterV,
    TuningSystem::KirnbergerI,
    TuningSystem::NeidhardtI,
    TuningSystem::Silbermann,
    TuningSystem::LehmanBach,
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

    /// Twelve-tone harmonic-series scale (Wendy Carlos's Harmonic).
    ///
    /// Not just intonation, despite its former name: the degrees are the
    /// harmonic series 16:17:18:19:20:21:22:24:26:27:28:30, which reaches the
    /// 19-limit. [`TuningSystem::FiveLimit`] is the classical 12-tone JI scale.
    CarlosHarmonic,
    /// Twenty-four-tone harmonic-series scale.
    CarlosHarmonic24,
    /// Twelve-tone Pythagorean tuning table.
    PythagoreanTuning,

    /// Twelve-tone five-limit table.
    FiveLimit,
    /// Twenty-nine-tone eleven-limit table.
    ElevenLimit,

    /// Forty-three-tone ratio table.
    FortyThreeTone,

    // Ethnic scales.
    /// Five-tone Javanese equal-temperament approximation.
    Javanese,
    /// Seven-tone Thai equal-temperament approximation.
    Thai,
    /// Ptolemy's intense diatonic, also Zarlino's just major scale.
    ///
    /// Greek and Renaissance European, despite once being filed here as an
    /// Indian scale — the archive names it `ptolemy.scl`.
    PtolemyIntenseDiatonic,
    /// Alternate seven-tone PtolemyIntenseDiatonic scale table.
    IndianAlt,
    /// Twenty-two-tone PtolemyIntenseDiatonic scale table.
    Indian22,

    // Historical keyboard temperaments, transcribed from the Scala archive.
    /// Twelve-tone quarter-comma meantone temperament (Aaron, 1523).
    QuarterCommaMeantone,
    /// Twelve-tone Werckmeister III well temperament (1681).
    WerckmeisterIII,
    /// Twelve-tone Rameau modified meantone temperament (1725).
    Rameau,
    /// Twelve-tone Kirnberger III well temperament (1744).
    KirnbergerIII,
    /// Twelve-tone Vallotti well temperament (c. 1754).
    Vallotti,
    /// Twelve-tone Thomas Young well temperament no. 2 (1799).
    YoungII,
    /// A twelve-tone third-comma meantone temperament (Salinas, 1577).
    ThirdCommaMeantone,
    /// A twelve-tone sixth-comma meantone temperament (Salinas, 1577).
    SixthCommaMeantone,
    /// A twelve-tone Werckmeister IV well temperament (1681).
    WerckmeisterIV,
    /// A twelve-tone Werckmeister V well temperament (1681).
    WerckmeisterV,
    /// A twelve-tone Kirnberger I well temperament (1766).
    KirnbergerI,
    /// A twelve-tone Neidhardt I well temperament (1724).
    NeidhardtI,
    /// A twelve-tone Gottfried Silbermann temperament no. 1 (c. 1730).
    Silbermann,
    /// A twelve-tone Lehman-Bach temperament (2005).
    LehmanBach,
}

impl TuningSystem {
    /// Returns the canonical identifier used by [`FromStr`].
    pub fn id(self) -> &'static str {
        match self {
            Self::EqualTemperament { .. } => "EqualTemperament",
            Self::WholeTone => "WholeTone",
            Self::QuarterTone => "QuarterTone",
            Self::CarlosHarmonic => "CarlosHarmonic",
            Self::CarlosHarmonic24 => "CarlosHarmonic24",
            Self::PythagoreanTuning => "PythagoreanTuning",
            Self::FiveLimit => "FiveLimit",
            Self::ElevenLimit => "ElevenLimit",
            Self::FortyThreeTone => "FortyThreeTone",
            Self::Javanese => "Javanese",
            Self::Thai => "Thai",
            Self::PtolemyIntenseDiatonic => "PtolemyIntenseDiatonic",
            Self::IndianAlt => "IndianAlt",
            Self::Indian22 => "Indian22",
            Self::QuarterCommaMeantone => "QuarterCommaMeantone",
            Self::WerckmeisterIII => "WerckmeisterIII",
            Self::Rameau => "Rameau",
            Self::KirnbergerIII => "KirnbergerIII",
            Self::Vallotti => "Vallotti",
            Self::YoungII => "YoungII",
            Self::ThirdCommaMeantone => "ThirdCommaMeantone",
            Self::SixthCommaMeantone => "SixthCommaMeantone",
            Self::WerckmeisterIV => "WerckmeisterIV",
            Self::WerckmeisterV => "WerckmeisterV",
            Self::KirnbergerI => "KirnbergerI",
            Self::NeidhardtI => "NeidhardtI",
            Self::Silbermann => "Silbermann",
            Self::LehmanBach => "LehmanBach",
        }
    }

    /// Returns a compact display name for this tuning system.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::EqualTemperament { .. } => "Equal temperament",
            Self::WholeTone => "Whole tone",
            Self::QuarterTone => "Quarter tone",
            Self::CarlosHarmonic => "Carlos Harmonic",
            Self::CarlosHarmonic24 => "Carlos Harmonic 24",
            Self::PythagoreanTuning => "Pythagorean",
            Self::FiveLimit => "Five-limit",
            Self::ElevenLimit => "Partch 11-limit diamond",
            Self::FortyThreeTone => "Partch 43-tone",
            Self::Javanese => "Javanese",
            Self::Thai => "Thai",
            Self::PtolemyIntenseDiatonic => "Ptolemy intense diatonic",
            Self::IndianAlt => "Indian Sa-grama",
            Self::Indian22 => "Indian shruti",
            Self::QuarterCommaMeantone => "Quarter-comma meantone",
            Self::WerckmeisterIII => "Werckmeister III",
            Self::Rameau => "Rameau",
            Self::KirnbergerIII => "Kirnberger III",
            Self::Vallotti => "Vallotti",
            Self::YoungII => "Young II",
            Self::ThirdCommaMeantone => "Third-comma meantone",
            Self::SixthCommaMeantone => "Sixth-comma meantone",
            Self::WerckmeisterIV => "Werckmeister IV",
            Self::WerckmeisterV => "Werckmeister V",
            Self::KirnbergerI => "Kirnberger I",
            Self::NeidhardtI => "Neidhardt I",
            Self::Silbermann => "Silbermann",
            Self::LehmanBach => "Lehman-Bach",
        }
    }

    /// Returns a short description of this tuning system.
    pub fn description(self) -> &'static str {
        match self {
            Self::EqualTemperament { .. } => "Twelve equal divisions of the octave.",
            Self::WholeTone => "Six equal whole-tone steps per octave.",
            Self::QuarterTone => "Twenty-four equal quarter-tone steps per octave.",
            Self::CarlosHarmonic => "A twelve-tone harmonic-series scale, reaching the 19-limit.",
            Self::CarlosHarmonic24 => "A twenty-four-tone harmonic-series scale.",
            Self::PythagoreanTuning => "A twelve-tone tuning table built from pure fifths.",
            Self::FiveLimit => "A twelve-tone table using five-limit just ratios.",
            Self::ElevenLimit => "Harry Partch's twenty-nine-tone 11-limit tonality diamond.",
            Self::FortyThreeTone => "Harry Partch's forty-three-tone pure scale.",
            Self::Javanese => "A five-tone Javanese equal-temperament approximation.",
            Self::Thai => "A seven-tone Thai equal-temperament approximation.",
            Self::PtolemyIntenseDiatonic => {
                "Ptolemy's intense diatonic, also Zarlino's just major scale."
            }
            Self::IndianAlt => "The Indian Sa-grama mode, the inverse of Didymus' diatonic.",
            Self::Indian22 => "The twenty-two-shruti Indian scale.",
            Self::QuarterCommaMeantone => {
                "A twelve-tone quarter-comma meantone temperament (Aaron, 1523)."
            }
            Self::WerckmeisterIII => "A twelve-tone Werckmeister III well temperament (1681).",
            Self::Rameau => "A twelve-tone Rameau modified meantone temperament (1725).",
            Self::KirnbergerIII => "A twelve-tone Kirnberger III well temperament (1744).",
            Self::Vallotti => "A twelve-tone Vallotti well temperament (c. 1754).",
            Self::YoungII => "A twelve-tone Thomas Young well temperament no. 2 (1799).",
            Self::ThirdCommaMeantone => {
                "A twelve-tone third-comma meantone temperament (Salinas, 1577)."
            }
            Self::SixthCommaMeantone => {
                "A twelve-tone sixth-comma meantone temperament (Salinas, 1577)."
            }
            Self::WerckmeisterIV => "A twelve-tone Werckmeister IV well temperament (1681).",
            Self::WerckmeisterV => "A twelve-tone Werckmeister V well temperament (1681).",
            Self::KirnbergerI => "A twelve-tone Kirnberger I well temperament (1766).",
            Self::NeidhardtI => "A twelve-tone Neidhardt I well temperament (1724).",
            Self::Silbermann => "A twelve-tone Gottfried Silbermann temperament no. 1 (c. 1730).",
            Self::LehmanBach => "A twelve-tone Lehman-Bach temperament (2005).",
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

    /// Returns cents offset from equal temperament for a degree index.
    pub fn cents(self, index: UnsignedIntegerType) -> FloatType {
        get_cents(self, index, None)
    }

    /// Returns cents offset from equal temperament for a fractional degree index.
    pub fn cents_at(self, index: FloatType) -> FloatType {
        get_cents_at(self, index, None)
    }

    /// Returns the number of degrees in one octave for this tuning system.
    pub fn octave_size(self) -> UnsignedIntegerType {
        match self {
            Self::EqualTemperament { octave_size } => octave_size,
            Self::WholeTone => 6,
            Self::QuarterTone | Self::CarlosHarmonic24 => 24,
            Self::FortyThreeTone => 43,
            Self::ElevenLimit => 29,
            Self::Javanese => 5,
            Self::Thai | Self::PtolemyIntenseDiatonic | Self::IndianAlt => 7,
            Self::Indian22 => 22,
            Self::QuarterCommaMeantone
            | Self::WerckmeisterIII
            | Self::Rameau
            | Self::KirnbergerIII
            | Self::Vallotti
            | Self::YoungII
            | Self::ThirdCommaMeantone
            | Self::SixthCommaMeantone
            | Self::WerckmeisterIV
            | Self::WerckmeisterV
            | Self::KirnbergerI
            | Self::NeidhardtI
            | Self::Silbermann
            | Self::LehmanBach => OCTAVE_SIZE,
            Self::CarlosHarmonic | Self::PythagoreanTuning | Self::FiveLimit => OCTAVE_SIZE,
        }
    }

    fn ratio_table(self) -> Option<&'static [Fraction]> {
        match self {
            Self::CarlosHarmonic => Some(&CARLOS_HARMONIC),
            Self::CarlosHarmonic24 => Some(&CARLOS_HARMONIC_24),
            Self::PythagoreanTuning => Some(&PYTHAGOREAN_TUNING),
            Self::FiveLimit => Some(&FIVE_LIMIT),
            Self::ElevenLimit => Some(&ELEVEN_LIMIT),
            Self::FortyThreeTone => Some(&FORTY_THREE_TONE),
            Self::Javanese => Some(&JAVANESE),
            Self::Thai => Some(&THAI),
            Self::PtolemyIntenseDiatonic => Some(&PTOLEMY_INTENSE_DIATONIC),
            Self::IndianAlt => Some(&INDIA_SCALE_ALT),
            Self::Indian22 => Some(&INDIAN_SCALE_22),
            Self::QuarterCommaMeantone => Some(&QUARTER_COMMA_MEANTONE),
            Self::WerckmeisterIII => Some(&WERCKMEISTER_III),
            Self::Rameau => Some(&RAMEAU),
            Self::KirnbergerIII => Some(&KIRNBERGER_III),
            Self::Vallotti => Some(&VALLOTTI),
            Self::YoungII => Some(&YOUNG_II),
            Self::ThirdCommaMeantone => Some(&THIRD_COMMA_MEANTONE),
            Self::SixthCommaMeantone => Some(&SIXTH_COMMA_MEANTONE),
            Self::WerckmeisterIV => Some(&WERCKMEISTER_IV),
            Self::WerckmeisterV => Some(&WERCKMEISTER_V),
            Self::KirnbergerI => Some(&KIRNBERGER_I),
            Self::NeidhardtI => Some(&NEIDHARDT_I),
            Self::Silbermann => Some(&SILBERMANN),
            Self::LehmanBach => Some(&LEHMAN_BACH),
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
            Self::PtolemyIntenseDiatonic | Self::IndianAlt if octave_size == 7 => {
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
            "CarlosHarmonic" => Ok(Self::CarlosHarmonic),
            "CarlosHarmonic24" => Ok(Self::CarlosHarmonic24),
            "PythagoreanTuning" => Ok(Self::PythagoreanTuning),
            "FiveLimit" => Ok(Self::FiveLimit),
            "ElevenLimit" => Ok(Self::ElevenLimit),
            "FortyThreeTone" => Ok(Self::FortyThreeTone),
            "Javanese" => Ok(Self::Javanese),
            "Thai" => Ok(Self::Thai),
            "PtolemyIntenseDiatonic" => Ok(Self::PtolemyIntenseDiatonic),
            "IndianAlt" => Ok(Self::IndianAlt),
            "Indian22" => Ok(Self::Indian22),
            "QuarterCommaMeantone" => Ok(Self::QuarterCommaMeantone),
            "WerckmeisterIII" => Ok(Self::WerckmeisterIII),
            "Rameau" => Ok(Self::Rameau),
            "KirnbergerIII" => Ok(Self::KirnbergerIII),
            "Vallotti" => Ok(Self::Vallotti),
            "YoungII" => Ok(Self::YoungII),
            "ThirdCommaMeantone" => Ok(Self::ThirdCommaMeantone),
            "SixthCommaMeantone" => Ok(Self::SixthCommaMeantone),
            "WerckmeisterIV" => Ok(Self::WerckmeisterIV),
            "WerckmeisterV" => Ok(Self::WerckmeisterV),
            "KirnbergerI" => Ok(Self::KirnbergerI),
            "NeidhardtI" => Ok(Self::NeidhardtI),
            "Silbermann" => Ok(Self::Silbermann),
            "LehmanBach" => Ok(Self::LehmanBach),
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

/// Degree labels for the seven-tone PtolemyIntenseDiatonic scale.
pub const INDIAN_SCALE_NAMES: [&str; 7] = ["Sa", "Re", "Ga", "Ma", "Pa", "Dha", "Ni"];

#[cfg(test)]
mod tests {
    use super::*;

    /// Cents above the tonic for a degree of a twelve-tone table.
    fn cents_at_degree(system: TuningSystem, degree: usize) -> FloatType {
        1200.0 * system.ratio(degree).log2()
    }

    #[test]
    fn meantone_fifths_match_their_comma_fractions() {
        // A 1/n-comma meantone narrows the pure fifth by 1/n of the syntonic
        // comma (21.506 cents). Checking the arithmetic rather than a quoted
        // table catches a table pointed at the wrong Scala file.
        const PURE_FIFTH: FloatType = 701.955;
        const SYNTONIC_COMMA: FloatType = 21.506;
        for (system, divisor) in [
            (TuningSystem::ThirdCommaMeantone, 3.0),
            (TuningSystem::QuarterCommaMeantone, 4.0),
            (TuningSystem::SixthCommaMeantone, 6.0),
        ] {
            let expected = PURE_FIFTH - SYNTONIC_COMMA / divisor;
            let actual = cents_at_degree(system, 7);
            assert!(
                (actual - expected).abs() < 0.01,
                "{} fifth: expected {expected:.3}, got {actual:.3}",
                system.display_name()
            );
        }
    }

    #[test]
    fn kirnberger_i_keeps_its_pure_fifth_and_third() {
        // Kirnberger I is the outlier of the well temperaments: it leaves the
        // C-G fifth pure rather than tempering it.
        assert!((cents_at_degree(TuningSystem::KirnbergerI, 7) - 701.955).abs() < 0.01);
        assert!((cents_at_degree(TuningSystem::KirnbergerI, 4) - 386.314).abs() < 0.01);
    }

    #[test]
    fn common_equal_temperaments_divide_the_octave_evenly() {
        for system in COMMON_EQUAL_TEMPERAMENTS {
            let size = system.octave_size();
            let step = 1200.0 / FloatType::from(size);
            for degree in 0..size as usize {
                let expected = step * degree as FloatType;
                let actual = cents_at_degree(system, degree);
                assert!(
                    (actual - expected).abs() < 0.001,
                    "{size}-EDO degree {degree}: expected {expected:.3}, got {actual:.3}"
                );
            }
        }
    }

    #[test]
    fn historical_temperaments_match_their_published_values() {
        // Major third and perfect fifth above C, in cents, as given in the
        // standard literature. Just values for reference: M3 386.314,
        // P5 701.955; equal temperament: 400.000 and 700.000.
        let cases = [
            (TuningSystem::QuarterCommaMeantone, 386.314, 696.578),
            (TuningSystem::WerckmeisterIII, 390.225, 696.090),
            (TuningSystem::Rameau, 386.314, 696.578),
            (TuningSystem::KirnbergerIII, 386.314, 696.578),
            (TuningSystem::Vallotti, 392.180, 698.045),
            (TuningSystem::YoungII, 392.180, 698.045),
        ];

        for (system, major_third, fifth) in cases {
            let actual_third = cents_at_degree(system, 4);
            let actual_fifth = cents_at_degree(system, 7);
            assert!(
                (actual_third - major_third).abs() < 0.001,
                "{} major third: expected {major_third}, got {actual_third}",
                system.display_name()
            );
            assert!(
                (actual_fifth - fifth).abs() < 0.001,
                "{} fifth: expected {fifth}, got {actual_fifth}",
                system.display_name()
            );
        }
    }

    #[test]
    fn quarter_comma_meantone_and_kirnberger_have_a_pure_major_third() {
        // The defining property of both: C-E is the just 5/4 (386.314 cents).
        //
        // Not exactly 5/4, though. The Scala archive writes these degrees in
        // cents to five decimal places rather than as an exact ratio, so the
        // table carries 386.31371 cents. That is 4e-6 cents shy of just - some
        // nine orders of magnitude below anything audible - but it is not the
        // rational 5/4, and a test asserting exact equality would be asserting
        // something the source data does not contain.
        for system in [
            TuningSystem::QuarterCommaMeantone,
            TuningSystem::KirnbergerIII,
        ] {
            let cents = cents_at_degree(system, 4);
            assert!(
                (cents - 386.313_714).abs() < 0.001,
                "{} major third should be the just 386.314 cents, got {cents}",
                system.display_name()
            );
        }
    }

    #[test]
    fn every_historical_temperament_is_wired_up() {
        for system in HISTORICAL_TEMPERAMENTS {
            assert_eq!(system.octave_size(), OCTAVE_SIZE, "{system:?}");
            assert!(system.ratio_table().is_some(), "{system:?} has no table");
            assert_eq!(system.ratio_table().unwrap().len(), 12, "{system:?}");
            assert!(!system.description().is_empty(), "{system:?}");
            assert_eq!(
                TuningSystem::from_str(system.id()).unwrap(),
                system,
                "{system:?} does not round-trip through its id"
            );
            assert!(
                ALL_TUNING_SYSTEMS.contains(&system),
                "{system:?} missing from ALL_TUNING_SYSTEMS"
            );
        }
    }

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
        assert!(
            (TuningSystem::EqualTemperament { octave_size: 12 }.frequency(0) - CN1).abs() < 1e-12
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
    fn ratio_helpers_cover_octaves() {
        let two_one: FloatType = Fraction::new(2, 1).into();
        assert_eq!(get_ratio(TuningSystem::CarlosHarmonic, 12, None), two_one);
        assert_eq!(get_ratio(TuningSystem::CarlosHarmonic24, 24, None), two_one);
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
        assert_eq!(TuningSystem::CarlosHarmonic.ratio(19), 3.0);
        assert_eq!(TuningSystem::FortyThreeTone.ratio(68), 3.0);
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.ratio(8), 2.25);
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

        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.label(8), "Re0");
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.octave_size(), 7);
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.ratio(8), 2.25);

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

    #[test]
    fn all_ratio_tables_keep_degrees_strictly_ascending_within_the_octave() {
        for system in ALL_TUNING_SYSTEMS {
            let octave_size = system.octave_size();
            let mut previous = system.ratio(0);
            for degree in 1..=octave_size {
                let ratio = system.ratio(degree as usize);
                assert!(
                    ratio > previous,
                    "{} degree {degree} ratio {ratio} should be higher than {previous} \
                     (a table entry is likely mistranscribed)",
                    system.id()
                );
                previous = ratio;
            }
        }
    }
}
