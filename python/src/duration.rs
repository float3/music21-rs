//! music21's `duration.Duration` over `music21-rs`: the length a note or
//! chord sounds for, the written values and tuplets it is spelled with,
//! and the grace durations that are written and do not sound.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyTuple};

use music21_rs::{Duration as RsDuration, DurationType as RsDurationType, Tuplet as RsTuplet};

use crate::note::{NoteException, deep_copied_objects, note_error, true_false_or_none};

/// The names the `duration` facade replaces in `music21.duration`.
/// Only `Duration` itself. `Tuplet`, `DurationTuple` and the module
/// functions beside them are registered on the facade module but *not*
/// swapped into music21's: music21's own are richer than these and already
/// pass their docstrings, and replacing a working implementation with a
/// thinner one costs more than it gains.
pub const NAMES: &[&str] = &["Duration", "GraceDuration", "AppoggiaturaDuration"];

pyo3::create_exception!(
    music21_rs_facade,
    DurationException,
    crate::Music21Exception
);

/// A number written the way music21's `common.mixedNumeral` writes it: a
/// whole number where it is one, and a whole number and a fraction where the
/// two are both there.
pub(crate) fn mixed_numeral(py: Python<'_>, value: f64) -> PyResult<String> {
    let exact = py
        .import("fractions")?
        .getattr("Fraction")?
        .call1((value,))?
        .call_method1("limit_denominator", (65_535,))?;
    let numerator: i64 = exact.getattr("numerator")?.extract()?;
    let denominator: i64 = exact.getattr("denominator")?.extract()?;
    if denominator == 1 {
        return Ok(numerator.to_string());
    }
    let whole = numerator.div_euclid(denominator);
    let rest = numerator.rem_euclid(denominator);
    if whole == 0 {
        return Ok(format!("{rest}/{denominator}"));
    }
    Ok(format!("{whole} {rest}/{denominator}"))
}

/// A written value's name with its first letter capitalized, which is what
/// Python's `str.title` does to the one-word names these carry.
fn title_case(name: &str) -> String {
    let mut letters = name.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

error_into!(duration_error, DurationException);

/// One written note value inside a duration: music21's `DurationTuple`.
///
/// music21 makes this a `NamedTuple` of a type name, a dot count and the
/// quarter length the two come to. The name is kept rather than the crate's
/// `DurationType` so that the two music21 uses for a length it cannot write
/// as a note — `zero` and `inexpressible` — have somewhere to go.
#[pyclass(
    name = "DurationTuple",
    module = "music21.duration",
    subclass,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct DurationTuple {
    kind: String,
    dots: u32,
    quarter_length: f64,
}

impl DurationTuple {
    pub(crate) fn of(kind: RsDurationType, dots: u32) -> Self {
        Self {
            kind: kind.music21_name().to_string(),
            dots,
            quarter_length: kind.quarter_length_with_dots(dots),
        }
    }

    /// The written value of that name carrying that many dots, which is
    /// music21's `durationTupleFromTypeDots`. The two names for a length no
    /// note value writes carry no length of their own.
    pub(crate) fn from_type_dots(kind: &str, dots: u32) -> Self {
        let quarter_length = RsDurationType::from_music21_name(kind)
            .map_or(0.0, |kind| kind.quarter_length_with_dots(dots));
        Self {
            kind: kind.to_string(),
            dots,
            quarter_length,
        }
    }

    /// The tuple music21 reads a bare quarter length as: a note value with
    /// dots when one fits, `zero` for nothing, and `inexpressible` for a
    /// length no note value reaches.
    pub(crate) fn from_quarter_length(quarter_length: f64) -> Self {
        if quarter_length == 0.0 {
            return Self {
                kind: "zero".to_string(),
                dots: 0,
                quarter_length: 0.0,
            };
        }
        match RsDuration::new(quarter_length)
            .ok()
            .and_then(|duration| duration.type_and_dots())
        {
            Some((kind, dots)) => Self::of(kind, dots),
            None => Self {
                kind: "inexpressible".to_string(),
                dots: 0,
                quarter_length,
            },
        }
    }
}

#[pymethods]
impl DurationTuple {
    #[new]
    #[pyo3(signature = (type_name, dots, quarterLength))]
    fn new(type_name: String, dots: u32, quarterLength: f64) -> Self {
        Self {
            kind: type_name,
            dots,
            quarter_length: quarterLength,
        }
    }

    #[getter]
    fn r#type(&self) -> String {
        self.kind.clone()
    }

    #[getter]
    fn dots(&self) -> u32 {
        self.dots
    }

    /// music21 writes a length through `opFrac`, so a triplet quarter is
    /// `Fraction(2, 3)` and not a float that nearly is.
    #[getter]
    fn quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        op_frac(py, self.quarter_length)
    }

    /// music21's `ordinal`: where the note value sits in the list running
    /// from the duplex maxima down to the 2048th.
    #[getter]
    fn ordinal(&self) -> PyResult<usize> {
        RsDurationType::from_music21_name(&self.kind)
            .and_then(RsDurationType::ordinal)
            .ok_or_else(|| {
                DurationException::new_err(format!(
                    "Could not determine durationNumber from {}",
                    self.kind
                ))
            })
    }

    /// The same value scaled, read back as a written note value.
    fn augmentOrDiminish(&self, amountToScale: f64) -> Self {
        Self::from_quarter_length(self.quarter_length * amountToScale)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<PyRef<'_, Self>>().is_ok_and(|other| {
            other.kind == self.kind
                && other.dots == self.dots
                && other.quarter_length == self.quarter_length
        })
    }

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(&self) -> String {
        format!(
            "DurationTuple(type='{}', dots={}, quarterLength={:?})",
            self.kind, self.dots, self.quarter_length
        )
    }
}

/// music21's `duration.Tuplet`: so many notes written in the time of so
/// many others.
#[pyclass(
    name = "Tuplet",
    module = "music21.duration",
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct Tuplet {
    inner: RsTuplet,
    /// Where this tuplet's bracket sits over the notes: music21's `type`,
    /// `'start'` on the first note and `'stop'` on the last, `None` while
    /// nothing has said. It is engraving rather than rhythm, which is why it
    /// lives here and not in the crate — but music21's own `makeTupletBrackets`
    /// writes it on whatever tuplets it finds, ours included.
    bracket_type: Option<String>,
    /// Whether a bracket is drawn at all.
    bracket: bool,
    /// Which side of the notes the number goes.
    placement: Option<String>,
    /// Whether the two counts are shown, and how.
    tuplet_actual_show: Option<String>,
    tuplet_normal_show: Option<String>,
    /// How deep inside other tuplets this one is.
    nested_level: u32,
    /// An identifier for grouping tuplets that belong together.
    tuplet_id: i64,
    /// Whether the tuplet may still be changed. music21 freezes a tuplet as
    /// it goes onto a duration, so that the duration's length cannot go
    /// stale underneath it.
    frozen: bool,
}

impl Tuplet {
    pub(crate) fn wrap(inner: RsTuplet) -> Self {
        Self {
            inner,
            bracket_type: None,
            bracket: true,
            placement: Some("above".to_string()),
            tuplet_actual_show: Some("number".to_string()),
            // music21 shows the actual number and says nothing about the
            // normal one, which is why a plain triplet writes `3` alone.
            tuplet_normal_show: None,
            nested_level: 1,
            tuplet_id: 0,
            frozen: false,
        }
    }

    /// The written value each of the `actual` notes carries.
    fn set_actual_tuple(&mut self, written: &DurationTuple) {
        let Some(kind) = RsDurationType::from_music21_name(&written.r#type()) else {
            return;
        };
        self.inner = RsTuplet::new(
            self.inner.actual(),
            self.inner.normal(),
            kind,
            written.dots(),
        )
        .with_normal(self.inner.normal_duration_type(), self.inner.normal_dots());
    }

    /// The written value the `normal` count is counted in.
    fn set_normal_tuple(&mut self, written: &DurationTuple) {
        let Some(kind) = RsDurationType::from_music21_name(&written.r#type()) else {
            return;
        };
        self.inner = self.inner.with_normal(kind, written.dots());
    }

    /// Refuses a change to a tuplet that is already on a duration, as
    /// music21 refuses one: the duration's length was worked out from it.
    fn thaw(&self) -> PyResult<()> {
        if self.frozen {
            return Err(DurationException::new_err(
                "A frozen tuplet (or one attached to a duration) is immutable",
            ));
        }
        Ok(())
    }
}

#[pymethods]
impl Tuplet {
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec!["Tuplet", "ProtoM21Object", "object"]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> = vec!["Tuplet", "ProtoM21Object", "object"];
        Ok(pyo3::types::PyFrozenSet::new(py, &names)?
            .into_any()
            .unbind())
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let Some(inner) = crate::unpickled::<_, RsTuplet>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (numberNotesActual = 3, numberNotesNormal = 2, durationActual = None, durationNormal = None, **_keywords))]
    fn new(
        numberNotesActual: u32,
        numberNotesNormal: u32,
        durationActual: Option<&Bound<'_, PyAny>>,
        durationNormal: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let actual = match durationActual {
            Some(value) => duration_tuple_from_any(value)?,
            None => DurationTuple::of(RsDurationType::Eighth, 0),
        };
        let normal = match durationNormal {
            Some(value) => duration_tuple_from_any(value)?,
            None => actual.clone(),
        };
        let mut tuplet = Self::wrap(RsTuplet::new(
            numberNotesActual,
            numberNotesNormal,
            RsDurationType::Eighth,
            0,
        ));
        tuplet.set_actual_tuple(&actual);
        tuplet.set_normal_tuple(&normal);
        Ok(tuplet)
    }

    #[getter]
    fn get_numberNotesActual(&self) -> u32 {
        self.inner.actual()
    }

    #[setter]
    fn set_numberNotesActual(&mut self, value: u32) -> PyResult<()> {
        self.thaw()?;
        self.inner = RsTuplet::new(
            value,
            self.inner.normal(),
            self.inner.duration_type(),
            self.inner.dots(),
        )
        .with_normal(self.inner.normal_duration_type(), self.inner.normal_dots());
        Ok(())
    }

    #[getter]
    fn get_numberNotesNormal(&self) -> u32 {
        self.inner.normal()
    }

    #[setter]
    fn set_numberNotesNormal(&mut self, value: u32) -> PyResult<()> {
        self.thaw()?;
        self.inner = RsTuplet::new(
            self.inner.actual(),
            value,
            self.inner.duration_type(),
            self.inner.dots(),
        )
        .with_normal(self.inner.normal_duration_type(), self.inner.normal_dots());
        Ok(())
    }

    /// music21's `durationActual`: the written value each of the `actual`
    /// notes carries.
    #[getter]
    fn get_durationActual(&self) -> DurationTuple {
        DurationTuple::of(self.inner.duration_type(), self.inner.dots())
    }

    #[setter]
    fn set_durationActual(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.thaw()?;
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Ok(());
        };
        self.set_actual_tuple(&duration_tuple_from_any(value)?);
        Ok(())
    }

    /// music21's `durationNormal`: the written value the `normal` count is
    /// counted in, which need not be the same one.
    #[getter]
    fn get_durationNormal(&self) -> DurationTuple {
        DurationTuple::of(self.inner.normal_duration_type(), self.inner.normal_dots())
    }

    #[setter]
    fn set_durationNormal(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.thaw()?;
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Ok(());
        };
        self.set_normal_tuple(&duration_tuple_from_any(value)?);
        Ok(())
    }

    /// music21's `setRatio`: both counts at once.
    fn setRatio(&mut self, actual: u32, normal: u32) -> PyResult<()> {
        self.thaw()?;
        self.inner = RsTuplet::new(
            actual,
            normal,
            self.inner.duration_type(),
            self.inner.dots(),
        )
        .with_normal(self.inner.normal_duration_type(), self.inner.normal_dots());
        Ok(())
    }

    /// music21's `setDurationType`: both written values at once.
    #[pyo3(signature = (durType, dots = 0))]
    fn setDurationType(&mut self, durType: &Bound<'_, PyAny>, dots: u32) -> PyResult<()> {
        self.thaw()?;
        let name = match durType.extract::<String>() {
            Ok(name) => name,
            Err(_) => {
                let quarter_length = durType.extract::<f64>()?;
                DurationTuple::from_quarter_length(quarter_length).r#type()
            }
        };
        let kind = RsDurationType::from_music21_name(&name)
            .ok_or_else(|| DurationException::new_err(format!("no such duration type: {name}")))?;
        self.inner = RsTuplet::new(self.inner.actual(), self.inner.normal(), kind, dots)
            .with_normal(kind, dots);
        Ok(())
    }

    /// music21's `totalTupletLength`: how long the whole tuplet lasts.
    fn totalTupletLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        op_frac(py, self.inner.total_tuplet_length())
    }

    /// music21's `tupletMultiplier`, `normal / actual`, as a `Fraction`.
    fn tupletMultiplier<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let normal = self
            .inner
            .normal_duration_type()
            .quarter_length_with_dots(self.inner.normal_dots());
        let actual = self
            .inner
            .duration_type()
            .quarter_length_with_dots(self.inner.dots());
        // The ratio of the counts, scaled by the ratio of the two written
        // values when they are not the same one.
        let scale = if actual == 0.0 { 1.0 } else { normal / actual };
        if scale == 1.0 {
            return op_frac_ratio(py, self.inner.normal() as i64, self.inner.actual() as i64);
        }
        op_frac(
            py,
            f64::from(self.inner.normal()) * scale / f64::from(self.inner.actual()),
        )
    }

    /// music21's `augmentOrDiminish`: the same ratio over longer or shorter
    /// written values.
    fn augmentOrDiminish(&mut self, amountToScale: f64) -> PyResult<()> {
        self.thaw()?;
        if amountToScale <= 0.0 || amountToScale.is_nan() {
            return Err(PyValueError::new_err(
                "amountToScale must be greater than zero",
            ));
        }
        let actual = self.get_durationActual().augmentOrDiminish(amountToScale);
        let normal = self.get_durationNormal().augmentOrDiminish(amountToScale);
        self.set_actual_tuple(&actual);
        self.set_normal_tuple(&normal);
        Ok(())
    }

    /// music21's `type`: where this tuplet's bracket sits over the notes.
    #[getter]
    fn get_type(&self) -> Option<String> {
        self.bracket_type.clone()
    }

    #[setter]
    fn set_type(&mut self, value: Option<String>) -> PyResult<()> {
        if let Some(value) = &value
            && !matches!(value.as_str(), "start" | "stop" | "startStop")
        {
            return Err(DurationException::new_err(format!(
                "Type must be 'start', 'stop', 'startStop', or None, not {value}"
            )));
        }
        self.bracket_type = value;
        Ok(())
    }

    #[getter]
    fn get_bracket(&self) -> bool {
        self.bracket
    }

    #[setter]
    fn set_bracket(&mut self, value: bool) {
        self.bracket = value;
    }

    #[getter]
    fn get_placement(&self) -> Option<String> {
        self.placement.clone()
    }

    #[setter]
    fn set_placement(&mut self, value: Option<String>) {
        self.placement = value;
    }

    #[getter]
    fn get_tupletActualShow(&self) -> Option<String> {
        self.tuplet_actual_show.clone()
    }

    #[setter]
    fn set_tupletActualShow(&mut self, value: Option<String>) {
        self.tuplet_actual_show = value;
    }

    #[getter]
    fn get_tupletNormalShow(&self) -> Option<String> {
        self.tuplet_normal_show.clone()
    }

    #[setter]
    fn set_tupletNormalShow(&mut self, value: Option<String>) {
        self.tuplet_normal_show = value;
    }

    #[getter]
    fn get_nestedLevel(&self) -> u32 {
        self.nested_level
    }

    #[setter]
    fn set_nestedLevel(&mut self, value: u32) {
        self.nested_level = value;
    }

    #[getter]
    fn get_tupletId(&self) -> i64 {
        self.tuplet_id
    }

    #[setter]
    fn set_tupletId(&mut self, value: i64) {
        self.tuplet_id = value;
    }

    /// music21's `tupletActual`: the count and the written value together.
    #[getter]
    fn get_tupletActual(&self) -> (u32, DurationTuple) {
        (self.inner.actual(), self.get_durationActual())
    }

    #[setter]
    fn set_tupletActual(&mut self, value: (u32, Bound<'_, PyAny>)) -> PyResult<()> {
        self.set_numberNotesActual(value.0)?;
        self.set_durationActual(Some(&value.1))
    }

    #[getter]
    fn get_tupletNormal(&self) -> (u32, DurationTuple) {
        (self.inner.normal(), self.get_durationNormal())
    }

    #[setter]
    fn set_tupletNormal(&mut self, value: (u32, Bound<'_, PyAny>)) -> PyResult<()> {
        self.set_numberNotesNormal(value.0)?;
        self.set_durationNormal(Some(&value.1))
    }

    #[getter]
    fn get_frozen(&self) -> bool {
        self.frozen
    }

    #[setter]
    fn set_frozen(&mut self, value: bool) {
        self.frozen = value;
    }

    #[getter]
    fn fullName(&self) -> String {
        self.inner.full_name()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.duration.Tuplet {}/{}/{}>",
            self.inner.actual(),
            self.inner.normal(),
            self.inner.duration_type().music21_name()
        )
    }

    /// A copy as an object of the class it was asked on: music21 compares
    /// two of these by class before anything else, so a copy built as the
    /// bare facade would not equal the original.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `common.opFrac`: a quarter length as a float where one says
/// the value exactly, and as a `Fraction` where none does.
///
/// Every length in music21 goes through this, which is why a triplet quarter
/// prints as `Fraction(2, 3)` and a dotted one as `1.5`. A binary fraction is
/// exactly a float, so it stays one; anything else is snapped onto the
/// nearest fraction with a denominator music21 will keep.
pub(crate) fn op_frac<'py>(py: Python<'py>, value: f64) -> PyResult<Bound<'py, PyAny>> {
    let float = PyFloat::new(py, value).into_any();
    if !value.is_finite() {
        return Ok(float);
    }
    let ratio = float.call_method0("as_integer_ratio")?;
    let numerator = ratio.get_item(0)?;
    let denominator = ratio.get_item(1)?;
    let Ok(exact) = denominator.extract::<u64>() else {
        return Ok(float);
    };
    if exact <= OFFSET_DENOMINATOR_LIMIT {
        return Ok(float);
    }
    let limited = py
        .import("fractions")?
        .getattr("Fraction")?
        .call1((numerator, denominator))?
        .call_method1("limit_denominator", (OFFSET_DENOMINATOR_LIMIT,))?;
    let denominator = limited.getattr("denominator")?.extract::<u64>()?;
    if denominator & (denominator - 1) == 0 {
        let numerator = limited.getattr("numerator")?.extract::<f64>()?;
        return Ok(PyFloat::new(py, numerator / denominator as f64).into_any());
    }
    Ok(limited)
}

/// The same, for a ratio that is already exact — a tuplet multiplier, which
/// is never anything but a ratio of small whole numbers.
pub(crate) fn op_frac_ratio<'py>(
    py: Python<'py>,
    numerator: i64,
    denominator: i64,
) -> PyResult<Bound<'py, PyAny>> {
    if denominator != 0 && denominator.unsigned_abs() & (denominator.unsigned_abs() - 1) == 0 {
        return Ok(PyFloat::new(py, numerator as f64 / denominator as f64).into_any());
    }
    py.import("fractions")?
        .getattr("Fraction")?
        .call1((numerator, denominator))
}

/// music21's `defaults.limitOffsetDenominator`: the largest denominator it
/// will keep a quarter length as a fraction for.
const OFFSET_DENOMINATOR_LIMIT: u64 = 65535;

/// A quarter length written as a float, a whole number or a `Fraction`.
fn quarter_length_from_any(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(value) = value.extract::<f64>() {
        return Ok(value);
    }
    value
        .call_method0("__float__")
        .and_then(|value| value.extract::<f64>())
        .map_err(|_| {
            NoteException::new_err(format!(
                "cannot read a quarter length from {}",
                value
                    .repr()
                    .map(|value| value.to_string())
                    .unwrap_or_default()
            ))
        })
}

/// music21's `duration.Duration`, over the crate's quarter-length duration.
///
/// music21's duration is not one number: it is a list of written note values
/// tied together, a stack of tuplets those values are written inside, and a
/// group of dots written above them, with the sounding quarter length the
/// product of all three. The crate's is the sounding length, which reads the
/// written values back off itself. That round-trips for a length with one
/// spelling, so the written side is only stored once a caller has asked for
/// it or set it — which is also when music21 stops inferring it.
#[pyclass(
    name = "Duration",
    module = "music21.duration",
    subclass,
    skip_from_py_object
)]
pub struct Duration {
    pub(crate) inner: RsDuration,
    /// The written values, once they have been asked for or set.
    components: Option<Vec<DurationTuple>>,
    /// The tuplets those values are written inside, once set.
    ///
    /// They are held as Python objects rather than as the crate's tuplet
    /// because music21's own `Tuplet` is what a caller hands in — it is
    /// richer than this crate's and is deliberately not replaced — and
    /// because `appendTuplet` freezes the very object it was given.
    tuplets: Option<Vec<Py<PyAny>>>,
    /// music21's `_dotGroups`: dots written above dots, for medieval music.
    /// `[0]` is none of them, which is not the same as an empty list.
    dot_groups: Vec<u32>,
    /// Whether the written values and the sounding length move together.
    /// Unlinked, a grace note is written as a quarter and sounds nothing.
    linked: bool,
    /// The written value an unlinked duration says it has, since its length
    /// no longer implies one.
    unlinked_type: Option<String>,
    /// Whether the written values were worked out from the length rather
    /// than given: music21's `expressionIsInferred`, which decides both
    /// whether they may be worked out again and how two durations compare.
    expression_is_inferred: bool,
    /// The object this duration belongs to. music21's `GeneralNote.duration`
    /// setter writes itself here, and reads the failure to do so as "not a
    /// Duration at all", so keeping the slot is what lets music21's own
    /// classes take one of ours.
    pub(crate) client: Option<Py<PyAny>>,
}

/// music21's `informSites`, called on whatever holds a duration that has
/// just been replaced. A thing no stream holds has no sites and nothing to
/// tell.
pub(crate) fn told_sites(holder: &Bound<'_, PyAny>, length: &Bound<'_, PyAny>) -> PyResult<()> {
    if !holder.hasattr("informSites")? {
        return Ok(());
    }
    let message = pyo3::types::PyDict::new(holder.py());
    message.set_item("changedElement", "duration")?;
    message.set_item("quarterLength", length)?;
    holder.call_method1("informSites", (message,))?;
    Ok(())
}

/// Tells a duration what holds it, the way music21's own `duration` setter
/// does. A duration announces a change to its holder, and one that was never
/// told who that is announces to nobody.
pub(crate) fn adopt_duration(py: Python<'_>, duration: &Py<PyAny>, holder: &Bound<'_, PyAny>) {
    let duration = duration.bind(py);
    if let Ok(mut ours) = duration.extract::<PyRefMut<'_, Duration>>() {
        ours.client = Some(holder.clone().unbind());
        return;
    }
    // One of music21's own durations keeps the same slot, and writing it is
    // what its `GeneralNote.duration` setter does.
    let _ = duration.setattr("client", holder);
}

impl Clone for Duration {
    /// A copy of a duration belongs to nobody yet, as music21's does.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner.clone(),
            components: self.components.clone(),
            tuplets: self
                .tuplets
                .as_ref()
                .map(|tuplets| tuplets.iter().map(|one| one.clone_ref(py)).collect()),
            dot_groups: self.dot_groups.clone(),
            linked: self.linked,
            unlinked_type: self.unlinked_type.clone(),
            expression_is_inferred: self.expression_is_inferred,
            client: None,
        })
    }
}

/// What one group of dots multiplies a written value by: music21's
/// `dotMultiplier`, `(2^(n+1) - 1) / 2^n`.
fn dot_multiplier(dots: u32) -> f64 {
    let scale = 2f64.powi(i32::try_from(dots).unwrap_or(i32::MAX));
    (scale * 2.0 - 1.0) / scale
}

/// A ratio in lowest terms, so that multiplying tuplet ratios together does
/// not run the numbers up until they overflow.
fn reduce_ratio(numerator: i64, denominator: i64) -> (i64, i64) {
    let mut left = numerator.abs();
    let mut right = denominator.abs();
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    if left == 0 {
        return (numerator, denominator);
    }
    (numerator / left, denominator / left)
}

impl Duration {
    pub(crate) fn wrap(inner: RsDuration) -> Self {
        Self {
            inner,
            components: None,
            tuplets: None,
            dot_groups: vec![0],
            linked: true,
            unlinked_type: None,
            expression_is_inferred: true,
            client: None,
        }
    }

    /// The written values this length is made of: the ones set or already
    /// worked out, and otherwise the ones the crate reads off the length.
    fn component_list(&self) -> Vec<DurationTuple> {
        match &self.components {
            Some(components) => components.clone(),
            None => self.inferred_components(),
        }
    }

    /// The written values the crate reads off the sounding length.
    fn inferred_components(&self) -> Vec<DurationTuple> {
        let components = self.inner.components();
        // No written value covers this length, and it is not nothing: music21
        // writes one component saying so rather than none at all.
        if components.is_empty() && self.inner.quarter_length() != 0.0 {
            return vec![DurationTuple {
                kind: "inexpressible".to_string(),
                dots: 0,
                quarter_length: self.inner.quarter_length(),
            }];
        }
        components
            .into_iter()
            .map(|(kind, dots)| DurationTuple::of(kind, dots))
            .collect()
    }

    /// Works the written values out and keeps them, which is music21's
    /// `_updateComponents`: after this the duration stops inferring and
    /// answers with what it holds.
    fn materialize(&mut self, py: Python<'_>) -> PyResult<()> {
        if self.components.is_none() {
            self.components = Some(self.inferred_components());
        }
        if self.tuplets.is_none() {
            self.tuplets = Some(self.tuplet_objects(py)?);
        }
        Ok(())
    }

    /// The tuplet objects this duration is written inside.
    ///
    /// An inferred one is built as this crate's, since no caller handed one
    /// in; a set one is whatever was handed in.
    fn tuplet_objects(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        if let Some(tuplets) = &self.tuplets {
            return Ok(tuplets.iter().map(|one| one.clone_ref(py)).collect());
        }
        match self.inner.tuplet() {
            Some(tuplet) => Ok(vec![
                Tuplet::wrap(tuplet).into_pyobject(py)?.into_any().unbind(),
            ]),
            None => Ok(Vec::new()),
        }
    }

    /// Every tuplet's ratio multiplied together, in lowest terms.
    fn aggregate_ratio(&self, py: Python<'_>) -> PyResult<(i64, i64)> {
        let fraction = py.import("fractions")?.getattr("Fraction")?;
        let mut numerator: i64 = 1;
        let mut denominator: i64 = 1;
        for tuplet in self.tuplet_objects(py)? {
            // Each tuplet's own multiplier, not the ratio of its counts:
            // the two written values a tuplet names need not be the same
            // one, and a Humdrum `6..` is three dotted quarters in the time
            // of two, which is seven sixths and not two thirds.
            let multiplier =
                fraction.call1((tuplet.bind(py).call_method0("tupletMultiplier")?,))?;
            numerator *= multiplier.getattr("numerator")?.extract::<i64>()?;
            denominator *= multiplier.getattr("denominator")?.extract::<i64>()?;
            (numerator, denominator) = reduce_ratio(numerator, denominator);
        }
        Ok(reduce_ratio(numerator, denominator))
    }

    /// The total of the written values, before the tuplets shorten them.
    fn written_quarter_length(&self) -> f64 {
        // Folded from a positive zero on purpose: Rust sums floats from
        // `-0.0`, and an empty duration would then answer `-0.0`.
        self.component_list()
            .iter()
            .map(|component| component.quarter_length)
            .fold(0.0, |total, length| total + length)
    }

    /// Sets the sounding length from the written values, the tuplets and the
    /// dot groups: music21's `_updateQuarterLength`. An unlinked duration
    /// keeps whatever length it was given.
    fn relength(&mut self, py: Python<'_>) -> PyResult<()> {
        if !self.linked {
            return Ok(());
        }
        let was = self.inner.quarter_length();
        let (numerator, denominator) = self.aggregate_ratio(py)?;
        let mut quarter_length =
            self.written_quarter_length() * numerator as f64 / denominator as f64;
        for dots in &self.dot_groups {
            if *dots != 0 {
                quarter_length *= dot_multiplier(*dots);
            }
        }
        self.inner = RsDuration::new(quarter_length).map_err(duration_error)?;
        if self.inner.quarter_length() != was {
            self.informClient(py)?;
        }
        Ok(())
    }

    /// Makes the given values the written ones, and the length their total.
    fn set_component_list(
        &mut self,
        py: Python<'_>,
        components: Vec<DurationTuple>,
    ) -> PyResult<()> {
        self.materialize(py)?;
        self.components = Some(components);
        self.relength(py)
    }

    /// The written values, working them out first if nobody has.
    fn component_list_materialized(&mut self, py: Python<'_>) -> PyResult<Vec<DurationTuple>> {
        self.materialize(py)?;
        Ok(self.component_list())
    }

    /// Forgets everything worked out from the length, so that it is worked
    /// out again: what music21 does by marking the components stale.
    fn reinfer(&mut self) {
        self.components = None;
        self.tuplets = None;
        self.dot_groups = vec![0];
    }

    /// music21's `Duration()` with nothing said: no written values at all,
    /// and so no length. A note that wants a quarter asks for one.
    pub(crate) fn empty() -> Self {
        let mut duration = Self::wrap(RsDuration::from_type(RsDurationType::Zero));
        duration.components = Some(Vec::new());
        duration.tuplets = Some(Vec::new());
        duration
    }

    pub(crate) fn from_keywords(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let Some(keywords) = keywords else {
            return Ok(Self::empty());
        };
        if let Some(value) = keywords.get_item("quarterLength")? {
            return Ok(Self::wrap(
                RsDuration::new(value.extract::<f64>()?).map_err(note_error)?,
            ));
        }
        if let Some(value) = keywords.get_item("type")? {
            let name = value.extract::<String>()?;
            let kind = RsDurationType::from_music21_name(&name)
                .ok_or_else(|| NoteException::new_err(format!("no such duration type: {name}")))?;
            let mut duration = Self::wrap(RsDuration::from_type(kind));
            // A written value that was given is not one that was inferred.
            duration.components = Some(vec![DurationTuple::of(kind, 0)]);
            duration.tuplets = Some(Vec::new());
            duration.expression_is_inferred = false;
            return Ok(duration);
        }
        Ok(Self::empty())
    }

    /// Whether any of the keywords say something about the duration, which
    /// is how music21 decides between the quarter it gives every note and a
    /// duration built from what it was told.
    pub(crate) fn keywords_say_duration(keywords: Option<&Bound<'_, PyDict>>) -> PyResult<bool> {
        let Some(keywords) = keywords else {
            return Ok(false);
        };
        for name in ["quarterLength", "type", "dots", "duration"] {
            if keywords.contains(name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// A copy of a duration object, keeping whatever kind of duration it is.
pub(crate) fn copied_duration(py: Python<'_>, object: &Py<PyAny>) -> PyResult<Py<PyAny>> {
    // Through the object's own copying, so that a copy is the kind of
    // duration the original was: a grace note copied as a plain duration
    // would be written out as a note that sounds.
    Ok(py
        .import("copy")?
        .getattr("deepcopy")?
        .call1((object.bind(py),))?
        .unbind())
}

pub(crate) fn duration_value_of(py: Python<'_>, object: &Py<PyAny>) -> Option<RsDuration> {
    let object = object.bind(py);
    if let Ok(ours) = object.extract::<PyRef<'_, Duration>>() {
        return Some(ours.inner.clone());
    }
    duration_from_any(object).ok()
}

pub(crate) fn duration_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsDuration> {
    if let Ok(facade) = value.extract::<PyRef<Duration>>() {
        return Ok(facade.inner.clone());
    }
    if let Ok(name) = value.extract::<String>() {
        return RsDurationType::from_music21_name(&name)
            .map(RsDuration::from_type)
            .ok_or_else(|| NoteException::new_err(format!("no such duration type: {name}")));
    }
    if let Ok(quarter_length) = value.extract::<f64>() {
        // A length that is not a number at all is the caller's mistake, and
        // music21 reports it the way Python does rather than as a duration
        // that could not be worked out. Left unguarded it would reach
        // `makeMeasures`, where a bar never as long as itself loops forever.
        if quarter_length.is_nan() {
            return Err(PyValueError::new_err(
                "Cannot convert nan to a quarter length",
            ));
        }
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    if let Ok(quarter_length) = value
        .getattr("quarterLength")
        .and_then(|value| value.extract::<f64>())
    {
        if quarter_length.is_nan() {
            // music21 names the written value in its message, and a
            // `DurationTuple` may be its own namedtuple rather than one of
            // ours — the class is not replaced — so it is known by name.
            let written = value
                .get_type()
                .name()
                .is_ok_and(|name| name == "DurationTuple");
            return Err(PyValueError::new_err(if written {
                "Invalid quarterLength for DurationTuple: nan"
            } else {
                "Cannot convert nan to a quarter length"
            }));
        }
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    Err(NoteException::new_err(format!(
        "cannot read a duration from {}",
        value.repr()?
    )))
}

#[pymethods]
impl Duration {
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec!["Duration", "ProtoM21Object", "SlottedObjectMixin", "object"]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> =
            vec!["Duration", "ProtoM21Object", "SlottedObjectMixin", "object"];
        Ok(pyo3::types::PyFrozenSet::new(py, &names)?
            .into_any()
            .unbind())
    }

    /// music21 freezes a score by pickling it. A duration keeps what it is
    /// in Rust, where a pickle cannot see it, so it is written out as text
    /// and read back, with whatever was said about how it is written beside
    /// it — a thawed duration that had forgotten its written value could not
    /// be written out to a file at all.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let duration = slf.borrow();
            extra.set_item("linked", duration.linked)?;
            extra.set_item("unlinkedType", duration.unlinked_type.as_ref())?;
            extra.set_item("expressionIsInferred", duration.expression_is_inferred)?;
            extra.set_item("dotGroups", duration.dot_groups.clone())?;
            // The tuplets a duration is written inside are objects, and a
            // score frozen to a file and read back is written the same way:
            // without them a nested tuplet came back as a plain note.
            extra.set_item("tuplets", duration.tuplets.as_ref())?;
        }
        crate::pickled_extra(slf, &slf.borrow().inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsDuration>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        let mut duration = slf.borrow_mut();
        duration.inner = inner;
        // The written values are worked out again from the length, since the
        // ones a blank object was made with say nothing about it.
        duration.components = None;
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            duration.linked = extra.get_item("linked")?.extract().unwrap_or(true);
            duration.unlinked_type = extra.get_item("unlinkedType")?.extract().unwrap_or(None);
            duration.expression_is_inferred = extra
                .get_item("expressionIsInferred")?
                .extract()
                .unwrap_or(true);
            if let Ok(groups) = extra.get_item("dotGroups")?.extract::<Vec<u32>>() {
                duration.dot_groups = groups;
            }
            if let Ok(tuplets) = extra.get_item("tuplets")?.extract::<Vec<Py<PyAny>>>() {
                duration.tuplets = Some(tuplets);
            }
        }
        Ok(())
    }

    /// music21's `Duration(value, **keywords)`, where `type`, `dots` and
    /// `quarterLength` are the keywords its `GeneralNote` passes through when
    /// a note is built with them.
    #[new]
    #[pyo3(signature = (value = None, **keywords))]
    pub(crate) fn new(
        value: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        // music21 reads the one positional argument as whichever keyword it
        // looks like, and refuses it outright when that keyword was given as
        // well: a duration told its written value twice is a mistake to
        // report rather than one to guess at.
        if let Some(given) = value.filter(|value| !value.is_none()) {
            let also_given = |name: &str| -> PyResult<bool> {
                let Some(keywords) = keywords else {
                    return Ok(false);
                };
                Ok(keywords
                    .get_item(name)?
                    .is_some_and(|value| !value.is_none()))
            };
            let clash = if given.extract::<String>().is_ok() {
                also_given("type")?
            } else if given.extract::<PyRef<'_, DurationTuple>>().is_ok() {
                also_given("durationTuple")?
            } else {
                also_given("quarterLength")?
            };
            if clash {
                return Err(PyTypeError::new_err(format!(
                    "Cannot parse argument {given} or conflicts with keywords"
                )));
            }
        }
        let mut duration = match value.filter(|value| !value.is_none()) {
            Some(value) => match value.extract::<String>() {
                Ok(name) => {
                    let kind = RsDurationType::from_music21_name(&name).ok_or_else(|| {
                        NoteException::new_err(format!("no such duration type: {name}"))
                    })?;
                    let mut duration = Self::wrap(RsDuration::from_type(kind));
                    duration.components = Some(vec![DurationTuple::of(kind, 0)]);
                    duration.tuplets = Some(Vec::new());
                    duration.expression_is_inferred = false;
                    duration
                }
                Err(_) => Self::wrap(duration_from_any(value)?),
            },
            None => Self::from_keywords(keywords)?,
        };
        if let Some(keywords) = keywords
            && let Some(dots) = keywords.get_item("dots")?
        {
            Python::attach(|py| duration.set_dots(py, &dots))?;
        }
        Ok(duration)
    }

    /// music21's `quarterLength`, as an `opFrac`: a float where the length is
    /// exactly one, and a `Fraction` where it is not — a triplet quarter is
    /// `Fraction(2, 3)`, never `0.6666666666666666`.
    #[getter]
    fn get_quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        op_frac(py, self.inner.quarter_length())
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = quarter_length_from_any(value)?;
        let was = self.inner.quarter_length();
        if !self.linked {
            self.inner = RsDuration::new(value).map_err(duration_error)?;
        } else if value != was || self.components.is_none() || self.get_type(py)? == "inexpressible"
        {
            // Only a length that has actually changed throws away the
            // written values: music21 leaves them alone otherwise, and its
            // own notation code sets a duration to the length it already has
            // — which would have cost a note the tuplets it was written in.
            // A length nothing can write is the exception, and setting one to
            // itself is how music21 asks for it to be worked out again.
            self.reinfer();
            self.expression_is_inferred = true;
            self.inner.set_quarter_length(value).map_err(note_error)?;
        }
        // A stream keeps the length of what it holds, so a note that has just
        // been made longer has to say so or the stream goes on reporting the
        // length it had.
        if self.inner.quarter_length() != was {
            self.informClient(py)?;
        }
        Ok(())
    }

    /// music21's `type`: the written value when there is one of them, and
    /// `'complex'` when the length has to be written as a tie.
    #[getter]
    fn get_type(&mut self, py: Python<'_>) -> PyResult<String> {
        if !self.linked {
            return Ok(self
                .unlinked_type
                .clone()
                .unwrap_or_else(|| "zero".to_string()));
        }
        self.materialize(py)?;
        let components = self.component_list();
        Ok(match components.len() {
            0 => "zero".to_string(),
            1 => components[0].r#type(),
            _ => "complex".to_string(),
        })
    }

    #[setter]
    fn set_type(&mut self, py: Python<'_>, value: &str) -> PyResult<()> {
        if RsDurationType::from_music21_name(value).is_none()
            && !matches!(value, "inexpressible" | "complex")
        {
            return Err(PyValueError::new_err(format!(
                "no such type exists: {value}"
            )));
        }
        if !self.linked {
            self.unlinked_type = Some(value.to_string());
            return Ok(());
        }
        let dots = self.get_dots(py)?;
        self.materialize(py)?;
        self.components = Some(vec![DurationTuple::from_type_dots(value, dots)]);
        self.expression_is_inferred = false;
        self.relength(py)
    }

    /// music21's `components`: the written note values this length is made
    /// of, tied together.
    #[getter]
    fn get_components<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.materialize(py)?;
        PyTuple::new(py, self.component_list())
    }

    #[setter]
    fn set_components(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut components = Vec::new();
        for item in value.try_iter()? {
            components.push(duration_tuple_from_any(&item?)?);
        }
        self.set_component_list(py, components)
    }

    /// music21's `currentComponents`: the written values as they stand, with
    /// no working-out — so a duration that has never been asked answers with
    /// nothing, and answers properly once anything has observed it.
    fn currentComponents<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.components.clone().unwrap_or_default())
    }

    /// music21's `_components`, which is `currentComponents` under its own
    /// name. Its tests read it to see that a length just set has not been
    /// written out into note values until something asks for them.
    #[getter]
    fn _components<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.currentComponents(py)
    }

    /// music21's `_componentsNeedUpdating`: whether the note values this
    /// length is written as are still to be worked out from it.
    #[getter(_componentsNeedUpdating)]
    fn get_componentsNeedUpdating(&self) -> bool {
        self.components.is_none()
    }

    #[setter(_componentsNeedUpdating)]
    fn set_componentsNeedUpdating(&mut self, py: Python<'_>, value: bool) -> PyResult<()> {
        if value {
            self.components = None;
            return Ok(());
        }
        self.materialize(py)
    }

    /// music21's `_quarterLengthNeedsUpdating`, which is never true here:
    /// the length is the value this duration keeps, so it is never the half
    /// that is behind.
    #[getter]
    fn _quarterLengthNeedsUpdating(&self) -> bool {
        false
    }

    /// music21's `addDurationTuple`: another written value tied on the end.
    /// A whole `Duration` adds every one of its values.
    fn addDurationTuple(&mut self, py: Python<'_>, dur: &Bound<'_, PyAny>) -> PyResult<()> {
        self.materialize(py)?;
        let mut components = self.component_list();
        if let Ok(mut duration) = dur.extract::<PyRefMut<'_, Duration>>() {
            let added = duration.get_components(py)?;
            let added: Vec<DurationTuple> = added
                .try_iter()?
                .map(|item| duration_tuple_from_any(&item?))
                .collect::<PyResult<_>>()?;
            components.extend(added);
        } else {
            components.push(duration_tuple_from_any(dur)?);
        }
        self.set_component_list(py, components)
    }

    /// music21's `clear`: no written values at all, and no length.
    fn clear(&mut self, py: Python<'_>) -> PyResult<()> {
        self.dot_groups = vec![0];
        self.set_component_list(py, Vec::new())
    }

    /// music21's `componentStartTime`: how far into the duration the written
    /// value at an index begins.
    fn componentStartTime(&mut self, py: Python<'_>, componentIndex: usize) -> PyResult<f64> {
        self.materialize(py)?;
        let components = self.component_list();
        if componentIndex >= components.len() {
            return Err(PyIndexError::new_err(format!(
                "invalid component index value {componentIndex} submitted; \
                 value must be an integer between 0 and {}",
                components.len().saturating_sub(1)
            )));
        }
        Ok(components[..componentIndex]
            .iter()
            .map(|component| component.quarter_length)
            .fold(0.0, |total, length| total + length))
    }

    /// music21's `componentIndexAtQtrPosition`, including its own oddity:
    /// at the very start or the very end it hands back the *component*
    /// rather than its index, which its docstring flags and keeps.
    fn componentIndexAtQtrPosition<'py>(
        &mut self,
        py: Python<'py>,
        quarterPosition: f64,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.materialize(py)?;
        let components = self.component_list();
        if components.is_empty() {
            return Err(DurationException::new_err(
                "Need components to run getComponentIndexAtQtrPosition",
            ));
        }
        let total = self.inner.quarter_length();
        if quarterPosition > total {
            return Err(PyValueError::new_err(
                "position is after the end of the duration",
            ));
        }
        if quarterPosition < 0.0 {
            return Err(PyValueError::new_err(
                "position is before the start of the duration",
            ));
        }
        if quarterPosition == 0.0 {
            return Ok(components[0].clone().into_pyobject(py)?.into_any());
        }
        if quarterPosition == total {
            return Ok(components[components.len() - 1]
                .clone()
                .into_pyobject(py)?
                .into_any());
        }
        let mut current = 0.0;
        for (index, component) in components.iter().enumerate() {
            current += component.quarter_length;
            if current > quarterPosition {
                return Ok(index.into_pyobject(py)?.into_any());
            }
        }
        Err(DurationException::new_err(
            "Could not match quarterLength within an index.",
        ))
    }

    /// music21's `sliceComponentAtPosition`: cuts the written value sounding
    /// at that point in two, leaving the whole length unchanged.
    fn sliceComponentAtPosition(&mut self, py: Python<'_>, quarterPosition: f64) -> PyResult<()> {
        self.materialize(py)?;
        let components = self.component_list();
        let mut start = 0.0;
        let mut index = components.len();
        for (position, component) in components.iter().enumerate() {
            let end = start + component.quarter_length;
            if quarterPosition > start && quarterPosition < end {
                index = position;
                break;
            }
            start = end;
        }
        if index == components.len() {
            return Err(DurationException::new_err(
                "no slice is possible at this quarter position",
            ));
        }
        let left = quarterPosition - start;
        let right = components[index].quarter_length - left;
        let mut sliced = components[..index].to_vec();
        sliced.push(DurationTuple::from_quarter_length(left));
        sliced.push(DurationTuple::from_quarter_length(right));
        sliced.extend_from_slice(&components[index + 1..]);
        self.set_component_list(py, sliced)
    }

    /// music21's `consolidate`: one written value for the whole length,
    /// losing how it had been written.
    fn consolidate(&mut self, py: Python<'_>) -> PyResult<()> {
        self.materialize(py)?;
        if self.component_list().len() == 1 {
            return Ok(());
        }
        let written = self.quarterLengthNoTuplets(py)?;
        self.set_component_list(py, vec![DurationTuple::from_quarter_length(written)])
    }

    /// music21's `tuplets`: the tuplets this length is written inside.
    ///
    /// Asking works them out and keeps them, so the objects handed back are
    /// the same ones next time and an edit to one sticks — music21's own
    /// `findTupletGroups` marks the end of a group by setting `type` on the
    /// tuplet it read off a note.
    #[getter]
    fn get_tuplets<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.materialize(py)?;
        PyTuple::new(py, self.tuplet_objects(py)?)
    }

    #[setter]
    fn set_tuplets(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.materialize(py)?;
        let mut tuplets = Vec::new();
        for item in value.try_iter()? {
            tuplets.push(item?.unbind());
        }
        self.tuplets = Some(tuplets);
        self.relength(py)
    }

    /// music21's `appendTuplet`: one more tuplet around this length, which
    /// shortens it by that tuplet's ratio and freezes the tuplet given.
    fn appendTuplet(&mut self, py: Python<'_>, newTuplet: &Bound<'_, PyAny>) -> PyResult<()> {
        if newTuplet.hasattr("frozen")? {
            newTuplet.setattr("frozen", true)?;
        }
        self.materialize(py)?;
        let mut tuplets = self.tuplet_objects(py)?;
        tuplets.push(newTuplet.clone().unbind());
        self.tuplets = Some(tuplets);
        self.relength(py)
    }

    /// music21's `aggregateTupletMultiplier`: every tuplet's ratio
    /// multiplied together, so a triplet inside a quintuplet is `8/15`.
    fn aggregateTupletMultiplier<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let (numerator, denominator) = self.aggregate_ratio(py)?;
        op_frac_ratio(py, numerator, denominator)
    }

    /// music21's `quarterLengthNoTuplets`: the total of the written values,
    /// before any tuplet shortens them.
    #[getter]
    fn quarterLengthNoTuplets(&mut self, py: Python<'_>) -> PyResult<f64> {
        self.materialize(py)?;
        Ok(self.written_quarter_length())
    }

    /// music21's `isComplex`: whether this length needs more than one
    /// written value tied together.
    #[getter]
    fn isComplex(&mut self, py: Python<'_>) -> PyResult<bool> {
        self.materialize(py)?;
        Ok(self.component_list().len() > 1)
    }

    /// music21's `linked`: whether the written values and the sounding
    /// length move together. Unlinked, each is set on its own.
    #[getter]
    fn get_linked(&self) -> bool {
        self.linked
    }

    #[setter]
    fn set_linked(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let Ok(linked) = value.extract::<bool>() else {
            return Err(PyTypeError::new_err(format!(
                "Linked can only be True or False, not {}",
                value.str()?
            )));
        };
        let value = linked;
        if !value && self.linked {
            self.unlinked_type = Some(self.get_type(py)?);
        } else if value && !self.linked {
            self.reinfer();
        }
        self.linked = value;
        Ok(())
    }

    /// music21's `expressionIsInferred`: whether the written values were
    /// worked out from the length rather than given.
    #[getter]
    fn get_expressionIsInferred(&self) -> bool {
        self.expression_is_inferred
    }

    #[setter]
    fn set_expressionIsInferred(&mut self, value: bool) {
        self.expression_is_inferred = value;
    }

    /// music21's `dotGroups`: dots written above dots, which is how medieval
    /// music writes a dotted note that is itself dotted.
    #[getter]
    fn get_dotGroups<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let dots = self.get_dots(py)?;
        if dots != 0 && self.dot_groups == vec![0] {
            return PyTuple::new(py, [dots]);
        }
        PyTuple::new(py, self.dot_groups.clone())
    }

    #[setter]
    fn set_dotGroups(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if !value.is_instance_of::<PyTuple>() {
            return Err(PyTypeError::new_err(
                "only tuple dotGroups values can be used with this method.",
            ));
        }
        let groups: Vec<u32> = value.extract()?;
        self.materialize(py)?;
        // Setting a dot group takes the plain dots off every written value:
        // the group is now what says how the note is dotted.
        let undotted: Vec<DurationTuple> = self
            .component_list()
            .into_iter()
            .map(|component| DurationTuple::from_type_dots(&component.r#type(), 0))
            .collect();
        self.components = Some(undotted);
        self.dot_groups = groups;
        self.relength(py)
    }

    /// music21's `splitDotGroups`: the same length written as a tie of
    /// singly dotted values, which is how a program with no dot groups can
    /// still show the notes.
    #[pyo3(signature = (*, inPlace = false))]
    fn splitDotGroups(&mut self, py: Python<'_>, inPlace: bool) -> PyResult<Option<Self>> {
        let written_type = self.get_type(py)?;
        let groups: Vec<u32> = self.get_dotGroups(py)?.extract()?;
        let mut split = if inPlace { None } else { Some(self.clone()) };
        let target: &mut Self = split.as_mut().unwrap_or(self);
        target.clear(py)?;
        let mut components = vec![DurationTuple::from_type_dots(
            &written_type,
            *groups.first().unwrap_or(&0),
        )];
        for _ in 1..groups.len() {
            let smaller: Vec<DurationTuple> = components
                .iter()
                .map(|component| {
                    let next = RsDurationType::from_music21_name(&component.r#type())
                        .and_then(RsDurationType::next_smaller)
                        .map_or_else(
                            || component.r#type(),
                            |kind| kind.music21_name().to_string(),
                        );
                    DurationTuple::from_type_dots(&next, component.dots())
                })
                .collect();
            components.extend(smaller);
        }
        target.set_component_list(py, components)?;
        Ok(split)
    }

    /// music21's `augmentOrDiminish`: the same length scaled, as a new
    /// duration. A scalar of zero or less is refused, as music21 refuses it.
    #[pyo3(signature = (amountToScale, retainComponents = false))]
    fn augmentOrDiminish(
        &mut self,
        py: Python<'_>,
        amountToScale: f64,
        retainComponents: bool,
    ) -> PyResult<Self> {
        if amountToScale <= 0.0 || amountToScale.is_nan() {
            return Err(PyValueError::new_err(
                "amountToScale must be greater than zero",
            ));
        }
        if !retainComponents {
            let mut scaled = Self::wrap(
                RsDuration::new(self.inner.quarter_length() * amountToScale)
                    .map_err(duration_error)?,
            );
            scaled.linked = self.linked;
            return Ok(scaled);
        }
        self.materialize(py)?;
        let mut scaled = self.clone();
        let components: Vec<DurationTuple> = self
            .component_list()
            .into_iter()
            .map(|component| component.augmentOrDiminish(amountToScale))
            .collect();
        scaled.set_component_list(py, components)?;
        Ok(scaled)
    }

    /// music21's `getGraceDuration`: the same written values, sounding
    /// nothing at all.
    #[pyo3(signature = (appoggiatura = false))]
    fn getGraceDuration(&mut self, py: Python<'_>, appoggiatura: bool) -> PyResult<Py<PyAny>> {
        self.materialize(py)?;
        let mut written_type = self.get_type(py)?;
        if written_type == "zero" {
            // Now that it is not a grace note, it needs a value to be
            // written as.
            written_type = "eighth".to_string();
        }
        let mut grace = self.clone();
        grace.components = Some(
            self.component_list()
                .into_iter()
                .map(|component| DurationTuple::new(component.r#type(), component.dots(), 0.0))
                .collect(),
        );
        grace.linked = false;
        grace.unlinked_type = Some(written_type);
        grace.inner = RsDuration::new(0.0).map_err(duration_error)?;
        let initializer = GraceDuration {
            slash: true,
            steal_previous: None,
            steal_following: None,
            make_time: false,
        };
        if appoggiatura {
            let marks = GraceDuration {
                slash: false,
                steal_previous: None,
                steal_following: None,
                make_time: false,
            };
            if let Some(class) =
                crate::installed_class(py, "music21.duration", "AppoggiaturaDuration")
            {
                let object = crate::blank_installed(&class)?;
                {
                    let cell = object.cast::<AppoggiaturaDuration>()?;
                    let mut me = cell.borrow_mut();
                    let graces = me.as_super();
                    **graces.as_super() = grace;
                    **graces = marks;
                }
                return Ok(object.unbind());
            }
            return Ok(Py::new(
                py,
                PyClassInitializer::from(grace)
                    .add_subclass(marks)
                    .add_subclass(AppoggiaturaDuration),
            )?
            .into_any());
        }
        if let Some(class) = crate::installed_class(py, "music21.duration", "GraceDuration") {
            let object = crate::blank_installed(&class)?;
            {
                let cell = object.cast::<GraceDuration>()?;
                let mut me = cell.borrow_mut();
                **me.as_super() = grace;
                *me = initializer;
            }
            return Ok(object.unbind());
        }
        Ok(Py::new(
            py,
            PyClassInitializer::from(grace).add_subclass(initializer),
        )?
        .into_any())
    }

    /// music21's `dots`: how many dots the first written value carries.
    #[getter]
    fn get_dots(&mut self, py: Python<'_>) -> PyResult<u32> {
        self.materialize(py)?;
        Ok(self.component_list().first().map_or(0, DurationTuple::dots))
    }

    /// Setting dots puts that many on *every* written value, as music21's
    /// setter does. Infinitely many is music21's easter egg: the next larger
    /// value, undotted, since that is what the dots converge on.
    #[setter]
    fn set_dots(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = value.extract::<f64>().map_err(|_| {
            PyTypeError::new_err("only numeric dot values can be used with this method.")
        })?;
        self.materialize(py)?;
        if value.is_infinite() && value > 0.0 {
            let larger = RsDurationType::from_music21_name(&self.get_type(py)?)
                .and_then(RsDurationType::next_larger)
                .ok_or_else(|| DurationException::new_err("no larger duration type"))?;
            self.set_type(py, larger.music21_name())?;
            return self.set_dots(py, PyFloat::new(py, 0.0).as_any());
        }
        let dots = value as u32;
        let dotted: Vec<DurationTuple> = self
            .component_list()
            .into_iter()
            .map(|component| DurationTuple::from_type_dots(&component.r#type(), dots))
            .collect();
        self.components = Some(dotted);
        if self.linked {
            self.expression_is_inferred = false;
        }
        self.relength(py)
    }

    /// music21's `ordinal`: where the written value sits in the list of note
    /// values, `'complex'` when there is more than one of them, and nothing
    /// at all when there is none.
    #[getter]
    fn ordinal<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.materialize(py)?;
        let components = self.component_list();
        match components.len() {
            0 => Ok(py.None().into_bound(py)),
            1 => Ok(components[0].ordinal()?.into_pyobject(py)?.into_any()),
            _ => Ok("complex".into_pyobject(py)?.into_any()),
        }
    }

    #[getter]
    fn fullName(&mut self, py: Python<'_>) -> PyResult<String> {
        // music21's `Duration.fullName`, written out here rather than read
        // off the value: the tuplets a caller has appended are Python
        // objects, and the name says which they are.
        let tuplets = self.tuplet_objects(py)?;
        // A duration nobody has written a tuplet onto, and whose length is
        // the one its written values add up to, knows its own name — and the
        // value's reading is the fuller one, since it names the odd ratio a
        // length like 53/25 needs. An unlinked one does not: a grace note
        // sounds for nothing while still being written as a sixteenth.
        if tuplets.is_empty() && self.linked {
            return Ok(self.inner.full_name());
        }
        let mut named = Vec::with_capacity(tuplets.len());
        for tuplet in &tuplets {
            named.push(tuplet.bind(py).getattr("fullName")?.extract::<String>()?);
        }
        let tuplet_name = named.join(" ");
        let components = self.component_list_materialized(py)?;
        let length = mixed_numeral(py, self.inner.quarter_length())?;
        let mut written: Vec<String> = Vec::with_capacity(components.len());
        for component in &components {
            let dots = component.dots();
            let dotted = match dots {
                0 => String::new(),
                1 => "Dotted".to_string(),
                2 => "Double Dotted".to_string(),
                3 => "Triple Dotted".to_string(),
                4 => "Quadruple Dotted".to_string(),
                many => format!("{many}-Times Dotted"),
            };
            let mut name = String::new();
            let value = component.r#type();
            // The old long values are perfect or imperfect rather than
            // dotted, which is how they were written before the bar line.
            if dots >= 2 || !matches!(value.as_str(), "longa" | "maxima") {
                name.push_str(&dotted);
                name.push(' ');
            } else if dots == 0 {
                name.push_str("Imperfect ");
            } else {
                name.push_str("Perfect ");
            }
            let value = if value.starts_with(['1', '2', '3', '5', '6']) {
                value
            } else {
                title_case(&value)
            };
            if !value.eq_ignore_ascii_case("complex") {
                name.push_str(&value);
                name.push(' ');
            }
            if !tuplet_name.is_empty() {
                name.push_str(&tuplet_name);
                name.push(' ');
            }
            if !tuplet_name.is_empty() || dots >= 3 || value.eq_ignore_ascii_case("complex") {
                name.push_str(&format!("({length} QL)"));
            }
            written.push(name.trim().to_string());
        }
        if components.is_empty() {
            written.push("Zero Duration".to_string());
        }
        let mut whole = written.join(" tied to ");
        if components.len() != 1 {
            whole.push_str(&format!(" ({length} total QL)"));
        }
        Ok(whole)
    }

    /// music21's `informClient`: tells whatever owns this duration that its
    /// length has changed, so that the streams holding it can re-sort.
    ///
    /// The crate has no such notification and wants none, but music21's own
    /// code calls this on durations it has just edited, and a duration of
    /// ours has to be able to pass the message on to a music21 client.
    fn informClient(&self, py: Python<'_>) -> PyResult<bool> {
        let Some(client) = &self.client else {
            return Ok(false);
        };
        let message = PyDict::new(py);
        message.set_item("changedAttribute", "duration")?;
        message.set_item("quarterLength", op_frac(py, self.inner.quarter_length())?)?;
        let client = client.bind(py);
        if !client.hasattr("informSites")? {
            return Ok(false);
        }
        client.call_method1("informSites", (message,))?;
        Ok(true)
    }

    /// music21's `isGrace`, which only a `GraceDuration` says yes to.
    #[getter]
    fn isGrace(&self) -> bool {
        false
    }

    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !slf.get_type().is(other.get_type()) {
            return Ok(false);
        }
        let py = slf.py();
        let other = other.cast::<Self>()?;
        if slf.borrow().linked != other.borrow().linked {
            return Ok(false);
        }
        let quarter_lengths_agree =
            slf.borrow().inner.quarter_length() == other.borrow().inner.quarter_length();
        if slf.borrow().expression_is_inferred && other.borrow().expression_is_inferred {
            return Ok(quarter_lengths_agree);
        }
        let mine = Duration::component_list_materialized(&mut slf.borrow_mut(), py)?;
        let theirs = Duration::component_list_materialized(&mut other.borrow_mut(), py)?;
        if mine.len() != theirs.len() {
            return Ok(false);
        }
        if mine.is_empty() {
            return Ok(true);
        }
        if mine != theirs {
            return Ok(false);
        }
        let my_tuplets = Duration::get_tuplets(&mut slf.borrow_mut(), py)?;
        let their_tuplets = Duration::get_tuplets(&mut other.borrow_mut(), py)?;
        if !my_tuplets.eq(&their_tuplets)? {
            return Ok(false);
        }
        Ok(quarter_lengths_agree)
    }

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let name = slf.get_type().qualname()?;
        let duration = slf.borrow();
        if duration.linked {
            let quarter_length = op_frac(slf.py(), duration.inner.quarter_length())?;
            return Ok(format!(
                "<music21.duration.{name} {}>",
                quarter_length.str()?
            ));
        }
        let quarter_length = op_frac(slf.py(), duration.inner.quarter_length())?;
        let written = duration
            .unlinked_type
            .clone()
            .unwrap_or_else(|| "zero".to_string());
        Ok(format!(
            "<music21.duration.{name} unlinked type:{written} quarterLength:{}>",
            quarter_length.str()?
        ))
    }

    #[getter]
    fn get_client(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.client.as_ref().map(|client| client.clone_ref(py))
    }

    #[setter]
    fn set_client(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.client = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// A copy as an object of the class it was asked on: music21 compares
    /// two of these by class before anything else, so a copy built as the
    /// bare facade would not equal the original.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut copied = slf.borrow().clone();
        copied.tuplets = deep_copied_objects(slf.py(), memo, copied.tuplets)?;
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().clone();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `duration.GraceDuration`: written as a note, sounding as
/// nothing.
#[pyclass(
    name = "GraceDuration",
    module = "music21.duration",
    extends = Duration,
    subclass,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct GraceDuration {
    slash: bool,
    /// music21's `stealTimePrevious` and `stealTimeFollowing`: how much of
    /// the neighbouring note's time this one takes, as a share of it. The
    /// crate models no performance, so what a caller writes is kept.
    steal_previous: Option<f64>,
    steal_following: Option<f64>,
    /// music21's `makeTime`: whether the grace note takes time of its own.
    make_time: bool,
}

#[pymethods]
impl GraceDuration {
    #[new]
    /// music21's `GraceDuration.__init__`: the written value it is given is
    /// kept and given no length, so a grace note written as a half note
    /// still says it is one while sounding for nothing at all. A grace note
    /// nobody has written a value for is an eighth.
    #[pyo3(signature = (value = None, **keywords))]
    fn new(
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut base = Duration::new(value, keywords)?;
        let mut written = base.get_type(py)?;
        if written == "zero" {
            written = "eighth".to_string();
        }
        let mut components: Vec<DurationTuple> = base
            .component_list_materialized(py)?
            .into_iter()
            .map(|component| DurationTuple::new(component.r#type(), component.dots(), 0.0))
            .collect();
        if components.is_empty() {
            components.push(DurationTuple::new("eighth".to_string(), 0, 0.0));
        }
        base.components = Some(components);
        base.linked = false;
        base.unlinked_type = Some(written);
        base.inner = RsDuration::new(0.0).map_err(duration_error)?;
        Ok(PyClassInitializer::from(base).add_subclass(Self {
            slash: true,
            steal_previous: None,
            steal_following: None,
            make_time: false,
        }))
    }

    #[getter]
    fn isGrace(&self) -> bool {
        true
    }

    /// A copy is a grace note too, and keeps what makes it one: music21's
    /// own notation code deep-copies a score before writing it, and a copy
    /// that had become a plain duration would be written as a real note.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::copied(slf)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::copied(slf)
    }

    #[getter]
    fn get_slash(&self) -> bool {
        self.slash
    }

    /// music21 takes `True`, `False` or `None` here and nothing else, and
    /// says so as a `ValueError` — its own tests catch that class.
    #[setter]
    fn set_slash(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.slash = true_false_or_none(value)?.unwrap_or(false);
        Ok(())
    }

    /// music21's `stealTimePrevious`: how much of the previous note's time
    /// this grace note takes, as a share of it. Its MusicXML reader writes
    /// the score's `steal-time-previous` here.
    #[getter]
    fn get_stealTimePrevious(&self) -> Option<f64> {
        self.steal_previous
    }

    #[setter]
    fn set_stealTimePrevious(&mut self, value: Option<f64>) {
        self.steal_previous = value;
    }

    /// music21's `stealTimeFollowing`, the same for the note after.
    #[getter]
    fn get_stealTimeFollowing(&self) -> Option<f64> {
        self.steal_following
    }

    #[setter]
    fn set_stealTimeFollowing(&mut self, value: Option<f64>) {
        self.steal_following = value;
    }

    /// music21's `makeTime`: whether the grace note takes time of its own in
    /// performance. Nothing here plays anything, so it is kept and handed
    /// back.
    #[getter]
    fn get_makeTime(&self) -> bool {
        self.make_time
    }

    #[setter]
    fn set_makeTime(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.make_time = true_false_or_none(value)?.unwrap_or(false);
        Ok(())
    }
}

impl GraceDuration {
    /// Both halves of a grace duration copied into a blank of its own class.
    fn copied<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let class = slf.as_any().get_type();
        let copy = crate::blank_installed(class.as_any())?;
        let (mut written, marks) = {
            let me = slf.borrow();
            ((**me.as_super()).clone(), me.clone())
        };
        // As for a plain duration: the tuplets are the copy's own.
        written.tuplets = deep_copied_objects(
            slf.py(),
            &slf.py().None().into_bound(slf.py()),
            written.tuplets,
        )?;
        {
            let cell = copy.cast::<Self>()?;
            let mut target = cell.borrow_mut();
            **target.as_super() = written;
            *target = marks;
        }
        Ok(copy)
    }
}

/// music21's `duration.AppoggiaturaDuration`: a grace note that takes its
/// time from the note it leans on.
#[pyclass(
    name = "AppoggiaturaDuration",
    module = "music21.duration",
    extends = GraceDuration,
    subclass,
    skip_from_py_object
)]
pub struct AppoggiaturaDuration;

#[pymethods]
impl AppoggiaturaDuration {
    #[new]
    #[pyo3(signature = (value = None, **keywords))]
    fn new(
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(GraceDuration::new(py, value, keywords)?.add_subclass(Self))
    }
}

/// Reads a written value argument: a `DurationTuple`, or anything with
/// music21's `type` and `dots` on it, a `Duration` included.
fn duration_tuple_from_any(value: &Bound<'_, PyAny>) -> PyResult<DurationTuple> {
    if let Ok(tuple) = value.extract::<PyRef<'_, DurationTuple>>() {
        return Ok(tuple.clone());
    }
    if let Ok(duration) = value.extract::<PyRef<'_, Duration>>() {
        let components = duration.component_list();
        if let [single] = components.as_slice() {
            return Ok(single.clone());
        }
        return Ok(DurationTuple::from_quarter_length(
            duration.inner.quarter_length(),
        ));
    }
    let kind: String = value.getattr("type")?.extract()?;
    let dots: u32 = value.getattr("dots")?.extract()?;
    let quarter_length: f64 = value.getattr("quarterLength")?.extract()?;
    Ok(DurationTuple::new(kind, dots, quarter_length))
}

/// music21's `durationTupleFromQuarterLength`.
#[pyfunction]
#[pyo3(name = "durationTupleFromQuarterLength", signature = (ql = 1.0))]
fn durationTupleFromQuarterLength(ql: f64) -> DurationTuple {
    DurationTuple::from_quarter_length(ql)
}

/// music21's `durationTupleFromTypeDots`.
#[pyfunction]
#[pyo3(name = "durationTupleFromTypeDots", signature = (durType = "quarter".to_string(), dots = 0))]
fn durationTupleFromTypeDots(durType: String, dots: u32) -> PyResult<DurationTuple> {
    let kind = RsDurationType::from_music21_name(&durType)
        .ok_or_else(|| DurationException::new_err(format!("no such duration type: {durType}")))?;
    Ok(DurationTuple::of(kind, dots))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Duration>()?;
    m.add_class::<GraceDuration>()?;
    m.add_class::<AppoggiaturaDuration>()?;
    m.add_class::<DurationTuple>()?;
    m.add_class::<Tuplet>()?;
    let exception = py.get_type::<DurationException>();
    exception.setattr("__module__", "music21.duration")?;
    m.add("DurationException", exception)?;
    m.add_function(wrap_pyfunction!(durationTupleFromQuarterLength, m)?)?;
    m.add_function(wrap_pyfunction!(durationTupleFromTypeDots, m)?)?;
    Ok(())
}
