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
    self as mx, Et, MusicXMLExportException, MusicXMLWarning, dumpString, element_tree,
    export_error, step_refused,
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
    let defaults = crate::defaults::defaults_module(py)?;
    options.software = defaults.getattr("software")?.extract()?;
    options.default_title = defaults.getattr("title")?.extract()?;
    options.default_author = defaults.getattr("author")?.extract()?;
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

    /// Sets each attribute named in `musicXMLNames` from the style
    /// attribute of the same place in `m21Names` (camel-cased from the
    /// MusicXML name where not given), as text, where it is set.
    /// `m21Object` may be a style itself.
    #[pyo3(signature = (mxObject, m21Object, musicXMLNames, m21Names = None))]
    fn setStyleAttributes(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
        musicXMLNames: &Bound<'_, PyAny>,
        m21Names: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let xml = names_of(musicXMLNames)?;
        let m21 = match m21Names.filter(|given| !given.is_none()) {
            Some(given) => names_of(given)?,
            None => xml
                .iter()
                .map(|name| music21::common::stringtools::hyphen_to_camel_case(name, "-"))
                .collect::<music21::Result<Vec<_>>>()
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?,
        };
        let pairs: Vec<(&str, &str)> = xml
            .iter()
            .map(String::as_str)
            .zip(m21.iter().map(String::as_str))
            .collect();
        set_style_attributes(mxObject, m21Object, &pairs)
    }

    /// The `justify`, `text-decoration` and other text-formatting
    /// attributes, then `setPrintStyleAlign`'s.
    fn setTextFormatting(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        set_style_attributes(mxObject, m21Object, &TEXT_FORMATTING)?;
        print_style_align(mxObject, m21Object)
    }

    /// `setPrintStyle`'s attributes, then `valign` and `halign`.
    fn setPrintStyleAlign(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        print_style_align(mxObject, m21Object)
    }

    /// Position, font and colour.
    fn setPrintStyle(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        print_style(mxObject, m21Object)
    }

    /// `print-object="no"` where the style hides the object.
    fn setPrintObject(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let Some(style) = style_of(m21Object)? else {
            return Ok(());
        };
        if style.getattr("hideObjectOnPrint")?.is_truthy()? {
            mxObject.call_method1("set", ("print-object", "no"))?;
        }
        Ok(())
    }

    /// The style's colour, as a hex value.
    fn setColor(&self, mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        set_color(mxObject, m21Object)
    }

    /// The style's font: family, style, size and weight. MusicXML has no
    /// bold style, which is written as a weight.
    fn setFont(&self, mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        set_font(mxObject, m21Object)
    }

    /// The style's place: `default-x`, `default-y`, `relative-x`,
    /// `relative-y`.
    fn setPosition(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        set_style_attributes(mxObject, m21Object, &POSITION)
    }

    /// The object's first footnote, or else its first comment, as a
    /// `<footnote>` and a `<level>`, MusicXML allowing one of each.
    fn setEditorial(
        &self,
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = mxObject.py();
        if !m21Object.getattr("hasEditorialInformation")?.is_truthy()? {
            return Ok(());
        }
        let editorial = m21Object.getattr("editorial")?;
        let first = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
            let Ok(list) = editorial.getattr(name) else {
                return Ok(None);
            };
            if list.len()? == 0 {
                return Ok(None);
            }
            list.get_item(0).map(Some)
        };
        let (comment, footnote) = match first("footnotes")? {
            Some(footnote) => (footnote, true),
            None => match first("comments")? {
                Some(comment) => (comment, false),
                None => return Ok(()),
            },
        };
        let sub = element_tree(py)?.getattr("SubElement")?;
        if footnote {
            let written = sub.call1((mxObject, "footnote"))?;
            set_style_attributes(&written, &comment, &TEXT_FORMATTING)?;
            print_style_align(&written, &comment)?;
            let text = comment.getattr("text")?;
            if !text.is_none() {
                written.setattr("text", text)?;
            }
        }
        let level = comment.getattr("levelInformation")?;
        if !level.is_none() {
            let written = sub.call1((mxObject, "level"))?;
            written.setattr("text", level.str()?)?;
            let reference = comment.getattr("isReference")?.is_truthy()?;
            written.call_method1("set", ("reference", if reference { "yes" } else { "no" }))?;
        }
        Ok(())
    }

    /// A page layout as its `<page-layout>`.
    #[pyo3(signature = (pageLayout, mxPageLayoutIn = None))]
    fn pageLayoutToXmlPageLayout<'py>(
        &self,
        pageLayout: &Bound<'py, PyAny>,
        mxPageLayoutIn: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        layout_element(pageLayout, mxPageLayoutIn)
    }

    /// A system layout as its `<system-layout>`.
    #[pyo3(signature = (systemLayout, mxSystemLayoutIn = None))]
    fn systemLayoutToXmlSystemLayout<'py>(
        &self,
        systemLayout: &Bound<'py, PyAny>,
        mxSystemLayoutIn: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        layout_element(systemLayout, mxSystemLayoutIn)
    }

    /// A staff layout as its `<staff-layout>`.
    #[pyo3(signature = (staffLayout, mxStaffLayoutIn = None))]
    fn staffLayoutToXmlStaffLayout<'py>(
        &self,
        staffLayout: &Bound<'py, PyAny>,
        mxStaffLayoutIn: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        layout_element(staffLayout, mxStaffLayoutIn)
    }

    /// A page layout as the `<print>` a measure starting with it writes.
    #[pyo3(signature = (pageLayout, mxPrintIn = None))]
    fn pageLayoutToXmlPrint<'py>(
        &self,
        pageLayout: &Bound<'py, PyAny>,
        mxPrintIn: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        print_element(pageLayout, mxPrintIn)
    }

    /// A system layout as the `<print>` a measure starting with it writes.
    #[pyo3(signature = (systemLayout, mxPrintIn = None))]
    fn systemLayoutToXmlPrint<'py>(
        &self,
        systemLayout: &Bound<'py, PyAny>,
        mxPrintIn: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        print_element(systemLayout, mxPrintIn)
    }
}

/// A page, system or staff layout as the element it writes, its children
/// and attributes put into `into` where one is given, as music21 fills an
/// element it is handed.
fn layout_element<'py>(
    layout: &Bound<'py, PyAny>,
    into: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = layout.py();
    let Some(music21::stream::StreamElement::Layout(value)) =
        crate::stream::element_written(layout)?
    else {
        return Err(MusicXMLExportException::new_err(format!(
            "{} is no layout",
            layout.repr()?
        )));
    };
    let text = music21::musicxml::layout_xml(&value).ok_or_else(|| {
        MusicXMLExportException::new_err("a score layout is written as <defaults>")
    })?;
    filled(&parsed(py, &text)?, into)
}

/// The written element, or `into` given its attributes and children.
fn filled<'py>(
    written: &Bound<'py, PyAny>,
    into: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(into) = into.filter(|given| !given.is_none()) else {
        return Ok(written.clone());
    };
    into.getattr("attrib")?
        .call_method1("update", (written.getattr("attrib")?,))?;
    for child in written.try_iter()? {
        into.call_method1("append", (child?,))?;
    }
    Ok(into.clone())
}

/// The `<print>` a measure starting with this layout writes.
fn print_element<'py>(
    layout: &Bound<'py, PyAny>,
    into: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = layout.py();
    let measure = lone_measure(&[deep_copy(layout)?])?;
    let mut part = RsStream::with_kind(music21::stream::StreamKind::Part);
    part.push(crate::stream::crate_stream(&measure)?);
    let written = written_in_score(py, part, "print")?;
    filled(&written, into)
}

/// What a score's style writes in its `<defaults>`.
const STYLE_DEFAULTS: [&str; 5] = [
    "appearance",
    "music-font",
    "word-font",
    "lyric-font",
    "lyric-language",
];

/// music21's `%text-formatting` attributes beside the style names they
/// are read from.
const TEXT_FORMATTING: [(&str, &str); 8] = [
    ("justify", "justify"),
    ("text-decoration", "textDecoration"),
    ("text-rotation", "textRotation"),
    ("letter-spacing", "letterSpacing"),
    ("line-height", "lineHeight"),
    ("lang", "language"),
    ("text-direction", "textDirection"),
    ("enclosure", "enclosure"),
];

/// music21's `%position` attributes beside the style names they are read
/// from.
const POSITION: [(&str, &str); 4] = [
    ("default-x", "absoluteX"),
    ("default-y", "absoluteY"),
    ("relative-x", "relativeX"),
    ("relative-y", "relativeY"),
];

/// The style names music21 writes as `yes` or `no`.
const YES_NO_STYLES: [&str; 1] = ["hideObjectOnPrint"];

/// One name or several, as a list of names.
fn names_of(names: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Ok(name) = names.extract::<String>() {
        return Ok(vec![name]);
    }
    names.try_iter()?.map(|name| name?.extract()).collect()
}

/// The style of an object, or the object where it is a style; `None`
/// for an object that carries no style information.
fn style_of<'py>(object: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let style = class(object.py(), "music21.style", "Style")?;
    if object.is_instance(&style)? {
        return Ok(Some(object.clone()));
    }
    if !object.getattr("hasStyleInformation")?.is_truthy()? {
        return Ok(None);
    }
    object.getattr("style").map(Some)
}

/// music21's `setStyleAttributes` over pairs of MusicXML and style names.
fn set_style_attributes(
    mxObject: &Bound<'_, PyAny>,
    m21Object: &Bound<'_, PyAny>,
    pairs: &[(&str, &str)],
) -> PyResult<()> {
    let Some(style) = style_of(m21Object)? else {
        return Ok(());
    };
    for (xml, m21) in pairs {
        let Ok(mut value) = style.getattr(*m21) else {
            continue;
        };
        if YES_NO_STYLES.contains(m21) {
            let yes = value.is_truthy()?;
            value = (if yes { "yes" } else { "no" })
                .into_pyobject(mxObject.py())?
                .into_any();
        }
        if value.is_none() {
            continue;
        }
        mxObject.call_method1("set", (*xml, value.str()?))?;
    }
    Ok(())
}

/// music21's `setColor`.
fn set_color(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
    set_style_attributes(mxObject, m21Object, &[("color", "color")])?;
    let held = mxObject.call_method1("get", ("color",))?;
    if held.is_none() {
        return Ok(());
    }
    let normal = normalizeColor(&held.extract::<String>()?)?;
    mxObject.call_method1("set", ("color", normal))?;
    Ok(())
}

/// music21's `setFont`.
fn set_font(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
    set_style_attributes(
        mxObject,
        m21Object,
        &[
            ("font-style", "fontStyle"),
            ("font-size", "fontSize"),
            ("font-weight", "fontWeight"),
        ],
    )?;
    let Some(style) = style_of(m21Object)? else {
        return Ok(());
    };
    if let Ok(font_style) = style.getattr("fontStyle") {
        let attributes = mxObject.getattr("attrib")?;
        if font_style.eq("bold")? {
            mxObject.call_method1("set", ("font-weight", "bold"))?;
            attributes.call_method1("pop", ("font-style", mxObject.py().None()))?;
        } else if font_style.eq("bolditalic")? {
            mxObject.call_method1("set", ("font-weight", "bold"))?;
            mxObject.call_method1("set", ("font-style", "italic"))?;
        }
    }
    if let Ok(family) = style.getattr("fontFamily")
        && family.is_truthy()?
    {
        let joined = match family.extract::<String>() {
            Ok(one) => one,
            Err(_) => names_of(&family)?.join(","),
        };
        mxObject.call_method1("set", ("font-family", joined))?;
    }
    Ok(())
}

/// music21's `setPrintStyle`.
fn print_style(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
    set_style_attributes(mxObject, m21Object, &POSITION)?;
    set_font(mxObject, m21Object)?;
    set_color(mxObject, m21Object)
}

/// music21's `setPrintStyleAlign`.
fn print_style_align(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
    print_style(mxObject, m21Object)?;
    set_style_attributes(
        mxObject,
        m21Object,
        &[("valign", "alignVertical"), ("halign", "alignHorizontal")],
    )
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
///
/// `parse` writes the whole score at once. The steps music21 takes on the
/// way are members too, for a caller that takes them one by one: each
/// header step adds to the root what writing the score writes there.
#[pyclass(name = "ScoreExporter", module = "music21.musicxml.m21ToXml", extends = XMLExporterBase, subclass)]
struct ScoreExporter {
    /// Whether each part's notation is made before it is written.
    #[pyo3(get, set)]
    makeNotation: bool,
    /// The metadata the header is written from: `None` until
    /// `setScoreHeader` reads it off the score or its context.
    #[pyo3(get, set)]
    scoreMetadata: Py<PyAny>,
    #[pyo3(get, set)]
    spannerBundle: Py<PyAny>,
    #[pyo3(get, set)]
    meterStream: Py<PyAny>,
    #[pyo3(get, set)]
    scoreLayouts: Py<PyAny>,
    #[pyo3(get, set)]
    firstScoreLayout: Py<PyAny>,
    #[pyo3(get, set)]
    textBoxes: Py<PyAny>,
    #[pyo3(get, set)]
    highestTime: f64,
    #[pyo3(get, set)]
    refStreamOrTimeRange: Py<PyAny>,
    #[pyo3(get, set)]
    partExporterList: Py<PyAny>,
    #[pyo3(get, set)]
    parts: Py<PyAny>,
    /// The `<identification>` written, once `setIdentification` has run.
    #[pyo3(get, set)]
    mxIdentification: Py<PyAny>,
    /// The staff groups written as one part each, once found.
    #[pyo3(get, set)]
    groupsToJoin: Py<PyAny>,
}

/// The score an exporter writes, read into the crate.
fn exported_stream(slf: &Bound<'_, ScoreExporter>) -> PyResult<RsStream> {
    let py = slf.py();
    let stream = slf.as_super().borrow().stream.clone_ref(py);
    crate::stream::crate_stream(stream.bind(py))
}

/// What writing the score puts before its parts, element by element, with
/// the exporter's `scoreMetadata` (or `md`) as its metadata.
fn header_elements<'py>(
    slf: &Bound<'py, ScoreExporter>,
    md: Option<&Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = slf.py();
    let mut score = exported_stream(slf)?;
    let held = slf.borrow().scoreMetadata.clone_ref(py);
    let metadata = md.cloned().unwrap_or_else(|| held.into_bound(py));
    let metadata = match metadata.is_none() {
        true => None,
        false => Some(crate::stream::metadata_value(&metadata)?),
    };
    score.set_metadata(metadata);
    let options = options(py, Notation::AsItStands)?;
    music21::musicxml::score_header_xml(&score, &options)
        .map_err(|e| export_error(&e))?
        .iter()
        .map(|text| parsed(py, text))
        .collect()
}

/// The header elements with one of these tags, in order.
fn header_tagged<'py>(
    slf: &Bound<'py, ScoreExporter>,
    md: Option<&Bound<'py, PyAny>>,
    tags: &[&str],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut found = Vec::new();
    for element in header_elements(slf, md)? {
        let tag: String = element.getattr("tag")?.extract()?;
        if tags.contains(&tag.as_str()) {
            found.push(element);
        }
    }
    Ok(found)
}

/// The one header element with this tag, appended to the root.
fn header_appended<'py>(slf: &Bound<'py, ScoreExporter>, tag: &str) -> PyResult<Bound<'py, PyAny>> {
    let found = header_tagged(slf, None, &[tag])?
        .into_iter()
        .next()
        .ok_or_else(|| MusicXMLExportException::new_err(format!("no <{tag}> is written")))?;
    root_of(slf.as_super()).call_method1("append", (&found,))?;
    Ok(found)
}

/// An exporter's root.
fn root_of<'py>(base: &Bound<'py, XMLExporterBase>) -> Bound<'py, PyAny> {
    base.borrow()
        .xmlRoot
        .clone_ref(base.py())
        .into_bound(base.py())
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
        let none = || py.None();
        let list = || -> PyResult<Py<PyAny>> { Ok(PyList::empty(py).into_any().unbind()) };
        Ok(PyClassInitializer::from(base).add_subclass(Self {
            makeNotation,
            scoreMetadata: none(),
            spannerBundle: none(),
            meterStream: none(),
            scoreLayouts: list()?,
            firstScoreLayout: none(),
            textBoxes: list()?,
            highestTime: 0.0,
            refStreamOrTimeRange: PyList::new(py, [0.0, 0.0])?.into_any().unbind(),
            partExporterList: list()?,
            parts: list()?,
            mxIdentification: none(),
            groupsToJoin: list()?,
        }))
    }

    /// The score as a `<score-partwise>` element, which becomes the root;
    /// an empty score is written as music21's blank page.
    fn parse<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let notation = Notation::of(slf.borrow().makeNotation);
        let text = document(py, &exported_stream(slf)?, notation)?;
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

    /// Reads off the score what the later steps use: its layouts, its
    /// meters, its parts and length, its text boxes and its spanners.
    fn scorePreliminaries(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        Self::setScoreLayouts(slf)?;
        Self::setMeterStream(slf)?;
        Self::setPartsAndRefStream(slf)?;
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        let boxes = stream.call_method1("getElementsByClass", ("TextBox",))?;
        let mut held = slf.borrow_mut();
        held.textBoxes = PyList::new(py, boxes.try_iter()?.collect::<PyResult<Vec<_>>>()?)?
            .into_any()
            .unbind();
        held.highestTime = 0.0;
        if held.spannerBundle.is_none(py) {
            held.spannerBundle = stream.getattr("spannerBundle")?.unbind();
        }
        Ok(())
    }

    /// The score's `ScoreLayout`s, and the first of them.
    fn setScoreLayouts(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        let layouts = stream.call_method1("getElementsByClass", ("ScoreLayout",))?;
        let first = layouts.call_method0("first")?;
        let listed = PyList::new(py, layouts.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
        let mut held = slf.borrow_mut();
        held.firstScoreLayout = first.unbind();
        held.scoreLayouts = listed.into_any().unbind();
        Ok(())
    }

    /// The score's meters, a default one where it has none.
    fn setMeterStream(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        let keywords = PyDict::new(py);
        keywords.set_item("searchContext", false)?;
        keywords.set_item("sortByCreationTime", false)?;
        keywords.set_item("returnDefault", true)?;
        let meters = stream
            .getattr("getTimeSignatures")?
            .call((), Some(&keywords))?;
        slf.borrow_mut().meterStream = meters.unbind();
        Ok(())
    }

    /// The score's length, from the end of whichever stream in it ends
    /// last, and its parts. A stream other than a measure has its offset
    /// moved onto what it holds first, as music21 does.
    fn setPartsAndRefStream(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        let mut highest: f64 = slf.borrow().highestTime;
        for inner in stream
            .call_method1("getElementsByClass", ("Stream",))?
            .try_iter()?
        {
            let inner = inner?;
            if is_a(&inner, "Score")? || is_a(&inner, "Opus")? {
                return Err(MusicXMLExportException::new_err(
                    "Exporting scores nested inside scores is not supported",
                ));
            }
            // A measure keeps its offset (music21's issue 580).
            let mut start: f64 = 0.0;
            if is_a(&inner, "Measure")? {
                start = inner.getattr("offset")?.extract()?;
            } else {
                inner.call_method0("transferOffsetToElements")?;
            }
            let length: f64 = inner.getattr("highestTime")?.extract()?;
            highest = highest.max(start + length);
        }
        let parts: Vec<Bound<'_, PyAny>> = stream
            .getattr("parts")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let mut held = slf.borrow_mut();
        held.highestTime = highest;
        held.refStreamOrTimeRange = PyList::new(py, [0.0, highest])?.into_any().unbind();
        held.parts = PyList::new(py, parts)?.into_any().unbind();
        Ok(())
    }

    /// A `PartExporter` for each part, each parsed.
    fn parsePartlikeScore(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let exporters = slf.borrow().partExporterList.clone_ref(py).into_bound(py);
        if exporters.len()? == 0 {
            let parts = slf.borrow().parts.clone_ref(py).into_bound(py);
            let class = py.get_type::<PartExporter>();
            for part in parts.try_iter()? {
                let keywords = PyDict::new(py);
                keywords.set_item("parent", slf)?;
                exporters.call_method1("append", (class.call((part?,), Some(&keywords))?,))?;
            }
        }
        for exporter in exporters.try_iter()? {
            exporter?.call_method0("parse")?;
        }
        Ok(())
    }

    /// One `PartExporter` for a score with no parts, parsed.
    fn parseFlatScore(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py);
        let keywords = PyDict::new(py);
        keywords.set_item("parent", slf)?;
        let exporter = py
            .get_type::<PartExporter>()
            .call((stream,), Some(&keywords))?;
        exporter.call_method0("parse")?;
        slf.borrow()
            .partExporterList
            .bind(py)
            .call_method1("append", (exporter,))?;
        Ok(())
    }

    /// Joins the staves of each joinable group, writes the header, then
    /// each part exporter's `<part>` after a divider comment.
    fn postPartProcess(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        Self::joinPartStaffs(slf)?;
        Self::setScoreHeader(slf)?;
        let base = slf.as_super();
        let exporters = slf.borrow().partExporterList.clone_ref(py).into_bound(py);
        for (index, exporter) in exporters.try_iter()?.enumerate() {
            base.borrow()
                .addDividerComment(py, &format!("Part {}", index + 1))?;
            let part = exporter?.getattr("xmlRoot")?;
            root_of(base).call_method1("append", (part,))?;
        }
        Ok(())
    }

    /// The header: titles, identification, defaults, a credit for each
    /// text box, and the part list, from the metadata of the score or of
    /// the nearest stream it is in.
    fn setScoreHeader(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        slf.borrow_mut().scoreMetadata = getMetadataFromContext(&stream)?.unbind();
        let root = root_of(slf.as_super());
        for element in header_elements(slf, None)? {
            root.call_method1("append", (element,))?;
        }
        Ok(())
    }

    /// The `<work>`, `<movement-number>` and `<movement-title>`, appended.
    fn setTitles(slf: &Bound<'_, Self>) -> PyResult<()> {
        let root = root_of(slf.as_super());
        for element in header_tagged(slf, None, &["work", "movement-number", "movement-title"])? {
            root.call_method1("append", (element,))?;
        }
        Ok(())
    }

    /// The `<identification>`: who made the score and how it was
    /// encoded, appended.
    fn setIdentification<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let found = header_appended(slf, "identification")?;
        slf.borrow_mut().mxIdentification = found.clone().unbind();
        Ok(found)
    }

    /// The `<encoding>`: the date, the software and what is supported,
    /// appended to the `<identification>` where there is one yet.
    fn setEncoding<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let identification = header_tagged(slf, None, &["identification"])?
            .into_iter()
            .next()
            .ok_or_else(|| MusicXMLExportException::new_err("no <identification> is written"))?;
        let encoding = found_in(&identification, "encoding")?;
        let held = slf.borrow().mxIdentification.clone_ref(py).into_bound(py);
        if !held.is_none() {
            held.call_method1("append", (&encoding,))?;
        }
        Ok(encoding)
    }

    /// The `<supports>` the encoding lists.
    fn getSupports<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let identification = header_tagged(slf, None, &["identification"])?
            .into_iter()
            .next()
            .ok_or_else(|| MusicXMLExportException::new_err("no <identification> is written"))?;
        all_in(&identification, "encoding/supports")
    }

    /// The `<defaults>`: scaling and the layouts of the score's first
    /// `ScoreLayout`, appended.
    fn setDefaults<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        header_appended(slf, "defaults")
    }

    /// The `<part-list>`: each part's `<score-part>`, inside the
    /// `<part-group>`s of the staff groups that bracket them, appended.
    fn setPartList<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        header_appended(slf, "part-list")
    }

    /// The `<miscellaneous>` of the metadata's other fields, of `md` or of
    /// the exporter's `scoreMetadata`; `None` with neither.
    #[pyo3(signature = (md = None))]
    fn metadataToMiscellaneous<'py>(
        slf: &Bound<'py, Self>,
        md: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let py = slf.py();
        let md = md.filter(|given| !given.is_none());
        if md.is_none() && slf.borrow().scoreMetadata.is_none(py) {
            return Ok(None);
        }
        let identification = header_tagged(slf, md, &["identification"])?;
        let found = match identification.first() {
            Some(identification) => identification.call_method1("find", ("miscellaneous",))?,
            None => py.None().into_bound(py),
        };
        if !found.is_none() {
            return Ok(Some(found));
        }
        Ok(Some(
            element_tree(py)?
                .getattr("Element")?
                .call1(("miscellaneous",))?,
        ))
    }

    /// The score's `StaffGroup`s whose staves are written as one `<part>`
    /// each: groups of more than one `PartStaff`, each with measures, a
    /// second group over the same staves passed over. Where two overlap,
    /// none is joined, with a `MusicXMLWarning`.
    fn joinableGroups<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        use music21::musicxml::JoinableGroups;
        let py = slf.py();
        let joinable = music21::musicxml::joinable_staff_groups(&exported_stream(slf)?)
            .map_err(|e| export_error(&e))?;
        let indices = match joinable {
            JoinableGroups::Joined(indices) => indices,
            JoinableGroups::Overlapping => {
                PyErr::warn(
                    py,
                    &py.get_type::<MusicXMLWarning>(),
                    c"Got overlapping StaffGroups; will not merge ANY groups.",
                    2,
                )?;
                Vec::new()
            }
        };
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        let groups: Vec<Bound<'py, PyAny>> = stream
            .getattr("spannerBundle")?
            .call_method1("getByClass", ("StaffGroup",))?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        PyList::new(py, indices.iter().filter_map(|index| groups.get(*index)))
    }

    /// Joins the staves of each joinable group into the `<part>` of its
    /// first, in music21's steps: staff tags, the later staves' measures
    /// moved into the first's, the first `<attributes>` given the staves'
    /// clefs, and the later staves' exporters taken off the list.
    fn joinPartStaffs(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        if slf.borrow().groupsToJoin.bind(py).len()? == 0 {
            let groups = Self::joinableGroups(slf)?;
            slf.borrow_mut().groupsToJoin = groups.into_any().unbind();
        }
        let groups = slf.borrow().groupsToJoin.clone_ref(py).into_bound(py);
        for group in groups.try_iter()? {
            let group = group?;
            Self::addStaffTagsMultiStaffParts(slf, &group)?;
            Self::movePartStaffMeasureContents(slf, &group)?;
            Self::setEarliestAttributesAndClefsPartStaff(slf, &group)?;
            Self::cleanUpSubsequentPartStaffs(slf, &group)?;
        }
        Ok(())
    }

    /// A `<staff>` in every note, direction, forward and harmony of each
    /// staff of the group, numbered from one.
    fn addStaffTagsMultiStaffParts(
        slf: &Bound<'_, Self>,
        group: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut roots = group_roots(slf, group)?;
        music21::musicxml::add_staff_tags_to_staves(&mut roots).map_err(|e| step_refused(&e))
    }

    /// Every later staff's measures moved into the first staff's.
    fn movePartStaffMeasureContents(
        slf: &Bound<'_, Self>,
        group: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut roots = group_roots(slf, group)?;
        music21::musicxml::move_part_staff_contents(&mut roots).map_err(|e| step_refused(&e))
    }

    /// The first staff's first `<attributes>` told how many staves there
    /// are and given a clef for each, and a key and a meter for each where
    /// the staves start differently.
    fn setEarliestAttributesAndClefsPartStaff(
        slf: &Bound<'_, Self>,
        group: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let mut roots = group_roots(slf, group)?;
        let staves: Vec<RsStream> = staves_of(group)?
            .try_iter()?
            .map(|staff| crate::stream::crate_stream(&staff?))
            .collect::<PyResult<_>>()?;
        let held: Vec<&RsStream> = staves.iter().collect();
        let differences = music21::musicxml::staff_differences(&held);
        music21::musicxml::set_earliest_attributes_and_clefs(&mut roots, differences)
            .map_err(|e| step_refused(&e))
    }

    /// The exporters of the group's later staves taken off the list, their
    /// contents now in the first's `<part>`.
    fn cleanUpSubsequentPartStaffs(
        slf: &Bound<'_, Self>,
        group: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut later = Vec::new();
        for staff in staves_of(group)?.try_iter()?.skip(1) {
            later.push(Self::getRootForPartStaff(slf, &staff?)?);
        }
        let exporters = slf.borrow().partExporterList.clone_ref(py).into_bound(py);
        let kept = PyList::empty(py);
        for exporter in exporters.try_iter()? {
            let exporter = exporter?;
            let root = exporter.getattr("xmlRoot")?;
            if !later.iter().any(|gone| gone.is(&root)) {
                kept.append(exporter)?;
            }
        }
        slf.borrow_mut().partExporterList = kept.into_any().unbind();
        Ok(())
    }

    /// Moves each measure of `source` into the measure of `target` of the
    /// same number, on staff `staffNum`; the measures `target` has no
    /// number for come back by where they go, each after its divider.
    fn processSubsequentPartStaff<'py>(
        &self,
        target: &Bound<'py, PyAny>,
        source: &Bound<'py, PyAny>,
        staffNum: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let py = target.py();
        let mut into = Et(target.clone().unbind());
        let from = Et(source.clone().unbind());
        let insertions =
            music21::musicxml::process_subsequent_part_staff(&mut into, &from, staffNum)
                .map_err(|e| step_refused(&e))?;
        let found = PyDict::new(py);
        for (at, nodes) in insertions {
            let nodes: Vec<Py<PyAny>> = nodes.into_iter().map(|node| node.0).collect();
            found.set_item(at, PyList::new(py, nodes)?)?;
        }
        Ok(found)
    }

    /// Moves what `measure` holds into `otherMeasure`, behind a `<backup>`
    /// to its start, its voices numbered on and its mid-measure clefs told
    /// staff `staffNumber`.
    #[staticmethod]
    fn moveMeasureContents(
        measure: &Bound<'_, PyAny>,
        otherMeasure: &Bound<'_, PyAny>,
        staffNumber: usize,
    ) -> PyResult<()> {
        // Refused here as music21 says it, with the elements' reprs.
        let is_measure = |element: &Bound<'_, PyAny>| -> PyResult<bool> {
            element.getattr("tag")?.eq("measure")
        };
        if !is_measure(measure)? || !is_measure(otherMeasure)? {
            return Err(MusicXMLExportException::new_err(format!(
                "moveMeasureContents() called on {} and {} (not measures).",
                measure.repr()?,
                otherMeasure.repr()?
            )));
        }
        let from = Et(measure.clone().unbind());
        let mut into = Et(otherMeasure.clone().unbind());
        music21::musicxml::move_measure_contents(&from, &mut into, staffNumber)
            .map_err(|e| step_refused(&e))
    }

    /// The `<part>` a part exporter wrote for a staff, found by the staff
    /// itself, then by what it was derived from, then by id.
    fn getRootForPartStaff<'py>(
        slf: &Bound<'py, Self>,
        partStaff: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let exporters: Vec<Bound<'py, PyAny>> = slf
            .borrow()
            .partExporterList
            .clone_ref(py)
            .into_bound(py)
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let chain = |exporter: &Bound<'py, PyAny>| -> PyResult<Vec<Bound<'py, PyAny>>> {
            exporter
                .getattr("stream")?
                .getattr("derivation")?
                .call_method0("chain")?
                .try_iter()?
                .collect()
        };
        let same_id = |left: &Bound<'py, PyAny>| -> PyResult<bool> {
            left.getattr("id")?.eq(partStaff.getattr("id")?)
        };
        for exporter in &exporters {
            if exporter.getattr("stream")?.is(partStaff) {
                return exporter.getattr("xmlRoot");
            }
        }
        for exporter in &exporters {
            if chain(exporter)?.iter().any(|derived| derived.is(partStaff)) {
                return exporter.getattr("xmlRoot");
            }
        }
        for exporter in &exporters {
            if same_id(&exporter.getattr("stream")?)? {
                return exporter.getattr("xmlRoot");
            }
        }
        for exporter in &exporters {
            for derived in chain(exporter)? {
                if same_id(&derived)? {
                    return exporter.getattr("xmlRoot");
                }
            }
        }
        Err(MusicXMLExportException::new_err(format!(
            "{} not found in self.partExporterList",
            partStaff.repr()?
        )))
    }

    /// A staff group as the `<part-group type="start">` the part list
    /// writes for it, with no number yet.
    fn staffGroupToXmlPartGroup<'py>(
        slf: &Bound<'py, Self>,
        staffGroup: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
        let parts: Vec<Bound<'py, PyAny>> = stream
            .getattr("parts")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let group = crate::stream::staff_group_value(staffGroup, &parts)?;
        parsed(py, &music21::musicxml::part_group_xml(&group))
    }

    /// The score style's line widths, note sizes, distances and other
    /// appearances as the `<appearance>` its `<defaults>` writes; `None`
    /// where it has none.
    fn styleToXmlAppearance<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let defaults = header_tagged(slf, None, &["defaults"])?;
        let Some(defaults) = defaults.first() else {
            return Ok(None);
        };
        let found = defaults.call_method1("find", ("appearance",))?;
        Ok((!found.is_none()).then_some(found))
    }

    /// Adds to a `<defaults>` what the score style puts there: its
    /// `<appearance>`, its music and word fonts, and a `<lyric-font>` and
    /// `<lyric-language>` for each lyric name.
    fn addStyleToXmlDefaults(slf: &Bound<'_, Self>, mxDefaults: &Bound<'_, PyAny>) -> PyResult<()> {
        for defaults in header_tagged(slf, None, &["defaults"])? {
            for child in defaults.try_iter()? {
                let child = child?;
                let tag: String = child.getattr("tag")?.extract()?;
                if STYLE_DEFAULTS.contains(&tag.as_str()) {
                    mxDefaults.call_method1("append", (child,))?;
                }
            }
        }
        Ok(())
    }

    /// A text box as the `<credit>` the score writes for it.
    fn textBoxToXmlCredit<'py>(&self, textBox: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        first_fragment(textBox)
    }

    /// A contributor as a `<creator>` of its role.
    fn contributorToXmlCreator<'py>(&self, c: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = c.py();
        let creator = element_tree(py)?.getattr("Element")?.call1(("creator",))?;
        let role = c.getattr("role")?;
        if !role.is_none() {
            creator.call_method1("set", ("type", role.str()?))?;
        }
        let name = c.getattr("name")?;
        if !name.is_none() {
            creator.setattr("text", name)?;
        }
        Ok(creator)
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

    /// An instrument as its `<score-instrument>`.
    fn instrumentToXmlScoreInstrument<'py>(
        &self,
        i: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (score, _) = music21::musicxml::instrument_xml(&crate::stream::instrument_value(i)?);
        parsed(i.py(), &score)
    }

    /// An instrument as its `<midi-instrument>`, its channel and program
    /// counted from one.
    fn instrumentToXmlMidiInstrument<'py>(
        &self,
        i: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (_, midi) = music21::musicxml::instrument_xml(&crate::stream::instrument_value(i)?);
        parsed(i.py(), &midi)
    }

    /// The part's `<score-part>`: its name, abbreviation and instruments.
    fn getXmlScorePart<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stream = slf.as_super().borrow().stream.clone_ref(py);
        let part = crate::stream::crate_stream(stream.bind(py))?;
        written_in_score(py, part, "score-part")
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

/// The staves a staff group spans.
fn staves_of<'py>(group: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    group.call_method0("getSpannedElements")
}

/// The `<part>` each staff of a group was written as, in order.
fn group_roots(slf: &Bound<'_, ScoreExporter>, group: &Bound<'_, PyAny>) -> PyResult<Vec<Et>> {
    staves_of(group)?
        .try_iter()?
        .map(|staff| {
            Ok(Et(
                ScoreExporter::getRootForPartStaff(slf, &staff?)?.unbind()
            ))
        })
        .collect()
}

/// The first element of this tag in the document a part makes when it
/// is written as the only part of a score.
fn written_in_score<'py>(
    py: Python<'py>,
    part: RsStream,
    tag: &str,
) -> PyResult<Bound<'py, PyAny>> {
    // A part goes in a score; a stream of measures is written as one.
    let score = match part.kind() {
        music21::stream::StreamKind::Part | music21::stream::StreamKind::PartStaff => {
            let mut score = RsStream::with_kind(music21::stream::StreamKind::Score);
            score.push(part);
            score
        }
        _ => part,
    };
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
    let offset: f64 = object
        .getattr("offset")
        .and_then(|offset| offset.extract())
        .unwrap_or(0.0);

    // The spanners joining it, and where it stands in each.
    let mut held = Vec::new();
    if let Ok(sites) = object.call_method0("getSpannerSites") {
        for site in sites.try_iter()? {
            let site = site?;
            let joined: Vec<Bound<'py, PyAny>> = site
                .call_method0("getSpannedElements")?
                .try_iter()?
                .collect::<PyResult<_>>()?;
            let Some(index) = joined.iter().position(|one| one.is(object)) else {
                continue;
            };
            if let Some(spanner) = crate::stream::spanner_value(&site, &[])? {
                held.push((spanner, index, joined.len()));
            }
        }
    }
    let spanners: Vec<music21::musicxml::Joined<'_>> = held
        .iter()
        .map(|(spanner, index, count)| music21::musicxml::Joined {
            spanner,
            index: *index,
            count: *count,
        })
        .collect();
    let around = music21::musicxml::Surroundings {
        measure: measure.as_ref(),
        offset,
        spanners: &spanners,
    };
    music21::musicxml::element_xml(&element, &around)
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

    /// One beam as its `<beam>`.
    fn beamToXml<'py>(&self, beamObject: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = beamObject.py();
        let beams = class(py, "music21.beam", "Beams")?.call0()?;
        beams
            .getattr("beamsList")?
            .call_method1("append", (deep_copy(beamObject)?,))?;
        let duration = class(py, "music21.duration", "Duration")?.call1(("eighth",))?;
        let note = carrier(py, "duration", &duration)?;
        note.setattr("beams", beams)?;
        found_in(&first_fragment(&note)?, "beam")
    }

    /// A tuplet as the list of `<tuplet>` a note in it writes in its
    /// notations, which is empty unless the tuplet starts or stops there.
    #[pyo3(signature = (tuplet, tupletIndex = 1))]
    fn tupletToXmlTuplet<'py>(
        &self,
        tuplet: &Bound<'py, PyAny>,
        tupletIndex: usize,
    ) -> PyResult<Bound<'py, PyList>> {
        let _ = tupletIndex;
        let py = tuplet.py();
        let duration = class(py, "music21.duration", "Duration")?.call1(("eighth",))?;
        duration.call_method1("appendTuplet", (deep_copy(tuplet)?,))?;
        let note = carrier(py, "duration", &duration)?;
        all_in(&first_fragment(&note)?, "notations/tuplet")
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

    /// A Roman numeral as the `<note>`s of its chord, or, with
    /// `writeAsChord` off, as a `<harmony>` holding MusicXML 4's
    /// `<numeral>`; either is appended.
    fn romanNumeralToXml<'py>(
        slf: &Bound<'py, Self>,
        rn: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if rn.getattr("writeAsChord")?.is_truthy()? {
            return Ok(Self::chordToXml(slf, rn)?.into_any());
        }
        appended(slf, fragments(rn)?)
    }

    /// A no-chord symbol as its `<harmony>` of kind `none`.
    fn noChordToXml<'py>(
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

    /// The attributes a spanner's start writes, as a dict: music21's
    /// `_spannerStartParameters`, which dispatches on the class named;
    /// here the spanner's own class decides.
    #[staticmethod]
    fn _spannerStartParameters<'py>(
        spannerClass: &str,
        sp: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let _ = spannerClass;
        spanner_parameters(sp, music21::musicxml::SpannerEnd::Start)
    }

    /// The attributes a spanner's stop writes, as a dict: music21's
    /// `_spannerEndParameters`.
    #[staticmethod]
    fn _spannerEndParameters<'py>(
        spannerClass: &str,
        sp: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyDict>> {
        let _ = spannerClass;
        spanner_parameters(sp, music21::musicxml::SpannerEnd::Stop)
    }

    /// A transposing interval as its `<transpose>`; the measure's own
    /// part's transposition where none is given.
    #[pyo3(signature = (i = None))]
    fn intervalToXmlTranspose<'py>(
        slf: &Bound<'py, Self>,
        i: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let interval = match i.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => {
                let stream = slf.as_super().borrow().stream.clone_ref(py).into_bound(py);
                let instrument = stream.call_method1("getInstrument", (false,))?;
                instrument.getattr("transposition")?
            }
        };
        if interval.is_none() {
            return Err(MusicXMLExportException::new_err(
                "there is no transposition to write",
            ));
        }
        let value = crate::interval::interval_from_any(&interval)?;
        parsed(py, &music21::musicxml::transpose_xml(&value))
    }

    /// A staff layout as its `<staff-details>`.
    fn staffLayoutToXmlStaffDetails<'py>(
        &self,
        staffLayout: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let Some(music21::stream::StreamElement::Layout(music21::layout::Layout::Staff(staff))) =
            crate::stream::element_written(staffLayout)?
        else {
            return Err(MusicXMLExportException::new_err("not a staff layout"));
        };
        parsed(
            staffLayout.py(),
            &music21::musicxml::staff_details_xml(&staff),
        )
    }

    /// A pedal's bounce or gap as the `<direction>` its pedal mark draws
    /// it with, appended; `None` where no pedal mark holds it.
    fn pedalObjectToXml<'py>(
        slf: &Bound<'py, Self>,
        po: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let written = fragments(po)?;
        if written.is_empty() {
            return Ok(None);
        }
        appended(slf, written).map(Some)
    }

    /// A stopped string as its `<frame-note>`.
    fn fretNoteToXml<'py>(&self, fretNote: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let note = fret_note_value(fretNote)?;
        parsed(fretNote.py(), &music21::musicxml::frame_note_xml(&note))
    }

    /// A chord diagram as its `<frame>`, lowest string first; `None` for
    /// one with no notes.
    fn fretBoardToXml<'py>(
        &self,
        fretBoard: &Bound<'py, PyAny>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let mut notes = Vec::new();
        for note in fretBoard.getattr("fretNotes")?.try_iter()? {
            notes.push(fret_note_value(&note?)?);
        }
        let board = music21::tablature::FretBoard::new(
            fretBoard.getattr("numStrings")?.extract()?,
            notes,
            fretBoard.getattr("displayFrets")?.extract()?,
        );
        music21::musicxml::frame_xml(&board)
            .map(|text| parsed(fretBoard.py(), &text))
            .transpose()
    }

    /// A chord symbol with a diagram as its `<harmony>`, the `<frame>` at
    /// its end.
    fn chordWithFretBoardToXml<'py>(
        slf: &Bound<'py, Self>,
        cwf: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let harmony = Self::chordSymbolToXml(slf, cwf)?;
        if let Some(frame) = slf.borrow().fretBoardToXml(cwf)? {
            harmony.call_method1("append", (frame,))?;
        }
        Ok(harmony)
    }

    /// A barline as its `<barline>`, with no location.
    fn barlineToXml<'py>(&self, barObject: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        first_fragment(barObject)
    }
}

/// A fret note of music21's, as the crate's.
fn fret_note_value(note: &Bound<'_, PyAny>) -> PyResult<music21::tablature::FretNote> {
    let read = |name: &str| -> PyResult<Option<i32>> {
        let value = note.getattr(name)?;
        if value.is_none() {
            return Ok(None);
        }
        value.extract().map(Some)
    };
    Ok(music21::tablature::FretNote::new(
        read("string")?,
        read("fret")?,
        read("fingering")?,
    ))
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

/// A spanner's parameters at one end, with music21's numbers as `int`s.
fn spanner_parameters<'py>(
    sp: &Bound<'py, PyAny>,
    end: music21::musicxml::SpannerEnd,
) -> PyResult<Bound<'py, PyDict>> {
    use music21::musicxml::SpannerParameter;
    let py = sp.py();
    let Some(spanner) = crate::stream::spanner_value(sp, &[])? else {
        return Err(MusicXMLExportException::new_err(format!(
            "{} is no spanner the writer knows",
            sp.repr()?
        )));
    };
    let post = PyDict::new(py);
    for (name, value) in music21::musicxml::spanner_parameters(&spanner, end) {
        match value {
            SpannerParameter::Text(text) => post.set_item(name, text)?,
            SpannerParameter::Whole(number) => post.set_item(name, number)?,
            SpannerParameter::Decimal(number) => post.set_item(name, number)?,
        }
    }
    Ok(post)
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
