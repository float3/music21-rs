use super::{IntegerType, convert_harmonic_to_cents};

use crate::common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait};
use crate::defaults::FloatType;
use crate::error::{Error, Result};
use crate::prebase::{ProtoM21Object, ProtoM21ObjectTrait};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

const MICROTONE_OPEN: &str = "(";
const MICROTONE_CLOSE: &str = ")";

/// Input accepted by [`Microtone::new`] and [`Microtone::with_harmonic_shift`].
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MicrotoneSpecifier {
    /// A cent offset from the notated pitch.
    Cents(FloatType),
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
    pub fn new(specifier: impl Into<MicrotoneSpecifier>) -> Result<Self> {
        match specifier.into() {
            MicrotoneSpecifier::Microtone(microtone) => Ok(microtone),
            specifier => Self::with_harmonic_shift(specifier, 1),
        }
    }

    /// Creates a microtone with an explicit harmonic shift.
    pub fn with_harmonic_shift(
        specifier: impl Into<MicrotoneSpecifier>,
        harmonic_shift: IntegerType,
    ) -> Result<Self> {
        match specifier.into() {
            MicrotoneSpecifier::Cents(cents) => {
                Self::from_cent_shift(Some(cents), Some(harmonic_shift))
            }
            MicrotoneSpecifier::Text(text) => {
                Self::from_cent_shift(Some(Self::parse_string(text)?), Some(harmonic_shift))
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
    ) -> Result<Self>
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
    pub fn harmonic_shift(&self) -> IntegerType {
        self._harmonic_shift
    }

    /// Sets the harmonic shift.
    pub fn set_harmonic_shift(&mut self, harmonic_shift: IntegerType) {
        self._harmonic_shift = harmonic_shift;
    }

    fn parse_string(value: String) -> Result<FloatType> {
        let value = value.replace(MICROTONE_OPEN, "");
        let value = value.replace(MICROTONE_CLOSE, "");
        let first = match value.chars().next() {
            Some(first) => first,
            None => {
                return Err(Error::Microtone(format!(
                    "input to Microtone was empty: {value}"
                )));
            }
        };

        let cent_value = if first == '+' || first.is_ascii_digit() {
            let (num, _) = crate::common::stringtools::get_num_from_str(&value, "0123456789.");
            if num.is_empty() {
                return Err(Error::Microtone(format!(
                    "no numbers found in string value: {value}"
                )));
            }
            num.parse::<FloatType>()
                .map_err(|e| Error::Microtone(e.to_string()))?
        } else if first == '-' {
            let trimmed: String = value.chars().skip(1).collect();
            let (num, _) = crate::common::stringtools::get_num_from_str(&trimmed, "0123456789.");
            if num.is_empty() {
                return Err(Error::Microtone(format!(
                    "no numbers found in string value: {value}"
                )));
            }
            let parsed = num
                .parse::<FloatType>()
                .map_err(|e| Error::Microtone(e.to_string()))?;
            -parsed
        } else {
            0.0
        };
        Ok(cent_value)
    }
}

impl Display for Microtone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let rounded = self._cent_shift.round() as IntegerType;
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

impl FromStr for Microtone {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Microtone {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<String> for Microtone {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<IntegerType> for Microtone {
    type Error = Error;

    fn try_from(value: IntegerType) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<FloatType> for Microtone {
    type Error = Error;

    fn try_from(value: FloatType) -> Result<Self> {
        Self::new(value)
    }
}

fn ordinal_suffix(value: IntegerType) -> &'static str {
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
    fn into_microtone(self) -> Result<Microtone>;
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

    fn into_microtone(self) -> Result<Microtone> {
        Microtone::new(self)
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

    fn into_microtone(self) -> Result<Microtone> {
        Microtone::new(self)
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

    fn into_microtone(self) -> Result<Microtone> {
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

    fn into_microtone(self) -> Result<Microtone> {
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

    fn into_microtone(self) -> Result<Microtone> {
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

    fn into_microtone(self) -> Result<Microtone> {
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
    use super::{IntoCentShift, Microtone, MicrotoneSpecifier};

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

    #[test]
    fn microtone_supports_rust_conversion_traits_and_errors() {
        let parsed: Microtone = "(+12c)".parse().unwrap();
        assert_eq!(parsed.cent_shift(), 12.0);

        let from_cents = Microtone::try_from(-25.0).unwrap();
        assert_eq!(from_cents.to_string(), "(-25c)");

        assert!(Microtone::try_from("+c").is_err());
    }

    #[test]
    fn microtone_parser_covers_text_edge_cases() {
        assert_eq!(Microtone::try_from("nonsense").unwrap().cent_shift(), 0.0);
        assert!(Microtone::try_from("").is_err());
        assert!(Microtone::try_from("-c").is_err());

        assert_eq!("not-a-cent-value".into_cent_shift(), 0.0);
        assert_eq!("+19.5c".to_string().into_cent_shift(), 19.5);
    }

    #[test]
    fn microtone_setters_and_harmonic_suffixes_work() {
        let mut microtone = Microtone::new(MicrotoneSpecifier::Cents(0.0)).unwrap();
        microtone.set_cent_shift(-0.4);
        assert_eq!(microtone.to_string(), "(-0c)");

        microtone.set_harmonic_shift(11);
        assert_eq!(microtone.harmonic_shift(), 11);
        assert_eq!(microtone.to_string(), "(-0c+11thH)");

        microtone.set_harmonic_shift(-2);
        assert_eq!(microtone.to_string(), "(-0c+-2ndH)");
    }

    #[test]
    fn microtone_specifier_reports_wrapped_microtones() {
        let wrapped = MicrotoneSpecifier::from(Microtone::new(7).unwrap());
        assert!(wrapped.is_microtone());
        assert_eq!(wrapped.clone().into_cent_shift(), 7.0);
        assert_eq!(wrapped.microtone().cent_shift(), 7.0);

        assert!(!MicrotoneSpecifier::from(7).is_microtone());
        assert_eq!(25.into_microtone().unwrap().cent_shift(), 25.0);
        assert_eq!((-12.5).into_microtone().unwrap().cent_shift(), -12.5);
    }
}
