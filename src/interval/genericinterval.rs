use crate::{
    common::numbertools::MUSICAL_ORDINAL_STRINGS,
    error::{Error, Result},
    pitch::Pitch,
};

use super::{
    IntegerType, IntervalBaseTrait, diatonicinterval::DiatonicInterval, direction::Direction,
    specifier::Specifier,
};

#[derive(Clone, Debug)]
pub(crate) struct GenericInterval {
    _value: IntegerType,
}

impl GenericInterval {
    pub(crate) fn simple_directed(&self) -> IntegerType {
        let simple_undirected = self.simple_undirected();
        if self.direction() == Direction::Descending && simple_undirected > 1 {
            -simple_undirected
        } else {
            simple_undirected
        }
    }

    pub(crate) fn from_int(value: IntegerType) -> Result<Self> {
        let mut slf = Self { _value: 1 };

        slf.value_setter(convert_generic(value))?;

        Ok(slf)
    }

    pub(crate) fn undirected(&self) -> IntegerType {
        self.value().abs()
    }

    pub(crate) fn direction(&self) -> Direction {
        let directed = self.directed();
        if directed == 1 {
            Direction::Oblique
        } else if directed < 0 {
            Direction::Descending
        } else {
            Direction::Ascending
        }
    }

    fn value_setter(&mut self, value: IntegerType) -> Result<()> {
        if value == 0 {
            return Err(Error::Interval("Interval cannot be zero".to_owned()));
        }
        self._value = value;
        Ok(())
    }

    pub(crate) fn get_diatonic(&self, spec: Specifier) -> DiatonicInterval {
        DiatonicInterval::new(spec, self)
    }

    pub(crate) fn staff_distance(&self) -> IntegerType {
        let directed = self.directed();
        if directed > 0 {
            directed - 1
        } else {
            directed + 1
        }
    }

    pub(crate) fn simple_undirected(&self) -> IntegerType {
        self.simple_steps_and_octaves().0
    }

    pub(crate) fn semi_simple_undirected(&self) -> IntegerType {
        let simple_undirected = self.simple_undirected();
        if self.simple_steps_and_octaves().1 >= 1 && simple_undirected == 1 {
            8
        } else {
            simple_undirected
        }
    }

    pub(crate) fn nice_name(&self) -> String {
        name_from_interval_number(self.undirected())
    }

    pub(crate) fn semi_simple_nice_name(&self) -> String {
        name_from_interval_number(self.semi_simple_undirected())
    }

    pub(crate) fn is_perfectable(&self) -> bool {
        matches!(self.simple_undirected(), 1 | 4 | 5)
    }

    fn directed(&self) -> IntegerType {
        self.value()
    }

    fn value(&self) -> IntegerType {
        self._value
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

fn convert_generic(value: IntegerType) -> IntegerType {
    let post = value;
    let direction_scalar = Direction::Ascending;
    post * direction_scalar as IntegerType
}

impl IntervalBaseTrait for GenericInterval {
    fn transpose_pitch(self, pitch1: Pitch) -> Result<Pitch> {
        let specifier = if self.is_perfectable() {
            Specifier::Perfect
        } else {
            Specifier::Major
        };
        let diatonic = self.get_diatonic(specifier);
        let chromatic = diatonic.get_chromatic()?;
        let interval = super::Interval::from_diatonic_and_chromatic(diatonic, chromatic)?;
        interval.transpose_pitch_with_options(&pitch1, false, Some(4))
    }

    fn reverse(self) -> Result<Self>
    where
        Self: Sized,
    {
        if self.undirected() == 1 {
            GenericInterval::from_int(1)
        } else {
            GenericInterval::from_int(self.undirected() * -self.direction().as_int())
        }
    }
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
    fn generic_interval_perfectable() {
        assert!(GenericInterval::from_int(1).unwrap().is_perfectable());
        assert!(GenericInterval::from_int(4).unwrap().is_perfectable());
        assert!(GenericInterval::from_int(12).unwrap().is_perfectable());
        assert!(!GenericInterval::from_int(3).unwrap().is_perfectable());
    }
}
