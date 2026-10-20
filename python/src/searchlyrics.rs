//! music21's `search.lyrics`: text found in a score's lyrics, and the
//! notes it is sung to.
//!
//! The indexing is the crate's `search::LyricSearcher` -- each verse read
//! as one text, syllables of a word run together and words apart, the
//! verses one after another -- and the facade reads a music21 stream into
//! the crate, asks it, and carries the answer back onto the stream's own
//! notes and lyrics: `el` *is* the score's note and `lyric` its lyric, as
//! music21's are, which is what the searcher's by-position twin
//! (`places`) is for.
//!
//! Only the matching of a regular expression is Python's. music21's own
//! `search` escapes a plain string and runs `re` over the index either
//! way; here a plain string is the crate's own search and a compiled
//! pattern is `finditer` over the crate's index text, whose spans the
//! crate then reads as the lyrics they touch (`indices_between`). A
//! pattern is Python's to match -- the crate has no regular expressions --
//! but which lyrics a stretch of text is sung to stays the crate's.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21_rs_crate::IntegerType;
use music21_rs_crate::search::{LyricIdentifier, LyricSearcher as RsLyricSearcher};

/// The names this module hands out.
pub const NAMES: &[&str] = &["IndexedLyric", "SearchMatch", "LyricSearcher"];

pyo3::create_exception!(
    music21_rs_facade,
    LyricSearcherException,
    crate::Music21Exception
);

/// The names of an `IndexedLyric`'s parts, in music21's order, which is
/// what one is read as a tuple by and what `modify` takes.
const INDEXED_PARTS: [&str; 9] = [
    "el",
    "start",
    "end",
    "measure",
    "lyric",
    "text",
    "identifier",
    "absoluteStart",
    "absoluteEnd",
];

/// music21's `search.lyrics.IndexedLyric`: one lyric, the note it is sung
/// to, and where its text sits in the text searched.
///
/// music21's is a `namedtuple`, so this answers `len`, indexing and
/// iteration as well as naming its parts.
#[pyclass(
    name = "IndexedLyric",
    module = "music21.search.lyrics",
    subclass,
    skip_from_py_object
)]
pub struct IndexedLyric {
    /// The note or chord the lyric is sung to, the stream's own object.
    #[pyo3(get)]
    el: Py<PyAny>,
    /// Where the lyric starts in its verse's text.
    #[pyo3(get)]
    start: usize,
    /// Where it ends there.
    #[pyo3(get)]
    end: usize,
    /// The number of the measure the note is in, if it is in one.
    #[pyo3(get)]
    measure: Option<IntegerType>,
    /// The lyric itself, the note's own object.
    #[pyo3(get)]
    lyric: Py<PyAny>,
    /// Its text.
    #[pyo3(get)]
    text: String,
    /// Its verse: the name the score gave it or, failing one, its number.
    #[pyo3(get)]
    identifier: Py<PyAny>,
    /// Where it starts in the text of every verse.
    #[pyo3(get)]
    absoluteStart: usize,
    /// Where it ends there.
    #[pyo3(get)]
    absoluteEnd: usize,
}

impl IndexedLyric {
    /// The parts in music21's order, which is the order a `namedtuple`
    /// reads them in.
    fn parts<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(vec![
            self.el.bind(py).clone(),
            self.start.into_pyobject(py)?.into_any(),
            self.end.into_pyobject(py)?.into_any(),
            match self.measure {
                Some(measure) => measure.into_pyobject(py)?.into_any(),
                None => py.None().into_bound(py),
            },
            self.lyric.bind(py).clone(),
            PyString::new(py, &self.text).into_any(),
            self.identifier.bind(py).clone(),
            self.absoluteStart.into_pyobject(py)?.into_any(),
            self.absoluteEnd.into_pyobject(py)?.into_any(),
        ])
    }
}

#[pymethods]
impl IndexedLyric {
    /// music21's `IndexedLyric(el, start, end, measure, lyric, text,
    /// identifier, absoluteStart, absoluteEnd)`.
    ///
    /// Every part has a default, as nothing of music21's `namedtuple`
    /// does, because the install helper builds a blank one with no
    /// arguments at all.
    #[new]
    #[pyo3(signature = (el = None, start = 0, end = 0, measure = None, lyric = None, text = String::new(), identifier = None, absoluteStart = 0, absoluteEnd = 0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        el: Option<Py<PyAny>>,
        start: usize,
        end: usize,
        measure: Option<IntegerType>,
        lyric: Option<Py<PyAny>>,
        text: String,
        identifier: Option<Py<PyAny>>,
        absoluteStart: usize,
        absoluteEnd: usize,
    ) -> Self {
        let el = el.unwrap_or_else(|| py.None());
        let lyric = lyric.unwrap_or_else(|| py.None());
        let identifier = identifier.unwrap_or_else(|| py.None());
        Self {
            el,
            start,
            end,
            measure,
            lyric,
            text,
            identifier,
            absoluteStart,
            absoluteEnd,
        }
    }

    /// One of these with some parts written afresh: music21's `modify`,
    /// which is how it writes the absolute positions in once every verse
    /// before it is known.
    #[pyo3(signature = (**keywords))]
    fn modify(slf: &Bound<'_, Self>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Py<Self>> {
        let py = slf.py();
        let mut parts = slf.borrow().parts(py)?;
        if let Some(keywords) = keywords {
            for (name, value) in keywords.iter() {
                let name: String = name.extract()?;
                let Some(at) = INDEXED_PARTS.iter().position(|part| *part == name) else {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "modify() got an unexpected keyword argument '{name}'"
                    )));
                };
                parts[at] = value;
            }
        }
        let made = PyTuple::new(py, parts)?;
        Ok(slf.get_type().call(made, None)?.extract()?)
    }

    /// How many parts one of these has, a `namedtuple` of nine.
    fn __len__(&self) -> usize {
        INDEXED_PARTS.len()
    }

    fn __getitem__<'py>(slf: &Bound<'py, Self>, at: isize) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let parts = slf.borrow().parts(py)?;
        let found = if at < 0 {
            parts.len().checked_sub(at.unsigned_abs())
        } else {
            Some(at.unsigned_abs())
        };
        found
            .and_then(|at| parts.get(at).cloned())
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("tuple index out of range"))
    }

    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        let py = slf.py();
        let parts = slf.borrow().parts(py)?;
        PyTuple::new(py, parts)?.as_any().try_iter()
    }

    /// Two of these are equal where their parts are, and one is equal to
    /// the tuple of its parts, as music21's `namedtuple` is.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py = slf.py();
        let parts = PyTuple::new(py, slf.borrow().parts(py)?)?;
        if let Ok(theirs) = other.extract::<PyRef<'_, Self>>() {
            let theirs = PyTuple::new(py, theirs.parts(py)?)?;
            return parts.eq(theirs);
        }
        parts.as_any().eq(other)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let slf = slf.borrow();
        let measure = match slf.measure {
            Some(measure) => measure.to_string(),
            None => "None".to_string(),
        };
        Ok(format!(
            "IndexedLyric(el={}, start={}, end={}, measure={}, lyric={}, text={}, identifier={})",
            slf.el.bind(py).repr()?,
            slf.start,
            slf.end,
            measure,
            slf.lyric.bind(py).repr()?,
            PyString::new(py, &slf.text).repr()?,
            slf.identifier.bind(py).repr()?,
        ))
    }
}

/// music21's `search.lyrics.SearchMatch`: one stretch of lyrics a search
/// found.
#[pyclass(
    name = "SearchMatch",
    module = "music21.search.lyrics",
    subclass,
    skip_from_py_object
)]
pub struct SearchMatch {
    /// The measure of the first lyric matched, if it is in one.
    #[pyo3(get)]
    mStart: Option<IntegerType>,
    /// The measure of the last.
    #[pyo3(get)]
    mEnd: Option<IntegerType>,
    /// The text matched, which for a plain search is the search itself.
    #[pyo3(get)]
    matchText: String,
    /// The notes the matched lyrics are sung to.
    #[pyo3(get)]
    els: Py<PyTuple>,
    /// Every lyric the match touches.
    #[pyo3(get)]
    indices: Py<PyList>,
    /// The verse of the first.
    #[pyo3(get)]
    identifier: Py<PyAny>,
}

impl SearchMatch {
    /// The parts in music21's order.
    fn parts<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let number = |value: Option<IntegerType>| -> PyResult<Bound<'py, PyAny>> {
            Ok(match value {
                Some(value) => value.into_pyobject(py)?.into_any(),
                None => py.None().into_bound(py),
            })
        };
        Ok(vec![
            number(self.mStart)?,
            number(self.mEnd)?,
            PyString::new(py, &self.matchText).into_any(),
            self.els.bind(py).as_any().clone(),
            self.indices.bind(py).as_any().clone(),
            self.identifier.bind(py).clone(),
        ])
    }
}

#[pymethods]
impl SearchMatch {
    /// music21's `SearchMatch(mStart, mEnd, matchText, els, indices,
    /// identifier)`, every part with a default for the same reason
    /// `IndexedLyric`'s have one.
    #[new]
    #[pyo3(signature = (mStart = None, mEnd = None, matchText = String::new(), els = None, indices = None, identifier = None))]
    fn new(
        py: Python<'_>,
        mStart: Option<IntegerType>,
        mEnd: Option<IntegerType>,
        matchText: String,
        els: Option<Py<PyTuple>>,
        indices: Option<Py<PyList>>,
        identifier: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let els = match els {
            Some(els) => els,
            None => PyTuple::empty(py).unbind(),
        };
        let indices = indices.unwrap_or_else(|| PyList::empty(py).unbind());
        let identifier = identifier.unwrap_or_else(|| py.None());
        Ok(Self {
            mStart,
            mEnd,
            matchText,
            els,
            indices,
            identifier,
        })
    }

    /// How many parts one of these has, a `namedtuple` of six.
    fn __len__(&self) -> usize {
        6
    }

    fn __getitem__<'py>(slf: &Bound<'py, Self>, at: isize) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let parts = slf.borrow().parts(py)?;
        let found = if at < 0 {
            parts.len().checked_sub(at.unsigned_abs())
        } else {
            Some(at.unsigned_abs())
        };
        found
            .and_then(|at| parts.get(at).cloned())
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("tuple index out of range"))
    }

    fn __iter__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        let py = slf.py();
        let parts = slf.borrow().parts(py)?;
        PyTuple::new(py, parts)?.as_any().try_iter()
    }

    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py = slf.py();
        let parts = PyTuple::new(py, slf.borrow().parts(py)?)?;
        if let Ok(theirs) = other.extract::<PyRef<'_, Self>>() {
            let theirs = PyTuple::new(py, theirs.parts(py)?)?;
            return parts.eq(theirs);
        }
        parts.as_any().eq(other)
    }

    /// music21's own `repr`, which says nothing of the lyrics themselves:
    /// there may be very many of them.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let slf = slf.borrow();
        let measure = |value: Option<IntegerType>| match value {
            Some(value) => value.to_string(),
            None => "None".to_string(),
        };
        Ok(format!(
            "SearchMatch(mStart={}, mEnd={}, matchText={}, els={}, indices=[...], identifier={})",
            measure(slf.mStart),
            measure(slf.mEnd),
            PyString::new(py, &slf.matchText).repr()?,
            slf.els.bind(py).as_any().repr()?,
            slf.identifier.bind(py).repr()?,
        ))
    }
}

/// music21's `search.lyrics.LyricSearcher`: a stream's lyrics read as one
/// text, and searched.
#[pyclass(
    name = "LyricSearcher",
    module = "music21.search.lyrics",
    subclass,
    skip_from_py_object
)]
pub struct LyricSearcher {
    /// The stream searched: music21's `stream`, which a caller may write.
    #[pyo3(get, set)]
    stream: Option<Py<PyAny>>,
    /// What goes between words: music21's `wordSeparator`.
    #[pyo3(get, set)]
    wordSeparator: String,
    /// music21's flag, which does nothing there either.
    #[pyo3(get, set)]
    includeIntermediateElements: bool,
    /// The same.
    #[pyo3(get, set)]
    includeTrailingMelisma: bool,
    /// The index last read, if any: the text and the lyrics in it.
    indexed: Option<(String, Py<PyList>)>,
}

impl LyricSearcher {
    /// The stream to read: the one given, else the one kept, else
    /// music21's refusal.
    fn searched<'py>(
        &self,
        py: Python<'py>,
        given: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(given) = given.filter(|given| !given.is_none()) {
            return Ok(given.clone());
        }
        match &self.stream {
            Some(stream) => Ok(stream.bind(py).clone()),
            None => Err(LyricSearcherException::new_err(
                "Cannot index without a Stream to search.",
            )),
        }
    }
}

/// A verse as music21 answers it: the name a score gave it, or its number
/// where it has none.
fn identifier_object<'py>(
    py: Python<'py>,
    identifier: &LyricIdentifier,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match identifier {
        LyricIdentifier::Name(name) => PyString::new(py, name).into_any(),
        LyricIdentifier::Number(number) => number.into_pyobject(py)?.into_any(),
    })
}

/// A stream read for searching, with the crate's reading of it beside the
/// objects at each position.
struct Reading {
    stream: music21_rs_crate::Stream,
    leaves: Vec<Py<PyAny>>,
}

impl Reading {
    fn of(stream: &Bound<'_, PyAny>) -> PyResult<Self> {
        let (stream, leaves) = crate::stream::read_with_leaves(stream)?;
        Ok(Self { stream, leaves })
    }

    /// The index of a searcher over this reading as music21's objects:
    /// each lyric with the note it is sung to, both the stream's own.
    fn indexed<'py>(
        &self,
        py: Python<'py>,
        searcher: &RsLyricSearcher<'_>,
    ) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for (lyric, place) in searcher.indexed().iter().zip(searcher.places()) {
            let el = self
                .leaves
                .get(place.position)
                .ok_or_else(|| {
                    LyricSearcherException::new_err(
                        "a lyric was indexed on a note the stream does not hold",
                    )
                })?
                .bind(py)
                .clone();
            // The note's own lyric object, which is what music21 hands
            // back: a caller writing into it writes into the score.
            let held = el.getattr("lyrics")?.get_item(place.lyric)?;
            out.append(crate::installed_new(
                py,
                "music21.search.lyrics",
                "IndexedLyric",
                IndexedLyric {
                    el: el.unbind(),
                    start: lyric.start,
                    end: lyric.end,
                    measure: lyric.measure,
                    lyric: held.unbind(),
                    text: lyric.text.clone(),
                    identifier: identifier_object(py, &lyric.identifier)?.unbind(),
                    absoluteStart: lyric.absolute_start,
                    absoluteEnd: lyric.absolute_end,
                },
            )?)?;
        }
        Ok(out)
    }
}

/// One match, built out of the lyrics a stretch of the text is sung to.
fn match_of<'py>(
    py: Python<'py>,
    indexed: &Bound<'py, PyList>,
    text: &str,
    found: core::ops::Range<usize>,
) -> PyResult<Bound<'py, PyAny>> {
    let touched = PyList::empty(py);
    let els = PyList::empty(py);
    for at in found {
        let lyric = indexed.get_item(at)?;
        els.append(lyric.getattr("el")?)?;
        touched.append(lyric)?;
    }
    let first = touched.get_item(0)?;
    let last = touched.get_item(touched.len() - 1)?;
    let made = SearchMatch {
        mStart: first.getattr("measure")?.extract()?,
        mEnd: last.getattr("measure")?.extract()?,
        matchText: text.to_string(),
        els: PyTuple::new(py, els.iter())?.unbind(),
        indices: touched.unbind(),
        identifier: first.getattr("identifier")?.unbind(),
    };
    Ok(
        crate::installed_new(py, "music21.search.lyrics", "SearchMatch", made)?
            .into_bound(py)
            .into_any(),
    )
}

#[pymethods]
impl LyricSearcher {
    /// music21's `LyricSearcher(s=None, *, wordSeparator=' ')`.
    #[new]
    #[pyo3(signature = (s = None, *, wordSeparator = " ".to_string()))]
    fn new(s: Option<Py<PyAny>>, wordSeparator: String) -> Self {
        Self {
            stream: s,
            wordSeparator,
            includeIntermediateElements: false,
            includeTrailingMelisma: false,
            indexed: None,
        }
    }

    /// The text that has been indexed: each verse's lyrics, the verses
    /// joined by music21's line break. music21's `indexText`.
    #[getter]
    fn indexText(slf: &Bound<'_, Self>) -> PyResult<String> {
        if slf.borrow().indexed.is_none() {
            Self::index(slf, None)?;
        }
        Ok(slf
            .borrow()
            .indexed
            .as_ref()
            .map(|(text, _)| text.clone())
            .unwrap_or_default())
    }

    /// Every lyric with text and where it sits in [`Self::indexText`]:
    /// music21's `indexTuples`.
    #[getter]
    fn indexTuples(slf: &Bound<'_, Self>) -> PyResult<Py<PyList>> {
        if slf.borrow().indexed.is_none() {
            Self::index(slf, None)?;
        }
        let py = slf.py();
        Ok(match &slf.borrow().indexed {
            Some((_, indexed)) => indexed.clone_ref(py),
            None => PyList::empty(py).unbind(),
        })
    }

    /// The stream's lyrics indexed, which is what a search is run over:
    /// music21's `index`, which also takes the stream to search and keeps
    /// it.
    #[pyo3(signature = (s = None))]
    fn index(slf: &Bound<'_, Self>, s: Option<&Bound<'_, PyAny>>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        let stream = slf.borrow().searched(py, s)?;
        slf.borrow_mut().stream = Some(stream.clone().unbind());
        let separator = slf.borrow().wordSeparator.clone();
        let reading = Reading::of(&stream)?;
        let searcher = RsLyricSearcher::with_word_separator(&reading.stream, &separator);
        let indexed = reading.indexed(py, &searcher)?;
        let text = searcher.index_text().to_string();
        slf.borrow_mut().indexed = Some((text, indexed.clone().unbind()));
        Ok(indexed.unbind())
    }

    /// Every place a string or a regular expression is sung: music21's
    /// `search`.
    #[pyo3(signature = (textOrRe, s = None))]
    fn search<'py>(
        slf: &Bound<'py, Self>,
        textOrRe: &Bound<'py, PyAny>,
        s: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let plain: Option<String> = textOrRe.extract().ok();
        if plain.is_none() && textOrRe.getattr("finditer").is_err() {
            return Err(LyricSearcherException::new_err(format!(
                "{} is not a string or RE with the finditer() function",
                textOrRe.str()?
            )));
        }
        let stream = slf.borrow().searched(py, s)?;
        slf.borrow_mut().stream = Some(stream.clone().unbind());
        let separator = slf.borrow().wordSeparator.clone();
        let reading = Reading::of(&stream)?;
        let searcher = RsLyricSearcher::with_word_separator(&reading.stream, &separator);
        let indexed = reading.indexed(py, &searcher)?;
        slf.borrow_mut().indexed =
            Some((searcher.index_text().to_string(), indexed.clone().unbind()));

        let out = PyList::empty(py);
        match plain {
            // A plain string is the crate's own search, text and lyrics
            // together.
            Some(text) => {
                for span in searcher.search_spans(&text).map_err(|error| {
                    LyricSearcherException::new_err(crate::pitch::message(&error))
                })? {
                    out.append(match_of(py, &indexed, &span.text, span.indices)?)?;
                }
            }
            // A pattern is Python's to match; which lyrics each stretch it
            // finds is sung to is the crate's.
            None => {
                for found in textOrRe
                    .call_method1("finditer", (searcher.index_text(),))?
                    .try_iter()?
                {
                    let found = found?;
                    let text: String = found.call_method0("group")?.extract()?;
                    let start: usize = found.call_method0("start")?.extract()?;
                    let end: usize = found.call_method0("end")?.extract()?;
                    let touched = searcher.indices_between(start, end).map_err(|error| {
                        LyricSearcherException::new_err(crate::pitch::message(&error))
                    })?;
                    out.append(match_of(py, &indexed, &text, touched)?)?;
                }
            }
        }
        Ok(out)
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<IndexedLyric>()?;
    m.add_class::<SearchMatch>()?;
    m.add_class::<LyricSearcher>()?;
    Ok(())
}
