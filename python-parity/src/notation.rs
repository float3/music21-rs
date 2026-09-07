//! music21's `tie.Tie`, `note.Lyric`, `volume.Volume` and the slice of
//! `style.Style` that a note carries, over `music21-rs`.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

use music21_rs::{
    Lyric as RsLyric, Placement, Syllabic, Tie as RsTie, TieStyle, TieType, Volume as RsVolume,
};

use crate::pitch::message;

pyo3::create_exception!(music21_rs_facade, TieException, PyValueError);
pyo3::create_exception!(music21_rs_facade, LyricException, PyException);
pyo3::create_exception!(music21_rs_facade, VolumeException, PyException);

fn tie_error(error: music21_rs::Error) -> PyErr {
    TieException::new_err(message(&error))
}

fn lyric_error(error: music21_rs::Error) -> PyErr {
    LyricException::new_err(message(&error))
}

fn volume_error(error: music21_rs::Error) -> PyErr {
    VolumeException::new_err(message(&error))
}

/// music21's `tie.Tie`.
#[pyclass(name = "Tie", module = "music21.tie", skip_from_py_object)]
#[derive(Clone)]
pub struct Tie {
    pub(crate) inner: RsTie,
}

impl Tie {
    pub(crate) fn wrap(inner: RsTie) -> Self {
        Self { inner }
    }
}

/// Reads a tie argument: a `Tie` or one of music21's type names.
pub(crate) fn tie_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsTie> {
    if let Ok(facade) = value.extract::<PyRef<Tie>>() {
        return Ok(facade.inner.clone());
    }
    let name: String = value.extract()?;
    RsTie::from_name(&name).map_err(tie_error)
}

#[pymethods]
impl Tie {
    #[new]
    #[pyo3(signature = (r#type = "start"))]
    fn new(r#type: &str) -> PyResult<Self> {
        Ok(Self::wrap(RsTie::from_name(r#type).map_err(tie_error)?))
    }

    #[getter]
    fn get_type(&self) -> &'static str {
        self.inner.tie_type().as_str()
    }

    #[setter]
    fn set_type(&mut self, value: &str) -> PyResult<()> {
        self.inner
            .set_tie_type(TieType::from_name(value).map_err(tie_error)?);
        Ok(())
    }

    #[getter]
    fn get_style(&self) -> &'static str {
        self.inner.style().as_str()
    }

    #[setter]
    fn set_style(&mut self, value: &str) -> PyResult<()> {
        self.inner
            .set_style(TieStyle::from_name(value).map_err(tie_error)?);
        Ok(())
    }

    #[getter]
    fn get_placement(&self) -> Option<&'static str> {
        self.inner.placement().map(Placement::as_str)
    }

    #[setter]
    fn set_placement(&mut self, value: Option<&str>) -> PyResult<()> {
        let placement = match value {
            None => None,
            Some(value) => Some(Placement::from_name(value).map_err(tie_error)?),
        };
        self.inner.set_placement(placement);
        Ok(())
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Tie>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    fn __hash__(&self) -> isize {
        self.inner.tie_type() as isize
    }

    fn __repr__(&self) -> String {
        format!("<music21.tie.Tie {}>", self.inner.tie_type())
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// music21's `note.Lyric`.
#[pyclass(name = "Lyric", module = "music21.note", skip_from_py_object)]
#[derive(Clone)]
pub struct Lyric {
    pub(crate) inner: RsLyric,
}

impl Lyric {
    pub(crate) fn wrap(inner: RsLyric) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl Lyric {
    #[new]
    #[pyo3(signature = (text = None, number = 1, *, applyRaw = false, syllabic = None, identifier = None))]
    fn new(
        text: Option<&str>,
        number: i32,
        applyRaw: bool,
        syllabic: Option<&str>,
        identifier: Option<String>,
    ) -> PyResult<Self> {
        let mut inner = match text {
            Some(text) if !applyRaw => RsLyric::from_raw_text(text),
            Some(text) => RsLyric::new(text),
            None => RsLyric::new(""),
        };
        inner.set_number(number).map_err(lyric_error)?;
        if let Some(syllabic) = syllabic {
            inner.set_syllabic(Syllabic::from_name(syllabic).map_err(lyric_error)?);
        }
        inner.set_identifier(identifier);
        Ok(Self::wrap(inner))
    }

    #[getter]
    fn get_text(&self) -> String {
        self.inner.text()
    }

    #[setter]
    fn set_text(&mut self, value: &str) {
        self.inner.set_text(value);
    }

    #[getter]
    fn get_rawText(&self) -> String {
        self.inner.raw_text()
    }

    #[setter]
    fn set_rawText(&mut self, value: &str) {
        self.inner.set_raw_text(value);
    }

    #[getter]
    fn get_number(&self) -> i32 {
        self.inner.number()
    }

    /// music21 refuses anything that is not a number here, `None` included,
    /// with a `LyricException` rather than the `TypeError` a typed argument
    /// would raise.
    #[setter]
    fn set_number(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let Ok(number) = value.extract::<i32>() else {
            return Err(LyricException::new_err("Number best be number"));
        };
        self.inner.set_number(number).map_err(lyric_error)
    }

    #[getter]
    fn get_syllabic(&self) -> &'static str {
        self.inner.syllabic().as_str()
    }

    #[setter]
    fn set_syllabic(&mut self, value: &str) -> PyResult<()> {
        self.inner
            .set_syllabic(Syllabic::from_name(value).map_err(lyric_error)?);
        Ok(())
    }

    /// music21's `identifier`: the name this verse goes by, and the verse
    /// *number* itself — an `int`, not its digits — when it has no name.
    #[getter]
    fn get_identifier(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.inner.explicit_identifier() {
            Some(identifier) => Ok(identifier.into_pyobject(py)?.into_any().unbind()),
            None => Ok(self.inner.number().into_pyobject(py)?.into_any().unbind()),
        }
    }

    #[setter]
    fn set_identifier(&mut self, value: Option<String>) {
        self.inner.set_identifier(value);
    }

    #[getter]
    fn isComposite(&self) -> bool {
        self.inner.is_composite()
    }

    /// music21's `components`: the lyrics a composite one is made of, and
    /// `None` when it is an ordinary syllable.
    #[getter]
    fn get_components(&self) -> Option<Vec<Lyric>> {
        if !self.inner.is_composite() {
            return None;
        }
        Some(
            self.inner
                .components()
                .iter()
                .cloned()
                .map(Lyric::wrap)
                .collect(),
        )
    }

    #[setter]
    fn set_components(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.inner.set_components(Vec::new());
            return Ok(());
        };
        let mut components = Vec::new();
        for item in value.try_iter()? {
            components.push(item?.extract::<PyRef<'_, Lyric>>()?.inner.clone());
        }
        self.inner.set_components(components);
        Ok(())
    }

    #[getter]
    fn get_elisionBefore(&self) -> String {
        self.inner.elision_before().to_string()
    }

    #[setter]
    fn set_elisionBefore(&mut self, value: &str) {
        self.inner.set_elision_before(value);
    }

    /// music21's `setTextAndSyllabic`. With `applyRaw` the hyphens stay in
    /// the text and the syllabic is left as it was, except that a lyric that
    /// never had one becomes a whole word.
    #[pyo3(signature = (rawText, applyRaw = false))]
    fn setTextAndSyllabic(&mut self, rawText: &str, applyRaw: bool) {
        if applyRaw {
            self.inner.set_text(rawText);
        } else {
            self.inner.set_raw_text(rawText);
        }
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Lyric>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    fn __hash__(&self) -> isize {
        self.inner.number() as isize
    }

    fn __repr__(&self) -> String {
        format!(
            "<music21.note.Lyric number={} syllabic={} text='{}'>",
            self.inner.number(),
            self.inner.syllabic(),
            self.inner.text()
        )
    }

    fn __str__(&self) -> String {
        self.inner.text()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// music21's `volume.Volume`.
#[pyclass(name = "Volume", module = "music21.volume", skip_from_py_object)]
#[derive(Clone)]
pub struct Volume {
    pub(crate) inner: RsVolume,
}

impl Volume {
    pub(crate) fn wrap(inner: RsVolume) -> Self {
        Self { inner }
    }
}

/// Reads a volume argument: a `Volume`, a velocity, or a scalar below one.
pub(crate) fn volume_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsVolume> {
    if let Ok(facade) = value.extract::<PyRef<Volume>>() {
        return Ok(facade.inner.clone());
    }
    let number: f64 = value.extract()?;
    if number < 1.0 {
        RsVolume::from_velocity_scalar(number).map_err(volume_error)
    } else {
        Ok(RsVolume::from_velocity(number as i32))
    }
}

#[pymethods]
impl Volume {
    #[new]
    #[pyo3(signature = (*, client = None, velocity = None, velocityScalar = None, velocityIsRelative = true))]
    fn new(
        client: Option<&Bound<'_, PyAny>>,
        velocity: Option<i32>,
        velocityScalar: Option<f64>,
        velocityIsRelative: bool,
    ) -> PyResult<Self> {
        let _ = client;
        let mut inner = RsVolume::new();
        if let Some(velocity) = velocity {
            inner.set_velocity(Some(velocity));
        } else if let Some(scalar) = velocityScalar {
            inner
                .set_velocity_scalar(Some(scalar))
                .map_err(volume_error)?;
        }
        inner.set_velocity_is_relative(velocityIsRelative);
        Ok(Self::wrap(inner))
    }

    #[getter]
    fn get_velocity(&self) -> Option<i32> {
        self.inner.velocity()
    }

    #[setter]
    fn set_velocity(&mut self, value: Option<f64>) -> PyResult<()> {
        match value {
            None => self.inner.set_velocity(None),
            Some(value) if value.is_nan() => {
                return Err(VolumeException::new_err(
                    "value provided for velocity must be a number, not nan",
                ));
            }
            Some(value) => self.inner.set_velocity(Some(value as i32)),
        }
        Ok(())
    }

    #[getter]
    fn get_velocityScalar(&self) -> Option<f64> {
        self.inner.velocity_scalar()
    }

    #[setter]
    fn set_velocityScalar(&mut self, value: Option<f64>) -> PyResult<()> {
        self.inner.set_velocity_scalar(value).map_err(volume_error)
    }

    #[getter]
    fn get_velocityIsRelative(&self) -> bool {
        self.inner.velocity_is_relative()
    }

    #[setter]
    fn set_velocityIsRelative(&mut self, value: bool) {
        self.inner.set_velocity_is_relative(value);
    }

    #[getter]
    fn realized(&self) -> f64 {
        self.inner.realized()
    }

    #[getter]
    fn cachedRealized(&self) -> f64 {
        self.inner.realized()
    }

    #[getter]
    fn cachedRealizedStr(&self) -> &'static str {
        self.inner.realized_str()
    }

    #[pyo3(signature = (
        useDynamicContext = None,
        useVelocity = true,
        useArticulations = None,
        baseLevel = 0.5,
        clip = true,
    ))]
    fn getRealized(
        &self,
        useDynamicContext: Option<f64>,
        useVelocity: bool,
        useArticulations: Option<f64>,
        baseLevel: f64,
        clip: bool,
    ) -> f64 {
        let mut volume = self.inner.clone();
        if !useVelocity {
            volume.set_velocity(None);
        }
        volume.realized_with(useDynamicContext, useArticulations, baseLevel, clip)
    }

    #[pyo3(signature = (**_keywords))]
    fn getRealizedStr(&self, _keywords: Option<&Bound<'_, PyAny>>) -> &'static str {
        self.inner.realized_str()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Volume>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    fn __repr__(&self) -> String {
        format!("<music21.volume.Volume {}>", self.inner)
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// Which object a [`Style`] writes back into.
pub(crate) enum StyleOwner {
    /// A note's style.
    Note(Py<crate::note::Note>),
    /// A chord's own style, which its notes fall back to.
    Chord(Py<crate::chord::Chord>),
    /// An accidental's style.
    Accidental(Py<crate::pitch::Accidental>),
}

/// The part of music21's `style.Style` a note or chord carries here: the
/// colour it is written in. music21's Style also holds layout — absolute and
/// relative positions, sizes, page breaks — which needs a page to mean
/// anything and is not modelled.
#[pyclass(name = "Style", module = "music21.style", skip_from_py_object)]
pub struct Style {
    pub(crate) owner: StyleOwner,
}

#[pymethods]
impl Style {
    #[getter]
    fn get_color(&self, py: Python<'_>) -> Option<String> {
        match &self.owner {
            StyleOwner::Note(note) => note.borrow(py).inner.color().map(str::to_string),
            StyleOwner::Chord(chord) => chord.borrow(py).inner.color().map(str::to_string),
            StyleOwner::Accidental(accidental) => {
                accidental.borrow(py).inner.color().map(str::to_string)
            }
        }
    }

    #[setter]
    fn set_color(&mut self, py: Python<'_>, value: Option<String>) {
        match &self.owner {
            StyleOwner::Note(note) => note.borrow_mut(py).inner.set_color(value),
            StyleOwner::Chord(chord) => chord.borrow_mut(py).inner.set_color(value),
            StyleOwner::Accidental(accidental) => accidental.borrow_mut(py).inner.set_color(value),
        }
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        match self.get_color(py) {
            Some(color) => format!("<music21.style.Style color={color:?}>"),
            None => "<music21.style.Style>".to_string(),
        }
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Tie>()?;
    m.add_class::<Lyric>()?;
    m.add_class::<Volume>()?;
    m.add_class::<Style>()?;
    let tie_exception = py.get_type::<TieException>();
    tie_exception.setattr("__module__", "music21.tie")?;
    m.add("TieException", tie_exception)?;
    let lyric_exception = py.get_type::<LyricException>();
    lyric_exception.setattr("__module__", "music21.note")?;
    m.add("LyricException", lyric_exception)?;
    let volume_exception = py.get_type::<VolumeException>();
    volume_exception.setattr("__module__", "music21.volume")?;
    m.add("VolumeException", volume_exception)?;
    Ok(())
}
