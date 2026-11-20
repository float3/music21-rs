//! music21's `metadata.RichMetadata` and `AmbitusShort`, over the crate's
//! `metadata::rich`.
//!
//! A `Metadata` that also holds thirteen values read off its score, which
//! music21's corpus cache keeps for every work so the corpus can be
//! searched by meter, key or range without reading a score. The score's
//! own metadata is the `Metadata` this extends; the thirteen values are
//! this class's own, and `rich_value` puts the two back together as the
//! crate's `RichMetadata`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::cell::RefCell;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21::common::numbertools::OffsetQL;
use music21::metadata::rich::{
    AmbitusShort, RICH_ATTRIBUTES, RichMetadata as RsRich, RichQuery, RichValue,
};

use crate::metadata::Metadata;

/// The module music21 defines both in.
const MODULE: &str = "music21.metadata";

/// The characters that make music21 read a string query as a regular
/// expression.
const PATTERN_CHARACTERS: [char; 7] = ['*', '.', '|', '+', '?', '{', '}'];

/// music21's `AmbitusShort`: a named tuple of the span in semitones, its
/// simple diatonic name and the lowest and highest pitch.
pub(crate) fn ambitus_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    static CLASS: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
    let class = CLASS.get_or_try_init(py, || {
        let kwargs = PyDict::new(py);
        kwargs.set_item("module", MODULE)?;
        py.import("collections")?
            .getattr("namedtuple")?
            .call(
                (
                    "AmbitusShort",
                    ["semitones", "diatonic", "pitchLowest", "pitchHighest"],
                ),
                Some(&kwargs),
            )
            .map(Bound::unbind)
    })?;
    Ok(class.bind(py).clone())
}

fn ambitus_object<'py>(py: Python<'py>, ambitus: &AmbitusShort) -> PyResult<Bound<'py, PyAny>> {
    ambitus_class(py)?.call1((
        ambitus.semitones,
        ambitus.diatonic.as_str(),
        ambitus.pitch_lowest.as_deref(),
        ambitus.pitch_highest.as_deref(),
    ))
}

fn ambitus_value(object: &Bound<'_, PyAny>) -> PyResult<Option<AmbitusShort>> {
    if object.is_none() {
        return Ok(None);
    }
    Ok(Some(AmbitusShort {
        semitones: object.getattr("semitones")?.extract()?,
        diatonic: object.getattr("diatonic")?.extract()?,
        pitch_lowest: object.getattr("pitchLowest")?.extract()?,
        pitch_highest: object.getattr("pitchHighest")?.extract()?,
    }))
}

/// A quarter length as music21 holds one: a float, or a `Fraction`.
pub(crate) fn length_object<'py>(py: Python<'py>, length: OffsetQL) -> PyResult<Bound<'py, PyAny>> {
    match length {
        OffsetQL::Float(value) => Ok(value.into_pyobject(py)?.into_any()),
        OffsetQL::Fraction {
            numerator,
            denominator,
        } => py
            .import("fractions")?
            .getattr("Fraction")?
            .call1((numerator, denominator)),
    }
}

fn length_value(object: &Bound<'_, PyAny>) -> PyResult<OffsetQL> {
    if let Ok(value) = object.extract::<f64>()
        && (!object.hasattr("denominator")?
            || object.getattr("denominator")?.extract::<i64>()? == 1)
    {
        return Ok(OffsetQL::Float(value));
    }
    Ok(OffsetQL::Fraction {
        numerator: object.getattr("numerator")?.extract()?,
        denominator: object.getattr("denominator")?.extract()?,
    })
}

/// A value read off a score, as music21 holds it.
pub(crate) fn rich_object<'py>(
    py: Python<'py>,
    value: &RichValue<'_>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        RichValue::None => py.None().into_bound(py),
        RichValue::Text(text) => PyString::new(py, text).into_any(),
        RichValue::Integer(value) => value.into_pyobject(py)?.into_any(),
        RichValue::Length(length) => length_object(py, *length)?,
        RichValue::Texts(texts) => PyList::new(py, texts.iter())?.into_any(),
        RichValue::Integers(values) => PyList::new(py, values.iter())?.into_any(),
        RichValue::Ambitus(ambitus) => ambitus_object(py, ambitus)?,
    })
}

/// A music21 query, read once and asked of many values: the crate's
/// [`RichQuery`] for each kind music21 tells apart. A function's error is
/// kept and raised after the search, as music21 would have raised it.
pub(crate) struct Query<'py> {
    kind: QueryKind<'py>,
    error: RefCell<Option<PyErr>>,
}

enum QueryKind<'py> {
    Text(String),
    Pattern(Bound<'py, PyAny>),
    Function(Bound<'py, PyAny>),
    Integer(i64),
    Sharps(i64),
}

impl<'py> Query<'py> {
    /// music21's reading of a query: a compiled pattern, a string holding
    /// any of `*.|+?{}` that compiles as one, anything callable, an int, a
    /// key signature, or text.
    pub(crate) fn read(query: &Bound<'py, PyAny>) -> PyResult<Self> {
        let py = query.py();
        let re = py.import("re")?;
        let kind = if query.is_instance(&re.getattr("Pattern")?)? {
            QueryKind::Pattern(query.getattr("search")?)
        } else if let Ok(text) = query.extract::<String>() {
            match compiled(query, &text)? {
                Some(search) => QueryKind::Pattern(search),
                None => QueryKind::Text(text),
            }
        } else if query.is_callable() {
            QueryKind::Function(query.clone())
        } else if query.is_instance_of::<pyo3::types::PyInt>() {
            QueryKind::Integer(query.extract()?)
        } else if query.hasattr("sharps")? {
            QueryKind::Sharps(query.getattr("sharps")?.extract()?)
        } else {
            QueryKind::Text(query.str()?.to_string())
        };
        Ok(Self {
            kind,
            error: RefCell::new(None),
        })
    }

    /// Runs `search` with the crate's query, raising what a function
    /// raised.
    pub(crate) fn run<T>(&self, search: impl FnOnce(&RichQuery<'_>) -> T) -> PyResult<T> {
        let ask = |function: &Bound<'py, PyAny>, argument: Bound<'py, PyAny>| -> bool {
            match function
                .call1((argument,))
                .and_then(|answer| answer.is_truthy())
            {
                Ok(answer) => answer,
                Err(error) => {
                    self.error.borrow_mut().get_or_insert(error);
                    false
                }
            }
        };
        let found = match &self.kind {
            QueryKind::Text(text) => search(&RichQuery::Text(text)),
            QueryKind::Integer(value) => search(&RichQuery::Integer(*value as _)),
            QueryKind::Sharps(value) => search(&RichQuery::Sharps(*value as _)),
            QueryKind::Pattern(pattern) => {
                let matches =
                    |text: &str| ask(pattern, PyString::new(pattern.py(), text).into_any());
                search(&RichQuery::Pattern(&matches))
            }
            QueryKind::Function(function) => {
                let matches = |value: &RichValue<'_>| match rich_object(function.py(), value) {
                    Ok(argument) => ask(function, argument),
                    Err(error) => {
                        self.error.borrow_mut().get_or_insert(error);
                        false
                    }
                };
                search(&RichQuery::Function(&matches))
            }
        };
        match self.error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(found),
        }
    }
}

/// A string query compiled as music21 compiles one: case ignored, and only
/// where it holds one of `*.|+?{}` and is a valid pattern.
fn compiled<'py>(query: &Bound<'py, PyAny>, text: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    if !text.contains(PATTERN_CHARACTERS) {
        return Ok(None);
    }
    let re = query.py().import("re")?;
    match re.call_method1("compile", (text, re.getattr("IGNORECASE")?)) {
        Ok(pattern) => Ok(Some(pattern.getattr("search")?)),
        Err(_) => Ok(None),
    }
}

/// music21's `RichMetadata`: a `Metadata` with what was read off its
/// score.
#[pyclass(name = "RichMetadata", module = "music21.metadata", extends = Metadata, subclass)]
pub struct RichMetadata {
    /// The thirteen values; its `metadata` is unused, the `Metadata` this
    /// extends holding that.
    pub(crate) rich: RsRich,
}

impl RichMetadata {
    /// The facade of the crate's rich metadata.
    pub(crate) fn object<'py>(py: Python<'py>, rich: &RsRich) -> PyResult<Bound<'py, Self>> {
        let mut values = rich.clone();
        let metadata = std::mem::take(&mut values.metadata);
        let initializer =
            crate::base::built(Metadata::wrap(metadata)).add_subclass(Self { rich: values });
        Bound::new(py, initializer)
    }
}

/// The crate's rich metadata of a `RichMetadata`, or of a plain
/// `Metadata` with nothing read off a score.
pub(crate) fn rich_value(object: &Bound<'_, PyAny>) -> PyResult<RsRich> {
    if let Ok(rich) = object.cast::<RichMetadata>() {
        let mut value = rich.borrow().rich.clone();
        value.metadata = rich.as_super().borrow().inner.clone();
        return Ok(value);
    }
    Ok(RsRich::of(crate::stream::metadata_value(object)?))
}

#[pymethods]
impl RichMetadata {
    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(
        py: Python<'_>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let made = Metadata::new(py, keywords)?;
        Ok(made.add_subclass(Self {
            rich: RsRich::default(),
        }))
    }

    /// The names of the values read off a score.
    #[classattr]
    fn additionalRichMetadataAttributes(py: Python<'_>) -> PyResult<Py<PyTuple>> {
        Ok(PyTuple::new(py, RICH_ATTRIBUTES)?.unbind())
    }

    #[getter]
    fn ambitus<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        rich_object(py, &self.rich.value("ambitus").unwrap_or(RichValue::None))
    }

    #[getter]
    fn keySignatureFirst(&self) -> Option<i64> {
        self.rich.key_signature_first.map(|sharps| sharps as i64)
    }

    #[getter]
    fn keySignatures(&self) -> Vec<i64> {
        self.rich
            .key_signatures
            .iter()
            .map(|sharps| *sharps as i64)
            .collect()
    }

    #[getter]
    fn noteCount(&self) -> Option<i64> {
        self.rich.note_count.map(|count| count as i64)
    }

    #[getter]
    fn numberOfParts(&self) -> Option<i64> {
        self.rich.number_of_parts.map(|count| count as i64)
    }

    #[getter]
    fn pitchHighest(&self) -> Option<String> {
        self.rich.pitch_highest.clone()
    }

    #[getter]
    fn pitchLowest(&self) -> Option<String> {
        self.rich.pitch_lowest.clone()
    }

    #[getter]
    fn scoreQuarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        length_object(py, self.rich.score_quarter_length)
    }

    #[getter]
    fn sourcePath(&self) -> String {
        self.rich.source_path.clone()
    }

    #[getter]
    fn tempoFirst(&self) -> Option<String> {
        self.rich.tempo_first.clone()
    }

    #[getter]
    fn tempos(&self) -> Vec<String> {
        self.rich.tempos.clone()
    }

    #[getter]
    fn timeSignatureFirst(&self) -> Option<String> {
        self.rich.time_signature_first.clone()
    }

    #[getter]
    fn timeSignatures(&self) -> Vec<String> {
        self.rich.time_signatures.clone()
    }

    /// Sets a value read off a score by its name, or one of the metadata
    /// vocabulary as `Metadata` does. A key signature is kept as its
    /// sharps.
    fn __setattr__(slf: &Bound<'_, Self>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        if !RICH_ATTRIBUTES.contains(&name) {
            return Metadata::__setattr__(slf.as_super(), name, value);
        }
        let sharps = |value: &Bound<'_, PyAny>| -> PyResult<Option<_>> {
            if value.is_none() {
                return Ok(None);
            }
            if value.hasattr("sharps")? {
                return value.getattr("sharps")?.extract().map(Some);
            }
            value.extract().map(Some)
        };
        let text = |value: &Bound<'_, PyAny>| -> PyResult<Option<String>> {
            if value.is_none() {
                return Ok(None);
            }
            Ok(Some(value.str()?.to_string()))
        };
        let mut me = slf.borrow_mut();
        let rich = &mut me.rich;
        match name {
            "ambitus" => rich.ambitus = ambitus_value(value)?,
            "keySignatureFirst" => rich.key_signature_first = sharps(value)?,
            "keySignatures" => {
                rich.key_signatures = value
                    .try_iter()?
                    .map(|each| sharps(&each?).map(Option::unwrap_or_default))
                    .collect::<PyResult<_>>()?;
            }
            "noteCount" => rich.note_count = value.extract()?,
            "numberOfParts" => rich.number_of_parts = value.extract()?,
            "pitchHighest" => rich.pitch_highest = text(value)?,
            "pitchLowest" => rich.pitch_lowest = text(value)?,
            "scoreQuarterLength" => rich.score_quarter_length = length_value(value)?,
            "sourcePath" => rich.source_path = value.str()?.to_string(),
            "tempoFirst" => rich.tempo_first = text(value)?,
            "tempos" => rich.tempos = value.extract()?,
            "timeSignatureFirst" => rich.time_signature_first = text(value)?,
            _ => rich.time_signatures = value.extract()?,
        }
        Ok(())
    }

    /// Takes another's metadata in place of this one's: music21's `merge`,
    /// which copies `_contents` alone. `favorSelf` keeps this one's.
    #[pyo3(signature = (other, favorSelf = false))]
    fn merge(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>, favorSelf: bool) -> PyResult<()> {
        if favorSelf || other.is_none() {
            return Ok(());
        }
        let metadata = crate::stream::metadata_value(other)?;
        slf.as_super().borrow_mut().inner = metadata;
        Ok(())
    }

    /// Reads the thirteen values off a score: music21's `update`.
    fn update(slf: &Bound<'_, Self>, streamObj: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let (stream, _) = crate::stream::read_with_leaves(streamObj)?;
        let corpus = crate::corpus::corpus_directory(py);
        let mut rich = rich_value(slf.as_any())?;
        rich.update(&stream, corpus.as_deref())
            .map_err(|error| crate::stream::stream_error(&error))?;
        rich.metadata = Default::default();
        slf.borrow_mut().rich = rich;
        Ok(())
    }

    /// The first field a query matches: music21's `search`, over the
    /// values read off the score as well as the metadata.
    #[pyo3(signature = (query = None, field = None, **keywords))]
    fn search(
        slf: &Bound<'_, Self>,
        query: Option<&Bound<'_, PyAny>>,
        field: Option<String>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<(bool, Option<String>)> {
        let (query, field) = match (query.filter(|query| !query.is_none()), field) {
            (Some(query), field) => (query.clone(), field),
            (None, Some(field)) => (slf.py().None().into_bound(slf.py()), Some(field)),
            (None, None) => {
                let Some((key, value)) = keywords.and_then(last_item) else {
                    return Ok((false, None));
                };
                (value, Some(key))
            }
        };
        let rich = rich_value(slf.as_any())?;
        let query = Query::read(&query)?;
        let found = query.run(|asked| {
            rich.search(asked, field.as_deref())
                .map(|found| found.map(str::to_string))
        })?;
        Ok((found.is_some(), found.flatten()))
    }

    /// music21's `all`, the values read off the score among the rest.
    #[pyo3(signature = (
        *,
        skipContributors = false,
        skipNonContributors = false,
        returnPrimitives = false,
        returnSorted = true
    ))]
    fn all<'py>(
        slf: &Bound<'py, Self>,
        skipContributors: bool,
        skipNonContributors: bool,
        returnPrimitives: bool,
        returnSorted: bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let kwargs = PyDict::new(py);
        kwargs.set_item("skipContributors", skipContributors)?;
        kwargs.set_item("skipNonContributors", skipNonContributors)?;
        kwargs.set_item("returnPrimitives", returnPrimitives)?;
        kwargs.set_item("returnSorted", returnSorted)?;
        let base = py.get_type::<Metadata>().getattr("all")?;
        let mut found: Vec<Bound<'py, PyAny>> = base
            .call((slf,), Some(&kwargs))?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        if skipNonContributors {
            return PyTuple::new(py, found);
        }
        let me = slf.borrow();
        for name in RICH_ATTRIBUTES {
            let value = rich_object(py, &me.rich.value(name).unwrap_or(RichValue::None))?;
            found.push(PyTuple::new(py, [PyString::new(py, name).into_any(), value])?.into_any());
        }
        let found = PyList::new(py, found)?;
        if returnSorted {
            found.sort()?;
        }
        PyTuple::new(py, found)
    }

    /// Whether a name is one of the vocabulary's or of the values read off
    /// a score.
    fn _isStandardUniqueName(slf: &Bound<'_, Self>, uniqueName: &str) -> PyResult<bool> {
        if RICH_ATTRIBUTES.contains(&uniqueName) {
            return Ok(true);
        }
        Ok(slf.as_super().borrow().isStandardName(uniqueName)
            && music21::metadata::namespace_name(uniqueName).is_some())
    }

    fn __repr__(slf: &Bound<'_, Self>) -> String {
        format!(
            "<music21.metadata.RichMetadata object at 0x{:x}>",
            slf.as_ptr() as usize
        )
    }
}

/// The last keyword given, as music21's `keywords.popitem()` takes it.
pub(crate) fn last_item<'py>(keywords: &Bound<'py, PyDict>) -> Option<(String, Bound<'py, PyAny>)> {
    let (key, value) = keywords.iter().last()?;
    Some((key.extract().ok()?, value))
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RichMetadata>()?;
    m.add("AmbitusShort", ambitus_class(m.py())?)?;
    Ok(())
}
