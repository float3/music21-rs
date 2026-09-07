//! music21's `tie.Tie`, `note.Lyric`, `volume.Volume` and the slice of
//! `style.Style` that a note carries, over `music21-rs`.

#![allow(non_snake_case)]

use pyo3::exceptions::{PyException, PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use music21_rs::{
    Beam as RsBeam, BeamDirection as RsBeamDirection, BeamType as RsBeamType, Beams as RsBeams,
    DurationType as RsDurationType, Lyric as RsLyric, Placement, Syllabic, Tie as RsTie, TieStyle,
    TieType, Volume as RsVolume,
};

use crate::pitch::message;

/// The names the `notation` facade replaces in `music21.tie`.
pub const TIE_NAMES: &[&str] = &["Tie", "TieException"];

/// The names the `notation` facade replaces in `music21.volume`.
pub const VOLUME_NAMES: &[&str] = &["Volume", "VolumeException"];

/// The names the `notation` facade replaces in `music21.style`.
pub const STYLE_NAMES: &[&str] = &["Style"];

/// The names the `notation` facade replaces in `music21.beam`.
pub const BEAM_NAMES: &[&str] = &["Beam", "Beams", "BeamException"];

pyo3::create_exception!(music21_rs_facade, TieException, PyValueError);
pyo3::create_exception!(music21_rs_facade, LyricException, PyException);
pyo3::create_exception!(music21_rs_facade, VolumeException, PyException);
pyo3::create_exception!(music21_rs_facade, BeamException, PyException);

fn beam_error(error: music21_rs::Error) -> PyErr {
    BeamException::new_err(message(&error))
}

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
pub struct Lyric {
    pub(crate) inner: RsLyric,
    /// The syllables an elided lyric is made of, as the Python objects a
    /// caller handed over.
    ///
    /// They are kept as objects rather than as values because music21's
    /// elisions are edited after the fact — the madrigal example sets the
    /// elision character and then the syllabic on a component already inside
    /// the composite, and both have to show through.
    components: Vec<Py<Lyric>>,
}

impl Lyric {
    pub(crate) fn wrap(inner: RsLyric) -> Self {
        Self {
            inner,
            components: Vec::new(),
        }
    }

    /// This lyric as a value, with whatever its component objects say now
    /// written into it, so the crate answers every question about it.
    pub(crate) fn synced(&self, py: Python<'_>) -> RsLyric {
        if self.components.is_empty() {
            return self.inner.clone();
        }
        let mut lyric = self.inner.clone();
        lyric.set_components(
            self.components
                .iter()
                .map(|component| component.borrow(py).synced(py))
                .collect(),
        );
        lyric
    }
}

impl Clone for Lyric {
    /// A copy takes copies of its syllables, as a deepcopy of music21's does.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner.clone(),
            components: self
                .components
                .iter()
                .filter_map(|component| Py::new(py, component.borrow(py).clone()).ok())
                .collect(),
        })
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
    fn get_text(&self, py: Python<'_>) -> String {
        self.synced(py).text()
    }

    /// Setting the text of an elided lyric makes it an ordinary one, since
    /// the syllables it was made of are what the text used to come from.
    #[setter]
    fn set_text(&mut self, value: &str) {
        self.components = Vec::new();
        self.inner.set_text(value);
    }

    #[getter]
    fn get_rawText(&self, py: Python<'_>) -> String {
        self.synced(py).raw_text()
    }

    #[setter]
    fn set_rawText(&mut self, value: &str) {
        self.components = Vec::new();
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
    fn get_syllabic(&self, py: Python<'_>) -> &'static str {
        self.synced(py).syllabic().as_str()
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
    fn isComposite(&self, py: Python<'_>) -> bool {
        self.synced(py).is_composite()
    }

    /// music21's `components`: the lyrics a composite one is made of, and
    /// `None` when it is an ordinary syllable.
    #[getter]
    fn get_components(&self, py: Python<'_>) -> Option<Vec<Py<Lyric>>> {
        if self.components.is_empty() {
            return None;
        }
        Some(
            self.components
                .iter()
                .map(|component| component.clone_ref(py))
                .collect(),
        )
    }

    #[setter]
    fn set_components(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.components = Vec::new();
            self.inner.set_components(Vec::new());
            return Ok(());
        };
        let mut components = Vec::new();
        for item in value.try_iter()? {
            components.push(item?.extract::<Py<Lyric>>()?);
        }
        self.components = components;
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

    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<Lyric>>()
            .is_ok_and(|other| other.synced(py) == self.synced(py))
    }

    fn __hash__(&self) -> isize {
        self.inner.number() as isize
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let lyric = self.synced(py);
        format!(
            "<music21.note.Lyric number={} syllabic={} text='{}'>",
            lyric.number(),
            lyric.syllabic(),
            lyric.text()
        )
    }

    fn __str__(&self, py: Python<'_>) -> String {
        self.synced(py).text()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// music21's `beam.Beam`: one beam at one level.
#[pyclass(name = "Beam", module = "music21.beam", skip_from_py_object)]
#[derive(Clone)]
pub struct Beam {
    pub(crate) inner: RsBeam,
}

/// Reads music21's beam type name.
fn beam_type_of(value: &Bound<'_, PyAny>) -> PyResult<RsBeamType> {
    let name: String = value.extract()?;
    RsBeamType::from_music21_name(&name)
        .ok_or_else(|| BeamException::new_err(format!("beam type cannot be {name}")))
}

/// The same where the type may be left unsaid, which is what a beam counted
/// but not yet decided has.
fn beam_type_or_none(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<RsBeamType>> {
    match value.filter(|value| !value.is_none()) {
        Some(value) => beam_type_of(value).map(Some),
        None => Ok(None),
    }
}

/// Reads music21's beam direction name, which only a stub has.
fn beam_direction_of(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<RsBeamDirection>> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(None);
    };
    let name: String = value.extract()?;
    RsBeamDirection::from_music21_name(&name)
        .map(Some)
        .ok_or_else(|| BeamException::new_err(format!("no such beam direction: {name}")))
}

#[pymethods]
impl Beam {
    #[new]
    #[pyo3(signature = (r#type = None, direction = None, number = None, **_keywords))]
    fn new(
        r#type: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
        number: Option<u32>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut inner = RsBeam::new(beam_type_or_none(r#type)?, beam_direction_of(direction)?);
        inner.set_number(number);
        Ok(Self { inner })
    }

    #[getter]
    fn get_type(&self) -> Option<&'static str> {
        self.inner.beam_type().map(RsBeamType::as_str)
    }

    #[setter]
    fn set_type(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.set_beam_type(beam_type_or_none(value)?);
        Ok(())
    }

    #[getter]
    fn get_direction(&self) -> Option<&'static str> {
        self.inner.direction().map(RsBeamDirection::as_str)
    }

    #[setter]
    fn set_direction(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.inner.set_direction(beam_direction_of(value)?);
        Ok(())
    }

    #[getter]
    fn get_number(&self) -> Option<u32> {
        self.inner.number()
    }

    #[setter]
    fn set_number(&mut self, value: Option<u32>) {
        self.inner.set_number(value);
    }

    fn __repr__(&self) -> String {
        format!("<music21.beam.Beam {}>", self.inner)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner)
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }
}

/// music21's `beam.Beams`: the beams of one note, one for each level it is
/// beamed at.
#[pyclass(name = "Beams", module = "music21.beam", skip_from_py_object)]
pub struct Beams {
    pub(crate) inner: RsBeams,
    /// The note or chord these belong to, so an edit through them reaches it.
    owner: Option<Py<PyAny>>,
}

impl Beams {
    pub(crate) fn wrap(inner: RsBeams) -> Self {
        Self { inner, owner: None }
    }

    /// The same, knowing what carries them.
    pub(crate) fn owned_by(inner: RsBeams, owner: Py<PyAny>) -> Self {
        Self {
            inner,
            owner: Some(owner),
        }
    }

    /// Writes these beams back into whatever carries them.
    fn write_back(&self, py: Python<'_>) -> PyResult<()> {
        let Some(owner) = &self.owner else {
            return Ok(());
        };
        let beams = Py::new(py, Self::wrap(self.inner.clone()))?;
        owner.bind(py).setattr("beams", beams.bind(py).as_any())
    }
}

impl Clone for Beams {
    /// A copy belongs to nobody yet, as a copy of a pitch does.
    fn clone(&self) -> Self {
        Self::wrap(self.inner.clone())
    }
}

/// How many beam levels a written value carries, from either of the two ways
/// music21 says it: the name of the value, or the number of lines.
fn beam_levels(level: Option<&Bound<'_, PyAny>>) -> PyResult<u32> {
    let Some(value) = level.filter(|value| !value.is_none()) else {
        return Ok(1);
    };
    if let Ok(name) = value.extract::<String>() {
        return RsDurationType::from_music21_name(&name)
            .and_then(RsBeams::levels_for)
            .ok_or_else(|| BeamException::new_err(format!("cannot fill beams for level {name}")));
    }
    let levels: u32 = value.extract()?;
    if levels == 0 || levels > 9 {
        return Err(BeamException::new_err(format!(
            "cannot fill beams for level {levels}"
        )));
    }
    Ok(levels)
}

/// The beams of one object, or nothing where it carries none: one entry of
/// music21's `naiveBeams` list, read back off Python.
fn beams_of(value: &Bound<'_, PyAny>) -> PyResult<Option<RsBeams>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract::<PyRef<'_, Beams>>()?.inner.clone()))
}

/// The same list, handed back to Python as `Beams` objects and `None`s.
fn beams_list<'py>(py: Python<'py>, beams: Vec<Option<RsBeams>>) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for entry in beams {
        match entry {
            Some(beams) => list.append(Py::new(py, Beams::wrap(beams))?)?,
            None => list.append(py.None())?,
        }
    }
    Ok(list)
}

#[pymethods]
impl Beams {
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self::wrap(RsBeams::new())
    }

    /// music21's `beamsList`: the beams themselves, in level order.
    #[getter]
    fn get_beamsList(&self) -> Vec<Beam> {
        self.inner
            .beams()
            .iter()
            .map(|beam| Beam { inner: *beam })
            .collect()
    }

    /// Setting it replaces them outright, which is how music21 says a note
    /// carries no beam at all.
    #[setter]
    fn set_beamsList(&mut self, py: Python<'_>, value: Vec<PyRef<'_, Beam>>) -> PyResult<()> {
        self.inner
            .set_beams(value.into_iter().map(|beam| beam.inner).collect());
        self.write_back(py)
    }

    /// music21's `append`: one more beam, a level down. It takes the type of
    /// the beam to make, or a beam already made.
    #[pyo3(signature = (r#type = None, direction = None))]
    fn append(
        &mut self,
        py: Python<'_>,
        r#type: Option<&Bound<'_, PyAny>>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        if let Some(value) = r#type
            && let Ok(beam) = value.extract::<PyRef<'_, Beam>>()
        {
            self.inner.beams_mut().push(beam.inner);
            return self.write_back(py);
        }
        self.inner
            .append(beam_type_or_none(r#type)?, beam_direction_of(direction)?);
        self.write_back(py)
    }

    /// music21's `fill`: as many beams as the written value has flags.
    ///
    /// It says how many lines there are and leaves what each does unsaid,
    /// unless a type is given.
    #[pyo3(signature = (level = None, r#type = None))]
    fn fill(
        &mut self,
        py: Python<'_>,
        level: Option<&Bound<'_, PyAny>>,
        r#type: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let levels = beam_levels(level)?;
        self.inner
            .fill_levels(levels, beam_type_or_none(r#type)?)
            .map_err(beam_error)?;
        self.write_back(py)
    }

    /// music21's `setAll`: every beam made the same.
    #[pyo3(signature = (r#type, direction = None))]
    fn setAll(
        &mut self,
        py: Python<'_>,
        r#type: &Bound<'_, PyAny>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.inner
            .set_all(beam_type_of(r#type)?, beam_direction_of(direction)?);
        self.write_back(py)
    }

    /// music21's `setByNumber`: the beam at one level made the same.
    ///
    /// The type and the direction may be written as one, hyphenated, which is
    /// how a partial beam is usually named.
    #[pyo3(signature = (number, r#type, direction = None))]
    fn setByNumber(
        &mut self,
        py: Python<'_>,
        number: u32,
        r#type: &Bound<'_, PyAny>,
        direction: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let (beam_type, beam_direction) = match r#type.extract::<String>() {
            Ok(written) if written.contains('-') => {
                let (name, side) = written.split_once('-').expect("a hyphen was just found");
                let name = name.to_string();
                let side = side.to_string();
                let name = name.into_pyobject(py)?;
                let side = side.into_pyobject(py)?;
                (
                    beam_type_of(name.as_any())?,
                    beam_direction_of(Some(side.as_any()))?,
                )
            }
            _ => (beam_type_of(r#type)?, beam_direction_of(direction)?),
        };
        if !self.inner.numbers().contains(&Some(number)) {
            return Err(PyIndexError::new_err(format!(
                "beam number {number} cannot be accessed"
            )));
        }
        self.inner
            .set_by_number(number, beam_type, beam_direction)
            .map_err(beam_error)?;
        self.write_back(py)
    }

    /// music21's `getByNumber`.
    fn getByNumber(&self, number: u32) -> PyResult<Beam> {
        self.inner
            .by_number(number)
            .map(|beam| Beam { inner: *beam })
            .ok_or_else(|| {
                PyIndexError::new_err(format!("beam number {number} cannot be accessed"))
            })
    }

    /// music21's `getTypeByNumber`, which writes a stub's direction into the
    /// name with a hyphen.
    fn getTypeByNumber(&self, number: u32) -> PyResult<Option<String>> {
        let beam = self.getByNumber(number)?;
        let Some(name) = beam.get_type() else {
            return Ok(None);
        };
        Ok(Some(match beam.get_direction() {
            Some(direction) => format!("{name}-{direction}"),
            None => name.to_string(),
        }))
    }

    /// music21's `getTypes`.
    fn getTypes(&self) -> Vec<Option<&'static str>> {
        self.inner
            .types()
            .into_iter()
            .map(|beam_type| beam_type.map(RsBeamType::as_str))
            .collect()
    }

    /// music21's `getNumbers`.
    fn getNumbers(&self) -> Vec<Option<u32>> {
        self.inner.numbers()
    }

    /// music21's `naiveBeams`: the fullest set of beams each of a run of
    /// objects could carry, with nothing said about what any of them does.
    ///
    /// Anything a quarter or longer carries none, and neither does a rest —
    /// there is nothing to beam a silence to.
    #[staticmethod]
    fn naiveBeams<'py>(
        py: Python<'py>,
        srcList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut beams = Vec::new();
        for element in srcList.try_iter()? {
            let element = element?;
            let name: String = element.getattr("duration")?.getattr("type")?.extract()?;
            let sounds = element
                .getattr("classSet")
                .and_then(|classes| classes.contains("NotRest"))
                .unwrap_or(false);
            let levels = RsDurationType::from_music21_name(&name)
                .and_then(RsBeams::levels_for)
                .filter(|_| sounds);
            beams.push(levels.and_then(|levels| {
                let mut made = RsBeams::new();
                made.fill_levels(levels, None).ok()?;
                Some(made)
            }));
        }
        beams_list(py, beams)
    }

    /// music21's `removeSandwichedUnbeamables`: a note with nothing beamable
    /// on either side of it has nothing to beam to, so it carries none.
    #[staticmethod]
    fn removeSandwichedUnbeamables<'py>(
        py: Python<'py>,
        beamsList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut beams: Vec<Option<RsBeams>> = Vec::new();
        for entry in beamsList.try_iter()? {
            beams.push(beams_of(&entry?)?);
        }
        music21_rs::notation::remove_sandwiched_unbeamables(&mut beams);
        let made = beams_list(py, beams)?;
        // music21 edits the list it was given and hands the same one back.
        let list = beamsList.cast::<PyList>()?;
        list.set_slice(0, list.len(), made.as_any())?;
        Ok(list.clone())
    }

    /// music21's `sanitizePartialBeams`: beams made only of stubs are no
    /// beams at all, and a stub next to a beam points towards it.
    #[staticmethod]
    fn sanitizePartialBeams<'py>(
        py: Python<'py>,
        beamsList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut beams: Vec<Option<RsBeams>> = Vec::new();
        for entry in beamsList.try_iter()? {
            beams.push(beams_of(&entry?)?);
        }
        music21_rs::notation::sanitize_partial_beams(&mut beams);
        let made = beams_list(py, beams)?;
        let list = beamsList.cast::<PyList>()?;
        list.set_slice(0, list.len(), made.as_any())?;
        Ok(list.clone())
    }

    /// music21's `mergeConnectingPartialBeams`: a stub pointing right into a
    /// stub pointing left is really one beam, and is written as one.
    #[staticmethod]
    fn mergeConnectingPartialBeams<'py>(
        py: Python<'py>,
        beamsList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let mut beams: Vec<Option<RsBeams>> = Vec::new();
        for entry in beamsList.try_iter()? {
            beams.push(beams_of(&entry?)?);
        }
        music21_rs::notation::merge_connecting_partial_beams(&mut beams);
        let made = beams_list(py, beams)?;
        let list = beamsList.cast::<PyList>()?;
        list.set_slice(0, list.len(), made.as_any())?;
        Ok(list.clone())
    }

    #[getter]
    fn get_feathered(&self) -> bool {
        self.inner.feathered()
    }

    #[setter]
    fn set_feathered(&mut self, value: bool) {
        self.inner.set_feathered(value);
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let list = PyList::empty(py);
        for beam in self.get_beamsList() {
            list.append(beam.into_pyobject(py)?)?;
        }
        Ok(list.try_iter()?.unbind().into_any())
    }

    fn __repr__(&self) -> String {
        format!("<music21.beam.Beams {}>", self.inner)
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, Self>>()
            .is_ok_and(|other| other.inner == self.inner)
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
pub struct Volume {
    pub(crate) inner: RsVolume,
    /// The note or chord this volume belongs to. music21's `_setVolume`
    /// reads the slot to decide whether the volume is already spoken for —
    /// a volume that has a client is copied rather than stolen — so without
    /// it music21's own `NotRest` cannot take one of ours at all.
    client: Option<Py<PyAny>>,
}

impl Volume {
    pub(crate) fn wrap(inner: RsVolume) -> Self {
        Self {
            inner,
            client: None,
        }
    }

    /// How much the dynamic in force scales this volume by.
    ///
    /// `False` says to ignore dynamics; a dynamic given outright is used as
    /// it stands; anything else means going and looking for one.
    fn dynamic_scalar(
        &self,
        py: Python<'_>,
        given: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<f64>> {
        let found = match given {
            Some(value) if value.extract::<bool>().is_ok_and(|say| !say) => return Ok(None),
            Some(value) if value.hasattr("volumeScalar")? => value.clone().unbind(),
            _ => self.getDynamicContext(py)?,
        };
        if found.is_none(py) {
            return Ok(None);
        }
        found.bind(py).getattr("volumeScalar")?.extract().map(Some)
    }

    /// How much the articulations on the note shift this volume.
    ///
    /// music21 adds each articulation's `volumeShift`, so an accent lifts a
    /// note rather than doubling it.
    fn articulation_shift(
        &self,
        py: Python<'_>,
        given: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<f64> {
        let marks = match given {
            Some(value) if value.extract::<bool>().is_ok_and(|say| !say) => return Ok(0.0),
            Some(value) if value.hasattr("volumeShift")? => {
                let list = PyList::empty(py);
                list.append(value)?;
                list.into_any().unbind()
            }
            Some(value) if value.try_iter().is_ok() => value.clone().unbind(),
            _ => match &self.client {
                Some(client) => client.bind(py).getattr("articulations")?.unbind(),
                None => PyList::empty(py).into_any().unbind(),
            },
        };
        let mut shift = 0.0;
        for mark in marks.bind(py).try_iter()? {
            shift += mark?.getattr("volumeShift")?.extract::<f64>()?;
        }
        Ok(shift)
    }

    /// The same, knowing what it is the volume of.
    pub(crate) fn owned_by(inner: RsVolume, client: Py<PyAny>) -> Self {
        Self {
            inner,
            client: Some(client),
        }
    }
}

impl Clone for Volume {
    /// A copy of a volume belongs to nobody yet, as music21's does.
    fn clone(&self) -> Self {
        Self::wrap(self.inner.clone())
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
        let mut inner = RsVolume::new();
        if let Some(velocity) = velocity {
            inner.set_velocity(Some(velocity));
        } else if let Some(scalar) = velocityScalar {
            inner
                .set_velocity_scalar(Some(scalar))
                .map_err(volume_error)?;
        }
        inner.set_velocity_is_relative(velocityIsRelative);
        let mut volume = Self::wrap(inner);
        volume.set_client(client);
        Ok(volume)
    }

    /// music21's `client`: the note or chord this volume is the volume of.
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
    fn cachedRealizedStr(&self) -> String {
        self.inner.realized_str()
    }

    /// music21's `mergeAttributes`: everything the other volume says except
    /// whose volume it is, copied rather than shared.
    fn mergeAttributes(&mut self, other: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(other) = other.filter(|value| !value.is_none()) else {
            return Ok(());
        };
        let other = other.extract::<PyRef<'_, Self>>()?;
        self.inner
            .set_velocity_scalar(other.inner.velocity_scalar())
            .map_err(volume_error)?;
        self.inner
            .set_velocity_is_relative(other.inner.velocity_is_relative());
        Ok(())
    }

    /// music21's `getDynamicContext`: the dynamic in force where the note
    /// this volume belongs to stands.
    ///
    /// The crate has no streams and cannot answer this; the note does, since
    /// it is a real `Music21Object` in a real stream, so the question is put
    /// to it.
    fn getDynamicContext(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let Some(client) = &self.client else {
            return Ok(py.None());
        };
        Ok(client
            .bind(py)
            .call_method1("getContextByClass", ("Dynamic",))?
            .unbind())
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
        py: Python<'_>,
        useDynamicContext: Option<&Bound<'_, PyAny>>,
        useVelocity: bool,
        useArticulations: Option<&Bound<'_, PyAny>>,
        baseLevel: f64,
        clip: bool,
    ) -> PyResult<f64> {
        let mut volume = self.inner.clone();
        if !useVelocity {
            volume.set_velocity(None);
        }
        Ok(volume.realized_with(
            self.dynamic_scalar(py, useDynamicContext)?,
            self.articulation_shift(py, useArticulations)?,
            baseLevel,
            clip,
        ))
    }

    #[pyo3(signature = (
        useDynamicContext = None,
        useVelocity = true,
        useArticulations = None,
        baseLevel = 0.5,
        clip = true,
    ))]
    fn getRealizedStr(
        &self,
        py: Python<'_>,
        useDynamicContext: Option<&Bound<'_, PyAny>>,
        useVelocity: bool,
        useArticulations: Option<&Bound<'_, PyAny>>,
        baseLevel: f64,
        clip: bool,
    ) -> PyResult<String> {
        let realized = self.getRealized(
            py,
            useDynamicContext,
            useVelocity,
            useArticulations,
            baseLevel,
            clip,
        )?;
        Ok(music21_rs::volume::rounded_str(realized))
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
    m.add_class::<Beam>()?;
    m.add_class::<Beams>()?;
    let tie_exception = py.get_type::<TieException>();
    tie_exception.setattr("__module__", "music21.tie")?;
    m.add("TieException", tie_exception)?;
    let lyric_exception = py.get_type::<LyricException>();
    lyric_exception.setattr("__module__", "music21.note")?;
    m.add("LyricException", lyric_exception)?;
    let volume_exception = py.get_type::<VolumeException>();
    volume_exception.setattr("__module__", "music21.volume")?;
    m.add("VolumeException", volume_exception)?;
    let beam_exception = py.get_type::<BeamException>();
    beam_exception.setattr("__module__", "music21.beam")?;
    m.add("BeamException", beam_exception)?;
    Ok(())
}
