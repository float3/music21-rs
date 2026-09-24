use crate::{
    chord::Chord,
    defaults::{FloatType, IntegerType},
    error::{Error, Result},
    interval::Interval,
    key::Key,
    pitch::Pitch,
};

pub mod enharmonics;
pub mod harmonic_function;
pub mod neoriemannian;
pub mod transposition;

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

/// A ranked key estimate.
#[derive(Clone, Debug)]
#[must_use]
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

/// Estimates likely keys from pitches using the given weights, each pitch
/// counting once.
pub fn estimate_key_from_pitches_with(
    profile: KeyProfile,
    pitches: &[Pitch],
) -> Result<Vec<KeyEstimate>> {
    if pitches.is_empty() {
        return Err(Error::Analysis(
            "key estimation needs at least one pitch".to_string(),
        ));
    }
    let distribution = pitch_class_distribution(
        pitches
            .iter()
            .map(|pitch| (std::slice::from_ref(pitch), 1.0)),
    )
    .expect("there is at least one pitch");
    Ok(estimate_key_from_distribution(profile, &distribution))
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

/// How long each pitch class sounds: for each note, its pitches and its
/// length in quarter notes, every pitch counting for the whole length.
/// Nothing when there are no notes. music21's
/// `_getPitchClassDistribution`, a quarter tone rounding to the even class
/// as music21's `pitchClass` does.
pub fn pitch_class_distribution<'a>(
    notes: impl IntoIterator<Item = (&'a [Pitch], FloatType)>,
) -> Option<[FloatType; 12]> {
    let mut distribution = [0.0; 12];
    let mut any = false;
    for (pitches, length) in notes {
        any = true;
        for pitch in pitches {
            let class = (pitch.ps().round_ties_even() as IntegerType).rem_euclid(12) as usize;
            distribution[class] += length;
        }
    }
    any.then_some(distribution)
}

/// The key names music21's key analysis spells a major tonic with.
const VALID_MAJOR: [&str; 15] = [
    "C", "C#", "C-", "D-", "D", "E-", "E", "F", "F#", "G-", "G", "A-", "A", "B-", "B",
];

/// The key names music21's key analysis spells a minor tonic with.
const VALID_MINOR: [&str; 15] = [
    "C", "C#", "D", "D#", "E-", "E", "F", "F#", "G", "G#", "A-", "A", "A#", "B-", "B",
];

/// Every key ranked by how well `profile` fits `distribution`, best first:
/// music21's key-weight analysis.
///
/// Each of the 24 keys is scored by the correlation between the
/// distribution and the profile turned to that tonic, summed in music21's
/// order and averaged as Python sums, so a score is music21's to within the
/// last digit -- music21 squares through the platform's `pow`, which is not
/// always exact, where this multiplies. Ties are broken as
/// music21 breaks them, by the higher tonic and then minor before major, and
/// each tonic is spelled as music21 spells it: the pitch class's usual name,
/// respelled where that is no key in the mode, so a major key on the eighth
/// class is `A-` and a minor one `G#`.
pub fn estimate_key_from_distribution(
    profile: KeyProfile,
    distribution: &[FloatType; 12],
) -> Vec<KeyEstimate> {
    let histogram_average = python_sum(distribution) / 12.0;
    let mut ranked: Vec<(FloatType, usize, &'static str)> = Vec::with_capacity(24);
    for (mode, weights) in [
        ("major", profile.major_weights()),
        ("minor", profile.minor_weights()),
    ] {
        let profile_average = python_sum(&weights) / 12.0;
        for tonic in 0..12 {
            let mut top = 0.0;
            let mut bottom_right = 0.0;
            let mut bottom_left = 0.0;
            for (class, value) in distribution.iter().enumerate() {
                let weight = weights[(class + 12 - tonic) % 12];
                top += (weight - profile_average) * (value - histogram_average);
                bottom_right += (weight - profile_average) * (weight - profile_average);
                bottom_left += (value - histogram_average) * (value - histogram_average);
            }
            let score = if bottom_right == 0.0 || bottom_left == 0.0 {
                0.0
            } else {
                top / (bottom_right * bottom_left).sqrt()
            };
            ranked.push((score, tonic, mode));
        }
    }
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(a.2)));
    ranked
        .into_iter()
        .map(|(score, tonic, mode)| KeyEstimate {
            key: key_on(tonic, mode),
            score,
        })
        .collect()
}

/// A sum of floats as Python's `sum` makes it, Neumaier's compensated sum,
/// so an average music21 takes of the same numbers is the same number.
fn python_sum(values: &[FloatType]) -> FloatType {
    let mut total: FloatType = 0.0;
    let mut compensation: FloatType = 0.0;
    for &value in values {
        let next = total + value;
        if total.abs() >= value.abs() {
            compensation += (total - next) + value;
        } else {
            compensation += (value - next) + total;
        }
        total = next;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    total
}

/// The key on pitch class `tonic` in `mode`, spelled as music21's analysis
/// spells it.
fn key_on(tonic: usize, mode: &str) -> Key {
    const USUAL: [&str; 12] = [
        "C", "C#", "D", "E-", "E", "F", "F#", "G", "G#", "A", "B-", "B",
    ];
    const RESPELLED: [&str; 12] = [
        "B#", "D-", "C##", "D#", "F-", "E#", "G-", "F##", "A-", "B--", "A#", "C-",
    ];
    let valid: &[&str] = if mode == "major" {
        &VALID_MAJOR
    } else {
        &VALID_MINOR
    };
    let name = if valid.contains(&USUAL[tonic]) {
        USUAL[tonic]
    } else {
        RESPELLED[tonic]
    };
    Key::from_tonic_mode(name, mode).expect("every analysis tonic names a key")
}

/// The lowest and the highest pitch among `pitches`, the first of each
/// where several share a place: music21's `Ambitus.getPitchSpan`. Nothing
/// for no pitches.
pub fn pitch_span(pitches: &[Pitch]) -> Option<(Pitch, Pitch)> {
    let first = pitches.first()?;
    let mut lowest = first;
    let mut highest = first;
    for pitch in &pitches[1..] {
        if pitch.ps() < lowest.ps() {
            lowest = pitch;
        }
        if pitch.ps() > highest.ps() {
            highest = pitch;
        }
    }
    Some((lowest.clone(), highest.clone()))
}

/// Every melodic interval between neighbouring notes of each line, with how
/// often it comes, in the order each is first met: music21's
/// `MelodicIntervalDiversity.countMelodicIntervals`. With `ignore_direction`
/// a falling interval counts as the rising one, and with `ignore_unison` a
/// repeated note does not count. Intervals are told apart by their directed
/// name, and the first of each is the one kept.
///
/// # Errors
///
/// Two neighbouring pitches no interval can be spelled between.
pub fn melodic_interval_counts(
    lines: &[Vec<Pitch>],
    ignore_direction: bool,
    ignore_unison: bool,
) -> Result<Vec<(Interval, usize)>> {
    let mut found: Vec<(String, Interval, usize)> = Vec::new();
    for line in lines {
        for pair in line.windows(2) {
            let mut interval = Interval::between_pitches(&pair[0], &pair[1])?;
            let semitones = interval.semitones();
            if ignore_unison && semitones == 0.0 {
                continue;
            }
            if ignore_direction && semitones < 0.0 {
                interval = interval.reversed()?;
            }
            let name = interval.directed_name();
            match found.iter_mut().find(|(known, _, _)| *known == name) {
                Some((_, _, count)) => *count += 1,
                None => found.push((name, interval, 1)),
            }
        }
    }
    Ok(found
        .into_iter()
        .map(|(_, interval, count)| (interval, count))
        .collect())
}

/// How decisively the first of a ranked list of key estimates wins: music21's
/// `Key.tonalCertainty` for a key that came out of analysis. It is the
/// leader's score plus twice its lead over the next positive score; with no
/// positive runner-up it is the leader's score, floored at zero.
pub fn tonal_certainty(estimates: &[KeyEstimate]) -> FloatType {
    let scores: Vec<FloatType> = estimates.iter().map(KeyEstimate::score).collect();
    tonal_certainty_from_scores(&scores)
}

/// The same measure over the scores alone, for a ranking that came from
/// somewhere else — music21 hands its own analysis results around as keys
/// carrying a `correlationCoefficient` rather than as estimates.
pub fn tonal_certainty_from_scores(scores: &[FloatType]) -> FloatType {
    let Some(leader) = scores.first().copied() else {
        return 0.0;
    };
    match scores[1..].iter().copied().find(|score| *score > 0.0) {
        Some(second) => leader + 2.0 * (leader - second),
        None => leader.max(0.0),
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_profile_names_its_music21_class() {
        assert_eq!(KeyProfile::AardenEssen.music21_class_name(), "AardenEssen");
        assert_eq!(
            KeyProfile::KrumhanslSchmuckler.music21_class_name(),
            "KrumhanslSchmuckler"
        );
    }

    #[test]
    fn tonal_certainty_rewards_a_clear_leader() {
        let scale: Vec<Pitch> = ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"]
            .iter()
            .map(|name| Pitch::from_name(*name).unwrap())
            .collect();
        let ranked = estimate_key_from_pitches(&scale).unwrap();
        let leader = ranked[0].score();
        let second = ranked[1].score();
        assert!(second > 0.0);
        assert!((tonal_certainty(&ranked) - (leader + 2.0 * (leader - second))).abs() < 1e-12);
        assert_eq!(tonal_certainty(&ranked[..1]), leader.max(0.0));
        assert_eq!(tonal_certainty(&[]), 0.0);
    }
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
