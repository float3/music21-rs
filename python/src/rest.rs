//! music21's `note.Rest`: a length with nothing sounding in it.
//!
//! Everything a rest has beyond its length is music21's `GeneralNote` — the
//! words under it, the tie, the ornaments and marks, how it is drawn — and
//! the facade answers all of it, since an installed class blocks whatever
//! the facade leaves out. The length and the words are the crate's `Rest`;
//! the rest are Python objects held as music21 holds them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::{Duration as RsDuration, Rest as RsRest};

use crate::duration::{
    Duration, adopt_duration, copied_duration, duration_from_any, duration_value_of, op_frac,
    told_sites,
};
use crate::notation::{Lyric, Tie, tie_from_any};
use crate::note::{augment_or_diminish_note, copied_list, grace_note, note_error, same_kinds};

/// music21's `note.Rest`.
#[pyclass(name = "Rest", module = "music21.note", subclass, skip_from_py_object)]
pub struct Rest {
    pub(crate) inner: RsRest,
    /// The `Duration` object handed out from `.duration`, once asked for:
    /// the same object every time, so `r.duration.type = 'half'` lengthens
    /// the rest, and one of music21's own kinds of duration if that is what
    /// was given.
    duration: Option<Py<PyAny>>,
    /// The verses as the list object itself, since music21's MusicXML reader
    /// appends to the list it is handed.
    lyrics: Option<Py<PyList>>,
    /// The tie as the object itself, which music21's note splitting writes
    /// through.
    tie: Option<Py<Tie>>,
    expressions: Option<Py<PyList>>,
    articulations: Option<Py<PyList>>,
    /// music21's `Style` object, made on first asking; the page is not
    /// something this crate models.
    style: Option<Py<PyAny>>,
    /// music21's `stepShift`: how many lines or spaces the rest is drawn
    /// above or below where it would be. Layout, so the facade's alone.
    step_shift: i32,
    /// music21's `fullMeasure`: `'auto'`, `True`, `False` or `'always'`,
    /// kept as the object it was given.
    full_measure: Py<PyAny>,
}

impl Rest {
    pub(crate) fn wrap(py: Python<'_>, inner: RsRest) -> Self {
        Self {
            inner,
            duration: None,
            lyrics: None,
            tie: None,
            expressions: None,
            articulations: None,
            style: None,
            step_shift: 0,
            full_measure: "auto"
                .into_pyobject(py)
                .map_or_else(|_| py.None(), |value| value.into_any().unbind()),
        }
    }

    /// The duration this rest has: the object a caller may have edited, and
    /// the value otherwise.
    fn duration_value(&self, py: Python<'_>) -> RsDuration {
        self.duration
            .as_ref()
            .and_then(|object| duration_value_of(py, object))
            .unwrap_or_else(|| self.inner.duration().clone())
    }

    fn duration_object(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(object) = &self.duration {
            return Ok(object.clone_ref(py));
        }
        let created = crate::installed_new(
            py,
            "music21.duration",
            "Duration",
            Duration::wrap(self.inner.duration().clone()),
        )?
        .into_any();
        self.duration = Some(created.clone_ref(py));
        Ok(created)
    }

    fn attach_duration(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let inner = duration_from_any(value)?;
        let object = if value.hasattr("quarterLength")? {
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
        self.duration = Some(object.clone_ref(py));
        Ok(object)
    }

    /// Folds the verse objects into the value and lets them go, so the
    /// crate's verse rules work on what Python actually holds.
    fn settle_lyrics(&mut self, py: Python<'_>) {
        let Some(lyrics) = self.lyrics.take() else {
            return;
        };
        let verses: Vec<music21_rs_crate::notation::Lyric> = lyrics
            .bind(py)
            .iter()
            .filter_map(|verse| verse.extract::<PyRef<'_, Lyric>>().ok())
            .map(|verse| verse.synced(py))
            .collect();
        let held = self.inner.lyrics_mut();
        held.clear();
        held.extend(verses);
    }

    /// The rest with whatever its objects now say written into it.
    pub(crate) fn synced(&self, py: Python<'_>) -> RsRest {
        let mut rest = self.inner.clone();
        rest.set_duration(self.duration_value(py));
        if let Some(lyrics) = &self.lyrics {
            let verses = rest.lyrics_mut();
            verses.clear();
            verses.extend(
                lyrics
                    .bind(py)
                    .iter()
                    .filter_map(|verse| verse.extract::<PyRef<'_, Lyric>>().ok())
                    .map(|verse| verse.synced(py)),
            );
        }
        rest
    }

    fn copied(&self, py: Python<'_>) -> PyResult<Self> {
        let mut copy = Self::wrap(py, self.synced(py));
        copy.expressions = copied_list(py, self.expressions.as_ref())?;
        copy.articulations = copied_list(py, self.articulations.as_ref())?;
        copy.style = crate::notation::copied_style(py, self.style.as_ref());
        copy.step_shift = self.step_shift;
        copy.full_measure = self.full_measure.clone_ref(py);
        if let Some(duration) = &self.duration {
            copy.duration = Some(copied_duration(py, duration)?);
        }
        if let Some(tie) = &self.tie {
            let inner = tie.borrow(py).inner.clone();
            copy.tie = Some(crate::installed_new(
                py,
                "music21.tie",
                "Tie",
                Tie::wrap(inner),
            )?);
        }
        Ok(copy)
    }

    /// music21's `_reprInternal`: the length's name, hyphenated, or the
    /// quarter length where the name would be too long to read.
    fn repr_internal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let duration = Self::get_duration(slf)?;
        let duration = duration.bind(slf.py());
        let name: String = duration.getattr("fullName")?.extract()?;
        let name = name.to_lowercase();
        if name.len() < 15 {
            return Ok(name.replace(' ', "-"));
        }
        // Written as Python writes the length, which is a fraction where
        // music21 keeps one: a triplet half rest is `4/3ql`.
        let length = duration.getattr("quarterLength")?;
        let whole = length
            .extract::<f64>()
            .ok()
            .filter(|value| *value == value.trunc());
        Ok(match whole {
            Some(value) => format!("{}ql", value as i64),
            None => format!("{}ql", length.str()?),
        })
    }
}

/// Reads a rest argument: one of these, or a length.
#[allow(dead_code)]
pub(crate) fn rest_from_any(value: &Bound<'_, PyAny>) -> PyResult<RsRest> {
    if let Ok(rest) = value.extract::<PyRef<'_, Rest>>() {
        return Ok(rest.synced(value.py()));
    }
    Ok(RsRest::new(duration_from_any(value)?))
}

#[pymethods]
impl Rest {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.duration)?;
        visit.call(&self.lyrics)?;
        visit.call(&self.tie)?;
        visit.call(&self.expressions)?;
        visit.call(&self.articulations)?;
        visit.call(&self.style)?;
        visit.call(&self.full_measure)?;
        Ok(())
    }

    fn __clear__(&mut self) {
        self.duration = None;
        self.lyrics = None;
        self.tie = None;
        self.expressions = None;
        self.articulations = None;
        self.style = None;
    }

    /// music21's `Rest(length=None, *, stepShift=0, fullMeasure='auto',
    /// **keywords)`: a length written as a note value's name is its type,
    /// and anything else its quarter length.
    #[new]
    #[pyo3(signature = (length = None, *, stepShift = 0, fullMeasure = None, **keywords))]
    fn new(
        py: Python<'_>,
        length: Option<&Bound<'_, PyAny>>,
        stepShift: i32,
        fullMeasure: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let keywords = match keywords {
            Some(keywords) => keywords.copy()?,
            None => PyDict::new(py),
        };
        if let Some(length) = length.filter(|value| !value.is_none()) {
            if length.extract::<String>().is_ok() && !keywords.contains("type")? {
                keywords.set_item("type", length)?;
            } else if !keywords.contains("quarterLength")? {
                keywords.set_item("quarterLength", length)?;
            }
        }
        let mut rest = Self::wrap(py, RsRest::default());
        if Duration::keywords_say_duration(Some(&keywords))? {
            let duration = match keywords.get_item("duration")? {
                Some(value) => value,
                None => crate::installed_new(
                    py,
                    "music21.duration",
                    "Duration",
                    Duration::new(None, Some(&keywords))?,
                )?
                .into_bound(py)
                .into_any(),
            };
            rest.attach_duration(py, &duration)?;
        }
        if let Some(lyric) = keywords.get_item("lyric")?.filter(|value| !value.is_none()) {
            rest.inner.set_lyric(Some(&lyric.str()?.to_string()));
        }
        rest.step_shift = stepShift;
        if let Some(full) = fullMeasure.filter(|value| !value.is_none()) {
            rest.full_measure = full.clone().unbind();
        }
        Ok(rest)
    }

    /// A rest is written out as text and read back; what the crate does not
    /// model is frozen beside it.
    fn __reduce__(slf: &Bound<'_, Self>) -> PyResult<crate::Pickled> {
        let py = slf.py();
        let extra = PyDict::new(py);
        {
            let rest = slf.borrow();
            extra.set_item("expressions", rest.expressions.as_ref())?;
            extra.set_item("articulations", rest.articulations.as_ref())?;
            extra.set_item("duration", rest.duration.as_ref())?;
            extra.set_item("tie", rest.tie.as_ref())?;
            extra.set_item("style", rest.style.as_ref())?;
            extra.set_item("stepShift", rest.step_shift)?;
            extra.set_item("fullMeasure", rest.full_measure.bind(py))?;
        }
        crate::pickled_extra(slf, &slf.borrow().synced(py), Some(&extra))
    }

    fn __setstate__(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        state: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let (inner, extra) = crate::unpickled_extra::<_, RsRest>(slf, state)?;
        let Some(inner) = inner else {
            return Ok(());
        };
        *slf.borrow_mut() = Self::wrap(py, inner);
        if let Some(extra) = extra {
            let extra = extra.bind(py);
            let present = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
                Ok(extra.get_item(name).ok().filter(|value| !value.is_none()))
            };
            let mut rest = slf.borrow_mut();
            rest.expressions = present("expressions")?.and_then(|value| value.extract().ok());
            rest.articulations = present("articulations")?.and_then(|value| value.extract().ok());
            rest.duration = present("duration")?.map(Bound::unbind);
            rest.tie = present("tie")?.and_then(|value| value.extract().ok());
            rest.style = present("style")?.map(Bound::unbind);
            if let Some(shift) = present("stepShift")? {
                rest.step_shift = shift.extract()?;
            }
            if let Some(full) = present("fullMeasure")? {
                rest.full_measure = full.unbind();
            }
        }
        Ok(())
    }

    /// music21's `name`, which for a rest is always `'rest'`, so that every
    /// member of `.notesAndRests` has one.
    #[getter]
    fn name(&self) -> &'static str {
        "rest"
    }

    /// music21's `fullName`: the length's name and the word, `Dotted Quarter
    /// Rest`. Read off the duration object, which is where a grace length or
    /// a tuplet a caller wrote lives.
    #[getter]
    fn fullName(slf: &Bound<'_, Self>) -> PyResult<String> {
        let duration = Self::get_duration(slf)?;
        let name: String = duration.bind(slf.py()).getattr("fullName")?.extract()?;
        Ok(format!("{name} Rest"))
    }

    #[getter]
    fn get_duration(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let duration = slf.borrow_mut().duration_object(py)?;
        adopt_duration(py, &duration, slf.as_any())?;
        Ok(duration)
    }

    #[setter]
    fn set_duration(slf: &Bound<'_, Self>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let had_one = slf.borrow().duration.is_some();
        let duration = slf.borrow_mut().attach_duration(py, value)?;
        adopt_duration(py, &duration, slf.as_any())?;
        if had_one {
            told_sites(slf.as_any(), &duration.bind(py).getattr("quarterLength")?)?;
        }
        Ok(())
    }

    #[getter]
    fn get_quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        op_frac(py, self.duration_value(py).quarter_length())
    }

    #[setter]
    fn set_quarterLength(&mut self, py: Python<'_>, value: f64) -> PyResult<()> {
        let inner = RsDuration::new(value).map_err(note_error)?;
        self.inner.set_duration(inner.clone());
        match &self.duration {
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
        true
    }

    #[getter]
    fn isNote(&self) -> bool {
        false
    }

    #[getter]
    fn isChord(&self) -> bool {
        false
    }

    /// music21's `pitches`: a rest sounds none.
    #[getter]
    fn get_pitches<'py>(&self, py: Python<'py>) -> Bound<'py, PyTuple> {
        PyTuple::empty(py)
    }

    /// Assigning pitches to a rest does nothing, as it does in music21.
    #[setter]
    fn set_pitches(&self, _value: &Bound<'_, PyAny>) {}

    #[getter]
    fn get_stepShift(&self) -> i32 {
        self.step_shift
    }

    #[setter]
    fn set_stepShift(&mut self, value: i32) {
        self.step_shift = value;
    }

    #[getter]
    fn get_fullMeasure(&self, py: Python<'_>) -> Py<PyAny> {
        self.full_measure.clone_ref(py)
    }

    #[setter]
    fn set_fullMeasure(&mut self, value: &Bound<'_, PyAny>) {
        self.full_measure = value.clone().unbind();
    }

    #[getter]
    fn get_tie(slf: &Bound<'_, Self>) -> Option<Py<Tie>> {
        slf.borrow().tie.as_ref().map(|tie| tie.clone_ref(slf.py()))
    }

    #[setter]
    fn set_tie(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let tie = match value.filter(|value| !value.is_none()) {
            None => None,
            Some(value) => Some(match value.extract::<Py<Tie>>() {
                Ok(object) => object,
                Err(_) => {
                    crate::installed_new(py, "music21.tie", "Tie", Tie::wrap(tie_from_any(value)?))?
                }
            }),
        };
        slf.borrow_mut().tie = tie;
        Ok(())
    }

    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let style = crate::notation::new_style(slf.as_any(), None)?;
        slf.borrow_mut().style = Some(style.clone_ref(py));
        Ok(style)
    }

    #[setter]
    fn set_style(&mut self, value: &Bound<'_, PyAny>) {
        self.style = Some(value.clone().unbind());
    }

    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    /// The words under the rest, one `Lyric` per verse; the same list every
    /// time.
    #[getter]
    fn get_lyrics<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
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

    #[setter]
    fn set_lyrics(slf: &Bound<'_, Self>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let list = PyList::empty(py);
        if let Some(value) = value.filter(|value| !value.is_none()) {
            for item in value.try_iter()? {
                let item = item?;
                if item.extract::<PyRef<'_, Lyric>>().is_ok() {
                    list.append(item)?;
                    continue;
                }
                let verse = music21_rs_crate::notation::Lyric::new(item.extract::<String>()?);
                list.append(crate::installed_new(
                    py,
                    "music21.note",
                    "Lyric",
                    Lyric::wrap(verse),
                )?)?;
            }
        }
        let mut rest = slf.borrow_mut();
        rest.inner.lyrics_mut().clear();
        rest.lyrics = Some(list.unbind());
        Ok(())
    }

    #[getter]
    fn get_lyric(&self, py: Python<'_>) -> Option<String> {
        self.synced(py).lyric()
    }

    #[setter]
    fn set_lyric(&mut self, py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.settle_lyrics(py);
        let Some(value) = value.filter(|value| !value.is_none()) else {
            self.inner.set_lyric(None);
            return Ok(());
        };
        if let Ok(lyric) = value.extract::<PyRef<'_, Lyric>>() {
            self.inner.set_lyric(None);
            self.inner.lyrics_mut().push(lyric.inner.clone());
            return Ok(());
        }
        self.inner.set_lyric(Some(&value.str()?.to_string()));
        Ok(())
    }

    /// music21's `insertLyric`.
    #[pyo3(signature = (text, index = 0, *, applyRaw = false, identifier = None))]
    fn insertLyric(
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
        self.inner.insert_lyric(&text, index, applyRaw);
        if identifier.is_some()
            && let Some(lyric) = self.inner.lyrics_mut().get_mut(index)
        {
            lyric.set_identifier(identifier);
        }
        Ok(())
    }

    /// music21's `addLyric`.
    #[pyo3(signature = (text, lyricNumber = None, *, applyRaw = false, lyricIdentifier = None))]
    fn addLyric(
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
        self.inner.add_lyric(&text, lyricNumber, applyRaw);
        if lyricIdentifier.is_some()
            && let Some(lyric) = self.inner.lyrics_mut().last_mut()
        {
            lyric.set_identifier(lyricIdentifier);
        }
        Ok(())
    }

    /// music21's `expressions`: the ornaments written over the rest — a
    /// fermata, most often.
    #[getter]
    fn get_expressions(&mut self, py: Python<'_>) -> Py<PyList> {
        self.expressions
            .get_or_insert_with(|| PyList::empty(py).unbind())
            .clone_ref(py)
    }

    #[setter]
    fn set_expressions(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.expressions = Some(list_holding(py, value)?);
        Ok(())
    }

    /// music21's `articulations`.
    #[getter]
    fn get_articulations(&mut self, py: Python<'_>) -> Py<PyList> {
        self.articulations
            .get_or_insert_with(|| PyList::empty(py).unbind())
            .clone_ref(py)
    }

    #[setter]
    fn set_articulations(&mut self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.articulations = Some(list_holding(py, value)?);
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

    /// Two rests are equal when they last as long, are tied alike and carry
    /// the same kinds of ornament and mark; a rest is never equal to a note.
    fn __eq__(&self, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let Ok(other) = other.extract::<PyRef<'_, Rest>>() else {
            return Ok(false);
        };
        let tie = |rest: &Rest| rest.tie.as_ref().map(|tie| tie.borrow(py).inner.clone());
        if self.duration_value(py) != other.duration_value(py) || tie(self) != tie(&other) {
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

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize >> 4
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        Ok(format!(
            "<music21.note.{} {}>",
            slf.get_type().qualname()?,
            Self::repr_internal(slf)?
        ))
    }

    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
        _memo: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        crate::copy_as_same_type(slf, slf.borrow().copied(py)?)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::copy_as_same_type(slf, slf.borrow().copied(py)?)
    }
}

/// A Python list holding whatever the value iterates over.
fn list_holding(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PyList>> {
    let list = PyList::empty(py);
    if !value.is_none() {
        for item in value.try_iter()? {
            list.append(item?)?;
        }
    }
    Ok(list.unbind())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Rest>()?;
    Ok(())
}
