//! music21's `tempo.MetronomeMark`, over the crate's tempo module.
//!
//! A metronome mark is a number of beats a minute, a tempo word, and the note
//! value the number counts. Either half may be implied from the other, and
//! which was implied is part of what the mark says.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs::duration::Duration as RsDuration;
use music21_rs::tempo::{
    MetronomeMark as RsMetronomeMark, convert_tempo_by_referent as rs_convert_tempo_by_referent,
};

use crate::note::{Duration, duration_from_any};
use crate::pitch::message;

/// The names the `tempo` facade replaces in `music21.tempo`.
pub const NAMES: &[&str] = &[
    "MetronomeMark",
    "MetronomeMarkException",
    "convertTempoByReferent",
];

pyo3::create_exception!(music21_rs_facade, TempoException, PyException);
pyo3::create_exception!(music21_rs_facade, MetronomeMarkException, TempoException);

fn tempo_error(error: music21_rs::Error) -> PyErr {
    MetronomeMarkException::new_err(message(&error))
}

/// A number as music21 writes it: whole numbers stay whole, so a tempo of
/// sixty reads `60` and not `60.0`.
fn number_object(py: Python<'_>, value: Option<f64>) -> PyResult<Py<PyAny>> {
    let Some(value) = value else {
        return Ok(py.None());
    };
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return Ok((value as i64).into_pyobject(py)?.into_any().unbind());
    }
    Ok(value.into_pyobject(py)?.into_any().unbind())
}

/// The note value a referent argument names.
///
/// music21 takes a duration, a quarter length, the name of a written value,
/// or any object that has a duration of its own — a note, say.
fn referent_duration(value: Option<&Bound<'_, PyAny>>) -> PyResult<RsDuration> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(RsDuration::quarter());
    };
    if let Ok(carried) = value.getattr("duration")
        && !carried.is_none()
        && value.extract::<PyRef<'_, Duration>>().is_err()
    {
        return duration_from_any(&carried);
    }
    duration_from_any(value)
}

/// music21's `tempo.MetronomeMark`.
#[pyclass(
    name = "MetronomeMark",
    module = "music21.tempo",
    subclass,
    skip_from_py_object
)]
pub struct MetronomeMark {
    pub(crate) inner: RsMetronomeMark,
    /// music21's `numberSounding`: what the mark is played at, where that
    /// differs from what it says.
    sounding: Option<f64>,
    parentheses: bool,
    placement: Option<String>,
    /// The referent as an object, so `mark.referent.type` answers and an edit
    /// through it is one the mark sees.
    referent: Option<Py<Duration>>,
}

impl MetronomeMark {
    pub(crate) fn wrap(inner: RsMetronomeMark) -> Self {
        Self {
            inner,
            sounding: None,
            parentheses: false,
            placement: None,
            referent: None,
        }
    }

    /// Reads music21's arguments: a word or a number, a number, and the note
    /// value the number counts.
    fn build(
        text: Option<&Bound<'_, PyAny>>,
        number: Option<f64>,
        referent: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        // A mark written with one argument that is a number is that number.
        let (text, number) = match (text.filter(|value| !value.is_none()), number) {
            (Some(value), None)
                if value.extract::<f64>().is_ok() && value.extract::<String>().is_err() =>
            {
                (None, value.extract::<f64>().ok())
            }
            (text, number) => (text, number),
        };
        let word = match text {
            Some(value) => Some(value.extract::<String>()?),
            None => None,
        };
        let mut inner = match (number, word) {
            (Some(number), Some(word)) => RsMetronomeMark::with_number_and_text(number, word),
            (Some(number), None) => RsMetronomeMark::new(number),
            (None, Some(word)) => RsMetronomeMark::from_text(word),
            (None, None) => RsMetronomeMark::default(),
        };
        inner.set_referent(referent_duration(referent)?);
        let mut mark = Self::wrap(inner);
        if let Some(keywords) = keywords {
            if let Some(value) = keywords.get_item("numberSounding")?
                && !value.is_none()
            {
                mark.sounding = Some(value.extract()?);
            }
            if let Some(value) = keywords.get_item("parentheses")? {
                mark.parentheses = value.extract()?;
            }
        }
        Ok(mark)
    }

    /// The same mark again. A copy has no referent object of its own yet;
    /// it makes one when something asks.
    fn copied(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            sounding: self.sounding,
            parentheses: self.parentheses,
            placement: self.placement.clone(),
            referent: None,
        }
    }

    /// The referent as an object, made on first asking and the same after.
    fn referent_object(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<Duration>> {
        if let Some(referent) = &slf.borrow().referent {
            return Ok(referent.clone_ref(py));
        }
        let made = crate::installed_new(
            py,
            "music21.duration",
            "Duration",
            Duration::wrap(slf.borrow().inner.referent().clone()),
        )?;
        slf.borrow_mut().referent = Some(made.clone_ref(py));
        Ok(made)
    }
}

#[pymethods]
impl MetronomeMark {
    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<(Py<PyAny>, (), Py<PyAny>)> {
        let me = slf.borrow();
        // What it is played at goes with it: a mark imported from a score
        // often says nothing and sounds at ninety-six.
        let written = (
            me.inner.clone(),
            me.sounding,
            me.parentheses,
            me.placement.clone(),
        );
        drop(me);
        crate::pickled(slf, &written)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        type State = (RsMetronomeMark, Option<f64>, bool, Option<String>);
        let Some((inner, sounding, parentheses, placement)) =
            crate::unpickled::<_, State>(slf, state)?
        else {
            return Ok(());
        };
        let mut me = slf.borrow_mut();
        me.inner = inner;
        me.sounding = sounding;
        me.parentheses = parentheses;
        me.placement = placement;
        me.referent = None;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (text = None, number = None, referent = None, **keywords))]
    fn new(
        text: Option<&Bound<'_, PyAny>>,
        number: Option<f64>,
        referent: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Self::build(text, number, referent, keywords)
    }

    /// Built again here, since a Python subclass hands `__new__` its own
    /// arguments and only then calls `super().__init__` with music21's.
    #[pyo3(signature = (text = None, number = None, referent = None, **keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        text: Option<&Bound<'_, PyAny>>,
        number: Option<f64>,
        referent: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let built = Self::build(text, number, referent, keywords)?;
        let mut me = slf.borrow_mut();
        me.inner = built.inner;
        me.sounding = built.sounding;
        me.parentheses = built.parentheses;
        me.referent = None;
        Ok(())
    }

    #[getter]
    fn get_number(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        number_object(py, self.inner.number())
    }

    #[setter]
    fn set_number(&mut self, value: Option<f64>) {
        self.inner.set_number(value);
    }

    /// music21's `numberImplicit`: whether the number was read off the word
    /// rather than written. Nothing at all where there is no number.
    #[getter]
    fn get_numberImplicit(&self) -> Option<bool> {
        self.inner.number().map(|_| self.inner.number_implicit())
    }

    #[setter]
    fn set_numberImplicit(&mut self, _value: Option<bool>) {}

    #[getter]
    fn get_numberSounding(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        number_object(py, self.sounding)
    }

    #[setter]
    fn set_numberSounding(&mut self, value: Option<f64>) {
        self.sounding = value;
    }

    #[getter]
    fn get_text(&self) -> Option<&str> {
        self.inner.text()
    }

    #[setter]
    fn set_text(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Ok(());
        };
        let written = match value.getattr("text") {
            Ok(text) if !text.is_none() => text.extract::<String>()?,
            _ => value.extract::<String>()?,
        };
        self.inner.set_text(written);
        Ok(())
    }

    /// music21's `textImplicit`: whether the word was read off the number.
    #[getter]
    fn get_textImplicit(&self) -> Option<bool> {
        self.inner.text().map(|_| self.inner.text_implicit())
    }

    #[setter]
    fn set_textImplicit(&mut self, _value: Option<bool>) {}

    #[getter]
    fn get_referent(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<Duration>> {
        Self::referent_object(slf, py)
    }

    #[setter]
    fn set_referent(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let referent = referent_duration(value).map_err(|_| {
            TempoException::new_err(format!(
                "Cannot get a Duration from the supplied object: {}",
                value.map_or_else(String::new, |value| value.to_string())
            ))
        })?;
        let mut me = slf.borrow_mut();
        me.inner.set_referent(referent);
        me.referent = None;
        Ok(())
    }

    #[getter]
    fn get_parentheses(&self) -> bool {
        self.parentheses
    }

    #[setter]
    fn set_parentheses(&mut self, value: bool) {
        self.parentheses = value;
    }

    #[getter]
    fn get_placement(&self) -> Option<String> {
        self.placement.clone()
    }

    #[setter]
    fn set_placement(&mut self, value: Option<String>) {
        self.placement = value;
    }

    /// music21's `getQuarterBPM`: the tempo counted in quarter notes,
    /// whatever the mark itself counts.
    ///
    /// It is always a plain count of quarters, so it reads as a float even
    /// where the mark's own number is whole.
    #[pyo3(signature = (useNumberSounding = true))]
    fn getQuarterBPM(&self, useNumberSounding: bool) -> Option<f64> {
        if useNumberSounding && let Some(sounding) = self.sounding {
            let mut sounding_mark = self.inner.clone();
            sounding_mark.set_number(Some(sounding));
            return sounding_mark.quarter_bpm();
        }
        self.inner.quarter_bpm()
    }

    /// music21's `setQuarterBPM`: sets the tempo by quarter notes, leaving
    /// the mark counting whatever it counted.
    #[pyo3(signature = (value, setNumber = true))]
    fn setQuarterBPM(&mut self, value: f64, setNumber: bool) {
        let counted =
            rs_convert_tempo_by_referent(value, 1.0, self.inner.referent().quarter_length());
        if !setNumber {
            self.sounding = Some(counted);
        } else {
            self.inner.set_number(Some(counted));
        }
    }

    /// music21's `getEquivalentByReferent`: the same speed counted in another
    /// note value.
    fn getEquivalentByReferent(
        &self,
        py: Python<'_>,
        referent: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let equivalent = self
            .inner
            .equivalent_by_referent(referent_duration(Some(referent))?);
        crate::installed_new(py, "music21.tempo", "MetronomeMark", Self::wrap(equivalent))
    }

    /// music21's `getMaintainedNumberWithReferent`: the same number counted
    /// in another note value, so the tempo itself changes.
    fn getMaintainedNumberWithReferent(
        &self,
        py: Python<'_>,
        referent: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let moved = self
            .inner
            .maintained_number_with_referent(referent_duration(Some(referent))?);
        crate::installed_new(py, "music21.tempo", "MetronomeMark", Self::wrap(moved))
    }

    /// music21's `secondsPerQuarter`, which counts what the mark is played
    /// at where that differs from what it says.
    fn secondsPerQuarter(&self) -> PyResult<f64> {
        self.getQuarterBPM(true)
            .map(|bpm| 60.0 / bpm)
            .ok_or_else(|| MetronomeMarkException::new_err("this mark says no tempo"))
    }

    /// music21's `durationToSeconds`: how long a span lasts at this tempo.
    fn durationToSeconds(&self, durationOrQuarterLength: &Bound<'_, PyAny>) -> PyResult<f64> {
        let duration = duration_from_any(durationOrQuarterLength)?;
        Ok(self.secondsPerQuarter()? * duration.quarter_length())
    }

    /// music21's `secondsToDuration`: the span that lasts that long.
    fn secondsToDuration(&self, py: Python<'_>, seconds: f64) -> PyResult<Py<Duration>> {
        if seconds.is_nan() || seconds <= 0.0 {
            return Err(MetronomeMarkException::new_err(
                "seconds must be a number greater than zero",
            ));
        }
        let duration = RsDuration::new(seconds / self.secondsPerQuarter()?).map_err(tempo_error)?;
        crate::installed_new(py, "music21.duration", "Duration", Duration::wrap(duration))
    }

    /// music21's `getTextExpression`: the word as something a score can
    /// carry, and nothing where the word was only implied.
    #[pyo3(signature = (returnImplicit = false))]
    fn getTextExpression(&self, py: Python<'_>, returnImplicit: bool) -> PyResult<Py<PyAny>> {
        let Some(text) = self.inner.text() else {
            return Ok(py.None());
        };
        if self.inner.text_implicit() && !returnImplicit {
            return Ok(py.None());
        }
        Ok(py
            .import("music21.expressions")?
            .getattr("TextExpression")?
            .call1((text,))?
            .unbind())
    }

    /// music21's `getSoundingMetronomeMark`: the mark a tempo indication of
    /// any kind comes to.
    #[pyo3(signature = (found = None))]
    fn getSoundingMetronomeMark(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        found: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let Some(found) = found.filter(|value| !value.is_none()) else {
            return Ok(slf.clone().into_any().unbind());
        };
        let classes: Vec<String> = found.getattr("classes")?.extract()?;
        if classes.iter().any(|name| name == "MetricModulation") {
            return Ok(found.getattr("newMetronome")?.unbind());
        }
        if classes.iter().any(|name| name == "MetronomeMark") {
            return Ok(found.clone().unbind());
        }
        if classes.iter().any(|name| name == "TempoText") {
            return Ok(found.call_method0("getMetronomeMark")?.unbind());
        }
        let _ = py;
        Err(TempoException::new_err(format!(
            "cannot derive a MetronomeMark from this TempoIndication: {found}"
        )))
    }

    /// music21's `getPreviousMetronomeMark`: the last one in force before
    /// this one, found by asking the stream this mark sits in.
    fn getPreviousMetronomeMark(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let search = py
            .import("music21.common.enums")?
            .getattr("ElementSearch")?
            .getattr("BEFORE_OFFSET")?;
        let arguments = PyDict::new(py);
        arguments.set_item("getElementMethod", search)?;
        let found = slf.as_any().call_method(
            "getContextByClass",
            ("TempoIndication",),
            Some(&arguments),
        )?;
        if found.is_none() {
            return Ok(py.None());
        }
        Self::getSoundingMetronomeMark(slf, py, Some(&found))
    }

    fn __repr__(&self) -> String {
        let mut sounding = "";
        let mut number = self.inner.number();
        if self.inner.number().is_none() && self.sounding.is_some() {
            sounding = " (playback only)";
            number = self.sounding;
        }
        let written = match number {
            Some(number) if number.fract() == 0.0 => format!("{}", number as i64),
            Some(number) => format!("{number}"),
            None => "None".to_string(),
        };
        let full = self.inner.referent().full_name();
        match self.inner.text() {
            Some(text) => {
                format!("<music21.tempo.MetronomeMark {text} {full}={written}{sounding}>")
            }
            None => format!("<music21.tempo.MetronomeMark {full}={written}{sounding}>"),
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner && other.sounding == self.sounding)
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().copied();
        crate::copy_as_same_type(slf, copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let copied = slf.borrow().copied();
        crate::copy_as_same_type(slf, copied)
    }
}

/// music21's `convertTempoByReferent`: the same tempo counted in another
/// note value.
#[pyfunction]
#[pyo3(signature = (numberSrc, quarterLengthBeatSrc, quarterLengthBeatDst = 1.0))]
pub fn convertTempoByReferent(
    numberSrc: f64,
    quarterLengthBeatSrc: f64,
    quarterLengthBeatDst: f64,
) -> f64 {
    rs_convert_tempo_by_referent(numberSrc, quarterLengthBeatSrc, quarterLengthBeatDst)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<MetronomeMark>()?;
    m.add("TempoException", py.get_type::<TempoException>())?;
    m.add(
        "MetronomeMarkException",
        py.get_type::<MetronomeMarkException>(),
    )?;
    m.add_function(wrap_pyfunction!(convertTempoByReferent, m)?)?;
    Ok(())
}
