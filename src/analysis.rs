use crate::{
    chord::Chord,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    key::Key,
    pitch::Pitch,
};

/// A set of key-finding weights for the Krumhansl-Schmuckler algorithm.
///
/// These are the profiles music21's `analysis.discrete` ships, with the
/// characterisations Craig Sapp gives them in the Humdrum `keycor` manual.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum KeyProfile {
    /// Krumhansl and Kessler's 1982 probe-tone ratings. Strong tendency to
    /// name the dominant as the tonic.
    KrumhanslSchmuckler,
    /// Aarden's 2003 profile from the Essen folksong collection, music21's
    /// default. Weak tendency to name the subdominant as the tonic.
    AardenEssen,
    /// Sapp's simple weights, most consistent over long stretches of music.
    SimpleWeights,
    /// Bellman and Budge's profile, with no particular neighbouring-key bias.
    BellmanBudge,
    /// Temperley's Kostka-Payne corpus profile. Strong tendency to name the
    /// relative major in minor keys.
    TemperleyKostkaPayne,
}

impl KeyProfile {
    /// Every profile, in music21's order.
    pub const ALL: [KeyProfile; 5] = [
        Self::KrumhanslSchmuckler,
        Self::AardenEssen,
        Self::SimpleWeights,
        Self::BellmanBudge,
        Self::TemperleyKostkaPayne,
    ];

    /// The weights for the twelve pitch classes above a major tonic.
    pub fn major_weights(self) -> [FloatType; 12] {
        match self {
            Self::KrumhanslSchmuckler => [
                6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
            ],
            Self::AardenEssen => [
                17.7661, 0.145624, 14.9265, 0.160186, 19.8049, 11.3587, 0.291248, 22.062, 0.145624,
                8.15494, 0.232998, 4.95122,
            ],
            Self::SimpleWeights => [2.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 2.0, 0.0, 1.0, 0.0, 1.0],
            Self::BellmanBudge => [
                16.80, 0.86, 12.95, 1.41, 13.49, 11.93, 1.25, 20.28, 1.80, 8.04, 0.62, 10.57,
            ],
            Self::TemperleyKostkaPayne => [
                0.748, 0.060, 0.488, 0.082, 0.670, 0.460, 0.096, 0.715, 0.104, 0.366, 0.057, 0.400,
            ],
        }
    }

    /// The weights for the twelve pitch classes above a minor tonic.
    pub fn minor_weights(self) -> [FloatType; 12] {
        match self {
            Self::KrumhanslSchmuckler => [
                6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
            ],
            Self::AardenEssen => [
                18.2648, 0.737619, 14.0499, 16.8599, 0.702494, 14.4362, 0.702494, 18.6161, 4.56621,
                1.93186, 7.37619, 1.75623,
            ],
            Self::SimpleWeights => [2.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 2.0, 1.0, 0.0, 0.5, 0.5],
            Self::BellmanBudge => [
                18.16, 0.69, 12.99, 13.34, 1.07, 11.15, 1.38, 21.07, 7.49, 1.53, 0.92, 10.21,
            ],
            Self::TemperleyKostkaPayne => [
                0.712, 0.084, 0.474, 0.618, 0.049, 0.460, 0.105, 0.747, 0.404, 0.067, 0.133, 0.330,
            ],
        }
    }

    /// The name of the music21 class carrying these weights.
    pub fn music21_class_name(self) -> &'static str {
        match self {
            Self::KrumhanslSchmuckler => "KrumhanslSchmuckler",
            Self::AardenEssen => "AardenEssen",
            Self::SimpleWeights => "SimpleWeights",
            Self::BellmanBudge => "BellmanBudge",
            Self::TemperleyKostkaPayne => "TemperleyKostkaPayne",
        }
    }
}

const TONICS: [&str; 12] = [
    "C", "C#", "D", "E-", "E", "F", "F#", "G", "A-", "A", "B-", "B",
];

/// A ranked key estimate.
#[derive(Clone, Debug)]
pub struct KeyEstimate {
    key: Key,
    score: FloatType,
}

impl KeyEstimate {
    /// Returns the estimated key.
    pub fn key(&self) -> &Key {
        &self.key
    }

    /// Returns the correlation score. Higher is a better fit.
    pub fn score(&self) -> FloatType {
        self.score
    }
}

/// Estimates likely keys from pitches using the Krumhansl-Schmuckler weights.
pub fn estimate_key_from_pitches(pitches: &[Pitch]) -> Result<Vec<KeyEstimate>> {
    estimate_key_from_pitches_with(KeyProfile::KrumhanslSchmuckler, pitches)
}

/// Estimates likely keys from pitches using the given weights.
pub fn estimate_key_from_pitches_with(
    profile: KeyProfile,
    pitches: &[Pitch],
) -> Result<Vec<KeyEstimate>> {
    if pitches.is_empty() {
        return Err(Error::Analysis(
            "key estimation needs at least one pitch".to_string(),
        ));
    }

    let mut histogram = [0.0; 12];
    for pitch in pitches {
        let pc = (pitch.ps().round() as IntegerType).rem_euclid(12) as usize;
        histogram[pc] += 1.0;
    }

    estimate_key_from_histogram(profile, &histogram)
}

/// Estimates likely keys from chords using the Krumhansl-Schmuckler weights.
pub fn estimate_key_from_chords(chords: &[Chord]) -> Result<Vec<KeyEstimate>> {
    estimate_key_from_chords_with(KeyProfile::KrumhanslSchmuckler, chords)
}

/// Estimates likely keys from chords using the given weights.
pub fn estimate_key_from_chords_with(
    profile: KeyProfile,
    chords: &[Chord],
) -> Result<Vec<KeyEstimate>> {
    let pitches = chords.iter().flat_map(Chord::pitches).collect::<Vec<_>>();
    estimate_key_from_pitches_with(profile, &pitches)
}

fn estimate_key_from_histogram(
    profile: KeyProfile,
    histogram: &[FloatType; 12],
) -> Result<Vec<KeyEstimate>> {
    let mut estimates = Vec::new();
    for (tonic_pc, tonic) in TONICS.iter().enumerate() {
        for (mode, weights) in [
            ("major", profile.major_weights()),
            ("minor", profile.minor_weights()),
        ] {
            let key = Key::from_tonic_mode(tonic, mode)?;
            let rotated = rotate_profile(&weights, tonic_pc);
            estimates.push(KeyEstimate {
                key,
                score: correlation(histogram, &rotated),
            });
        }
    }

    estimates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(estimates)
}

fn rotate_profile(profile: &[FloatType; 12], tonic_pc: usize) -> [FloatType; 12] {
    let mut rotated = [0.0; 12];
    for pc in 0..12 {
        rotated[pc] = profile[(pc + 12 - tonic_pc) % 12];
    }
    rotated
}

fn correlation(left: &[FloatType; 12], right: &[FloatType; 12]) -> FloatType {
    let left_mean = left.iter().sum::<FloatType>() / 12.0;
    let right_mean = right.iter().sum::<FloatType>() / 12.0;
    let mut numerator = 0.0;
    let mut left_sum = 0.0;
    let mut right_sum = 0.0;

    for (left_value, right_value) in left.iter().zip(right) {
        let left_centered = left_value - left_mean;
        let right_centered = right_value - right_mean;
        numerator += left_centered * right_centered;
        left_sum += left_centered.powi(2);
        right_sum += right_centered.powi(2);
    }

    let denominator = left_sum.sqrt() * right_sum.sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_c_major_from_tonic_triad_material() {
        let pitches = ["C4", "E4", "G4", "C5", "E5", "G5"]
            .into_iter()
            .map(Pitch::from_name)
            .collect::<Result<Vec<_>>>()
            .unwrap();
        let estimates = estimate_key_from_pitches(&pitches).unwrap();
        assert_eq!(estimates[0].key().tonic().name(), "C");
        assert_eq!(estimates[0].key().mode(), "major");
    }

    #[test]
    fn estimates_from_chords() {
        let chords = [Chord::new("C E G").unwrap(), Chord::new("F A C").unwrap()];
        let estimates = estimate_key_from_chords(&chords).unwrap();
        assert!(!estimates.is_empty());
    }

    #[test]
    fn every_profile_agrees_on_unambiguous_material() {
        let pitches = ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5", "G4", "C4"]
            .into_iter()
            .map(Pitch::from_name)
            .collect::<Result<Vec<_>>>()
            .unwrap();
        for profile in KeyProfile::ALL {
            let estimates = estimate_key_from_pitches_with(profile, &pitches).unwrap();
            assert_eq!(estimates[0].key().tonic().name(), "C", "{profile:?}");
            assert_eq!(estimates[0].key().mode(), "major", "{profile:?}");
        }
    }
}
