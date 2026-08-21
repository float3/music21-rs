//! music21's `WeightedHexatonicBlues`, made reproducible.
//!
//! Upstream this is the one `ConcreteScale` subclass that is *not*
//! deterministic: its `IntervalNetwork` sets `deterministic=False` and branches
//! at the fourth degree, so two calls on the same tonic can return six or seven
//! pitches. It is a sampler, not a table, which is why it cannot be a
//! [`ScaleType`](super::ScaleType) variant — a generated parity fixture for it
//! would go red at random.
//!
//! The branch is the only source of that randomness, and it has exactly two
//! outcomes: take the blue note or skip it. Both are exposed here as named
//! [`BluesForm`] variants that are perfectly deterministic, plus a
//! [`WeightedHexatonicBlues::sample`] that picks between them from a seed you
//! supply. Nothing in this crate reaches for a global random number generator.

use crate::error::Result;
use crate::pitch::Pitch;

use super::stepscale::StepScale;

/// Which side of the network's branch a realization takes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BluesForm {
    /// Skip the blue note: the minor pentatonic, six pitches with the octave.
    Pentatonic,
    /// Take the blue note: the hexatonic blues scale, seven pitches.
    Hexatonic,
}

impl BluesForm {
    /// Both forms, in declaration order.
    pub const ALL: [BluesForm; 2] = [Self::Pentatonic, Self::Hexatonic];

    /// Returns the step intervals from the tonic.
    fn steps(self) -> &'static [&'static str] {
        match self {
            // c -> e- -> f -> g -> b- -> c
            Self::Pentatonic => &["m3", "M2", "M2", "m3", "M2"],
            // c -> e- -> f -> f# -> g -> b- -> c, the blue note between f and g
            Self::Hexatonic => &["m3", "M2", "a1", "m2", "m3", "M2"],
        }
    }
}

/// music21's `WeightedHexatonicBlues`, with the randomness made explicit.
///
/// ```
/// use music21_rs::{BluesForm, Pitch, WeightedHexatonicBlues};
///
/// let blues = WeightedHexatonicBlues::new(Pitch::from_name("C4")?);
///
/// let names: Vec<String> = blues.pitches(BluesForm::Hexatonic)?
///     .iter().map(|p| p.name()).collect();
/// assert_eq!(names, ["C", "E-", "F", "F#", "G", "B-", "C"]);
///
/// let names: Vec<String> = blues.pitches(BluesForm::Pentatonic)?
///     .iter().map(|p| p.name()).collect();
/// assert_eq!(names, ["C", "E-", "F", "G", "B-", "C"]);
/// # Ok::<(), music21_rs::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeightedHexatonicBlues {
    tonic: Pitch,
}

impl WeightedHexatonicBlues {
    /// Builds the scale on a tonic.
    pub fn new(tonic: Pitch) -> Self {
        Self { tonic }
    }

    /// Returns the tonic pitch.
    pub fn tonic(&self) -> &Pitch {
        &self.tonic
    }

    /// Returns the pitches of one form, from the tonic through its octave.
    pub fn pitches(&self, form: BluesForm) -> Result<Vec<Pitch>> {
        self.scale(form)?.pitches()
    }

    /// Returns one form as a [`StepScale`], for degree and interval access.
    pub fn scale(&self, form: BluesForm) -> Result<StepScale> {
        StepScale::cyclical(self.tonic.clone(), form.steps())
    }

    /// Picks a form from `seed`, the way music21 picks one at random.
    ///
    /// Deterministic for a given seed, so a caller who wants music21's
    /// behaviour can supply entropy and a caller who wants a reproducible
    /// result can supply a constant. The two forms are equally likely.
    pub fn form_for_seed(seed: u64) -> BluesForm {
        if split_mix_64(seed) & 1 == 0 {
            BluesForm::Pentatonic
        } else {
            BluesForm::Hexatonic
        }
    }

    /// Returns the pitches of the form [`form_for_seed`] picks.
    ///
    /// [`form_for_seed`]: WeightedHexatonicBlues::form_for_seed
    pub fn sample(&self, seed: u64) -> Result<Vec<Pitch>> {
        self.pitches(Self::form_for_seed(seed))
    }
}

/// SplitMix64, so a seed maps to a well-mixed bit pattern.
///
/// A whole PRNG would be overkill: one branch is drawn per realization, and the
/// crate has no random-number dependency to reach for.
fn split_mix_64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tonic: &str, form: BluesForm) -> Vec<String> {
        WeightedHexatonicBlues::new(Pitch::from_name(tonic).expect("valid tonic"))
            .pitches(form)
            .expect("scale realizes")
            .iter()
            .map(|pitch| pitch.name())
            .collect()
    }

    #[test]
    fn both_forms_match_music21() {
        // Captured from music21, which returns one or the other at random.
        assert_eq!(
            names("C4", BluesForm::Hexatonic),
            ["C", "E-", "F", "F#", "G", "B-", "C"]
        );
        assert_eq!(
            names("C4", BluesForm::Pentatonic),
            ["C", "E-", "F", "G", "B-", "C"]
        );
        assert_eq!(
            names("G4", BluesForm::Hexatonic),
            ["G", "B-", "C", "C#", "D", "F", "G"]
        );
        assert_eq!(
            names("E-4", BluesForm::Hexatonic),
            ["E-", "G-", "A-", "A", "B-", "D-", "E-"]
        );
        assert_eq!(
            names("F#4", BluesForm::Hexatonic),
            ["F#", "A", "B", "B#", "C#", "E", "F#"]
        );
    }

    #[test]
    fn the_blue_note_is_the_only_difference() {
        let hexatonic = names("C4", BluesForm::Hexatonic);
        let pentatonic = names("C4", BluesForm::Pentatonic);
        assert_eq!(hexatonic.len(), pentatonic.len() + 1);

        let without_blue_note: Vec<&String> =
            hexatonic.iter().filter(|name| *name != "F#").collect();
        assert_eq!(without_blue_note, pentatonic.iter().collect::<Vec<_>>());
    }

    #[test]
    fn sampling_is_reproducible_and_reaches_both_forms() {
        let blues = WeightedHexatonicBlues::new(Pitch::from_name("C4").unwrap());
        for seed in [0, 1, 7, 42, u64::MAX] {
            assert_eq!(
                blues.sample(seed).unwrap(),
                blues.sample(seed).unwrap(),
                "seed {seed} should be reproducible"
            );
        }

        let forms: Vec<BluesForm> = (0..64).map(WeightedHexatonicBlues::form_for_seed).collect();
        assert!(forms.contains(&BluesForm::Pentatonic));
        assert!(forms.contains(&BluesForm::Hexatonic));
    }

    #[test]
    fn every_form_realizes_on_every_common_tonic() {
        for form in BluesForm::ALL {
            for tonic in [
                "C4", "G4", "D4", "A4", "E4", "B4", "F#4", "F4", "B-4", "E-4",
            ] {
                let pitches = WeightedHexatonicBlues::new(Pitch::from_name(tonic).unwrap())
                    .pitches(form)
                    .expect("scale realizes");
                assert_eq!(pitches.len(), form.steps().len() + 1, "{form:?} on {tonic}");
            }
        }
    }
}
