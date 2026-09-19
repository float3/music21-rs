/// Tuning systems whose frequencies depend on the harmonic context they
/// sound in, rather than on a fixed table.
pub mod adaptive;
// Finding the same tuning under two different names. Every caller is one of
// this crate's own tests -- nothing in the library asks the question, and a
// caller who wants to ask it wants a scale, not a fingerprint -- so the module
// is compiled for tests alone rather than published as API.
#[cfg(test)]
mod duplicates;
/// Equal divisions of any interval, not only of the octave.
pub mod equal;
mod generated;
/// Monzos and vals, the vectors regular temperament theory is written in.
pub mod monzo;
/// Moments of symmetry, the scales a single generator makes.
pub mod mos;
/// Runtime parsing of Scala `.scl` scale files.
pub mod scala;
#[cfg(feature = "scala-archive")]
pub mod scala_bundled;
/// Rank-2 regular temperaments, a period and a generator over a mapping.
pub mod temperament;
mod temperaments_generated;

pub use equal::{EqualDivision, TRITAVE_CENTS};

pub use generated::*;
pub use monzo::{Monzo, PRIMES, Val};
pub use mos::{Mos, MosScale, OCTAVE_CENTS, moment_of_symmetry_sizes};
use std::num::NonZeroU32;
pub use temperament::Temperament;
pub use temperaments_generated::*;

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
    TuningSystem::EQUAL_TEMPERAMENT,
    TuningSystem::CarlosHarmonic,
    TuningSystem::PythagoreanTuning,
    TuningSystem::FiveLimit,
];

/// The equal divisions of the octave that xenharmonic practice actually uses.
///
/// Any equal division is already expressible as a [`TuningSystem::Equal`];
/// this names the ones worth reaching for. 19 and 31 support meantone, 22 deliberately does not,
/// 53 gets 5-limit harmony almost exact, and 72 is the usual choice for
/// notating 11-limit music.
pub const COMMON_EQUAL_TEMPERAMENTS: [TuningSystem; 8] = [
    TuningSystem::edo(12),
    TuningSystem::edo(19),
    TuningSystem::edo(22),
    TuningSystem::edo(24),
    TuningSystem::edo(31),
    TuningSystem::edo(41),
    TuningSystem::edo(53),
    TuningSystem::edo(72),
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
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub enum AnyTuningSystem {
    /// A fixed system, which answers the same way whatever it sounds against.
    Fixed(TuningSystem),
    /// An adaptive system, whose answer depends on the context it is given.
    Adaptive(AdaptiveTuningSystem),
}

impl AnyTuningSystem {
    /// The frequency of a degree, in Hz.
    ///
    /// `context` is the frequency an adaptive system tunes against; a fixed
    /// system ignores it.
    pub fn frequency_at(self, context: FloatType, index: FloatType) -> FloatType {
        match self {
            Self::Fixed(tuning_system) => {
                let _ = context;
                tuning_system.frequency_at(index)
            }
            Self::Adaptive(adaptive_tuning_system) => {
                adaptive_tuning_system.frequency_at(context, index)
            }
        }
    }

    /// The degree's distance above the tonic, in cents.
    ///
    /// `context` is the frequency an adaptive system tunes against; a fixed
    /// system ignores it.
    pub fn cents_at(self, context: FloatType, index: FloatType) -> FloatType {
        match self {
            Self::Fixed(tuning_system) => {
                let _ = context;
                tuning_system.cents_at(index)
            }
            Self::Adaptive(adaptive_tuning_system) => {
                adaptive_tuning_system.cents_at(context, index)
            }
        }
    }

    /// Whether this system's answers depend on the context they are asked in.
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
    TuningSystem::EQUAL_TEMPERAMENT,
    TuningSystem::WHOLE_TONE,
    TuningSystem::QUARTER_TONE,
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
#[must_use]
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

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Supported tuning systems and ratio tables.
///
/// Every system repeats at a period, and a degree past the end of one is the
/// same degree a period up. The ratio tables repeat at the octave; an equal
/// division repeats wherever its period is, so Bohlen-Pierce climbs by
/// tritaves.
#[must_use]
pub enum TuningSystem {
    /// Equal steps of one period. `12edo` is ordinary equal temperament,
    /// `6edo` the whole-tone scale and `24edo` quarter tones, and the period
    /// need not be an octave: `13edt` is Bohlen-Pierce.
    Equal(EqualDivision),

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
    pub fn id(self) -> String {
        let id = match self {
            Self::Equal(division) => return division.to_string(),
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
        };
        id.to_string()
    }

    /// Returns a compact display name for this tuning system.
    pub fn display_name(self) -> String {
        let name = match self {
            Self::Equal(division) => return equal_display_name(division),
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
        };
        name.to_string()
    }

    /// Returns a short description of this tuning system.
    pub fn description(self) -> String {
        let description = match self {
            Self::Equal(division) => return equal_description(division),
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
        };
        description.to_string()
    }

    /// The frequency ratio of a degree above the system's starting pitch.
    pub fn ratio(self, index: usize) -> FloatType {
        self.ratio_at(index as FloatType)
    }

    /// The same for a fractional degree. A whole degree is the system's own
    /// step exactly; between two, the pitch is interpolated as the equal
    /// division of the period with as many steps would place it.
    pub fn ratio_at(self, index: FloatType) -> FloatType {
        assert!(index.is_finite(), "degree index must be finite");
        let steps = FloatType::from(self.degrees_per_period());
        let period = self.period_ratio();
        let Some(table) = self.ratio_table() else {
            return period.powf(index / steps);
        };
        let whole = index.floor() as IntegerType;
        let len = IntegerType::try_from(table.len()).expect("ratio table length exceeds i32 range");
        let fraction = index - FloatType::from(whole);
        table[whole.rem_euclid(len) as usize].ratio()
            * period.powi(whole.div_euclid(len))
            * period.powf(fraction / steps)
    }

    /// The degree as an exact ratio, where it is one.
    ///
    /// Every ratio table has one for each degree, and so does an equal
    /// division of a whole-number period, as a root of it: a step of `13edt`
    /// is the thirteenth root of three. A division of an irrational period,
    /// Carlos Alpha's 78 cents, has none.
    pub fn fraction(self, index: usize) -> Option<Fraction> {
        let index = UnsignedIntegerType::try_from(index).expect("tone index exceeds u32 range");
        match self {
            Self::Equal(division) => match division.period_ratio() {
                Some((base, 1)) => Some(Fraction::new_with_base(
                    index,
                    division.divisions(),
                    UnsignedIntegerType::try_from(base).ok()?,
                )),
                _ => None,
            },
            _ => {
                let table = self.ratio_table()?;
                let len = table.len() as UnsignedIntegerType;
                Some(table[(index % len) as usize].with_octaves(index / len))
            }
        }
    }

    /// A label for a degree: its name within the period and which period it
    /// is in.
    pub fn label(self, index: UnsignedIntegerType) -> String {
        let steps = self.degrees_per_period();
        degree_name_with_octave(&self.degree_label(index, steps), index / steps)
    }

    /// Which period a degree falls in, counting from nought.
    pub fn period_of(self, index: UnsignedIntegerType) -> UnsignedIntegerType {
        index / self.degrees_per_period()
    }

    /// The frequency in hertz of a degree above `C-1`.
    pub fn frequency(self, index: UnsignedIntegerType) -> FloatType {
        self.frequency_at(FloatType::from(index))
    }

    /// The frequency in hertz of a fractional degree above `C-1`.
    pub fn frequency_at(self, index: FloatType) -> FloatType {
        CN1 * self.ratio_at(index)
    }

    /// How far a degree lies from the same degree of the equal division of
    /// the same period into as many steps, in cents. Nought all the way up
    /// for an equal division, which is its own reference.
    pub fn cents(self, index: UnsignedIntegerType) -> FloatType {
        self.cents_at(FloatType::from(index))
    }

    /// The same for a fractional degree.
    pub fn cents_at(self, index: FloatType) -> FloatType {
        let equal = self
            .period_ratio()
            .powf(index / FloatType::from(self.degrees_per_period()));
        1200.0 * (self.ratio_at(index) / equal).log2()
    }

    /// How many degrees the system has before it repeats: twelve for most of
    /// the tables, and the number of divisions for an equal one.
    pub fn degrees_per_period(self) -> UnsignedIntegerType {
        match self {
            Self::Equal(division) => division.divisions(),
            _ => self
                .ratio_table()
                .map_or(OCTAVE_SIZE, |table| table.len() as UnsignedIntegerType),
        }
    }

    /// The interval the system repeats at, in cents: an octave for every
    /// table, and whatever an equal division divides.
    pub fn period_cents(self) -> FloatType {
        match self {
            Self::Equal(division) => division.period_cents(),
            _ => OCTAVE_CENTS,
        }
    }

    /// Whether the system repeats at the octave.
    pub fn repeats_at_the_octave(self) -> bool {
        match self {
            Self::Equal(division) => division.repeats_at_the_octave(),
            _ => true,
        }
    }

    fn period_ratio(self) -> FloatType {
        (2.0 as FloatType).powf(self.period_cents() / OCTAVE_CENTS)
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
            Self::Equal(_) => None,
        }
    }

    fn degree_label(self, index: UnsignedIntegerType, octave_size: UnsignedIntegerType) -> String {
        if octave_size == 0 {
            return default_degree_label(OCTAVE_SIZE, index);
        }

        let degree = index % octave_size;
        match self {
            Self::Equal(division) if division == EqualDivision::edo(nonzero(6)) => {
                WHOLE_TONE_NAMES[degree as usize].to_string()
            }
            Self::PtolemyIntenseDiatonic | Self::IndianAlt if octave_size == 7 => {
                INDIAN_SCALE_NAMES[degree as usize].to_string()
            }
            _ if !self.repeats_at_the_octave() => format!("T{degree}"),
            _ => default_degree_label(octave_size, index),
        }
    }

    /// Twelve-tone equal temperament, `12edo`.
    pub const EQUAL_TEMPERAMENT: Self = Self::edo(12);

    /// The whole-tone scale, six equal steps to the octave.
    pub const WHOLE_TONE: Self = Self::edo(6);

    /// Quarter tones, twenty-four equal steps to the octave.
    pub const QUARTER_TONE: Self = Self::edo(24);

    /// An equal division of the octave into a number of steps fixed where it
    /// is written, for naming one in a constant. It is a compile error in a
    /// constant, and a panic otherwise, to ask for no steps at all; a count
    /// read at runtime goes through [`EqualDivision::octave`], which is an
    /// error instead.
    pub const fn edo(divisions: UnsignedIntegerType) -> Self {
        Self::Equal(EqualDivision::edo(nonzero(divisions)))
    }
}

/// A step count that is not nought, for the constants above.
const fn nonzero(divisions: UnsignedIntegerType) -> NonZeroU32 {
    match NonZeroU32::new(divisions) {
        Some(divisions) => divisions,
        None => panic!("a period cannot be divided into no steps"),
    }
}

fn equal_display_name(division: EqualDivision) -> String {
    match (division.period_ratio(), division.divisions()) {
        (Some((2, 1)), 12) => "Equal temperament".to_string(),
        (Some((2, 1)), 6) => "Whole tone".to_string(),
        (Some((2, 1)), 24) => "Quarter tone".to_string(),
        (Some((2, 1)), divisions) => format!("{divisions}-tone equal temperament"),
        _ => format!("{division}"),
    }
}

fn equal_description(division: EqualDivision) -> String {
    let divisions = division.divisions();
    match division.period_ratio() {
        Some((2, 1)) => match divisions {
            12 => "Twelve equal divisions of the octave.".to_string(),
            6 => "Six equal whole-tone steps per octave.".to_string(),
            24 => "Twenty-four equal quarter-tone steps per octave.".to_string(),
            _ => format!("{divisions} equal divisions of the octave."),
        },
        Some((numerator, denominator)) => format!(
            "{divisions} equal divisions of {numerator}/{denominator}, repeating there rather than at the octave."
        ),
        None => format!(
            "{divisions} equal divisions of {:.3} cents, repeating there rather than at the octave.",
            division.period_cents()
        ),
    }
}

impl Display for TuningSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id())
    }
}

impl FromStr for TuningSystem {
    type Err = Error;

    /// Reads a system's id: a table's name, or an equal division as it
    /// writes itself (`19edo`, `13edt`, `9ed3/2`). `EqualTemperament`,
    /// `WholeTone` and `QuarterTone` read as `12edo`, `6edo` and `24edo`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "EqualTemperament" => Ok(Self::EQUAL_TEMPERAMENT),
            "WholeTone" => Ok(Self::WHOLE_TONE),
            "QuarterTone" => Ok(Self::QUARTER_TONE),
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
            _ if s.contains("ed") => s.parse().map(Self::Equal),
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

    #[test]
    fn every_system_answers_cents_and_a_fraction_for_its_degrees() {
        for system in ALL_TUNING_SYSTEMS {
            let unison = system.fraction(0).expect("every listed system is exact");
            assert!(unison.numerator == 0 || unison.numerator == unison.denominator);
            assert_eq!(system.cents(0), 0.0);
            assert!(system.cents(system.degrees_per_period()).abs() < 1e-9);
        }
    }

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
            let size = system.degrees_per_period();
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
            assert_eq!(system.degrees_per_period(), OCTAVE_SIZE, "{system:?}");
            assert!(system.ratio_table().is_some(), "{system:?} has no table");
            assert_eq!(system.ratio_table().unwrap().len(), 12, "{system:?}");
            assert!(!system.description().is_empty(), "{system:?}");
            assert_eq!(
                TuningSystem::from_str(&system.id()).unwrap(),
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
        assert_eq!(TuningSystem::edo(12).label(0), "CN1");
        assert_eq!(TuningSystem::edo(12).period_of(0), 0);
        assert!((TuningSystem::edo(12).frequency(0) - CN1).abs() < 1e-12);

        assert_eq!(TuningSystem::edo(12).label(69), "A4");
        assert_eq!(TuningSystem::edo(12).period_of(69), 5);
        assert!((TuningSystem::edo(12).frequency(69) - 440.0).abs() < 0.0001);
    }

    #[test]
    fn fractional_frequency_helpers_support_pitch_space_values() {
        let equal = TuningSystem::EQUAL_TEMPERAMENT;
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
        assert_eq!(TuningSystem::CarlosHarmonic.ratio(12), two_one);
        assert_eq!(TuningSystem::CarlosHarmonic24.ratio(24), two_one);
        assert_eq!(TuningSystem::EQUAL_TEMPERAMENT.ratio(12), two_one);
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
    fn an_equal_division_carries_its_own_step_count() {
        assert_eq!(equal_temperament_12(12), Fraction::new_with_base(12, 12, 2));
        assert_eq!(
            equal_temperament_default(3),
            Fraction::new_with_base(3, OCTAVE_SIZE, 2)
        );
        let quarter = TuningSystem::QUARTER_TONE;
        assert_eq!(quarter.fraction(6), Some(Fraction::new_with_base(6, 24, 2)));
        assert_eq!(quarter.label(24), "T0O0");
        assert!((quarter.frequency(12) - CN1 * 2.0_f64.sqrt()).abs() < 1e-10);
        assert_eq!(quarter.cents(12), 0.0);
    }

    /// Bohlen-Pierce: thirteen equal steps of a tritave, which repeats there
    /// and not at the octave, and is still exact, as roots of three.
    #[test]
    fn an_equal_division_of_the_tritave_climbs_by_tritaves() {
        let bohlen_pierce = TuningSystem::Equal(EqualDivision::tritave(13).unwrap());
        assert_eq!(bohlen_pierce.degrees_per_period(), 13);
        assert!(!bohlen_pierce.repeats_at_the_octave());
        assert!((bohlen_pierce.ratio(13) - 3.0).abs() < 1e-12);
        assert!((bohlen_pierce.ratio(26) - 9.0).abs() < 1e-12);
        assert_eq!(bohlen_pierce.period_of(26), 2);
        assert_eq!(
            bohlen_pierce.fraction(13),
            Some(Fraction::new_with_base(13, 13, 3))
        );
        assert!(
            (bohlen_pierce.fraction(4).unwrap().ratio() - bohlen_pierce.ratio(4)).abs() < 1e-12
        );
        assert_eq!(bohlen_pierce.cents(5), 0.0);
        assert_eq!(bohlen_pierce.label(13), "T0O0");
        assert_eq!(bohlen_pierce.id(), "13edt");
        assert_eq!("13edt".parse::<TuningSystem>().unwrap(), bohlen_pierce);
    }

    /// A period with no whole-number ratio has no exact degrees, and still
    /// sounds and names itself.
    #[test]
    fn an_equal_division_of_an_irrational_period_has_no_fractions() {
        let alpha = TuningSystem::Equal(EqualDivision::new(18, 1404.0).unwrap());
        assert_eq!(alpha.fraction(1), None);
        assert!((1200.0 * alpha.ratio(1).log2() - 78.0).abs() < 1e-9);
        assert!((1200.0 * alpha.ratio(18).log2() - 1404.0).abs() < 1e-9);
        assert_eq!(alpha.id().parse::<TuningSystem>().unwrap(), alpha);
    }

    /// The names the equal systems had as variants still read.
    #[test]
    fn the_old_equal_names_still_parse() {
        assert_eq!(
            "EqualTemperament".parse::<TuningSystem>().unwrap(),
            TuningSystem::EQUAL_TEMPERAMENT
        );
        assert_eq!(
            "WholeTone".parse::<TuningSystem>().unwrap(),
            TuningSystem::WHOLE_TONE
        );
        assert_eq!(
            "QuarterTone".parse::<TuningSystem>().unwrap(),
            TuningSystem::QUARTER_TONE
        );
        assert_eq!(
            "19edo".parse::<TuningSystem>().unwrap(),
            TuningSystem::edo(19)
        );
        assert_eq!(TuningSystem::EQUAL_TEMPERAMENT.id(), "12edo");
        assert_eq!(
            TuningSystem::EQUAL_TEMPERAMENT.display_name(),
            "Equal temperament"
        );
        assert_eq!(
            TuningSystem::edo(19).display_name(),
            "19-tone equal temperament"
        );
    }

    #[test]
    fn current_tuning_system_variants_return_ratios() {
        assert_eq!(TuningSystem::WHOLE_TONE.ratio(6), 2.0);
        assert_eq!(TuningSystem::QUARTER_TONE.ratio(24), 2.0);
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
        assert_eq!(TuningSystem::WHOLE_TONE.label(1), "DN1");
        assert_eq!(TuningSystem::WHOLE_TONE.degrees_per_period(), 6);
        assert!(
            (TuningSystem::WHOLE_TONE.ratio(1) - (2.0 as FloatType).powf(1.0 / 6.0)).abs() < 1e-12
        );

        assert_eq!(TuningSystem::QUARTER_TONE.label(13), "T13ON1");
        assert_eq!(TuningSystem::QUARTER_TONE.degrees_per_period(), 24);
        assert!(
            (TuningSystem::QUARTER_TONE.ratio(13) - (2.0 as FloatType).powf(13.0 / 24.0)).abs()
                < 1e-12
        );

        assert_eq!(TuningSystem::Thai.label(7), "T0O0");
        assert_eq!(TuningSystem::Thai.degrees_per_period(), 7);
        assert_eq!(TuningSystem::Thai.ratio(7), 2.0);

        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.label(8), "Re0");
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.degrees_per_period(), 7);
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.ratio(8), 2.25);

        assert_eq!(TuningSystem::FortyThreeTone.label(68), "T25O0");
        assert_eq!(TuningSystem::FortyThreeTone.degrees_per_period(), 43);
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
            assert!(system.degrees_per_period() > 0);
            assert_eq!(system.to_string(), system.id());
        }
    }

    #[test]
    fn twelve_tone_systems_keep_chromatic_ratios_ascending() {
        for system in ALL_TUNING_SYSTEMS
            .into_iter()
            .filter(|system| system.degrees_per_period() == OCTAVE_SIZE)
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
            let octave_size = system.degrees_per_period();
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
