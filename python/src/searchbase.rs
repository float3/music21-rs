//! music21's `search.base`: runs of notes found in a stream, and the text
//! a stream is turned into so that one line can be compared with another.
//!
//! Everything musical here is the crate's -- `search::StreamSearcher` and
//! the three comparisons it is run with, the four searches, the five
//! translations, the four approximate searches and the measure rhythms --
//! and the facade's work is reading what music21 hands over. music21's own
//! searches and translations take a stream, one of its stream iterators or
//! any list of elements, and iterate it as it stands; so each element is
//! read as the value it is (`stream::element_read`), with where it stands
//! and the measure it is in read off the object itself, since a value
//! knows neither.
//!
//! `Wildcard` and `WildcardDuration` stay music21's: they are objects a
//! caller puts in a search list, and the crate says what they *mean* --
//! `SearchTerm::Wildcard` for anything at all and `AnyLength` for an
//! element whose length does not matter. `SearchMatch` stays music21's
//! too, being four values in a row: music21's own carries its `repr` and
//! its documentation, and a wheel with no music21 beside it answers a
//! `namedtuple` of the same name and parts.
//!
//! **An algorithm of one's own is refused rather than run.**
//! `StreamSearcher.algorithms` is a list of functions a caller may add to,
//! and the three music21 ships are the crate's `Algorithm::Wildcard`,
//! `Rhythm` and `NoteName`; a function from somewhere else cannot be given
//! to the crate's search, and running the window loop here instead would
//! be a second reading of the rule the crate carries. So it is said, as
//! `jitter` is in the segment search.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21_rs_crate::search::{
    Algorithm, SearchTerm, Searched, StreamSearcher as RsStreamSearcher, Translation,
    approximate_note_search, approximate_note_search_no_rhythm,
    approximate_note_search_only_rhythm, approximate_note_search_weighted,
    most_common_measure_rhythm_places, note_name_rhythmic_search, note_name_search,
    rhythmic_search, translate_diatonic_stream_to_string, translate_duration_to_bytes,
    translate_intervals_and_speed, translate_note_tie_to_byte, translate_note_to_byte,
    translate_note_with_duration_to_bytes, translate_stream_to_string,
    translate_stream_to_string_no_rhythm, translate_stream_to_string_only_rhythm,
};
use music21_rs_crate::stream::{Stream as RsStream, StreamElement};
use music21_rs_crate::{FloatType, IntegerType};

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "StreamSearcher",
    "streamSearchBase",
    "rhythmicSearch",
    "noteNameSearch",
    "noteNameRhythmicSearch",
    "approximateNoteSearch",
    "approximateNoteSearchNoRhythm",
    "approximateNoteSearchOnlyRhythm",
    "approximateNoteSearchWeighted",
    "translateStreamToString",
    "translateDiatonicStreamToString",
    "translateIntervalsAndSpeed",
    "translateStreamToStringNoRhythm",
    "translateStreamToStringOnlyRhythm",
    "translateNoteToByte",
    "translateNoteWithDurationToBytes",
    "translateNoteTieToByte",
    "translateDurationToBytes",
    "mostCommonMeasureRhythms",
];

pyo3::create_exception!(music21_rs_facade, SearchException, crate::Music21Exception);

/// Whether an object answers to a music21 class by name, which is how a
/// `Wildcard` and a `WildcardDuration` are told apart from a note and its
/// duration.
fn answers_to(object: &Bound<'_, PyAny>, name: &str) -> bool {
    object
        .getattr("classes")
        .and_then(|classes| classes.contains(name))
        .unwrap_or(false)
}

/// Everything a search or a translation is handed: each element as the
/// value it is, beside the object it was read from, where it stands, and
/// the measure it is in.
///
/// music21 iterates whatever it is given -- a stream, a stream iterator or
/// a list -- so this does too, and reads the offset and the measure number
/// off each object: a value says neither, its holder does.
struct Handed {
    values: Vec<StreamElement>,
    objects: Vec<Py<PyAny>>,
    offsets: Vec<FloatType>,
    measures: Vec<Option<IntegerType>>,
}

impl Handed {
    fn of(given: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut handed = Self {
            values: Vec::new(),
            objects: Vec::new(),
            offsets: Vec::new(),
            measures: Vec::new(),
        };
        for element in given.walk()? {
            let element = element?;
            let Some(value) = crate::stream::element_read(&element)? else {
                return Err(SearchException::new_err(format!(
                    "{} is not an element this package has a value for",
                    element.repr()?
                )));
            };
            handed.offsets.push(
                element
                    .getattr("offset")
                    .and_then(|offset| offset.extract())
                    .unwrap_or(0.0),
            );
            handed.measures.push(
                element
                    .getattr("measureNumber")
                    .and_then(|number| number.extract::<Option<IntegerType>>())
                    .unwrap_or(None),
            );
            handed.values.push(value);
            handed.objects.push(element.unbind());
        }
        Ok(handed)
    }

    /// What the crate's searches and translations take.
    fn searched(&self) -> Vec<Searched<'_>> {
        self.values
            .iter()
            .zip(&self.offsets)
            .zip(&self.measures)
            .map(|((element, offset), measure)| Searched {
                element,
                offset: *offset,
                measure: *measure,
            })
            .collect()
    }
}

/// What a search looks for: music21's search list, which may be a stream,
/// a list or anything iterable, and may hold its `Wildcard`.
fn terms_of(searchList: &Bound<'_, PyAny>) -> PyResult<Vec<SearchTerm>> {
    let mut terms = Vec::new();
    for element in searchList.walk()? {
        let element = element?;
        if answers_to(&element, "Wildcard") {
            terms.push(SearchTerm::Wildcard);
            continue;
        }
        let Some(value) = crate::stream::element_read(&element)? else {
            return Err(SearchException::new_err(format!(
                "{} is not an element this package has a value for",
                element.repr()?
            )));
        };
        // A term written with music21's `WildcardDuration` matches
        // whatever length it meets.
        let any_length = element
            .getattr("duration")
            .is_ok_and(|duration| answers_to(&duration, "WildcardDuration"));
        terms.push(if any_length {
            SearchTerm::AnyLength(value)
        } else {
            SearchTerm::Element(value)
        });
    }
    Ok(terms)
}

/// music21's error for what a search cannot answer, out of the crate's.
fn search_error(error: &music21_rs_crate::Error) -> PyErr {
    SearchException::new_err(crate::pitch::message(error))
}

/// The places a search found, as music21 answers them.
fn places<'py>(py: Python<'py>, found: Vec<usize>) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for at in found {
        out.append(at)?;
    }
    Ok(out)
}

/// music21's own `SearchMatch` for this module, or a named tuple of its
/// four parts where music21 cannot be asked: four values in a row are
/// nothing musical, and music21's own carries its `repr`, which counts
/// the elements rather than listing them.
static MATCH: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

fn match_class<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    crate::music21_or_namedtuple(
        py,
        "music21.search.base",
        "SearchMatch",
        "elStart els index iterator",
        &MATCH,
    )
}

/// music21's `StreamSearcher`: a stream searched for a run of elements,
/// compared by a list of algorithms.
#[pyclass(
    name = "StreamSearcher",
    module = "music21.search.base",
    subclass,
    skip_from_py_object
)]
pub struct StreamSearcher {
    /// The stream or stream iterator searched.
    #[pyo3(get, set)]
    streamSearch: Py<PyAny>,
    /// What is looked for, in order.
    #[pyo3(get, set)]
    searchList: Py<PyAny>,
    /// Whether to look inside the streams the stream holds.
    #[pyo3(get, set)]
    recurse: bool,
    /// Whether to look at notes and chords alone.
    #[pyo3(get, set)]
    filterNotes: bool,
    /// Whether to look at notes, chords and rests.
    #[pyo3(get, set)]
    filterNotesAndRests: bool,
    /// How to compare, in order: music21's list of functions, which a
    /// caller adds to.
    #[pyo3(get, set)]
    algorithms: Py<PyList>,
    /// The iterator the last run read, which every match names.
    #[pyo3(get, set)]
    activeIterator: Py<PyAny>,
}

impl StreamSearcher {
    /// The crate's reading of music21's list of algorithm functions, which
    /// are compared by name: a function from somewhere else cannot be
    /// given to the crate's search and is refused.
    fn read_algorithms(&self, py: Python<'_>) -> PyResult<Vec<Algorithm>> {
        let mut read = Vec::new();
        for algorithm in self.algorithms.bind(py).iter() {
            let named: String = algorithm
                .getattr("__name__")
                .and_then(|name| name.extract())
                .unwrap_or_default();
            read.push(match named.as_str() {
                "wildcardAlgorithm" => Algorithm::Wildcard,
                "rhythmAlgorithm" => Algorithm::Rhythm,
                "noteNameAlgorithm" => Algorithm::NoteName,
                _ => {
                    return Err(SearchException::new_err(format!(
                        "{named} is an algorithm this package cannot run; \
                         the three music21 ships are the crate's"
                    )));
                }
            });
        }
        Ok(read)
    }

    /// The iterator music21 would search: the one it was given, or the
    /// stream's own walk, filtered as the flags ask.
    fn iterator<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let given = self.streamSearch.bind(py).clone();
        if answers_to(&given, "StreamIterator") || given.getattr("recurse").is_err() {
            return Ok(given);
        }
        let walked = if self.recurse {
            given.call_method0("recurse")?
        } else {
            given.call_method0("iter")?
        };
        if self.filterNotesAndRests {
            return walked.getattr("notesAndRests");
        }
        if self.filterNotes {
            return walked.getattr("notes");
        }
        Ok(walked)
    }
}

#[pymethods]
impl StreamSearcher {
    /// music21's `StreamSearcher(streamSearch, searchList)`.
    #[new]
    #[pyo3(signature = (streamSearch = None, searchList = None))]
    fn new(
        py: Python<'_>,
        streamSearch: Option<Py<PyAny>>,
        searchList: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let algorithms = PyList::empty(py);
        algorithms.append(&algorithm_functions(py)?[0])?;
        Ok(Self {
            streamSearch: streamSearch.unwrap_or_else(|| py.None()),
            searchList: searchList.unwrap_or_else(|| py.None()),
            recurse: false,
            filterNotes: false,
            filterNotesAndRests: false,
            algorithms: algorithms.unbind(),
            activeIterator: py.None(),
        })
    }

    /// Every run of the stream's elements that matches: music21's `run`.
    fn run<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let (iterator, algorithms, terms) = {
            let me = slf.borrow();
            (
                me.iterator(py)?,
                me.read_algorithms(py)?,
                terms_of(me.searchList.bind(py))?,
            )
        };
        slf.borrow_mut().activeIterator = iterator.clone().unbind();
        let handed = Handed::of(&iterator)?;
        let searched = handed.searched();
        let mut searcher = RsStreamSearcher::new(terms);
        searcher.algorithms = algorithms;
        let found = searcher.run_over(&searched).map_err(|error| {
            // music21's own words for a search with nothing in it.
            search_error(&error)
        })?;
        let out = PyList::empty(py);
        let class = match_class(py)?;
        for one in found {
            let els = PyList::empty(py);
            for at in one.index..one.index + searcher.search.len() {
                els.append(handed.objects[at].bind(py))?;
            }
            out.append(class.call1((
                handed.objects[one.index].bind(py),
                PyTuple::new(py, els.iter())?,
                one.index,
                &iterator,
            ))?)?;
        }
        Ok(out)
    }

    /// Whether a wildcard stands where this element does: music21's
    /// `wildcardAlgorithm`, which every search starts with.
    #[classattr]
    fn wildcardAlgorithm(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(algorithm_functions(py)?[0].clone_ref(py))
    }

    /// Whether the two last as long: music21's `rhythmAlgorithm`.
    #[classattr]
    fn rhythmAlgorithm(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(algorithm_functions(py)?[1].clone_ref(py))
    }

    /// Whether the two are called the same: music21's `noteNameAlgorithm`.
    #[classattr]
    fn noteNameAlgorithm(py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(algorithm_functions(py)?[2].clone_ref(py))
    }
}

/// The three comparisons as Python functions, which is what music21's are:
/// a caller adds one to `algorithms` and its own docstrings print the list,
/// where a method of a pyo3 class reads as a method descriptor rather than
/// as `<function StreamSearcher.rhythmAlgorithm at 0x...>`. Each asks the
/// crate; nothing of the comparison itself is written here.
const ALGORITHM_BUILDER: &str = r#"
def build(compare):
    def wildcardAlgorithm(self, streamEl, searchEl):
        """Whether a wildcard stands where this element does."""
        return compare('wildcard', streamEl, searchEl)

    def rhythmAlgorithm(self, streamEl, searchEl):
        """Whether the two last as long."""
        return compare('rhythm', streamEl, searchEl)

    def noteNameAlgorithm(self, streamEl, searchEl):
        """Whether the two are called the same."""
        return compare('noteName', streamEl, searchEl)

    built = (wildcardAlgorithm, rhythmAlgorithm, noteNameAlgorithm)
    for function in built:
        function.__qualname__ = f'StreamSearcher.{function.__name__}'
        function.__module__ = 'music21.search.base'
    return built
"#;

/// The three built once.
static ALGORITHMS: pyo3::sync::PyOnceLock<[Py<PyAny>; 3]> = pyo3::sync::PyOnceLock::new();

fn algorithm_functions(py: Python<'_>) -> PyResult<&[Py<PyAny>; 3]> {
    ALGORITHMS.get_or_try_init(py, || -> PyResult<[Py<PyAny>; 3]> {
        let builder = PyModule::from_code(
            py,
            &std::ffi::CString::new(ALGORITHM_BUILDER)?,
            c"music21_rs_search_algorithms.py",
            c"music21_rs_search_algorithms",
        )?;
        let built = builder
            .getattr("build")?
            .call1((wrap_pyfunction!(_compare, py)?,))?;
        Ok([
            built.get_item(0)?.unbind(),
            built.get_item(1)?.unbind(),
            built.get_item(2)?.unbind(),
        ])
    })
}

/// One pair compared by the crate, named as the Python functions above
/// name it. It is handed to those and to nothing else.
#[pyfunction]
fn _compare(
    kind: &str,
    streamEl: &Bound<'_, PyAny>,
    searchEl: &Bound<'_, PyAny>,
) -> PyResult<Option<bool>> {
    let algorithm = match kind {
        "wildcard" => Algorithm::Wildcard,
        "rhythm" => Algorithm::Rhythm,
        _ => Algorithm::NoteName,
    };
    compared(algorithm, streamEl, searchEl)
}

/// One pair compared by one of the crate's algorithms, which answers that
/// they match, that they do not, or nothing at all.
fn compared(
    algorithm: Algorithm,
    streamEl: &Bound<'_, PyAny>,
    searchEl: &Bound<'_, PyAny>,
) -> PyResult<Option<bool>> {
    let Some(element) = crate::stream::element_read(streamEl)? else {
        return Err(SearchException::new_err(format!(
            "{} is not an element this package has a value for",
            streamEl.repr()?
        )));
    };
    let terms = terms_of(PyList::new(streamEl.py(), [searchEl])?.as_any())?;
    let Some(term) = terms.first() else {
        return Ok(Some(false));
    };
    Ok(algorithm.compare(&element, term))
}

/// music21's `streamSearchBase`, which runs a comparison of the caller's
/// own over every run of a stream's elements.
///
/// The three comparisons music21 itself passes are the crate's, and so is
/// the search; a comparison from somewhere else is refused rather than run
/// by a second reading of the search here.
#[pyfunction]
#[pyo3(signature = (thisStreamOrIterator, searchList, algorithm = None))]
fn streamSearchBase<'py>(
    py: Python<'py>,
    thisStreamOrIterator: &Bound<'py, PyAny>,
    searchList: &Bound<'py, PyAny>,
    algorithm: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let Some(algorithm) = algorithm.filter(|given| !given.is_none()) else {
        return Err(SearchException::new_err(
            "algorithm must be a function not None",
        ));
    };
    let named: String = algorithm
        .getattr("__name__")
        .and_then(|name| name.extract())
        .unwrap_or_default();
    let handed = Handed::of(&recursed_if_stream(thisStreamOrIterator)?)?;
    let searched = handed.searched();
    let terms = terms_of(searchList)?;
    let found = match named.as_str() {
        "rhythmAlgorithm" => rhythmic_search(&searched, &terms),
        "noteNameAlgorithm" => note_name_search(&searched, &terms),
        "noteNameRhythmAlgorithm" => note_name_rhythmic_search(&searched, &terms),
        _ => {
            return Err(SearchException::new_err(format!(
                "{named} is a comparison this package cannot run; \
                 the ones music21 searches with are the crate's"
            )));
        }
    };
    places(py, found.map_err(|error| search_error(&error))?)
}

/// What music21's searches read: the thing itself where it is an iterator
/// or a list, and the whole walk of a stream otherwise.
fn recursed_if_stream<'py>(given: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    if answers_to(given, "StreamIterator") {
        return Ok(given.clone());
    }
    match given.call_method0("recurse") {
        Ok(walked) => Ok(walked),
        Err(_) => Ok(given.clone()),
    }
}

/// music21's `rhythmicSearch`: where each run of elements starts whose
/// lengths are those searched for.
#[pyfunction]
fn rhythmicSearch<'py>(
    py: Python<'py>,
    thisStreamOrIterator: &Bound<'py, PyAny>,
    searchList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let handed = Handed::of(&recursed_if_stream(thisStreamOrIterator)?)?;
    let found = rhythmic_search(&handed.searched(), &terms_of(searchList)?)
        .map_err(|error| search_error(&error))?;
    places(py, found)
}

/// music21's `noteNameSearch`: the same by what each element is called.
#[pyfunction]
fn noteNameSearch<'py>(
    py: Python<'py>,
    thisStreamOrIterator: &Bound<'py, PyAny>,
    searchList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let handed = Handed::of(&recursed_if_stream(thisStreamOrIterator)?)?;
    let found = note_name_search(&handed.searched(), &terms_of(searchList)?)
        .map_err(|error| search_error(&error))?;
    places(py, found)
}

/// music21's `noteNameRhythmicSearch`: by name and by length together.
#[pyfunction]
fn noteNameRhythmicSearch<'py>(
    py: Python<'py>,
    thisStreamOrIterator: &Bound<'py, PyAny>,
    searchList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    let handed = Handed::of(&recursed_if_stream(thisStreamOrIterator)?)?;
    let found = note_name_rhythmic_search(&handed.searched(), &terms_of(searchList)?)
        .map_err(|error| search_error(&error))?;
    places(py, found)
}

/// How one of the crate's approximate searches reads: it answers where
/// each other stream stands and how alike it is.
type Approximate = fn(&RsStream, &[&RsStream]) -> Vec<(usize, FloatType)>;

/// The streams ranked by how alike they are, each told what it scored:
/// music21 writes `matchProbability` onto the stream objects themselves
/// and hands them back best first.
fn ranked<'py>(
    py: Python<'py>,
    thisStream: &Bound<'py, PyAny>,
    otherStreams: &Bound<'py, PyAny>,
    search: Approximate,
) -> PyResult<Bound<'py, PyList>> {
    let (this, _) = crate::stream::read_with_leaves(thisStream)?;
    let mut objects = Vec::new();
    let mut others = Vec::new();
    for other in otherStreams.walk()? {
        let other = other?;
        let (read, _) = crate::stream::read_with_leaves(&other)?;
        others.push(read);
        objects.push(other);
    }
    let borrowed: Vec<&RsStream> = others.iter().collect();
    let out = PyList::empty(py);
    for (at, ratio) in search(&this, &borrowed) {
        let object = &objects[at];
        object.setattr("matchProbability", ratio)?;
        out.append(object)?;
    }
    Ok(out)
}

/// music21's `approximateNoteSearch`: the other streams ranked by how
/// alike their notes and rests read -- pitch, length and tie.
#[pyfunction]
fn approximateNoteSearch<'py>(
    py: Python<'py>,
    thisStream: &Bound<'py, PyAny>,
    otherStreams: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    ranked(py, thisStream, otherStreams, approximate_note_search)
}

/// The same by pitch alone: music21's `approximateNoteSearchNoRhythm`.
#[pyfunction]
fn approximateNoteSearchNoRhythm<'py>(
    py: Python<'py>,
    thisStream: &Bound<'py, PyAny>,
    otherStreams: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    ranked(
        py,
        thisStream,
        otherStreams,
        approximate_note_search_no_rhythm,
    )
}

/// The same by length alone: music21's `approximateNoteSearchOnlyRhythm`.
#[pyfunction]
fn approximateNoteSearchOnlyRhythm<'py>(
    py: Python<'py>,
    thisStream: &Bound<'py, PyAny>,
    otherStreams: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    ranked(
        py,
        thisStream,
        otherStreams,
        approximate_note_search_only_rhythm,
    )
}

/// Three parts pitch to one part length: music21's
/// `approximateNoteSearchWeighted`.
#[pyfunction]
fn approximateNoteSearchWeighted<'py>(
    py: Python<'py>,
    thisStream: &Bound<'py, PyAny>,
    otherStreams: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyList>> {
    ranked(
        py,
        thisStream,
        otherStreams,
        approximate_note_search_weighted,
    )
}

/// The text a translation wrote, with the measures beside it where
/// music21's `returnMeasures` asks for them.
fn written<'py>(
    py: Python<'py>,
    translation: &Translation,
    returnMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let text = PyString::new(py, &translation.text);
    if !returnMeasures {
        return Ok(text.into_any());
    }
    let measures = PyList::empty(py);
    for measure in &translation.measures {
        match measure {
            Some(measure) => measures.append(*measure)?,
            None => measures.append(py.None())?,
        }
    }
    Ok(PyTuple::new(py, [text.into_any(), measures.into_any()])?.into_any())
}

/// Every element's pitch and length as a pair of characters: music21's
/// `translateStreamToString`.
#[pyfunction]
#[pyo3(signature = (inputStreamOrIterator, returnMeasures = false))]
fn translateStreamToString<'py>(
    py: Python<'py>,
    inputStreamOrIterator: &Bound<'py, PyAny>,
    returnMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let handed = Handed::of(inputStreamOrIterator)?;
    written(
        py,
        &translate_stream_to_string(&handed.searched()),
        returnMeasures,
    )
}

/// Each note as the step it moves by and whether it is faster or slower
/// than the one before: music21's `translateDiatonicStreamToString`.
#[pyfunction]
#[pyo3(signature = (inputStreamOrIterator, returnMeasures = false))]
fn translateDiatonicStreamToString<'py>(
    py: Python<'py>,
    inputStreamOrIterator: &Bound<'py, PyAny>,
    returnMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let handed = Handed::of(inputStreamOrIterator)?;
    let translation = translate_diatonic_stream_to_string(&handed.searched())
        .map_err(|error| search_error(&error))?;
    written(py, &translation, returnMeasures)
}

/// The same written as the interval and the change of speed: music21's
/// `translateIntervalsAndSpeed`.
#[pyfunction]
#[pyo3(signature = (inputStreamOrIterator, returnMeasures = false))]
fn translateIntervalsAndSpeed<'py>(
    py: Python<'py>,
    inputStreamOrIterator: &Bound<'py, PyAny>,
    returnMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let handed = Handed::of(inputStreamOrIterator)?;
    let translation =
        translate_intervals_and_speed(&handed.searched()).map_err(|error| search_error(&error))?;
    written(py, &translation, returnMeasures)
}

/// The pitches alone: music21's `translateStreamToStringNoRhythm`.
#[pyfunction]
#[pyo3(signature = (inputStreamOrIterator, returnMeasures = false))]
fn translateStreamToStringNoRhythm<'py>(
    py: Python<'py>,
    inputStreamOrIterator: &Bound<'py, PyAny>,
    returnMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let handed = Handed::of(inputStreamOrIterator)?;
    written(
        py,
        &translate_stream_to_string_no_rhythm(&handed.searched()),
        returnMeasures,
    )
}

/// The lengths alone: music21's `translateStreamToStringOnlyRhythm`.
#[pyfunction]
#[pyo3(signature = (inputStreamOrIterator, returnMeasures = false))]
fn translateStreamToStringOnlyRhythm<'py>(
    py: Python<'py>,
    inputStreamOrIterator: &Bound<'py, PyAny>,
    returnMeasures: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let handed = Handed::of(inputStreamOrIterator)?;
    written(
        py,
        &translate_stream_to_string_only_rhythm(&handed.searched()),
        returnMeasures,
    )
}

/// One element read as the value it is, for the functions that take one.
fn one(element: &Bound<'_, PyAny>) -> PyResult<StreamElement> {
    crate::stream::element_read(element)?.ok_or_else(|| {
        SearchException::new_err(format!(
            "{} is not an element this package has a value for",
            element
                .repr()
                .map(|repr| repr.to_string())
                .unwrap_or_default()
        ))
    })
}

/// A note's pitch as one character: music21's `translateNoteToByte`.
#[pyfunction]
fn translateNoteToByte(n: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(translate_note_to_byte(&one(n)?).to_string())
}

/// A note's pitch, length and tie: music21's
/// `translateNoteWithDurationToBytes`.
#[pyfunction]
#[pyo3(signature = (n, includeTieByte = true))]
fn translateNoteWithDurationToBytes(
    n: &Bound<'_, PyAny>,
    includeTieByte: bool,
) -> PyResult<String> {
    Ok(translate_note_with_duration_to_bytes(
        &one(n)?,
        includeTieByte,
    ))
}

/// A note's tie as one character: music21's `translateNoteTieToByte`.
#[pyfunction]
fn translateNoteTieToByte(n: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(translate_note_tie_to_byte(&one(n)?).to_string())
}

/// A note's length as one character: music21's `translateDurationToBytes`.
#[pyfunction]
fn translateDurationToBytes(n: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(translate_duration_to_bytes(&one(n)?).to_string())
}

/// Every measure a stream holds, in the order music21's own
/// `streamIn[Measure]` hands them over, which is the order the crate's
/// walk meets them.
fn measures_of<'py>(streamIn: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let walked = streamIn.call_method0("recurse")?;
    let mut out = Vec::new();
    for measure in walked
        .call_method1("getElementsByClass", ("Measure",))?
        .walk()?
    {
        out.push(measure?);
    }
    Ok(out)
}

/// A copy of a measure with its notes moved so that the lowest pitch of
/// its first note or chord sounds C5, which is the rhythm music21 keeps
/// beside each count -- the measure itself where it has no pitch to move.
fn transposed_to_c5<'py>(measure: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = measure.py();
    for sounded in measure.getattr("notes")?.walk()? {
        let sounded = sounded?;
        let mut lowest: Option<FloatType> = None;
        for pitch in sounded.getattr("pitches")?.walk()? {
            let sounds: FloatType = pitch?.getattr("ps")?.extract()?;
            lowest = Some(lowest.map_or(sounds, |known: FloatType| known.min(sounds)));
        }
        // An unpitched stroke has no pitch to move, and music21 passes it
        // over looking for the note it transposes by.
        let Some(lowest) = lowest else {
            continue;
        };
        let distance = 72.0 - lowest;
        let copied = py.import("copy")?.getattr("deepcopy")?.call1((measure,))?;
        for element in copied.getattr("notes")?.walk()? {
            let element = element?;
            if element
                .getattr("pitches")
                .and_then(|pitches| pitches.len())
                .unwrap_or(0)
                == 0
            {
                continue;
            }
            let keywords = PyDict::new(py);
            keywords.set_item("inPlace", true)?;
            element.call_method("transpose", (distance,), Some(&keywords))?;
        }
        return Ok(copied);
    }
    Ok(measure.clone())
}

/// music21's `mostCommonMeasureRhythms`: each rhythm a stream's measures
/// share, most common first, with how many have it, which they are, and a
/// copy of the first moved to start on C5.
///
/// music21's `transposeDiatonic` is taken and does nothing, as it does
/// there: its own code carries a note to that effect and no branch reads
/// the flag.
#[pyfunction]
#[pyo3(signature = (streamIn, transposeDiatonic = false))]
fn mostCommonMeasureRhythms<'py>(
    py: Python<'py>,
    streamIn: &Bound<'py, PyAny>,
    transposeDiatonic: bool,
) -> PyResult<Bound<'py, PyList>> {
    let _ = transposeDiatonic;
    let (stream, _) = crate::stream::read_with_leaves(streamIn)?;
    let measures = measures_of(streamIn)?;
    let out = PyList::empty(py);
    for found in most_common_measure_rhythm_places(&stream) {
        let held = PyList::empty(py);
        for at in &found.measures {
            let Some(measure) = measures.get(*at) else {
                return Err(SearchException::new_err(
                    "a rhythm was counted in a measure the stream does not hold",
                ));
            };
            held.append(measure)?;
        }
        let made = PyDict::new(py);
        made.set_item("number", found.count)?;
        made.set_item("rhythmString", &found.rhythm)?;
        made.set_item("rhythm", transposed_to_c5(&held.get_item(0)?)?)?;
        made.set_item("measures", held)?;
        out.append(made)?;
    }
    Ok(out)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<StreamSearcher>()?;
    m.add_function(wrap_pyfunction!(streamSearchBase, m)?)?;
    m.add_function(wrap_pyfunction!(rhythmicSearch, m)?)?;
    m.add_function(wrap_pyfunction!(noteNameSearch, m)?)?;
    m.add_function(wrap_pyfunction!(noteNameRhythmicSearch, m)?)?;
    m.add_function(wrap_pyfunction!(approximateNoteSearch, m)?)?;
    m.add_function(wrap_pyfunction!(approximateNoteSearchNoRhythm, m)?)?;
    m.add_function(wrap_pyfunction!(approximateNoteSearchOnlyRhythm, m)?)?;
    m.add_function(wrap_pyfunction!(approximateNoteSearchWeighted, m)?)?;
    m.add_function(wrap_pyfunction!(translateStreamToString, m)?)?;
    m.add_function(wrap_pyfunction!(translateDiatonicStreamToString, m)?)?;
    m.add_function(wrap_pyfunction!(translateIntervalsAndSpeed, m)?)?;
    m.add_function(wrap_pyfunction!(translateStreamToStringNoRhythm, m)?)?;
    m.add_function(wrap_pyfunction!(translateStreamToStringOnlyRhythm, m)?)?;
    m.add_function(wrap_pyfunction!(translateNoteToByte, m)?)?;
    m.add_function(wrap_pyfunction!(translateNoteWithDurationToBytes, m)?)?;
    m.add_function(wrap_pyfunction!(translateNoteTieToByte, m)?)?;
    m.add_function(wrap_pyfunction!(translateDurationToBytes, m)?)?;
    m.add_function(wrap_pyfunction!(mostCommonMeasureRhythms, m)?)?;
    Ok(())
}
