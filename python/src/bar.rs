//! music21's `bar.Barline` and `bar.Repeat`, over the crate's `bar` module.
//!
//! A barline is how a measure is closed and, where it is a repeat sign,
//! which way the music goes back. music21 keeps one on a measure as its
//! `leftBarline` or `rightBarline` rather than among its elements, except
//! for a barline standing inside a measure, which is an element like any
//! other -- and this package keeps them the same way.
//!
//! **Nothing is installed over music21's own `bar`.** A barline there
//! carries a `pause`, which is a `Fermata` or a `Rest` and so an object of
//! music21's that this package has no class for, and its `style` and
//! `editorial` besides. Standing in for it would be quietly different
//! rather than missing, which is the bargain `metadata` could not keep
//! either.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::bar::{Barline as RsBarline, BarlineType, RepeatDirection};

/// The names this module hands out.
pub const NAMES: &[&str] = &["Barline", "Repeat", "BarException"];

pyo3::create_exception!(music21_rs_facade, BarException, crate::Music21Exception);

/// music21's `bar.Barline`.
#[pyclass(
    name = "Barline",
    module = "music21.bar",
    subclass,
    skip_from_py_object
)]
pub struct Barline {
    pub(crate) inner: RsBarline,
    /// music21's `location`: `'left'`, `'right'`, `'middle'` or nothing.
    /// A barline a measure holds says nothing of its own, the measure
    /// saying which of its own it is.
    location: Option<String>,
    /// music21's `pause`, which is a `Fermata` or a `Rest` there. Kept as
    /// the object it was given, there being nothing here to read it as.
    pause: Option<Py<PyAny>>,
}

impl Barline {
    /// The facade around a barline of the crate's.
    pub(crate) fn wrap(inner: RsBarline) -> Self {
        Self {
            inner,
            location: None,
            pause: None,
        }
    }

    /// The object for a barline of the crate's: a `Repeat` where it is a
    /// repeat sign, as music21's reader makes one, and a `Barline`
    /// otherwise.
    pub(crate) fn object(py: Python<'_>, inner: RsBarline) -> PyResult<Py<PyAny>> {
        let name = if inner.repeat_direction().is_some() {
            "Repeat"
        } else {
            "Barline"
        };
        let class = match crate::installed_class(py, "music21.bar", name) {
            Some(class) => class,
            None => py
                .import("music21_rs")
                .or_else(|_| py.import("music21_rs_facade"))?
                .getattr(name)?,
        };
        let made = class.call0()?;
        made.extract::<PyRefMut<'_, Self>>()?.inner = inner;
        Ok(made.unbind())
    }
}

#[pymethods]
impl Barline {
    /// music21's `Barline(type=None, location=None)`, the type read by its
    /// `standardizeBarType`: nothing is a regular line, and MusicXML's
    /// `light-light` and `light-heavy` are `double` and `final`.
    #[new]
    #[pyo3(signature = (r#type = None, location = None, **_keywords))]
    fn new(
        r#type: Option<&Bound<'_, PyAny>>,
        location: Option<String>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut made = Self::wrap(RsBarline::new(BarlineType::Regular));
        if let Some(said) = r#type.filter(|said| !said.is_none()) {
            made.set_type(said)?;
        }
        made.location = location;
        Ok(made)
    }

    /// What music21 says this is: its own class and every class it is a
    /// kind of, as music21's `classes` answers.
    #[getter]
    fn classes(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        crate::stream::lineage_of(slf.as_any())
    }

    /// music21's sort order for a barline among the things standing at one
    /// offset: before the notes of the measure it closes.
    #[classattr]
    fn classSortOrder() -> i32 {
        -5
    }

    /// How the barline is drawn: music21's `type`.
    #[getter]
    fn get_type(&self) -> &'static str {
        self.inner.bar_type().as_str()
    }

    /// Changes how the barline is drawn, reading the name as music21's
    /// `standardizeBarType` reads it.
    #[setter]
    fn set_type(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let said = match value.extract::<String>() {
            Ok(said) => said,
            Err(_) if value.is_none() => "regular".to_string(),
            Err(error) => return Err(error),
        };
        let kind = BarlineType::from_name(&said)
            .map_err(|error| BarException::new_err(crate::pitch::message(&error)))?;
        self.inner.set_bar_type(kind);
        Ok(())
    }

    /// The MusicXML style the type is drawn by: music21's
    /// `musicXMLBarStyle`, the type's own name but for `double` and
    /// `final`.
    fn musicXMLBarStyle(&self) -> &'static str {
        self.inner.bar_type().musicxml_bar_style()
    }

    /// Which side of a measure the barline is on, where it says: music21's
    /// `location`.
    #[getter]
    fn get_location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    #[setter]
    fn set_location(&mut self, value: Option<String>) {
        self.location = value;
    }

    /// What is held over the barline: music21's `pause`, a `Fermata` or a
    /// `Rest` there, kept as the object it was given.
    #[getter]
    fn get_pause(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.pause.as_ref().map(|pause| pause.clone_ref(py))
    }

    #[setter]
    fn set_pause(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.pause = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let me = slf.borrow();
        Ok(format!(
            "<music21.bar.Barline type={}>",
            me.inner.bar_type().as_str()
        ))
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner && self.location == other.location
    }
}

/// music21's `bar.Repeat`: a barline that says which way the music goes
/// back.
#[pyclass(
    name = "Repeat",
    module = "music21.bar",
    extends = Barline,
    subclass
)]
pub struct Repeat;

#[pymethods]
impl Repeat {
    /// music21's `Repeat(direction='start', times=None)`, whose type
    /// follows the direction: `heavy-light` where a repeat starts and
    /// `final` where it ends.
    #[new]
    #[pyo3(signature = (direction = "start".to_string(), times = None, **_keywords))]
    fn new(
        direction: String,
        times: Option<u32>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let way = read_direction(&direction)?;
        Ok(
            PyClassInitializer::from(Barline::wrap(RsBarline::repeat(way, times)))
                .add_subclass(Repeat),
        )
    }

    /// Which way the music repeats: music21's `direction`, `'start'` or
    /// `'end'`.
    #[getter]
    fn get_direction(slf: &Bound<'_, Self>) -> PyResult<&'static str> {
        let base = slf.as_super().borrow();
        Ok(base
            .inner
            .repeat_direction()
            .unwrap_or(RepeatDirection::Start)
            .as_str())
    }

    /// Sets which way the music repeats, which changes how it is drawn, as
    /// music21's setter does.
    #[setter]
    fn set_direction(slf: &Bound<'_, Self>, value: &str) -> PyResult<()> {
        let way = read_direction(value)?;
        let mut base = slf.as_super().borrow_mut();
        let times = base.inner.repeat_times();
        base.inner = RsBarline::repeat(way, times);
        Ok(())
    }

    /// How many times the passage is played, where the score says:
    /// music21's `times`.
    #[getter]
    fn get_times(slf: &Bound<'_, Self>) -> Option<u32> {
        slf.as_super().borrow().inner.repeat_times()
    }

    /// Says how many times the passage is played.
    #[setter]
    fn set_times(slf: &Bound<'_, Self>, value: Option<u32>) -> PyResult<()> {
        if value.is_some_and(|times| times < 1) {
            return Err(BarException::new_err(
                "cannot set repeat times to a value less than one",
            ));
        }
        let mut base = slf.as_super().borrow_mut();
        let way = base
            .inner
            .repeat_direction()
            .unwrap_or(RepeatDirection::Start);
        base.inner = RsBarline::repeat(way, value);
        Ok(())
    }

    /// The words the count is written as, for a caller putting them over
    /// the staff: music21's `getTextExpression`, which writes the times
    /// between a prefix and a postfix.
    ///
    /// A repeat that says no count answers nothing, where music21 writes
    /// the word `None` into the text.
    #[pyo3(signature = (prefix = String::new(), postfix = "x".to_string()))]
    fn getTextExpression<'py>(
        slf: &Bound<'py, Self>,
        prefix: String,
        postfix: String,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let said = slf
            .as_super()
            .borrow()
            .inner
            .repeat_text_expression(&prefix, &postfix);
        let Some(said) = said else {
            return Ok(None);
        };
        Ok(Some(
            crate::expressions::TextExpression::object(py, said)?.into_bound(py),
        ))
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let direction = Self::get_direction(slf)?;
        Ok(match Self::get_times(slf) {
            Some(times) => format!("<music21.bar.Repeat direction={direction} times={times}>"),
            None => format!("<music21.bar.Repeat direction={direction}>"),
        })
    }
}

/// music21's reading of a repeat's direction, which takes `start` and
/// `end` in any case and refuses anything else.
fn read_direction(said: &str) -> PyResult<RepeatDirection> {
    match said.to_lowercase().as_str() {
        "start" => Ok(RepeatDirection::Start),
        "end" => Ok(RepeatDirection::End),
        other => Err(BarException::new_err(format!(
            "cannot set repeat direction to {other}"
        ))),
    }
}

/// Adds the barline classes to a module.
///
/// The exception is not among them: building one builds music21's own
/// `Music21Exception` and so imports music21, which this package is not to
/// do for a name nobody has asked for. `lib.rs`'s table hands it out on
/// first asking.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Barline>()?;
    m.add_class::<Repeat>()?;
    Ok(())
}
