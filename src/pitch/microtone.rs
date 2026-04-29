use super::{IntegerType, convert_harmonic_to_cents};

use crate::common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait};
use crate::defaults::FloatType;
use crate::exception::{Exception, ExceptionResult};
use crate::prebase::{ProtoM21Object, ProtoM21ObjectTrait};
use std::fmt::{Display, Formatter};

const MICROTONE_OPEN: &str = "(";
const MICROTONE_CLOSE: &str = ")";

/// Input accepted by [`Microtone::new`] and [`Microtone::with_harmonic_shift`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MicrotoneSpecifier {
    /// A cent offset from the notated pitch.
    Cents(f64),
    /// A textual cent offset such as `"+20c"` or `"(-33.333)"`.
    Text(String),
    /// An existing microtone to clone.
    Microtone(Microtone),
}

impl From<&str> for MicrotoneSpecifier {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for MicrotoneSpecifier {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<IntegerType> for MicrotoneSpecifier {
    fn from(value: IntegerType) -> Self {
        Self::Cents(value as FloatType)
    }
}

impl From<FloatType> for MicrotoneSpecifier {
    fn from(value: FloatType) -> Self {
        Self::Cents(value)
    }
}

impl From<Microtone> for MicrotoneSpecifier {
    fn from(value: Microtone) -> Self {
        Self::Microtone(value)
    }
}

impl Display for MicrotoneSpecifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cents(cents) => write!(f, "{cents}"),
            Self::Text(text) => write!(f, "{text}"),
            Self::Microtone(microtone) => write!(f, "{microtone}"),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// A microtonal pitch adjustment measured in cents, optionally shifted by a
/// harmonic like Python music21's `music21.pitch.Microtone`.
pub struct Microtone {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
    _cent_shift: FloatType,
    _harmonic_shift: IntegerType,
}

impl Microtone {
    /// Creates a microtone with no harmonic shift.
    pub fn new(specifier: impl Into<MicrotoneSpecifier>) -> ExceptionResult<Self> {
        match specifier.into() {
            MicrotoneSpecifier::Microtone(microtone) => Ok(microtone),
            specifier => Self::with_harmonic_shift(specifier, 1),
        }
    }

    /// Creates a microtone with an explicit harmonic shift.
    pub fn with_harmonic_shift(
        specifier: impl Into<MicrotoneSpecifier>,
        harmonic_shift: i32,
    ) -> ExceptionResult<Self> {
        match specifier.into() {
            MicrotoneSpecifier::Cents(cents) => {
                Self::from_cent_shift(Some(cents), Some(harmonic_shift))
            }
            MicrotoneSpecifier::Text(text) => {
                Self::from_cent_shift(Some(text), Some(harmonic_shift))
            }
            MicrotoneSpecifier::Microtone(mut microtone) => {
                microtone._harmonic_shift = harmonic_shift;
                Ok(microtone)
            }
        }
    }

    pub(crate) fn from_cent_shift<T>(
        cents_or_string: Option<T>,
        harmonic_shift: Option<IntegerType>,
    ) -> ExceptionResult<Self>
    where
        T: IntoCentShift,
    {
        let _harmonic_shift = harmonic_shift.unwrap_or(1);

        let _cent_shift = match cents_or_string {
            Some(cents_or_string) => cents_or_string.into_cent_shift(),
            None => 0.0,
        };

        Ok(Self {
            proto: ProtoM21Object::new(),
            slottedobjectmixin: SlottedObjectMixin::new(),
            _cent_shift,
            _harmonic_shift,
        })
    }

    /// Returns the microtone in accidental alter units, where 100 cents is 1.0.
    pub fn alter(&self) -> FloatType {
        self.cents() * 0.01
    }

    /// Returns the total cent displacement, including harmonic shift.
    pub fn cents(&self) -> FloatType {
        convert_harmonic_to_cents(self._harmonic_shift) as FloatType + self._cent_shift
    }

    /// Returns the direct cent shift before harmonic adjustment.
    pub fn cent_shift(&self) -> FloatType {
        self._cent_shift
    }

    /// Sets the direct cent shift before harmonic adjustment.
    pub fn set_cent_shift(&mut self, cents: FloatType) {
        self._cent_shift = cents;
    }

    /// Returns the harmonic shift.
    pub fn harmonic_shift(&self) -> i32 {
        self._harmonic_shift
    }

    /// Sets the harmonic shift.
    pub fn set_harmonic_shift(&mut self, harmonic_shift: i32) {
        self._harmonic_shift = harmonic_shift;
    }

    fn parse_string(value: String) -> ExceptionResult<FloatType> {
        let value = value.replace(MICROTONE_OPEN, "");
        let value = value.replace(MICROTONE_CLOSE, "");
        let first = match value.chars().next() {
            Some(first) => first,
            None => {
                return Err(Exception::Microtone(format!(
                    "input to Microtone was empty: {value}"
                )));
            }
        };

        let cent_value = if first == '+' || first.is_ascii_digit() {
            let (num, _) = crate::common::stringtools::get_num_from_str(&value, "0123456789.");
            if num.is_empty() {
                return Err(Exception::Microtone(format!(
                    "no numbers found in string value: {value}"
                )));
            }
            num.parse::<FloatType>()
                .map_err(|e| Exception::Microtone(e.to_string()))?
        } else if first == '-' {
            let trimmed: String = value.chars().skip(1).collect();
            let (num, _) = crate::common::stringtools::get_num_from_str(&trimmed, "0123456789.");
            if num.is_empty() {
                return Err(Exception::Microtone(format!(
                    "no numbers found in string value: {value}"
                )));
            }
            let parsed = num
                .parse::<FloatType>()
                .map_err(|e| Exception::Microtone(e.to_string()))?;
            -parsed
        } else {
            0.0
        };
        Ok(cent_value)
    }
}

impl Display for Microtone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let rounded = self._cent_shift.round() as i32;
        let mut text = if self._cent_shift >= 0.0 {
            format!("+{rounded}c")
        } else {
            let text = format!("{rounded}c");
            if text == "0c" {
                "-0c".to_string()
            } else {
                text
            }
        };

        if self._harmonic_shift != 1 {
            text.push_str(&format!(
                "+{}{}H",
                self._harmonic_shift,
                ordinal_suffix(self._harmonic_shift)
            ));
        }

        write!(f, "{MICROTONE_OPEN}{text}{MICROTONE_CLOSE}")
    }
}

fn ordinal_suffix(value: i32) -> &'static str {
    if (value % 100).abs() >= 11 && (value % 100).abs() <= 13 {
        return "th";
    }

    match value.abs() % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

impl PartialEq for Microtone {
    fn eq(&self, other: &Self) -> bool {
        self.cents() == other.cents()
    }
}

impl ProtoM21ObjectTrait for Microtone {}

impl SlottedObjectMixinTrait for Microtone {}

pub(crate) trait IntoCentShift {
    fn into_cent_shift(self) -> FloatType;
    fn is_microtone(&self) -> bool;
    /// tries to construct a microtone.
    ///
    /// # Panics
    ///
    /// This method assumes that `is_microtone()` is `false`.
    /// Calling this method when `is_microtone()` is `true` will panic.
    fn into_microtone(self) -> ExceptionResult<Microtone>;
    /// Returns the contained microtone.
    ///
    /// # Panics
    ///
    /// This method assumes that `is_microtone()` is `true`.
    /// Calling this method when `is_microtone()` is `false` will panic.
    fn microtone(self) -> Microtone;
}

impl IntoCentShift for String {
    fn into_cent_shift(self) -> FloatType {
        Microtone::parse_string(self).unwrap_or(0.0)
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::from_cent_shift(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for &str {
    fn into_cent_shift(self) -> FloatType {
        Microtone::parse_string(self.to_string()).unwrap_or(0.0)
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::from_cent_shift(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for IntegerType {
    fn into_cent_shift(self) -> FloatType {
        self as FloatType
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::from_cent_shift(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for FloatType {
    fn into_cent_shift(self) -> FloatType {
        self
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::from_cent_shift(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for Microtone {
    fn into_cent_shift(self) -> FloatType {
        panic!("don't call this on Microtones");
    }

    fn is_microtone(&self) -> bool {
        true
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        panic!("don't call this on Microtones");
    }

    fn microtone(self) -> Microtone {
        self
    }
}

impl IntoCentShift for MicrotoneSpecifier {
    fn into_cent_shift(self) -> FloatType {
        match self {
            MicrotoneSpecifier::Cents(cents) => cents,
            MicrotoneSpecifier::Text(text) => text.into_cent_shift(),
            MicrotoneSpecifier::Microtone(microtone) => microtone.cents(),
        }
    }

    fn is_microtone(&self) -> bool {
        matches!(self, MicrotoneSpecifier::Microtone(_))
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::new(self)
    }

    fn microtone(self) -> Microtone {
        match self {
            MicrotoneSpecifier::Microtone(microtone) => microtone,
            _ => panic!("only call this on Microtones"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Microtone, MicrotoneSpecifier};

    #[test]
    fn public_microtone_api_matches_music21_basics() {
        let microtone = Microtone::new(20).unwrap();
        assert_eq!(microtone.cent_shift(), 20.0);
        assert_eq!(microtone.cents(), 20.0);
        assert_eq!(microtone.alter(), 0.2);
        assert_eq!(microtone.to_string(), "(+20c)");

        let parsed = Microtone::new("(-33.333333)").unwrap();
        assert!((parsed.cents() + 33.333333).abs() < 0.000001);
        assert_eq!(parsed.to_string(), "(-33c)");
    }

    #[test]
    fn harmonic_shift_contributes_to_cents_and_display() {
        let mut microtone = Microtone::new(20).unwrap();
        microtone.set_harmonic_shift(3);
        assert_eq!(microtone.harmonic_shift(), 3);
        assert_eq!(microtone.to_string(), "(+20c+3rdH)");
        assert!(microtone.cents() > 1900.0);
    }

    #[test]
    fn microtone_specifier_can_wrap_existing_microtone() {
        let microtone = Microtone::with_harmonic_shift(12.5, 5).unwrap();
        let clone = Microtone::new(MicrotoneSpecifier::from(microtone.clone())).unwrap();
        assert_eq!(clone, microtone);
    }
}
