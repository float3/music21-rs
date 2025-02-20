use super::{IntegerType, convert_harmonic_to_cents};

use crate::common::objects::slottedobjectmixin::{SlottedObjectMixin, SlottedObjectMixinTrait};
use crate::defaults::FloatType;
use crate::exception::{Exception, ExceptionResult};
use crate::prebase::{ProtoM21Object, ProtoM21ObjectTrait};

const MICROTONE_OPEN: &str = "(";
const MICROTONE_CLOSE: &str = ")";

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Microtone {
    proto: ProtoM21Object,
    slottedobjectmixin: SlottedObjectMixin,
    _cent_shift: FloatType,
    _harmonic_shift: IntegerType,
}

impl Microtone {
    pub(crate) fn new<T>(
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

    pub(crate) fn alter(&self) -> FloatType {
        self.cents() * 0.01
    }

    fn cents(&self) -> FloatType {
        convert_harmonic_to_cents(self._harmonic_shift) as FloatType + self._cent_shift
    }

    fn parse_string(value: String) -> ExceptionResult<FloatType> {
        let value = value.replace(MICROTONE_OPEN, "");
        let value = value.replace(MICROTONE_CLOSE, "");
        let first = match value.chars().next() {
            Some(first) => first,
            None => {
                return Err(Exception::Microtone(format!(
                    "input to Microtone was empty: {}",
                    value
                )));
            }
        };

        let cent_value = if first == '+' || first.is_ascii_digit() {
            let (num, _) = crate::common::stringtools::get_num_from_str(&value, "0123456789.");
            if num.is_empty() {
                return Err(Exception::Microtone(format!(
                    "no numbers found in string value: {}",
                    value
                )));
            }
            num.parse::<FloatType>()
                .map_err(|e| Exception::Microtone(e.to_string()))?
        } else if first == '-' {
            let trimmed: String = value.chars().skip(1).collect();
            let (num, _) = crate::common::stringtools::get_num_from_str(&trimmed, "0123456789.");
            if num.is_empty() {
                return Err(Exception::Microtone(format!(
                    "no numbers found in string value: {}",
                    value
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
        todo!()
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::new(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for &str {
    fn into_cent_shift(self) -> FloatType {
        todo!()
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::new(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for IntegerType {
    fn into_cent_shift(self) -> FloatType {
        todo!()
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::new(Some(self), None)
    }

    fn microtone(self) -> Microtone {
        panic!("only call this on Microtones");
    }
}

impl IntoCentShift for FloatType {
    fn into_cent_shift(self) -> FloatType {
        todo!()
    }

    fn is_microtone(&self) -> bool {
        false
    }

    fn into_microtone(self) -> ExceptionResult<Microtone> {
        Microtone::new(Some(self), None)
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
