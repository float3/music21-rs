use crate::defaults::UnsignedIntegerType;

pub enum TuningSystem {
    EqualTemperament { octave_size: UnsignedIntegerType },
    RecursiveEqualTemperament { octave_size: UnsignedIntegerType },
    WholeTone,
    QuarterTone,

    JustIntonation,
    JustIntonation24,
    PythagoreanTuning,

    FiveLimit,
    ElevenLimit,

    FortyThreeTone,

    // ethnic scales
    Javanese,
    Thai,
    Indian,
    IndianAlt,
    Indian22,
}
