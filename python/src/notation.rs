//! music21's `tie.Tie`, `note.Lyric`, `volume.Volume`, its beams and the
//! colour a note is written in, over `music21-rs`.

#![allow(non_snake_case)]

use pyo3::exceptions::PyIndexError;
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

/// The names the `notation` facade replaces in `music21.beam`.
pub const BEAM_NAMES: &[&str] = &["Beam", "Beams", "BeamException"];

pyo3::create_exception!(music21_rs_facade, TieException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, LyricException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, VolumeException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, BeamException, crate::Music21Exception);

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
#[pyclass(name = "Tie", module = "music21.tie", subclass, skip_from_py_object)]
pub struct Tie {
    pub(crate) inner: RsTie,
    /// music21's `id`, which its MusicXML exporter writes out. It is an
    /// identifier and not a musical fact, so nothing here works it out — it
    /// starts as the object's own address, as music21's does.
    identifier: Option<Py<PyAny>>,
}

impl Clone for Tie {
    /// A copy of a tie says the same thing and goes by no identifier: the
    /// one the original went by named that one.
    fn clone(&self) -> Self {
        Self::wrap(self.inner.clone())
    }
}

impl Tie {
    pub(crate) fn wrap(inner: RsTie) -> Self {
        Self {
            inner,
            identifier: None,
        }
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
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec!["Tie", "ProtoM21Object", "SlottedObjectMixin", "object"]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> =
            vec!["Tie", "ProtoM21Object", "SlottedObjectMixin", "object"];
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
        let Some(inner) = crate::unpickled::<_, RsTie>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

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

    /// music21's `id`: an identifier its MusicXML exporter writes out. It
    /// starts as the object's own address, as music21's does.
    #[getter]
    fn get_id(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(identifier) = &slf.borrow().identifier {
            return Ok(identifier.clone_ref(py));
        }
        let address = (slf.as_ptr() as usize)
            .into_pyobject(py)?
            .into_any()
            .unbind();
        slf.borrow_mut().identifier = Some(address.clone_ref(py));
        Ok(address)
    }

    #[setter]
    fn set_id(&mut self, value: &Bound<'_, PyAny>) {
        self.identifier = Some(value.clone().unbind());
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

/// music21's `note.Lyric`.
#[pyclass(name = "Lyric", module = "music21.note", subclass, skip_from_py_object)]
pub struct Lyric {
    pub(crate) inner: RsLyric,
    /// The syllables an elided lyric is made of, as the Python list a
    /// caller handed over.
    ///
    /// It is the list object itself, not a copy of what was in it, because
    /// music21's own MusicXML reader assigns an empty list and then appends
    /// to it; and the syllables are objects rather than values because
    /// elisions are edited after the fact — the madrigal example sets the
    /// elision character and then the syllabic on a component already inside
    /// the composite, and both have to show through.
    components: Option<Py<PyList>>,
    /// music21's `style`, once something has asked for one.
    style: Option<Py<PyAny>>,
}

impl Lyric {
    pub(crate) fn wrap(inner: RsLyric) -> Self {
        Self {
            inner,
            components: None,
            style: None,
        }
    }

    /// This lyric as a value, with whatever its component objects say now
    /// written into it, so the crate answers every question about it.
    pub(crate) fn synced(&self, py: Python<'_>) -> RsLyric {
        let Some(components) = self.components.as_ref().map(|list| list.bind(py)) else {
            return self.inner.clone();
        };
        let syllables: Vec<RsLyric> = components
            .iter()
            .filter_map(|component| component.extract::<PyRef<'_, Lyric>>().ok())
            .map(|component| component.synced(py))
            .collect();
        if syllables.is_empty() {
            return self.inner.clone();
        }
        let mut lyric = self.inner.clone();
        lyric.set_components(syllables);
        lyric
    }
}

impl Clone for Lyric {
    /// A copy takes copies of its syllables, as a deepcopy of music21's does.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner.clone(),
            style: copied_style(py, self.style.as_ref()),
            components: self.components.as_ref().and_then(|list| {
                let copies: Vec<Py<Lyric>> = list
                    .bind(py)
                    .iter()
                    .filter_map(|component| component.extract::<PyRef<'_, Lyric>>().ok())
                    .filter_map(|component| Py::new(py, component.clone()).ok())
                    .collect();
                Some(PyList::new(py, copies).ok()?.unbind())
            }),
        })
    }
}

#[pymethods]
impl Lyric {
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec![
            "Lyric",
            "ProtoM21Object",
            "StyleMixin",
            "SlottedObjectMixin",
            "object",
        ]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> = vec![
            "Lyric",
            "ProtoM21Object",
            "StyleMixin",
            "SlottedObjectMixin",
            "object",
        ];
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
        let Some(inner) = crate::unpickled::<_, RsLyric>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

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
            // Nothing sung to it yet, which is not the same as a whole
            // word with no letters in it.
            None => RsLyric::unsung(),
        };
        inner.set_number(number);
        if let Some(syllabic) = syllabic {
            inner.set_syllabic(Syllabic::from_name(syllabic).map_err(lyric_error)?);
        }
        inner.set_identifier(identifier);
        Ok(Self::wrap(inner))
    }

    /// music21's `style`: the object saying how this is drawn, made on
    /// first asking and the same one after that. It is music21's own — the
    /// page is not something this crate models.
    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let style = new_style(slf.as_any(), None)?;
        slf.borrow_mut().style = Some(style.clone_ref(py));
        Ok(style)
    }

    #[setter]
    fn set_style(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().style = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `hasStyleInformation`: whether a style object has been made
    /// for this yet, which is what its own code asks before making one.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    #[getter]
    fn get_text(&self, py: Python<'_>) -> Option<String> {
        self.synced(py).explicit_text()
    }

    /// Setting the text of an elided lyric makes it an ordinary one, since
    /// the syllables it was made of are what the text used to come from.
    /// Setting it to nothing at all makes it a lyric nobody has sung to.
    #[setter]
    fn set_text(&mut self, value: Option<&str>) {
        self.components = None;
        match value {
            Some(value) => self.inner.set_text(value),
            None => self.inner.clear_text(),
        }
    }

    #[getter]
    fn get_rawText(&self, py: Python<'_>) -> String {
        self.synced(py).raw_text()
    }

    #[setter]
    fn set_rawText(&mut self, value: &str) {
        self.components = None;
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
        self.inner.set_number(number);
        Ok(())
    }

    /// music21's `syllabic`, which is nothing at all until something says
    /// where the syllable falls.
    #[getter]
    fn get_syllabic(&self, py: Python<'_>) -> Option<&'static str> {
        self.synced(py)
            .explicit_syllabic()
            .map(music21_rs::Syllabic::as_str)
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
    ///
    /// A lyric read from a value already knows its syllables, and music21's
    /// own writer walks them off this attribute, so they are made into
    /// objects the first time something asks.
    #[getter]
    fn get_components(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyList>>> {
        if let Some(components) = &self.components {
            return Ok(Some(components.clone_ref(py)));
        }
        if self.inner.components().is_empty() {
            return Ok(None);
        }
        let syllables = self
            .inner
            .components()
            .iter()
            .map(|syllable| {
                crate::installed_new(py, "music21.note", "Lyric", Self::wrap(syllable.clone()))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let list = PyList::new(py, syllables)?.unbind();
        self.components = Some(list.clone_ref(py));
        Ok(Some(list))
    }

    /// The list assigned is kept as it stands, not copied: music21's own
    /// MusicXML reader assigns an empty list and then appends each syllable
    /// to it through this attribute.
    #[setter]
    fn set_components(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.components = None;
            self.inner.set_components(Vec::new());
            return Ok(());
        };
        let list = match value.cast::<PyList>() {
            Ok(list) => list.clone().unbind(),
            Err(_) => PyList::new(py, value.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.unbind(),
        };
        self.components = Some(list);
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
        self.inner.set_text_and_syllabic(rawText, applyRaw);
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
        // music21 names only what the lyric actually says: a lyric nobody
        // has sung anything to is `<music21.note.Lyric number=1>`.
        let lyric = self.synced(py);
        let mut said = format!("number={}", lyric.number());
        if let Some(identifier) = lyric.explicit_identifier() {
            said.push_str(&format!(" identifier='{identifier}'"));
        }
        if let Some(syllabic) = lyric.explicit_syllabic() {
            said.push_str(&format!(" syllabic={}", syllabic.as_str()));
        }
        if !lyric.text().is_empty() {
            said.push_str(&format!(" text='{}'", lyric.text()));
        }
        format!("<music21.note.Lyric {said}>")
    }

    fn __str__(&self, py: Python<'_>) -> String {
        self.synced(py).text()
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

/// music21's `beam.Beam`: one beam at one level.
#[pyclass(name = "Beam", module = "music21.beam", subclass, skip_from_py_object)]
pub struct Beam {
    pub(crate) inner: RsBeam,
    /// music21's `id`, which its MusicXML exporter reads. It is an
    /// identifier and not a musical fact, so nothing here works it out — it
    /// starts as the object's own address, as music21's does, and whatever
    /// a caller writes stands.
    identifier: Option<Py<PyAny>>,
    /// music21's `independentAngle`: the angle this one beam is drawn at,
    /// where it is not the angle the rest of the beam group takes. It is
    /// the page, which the crate does not model, so it is kept as given.
    independent_angle: Option<Py<PyAny>>,
    /// music21's `style`, once something has asked for one: the object
    /// saying how this is drawn. It is music21's own object — the page is
    /// not something this crate models — and its mere existence is what
    /// `hasStyleInformation` answers, as music21's does.
    style: Option<Py<PyAny>>,
    /// A beam type music21 has no name for.
    ///
    /// Its own `type` is a plain attribute that takes anything; a beam
    /// written as `crazy` is refused by its MusicXML exporter and not by the
    /// beam, and its own doctest for that refusal writes one.
    unnamed_type: Option<String>,
}

impl Clone for Beam {
    /// A copy of a beam is a loose one: it says the same thing about the
    /// note and is drawn the same way, and carries neither the identifier
    /// the original went by nor its angle.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner,
            identifier: None,
            independent_angle: None,
            style: copied_style(py, self.style.as_ref()),
            unnamed_type: self.unnamed_type.clone(),
        })
    }
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
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec![
            "Beam",
            "ProtoM21Object",
            "EqualSlottedObjectMixin",
            "StyleMixin",
            "SlottedObjectMixin",
            "object",
        ]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> = vec![
            "Beam",
            "ProtoM21Object",
            "EqualSlottedObjectMixin",
            "StyleMixin",
            "SlottedObjectMixin",
            "object",
        ];
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
        let Some(inner) = crate::unpickled::<_, RsBeam>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

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
        Ok(Self {
            inner,
            identifier: None,
            independent_angle: None,
            style: None,
            unnamed_type: None,
        })
    }

    #[getter]
    fn get_type(&self) -> Option<String> {
        if let Some(unnamed) = &self.unnamed_type {
            return Some(unnamed.clone());
        }
        self.inner.beam_type().map(|kind| kind.as_str().to_string())
    }

    /// music21's `type` is a plain attribute and takes whatever it is given:
    /// a beam written as something no notation knows is refused by its
    /// MusicXML exporter, not by the beam.
    #[setter]
    fn set_type(&mut self, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        match beam_type_or_none(value) {
            Ok(beam_type) => {
                self.inner.set_beam_type(beam_type);
                self.unnamed_type = None;
            }
            Err(refusal) => {
                let Some(value) = value else {
                    return Err(refusal);
                };
                self.unnamed_type = Some(value.extract::<String>().map_err(|_| refusal)?);
            }
        }
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

    /// music21's `style`: the object saying how this is drawn, made on
    /// first asking and the same one after that. It is music21's own — the
    /// page is not something this crate models.
    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let style = new_style(slf.as_any(), None)?;
        slf.borrow_mut().style = Some(style.clone_ref(py));
        Ok(style)
    }

    #[setter]
    fn set_style(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        slf.borrow_mut().style = Some(value.clone().unbind());
        Ok(())
    }

    /// music21's `hasStyleInformation`: whether a style object has been made
    /// for this yet, which is what its own code asks before making one.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    /// music21's `id`: an identifier its MusicXML exporter writes out. It
    /// starts as the object's own address, as music21's does.
    #[getter]
    fn get_id(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(identifier) = &slf.borrow().identifier {
            return Ok(identifier.clone_ref(py));
        }
        let address = (slf.as_ptr() as usize)
            .into_pyobject(py)?
            .into_any()
            .unbind();
        slf.borrow_mut().identifier = Some(address.clone_ref(py));
        Ok(address)
    }

    #[setter]
    fn set_id(&mut self, value: &Bound<'_, PyAny>) {
        self.identifier = Some(value.clone().unbind());
    }

    /// music21's `independentAngle`: the angle this one beam is drawn at.
    /// The page is not modelled here, so what a caller writes is kept and
    /// handed back.
    #[getter]
    fn get_independentAngle(&self, py: Python<'_>) -> Py<PyAny> {
        match &self.independent_angle {
            Some(angle) => angle.clone_ref(py),
            None => py.None(),
        }
    }

    #[setter]
    fn set_independentAngle(&mut self, value: &Bound<'_, PyAny>) {
        self.independent_angle = (!value.is_none()).then(|| value.clone().unbind());
    }

    fn __repr__(&self) -> String {
        format!("<music21.beam.Beam {}>", self.inner)
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

/// music21's `beam.Beams`: the beams of one note, one for each level it is
/// beamed at.
#[pyclass(name = "Beams", module = "music21.beam", subclass, skip_from_py_object)]
pub struct Beams {
    pub(crate) inner: RsBeams,
    /// music21's `beamsList`, as the list object itself.
    ///
    /// Its own MusicXML reader appends each beam to what `beams.beamsList`
    /// hands back, so a getter that built a fresh list every time dropped
    /// every beam the score wrote. Once the list exists it is what the beams
    /// are, and `settle` writes it into the value.
    beams: Option<Py<PyList>>,
}

impl Beams {
    pub(crate) fn wrap(inner: RsBeams) -> Self {
        Self { inner, beams: None }
    }

    /// Folds the beam objects into the value, so every question about the
    /// beams is answered from what Python has actually got.
    fn settle(&mut self, py: Python<'_>) {
        let Some(beams) = &self.beams else {
            return;
        };
        let held: Vec<RsBeam> = beams
            .bind(py)
            .iter()
            .filter_map(|beam| beam.extract::<PyRef<'_, Beam>>().ok())
            .map(|beam| beam.inner)
            .collect();
        self.inner.set_beams(held);
    }

    /// The value, with whatever Python has done to the beam objects folded
    /// in: what the note carrying them is really beamed as.
    pub(crate) fn settled_value(&mut self, py: Python<'_>) -> RsBeams {
        self.settle(py);
        self.inner.clone()
    }

    /// The same, and then letting the objects go so the next reader builds
    /// them again from the answer.
    fn settled(&mut self, py: Python<'_>) {
        self.settle(py);
        self.beams = None;
    }

    /// Writes these beams back into whatever carries them.
    fn write_back(&mut self, py: Python<'_>) -> PyResult<()> {
        self.settle(py);
        Ok(())
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
            Some(beams) => list.append(crate::installed_new(
                py,
                "music21.beam",
                "Beams",
                Beams::wrap(beams),
            )?)?,
            None => list.append(py.None())?,
        }
    }
    Ok(list)
}

#[pymethods]
impl Beams {
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec![
            "Beams",
            "ProtoM21Object",
            "EqualSlottedObjectMixin",
            "SlottedObjectMixin",
            "object",
        ]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> = vec![
            "Beams",
            "ProtoM21Object",
            "EqualSlottedObjectMixin",
            "SlottedObjectMixin",
            "object",
        ];
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
        let Some(inner) = crate::unpickled::<_, RsBeams>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self::wrap(RsBeams::new())
    }

    /// music21's `beamsList`: the beams themselves, in level order.
    #[getter]
    fn get_beamsList<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        if let Some(beams) = &slf.borrow().beams {
            return Ok(beams.bind(py).clone());
        }
        let held = slf.borrow().inner.beams().to_vec();
        let list = PyList::empty(py);
        for beam in held {
            list.append(crate::installed_new(
                py,
                "music21.beam",
                "Beam",
                Beam {
                    inner: beam,
                    identifier: None,
                    independent_angle: None,
                    style: None,
                    unnamed_type: None,
                },
            )?)?;
        }
        slf.borrow_mut().beams = Some(list.clone().unbind());
        Ok(list)
    }

    /// Setting it replaces them outright, which is how music21 says a note
    /// carries no beam at all.
    #[setter]
    fn set_beamsList(&mut self, py: Python<'_>, value: Vec<Py<Beam>>) -> PyResult<()> {
        self.inner
            .set_beams(value.iter().map(|beam| beam.borrow(py).inner).collect());
        // The objects given are kept, as music21 keeps them.
        self.beams = Some(PyList::new(py, value)?.unbind());
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
        self.settled(py);
        if let Some(value) = r#type
            && let Ok(beam) = value.extract::<PyRef<'_, Beam>>()
        {
            self.inner.beams_mut().push(beam.inner);
            drop(beam);
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
        self.settled(py);
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
        self.settled(py);
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
        self.settled(py);
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
    fn getByNumber(&mut self, py: Python<'_>, number: u32) -> PyResult<Beam> {
        self.settle(py);
        self.inner
            .by_number(number)
            .map(|beam| Beam {
                inner: *beam,
                identifier: None,
                independent_angle: None,
                style: None,
                unnamed_type: None,
            })
            .ok_or_else(|| {
                PyIndexError::new_err(format!("beam number {number} cannot be accessed"))
            })
    }

    /// music21's `getTypeByNumber`, which writes a stub's direction into the
    /// name with a hyphen.
    fn getTypeByNumber(&mut self, py: Python<'_>, number: u32) -> PyResult<Option<String>> {
        let beam = self.getByNumber(py, number)?;
        let Some(name) = beam.get_type() else {
            return Ok(None);
        };
        Ok(Some(match beam.get_direction() {
            Some(direction) => format!("{name}-{direction}"),
            None => name.to_string(),
        }))
    }

    /// music21's `getTypes`.
    fn getTypes(&mut self, py: Python<'_>) -> Vec<Option<&'static str>> {
        self.settle(py);
        self.inner
            .types()
            .into_iter()
            .map(|beam_type| beam_type.map(RsBeamType::as_str))
            .collect()
    }

    /// music21's `getNumbers`.
    fn getNumbers(&mut self, py: Python<'_>) -> Vec<Option<u32>> {
        self.settle(py);
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

    fn __len__(&mut self, py: Python<'_>) -> usize {
        self.settle(py);
        self.inner.len()
    }

    fn __iter__(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Ok(Self::get_beamsList(slf)?.try_iter()?.unbind().into_any())
    }

    fn __repr__(&mut self, py: Python<'_>) -> String {
        self.settle(py);
        // music21 writes the class alone when there is nothing to say about
        // it, so a note with no beams is `<music21.beam.Beams>`.
        let written = self.inner.to_string();
        if written.is_empty() {
            return "<music21.beam.Beams>".to_string();
        }
        format!("<music21.beam.Beams {written}>")
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

/// music21's `volume.Volume`.
#[pyclass(
    name = "Volume",
    module = "music21.volume",
    subclass,
    skip_from_py_object
)]
pub struct Volume {
    pub(crate) inner: RsVolume,
    /// The note or chord this volume belongs to. music21's `_setVolume`
    /// reads the slot to decide whether the volume is already spoken for —
    /// a volume that has a client is copied rather than stolen — so without
    /// it music21's own `NotRest` cannot take one of ours at all.
    client: Option<Py<PyAny>>,
    /// music21's `_cachedRealized`: the last answer `getRealized` gave.
    ///
    /// Realizing a volume means searching the stream for the dynamic in
    /// force, which is slow, so music21 keeps the answer and hands it back
    /// until something realizes it again. Its own `realizeVolume` walks a
    /// score realizing every note once so that the cache is what the score
    /// says.
    cached: Option<f64>,
}

impl Volume {
    pub(crate) fn wrap(inner: RsVolume) -> Self {
        Self {
            inner,
            client: None,
            cached: None,
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
    /// Whether something has already claimed this volume as its own.
    pub(crate) fn is_claimed(&self) -> bool {
        self.client.is_some()
    }

    pub(crate) fn owned_by(inner: RsVolume, client: Py<PyAny>) -> Self {
        Self {
            inner,
            client: Some(client),
            cached: None,
        }
    }
}

impl Clone for Volume {
    /// A copy of a volume belongs to whoever the original did: music21's
    /// `__deepcopy__` leaves the client out of the copying and then writes
    /// the same one in, so a copy is still the volume of that note.
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            inner: self.inner.clone(),
            client: self.client.as_ref().map(|client| client.clone_ref(py)),
            cached: self.cached,
        })
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
    /// music21's `classes`: what this is, and everything it is a kind of.
    /// Its own code reads this to decide what it is looking at.
    #[getter]
    fn classes(&self) -> Vec<&'static str> {
        vec!["Volume", "ProtoM21Object", "SlottedObjectMixin", "object"]
    }

    #[getter]
    fn classSet(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let names: Vec<&'static str> =
            vec!["Volume", "ProtoM21Object", "SlottedObjectMixin", "object"];
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
        let Some(inner) = crate::unpickled::<_, RsVolume>(slf, state)? else {
            return Ok(());
        };
        slf.borrow_mut().inner = inner;
        Ok(())
    }

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
    pub(crate) fn set_client(&mut self, value: Option<&Bound<'_, PyAny>>) {
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

    /// music21's `cachedRealized`: the last answer, or a fresh one where
    /// nothing has realized this volume yet.
    #[getter]
    fn cachedRealized(&mut self, py: Python<'_>) -> PyResult<f64> {
        if let Some(cached) = self.cached {
            return Ok(cached);
        }
        self.getRealized(py, None, true, None, 0.5, true)
    }

    #[getter]
    fn cachedRealizedStr(&mut self, py: Python<'_>) -> PyResult<String> {
        Ok(music21_rs::volume::rounded_str(self.cachedRealized(py)?))
    }

    /// music21's `mergeAttributes`: everything the other volume says except
    /// whose volume it is, copied rather than shared.
    fn mergeAttributes(&mut self, other: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let Some(other) = other.filter(|value| !value.is_none()) else {
            return Ok(());
        };
        let other = other.extract::<PyRef<'_, Self>>()?;
        self.inner.merge_attributes(&other.inner);
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
        &mut self,
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
        let realized = volume.realized_with(
            self.dynamic_scalar(py, useDynamicContext)?,
            self.articulation_shift(py, useArticulations)?,
            baseLevel,
            clip,
        );
        // music21 keeps every answer, which is what `realizeVolume` walks a
        // score to fill in.
        self.cached = Some(realized);
        Ok(realized)
    }

    #[pyo3(signature = (
        useDynamicContext = None,
        useVelocity = true,
        useArticulations = None,
        baseLevel = 0.5,
        clip = true,
    ))]
    fn getRealizedStr(
        &mut self,
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

    /// music21 restores hashing on identity where it defines equality, so
    /// that a set or a dictionary can hold one of these however it compares.
    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(&self) -> String {
        format!("<music21.volume.Volume {}>", self.inner)
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

/// The music21 style object something is drawn with, made the way music21
/// makes it.
///
/// This crate models the colour and nothing else of a style; music21's own
/// `Style` carries the page — where the object sits, how large it is drawn,
/// what encloses it, what an editor wrote about it — and a class of ours
/// standing in its place would take all of that away. So the object handed
/// back is music21's, of whatever `_styleClass` the thing being drawn asks
/// for, and the colour the crate does model is written into it and read
/// back out of it.
pub(crate) fn new_style<'py>(
    owner: &Bound<'py, PyAny>,
    colour: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let py = owner.py();
    let module = py.import("music21.style")?;
    let class = match owner.get_type().getattr("_styleClass") {
        Ok(class) => class,
        Err(_) => module.getattr("Style")?,
    };
    let style = class.call0()?;
    if let Some(colour) = colour {
        style.setattr("color", colour)?;
    }
    Ok(style.unbind())
}

/// A copy of a style object, as music21's own deepcopy makes one: a copy of
/// a note is drawn the same way the note was.
pub(crate) fn copied_style(py: Python<'_>, style: Option<&Py<PyAny>>) -> Option<Py<PyAny>> {
    let style = style?;
    Some(
        py.import("copy")
            .ok()?
            .getattr("deepcopy")
            .ok()?
            .call1((style,))
            .ok()?
            .unbind(),
    )
}

/// The colour a style object is carrying, if it is carrying one.
pub(crate) fn style_colour(py: Python<'_>, style: Option<&Py<PyAny>>) -> Option<String> {
    style?.bind(py).getattr("color").ok()?.extract().ok()
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<Tie>()?;
    m.add_class::<Lyric>()?;
    m.add_class::<Volume>()?;
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
