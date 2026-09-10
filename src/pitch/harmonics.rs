//! A pitch as a harmonic of a fundamental, and the microtone that
//! separates the two readings of a quarter tone.

use super::*;

impl Pitch {
    /// Returns the pitch with any quarter-tone accidental folded into the
    /// microtone: music21's `convertQuarterTonesToMicrotones`, so a half-sharp
    /// C becomes C with `+50c` and a one-and-a-half-sharp D becomes D-sharp
    /// with `+50c`. Other accidentals are untouched.
    pub fn convert_quarter_tones_to_microtones(&self) -> Result<Pitch> {
        let (alter, shift) = match self.accidental.name() {
            "half-flat" => (0.0, -50.0),
            "half-sharp" => (0.0, 50.0),
            "one-and-a-half-sharp" => (1.0, 50.0),
            "one-and-a-half-flat" => (-1.0, -50.0),
            _ => return Ok(self.clone()),
        };
        let cents = self.microtone.as_ref().map_or(0.0, Microtone::cents);
        let mut pitch = self.clone();
        pitch.accidental_setter(Accidental::new(alter)?);
        pitch.microtone_setter(Microtone::new(cents + shift)?);
        Ok(pitch)
    }

    /// Returns the pitch with its microtone rounded into the nearest
    /// quarter-tone accidental and the remainder kept as a microtone:
    /// music21's `convertMicrotonesToQuarterTones`, so C with `+30c` becomes a
    /// half-sharp C with `-20c` and C with `+150c` becomes C-sharp with `+50c`.
    pub fn convert_microtones_to_quarter_tones(&self) -> Result<Pitch> {
        let cents = self.microtone.as_ref().map_or(0.0, Microtone::cents);
        let (shift, remainder) = cents_to_alter_and_cents(cents);
        let mut pitch = self.clone();
        pitch.accidental_setter(Accidental::new(self.accidental.alter() + shift)?);
        pitch.microtone_setter(Microtone::new(remainder)?);
        Ok(pitch)
    }

    /// Returns which harmonic of `fundamental` this pitch is closest to, and
    /// the fundamental retuned by the difference so that the harmonic lands
    /// exactly here: music21's `harmonicAndFundamentalFromPitch`, so `E5`
    /// over `C2` is the tenth harmonic of `C2` raised 14 cents.
    pub fn harmonic_and_fundamental_from_pitch(&self, fundamental: &Pitch) -> Result<(u32, Pitch)> {
        let (number, cents) = self.harmonic_from_fundamental(fundamental)?;
        let cents = -cents;
        let mut retuned = fundamental.clone();
        match retuned.microtone.as_ref().map(Microtone::cents) {
            Some(existing) => retuned.microtone_setter(Microtone::new(existing + cents)?),
            None if cents != 0.0 => retuned.microtone_setter(Microtone::new(cents)?),
            None => {}
        }
        Ok((number, retuned))
    }

    /// Returns [`Self::harmonic_and_fundamental_from_pitch`] in music21's
    /// notation, `10thH/C2(+14c)`.
    pub fn harmonic_and_fundamental_string_from_pitch(
        &self,
        fundamental: &Pitch,
    ) -> Result<String> {
        let (number, retuned) = self.harmonic_and_fundamental_from_pitch(fundamental)?;
        let suffix = crate::pitch::microtone::ordinal_suffix(number as IntegerType);
        Ok(format!(
            "{number}{suffix}H/{}",
            retuned.name_with_octave_and_microtone()
        ))
    }

    /// Returns the `number`th harmonic of this pitch as a fundamental, spelled
    /// to the nearest twelve-tone pitch with the remainder as a microtone and
    /// this pitch recorded as its fundamental. The first harmonic is the
    /// pitch itself.
    pub fn harmonic(&self, number: u32) -> Result<Pitch> {
        let cent_shift = convert_harmonic_to_cents(number as IntegerType);
        if cent_shift == 0 {
            return Ok(self.clone());
        }
        let mut shifted = self.clone();
        let existing = self.microtone.as_ref().map_or(0.0, Microtone::cents);
        shifted.microtone_setter(Microtone::new(existing + cent_shift as FloatType)?);
        let mut harmonic = Pitch::from_frequency(shifted.frequency_hz())?;
        harmonic.fundamental_setter(self.clone());
        Ok(harmonic)
    }

    /// Returns which harmonic of `fundamental` this pitch is closest to, and
    /// the distance to that harmonic in cents: negative when this pitch lies
    /// above the harmonic, positive when below.
    pub fn harmonic_from_fundamental(&self, fundamental: &Pitch) -> Result<(u32, FloatType)> {
        if self.ps() <= fundamental.ps() {
            return Err(Error::Pitch(format!(
                "cannot find an equivalent harmonic for a fundamental ({fundamental}) that is not above this Pitch ({self})"
            )));
        }
        let mut found = Vec::new();
        for number in 1..32 {
            let candidate = fundamental.harmonic(number)?;
            let above = candidate.ps() > self.ps();
            found.push((number, candidate));
            if above {
                break;
            }
        }
        let (number, gap) = match found.as_slice() {
            [(number, only)] => (*number, only.ps() - self.ps()),
            [.., (lower_number, lower), (higher_number, higher)] => {
                let below = self.ps() - lower.ps();
                let above = higher.ps() - self.ps();
                if below <= above {
                    (*lower_number, -below.abs())
                } else {
                    (*higher_number, above.abs())
                }
            }
            [] => unreachable!("the first harmonic is always collected"),
        };
        Ok((
            number,
            round_to_digits(gap, PITCH_SPACE_SIGNIFICANT_DIGITS) * 100.0,
        ))
    }

    /// Describes this pitch as a harmonic of `fundamental`, or of its own
    /// fundamental when none is given, in music21's notation: `"3rdH(-2c)/C2"`.
    pub fn harmonic_string(&self, fundamental: Option<&Pitch>) -> Result<String> {
        let fundamental = fundamental.or(self.fundamental()).ok_or_else(|| {
            Error::Pitch("no fundamental is defined for this Pitch: provide one".to_string())
        })?;
        let (number, cents) = self.harmonic_from_fundamental(fundamental)?;
        let suffix = crate::pitch::microtone::ordinal_suffix(number as IntegerType);
        let fundamental = fundamental.name_with_octave_and_microtone();
        if cents == 0.0 {
            Ok(format!("{number}{suffix}H/{fundamental}"))
        } else {
            let microtone = Microtone::new(-cents)?;
            Ok(format!("{number}{suffix}H{microtone}/{fundamental}"))
        }
    }
}

pub(super) fn convert_harmonic_to_cents(harmonic_shift: IntegerType) -> IntegerType {
    let mut value = harmonic_shift as FloatType;
    if value < 0.0 {
        value = 1.0 / value.abs();
    }
    (1200.0 * value.log2()).round() as IntegerType
}

/// music21's `_convertCentsToAlterAndCents`: how much of a cent shift becomes
/// an accidental, in quarter-tone steps, and what is left as a microtone.
/// Ported as written, including the loop upstream runs for shifts below
/// -150 cents, which adds whole tones until the value passes +100 rather than
/// stopping at zero; nothing here relies on that range.
pub(super) fn cents_to_alter_and_cents(shift: FloatType) -> (FloatType, FloatType) {
    let mut value = shift;
    let mut alter_add = 0.0;
    if value > 150.0 {
        let increment = (value / 100.0).floor();
        value -= increment * 100.0;
        alter_add += increment;
    } else if value < -150.0 {
        while value < 100.0 {
            value += 100.0;
            alter_add -= 1.0;
        }
    }
    let (alter_shift, cents) = if value < -75.0 {
        (-1.0, value + 100.0)
    } else if value < -25.0 {
        (-0.5, value + 50.0)
    } else if value <= 25.0 {
        (0.0, value)
    } else if value <= 75.0 {
        (0.5, value - 50.0)
    } else {
        (1.0, value - 100.0)
    };
    (alter_shift + alter_add, cents)
}
