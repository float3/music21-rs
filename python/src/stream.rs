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
#[pyclass(name = "Stream", module = "music21.stream", subclass, dict)]
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

/// A crate stream as this wheel's objects: a stream of its kind holding a
/// wheel object for each element the wheel has a class for, at the offsets
/// they stand at, with what the stream is called and numbered as the
/// attributes music21 keeps them in.
///
/// What the wheel has no class for is left out: words and repeat marks,
/// unpitched strokes, barlines, pedal bounces and gaps, spanners and a
/// score's metadata.
pub(crate) fn from_crate<'py>(py: Python<'py>, stream: &RsStream) -> PyResult<Bound<'py, PyAny>> {
    let class = match stream.kind() {
        StreamKind::Score => py.get_type::<Score>(),
        StreamKind::Part => py.get_type::<Part>(),
        StreamKind::PartStaff => py.get_type::<PartStaff>(),
        StreamKind::Measure => py.get_type::<Measure>(),
        StreamKind::Voice => py.get_type::<Voice>(),
        StreamKind::Opus => py.get_type::<Opus>(),
        _ => py.get_type::<Stream>(),
    };
    let made = class.call0()?;
    let mut objects = Vec::new();
    for event in stream.events() {
        if let Some(object) = element_object(py, event.element())? {
            objects.push((event.offset(), object));
        }
    }
    {
        let cell = made.cast::<Stream>()?;
        let mut held = cell.borrow_mut();
        for (offset, object) in &objects {
            held.put(*offset, object);
        }
        if let Some(id) = stream.id() {
            held.id = Some(pyo3::types::PyString::new(py, id).into_any().unbind());
        }
    }
    match stream.kind() {
        StreamKind::Measure => {
            made.setattr("number", stream.number())?;
            made.setattr("numberSuffix", stream.number_suffix())?;
            made.setattr("paddingLeft", stream.padding_left())?;
            made.setattr("paddingRight", stream.padding_right())?;
        }
        StreamKind::Part | StreamKind::PartStaff => {
            made.setattr("partName", stream.name())?;
            made.setattr("partAbbreviation", stream.abbreviation())?;
        }
        _ => {}
    }
    Ok(made)
}

/// The wheel's object for one element of a crate stream, where it has a
/// class for it.
fn element_object<'py>(
    py: Python<'py>,
    element: &StreamElement,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let class_named = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        py.import("music21_rs")
            .or_else(|_| py.import("music21_rs_facade"))?
            .getattr(name)
    };
    Ok(Some(match element {
        StreamElement::Stream(inner) => from_crate(py, inner)?,
        StreamElement::Note(note) => {
            crate::installed_new(py, "music21.note", "Note", Note::wrap(py, note.clone())?)?
                .into_bound(py)
                .into_any()
        }
        StreamElement::Chord(chord) => crate::installed_new(
            py,
            "music21.chord",
            "Chord",
            Chord::from_inner(py, chord.clone())?,
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Rest(rest) => {
            crate::installed_new(py, "music21.note", "Rest", Rest::wrap(py, rest.clone()))?
                .into_bound(py)
                .into_any()
        }
        StreamElement::Clef(clef) => crate::clef::Clef::object(py, clef.clone())?.into_bound(py),
        StreamElement::Instrument(instrument) => {
            crate::instrument::Instrument::object(py, (**instrument).clone())?.into_bound(py)
        }
        StreamElement::KeySignature(signature) => match signature.sharps() {
            Some(sharps) => class_named("KeySignature")?.call1((sharps,))?,
            None => return Ok(None),
        },
        StreamElement::Key(key) => class_named("Key")?.call1((key.tonic().name(), key.mode()))?,
        StreamElement::TimeSignature(meter) => {
            class_named("TimeSignature")?.call1((meter.ratio_string(),))?
        }
        StreamElement::MetronomeMark(mark) => crate::installed_new(
            py,
            "music21.tempo",
            "MetronomeMark",
            MetronomeMark::wrap(mark.clone()),
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::Dynamic(dynamic) => crate::installed_new(
            py,
            "music21.dynamics",
            "Dynamic",
            Dynamic::wrap(dynamic.clone()),
        )?
        .into_bound(py)
        .into_any(),
        StreamElement::ChordSymbol(symbol) => {
            if symbol.is_no_chord() {
                class_named("NoChord")?.call0()?
            } else {
                let keywords = PyDict::new(py);
                keywords.set_item("root", symbol.root().name())?;
                if let Some(bass) = symbol.bass() {
                    keywords.set_item("bass", bass.name())?;
                }
                keywords.set_item("kind", symbol.kind().unwrap_or("major"))?;
                if let Some(text) = symbol.kind_text() {
                    keywords.set_item("kindStr", text)?;
                }
                class_named("ChordSymbol")?.call((), Some(&keywords))?
            }
        }
        StreamElement::TempoText(_)
        | StreamElement::TextExpression(_)
        | StreamElement::RepeatExpression(_)
        | StreamElement::Unpitched(_)
        | StreamElement::PercussionChord(_)
        | StreamElement::Barline(_)
        | StreamElement::PedalObject(_)
        | StreamElement::RehearsalMark(_)
        | StreamElement::MetricModulation(_) => return Ok(None),
    }))
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
            return Ok(kind);
        }
    }
    Ok(StreamKind::Stream)
}

/// The crate's value of one element, or `None` for one it has no value for.
fn element_value(object: &Bound<'_, PyAny>, stand_ins: bool) -> PyResult<Option<StreamElement>> {
    let py = object.py();
    if let Ok(symbol) = object.cast::<ChordSymbol>() {
        return Ok(Some(StreamElement::ChordSymbol(
            crate::harmony::crate_value(symbol)?,
        )));
    }
    if let Ok(chord) = object.extract::<PyRef<'_, Chord>>() {
        let mut value = chord.synced_inner(py);
        drop(chord);
        // music21 hides a chord's notes one by one.
        if !stand_ins && let Ok(notes) = object.getattr("notes") {
            for (index, note) in notes.walk()?.enumerate() {
                let hidden = hidden_on_print(&note?);
                if let Some(held) = value.notes_mut().get_mut(index) {
                    held.set_hidden(hidden);
                }
            }
        }
        *value.expressions_mut() = expressions_of(object)?;
        *value.articulations_mut() = articulations_of(object)?;
        if !stand_ins && let Some(duration) = written_duration(object, value.duration())? {
            value = value.with_duration(duration);
        }
        if !stand_ins && let Some(duration) = inferred_duration(object, value.duration()) {
            value = value.with_duration(duration);
        }
        return Ok(Some(StreamElement::Chord(value)));
    }
    // music21's own percussion classes, read for a score writer. The walks
    // keep them as the stand-ins they have always been.
    if !stand_ins && is_of_class(object, &pyo3::types::PyString::new(py, "Unpitched"))? {
        return Ok(Some(StreamElement::Unpitched(unpitched_value(object)?)));
    }
    if !stand_ins && is_of_class(object, &pyo3::types::PyString::new(py, "PercussionChord"))? {
        return percussion_chord_value(object).map(|chord| Some(chord.into()));
    }
    if let Ok(note) = object.extract::<PyRef<'_, Note>>() {
        let mut value = note.synced(py);
        drop(note);
        value.set_hidden(hidden_on_print(object));
        *value.expressions_mut() = expressions_of(object)?;
        *value.articulations_mut() = articulations_of(object)?;
        if !stand_ins && let Some(duration) = written_duration(object, value.duration())? {
            value.set_duration(duration);
        }
        if !stand_ins && let Some(duration) = inferred_duration(object, value.duration()) {
            value.set_duration(duration);
        }
        return Ok(Some(StreamElement::Note(value)));
    }
    if let Ok(rest) = object.extract::<PyRef<'_, Rest>>() {
        let mut value = rest.synced(py);
        drop(rest);
        value.set_hidden(hidden_on_print(object));
        if let Some(shift) = object
            .getattr("stepShift")
            .ok()
            .and_then(|shift| shift.extract::<i32>().ok())
        {
            value.set_step_shift(shift);
        }
        let styled = object
            .getattr("hasStyleInformation")
            .and_then(|said| said.is_truthy())
            .unwrap_or(false);
        if styled {
            value.set_size(
                object
                    .getattr("style")
                    .and_then(|style| style.getattr("noteSize"))
                    .ok()
                    .and_then(|size| size.extract::<String>().ok())
                    .and_then(|size| music21_rs_crate::notation::NoteSize::from_name(&size)),
            );
        }
        *value.expressions_mut() = expressions_of(object)?;
        *value.articulations_mut() = articulations_of(object)?;
        if !stand_ins && let Some(duration) = written_duration(object, Some(value.duration()))? {
            value.set_duration(duration);
        }
        if !stand_ins && let Some(duration) = inferred_duration(object, Some(value.duration())) {
            value.set_duration(duration);
        }
        return Ok(Some(StreamElement::Rest(value)));
    }
    if let Ok(dynamic) = object.extract::<PyRef<'_, Dynamic>>() {
        let mut value = dynamic.inner.clone();
        drop(dynamic);
        value.set_placement(placement_of(object));
        return Ok(Some(StreamElement::Dynamic(value)));
    }
    if let Ok(key) = object.extract::<PyRef<'_, crate::key::Key>>() {
        let mut value = key.inner.clone();
        drop(key);
        value.set_color(style_color(object));
        return Ok(Some(StreamElement::Key(value)));
    }
    if let Ok(key) = object.extract::<PyRef<'_, KeySignature>>() {
        let mut value = key.signature.clone();
        drop(key);
        value.set_color(style_color(object));
        return Ok(Some(StreamElement::KeySignature(value)));
    }
    if let Ok(meter) = object.extract::<PyRef<'_, TimeSignature>>() {
        let mut value = meter.inner.clone();
        drop(meter);
        value.set_color(style_color(object));
        value.set_hidden(hidden_on_print(object));
        return Ok(Some(StreamElement::TimeSignature(value)));
    }
    if let Ok(mark) = object.extract::<PyRef<'_, MetronomeMark>>() {
        return Ok(Some(StreamElement::MetronomeMark(mark.inner.clone())));
    }
    // The walks keep a metric modulation as the stand-in it has always been.
    if !stand_ins
        && let Ok(modulation) = object.extract::<PyRef<'_, crate::tempo::MetricModulation>>()
    {
        let mut value = modulation.written_value(py)?;
        drop(modulation);
        value.set_placement(placement_of(object));
        return Ok(Some(StreamElement::from(value)));
    }
    if let Ok(clef) = object.extract::<PyRef<'_, crate::clef::Clef>>() {
        let mut value = clef.inner.clone();
        drop(clef);
        value.set_color(style_color(object));
        return Ok(Some(StreamElement::Clef(value)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "TextExpression"))? {
        let mut text = music21_rs_crate::expressions::TextExpression::new(
            object.getattr("content")?.str()?.to_string(),
        );
        text.set_placement(placement_of(object));
        return Ok(Some(StreamElement::TextExpression(text)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "RepeatExpression"))? {
        let class: String = object.get_type().getattr("__name__")?.extract()?;
        // A subclass of music21's own is read as the kind it derives from.
        let mut kind = music21_rs_crate::RepeatExpressionKind::from_class_name(&class);
        if kind.is_none() {
            for base in object.get_type().getattr("__mro__")?.walk()? {
                let name: String = base?.getattr("__name__")?.extract()?;
                kind = music21_rs_crate::RepeatExpressionKind::from_class_name(&name);
                if kind.is_some() {
                    break;
                }
            }
        }
        let Some(kind) = kind else {
            return Err(StreamException::new_err(format!(
                "a repeat expression of class {class} has no kind this crate knows"
            )));
        };
        let mut mark = music21_rs_crate::RepeatExpression::new(kind);
        if let Ok(text) = object.call_method0("getText")
            && !text.is_none()
        {
            mark.set_text(text.str()?.to_string());
        }
        mark.set_use_symbol(object.getattr("useSymbol")?.is_truthy()?);
        // A sign is placed as the mark is, and words as the text expression
        // music21 writes them from.
        if mark.use_symbol() {
            mark.set_placement(placement_of(object));
        } else if let Ok(expression) = object.call_method0("getTextExpression")
            && !expression.is_none()
        {
            mark.set_placement(placement_of(&expression));
        }
        return Ok(Some(StreamElement::RepeatExpression(mark)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "TempoText"))? {
        let mut text =
            music21_rs_crate::TempoText::new(text_attribute(object, "text").unwrap_or_default());
        // music21 keeps where the words sit on the text expression inside.
        if let Ok(expression) = object.call_method0("getTextExpression")
            && !expression.is_none()
        {
            text.set_placement(placement_of(&expression));
        }
        return Ok(Some(StreamElement::TempoText(text)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "Clef"))? {
        return clef_value(object).map(|clef| Some(StreamElement::Clef(clef)));
    }
    if is_of_class(object, &pyo3::types::PyString::new(py, "Instrument"))? {
        return instrument_value(object).map(|instrument| Some(StreamElement::from(instrument)));
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
    if !stand_ins {
        if is_of_class(object, &pyo3::types::PyString::new(py, "PedalObject"))? {
            let class: String = object.get_type().getattr("__name__")?.extract()?;
            let mut kind = music21_rs_crate::PedalObjectKind::from_class_name(&class);
            if kind.is_none() {
                for base in object.get_type().getattr("__mro__")?.walk()? {
                    let name: String = base?.getattr("__name__")?.extract()?;
                    kind = music21_rs_crate::PedalObjectKind::from_class_name(&name);
                    if kind.is_some() {
                        break;
                    }
                }
            }
            // music21's own base class, which is none of the three, writes
            // nothing.
            let Some(kind) = kind else {
                return Ok(None);
            };
            let mut value = music21_rs_crate::PedalObject::new(kind);
            value.set_placement(placement_of(object));
            return Ok(Some(StreamElement::PedalObject(value)));
        }
        if is_of_class(object, &pyo3::types::PyString::new(py, "RehearsalMark"))? {
            // music21 writes whatever the mark holds as text.
            let mut mark = music21_rs_crate::expressions::RehearsalMark::new(
                object.getattr("content")?.str()?.to_string(),
            );
            let styled = object
                .getattr("hasStyleInformation")
                .and_then(|said| said.is_truthy())
                .unwrap_or(false);
            if styled && let Ok(style) = object.getattr("style") {
                mark.set_placement(placement_of(&style));
                mark.set_enclosure(
                    style
                        .getattr("enclosure")
                        .ok()
                        .filter(|enclosure| !enclosure.is_none())
                        .and_then(|enclosure| enclosure.str().ok())
                        .map(|enclosure| enclosure.to_string()),
                );
            }
            return Ok(Some(StreamElement::RehearsalMark(mark)));
        }
        // Read for a score writer, something a score writes out as a note or
        // a direction cannot be left out without the score saying less.
        for written in ["GeneralNote", "Expression", "TempoIndication", "Dynamic"] {
            if is_of_class(object, &pyo3::types::PyString::new(py, written))? {
                let class: String = object.get_type().getattr("__name__")?.extract()?;
                return Err(StreamException::new_err(format!(
                    "a {class} cannot be read into a score yet"
                )));
            }
        }
        return Ok(None);
    }
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

/// A music21 `Unpitched`, read off its attributes.
fn unpitched_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Unpitched> {
    let step: String = object.getattr("displayStep")?.str()?.to_string();
    let octave: i32 = object.getattr("displayOctave")?.extract()?;
    let mut stroke =
        music21_rs_crate::Unpitched::displayed_at(step.chars().next().unwrap_or('B'), octave)
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    read_written(object, stroke.written_mut())?;
    Ok(stroke)
}

/// A music21 `PercussionChord`, read off its attributes: each member as the
/// stroke or the note it is, and what is written of the chord as a whole.
fn percussion_chord_value(
    object: &Bound<'_, PyAny>,
) -> PyResult<music21_rs_crate::PercussionChord> {
    use music21_rs_crate::PercussionNote;
    let py = object.py();
    let mut members = Vec::new();
    for member in object.getattr("notes")?.walk()? {
        let member = member?;
        if let Ok(note) = member.extract::<PyRef<'_, Note>>() {
            members.push(PercussionNote::Note(note.synced(py)));
        } else {
            members.push(PercussionNote::Unpitched(unpitched_value(&member)?));
        }
    }
    let mut chord = music21_rs_crate::PercussionChord::new(members)
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    // The chord's own notation, read onto a note and carried across.
    let mut carrier = music21_rs_crate::Note::from_name("C4")
        .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    read_written(object, &mut carrier)?;
    let written = chord.written_mut();
    if let Some(duration) = carrier.duration() {
        written.set_duration(duration.clone());
    }
    written.set_stem_direction(carrier.stem_direction());
    written.set_beams(carrier.beams().clone());
    written.set_color(carrier.color().map(str::to_string));
    written.set_stored_instrument(carrier.stored_instrument().cloned());
    if carrier.has_volume_information() {
        written.set_volume(Some(carrier.volume()));
    }
    *written.expressions_mut() = carrier.expressions().to_vec();
    *written.articulations_mut() = carrier.articulations().to_vec();
    Ok(chord)
}

/// Reads what music21 writes about one of its own `NotRest` objects -- its
/// length, stem, notehead, beams, tie, lyrics, volume, colour and marks --
/// onto a note.
fn read_written(object: &Bound<'_, PyAny>, note: &mut music21_rs_crate::Note) -> PyResult<()> {
    use music21_rs_crate::notation::{Notehead, StemDirection};
    let py = object.py();
    let failed =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    if let Ok(duration) = object.getattr("duration") {
        if let Some(value) = crate::duration::duration_value_of(py, &duration.clone().unbind()) {
            note.set_duration(value);
        }
        if let Some(written) = written_duration(object, note.duration())? {
            note.set_duration(written);
        }
        if let Some(inferred) = inferred_duration(object, note.duration()) {
            note.set_duration(inferred);
        }
    }
    if let Some(name) = text_attribute(object, "stemDirection") {
        note.set_stem_direction(StemDirection::from_name(&name).map_err(failed)?);
    }
    if let Some(name) = text_attribute(object, "notehead") {
        note.set_notehead(Notehead::from_name(&name).map_err(failed)?);
    }
    if let Ok(fill) = object.getattr("noteheadFill") {
        note.set_notehead_fill(fill.extract::<bool>().ok());
    }
    if let Ok(parenthesis) = object.getattr("noteheadParenthesis") {
        note.set_notehead_parenthesis(parenthesis.is_truthy()?);
    }
    if let Ok(beams) = object.getattr("beams")
        && let Ok(mut beams) = beams.extract::<PyRefMut<'_, crate::notation::Beams>>()
    {
        note.set_beams(beams.settled_value(py));
    }
    if let Ok(tie) = object.getattr("tie")
        && let Ok(tie) = tie.extract::<PyRef<'_, crate::notation::Tie>>()
    {
        note.set_tie(Some(tie.inner.clone()));
    }
    if let Ok(lyrics) = object.getattr("lyrics") {
        let mut verses = Vec::new();
        for lyric in lyrics.walk()? {
            if let Ok(lyric) = lyric?.extract::<PyRef<'_, crate::notation::Lyric>>() {
                verses.push(lyric.synced(py));
            }
        }
        *note.lyrics_mut() = verses;
    }
    let has_volume = object
        .call_method0("hasVolumeInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if has_volume
        && let Ok(volume) = object.getattr("volume")
        && let Ok(volume) = volume.extract::<PyRef<'_, crate::notation::Volume>>()
    {
        note.set_volume(Some(volume.inner.clone()));
    }
    if let Ok(stored) = object.getattr("storedInstrument")
        && !stored.is_none()
    {
        note.set_stored_instrument(Some(instrument_value(&stored)?));
    }
    note.set_color(style_color(object));
    note.set_hidden(hidden_on_print(object));
    *note.expressions_mut() = expressions_of(object)?;
    *note.articulations_mut() = articulations_of(object)?;
    Ok(())
}

/// An attribute as text, where it is set and is text.
fn text_attribute(object: &Bound<'_, PyAny>, name: &str) -> Option<String> {
    object
        .getattr(name)
        .ok()
        .filter(|value| !value.is_none())
        .and_then(|value| value.extract::<String>().ok())
}

/// An attribute as a small whole number, where it is set.
fn u8_attribute(object: &Bound<'_, PyAny>, name: &str) -> Option<u8> {
    object
        .getattr(name)
        .ok()
        .filter(|value| !value.is_none())
        .and_then(|value| value.extract::<u8>().ok())
}

/// A duration with the tuplets its object was set to written into it, as a
/// score writer needs them: the facade keeps set tuplets as Python objects,
/// which carry how each is bracketed. Nothing where none were set.
fn written_duration(
    object: &Bound<'_, PyAny>,
    duration: Option<&music21_rs_crate::Duration>,
) -> PyResult<Option<music21_rs_crate::Duration>> {
    let py = object.py();
    let Ok(held) = object.getattr("duration") else {
        return Ok(None);
    };
    if let Some(grace) = crate::duration::grace_value(&held)? {
        return Ok(Some(grace));
    }
    let Ok(held) = held.extract::<PyRef<'_, crate::duration::Duration>>() else {
        return Ok(None);
    };
    let Some(mut written) = held.written_value(py) else {
        return Ok(None);
    };
    let length = duration.map_or(1.0, music21_rs_crate::Duration::quarter_length);
    // A score may say a note lasts other than what it is written as -- a
    // file's durations are rounded to its divisions -- and music21 keeps
    // both, writing the one as a length and the other as a note value.
    if (written.quarter_length() - length).abs() > 1e-9 * length.max(1.0) {
        written.set_linked(false);
        written
            .set_quarter_length(length)
            .map_err(|error| StreamException::new_err(error.to_string()))?;
    }
    Ok(Some(written))
}

/// A duration saying, as its object's does, whether the way it is written
/// was worked out from its length: music21's `expressionIsInferred`, which
/// making notation asks before it writes a length another way. Nothing
/// where the duration says so already.
fn inferred_duration(
    object: &Bound<'_, PyAny>,
    duration: Option<&music21_rs_crate::Duration>,
) -> Option<music21_rs_crate::Duration> {
    let inferred = object
        .getattr("duration")
        .ok()?
        .getattr("expressionIsInferred")
        .ok()?
        .extract::<bool>()
        .ok()?;
    let mut duration = duration.cloned().unwrap_or_default();
    if duration.expression_is_inferred() == inferred {
        return None;
    }
    duration.set_expression_is_inferred(inferred);
    Some(duration)
}

/// A note's or chord's `articulations`, which are this wheel's own.
fn articulations_of(object: &Bound<'_, PyAny>) -> PyResult<Vec<music21_rs_crate::Articulation>> {
    let mut out = Vec::new();
    let Ok(articulations) = object.getattr("articulations") else {
        return Ok(out);
    };
    for articulation in articulations.walk()? {
        let articulation = articulation?;
        if let Ok(ours) = articulation.extract::<PyRef<'_, crate::articulations::Articulation>>() {
            out.push(ours.inner.clone());
        }
    }
    Ok(out)
}

/// A note's or chord's `expressions`, those the crate carries.
///
/// An ornament is this wheel's own; a fermata and an arpeggio mark are
/// music21's, read off their attributes. Anything else music21 files there
/// is left out, and a MusicXML writer does not write it.
fn expressions_of(
    object: &Bound<'_, PyAny>,
) -> PyResult<Vec<music21_rs_crate::expressions::Expression>> {
    use music21_rs_crate::expressions::{ArpeggioType, Expression, Fermata, FermataType};
    let py = object.py();
    let mut out = Vec::new();
    let Ok(expressions) = object.getattr("expressions") else {
        return Ok(out);
    };
    for expression in expressions.walk()? {
        let expression = expression?;
        if let Ok(ornament) = expression.extract::<PyRef<'_, crate::expressions::Ornament>>() {
            out.push(Expression::Ornament(Box::new(ornament.inner.clone())));
        } else if is_of_class(&expression, &pyo3::types::PyString::new(py, "Fermata"))? {
            let mut fermata = Fermata::new();
            fermata.set_fermata_type(
                if text_attribute(&expression, "type").as_deref() == Some("upright") {
                    FermataType::Upright
                } else {
                    FermataType::Inverted
                },
            );
            fermata.set_shape(text_attribute(&expression, "shape"));
            out.push(Expression::Fermata(fermata));
        } else if is_of_class(&expression, &pyo3::types::PyString::new(py, "ArpeggioMark"))? {
            let kind = text_attribute(&expression, "type").unwrap_or_default();
            let arpeggio = ArpeggioType::from_name(&kind)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
            out.push(Expression::Arpeggio(arpeggio));
        }
    }
    Ok(out)
}

/// The colour music21 keeps on an object's style, where it has one.
fn style_color(object: &Bound<'_, PyAny>) -> Option<String> {
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if !styled {
        return None;
    }
    text_attribute(&object.getattr("style").ok()?, "color").filter(|color| !color.is_empty())
}

/// Whether music21 leaves an object off the printed page.
fn hidden_on_print(object: &Bound<'_, PyAny>) -> bool {
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    styled
        && object
            .getattr("style")
            .and_then(|style| style.getattr("hideObjectOnPrint"))
            .and_then(|hidden| hidden.is_truthy())
            .unwrap_or(false)
}

/// An object's `placement`, where it is `above` or `below`.
fn placement_of(object: &Bound<'_, PyAny>) -> Option<music21_rs_crate::Placement> {
    text_attribute(object, "placement")
        .and_then(|name| music21_rs_crate::Placement::from_name(&name).ok())
}

/// A music21 clef this wheel does not hold, read off its attributes.
fn clef_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::clef::Clef> {
    use music21_rs_crate::clef::{Clef as RsClef, ClefKind};
    let class: String = object.get_type().getattr("__name__")?.extract()?;
    let mut clef = ClefKind::from_class_name(&class).map_or_else(RsClef::default, RsClef::of_kind);
    clef.set_sign(text_attribute(object, "sign"));
    clef.set_line(u8_attribute(object, "line"));
    clef.set_color(style_color(object));
    if let Some(change) = object
        .getattr("octaveChange")
        .ok()
        .and_then(|value| value.extract::<i32>().ok())
    {
        clef.set_octave_change(change);
    }
    Ok(clef)
}

/// A music21 instrument, read off its attributes: this wheel's instruments
/// are not installed, so a score holds music21's own.
fn instrument_value(object: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Instrument> {
    use music21_rs_crate::Instrument as RsInstrument;
    let class: String = object.get_type().getattr("__name__")?.extract()?;
    let mut instrument = RsInstrument::of_kind(&class).unwrap_or_default();
    instrument.set_name(text_attribute(object, "instrumentName"));
    instrument.set_abbreviation(text_attribute(object, "instrumentAbbreviation"));
    instrument.set_part_name(text_attribute(object, "partName"));
    instrument.set_part_abbreviation(text_attribute(object, "partAbbreviation"));
    instrument.set_part_id(text_attribute(object, "partId"));
    instrument.set_instrument_id(text_attribute(object, "instrumentId"));
    instrument.set_midi_program(u8_attribute(object, "midiProgram"));
    instrument.set_midi_channel(u8_attribute(object, "midiChannel"));
    if object.hasattr("percMapPitch")? {
        instrument.set_percussion_pitch(u8_attribute(object, "percMapPitch"));
    }
    let transposition = match object.getattr("transposition") {
        Ok(interval) if !interval.is_none() => {
            let name: String = interval.getattr("directedName")?.extract()?;
            Some(
                music21_rs_crate::Interval::from_name(&name)
                    .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
            )
        }
        _ => None,
    };
    instrument.set_transposition(transposition);
    Ok(instrument)
}

/// music21's `id` where it was given rather than taken from a memory
/// address: music21 reads any whole number from `defaults.
/// minIdNumberToConsiderMemoryLocation` up as the latter.
fn given_id(object: &Bound<'_, PyAny>) -> Option<String> {
    let id = object.getattr("id").ok().filter(|id| !id.is_none())?;
    if let Ok(number) = id.extract::<i64>() {
        return (number < 100_000_001).then(|| number.to_string());
    }
    id.extract::<String>().ok()
}

/// What a stream is called and numbered: a measure's number, a part's name,
/// a score's metadata.
fn read_labels(object: &Bound<'_, PyAny>, stream: &mut RsStream) -> PyResult<()> {
    stream.set_id(given_id(object));
    if let Some(number) = object
        .getattr("number")
        .ok()
        .and_then(|number| number.extract::<i32>().ok())
    {
        stream.set_number(number);
    }
    stream.set_number_suffix(text_attribute(object, "numberSuffix"));
    // How much of its bar a pickup or a measure cut short leaves unfilled.
    let padding = |name: &str| {
        object
            .getattr(name)
            .ok()
            .and_then(|padding| padding.extract::<f64>().ok())
            .unwrap_or(0.0)
    };
    stream.set_padding_left(padding("paddingLeft"));
    stream.set_padding_right(padding("paddingRight"));
    if let Ok(show) = object.getattr("showNumber") {
        stream.set_number_hidden(show.str()?.to_str()?.to_lowercase().ends_with("never"));
    }
    stream.set_name(text_attribute(object, "partName"));
    stream.set_abbreviation(text_attribute(object, "partAbbreviation"));
    let styled = object
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if styled && let Ok(style) = object.getattr("style") {
        let hidden = |name: &str| {
            style
                .getattr(name)
                .and_then(|shown| shown.is_truthy())
                .is_ok_and(|shown| !shown)
        };
        stream.set_name_hidden(hidden("printPartName"));
        stream.set_abbreviation_hidden(hidden("printPartAbbreviation"));
    }
    if let Ok(metadata) = object.getattr("metadata")
        && !metadata.is_none()
    {
        stream.set_metadata(Some(metadata_value(&metadata)?));
    }
    read_staff_groups(object, stream)?;
    read_ending(object, stream)?;
    for (name, set) in [
        (
            "leftBarline",
            RsStream::set_left_barline as fn(&mut RsStream, _),
        ),
        ("rightBarline", RsStream::set_right_barline),
    ] {
        if let Ok(barline) = object.getattr(name)
            && !barline.is_none()
        {
            set(stream, Some(barline_value(&barline)?));
        }
    }
    Ok(())
}

/// The alternative ending a measure is in: the first `RepeatBracket` it is
/// spanned by, as music21's `setRbSpanners` takes it.
fn read_ending(object: &Bound<'_, PyAny>, stream: &mut RsStream) -> PyResult<()> {
    if stream.kind() != StreamKind::Measure {
        return Ok(());
    }
    let Ok(sites) = object.call_method0("getSpannerSites") else {
        return Ok(());
    };
    for spanner in sites.walk()? {
        let spanner = spanner?;
        if !is_of_class(
            &spanner,
            &pyo3::types::PyString::new(object.py(), "RepeatBracket"),
        )? {
            continue;
        }
        let numbers: Vec<u32> = spanner.getattr("numberRange")?.extract()?;
        let starts = spanner.call_method1("isFirst", (object,))?.is_truthy()?;
        let stops = spanner.call_method1("isLast", (object,))?.is_truthy()?;
        stream.set_ending(Some(music21_rs_crate::Ending::new(numbers, starts, stops)));
        break;
    }
    Ok(())
}

/// A music21 barline or repeat sign.
fn barline_value(barline: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Barline> {
    use music21_rs_crate::{Barline, BarlineType, RepeatDirection};
    let invalid =
        |error: music21_rs_crate::Error| StreamException::new_err(crate::pitch::message(&error));
    let bar_type = BarlineType::from_name(&text_attribute(barline, "type").unwrap_or_default())
        .map_err(invalid)?;
    let direction = match text_attribute(barline, "direction").as_deref() {
        Some("start") => Some(RepeatDirection::Start),
        Some("end") => Some(RepeatDirection::End),
        _ => None,
    };
    let mut read = match direction {
        Some(direction) => {
            let times = barline
                .getattr("times")
                .ok()
                .and_then(|times| times.extract::<u32>().ok());
            Barline::repeat(direction, times)
        }
        None => Barline::default(),
    };
    read.set_bar_type(bar_type);
    Ok(read)
}

/// A score's `StaffGroup` spanners, each naming the parts it spans by where
/// they stand among the score's parts.
fn read_staff_groups(object: &Bound<'_, PyAny>, stream: &mut RsStream) -> PyResult<()> {
    use music21_rs_crate::{BarTogether, StaffGroup};
    let py = object.py();
    let Ok(bundle) = object.getattr("spannerBundle") else {
        return Ok(());
    };
    let mut parts: Vec<Bound<'_, PyAny>> = Vec::new();
    for element in object.getattr("elements")?.walk()? {
        let element = element?;
        if matches!(
            kind_of(&element),
            Ok(StreamKind::Part | StreamKind::PartStaff)
        ) && element
            .getattr("isStream")
            .and_then(|flag| flag.is_truthy())
            .unwrap_or(false)
        {
            parts.push(element);
        }
    }
    if parts.is_empty() {
        return Ok(());
    }
    for group in bundle.call_method1("getByClass", ("StaffGroup",))?.walk()? {
        let group = group?;
        let mut positions = Vec::new();
        for spanned in group.call_method0("getSpannedElements")?.walk()? {
            let spanned = spanned?;
            if let Some(position) = parts.iter().position(|part| part.is(&spanned)) {
                positions.push(position);
            }
        }
        let mut read = StaffGroup::new(positions);
        read.set_name(text_attribute(&group, "name"));
        read.set_abbreviation(text_attribute(&group, "abbreviation"));
        read.set_symbol(text_attribute(&group, "symbol").as_deref())
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
        let bar_together = group.getattr("barTogether")?;
        read.set_bar_together(if bar_together.is_none() {
            None
        } else if let Ok(flag) = bar_together.cast::<pyo3::types::PyBool>() {
            Some(if flag.is_true() {
                BarTogether::Yes
            } else {
                BarTogether::No
            })
        } else {
            Some(BarTogether::Mensurstrich)
        });
        let hidden = group
            .getattr("style")
            .and_then(|style| style.getattr("hideObjectOnPrint"))
            .map(|hidden| hidden.is_truthy().unwrap_or(false))
            .unwrap_or(false);
        read.set_name_hidden(hidden);
        stream.add_staff_group(read);
    }
    let _ = py;
    Ok(())
}

/// A music21 `Metadata`, name by name in the order music21 keeps them.
fn metadata_value(metadata: &Bound<'_, PyAny>) -> PyResult<music21_rs_crate::Metadata> {
    use music21_rs_crate::MetadataValue;
    let kwargs = PyDict::new(metadata.py());
    kwargs.set_item("returnPrimitives", true)?;
    kwargs.set_item("returnSorted", false)?;
    let mut out = music21_rs_crate::Metadata::new();
    for pair in metadata.call_method("all", (), Some(&kwargs))?.walk()? {
        let pair = pair?;
        let unique_name: String = pair.get_item(0)?.extract()?;
        let value = pair.get_item(1)?;
        // A contributor writes as its name, which is not always what
        // `str` gives it.
        let text = match value.getattr("name") {
            Ok(name) if !name.is_none() && value.hasattr("role")? => name.str()?.to_string(),
            _ => value.str()?.to_string(),
        };
        let entry = match text_attribute(&value, "role") {
            Some(role) => MetadataValue::with_role(text, role),
            None => MetadataValue::new(text),
        };
        out.add(unique_name, entry);
    }
    Ok(out)
}

/// A score, this wheel's or music21's, as the crate's `Stream`, with what it
/// is called and numbered: what the crate's MusicXML writer is handed.
///
/// Unlike the reading the walks do, an element the crate has no value for is
/// left out rather than held by a stand-in, since a writer would write the
/// stand-in.
///
/// Every spanner anywhere in the score is gathered onto the stream handed in,
/// in the order music21's `spannerBundle` holds them, which is the order
/// music21 numbers them in; each names what it joins by position among that
/// stream's leaves.
pub fn crate_stream(stream: &Bound<'_, PyAny>) -> PyResult<RsStream> {
    let mut leaves = Vec::new();
    let mut read = read_into(stream, &mut leaves, false)?;
    if let Ok(bundle) = stream.getattr("spannerBundle") {
        for spanner in bundle.walk()? {
            if let Some(value) = spanner_value(&spanner?, &leaves)? {
                read.add_spanner(value);
            }
        }
    }
    Ok(read)
}

/// Reads a stream, this wheel's or music21's, into the crate.
fn read(stream: &Bound<'_, PyAny>) -> PyResult<Read> {
    let mut leaves = Vec::new();
    let stream = read_into(stream, &mut leaves, true)?;
    Ok(Read { stream, leaves })
}

fn read_into(
    stream: &Bound<'_, PyAny>,
    leaves: &mut Vec<Py<PyAny>>,
    stand_ins: bool,
) -> PyResult<RsStream> {
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
        // A spanner is read from the score's bundle, beside the stream.
        if is_of_class(
            &element,
            &pyo3::types::PyString::new(element.py(), "Spanner"),
        )? {
            continue;
        }
        if is_stream {
            let mut inner = read_into(&element, leaves, stand_ins)?;
            inner.set_kind(kind_of(&element)?);
            events.push(StreamEvent::new(
                offset,
                StreamElement::Stream(Box::new(inner)),
            ));
        } else if !stand_ins && is_inner_barline(stream, &element)? {
            // A measure's own barlines are read with the measure; one
            // standing inside it is an element of its own.
            leaves.push(element.clone().unbind());
            events.push(StreamEvent::new(
                offset,
                StreamElement::Barline(barline_value(&element)?),
            ));
        } else if let Some(value) = element_value(&element, stand_ins)? {
            leaves.push(element.unbind());
            events.push(StreamEvent::new(offset, value));
        }
    }
    let mut read = RsStream::from_events(events);
    read.set_kind(kind_of(stream)?);
    read_labels(stream, &mut read)?;
    Ok(read)
}

/// Whether an element of a stream is a barline that is not the stream's own
/// left or right one.
fn is_inner_barline(stream: &Bound<'_, PyAny>, element: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !is_of_class(
        element,
        &pyo3::types::PyString::new(element.py(), "Barline"),
    )? {
        return Ok(false);
    }
    for end in ["leftBarline", "rightBarline"] {
        if stream.getattr(end).is_ok_and(|barline| barline.is(element)) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A music21 spanner of a kind the crate carries, naming what it joins by
/// position among `leaves`. One joining anything outside them -- a slur
/// from one part into another -- is left out.
fn spanner_value(
    spanner: &Bound<'_, PyAny>,
    leaves: &[Py<PyAny>],
) -> PyResult<Option<music21_rs_crate::Spanner>> {
    use music21_rs_crate::{Spanner, SpannerKind};
    let py = spanner.py();
    let is = |class: &str| is_of_class(spanner, &pyo3::types::PyString::new(py, class));
    let kind = if is("Slur")? {
        SpannerKind::Slur
    } else if is("Crescendo")? {
        SpannerKind::Crescendo
    } else if is("Diminuendo")? {
        SpannerKind::Diminuendo
    } else if is("Ottava")? {
        SpannerKind::Ottava
    } else if is("Line")? {
        SpannerKind::Line
    } else if is("PedalMark")? {
        SpannerKind::PedalMark
    } else if is("ArpeggioMarkSpanner")? {
        SpannerKind::ArpeggioMark
    } else if is("TrillExtension")? {
        SpannerKind::TrillExtension
    } else if is("Glissando")? {
        SpannerKind::Glissando
    } else if is("TremoloSpanner")? {
        SpannerKind::TremoloSpanner
    } else {
        return Ok(None);
    };
    // An element the score does not hold -- music21's reader can leave a
    // spanner holding one -- is still one of the spanner's.
    let mut positions = Vec::new();
    for spanned in spanner.call_method0("getSpannedElements")?.walk()? {
        let spanned = spanned?;
        positions.push(leaves.iter().position(|leaf| leaf.bind(py).is(&spanned)));
    }
    let mut value = Spanner::with_unplaced(kind, positions);
    value.set_placement(placement_of(spanner));
    if let Some(offset) = spanner
        .getattr("offset")
        .ok()
        .and_then(|offset| offset.extract::<f64>().ok())
    {
        value.set_offset(offset);
    }
    if kind == SpannerKind::Glissando {
        value.set_glissando_details(Some(music21_rs_crate::Glissando {
            slide_type: music21_rs_crate::SlideType::from_name(
                &text_attribute(spanner, "slideType").unwrap_or_default(),
            )
            .unwrap_or_default(),
            label: spanner
                .getattr("label")
                .ok()
                .filter(|label| !label.is_none())
                .map(|label| label.str().map(|text| text.to_string()))
                .transpose()?,
        }));
    }
    if kind == SpannerKind::TremoloSpanner {
        let marks = spanner
            .getattr("numberOfMarks")
            .ok()
            .and_then(|marks| marks.extract::<u8>().ok())
            .unwrap_or(3);
        value
            .set_number_of_marks(Some(marks))
            .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?;
    }
    if kind == SpannerKind::Ottava {
        let name = text_attribute(spanner, "type").unwrap_or_default();
        let transposing = spanner
            .getattr("transposing")
            .and_then(|transposing| transposing.is_truthy())
            .unwrap_or(true);
        value.set_shift(Some(
            music21_rs_crate::OctaveShift::from_name(&name, transposing)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
        ));
    }
    if kind.is_wedge() {
        value.set_spread(
            spanner
                .getattr("spread")
                .ok()
                .and_then(|spread| spread.extract::<f64>().ok()),
        );
    }
    let styled = spanner
        .getattr("hasStyleInformation")
        .and_then(|said| said.is_truthy())
        .unwrap_or(false);
    if styled && let Ok(style) = spanner.getattr("style") {
        value.set_line_type(text_attribute(&style, "lineType"));
    }
    // A glissando's line is its own attribute, not its style's.
    if kind == SpannerKind::Glissando {
        value.set_line_type(text_attribute(spanner, "lineType"));
    }
    if kind == SpannerKind::Line {
        let tick = |name: &str| -> PyResult<music21_rs_crate::LineEnd> {
            let written = text_attribute(spanner, name).unwrap_or_else(|| "down".to_string());
            music21_rs_crate::LineEnd::from_name(&written)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))
        };
        let height = |name: &str| {
            spanner
                .getattr(name)
                .ok()
                .filter(|height| !height.is_none())
                .and_then(|height| height.extract::<f64>().ok())
        };
        value.set_ends(music21_rs_crate::LineEnds {
            start: tick("startTick")?,
            end: tick("endTick")?,
            start_height: height("startHeight"),
            end_height: height("endHeight"),
        });
        value.set_line_type(text_attribute(spanner, "lineType"));
    }
    if kind == SpannerKind::ArpeggioMark {
        let name = text_attribute(spanner, "type").unwrap_or_else(|| "normal".to_string());
        value.set_arpeggio(
            music21_rs_crate::expressions::ArpeggioType::from_name(&name)
                .map_err(|error| StreamException::new_err(crate::pitch::message(&error)))?,
        );
    }
    if kind == SpannerKind::PedalMark {
        // Both are enums in music21, whose values are the names.
        let named = |name: &str| {
            spanner
                .getattr(name)
                .ok()
                .filter(|value| !value.is_none())
                .and_then(|value| value.getattr("value").ok())
                .and_then(|value| value.extract::<String>().ok())
        };
        value.set_pedal(Some(music21_rs_crate::Pedal {
            pedal_type: named("pedalType")
                .and_then(|name| music21_rs_crate::PedalType::from_name(&name)),
            form: named("pedalForm").and_then(|name| music21_rs_crate::PedalForm::from_name(&name)),
            abbreviated: spanner
                .getattr("abbreviated")
                .and_then(|flag| flag.is_truthy())
                .unwrap_or(false),
        }));
    }
    Ok(Some(value))
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
