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
/// Tuning spelled pitches: a way of sizing intervals, and where it starts.
pub mod tuning;

pub use equal::{EqualDivision, TRITAVE_CENTS};

pub use generated::*;
pub use monzo::{Monzo, PRIMES, Val};
pub use mos::{Mos, MosScale, OCTAVE_CENTS, moment_of_symmetry_sizes};
use std::num::NonZeroU32;
pub use temperament::Temperament;
pub use temperaments_generated::*;
pub use tuning::{Just, Reference, Tuning};

use crate::defaults::{FloatType, FractionType, IntegerType, UnsignedIntegerType};
use crate::error::{Error, Result};
use crate::pitch::Pitch;
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

/// An exact interval as a tuning table writes one: a ratio of whole numbers,
/// or a root of one.
///
/// A root is how an equal division is exact: a step of twelve-tone equal
/// temperament is `2^(1/12)`, and one of Bohlen-Pierce `3^(1/13)`. Neither is
/// a fraction, which is why this is not a [`FractionType`]; a rational one
/// converts to that with [`Ratio::to_fraction`], and to a [`Monzo`] with
/// [`Ratio::monzo`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub enum Ratio {
    /// `numerator / denominator`.
    Rational {
        /// The number on top.
        numerator: UnsignedIntegerType,
        /// The number below.
        denominator: UnsignedIntegerType,
    },
    /// `base^(numerator / denominator)`.
    Root {
        /// The number a root is taken of.
        base: UnsignedIntegerType,
        /// The power it is raised to.
        numerator: UnsignedIntegerType,
        /// Which root is taken.
        denominator: UnsignedIntegerType,
    },
}

impl Ratio {
    /// `numerator / denominator`.
    pub const fn new(numerator: UnsignedIntegerType, denominator: UnsignedIntegerType) -> Self {
        Self::Rational {
            numerator,
            denominator,
        }
    }

    /// `base^(numerator / denominator)`.
    pub const fn root(
        base: UnsignedIntegerType,
        numerator: UnsignedIntegerType,
        denominator: UnsignedIntegerType,
    ) -> Self {
        Self::Root {
            base,
            numerator,
            denominator,
        }
    }

    /// How many times wider than a unison the interval is.
    #[must_use]
    pub fn value(self) -> FloatType {
        match self {
            Self::Rational {
                numerator,
                denominator,
            } => FloatType::from(numerator) / FloatType::from(denominator),
            Self::Root {
                base,
                numerator,
                denominator,
            } => FloatType::from(base)
                .powf(FloatType::from(numerator) / FloatType::from(denominator)),
        }
    }

    /// How wide the interval is, in cents.
    #[must_use]
    pub fn cents(self) -> FloatType {
        OCTAVE_CENTS * self.value().log2()
    }

    /// The ratio as a [`FractionType`]: `None` for a root, and for a ratio
    /// too wide for one.
    #[must_use]
    pub fn to_fraction(self) -> Option<FractionType> {
        match self {
            Self::Rational {
                numerator,
                denominator,
            } => Some(FractionType::new(
                IntegerType::try_from(numerator).ok()?,
                IntegerType::try_from(denominator).ok()?,
            )),
            Self::Root { .. } => None,
        }
    }

    /// The ratio's prime exponents. Errors on a root, whose exponents are not
    /// whole, and on a ratio with a prime past the ones [`Monzo`] carries.
    pub fn monzo(self) -> Result<Monzo> {
        let fraction = self.to_fraction().ok_or_else(|| {
            Error::TuningSystem(format!(
                "{self} is not a ratio of two i32s, so has no monzo"
            ))
        })?;
        Monzo::from_fraction(fraction)
    }

    /// The interval raised by `octaves` periods: octaves for a ratio, and
    /// whole powers of its base for a root.
    pub fn with_octaves(self, octaves: UnsignedIntegerType) -> Self {
        match self {
            Self::Rational {
                numerator,
                denominator,
            } => {
                let multiplier = (2 as UnsignedIntegerType)
                    .checked_pow(octaves)
                    .expect("octave multiplier exceeds u32 range");
                Self::new(
                    numerator
                        .checked_mul(multiplier)
                        .expect("ratio numerator exceeds u32 range"),
                    denominator,
                )
            }
            Self::Root {
                base,
                numerator,
                denominator,
            } => Self::root(
                base,
                numerator
                    .checked_add(
                        denominator
                            .checked_mul(octaves)
                            .expect("root octave offset exceeds u32 range"),
                    )
                    .expect("root numerator exceeds u32 range"),
                denominator,
            ),
        }
    }
}

impl From<Ratio> for FloatType {
    fn from(ratio: Ratio) -> Self {
        ratio.value()
    }
}

impl Display for Ratio {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Rational {
                numerator,
                denominator: 1,
            } => write!(f, "{numerator}"),
            Self::Rational {
                numerator,
                denominator,
            } => write!(f, "{numerator}/{denominator}"),
            Self::Root { numerator: 0, .. } => write!(f, "1"),
            Self::Root {
                base,
                numerator,
                denominator,
            } => write!(f, "{base}^({numerator}/{denominator})"),
        }
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
        table[whole.rem_euclid(len) as usize].value()
            * period.powi(whole.div_euclid(len))
            * period.powf(fraction / steps)
    }

    /// The degree as an exact ratio, where it is one.
    ///
    /// Every ratio table has one for each degree, and so does an equal
    /// division of a whole-number period, as a root of it: a step of `13edt`
    /// is the thirteenth root of three. A division of an irrational period,
    /// Carlos Alpha's 78 cents, has none.
    pub fn exact_ratio(self, index: usize) -> Option<Ratio> {
        let index = UnsignedIntegerType::try_from(index).expect("tone index exceeds u32 range");
        match self {
            Self::Equal(division) => match division.period_ratio() {
                Some((base, 1)) => Some(Ratio::root(
                    UnsignedIntegerType::try_from(base).ok()?,
                    index,
                    division.divisions(),
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

    /// A name for a degree, counted from `C-1`.
    ///
    /// A system of twelve degrees to the octave names each degree for its key,
    /// `C♯/D♭4`, however it is tuned, and so does any other system on a degree
    /// that lands on a key. Every other degree is the pitch it sounds, as
    /// [`TuningSystem::pitch_at`] spells it, with its microtone: `G0(+2c)` is
    /// the thirteenth step of Bohlen-Pierce. Ptolemy's intense diatonic and
    /// the Sa-grama are named in sargam, `Re0`.
    pub fn label(self, index: UnsignedIntegerType) -> String {
        let steps = self.degrees_per_period();
        if matches!(self, Self::PtolemyIntenseDiatonic | Self::IndianAlt) && steps == 7 {
            let octave = IntegerType::try_from(index / steps).expect("an octave fits an i32") - 1;
            return format!("{}{octave}", INDIAN_SCALE_NAMES[(index % steps) as usize]);
        }
        let index = IntegerType::try_from(index).expect("a degree index fits an i32");
        if self.is_keyboard() {
            return key_label(index);
        }
        let pitch = self
            .pitch_at(index)
            .expect("every degree of a tuning system is a pitch");
        if pitch.is_twelve_tone() {
            return key_label(pitch.pitch_space().round() as IntegerType);
        }
        let microtone = pitch.microtone().map(ToString::to_string);
        format!(
            "{}{}",
            pitch.unicode_name_with_octave(),
            microtone.unwrap_or_default()
        )
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

    /// The degree sounding nearest `hertz`, counted from `C-1` as
    /// [`TuningSystem::frequency`] counts, and measured in cents. Errors on a
    /// frequency that is not a finite number above nought.
    ///
    /// ```
    /// use music21_rs::tuningsystem::TuningSystem;
    ///
    /// let nineteen = TuningSystem::edo(19);
    /// // A4 at 440 Hz is nearest degree 5 of the fifth period above C-1.
    /// assert_eq!(nineteen.nearest_degree(440.0)?, 5 * 19 + 14);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn nearest_degree(self, hertz: FloatType) -> Result<IntegerType> {
        if !hertz.is_finite() || hertz <= 0.0 {
            return Err(Error::TuningSystem(format!(
                "a frequency must be a finite number above nought, got {hertz}"
            )));
        }
        Ok(self.nearest_degree_to_cents(OCTAVE_CENTS * (hertz / CN1).log2()))
    }

    /// The degree nearest a distance above `C-1` in cents. A table may be
    /// uneven enough that the nearest degree lies in the next period, so the
    /// periods either side are searched too.
    fn nearest_degree_to_cents(self, cents: FloatType) -> IntegerType {
        let steps = IntegerType::try_from(self.degrees_per_period())
            .expect("a period's degree count fits an i32");
        let period = (cents / self.period_cents()).floor() as IntegerType;
        let distance = |degree: IntegerType| {
            (OCTAVE_CENTS * self.ratio_at(degree.into()).log2() - cents).abs()
        };
        ((period - 1) * steps..=(period + 2) * steps)
            .min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
            .expect("the search range is never empty")
    }

    /// The pitch a degree sounds, counted from `C-1`: spelled as the nearest
    /// pitch-space value is spelled, with any remainder as a microtone.
    ///
    /// ```
    /// use music21_rs::tuningsystem::TuningSystem;
    ///
    /// // Degree 4 of the five-limit table is a just major third above C-1.
    /// let e = TuningSystem::FiveLimit.pitch_at(64)?;
    /// assert_eq!(e.name_with_octave(), "E4");
    /// assert!((e.microtone().unwrap().cents() + 13.686).abs() < 1e-3);
    /// # Ok::<(), music21_rs::Error>(())
    /// ```
    pub fn pitch_at(self, index: IntegerType) -> Result<Pitch> {
        Pitch::from_pitch_space(OCTAVE_CENTS * self.ratio_at(index.into()).log2() / 100.0)
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

    /// Whether the system has twelve degrees to the octave, one for each key
    /// of a keyboard.
    pub fn is_keyboard(self) -> bool {
        self.degrees_per_period() == 12 && self.repeats_at_the_octave()
    }

    fn period_ratio(self) -> FloatType {
        (2.0 as FloatType).powf(self.period_cents() / OCTAVE_CENTS)
    }

    fn ratio_table(self) -> Option<&'static [Ratio]> {
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

/// A step of an equal division of the octave: `tone` steps of `octave_size`.
pub const fn equal_temperament(
    tone: UnsignedIntegerType,
    octave_size: UnsignedIntegerType,
) -> Ratio {
    Ratio::root(2, tone, octave_size)
}

/// A key of a keyboard by its pitch-space number: a white key by its
/// letter, a black one by both of its names, `C♯/D♭4`.
fn key_label(pitch_space: IntegerType) -> String {
    let pitch =
        Pitch::from_pitch_space(pitch_space.into()).expect("a whole pitch-space value is a pitch");
    let octave = pitch_space.div_euclid(12) - 1;
    let alter = pitch.alter();
    if alter == 0.0 {
        return format!("{}{octave}", pitch.unicode_name());
    }
    let (sharp, flat) = if alter < 0.0 {
        (pitch.get_lower_enharmonic(), Ok(pitch))
    } else {
        (Ok(pitch.clone()), pitch.get_higher_enharmonic())
    };
    let (sharp, flat) = (
        sharp.expect("a black key has a sharp name"),
        flat.expect("a black key has a flat name"),
    );
    format!("{}/{}{octave}", sharp.unicode_name(), flat.unicode_name())
}

/// Five-tone Javanese equal-temperament approximation.
pub const JAVANESE: [Ratio; 5] = [
    Ratio::root(2, 0, 5),
    Ratio::root(2, 1, 5),
    Ratio::root(2, 2, 5),
    Ratio::root(2, 3, 5),
    Ratio::root(2, 4, 5),
];

/// Seven-tone Thai equal-temperament approximation.
pub const THAI: [Ratio; 7] = [
    Ratio::root(2, 0, 7),
    Ratio::root(2, 1, 7),
    Ratio::root(2, 2, 7),
    Ratio::root(2, 3, 7),
    Ratio::root(2, 4, 7),
    Ratio::root(2, 5, 7),
    Ratio::root(2, 6, 7),
];

/// Degree labels for the seven-tone PtolemyIntenseDiatonic scale.
pub const INDIAN_SCALE_NAMES: [&str; 7] = ["Sa", "Re", "Ga", "Ma", "Pa", "Dha", "Ni"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_system_answers_cents_and_an_exact_ratio_for_its_degrees() {
        for system in ALL_TUNING_SYSTEMS {
            let unison = system.exact_ratio(0).expect("every listed system is exact");
            assert_eq!(unison.value(), 1.0);
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
        assert_eq!(TuningSystem::edo(12).label(0), "C-1");
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
        let two_one: FloatType = Ratio::new(2, 1).into();
        assert_eq!(TuningSystem::CarlosHarmonic.ratio(12), two_one);
        assert_eq!(TuningSystem::CarlosHarmonic24.ratio(24), two_one);
        assert_eq!(TuningSystem::EQUAL_TEMPERAMENT.ratio(12), two_one);
    }

    #[test]
    fn a_ratio_is_rational_or_a_root() {
        let rational = Ratio::new(3, 2);
        assert_eq!(rational.value(), 1.5);
        assert_eq!(rational.to_string(), "3/2");
        assert_eq!(rational.with_octaves(2), Ratio::new(12, 2));
        assert_eq!(rational.to_fraction(), Some(FractionType::new(3, 2)));
        assert_eq!(rational.monzo().unwrap().to_string(), "[-1 1⟩");
        assert!((rational.cents() - 701.955).abs() < 1e-3);

        let root = Ratio::root(2, 7, 12);
        assert_eq!(root.to_string(), "2^(7/12)");
        assert_eq!(root.with_octaves(1), Ratio::root(2, 19, 12));
        assert!((root.value() - 2.0_f64.powf(7.0 / 12.0)).abs() < 1e-12);
        assert_eq!(root.to_fraction(), None);
        assert!(root.monzo().is_err());
    }

    #[test]
    fn an_equal_division_carries_its_own_step_count() {
        assert_eq!(equal_temperament(12, 12), Ratio::root(2, 12, 12));
        let quarter = TuningSystem::QUARTER_TONE;
        assert_eq!(quarter.exact_ratio(6), Some(Ratio::root(2, 6, 24)));
        assert_eq!(quarter.label(24), "C0");
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
        assert_eq!(bohlen_pierce.exact_ratio(13), Some(Ratio::root(3, 13, 13)));
        assert!(
            (bohlen_pierce.exact_ratio(4).unwrap().value() - bohlen_pierce.ratio(4)).abs() < 1e-12
        );
        assert_eq!(bohlen_pierce.cents(5), 0.0);
        assert_eq!(bohlen_pierce.label(13), "G0(+2c)");
        assert_eq!(bohlen_pierce.id(), "13edt");
        assert_eq!("13edt".parse::<TuningSystem>().unwrap(), bohlen_pierce);
    }

    /// A period with no whole-number ratio has no exact degrees, and still
    /// sounds and names itself.
    #[test]
    fn an_equal_division_of_an_irrational_period_has_no_fractions() {
        let alpha = TuningSystem::Equal(EqualDivision::new(18, 1404.0).unwrap());
        assert_eq!(alpha.exact_ratio(1), None);
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
    fn a_keyboard_system_names_its_degrees_for_their_keys() {
        let five_limit = TuningSystem::FiveLimit;
        assert_eq!(five_limit.label(60), "C4");
        assert_eq!(five_limit.label(61), "C♯/D♭4");
        assert_eq!(five_limit.label(63), "D♯/E♭4");
        assert_eq!(five_limit.label(70), "A♯/B♭4");
        assert_eq!(TuningSystem::WHOLE_TONE.label(34), "G♯/A♭4");
        // Nineteen steps do not land on keys, so each is the pitch it sounds.
        assert_eq!(TuningSystem::edo(19).label(95), "C4");
        assert_eq!(TuningSystem::edo(19).label(96), "C𝄲4(+13c)");
    }

    #[test]
    fn the_nearest_degree_to_a_degree_is_that_degree() {
        for system in ALL_TUNING_SYSTEMS
            .into_iter()
            .chain([TuningSystem::Equal(EqualDivision::tritave(13).unwrap())])
        {
            let steps = system.degrees_per_period() as IntegerType;
            for degree in (2 * steps)..(6 * steps) {
                let hertz = system.frequency_at(degree.into());
                assert_eq!(system.nearest_degree(hertz).unwrap(), degree, "{system:?}");
            }
        }
        assert!(TuningSystem::FiveLimit.nearest_degree(0.0).is_err());
        assert!(
            TuningSystem::FiveLimit
                .nearest_degree(FloatType::INFINITY)
                .is_err()
        );
    }

    #[test]
    fn a_pitch_sounds_the_nearest_degree_of_a_system_without_its_keys() {
        let nineteen = TuningSystem::edo(19);
        let a4 = Pitch::from_name("A4").unwrap();
        // 440 Hz lies between two steps of nineteen; the nearer is 436.0 Hz.
        let sounded = a4.frequency_hz_in(nineteen);
        assert!((sounded - nineteen.frequency(109)).abs() < 1e-9);
        assert!((sounded - 440.0).abs() < 1200.0 / 19.0);
        // A keyboard system still sounds each key's own degree.
        let e4 = Pitch::from_name("E4").unwrap();
        assert!((e4.frequency_hz_in(TuningSystem::FiveLimit) - 327.032).abs() < 0.001);

        // Spelled back to within the six significant digits a pitch-space
        // value is kept to.
        let back = Pitch::from_frequency_in(sounded, nineteen).unwrap();
        assert!((1200.0 * (back.frequency_hz() / sounded).log2()).abs() < 0.01);
    }

    #[test]
    fn table_ratios_shift_by_real_octaves() {
        assert_eq!(TuningSystem::CarlosHarmonic.ratio(19), 3.0);
        assert_eq!(TuningSystem::FortyThreeTone.ratio(68), 3.0);
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.ratio(8), 2.25);
    }

    #[test]
    fn non_twelve_tone_systems_keep_system_octaves_and_labels() {
        assert_eq!(TuningSystem::WHOLE_TONE.label(1), "D-1");
        assert_eq!(TuningSystem::WHOLE_TONE.degrees_per_period(), 6);
        assert!(
            (TuningSystem::WHOLE_TONE.ratio(1) - (2.0 as FloatType).powf(1.0 / 6.0)).abs() < 1e-12
        );

        assert_eq!(TuningSystem::QUARTER_TONE.label(13), "F♯𝄲-1");
        assert_eq!(TuningSystem::QUARTER_TONE.degrees_per_period(), 24);
        assert!(
            (TuningSystem::QUARTER_TONE.ratio(13) - (2.0 as FloatType).powf(13.0 / 24.0)).abs()
                < 1e-12
        );

        assert_eq!(TuningSystem::Thai.label(7), "C0");
        assert_eq!(TuningSystem::Thai.degrees_per_period(), 7);
        assert_eq!(TuningSystem::Thai.ratio(7), 2.0);

        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.label(8), "Re0");
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.degrees_per_period(), 7);
        assert_eq!(TuningSystem::PtolemyIntenseDiatonic.ratio(8), 2.25);

        assert_eq!(TuningSystem::FortyThreeTone.label(68), "G0(+2c)");
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
