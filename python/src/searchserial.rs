//! music21's `search.serial`: the runs of a stream's notes and chords that
//! are the segments or sets of pitch classes searched for, and the stream
//! with each marked.
//!
//! The searching is the crate's -- `search::ContiguousSegmentSearcher` for
//! every run of a length and `search::SegmentMatcher` for the runs that
//! match, one matcher taking `Matching` where music21 has five subclasses
//! -- and `search_serial_parity` holds both to music21 on built streams
//! and corpus scores. What the facade does is read the stream in and carry
//! the answer back onto the stream's own notes: a run found *is* the
//! score's notes, which music21's own examples print, and that is what the
//! searcher's places are for -- each note of a run says where it stands
//! among the leaves of its part.
//!
//! A found run reads everything else off those notes, as music21's own
//! does: the measure it starts in, where it starts, and the pitch classes
//! it sounds, each asked of the note objects themselves. What is normalized
//! -- the pitch classes a run was *read* as -- is the crate's `ToneRow`
//! answering music21's three shapes: the classes themselves, the intervals
//! they make as a string, and the row they are.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21_rs_crate::IntegerType;
use music21_rs_crate::search::{
    ContiguousSegmentSearcher as RsSearcher, Matching, Repetitions, SegmentMatcher as RsMatcher,
};
use music21_rs_crate::stream::Stream as RsStream;

use crate::Walkable;

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "ContiguousSegmentOfNotes",
    "ContiguousSegmentSearcher",
    "SegmentMatcher",
    "TransposedSegmentMatcher",
    "TransformedSegmentMatcher",
    "MultisetSegmentMatcher",
    "TransposedMultisetMatcher",
    "TransposedInvertedMultisetMatcher",
    "labelSegments",
    "labelTransposedSegments",
    "labelTransformedSegments",
    "labelMultisets",
    "labelTransposedMultisets",
    "labelTransposedAndInvertedMultisets",
];

/// How music21 names each way of reading repeated pitch classes.
fn repetitions_of(reps: &str) -> PyResult<Repetitions> {
    Ok(match reps {
        "skipConsecutive" => Repetitions::SkipConsecutive,
        "rowsOnly" => Repetitions::RowsOnly,
        "includeAll" => Repetitions::IncludeAll,
        "ignoreAll" => Repetitions::IgnoreAll,
        _ => {
            return Err(crate::stream::StreamException::new_err(format!(
                "{reps} is not a way of reading repetitions"
            )));
        }
    })
}

/// The pitch classes a segment searched for is written as, which music21
/// reads through `pcToToneRow`: a list of numbers, of names, or a row.
fn classes_of(segment: &Bound<'_, PyAny>) -> PyResult<Vec<IntegerType>> {
    Ok(crate::serial::row_from_any(Some(segment))?
        .pitch_classes()
        .iter()
        .map(|class| IntegerType::from(*class))
        .collect())
}

/// Every segment a search was given, each as the pitch classes the crate
/// searches by and as the list it was written as -- which is what music21
/// hands back in `matchedSegment`, numbers and names as the caller wrote
/// them.
///
/// music21 takes a single segment as well as a list of them.
fn written_segments(searchList: &Bound<'_, PyAny>) -> PyResult<Vec<(Vec<IntegerType>, Py<PyAny>)>> {
    let py = searchList.py();
    let given: Vec<Bound<'_, PyAny>> = searchList.walk()?.collect::<PyResult<_>>()?;
    let nested = given.first().is_some_and(|first| {
        first.try_iter().is_ok() && first.extract::<Bound<'_, pyo3::types::PyString>>().is_err()
    });
    let written = |segment: &Bound<'_, PyAny>| -> PyResult<(Vec<IntegerType>, Py<PyAny>)> {
        let listed = PyList::empty(py);
        for part in segment.walk()? {
            listed.append(part?)?;
        }
        Ok((classes_of(segment)?, listed.into_any().unbind()))
    };
    if !nested {
        if given.is_empty() {
            return Ok(Vec::new());
        }
        // A single segment, as music21 reads one.
        return Ok(vec![written(searchList)?]);
    }
    given.iter().map(written).collect()
}

/// A stream read for a serial search: the crate's reading of it, the
/// objects at each of its leaves, and the objects at each leaf of each of
/// its parts -- which is what a run's places count by.
struct Searched {
    stream: RsStream,
    whole: Vec<Py<PyAny>>,
    parts: Vec<Vec<Py<PyAny>>>,
}

impl Searched {
    fn of(stream: &Bound<'_, PyAny>) -> PyResult<Self> {
        let (read, whole) = crate::stream::read_with_leaves(stream)?;
        let mut parts = Vec::new();
        // The parts in the order the crate's walk meets them, which is the
        // order music21's own recursion hands them over.
        if let Ok(walked) = stream.call_method0("recurse")
            && let Ok(found) = walked.call_method1("getElementsByClass", ("Part",))
        {
            for part in found.walk()? {
                let (_, leaves) = crate::stream::read_with_leaves(&part?)?;
                parts.push(leaves);
            }
        }
        Ok(Self {
            stream: read,
            whole,
            parts,
        })
    }

    /// The object a run's note stands for: the leaf of its part, or of the
    /// stream searched where it has no parts.
    fn object(&self, py: Python<'_>, part: Option<usize>, leaf: usize) -> PyResult<Py<PyAny>> {
        let leaves = match part {
            Some(part) => self.parts.get(part),
            None => Some(&self.whole),
        };
        leaves
            .and_then(|leaves| leaves.get(leaf))
            .map(|object| object.clone_ref(py))
            .ok_or_else(|| {
                crate::stream::StreamException::new_err(
                    "a run was found on a note the stream does not hold",
                )
            })
    }
}

/// An empty stream of music21's where music21 can be asked, and of this
/// package's otherwise: what a run with no container is given.
fn empty_stream(py: Python<'_>) -> PyResult<Py<PyAny>> {
    if let Ok(made) = py
        .import("music21.stream")
        .and_then(|module| module.getattr("Stream"))
        .and_then(|class| class.call0())
    {
        return Ok(made.unbind());
    }
    Ok(py
        .get_type::<crate::stream::Stream>()
        .call0()?
        .unbind()
        .into_any())
}

/// music21's `search.serial.ContiguousSegmentOfNotes`: a run of a stream's
/// notes and chords, which reads everything it says off those notes.
#[pyclass(
    name = "ContiguousSegmentOfNotes",
    module = "music21.search.serial",
    subclass,
    skip_from_py_object
)]
pub struct ContiguousSegmentOfNotes {
    /// The notes and chords of the run, in order.
    #[pyo3(get, set)]
    segment: Py<PyAny>,
    /// The stream it was drawn from, an empty one where it was given none.
    #[pyo3(get, set)]
    containerStream: Py<PyAny>,
    /// The part it is in, counted from nought, or nothing.
    #[pyo3(get, set)]
    partNumber: Py<PyAny>,
    /// The pitch classes a search read the run as, normalized its way.
    #[pyo3(get, set)]
    activeSegment: Py<PyAny>,
    /// What it was matched to, where it was matched at all.
    #[pyo3(get, set)]
    matchedSegment: Py<PyAny>,
}

impl ContiguousSegmentOfNotes {
    /// The notes themselves.
    fn notes<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        self.segment.bind(py).walk()?.collect()
    }

    /// The two rows music21 compares: the run as it was read and the
    /// segment it was matched to, each as a `ToneRow`.
    fn rows<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let row = |value: &Py<PyAny>| -> PyResult<Bound<'py, PyAny>> {
            let value = value.bind(py);
            if value.getattr("pitchClasses").is_ok() {
                return Ok(value.clone());
            }
            Ok(
                crate::serial::ToneRow::object(py, crate::serial::row_from_any(Some(value))?)?
                    .into_bound(py),
            )
        };
        Ok((row(&self.activeSegment)?, row(&self.matchedSegment)?))
    }
}

#[pymethods]
impl ContiguousSegmentOfNotes {
    /// music21's `ContiguousSegmentOfNotes(segment=None,
    /// containerStream=None, partNumber=0)`.
    #[new]
    #[pyo3(signature = (segment = None, containerStream = None, partNumber = None))]
    fn new(
        py: Python<'_>,
        segment: Option<&Bound<'_, PyAny>>,
        containerStream: Option<&Bound<'_, PyAny>>,
        partNumber: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let held = PyList::empty(py);
        if let Some(segment) = segment.filter(|given| !given.is_none()) {
            for note in segment.walk()? {
                held.append(note?)?;
            }
        }
        // Every run must have a container stream, so one with none is
        // given an empty one rather than left without.
        let containerStream = match containerStream.filter(|given| !given.is_none()) {
            Some(given) => given.clone().unbind(),
            None => empty_stream(py)?,
        };
        Ok(Self {
            segment: held.into_any().unbind(),
            containerStream,
            partNumber: match partNumber {
                Some(given) => given.clone().unbind(),
                None => 0i64.into_pyobject(py)?.into_any().unbind(),
            },
            activeSegment: PyList::empty(py).into_any().unbind(),
            matchedSegment: py.None(),
        })
    }

    /// The measure the run starts in: music21's `startMeasureNumber`,
    /// which is the first note's own -- and nothing where there is no
    /// first note, or where it is one of this package's, which does not
    /// know what holds it.
    #[getter]
    fn startMeasureNumber<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self
            .notes(py)?
            .first()
            .and_then(|first| first.getattr("measureNumber").ok())
            .unwrap_or_else(|| py.None().into_bound(py)))
    }

    /// Where it starts in the stream holding its first note: music21's
    /// `startOffset`, and nothing for the same two reasons.
    #[getter]
    fn startOffset<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self
            .notes(py)?
            .first()
            .and_then(|first| first.getattr("offset").ok())
            .unwrap_or_else(|| py.None().into_bound(py)))
    }

    /// The run as it was read and the segment it was matched to, as rows:
    /// music21's `getActiveMatchedRows`.
    fn getActiveMatchedRows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let (active, matched) = self.rows(py)?;
        PyTuple::new(py, [active, matched])
    }

    /// The zero-centred transformations taking the segment matched to the
    /// run: music21's `zeroCenteredTransformationsFromMatched`.
    #[getter]
    fn zeroCenteredTransformationsFromMatched<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (active, matched) = self.rows(py)?;
        matched.call_method1("findZeroCenteredTransformations", (active,))
    }

    /// The original-centred transformations: music21's
    /// `originalCenteredTransformationsFromMatched`.
    #[getter]
    fn originalCenteredTransformationsFromMatched<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (active, matched) = self.rows(py)?;
        matched.call_method1("findOriginalCenteredTransformations", (active,))
    }

    /// Every pitch class the run sounds, each chord from its bottom:
    /// music21's `readPitchClassesFromBottom`, which asks each pitch what
    /// class it is.
    fn readPitchClassesFromBottom<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for note in self.notes(py)? {
            for pitch in note.getattr("pitches")?.walk()? {
                out.append(pitch?.getattr("pitchClass")?)?;
            }
        }
        Ok(out)
    }

    /// The same, each class once and in the order first met: music21's
    /// `getDistinctPitchClasses`.
    fn getDistinctPitchClasses<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        for class in self.readPitchClassesFromBottom(py)? {
            if !out.contains(&class)? {
                out.append(class)?;
            }
        }
        Ok(out)
    }

    /// How many notes and chords are in the run.
    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        Ok(self.notes(py)?.len())
    }

    /// music21's own `repr`, which lists the pitches of each note and
    /// chord of the run.
    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let listed = PyList::empty(py);
        for note in slf.borrow().notes(py)? {
            let mut pitches = Vec::new();
            for pitch in note.getattr("pitches")?.walk()? {
                pitches.push(pitch?.str()?.to_string());
            }
            listed.append(pitches.join(" "))?;
        }
        Ok(format!(
            "<music21.search.serial.ContiguousSegmentOfNotes {}>",
            listed.str()?
        ))
    }
}

/// The run objects a search found, each on the stream's own notes.
fn found_runs<'py>(
    py: Python<'py>,
    searched: &Searched,
    container: &Bound<'py, PyAny>,
    runs: &[music21_rs_crate::search::ContiguousSegment<'_>],
    matching: Option<Matching>,
    given: &[(Vec<IntegerType>, Py<PyAny>)],
) -> PyResult<Bound<'py, PyList>> {
    let out = PyList::empty(py);
    for run in runs {
        let notes = PyList::empty(py);
        for note in &run.notes {
            notes.append(searched.object(py, run.part, note.leaf)?)?;
        }
        let part = match run.part {
            Some(part) => part.into_pyobject(py)?.into_any(),
            None => py.None().into_bound(py),
        };
        let made = crate::installed_new(
            py,
            "music21.search.serial",
            "ContiguousSegmentOfNotes",
            ContiguousSegmentOfNotes {
                segment: notes.into_any().unbind(),
                containerStream: container.clone().unbind(),
                partNumber: part.unbind(),
                activeSegment: normalized(py, &run.active, matching)?,
                // music21 keeps the segment *as it was written*, names
                // and negative numbers and all, not the pitch classes it
                // read from it.
                matchedSegment: match &run.matched {
                    Some(matched) => given
                        .iter()
                        .find(|(classes, _)| classes == matched)
                        .map(|(_, written)| written.clone_ref(py))
                        .unwrap_or_else(|| py.None()),
                    None => py.None(),
                },
            },
        )?;
        out.append(made)?;
    }
    Ok(out)
}

/// The pitch classes a run was read as, written the way the matcher that
/// read them writes one: the classes themselves, the intervals they make
/// as a string, or the row they are. music21's `normalize` of the run.
fn normalized(py: Python<'_>, active: &[u8], matching: Option<Matching>) -> PyResult<Py<PyAny>> {
    let classes: Vec<IntegerType> = active
        .iter()
        .map(|class| IntegerType::from(*class))
        .collect();
    let row = music21_rs_crate::serial::ToneRow::new(classes.iter().copied());
    Ok(match matching {
        Some(Matching::Transposed) => pyo3::types::PyString::new(py, &row.intervals_as_string())
            .into_any()
            .unbind(),
        Some(Matching::Transformed) => crate::serial::ToneRow::object(py, row)?,
        _ => {
            let listed = PyList::empty(py);
            for class in classes {
                listed.append(class)?;
            }
            listed.into_any().unbind()
        }
    })
}

/// A segment as a matcher of this kind reads one before comparing:
/// music21's `normalize`, which is the classes themselves, the intervals
/// they make as a string, or the row they are.
fn normalize_as(
    py: Python<'_>,
    matching: Matching,
    segment: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let classes = classes_of(segment)?;
    let row = music21_rs_crate::serial::ToneRow::new(classes.iter().copied());
    match matching {
        Matching::Transposed => as_intervals(py, row),
        Matching::Transformed => as_row(py, row),
        _ => as_classes(py, row),
    }
}

/// The pitch classes a reading was made from, where they can be read back:
/// a list of numbers as it stands and a row by its own classes. An
/// interval string says nothing of the classes, and a matcher that reads
/// one compares the strings themselves.
fn classes_read(value: &Bound<'_, PyAny>) -> PyResult<Option<Vec<IntegerType>>> {
    if value.extract::<Bound<'_, pyo3::types::PyString>>().is_ok() {
        return Ok(None);
    }
    Ok(Some(classes_of(value)?))
}

/// Whether a matcher of this kind tells two readings the same: music21's
/// `equalSubset`, which for a row is whether a transformation takes one to
/// the other and for an interval string is the strings themselves.
fn equal_readings(
    matching: Matching,
    search: &Bound<'_, PyAny>,
    subset: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    match (classes_read(search)?, classes_read(subset)?) {
        (Some(search), Some(subset)) => Ok(matching.equal_segments(&search, &subset)),
        _ => search.eq(subset),
    }
}

/// music21's `search.serial.ContiguousSegmentSearcher`: every run of a
/// given length in a stream.
#[pyclass(
    name = "ContiguousSegmentSearcher",
    module = "music21.search.serial",
    subclass,
    skip_from_py_object
)]
pub struct ContiguousSegmentSearcher {
    /// The stream searched: music21 calls it `stream`, not `inputStream`.
    #[pyo3(get, set)]
    stream: Py<PyAny>,
    /// How repeated pitch classes are read.
    #[pyo3(get, set)]
    reps: String,
    /// Whether a run may hold a chord.
    #[pyo3(get, set)]
    includeChords: bool,
    /// How long a run is looked for, which `byLength` sets.
    #[pyo3(get, set)]
    searchLength: usize,
    /// music21's flag for cutting the search short, which changes no
    /// answer.
    #[pyo3(get, set)]
    trimToShortestLengthFast: bool,
}

#[pymethods]
impl ContiguousSegmentSearcher {
    /// music21's `ContiguousSegmentSearcher(inputStream=None,
    /// reps='skipConsecutive', includeChords=True)`.
    #[new]
    #[pyo3(signature = (inputStream = None, reps = "skipConsecutive".to_string(), includeChords = true))]
    fn new(
        py: Python<'_>,
        inputStream: Option<&Bound<'_, PyAny>>,
        reps: String,
        #[pyo3(from_py_with = crate::truthy)] includeChords: bool,
    ) -> Self {
        Self {
            stream: match inputStream.filter(|given| !given.is_none()) {
                Some(given) => given.clone().unbind(),
                None => py.None(),
            },
            reps,
            includeChords,
            searchLength: 1,
            trimToShortestLengthFast: false,
        }
    }

    /// Every run of this many notes: music21's `byLength`.
    fn byLength<'py>(slf: &Bound<'py, Self>, length: usize) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        slf.borrow_mut().searchLength = length;
        let (stream, reps, chords) = {
            let me = slf.borrow();
            (
                me.stream.bind(py).clone(),
                me.reps.clone(),
                me.includeChords,
            )
        };
        let searched = Searched::of(&stream)?;
        let searcher = RsSearcher::new(repetitions_of(&reps)?, chords);
        let runs = searcher
            .by_length(&searched.stream, length)
            .map_err(|error| crate::stream::stream_error(&error))?;
        found_runs(py, &searched, &stream, &runs, None, &[])
    }
}

/// music21's `search.serial.SegmentMatcher`, and the five matchers built
/// on it, which are one `Matching` each.
#[pyclass(
    name = "SegmentMatcher",
    module = "music21.search.serial",
    subclass,
    skip_from_py_object
)]
pub struct SegmentMatcher {
    /// How a run is told the same as a segment searched for.
    matching: Matching,
    /// The stream searched.
    #[pyo3(get, set)]
    stream: Py<PyAny>,
    /// How repeated pitch classes are read.
    #[pyo3(get, set)]
    reps: String,
    /// Whether a run may hold a chord.
    #[pyo3(get, set)]
    includeChords: bool,
    /// The runs the last search matched.
    #[pyo3(get, set)]
    matchedSegments: Py<PyAny>,
    /// The segments searched for already, each as this matcher reads one:
    /// music21's `searchedAlready`, which `checkSearchedAlready` keeps.
    #[pyo3(get, set)]
    searchedAlready: Py<PyAny>,
}

impl SegmentMatcher {
    fn of(
        py: Python<'_>,
        matching: Matching,
        inputStream: Option<&Bound<'_, PyAny>>,
        reps: String,
        includeChords: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            matching,
            stream: match inputStream.filter(|given| !given.is_none()) {
                Some(given) => given.clone().unbind(),
                None => py.None(),
            },
            reps,
            includeChords,
            matchedSegments: PyList::empty(py).into_any().unbind(),
            searchedAlready: PyList::empty(py).into_any().unbind(),
        })
    }

    /// The runs of a stream that match, as music21's `find` answers them.
    fn matched<'py>(
        slf: &Bound<'py, Self>,
        searchList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let (matching, stream, reps, chords) = {
            let me = slf.borrow();
            (
                me.matching,
                me.stream.bind(py).clone(),
                me.reps.clone(),
                me.includeChords,
            )
        };
        let given = written_segments(searchList)?;
        let search: Vec<Vec<IntegerType>> =
            given.iter().map(|(classes, _)| classes.clone()).collect();
        if search.is_empty() {
            return Ok(PyList::empty(py));
        }
        // music21 starts a search with nothing searched for yet, and notes
        // each segment as it goes.
        slf.borrow_mut().searchedAlready = PyList::empty(py).into_any().unbind();
        for (_, segment) in &given {
            Self::checkSearchedAlready(slf, segment.bind(py))?;
        }
        let searched = Searched::of(&stream)?;
        let matcher = RsMatcher::new(matching, repetitions_of(&reps)?, chords);
        let runs = matcher
            .find(&searched.stream, &search)
            .map_err(|error| crate::stream::stream_error(&error))?;
        let found = found_runs(py, &searched, &stream, &runs, Some(matching), &given)?;
        slf.borrow_mut().matchedSegments = found.clone().into_any().unbind();
        Ok(found)
    }
}

#[pymethods]
impl SegmentMatcher {
    /// music21's `SegmentMatcher(inputStream, reps='skipConsecutive',
    /// includeChords=True)`.
    #[new]
    #[pyo3(signature = (inputStream = None, reps = "skipConsecutive".to_string(), includeChords = true))]
    fn new(
        py: Python<'_>,
        inputStream: Option<&Bound<'_, PyAny>>,
        reps: String,
        #[pyo3(from_py_with = crate::truthy)] includeChords: bool,
    ) -> PyResult<Self> {
        Self::of(py, Matching::Exact, inputStream, reps, includeChords)
    }

    /// Whether a set matched more than once over is listed each time:
    /// music21's `includeMultisetDuplicates`.
    #[classattr]
    fn includeMultisetDuplicates() -> bool {
        false
    }

    /// The runs that match one of the segments searched for: music21's
    /// `find`.
    fn find<'py>(
        slf: &Bound<'py, Self>,
        searchList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        Self::matched(slf, searchList)
    }

    /// A segment as this matcher reads one before comparing: music21's
    /// `normalize`, the pitch classes themselves.
    #[staticmethod]
    fn normalize(py: Python<'_>, segment: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let classes = classes_of(segment)?;
        let listed = PyList::empty(py);
        for class in classes {
            listed.append(class)?;
        }
        Ok(listed.into_any().unbind())
    }

    /// Whether this segment has been searched for already, noting it if
    /// not: music21's `checkSearchedAlready`, which is how a search passes
    /// over a segment it has looked for -- for a transformed or a
    /// transposed matcher, one it has looked for in another form.
    fn checkSearchedAlready(slf: &Bound<'_, Self>, segment: &Bound<'_, PyAny>) -> PyResult<bool> {
        let py = slf.py();
        let matching = slf.borrow().matching;
        let read = normalize_as(py, matching, segment)?.into_bound(py);
        let searched = slf.borrow().searchedAlready.bind(py).clone();
        for used in searched.walk()? {
            if equal_readings(matching, &read, &used?)? {
                return Ok(true);
            }
        }
        searched.call_method1("append", (read,))?;
        Ok(false)
    }

    /// Whether the run read one way is the segment read the other:
    /// music21's `equalSubset`.
    fn equalSubset(
        slf: &Bound<'_, Self>,
        searchSegment: &Bound<'_, PyAny>,
        subsetToCheck: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        equal_readings(slf.borrow().matching, searchSegment, subsetToCheck)
    }
}

/// The five matchers music21 writes as subclasses, each one `Matching`.
macro_rules! matchers {
    ($(($class:ident, $name:literal, $matching:expr, $duplicates:literal, $shape:ident, {$($extra:item)*})),* $(,)?) => {
        $(
            #[pyclass(
                name = $name,
                module = "music21.search.serial",
                extends = SegmentMatcher,
                subclass,
                skip_from_py_object
            )]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (inputStream = None, reps = "skipConsecutive".to_string(), includeChords = true))]
                fn new(
                    py: Python<'_>,
                    inputStream: Option<&Bound<'_, PyAny>>,
                    reps: String,
                    #[pyo3(from_py_with = crate::truthy)] includeChords: bool,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(
                        PyClassInitializer::from(SegmentMatcher::of(
                            py,
                            $matching,
                            inputStream,
                            reps,
                            includeChords,
                        )?)
                        .add_subclass(Self),
                    )
                }

                /// Whether a set matched more than once over is listed
                /// each time: music21's `includeMultisetDuplicates`.
                #[classattr]
                fn includeMultisetDuplicates() -> bool {
                    $duplicates
                }

                /// A segment as this matcher reads one before comparing:
                /// music21's `normalize`.
                #[staticmethod]
                fn normalize(py: Python<'_>, segment: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                    let classes = classes_of(segment)?;
                    let row = music21_rs_crate::serial::ToneRow::new(classes.iter().copied());
                    $shape(py, row)
                }

                $($extra)*
            }
        )*
    };
}

/// The pitch classes as a list: what music21's plain matcher normalizes to.
fn as_classes(py: Python<'_>, row: music21_rs_crate::serial::ToneRow) -> PyResult<Py<PyAny>> {
    let listed = PyList::empty(py);
    for class in row.pitch_classes() {
        listed.append(*class)?;
    }
    Ok(listed.into_any().unbind())
}

/// The intervals the classes make, written as music21 writes them.
fn as_intervals(py: Python<'_>, row: music21_rs_crate::serial::ToneRow) -> PyResult<Py<PyAny>> {
    Ok(pyo3::types::PyString::new(py, &row.intervals_as_string())
        .into_any()
        .unbind())
}

/// The row the classes are.
fn as_row(py: Python<'_>, row: music21_rs_crate::serial::ToneRow) -> PyResult<Py<PyAny>> {
    crate::serial::ToneRow::object(py, row)
}

matchers![
    (
        TransposedSegmentMatcher,
        "TransposedSegmentMatcher",
        Matching::Transposed,
        false,
        as_intervals,
        {}
    ),
    (
        TransformedSegmentMatcher,
        "TransformedSegmentMatcher",
        Matching::Transformed,
        false,
        as_row,
        {
            /// The transformations taking the first row to the second:
            /// music21's `getTransformations`, which is the crate's row
            /// answering.
            #[staticmethod]
            fn getTransformations<'py>(
                row1: &Bound<'py, PyAny>,
                row2: &Bound<'py, PyAny>,
            ) -> PyResult<Bound<'py, PyAny>> {
                row2.call_method1("findZeroCenteredTransformations", (row1,))
            }
        }
    ),
    (
        MultisetSegmentMatcher,
        "MultisetSegmentMatcher",
        Matching::Multiset,
        true,
        as_classes,
        {}
    ),
    (
        TransposedMultisetMatcher,
        "TransposedMultisetMatcher",
        Matching::TransposedMultiset,
        true,
        as_classes,
        {}
    ),
    (
        TransposedInvertedMultisetMatcher,
        "TransposedInvertedMultisetMatcher",
        Matching::TransposedInvertedMultiset,
        true,
        as_classes,
        {}
    ),
];

/// Which transformations a labelling writes beside each name, where it
/// writes any.
#[derive(Clone, Copy)]
enum Convention {
    None,
    Zero,
    Original,
}

/// Marks every run found in a copy of the stream, as music21's labelling
/// functions mark them: a line from the run's first note to its last, in
/// its part, and the name of the segment it matched as a lyric of the
/// first note.
///
/// Which runs are found, in which order they are marked and what each name
/// comes to are the crate's (`SegmentMatcher::label_places`); the line and
/// the lyric are music21's own objects, written here.
fn labelled<'py>(
    py: Python<'py>,
    inputStream: &Bound<'py, PyAny>,
    segmentDict: &Bound<'py, PyDict>,
    reps: &str,
    includeChords: bool,
    matching: Matching,
    convention: Convention,
) -> PyResult<Bound<'py, PyAny>> {
    use music21_rs_crate::serial::TransformationConvention;

    let copied = py
        .import("copy")?
        .getattr("deepcopy")?
        .call1((inputStream,))?;
    let mut named = Vec::new();
    for (label, segment) in segmentDict.iter() {
        named.push((label.str()?.to_string(), classes_of(&segment)?));
    }
    let searched = Searched::of(&copied)?;
    let matcher = RsMatcher::new(matching, repetitions_of(reps)?, includeChords);
    let convention = match convention {
        Convention::None => None,
        Convention::Zero => Some(TransformationConvention::ZeroCentered),
        Convention::Original => Some(TransformationConvention::OriginalCentered),
    };
    let places = matcher
        .label_places(&searched.stream, &named, convention)
        .map_err(|error| crate::stream::stream_error(&error))?;

    // The part a line goes in, as music21 puts it there: the part the run
    // is in, or the stream searched where it has no parts.
    let parts: Vec<Bound<'py, PyAny>> = match copied.call_method0("recurse") {
        Ok(walked) => match walked.call_method1("getElementsByClass", ("Part",)) {
            Ok(found) => found.walk()?.collect::<PyResult<_>>()?,
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    };
    for place in places {
        let first = searched.object(py, place.part, place.first)?.into_bound(py);
        let last = searched.object(py, place.part, place.last)?.into_bound(py);
        // The line joining the run's ends: music21's own `spanner.Line`
        // wherever music21 is there to ask -- its own readers and writers
        // look for that class -- and this package's otherwise.
        //
        // "There to ask" means it will *hold these notes*, not merely that
        // it imports. music21's `Spanner.__init__` appends what is a
        // `Music21Object` and iterates whatever is not, so handing it a
        // note of this package's raised `'music21.note.Note' object is not
        // iterable`; the notes are only music21's where something is
        // installed over music21.
        let holds = crate::stream::music21_holds(&first);
        let line = match py
            .import("music21.spanner")
            .and_then(|module| module.getattr("Line"))
            .and_then(|class| {
                if holds {
                    Ok(class)
                } else {
                    Err(pyo3::exceptions::PyTypeError::new_err(
                        "music21 would not hold these notes",
                    ))
                }
            }) {
            Ok(class) => class.call1((&first, &last))?,
            Err(_) => crate::spanner::Spanner::object(
                py,
                &music21_rs_crate::Spanner::line(vec![place.first, place.last]),
                vec![first.clone().unbind(), last.clone().unbind()],
            )?
            .into_bound(py),
        };
        let holder = match place.part.and_then(|part| parts.get(part)) {
            Some(part) => part.clone(),
            None => copied.clone(),
        };
        holder.call_method1("insert", (0, line))?;
        if place.label.is_empty() {
            continue;
        }
        // music21 writes the name once: a note already singing it is left
        // alone.
        let mut sung = false;
        for lyric in first.getattr("lyrics")?.walk()? {
            if lyric?.getattr("text")?.str()?.to_string() == place.label {
                sung = true;
                break;
            }
        }
        if !sung {
            first.call_method1("addLyric", (&place.label,))?;
        }
    }
    Ok(copied)
}

/// The five labelling functions that write no transformation beside each
/// name, each one matcher.
macro_rules! labellers {
    ($(($function:ident, $matching:expr)),* $(,)?) => {
        $(
            /// music21's labelling function of this name: a copy of the
            /// stream with every run found marked.
            #[pyfunction]
            #[pyo3(signature = (inputStream, segmentDict, reps = "skipConsecutive".to_string(), includeChords = true))]
            fn $function<'py>(
                py: Python<'py>,
                inputStream: &Bound<'py, PyAny>,
                segmentDict: &Bound<'py, PyDict>,
                reps: String,
                includeChords: bool,
            ) -> PyResult<Bound<'py, PyAny>> {
                labelled(
                    py,
                    inputStream,
                    segmentDict,
                    &reps,
                    includeChords,
                    $matching,
                    Convention::None,
                )
            }
        )*
    };
}

labellers![
    (labelSegments, Matching::Exact),
    (labelTransposedSegments, Matching::Transposed),
    (labelMultisets, Matching::Multiset),
    (labelTransposedMultisets, Matching::TransposedMultiset),
    (
        labelTransposedAndInvertedMultisets,
        Matching::TransposedInvertedMultiset
    ),
];

/// music21's `labelTransformedSegments`, the one labelling that writes the
/// transformations taking each segment to the run beside its name, in
/// whichever of music21's two conventions is asked for.
#[pyfunction]
#[pyo3(signature = (inputStream, segmentDict, reps = "skipConsecutive".to_string(), includeChords = true, convention = "original".to_string()))]
fn labelTransformedSegments<'py>(
    py: Python<'py>,
    inputStream: &Bound<'py, PyAny>,
    segmentDict: &Bound<'py, PyDict>,
    reps: String,
    #[pyo3(from_py_with = crate::truthy)] includeChords: bool,
    convention: String,
) -> PyResult<Bound<'py, PyAny>> {
    let convention = match convention.as_str() {
        "original" => Convention::Original,
        "zero" => Convention::Zero,
        _ => {
            return Err(crate::stream::StreamException::new_err(format!(
                "{convention} is not a convention for naming transformations"
            )));
        }
    };
    labelled(
        py,
        inputStream,
        segmentDict,
        &reps,
        includeChords,
        Matching::Transformed,
        convention,
    )
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ContiguousSegmentOfNotes>()?;
    m.add_class::<ContiguousSegmentSearcher>()?;
    m.add_class::<SegmentMatcher>()?;
    m.add_class::<TransposedSegmentMatcher>()?;
    m.add_class::<TransformedSegmentMatcher>()?;
    m.add_class::<MultisetSegmentMatcher>()?;
    m.add_class::<TransposedMultisetMatcher>()?;
    m.add_class::<TransposedInvertedMultisetMatcher>()?;
    m.add_function(wrap_pyfunction!(labelSegments, m)?)?;
    m.add_function(wrap_pyfunction!(labelTransposedSegments, m)?)?;
    m.add_function(wrap_pyfunction!(labelTransformedSegments, m)?)?;
    m.add_function(wrap_pyfunction!(labelMultisets, m)?)?;
    m.add_function(wrap_pyfunction!(labelTransposedMultisets, m)?)?;
    m.add_function(wrap_pyfunction!(labelTransposedAndInvertedMultisets, m)?)?;
    Ok(())
}
