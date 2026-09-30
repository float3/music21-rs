use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::pitch::Pitch;
use crate::tuningsystem::{A4, EqualDivision, Monzo, Temperament, Val};

/// A way of sizing an interval by what it is, not by which keys it spans.
///
/// A spelled interval is a [`Monzo`] (see
/// [`Interval::pythagorean_monzo`](crate::Interval::pythagorean_monzo)), so a
/// tuning here says how wide each monzo sounds. `C#` and `D-` are then two
/// notes wherever the tuning keeps them apart, which a keyboard table indexed
/// by semitone cannot do.
pub trait Tuning {
    /// How wide `interval` sounds, in cents.
    fn cents_of(&self, interval: &Monzo) -> Result<FloatType>;
}

/// Just intonation: every interval exactly as wide as its ratio.
///
/// For a spelled interval, which is octaves and fifths, this is Pythagorean
/// tuning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Just;

impl Tuning for Just {
    fn cents_of(&self, interval: &Monzo) -> Result<FloatType> {
        Ok(interval.cents())
    }
}

impl Tuning for Temperament {
    fn cents_of(&self, interval: &Monzo) -> Result<FloatType> {
        self.cents(interval)
    }
}

impl Tuning for EqualDivision {
    /// Each prime takes its nearest number of steps (the patent val).
    fn cents_of(&self, interval: &Monzo) -> Result<FloatType> {
        let Some(limit) = interval.limit() else {
            return Ok(0.0);
        };
        let val: Val = self.patent_val(limit)?;
        Ok(FloatType::from(val.map(interval)) * self.step_cents())
    }
}

/// The pitch a tuning is measured from, and the frequency it sounds at.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct Reference {
    pitch: Pitch,
    hertz: FloatType,
}

impl Reference {
    /// `pitch` sounding at `hertz`. Errors on a frequency that is not a
    /// finite number above nought.
    pub fn new(pitch: Pitch, hertz: FloatType) -> Result<Self> {
        if !hertz.is_finite() || hertz <= 0.0 {
            return Err(Error::TuningSystem(format!(
                "a reference frequency must be a finite number above nought, got {hertz}"
            )));
        }
        Ok(Self { pitch, hertz })
    }

    /// `A4` at 440 Hz.
    pub fn a440() -> Self {
        Self {
            pitch: Pitch::from_name("A4").expect("A4 is a pitch name"),
            hertz: A4,
        }
    }

    /// The pitch the tuning is measured from.
    pub fn pitch(&self) -> &Pitch {
        &self.pitch
    }

    /// The frequency the reference pitch sounds at, in hertz.
    #[must_use]
    pub fn hertz(&self) -> FloatType {
        self.hertz
    }
}

impl Default for Reference {
    fn default() -> Self {
        Self::a440()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuningsystem::{C4, MEANTONE};

    fn hz(name: &str, tuning: &impl Tuning) -> FloatType {
        Pitch::from_name(name)
            .unwrap()
            .frequency_hz_tuned(tuning, &Reference::a440())
            .unwrap()
    }

    #[test]
    fn twelve_edo_agrees_with_the_plain_frequency() {
        let twelve = EqualDivision::octave(12).unwrap();
        for name in ["C4", "C#4", "D-4", "B#3", "F##5", "E--2", "A4"] {
            let plain = Pitch::from_name(name).unwrap().frequency_hz();
            assert!((hz(name, &twelve) - plain).abs() < 1e-9, "{name}");
        }
        assert!((hz("C4", &twelve) - C4).abs() < 1e-9);
    }

    #[test]
    fn a_tuning_keeps_enharmonics_apart_where_it_should() {
        let nineteen = EqualDivision::octave(19).unwrap();
        assert!(hz("C#4", &nineteen) < hz("D-4", &nineteen));
        // A Pythagorean sharp is higher than the flat above it.
        assert!(hz("C#4", &Just) > hz("D-4", &Just));
        let meantone = MEANTONE.temperament().unwrap();
        assert!(hz("C#4", &meantone) < hz("D-4", &meantone));
    }

    #[test]
    fn just_is_pythagorean_from_the_reference() {
        let e5 = hz("E5", &Just);
        assert!((e5 - 660.0).abs() < 1e-9);
        let c4 = hz("C4", &Just);
        assert!((c4 - 440.0 * 16.0 / 27.0).abs() < 1e-9);
    }

    #[test]
    fn a_microtone_is_added_on_top() {
        let mut pitch = Pitch::from_name("A4").unwrap();
        pitch.set_microtone_cents(-14.0).unwrap();
        let tuned = pitch.frequency_hz_tuned(&Just, &Reference::a440()).unwrap();
        assert!((tuned - 440.0 * (2.0 as FloatType).powf(-14.0 / 1200.0)).abs() < 1e-9);
    }

    #[test]
    fn a_reference_needs_a_real_frequency() {
        let a4 = Pitch::from_name("A4").unwrap();
        assert!(Reference::new(a4.clone(), 0.0).is_err());
        assert!(Reference::new(a4, FloatType::NAN).is_err());
    }
}
