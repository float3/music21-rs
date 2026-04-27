use crate::{
    base::Music21ObjectTrait,
    common::{numbertools::MUSICAL_ORDINAL_STRINGS, stringtools::get_num_from_str},
    exception::{Exception, ExceptionResult},
    note::Note,
    pitch::Pitch,
    prebase::ProtoM21ObjectTrait,
};

use super::{
    IntegerType, IntervalBaseTrait, diatonicinterval::DiatonicInterval, direction::Direction,
    intervalbase::IntervalBase, specifier::Specifier,
};

#[derive(Clone, Debug)]
pub(crate) struct GenericInterval {
    pub(crate) intervalbase: IntervalBase,
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

    ///default value is "Unison"
    pub(crate) fn from_string(value: String) -> ExceptionResult<Self> {
        let mut slf = Self {
            intervalbase: IntervalBase::new(),
            _value: 1,
        };

        slf.value_setter(convert_generic_string(value))?;

        Ok(slf)
    }

    pub(crate) fn from_int(value: IntegerType) -> ExceptionResult<Self> {
        let mut slf = Self {
            intervalbase: IntervalBase::new(),
            _value: 1,
        };

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

    fn value_setter(&mut self, value: IntegerType) -> ExceptionResult<()> {
        if value == 0 {
            return Err(Exception::Interval("Interval cannot be zero".to_owned()));
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

fn convert_generic_string(value: String) -> IntegerType {
    let mut normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return 0;
    }

    let mut direction_scalar = 1;
    if normalized.contains("descending") {
        direction_scalar = -1;
        normalized = normalized.replace("descending", "").trim().to_string();
    } else if normalized.contains("ascending") {
        normalized = normalized.replace("ascending", "").trim().to_string();
    } else if normalized.starts_with('-') {
        direction_scalar = -1;
        normalized = normalized.trim_start_matches('-').trim().to_string();
    }

    if let Ok(number) = normalized.parse::<IntegerType>() {
        return number * direction_scalar;
    }

    for (idx, ordinal) in MUSICAL_ORDINAL_STRINGS.iter().enumerate() {
        if normalized == ordinal.to_lowercase() {
            return (idx as IntegerType) * direction_scalar;
        }
    }

    let (digits, remain) = get_num_from_str(&normalized, "0123456789");
    let remain = remain.trim().to_lowercase();
    if !digits.is_empty()
        && (remain.is_empty()
            || remain == "st"
            || remain == "nd"
            || remain == "rd"
            || remain == "th")
    {
        if let Ok(number) = digits.parse::<IntegerType>() {
            return number * direction_scalar;
        }
    }

    0
}

fn convert_generic(value: IntegerType) -> IntegerType {
    let post = value;
    let direction_scalar = Direction::Ascending;
    post * direction_scalar as IntegerType
}

impl IntervalBaseTrait for GenericInterval {
    fn transpose_note(self, note1: Note) -> ExceptionResult<Note> {
        let specifier = if self.is_perfectable() {
            Specifier::Perfect
        } else {
            Specifier::Major
        };
        let diatonic = self.get_diatonic(specifier);
        let chromatic = diatonic.get_chromatic()?;
        let interval = super::Interval::from_diatonic_and_chromatic(diatonic, chromatic)?;
        interval.transpose_note(note1)
    }

    fn transpose_pitch(self, pitch1: Pitch) -> ExceptionResult<Pitch> {
        let specifier = if self.is_perfectable() {
            Specifier::Perfect
        } else {
            Specifier::Major
        };
        let diatonic = self.get_diatonic(specifier);
        let chromatic = diatonic.get_chromatic()?;
        let interval = super::Interval::from_diatonic_and_chromatic(diatonic, chromatic)?;
        interval.transpose_pitch(&pitch1, false, Some(4))
    }

    fn transpose_pitch_in_place(self, pitch1: &mut Pitch) -> ExceptionResult<()> {
        let transposed = self.transpose_pitch(pitch1.clone())?;
        *pitch1 = transposed;
        Ok(())
    }

    fn reverse(self) -> ExceptionResult<Self>
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

impl Music21ObjectTrait for GenericInterval {}

impl ProtoM21ObjectTrait for GenericInterval {}

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

    #[test]
    fn generic_interval_from_string() {
        assert_eq!(
            GenericInterval::from_string("Descending Twelfth".to_string())
                .unwrap()
                .staff_distance(),
            -11
        );
        assert!(GenericInterval::from_string("not-an-interval".to_string()).is_err());
    }
}
