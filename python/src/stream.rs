//! music21's `stream` module, as far as the wheel needs one: a place to put
//! notes at offsets, and the five walks over such a place that music21 keeps
//! in other modules.
//!
//! A stream here holds the very objects it is handed, at offsets it keeps
//! itself, so what `realizeVolume` or `iterateAllVoiceLeadingQuartets` hands
//! back or changes is the object a caller put in. The walks themselves are
//! the crate's: each stream is read into a `music21_rs::Stream` beside the
//! list of objects standing at each of its positions
//! (`music21_rs::Stream::leaves`), the crate answers by position, and the
//! answer is carried back onto the objects.
//!
//! The walks read music21's own streams as readily as these, through the
//! same `elements` and `elementOffset` both answer to. An element the crate
//! has no value for -- a clef, an instrument, a text mark -- is read as a
//! stand-in that keeps its place, its length and whether it sorts ahead of a
//! note, which is all any of the walks asks of it.
//!
//! None of it is installed over music21. music21's `Stream` is the machinery
//! its whole library is built on, sites and contexts and derivations, and
//! nothing here stands in for that; the classes are the wheel's own, for a
//! caller with no music21 at all.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::{PyIndexError, PyTypeError};
use pyo3::prelude::*;

use crate::Walkable;
use pyo3::types::{PyDict, PyList, PySlice, PyTuple};

use music21_rs_crate::voiceleading::QuartetOptions;
use music21_rs_crate::{Stream as RsStream, StreamElement, StreamEvent, StreamKind};

use crate::chord::Chord;
use crate::dynamics::Dynamic;
use crate::harmony::ChordSymbol;
use crate::key::KeySignature;
use crate::meter::TimeSignature;
use crate::note::Note;
use crate::rest::Rest;
use crate::tempo::{MetronomeMark, TempoException};

pyo3::create_exception!(music21_rs_facade, StreamException, crate::Music21Exception);

/// One thing a stream holds: where it stands, music21's `classSortOrder`
/// for it, the order it was put in, and the object itself.
struct Held {
    offset: f64,
    order: i64,
    index: u64,
    object: Py<PyAny>,
}

/// music21's `stream.Stream`: objects at quarter-length offsets, kept in
/// order of offset, then of `classSortOrder`, then of when each was put in.
///
/// It holds the objects it is given and hands the same ones back. `Score`,
/// `Part`, `PartStaff`, `Measure`, `Voice` and `Opus` are the same class
/// under music21's other names; a score's parts are what
/// `iterateAllVoiceLeadingQuartets` reads as its voices.
#[pyclass(name = "Stream", module = "music21.stream", subclass)]
pub struct Stream {
    kind: StreamKind,
    held: Vec<Held>,
    next: u64,
    /// music21's `id`, where a caller gave one.
    id: Option<Py<PyAny>>,
}

/// music21's `classSortOrder` for an object, which decides what comes first
/// among things at one offset: 20 for one that says nothing.
fn sort_order(object: &Bound<'_, PyAny>) -> i64 {
    object
        .getattr("classSortOrder")
        .and_then(|order| order.extract::<i64>())
        .unwrap_or(20)
}

/// How long an object lasts: a stream as long as what it holds, anything
/// else as long as its duration, a thing with no duration no time at all.
fn length_of(object: &Bound<'_, PyAny>) -> PyResult<f64> {
    if let Ok(stream) = object.cast::<Stream>() {
        return Stream::highest_time(stream);
    }
    match object.getattr("duration") {
        Ok(duration) if !duration.is_none() => duration.getattr("quarterLength")?.extract(),
        _ => Ok(0.0),
    }
}

/// Whether an object is of a class, named or given: a name matches any
/// class in the object's lineage, or any name in music21's `classes`.
fn is_of_class(object: &Bound<'_, PyAny>, class: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Ok(name) = class.extract::<String>() {
        let lineage = object.get_type().getattr("__mro__")?;
        for each in lineage.walk()? {
            if each?.getattr("__name__")?.extract::<String>()? == name {
                return Ok(true);
            }
        }
        if let Ok(classes) = object.getattr("classes") {
            for each in classes.walk()? {
                if each?.extract::<String>().is_ok_and(|each| each == name) {
                    return Ok(true);
                }
            }
        }
        return Ok(false);
    }
    object.is_instance(class)
}

/// Whether an object is of any of a list of classes, or of the one given.
fn is_of_any(object: &Bound<'_, PyAny>, classes: &Bound<'_, PyAny>) -> PyResult<bool> {
    if classes.extract::<String>().is_ok() || classes.is_instance_of::<pyo3::types::PyType>() {
        return is_of_class(object, classes);
    }
    for class in classes.walk()? {
        if is_of_class(object, &class?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Stream {
    fn empty(kind: StreamKind) -> Self {
        Self {
            kind,
            held: Vec::new(),
            next: 0,
            id: None,
        }
    }

    /// A stream of a kind holding what music21's constructor would put in
    /// it: at their own offsets, unless every one stands at nought, when
    /// they follow one another -- except that parts or voices given together
    /// stand side by side, since those are music at one time.
    fn filled(
        py: Python<'_>,
        kind: StreamKind,
        given: Option<&Bound<'_, PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut stream = Self::empty(kind);
        if let Some(keywords) = keywords
            && let Some(id) = keywords.get_item("id")?
        {
            stream.id = Some(id.unbind());
        }
        let Some(given) = given.filter(|given| !given.is_none()) else {
            return Ok(stream);
        };
        let elements: Vec<Bound<'_, PyAny>> =
            if given.is_instance_of::<PyList>() || given.is_instance_of::<PyTuple>() {
                given.walk()?.collect::<PyResult<_>>()?
            } else {
                vec![given.clone()]
            };
        let behaviour: String = match keywords {
            Some(keywords) => match keywords.get_item("givenElementsBehavior")? {
                Some(value) => value.str()?.to_string().to_lowercase(),
                None => "offsets".to_string(),
            },
            None => "offsets".to_string(),
        };
        let offset_of = |element: &Bound<'_, PyAny>| -> f64 {
            element
                .getattr("offset")
                .and_then(|offset| offset.extract::<f64>())
                .unwrap_or(0.0)
        };
        let append = if behaviour.ends_with("insert") {
            false
        } else if behaviour.ends_with("append") {
            true
        } else {
            let all_at_nought = elements.iter().all(|element| offset_of(element) == 0.0);
            let side_by_side = elements.iter().all(|element| {
                element
                    .getattr("isStream")
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
                    && !is_of_class(element, &pyo3::types::PyString::new(py, "Measure"))
                        .unwrap_or(false)
                    && !is_of_class(element, &pyo3::types::PyString::new(py, "Score"))
                        .unwrap_or(false)
            });
            all_at_nought && !side_by_side
        };
        for element in &elements {
            let offset = if append {
                stream.end(py)?
            } else {
                offset_of(element)
            };
            stream.put(offset, element);
        }
        Ok(stream)
    }

    fn put(&mut self, offset: f64, object: &Bound<'_, PyAny>) {
        self.held.push(Held {
            offset,
            order: sort_order(object),
            index: self.next,
            object: object.clone().unbind(),
        });
        self.next += 1;
    }

    /// What this stream holds, in music21's order.
    fn sorted(&self) -> Vec<&Held> {
        let mut sorted: Vec<&Held> = self.held.iter().collect();
        sorted.sort_by(|left, right| {
            left.offset
                .total_cmp(&right.offset)
                .then(left.order.cmp(&right.order))
                .then(left.index.cmp(&right.index))
        });
        sorted
    }

    /// The offset just past the last thing held.
    fn end(&self, py: Python<'_>) -> PyResult<f64> {
        let mut end: f64 = 0.0;
        for held in &self.held {
            end = end.max(held.offset + length_of(held.object.bind(py))?);
        }
        Ok(end)
    }

    fn highest_time(slf: &Bound<'_, Self>) -> PyResult<f64> {
        slf.borrow().end(slf.py())
    }

    /// A stream of this one's class holding what is picked, at the offsets
    /// it stands at here.
    fn picked<'py>(
        slf: &Bound<'py, Self>,
        kind: Option<StreamKind>,
        mut keep: impl FnMut(&Bound<'py, PyAny>) -> PyResult<bool>,
    ) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let mut out = Self::empty(kind.unwrap_or(slf.borrow().kind));
        let chosen: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        for (offset, object) in chosen {
            let object = object.into_bound(py);
            if keep(&object)? {
                out.put(offset, &object);
            }
        }
        Bound::new(py, out)
    }

    fn flatten_into(
        slf: &Bound<'_, Self>,
        base: f64,
        out: &mut Vec<(f64, Py<PyAny>)>,
    ) -> PyResult<()> {
        let py = slf.py();
        let held: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        for (offset, object) in held {
            let object = object.into_bound(py);
            match object.cast::<Stream>() {
                Ok(inner) => Self::flatten_into(inner, base + offset, out)?,
                Err(_) => out.push((base + offset, object.unbind())),
            }
        }
        Ok(())
    }

    fn class_name(slf: &Bound<'_, Self>) -> PyResult<String> {
        slf.get_type().name()?.extract()
    }
}

#[pymethods]
impl Stream {
    #[new]
    #[pyo3(signature = (givenElements = None, *_arguments, **keywords))]
    fn new(
        py: Python<'_>,
        givenElements: Option<&Bound<'_, PyAny>>,
        _arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Self::filled(py, StreamKind::Stream, givenElements, keywords)
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for held in &self.held {
            visit.call(&held.object)?;
        }
        visit.call(&self.id)
    }

    fn __clear__(&mut self) {
        self.held.clear();
        self.id = None;
    }

    /// A stream is a stream.
    #[classattr]
    fn isStream() -> bool {
        true
    }

    /// music21's sort order for a stream among other things at one offset.
    #[classattr]
    fn classSortOrder() -> i64 {
        -20
    }

    #[getter]
    fn get_id(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.id.as_ref().map(|id| id.clone_ref(py))
    }

    #[setter]
    fn set_id(&mut self, value: Option<&Bound<'_, PyAny>>) {
        self.id = value
            .filter(|value| !value.is_none())
            .map(|value| value.clone().unbind());
    }

    /// Puts an object, or each of a list of them, after everything held.
    fn append(slf: &Bound<'_, Self>, others: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let others: Vec<Bound<'_, PyAny>> =
            if others.is_instance_of::<PyList>() || others.is_instance_of::<PyTuple>() {
                others.walk()?.collect::<PyResult<_>>()?
            } else {
                vec![others.clone()]
            };
        for other in &others {
            let offset = slf.borrow().end(py)?;
            slf.borrow_mut().put(offset, other);
        }
        Ok(())
    }

    /// Puts an object at an offset: `insert(offset, item)`, `insert(item)`
    /// at the item's own offset or nought, or `insert([offset, item, ...])`.
    #[pyo3(signature = (offsetOrItemOrList, itemOrNone = None))]
    fn insert(
        &mut self,
        offsetOrItemOrList: &Bound<'_, PyAny>,
        itemOrNone: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        if let Some(item) = itemOrNone.filter(|item| !item.is_none()) {
            self.put(offsetOrItemOrList.extract()?, item);
            return Ok(());
        }
        if offsetOrItemOrList.is_instance_of::<PyList>()
            || offsetOrItemOrList.is_instance_of::<PyTuple>()
        {
            let items: Vec<Bound<'_, PyAny>> =
                offsetOrItemOrList.walk()?.collect::<PyResult<_>>()?;
            if !items.len().is_multiple_of(2) {
                return Err(StreamException::new_err(
                    "an insert list must alternate offsets and objects",
                ));
            }
            for pair in items.chunks(2) {
                self.put(pair[0].extract()?, &pair[1]);
            }
            return Ok(());
        }
        let offset = offsetOrItemOrList
            .getattr("offset")
            .and_then(|offset| offset.extract::<f64>())
            .unwrap_or(0.0);
        self.put(offset, offsetOrItemOrList);
        Ok(())
    }

    /// Takes an object out, found by identity.
    fn remove(&mut self, target: &Bound<'_, PyAny>) -> PyResult<()> {
        match self
            .held
            .iter()
            .position(|held| held.object.bind(target.py()).is(target))
        {
            Some(at) => {
                self.held.remove(at);
                Ok(())
            }
            None => Err(StreamException::new_err(format!(
                "cannot find {} in this stream",
                target.repr()?
            ))),
        }
    }

    /// Everything held, in order.
    #[getter]
    fn elements<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.sorted().iter().map(|held| held.object.bind(py)))
    }

    /// Where an object stands in this stream.
    fn elementOffset(&self, element: &Bound<'_, PyAny>) -> PyResult<f64> {
        self.held
            .iter()
            .find(|held| held.object.bind(element.py()).is(element))
            .map(|held| held.offset)
            .ok_or_else(|| {
                StreamException::new_err(format!(
                    "an entry for this object {} is not stored in this stream",
                    element
                        .repr()
                        .map(|text| text.to_string())
                        .unwrap_or_default()
                ))
            })
    }

    /// Moves an object this stream holds to another offset.
    fn setElementOffset(&mut self, element: &Bound<'_, PyAny>, offset: f64) -> PyResult<()> {
        let held = self
            .held
            .iter_mut()
            .find(|held| held.object.bind(element.py()).is(element))
            .ok_or_else(|| StreamException::new_err("this object is not stored in this stream"))?;
        held.offset = offset;
        Ok(())
    }

    /// The offset just past the last thing held.
    #[getter]
    fn highestTime(slf: &Bound<'_, Self>) -> PyResult<f64> {
        Self::highest_time(slf)
    }

    /// How long the stream lasts, which is its highest time.
    #[getter]
    fn quarterLength(slf: &Bound<'_, Self>) -> PyResult<f64> {
        Self::highest_time(slf)
    }

    fn __len__(&self) -> usize {
        self.held.len()
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.elements(py)?.try_iter()?.into_any())
    }

    fn __contains__(&self, element: &Bound<'_, PyAny>) -> bool {
        self.held
            .iter()
            .any(|held| held.object.bind(element.py()).is(element))
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let elements = self.elements(py)?;
        if key.is_instance_of::<PySlice>() {
            return elements.as_any().get_item(key);
        }
        let index: isize = key
            .extract()
            .map_err(|_| PyTypeError::new_err("a stream is indexed by a number or a slice"))?;
        let length = elements.len() as isize;
        let at = if index < 0 { index + length } else { index };
        if at < 0 || at >= length {
            return Err(PyIndexError::new_err(format!(
                "attempting to access index {index} while elements is of size {length}"
            )));
        }
        elements.get_item(at as usize)
    }

    /// One timeline with the nesting dissolved, holding the same objects at
    /// their offsets from this stream's start.
    fn flatten<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let mut leaves = Vec::new();
        Self::flatten_into(slf, 0.0, &mut leaves)?;
        let mut out = Self::empty(slf.borrow().kind);
        for (offset, object) in leaves {
            out.put(offset, object.bind(py));
        }
        Bound::new(py, out)
    }

    /// Everything held at every depth, streams included, in order.
    fn recurse<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let out = PyList::empty(py);
        let held: Vec<Py<PyAny>> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| held.object.clone_ref(py))
            .collect();
        for object in held {
            let object = object.into_bound(py);
            out.append(&object)?;
            if let Ok(inner) = object.cast::<Stream>() {
                for each in Self::recurse(inner)?.iter() {
                    out.append(each)?;
                }
            }
        }
        Ok(out)
    }

    /// The notes and chords this stream holds directly.
    #[getter]
    fn notes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            Ok(["isNote", "isChord"].iter().any(|flag| {
                object
                    .getattr(*flag)
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
            }))
        })
    }

    /// The notes, chords and rests this stream holds directly.
    #[getter]
    fn notesAndRests<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            for flag in ["isNote", "isChord", "isRest"] {
                if object
                    .getattr(flag)
                    .and_then(|flag| flag.is_truthy())
                    .unwrap_or(false)
                {
                    return Ok(true);
                }
            }
            Ok(false)
        })
    }

    /// What this stream holds directly of the classes given, by class or by
    /// name.
    fn getElementsByClass<'py>(
        slf: &Bound<'py, Self>,
        classFilterList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, Stream>> {
        Self::picked(slf, Some(StreamKind::Stream), |object| {
            is_of_any(object, classFilterList)
        })
    }

    /// The parts this stream holds directly.
    #[getter]
    fn parts<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, Stream>> {
        let py = slf.py();
        let part = pyo3::types::PyString::new(py, "Part");
        Self::picked(slf, Some(StreamKind::Score), |object| {
            is_of_class(object, &part)
        })
    }

    #[pyo3(signature = (memo = None))]
    fn __deepcopy__<'py>(
        slf: &Bound<'py, Self>,
        memo: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let deepcopy = py.import("copy")?.getattr("deepcopy")?;
        let copied = slf.get_type().call0()?;
        let held: Vec<(f64, Py<PyAny>)> = slf
            .borrow()
            .sorted()
            .iter()
            .map(|held| (held.offset, held.object.clone_ref(py)))
            .collect();
        {
            let stream = copied.cast::<Stream>()?;
            let mut stream = stream.borrow_mut();
            stream.id = slf.borrow().id.as_ref().map(|id| id.clone_ref(py));
            for (offset, object) in held {
                let object = deepcopy.call1((object, memo))?;
                stream.put(offset, &object);
            }
        }
        Ok(copied)
    }

    fn __copy__<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let copied = slf.get_type().call0()?;
        {
            let stream = copied.cast::<Stream>()?;
            let mut stream = stream.borrow_mut();
            let me = slf.borrow();
            stream.id = me.id.as_ref().map(|id| id.clone_ref(py));
            for held in me.sorted() {
                stream.put(held.offset, held.object.bind(py));
            }
        }
        Ok(copied)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let label = match &slf.borrow().id {
            Some(id) => id.bind(py).str()?.to_string(),
            None => format!("0x{:x}", slf.as_ptr() as usize),
        };
        Ok(format!(
            "<music21.stream.{} {label}>",
            Self::class_name(slf)?
        ))
    }
}

/// music21's other stream classes, each the same `Stream` under its name.
macro_rules! stream_kinds {
    ($(($class:ident, $name:literal, $kind:expr, $doc:literal)),* $(,)?) => {
        $(
            #[doc = $doc]
            #[pyclass(name = $name, module = "music21.stream", extends = Stream, subclass)]
            pub struct $class;

            #[pymethods]
            impl $class {
                #[new]
                #[pyo3(signature = (givenElements = None, *_arguments, **keywords))]
                fn new(
                    py: Python<'_>,
                    givenElements: Option<&Bound<'_, PyAny>>,
                    _arguments: &Bound<'_, PyTuple>,
                    keywords: Option<&Bound<'_, PyDict>>,
                ) -> PyResult<PyClassInitializer<Self>> {
                    Ok(PyClassInitializer::from(Stream::filled(
                        py,
                        $kind,
                        givenElements,
                        keywords,
                    )?)
                    .add_subclass($class))
                }
            }
        )*

        fn register_kinds(m: &Bound<'_, PyModule>) -> PyResult<()> {
            $(m.add_class::<$class>()?;)*
            Ok(())
        }
    };
}

stream_kinds!(
    (
        Score,
        "Score",
        StreamKind::Score,
        "music21's `Score`: parts at one time."
    ),
    (
        Part,
        "Part",
        StreamKind::Part,
        "music21's `Part`: one voice's line through the piece."
    ),
    (
        PartStaff,
        "PartStaff",
        StreamKind::PartStaff,
        "music21's `PartStaff`: one staff of a part written on several."
    ),
    (
        Measure,
        "Measure",
        StreamKind::Measure,
        "music21's `Measure`: one bar."
    ),
    (
        Voice,
        "Voice",
        StreamKind::Voice,
        "music21's `Voice`: one line inside a bar."
    ),
    (
        Opus,
        "Opus",
        StreamKind::Opus,
        "music21's `Opus`: a collection of scores."
    ),
);

/// A stream read into the crate, beside the objects standing at each of the
/// crate stream's positions (`music21_rs::Stream::leaves`).
struct Read {
    stream: RsStream,
    leaves: Vec<Py<PyAny>>,
}

/// Which of music21's classes a stream object is, read off its lineage.
fn kind_of(object: &Bound<'_, PyAny>) -> PyResult<StreamKind> {
    if let Ok(stream) = object.cast::<Stream>() {
        return Ok(stream.borrow().kind);
    }
    for kind in [
        StreamKind::Opus,
        StreamKind::Score,
        StreamKind::PartStaff,
        StreamKind::Part,
        StreamKind::Measure,
        StreamKind::Voice,
    ] {
        if is_of_class(
            object,
            &pyo3::types::PyString::new(object.py(), kind.as_str()),
        )? {
            // music21's `PartStaff` is a `Part`, and is read as one.
            return Ok(if kind == StreamKind::PartStaff {
                StreamKind::Part
            } else {
                kind
            });
        }
    }
    Ok(StreamKind::Stream)
}

/// The crate's value of one element, or `None` for one it has no value for.
fn element_value(object: &Bound<'_, PyAny>) -> PyResult<Option<StreamElement>> {
    let py = object.py();
    if let Ok(symbol) = object.cast::<ChordSymbol>() {
        return Ok(Some(StreamElement::ChordSymbol(
            crate::harmony::crate_value(symbol)?,
        )));
    }
    if let Ok(chord) = object.extract::<PyRef<'_, Chord>>() {
        return Ok(Some(StreamElement::Chord(chord.synced_inner(py))));
    }
    if let Ok(note) = object.extract::<PyRef<'_, Note>>() {
        return Ok(Some(StreamElement::Note(note.synced(py))));
    }
    if let Ok(rest) = object.extract::<PyRef<'_, Rest>>() {
        return Ok(Some(StreamElement::Rest(rest.synced(py))));
    }
    if let Ok(dynamic) = object.extract::<PyRef<'_, Dynamic>>() {
        return Ok(Some(StreamElement::Dynamic(dynamic.inner.clone())));
    }
    if let Ok(key) = object.extract::<PyRef<'_, KeySignature>>() {
        return Ok(Some(StreamElement::KeySignature(key.signature.clone())));
    }
    if let Ok(meter) = object.extract::<PyRef<'_, TimeSignature>>() {
        return Ok(Some(StreamElement::TimeSignature(meter.inner.clone())));
    }
    if let Ok(mark) = object.extract::<PyRef<'_, MetronomeMark>>() {
        return Ok(Some(StreamElement::MetronomeMark(mark.inner.clone())));
    }
    // Something the crate has no value for: a clef, a barline, an
    // instrument, music21's own metre. What the walks read of one is where it
    // stands, how long it lasts and whether it sorts ahead of a note -- a
    // voice does not pair across a clef that stands where its last note
    // started -- so it is held by a stand-in with the same: a rest as long as
    // it is, or a tempo mark where it takes no time. The object itself is
    // still the one listed at that position. One that takes no time and
    // sorts with the notes, a spanner music21 keeps at the start of a part,
    // takes part in no walk and is left out.
    let length = length_of(object)?;
    if length > 0.0 {
        let duration = music21_rs_crate::Duration::new(length)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        return Ok(Some(StreamElement::Rest(music21_rs_crate::Rest::new(
            duration,
        ))));
    }
    if sort_order(object) < 20 {
        return Ok(Some(StreamElement::MetronomeMark(
            music21_rs_crate::MetronomeMark::new(60.0),
        )));
    }
    Ok(None)
}

/// Reads a stream, this wheel's or music21's, into the crate.
fn read(stream: &Bound<'_, PyAny>) -> PyResult<Read> {
    let mut leaves = Vec::new();
    let stream = read_into(stream, &mut leaves)?;
    Ok(Read { stream, leaves })
}

fn read_into(stream: &Bound<'_, PyAny>, leaves: &mut Vec<Py<PyAny>>) -> PyResult<RsStream> {
    let mut placed: Vec<(f64, Bound<'_, PyAny>)> = Vec::new();
    for element in stream.getattr("elements")?.walk()? {
        let element = element?;
        let offset: f64 = stream
            .call_method1("elementOffset", (&element,))?
            .extract()?;
        placed.push((offset, element));
    }
    // Stable, as the crate's own ordering is, so the positions the crate
    // counts are the order these objects are listed in.
    placed.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut events = Vec::new();
    for (offset, element) in placed {
        let is_stream = element
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false);
        if is_stream {
            let mut inner = read_into(&element, leaves)?;
            inner.set_kind(kind_of(&element)?);
            events.push(StreamEvent::new(
                offset,
                StreamElement::Stream(Box::new(inner)),
            ));
        } else if let Some(value) = element_value(&element)? {
            leaves.push(element.unbind());
            events.push(StreamEvent::new(offset, value));
        }
    }
    let mut read = RsStream::from_events(events);
    read.set_kind(kind_of(stream)?);
    Ok(read)
}

/// Whether a flag music21 takes as `True`, `False` or an object is `True`
/// itself.
fn is_true(value: &Bound<'_, PyAny>) -> bool {
    value
        .cast::<pyo3::types::PyBool>()
        .is_ok_and(|flag| flag.is_true())
}

/// music21's `volume.realizeVolume`: realizes the volume of every note and
/// chord of a stream under the dynamic in force where it stands, in place.
///
/// Each volume keeps what it realized, which is what `cachedRealized` hands
/// back; with `setAbsoluteVelocity` the answer also becomes the velocity,
/// no longer relative, so the stream plays as marked from velocities alone.
/// `useDynamicContext` is `True` for the stream's own dynamics, `False` for
/// none, or a dynamic to realize everything under.
#[pyfunction]
#[pyo3(signature = (
    srcStream,
    setAbsoluteVelocity = false,
    useDynamicContext = None,
    useVelocity = true,
    useArticulations = None,
))]
fn realizeVolume(
    py: Python<'_>,
    srcStream: &Bound<'_, PyAny>,
    setAbsoluteVelocity: bool,
    useDynamicContext: Option<&Bound<'_, PyAny>>,
    useVelocity: bool,
    useArticulations: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let read = read(srcStream)?;
    let elements = read.stream.leaves();
    let false_ = pyo3::types::PyBool::new(py, false).to_owned().into_any();
    let true_ = pyo3::types::PyBool::new(py, true).to_owned().into_any();
    let context = useDynamicContext.cloned().unwrap_or_else(|| true_.clone());
    let articulations = useArticulations.cloned().unwrap_or_else(|| true_.clone());
    let from_stream = is_true(&context)
        && elements
            .iter()
            .any(|(_, element)| matches!(element, StreamElement::Dynamic(_)));
    let in_force = music21_rs_crate::volume::dynamics_in_force(&read.stream);
    for (position, (_, element)) in elements.iter().enumerate() {
        if !matches!(
            element,
            StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::ChordSymbol(_)
        ) {
            continue;
        }
        let dynamic = if from_stream {
            in_force[position]
                .map(|found| read.leaves[found].bind(py).clone())
                .unwrap_or_else(|| false_.clone())
        } else if is_true(&context) {
            false_.clone()
        } else {
            context.clone()
        };
        let volume = read.leaves[position].bind(py).getattr("volume")?;
        let keywords = PyDict::new(py);
        keywords.set_item("useDynamicContext", dynamic)?;
        keywords.set_item("useVelocity", useVelocity)?;
        keywords.set_item("useArticulations", &articulations)?;
        let realized = volume.call_method("getRealized", (), Some(&keywords))?;
        if setAbsoluteVelocity {
            volume.setattr("velocityIsRelative", false)?;
            volume.setattr("velocityScalar", realized)?;
        }
    }
    Ok(())
}

/// music21's `tempo.interpolateElements`: carries what stands between two
/// objects of one stream into another in which both also stand, keeping
/// each thing's place between them.
///
/// Something already in the destination stays where it is; anything else is
/// put in at the same proportion of the way across, or, without `autoAdd`,
/// is an error.
#[pyfunction]
#[pyo3(signature = (element1, element2, sourceStream, destinationStream, autoAdd = true))]
fn interpolateElements(
    element1: &Bound<'_, PyAny>,
    element2: &Bound<'_, PyAny>,
    sourceStream: &Bound<'_, PyAny>,
    destinationStream: &Bound<'_, PyAny>,
    autoAdd: bool,
) -> PyResult<()> {
    let offset_in = |element: &Bound<'_, PyAny>,
                     stream: &Bound<'_, PyAny>,
                     which: &str,
                     place: &str|
     -> PyResult<f64> {
        stream
            .call_method1("elementOffset", (element,))
            .and_then(|offset| offset.extract())
            .map_err(|_| TempoException::new_err(format!("could not find {which} in {place}")))
    };
    let start = (
        offset_in(element1, sourceStream, "element1", "sourceStream")?,
        offset_in(element1, destinationStream, "element1", "destinationStream")?,
    );
    let end = (
        offset_in(element2, sourceStream, "element2", "sourceStream")?,
        offset_in(element2, destinationStream, "element2", "destinationStream")?,
    );
    let between: Vec<(f64, Bound<'_, PyAny>)> = sourceStream
        .getattr("elements")?
        .walk()?
        .map(|element| {
            let element = element?;
            let offset: f64 = sourceStream
                .call_method1("elementOffset", (&element,))?
                .extract()?;
            Ok((offset, element))
        })
        .collect::<PyResult<Vec<_>>>()?
        .into_iter()
        .filter(|(offset, _)| start.0 <= *offset && *offset <= end.0)
        .collect();
    for (offset, element) in between {
        if destinationStream
            .call_method1("elementOffset", (&element,))
            .is_ok()
        {
            continue;
        }
        if !autoAdd {
            let id = element.getattr("id").map_or_else(
                |_| "None".to_string(),
                |id| id.repr().map(|text| text.to_string()).unwrap_or_default(),
            );
            return Err(TempoException::new_err(format!(
                "Could not find element {} with id {id} in destinationStream and autoAdd is false",
                element.repr()?
            )));
        }
        let landed = music21_rs_crate::tempo::interpolated_offset(offset, start, end)
            .map_err(|error| TempoException::new_err(crate::pitch::message(&error)))?;
        destinationStream.call_method1("insert", (landed, &element))?;
    }
    Ok(())
}

/// music21's `harmony.realizeChordSymbolDurations`: gives each chord symbol
/// of a piece the time until the next, the last until the end of the piece,
/// and hands back the piece flattened, holding those same symbols.
#[pyfunction]
fn realizeChordSymbolDurations<'py>(piece: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = piece.py();
    let flat = piece.call_method0("flatten")?;
    let read = read(&flat)?;
    for (position, length) in music21_rs_crate::chordsymbol::chord_symbol_durations(&read.stream) {
        read.leaves[position]
            .bind(py)
            .getattr("duration")?
            .setattr("quarterLength", length)?;
    }
    Ok(flat)
}

/// What sounds or stands in each part of a score at one time: music21's
/// `voiceLeading.Verticality`, as far as `getVerticalityFromObject` builds
/// one.
#[pyclass(name = "Verticality", module = "music21.voiceLeading")]
pub struct Verticality {
    /// Each part's number and what it holds there.
    content: Py<PyDict>,
}

#[pymethods]
impl Verticality {
    #[new]
    #[pyo3(signature = (contentDict = None))]
    fn new(py: Python<'_>, contentDict: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let content = match contentDict {
            Some(given) => given.copy()?,
            None => PyDict::new(py),
        };
        Ok(Self {
            content: content.unbind(),
        })
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.content)
    }

    /// Each part's number and the objects it holds at this time.
    #[getter]
    fn contentDict(&self, py: Python<'_>) -> Py<PyDict> {
        self.content.clone_ref(py)
    }

    /// Every object, part by part.
    #[getter]
    fn objects<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let out = PyList::empty(py);
        let content = self.content.bind(py);
        let mut parts: Vec<i64> = content
            .keys()
            .iter()
            .map(|key| key.extract::<i64>())
            .collect::<PyResult<_>>()?;
        parts.sort_unstable();
        for part in parts {
            if let Some(held) = content.get_item(part)? {
                for object in held.walk()? {
                    out.append(object?)?;
                }
            }
        }
        Ok(out)
    }

    /// The objects one part holds at this time.
    fn getObjectsByPart<'py>(&self, py: Python<'py>, partNum: i64) -> PyResult<Bound<'py, PyAny>> {
        match self.content.bind(py).get_item(partNum)? {
            Some(held) => Ok(held),
            None => Ok(PyList::empty(py).into_any()),
        }
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "<music21.voiceLeading.Verticality contentDict={}>",
            self.content.bind(py).repr()?
        ))
    }
}

/// music21's `voiceLeading.getVerticalityFromObject`: what sounds or stands
/// in each part of a score at the time one of its objects stands, as a
/// `Verticality` keyed by part number.
///
/// A stream holding no parts is read as one part. A spanner, which music21
/// keeps at the start of a part and lists at every time a note there
/// sounds, is not listed.
#[pyfunction]
#[pyo3(signature = (music21Obj, scoreObjectIsFrom, classFilterList = None))]
fn getVerticalityFromObject(
    py: Python<'_>,
    music21Obj: &Bound<'_, PyAny>,
    scoreObjectIsFrom: &Bound<'_, PyAny>,
    classFilterList: Option<&Bound<'_, PyAny>>,
) -> PyResult<Verticality> {
    let read = read(scoreObjectIsFrom)?;
    let Some(position) = read
        .leaves
        .iter()
        .position(|leaf| leaf.bind(py).is(music21Obj))
    else {
        return Err(StreamException::new_err(format!(
            "{} is not in this score",
            music21Obj.repr()?
        )));
    };
    let elements = read.stream.leaves();
    let offset = elements[position].0;
    let content = PyDict::new(py);
    for (part, mut positions) in
        music21_rs_crate::voiceleading::verticality_positions_at(&read.stream, offset)
    {
        // music21 lists what starts together by `classSortOrder`, which the
        // stand-ins for what the crate cannot read do not carry; the objects
        // do. Stable, so the order the part holds them in breaks a tie.
        positions.sort_by(|left, right| {
            elements[*left].0.total_cmp(&elements[*right].0).then(
                sort_order(read.leaves[*left].bind(py))
                    .cmp(&sort_order(read.leaves[*right].bind(py))),
            )
        });
        let held = PyList::empty(py);
        for position in positions {
            let object = read.leaves[position].bind(py);
            let keep = match classFilterList.filter(|filter| !filter.is_none()) {
                Some(filter) => is_of_any(object, filter)?,
                None => true,
            };
            if keep {
                held.append(object)?;
            }
        }
        if !held.is_empty() {
            content.set_item(part, held)?;
        }
    }
    Ok(Verticality {
        content: content.unbind(),
    })
}

/// music21's `voiceLeading.iterateAllVoiceLeadingQuartets`: every quartet of
/// two voices moving together in a score, each made of the score's own note
/// objects.
///
/// The voices are the score's parts, or the stream itself where it holds
/// none. music21 hands back a generator; this is a list of the same.
#[pyfunction]
#[pyo3(signature = (
    s,
    *,
    includeRests = true,
    includeOblique = true,
    includeNoMotion = false,
    reverse = false,
))]
fn iterateAllVoiceLeadingQuartets<'py>(
    py: Python<'py>,
    s: &Bound<'py, PyAny>,
    includeRests: bool,
    includeOblique: bool,
    includeNoMotion: bool,
    reverse: bool,
) -> PyResult<Bound<'py, PyList>> {
    let read = read(s)?;
    let mut found = music21_rs_crate::voiceleading::voice_leading_quartet_positions(
        &read.stream,
        QuartetOptions {
            include_rests: includeRests,
            include_oblique: includeOblique,
            include_no_motion: includeNoMotion,
        },
    );
    if reverse {
        // Stable, so the quartets at one time keep their order, as music21
        // walks the times backwards and each one forwards.
        found.sort_by(|left, right| right.0.total_cmp(&left.0));
    }
    let class = crate::installed_class(py, "music21.voiceLeading", "VoiceLeadingQuartet")
        .unwrap_or_else(|| {
            py.get_type::<crate::voiceleading::VoiceLeadingQuartet>()
                .into_any()
        });
    let out = PyList::empty(py);
    for (_, notes) in found {
        let notes = notes.map(|position| read.leaves[position].bind(py).clone());
        out.append(class.call1((&notes[0], &notes[1], &notes[2], &notes[3]))?)?;
    }
    Ok(out)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Stream>()?;
    register_kinds(m)?;
    m.add_class::<Verticality>()?;
    m.add_function(wrap_pyfunction!(realizeVolume, m)?)?;
    m.add_function(wrap_pyfunction!(interpolateElements, m)?)?;
    m.add_function(wrap_pyfunction!(realizeChordSymbolDurations, m)?)?;
    m.add_function(wrap_pyfunction!(getVerticalityFromObject, m)?)?;
    m.add_function(wrap_pyfunction!(iterateAllVoiceLeadingQuartets, m)?)?;
    Ok(())
}
