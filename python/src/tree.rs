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
//! What edits a tree in place (`insert`, `removeTimespan`, `splitAt`) is not
//! here: the tree is read from its stream and stays as read.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::collections::HashMap;

use pyo3::exceptions::{PyIndexError, PyNotImplementedError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PySet, PySlice, PyString, PyTuple};

use music21_rs_crate::braille::SegmentDivision;
use music21_rs_crate::stream::{Stream as RsStream, StreamElement, StreamEvent, StreamKind};
use music21_rs_crate::tree::{
    self as rs_tree, ElementTimespan as RsSpan, Flatten, Horizontality as RsHorizontality,
    TimespanTree as RsTree, Verticality as RsVerticality,
};

use crate::duration::op_frac;
use crate::stream::{
    answers_to_class, element_object, element_read, element_written, stream_error,
};

/// An offset as music21's timespans hold one: a float, as its `Timespan`
/// makes every offset it is given.
fn float_of(py: Python<'_>, value: f64) -> PyResult<Bound<'_, PyAny>> {
    Ok(PyFloat::new(py, value).into_any())
}

/// Which elements of a crate tree the tree keeps, as the crate's filter.
type Keep = Option<fn(&StreamElement) -> bool>;

/// One element of a stream as the stretch of time it sounds for: music21's
/// `tree.spans.ElementTimespan`.
#[pyclass(
    name = "ElementTimespan",
    module = "music21.tree.spans",
    subclass,
    frozen
)]
pub struct ElementTimespan {
    /// The element, the stream's own object.
    element: Py<PyAny>,
    offset: f64,
    end_time: f64,
    parent_offset: f64,
    parent_end_time: f64,
    /// The streams it stands in, the one holding it first.
    parentage: Py<PyTuple>,
    /// The number of the measure it is in.
    measure: Option<i32>,
}

/// A timespan of a note, chord or stream, whose pitches it answers:
/// music21's `tree.spans.PitchedTimespan`.
#[pyclass(name = "PitchedTimespan", module = "music21.tree.spans", extends = ElementTimespan, frozen)]
pub struct PitchedTimespan;

#[pymethods]
impl ElementTimespan {
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

    /// Where it starts in the stream read.
    #[getter]
    fn offset<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.offset)
    }

    /// Where it ends.
    #[getter]
    fn endTime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.end_time)
    }

    /// How long it lasts.
    #[getter]
    fn quarterLength<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.end_time - self.offset)
    }

    /// Where the stream holding it starts.
    #[getter]
    fn parentOffset(&self) -> f64 {
        self.parent_offset
    }

    /// Where the stream holding it ends.
    #[getter]
    fn parentEndTime(&self) -> f64 {
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
        let span = slf.get();
        Ok(format!(
            "<{} ({} to {}) {}>",
            slf.get_type().name()?,
            float_of(py, span.offset)?.str()?,
            float_of(py, span.end_time)?.str()?,
            span.element.bind(py).repr()?
        ))
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
struct Reading<'a, 'py> {
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
        return Ok(StreamElement::MetronomeMark(
            music21_rs_crate::MetronomeMark::new(60.0),
        ));
    }
    let duration = music21_rs_crate::Duration::new(length).map_err(|error| stream_error(&error))?;
    Ok(StreamElement::Rest(music21_rs_crate::Rest::new(duration)))
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

impl Reading<'_, '_> {
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
pub(crate) fn as_timespans(
    stream: &Bound<'_, PyAny>,
    flatten: Option<&Bound<'_, PyAny>>,
    classList: Option<&Bound<'_, PyAny>>,
) -> PyResult<TimespanTree> {
    let py = stream.py();
    let (flatten, nested) = flatten_of(flatten)?;
    // An empty class list is music21's none at all.
    let classes = match classList {
        Some(classes) if !classes.is_none() && classes.is_truthy()? => Some(classes),
        _ => None,
    };
    let mut reading = Reading {
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
    Ok(TimespanTree {
        source: stream.clone().unbind(),
        stream: Box::new(read),
        flatten,
        keep,
        spans,
    })
}

/// The crate's filter for what a class list kept: the markers out, and in a
/// semi-flat read the streams too where none answered to it.
fn keep_of(reading: &Reading<'_, '_>, flatten: Flatten) -> PyResult<Keep> {
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
    let value = ElementTimespan {
        element,
        offset: span.offset(),
        end_time: span.end_time(),
        parent_offset: span.parent_offset(),
        parent_end_time: span.parent_end_time(),
        parentage: PyTuple::new(py, parentage)?.unbind(),
        measure: span.measure_number(),
    };
    if !span.is_pitched() {
        return Py::new(py, value);
    }
    let pitched = Bound::new(
        py,
        PyClassInitializer::from(value).add_subclass(PitchedTimespan),
    )?;
    Ok(pitched.into_any().cast_into::<ElementTimespan>()?.unbind())
}

/// Every element of a stream as a stretch of time, in music21's order --
/// by where each starts, then where it ends, then as the stream holds them
/// -- and what sounds together at each offset: music21's
/// `tree.timespanTree.TimespanTree`, read from a stream by
/// `Stream.asTimespans`.
#[pyclass(name = "TimespanTree", module = "music21.tree.timespanTree", frozen)]
pub struct TimespanTree {
    /// The stream read.
    source: Py<PyAny>,
    /// The same, as the crate reads it.
    stream: Box<RsStream>,
    flatten: Flatten,
    keep: Keep,
    /// The timespans, in the crate tree's order.
    spans: Vec<Py<ElementTimespan>>,
}

impl TimespanTree {
    /// The crate's tree of the stream read: the same timespans, in the same
    /// order, as `spans`.
    fn crate_tree(&self) -> RsTree<'_> {
        rs_tree::as_timespans(&self.stream, self.flatten, self.keep)
    }

    /// The timespan object for one of the crate tree's.
    fn span(&self, py: Python<'_>, tree: &RsTree<'_>, span: &RsSpan<'_>) -> Py<ElementTimespan> {
        // The crate answers with references into its list of timespans, so
        // where one stands in memory says its place in the list.
        let start = tree.timespans().as_ptr() as usize;
        let place = (std::ptr::from_ref(span) as usize - start) / std::mem::size_of::<RsSpan<'_>>();
        self.spans[place].clone_ref(py)
    }

    /// Several of the crate tree's timespans, as a tuple of their objects.
    fn spans_of<'py>(
        &self,
        py: Python<'py>,
        tree: &RsTree<'_>,
        spans: &[&RsSpan<'_>],
    ) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, spans.iter().map(|span| self.span(py, tree, span)))
    }

    /// Where a timespan object stands among the timespans.
    fn place_of(&self, py: Python<'_>, span: &Bound<'_, PyAny>) -> Option<usize> {
        self.spans.iter().position(|held| held.bind(py).is(span))
    }

    /// The positions given, as music21 answers them.
    fn positions<'py>(py: Python<'py>, offsets: Vec<f64>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        offsets
            .into_iter()
            .map(|offset| float_of(py, offset))
            .collect()
    }
}

/// A position as music21 answers one: a quarter length, or none.
fn position_of<'py>(py: Python<'py>, offset: Option<f64>) -> PyResult<Option<Bound<'py, PyAny>>> {
    offset.map(|offset| float_of(py, offset)).transpose()
}

/// A verticality of a tree, for one the crate answers with.
fn verticality_of(
    py: Python<'_>,
    tree: &Bound<'_, TimespanTree>,
    verticality: &RsVerticality<'_, '_>,
) -> PyResult<TreeVerticality> {
    // A run padded out ends with verticalities that hold nothing; any other
    // holds what starts there.
    let sentinel = verticality.start_timespans().is_empty();
    Ok(TreeVerticality {
        tree: tree.clone().unbind(),
        offset: verticality.offset(),
        given: float_of(py, verticality.offset())?.unbind(),
        sentinel,
    })
}

/// Runs of verticalities, each as music21's `VerticalitySequence`.
fn sequences_of<'py>(
    tree: &Bound<'py, TimespanTree>,
    runs: Vec<Vec<RsVerticality<'_, '_>>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = tree.py();
    let out = PyList::empty(py);
    for run in runs {
        let members = run
            .iter()
            .map(|verticality| verticality_of(py, tree, verticality))
            .collect::<PyResult<Vec<_>>>()?;
        let sequence = VerticalitySequence {
            verticalities: PyTuple::new(py, members)?.unbind(),
        };
        out.append(sequence)?;
    }
    Ok(out.try_iter()?.into_any())
}

#[pymethods]
impl TimespanTree {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.source)?;
        for span in &self.spans {
            visit.call(span)?;
        }
        Ok(())
    }

    /// How many timespans there are.
    fn __len__(&self) -> usize {
        self.spans.len()
    }

    /// The timespans, in order.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let spans = PyList::new(py, self.spans.iter().map(|span| span.clone_ref(py)))?;
        Ok(spans.try_iter()?.into_any())
    }

    /// A timespan by its place, or a list of them by a slice.
    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        index: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let spans = PyList::new(py, self.spans.iter().map(|span| span.clone_ref(py)))?;
        if index.is_instance_of::<PySlice>() {
            return spans.as_any().get_item(index);
        }
        let Ok(place) = index.extract::<isize>() else {
            return Err(PyTypeError::new_err(format!(
                "Indices must be integers or slices, got {}",
                index.str()?
            )));
        };
        let length = self.spans.len() as isize;
        let place = if place < 0 { place + length } else { place };
        if place < 0 || place >= length {
            return Err(PyIndexError::new_err(""));
        }
        Ok(self.spans[place as usize].bind(py).clone().into_any())
    }

    /// Whether a timespan is one of this tree's.
    fn __contains__(&self, py: Python<'_>, span: &Bound<'_, PyAny>) -> bool {
        self.place_of(py, span).is_some()
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let low = float_of(py, self.lowest())?;
        let end = float_of(py, self.end())?;
        Ok(format!(
            "<TimespanTree {{{}}} ({} to {}) {}>",
            self.spans.len(),
            low.str()?,
            end.str()?,
            self.source.bind(py).repr()?
        ))
    }

    /// The tree, then each timespan on a line of its own.
    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        let mut lines = vec![self.__repr__(py)?];
        for span in &self.spans {
            lines.push(format!("\t{}", span.bind(py).repr()?));
        }
        Ok(lines.join("\n"))
    }

    /// The stream the tree was read from.
    #[getter]
    fn source(&self, py: Python<'_>) -> Py<PyAny> {
        self.source.clone_ref(py)
    }

    /// The same, so a tree can stand where a timespan does.
    #[getter]
    fn element(&self, py: Python<'_>) -> Py<PyAny> {
        self.source.clone_ref(py)
    }

    /// Where the earliest timespan starts, `-inf` for an empty tree.
    fn lowestPosition<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.lowest())
    }

    /// Where the latest starts, `-inf` for an empty tree.
    fn highestPosition<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let highest = self.crate_tree().highest_position();
        float_of(py, highest.unwrap_or(f64::NEG_INFINITY))
    }

    /// Where the last to end ends, `inf` for an empty tree.
    #[getter]
    fn endTime<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.end())
    }

    /// Where the tree starts, so it can stand where a timespan does.
    #[getter]
    fn offset<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        float_of(py, self.lowest())
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

    /// Where a timespan stands among the tree's.
    fn index(&self, py: Python<'_>, span: &Bound<'_, PyAny>) -> PyResult<usize> {
        if let Some(place) = self.place_of(py, span) {
            return Ok(place);
        }
        Err(PyValueError::new_err(format!(
            "{} not in Tree at offset {}.",
            span.str()?,
            span.getattr("offset")?.str()?
        )))
    }

    /// The first offset after this one a timespan starts at.
    fn getPositionAfter<'py>(
        &self,
        py: Python<'py>,
        position: f64,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        position_of(py, self.crate_tree().position_after(position))
    }

    /// The last offset before this one a timespan starts at.
    fn getPositionBefore<'py>(
        &self,
        py: Python<'py>,
        position: f64,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        position_of(py, self.crate_tree().position_before(position))
    }

    /// The timespans starting at an offset.
    fn elementsStartingAt<'py>(
        &self,
        py: Python<'py>,
        position: f64,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let tree = self.crate_tree();
        self.spans_of(py, &tree, &tree.starting_at(position))
    }

    /// The timespans ending at an offset.
    fn elementsStoppingAt<'py>(
        &self,
        py: Python<'py>,
        offset: f64,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let tree = self.crate_tree();
        self.spans_of(py, &tree, &tree.stopping_at(offset))
    }

    /// The timespans sounding across an offset, started before it and
    /// ending after.
    fn elementsOverlappingOffset<'py>(
        &self,
        py: Python<'py>,
        offset: f64,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let tree = self.crate_tree();
        self.spans_of(py, &tree, &tree.overlapping(offset))
    }

    /// Each offset a timespan starts at, once, in order.
    fn allOffsets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, Self::positions(py, self.crate_tree().all_offsets())?)
    }

    /// Each offset a timespan starts or ends at, once, in order.
    fn allTimePoints<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(
            py,
            Self::positions(py, self.crate_tree().all_time_points())?,
        )
    }

    /// The offsets a timespan starts at, or with `includeStopPoints` also
    /// ends at, where another sounds across; with `returnVerticality`, each
    /// as a dictionary of it and its verticality.
    #[pyo3(signature = (includeStopPoints = false, returnVerticality = false))]
    fn overlapTimePoints<'py>(
        slf: &Bound<'py, Self>,
        includeStopPoints: bool,
        returnVerticality: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let points = slf
            .get()
            .crate_tree()
            .overlap_time_points(includeStopPoints);
        let out = PyList::empty(py);
        for point in points {
            let offset = float_of(py, point)?;
            if !returnVerticality {
                out.append(offset)?;
                continue;
            }
            let verticality = Self::getVerticalityAt(slf, &offset)?;
            let entry = PyDict::new(py);
            entry.set_item(offset, verticality)?;
            out.append(entry)?;
        }
        Ok(out)
    }

    /// What starts, stops and sounds across an offset.
    fn getVerticalityAt(
        slf: &Bound<'_, Self>,
        offset: &Bound<'_, PyAny>,
    ) -> PyResult<TreeVerticality> {
        Ok(TreeVerticality {
            tree: slf.clone().unbind(),
            offset: offset.extract()?,
            given: offset.clone().unbind(),
            sentinel: false,
        })
    }

    /// The verticality at an offset where something starts there, or the
    /// one before.
    fn getVerticalityAtOrBefore(
        slf: &Bound<'_, Self>,
        offset: f64,
    ) -> PyResult<Option<TreeVerticality>> {
        let tree = slf.get().crate_tree();
        tree.verticality_at_or_before(offset)
            .map(|verticality| verticality_of(slf.py(), slf, &verticality))
            .transpose()
    }

    /// The verticality at each offset a timespan starts at, in order or
    /// `reverse`d.
    #[pyo3(signature = (reverse = false))]
    fn iterateVerticalities<'py>(
        slf: &Bound<'py, Self>,
        reverse: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let tree = slf.get().crate_tree();
        let mut verticalities = tree.verticalities();
        if reverse {
            verticalities.reverse();
        }
        let out = PyList::empty(py);
        for verticality in &verticalities {
            out.append(verticality_of(py, slf, verticality)?)?;
        }
        Ok(out.try_iter()?.into_any())
    }

    /// Runs of `n` verticalities, each starting one after the last, in
    /// order or `reverse`d; with `padEnd`, the last runs are filled out
    /// with empty verticalities at the tree's end.
    #[pyo3(signature = (n = 3, *, reverse = false, padEnd = false))]
    fn iterateVerticalitiesNwise<'py>(
        slf: &Bound<'py, Self>,
        n: i64,
        reverse: bool,
        padEnd: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let tree = slf.get().crate_tree();
        // music21 reads the runs off what iterating finds, so an empty
        // tree has none unless padded.
        if tree.is_empty() && n > 0 && !padEnd {
            return Ok(PyList::empty(slf.py()).try_iter()?.into_any());
        }
        let count = usize::try_from(n).unwrap_or(0);
        let runs = tree
            .verticalities_nwise(count, reverse, padEnd)
            .map_err(|error| stream_error(&error))?;
        sequences_of(slf, runs)
    }

    /// Each run of verticalities from one whose chord is consonant to the
    /// next that is, with at least one between.
    fn iterateConsonanceBoundedVerticalities<'py>(
        slf: &Bound<'py, Self>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let tree = slf.get().crate_tree();
        let runs = tree
            .consonance_bounded_verticalities()
            .map_err(|error| stream_error(&error))?;
        let out = PyList::empty(py);
        for run in runs {
            let members = run
                .iter()
                .map(|verticality| verticality_of(py, slf, verticality))
                .collect::<PyResult<Vec<_>>>()?;
            out.append(PyTuple::new(py, members)?)?;
        }
        Ok(out.try_iter()?.into_any())
    }

    /// The next timespan starting after this one in the same stream of one
    /// of these classes, a part where none are given.
    #[pyo3(signature = (pitchedTimespan, classList = None))]
    fn findNextPitchedTimespanInSameStreamByClass<'py>(
        &self,
        pitchedTimespan: &Bound<'py, PyAny>,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.same_stream(pitchedTimespan, classList, true)
    }

    /// The timespan starting before this one in the same stream of one of
    /// these classes, a part where none are given.
    #[pyo3(signature = (pitchedTimespan, classList = None))]
    fn findPreviousPitchedTimespanInSameStreamByClass<'py>(
        &self,
        pitchedTimespan: &Bound<'py, PyAny>,
        classList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.same_stream(pitchedTimespan, classList, false)
    }

    /// Each part's timespans across a run of verticalities, as a
    /// dictionary of the part and its `Horizontality`.
    #[staticmethod]
    fn unwrapVerticalities<'py>(verticalities: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
        unwrap_all(verticalities)
    }

    /// The parts the timespans are in: those of the stream read in its
    /// order, then any other by its instrument's `partId`.
    fn allParts<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut parts: Vec<Bound<'py, PyAny>> = Vec::new();
        for span in &self.spans {
            if let Some(part) = span.get().part(py)?
                && !parts.iter().any(|known| known.is(&part))
            {
                parts.push(part);
            }
        }
        let order: Vec<Bound<'py, PyAny>> = match self.source.bind(py).getattr("parts") {
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
    fn maximumOverlap(&self) -> Option<usize> {
        self.crate_tree().maximum_overlap()
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

impl TimespanTree {
    /// Where the earliest timespan starts.
    fn lowest(&self) -> f64 {
        self.crate_tree()
            .lowest_position()
            .unwrap_or(f64::NEG_INFINITY)
    }

    /// Where the last to end ends.
    fn end(&self) -> f64 {
        self.crate_tree().end_time().unwrap_or(f64::INFINITY)
    }

    /// The next or previous timespan in the same stream of one of these
    /// classes as this one, starting at a later or an earlier offset.
    fn same_stream<'py>(
        &self,
        span: &Bound<'py, PyAny>,
        classList: Option<&Bound<'py, PyAny>>,
        forward: bool,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = span.py();
        let Ok(found) = span.cast::<PitchedTimespan>() else {
            return Err(stream_error(&music21_rs_crate::Error::Tree(format!(
                "PitchedTimespan {}, must be a PitchedTimespan",
                span.repr()?
            ))));
        };
        let classes = match classList {
            Some(classes) if !classes.is_none() => classes.clone(),
            _ => PyString::new(py, "Part").into_any(),
        };
        let held = found.as_super().get();
        let home = held.first_parent(&classes)?;
        let tree = self.crate_tree();
        let mut offset = held.offset;
        loop {
            let next = match forward {
                true => tree.position_after(offset),
                false => tree.position_before(offset),
            };
            let Some(next) = next else {
                return Ok(None);
            };
            for candidate in tree.starting_at(next) {
                let object = self.span(py, &tree, candidate).into_bound(py);
                let stream = object.get().first_parent(&classes)?;
                if same_object(stream.as_ref(), home.as_ref()) {
                    return Ok(Some(object.into_any()));
                }
            }
            offset = next;
        }
    }
}

/// Whether two answers are the same object, or both none.
fn same_object(left: Option<&Bound<'_, PyAny>>, right: Option<&Bound<'_, PyAny>>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.is(right),
        (None, None) => true,
        _ => false,
    }
}

/// What starts, stops and sounds across one offset of a `TimespanTree`:
/// music21's `tree.verticality.Verticality`.
#[pyclass(name = "Verticality", module = "music21.tree.verticality", frozen)]
pub struct TreeVerticality {
    tree: Py<TimespanTree>,
    offset: f64,
    /// The offset as given, which is how music21 shows it.
    given: Py<PyAny>,
    /// Whether it is one a run is padded out with, which holds nothing.
    sentinel: bool,
}

impl TreeVerticality {
    /// The crate's verticality of this one, over the crate tree given.
    fn of<'t, 'a>(&self, tree: &'t RsTree<'a>) -> RsVerticality<'t, 'a> {
        if !self.sentinel {
            return tree.verticality_at(self.offset);
        }

        // The crate pads a run longer than the tree with the empty
        // verticality at its end, which is this one.
        let count = tree.all_offsets().len() + 1;
        tree.verticalities_nwise(count, false, false)
            .ok()
            .and_then(|mut runs| runs.pop())
            .and_then(|mut run| run.pop())
            .unwrap_or_else(|| tree.verticality_at(self.offset))
    }

    /// The tree's timespans of a part of this verticality.
    fn listed<'py>(
        &self,
        py: Python<'py>,
        which: for<'t, 'a> fn(&RsVerticality<'t, 'a>) -> Vec<&'t RsSpan<'a>>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let owner = self.tree.get();
        let tree = owner.crate_tree();
        let verticality = self.of(&tree);
        owner.spans_of(py, &tree, &which(&verticality))
    }

    /// The pitches of what starts or sounds across, each name and octave
    /// once, in the order first met: the elements' own.
    fn pitch_objects<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut names: Vec<String> = Vec::new();
        let mut pitches = Vec::new();
        for span in self.listed(py, sounding)?.iter() {
            if span.cast::<PitchedTimespan>().is_err() {
                continue;
            }
            for pitch in span.getattr("pitches")?.try_iter()? {
                let pitch = pitch?;
                let name: String = pitch.getattr("nameWithOctave")?.extract()?;
                if !names.contains(&name) {
                    names.push(name);
                    pitches.push(pitch);
                }
            }
        }
        Ok(pitches)
    }

    /// The same, lowest first.
    fn sorted_pitches<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let mut keyed = Vec::new();
        for pitch in self.pitch_objects(py)? {
            keyed.push((pitch.getattr("ps")?.extract::<f64>()?, pitch));
        }
        keyed.sort_by(|left, right| left.0.total_cmp(&right.0));
        Ok(keyed.into_iter().map(|(_, pitch)| pitch).collect())
    }
}

/// The crate verticality's timespans starting there.
fn starts<'t, 'a>(verticality: &RsVerticality<'t, 'a>) -> Vec<&'t RsSpan<'a>> {
    verticality.start_timespans().to_vec()
}

/// Those sounding across.
fn overlaps<'t, 'a>(verticality: &RsVerticality<'t, 'a>) -> Vec<&'t RsSpan<'a>> {
    verticality.overlap_timespans().to_vec()
}

/// Those starting, then those sounding across.
fn sounding<'t, 'a>(verticality: &RsVerticality<'t, 'a>) -> Vec<&'t RsSpan<'a>> {
    verticality.start_and_overlap_timespans()
}

/// Those ending there.
fn stops<'t, 'a>(verticality: &RsVerticality<'t, 'a>) -> Vec<&'t RsSpan<'a>> {
    verticality.stop_timespans().to_vec()
}

#[pymethods]
impl TreeVerticality {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.tree)?;
        visit.call(&self.given)
    }

    /// The offset.
    #[getter]
    fn offset(&self, py: Python<'_>) -> Py<PyAny> {
        self.given.clone_ref(py)
    }

    /// The tree it is of.
    #[getter]
    fn timespanTree(&self, py: Python<'_>) -> Py<TimespanTree> {
        self.tree.clone_ref(py)
    }

    /// The same, which music21 keeps for any tree.
    #[getter]
    fn offsetTree(&self, py: Python<'_>) -> Py<TimespanTree> {
        self.tree.clone_ref(py)
    }

    /// The timespans starting here.
    #[getter]
    fn startTimespans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.listed(py, starts)
    }

    /// The timespans sounding across.
    #[getter]
    fn overlapTimespans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.listed(py, overlaps)
    }

    /// The timespans ending here.
    #[getter]
    fn stopTimespans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.listed(py, stops)
    }

    /// Those starting, then those sounding across.
    #[getter]
    fn startAndOverlapTimespans<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.listed(py, sounding)
    }

    /// The timespan sounding the lowest pitch, the last of those sounding
    /// it alike.
    #[getter]
    fn bassTimespan(&self, py: Python<'_>) -> Option<Py<ElementTimespan>> {
        let owner = self.tree.get();
        let tree = owner.crate_tree();
        let verticality = self.of(&tree);
        verticality
            .bass_timespan()
            .map(|span| owner.span(py, &tree, span))
    }

    /// Each pitch sounding, each name and octave once: the elements' own
    /// pitch objects.
    #[getter]
    fn pitchSet<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PySet>> {
        PySet::new(py, self.pitch_objects(py)?)
    }

    /// One pitch sounding of each pitch class.
    #[getter]
    fn pitchClassSet<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PySet>> {
        let mut classes: Vec<i64> = Vec::new();
        let mut pitches = Vec::new();
        for pitch in self.pitch_objects(py)? {
            let class: i64 = pitch.getattr("pitchClass")?.extract()?;
            if !classes.contains(&class) {
                classes.push(class);
                pitches.push(pitch);
            }
        }
        PySet::new(py, pitches)
    }

    /// A chord of the pitches sounding, lowest first.
    fn toChord<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pitches = PyList::new(py, self.sorted_pitches(py)?)?;
        py.get_type::<crate::chord::Chord>().call1((pitches,))
    }

    /// The measure of the first timespan starting here.
    #[getter]
    fn measureNumber(&self) -> PyResult<Option<i32>> {
        let tree = self.tree.get().crate_tree();
        let verticality = self.of(&tree);
        if verticality.start_timespans().is_empty() {
            return Err(PyIndexError::new_err("tuple index out of range"));
        }
        Ok(verticality.measure_number())
    }

    /// The next offset a timespan starts at.
    #[getter]
    fn nextStartOffset<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let tree = self.tree.get().crate_tree();
        position_of(py, self.of(&tree).next_start_offset())
    }

    /// The verticality there.
    #[getter]
    fn nextVerticality(&self, py: Python<'_>) -> PyResult<Option<Self>> {
        let owner = self.tree.bind(py);
        let tree = owner.get().crate_tree();
        let next = self.of(&tree).next();
        next.map(|next| verticality_of(py, owner, &next))
            .transpose()
    }

    /// The verticality at the last offset before this one a timespan
    /// starts at.
    #[getter]
    fn previousVerticality(&self, py: Python<'_>) -> PyResult<Option<Self>> {
        let owner = self.tree.bind(py);
        let tree = owner.get().crate_tree();
        let previous = self.of(&tree).previous();
        previous
            .map(|previous| verticality_of(py, owner, &previous))
            .transpose()
    }

    /// How long until the next start, or the end of the tree after the
    /// last.
    #[getter]
    fn timeToNextEvent<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let tree = self.tree.get().crate_tree();
        // A length, which music21 gives as `opFrac` gives it.
        let length = self.of(&tree).time_to_next_event();
        length.map(|length| op_frac(py, length)).transpose()
    }

    /// A chord of every pitch sounding here, lasting `quarterLength` or,
    /// without one, until the next start, or a rest where nothing sounds.
    #[pyo3(signature = (
        quarterLength = None,
        *,
        addTies = true,
        addPartIdAsGroup = false,
        removeRedundantPitches = true,
        gatherArticulations = None,
        gatherExpressions = None,
        copyPitches = true,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn makeElement<'py>(
        &self,
        py: Python<'py>,
        quarterLength: Option<f64>,
        addTies: bool,
        addPartIdAsGroup: bool,
        removeRedundantPitches: bool,
        gatherArticulations: Option<&Bound<'py, PyAny>>,
        gatherExpressions: Option<&Bound<'py, PyAny>>,
        copyPitches: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        refuse_unported(
            addPartIdAsGroup,
            gatherArticulations,
            gatherExpressions,
            copyPitches,
        )?;
        let tree = self.tree.get().crate_tree();
        let made = self
            .of(&tree)
            .make_element(quarterLength, addTies, removeRedundantPitches)
            .map_err(|error| stream_error(&error))?;
        element_object(py, &made)?
            .ok_or_else(|| PyNotImplementedError::new_err("makeElement made nothing to hand back"))
    }

    /// Each two pairs of `getPairedMotion`, the earlier pair first, as
    /// music21's `VoiceLeadingQuartet` where all four are notes or, without
    /// `returnObjects`, as the pairs of timespans.
    #[pyo3(signature = (
        *,
        includeRests = true,
        includeOblique = true,
        includeNoMotion = false,
        returnObjects = true,
        partPairNumbers = None,
    ))]
    fn getAllVoiceLeadingQuartets<'py>(
        &self,
        py: Python<'py>,
        includeRests: bool,
        includeOblique: bool,
        includeNoMotion: bool,
        returnObjects: bool,
        partPairNumbers: Option<Vec<(usize, usize)>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let owner = self.tree.get();
        let tree = owner.crate_tree();
        let quartets = self
            .of(&tree)
            .voice_leading_timespans(
                includeRests,
                includeOblique,
                includeNoMotion,
                partPairNumbers.as_deref(),
            )
            .map_err(|error| stream_error(&error))?;
        let out = PyList::empty(py);
        for quartet in quartets {
            let spans = quartet.map(|span| owner.span(py, &tree, span).into_bound(py));
            if !returnObjects {
                let upper = PyTuple::new(py, [&spans[0], &spans[1]])?;
                let lower = PyTuple::new(py, [&spans[2], &spans[3]])?;
                out.append(PyTuple::new(py, [upper, lower])?)?;
                continue;
            }
            if let Some(made) = quartet_of(py, &spans)? {
                out.append(made)?;
            }
        }
        Ok(out)
    }

    /// Each pitched timespan starting here paired with the one before it
    /// in its part, and, with `includeOblique`, each pitched one sounding
    /// across paired with itself.
    #[pyo3(signature = (*, includeRests = true, includeOblique = true))]
    fn getPairedMotion<'py>(
        &self,
        py: Python<'py>,
        includeRests: bool,
        includeOblique: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let owner = self.tree.get();
        let tree = owner.crate_tree();
        let pairs = self.of(&tree).paired_motion(includeRests, includeOblique);
        let out = PyList::empty(py);
        for (before, after) in pairs {
            let pair = [owner.span(py, &tree, before), owner.span(py, &tree, after)];
            out.append(PyTuple::new(py, pair)?)?;
        }
        Ok(out)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut names = Vec::new();
        for pitch in self.sorted_pitches(py)? {
            names.push(pitch.getattr("nameWithOctave")?.extract::<String>()?);
        }
        Ok(format!(
            "<music21.tree.verticality.Verticality {} {{{}}}>",
            self.given.bind(py).str()?,
            names.join(" ")
        ))
    }
}

/// `makeElement`'s keywords whose other settings are not ported.
fn refuse_unported(
    addPartIdAsGroup: bool,
    gatherArticulations: Option<&Bound<'_, PyAny>>,
    gatherExpressions: Option<&Bound<'_, PyAny>>,
    copyPitches: bool,
) -> PyResult<()> {
    let single = |given: Option<&Bound<'_, PyAny>>| {
        given.is_none_or(|said| said.extract::<String>().is_ok_and(|said| said == "single"))
    };
    if !addPartIdAsGroup && single(gatherArticulations) && single(gatherExpressions) && copyPitches
    {
        return Ok(());
    }
    Err(PyNotImplementedError::new_err(
        "makeElement's addPartIdAsGroup, gatherArticulations, gatherExpressions and \
         copyPitches are ported at their defaults only",
    ))
}

/// music21's `VoiceLeadingQuartet` of four timespans' notes, where all four
/// are notes.
fn quartet_of<'py>(
    py: Python<'py>,
    spans: &[Bound<'py, ElementTimespan>; 4],
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let note = PyString::new(py, "Note");
    let mut notes = Vec::new();
    for span in spans {
        let element = span.get().element.bind(py).clone();
        if !answers_to_class(&element, &note)? {
            return Ok(None);
        }
        notes.push(element);
    }
    let class = crate::installed_class(py, "music21.voiceLeading", "VoiceLeadingQuartet")
        .unwrap_or_else(|| {
            py.get_type::<crate::voiceleading::VoiceLeadingQuartet>()
                .into_any()
        });
    class
        .call1((&notes[0], &notes[1], &notes[2], &notes[3]))
        .map(Some)
}

/// A run of verticalities: music21's `tree.verticality.VerticalitySequence`.
#[pyclass(
    name = "VerticalitySequence",
    module = "music21.tree.verticality",
    frozen
)]
pub struct VerticalitySequence {
    verticalities: Py<PyTuple>,
}

#[pymethods]
impl VerticalitySequence {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.verticalities)
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.verticalities.bind(py).len()
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.verticalities.bind(py).as_any().get_item(item)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.verticalities.bind(py).as_any().try_iter()?.into_any())
    }

    /// Each part's timespans across the run, as a dictionary of the part
    /// and its `Horizontality`.
    fn unwrap<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        unwrap_all(self.verticalities.bind(py).as_any())
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut inner = Vec::new();
        for verticality in self.verticalities.bind(py).iter() {
            let shown = verticality.repr()?.to_string();
            let body = shown
                .trim_start_matches("<music21.tree.verticality.Verticality ")
                .trim_end_matches('>');
            inner.push(format!("({body})"));
        }
        Ok(format!(
            "<music21.tree.verticality.VerticalitySequence [\n\t{}\n\t]>",
            inner.join(",\n\t")
        ))
    }
}

/// Each part's timespans across a run of verticalities -- those sounding
/// across or starting at the first, then those starting at the rest -- part
/// by part in the order first met.
fn unwrap_all<'py>(verticalities: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py = verticalities.py();
    let out = PyDict::new(py);
    let given: Vec<Bound<'py, TreeVerticality>> = verticalities
        .try_iter()?
        .map(|each| Ok(each?.cast_into::<TreeVerticality>()?))
        .collect::<PyResult<_>>()?;
    let Some(first) = given.first() else {
        return Ok(out);
    };
    let owner = first.get().tree.get();
    let tree = owner.crate_tree();
    let crate_verticalities: Vec<RsVerticality<'_, '_>> =
        given.iter().map(|each| each.get().of(&tree)).collect();
    for (_, horizontality) in rs_tree::unwrap(&crate_verticalities) {
        let spans = owner.spans_of(py, &tree, horizontality.timespans())?;
        let part = match spans
            .get_item(0)?
            .cast::<ElementTimespan>()?
            .get()
            .part(py)?
        {
            Some(part) => part,
            None => py.None().into_bound(py),
        };
        out.set_item(part, horizontality_of(&horizontality, spans))?;
    }
    Ok(out)
}

/// The `Horizontality` of the crate's, over the timespan objects.
fn horizontality_of(
    horizontality: &RsHorizontality<'_, '_>,
    spans: Bound<'_, PyTuple>,
) -> Horizontality {
    Horizontality {
        timespans: spans.unbind(),
        passing: horizontality.has_passing_tone(),
        neighbor: horizontality.has_neighbor_tone(),
        no_motion: horizontality.has_no_motion(),
    }
}

/// One part's timespans one after another: music21's
/// `tree.analysis.Horizontality`.
#[pyclass(name = "Horizontality", module = "music21.tree.analysis", frozen)]
pub struct Horizontality {
    timespans: Py<PyTuple>,
    passing: bool,
    neighbor: bool,
    no_motion: bool,
}

#[pymethods]
impl Horizontality {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.timespans)
    }

    /// The timespans, in order.
    #[getter]
    fn timespans(&self, py: Python<'_>) -> Py<PyTuple> {
        self.timespans.clone_ref(py)
    }

    fn __len__(&self, py: Python<'_>) -> usize {
        self.timespans.bind(py).len()
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.timespans.bind(py).as_any().get_item(item)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.timespans.bind(py).as_any().try_iter()?.into_any())
    }

    /// Whether the first pitches of the first three rise all the way or
    /// fall all the way.
    #[getter]
    fn hasPassingTone(&self) -> bool {
        self.passing
    }

    /// Whether the first and third of the first three are the same pitch
    /// and the second is less than three semitones from them.
    #[getter]
    fn hasNeighborTone(&self) -> bool {
        self.neighbor
    }

    /// Whether every timespan sounds the same pitches.
    #[getter]
    fn hasNoMotion(&self) -> bool {
        self.no_motion
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let mut shown = Vec::new();
        for span in self.timespans.bind(py).iter() {
            let mut names = Vec::new();
            if let Ok(pitches) = span.getattr("pitches") {
                for pitch in pitches.try_iter()? {
                    names.push(pitch?.getattr("nameWithOctave")?.extract::<String>()?);
                }
            }
            shown.push(format!("({},)", names.join(", ")));
        }
        Ok(format!("<Horizontality: {}>", shown.join(" ")))
    }
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TimespanTree>()?;
    m.add_class::<ElementTimespan>()?;
    m.add_class::<PitchedTimespan>()?;
    m.add_class::<Horizontality>()?;
    m.add_class::<VerticalitySequence>()?;
    Ok(())
}
