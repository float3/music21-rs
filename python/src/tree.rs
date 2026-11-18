//! music21's `tree` package over this package's streams: a stream read as
//! timespans (`Stream.asTimespans`), the `TimespanTree` holding them, and
//! the `Verticality` of what starts, sounds and stops at one offset.
//!
//! The crate's `tree` module answers every question of time -- what starts
//! where, what sounds across, which verticality follows which, what pairs
//! move together -- over a crate stream read from the stream given. The
//! answers come back as the timespans made once for the tree, each holding
//! the stream's own element and streams, so `span.element` is the note the
//! stream holds, as in music21.
//!
//! A `TimespanTree` is music21's AVL tree of these timespans
//! ([`trees::OffsetTree`]), which can be edited in place. Its verticalities
//! are the crate's, so they are answered while the tree is as read from its
//! stream, and refused once it has been edited.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

pub mod analysis;
pub mod core;
pub mod examples;
pub mod fromstream;
pub mod makeelement;
pub mod node;
pub mod trees;
pub mod verticality;

use std::collections::HashMap;

use pyo3::exceptions::{PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyString, PyTuple};

use music21::braille::SegmentDivision;
use music21::stream::{Stream as RsStream, StreamElement, StreamEvent, StreamKind};
use music21::tree::{self as rs_tree, ElementTimespan as RsSpan, Flatten, TimespanTree as RsTree};

use crate::exceptions21::TreeException;
use crate::stream::{answers_to_class, element_read, element_written, stream_error};

/// An offset as music21's timespans hold one: a float, as its `Timespan`
/// makes every offset it is given.
fn float_of(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    Ok(PyFloat::new(py, value).into_any())
}

/// Which elements of a crate tree the tree keeps, as the crate's filter.
type Keep = Option<fn(&StreamElement) -> bool>;

pyo3::create_exception!(music21_rs_facade, TimespanException, TreeException);

/// A stretch of time from one offset to another: music21's
/// `tree.spans.Timespan`.
#[pyclass(name = "Timespan", module = "music21.tree.spans", subclass, frozen)]
pub struct Timespan {
    offset: f64,
    end_time: f64,
}

impl Timespan {
    /// From `offset` to `end_time`, which may not come before it.
    fn spanning(offset: f64, end_time: f64) -> PyResult<Self> {
        if offset > end_time {
            return Err(TimespanException::new_err(format!(
                "offset {offset:?} must be after endTime {end_time:?}"
            )));
        }
        Ok(Self { offset, end_time })
    }
}

/// A float of a quarter length given as any number.
fn float_given(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    value
        .py()
        .import("builtins")?
        .getattr("float")?
        .call1((value,))?
        .extract()
}

#[pymethods]
impl Timespan {
    #[new]
    fn py_new(offset: &Bound<'_, PyAny>, endTime: &Bound<'_, PyAny>) -> PyResult<Self> {
        Self::spanning(float_given(offset)?, float_given(endTime)?)
    }

    /// Where it starts.
    #[getter]
    fn offset<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.offset)
    }

    /// Where it ends.
    #[getter]
    fn endTime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.end_time)
    }

    /// Equal to a timespan of the same class from and to the same offsets.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !slf.get_type().is(other.get_type()) {
            return Ok(false);
        }
        let other = other.cast::<Self>()?.get();
        let held = slf.get();
        Ok(held.offset == other.offset && held.end_time == other.end_time)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let held = slf.get();
        Ok(format!(
            "<{} {} {}>",
            slf.get_type().name()?,
            float_of(py, held.offset)?.str()?,
            float_of(py, held.end_time)?.str()?
        ))
    }

    /// One of the same class with a new offset or end.
    #[pyo3(name = "new", signature = (*, offset = None, endTime = None))]
    fn renewed<'py>(
        slf: &Bound<'py, Self>,
        offset: Option<&Bound<'py, PyAny>>,
        endTime: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let keywords = PyDict::new(slf.py());
        keywords.set_item("offset", given_or(offset, slf.getattr("offset")?))?;
        keywords.set_item("endTime", given_or(endTime, slf.getattr("endTime")?))?;
        slf.get_type().call((), Some(&keywords))
    }

    /// Whether another can be merged with this one, and why not: it must
    /// be of the same class and start where this ends, or end where it
    /// starts.
    fn canMerge<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<(bool, String)> {
        if !other.is_instance(&slf.get_type())? {
            return Ok((
                false,
                format!(
                    "Cannot merge {} with {}: wrong types",
                    slf.str()?,
                    other.str()?
                ),
            ));
        }
        let held = slf.get();
        let other_held = other.cast::<Self>()?.get();
        if held.end_time != other_held.offset && other_held.end_time != held.offset {
            return Ok((
                false,
                format!(
                    "Cannot merge {} with {}: not contiguous",
                    slf.str()?,
                    other.str()?
                ),
            ));
        }
        Ok((true, String::new()))
    }

    /// The two as one, keeping what the earlier holds.
    fn mergeWith<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (can, message) = Self::canMerge(slf, other)?;
        if !can {
            return Err(TimespanException::new_err(message));
        }
        let keywords = PyDict::new(slf.py());
        if slf.get().offset < other.cast::<Self>()?.get().offset {
            keywords.set_item("endTime", other.getattr("endTime")?)?;
            return slf.call_method("new", (), Some(&keywords));
        }
        keywords.set_item("endTime", slf.getattr("endTime")?)?;
        other.call_method("new", (), Some(&keywords))
    }

    /// The two halves either side of an offset inside it, or itself alone.
    fn splitAt<'py>(
        slf: &Bound<'py, Self>,
        offset: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let py = slf.py();
        let at = float_given(offset)?;
        let held = slf.get();
        if at < held.offset || held.end_time < at {
            return PyTuple::new(py, [slf.clone().into_any()]);
        }
        let left = PyDict::new(py);
        left.set_item("endTime", offset)?;
        let right = PyDict::new(py);
        right.set_item("offset", offset)?;
        PyTuple::new(
            py,
            [
                slf.call_method("new", (), Some(&left))?,
                slf.call_method("new", (), Some(&right))?,
            ],
        )
    }
}

/// A value given, or what stands in for it.
fn given_or<'py>(
    given: Option<&Bound<'py, PyAny>>,
    otherwise: Bound<'py, PyAny>,
) -> Bound<'py, PyAny> {
    match given {
        Some(given) if !given.is_none() => given.clone(),
        _ => otherwise,
    }
}

/// One element of a stream as the stretch of time it sounds for: music21's
/// `tree.spans.ElementTimespan`.
#[pyclass(
    name = "ElementTimespan",
    module = "music21.tree.spans",
    extends = Timespan,
    subclass,
    frozen
)]
pub struct ElementTimespan {
    /// The element, the stream's own object.
    element: Py<PyAny>,
    parent_offset: Option<f64>,
    parent_end_time: Option<f64>,
    /// The streams it stands in, the one holding it first.
    parentage: Py<PyTuple>,
    /// The number of the measure it is in.
    measure: Option<i32>,
}

/// A timespan of a note, chord or stream, whose pitches it answers:
/// music21's `tree.spans.PitchedTimespan`.
#[pyclass(name = "PitchedTimespan", module = "music21.tree.spans", extends = ElementTimespan, frozen)]
pub struct PitchedTimespan;

impl ElementTimespan {
    /// Its parts as music21's constructor takes them.
    fn made(
        offset: &Bound<'_, PyAny>,
        endTime: &Bound<'_, PyAny>,
        element: &Bound<'_, PyAny>,
        parentOffset: Option<&Bound<'_, PyAny>>,
        parentEndTime: Option<&Bound<'_, PyAny>>,
        parentage: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let py = element.py();
        let optional = |value: Option<&Bound<'_, PyAny>>| -> PyResult<Option<f64>> {
            match value {
                Some(value) if !value.is_none() => float_given(value).map(Some),
                _ => Ok(None),
            }
        };
        let parent_offset = optional(parentOffset)?;
        let parent_end_time = optional(parentEndTime)?;
        if let (Some(start), Some(end)) = (parent_offset, parent_end_time)
            && start > end
        {
            return Err(TimespanException::new_err(format!(
                "offset {start:?} must be after parentEndTime {end:?}"
            )));
        }
        let parentage = match parentage {
            Some(parentage) => {
                PyTuple::new(py, parentage.try_iter()?.collect::<PyResult<Vec<_>>>()?)?
            }
            None => PyTuple::empty(py),
        };
        // music21 asks the element for its measure each time; it is asked
        // once here.
        let measure = element
            .getattr("measureNumber")
            .ok()
            .and_then(|number| number.extract::<Option<i32>>().ok())
            .flatten();
        let times = Timespan::spanning(float_given(offset)?, float_given(endTime)?)?;
        Ok(PyClassInitializer::from(times).add_subclass(Self {
            element: element.clone().unbind(),
            parent_offset,
            parent_end_time,
            parentage: parentage.unbind(),
            measure,
        }))
    }
}

#[pymethods]
impl ElementTimespan {
    #[new]
    #[pyo3(signature = (offset, endTime, element, *, parentOffset = None, parentEndTime = None, parentage = None))]
    fn py_new(
        offset: &Bound<'_, PyAny>,
        endTime: &Bound<'_, PyAny>,
        element: &Bound<'_, PyAny>,
        parentOffset: Option<&Bound<'_, PyAny>>,
        parentEndTime: Option<&Bound<'_, PyAny>>,
        parentage: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Self::made(
            offset,
            endTime,
            element,
            parentOffset,
            parentEndTime,
            parentage,
        )
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.element)?;
        visit.call(&self.parentage)
    }

    /// The element.
    #[getter]
    fn element(&self, py: Python<'_>) -> Py<PyAny> {
        self.element.clone_ref(py)
    }

    /// How long it lasts.
    #[getter]
    fn quarterLength<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let span = slf.as_super().get();
        float_of(slf.py(), span.end_time - span.offset)
    }

    /// Where the stream holding it starts.
    #[getter]
    fn parentOffset(&self) -> Option<f64> {
        self.parent_offset
    }

    /// Where the stream holding it ends.
    #[getter]
    fn parentEndTime(&self) -> Option<f64> {
        self.parent_end_time
    }

    /// The streams it stands in, the one holding it first and the stream
    /// read last.
    #[getter]
    fn parentage(&self, py: Python<'_>) -> Py<PyTuple> {
        self.parentage.clone_ref(py)
    }

    /// The number of the measure it is in, if any.
    #[getter]
    fn measureNumber(&self) -> Option<i32> {
        self.measure
    }

    /// The part it is in, if any.
    #[getter]
    fn part<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.first_parent(&PyString::new(py, "Part"))
    }

    /// The nearest stream it stands in of one of these classes.
    fn getParentageByClass<'py>(
        &self,
        classList: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.first_parent(classList)
    }

    fn __repr__(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let times = slf.as_super().get();
        Ok(format!(
            "<{} ({} to {}) {}>",
            slf.get_type().name()?,
            float_of(py, times.offset)?.str()?,
            float_of(py, times.end_time)?.str()?,
            slf.get().element.bind(py).repr()?
        ))
    }

    /// Equal only to itself.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> bool {
        slf.is(other)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        slf.as_ptr() as isize
    }

    /// One of the same class with some of its values replaced.
    #[pyo3(name = "new", signature = (*, offset = None, endTime = None, element = None, parentOffset = None, parentEndTime = None))]
    fn renewed<'py>(
        slf: &Bound<'py, Self>,
        offset: Option<&Bound<'py, PyAny>>,
        endTime: Option<&Bound<'py, PyAny>>,
        element: Option<&Bound<'py, PyAny>>,
        parentOffset: Option<&Bound<'py, PyAny>>,
        parentEndTime: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let keywords = PyDict::new(slf.py());
        keywords.set_item("offset", given_or(offset, slf.getattr("offset")?))?;
        keywords.set_item("endTime", given_or(endTime, slf.getattr("endTime")?))?;
        let element = match element {
            Some(element) if element.is_truthy()? => element.clone(),
            _ => slf.getattr("element")?,
        };
        keywords.set_item("element", element)?;
        keywords.set_item(
            "parentOffset",
            given_or(parentOffset, slf.getattr("parentOffset")?),
        )?;
        keywords.set_item(
            "parentEndTime",
            given_or(parentEndTime, slf.getattr("parentEndTime")?),
        )?;
        keywords.set_item("parentage", slf.getattr("parentage")?)?;
        slf.get_type().call((), Some(&keywords))
    }
}

impl ElementTimespan {
    /// The first stream of its parentage that is one of these classes.
    fn first_parent<'py>(
        &self,
        classes: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = classes.py();
        for parent in self.parentage.bind(py).iter() {
            if is_of_any(&parent, classes)? {
                return Ok(Some(parent));
            }
        }
        Ok(None)
    }
}

#[pymethods]
impl PitchedTimespan {
    #[new]
    #[pyo3(signature = (offset, endTime, element, *, parentOffset = None, parentEndTime = None, parentage = None))]
    fn py_new(
        offset: &Bound<'_, PyAny>,
        endTime: &Bound<'_, PyAny>,
        element: &Bound<'_, PyAny>,
        parentOffset: Option<&Bound<'_, PyAny>>,
        parentEndTime: Option<&Bound<'_, PyAny>>,
        parentage: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(ElementTimespan::made(
            offset,
            endTime,
            element,
            parentOffset,
            parentEndTime,
            parentage,
        )?
        .add_subclass(Self))
    }

    /// The pitches of its element, the element's own.
    #[getter]
    fn pitches<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let element = slf.as_super().get().element.bind(slf.py()).clone();
        element.getattr("pitches")
    }
}

/// Whether an object is of a class, or of any of a list of them, each read
/// as music21 reads a class: by itself or by its name.
fn is_of_any(object: &Bound<'_, PyAny>, classes: &Bound<'_, PyAny>) -> PyResult<bool> {
    if classes.is_instance_of::<PyString>() || classes.is_instance_of::<pyo3::types::PyType>() {
        return answers_to_class(object, classes);
    }
    for class in classes.try_iter()? {
        if answers_to_class(object, &class?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The elements of a stream read for a tree, as the crate stream holds
/// them: each element's object, and a stream's own elements beside it. A
/// marker stands for an element the class list leaves out.
struct Read {
    object: Option<Py<PyAny>>,
    inner: Vec<Read>,
}

/// What reading a stream for a tree is asked: music21's `classList`, and
/// what the streams met said to it.
struct Asked<'a, 'py> {
    classes: Option<&'a Bound<'py, PyAny>>,
    /// Whether nested streams are read at all: music21's `flatten`.
    nested: bool,
    /// How many streams met answered to the class list, and how many not.
    streams_kept: usize,
    streams_left: usize,
}

/// The crate's stand-in for an element the class list leaves out: a
/// braille division, which a tree read here holds nowhere else. It stands
/// where the element ends, so the stream it is in lasts as long as it did.
fn left_out_marker() -> StreamElement {
    StreamElement::BrailleSegmentDivision(SegmentDivision::Required)
}

/// Every element but the markers.
fn unmarked(element: &StreamElement) -> bool {
    !matches!(element, StreamElement::BrailleSegmentDivision(_))
}

/// Every element but the markers and the streams.
fn unmarked_leaf(element: &StreamElement) -> bool {
    unmarked(element) && !matches!(element, StreamElement::Stream(_))
}

/// How long an element lasts, nothing for one without a duration.
fn length_of(element: &Bound<'_, PyAny>) -> PyResult<f64> {
    let Ok(duration) = element.getattr("duration") else {
        return Ok(0.0);
    };
    if duration.is_none() {
        return Ok(0.0);
    }
    duration.getattr("quarterLength")?.extract()
}

/// Whether music21 makes a `PitchedTimespan` of a value read.
fn is_sounding(value: &StreamElement) -> bool {
    matches!(
        value,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

/// Whether music21 says an object is of a class, by its `classes`.
fn says_class(object: &Bound<'_, PyAny>, name: &str) -> bool {
    object
        .getattr("classes")
        .and_then(|classes| classes.contains(name))
        .unwrap_or(false)
}

/// An element read as the crate holds it for a tree: a note, chord or rest
/// as itself, a percussion stroke as written, and whatever else as what
/// the tree reads of it -- how long it lasts -- by a stand-in.
fn leaf_value(element: &Bound<'_, PyAny>) -> PyResult<StreamElement> {
    let value = element_read(element)?;
    if value.as_ref().is_some_and(is_sounding) || matches!(value, Some(StreamElement::Rest(_))) {
        return Ok(value.expect("a value just matched"));
    }

    // An unpitched stroke is read as a rest by the walks; the tree wants
    // it as a stroke, whose timespan is music21's `PitchedTimespan`.
    if says_class(element, "NotRest")
        && let Ok(Some(written)) = element_written(element)
        && is_sounding(&written)
    {
        return Ok(written);
    }
    if let Some(value) = value {
        return Ok(value);
    }
    stand_in(length_of(element)?)
}

/// A value lasting as long as an element the crate has none for.
fn stand_in(length: f64) -> PyResult<StreamElement> {
    if length <= 0.0 {
        return Ok(StreamElement::MetronomeMark(music21::MetronomeMark::new(
            60.0,
        )));
    }
    let duration = music21::Duration::new(length).map_err(|error| stream_error(&error))?;
    Ok(StreamElement::Rest(music21::Rest::new(duration)))
}

/// A measure's own barlines among its elements, where music21 keeps them:
/// the left one first, the right one at the end, where the measure ends.
/// This package's measures hold them beside their elements.
fn add_barlines<'py>(
    stream: &Bound<'py, PyAny>,
    placed: &mut Vec<(f64, Bound<'py, PyAny>)>,
) -> PyResult<()> {
    let barline = |name: &str| {
        stream
            .getattr(name)
            .ok()
            .filter(|barline| !barline.is_none())
            .filter(|barline| !placed.iter().any(|(_, held)| held.is(barline)))
    };
    let left = barline("leftBarline");
    let right = barline("rightBarline");
    if let Some(left) = left {
        placed.insert(0, (0.0, left));
    }
    if let Some(right) = right {
        let end: f64 = stream.getattr("highestTime")?.extract()?;
        placed.push((end, right));
    }
    Ok(())
}

/// Which of music21's stream classes a stream is.
fn kind_of(stream: &Bound<'_, PyAny>) -> PyResult<StreamKind> {
    for kind in [
        StreamKind::Opus,
        StreamKind::Score,
        StreamKind::PartStaff,
        StreamKind::Part,
        StreamKind::System,
        StreamKind::Measure,
        StreamKind::Voice,
    ] {
        if answers_to_class(stream, &PyString::new(stream.py(), kind.as_str()))? {
            return Ok(kind);
        }
    }
    Ok(StreamKind::Stream)
}

/// Whether an object is a stream.
fn is_stream(object: &Bound<'_, PyAny>) -> bool {
    object
        .getattr("isStream")
        .and_then(|flag| flag.is_truthy())
        .unwrap_or(false)
}

impl Asked<'_, '_> {
    /// A stream read into the crate, beside the objects at each of its
    /// events, in the crate's order.
    fn read(&mut self, stream: &Bound<'_, PyAny>) -> PyResult<(RsStream, Vec<Read>)> {
        let mut placed: Vec<(f64, Bound<'_, PyAny>)> = Vec::new();
        for element in stream.getattr("elements")?.try_iter()? {
            let element = element?;
            let offset: f64 = stream
                .call_method1("elementOffset", (&element,))?
                .extract()?;
            placed.push((offset, element));
        }
        placed.sort_by(|left, right| left.0.total_cmp(&right.0));
        add_barlines(stream, &mut placed)?;
        let mut events: Vec<(StreamEvent, Read)> = Vec::new();
        for (offset, element) in placed {
            events.push(self.event(offset, element)?);
        }

        // Sorted as the crate sorts, which keeps each object beside its
        // event once the crate stream is built.
        events.sort_by(|left, right| {
            left.0
                .offset()
                .partial_cmp(&right.0.offset())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let (events, reads): (Vec<StreamEvent>, Vec<Read>) = events.into_iter().unzip();
        let mut read = RsStream::from_events(events);
        let kind = kind_of(stream)?;
        read.set_kind(kind);
        if kind == StreamKind::Measure {
            read.set_number(stream.getattr("number")?.extract()?);
        }
        Ok((read, reads))
    }

    /// One element of a stream as an event, a nested stream read through.
    fn event(&mut self, offset: f64, element: Bound<'_, PyAny>) -> PyResult<(StreamEvent, Read)> {
        if is_stream(&element) {
            return self.nested(offset, element);
        }
        let kept = match self.classes {
            Some(classes) => is_of_any(&element, classes)?,
            None => true,
        };
        if !kept {
            let end = offset + length_of(&element)?;
            let marker = Read {
                object: None,
                inner: Vec::new(),
            };
            return Ok((StreamEvent::new(end, left_out_marker()), marker));
        }
        let value = leaf_value(&element)?;
        let read = Read {
            object: Some(element.unbind()),
            inner: Vec::new(),
        };
        Ok((StreamEvent::new(offset, value), read))
    }

    /// A stream held in the stream read.
    fn nested(&mut self, offset: f64, element: Bound<'_, PyAny>) -> PyResult<(StreamEvent, Read)> {
        if !self.nested {
            return Err(PyNotImplementedError::new_err(
                "asTimespans(flatten=False) of a stream holding streams builds a tree \
                 of trees, which is not ported to music21-rs",
            ));
        }
        if let Some(classes) = self.classes {
            match is_of_any(&element, classes)? {
                true => self.streams_kept += 1,
                false => self.streams_left += 1,
            }
        }
        let (inner, reads) = self.read(&element)?;
        let read = Read {
            object: Some(element.unbind()),
            inner: reads,
        };
        Ok((
            StreamEvent::new(offset, StreamElement::Stream(Box::new(inner))),
            read,
        ))
    }
}

/// Where each element and stream of a crate stream stands in memory, with
/// its object, for finding the objects a crate tree answers with.
#[derive(Default)]
struct Objects {
    elements: HashMap<usize, Py<PyAny>>,
    streams: HashMap<usize, Py<PyAny>>,
}

impl Objects {
    /// Pairs each event of a crate stream with its object, nested streams
    /// too.
    fn gather(&mut self, py: Python<'_>, stream: &RsStream, reads: &[Read]) {
        for (event, read) in stream.events().iter().zip(reads) {
            let Some(object) = &read.object else {
                continue;
            };
            let element = event.element();
            self.elements
                .insert(std::ptr::from_ref(element) as usize, object.clone_ref(py));
            if let StreamElement::Stream(inner) = element {
                self.streams
                    .insert(std::ptr::from_ref(&**inner) as usize, object.clone_ref(py));
                self.gather(py, inner, &read.inner);
            }
        }
    }

    /// The object of a crate stream, the one read for the top.
    fn stream<'py>(&self, py: Python<'py>, stream: &RsStream) -> Option<Bound<'py, PyAny>> {
        self.streams
            .get(&(std::ptr::from_ref(stream) as usize))
            .map(|object| object.bind(py).clone())
    }
}

/// music21's `flatten`: `True`, `False` or `'semiFlat'`.
fn flatten_of(flatten: Option<&Bound<'_, PyAny>>) -> PyResult<(Flatten, bool)> {
    let Some(flatten) = flatten else {
        return Ok((Flatten::Flat, true));
    };
    if let Ok(said) = flatten.extract::<String>() {
        if said == "semiFlat" {
            return Ok((Flatten::SemiFlat, true));
        }
        return Err(PyValueError::new_err(format!(
            "flatten must be True, False or 'semiFlat', not {said:?}"
        )));
    }
    Ok((Flatten::Flat, flatten.is_truthy()?))
}

/// music21's `Stream.asTimespans`: every element of a stream, those of the
/// streams it holds too, as a timespan in a tree.
pub(crate) fn as_timespans<'py>(
    stream: &Bound<'py, PyAny>,
    flatten: Option<&Bound<'py, PyAny>>,
    classList: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, TimespanTree>> {
    let py = stream.py();
    let (flatten, nested) = flatten_of(flatten)?;
    // An empty class list is music21's none at all.
    let classes = match classList {
        Some(classes) if !classes.is_none() && classes.is_truthy()? => Some(classes),
        _ => None,
    };
    let mut reading = Asked {
        classes,
        nested,
        streams_kept: 0,
        streams_left: 0,
    };
    let (read, reads) = reading.read(stream)?;
    let keep = keep_of(&reading, flatten)?;

    let mut objects = Objects::default();
    objects
        .streams
        .insert(std::ptr::from_ref(&read) as usize, stream.clone().unbind());
    objects.gather(py, &read, &reads);
    let spans = {
        let tree = rs_tree::as_timespans(&read, flatten, keep);
        tree.timespans()
            .iter()
            .map(|span| span_object(py, span, &objects))
            .collect::<PyResult<Vec<_>>>()?
    };

    // The timespans go into music21's tree, then the reading is kept for
    // the verticalities, as inserting forgets it.
    let tree = Bound::new(py, TimespanTree::of(stream)?)?;
    tree.call_method1("insert", (PyList::new(py, &spans)?,))?;
    tree.borrow_mut().reading = Reading::Read(Box::new(ReadStream {
        stream: read,
        flatten,
        keep,
        spans,
    }));
    Ok(tree)
}

/// The crate's filter for what a class list kept: the markers out, and in a
/// semi-flat read the streams too where none answered to it.
fn keep_of(reading: &Asked<'_, '_>, flatten: Flatten) -> PyResult<Keep> {
    if reading.classes.is_none() {
        return Ok(None);
    }
    if flatten == Flatten::Flat || reading.streams_left == 0 {
        return Ok(Some(unmarked));
    }
    if reading.streams_kept == 0 {
        return Ok(Some(unmarked_leaf));
    }
    Err(PyNotImplementedError::new_err(
        "asTimespans(flatten='semiFlat') with a classList keeping some streams \
         and not others is not ported to music21-rs",
    ))
}

/// The timespan object for one of the crate's.
fn span_object(
    py: Python<'_>,
    span: &RsSpan<'_>,
    objects: &Objects,
) -> PyResult<Py<ElementTimespan>> {
    let element = objects
        .elements
        .get(&(std::ptr::from_ref(span.element()) as usize))
        .map(|object| object.clone_ref(py))
        .unwrap_or_else(|| py.None());
    let parentage: Vec<Bound<'_, PyAny>> = span
        .parentage()
        .iter()
        .map(|stream| {
            objects
                .stream(py, stream)
                .unwrap_or_else(|| py.None().into_bound(py))
        })
        .collect();
    let times = Timespan {
        offset: span.offset(),
        end_time: span.end_time(),
    };
    let value = PyClassInitializer::from(times).add_subclass(ElementTimespan {
        element,
        parent_offset: Some(span.parent_offset()),
        parent_end_time: Some(span.parent_end_time()),
        parentage: PyTuple::new(py, parentage)?.unbind(),
        measure: span.measure_number(),
    });
    if !span.is_pitched() {
        return Py::new(py, value);
    }
    let pitched = Bound::new(py, value.add_subclass(PitchedTimespan))?;
    Ok(pitched.into_any().cast_into::<ElementTimespan>()?.unbind())
}

/// The crate's reading of the stream a tree was read from: the stream as
/// the crate holds it, how it was read, and the timespan objects in the
/// crate tree's order.
pub(crate) struct ReadStream {
    stream: RsStream,
    flatten: Flatten,
    keep: Keep,
    spans: Vec<Py<ElementTimespan>>,
}

/// Whether a tree is still as read from its stream, which its
/// verticalities need.
pub(crate) enum Reading {
    /// As read, with the crate's reading.
    Read(Box<ReadStream>),
    /// Built by hand, or edited since it was read.
    Unread,
}

/// Every element of a stream as a stretch of time, in music21's order --
/// by where each starts, then where it ends, then as the stream holds them
/// -- and what sounds together at each offset: music21's
/// `tree.timespanTree.TimespanTree`, read from a stream by
/// `Stream.asTimespans`.
#[pyclass(
    name = "TimespanTree",
    module = "music21.tree.timespanTree",
    extends = trees::OffsetTree,
    subclass
)]
pub struct TimespanTree {
    reading: Reading,
}

impl TimespanTree {
    /// An empty one of a stream.
    pub(crate) fn of(source: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        Ok(trees::OffsetTree::of(source)?.add_subclass(Self {
            reading: Reading::Unread,
        }))
    }

    /// Forgets the reading, once the tree is edited.
    pub(crate) fn forget_reading(&mut self) {
        self.reading = Reading::Unread;
    }

    /// The reading, or a refusal for a tree not as read.
    fn read(&self) -> PyResult<&ReadStream> {
        match &self.reading {
            Reading::Read(read) => Ok(read),
            Reading::Unread => Err(PyNotImplementedError::new_err(
                "the verticalities of a TimespanTree built by hand or edited since \
                 asTimespans() read it are not ported to music21-rs",
            )),
        }
    }

    /// The crate's tree of the stream read: the same timespans, in the same
    /// order, as `spans`.
    fn crate_tree(&self) -> PyResult<RsTree<'_>> {
        let read = self.read()?;
        Ok(rs_tree::as_timespans(&read.stream, read.flatten, read.keep))
    }

    /// The timespan object for one of the crate tree's.
    fn span(
        &self,
        py: Python<'_>,
        tree: &RsTree<'_>,
        span: &RsSpan<'_>,
    ) -> PyResult<Py<ElementTimespan>> {
        // The crate answers with references into its list of timespans, so
        // where one stands in memory says its place in the list.
        let start = tree.timespans().as_ptr() as usize;
        let place = (std::ptr::from_ref(span) as usize - start) / std::mem::size_of::<RsSpan<'_>>();
        Ok(self.read()?.spans[place].clone_ref(py))
    }
}

#[pymethods]
impl TimespanTree {
    #[new]
    #[pyo3(signature = (elements = None, source = None))]
    fn py_new(
        py: Python<'_>,
        elements: Option<&Bound<'_, PyAny>>,
        source: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let built = trees::OffsetTree::py_new(py, elements, source)?;
        Ok(built.add_subclass(Self {
            reading: Reading::Unread,
        }))
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        if let Reading::Read(read) = &self.reading {
            for span in &read.spans {
                visit.call(span)?;
            }
        }
        Ok(())
    }

    /// The stream the tree was read from, so a tree can stand where a
    /// timespan does.
    #[getter]
    fn element<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        slf.getattr("source")
    }

    /// Where the tree starts, so it can stand where a timespan does.
    #[getter]
    fn offset<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        slf.call_method0("lowestPosition")
    }

    /// How a node's timespans are sorted: by where each ends.
    #[staticmethod]
    fn _insertCorePayloadSortKey<'py>(x: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        x.getattr("endTime")
    }

    /// Where a timespan stands among the tree's.
    fn index(slf: &Bound<'_, Self>, span: &Bound<'_, PyAny>) -> PyResult<i64> {
        let offset = span.getattr("offset")?;
        let tree = slf.as_any();
        let found = tree.call_method1("getNodeByPosition", (&offset,))?;
        if !found.is_none() {
            let payload = found.getattr("payload")?;
            if payload.contains(span)? {
                let start: i64 = found.getattr("payloadElementsStartIndex")?.extract()?;
                let place: i64 = payload.call_method1("index", (span,))?.extract()?;
                return Ok(place + start);
            }
        }
        Err(PyValueError::new_err(format!(
            "{} not in Tree at offset {}.",
            span.str()?,
            offset.str()?
        )))
    }

    /// Splits every timespan sounding across each offset given into the
    /// two either side of it, in place.
    fn splitAt(slf: &Bound<'_, Self>, offsets: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let offsets = match offsets.try_iter() {
            Ok(many) => many.collect::<PyResult<Vec<_>>>()?,
            Err(_) => vec![offsets.clone()],
        };
        for offset in offsets {
            let overlaps = slf.call_method1("elementsOverlappingOffset", (&offset,))?;
            for overlap in overlaps.try_iter()? {
                let overlap = overlap?;
                slf.call_method1("removeTimespan", (&overlap,))?;
                let shards = overlap.call_method1("splitAt", (&offset,))?;
                slf.call_method1(
                    "insert",
                    (PyList::new(
                        py,
                        shards.try_iter()?.collect::<PyResult<Vec<_>>>()?,
                    )?,),
                )?;
            }
        }
        Ok(())
    }

    /// A tree of each part's timespans, by part, in the order of
    /// `allParts`.
    fn toPartwiseTimespanTrees<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let grouped = PyDict::new(py);
        for part in Self::allParts(slf)? {
            grouped.set_item(part, PyList::empty(py))?;
        }
        for span in slf.try_iter()? {
            let span = span?;
            let part = span.getattr("part")?;
            if let Some(held) = grouped.get_item(&part)? {
                held.call_method1("append", (span,))?;
            }
        }
        let trees = PyDict::new(py);
        for (part, spans) in grouped.iter() {
            let tree = py.get_type::<Self>().call0()?;
            tree.call_method1("insert", (spans,))?;
            trees.set_item(part, tree)?;
        }
        Ok(trees)
    }

    /// Takes out timespans, as `removeElements` does.
    #[pyo3(signature = (elements, offsets = None, runUpdate = true))]
    fn removeTimespanList(
        slf: &Bound<'_, Self>,
        elements: &Bound<'_, PyAny>,
        offsets: Option<&Bound<'_, PyAny>>,
        runUpdate: bool,
    ) -> PyResult<()> {
        slf.call_method1("removeElements", (elements, offsets, runUpdate))?;
        Ok(())
    }

    /// Takes out a timespan or several, as `removeElements` does.
    #[pyo3(signature = (elements, offsets = None, runUpdate = true))]
    fn removeTimespan(
        slf: &Bound<'_, Self>,
        elements: &Bound<'_, PyAny>,
        offsets: Option<&Bound<'_, PyAny>>,
        runUpdate: bool,
    ) -> PyResult<()> {
        Self::removeTimespanList(slf, elements, offsets, runUpdate)
    }
    /// Where a timespan ends: what a tree asks of what it holds.
    #[staticmethod]
    fn elementEndTime<'py>(
        el: &Bound<'py, PyAny>,
        unused_node: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = unused_node;
        el.getattr("endTime")
    }

    /// The verticality at an offset where something starts there, or else
    /// the one before.
    fn getVerticalityAtOrBefore<'py>(
        slf: &Bound<'py, Self>,
        offset: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let verticality = slf.call_method1("getVerticalityAt", (offset,))?;
        if verticality.getattr("startTimespans")?.is_truthy()? {
            return Ok(Some(verticality));
        }
        let previous = verticality.getattr("previousVerticality")?;
        Ok(Some(previous).filter(|previous| !previous.is_none()))
    }

    /// The verticality at each offset a timespan starts at, in order or
    /// `reverse`d, each worked out from the last as the tree then stands.
    #[pyo3(signature = (reverse = false))]
    fn iterateVerticalities(
        slf: &Bound<'_, Self>,
        #[pyo3(from_py_with = crate::truthy)] reverse: bool,
    ) -> verticality::VerticalityWalk {
        verticality::VerticalityWalk::of(slf.as_any(), direction(reverse))
    }

    /// Runs of `n` verticalities, each starting one after the last, in
    /// order or `reverse`d; with `padEnd`, the last runs are filled out
    /// with empty verticalities at the tree's end.
    #[pyo3(signature = (n = 3, *, reverse = false, padEnd = false))]
    fn iterateVerticalitiesNwise(
        slf: &Bound<'_, Self>,
        n: i64,
        #[pyo3(from_py_with = crate::truthy)] reverse: bool,
        #[pyo3(from_py_with = crate::truthy)] padEnd: bool,
    ) -> PyResult<verticality::VerticalityRuns> {
        let Ok(size) = usize::try_from(n) else {
            return Err(TreeException::new_err(format!(
                "The number of verticalities in the group must be at least one. Got {n}"
            )));
        };
        if size == 0 {
            return Err(TreeException::new_err(
                "The number of verticalities in the group must be at least one. Got 0",
            ));
        }
        verticality::VerticalityRuns::of(slf.as_any(), size, direction(reverse), padEnd)
    }

    /// Each run of verticalities from one whose chord is consonant to the
    /// next that is, with at least one between.
    fn iterateConsonanceBoundedVerticalities<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let consonant = |verticality: &Bound<'py, PyAny>| -> PyResult<bool> {
            verticality
                .call_method0("toChord")?
                .call_method0("isConsonant")?
                .is_truthy()
        };
        let out = PyList::empty(py);
        let mut run: Vec<Bound<'py, PyAny>> = Vec::new();
        for verticality in Self::iterateVerticalities(slf, false)
            .into_pyobject(py)?
            .try_iter()?
        {
            let verticality = verticality?;
            let bounds = consonant(&verticality)?;
            if run.is_empty() {
                if bounds {
                    run.push(verticality);
                }
                continue;
            }
            run.push(verticality.clone());
            if bounds {
                if run.len() > 2 {
                    out.append(PyTuple::new(py, &run)?)?;
                }
                run = vec![verticality];
            }
        }
        Ok(out.try_iter()?.into_any())
    }

    /// The next timespan starting after this one in the same stream of one
    /// of these classes, a part where none are given.
    #[pyo3(signature = (pitchedTimespan, classList = None))]
    fn findNextPitchedTimespanInSameStreamByClass<'py>(
        slf: &Bound<'py, Self>,
        pitchedTimespan: &Bound<'py, PyAny>,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        same_stream(slf, pitchedTimespan, classList, "nextVerticality")
    }

    /// The timespan starting before this one in the same stream of one of
    /// these classes, a part where none are given.
    #[pyo3(signature = (pitchedTimespan, classList = None))]
    fn findPreviousPitchedTimespanInSameStreamByClass<'py>(
        slf: &Bound<'py, Self>,
        pitchedTimespan: &Bound<'py, PyAny>,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        same_stream(slf, pitchedTimespan, classList, "previousVerticality")
    }

    /// Each part's timespans across a run of verticalities, as a
    /// dictionary of the part and its `Horizontality`.
    #[staticmethod]
    fn unwrapVerticalities<'py>(verticalities: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
        verticality::VerticalitySequence::py_new(verticalities)?.unwrap(verticalities.py())
    }

    /// The parts the timespans are in: those of the stream read in its
    /// order, then any other by its instrument's `partId`.
    fn allParts<'py>(slf: &Bound<'py, Self>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut parts: Vec<Bound<'py, PyAny>> = Vec::new();
        for span in slf.try_iter()? {
            let part = span?.getattr("part")?;
            if !part.is_none() && !parts.iter().any(|known| known.is(&part)) {
                parts.push(part);
            }
        }
        let order: Vec<Bound<'py, PyAny>> = match slf.getattr("source")?.getattr("parts") {
            Ok(held) => held.try_iter()?.collect::<PyResult<_>>()?,
            Err(_) => Vec::new(),
        };
        let mut keyed = Vec::new();
        for part in parts {
            let place = order
                .iter()
                .position(|known| known.is(&part))
                .unwrap_or(order.len());
            let id = part_id(&part);
            keyed.push(((place, id.is_none(), id.unwrap_or_default()), part));
        }
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(keyed.into_iter().map(|(_, part)| part).collect())
    }

    /// The most timespans starting at or sounding across any one offset a
    /// timespan starts at, `None` for an empty tree.
    fn maximumOverlap(slf: &Bound<'_, Self>) -> PyResult<Option<usize>> {
        let py = slf.py();
        let mut most: Option<usize> = None;
        for verticality in Self::iterateVerticalities(slf, false)
            .into_pyobject(py)?
            .try_iter()?
        {
            let sounding = verticality?.getattr("startAndOverlapTimespans")?.len()?;
            most = Some(most.map_or(sounding, |most| most.max(sounding)));
        }
        Ok(most)
    }
}

/// The `partId` of a part's instrument, as text, where it has one.
fn part_id(part: &Bound<'_, PyAny>) -> Option<String> {
    let instrument = part.call_method0("getInstrument").ok()?;
    let id = instrument.getattr("partId").ok()?;
    if id.is_none() {
        return None;
    }
    id.str().ok().map(|text| text.to_string())
}

/// Which way a walk of verticalities goes, from music21's `reverse`.
fn direction(reverse: bool) -> verticality::Direction {
    match reverse {
        true => verticality::Direction::Backward,
        false => verticality::Direction::Forward,
    }
}

/// The next or previous timespan in the same stream of one of these
/// classes as this one, found by walking the verticalities from its own
/// with `step`, `nextVerticality` or `previousVerticality`.
fn same_stream<'py>(
    tree: &Bound<'py, TimespanTree>,
    span: &Bound<'py, PyAny>,
    classList: Option<&Bound<'py, PyAny>>,
    step: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = span.py();
    if span.cast::<PitchedTimespan>().is_err() {
        return Err(TreeException::new_err(format!(
            "PitchedTimespan {}, must be a PitchedTimespan",
            span.repr()?
        )));
    }
    let classes = match classList {
        Some(classes) if !classes.is_none() => classes.clone(),
        _ => PyString::new(py, "Part").into_any(),
    };
    let home = span.call_method1("getParentageByClass", (&classes,))?;
    let mut verticality = tree.call_method1("getVerticalityAt", (span.getattr("offset")?,))?;
    loop {
        verticality = verticality.getattr(step)?;
        if verticality.is_none() {
            return Ok(None);
        }
        for candidate in verticality.getattr("startTimespans")?.try_iter()? {
            let candidate = candidate?;
            let stream = candidate.call_method1("getParentageByClass", (&classes,))?;
            if stream.is(&home) {
                return Ok(Some(candidate));
            }
        }
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    core::register(m)?;
    node::register(m)?;
    trees::register(m)?;
    m.add_class::<Timespan>()?;
    m.add_class::<TimespanTree>()?;
    m.add_class::<ElementTimespan>()?;
    m.add_class::<PitchedTimespan>()?;
    analysis::register(m)?;
    verticality::register(m)?;
    Ok(())
}
