//! music21's `metadata.primitives`: the values a score's metadata is made
//! of.
//!
//! These are the crate's `metadata::primitives` -- a date that may be
//! approximate, uncertain, a bound, a span or a choice; a piece of text with
//! the language it is written in; somebody who had a hand in the work --
//! which `metadata_parity` holds to music21 name for name, relevance for
//! relevance and date for date.
//!
//! **One crate type carries all four of music21's date classes.**
//! `DateSingle`, `DateRelative`, `DateBetween` and `DateSelection` differ
//! only in the relevances they take and in how they are written, so each is
//! a facade class over `DatePrimitive` and each refuses a relevance that is
//! not its own, exactly as music21's own setters refuse one.
//!
//! **A date is written freely and checked when it is loaded.** music21 sets
//! its own attributes without asking whether a calendar has such a date and
//! only `load` raises -- keeping the values it read, so that `str(d)` shows
//! the impossible date it was given. `Date::set_unchecked` and
//! `Date::from_written_unchecked` are what let the facade do the same.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::metadata::primitives::{
    Certainty, Contributor as RsContributor, Date as RsDate, DateKind, DatePart,
    DatePrimitive as RsDatePrimitive, ROLE_ABBREVIATIONS, Relevance, Text as RsText,
    strip_certainty,
};

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "Date",
    "DatePrimitive",
    "DateSingle",
    "DateRelative",
    "DateBetween",
    "DateSelection",
    "Text",
    "Copyright",
    "Contributor",
    "Creator",
];

use crate::metadata::MetadataException;

/// music21's own `MetadataException` where music21 is there to be asked,
/// and this package's where it is not.
///
/// music21 raises that class from every one of these and its own doctests
/// read the module it lives in -- `music21.exceptions21`, not
/// `music21.metadata` -- so the class has to be the one music21 keeps
/// rather than one of this package's stamped with its name.
fn refused(said: String) -> PyErr {
    Python::attach(|py| {
        if let Ok(class) = py
            .import("music21.exceptions21")
            .and_then(|module| module.getattr("MetadataException"))
            && let Ok(made) = class.call1((said.clone(),))
        {
            return PyErr::from_value(made);
        }
        MetadataException::new_err(said)
    })
}

/// A crate error as music21's `MetadataException`.
fn primitive_error(error: music21_rs_crate::Error) -> PyErr {
    crate::pitch::specific_error(&error).unwrap_or_else(|| refused(crate::pitch::message(&error)))
}

/// A written value music21 takes as a number or as a string with a symbol
/// in it, read into a value and how sure it is.
fn read_part(value: Option<&Bound<'_, PyAny>>) -> PyResult<(Option<i32>, Option<Certainty>)> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok((None, None));
    };
    if let Ok(whole) = value.extract::<i32>() {
        return Ok((Some(whole), None));
    }
    if let Ok(number) = value.extract::<f64>() {
        return Ok((Some(number as i32), None));
    }
    let written = value.str()?.to_string();
    let (read, certainty) = strip_certainty(&written).map_err(primitive_error)?;
    Ok((Some(read), certainty))
}

/// How sure a part is, as music21 names it.
fn read_certainty(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Certainty>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let name = value.str()?.to_string();
    Ok(Certainty::named(&name))
}

/// music21's `metadata.Date`: a date any part of which may be missing and
/// any part of which may say how sure it is.
#[pyclass(
    name = "Date",
    module = "music21.metadata.primitives",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct Date {
    pub(crate) inner: RsDate,
}

impl Date {
    /// The facade over a date already read.
    pub(crate) fn wrap(inner: RsDate) -> Self {
        Self { inner }
    }
}

/// The six parts in music21's order, by the names it gives them.
const PART_NAMES: [&str; 6] = ["year", "month", "day", "hour", "minute", "second"];

#[pymethods]
impl Date {
    /// music21's `Date(year=..., yearError=..., ...)`, every argument
    /// keyword-only as music21 takes them. A value written with a symbol
    /// in it says how sure it is.
    #[new]
    #[pyo3(signature = (
        *,
        year = None,
        month = None,
        day = None,
        hour = None,
        minute = None,
        second = None,
        yearError = None,
        monthError = None,
        dayError = None,
        hourError = None,
        minuteError = None,
        secondError = None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        year: Option<&Bound<'_, PyAny>>,
        month: Option<&Bound<'_, PyAny>>,
        day: Option<&Bound<'_, PyAny>>,
        hour: Option<&Bound<'_, PyAny>>,
        minute: Option<&Bound<'_, PyAny>>,
        second: Option<&Bound<'_, PyAny>>,
        yearError: Option<&Bound<'_, PyAny>>,
        monthError: Option<&Bound<'_, PyAny>>,
        dayError: Option<&Bound<'_, PyAny>>,
        hourError: Option<&Bound<'_, PyAny>>,
        minuteError: Option<&Bound<'_, PyAny>>,
        secondError: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let given = [year, month, day, hour, minute, second];
        let said = [
            yearError,
            monthError,
            dayError,
            hourError,
            minuteError,
            secondError,
        ];
        let mut inner = RsDate::new();
        for (at, part) in DatePart::ALL.iter().enumerate() {
            // music21 reads the symbol off the value only where no error
            // was given beside it.
            let error = read_certainty(said[at])?;
            let (value, read) = read_part(given[at])?;
            inner.set_unchecked(*part, value, error.or(read));
        }
        inner.check().map_err(primitive_error)?;
        Ok(Self { inner })
    }

    /// The names music21 loops over its own parts by.
    #[classattr]
    fn attrNames(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, PART_NAMES)?.into_any().unbind())
    }

    /// The symbols that mark a part approximate.
    #[classattr]
    fn approximateSymbols(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, ["~", "x"])?.into_any().unbind())
    }

    /// The symbols that mark a part uncertain.
    #[classattr]
    fn uncertainSymbols(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, ["?", "z"])?.into_any().unbind())
    }

    /// The symbols music21 reads as its `priority`.
    #[classattr]
    fn priorTimeSymbols(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(PyTuple::new(py, ["<", "{", ">", "}"])?.into_any().unbind())
    }

    /// The symbol an error name is written with: music21's
    /// `errorToSymbol`.
    #[staticmethod]
    fn errorToSymbol(value: &str) -> Option<String> {
        Certainty::named(value)
            .and_then(Certainty::symbol)
            .map(|symbol| symbol.to_string())
    }

    /// music21's `_stripError`: a written number with its symbol taken
    /// off it, and what that symbol said.
    fn _stripError<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let (read, certainty) = read_part(Some(value))?;
        PyTuple::new(
            py,
            [
                read.into_pyobject(py)?.into_any(),
                certainty
                    .map(|found| found.name())
                    .into_pyobject(py)?
                    .into_any(),
            ],
        )
    }

    /// music21's `_sanityCheck`: whether a calendar has such a date.
    #[pyo3(signature = (*, year = None, month = None, day = None, hour = None, minute = None, second = None))]
    #[allow(clippy::too_many_arguments)]
    fn _sanityCheck(
        &self,
        year: Option<&Bound<'_, PyAny>>,
        month: Option<&Bound<'_, PyAny>>,
        day: Option<&Bound<'_, PyAny>>,
        hour: Option<&Bound<'_, PyAny>>,
        minute: Option<&Bound<'_, PyAny>>,
        second: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let given = [year, month, day, hour, minute, second];
        let mut date = RsDate::new();
        for (at, part) in DatePart::ALL.iter().enumerate() {
            let (value, _) = read_part(given[at])?;
            date.set_unchecked(*part, value, None);
        }
        date.check().map_err(primitive_error)
    }

    /// music21's `load`: a date read from a string, a number, one of
    /// Python's datetimes, or another date. The values it read stay in
    /// place where a calendar has no such date, as music21's do.
    fn load(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if value.is_instance(&py.import("datetime")?.getattr("datetime")?)? {
            Self::loadDatetime(slf, value)?;
        } else if let Ok(other) = value.cast::<Self>() {
            Self::loadOther(slf, other)?;
        } else if value.extract::<i64>().is_ok() || value.extract::<String>().is_ok() {
            Self::loadStr(slf, &value.str()?.to_string())?;
        } else {
            return Err(refused(format!("Cannot load data: {}", value.str()?)));
        }
        let read = slf.borrow().inner.check();
        read.map_err(primitive_error)
    }

    /// music21's `loadStr`: `year/month/day/hour:minute:second`, with `--`
    /// for a part it does not give.
    fn loadStr(slf: &Bound<'_, Self>, dateStr: &str) -> PyResult<()> {
        let read = RsDate::from_written_unchecked(dateStr).map_err(primitive_error)?;
        let mut me = slf.borrow_mut();
        for part in DatePart::ALL {
            if let Some(value) = read.value(part) {
                me.inner
                    .set_unchecked(part, Some(value), read.certainty(part));
            }
        }
        Ok(())
    }

    /// music21's `loadOther`: every part another date gives, with its
    /// certainty, written over this one's.
    fn loadOther(slf: &Bound<'_, Self>, other: &Bound<'_, Self>) -> PyResult<()> {
        let read = other.borrow().inner.clone();
        let mut me = slf.borrow_mut();
        for part in DatePart::ALL {
            if let Some(value) = read.value(part) {
                me.inner
                    .set_unchecked(part, Some(value), read.certainty(part));
            }
        }
        Ok(())
    }

    /// music21's `loadDatetime`: the parts one of Python's datetimes
    /// gives, nought and nothing passed over as music21 passes them over.
    fn loadDatetime(slf: &Bound<'_, Self>, dt: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut read: Vec<(DatePart, Option<i32>)> = Vec::new();
        for (at, part) in DatePart::ALL.iter().enumerate() {
            let Ok(value) = dt.getattr(PART_NAMES[at]) else {
                continue;
            };
            let Ok(whole) = value.extract::<i32>() else {
                continue;
            };
            if whole != 0 {
                read.push((*part, Some(whole)));
            }
        }
        let mut me = slf.borrow_mut();
        for (part, value) in read {
            me.inner.set_unchecked(part, value, None);
        }
        Ok(())
    }

    /// One of Python's datetimes for this date: music21's `datetime`,
    /// which refuses an incomplete one as Python's own constructor does.
    #[getter]
    fn datetime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut parts: Vec<i32> = Vec::new();
        for part in DatePart::ALL {
            match self.inner.value(part) {
                None => break,
                Some(value) => parts.push(value),
            }
        }
        py.import("datetime")?
            .getattr("datetime")?
            .call1(PyTuple::new(py, parts)?)
    }

    /// Whether any part of the time of day is given: music21's `hasTime`.
    #[getter]
    fn hasTime(&self) -> bool {
        self.inner.has_time()
    }

    /// Whether any part says how sure it is: music21's `hasError`.
    #[getter]
    fn hasError(&self) -> bool {
        self.inner.has_certainty()
    }

    #[getter]
    fn year(&self) -> Option<i32> {
        self.inner.value(DatePart::Year)
    }

    #[setter]
    fn set_year(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.write(DatePart::Year, value)
    }

    #[getter]
    fn month(&self) -> Option<i32> {
        self.inner.value(DatePart::Month)
    }

    #[setter]
    fn set_month(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.write(DatePart::Month, value)
    }

    #[getter]
    fn day(&self) -> Option<i32> {
        self.inner.value(DatePart::Day)
    }

    #[setter]
    fn set_day(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.write(DatePart::Day, value)
    }

    #[getter]
    fn hour(&self) -> Option<i32> {
        self.inner.value(DatePart::Hour)
    }

    #[setter]
    fn set_hour(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.write(DatePart::Hour, value)
    }

    #[getter]
    fn minute(&self) -> Option<i32> {
        self.inner.value(DatePart::Minute)
    }

    #[setter]
    fn set_minute(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.write(DatePart::Minute, value)
    }

    #[getter]
    fn second(&self) -> Option<i32> {
        self.inner.value(DatePart::Second)
    }

    #[setter]
    fn set_second(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.write(DatePart::Second, value)
    }

    #[getter]
    fn yearError(&self) -> Option<&'static str> {
        self.said(DatePart::Year)
    }

    #[setter]
    fn set_yearError(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.say(DatePart::Year, value)
    }

    #[getter]
    fn monthError(&self) -> Option<&'static str> {
        self.said(DatePart::Month)
    }

    #[setter]
    fn set_monthError(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.say(DatePart::Month, value)
    }

    #[getter]
    fn dayError(&self) -> Option<&'static str> {
        self.said(DatePart::Day)
    }

    #[setter]
    fn set_dayError(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.say(DatePart::Day, value)
    }

    #[getter]
    fn hourError(&self) -> Option<&'static str> {
        self.said(DatePart::Hour)
    }

    #[setter]
    fn set_hourError(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.say(DatePart::Hour, value)
    }

    #[getter]
    fn minuteError(&self) -> Option<&'static str> {
        self.said(DatePart::Minute)
    }

    #[setter]
    fn set_minuteError(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.say(DatePart::Minute, value)
    }

    #[getter]
    fn secondError(&self) -> Option<&'static str> {
        self.said(DatePart::Second)
    }

    #[setter]
    fn set_secondError(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.say(DatePart::Second, value)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn _reprInternal(&self) -> String {
        self.inner.to_string()
    }

    /// music21 compares two dates as the strings they write.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(slf.borrow().inner.to_string() == other.str()?.to_string())
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = crate::blank_installed(&slf.get_type())?;
        copied.extract::<PyRefMut<'_, Self>>()?.inner = slf.borrow().inner.clone();
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsDate>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

impl Date {
    /// Writes one part without asking whether a calendar has such a date,
    /// as music21's own attribute assignment does.
    fn write(&mut self, part: DatePart, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let (read, certainty) = read_part(value)?;
        self.inner.set_unchecked(part, read, certainty);
        Ok(())
    }

    /// How sure one part is, by the name music21 gives it.
    fn said(&self, part: DatePart) -> Option<&'static str> {
        self.inner.certainty(part).map(Certainty::name)
    }

    /// Says how sure one part is.
    fn say(&mut self, part: DatePart, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let certainty = read_certainty(value)?;
        let held = self.inner.value(part);
        self.inner.set_unchecked(part, held, certainty);
        if certainty.is_none() {
            // Saying nothing takes the certainty away, which setting the
            // value does not.
            let mut cleared = RsDate::new();
            for one in DatePart::ALL {
                let keep = if one == part {
                    None
                } else {
                    self.inner.certainty(one)
                };
                cleared.set_unchecked(one, self.inner.value(one), keep);
            }
            self.inner = cleared;
        }
        Ok(())
    }
}

/// music21's `metadata.DatePrimitive`: one or more dates and what they say.
#[pyclass(
    name = "DatePrimitive",
    module = "music21.metadata.primitives",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct DatePrimitive {
    pub(crate) inner: RsDatePrimitive,
}

impl DatePrimitive {
    /// The facade over a date already read.
    pub(crate) fn wrap(inner: RsDatePrimitive) -> Self {
        Self { inner }
    }

    /// The dates a caller handed over: a string, a number, one of Python's
    /// datetimes, or one of this module's dates.
    fn read_dates(data: &Bound<'_, PyAny>, several: bool) -> PyResult<Vec<RsDate>> {
        let py = data.py();
        if !several {
            return Ok(vec![Self::read_date(data)?]);
        }
        let mut dates = Vec::new();
        for one in crate::Walkable::walk(data)? {
            dates.push(Self::read_date(&one?)?);
        }
        let _ = py;
        Ok(dates)
    }

    /// One date read as music21's `Date.load` reads one.
    fn read_date(value: &Bound<'_, PyAny>) -> PyResult<RsDate> {
        if let Ok(ours) = value.cast::<Date>() {
            return Ok(ours.borrow().inner.clone());
        }
        let blank = Date::wrap(RsDate::new());
        let held = Bound::new(value.py(), blank)?;
        Date::load(&held, value)?;
        Ok(held.borrow().inner.clone())
    }

    /// The relevance a name stands for, refused where the class does not
    /// take it -- which is music21's own setter.
    fn read_relevance(name: &str, kind: DateKind) -> PyResult<Relevance> {
        let refused = || {
            super::primitives::refused(format!(
                "Relevance value is not supported by this object: '{name}'"
            ))
        };
        let found = Relevance::named(name).ok_or_else(refused)?;
        if found.kind() != kind {
            return Err(refused());
        }
        Ok(found)
    }
}

#[pymethods]
impl DatePrimitive {
    /// music21's `DatePrimitive(relevance='certain')`.
    #[new]
    #[pyo3(signature = (relevance = "certain".to_string()))]
    fn new(relevance: String) -> PyResult<Self> {
        let found = Self::read_relevance(&relevance, DateKind::Single)?;
        Ok(Self {
            inner: RsDatePrimitive::of_dates(Vec::new(), found, DateKind::Single)
                .map_err(primitive_error)?,
        })
    }

    /// What the dates say: music21's `relevance`.
    #[getter]
    fn relevance(&self) -> &'static str {
        self.inner.relevance().name()
    }

    #[setter]
    fn set_relevance(&mut self, value: &str) -> PyResult<()> {
        let found = Self::read_relevance(value, self.inner.kind())?;
        self.inner.set_relevance(found).map_err(primitive_error)
    }

    /// One of Python's datetimes for the first date: music21's `datetime`.
    #[getter]
    fn datetime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let Some(first) = self.inner.dates().first() else {
            return Err(pyo3::exceptions::PyIndexError::new_err(
                "list index out of range",
            ));
        };
        Date::wrap(first.clone()).datetime(py)
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn _reprInternal(&self) -> String {
        self.inner.to_string()
    }

    /// music21 compares the dates, what they say and the class they are
    /// of; the relevance says which class that is.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        if !slf.get_type().is(other.get_type()) {
            return Ok(false);
        }
        Ok(slf.borrow().inner == other.borrow().inner)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = crate::blank_installed(&slf.get_type())?;
        copied.extract::<PyRefMut<'_, Self>>()?.inner = slf.borrow().inner.clone();
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsDatePrimitive>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's `DateSingle`: one date, certain, approximate or uncertain.
#[pyclass(
    name = "DateSingle",
    module = "music21.metadata.primitives",
    extends = DatePrimitive,
    subclass
)]
pub struct DateSingle;

#[pymethods]
impl DateSingle {
    /// music21's `DateSingle(data='', relevance='certain')`.
    #[new]
    #[pyo3(signature = (data = None, relevance = "certain".to_string()))]
    fn new(
        data: Option<&Bound<'_, PyAny>>,
        relevance: String,
    ) -> PyResult<PyClassInitializer<Self>> {
        let found = DatePrimitive::read_relevance(&relevance, DateKind::Single)?;
        let dates = match data.filter(|data| !data.is_none()) {
            None => vec![RsDate::new()],
            Some(data) => DatePrimitive::read_dates(data, false)?,
        };
        let inner =
            RsDatePrimitive::of_dates(dates, found, DateKind::Single).map_err(primitive_error)?;
        Ok(PyClassInitializer::from(DatePrimitive::wrap(inner)).add_subclass(DateSingle))
    }
}

/// music21's `DateRelative`: a date something happened before or after.
#[pyclass(
    name = "DateRelative",
    module = "music21.metadata.primitives",
    extends = DatePrimitive,
    subclass
)]
pub struct DateRelative;

#[pymethods]
impl DateRelative {
    /// music21's `DateRelative(data='', relevance='after')`.
    #[new]
    #[pyo3(signature = (data = None, relevance = "after".to_string()))]
    fn new(
        data: Option<&Bound<'_, PyAny>>,
        relevance: String,
    ) -> PyResult<PyClassInitializer<Self>> {
        let found = DatePrimitive::read_relevance(&relevance, DateKind::Relative)?;
        let dates = match data.filter(|data| !data.is_none()) {
            None => vec![RsDate::new()],
            Some(data) => DatePrimitive::read_dates(data, false)?,
        };
        let inner =
            RsDatePrimitive::of_dates(dates, found, DateKind::Relative).map_err(primitive_error)?;
        Ok(PyClassInitializer::from(DatePrimitive::wrap(inner)).add_subclass(DateRelative))
    }
}

/// music21's `DateBetween`: a span between two dates.
#[pyclass(
    name = "DateBetween",
    module = "music21.metadata.primitives",
    extends = DatePrimitive,
    subclass
)]
pub struct DateBetween;

#[pymethods]
impl DateBetween {
    /// music21's `DateBetween(data=(), relevance='between')`.
    #[new]
    #[pyo3(signature = (data = None, relevance = "between".to_string()))]
    fn new(
        data: Option<&Bound<'_, PyAny>>,
        relevance: String,
    ) -> PyResult<PyClassInitializer<Self>> {
        let found = DatePrimitive::read_relevance(&relevance, DateKind::Between)?;
        let dates = match data.filter(|data| !data.is_none()) {
            None => Vec::new(),
            Some(data) => DatePrimitive::read_dates(data, true)?,
        };
        let inner =
            RsDatePrimitive::of_dates(dates, found, DateKind::Between).map_err(primitive_error)?;
        Ok(PyClassInitializer::from(DatePrimitive::wrap(inner)).add_subclass(DateBetween))
    }
}

/// music21's `DateSelection`: a choice among several dates.
#[pyclass(
    name = "DateSelection",
    module = "music21.metadata.primitives",
    extends = DatePrimitive,
    subclass
)]
pub struct DateSelection;

#[pymethods]
impl DateSelection {
    /// music21's `DateSelection(data=(), relevance='or')`.
    #[new]
    #[pyo3(signature = (data = None, relevance = "or".to_string()))]
    fn new(
        data: Option<&Bound<'_, PyAny>>,
        relevance: String,
    ) -> PyResult<PyClassInitializer<Self>> {
        let found = DatePrimitive::read_relevance(&relevance, DateKind::Selection)?;
        let dates = match data.filter(|data| !data.is_none()) {
            None => Vec::new(),
            Some(data) => DatePrimitive::read_dates(data, true)?,
        };
        let inner = RsDatePrimitive::of_dates(dates, found, DateKind::Selection)
            .map_err(primitive_error)?;
        Ok(PyClassInitializer::from(DatePrimitive::wrap(inner)).add_subclass(DateSelection))
    }

    /// Whether the class holds one date, which this one does not.
    #[classattr]
    fn isSingle() -> bool {
        false
    }
}

/// music21's `metadata.Text`: one piece of text with the language it is
/// written in.
#[pyclass(
    name = "Text",
    module = "music21.metadata.primitives",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct Text {
    pub(crate) inner: RsText,
}

impl Text {
    /// The facade over a text already read.
    pub(crate) fn wrap(inner: RsText) -> Self {
        Self { inner }
    }

    /// The text a caller handed over, which may be a text already.
    pub(crate) fn read(value: &Bound<'_, PyAny>) -> PyResult<RsText> {
        if let Ok(ours) = value.cast::<Self>() {
            return Ok(ours.borrow().inner.clone());
        }
        if value.hasattr("language")? && value.hasattr("isTranslated")? {
            // music21's own `Text`, which it keeps the parts of as
            // attributes.
            let mut read = RsText::new(value.str()?.to_string());
            read.set_language(value.getattr("language")?.extract()?);
            read.set_translated(value.getattr("isTranslated")?.extract()?);
            if let Ok(scheme) = value.getattr("encodingScheme") {
                read.set_encoding_scheme(scheme.extract()?);
            }
            if let Ok(role) = value.getattr("role") {
                read.set_role(role.extract()?);
            }
            return Ok(read);
        }
        Ok(RsText::new(value.str()?.to_string()))
    }
}

#[pymethods]
impl Text {
    /// music21's `Text(data='', language=None, isTranslated=None,
    /// encodingScheme=None)`. A text handed over as the data is read whole,
    /// as music21 reads one.
    #[new]
    #[pyo3(signature = (data = None, language = None, isTranslated = None, encodingScheme = None))]
    fn new(
        data: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        #[pyo3(from_py_with = crate::truthy_or_none)] isTranslated: Option<bool>,
        encodingScheme: Option<String>,
    ) -> PyResult<Self> {
        let mut inner = match data.filter(|data| !data.is_none()) {
            None => RsText::new(String::new()),
            Some(data) => Self::read(data)?,
        };
        // Whatever was given beside the data wins, as music21's own
        // constructor takes it; a text read whole keeps what it said.
        let given = data.is_some_and(|data| data.cast::<Self>().is_ok());
        if !given {
            inner.set_language(language);
            inner.set_translated(isTranslated);
            inner.set_encoding_scheme(encodingScheme);
        }
        Ok(Self { inner })
    }

    /// The language the text is written in: music21's `language`.
    #[getter]
    fn language(&self) -> Option<String> {
        self.inner.language().map(str::to_string)
    }

    #[setter]
    fn set_language(&mut self, value: Option<String>) {
        self.inner.set_language(value);
    }

    /// Whether the text is a translation, nothing at all meaning nobody
    /// has said: music21's `isTranslated`.
    #[getter]
    fn isTranslated(&self) -> Option<bool> {
        self.inner.is_translated()
    }

    #[setter]
    fn set_isTranslated(
        &mut self,
        #[pyo3(from_py_with = crate::truthy_or_none)] value: Option<bool>,
    ) {
        self.inner.set_translated(value);
    }

    /// The scheme the text was encoded by: music21's `encodingScheme`.
    #[getter]
    fn encodingScheme(&self) -> Option<String> {
        self.inner.encoding_scheme().map(str::to_string)
    }

    #[setter]
    fn set_encodingScheme(&mut self, value: Option<String>) {
        self.inner.set_encoding_scheme(value);
    }

    /// The text with an article written after a comma put back in front:
    /// music21's `getNormalizedArticle`.
    fn getNormalizedArticle(&self) -> PyResult<String> {
        self.inner.normalized_article().map_err(primitive_error)
    }

    fn __str__(&self) -> String {
        self.inner.data().to_string()
    }

    fn _reprInternal(&self) -> String {
        self.inner.data().to_string()
    }

    /// music21 compares a text with a text of its own class alone, and
    /// every part of it must match.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        if !slf.get_type().is(other.get_type()) {
            return Ok(false);
        }
        Ok(slf.borrow().inner == other.borrow().inner)
    }

    /// music21 sorts two texts by their parts, which is how a list of them
    /// is put in order.
    fn __lt__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let Ok(other) = other.cast::<Self>() else {
            return Ok(py.NotImplemented());
        };
        if !slf.get_type().is(other.get_type()) {
            return Ok(py.NotImplemented());
        }
        let less = slf.borrow().inner < other.borrow().inner;
        Ok(less.into_pyobject(py)?.to_owned().into_any().unbind())
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = crate::blank_installed(&slf.get_type())?;
        copied.extract::<PyRefMut<'_, Self>>()?.inner = slf.borrow().inner.clone();
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Some(inner) = crate::unpickled::<_, RsText>(slf, state)? {
            slf.borrow_mut().inner = inner;
        }
        Ok(())
    }
}

/// music21's `Copyright`: a text with a role, which is the only thing it
/// adds to a `Text`.
#[pyclass(
    name = "Copyright",
    module = "music21.metadata.primitives",
    extends = Text,
    subclass
)]
pub struct Copyright;

#[pymethods]
impl Copyright {
    /// music21's `Copyright(data='', language=None, isTranslated=None, *,
    /// role=None)`.
    #[new]
    #[pyo3(signature = (data = None, language = None, isTranslated = None, *, role = None))]
    fn new(
        data: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        #[pyo3(from_py_with = crate::truthy_or_none)] isTranslated: Option<bool>,
        role: Option<String>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut base = Text::new(data, language, isTranslated, None)?;
        base.inner.set_role(role);
        Ok(PyClassInitializer::from(base).add_subclass(Copyright))
    }

    /// What part the copyright plays: music21's `role`.
    #[getter]
    fn role(slf: &Bound<'_, Self>) -> Option<String> {
        slf.as_super().borrow().inner.role().map(str::to_string)
    }

    #[setter]
    fn set_role(slf: &Bound<'_, Self>, value: Option<String>) {
        slf.as_super().borrow_mut().inner.set_role(value);
    }
}

/// music21's `metadata.Contributor`: somebody who had a hand in a work.
#[pyclass(
    name = "Contributor",
    module = "music21.metadata.primitives",
    subclass,
    skip_from_py_object,
    dict
)]
pub struct Contributor {
    pub(crate) inner: RsContributor,
    /// When they were born, as music21 keeps it: one of this module's
    /// `DateSingle` objects, which a caller may hold.
    birth: Py<PyAny>,
    /// When they died, likewise.
    death: Py<PyAny>,
}

impl Contributor {
    /// The facade over a contributor already read.
    pub(crate) fn wrap(py: Python<'_>, inner: RsContributor) -> PyResult<Self> {
        let birth = match inner.birth() {
            None => py.None(),
            Some(date) => single_of(py, date.clone())?.unbind(),
        };
        let death = match inner.death() {
            None => py.None(),
            Some(date) => single_of(py, date.clone())?.unbind(),
        };
        Ok(Self {
            inner,
            birth,
            death,
        })
    }
}

/// One of this module's classes, built blank: the installed one where
/// something stands in for music21's, and the bare facade otherwise.
fn blank_primitive<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let class = crate::class_of(py, "music21.metadata.primitives", name)?;
    crate::blank_installed(&class)
}

/// One of this module's four date classes holding a date already read:
/// whichever class the date's own relevance says it is.
pub(crate) fn date_object<'py>(
    py: Python<'py>,
    date: RsDatePrimitive,
) -> PyResult<Bound<'py, PyAny>> {
    let made = blank_primitive(py, date.kind().name())?;
    *made.cast::<DatePrimitive>()?.borrow_mut() = DatePrimitive::wrap(date);
    Ok(made)
}

/// This module's `Text`, or its `Copyright` where that is what the name
/// takes, holding a text already read.
pub(crate) fn text_object<'py>(
    py: Python<'py>,
    text: RsText,
    copyright: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let made = blank_primitive(py, if copyright { "Copyright" } else { "Text" })?;
    *made.cast::<Text>()?.borrow_mut() = Text::wrap(text);
    Ok(made)
}

/// This module's `Contributor` holding a contributor already read.
pub(crate) fn contributor_object<'py>(
    py: Python<'py>,
    contributor: RsContributor,
) -> PyResult<Bound<'py, PyAny>> {
    let made = blank_primitive(py, "Contributor")?;
    *made.cast::<Contributor>()?.borrow_mut() = Contributor::wrap(py, contributor)?;
    Ok(made)
}

/// One of this module's `DateSingle` objects for a date already read.
fn single_of<'py>(py: Python<'py>, date: RsDatePrimitive) -> PyResult<Bound<'py, PyAny>> {
    date_object(py, date)
}

/// A text a caller handed over, read into the crate: one of this module's,
/// one of music21's own, or anything that says a string.
pub(crate) fn read_text(value: &Bound<'_, PyAny>) -> PyResult<RsText> {
    Text::read(value)
}

/// A date a caller handed over, read into the crate: one of this module's
/// four date classes, one of music21's own, or anything a date is read
/// from -- a string, a number, one of Python's datetimes.
pub(crate) fn read_date(value: &Bound<'_, PyAny>) -> PyResult<RsDatePrimitive> {
    if let Ok(ours) = value.cast::<DatePrimitive>() {
        return Ok(ours.borrow().inner.clone());
    }
    // music21's own date classes keep their dates in `_data` and what they
    // say in `relevance`.
    if let (Ok(held), Ok(said)) = (value.getattr("_data"), value.getattr("relevance")) {
        let name = said.str()?.to_string();
        if let Some(relevance) = Relevance::named(&name) {
            let mut dates = Vec::new();
            for one in crate::Walkable::walk(&held)? {
                dates.push(DatePrimitive::read_date(&one?)?);
            }
            return RsDatePrimitive::of_dates(dates, relevance, relevance.kind())
                .map_err(primitive_error);
        }
    }
    RsDatePrimitive::of_dates(
        vec![DatePrimitive::read_date(value)?],
        Relevance::Certain,
        DateKind::Single,
    )
    .map_err(primitive_error)
}

/// A contributor a caller handed over, read into the crate: one of this
/// module's, one of music21's own, or anything answering to a role and a
/// name.
pub(crate) fn read_contributor(value: &Bound<'_, PyAny>) -> PyResult<RsContributor> {
    if let Ok(ours) = value.cast::<Contributor>() {
        return Ok(ours.borrow().inner.clone());
    }
    let mut made = RsContributor::unnamed();
    if let Ok(role) = value.getattr("role")
        && !role.is_none()
    {
        made.set_role(Some(role.str()?.to_string()));
    }
    if let Ok(names) = value.getattr("names")
        && !names.is_none()
    {
        let mut read = Vec::new();
        for one in crate::Walkable::walk(&names)? {
            read.push(Text::read(&one?)?);
        }
        made.set_names(read);
    } else if let Ok(name) = value.getattr("name")
        && !name.is_none()
    {
        made.set_name(Text::read(&name)?);
    } else {
        made.set_name(Text::read(value)?);
    }
    for (born, name) in [(true, "birth"), (false, "death")] {
        if let Ok(date) = value.getattr(name)
            && !date.is_none()
        {
            let read = read_date(&date)?;
            if born {
                made.set_birth(Some(read));
            } else {
                made.set_death(Some(read));
            }
        }
    }
    Ok(made)
}

#[pymethods]
impl Contributor {
    /// music21's `Contributor(name=..., names=..., role=..., birth=...,
    /// death=...)`, every argument keyword-only as music21 takes them.
    #[new]
    #[pyo3(signature = (*, name = None, names = None, role = None, birth = None, death = None, **_keywords))]
    fn new(
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        names: Option<&Bound<'_, PyAny>>,
        role: Option<&Bound<'_, PyAny>>,
        birth: Option<&Bound<'_, PyAny>>,
        death: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut inner = RsContributor::unnamed();
        if let Some(role) = role.filter(|role| !role.is_none()) {
            inner.set_role(Some(role.str()?.to_string()));
        }
        let mut read = Vec::new();
        if let Some(name) = name.filter(|name| !name.is_none()) {
            read.push(Text::read(name)?);
        }
        if let Some(names) = names.filter(|names| !names.is_none()) {
            for one in crate::Walkable::walk(names)? {
                read.push(Text::read(&one?)?);
            }
        }
        inner.set_names(read);
        let mut made = Self {
            inner,
            birth: py.None(),
            death: py.None(),
        };
        if let Some(birth) = birth.filter(|birth| !birth.is_none()) {
            made.write_date(py, birth, true)?;
        }
        if let Some(death) = death.filter(|death| !death.is_none()) {
            made.write_date(py, death, false)?;
        }
        Ok(made)
    }

    /// The ten roles Humdrum writes an abbreviation for.
    #[classattr]
    fn roleAbbreviationsDict(py: Python<'_>) -> PyResult<Py<PyAny>> {
        let table = PyDict::new(py);
        for (written, role) in ROLE_ABBREVIATIONS {
            table.set_item(written, role)?;
        }
        Ok(table.into_any().unbind())
    }

    /// Those abbreviations.
    #[classattr]
    fn roleAbbreviations(py: Python<'_>) -> PyResult<Py<PyAny>> {
        let found = PyList::empty(py);
        for (written, _) in ROLE_ABBREVIATIONS {
            found.append(written)?;
        }
        Ok(found.into_any().unbind())
    }

    /// The roles they stand for.
    #[classattr]
    fn roleNames(py: Python<'_>) -> PyResult<Py<PyAny>> {
        let found = PyList::empty(py);
        for (_, role) in ROLE_ABBREVIATIONS {
            found.append(role)?;
        }
        Ok(found.into_any().unbind())
    }

    /// What music21 calls this kind of value.
    #[classattr]
    fn relevance() -> &'static str {
        "contributor"
    }

    /// The role an abbreviation stands for: music21's
    /// `abbreviationToRole`.
    #[staticmethod]
    fn abbreviationToRole(abbreviation: &str) -> PyResult<&'static str> {
        RsContributor::role_of_abbreviation(abbreviation).map_err(primitive_error)
    }

    /// The abbreviation a role is written with: music21's
    /// `roleToAbbreviation`.
    #[staticmethod]
    fn roleToAbbreviation(roleName: &str) -> PyResult<&'static str> {
        RsContributor::abbreviation_of_role(roleName).map_err(primitive_error)
    }

    /// What they did: music21's `role`, which reads one of Humdrum's
    /// abbreviations as the role it stands for.
    #[getter]
    fn role(&self) -> Option<String> {
        self.inner.role().map(str::to_string)
    }

    #[setter]
    fn set_role(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match value.filter(|value| !value.is_none()) {
            None => self.inner.set_role(None),
            Some(value) => self.inner.set_role(Some(value.str()?.to_string())),
        }
        Ok(())
    }

    /// The name they go by, or the first of several: music21's `name`.
    #[getter]
    fn name(&self) -> Option<String> {
        self.inner.name().map(|name| name.data().to_string())
    }

    #[setter]
    fn set_name(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.inner.set_name(Text::read(value)?);
        Ok(())
    }

    /// Every name they go by: music21's `names`, which answers the texts
    /// as the strings they say.
    #[getter]
    fn names(&self) -> Vec<String> {
        self.inner
            .names()
            .iter()
            .map(|name| name.data().to_string())
            .collect()
    }

    #[setter]
    fn set_names(&mut self, values: &Bound<'_, PyAny>) -> PyResult<()> {
        if values.extract::<String>().is_ok() {
            return Err(refused(
                ".names must be a list -- do you mean .name instead?".to_string(),
            ));
        }
        let mut read = Vec::new();
        for one in crate::Walkable::walk(values).map_err(|_| {
            refused(".names must be a list -- do you mean .name instead?".to_string())
        })? {
            read.push(Text::read(&one?)?);
        }
        self.inner.set_names(read);
        Ok(())
    }

    /// When they were born, where it is known: music21's `birth`.
    #[getter]
    fn birth(&self, py: Python<'_>) -> Py<PyAny> {
        self.birth.clone_ref(py)
    }

    #[setter]
    fn set_birth(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match value.filter(|value| !value.is_none()) {
            None => {
                self.birth = py.None();
                self.inner.set_birth(None);
            }
            Some(value) => self.write_date(py, value, true)?,
        }
        Ok(())
    }

    /// When they died, where it is known: music21's `death`.
    #[getter]
    fn death(&self, py: Python<'_>) -> Py<PyAny> {
        self.death.clone_ref(py)
    }

    #[setter]
    fn set_death(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match value.filter(|value| !value.is_none()) {
            None => {
                self.death = py.None();
                self.inner.set_death(None);
            }
            Some(value) => self.write_date(py, value, false)?,
        }
        Ok(())
    }

    /// How long they lived, or how old they are: music21's `age`, which
    /// answers one of Python's `timedelta`s, and nothing where no birth is
    /// known.
    fn age<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if self.birth.bind(py).is_none() {
            return Ok(py.None().into_bound(py));
        }
        let born = self.birth.bind(py).getattr("datetime")?;
        let died = if self.death.bind(py).is_none() {
            py.import("datetime")?
                .getattr("datetime")?
                .call_method0("now")?
        } else {
            self.death.bind(py).getattr("datetime")?
        };
        died.sub(born)
    }

    fn __str__(&self) -> String {
        self.name().unwrap_or_default()
    }

    fn _reprInternal(&self) -> String {
        format!(
            "{}:{}",
            self.inner.role().unwrap_or(""),
            self.name().unwrap_or_default()
        )
    }

    /// music21 compares the role, every name, and the two dates.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(false);
        };
        if !slf.get_type().is(other.get_type()) {
            return Ok(false);
        }
        Ok(slf.borrow().inner == other.borrow().inner)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = crate::blank_installed(&slf.get_type())?;
        let made = Self::wrap(py, slf.borrow().inner.clone())?;
        let mut copy = copied.extract::<PyRefMut<'_, Self>>()?;
        copy.inner = made.inner;
        copy.birth = made.birth;
        copy.death = made.death;
        drop(copy);
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::__deepcopy__(slf, None)
    }

    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        crate::pickled(slf, &slf.borrow().inner)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if let Some(inner) = crate::unpickled::<_, RsContributor>(slf, state)? {
            let made = Self::wrap(py, inner)?;
            let mut me = slf.borrow_mut();
            me.inner = made.inner;
            me.birth = made.birth;
            me.death = made.death;
        }
        Ok(())
    }
}

impl Contributor {
    /// Writes a birth or a death, which music21 reads as a `DateSingle`
    /// where it was given anything else.
    fn write_date(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>, born: bool) -> PyResult<()> {
        let held = if let Ok(ours) = value.cast::<DatePrimitive>() {
            let date = ours.borrow().inner.clone();
            (value.clone(), date)
        } else {
            let made = single_of(
                py,
                RsDatePrimitive::of_dates(
                    vec![DatePrimitive::read_date(value)?],
                    Relevance::Certain,
                    DateKind::Single,
                )
                .map_err(primitive_error)?,
            )?;
            let date = made.cast::<DatePrimitive>()?.borrow().inner.clone();
            (made, date)
        };
        if born {
            self.birth = held.0.unbind();
            self.inner.set_birth(Some(held.1));
        } else {
            self.death = held.0.unbind();
            self.inner.set_death(Some(held.1));
        }
        Ok(())
    }
}

/// music21's `Creator`: a contributor who created the work.
#[pyclass(
    name = "Creator",
    module = "music21.metadata.primitives",
    extends = Contributor,
    subclass
)]
pub struct Creator;

#[pymethods]
impl Creator {
    /// music21's `Creator(...)`, which takes what a contributor takes.
    #[new]
    #[pyo3(signature = (*, name = None, names = None, role = None, birth = None, death = None, **_keywords))]
    fn new(
        py: Python<'_>,
        name: Option<&Bound<'_, PyAny>>,
        names: Option<&Bound<'_, PyAny>>,
        role: Option<&Bound<'_, PyAny>>,
        birth: Option<&Bound<'_, PyAny>>,
        death: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = Contributor::new(py, name, names, role, birth, death, None)?;
        Ok(PyClassInitializer::from(base).add_subclass(Creator))
    }

    /// What music21 calls this kind of value.
    #[classattr]
    fn relevance() -> &'static str {
        "creator"
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Date>()?;
    m.add_class::<DatePrimitive>()?;
    m.add_class::<DateSingle>()?;
    m.add_class::<DateRelative>()?;
    m.add_class::<DateBetween>()?;
    m.add_class::<DateSelection>()?;
    m.add_class::<Text>()?;
    m.add_class::<Copyright>()?;
    m.add_class::<Contributor>()?;
    m.add_class::<Creator>()?;
    Ok(())
}
