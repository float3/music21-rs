//! music21's `note.Note` and `duration.Duration` over `music21-rs`, enough of
//! each for the chord facade to hand real notes back and forth.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyTuple};

use music21_rs::{
    Duration as RsDuration, DurationType as RsDurationType, KeySignature as RsKeySignature,
    Note as RsNote, Notehead as RsNotehead, Pitch as RsPitch, StemDirection as RsStemDirection,
    Tuplet as RsTuplet,
};

use crate::interval::transpose_pitch_by_any;
use crate::notation::{Beams, Lyric, Tie, Volume, tie_from_any, volume_from_any};
use crate::pitch::{Pitch, message, pitch_from_any};

/// The names the `note` facade replaces in `music21.note`.
pub const NAMES: &[&str] = &[
    "Note",
    "NoteException",
    "NotRestException",
    "Lyric",
    "LyricException",
];

/// The names the `note` facade replaces in `music21.duration`.
/// Only `Duration` itself. `Tuplet`, `DurationTuple` and the module
/// functions beside them are registered on the facade module but *not*
/// swapped into music21's: music21's own are richer than these and already
/// pass their docstrings, and replacing a working implementation with a
/// thinner one costs more than it gains.
pub const DURATION_NAMES: &[&str] = &["Duration", "GraceDuration", "AppoggiaturaDuration"];

pyo3::create_exception!(music21_rs_facade, NoteException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, NotRestException, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    DurationException,
    crate::Music21Exception
);

/// A note or chord's name with the length written on the end taken from its
/// duration object rather than from its value.
///
/// The object is where an unlinked length lives: a grace note sounds for
/// nothing at all while still being written as a sixteenth, and music21
/// names it by what is written.
pub(crate) fn named_with_duration(holder: &Bound<'_, PyAny>, named: &str, value: &str) -> String {
    let Ok(written) = holder
        .getattr("duration")
        .and_then(|duration| duration.getattr("fullName"))
        .and_then(|name| name.extract::<String>())
    else {
        return named.to_string();
    };
    if written == value {
        return named.to_string();
    }
    named.replacen(value, &written, 1)
}

/// Whether two lists of ornaments or marks hold the same kinds of thing.
///
/// music21 compares them by class rather than by object, and by set as well
/// as by count, which is how a note that has been given a turn stops being
/// equal to the same note without one.
pub(crate) fn same_kinds(
    py: Python<'_>,
    mine: Option<&Py<PyList>>,
    theirs: Option<&Py<PyList>>,
) -> PyResult<bool> {
    let kinds = |list: Option<&Py<PyList>>| -> PyResult<Vec<Py<PyAny>>> {
        let Some(list) = list else {
            return Ok(Vec::new());
        };
        list.bind(py)
            .iter()
            .map(|item| Ok(item.get_type().into_any().unbind()))
            .collect()
    };
    let (mine, theirs) = (kinds(mine)?, kinds(theirs)?);
    if mine.len() != theirs.len() {
        return Ok(false);
    }
    let set = |kinds: Vec<Py<PyAny>>| -> PyResult<Bound<'_, PyAny>> {
        py.import("builtins")?.getattr("set")?.call1((kinds,))
    };
    set(mine)?.eq(set(theirs)?)
}

/// A number written the way music21's `common.mixedNumeral` writes it: a
/// whole number where it is one, and a whole number and a fraction where the
/// two are both there.
fn mixed_numeral(py: Python<'_>, value: f64) -> PyResult<String> {
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

fn duration_error(error: music21_rs::Error) -> PyErr {
    DurationException::new_err(message(&error))
}

pub(crate) fn note_error(error: music21_rs::Error) -> PyErr {
    NoteException::new_err(message(&error))
}

/// The error music21 raises out of the `NotRest` properties — notehead, its
/// fill and parentheses, and stem direction — which is a different class
/// from the one its `Note` methods raise.
fn not_rest_error(error: music21_rs::Error) -> PyErr {
    NotRestException::new_err(message(&error))
}

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

/// The key signature in force where this note sits, if it sits anywhere.
///
/// The search up the containing streams is music21's `getContextByClass`,
/// since it is music21 that holds the streams. A signature found this way is
/// as often music21's own class as ours, so it is read off its `sharps`.
fn key_signature_around(note: &Bound<'_, PyAny>) -> PyResult<Option<RsKeySignature>> {
    if !note.hasattr("getContextByClass")? {
        return Ok(None);
    }
    let found = note.call_method1("getContextByClass", ("KeySignature",))?;
    if found.is_none() {
        return Ok(None);
    }
    let Ok(sharps) = found.getattr("sharps").and_then(|s| s.extract::<i32>()) else {
        return Ok(None);
    };
    Ok(Some(RsKeySignature::new(sharps)))
}

/// music21's `NotRest.getInstrument`: the instrument stored on this note,
/// or the one in force where it sits.
///
/// The search up the containing streams is music21's, as is the default
/// instrument it falls back to; the crate models neither streams nor
/// instruments. Written against the Python object because music21 writes it
/// once on `NotRest` and both a note and a chord inherit it.
pub(crate) fn instrument_for_note<'py>(
    note: &Bound<'py, PyAny>,
    return_default: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let stored = note.getattr("storedInstrument")?;
    if !stored.is_none() {
        return Ok(Some(stored));
    }
    let py = note.py();
    let Ok(instrument) = py.import("music21.instrument") else {
        return Ok(None);
    };
    let mut found = py.None().into_bound(py);
    if note.hasattr("getContextByClass")? {
        let keywords = PyDict::new(py);
        keywords.set_item("followDerivation", false)?;
        found = note.call_method(
            "getContextByClass",
            (instrument.getattr("Instrument")?,),
            Some(&keywords),
        )?;
    }
    if !found.is_none() {
        return Ok(Some(found));
    }
    if return_default {
        return Ok(Some(instrument.getattr("Instrument")?.call0()?));
    }
    Ok(None)
}

/// music21's `GeneralNote.augmentOrDiminish`: the same note with its length
/// scaled, in place or as a copy.
///
/// Written against the Python object rather than against either facade,
/// because music21 writes it once on `GeneralNote` and both a note and a
/// chord inherit it — and because scaling is entirely a question for the
/// duration, whichever of the two is holding it.
pub(crate) fn augment_or_diminish_note<'py>(
    note: &Bound<'py, PyAny>,
    scalar: f64,
    in_place: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if scalar <= 0.0 || scalar.is_nan() {
        return Err(NoteException::new_err("scalar must be greater than zero"));
    }
    let target = target_note(note, in_place)?;
    let scaled = target
        .getattr("duration")?
        .call_method1("augmentOrDiminish", (scalar,))?;
    target.setattr("duration", scaled)?;
    Ok(if in_place { None } else { Some(target) })
}

/// music21's `GeneralNote.getGrace`: the same note written as it was and
/// sounding nothing.
pub(crate) fn grace_note<'py>(
    note: &Bound<'py, PyAny>,
    appoggiatura: bool,
    in_place: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let target = target_note(note, in_place)?;
    let grace = target
        .getattr("duration")?
        .call_method1("getGraceDuration", (appoggiatura,))?;
    target.setattr("duration", grace)?;
    Ok(if in_place { None } else { Some(target) })
}

/// The note the change lands on: this one, or a deep copy of it.
fn target_note<'py>(note: &Bound<'py, PyAny>, in_place: bool) -> PyResult<Bound<'py, PyAny>> {
    if in_place {
        return Ok(note.clone());
    }
    note.py()
        .import("copy")?
        .getattr("deepcopy")?
        .call1((note,))
}

/// A Python list holding whatever the value iterates over.
fn list_of(value: &Bound<'_, PyAny>) -> PyResult<Py<PyList>> {
    let list = PyList::empty(value.py());
    if !value.is_none() {
        for item in value.try_iter()? {
            list.append(item?)?;
        }
    }
    Ok(list.unbind())
}

/// Reads a duration argument: a `Duration`, a quarter length, or a type name
/// such as `"half"`.
/// The duration value an object stands for: one of ours as it stands, and
/// anything else — music21's own `GraceDuration`, say — by what it says its
/// length is.
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
        return RsDuration::new(quarter_length).map_err(note_error);
    }
    if let Ok(quarter_length) = value
        .getattr("quarterLength")
        .and_then(|value| value.extract::<f64>())
    {
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

    #[setter]
    fn set_slash(&mut self, value: bool) {
        self.slash = value;
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
    fn set_makeTime(&mut self, value: bool) {
        self.make_time = value;
    }
}

impl GraceDuration {
    /// Both halves of a grace duration copied into a blank of its own class.
    fn copied<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let class = slf.as_any().get_type();
        let copy = crate::blank_installed(class.as_any())?;
        let (written, marks) = {
            let me = slf.borrow();
            ((**me.as_super()).clone(), me.clone())
        };
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

/// music21's `note.Note`: a pitch with a duration.
#[pyclass(name = "Note", module = "music21.note", subclass, skip_from_py_object)]
pub struct Note {
    pub(crate) inner: RsNote,
    /// The `Pitch` object music21 hands back from `.pitch`. It is the same
    /// object every time, so `chord.pitches[0].getEnharmonic(inPlace=True)`
    /// reaches the chord; `inner` mirrors whatever it holds.
    pitch: Py<Pitch>,
    /// The `Volume` object music21 hands back from `.volume`, once something
    /// has asked for one. music21 makes it on demand and reads its mere
    /// existence as `hasVolumeInformation`.
    volume: Option<Py<Volume>>,
    /// The `Duration` object music21 hands back from `.duration`, once
    /// something has asked for one. Notes a chord builds are given the
    /// chord's, which is what makes `chord.duration is chord[0].duration`
    /// hold; a note that came with its own keeps it.
    /// The `Duration` object music21 hands back from `.duration`, which may
    /// be one of music21's own subclasses of it — a `GraceDuration` is what
    /// makes a note a grace note, and one replaced by a plain duration of
    /// ours would stop being one.
    duration: Option<Py<PyAny>>,
    /// The chord this note is part of: music21's `_chordAttached`, which its
    /// own `ChordBase` sets on every note it takes in. An edit to the pitch
    /// has to reach the chord through it.
    chord: Option<Py<PyAny>>,
    /// music21's `expressions` and `articulations`: the ornaments written
    /// over the note and the marks written under it.
    ///
    /// The crate models neither, and the lists hold whatever objects a
    /// caller puts in them. They are kept because they are where music21
    /// looks — `makeAccidentals` walks the expressions of every note to see
    /// whether an ornament needs an accidental of its own — and a note with
    /// no such list is a note music21's own notation code cannot process.
    expressions: Option<Py<PyList>>,
    articulations: Option<Py<PyList>>,
    /// The `Tie` object music21 hands back from `.tie`, once something has
    /// asked for one.
    ///
    /// music21's own `splitAtQuarterLength` writes through it — the middle
    /// of a note split across three bars is turned from a stop into a
    /// continue by `e.tie.type = 'continue'` — so the object has to be the
    /// note's own and not one made afresh each time.
    tie: Option<Py<Tie>>,
    /// music21's `lyrics`, as the list object itself.
    ///
    /// Its own MusicXML reader appends each verse to what `n.lyrics` hands
    /// back, so a getter that built a fresh list every time dropped every
    /// word the score was sung to. Once the list exists it is what the note
    /// is sung to, and `synced` writes it into the value.
    lyrics: Option<Py<PyList>>,
    /// music21's `storedInstrument`: the instrument this one note is played
    /// on, when it is not simply the one the part is written for. The crate
    /// models no instruments, so whatever object a caller stores is kept as
    /// it was given.
    stored_instrument: Option<Py<PyAny>>,
    /// Whatever was assigned through `pitches` that was not a pitch.
    ///
    /// music21's `pitches` setter takes the first item of the sequence and
    /// stores it as the pitch without looking at it, so `n.pitches = ('C4',)`
    /// leaves a *string* where the pitch should be, and its own docstring
    /// says so: "Don't use strings, or you will get a string back!". The
    /// note goes on answering every musical question from the pitch it
    /// already had, which is what music21 does too — nothing there reads the
    /// stored value except `.pitch`.
    unread_pitch: Option<Py<PyAny>>,
    /// music21's `style`, once something has asked for one: the object
    /// saying how this is drawn. It is music21's own object — the page is
    /// not something this crate models — and its mere existence is what
    /// `hasStyleInformation` answers, as music21's does.
    style: Option<Py<PyAny>>,
    /// music21's `beams`, as the object itself.
    ///
    /// Its own MusicXML reader reads a note's beams *into* what `n.beams`
    /// hands back — `xmlToBeams(mxBeamList, inputM21=n.beams)` — so a getter
    /// that built a fresh object every time dropped every beam the score
    /// wrote. Once the object exists it is what the note's beams are, and
    /// `synced` writes it into the value.
    beams: Option<Py<Beams>>,
}

impl Note {
    /// Builds the facade around a note, giving its pitch a Python object of
    /// its own.
    pub(crate) fn wrap(py: Python<'_>, inner: RsNote) -> PyResult<Self> {
        // Whether the spelling was chosen or given is the pitch's own answer:
        // `note.Note(63)` spells an E flat that nobody asked for by name.
        let inferred = inner.pitch().spelling_is_inferred();
        let pitch = crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            Pitch::wrap(inner.pitch().clone(), inferred),
        )?;
        Ok(Self {
            inner,
            pitch,
            volume: None,
            duration: None,
            chord: None,
            expressions: None,
            articulations: None,
            lyrics: None,
            tie: None,
            style: None,
            beams: None,
            stored_instrument: None,
            unread_pitch: None,
        })
    }

    /// A Python note object whose pitch already points back at it.
    pub(crate) fn object(py: Python<'_>, inner: RsNote) -> PyResult<Py<Self>> {
        let note = crate::installed_new(py, "music21.note", "Note", Self::wrap(py, inner)?)?;
        Self::claim_pitch(py, &note);
        Ok(note)
    }

    /// A note built around a pitch object the caller already holds, so that
    /// `chord.Chord([p1, p2]).pitches[0] is p1`, as music21's is.
    pub(crate) fn object_for_pitch(py: Python<'_>, pitch: Py<Pitch>) -> PyResult<Py<Self>> {
        let inner = RsNote::from_pitch(pitch.borrow(py).inner.clone());
        let note = crate::installed_new(
            py,
            "music21.note",
            "Note",
            Self {
                inner,
                pitch,
                volume: None,
                duration: None,
                chord: None,
                expressions: None,
                articulations: None,
                lyrics: None,
                tie: None,
                style: None,
                beams: None,
                stored_instrument: None,
                unread_pitch: None,
            },
        )?;
        Self::claim_pitch(py, &note);
        Ok(note)
    }

    /// Points the note's pitch object back at the note, so an edit through
    /// the pitch finds its way home.
    fn claim_pitch(py: Python<'_>, note: &Py<Self>) {
        let pitch = note.borrow(py).pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(note.clone_ref(py));
    }

    /// Takes the value a `Pitch` object now holds and passes it on: into the
    /// note, and through the note into its chord. The pitch object itself is
    /// left alone, because it is the one calling and is already borrowed.
    pub(crate) fn adopt_pitch(py: Python<'_>, note: &Py<Self>, pitch: &RsPitch) -> PyResult<()> {
        note.borrow_mut(py).inner.set_pitch(pitch.clone());
        Self::tell_chord(py, note, pitch)
    }

    /// Sends the note's own pitch the other way, out to its pitch object and
    /// to its chord: what a setter on the note ends with.
    /// The note's pitch as it stands now.
    ///
    /// The object is the source: a caller holds it and edits it, and one
    /// pitch object may belong to more than one note — music21's `chordify`
    /// and `Verticality.makeElement` hand a chord the very pitches of the
    /// notes they were built from, so renaming one there renames the note it
    /// came off.
    pub(crate) fn pitch_value(&self, py: Python<'_>) -> RsPitch {
        self.pitch.borrow(py).inner.clone()
    }

    pub(crate) fn broadcast_pitch(py: Python<'_>, note: &Py<Self>) -> PyResult<()> {
        let (object, value) = {
            let me = note.borrow(py);
            (me.pitch.clone_ref(py), me.inner.pitch().clone())
        };
        object.borrow_mut(py).inner = value.clone();
        Self::tell_chord(py, note, &value)
    }

    /// Writes a new pitch for this note into the chord holding it, when a
    /// chord of ours is holding it.
    fn tell_chord(py: Python<'_>, note: &Py<Self>, pitch: &RsPitch) -> PyResult<()> {
        let Some(chord) = note
            .borrow(py)
            .chord
            .as_ref()
            .map(|chord| chord.clone_ref(py))
        else {
            return Ok(());
        };
        // music21's own `ChordBase` writes itself here as well; it keeps its
        // own pitches and wants nothing from us.
        let Ok(chord) = chord.cast_bound::<crate::chord::Chord>(py).cloned() else {
            return Ok(());
        };
        crate::chord::Chord::adopt_note_pitch(&chord, note, pitch)
    }

    /// Tells the note which chord holds it, and makes sure its pitch object
    /// knows the note.
    pub(crate) fn attach_to_chord(py: Python<'_>, note: &Py<Self>, chord: &Bound<'_, PyAny>) {
        note.borrow_mut(py).chord = Some(chord.clone().unbind());
        Self::claim_pitch(py, note);
    }

    /// The duration this note actually has: the object music21 hands out
    /// when something has asked for one, since an edit through that object
    /// is an edit to the note, and `inner`'s otherwise.
    pub(crate) fn duration_value(&self, py: Python<'_>) -> Option<RsDuration> {
        match &self.duration {
            Some(object) => duration_value_of(py, object),
            None => self.inner.duration().cloned(),
        }
    }

    /// The `Duration` object for this note, made on first asking as music21
    /// makes one on first asking.
    pub(crate) fn duration_object(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(object) = &self.duration {
            return Ok(object.clone_ref(py));
        }
        let inner = self
            .inner
            .duration()
            .cloned()
            .unwrap_or_else(RsDuration::quarter);
        let created =
            crate::installed_new(py, "music21.duration", "Duration", Duration::wrap(inner))?
                .into_any();
        self.duration = Some(created.clone_ref(py));
        Ok(created)
    }

    /// Takes whatever a caller wrote as a duration and holds it, as an
    /// object if that is what was given and as a fresh one if not.
    pub(crate) fn attach_duration(
        &mut self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let inner = duration_from_any(value)?;
        // A duration object is kept as it stands, whether it is one of ours
        // or one of music21's own kinds of duration; anything else — a
        // length, a note-value name — becomes one of ours.
        let duration = if value.hasattr("quarterLength")? {
            value.clone().unbind()
        } else {
            crate::installed_new(
                py,
                "music21.duration",
                "Duration",
                Duration::wrap(inner.clone()),
            )?
            .into_any()
        };
        self.inner.set_duration(inner);
        self.duration = Some(duration.clone_ref(py));
        Ok(duration)
    }

    /// Writes a volume straight in, letting go of whatever object was
    /// standing for the old one.
    pub(crate) fn replace_volume(&mut self, volume: Option<music21_rs::Volume>) {
        self.inner.set_volume(volume);
        self.volume = None;
    }

    /// The colour the note is written in: what its style says if it has
    /// one, since that is where music21 keeps it, and what the value says
    /// otherwise.
    pub(crate) fn colour(&self, py: Python<'_>) -> Option<String> {
        if self.style.is_some() {
            return crate::notation::style_colour(py, self.style.as_ref());
        }
        self.inner.color().map(str::to_string)
    }

    /// Folds the lyric objects into the value and lets them go, so a method
    /// that works on the value works on what Python has actually got and the
    /// next reader builds the objects again from the answer.
    fn settle_lyrics(&mut self, py: Python<'_>) {
        let Some(lyrics) = self.lyrics.take() else {
            return;
        };
        let verses: Vec<music21_rs::notation::Lyric> = lyrics
            .bind(py)
            .iter()
            .filter_map(|verse| verse.extract::<PyRef<'_, Lyric>>().ok())
            .map(|verse| verse.synced(py))
            .collect();
        let held = self.inner.lyrics_mut();
        held.clear();
        held.extend(verses);
    }

    /// Hands this note a `Duration` object to share, the way a chord shares
    /// its own with the notes it builds.
    pub(crate) fn share_duration(&mut self, py: Python<'_>, duration: &Py<PyAny>) {
        if let Some(value) = duration_value_of(py, duration) {
            self.inner.set_duration(value);
        }
        self.duration = Some(duration.clone_ref(py));
    }

    fn quarter_length(&self, py: Python<'_>) -> f64 {
        self.duration_value(py)
            .as_ref()
            .map_or(1.0, RsDuration::quarter_length)
    }

    /// This note with whatever its duration and volume objects now say
    /// written into it, for the answers the crate reads off a whole note.
    pub(crate) fn synced(&self, py: Python<'_>) -> RsNote {
        let mut note = self.inner.clone();
        // The pitch object is what a caller holds and edits, and one pitch
        // object may belong to more than one note: music21's `chordify` with
        // `copyPitches=False` hands a chord the very pitches of the notes it
        // was built from, and raising one of those raises the note it came
        // off. Reading the object rather than only writing to it is what
        // makes that hold.
        note.set_pitch(self.pitch.borrow(py).inner.clone());
        // music21's own readers write beams into the object `n.beams` gave
        // them, so the object is where the score's beaming is.
        if let Some(beams) = &self.beams {
            note.set_beams(beams.borrow_mut(py).settled_value(py));
        }
        if let Some(duration) = self.duration_value(py) {
            note.set_duration(duration);
        }
        if let Some(volume) = &self.volume {
            note.set_volume(Some(volume.borrow(py).inner.clone()));
        }
        // music21 keeps the colour on the style, so an edit through
        // `n.style.color` is an edit to the note.
        if self.style.is_some() {
            note.set_color(crate::notation::style_colour(py, self.style.as_ref()));
        }
        // music21 keeps the tie on an object a caller may still be holding,
        // and its own note-splitting writes through it.
        if let Some(tie) = &self.tie {
            note.set_tie(Some(tie.borrow(py).inner.clone()));
        }
        // Every note music21 has sounds for some length. The crate lets a
        // note carry none — a pitch nobody has said a length for — but one
        // reached through here has been asked as music21 asks, and music21's
        // answer for a note nobody has timed is a quarter.
        if note.duration().is_none() {
            note.set_duration(RsDuration::quarter());
        }
        if let Some(lyrics) = &self.lyrics {
            let verses = note.lyrics_mut();
            verses.clear();
            verses.extend(
                lyrics
                    .bind(py)
                    .iter()
                    .filter_map(|verse| verse.extract::<PyRef<'_, Lyric>>().ok())
                    .map(|verse| verse.synced(py)),
            );
        }
        note
    }

    /// music21 orders notes by pitch alone, and refuses anything without a
    /// `.pitch` — its `__lt__` answers `NotImplemented` and Python raises.
    /// The message is written out here rather than left to Python because
    /// pyo3 puts the module into the type name, so CPython's own wording
    /// would say `music21.note.Note` where music21 says `Note`.
    fn ordered(
        slf: &Bound<'_, Self>,
        other: &Bound<'_, PyAny>,
        operator: &str,
        compare: impl Fn(f64, f64) -> bool,
    ) -> PyResult<bool> {
        let Ok(pitch) = other
            .getattr("pitch")
            .and_then(|pitch| pitch_from_any(&pitch))
        else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "'{operator}' not supported between instances of '{}' and '{}'",
                slf.get_type().name()?,
                other.get_type().name()?,
            )));
        };
        Ok(compare(slf.borrow().pitch_value(slf.py()).ps(), pitch.ps()))
    }

    /// A detached copy: new pitch and duration objects, and no chord.
    /// A copy of the note as a Python object of the class it was asked on,
    /// with its pitch a copy of the very pitch object it had — so anything
    /// written on that pitch, such as the part name `chordify` tags it
    /// with, comes across — and the copy owning its pitch as the original
    /// did.
    fn copied_object<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let mut copied = slf.borrow().copied(py)?;
        // Through the copier's own record of what it has copied, so a pitch
        // object something else holds as well — a chord symbol fixes its
        // root to the pitch of one of its notes — is copied once and shared
        // by the copies, as music21's own copying shares it.
        let deepcopy = py.import("copy")?.getattr("deepcopy")?;
        let held = slf.borrow().pitch.bind(py).clone();
        let pitch = match memo {
            Some(memo) => deepcopy.call1((held, memo))?,
            None => deepcopy.call1((held,))?,
        };
        if let Ok(pitch) = pitch.extract::<Py<Pitch>>() {
            copied.pitch = pitch;
        }
        let object = crate::copy_as_same_type(slf, copied)?;
        Self::claim_pitch(py, &object.clone().cast_into::<Self>()?.unbind());
        Ok(object)
    }

    fn copied(&self, py: Python<'_>) -> PyResult<Self> {
        let mut copy = Self::wrap(py, self.synced(py))?;
        // The ornaments and the marks come across: music21 copies a note to
        // realize a mordent and then takes the mordent off the copy, and a
        // copy with none would have nothing to take.
        copy.expressions = copied_list(py, self.expressions.as_ref())?;
        copy.articulations = copied_list(py, self.articulations.as_ref())?;
        copy.style = crate::notation::copied_style(py, self.style.as_ref());
        // The duration comes across as the kind of duration it is: a grace
        // note whose copy carried a plain duration would stop being one.
        if let Some(duration) = &self.duration {
            copy.duration = Some(copied_duration(py, duration)?);
        }
        Ok(copy)
    }
}

/// A list copied the way `copy.deepcopy` would copy it, which is what a
/// note's ornaments and marks are when the note is copied.
pub(crate) fn copied_list(
    py: Python<'_>,
    list: Option<&Py<PyList>>,
) -> PyResult<Option<Py<PyList>>> {
    let Some(list) = list else {
        return Ok(None);
    };
    let copier = py.import("copy")?.getattr("deepcopy")?;
    let copied = PyList::empty(py);
    for item in list.bind(py).iter() {
        copied.append(copier.call1((item,))?)?;
    }
    Ok(Some(copied.unbind()))
}

/// Reads a note argument: a `Note`, a pitch, or a name. A facade note comes
/// back with its duration object's value written in, since that object is
/// where an edit like `n.duration.type = 'half'` landed.
pub(crate) fn note_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsNote> {
    if let Ok(facade) = value.extract::<PyRef<Note>>() {
        return Ok(facade.synced(value.py()));
    }
    Ok(RsNote::from_pitch(pitch_from_any(value)?))
}

#[pymethods]
impl Note {
    /// A note is written out as text and read back, and its pitch and its
    /// duration are made again from what it says. Its ornaments and marks
    /// are Python objects the crate does not model, so they are frozen
    /// beside it — music21 freezes every score it parses, and a thawed note
    /// with no articulations on it would have lost what the score said.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let note = slf.borrow();
            extra.set_item("expressions", note.expressions.as_ref())?;
            extra.set_item("articulations", note.articulations.as_ref())?;
            extra.set_item("storedInstrument", note.stored_instrument.as_ref())?;
            // The duration object goes with it: how long the note sounds is
            // in the value, but the tuplets it is written inside and whether
            // it is a grace note are the object's, and a score is frozen to
            // a file and read back.
            extra.set_item("duration", note.duration.as_ref())?;
        }
        crate::pickled_extra(slf, &slf.borrow().synced(py), Some(&extra))
    }

    fn __setstate__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (inner, extra) = crate::unpickled_extra::<_, RsNote>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        let rebuilt = Self::wrap(py, inner)?;
        *slf.borrow_mut() = rebuilt;
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(slf.clone().unbind());
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            let mut note = slf.borrow_mut();
            note.expressions = extra.get_item("expressions")?.extract().ok();
            note.articulations = extra.get_item("articulations")?.extract().ok();
            note.duration = extra
                .get_item("duration")
                .ok()
                .filter(|duration| !duration.is_none())
                .map(pyo3::Bound::unbind);
            note.stored_instrument = Some(extra.get_item("storedInstrument")?.unbind())
                .filter(|value| !value.is_none(py));
        }
        Ok(())
    }

    #[new]
    #[pyo3(signature = (pitch = None, **keywords))]
    fn new(
        py: Python<'_>,
        pitch: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let inner = match pitch.filter(|value| !value.is_none()) {
            Some(value) => RsNote::from_pitch(crate::pitch::pitch_from_any_with_keywords(
                py, value, keywords,
            )?),
            None => match crate::pitch::pitch_from_keywords(py, keywords)? {
                Some(pitch) => RsNote::from_pitch(pitch),
                None => RsNote::from_name("C4").map_err(note_error)?,
            },
        };
        let mut note = Self::wrap(py, inner)?;
        // A note built on a pitch object keeps that object, as music21's
        // does: its own harmony code fixes a chord's root to the pitch of
        // one of its notes, and moving the note has to move the root.
        if let Some(given) = pitch
            .filter(|value| !value.is_none())
            .and_then(|value| value.cast::<Pitch>().ok())
        {
            note.inner.set_pitch(given.borrow().inner.clone());
            note.pitch = given.clone().unbind();
        }
        // music21 hands the same keywords on to `Duration`, so `type='eighth',
        // dots=2` is an eighth with two dots and not a quarter.
        if Duration::keywords_say_duration(keywords)? {
            let duration = match keywords.and_then(|keywords| keywords.get_item("duration").ok()?) {
                Some(value) => value,
                None => Py::new(py, Duration::new(None, keywords)?)?
                    .into_bound(py)
                    .into_any(),
            };
            note.attach_duration(py, &duration)?;
        }
        Ok(note)
    }

    /// The note's own `Pitch` object, the same one every time: an edit
    /// through it is an edit to the note, and to the chord holding the note.
    pub(crate) fn get_pitch(slf: &Bound<'_, Self>) -> Py<Pitch> {
        let py = slf.py();
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.borrow_mut(py).owner = Some(slf.clone().unbind());
        pitch
    }

    /// Setting it keeps the pitch object given, as music21 does: its own
    /// `Verticality.makeElement` gives a copied note the very pitch of the
    /// note it was copied from, and renaming it there renames both.
    #[setter]
    fn set_pitch(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if let Ok(object) = value.extract::<Py<Pitch>>() {
            let inner = object.borrow(py).inner.clone();
            {
                let mut me = slf.borrow_mut();
                me.pitch = object;
                me.inner.set_pitch(inner.clone());
                me.unread_pitch = None;
            }
            let note = slf.clone().unbind();
            Self::claim_pitch(py, &note);
            return Self::tell_chord(py, &note, &inner);
        }
        let pitch = pitch_from_any(value)?;
        {
            let mut me = slf.borrow_mut();
            me.inner.set_pitch(pitch);
            me.unread_pitch = None;
        }
        Self::broadcast_pitch(py, &slf.clone().unbind())
    }

    /// music21's `.pitch`, which is whatever is stored there — a `Pitch`
    /// unless something put a bare value in through `pitches`.
    #[getter(pitch)]
    fn pitch_attribute(slf: &Bound<'_, Self>) -> Py<PyAny> {
        let py = slf.py();
        if let Some(unread) = &slf.borrow().unread_pitch {
            return unread.clone_ref(py);
        }
        Self::get_pitch(slf).into_any()
    }

    #[getter]
    fn get_name(&self, py: Python<'_>) -> String {
        self.pitch_value(py).name()
    }

    #[setter]
    fn set_name(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        let name = {
            let me = slf.borrow();
            match me.inner.pitch().octave() {
                Some(octave) if !value.chars().any(|ch| ch.is_ascii_digit()) => {
                    format!("{value}{octave}")
                }
                _ => value.to_string(),
            }
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    #[getter]
    fn get_nameWithOctave(&self, py: Python<'_>) -> String {
        self.pitch_value(py).name_with_octave()
    }

    /// Setting it renames the note's pitch, which is what music21 does.
    #[setter]
    fn set_nameWithOctave(slf: &Bound<'_, Self>, py: Python<'_>, value: &str) -> PyResult<()> {
        let pitch = slf.borrow().pitch.clone_ref(py);
        pitch.bind(py).setattr("nameWithOctave", value)?;
        let renamed = pitch.borrow(py).inner.clone();
        slf.borrow_mut().inner.set_pitch(renamed);
        Ok(())
    }

    #[getter]
    fn get_step(&self, py: Python<'_>) -> String {
        self.pitch_value(py)
            .name()
            .chars()
            .next()
            .unwrap_or('C')
            .to_string()
    }

    /// music21's `step` setter, which writes through to the pitch and keeps
    /// the accidental and the octave: `n.step = 'D'` on a `C#4` gives `D#4`.
    #[setter]
    fn set_step(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        // Through the note's own pitch object, so the change lands where a
        // caller holding that pitch will see it, and so the one reading of
        // a step name lives in one place.
        Self::get_pitch(slf).setattr(slf.py(), "step", value)
    }

    #[getter]
    fn get_octave(&self, py: Python<'_>) -> Option<i32> {
        self.pitch_value(py).octave()
    }

    #[setter]
    fn set_octave(slf: &Bound<'_, Self>, value: Option<i32>) -> PyResult<()> {
        let name = {
            let me = slf.borrow();
            match value {
                Some(octave) => format!("{}{octave}", me.inner.pitch().name()),
                None => me.inner.pitch().name(),
            }
        };
        let pitch = RsPitch::from_name(name).map_err(note_error)?;
        slf.borrow_mut().inner.set_pitch(pitch);
        Self::broadcast_pitch(slf.py(), &slf.clone().unbind())
    }

    /// music21's `.pitches`, the chord-shaped view of a note: its one pitch
    /// in a tuple.
    #[getter]
    fn get_pitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(slf.py(), [Self::get_pitch(slf)])
    }

    /// Setting it takes the first pitch of a list or tuple and ignores the
    /// rest, since a note has only one; anything that is not a sequence is
    /// refused.
    #[setter]
    fn set_pitches(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let refused = || {
            NoteException::new_err(format!(
                "cannot set pitches with provided object: {}",
                value
                    .str()
                    .map_or_else(|_| "?".to_string(), |v| v.to_string())
            ))
        };
        if !value.is_instance_of::<PyList>() && !value.is_instance_of::<PyTuple>() {
            return Err(refused());
        }
        let Some(first) = value.try_iter()?.next() else {
            return Err(refused());
        };
        let first = first?;
        // music21 stores the first item without looking at it. A value that
        // is not a pitch is kept as it was given and handed back by `.pitch`,
        // which is the footgun its own docstring warns about.
        if pitch_from_any(&first).is_err() || first.extract::<String>().is_ok() {
            slf.borrow_mut().unread_pitch = Some(first.unbind());
            return Ok(());
        }
        Self::set_pitch(slf, &first)
    }

    #[getter]
    fn fullName(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<String> {
        let value = slf.borrow().synced(py);
        let named = value.full_name();
        let timed = value
            .duration()
            .map(RsDuration::full_name)
            .unwrap_or_default();
        Ok(crate::note::named_with_duration(
            slf.as_any(),
            &named,
            &timed,
        ))
    }

    /// music21's `.duration`, the same object every time: `n.duration.type =
    /// 'half'` is how music21's own doctests lengthen a note.
    #[getter]
    fn get_duration(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let duration = slf.borrow_mut().duration_object(py)?;
        adopt_duration(py, &duration, slf.as_any());
        Ok(duration)
    }

    #[setter]
    fn set_duration(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let had_one = slf.borrow().duration.is_some();
        let duration = slf.borrow_mut().attach_duration(py, value)?;
        adopt_duration(py, &duration, slf.as_any());
        // Replacing the duration a note already had changes how long the
        // note is, and the streams holding it keep that length; music21
        // tells them so here, and a note whose length nobody has asked for
        // yet has nothing to tell.
        if had_one {
            told_sites(slf.as_any(), &duration.bind(py).getattr("quarterLength")?)?;
        }
        Ok(())
    }

    #[getter]
    fn get_quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        op_frac(py, self.quarter_length(py))
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        let inner = RsDuration::new(value).map_err(note_error)?;
        self.inner.set_duration(inner.clone());
        match &self.duration {
            // Through the duration's own setter, so the written values it
            // stands for are worked out again: a whole note lengthened to
            // four and three quarters is written as two notes tied.
            Some(duration) => duration.bind(py).setattr("quarterLength", value)?,
            None => {
                self.duration = Some(
                    crate::installed_new(
                        py,
                        "music21.duration",
                        "Duration",
                        Duration::wrap(inner),
                    )?
                    .into_any(),
                );
            }
        }
        Ok(())
    }

    #[getter]
    fn isRest(&self) -> bool {
        false
    }

    #[getter]
    fn isNote(&self) -> bool {
        true
    }

    #[getter]
    fn isChord(&self) -> bool {
        false
    }

    // ---- notation --------------------------------------------------------

    #[getter]
    pub(crate) fn get_tie(slf: &Bound<'_, Self>) -> PyResult<Option<Py<Tie>>> {
        let py = slf.py();
        if let Some(tie) = &slf.borrow().tie {
            return Ok(Some(tie.clone_ref(py)));
        }
        let Some(value) = slf.borrow().inner.tie().cloned() else {
            return Ok(None);
        };
        let tie = crate::installed_new(py, "music21.tie", "Tie", Tie::wrap(value))?;
        slf.borrow_mut().tie = Some(tie.clone_ref(py));
        Ok(Some(tie))
    }

    /// A tie object handed over is kept, as music21 keeps it: its own
    /// `splitAtQuarterLength` writes through the object it reads back.
    #[setter]
    pub(crate) fn set_tie(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let Some(value) = value.filter(|value| !value.is_none()) else {
            let mut note = slf.borrow_mut();
            note.inner.set_tie(None);
            note.tie = None;
            return Ok(());
        };
        let inner = tie_from_any(value)?;
        let tie = match value.extract::<Py<Tie>>() {
            Ok(object) => object,
            Err(_) => crate::installed_new(py, "music21.tie", "Tie", Tie::wrap(inner.clone()))?,
        };
        let mut note = slf.borrow_mut();
        note.inner.set_tie(Some(inner));
        note.tie = Some(tie);
        Ok(())
    }

    #[getter]
    fn get_notehead(&self) -> &'static str {
        self.inner.notehead().as_str()
    }

    #[setter]
    pub(crate) fn set_notehead(&mut self, value: Option<&str>) -> PyResult<()> {
        let notehead = match value {
            None | Some("") => RsNotehead::Normal,
            Some(name) => RsNotehead::from_name(name).map_err(not_rest_error)?,
        };
        self.inner.set_notehead(notehead);
        Ok(())
    }

    #[getter]
    fn get_noteheadFill(&self) -> Option<bool> {
        self.inner.notehead_fill()
    }

    #[setter]
    pub(crate) fn set_noteheadFill(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let fill = if value.is_none() {
            None
        } else if let Ok(flag) = value.extract::<bool>() {
            Some(flag)
        } else {
            match value.extract::<String>()?.as_str() {
                "none" | "default" => None,
                "filled" | "yes" => Some(true),
                "notfilled" | "no" => Some(false),
                other => {
                    return Err(not_rest_error(music21_rs::Error::Notation(format!(
                        "not a valid notehead fill value: '{other}'"
                    ))));
                }
            }
        };
        self.inner.set_notehead_fill(fill);
        Ok(())
    }

    #[getter]
    fn get_noteheadParenthesis(&self) -> bool {
        self.inner.notehead_parenthesis()
    }

    #[setter]
    fn set_noteheadParenthesis(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let parenthesis = if let Ok(flag) = value.extract::<bool>() {
            flag
        } else if let Ok(number) = value.extract::<i64>() {
            number != 0
        } else {
            match value.extract::<String>()?.as_str() {
                "yes" => true,
                "no" => false,
                other => {
                    return Err(not_rest_error(music21_rs::Error::Notation(format!(
                        "notehead parentheses must be True or False, not '{other}'"
                    ))));
                }
            }
        };
        self.inner.set_notehead_parenthesis(parenthesis);
        Ok(())
    }

    #[getter]
    fn get_stemDirection(&self) -> &'static str {
        self.inner.stem_direction().as_str()
    }

    #[setter]
    pub(crate) fn set_stemDirection(&mut self, value: Option<&str>) -> PyResult<()> {
        let direction = match value {
            None => RsStemDirection::Unspecified,
            Some(name) => RsStemDirection::from_name(name).map_err(not_rest_error)?,
        };
        self.inner.set_stem_direction(direction);
        Ok(())
    }

    /// music21's `style`: the object saying how this is drawn, made on
    /// first asking and the same one after that. It is music21's own — the
    /// page is not something this crate models — with the colour, which it
    /// does model, written into it.
    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let colour = slf.borrow().inner.color().map(str::to_string);
        let style = crate::notation::new_style(slf.as_any(), colour.as_deref())?;
        slf.borrow_mut().style = Some(style.clone_ref(py));
        Ok(style)
    }

    #[setter]
    fn set_style(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let colour: Option<String> = value
            .getattr("color")
            .ok()
            .and_then(|colour| colour.extract().ok());
        slf.borrow_mut().inner.set_color(colour);
        slf.borrow_mut().style = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `hasStyleInformation`: whether a style object has been made
    /// for this yet, which is what its own code asks before making one.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    /// music21's `.volume`, made on first asking and the same object after
    /// that, so `n.volume.velocity = 20` sticks.
    #[getter]
    pub(crate) fn get_volume(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<Volume>> {
        if let Some(volume) = &slf.borrow().volume {
            return Ok(volume.clone_ref(py));
        }
        // The volume knows whose it is, which is how music21 tells a volume
        // already spoken for from a loose one.
        let inner = slf.borrow().inner.volume();
        let created = crate::installed_new(
            py,
            "music21.volume",
            "Volume",
            Volume::owned_by(inner, slf.clone().into_any().unbind()),
        )?;
        slf.borrow_mut().volume = Some(created.clone_ref(py));
        Ok(created)
    }

    /// music21 takes the volume object itself when nothing else has claimed
    /// it, and a copy of it when something has — either way the note is what
    /// the volume is the volume of, which is what `Volume.client` answers.
    #[setter]
    fn set_volume(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            let mut me = slf.borrow_mut();
            me.inner.set_volume(None);
            me.volume = None;
            return Ok(());
        };
        let inner = volume_from_any(value)?;
        slf.borrow_mut().inner.set_volume(Some(inner.clone()));
        let object = match value.extract::<Py<Volume>>() {
            Ok(object) => {
                if object.borrow(py).is_claimed() {
                    crate::installed_new(py, "music21.volume", "Volume", object.borrow(py).clone())?
                } else {
                    object
                }
            }
            Err(_) => crate::installed_new(py, "music21.volume", "Volume", Volume::wrap(inner))?,
        };
        object.borrow_mut(py).set_client(Some(slf.as_any()));
        slf.borrow_mut().volume = Some(object);
        Ok(())
    }

    /// Whether this note carries a volume at all. music21 asks only whether
    /// the object is there, which is why reading `.volume` once makes this
    /// true.
    fn hasVolumeInformation(&self) -> bool {
        self.volume.is_some()
    }

    /// music21's `lyrics`: the list itself, the same one every time. Its
    /// own MusicXML reader appends each verse to what this hands back.
    #[getter]
    pub(crate) fn get_lyrics<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        if let Some(lyrics) = &slf.borrow().lyrics {
            return Ok(lyrics.bind(py).clone());
        }
        let verses = slf.borrow().inner.lyrics().to_vec();
        let list = PyList::empty(py);
        for verse in verses {
            list.append(crate::installed_new(
                py,
                "music21.note",
                "Lyric",
                Lyric::wrap(verse),
            )?)?;
        }
        slf.borrow_mut().lyrics = Some(list.clone().unbind());
        Ok(list)
    }

    /// Setting them replaces every verse at once, which is how music21 says
    /// a note is sung to something else, or to nothing.
    #[setter]
    pub(crate) fn set_lyrics(
        slf: &Bound<'_, Self>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let list = PyList::empty(py);
        if let Some(value) = value.filter(|value| !value.is_none()) {
            for item in value.try_iter()? {
                let item = item?;
                if item.extract::<PyRef<'_, Lyric>>().is_ok() {
                    list.append(item)?;
                    continue;
                }
                let verse = music21_rs::notation::Lyric::new(item.extract::<String>()?);
                list.append(crate::installed_new(
                    py,
                    "music21.note",
                    "Lyric",
                    Lyric::wrap(verse),
                )?)?;
            }
        }
        let mut note = slf.borrow_mut();
        note.inner.lyrics_mut().clear();
        note.lyrics = Some(list.unbind());
        Ok(())
    }

    #[getter]
    fn get_lyric(&self, py: Python<'_>) -> Option<String> {
        // Through the value as it stands: a verse appended to the list the
        // note handed out has not reached the value until something reads it.
        self.synced(py).lyric()
    }

    /// music21 takes a string here, splitting it into a verse per line, or a
    /// `Lyric` to hold as the one verse, or `None` to clear them.
    #[setter]
    pub(crate) fn set_lyric(
        &mut self,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.settle_lyrics(py);
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return self.inner.set_lyric(None).map_err(note_error);
        };
        if let Ok(lyric) = value.extract::<PyRef<'_, Lyric>>() {
            self.inner.set_lyric(None).map_err(note_error)?;
            self.inner.lyrics_mut().push(lyric.inner.clone());
            return Ok(());
        }
        let text = value.str()?.to_string();
        self.inner.set_lyric(Some(&text)).map_err(note_error)
    }

    /// music21's `insertLyric`: puts a syllable in front of the verse at
    /// `index` and moves the rest down a line.
    #[pyo3(signature = (text, index = 0, *, applyRaw = false, identifier = None))]
    pub(crate) fn insertLyric(
        &mut self,
        text: &Bound<'_, PyAny>,
        index: usize,
        applyRaw: bool,
        identifier: Option<String>,
    ) -> PyResult<()> {
        self.settle_lyrics(text.py());
        let text = if text.is_none() {
            String::new()
        } else {
            text.str()?.to_string()
        };
        self.inner
            .insert_lyric(&text, index, applyRaw)
            .map_err(note_error)?;
        if identifier.is_some()
            && let Some(lyric) = self.inner.lyrics_mut().get_mut(index)
        {
            lyric.set_identifier(identifier);
        }
        Ok(())
    }

    #[pyo3(signature = (text, lyricNumber = None, *, applyRaw = false, lyricIdentifier = None))]
    pub(crate) fn addLyric(
        &mut self,
        text: &Bound<'_, PyAny>,
        lyricNumber: Option<i32>,
        applyRaw: bool,
        lyricIdentifier: Option<String>,
    ) -> PyResult<()> {
        self.settle_lyrics(text.py());
        let text = if text.is_none() {
            String::new()
        } else {
            text.str()?.to_string()
        };
        self.inner
            .add_lyric(&text, lyricNumber, applyRaw)
            .map_err(note_error)?;
        if lyricIdentifier.is_some()
            && let Some(lyric) = self.inner.lyrics_mut().last_mut()
        {
            lyric.set_identifier(lyricIdentifier);
        }
        Ok(())
    }

    /// music21's `GeneralNote.augmentOrDiminish`.
    #[pyo3(signature = (scalar, *, inPlace = false))]
    fn augmentOrDiminish<'py>(
        slf: &Bound<'py, Self>,
        scalar: f64,
        inPlace: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        augment_or_diminish_note(slf.as_any(), scalar, inPlace)
    }

    /// music21's `GeneralNote.getGrace`.
    #[pyo3(signature = (*, appoggiatura = false, inPlace = false))]
    fn getGrace<'py>(
        slf: &Bound<'py, Self>,
        appoggiatura: bool,
        inPlace: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        grace_note(slf.as_any(), appoggiatura, inPlace)
    }

    #[pyo3(signature = (value, *, inPlace = false))]
    fn transpose(
        slf: &Bound<'_, Self>,
        value: &Bound<'_, PyAny>,
        inPlace: bool,
    ) -> PyResult<Option<Py<Self>>> {
        let py = slf.py();
        let mut pitch = transpose_pitch_by_any(slf.borrow().inner.pitch(), value)?;
        // A move given as a number of semitones says how far, not how to
        // spell what it lands on, so music21 lets the key signature in force
        // decide: a semitone above F is F# in D major and G- in B-flat minor.
        if value.extract::<i32>().is_ok()
            && let Some(signature) = key_signature_around(slf.as_any())?
        {
            pitch = pitch.respelled_for(&signature).map_err(note_error)?;
        }
        if inPlace {
            slf.borrow_mut().inner.set_pitch(pitch);
            Self::broadcast_pitch(py, &slf.clone().unbind())?;
            return Ok(None);
        }
        let mut moved = RsNote::from_pitch(pitch);
        if let Some(duration) = slf.borrow().inner.duration().cloned() {
            moved.set_duration(duration);
        }
        let mut copy = Self::wrap(py, moved)?;
        copy.style = crate::notation::copied_style(py, slf.borrow().style.as_ref());
        let copy = crate::copy_as_same_type(slf, copy)?;
        Self::claim_pitch(py, &copy.clone().cast_into::<Self>()?.unbind());
        crate::derived_from(&copy, slf.as_any(), "transpose")?;
        Ok(Some(copy.cast_into::<Self>()?.unbind()))
    }

    /// music21 compares two notes on what it lists as their equality
    /// attributes: the pitch, the duration, the tie, and how the note is
    /// written — its notehead and its beams. What is sung to it and how loud
    /// it is do not count; the ornaments over it and the marks under it do,
    /// by their kinds rather than by the objects themselves.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.extract::<PyRef<Note>>() else {
            return Ok(false);
        };
        let (mine, theirs) = (self.synced(py), other.synced(py));
        let same = mine.pitch() == theirs.pitch()
            && self.quarter_length(py) == other.quarter_length(py)
            && mine.tie() == theirs.tie()
            && mine.notehead() == theirs.notehead()
            && mine.notehead_fill() == theirs.notehead_fill()
            && mine.notehead_parenthesis() == theirs.notehead_parenthesis()
            && mine.beams() == theirs.beams();
        if !same {
            return Ok(false);
        }
        Ok(
            same_kinds(py, self.expressions.as_ref(), other.expressions.as_ref())?
                && same_kinds(
                    py,
                    self.articulations.as_ref(),
                    other.articulations.as_ref(),
                )?,
        )
    }

    fn __lt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, "<", |left, right| left < right)
    }

    fn __le__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, "<=", |left, right| left <= right)
    }

    fn __gt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, ">", |left, right| left > right)
    }

    fn __ge__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Self::ordered(slf, other, ">=", |left, right| left >= right)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.note.{} {}>",
            slf.get_type().qualname()?,
            slf.borrow().pitch_value(slf.py()).name()
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::copied_object(slf, py, Some(memo))
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Self::copied_object(slf, py, None)
    }

    /// music21's `storedInstrument`.
    #[getter]
    fn get_storedInstrument(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.stored_instrument
            .as_ref()
            .map(|instrument| instrument.clone_ref(py))
    }

    #[setter]
    fn set_storedInstrument(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.stored_instrument = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// music21's `NotRest.getInstrument`.
    #[pyo3(signature = (*, returnDefault = true))]
    fn getInstrument<'py>(
        slf: &Bound<'py, Self>,
        returnDefault: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_for_note(slf.as_any(), returnDefault)
    }

    /// music21's `beams`: the beams joining this note's flags to its
    /// neighbours'. The object knows the note it came off, so an edit
    /// through it is an edit to the note.
    #[getter]
    fn get_beams(slf: &Bound<'_, Self>) -> PyResult<Py<Beams>> {
        let py = slf.py();
        if let Some(beams) = &slf.borrow().beams {
            return Ok(beams.clone_ref(py));
        }
        let beams = crate::installed_new(
            py,
            "music21.beam",
            "Beams",
            Beams::wrap(slf.borrow().inner.beams().clone()),
        )?;
        slf.borrow_mut().beams = Some(beams.clone_ref(py));
        Ok(beams)
    }

    /// Setting them keeps the object given, as music21 does: its own
    /// `stripTies` clears a note's beams by handing it a fresh `Beams()` and
    /// writing into that afterwards.
    #[setter]
    fn set_beams(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let beams = value.extract::<Py<Beams>>()?;
        self.inner.set_beams(beams.borrow(py).inner.clone());
        self.beams = Some(beams);
        Ok(())
    }

    /// music21's `expressions`: the ornaments written over this note. The
    /// same list every time, so appending to it sticks.
    #[getter]
    fn get_expressions(&mut self, py: Python<'_>) -> Py<PyList> {
        let list = self
            .expressions
            .get_or_insert_with(|| PyList::empty(py).unbind());
        list.clone_ref(py)
    }

    #[setter]
    fn set_expressions(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.expressions = Some(list_of(value)?);
        Ok(())
    }

    /// music21's `articulations`: the marks written under this note.
    #[getter]
    fn get_articulations(&mut self, py: Python<'_>) -> Py<PyList> {
        let list = self
            .articulations
            .get_or_insert_with(|| PyList::empty(py).unbind());
        list.clone_ref(py)
    }

    #[setter]
    fn set_articulations(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.articulations = Some(list_of(value)?);
        Ok(())
    }

    /// music21's `_chordAttached`, which its own `ChordBase` sets on every
    /// note it takes in. Keeping the slot is what lets music21's chord
    /// classes hold facade notes at all.
    #[getter(_chordAttached)]
    fn get_chordAttached(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.chord.as_ref().map(|chord| chord.clone_ref(py))
    }

    #[setter(_chordAttached)]
    fn set_chordAttached(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.chord = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
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
    m.add_class::<Note>()?;
    m.add_class::<Duration>()?;
    m.add_class::<GraceDuration>()?;
    m.add_class::<AppoggiaturaDuration>()?;
    m.add_class::<DurationTuple>()?;
    m.add_class::<Tuplet>()?;
    let duration_exception = py.get_type::<DurationException>();
    duration_exception.setattr("__module__", "music21.duration")?;
    m.add("DurationException", duration_exception)?;
    m.add_function(wrap_pyfunction!(durationTupleFromQuarterLength, m)?)?;
    m.add_function(wrap_pyfunction!(durationTupleFromTypeDots, m)?)?;
    let exception = py.get_type::<NoteException>();
    exception.setattr("__module__", "music21.note")?;
    m.add("NoteException", exception)?;
    let not_rest = py.get_type::<NotRestException>();
    not_rest.setattr("__module__", "music21.note")?;
    m.add("NotRestException", not_rest)?;
    Ok(())
}
