use std::str::FromStr;

pub const OCTAVE_SIZE: u32 = 12;

pub const C4: f64 = 261.6256;
pub const C0: f64 = C4 / 16.0;
pub const CN1: f64 = C4 / 32.0;

pub const A4: f64 = 440.0;
pub const A0: f64 = A4 / 16.0;
pub const AN1: f64 = A4 / 32.0;

pub const TWELVE_TONE_NAMES: [&str; 12] = [
    "C", "C#/Db", "D", "D#/Eb", "E", "F", "F#/Gb", "G", "G#/Ab", "A", "A#/Bb", "B",
];

pub const WHOLE_TONE_NAMES: [&str; 6] = ["C", "D", "E", "F#/Gb", "G#/Ab", "A#/Bb"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fraction {
    pub numerator: u32,
    pub denominator: u32,
    pub base: u32,
}

impl Fraction {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self::new_with_base(numerator, denominator, 0)
    }

    pub const fn new_with_base(numerator: u32, denominator: u32, base: u32) -> Self {
        Self {
            numerator,
            denominator,
            base,
        }
    }

    pub const fn numerator(&self) -> u32 {
        self.numerator
    }

    pub const fn denominator(&self) -> u32 {
        self.denominator
    }

    pub const fn base(&self) -> u32 {
        self.base
    }

    pub fn ratio(self) -> f64 {
        self.into()
    }

    pub fn with_octaves(mut self, octaves: u32) -> Self {
        if octaves == 0 {
            return self;
        }

        if self.base == 0 {
            let multiplier = 2u32
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

impl From<Fraction> for f64 {
    fn from(frac: Fraction) -> Self {
        if frac.base == 0 {
            frac.numerator as f64 / frac.denominator as f64
        } else {
            (frac.base as f64).powf(frac.numerator as f64 / frac.denominator as f64)
        }
    }
}

impl From<(u32, u32)> for Fraction {
    fn from(frac: (u32, u32)) -> Self {
        Self::new(frac.0, frac.1)
    }
}

impl From<(u32, u32, u32)> for Fraction {
    fn from(frac: (u32, u32, u32)) -> Self {
        Self::new_with_base(frac.0, frac.1, frac.2)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TuningSystem {
    EqualTemperament { octave_size: u32 },
    RecursiveEqualTemperament { octave_size: u32 },
    WholeTone,
    QuarterTone,

    JustIntonation,
    JustIntonation24,
    PythagoreanTuning,

    FiveLimit,
    ElevenLimit,

    FortyThreeTone,

    StepMethod,

    // Ethnic scales.
    Javanese,
    Thai,
    Indian,
    IndianAlt,
    Indian22,
    IndianFull,
}

impl TuningSystem {
    pub fn ratio(self, index: usize) -> f64 {
        get_ratio(self, index, None)
    }

    pub fn fraction(self, index: usize) -> Fraction {
        get_fraction(self, index, None)
    }

    pub fn label(self, index: u32) -> String {
        get_label(self, index, None)
    }

    pub fn octave(self, index: u32) -> u32 {
        index / self.octave_size()
    }

    pub fn frequency(self, index: u32) -> f64 {
        get_frequency(self, index, None)
    }

    pub fn cents(self, index: u32) -> f64 {
        get_cents(self, index, None)
    }

    pub fn octave_size(self) -> u32 {
        match self {
            Self::EqualTemperament { octave_size }
            | Self::RecursiveEqualTemperament { octave_size } => octave_size,
            Self::WholeTone => 6,
            Self::QuarterTone | Self::JustIntonation24 => 24,
            Self::FortyThreeTone => 43,
            Self::ElevenLimit => 29,
            Self::Javanese => 5,
            Self::Thai | Self::Indian | Self::IndianAlt => 7,
            Self::Indian22 | Self::IndianFull => 22,
            Self::JustIntonation | Self::PythagoreanTuning | Self::FiveLimit | Self::StepMethod => {
                OCTAVE_SIZE
            }
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
            Self::Javanese => Some(&JAVANESE),
            Self::Thai => Some(&THAI),
            Self::Indian => Some(&INDIAN_SCALE),
            Self::IndianAlt => Some(&INDIA_SCALE_ALT),
            Self::Indian22 | Self::IndianFull => Some(&INDIAN_SCALE_22),
            Self::EqualTemperament { .. }
            | Self::RecursiveEqualTemperament { .. }
            | Self::WholeTone
            | Self::QuarterTone
            | Self::StepMethod => None,
        }
    }

    fn degree_label(self, index: u32, octave_size: u32) -> String {
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

impl FromStr for TuningSystem {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "EqualTemperament" => Ok(Self::EqualTemperament {
                octave_size: OCTAVE_SIZE,
            }),
            "RecursiveEqualTemperament" => Ok(Self::RecursiveEqualTemperament {
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
            "StepMethod" => Ok(Self::StepMethod),
            "Javanese" => Ok(Self::Javanese),
            "Thai" => Ok(Self::Thai),
            "Indian" => Ok(Self::Indian),
            "IndianAlt" => Ok(Self::IndianAlt),
            "Indian22" => Ok(Self::Indian22),
            "IndianFull" => Ok(Self::IndianFull),
            _ => Err(()),
        }
    }
}

pub fn equal_temperament(tone: u32, octave_size: u32) -> Fraction {
    Fraction::new_with_base(tone, octave_size, 2)
}

pub fn equal_temperament_12(tone: u32) -> Fraction {
    equal_temperament(tone, 12)
}

pub fn equal_temperament_default(tone: u32) -> Fraction {
    equal_temperament(tone, OCTAVE_SIZE)
}

pub fn get_ratio(tuning_system: TuningSystem, index: usize, size: Option<u32>) -> f64 {
    get_fraction(tuning_system, index, size).into()
}

pub fn get_fraction(tuning_system: TuningSystem, index: usize, size: Option<u32>) -> Fraction {
    match tuning_system {
        TuningSystem::EqualTemperament { octave_size }
        | TuningSystem::RecursiveEqualTemperament { octave_size } => {
            equal_temperament(index_to_u32(index), size.unwrap_or(octave_size))
        }
        TuningSystem::WholeTone => equal_temperament(index_to_u32(index), size.unwrap_or(6)),
        TuningSystem::QuarterTone => equal_temperament(index_to_u32(index), size.unwrap_or(24)),
        TuningSystem::StepMethod => {
            equal_temperament(index_to_u32(index), size.unwrap_or(OCTAVE_SIZE))
        }
        _ => get_fraction_from_table(tuning_system, index),
    }
}

pub fn get_label(tuning_system: TuningSystem, index: u32, size: Option<u32>) -> String {
    let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
    assert!(octave_size > 0, "octave_size must be greater than zero");
    degree_name_with_octave(
        &tuning_system.degree_label(index, octave_size),
        index / octave_size,
    )
}

pub fn get_frequency(tuning_system: TuningSystem, index: u32, size: Option<u32>) -> f64 {
    if let Some(size) = size {
        assert!(size > 0, "octave_size must be greater than zero");
    }
    get_ratio(tuning_system, index as usize, size) * CN1
}

pub fn get_cents(tuning_system: TuningSystem, index: u32, size: Option<u32>) -> f64 {
    let octave_size = size.unwrap_or_else(|| tuning_system.octave_size());
    assert!(octave_size > 0, "octave_size must be greater than zero");
    let reference_freq: f64 = equal_temperament(index, octave_size).into();
    let comparison_freq = get_frequency(tuning_system, index, size);
    1200.0 * (comparison_freq / reference_freq).log2()
}

fn get_fraction_from_table(tuning_system: TuningSystem, index: usize) -> Fraction {
    let table = tuning_system
        .ratio_table()
        .expect("tuning system does not have a ratio table");
    let len = table.len();
    let octaves = (index / len) as u32;
    table[index % len].with_octaves(octaves)
}

fn index_to_u32(index: usize) -> u32 {
    u32::try_from(index).expect("tone index exceeds u32 range")
}

fn default_degree_label(octave_size: u32, index: u32) -> String {
    if octave_size == OCTAVE_SIZE {
        TWELVE_TONE_NAMES[(index % OCTAVE_SIZE) as usize].to_string()
    } else {
        format!("T{}", index % octave_size)
    }
}

fn degree_name_with_octave(degree_label: &str, octave: u32) -> String {
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

pub const PYTHAGOREAN_TUNING: [Fraction; 12] = [
    Fraction::new(1, 1),
    Fraction::new(256, 243),
    Fraction::new(9, 8),
    Fraction::new(32, 27),
    Fraction::new(81, 64),
    Fraction::new(4, 3),
    Fraction::new(729, 512),
    Fraction::new(3, 2),
    Fraction::new(27, 16),
    Fraction::new(16, 9),
    Fraction::new(243, 128),
    Fraction::new(15, 8),
];

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

pub const FORTYTHREE_TONE: [Fraction; 43] = FORTY_THREE_TONE;

pub const JAVANESE: [Fraction; 5] = [
    Fraction::new_with_base(0, 5, 2),
    Fraction::new_with_base(1, 5, 2),
    Fraction::new_with_base(2, 5, 2),
    Fraction::new_with_base(3, 5, 2),
    Fraction::new_with_base(4, 5, 2),
];

pub const THAI: [Fraction; 7] = [
    Fraction::new_with_base(0, 7, 2),
    Fraction::new_with_base(1, 7, 2),
    Fraction::new_with_base(2, 7, 2),
    Fraction::new_with_base(3, 7, 2),
    Fraction::new_with_base(4, 7, 2),
    Fraction::new_with_base(5, 7, 2),
    Fraction::new_with_base(6, 7, 2),
];

pub const INDIAN_SCALE_NAMES: [&str; 7] = ["Sa", "Re", "Ga", "Ma", "Pa", "Dha", "Ni"];

pub const INDIAN_SCALE: [Fraction; 7] = [
    Fraction::new(1, 1),
    Fraction::new(9, 8),
    Fraction::new(5, 4),
    Fraction::new(4, 3),
    Fraction::new(3, 2),
    Fraction::new(5, 3),
    Fraction::new(15, 8),
];

pub const INDIA_SCALE_ALT: [Fraction; 7] = [
    Fraction::new(1, 1),
    Fraction::new(9, 8),
    Fraction::new(5, 4),
    Fraction::new(4, 3),
    Fraction::new(3, 2),
    Fraction::new(27, 16),
    Fraction::new(15, 8),
];

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
    fn ratio_helpers_cover_octaves() {
        let two_one: f64 = Fraction::new(2, 1).into();
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
        assert!((TuningSystem::WholeTone.ratio(1) - 2.0_f64.powf(1.0 / 6.0)).abs() < 1e-12);

        assert_eq!(TuningSystem::QuarterTone.label(13), "T13ON1");
        assert_eq!(TuningSystem::QuarterTone.octave_size(), 24);
        assert!((TuningSystem::QuarterTone.ratio(13) - 2.0_f64.powf(13.0 / 24.0)).abs() < 1e-12);

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
}
