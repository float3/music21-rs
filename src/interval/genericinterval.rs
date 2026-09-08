//! The generic half of an interval: a signed staff distance in scale steps,
//! music21's `interval.GenericInterval`.

use std::fmt;

use crate::{
    common::numbertools::MUSICAL_ORDINAL_STRINGS,
    error::{Error, Result},
    key::KeySignature,
    pitch::{Accidental, Pitch},
};

use super::{
    IntegerType, diatonicinterval::DiatonicInterval, direction::Direction, specifier::Specifier,
};

/// A directed interval counted in scale steps: `3` is a third up, `-3` a
/// third down, `1` a unison. Zero is not an interval.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct GenericInterval {
    value: IntegerType,
}

impl GenericInterval {
    /// A generic interval of `value` steps, signed. Zero is an error, as
    /// music21 says: "The Zeroth is not an interval".
    pub fn new(value: IntegerType) -> Result<Self> {
        Self::from_int(value)
    }

    /// Parses a generic interval from music21's spellings: a number, an
    /// ordinal such as `"third"` or `"octave"`, optionally preceded by
    /// `"descending"` or `"ascending"`.
    pub fn from_name(name: &str) -> Result<Self> {
        let trimmed = name.trim();
        let (body, scale) = if let Some(rest) = strip_word_prefix(trimmed, "descending") {
            (rest, -1)
        } else if let Some(rest) = strip_word_prefix(trimmed, "ascending") {
            (rest, 1)
        } else {
            (trimmed, 1)
        };
        let body = body.trim();
        let lower = body.to_ascii_lowercase();
        let musical = MUSICAL_ORDINAL_STRINGS
            .iter()
            .position(|ordinal| ordinal.to_ascii_lowercase() == lower);
        let plain = PLAIN_ORDINALS.iter().position(|ordinal| *ordinal == lower);
        match musical.or(plain).or_else(|| numbered_ordinal(&lower)) {
            Some(index) if index > 0 => Self::from_int(index as IntegerType * scale),
            _ => Err(Error::Interval(format!(
                "Cannot convert '{name}' to an interval."
            ))),
        }
    }

    /// The signed step count: music21's `value` and `directed`.
    pub fn value(&self) -> IntegerType {
        self.value
    }

    /// The signed step count.
    pub fn directed(&self) -> IntegerType {
        self.value()
    }

    /// The step count without its sign.
    pub fn undirected(&self) -> IntegerType {
        self.value().abs()
    }

    /// Ascending, descending, or oblique for a unison.
    pub fn direction(&self) -> Direction {
        let directed = self.directed();
        if directed == 1 {
            Direction::Oblique
        } else if directed < 0 {
            Direction::Descending
        } else {
            Direction::Ascending
        }
    }

    /// The interval within one octave, keeping the sign, so a descending
    /// ninth is `-2` and any octave is `1`.
    pub fn simple_directed(&self) -> IntegerType {
        let simple_undirected = self.simple_undirected();
        if self.direction() == Direction::Descending && simple_undirected > 1 {
            -simple_undirected
        } else {
            simple_undirected
        }
    }

    /// The interval within one octave, without the sign.
    pub fn simple_undirected(&self) -> IntegerType {
        self.simple_steps_and_octaves().0
    }

    /// Like [`Self::simple_directed`], but an octave stays `8` (or `-8`)
    /// rather than folding to a unison.
    pub fn semi_simple_directed(&self) -> IntegerType {
        let semi_simple_undirected = self.semi_simple_undirected();
        if self.direction() == Direction::Descending && semi_simple_undirected > 1 {
            -semi_simple_undirected
        } else {
            semi_simple_undirected
        }
    }

    /// Like [`Self::simple_undirected`], but an octave stays `8`.
    pub fn semi_simple_undirected(&self) -> IntegerType {
        let simple_undirected = self.simple_undirected();
        if self.simple_steps_and_octaves().1 >= 1 && simple_undirected == 1 {
            8
        } else {
            simple_undirected
        }
    }

    /// How many whole octaves the interval spans, ignoring direction.
    pub fn undirected_octaves(&self) -> IntegerType {
        self.simple_steps_and_octaves().1
    }

    /// How many whole octaves the interval spans, negative when descending.
    pub fn octaves(&self) -> IntegerType {
        if self.direction() == Direction::Descending {
            -self.undirected_octaves()
        } else {
            self.undirected_octaves()
        }
    }

    /// The signed number of staff positions moved: a third is `2`, a
    /// descending third `-2`, a unison `0`.
    pub fn staff_distance(&self) -> IntegerType {
        let directed = self.directed();
        if directed > 0 {
            directed - 1
        } else {
            directed + 1
        }
    }

    /// The simple interval measured upward, so a descending third is `6`.
    pub fn mod7(&self) -> IntegerType {
        if self.direction() == Direction::Descending {
            self.mod7_inversion()
        } else {
            self.simple_undirected()
        }
    }

    /// The inversion of the simple interval: a third becomes a sixth.
    pub fn mod7_inversion(&self) -> IntegerType {
        9 - self.semi_simple_undirected()
    }

    /// Whether the simple interval is a unison, fourth or fifth, the ones
    /// that take perfect rather than major and minor qualities.
    pub fn is_perfectable(&self) -> bool {
        matches!(self.simple_undirected(), 1 | 4 | 5)
    }

    /// Whether the interval is a second in either direction.
    pub fn is_step(&self) -> bool {
        self.undirected() == 2
    }

    /// The same as [`Self::is_step`], as music21 defines `isDiatonicStep`
    /// on a generic interval.
    pub fn is_diatonic_step(&self) -> bool {
        self.is_step()
    }

    /// Whether the interval is larger than a second.
    pub fn is_skip(&self) -> bool {
        self.undirected() > 2
    }

    /// Whether the interval is a unison.
    pub fn is_unison(&self) -> bool {
        self.undirected() == 1
    }

    /// The spelled-out name, `Third`, `Octave`, `Fifteenth`, `23rd`.
    pub fn nice_name(&self) -> String {
        name_from_interval_number(self.undirected())
    }

    /// The spelled-out name of the simple interval.
    pub fn simple_nice_name(&self) -> String {
        name_from_interval_number(self.simple_undirected())
    }

    /// The spelled-out name of the semi-simple interval.
    pub fn semi_simple_nice_name(&self) -> String {
        name_from_interval_number(self.semi_simple_undirected())
    }

    /// The interval that completes the octave: a third's complement is a
    /// sixth, whatever the direction.
    pub fn complement(&self) -> Self {
        Self {
            value: self.mod7_inversion(),
        }
    }

    /// The same interval in the other direction; a unison stays a unison.
    pub fn reverse(&self) -> Self {
        if self.undirected() == 1 {
            Self { value: 1 }
        } else {
            Self {
                value: self.undirected() * -self.direction().as_int(),
            }
        }
    }

    /// Pairs the interval with a quality.
    pub fn get_diatonic(&self, spec: Specifier) -> DiatonicInterval {
        DiatonicInterval::new(spec, self)
    }

    /// Moves a pitch by the staff distance, keeping its accidental, as
    /// music21's generic `transposePitch` does: a third above `C#4` is
    /// `E#4`.
    pub fn transpose_pitch(&self, pitch: &Pitch) -> Result<Pitch> {
        self.transpose_pitch_key_aware(pitch, None)
    }

    /// Moves a pitch by the staff distance, taking the accidental from the
    /// key signature when one is given and the pitch had none of its own.
    pub fn transpose_pitch_key_aware(
        &self,
        pitch: &Pitch,
        key_signature: Option<&KeySignature>,
    ) -> Result<Pitch> {
        let mut out = pitch.clone();
        let had_octave = pitch.octave().is_some();
        let dnn = pitch.diatonic_note_number();
        out.set_diatonic_note_number(dnn + self.staff_distance())?;
        if let Some(key_signature) = key_signature {
            let step_alter = key_signature
                .accidental_by_step(pitch.step().as_char())?
                .map_or(0.0, |accidental| accidental.alter());
            let offset_from_key = pitch.accidental().alter() - step_alter;
            let new_step_alter = key_signature
                .accidental_by_step(out.step().as_char())?
                .map_or(0.0, |accidental| accidental.alter());
            let alter = new_step_alter + offset_from_key;
            let accidental = if alter == 0.0 {
                None
            } else {
                Some(Accidental::new(alter)?)
            };
            out.set_accidental_or_natural(accidental);
        }
        if !had_octave {
            out.octave_setter(None);
        }
        Ok(out)
    }

    pub(crate) fn from_int(value: IntegerType) -> Result<Self> {
        let mut slf = Self { value: 1 };
        slf.value_setter(convert_generic(value))?;
        Ok(slf)
    }

    fn value_setter(&mut self, value: IntegerType) -> Result<()> {
        if value == 0 {
            return Err(Error::Interval("The Zeroth is not an interval".to_owned()));
        }
        self.value = value;
        Ok(())
    }

    pub(crate) fn simple_steps_and_octaves(&self) -> (IntegerType, IntegerType) {
        let undirected = self.undirected();
        let mut octaves = undirected / 7;
        let mut steps = undirected % 7;
        if steps == 0 {
            octaves -= 1;
            steps = 7;
        }
        (steps, octaves)
    }
}

impl fmt::Display for GenericInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

const PLAIN_ORDINALS: [&str; 23] = [
    "zeroth",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
    "twentieth",
    "twenty-first",
    "twenty-second",
];

/// Reads `"1st"`, `"2nd"`, `"3rd"`, `"4th"` and so on; a bare number is
/// not an ordinal, as music21's `convertGeneric` also refuses `"1"`.
fn numbered_ordinal(value: &str) -> Option<usize> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let number: usize = digits.parse().ok()?;
    let expected = match number % 100 {
        11..=13 => "th",
        _ => match number % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    (&value[digits.len()..] == expected).then_some(number)
}

fn strip_word_prefix<'a>(value: &'a str, word: &str) -> Option<&'a str> {
    let head = value.get(..word.len())?;
    if head.eq_ignore_ascii_case(word) {
        Some(&value[word.len()..])
    } else {
        None
    }
}

fn name_from_interval_number(value: IntegerType) -> String {
    let value = value.unsigned_abs() as usize;
    if let Some(name) = MUSICAL_ORDINAL_STRINGS.get(value) {
        return name.clone();
    }
    let suffix = match value % 100 {
        11..=13 => "th",
        _ => match value % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{value}{suffix}")
}

/// music21's `convertGeneric` for a number: the number itself, since the
/// sign already carries the direction.
pub fn convert_generic(value: IntegerType) -> IntegerType {
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_interval_direction_and_simple_values() {
        let descending_ninth = GenericInterval::from_int(-9).unwrap();
        assert_eq!(descending_ninth.simple_undirected(), 2);
        assert_eq!(descending_ninth.simple_directed(), -2);
        assert!(matches!(
            descending_ninth.direction(),
            Direction::Descending
        ));
    }

    #[test]
    fn generic_interval_octave_and_unison_edges() {
        let octave = GenericInterval::from_int(8).unwrap();
        assert_eq!(octave.simple_undirected(), 1);
        assert_eq!(octave.semi_simple_undirected(), 8);
        assert_eq!(octave.semi_simple_directed(), 8);
        assert_eq!(octave.undirected_octaves(), 1);
        assert_eq!(GenericInterval::from_int(-15).unwrap().octaves(), -2);
        assert_eq!(
            GenericInterval::from_int(-9)
                .unwrap()
                .semi_simple_directed(),
            -2
        );
        let unison = GenericInterval::from_int(1).unwrap();
        assert!(unison.is_unison());
        assert_eq!(unison.direction(), Direction::Oblique);
        assert_eq!(unison.staff_distance(), 0);
        assert!(GenericInterval::from_int(0).is_err());
    }

    #[test]
    fn generic_interval_mod7_and_complement() {
        let descending_third = GenericInterval::from_int(-3).unwrap();
        assert_eq!(descending_third.mod7(), 6);
        assert_eq!(descending_third.mod7_inversion(), 6);
        assert_eq!(descending_third.complement().value(), 6);
        assert_eq!(descending_third.reverse().value(), 3);
        assert_eq!(GenericInterval::from_int(1).unwrap().reverse().value(), 1);
        assert!(GenericInterval::from_int(2).unwrap().is_step());
        assert!(GenericInterval::from_int(-2).unwrap().is_diatonic_step());
        assert!(GenericInterval::from_int(4).unwrap().is_skip());
        assert!(GenericInterval::from_int(5).unwrap().is_perfectable());
        assert!(!GenericInterval::from_int(10).unwrap().is_perfectable());
    }

    #[test]
    fn generic_interval_names() {
        assert_eq!(GenericInterval::from_int(3).unwrap().nice_name(), "Third");
        assert_eq!(
            GenericInterval::from_int(10).unwrap().simple_nice_name(),
            "Third"
        );
        assert_eq!(
            GenericInterval::from_int(15)
                .unwrap()
                .semi_simple_nice_name(),
            "Octave"
        );
        assert_eq!(GenericInterval::from_int(23).unwrap().nice_name(), "23rd");
        assert_eq!(GenericInterval::from_int(-3).unwrap().to_string(), "-3");
    }

    #[test]
    fn generic_interval_from_name() {
        assert_eq!(GenericInterval::from_name("Third").unwrap().value(), 3);
        assert_eq!(
            GenericInterval::from_name("descending fifth")
                .unwrap()
                .value(),
            -5
        );
        assert_eq!(GenericInterval::from_name("octave").unwrap().value(), 8);
        assert_eq!(GenericInterval::from_name("3rd").unwrap().value(), 3);
        assert_eq!(
            GenericInterval::from_name("Descending 2nd")
                .unwrap()
                .value(),
            -2
        );
        assert!(GenericInterval::from_name("blah").is_err());
        assert!(GenericInterval::from_name("zeroth").is_err());
        assert!(GenericInterval::from_name("1").is_err());
        assert!(GenericInterval::from_name("3nd").is_err());
    }

    #[test]
    fn generic_transposition_keeps_the_accidental() {
        let c_sharp = Pitch::from_name("C#4").unwrap();
        let third = GenericInterval::from_int(3).unwrap();
        assert_eq!(
            third.transpose_pitch(&c_sharp).unwrap().name_with_octave(),
            "E#4"
        );
        let b_flat = Pitch::from_name("B-4").unwrap();
        assert_eq!(
            GenericInterval::from_int(-3)
                .unwrap()
                .transpose_pitch(&b_flat)
                .unwrap()
                .name_with_octave(),
            "G-4"
        );
        let no_octave = Pitch::from_name("C").unwrap();
        let up = third.transpose_pitch(&no_octave).unwrap();
        assert_eq!(up.name(), "E");
        assert!(up.octave().is_none());
    }

    #[test]
    fn key_aware_generic_transposition_reads_the_signature() {
        let a_major = KeySignature::new(3);
        let a = Pitch::from_name("A4").unwrap();
        let third = GenericInterval::from_int(3).unwrap();
        assert_eq!(
            third
                .transpose_pitch_key_aware(&a, Some(&a_major))
                .unwrap()
                .name_with_octave(),
            "C#5"
        );
        let c_sharp = Pitch::from_name("C#5").unwrap();
        assert_eq!(
            third
                .transpose_pitch_key_aware(&c_sharp, Some(&a_major))
                .unwrap()
                .name_with_octave(),
            "E5"
        );
        let g_major = KeySignature::new(1);
        let step = GenericInterval::from_int(2).unwrap();
        let f_natural = Pitch::from_name("F4").unwrap();
        assert_eq!(
            step.transpose_pitch_key_aware(&f_natural, Some(&g_major))
                .unwrap()
                .name_with_octave(),
            "G-4"
        );
        let e = Pitch::from_name("E4").unwrap();
        assert_eq!(
            step.transpose_pitch_key_aware(&e, Some(&g_major))
                .unwrap()
                .name_with_octave(),
            "F#4"
        );
    }
}
