//! music21's `musicxml.m21ToXml`: anything from a score to a pitch written
//! as MusicXML, over the crate's writer.
//!
//! The crate writes the document and works out the notation music21's
//! exporter makes first; these classes give music21's names to it.
//! `ScoreExporter.parse` hands back the document read into
//! `xml.etree.ElementTree`, as music21's does, and a `MeasureExporter`
//! method hands back what the crate's measure writer writes for the one
//! object it is given, standing alone at the start of a measure.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::ffi::CString;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyList, PyModule, PyTuple};

use music21::musicxml::ExportOptions;
use music21::stream::Stream as RsStream;

use crate::musicxml::{
    self as mx, MusicXMLExportException, MusicXMLWarning, dumpString, element_tree, export_error,
};

/// The module music21 keeps these in.
const MODULE: &str = "music21.musicxml.m21ToXml";

/// The longest note `fromGeneralNote` finds a meter of its own for, in
/// quarters; a longer one is cut into `4/4` measures.
const LONGEST_METERED: f64 = 6.0;

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

// ------------------------------------------------------------- the crate

/// Whether an object answers to a class of music21's, by its `classes`.
fn is_a(object: &Bound<'_, PyAny>, class: &str) -> PyResult<bool> {
    match object.getattr("classes") {
        Ok(classes) => classes.contains(class),
        Err(_) => Ok(false),
    }
}

/// A class of music21's, as this package answers for it.
fn class<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    crate::class_of(py, module, name)
}

/// A deep copy, as music21's exporter makes of what it is handed.
fn deep_copy<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    object
        .py()
        .import("copy")?
        .getattr("deepcopy")?
        .call1((object,))
}

/// What the writer is told: today's date, as music21 writes it, music21's
/// software line, and whether to make the notation first.
fn options(py: Python<'_>, notation: Notation) -> PyResult<ExportOptions> {
    let mut options = ExportOptions::default();
    let today = py
        .import("datetime")?
        .getattr("date")?
        .call_method0("today")?
        .call_method0("isoformat")?;
    options.encoding_date = Some(today.extract()?);
    options.software = crate::defaults::defaults_module(py)?
        .getattr("software")?
        .extract()?;
    options.make_notation = notation == Notation::Make;
    Ok(options)
}

/// Whether the writer works out the notation the score leaves unsaid.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Notation {
    Make,
    AsItStands,
}

impl Notation {
    fn of(make: bool) -> Self {
        if make { Self::Make } else { Self::AsItStands }
    }
}

/// The document the crate writes for a stream.
fn document(py: Python<'_>, stream: &RsStream, notation: Notation) -> PyResult<String> {
    music21::musicxml::to_musicxml(stream, &options(py, notation)?).map_err(|e| export_error(&e))
}

/// A document's or a fragment's text read into `ElementTree`, its
/// comments kept, as music21's divider comments are part of what it
/// builds.
fn parsed<'py>(py: Python<'py>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    let tree = element_tree(py)?;
    let keywords = PyDict::new(py);
    keywords.set_item("insert_comments", true)?;
    let builder = tree.getattr("TreeBuilder")?.call((), Some(&keywords))?;
    let keywords = PyDict::new(py);
    keywords.set_item("target", builder)?;
    let parser = tree.getattr("XMLParser")?.call((), Some(&keywords))?;
    let keywords = PyDict::new(py);
    keywords.set_item("parser", parser)?;
    tree.getattr("fromstring")?.call((text,), Some(&keywords))
}

/// A stream, the package's or music21's, with its notation made, handed
/// back as the package's streams.
fn notated<'py>(py: Python<'py>, stream: &RsStream) -> PyResult<Bound<'py, PyAny>> {
    let made = music21::musicxml::make_notation(stream).map_err(|e| export_error(&e))?;
    crate::formats::handed_back(py, Ok(made))
}

// --------------------------------------------------- wrapping an object

/// What music21's `fromGeneralObject` puts an object in before making its
/// notation: a stream is itself; a note, a pitch, a duration, a dynamic, a
/// scale or any other element goes in the stream music21 gives it.
fn wrapped<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = object.py();
    if is_a(object, "Stream")? {
        return Ok(object.clone());
    }
    if is_a(object, "GeneralNote")? {
        return note_part(&deep_copy(object)?);
    }
    if is_a(object, "Pitch")? {
        let note = class(py, "music21.note", "Note")?.call0()?;
        note.setattr("pitch", deep_copy(object)?)?;
        return lone_measure(&[note]);
    }
    if is_a(object, "Duration")?
        || object.is_instance(&class(py, "music21.duration", "Duration")?)?
    {
        let note = class(py, "music21.note", "Note")?.call0()?;
        note.setattr("duration", deep_copy(object)?)?;
        return note_part(&note);
    }
    if is_a(object, "Dynamic")? {
        let stream = class(py, "music21.stream", "Stream")?.call0()?;
        stream.call_method1("append", (deep_copy(object)?,))?;
        return Ok(stream);
    }
    if is_a(object, "DiatonicScale")? {
        return scale_measure(object, Highlight::TonicAndDominant);
    }
    if is_a(object, "Scale")? {
        return scale_measure(object, Highlight::Tonic);
    }
    if is_a(object, "Music21Object")? {
        return lone_measure(&[deep_copy(object)?]);
    }
    Err(MusicXMLExportException::new_err(format!(
        "Cannot translate the object {} to a complete musicXML document; put it in a Stream first!",
        object.str()?
    )))
}

/// A measure numbered 1 holding these, in order.
fn lone_measure<'py>(elements: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyAny>> {
    let py = elements[0].py();
    let keywords = PyDict::new(py);
    keywords.set_item("number", 1)?;
    let measure = class(py, "music21.stream", "Measure")?.call((), Some(&keywords))?;
    for element in elements {
        measure.call_method1("append", (element,))?;
    }
    Ok(measure)
}

/// music21's `fromGeneralNote`: a note in a part of its own, with the
/// meter it fits best where it is no longer than six quarters.
fn note_part<'py>(note: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = note.py();
    let part = class(py, "music21.stream", "Part")?.call1((note,))?;
    let length: f64 = note.getattr("quarterLength")?.extract()?;
    if length > 0.0 && length <= LONGEST_METERED {
        let meter = py
            .import("music21.meter")?
            .getattr("bestTimeSignature")?
            .call1((&part,))?;
        part.call_method1("insert", (0, meter))?;
    }
    Ok(part)
}

/// Which of a scale's degrees `fromScale` and `fromDiatonicScale` hold
/// longer than the rest.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Highlight {
    Tonic,
    TonicAndDominant,
}

/// music21's `fromScale` and `fromDiatonicScale`: a measure of the
/// scale's degrees, the first carrying the scale's name, the tonic held
/// four quarters (and a diatonic scale's dominant two), in the meter they
/// fit best.
fn scale_measure<'py>(
    scale: &Bound<'py, PyAny>,
    highlight: Highlight,
) -> PyResult<Bound<'py, PyAny>> {
    let py = scale.py();
    let degrees: usize = scale
        .getattr("abstract")?
        .call_method0("getDegreeMaxUnique")?
        .extract()?;
    let tonic: String = scale.call_method0("getTonic")?.getattr("name")?.extract()?;
    let dominant: Option<String> = match highlight {
        Highlight::TonicAndDominant => Some(
            scale
                .call_method0("getDominant")?
                .getattr("name")?
                .extract()?,
        ),
        Highlight::Tonic => None,
    };

    // A note a degree, held as long as the degree's part in the scale.
    let mut notes = Vec::with_capacity(degrees);
    for degree in 1..=degrees {
        let pitch = scale.call_method1("pitchFromDegree", (degree,))?;
        let name: String = pitch.getattr("name")?.extract()?;
        let note = class(py, "music21.note", "Note")?.call0()?;
        note.setattr("pitch", pitch)?;
        if degree == 1 {
            note.call_method1("addLyric", (scale.getattr("name")?,))?;
        }
        let length = if name == tonic {
            4
        } else if dominant.as_deref() == Some(name.as_str()) {
            2
        } else {
            1
        };
        note.setattr("quarterLength", length)?;
        notes.push(note);
    }
    let measure = lone_measure(&notes)?;
    let meter = measure.call_method0("bestTimeSignature")?;
    measure.setattr("timeSignature", meter)?;
    Ok(measure)
}

// ------------------------------------------------- GeneralObjectExporter

/// music21's `GeneralObjectExporter`: anything from a score to a pitch as
/// the bytes of a whole MusicXML document.
#[pyclass(
    name = "GeneralObjectExporter",
    module = "music21.musicxml.m21ToXml",
    subclass,
    dict
)]
struct GeneralObjectExporter {
    #[pyo3(get, set)]
    generalObj: Py<PyAny>,
    /// Whether the notation is made before writing, as music21's
    /// `makeNotation` attribute says; off, only a score is written.
    #[pyo3(get, set)]
    makeNotation: bool,
}

#[pymethods]
impl GeneralObjectExporter {
    #[new]
    #[pyo3(signature = (obj = None))]
    fn new(py: Python<'_>, obj: Option<Py<PyAny>>) -> Self {
        Self {
            generalObj: obj.unwrap_or_else(|| py.None()),
            makeNotation: true,
        }
    }

    /// The document's bytes, for the object given or the one this was
    /// made with: put in a well-formed score and its notation made first,
    /// unless `makeNotation` is false, when it has to be a score already.
    #[pyo3(signature = (obj = None))]
    fn parse<'py>(
        &self,
        py: Python<'py>,
        obj: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let object = match obj.filter(|given| !given.is_none()) {
            Some(given) => given,
            None => self.generalObj.bind(py).clone(),
        };
        if object.is_none() {
            return Err(MusicXMLExportException::new_err(
                "Must have an object to export",
            ));
        }
        if !self.makeNotation && !is_a(&object, "Score")? {
            return Err(MusicXMLExportException::new_err(
                "Can only export Scores with makeNotation=False",
            ));
        }
        let stream = crate::stream::crate_stream(&wrapped(&object)?)?;
        let text = document(py, &stream, Notation::of(self.makeNotation))?;
        Ok(PyBytes::new(py, text.as_bytes()))
    }

    /// The document's bytes for a score `fromGeneralObject` made.
    fn parseWellformedObject<'py>(
        &self,
        py: Python<'py>,
        sc: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let stream = crate::stream::crate_stream(sc)?;
        let text = document(py, &stream, Notation::of(self.makeNotation))?;
        Ok(PyBytes::new(py, text.as_bytes()))
    }

    /// The object put in a score with its notation made, as it is written.
    fn fromGeneralObject<'py>(&self, obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let stream = crate::stream::crate_stream(&wrapped(obj)?)?;
        notated(obj.py(), &stream)
    }

    /// A score with its notation made.
    fn fromScore<'py>(&self, sc: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(sc)
    }

    /// A part put in a score of its own, its notation made.
    fn fromPart<'py>(&self, p: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(p)
    }

    /// A measure put in a part and a score, its notation made.
    fn fromMeasure<'py>(&self, m: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(m)
    }

    /// A voice put in a measure, a part and a score.
    fn fromVoice<'py>(&self, v: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(v)
    }

    /// A stream as the score it stands for: a part where it is one, a
    /// score where it holds parts.
    fn fromStream<'py>(&self, st: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(st)
    }

    /// A note as a score: its meter where it fits one, else cut into `4/4`.
    fn fromGeneralNote<'py>(&self, n: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(n)
    }

    /// A pitch as a score: a quarter note of it in a measure.
    fn fromPitch<'py>(&self, p: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(p)
    }

    /// A duration as a score: a C of that length.
    fn fromDuration<'py>(&self, d: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(d)
    }

    /// A dynamic as a score.
    fn fromDynamic<'py>(&self, dynamicObject: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.fromGeneralObject(dynamicObject)
    }

    /// A scale as a measure of its degrees, the tonic held longer.
    fn fromScale<'py>(&self, scaleObject: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let measure = scale_measure(scaleObject, Highlight::Tonic)?;
        notated(scaleObject.py(), &crate::stream::crate_stream(&measure)?)
    }

    /// A diatonic scale as a measure of its degrees, the tonic and the
    /// dominant held longer.
    fn fromDiatonicScale<'py>(
        &self,
        diatonicScaleObject: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let measure = scale_measure(diatonicScaleObject, Highlight::TonicAndDominant)?;
        notated(
            diatonicScaleObject.py(),
            &crate::stream::crate_stream(&measure)?,
        )
    }

    /// Any other element as a score: alone in a measure.
    fn fromMusic21Object<'py>(&self, obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let measure = lone_measure(&[deep_copy(obj)?])?;
        notated(obj.py(), &crate::stream::crate_stream(&measure)?)
    }
}

// ------------------------------------------------------- XMLExporterBase

/// music21's `XMLExporterBase`: what the score, part and measure
/// exporters share.
#[pyclass(
    name = "XMLExporterBase",
    module = "music21.musicxml.m21ToXml",
    subclass,
    dict
)]
struct XMLExporterBase {
    /// The element being built, as music21's `xmlRoot`.
    #[pyo3(get, set)]
    xmlRoot: Py<PyAny>,
    /// What is being written.
    #[pyo3(get, set)]
    stream: Py<PyAny>,
}

impl XMLExporterBase {
    /// The base of an exporter whose root has this tag, writing `stream`
    /// or an empty stream of music21's class `empty`.
    fn rooted(py: Python<'_>, tag: &str, stream: Option<Py<PyAny>>, empty: &str) -> PyResult<Self> {
        let root = element_tree(py)?.getattr("Element")?.call1((tag,))?;
        let stream = match stream.filter(|given| !given.is_none(py)) {
            Some(given) => given,
            None => class(py, "music21.stream", empty)?.call0()?.unbind(),
        };
        Ok(Self {
            xmlRoot: root.unbind(),
            stream,
        })
    }
}

#[pymethods]
impl XMLExporterBase {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        Self::rooted(py, "override-me-in-subclasses", None, "Stream")
    }

    /// The root as the bytes of a document: the XML declaration and
    /// doctype, then the root indented. `noCopy` indents it in place.
    #[pyo3(signature = (noCopy = true))]
    fn asBytes<'py>(&self, py: Python<'py>, noCopy: bool) -> PyResult<Bound<'py, PyBytes>> {
        let mut text = header();
        text.push_str(&dumpString(self.xmlRoot.bind(py), noCopy)?);
        Ok(PyBytes::new(py, text.as_bytes()))
    }

    /// Appends a comment to the root: the text centred in a line of signs.
    #[pyo3(signature = (comment = ""))]
    fn addDividerComment(&self, py: Python<'_>, comment: &str) -> PyResult<()> {
        let divider = element_tree(py)?
            .getattr("Comment")?
            .call1((music21::musicxml::divider_text(comment),))?;
        self.xmlRoot.bind(py).call_method1("append", (divider,))?;
        Ok(())
    }

    /// Prints an element as music21's `helpers.dump` does.
    #[staticmethod]
    fn dump(obj: &Bound<'_, PyAny>) -> PyResult<()> {
        mx::dump(obj)
    }

    /// The XML declaration and doctype every document starts with.
    fn xmlHeader<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, header().as_bytes())
    }

    /// An accidental as the `<accidental>` a note writes for it, or the
    /// element named `elName` that holds the same.
    #[pyo3(signature = (a, *, elName = "accidental"))]
    fn accidentalToMx<'py>(
        &self,
        a: &Bound<'py, PyAny>,
        elName: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = a.py();
        let pitch = class(py, "music21.pitch", "Pitch")?.call1(("C4",))?;
        pitch.setattr("accidental", deep_copy(a)?)?;
        let note = class(py, "music21.note", "Note")?.call0()?;
        note.setattr("pitch", pitch)?;
        let written = first_fragment(&note)?;
        let found = written.call_method1("find", ("accidental",))?;
        if found.is_none() {
            return Err(MusicXMLExportException::new_err(format!(
                "{} writes no accidental",
                a.repr()?
            )));
        }
        found.setattr("tag", elName)?;
        Ok(found)
    }
}

/// music21's `xmlHeader`, as text.
fn header() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE score-partwise  PUBLIC \
         \"-//Recordare//DTD MusicXML {} Partwise//EN\" \
         \"http://www.musicxml.org/dtds/partwise.dtd\">\n",
        music21::musicxml::MUSICXML_VERSION
    )
}

// --------------------------------------------------------- ScoreExporter

/// music21's `ScoreExporter`: a score as a `<score-partwise>` element.
#[pyclass(name = "ScoreExporter", module = "music21.musicxml.m21ToXml", extends = XMLExporterBase, subclass)]
struct ScoreExporter {
    /// Whether each part's notation is made before it is written.
    #[pyo3(get, set)]
    makeNotation: bool,
}

#[pymethods]
impl ScoreExporter {
    #[new]
    #[pyo3(signature = (score = None, makeNotation = true))]
    fn new(
        py: Python<'_>,
        score: Option<Py<PyAny>>,
        makeNotation: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = XMLExporterBase::rooted(py, "score-partwise", score, "Score")?;
        base.xmlRoot
            .bind(py)
            .call_method1("set", ("version", music21::musicxml::MUSICXML_VERSION))?;
        Ok(PyClassInitializer::from(base).add_subclass(Self { makeNotation }))
    }

    /// The score as a `<score-partwise>` element, which becomes the root;
    /// an empty score is written as music21's blank page.
    fn parse<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let notation = Notation::of(slf.borrow().makeNotation);
        let stream = slf.as_super().borrow().stream.clone_ref(py);
        let text = document(py, &crate::stream::crate_stream(stream.bind(py))?, notation)?;
        let root = parsed(py, &text)?;
        slf.as_super().borrow_mut().xmlRoot = root.clone().unbind();
        Ok(root)
    }

    /// music21's blank page, "This Page Intentionally Left Blank": a part
    /// of one measure of rest, written as the root.
    fn emptyObject<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let empty = class(py, "music21.stream", "Score")?.call0()?;
        slf.as_super().borrow_mut().stream = empty.unbind();
        Self::parse(slf)
    }
}

// ---------------------------------------------------------- PartExporter

/// music21's `PartExporter`: a part as a `<part>` element.
#[pyclass(name = "PartExporter", module = "music21.musicxml.m21ToXml", extends = XMLExporterBase, subclass)]
struct PartExporter {
    /// The `ScoreExporter` this belongs to, or `None`.
    #[pyo3(get, set)]
    parent: Py<PyAny>,
}

#[pymethods]
impl PartExporter {
    #[new]
    #[pyo3(signature = (partObj = None, parent = None))]
    fn new(
        py: Python<'_>,
        partObj: Option<Py<PyAny>>,
        parent: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = XMLExporterBase::rooted(py, "part", partObj, "Part")?;
        let parent = parent.unwrap_or_else(|| py.None());
        Ok(PyClassInitializer::from(base).add_subclass(Self { parent }))
    }

    /// The part as the `<part>` its score writes, which becomes the root.
    fn parse<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py);
        let part = crate::stream::crate_stream(stream.bind(py))?;
        let found = written_in_score(py, part, "part")?;
        slf.as_super().borrow_mut().xmlRoot = found.clone().unbind();
        Ok(found)
    }
}

/// The first element of this tag in the document a part makes when it
/// is written as the only part of a score.
fn written_in_score<'py>(
    py: Python<'py>,
    part: RsStream,
    tag: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let mut score = RsStream::with_kind(music21::stream::StreamKind::Score);
    score.push(part);
    let root = parsed(py, &document(py, &score, Notation::AsItStands)?)?;
    let found = root.call_method1("find", (format!(".//{tag}"),))?;
    if found.is_none() {
        return Err(MusicXMLExportException::new_err(format!(
            "the score written has no <{tag}>"
        )));
    }
    Ok(found)
}

// ------------------------------------------------------- MeasureExporter

/// music21's `MeasureExporter`: a measure as a `<measure>` element, and
/// each kind of object a measure holds as the element it writes.
#[pyclass(name = "MeasureExporter", module = "music21.musicxml.m21ToXml", extends = XMLExporterBase, subclass)]
struct MeasureExporter {
    /// The `PartExporter` this belongs to, or `None`.
    #[pyo3(get, set)]
    parent: Py<PyAny>,
    /// How far into the measure `moveForward` and `moveBackward` have
    /// moved, in quarters.
    #[pyo3(get, set)]
    offsetInMeasure: f64,
    /// The divisions of a quarter durations are counted in.
    #[pyo3(get, set)]
    currentDivisions: f64,
}

/// Appends what was written to the exporter's root, as music21's methods
/// that write a measure's contents do, and hands back the first of it.
fn appended<'py>(
    slf: &Bound<'py, MeasureExporter>,
    written: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let root = slf.as_super().borrow().xmlRoot.clone_ref(slf.py());
    for element in &written {
        root.bind(slf.py()).call_method1("append", (element,))?;
    }
    written
        .into_iter()
        .next()
        .ok_or_else(|| MusicXMLExportException::new_err("a measure writes nothing for this"))
}

/// A `<forward>` or `<backup>` of a length, appended to the root.
fn moved(slf: &Bound<'_, MeasureExporter>, tag: &str, byOffset: f64) -> PyResult<i64> {
    let py = slf.py();
    let amount = (byOffset * slf.borrow().currentDivisions).round() as i64;
    if amount == 0 {
        return Ok(0);
    }
    let tree = element_tree(py)?;
    let element = tree.getattr("Element")?.call1((tag,))?;
    let duration = tree.getattr("SubElement")?.call1((&element, "duration"))?;
    duration.setattr("text", amount.to_string())?;
    slf.as_super()
        .borrow()
        .xmlRoot
        .bind(py)
        .call_method1("append", (element,))?;
    Ok(amount)
}

/// The elements the measure writer writes for one object, each read into
/// `ElementTree`.
fn fragments<'py>(object: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = object.py();
    let Some(element) = crate::stream::element_written(object)? else {
        return Err(MusicXMLExportException::new_err(format!(
            "{} is nothing a measure writes",
            object.repr()?
        )));
    };
    // The measure it stands in, whose meter and clef decide how a rest
    // is written, as music21 asks the rest's context.
    let site = object
        .getattr("activeSite")
        .ok()
        .filter(|site| !site.is_none());
    let measure = match site {
        Some(site) if is_a(&site, "Measure")? => Some(crate::stream::crate_stream(&site)?),
        _ => None,
    };
    let offset: f64 = match &measure {
        Some(_) => object.getattr("offset")?.extract()?,
        None => 0.0,
    };
    music21::musicxml::element_xml(&element, measure.as_ref(), offset)
        .map_err(|e| export_error(&e))?
        .iter()
        .map(|text| parsed(py, text))
        .collect()
}

/// The first element the measure writer writes for one object.
fn first_fragment<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    fragments(object)?.into_iter().next().ok_or_else(|| {
        MusicXMLExportException::new_err(format!(
            "a measure writes nothing for {}",
            object
                .repr()
                .map(|shown| shown.to_string())
                .unwrap_or_default()
        ))
    })
}

/// A quarter-note C carrying something a note writes, such as a lyric or
/// a tie, set as the attribute of the note's named.
fn carrier<'py>(
    py: Python<'py>,
    attribute: &str,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let note = class(py, "music21.note", "Note")?.call0()?;
    note.setattr(attribute, value)?;
    Ok(note)
}

/// The child of a written note with this path, or an error naming it.
fn found_in<'py>(written: &Bound<'py, PyAny>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    let found = written.call_method1("find", (path,))?;
    if found.is_none() {
        return Err(MusicXMLExportException::new_err(format!(
            "nothing is written at {path}"
        )));
    }
    Ok(found)
}

/// Every child of a written note with this path, as a list.
fn all_in<'py>(written: &Bound<'py, PyAny>, path: &str) -> PyResult<Bound<'py, PyList>> {
    Ok(written
        .call_method1("findall", (path,))?
        .cast_into::<PyList>()?)
}

#[pymethods]
impl MeasureExporter {
    #[new]
    #[pyo3(signature = (measureObj = None, parent = None))]
    fn new(
        py: Python<'_>,
        measureObj: Option<Py<PyAny>>,
        parent: Option<Py<PyAny>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = XMLExporterBase::rooted(py, "measure", measureObj, "Measure")?;
        let parent = parent.unwrap_or_else(|| py.None());
        Ok(PyClassInitializer::from(base).add_subclass(Self {
            parent,
            offsetInMeasure: 0.0,
            currentDivisions: music21::musicxml::DIVISIONS_PER_QUARTER,
        }))
    }

    /// Moves forward by a length in quarters, appending a `<forward>`.
    fn moveForward(slf: &Bound<'_, Self>, byOffset: f64) -> PyResult<()> {
        if moved(slf, "forward", byOffset)? != 0 {
            slf.borrow_mut().offsetInMeasure += byOffset;
        }
        Ok(())
    }

    /// Moves back by a length in quarters, appending a `<backup>`.
    fn moveBackward(slf: &Bound<'_, Self>, byOffset: f64) -> PyResult<()> {
        if moved(slf, "backup", byOffset)? != 0 {
            slf.borrow_mut().offsetInMeasure -= byOffset;
        }
        Ok(())
    }

    /// The measure as the `<measure>` it writes as the only measure of a
    /// part, which becomes the root.
    fn parse<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py);
        let measure = crate::stream::crate_stream(stream.bind(py))?;
        let mut part = RsStream::with_kind(music21::stream::StreamKind::Part);
        part.push(measure);
        let found = written_in_score(py, part, "measure")?;
        slf.as_super().borrow_mut().xmlRoot = found.clone().unbind();
        Ok(found)
    }

    /// A note as its `<note>`, or the `noteIndexInChord`th note of
    /// `chordParent` where one is given.
    #[pyo3(signature = (n, noteIndexInChord = 0, chordParent = None))]
    fn noteToXml<'py>(
        slf: &Bound<'py, Self>,
        n: &Bound<'py, PyAny>,
        noteIndexInChord: usize,
        chordParent: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let written = chosen(n, noteIndexInChord, chordParent)?;
        appended(slf, vec![written])
    }

    /// An unpitched stroke as its `<note>`.
    #[pyo3(signature = (up, noteIndexInChord = 0, chordParent = None))]
    fn unpitchedToXml<'py>(
        &self,
        up: &Bound<'py, PyAny>,
        noteIndexInChord: usize,
        chordParent: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        chosen(up, noteIndexInChord, chordParent)
    }

    /// A rest as its `<note>`.
    fn restToXml<'py>(
        slf: &Bound<'py, Self>,
        r: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(r)?)
    }

    /// A chord as a `<note>` for each of its notes, lowest first.
    fn chordToXml<'py>(
        slf: &Bound<'py, Self>,
        c: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let written = fragments(c)?;
        let listed = PyList::new(c.py(), &written)?;
        appended(slf, written)?;
        Ok(listed)
    }

    /// A pitch as its `<pitch>`.
    fn pitchToXml<'py>(&self, p: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let note = carrier(p.py(), "pitch", &deep_copy(p)?)?;
        found_in(&first_fragment(&note)?, "pitch")
    }

    /// A duration as its `<duration>`, counted in divisions.
    fn durationXml<'py>(&self, dur: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let note = carrier(dur.py(), "duration", &deep_copy(dur)?)?;
        found_in(&first_fragment(&note)?, "duration")
    }

    /// A tuplet as the `<time-modification>` of a note in it.
    fn tupletToTimeModification<'py>(
        &self,
        tup: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = tup.py();
        let duration = class(py, "music21.duration", "Duration")?.call1(("eighth",))?;
        duration.call_method1("appendTuplet", (deep_copy(tup)?,))?;
        let note = carrier(py, "duration", &duration)?;
        found_in(&first_fragment(&note)?, "time-modification")
    }

    /// A lyric as its `<lyric>`.
    fn lyricToXml<'py>(&self, ly: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = ly.py();
        let note = carrier(py, "lyrics", &PyList::new(py, [deep_copy(ly)?])?.into_any())?;
        found_in(&first_fragment(&note)?, "lyric")
    }

    /// A note's beams as a list of `<beam>`.
    fn beamsToXml<'py>(&self, beams: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
        let py = beams.py();
        let duration = class(py, "music21.duration", "Duration")?.call1(("64th",))?;
        let note = carrier(py, "duration", &duration)?;
        note.setattr("beams", deep_copy(beams)?)?;
        all_in(&first_fragment(&note)?, "beam")
    }

    /// A tie as the list of `<tie>` it writes.
    fn tieToXmlTie<'py>(&self, m21Tie: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
        let note = carrier(m21Tie.py(), "tie", &deep_copy(m21Tie)?)?;
        all_in(&first_fragment(&note)?, "tie")
    }

    /// A tie as the list of `<tied>` its notations write.
    fn tieToXmlTied<'py>(&self, m21Tie: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
        let note = carrier(m21Tie.py(), "tie", &deep_copy(m21Tie)?)?;
        all_in(&first_fragment(&note)?, "notations/tied")
    }

    /// An articulation as the element in a note's `<articulations>`.
    fn articulationToXmlArticulation<'py>(
        &self,
        articulationMark: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        marked_note(
            articulationMark,
            "articulations",
            "notations/articulations/*",
        )
    }

    /// A technical mark as the element in a note's `<technical>`.
    fn articulationToXmlTechnical<'py>(
        &self,
        articulationMark: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        marked_note(articulationMark, "articulations", "notations/technical/*")
    }

    /// An expression as the element in a note's notations: an ornament
    /// as the mark in its `<ornaments>`, a fermata as itself.
    fn expressionToXml<'py>(&self, expression: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = expression.py();
        let note = carrier(
            py,
            "expressions",
            &PyList::new(py, [deep_copy(expression)?])?.into_any(),
        )?;
        let notations = found_in(&first_fragment(&note)?, "notations")?;
        let first = found_in(&notations, "*")?;
        let tag: String = first.getattr("tag")?.extract()?;
        if tag == "ornaments" {
            return found_in(&first, "*");
        }
        Ok(first)
    }

    /// A note's notehead as its `<notehead>`.
    fn noteheadToXml<'py>(&self, n: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        found_in(&first_fragment(n)?, "notehead")
    }

    /// A dynamic as its `<direction>`.
    fn dynamicToXml<'py>(
        slf: &Bound<'py, Self>,
        d: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(d)?)
    }

    /// A segno as its `<direction>`.
    fn segnoToXml<'py>(
        slf: &Bound<'py, Self>,
        segno: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(segno)?)
    }

    /// A coda as its `<direction>`.
    fn codaToXml<'py>(
        slf: &Bound<'py, Self>,
        coda: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(coda)?)
    }

    /// A tempo mark as its `<direction>`.
    fn tempoIndicationToXml<'py>(
        slf: &Bound<'py, Self>,
        ti: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(ti)?)
    }

    /// A rehearsal mark as its `<direction>`.
    fn rehearsalMarkToXml<'py>(
        slf: &Bound<'py, Self>,
        rm: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(rm)?)
    }

    /// A text expression as its `<direction>`.
    fn textExpressionToXml<'py>(
        slf: &Bound<'py, Self>,
        teOrRe: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(teOrRe)?)
    }

    /// A chord symbol as its `<harmony>`.
    fn chordSymbolToXml<'py>(
        slf: &Bound<'py, Self>,
        cs: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        appended(slf, fragments(cs)?)
    }

    /// A clef as its `<clef>`.
    fn clefToXml<'py>(&self, clefObj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        first_fragment(clefObj)
    }

    /// A key or key signature as its `<key>`.
    fn keySignatureToXml<'py>(
        &self,
        keyOrKeySignature: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        first_fragment(keyOrKeySignature)
    }

    /// A meter as its `<time>`.
    fn timeSignatureToXml<'py>(&self, ts: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        first_fragment(ts)
    }

    /// A barline as its `<barline>`, with no location.
    fn barlineToXml<'py>(&self, barObject: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        first_fragment(barObject)
    }
}

/// The `<note>` for a note, or for the `index`th note of a chord.
fn chosen<'py>(
    n: &Bound<'py, PyAny>,
    index: usize,
    chord: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    match chord.filter(|chord| !chord.is_none()) {
        Some(chord) => fragments(chord)?
            .into_iter()
            .nth(index)
            .ok_or_else(|| MusicXMLExportException::new_err("no such note in the chord")),
        None => first_fragment(n),
    }
}

/// The element a mark writes in a note's notations, the mark put in the
/// list of the note's named.
fn marked_note<'py>(
    mark: &Bound<'py, PyAny>,
    list: &str,
    path: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = mark.py();
    let note = carrier(py, list, &PyList::new(py, [deep_copy(mark)?])?.into_any())?;
    found_in(&first_fragment(&note)?, path)
}

// --------------------------------------------------------------- functions

/// music21's `typeToMusicXMLType`: `longa` as `long`, the types MusicXML
/// has no value for refused.
#[pyfunction]
fn typeToMusicXMLType(value: &str) -> PyResult<String> {
    music21::musicxml::type_to_musicxml(value)
        .map(str::to_string)
        .map_err(|e| export_error(&e))
}

/// music21's `normalizeColor`: a CSS colour name as its hex value, and a
/// hex value upper-cased.
#[pyfunction]
fn normalizeColor(color: &str) -> PyResult<String> {
    music21::musicxml::normalize_color(color).map_err(|_| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "{color:?} is not defined as a named color in css3"
        ))
    })
}

/// music21's `getMetadataFromContext`: the stream's metadata, or that of
/// the nearest stream it is in, or `None`.
#[pyfunction]
fn getMetadataFromContext<'py>(s: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let own = s.getattr("metadata")?;
    if !own.is_none() {
        return Ok(own);
    }
    for context in s.call_method0("contextSites")?.try_iter()? {
        let found = context?.getattr("site")?.getattr("metadata")?;
        if !found.is_none() {
            return Ok(found);
        }
    }
    Ok(s.py().None().into_bound(s.py()))
}

/// music21's `_setTagTextFromAttribute`: a child of `xmlEl` tagged `tag`
/// holding the object's attribute as text, through `transform` if given.
/// No child is made for a missing attribute, nor for an empty one unless
/// `forceEmpty`.
#[pyfunction]
#[pyo3(signature = (m21El, xmlEl, tag, attributeName = None, *, transform = None, forceEmpty = false))]
fn _setTagTextFromAttribute<'py>(
    m21El: &Bound<'py, PyAny>,
    xmlEl: &Bound<'py, PyAny>,
    tag: &str,
    attributeName: Option<String>,
    transform: Option<&Bound<'py, PyAny>>,
    forceEmpty: bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = m21El.py();
    let name = match attributeName {
        Some(name) => name,
        None => music21::common::stringtools::hyphen_to_camel_case(tag, "-")
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?,
    };
    let Ok(mut value) = m21El.getattr(name.as_str()) else {
        return Ok(None);
    };
    if let Some(transform) = transform.filter(|given| !given.is_none()) {
        value = transform.call1((value,))?;
    }
    let empty = value.is_none() || value.eq("")?;
    if empty && !forceEmpty {
        return Ok(None);
    }
    let child = element_tree(py)?
        .getattr("SubElement")?
        .call1((xmlEl, tag))?;
    if !value.is_none() {
        child.setattr("text", value.str()?)?;
    }
    Ok(Some(child))
}

/// music21's `moveOctaveIntoMusicxmlRange`: an octave moved into the
/// range MusicXML can say, with a `MusicXMLWarning` naming the pitch where
/// it had to move.
#[pyfunction]
fn moveOctaveIntoMusicxmlRange(py: Python<'_>, octave: i32, name: &str) -> PyResult<i32> {
    let moved = music21::musicxml::musicxml_octave(octave);
    if moved != octave {
        let message = format!("{name} is out of MusicXML range; writing octave {moved}");
        let text = CString::new(message.replace('\0', "")).unwrap_or_default();
        PyErr::warn(py, &py.get_type::<MusicXMLWarning>(), &text, 2)?;
    }
    Ok(moved)
}

/// `music21.musicxml.m21ToXml`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;

        // What music21's module imports from `ElementTree` and its own
        // package, which music21's examples reach for through it.
        let tree = element_tree(py)?;
        for name in ["Element", "SubElement", "Comment", "ElementTree"] {
            m.add(name, tree.getattr(name)?)?;
        }
        let objects = mx::objects_module(py)?;
        for name in ["MusicXMLExportException", "MusicXMLWarning"] {
            m.add(name, objects.getattr(name)?)?;
        }
        let helpers = mx::helpers_module(py)?;
        m.add("helpers", &helpers)?;
        m.add("xmlObjects", &objects)?;
        m.add("synchronizeIds", helpers.getattr("synchronizeIdsToXML")?)?;
        m.add(
            "setAttributeFromAttribute",
            helpers.getattr("setXMLAttributeFromAttribute")?,
        )?;
        m.add("PartStaffExporterMixin", mx::part_staff_mixin(py)?)?;

        // The exporters; a score's is also a `PartStaffExporterMixin`.
        m.add_class::<GeneralObjectExporter>()?;
        m.add_class::<XMLExporterBase>()?;
        m.add_class::<ScoreExporter>()?;
        m.add_class::<PartExporter>()?;
        m.add_class::<MeasureExporter>()?;
        let score = m.getattr("ScoreExporter")?;
        score.setattr(
            "__bases__",
            PyTuple::new(
                py,
                [m.getattr("XMLExporterBase")?, mx::part_staff_mixin(py)?],
            )?,
        )?;

        macro_rules! add {
            ($($name:ident),* $(,)?) => {
                $(m.add_function(wrap_pyfunction!($name, &m)?)?;)*
            };
        }
        add!(
            typeToMusicXMLType,
            normalizeColor,
            getMetadataFromContext,
            _setTagTextFromAttribute,
            moveOctaveIntoMusicxmlRange,
        );
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
