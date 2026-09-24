//! music21's `tempo.MetronomeMark`, over the crate's tempo module.
//!
//! A metronome mark is a number of beats a minute, a tempo word, and the note
//! value the number counts. Either half may be implied from the other, and
//! which was implied is part of what the mark says.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::PyDict;

use music21_rs_crate::duration::Duration as RsDuration;
use music21_rs_crate::tempo::{
    MetricModulation as RsMetricModulation, MetronomeMark as RsMetronomeMark, ModulationSide,
    TempoText as RsTempoText, convert_tempo_by_referent as rs_convert_tempo_by_referent,
};

use crate::duration::{Duration, duration_from_any};

/// The names the `tempo` facade replaces in `music21.tempo`.
pub const NAMES: &[&str] = &[
    "MetronomeMark",
    "MetronomeMarkException",
    "MetricModulation",
    "MetricModulationException",
    "TempoText",
    "convertTempoByReferent",
];

pyo3::create_exception!(music21_rs_facade, TempoException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, MetronomeMarkException, TempoException);
pyo3::create_exception!(music21_rs_facade, MetricModulationException, TempoException);

error_into!(tempo_error, MetronomeMarkException);

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
    parentheses: bool,
    placement: Option<String>,
    /// The referent as an object, so `mark.referent.type` answers and an edit
    /// through it is one the mark sees.
    referent: Option<Py<Duration>>,
    /// music21's `_tempoText`, built when it is first asked for. The word is
    /// kept here as a value; music21 keeps it in a `TempoText` of its own and
    /// shares this mark's style with it.
    tempo_text: Option<Py<PyAny>>,
}

impl MetronomeMark {
    pub(crate) fn wrap(inner: RsMetronomeMark) -> Self {
        Self {
            inner,
            parentheses: false,
            placement: None,
            referent: None,
            tempo_text: None,
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
                mark.inner = mark.inner.with_number_sounding(value.extract()?);
            }
            if let Some(value) = keywords.get_item("parentheses")? {
                mark.parentheses = value.extract()?;
            }
        }
        Ok(mark)
    }

    /// The same mark again. A copy has no referent object and no words of
    /// its own yet; it makes them when something asks, so that the copy's
    /// are its own and share the copy's style rather than the original's.
    fn copied(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            parentheses: self.parentheses,
            placement: self.placement.clone(),
            referent: None,
            tempo_text: None,
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
    /// The Python objects this holds, shown to the cycle collector. A note
    /// and the pitch it hands out point at each other through Rust, and
    /// without this neither of them is ever freed.
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.referent)?;
        visit.call(&self.tempo_text)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.referent = None;
        self.tempo_text = None;
    }

    /// music21 freezes a score by pickling it, and what this object is lives
    /// in Rust where a pickle cannot see it — so it is written out as text,
    /// and read back into a fresh one of these.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let me = slf.borrow();
        // What it is played at goes with it: a mark imported from a score
        // often says nothing and sounds at ninety-six.
        let written = (me.inner.clone(), me.parentheses, me.placement.clone());
        drop(me);
        crate::pickled(slf, &written)
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        type State = (RsMetronomeMark, bool, Option<String>);
        let Some((inner, parentheses, placement)) = crate::unpickled::<_, State>(slf, state)?
        else {
            return Ok(());
        };
        let mut me = slf.borrow_mut();
        me.inner = inner;
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
    fn set_numberImplicit(&mut self, value: Option<bool>) {
        self.inner.set_number_implicit(value.unwrap_or(false));
    }

    #[getter]
    fn get_numberSounding(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        number_object(py, self.inner.number_sounding())
    }

    #[setter]
    fn set_numberSounding(&mut self, value: Option<f64>) {
        self.inner.set_number_sounding(value);
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
        // The `TempoText` built for the old word says the old word.
        self.tempo_text = None;
        Ok(())
    }

    /// music21's `_tempoText`: the word as a `TempoText` of its own.
    ///
    /// The word is a value here, so the object is built when it is first
    /// asked for — and built sharing this mark's style, as music21's own
    /// `text` setter does, so that anything said about how the mark is
    /// written is said about the words and the text expression inside them.
    #[getter]
    fn _tempoText(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let Some(text) = slf.borrow().inner.text().map(str::to_string) else {
            return Ok(py.None());
        };
        if let Some(existing) = &slf.borrow().tempo_text {
            return Ok(existing.clone_ref(py));
        }
        let object = py
            .import("music21.tempo")?
            .getattr("TempoText")?
            .call1((text,))?;
        if slf
            .getattr("hasStyleInformation")
            .and_then(|said| said.extract::<bool>())
            .unwrap_or(false)
        {
            object.setattr("style", slf.getattr("style")?)?;
        } else {
            slf.setattr("style", object.getattr("style")?)?;
        }
        // `TempoText` gave its own style to the expression when it built it,
        // so the expression has to be pointed at the shared one too.
        object
            .getattr("_textExpression")?
            .setattr("style", slf.getattr("style")?)?;
        slf.borrow_mut().tempo_text = Some(object.clone().unbind());
        Ok(object.unbind())
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
        if useNumberSounding {
            return self.inner.sounding_quarter_bpm();
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
            self.inner.set_number_sounding(Some(counted));
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
        let quarter_bpm = self
            .getQuarterBPM(true)
            .ok_or_else(|| MetronomeMarkException::new_err("this mark says no tempo"))?;
        if quarter_bpm == 0.0 {
            return Err(pyo3::exceptions::PyZeroDivisionError::new_err(
                "float division by zero",
            ));
        }
        Ok(60.0 / quarter_bpm)
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
    fn getTextExpression(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        returnImplicit: bool,
    ) -> PyResult<Py<PyAny>> {
        let (text, number_implicit) = {
            let mark = slf.borrow();
            let Some(text) = mark.inner.text() else {
                return Ok(py.None());
            };
            if mark.inner.text_implicit() && !returnImplicit {
                return Ok(py.None());
            }
            (text.to_string(), mark.inner.number_implicit())
        };
        let expression = py
            .import("music21.expressions")?
            .getattr("TextExpression")?
            .call1((text,))?;
        // music21's `applyTextFormatting`: a tempo word is written in bold,
        // four and a half staff lines up — or two, when the number it stands
        // for is not written beside it.
        // music21 links the mark's style to the words, so anything said
        // about how the mark is written is said about them: its own
        // `setTextExpression` shares the one object between the two.
        if slf
            .getattr("hasStyleInformation")
            .and_then(|said| said.extract::<bool>())
            .unwrap_or(false)
        {
            expression.setattr("style", slf.getattr("style")?)?;
        }
        let style = expression.getattr("style")?;
        style.setattr("fontStyle", "bold")?;
        style.setattr("absoluteY", if number_implicit { 20 } else { 45 })?;
        Ok(expression.unbind())
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
        let _ = py;
        sounding_mark(found)
    }

    /// music21's `getPreviousMetronomeMark`: the last one in force before
    /// this one, found by asking the stream this mark sits in.
    fn getPreviousMetronomeMark(slf: &Bound<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let Some(found) = previous_indication(slf.as_any())? else {
            return Ok(py.None());
        };
        Self::getSoundingMetronomeMark(slf, py, Some(&found))
    }

    fn __repr__(&self) -> String {
        let mut sounding = "";
        let mut number = self.inner.number();
        if self.inner.number().is_none() && self.inner.number_sounding().is_some() {
            sounding = " (playback only)";
            number = self.inner.number_sounding();
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
            .is_ok_and(|other| other.inner == self.inner)
    }

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
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

/// music21's `tempo.TempoText`: a tempo said in words.
///
/// The words are the crate's value. Where music21 is there they are also
/// carried in music21's own `TextExpression`, as music21 keeps them, whose
/// style this object shares -- which is how a score writes them, in bold and
/// four and a half staff lines up.
#[pyclass(
    name = "TempoText",
    module = "music21.tempo",
    subclass,
    skip_from_py_object
)]
pub struct TempoText {
    inner: RsTempoText,
    /// Whether any words were given: music21's `TempoText()` has none.
    said: bool,
    /// music21's `_textExpression`, where music21 is there to build one.
    text_expression: Option<Py<PyAny>>,
}

impl TempoText {
    /// Whether the music21 half of this object has been told how it is
    /// written; false where there is no music21 half.
    fn has_style(slf: &Bound<'_, Self>) -> bool {
        slf.getattr("hasStyleInformation")
            .and_then(|said| said.extract::<bool>())
            .unwrap_or(false)
    }

    /// music21's `text` setter.
    fn say(slf: &Bound<'_, Self>, text: String) -> PyResult<()> {
        let py = slf.py();
        {
            let mut me = slf.borrow_mut();
            me.inner.set_text(text.clone());
            me.said = true;
        }
        let existing = slf
            .borrow()
            .text_expression
            .as_ref()
            .map(|expression| expression.clone_ref(py));
        if let Some(expression) = existing {
            return expression.bind(py).setattr("content", text);
        }
        let Ok(class) = py
            .import("music21.expressions")
            .and_then(|module| module.getattr("TextExpression"))
        else {
            return Ok(());
        };
        let expression = class.call1((text,))?;
        if Self::has_style(slf) {
            expression.setattr("style", slf.getattr("style")?)?;
        } else {
            if let Ok(style) = expression.getattr("style") {
                let _ = slf.setattr("style", style);
            }
            Self::format(&expression, false)?;
        }
        slf.borrow_mut().text_expression = Some(expression.unbind());
        Ok(())
    }

    /// music21's `applyTextFormatting` on one expression.
    fn format(expression: &Bound<'_, PyAny>, number_implicit: bool) -> PyResult<()> {
        let style = expression.getattr("style")?;
        style.setattr("fontStyle", "bold")?;
        style.setattr("absoluteY", if number_implicit { 20 } else { 45 })
    }
}

#[pymethods]
impl TempoText {
    #[new]
    #[pyo3(signature = (text = None, **_keywords))]
    fn new(
        text: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let written = match text.filter(|text| !text.is_none()) {
            Some(text) => Some(text.str()?.extract::<String>()?),
            None => None,
        };
        Ok(Self {
            said: written.is_some(),
            inner: RsTempoText::new(written.unwrap_or_default()),
            text_expression: None,
        })
    }

    /// Built again here, where the music21 half exists to share a style
    /// with, since a Python subclass hands `__new__` its own arguments.
    #[pyo3(signature = (text = None, **_keywords))]
    fn __init__(
        slf: &Bound<'_, Self>,
        text: Option<&Bound<'_, PyAny>>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        {
            let mut me = slf.borrow_mut();
            me.text_expression = None;
            me.said = false;
        }
        if let Some(text) = text.filter(|text| !text.is_none()) {
            Self::say(slf, text.str()?.extract()?)?;
        }
        Ok(())
    }

    fn _reprInternal(&self) -> PyResult<String> {
        Python::attach(|py| self.inner.text().into_pyobject(py)?.repr()?.extract())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.tempo.TempoText {}>",
            slf.borrow()._reprInternal()?
        ))
    }

    /// The words, as the text expression holding them now says them.
    #[getter]
    fn get_text(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let me = slf.borrow();
        if let Some(expression) = &me.text_expression {
            return Ok(expression.bind(py).getattr("content")?.unbind());
        }
        if !me.said {
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                "'NoneType' object has no attribute 'content'",
            ));
        }
        Ok(me.inner.text().into_pyobject(py)?.into_any().unbind())
    }

    #[setter]
    fn set_text(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::say(slf, value.str()?.extract()?)
    }

    /// music21's `_textExpression`, which its own code reads directly.
    #[getter]
    fn get__textExpression(&self, py: Python<'_>) -> Py<PyAny> {
        self.text_expression
            .as_ref()
            .map_or_else(|| py.None(), |expression| expression.clone_ref(py))
    }

    #[setter]
    fn set__textExpression(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            self.text_expression = None;
            return Ok(());
        }
        if let Ok(content) = value
            .getattr("content")
            .and_then(|content| content.extract::<String>())
        {
            self.inner.set_text(content);
            self.said = true;
        }
        self.text_expression = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `getMetronomeMark`: the mark the words imply, sharing this
    /// object's style.
    fn getMetronomeMark(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let text = Self::get_text(slf)?.bind(py).extract::<String>()?;
        let mark = new_mark(py, RsTempoText::new(text).metronome_mark())?;
        let mark = mark.bind(py);
        if Self::has_style(slf) {
            mark.setattr("style", slf.getattr("style")?)?;
        } else if let Ok(style) = mark.getattr("style") {
            let _ = slf.setattr("style", style);
        }
        Ok(mark.clone().unbind())
    }

    /// music21's `getTextExpression`: a copy of the text expression, or
    /// nothing where there is none.
    #[pyo3(signature = (numberImplicit = false))]
    fn getTextExpression(&self, py: Python<'_>, numberImplicit: bool) -> PyResult<Py<PyAny>> {
        let _ = numberImplicit;
        match &self.text_expression {
            Some(expression) => Ok(py
                .import("copy")?
                .getattr("deepcopy")?
                .call1((expression.bind(py),))?
                .unbind()),
            None => Ok(py.None()),
        }
    }

    /// music21's `setTextExpression`: holds `value`, linking styles as
    /// music21 does.
    fn setTextExpression(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().set__textExpression(value)?;
        let expression_styled = value
            .getattr("hasStyleInformation")
            .and_then(|said| said.extract::<bool>())
            .unwrap_or(false);
        if expression_styled {
            slf.setattr("style", value.getattr("style")?)?;
        } else if Self::has_style(slf) {
            value.setattr("style", slf.getattr("style")?)?;
        } else {
            slf.setattr("style", value.getattr("style")?)?;
            Self::format(value, false)?;
        }
        Ok(())
    }

    /// music21's `applyTextFormatting`: a tempo word in bold, four and a half
    /// staff lines up, or two where no number is written beside it.
    #[pyo3(signature = (te = None, numberImplicit = false))]
    fn applyTextFormatting(
        &self,
        py: Python<'_>,
        te: Option<&Bound<'_, PyAny>>,
        numberImplicit: bool,
    ) -> PyResult<Py<PyAny>> {
        let expression = match te.filter(|te| !te.is_none()) {
            Some(te) => te.clone(),
            None => match &self.text_expression {
                Some(expression) => expression.bind(py).clone(),
                None => return Ok(py.None()),
            },
        };
        Self::format(&expression, numberImplicit)?;
        Ok(expression.unbind())
    }

    /// music21's `isCommonTempoText`: whether the words, or `value`, read as
    /// a tempo.
    #[pyo3(signature = (value = None))]
    fn isCommonTempoText(slf: &Bound<'_, Self>, value: Option<String>) -> PyResult<bool> {
        let text = match value {
            Some(value) => value,
            None => Self::get_text(slf)?.bind(slf.py()).extract()?,
        };
        Ok(music21_rs_crate::tempo::is_common_tempo_text(&text))
    }

    /// music21 freezes a score by pickling it; the words go as the crate's
    /// value and the text expression beside it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let me = slf.borrow();
        let extra = PyDict::new(py);
        extra.set_item("said", me.said)?;
        extra.set_item("textExpression", me.text_expression.as_ref())?;
        crate::pickled_extra(slf, &me.inner, Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (inner, extra) = crate::unpickled_extra::<_, RsTempoText>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        let mut me = slf.borrow_mut();
        me.inner = inner;
        if let Some(extra) = extra
            && let Ok(extra) = extra.bind(py).cast::<PyDict>()
        {
            if let Some(said) = extra.get_item("said")? {
                me.said = said.extract()?;
            }
            if let Some(expression) = extra.get_item("textExpression")?
                && !expression.is_none()
            {
                me.text_expression = Some(expression.unbind());
            }
        }
        Ok(())
    }

    /// A copy carries its own text expression, copied through the copier's
    /// memo, so the copy's words can change without the original's.
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let me = slf.borrow();
        let expression = match &me.text_expression {
            Some(expression) => Some(
                py.import("copy")?
                    .getattr("deepcopy")?
                    .call1((expression.bind(py), memo))?
                    .unbind(),
            ),
            None => None,
        };
        let copied = Self {
            inner: me.inner.clone(),
            said: me.said,
            text_expression: expression,
        };
        drop(me);
        crate::copy_as_same_type(slf, copied)
    }
}

/// A metronome mark of the class a new one is made as: the installed one
/// where music21 is there, this wheel's where not.
fn new_mark(py: Python<'_>, inner: RsMetronomeMark) -> PyResult<Py<PyAny>> {
    Ok(crate::installed_new(
        py,
        "music21.tempo",
        "MetronomeMark",
        MetronomeMark::wrap(inner),
    )?
    .into_any())
}

/// The crate's value of a metronome mark, whichever class holds it.
fn mark_value(mark: &Bound<'_, PyAny>) -> PyResult<RsMetronomeMark> {
    match mark.extract::<PyRef<'_, MetronomeMark>>() {
        Ok(ours) => Ok(ours.inner.clone()),
        Err(_) => {
            let number: Option<f64> = mark.getattr("number")?.extract()?;
            let mut value = RsMetronomeMark::default()
                .with_referent(referent_duration(Some(&mark.getattr("referent")?))?);
            value.set_number(number);
            Ok(value)
        }
    }
}

/// music21's `tempo.MetricModulation`: a change of tempo written as an
/// equation between two metronome marks.
///
/// It keeps the mark objects it is given, as music21 does, and works out the
/// sides it computes with the crate's `MetricModulation`. Where the marks
/// leave a number unsaid it asks the stream it sits in for the mark in force
/// before it, which only an object music21 holds in a stream can answer.
#[pyclass(
    name = "MetricModulation",
    module = "music21.tempo",
    subclass,
    skip_from_py_object
)]
pub struct MetricModulation {
    old: Option<Py<PyAny>>,
    new: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    classicalStyle: bool,
    #[pyo3(get, set)]
    maintainBeat: bool,
    #[pyo3(get, set)]
    transitionSymbol: String,
    #[pyo3(get, set)]
    arrowDirection: Option<Py<PyAny>>,
    #[pyo3(get, set)]
    parentheses: bool,
}

impl MetricModulation {
    fn blank() -> Self {
        Self {
            old: None,
            new: None,
            classicalStyle: false,
            maintainBeat: false,
            transitionSymbol: "=".to_string(),
            arrowDirection: None,
            parentheses: false,
        }
    }

    /// The crate's modulation over the marks this one holds.
    fn value(&self, py: Python<'_>) -> PyResult<RsMetricModulation> {
        let mut value = RsMetricModulation::new();
        if let Some(old) = &self.old {
            value.set_old_metronome(Some(mark_value(old.bind(py))?));
        }
        if let Some(new) = &self.new {
            value.set_new_metronome(Some(mark_value(new.bind(py))?));
        }
        Ok(value)
    }

    /// Takes the sides the crate changed, as new mark objects; a side it did
    /// not change keeps the object it was.
    fn adopt(
        &mut self,
        py: Python<'_>,
        before: &RsMetricModulation,
        after: RsMetricModulation,
    ) -> PyResult<()> {
        if before.old_metronome() != after.old_metronome() {
            self.old = after
                .old_metronome()
                .cloned()
                .map(|mark| new_mark(py, mark))
                .transpose()?;
        }
        if before.new_metronome() != after.new_metronome() {
            self.new = after
                .new_metronome()
                .cloned()
                .map(|mark| new_mark(py, mark))
                .transpose()?;
        }
        Ok(())
    }

    fn check_mark(value: Option<&Bound<'_, PyAny>>, which: &str) -> PyResult<Option<Py<PyAny>>> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Ok(None);
        };
        let is_mark = value.is_instance_of::<MetronomeMark>()
            || value
                .getattr("classes")
                .and_then(|classes| classes.extract::<Vec<String>>())
                .is_ok_and(|classes| classes.iter().any(|name| name == "MetronomeMark"));
        if !is_mark {
            return Err(MetricModulationException::new_err(format!(
                "{which} property must be set with a MetronomeMark instance"
            )));
        }
        Ok(Some(value.clone().unbind()))
    }

    fn side(side: Option<&str>) -> PyResult<Option<ModulationSide>> {
        match side {
            None => Ok(None),
            Some("left") => Ok(Some(ModulationSide::Old)),
            Some("right") => Ok(Some(ModulationSide::New)),
            Some(other) => Err(TempoException::new_err(format!(
                "cannot set equality for a side of {other}"
            ))),
        }
    }

    /// The mark in force before this one in the stream it sits in, as the
    /// crate's value.
    fn previous(slf: &Bound<'_, Self>) -> PyResult<Option<RsMetronomeMark>> {
        let previous = Self::getPreviousMetronomeMark(slf)?;
        let previous = previous.bind(slf.py());
        if previous.is_none() {
            return Ok(None);
        }
        mark_value(previous).map(Some)
    }

    /// Asks the context for what a side leaves unsaid, as music21's getters
    /// do before handing a side back.
    fn settle(slf: &Bound<'_, Self>, which_new: bool) -> PyResult<()> {
        let py = slf.py();
        let unsaid = {
            let me = slf.borrow();
            let side = if which_new { &me.new } else { &me.old };
            match side {
                Some(mark) => mark.bind(py).getattr("number")?.is_none(),
                None => false,
            }
        };
        if unsaid {
            Self::updateByContext(slf)?;
        }
        Ok(())
    }

    fn apply(
        slf: &Bound<'_, Self>,
        change: impl FnOnce(&mut RsMetricModulation) -> music21_rs_crate::Result<()>,
    ) -> PyResult<()> {
        let py = slf.py();
        let before = slf.borrow().value(py)?;
        let mut after = before.clone();
        change(&mut after)
            .map_err(|error| TempoException::new_err(crate::pitch::message(&error)))?;
        slf.borrow_mut().adopt(py, &before, after)
    }
}

#[pymethods]
impl MetricModulation {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, pyo3::types::PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> Self {
        Self::blank()
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.old)?;
        visit.call(&self.new)?;
        visit.call(&self.arrowDirection)
    }

    fn __clear__(&mut self) {
        self.old = None;
        self.new = None;
        self.arrowDirection = None;
    }

    /// music21's sort order for a tempo indication among things at one
    /// offset: before the notes it governs.
    #[classattr]
    fn classSortOrder() -> i32 {
        1
    }

    /// The mark in force before the modulation, its number filled in from the
    /// context where it has none.
    #[getter]
    fn get_oldMetronome(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::settle(slf, false)?;
        let py = slf.py();
        Ok(slf.borrow().old.as_ref().map(|mark| mark.clone_ref(py)))
    }

    #[setter]
    fn set_oldMetronome(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.old = Self::check_mark(value, "oldMetronome")?;
        Ok(())
    }

    /// The mark in force after it, its number filled in from the context
    /// where it has none.
    #[getter]
    fn get_newMetronome(slf: &Bound<'_, Self>) -> PyResult<Option<Py<PyAny>>> {
        Self::settle(slf, true)?;
        let py = slf.py();
        Ok(slf.borrow().new.as_ref().map(|mark| mark.clone_ref(py)))
    }

    #[setter]
    fn set_newMetronome(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.new = Self::check_mark(value, "newMetronome")?;
        Ok(())
    }

    /// The note value the old mark counts.
    #[getter]
    fn get_oldReferent(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.old
            .as_ref()
            .map(|mark| Ok(mark.bind(py).getattr("referent")?.unbind()))
            .transpose()
    }

    #[setter]
    fn set_oldReferent(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Err(MetricModulationException::new_err(
                "cannot set old referent to None",
            ));
        };
        let referent = referent_duration(Some(value))?;
        let previous = if slf.borrow().old.is_none() {
            Self::previous(slf)?
        } else {
            None
        };
        Self::apply(slf, |modulation| {
            modulation.set_old_referent(referent, previous.as_ref());
            Ok(())
        })
    }

    /// The note value the new mark counts.
    #[getter]
    fn get_newReferent(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        self.new
            .as_ref()
            .map(|mark| Ok(mark.bind(py).getattr("referent")?.unbind()))
            .transpose()
    }

    #[setter]
    fn set_newReferent(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            return Err(MetricModulationException::new_err(
                "cannot set new referent to None",
            ));
        };
        let referent = referent_duration(Some(value))?;
        Self::apply(slf, |modulation| {
            modulation.set_new_referent(referent);
            Ok(())
        })
    }

    /// The number of the new mark, which is what the modulation sets.
    #[getter]
    fn number(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match &self.new {
            Some(mark) => Ok(mark.bind(py).getattr("number")?.unbind()),
            None => Ok(py.None()),
        }
    }

    /// music21's `updateByContext`: fills in what the marks leave unsaid from
    /// the mark in force before this one.
    fn updateByContext(slf: &Bound<'_, Self>) -> PyResult<()> {
        let previous = Self::previous(slf)?;
        let py = slf.py();
        let before = slf.borrow().value(py)?;
        let mut after = before.clone();
        after.update_from(previous.as_ref());
        // The new mark is changed where it stands, as music21 writes its
        // number into the object it holds.
        let number = after.new_metronome().and_then(RsMetronomeMark::number);
        let new = slf.borrow().new.as_ref().map(|mark| mark.clone_ref(py));
        if let (Some(new), Some(number)) = (new, number)
            && before.new_metronome().and_then(RsMetronomeMark::number) != Some(number)
        {
            new.bind(py).setattr("number", number)?;
        }
        let mut me = slf.borrow_mut();
        if before.old_metronome() != after.old_metronome() {
            me.old = after
                .old_metronome()
                .cloned()
                .map(|mark| new_mark(py, mark))
                .transpose()?;
        }
        Ok(())
    }

    /// music21's `setEqualityByReferent`: one side the same tempo as the
    /// other, counted in `referent`.
    #[pyo3(signature = (side = None, referent = None))]
    fn setEqualityByReferent(
        slf: &Bound<'_, Self>,
        side: Option<&str>,
        referent: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let side = Self::side(side)?;
        let referent = referent_duration(referent)?;
        Self::apply(slf, |modulation| {
            modulation.set_equality_by_referent(side, referent)
        })
    }

    /// music21's `setOtherByReferent`: one side the other's number, counted in
    /// `referent`.
    #[pyo3(signature = (side = None, referent = None))]
    fn setOtherByReferent(
        slf: &Bound<'_, Self>,
        side: Option<&str>,
        referent: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let side = Self::side(side)?;
        let referent = referent_duration(referent)?;
        Self::apply(slf, |modulation| {
            modulation.set_other_by_referent(side, referent)
        })
    }

    /// music21's `getSoundingMetronomeMark`: the mark a tempo indication
    /// comes to, which for a modulation is its new mark.
    #[pyo3(signature = (found = None))]
    fn getSoundingMetronomeMark(
        slf: &Bound<'_, Self>,
        found: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        match found.filter(|value| !value.is_none()) {
            None => Ok(Self::get_newMetronome(slf)?.unwrap_or_else(|| slf.py().None())),
            Some(found) if found.is(slf) => {
                Ok(Self::get_newMetronome(slf)?.unwrap_or_else(|| slf.py().None()))
            }
            Some(found) => sounding_mark(found),
        }
    }

    /// music21's `getPreviousMetronomeMark`: the last mark in force before
    /// this one, found by asking the stream it sits in; `None` where it sits
    /// in none.
    fn getPreviousMetronomeMark(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if !slf.hasattr("getContextByClass")? {
            return Ok(py.None());
        }
        let Some(found) = previous_indication(slf.as_any())? else {
            return Ok(py.None());
        };
        Self::getSoundingMetronomeMark(slf, Some(&found))
    }

    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let old = Self::get_oldMetronome(slf)?;
        let new = Self::get_newMetronome(slf)?;
        let written = |mark: Option<Py<PyAny>>| -> PyResult<String> {
            match mark {
                Some(mark) => Ok(mark.bind(py).str()?.to_string()),
                None => Ok("None".to_string()),
            }
        };
        Ok(format!("{}={}", written(old)?, written(new)?))
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.tempo.MetricModulation {}>",
            Self::_reprInternal(slf)?
        ))
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let deepcopy = py.import("copy")?.getattr("deepcopy")?;
        let copied = crate::blank_installed(&slf.get_type().into_any())?;
        let copy_of = |mark: &Option<Py<PyAny>>| -> PyResult<Option<Py<PyAny>>> {
            mark.as_ref()
                .map(|mark| Ok(deepcopy.call1((mark, memo))?.unbind()))
                .transpose()
        };
        let me = slf.borrow();
        let fresh = Self {
            old: copy_of(&me.old)?,
            new: copy_of(&me.new)?,
            classicalStyle: me.classicalStyle,
            maintainBeat: me.maintainBeat,
            transitionSymbol: me.transitionSymbol.clone(),
            arrowDirection: me.arrowDirection.as_ref().map(|value| value.clone_ref(py)),
            parentheses: me.parentheses,
        };
        drop(me);
        *copied.extract::<PyRefMut<'_, Self>>()? = fresh;
        Ok(copied)
    }

    /// Pickled as the marks it holds and its flags.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let me = slf.borrow();
            extra.set_item("old", me.old.as_ref())?;
            extra.set_item("new", me.new.as_ref())?;
            extra.set_item("classicalStyle", me.classicalStyle)?;
            extra.set_item("maintainBeat", me.maintainBeat)?;
            extra.set_item("transitionSymbol", &me.transitionSymbol)?;
            extra.set_item("arrowDirection", me.arrowDirection.as_ref())?;
            extra.set_item("parentheses", me.parentheses)?;
        }
        crate::pickled_extra(slf, &(), Some(&extra))
    }

    fn __setstate__(slf: &Bound<'_, Self>, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (_, extra) = crate::unpickled_extra::<_, ()>(slf, state)?;
        let Some(extra) = extra else {
            return Ok(());
        };
        let extra = extra.bind(py);
        let present = |name: &str| -> Option<Bound<'_, PyAny>> {
            extra.get_item(name).ok().filter(|value| !value.is_none())
        };
        let mut me = slf.borrow_mut();
        me.old = present("old").map(Bound::unbind);
        me.new = present("new").map(Bound::unbind);
        me.classicalStyle =
            present("classicalStyle").is_some_and(|value| value.is_truthy().unwrap_or(false));
        me.maintainBeat =
            present("maintainBeat").is_some_and(|value| value.is_truthy().unwrap_or(false));
        if let Some(symbol) = present("transitionSymbol") {
            me.transitionSymbol = symbol.extract()?;
        }
        me.arrowDirection = present("arrowDirection").map(Bound::unbind);
        me.parentheses =
            present("parentheses").is_some_and(|value| value.is_truthy().unwrap_or(false));
        Ok(())
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

/// The mark a tempo indication comes to: a modulation's new mark, a mark
/// itself, or the mark a tempo text implies.
fn sounding_mark(found: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
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
    Err(TempoException::new_err(format!(
        "cannot derive a MetronomeMark from this TempoIndication: {found}"
    )))
}

/// The tempo indication in force before `holder`, asked of the stream it
/// sits in, or `None` where there is none.
fn previous_indication<'py>(holder: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = holder.py();
    let search = py
        .import("music21.common.enums")?
        .getattr("ElementSearch")?
        .getattr("BEFORE_OFFSET")?;
    let arguments = PyDict::new(py);
    arguments.set_item("getElementMethod", search)?;
    let found = holder.call_method("getContextByClass", ("TempoIndication",), Some(&arguments))?;
    Ok((!found.is_none()).then_some(found))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<MetronomeMark>()?;
    m.add_class::<MetricModulation>()?;
    m.add_class::<TempoText>()?;
    m.add_function(wrap_pyfunction!(convertTempoByReferent, m)?)?;
    Ok(())
}
