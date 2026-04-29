use crate::{
    chord::Chord,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    key::Key,
    pitch::Pitch,
};

const MAJOR_PROFILE: [FloatType; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [FloatType; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
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

/// Estimates likely keys from pitches using Krumhansl-Schmuckler profiles.
pub fn estimate_key_from_pitches(pitches: &[Pitch]) -> Result<Vec<KeyEstimate>> {
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

    estimate_key_from_histogram(&histogram)
}

/// Estimates likely keys from chords using Krumhansl-Schmuckler profiles.
pub fn estimate_key_from_chords(chords: &[Chord]) -> Result<Vec<KeyEstimate>> {
    let pitches = chords.iter().flat_map(Chord::pitches).collect::<Vec<_>>();
    estimate_key_from_pitches(&pitches)
}

fn estimate_key_from_histogram(histogram: &[FloatType; 12]) -> Result<Vec<KeyEstimate>> {
    let mut estimates = Vec::new();
    for (tonic_pc, tonic) in TONICS.iter().enumerate() {
        for (mode, profile) in [("major", MAJOR_PROFILE), ("minor", MINOR_PROFILE)] {
            let key = Key::from_tonic_mode(tonic, mode)?;
            let rotated = rotate_profile(&profile, tonic_pc);
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
}
