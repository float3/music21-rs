//! music21's `musicxml.xmlToM21`: `MusicXMLImporter`, `PartParser` and
//! `MeasureParser`, with the per-element steps music21's callers use.
//!
//! The steps read ElementTree elements, as music21's do: each element is
//! written back to text and read by the crate's `musicxml::step`, and what
//! that reads is handed back as this package's objects. A parser keeps
//! music21's attributes in its `__dict__` -- `stream`, `divisions`,
//! `offsetMeasureNote`, `lastClefs` and the rest -- so code reading or
//! setting them works as on music21's.
//!
//! What the crate's reader does not expose a step for -- notations,
//! ornaments, spanners read off one direction -- is not provided.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use music21::musicxml::step;

use crate::subconverters::music21_name;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyString, PyTuple};

/// The module music21 keeps these in.
const MODULE: &str = "music21.musicxml.xmlToM21";

/// music21's `NO_STAFF_ASSIGNED`: the staff of an element that names none.
const NO_STAFF_ASSIGNED: i32 = 0;

/// The tag joining a note to the chord of the note before it.
const CHORD: &str = "chord";

/// How many tuplets deep music21 keeps track of at once.
const TUPLET_DEPTH: usize = 7;

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    MusicXMLImportException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    MusicXMLWarning,
    pyo3::exceptions::PyUserWarning
);

/// A crate error as music21's reader raises it.
fn import_error(error: music21::Error) -> PyErr {
    MusicXMLImportException::new_err(crate::pitch::message(&error))
}

/// An ElementTree element written back as text.
fn written(element: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = element.py();
    let keywords = PyDict::new(py);
    keywords.set_item("encoding", "unicode")?;
    py.import("xml.etree.ElementTree")?
        .getattr("tostring")?
        .call((element,), Some(&keywords))?
        .extract()
}

/// A crate element as this package's object, `None` where it has no class.
fn object_of<'py>(
    py: Python<'py>,
    element: &music21::StreamElement,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(crate::stream::element_object(py, element)?.unwrap_or_else(|| py.None().into_bound(py)))
}

/// The parser's divisions to the quarter, where it has a number for them.
fn divisions_of(slf: &Bound<'_, PyAny>) -> PyResult<Option<i32>> {
    let divisions = slf.getattr("divisions")?;
    if divisions.is_none() {
        return Ok(None);
    }
    Ok(Some(divisions.extract::<f64>()?.round() as i32))
}

/// A measure's text read with the parser's divisions, as this package's
/// `Measure`.
fn read_measure<'py>(slf: &Bound<'py, PyAny>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    let read = step::read_measure(text, divisions_of(slf)?).map_err(import_error)?;
    crate::stream::from_crate(slf.py(), &read)
}

/// The elements of a measure, its voices' included, that are one of
/// `class`, in order.
fn of_class<'py>(measure: &Bound<'py, PyAny>, class: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut found = Vec::new();
    for element in measure.call_method0("recurse")?.try_iter()? {
        let element = element?;
        if element.getattr("classes")?.contains(class)? {
            found.push(element);
        }
    }
    Ok(found)
}

/// The first element of `class` in a measure, or `None`.
fn first_of<'py>(measure: &Bound<'py, PyAny>, class: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = measure.py();
    Ok(of_class(measure, class)?
        .into_iter()
        .next()
        .unwrap_or_else(|| py.None().into_bound(py)))
}

/// A `<note>` copied with its children of `tags` taken off: `<chord/>`
/// so it reads alone, `<staff>` so it reads on the one staff.
fn without(note: &Bound<'_, PyAny>, tags: &[&str]) -> PyResult<String> {
    let py = note.py();
    let copied = py.import("copy")?.call_method1("deepcopy", (note,))?;
    for tag in tags {
        for child in copied.call_method1("findall", (*tag,))?.try_iter()? {
            copied.call_method1("remove", (child?,))?;
        }
    }
    written(&copied)
}

/// A clef of music21's written back as `<attributes>`, nothing where it
/// is none or has no line.
fn clef_written(clef: &Bound<'_, PyAny>) -> PyResult<String> {
    if clef.is_none() {
        return Ok(String::new());
    }
    let sign: Option<String> = clef.getattr("sign")?.extract()?;
    let line: Option<i64> = clef.getattr("line")?.extract()?;
    let (Some(sign), Some(line)) = (sign, line) else {
        return Ok(String::new());
    };
    let octave: i64 = clef.getattr("octaveChange")?.extract()?;
    Ok(format!(
        "<attributes><clef><sign>{sign}</sign><line>{line}</line>\
         <clef-octave-change>{octave}</clef-octave-change></clef></attributes>"
    ))
}

/// One element read inside a measure of its own.
fn in_measure(texts: &[String]) -> String {
    format!("<measure>{}</measure>", texts.concat())
}

/// The quarter length a `<duration>`-like child names at the parser's
/// divisions, snapped as music21's `opFrac` snaps it.
fn quarters<'py>(slf: &Bound<'py, PyAny>, element: &Bound<'py, PyAny>) -> PyResult<f64> {
    let text = strippedText(Some(element.clone()));
    let Ok(value) = text.parse::<f64>() else {
        return Ok(0.0);
    };
    let divisions = divisions_of(slf)?.unwrap_or(1).max(1);
    Ok(
        music21::common::numbertools::op_frac(value / f64::from(divisions))
            .map_err(import_error)?
            .to_f64(),
    )
}

// ------------------------------------------------------------------ functions

/// music21's `_clean`: the text stripped, its line breaks made spaces.
#[pyfunction]
#[pyo3(name = "_clean")]
fn clean(badStr: Option<&str>) -> Option<String> {
    badStr.map(|text| text.trim().replace('\n', " "))
}

/// music21's `strippedText`: an element's text stripped, empty where it
/// has none or is no element.
#[pyfunction]
#[pyo3(signature = (mxObj))]
fn strippedText(mxObj: Option<Bound<'_, PyAny>>) -> String {
    let Some(element) = mxObj else {
        return String::new();
    };
    let Ok(text) = element.getattr("text") else {
        return String::new();
    };
    text.extract::<String>()
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

/// music21's `musicXMLTypeToType`: a MusicXML duration type as music21's,
/// `long` as `longa` and Guitar Pro's `32th` as `32nd`.
#[pyfunction]
fn musicXMLTypeToType(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: Option<String> = value.extract().ok();
    let known = name
        .as_deref()
        .is_some_and(|name| music21::duration::DurationType::from_music21_name(name).is_some());
    match name.as_deref() {
        Some(name) if known => Ok(name.to_string()),
        Some("long") => Ok("longa".to_string()),
        Some("32th") => Ok("32nd".to_string()),
        _ => Err(MusicXMLImportException::new_err(format!(
            "found unknown MusicXML type: {}",
            value.repr()?
        ))),
    }
}

/// music21's `_floatOrIntStr`: a number where the text is one, an `int`
/// where it is whole, else the text.
#[pyfunction]
#[pyo3(name = "_floatOrIntStr")]
fn float_or_int<'py>(py: Python<'py>, strObj: Option<&str>) -> PyResult<Bound<'py, PyAny>> {
    let Some(text) = strObj else {
        return Ok(py.None().into_bound(py));
    };
    let Ok(value) = text.trim().parse::<f64>() else {
        return Ok(PyString::new(py, text).into_any());
    };
    if value.is_finite() && value == value.trunc() {
        return Ok((value as i64).into_pyobject(py)?.into_any());
    }
    Ok(value.into_pyobject(py)?.into_any())
}

// ------------------------------------------------------------------ classes

/// music21's `XMLParserBase`: what the three parsers share.
#[pyclass(
    name = "XMLParserBase",
    module = "music21.musicxml.xmlToM21",
    subclass,
    dict
)]
pub struct XMLParserBase;

#[pymethods]
impl XMLParserBase {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        let _ = (args, kwargs);
        Self
    }
}

/// music21's `MusicXMLImporter`: a whole document, read into `stream`.
#[pyclass(
    name = "MusicXMLImporter",
    module = "music21.musicxml.xmlToM21",
    extends = XMLParserBase,
    subclass
)]
pub struct MusicXMLImporter;

#[pymethods]
impl MusicXMLImporter {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(XMLParserBase::new(args, kwargs)).add_subclass(Self)
    }

    /// music21's attributes, each as a new importer starts with it.
    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let score = music21_name(py, "music21.stream", "Score")?.call0()?;
        for (name, value) in [
            ("xmlText", py.None().into_bound(py)),
            ("xmlFilename", py.None().into_bound(py)),
            ("xmlRoot", py.None().into_bound(py)),
            ("stream", score),
            (
                "definesExplicitSystemBreaks",
                false.into_pyobject(py)?.to_owned().into_any(),
            ),
            (
                "definesExplicitPageBreaks",
                false.into_pyobject(py)?.to_owned().into_any(),
            ),
            (
                "applyFinaleWorkarounds",
                false.into_pyobject(py)?.to_owned().into_any(),
            ),
            ("partIdDictionary", PyDict::new(py).into_any()),
            ("m21PartObjectsById", PyDict::new(py).into_any()),
            ("partGroupList", PyList::empty(py).into_any()),
            ("parts", PyList::empty(py).into_any()),
            ("musicXmlVersion", PyString::new(py, "4.0").into_any()),
        ] {
            slf.setattr(name, value)?;
        }
        Ok(())
    }

    /// music21's `scoreFromFile`: the file read and its score handed back.
    fn scoreFromFile(slf: &Bound<'_, Self>, filename: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::readFile(slf, filename)?;
        Self::parseXMLText(slf)?;
        Ok(slf.getattr("stream")?.unbind())
    }

    /// music21's `readFile`: the file's text kept as `xmlText`.
    fn readFile(slf: &Bound<'_, Self>, filename: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let keywords = PyDict::new(py);
        keywords.set_item("encoding", "utf-8")?;
        let file = py
            .import("builtins")?
            .getattr("open")?
            .call((filename,), Some(&keywords))?;
        let text = file.call_method0("read")?;
        file.call_method0("close")?;
        slf.setattr("xmlFilename", filename)?;
        slf.setattr("xmlText", text)
    }

    /// music21's `parseXMLText`: `xmlText` read into `stream`.
    fn parseXMLText(slf: &Bound<'_, Self>) -> PyResult<()> {
        let text = slf.getattr("xmlText")?;
        let text: String = match text.extract::<Vec<u8>>() {
            Ok(bytes) if !text.is_instance_of::<PyString>() => {
                String::from_utf8_lossy(&bytes).into_owned()
            }
            _ => text.extract()?,
        };
        Self::read_into(slf, &text)
    }

    /// music21's `xmlRootToScore`: a `<score-partwise>` element read into
    /// `stream`, which is handed back.
    #[pyo3(signature = (mxScore, inputM21 = None))]
    fn xmlRootToScore(
        slf: &Bound<'_, Self>,
        mxScore: &Bound<'_, PyAny>,
        inputM21: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let _ = inputM21;
        slf.setattr("xmlRoot", mxScore)?;
        Self::read_into(slf, &written(mxScore)?)?;
        Ok(slf.getattr("stream")?.unbind())
    }

    /// music21's `isRecognizableMetadataKey`: whether a `<miscellaneous-field>`
    /// names one of music21's metadata properties.
    #[staticmethod]
    fn isRecognizableMetadataKey(py: Python<'_>, miscFieldName: &str) -> PyResult<bool> {
        let metadata = music21_name(py, "music21.metadata", "Metadata")?;
        if metadata
            .call_method1("_isStandardUniqueName", (miscFieldName,))
            .and_then(|found| found.is_truthy())
            .unwrap_or(false)
        {
            return Ok(true);
        }
        metadata
            .call_method1("_isStandardNamespaceName", (miscFieldName,))
            .and_then(|found| found.is_truthy())
            .or(Ok(false))
    }
}

impl MusicXMLImporter {
    /// A document's text read as the score `stream` holds.
    fn read_into(slf: &Bound<'_, Self>, text: &str) -> PyResult<()> {
        let py = slf.py();
        let read = music21::musicxml::from_musicxml(text).map_err(import_error)?;
        let score = crate::formats::handed_back(py, Ok(read))?;
        slf.setattr("stream", score)
    }
}

/// music21's `PartParser`: one `<part>` and its `<score-part>`, read into
/// `stream`.
#[pyclass(
    name = "PartParser",
    module = "music21.musicxml.xmlToM21",
    extends = XMLParserBase,
    subclass
)]
pub struct PartParser;

#[pymethods]
impl PartParser {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(XMLParserBase::new(args, kwargs)).add_subclass(Self)
    }

    /// music21's attributes, each as a new part parser starts with it.
    #[pyo3(signature = (mxPart = None, mxScorePart = None, parent = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        mxPart: Option<Bound<'_, PyAny>>,
        mxScorePart: Option<Bound<'_, PyAny>>,
        parent: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let none = || py.None().into_bound(py);
        let parent = match parent.filter(|parent| !parent.is_none()) {
            Some(given) => given,
            None => py.get_type::<MusicXMLImporter>().call0()?,
        };
        let part_id = match &mxPart {
            Some(part) if !part.is_none() => part.call_method1("get", ("id", ""))?,
            _ => PyString::new(py, "").into_any(),
        };
        let clefs = PyDict::new(py);
        clefs.set_item(
            NO_STAFF_ASSIGNED,
            music21_name(py, "music21.clef", "TrebleClef")?.call0()?,
        )?;
        let tuplets = PyList::new(py, (0..TUPLET_DEPTH).map(|_| py.None()))?;
        let divisions = music21_name(py, "music21.defaults", "divisionsPerQuarter")
            .unwrap_or_else(|_| 10080_i32.into_pyobject(py).unwrap().into_any());
        let falsy = false.into_pyobject(py)?.to_owned().into_any();
        for (name, value) in [
            ("mxPart", mxPart.unwrap_or_else(none)),
            ("mxScorePart", mxScorePart.unwrap_or_else(none)),
            ("partId", part_id),
            ("parent", parent),
            (
                "stream",
                music21_name(py, "music21.stream", "Part")?.call0()?,
            ),
            ("spannerBundle", PyList::empty(py).into_any()),
            ("activeTuplets", tuplets.into_any()),
            ("lastClefs", clefs.into_any()),
            ("lastDivisions", divisions),
            ("lastMeasureOffset", 0.0_f64.into_pyobject(py)?.into_any()),
            ("lastTimeSignature", none()),
            ("lastMeasureWasShort", falsy.clone()),
            ("lastMeasureNumber", 0_i32.into_pyobject(py)?.into_any()),
            ("lastNumberSuffix", none()),
            ("activeMultiMeasureRestSpanner", none()),
            (
                "multiMeasureRestsToCapture",
                0_i32.into_pyobject(py)?.into_any(),
            ),
            ("activeInstrument", none()),
            ("firstMeasureParsed", falsy.clone()),
            (
                "atSoundingPitch",
                true.into_pyobject(py)?.to_owned().into_any(),
            ),
            ("staffReferenceList", PyList::empty(py).into_any()),
            ("maxStaves", 1_i32.into_pyobject(py)?.into_any()),
        ] {
            slf.setattr(name, value)?;
        }
        Ok(())
    }

    /// music21's `parse`: the part read into `stream`, and each staff of a
    /// part written on several put in the parent's score.
    fn parse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let part = slf.getattr("mxPart")?;
        let score_part = slf.getattr("mxScorePart")?;
        let score_part = match score_part.is_none() {
            true => None,
            false => Some(written(&score_part)?),
        };
        let read = step::read_part(&written(&part)?, score_part.as_deref())
            .map_err(|error| Self::placed(slf, &error))?;
        let mut made = Vec::new();
        for staff in &read {
            made.push(crate::stream::from_crate(py, staff)?);
        }
        let Some(first) = made.first() else {
            return Ok(());
        };
        slf.setattr("stream", first)?;
        if made.len() > 1 {
            let score = slf.getattr("parent")?.getattr("stream")?;
            for staff in &made {
                score.call_method1("insert", (0.0, staff))?;
            }
        }
        Ok(())
    }

    /// music21's `xmlMeasureToMeasure`: one `<measure>` read and appended
    /// to `stream` after the measure before it.
    fn xmlMeasureToMeasure<'py>(
        slf: &Bound<'py, Self>,
        mxMeasure: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let parser = py.get_type::<MeasureParser>().call1((mxMeasure, slf))?;
        parser.call_method0("parse")?;
        let measure = parser.getattr("stream")?;
        let offset = slf.getattr("lastMeasureOffset")?;
        slf.getattr("stream")?
            .call_method1("insert", (&offset, &measure))?;
        let length: f64 = measure
            .getattr("duration")?
            .getattr("quarterLength")?
            .extract()?;
        let offset: f64 = offset.extract()?;
        slf.setattr("lastMeasureOffset", offset + length)?;
        slf.setattr("lastDivisions", parser.getattr("divisions")?)?;
        slf.setattr("lastMeasureNumber", parser.getattr("measureNumber")?)?;
        slf.setattr("lastNumberSuffix", parser.getattr("numberSuffix")?)?;
        Ok(measure)
    }
}

/// music21's `MeasureParser`: one `<measure>`, read into `stream`, and the
/// steps that read each element in it.
#[pyclass(
    name = "MeasureParser",
    module = "music21.musicxml.xmlToM21",
    extends = XMLParserBase,
    subclass
)]
pub struct MeasureParser;

#[pymethods]
impl MeasureParser {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn new(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(XMLParserBase::new(args, kwargs)).add_subclass(Self)
    }

    /// music21's attributes, each as a new measure parser starts with it;
    /// a parser made with no parent makes a `PartParser` for one.
    #[pyo3(signature = (mxMeasure = None, parent = None))]
    fn __init__(
        slf: &Bound<'_, Self>,
        mxMeasure: Option<Bound<'_, PyAny>>,
        parent: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let none = || py.None().into_bound(py);
        let parent = match parent.filter(|parent| !parent.is_none()) {
            Some(given) => given,
            None => py.get_type::<PartParser>().call0()?,
        };
        let counts = PyDict::new(py);
        counts.set_item("rest", 0)?;
        counts.set_item("note", 0)?;
        let falsy =
            || -> PyResult<Bound<'_, PyAny>> { Ok(false.into_pyobject(py)?.to_owned().into_any()) };
        let zero = || -> PyResult<Bound<'_, PyAny>> { Ok(0_i32.into_pyobject(py)?.into_any()) };
        let pyset = py.import("builtins")?.getattr("set")?.call0()?;
        for (name, value) in [
            ("mxMeasure", mxMeasure.unwrap_or_else(none)),
            ("mxMeasureElements", PyList::empty(py).into_any()),
            ("transposition", none()),
            ("spannerBundle", parent.getattr("spannerBundle")?),
            ("staffReference", PyDict::new(py).into_any()),
            ("activeTuplets", parent.getattr("activeTuplets")?),
            ("useVoices", falsy()?),
            ("voicesById", PyDict::new(py).into_any()),
            ("voiceIndices", pyset),
            ("staves", 1_i32.into_pyobject(py)?.into_any()),
            ("activeAttributes", none()),
            (
                "attributesAreInternal",
                true.into_pyobject(py)?.to_owned().into_any(),
            ),
            ("measureNumber", zero()?),
            ("numberSuffix", PyString::new(py, "").into_any()),
            ("divisions", parent.getattr("lastDivisions")?),
            ("staffLayoutObjects", PyDict::new(py).into_any()),
            (
                "stream",
                music21_name(py, "music21.stream", "Measure")?.call0()?,
            ),
            ("mxNoteList", PyList::empty(py).into_any()),
            ("mxLyricList", PyList::empty(py).into_any()),
            ("nLast", none()),
            ("lastVoice", none()),
            ("fullMeasureRest", falsy()?),
            ("restAndNoteCount", counts.into_any()),
            ("lastClefs", parent.getattr("lastClefs")?),
            ("parseIndex", zero()?),
            ("offsetMeasureNote", 0.0_f64.into_pyobject(py)?.into_any()),
            ("lastForwardTagCreatedByFinale", none()),
            ("parent", parent),
        ] {
            slf.setattr(name, value)?;
        }
        Ok(())
    }

    /// music21's `getStaffNumber`: the staff an element or a number names,
    /// `NO_STAFF_ASSIGNED` for `None`.
    #[staticmethod]
    fn getStaffNumber(mxObject: Option<&Bound<'_, PyAny>>) -> PyResult<i32> {
        let Some(given) = mxObject.filter(|given| !given.is_none()) else {
            return Ok(NO_STAFF_ASSIGNED);
        };
        if let Ok(number) = given.extract::<i32>() {
            return Ok(number);
        }
        step::read_staff(&written(given)?).map_err(import_error)
    }

    /// music21's `addToStaffReference`: `m21Object` listed under the staff
    /// the element names.
    fn addToStaffReference(
        slf: &Bound<'_, Self>,
        mxObjectOrNumber: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let staff = Self::getStaffNumber(Some(mxObjectOrNumber))?;
        let reference = slf.getattr("staffReference")?;
        let listed = reference.call_method1("setdefault", (staff, PyList::empty(py)))?;
        listed.call_method1("append", (m21Object,))?;
        Ok(())
    }

    /// music21's `insertCoreAndRef`: the object put in the measure at
    /// `offset`, and listed under its staff.
    fn insertCoreAndRef(
        slf: &Bound<'_, Self>,
        offset: &Bound<'_, PyAny>,
        mxObjectOrNumber: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        slf.getattr("stream")?
            .call_method1("insert", (offset, m21Object))?;
        Self::addToStaffReference(slf, mxObjectOrNumber, m21Object)
    }

    /// music21's `parse`: the whole `<measure>` read into `stream`.
    fn parse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let measure = slf.getattr("mxMeasure")?;
        let read = Self::in_context(slf, &measure)?;
        slf.setattr("stream", &read)?;
        Self::parseMeasureNumbers(slf, None)?;

        // The divisions set inside the measure hold for the next one.
        for attributes in measure
            .call_method1("findall", ("attributes",))?
            .try_iter()?
        {
            let divisions = attributes?.call_method1("find", ("divisions",))?;
            let text = strippedText(Some(divisions));
            if let Ok(value) = text.parse::<f64>() {
                slf.setattr("divisions", value)?;
            }
        }

        // Counted as music21 counts them: a chord once, and a lone rest
        // fills the measure.
        let counts = slf.getattr("restAndNoteCount")?;
        let mut full = false;
        for note in measure.call_method1("findall", ("note",))?.try_iter()? {
            let note = note?;
            if !note.call_method1("find", (CHORD,))?.is_none() {
                continue;
            }
            let rest = note.call_method1("find", ("rest",))?;
            let kind = if rest.is_none() { "note" } else { "rest" };
            if !rest.is_none()
                && rest
                    .call_method1("get", ("measure",))?
                    .extract::<Option<String>>()?
                    .as_deref()
                    == Some("yes")
            {
                full = true;
            }
            let count: i64 = counts.get_item(kind)?.extract()?;
            counts.set_item(kind, count + 1)?;
        }
        let rests: i64 = counts.get_item("rest")?.extract()?;
        let notes: i64 = counts.get_item("note")?.extract()?;
        if full || (rests == 1 && notes == 0) {
            slf.setattr("fullMeasureRest", true)?;
        }
        Ok(())
    }

    /// music21's `parseMeasureNumbers`: the measure's number and suffix,
    /// from its `number` attribute or the text given.
    #[pyo3(signature = (mNumRaw = None))]
    fn parseMeasureNumbers(slf: &Bound<'_, Self>, mNumRaw: Option<String>) -> PyResult<()> {
        let measure = slf.getattr("stream")?;
        let raw = match mNumRaw {
            Some(given) => Some(given),
            None => {
                let written = slf.getattr("mxMeasure")?;
                match written.is_none() {
                    true => None,
                    false => written.call_method1("get", ("number",))?.extract()?,
                }
            }
        };
        if let Some(raw) = raw {
            let (number, suffix) =
                music21::common::stringtools::get_num_from_str(&raw, "0123456789");
            if let Ok(number) = number.parse::<i64>() {
                measure.setattr("number", number)?;
            }
            if !suffix.is_empty() {
                measure.setattr("numberSuffix", suffix)?;
            }
        }
        slf.setattr("measureNumber", measure.getattr("number")?)?;
        slf.setattr("numberSuffix", measure.getattr("numberSuffix")?)
    }

    /// music21's `xmlBackup`: the position in the measure moved back.
    fn xmlBackup(slf: &Bound<'_, Self>, mxObj: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::moved(slf, mxObj, -1.0)
    }

    /// music21's `xmlForward`: the position in the measure moved on.
    fn xmlForward(slf: &Bound<'_, Self>, mxObj: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::moved(slf, mxObj, 1.0)
    }

    /// music21's `xmlToOffset`: an element's `<offset>` in quarters.
    fn xmlToOffset(slf: &Bound<'_, Self>, mxObj: &Bound<'_, PyAny>) -> PyResult<f64> {
        let offset = mxObj.call_method1("find", ("offset",))?;
        if offset.is_none() {
            return Ok(0.0);
        }
        quarters(slf.as_any(), &offset)
    }

    /// music21's `xmlToNote`: a `<note>` read and put in the measure where
    /// the reading stands, which then moves past it.
    fn xmlToNote(slf: &Bound<'_, Self>, mxNote: &Bound<'_, PyAny>) -> PyResult<()> {
        let measure = read_measure(slf.as_any(), &in_measure(&[written(mxNote)?]))?;
        let start: f64 = slf.getattr("offsetMeasureNote")?.extract()?;
        let stream = slf.getattr("stream")?;
        let mut last = None;
        for element in of_class(&measure, "GeneralNote")? {
            let offset: f64 = element.getattr("offset")?.extract()?;
            stream.call_method1("insert", (start + offset, &element))?;
            Self::addToStaffReference(slf, mxNote, &element)?;
            last = Some(element);
        }

        let kind = if mxNote.call_method1("find", ("rest",))?.is_none() {
            "note"
        } else {
            "rest"
        };
        let counts = slf.getattr("restAndNoteCount")?;
        let count: i64 = counts.get_item(kind)?.extract()?;
        counts.set_item(kind, count + 1)?;
        let Some(last) = last else {
            return Ok(());
        };
        let length: f64 = measure.getattr("highestTime")?.extract()?;
        slf.setattr("offsetMeasureNote", start + length)?;
        slf.setattr("nLast", last)
    }

    /// music21's `xmlToSimpleNote`: a `<note>` as a lone note.
    #[pyo3(signature = (mxNote, freeSpanners = true))]
    fn xmlToSimpleNote<'py>(
        slf: &Bound<'py, Self>,
        mxNote: &Bound<'py, PyAny>,
        freeSpanners: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = freeSpanners;
        let measure = read_measure(slf.as_any(), &in_measure(&[without(mxNote, &[CHORD])?]))?;
        first_of(&measure, "GeneralNote")
    }

    /// music21's `xmlToChord`: `<note>`s sharing a stem, as a chord.
    fn xmlToChord<'py>(
        slf: &Bound<'py, Self>,
        mxNoteList: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let mut texts = Vec::new();
        for (index, note) in mxNoteList.try_iter()?.enumerate() {
            let note = note?;
            if index == 0 {
                texts.push(without(&note, &[CHORD])?);
                continue;
            }
            if !note.call_method1("find", (CHORD,))?.is_none() {
                texts.push(written(&note)?);
                continue;
            }
            // Only the notes after the first carry `<chord/>`.
            let copied = py.import("copy")?.call_method1("deepcopy", (&note,))?;
            let chord = py
                .import("xml.etree.ElementTree")?
                .getattr("Element")?
                .call1((CHORD,))?;
            copied.call_method1("insert", (0, chord))?;
            texts.push(written(&copied)?);
        }
        let measure = read_measure(slf.as_any(), &in_measure(&texts))?;
        first_of(&measure, "ChordBase")
    }

    /// music21's `xmlToRest`: a `<note>` holding `<rest>`, as a rest.
    fn xmlToRest<'py>(
        slf: &Bound<'py, Self>,
        mxRest: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let rest = mxRest.call_method1("find", ("rest",))?;
        if !rest.is_none()
            && rest
                .call_method1("get", ("measure",))?
                .extract::<Option<String>>()?
                .as_deref()
                == Some("yes")
        {
            slf.setattr("fullMeasureRest", true)?;
        }
        // The rest read under its staff's last clef, which places it.
        let staff = Self::getStaffNumber(Some(mxRest))?.max(1);
        let clef = slf.getattr("lastClefs")?.call_method1("get", (staff,))?;
        let mut texts = vec![clef_written(&clef)?];
        texts.push(without(mxRest, &[CHORD, "staff"])?);
        let measure = read_measure(slf.as_any(), &in_measure(&texts))?;
        first_of(&measure, "Rest")
    }

    /// music21's `xmlToDuration`: a `<note>`'s duration, also given to
    /// `inputM21` where there is one.
    #[pyo3(signature = (mxNote, inputM21 = None))]
    fn xmlToDuration<'py>(
        slf: &Bound<'py, Self>,
        mxNote: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let measure = read_measure(slf.as_any(), &in_measure(&[without(mxNote, &[CHORD])?]))?;
        let note = first_of(&measure, "GeneralNote")?;
        let duration = note.getattr("duration")?;
        let Some(given) = inputM21.filter(|given| !given.is_none()) else {
            return Ok(duration);
        };
        given.setattr("quarterLength", duration.getattr("quarterLength")?)?;
        Ok(given.clone())
    }

    /// music21's `xmlToTuplets`: the tuplets a `<note>`'s duration is in.
    fn xmlToTuplets<'py>(
        slf: &Bound<'py, Self>,
        mxNote: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let duration = Self::xmlToDuration(slf, mxNote, None)?;
        let tuplets = duration.getattr("tuplets")?;
        Ok(PyList::new(slf.py(), tuplets.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
    }

    /// music21's `xmlToPitch`: a `<note>`'s pitch.
    #[pyo3(signature = (mxNote, inputM21 = None))]
    fn xmlToPitch(
        slf: &Bound<'_, Self>,
        mxNote: &Bound<'_, PyAny>,
        inputM21: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let _ = inputM21;
        let read = step::read_pitch(&written(mxNote)?).map_err(import_error)?;
        Ok(crate::installed_new(
            py,
            "music21.pitch",
            "Pitch",
            crate::pitch::Pitch::wrap(read, false),
        )?
        .into_any())
    }

    /// music21's `xmlToAccidental`: an `<accidental>`.
    #[pyo3(signature = (mxAccidental, inputM21 = None))]
    fn xmlToAccidental(
        slf: &Bound<'_, Self>,
        mxAccidental: &Bound<'_, PyAny>,
        inputM21: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let _ = inputM21;
        let read = step::read_accidental(&written(mxAccidental)?).map_err(import_error)?;
        Ok(crate::installed_new(
            py,
            "music21.pitch",
            "Accidental",
            crate::pitch::Accidental::from_inner(read),
        )?
        .into_any())
    }

    /// music21's `xmlToTie`: a `<note>`'s tie, or `None`.
    fn xmlToTie(slf: &Bound<'_, Self>, mxNote: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let Some(read) = step::read_tie(&written(mxNote)?).map_err(import_error)? else {
            return Ok(py.None());
        };
        Ok(
            crate::installed_new(py, "music21.tie", "Tie", crate::notation::Tie::wrap(read))?
                .into_any(),
        )
    }

    /// music21's `xmlToBeams`: `<beam>`s as `Beams`.
    #[pyo3(signature = (mxBeamList, inputM21 = None))]
    fn xmlToBeams(
        slf: &Bound<'_, Self>,
        mxBeamList: &Bound<'_, PyAny>,
        inputM21: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let _ = inputM21;
        let mut texts = Vec::new();
        for beam in mxBeamList.try_iter()? {
            texts.push(written(&beam?)?);
        }
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let read = step::read_beams(&texts).map_err(import_error)?;
        Ok(crate::installed_new(
            py,
            "music21.beam",
            "Beams",
            crate::notation::Beams::wrap(read),
        )?
        .into_any())
    }

    /// music21's `xmlToBeam`: one `<beam>`.
    #[pyo3(signature = (mxBeam, inputM21 = None))]
    fn xmlToBeam<'py>(
        slf: &Bound<'py, Self>,
        mxBeam: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = inputM21;
        let list = PyList::new(slf.py(), [mxBeam])?;
        let beams = Self::xmlToBeams(slf, list.as_any(), None)?;
        beams.bind(slf.py()).getattr("beamsList")?.get_item(0)
    }

    /// music21's `xmlToLyric`: a `<lyric>`, or `None` for one with nothing.
    #[pyo3(signature = (mxLyric, inputM21 = None))]
    fn xmlToLyric(
        slf: &Bound<'_, Self>,
        mxLyric: &Bound<'_, PyAny>,
        inputM21: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let _ = inputM21;
        let Some(read) = step::read_lyric(&written(mxLyric)?).map_err(import_error)? else {
            return Ok(py.None());
        };
        Ok(crate::installed_new(
            py,
            "music21.note",
            "Lyric",
            crate::notation::Lyric::wrap(read),
        )?
        .into_any())
    }

    /// music21's `xmlToArticulation`: one mark of `<articulations>`.
    fn xmlToArticulation(slf: &Bound<'_, Self>, mxObj: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        match step::read_articulation(&written(mxObj)?).map_err(import_error)? {
            Some(read) => crate::articulations::Articulation::object(py, read),
            None => Ok(py.None()),
        }
    }

    /// music21's `xmlTechnicalToArticulation`: one mark of `<technical>`.
    fn xmlTechnicalToArticulation(
        slf: &Bound<'_, Self>,
        mxObj: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        match step::read_technical(&written(mxObj)?).map_err(import_error)? {
            Some(read) => crate::articulations::Articulation::object(py, read),
            None => Ok(py.None()),
        }
    }

    /// music21's `xmlToClef`: a `<clef>`.
    fn xmlToClef(slf: &Bound<'_, Self>, mxClef: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let read = step::read_clef(&written(mxClef)?).map_err(import_error)?;
        crate::clef::Clef::object(slf.py(), read)
    }

    /// music21's `handleClef`: a `<clef>` put in the measure and kept as
    /// its staff's last.
    fn handleClef<'py>(
        slf: &Bound<'py, Self>,
        mxClef: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let clef = Self::xmlToClef(slf, mxClef)?.into_bound(py);
        Self::insert_here(slf, mxClef, &clef)?;
        let staff = Self::getStaffNumber(Some(mxClef))?;
        slf.getattr("lastClefs")?.set_item(staff, &clef)?;
        Ok(clef)
    }

    /// music21's `xmlToKeySignature`: a `<key>`.
    fn xmlToKeySignature<'py>(
        slf: &Bound<'py, Self>,
        mxKey: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let read = step::read_key(&written(mxKey)?).map_err(import_error)?;
        object_of(slf.py(), &read)
    }

    /// music21's `handleKeySignature`: a `<key>` put in the measure.
    fn handleKeySignature(slf: &Bound<'_, Self>, mxKey: &Bound<'_, PyAny>) -> PyResult<()> {
        let key = Self::xmlToKeySignature(slf, mxKey)?;
        Self::insert_here(slf, mxKey, &key)
    }

    /// music21's `xmlToTimeSignature`: a `<time>`.
    fn xmlToTimeSignature<'py>(
        slf: &Bound<'py, Self>,
        mxTime: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        match step::read_time(&written(mxTime)?).map_err(import_error)? {
            Some(read) => object_of(py, &music21::StreamElement::TimeSignature(read)),
            None => Ok(py.None().into_bound(py)),
        }
    }

    /// music21's `handleTimeSignature`: a `<time>` put in the measure and
    /// kept as the part's last.
    fn handleTimeSignature(slf: &Bound<'_, Self>, mxTime: &Bound<'_, PyAny>) -> PyResult<()> {
        let meter = Self::xmlToTimeSignature(slf, mxTime)?;
        if meter.is_none() {
            return Ok(());
        }
        Self::insert_here(slf, mxTime, &meter)?;
        slf.getattr("parent")?.setattr("lastTimeSignature", meter)
    }

    /// music21's `xmlTransposeToInterval`: a `<transpose>`.
    fn xmlTransposeToInterval(
        slf: &Bound<'_, Self>,
        mxTranspose: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let read = step::read_transpose(&written(mxTranspose)?).map_err(import_error)?;
        crate::interval::interval_object(slf.py(), read)
    }

    /// music21's `xmlToTempoIndication`: a `<metronome>`.
    #[pyo3(signature = (mxMetronome, mxWords = None))]
    fn xmlToTempoIndication<'py>(
        slf: &Bound<'py, Self>,
        mxMetronome: &Bound<'py, PyAny>,
        mxWords: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = mxWords;
        let read = step::read_metronome(&written(mxMetronome)?).map_err(import_error)?;
        object_of(slf.py(), &read)
    }

    /// music21's `xmlToChordSymbol`: a `<harmony>`.
    fn xmlToChordSymbol<'py>(
        slf: &Bound<'py, Self>,
        mxHarmony: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let measure = read_measure(slf.as_any(), &in_measure(&[written(mxHarmony)?]))?;
        first_of(&measure, "Harmony")
    }

    /// music21's `xmlHarmony`: a `<harmony>` put in the measure.
    fn xmlHarmony(slf: &Bound<'_, Self>, mxHarmony: &Bound<'_, PyAny>) -> PyResult<()> {
        let symbol = Self::xmlToChordSymbol(slf, mxHarmony)?;
        if symbol.is_none() {
            return Ok(());
        }
        Self::insert_here(slf, mxHarmony, &symbol)
    }

    /// music21's `xmlToTextExpression`: a `<words>`.
    fn xmlToTextExpression<'py>(
        slf: &Bound<'py, Self>,
        mxWords: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let direction = format!(
            "<direction><direction-type>{}</direction-type></direction>",
            written(mxWords)?
        );
        let measure = read_measure(slf.as_any(), &in_measure(&[direction]))?;
        first_of(&measure, "TextExpression")
    }

    /// music21's `xmlToBarline`: a `<barline>`, as the barline it draws.
    #[pyo3(signature = (mxBarline, inputM21 = None))]
    fn xmlToBarline<'py>(
        slf: &Bound<'py, Self>,
        mxBarline: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = inputM21;
        Self::barline_of(slf, mxBarline)
    }

    /// music21's `xmlToRepeat`: a `<barline>` holding `<repeat>`.
    #[pyo3(signature = (mxBarline, inputM21 = None))]
    fn xmlToRepeat<'py>(
        slf: &Bound<'py, Self>,
        mxBarline: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = inputM21;
        if mxBarline.call_method1("find", ("repeat",))?.is_none() {
            let barException = music21_name(slf.py(), "music21.bar", "BarException")?;
            return Err(PyErr::from_value(barException.call1((
                "attempting to create a Repeat from an MusicXML bar that does not define a repeat",
            ))?));
        }
        Self::barline_of(slf, mxBarline)
    }
}

impl PartParser {
    /// A crate error as music21's `parse` raises it: prefixed with the
    /// part's name and the measure it was met in.
    fn placed(slf: &Bound<'_, Self>, error: &music21::Error) -> PyErr {
        let message = crate::pitch::message(error);
        let measure = error
            .position()
            .and_then(|position| position.measure.clone())
            .filter(|measure| !measure.is_empty())
            .unwrap_or_else(|| "0".to_string());
        let name = Self::part_name(slf).unwrap_or_default();
        MusicXMLImportException::new_err(format!(
            "In part ({name}), measure ({measure}): {message}"
        ))
    }

    /// The `<part-name>` of the score part, else the part's id.
    fn part_name(slf: &Bound<'_, Self>) -> PyResult<String> {
        let score_part = slf.getattr("mxScorePart")?;
        if !score_part.is_none() {
            let name = strippedText(Some(score_part.call_method1("find", ("part-name",))?));
            if !name.is_empty() {
                return Ok(name);
            }
        }
        slf.getattr("partId")?.str()?.extract()
    }
}

impl MeasureParser {
    /// A `<measure>` read under the part's last time signature, which
    /// decides how long a rest filling it is, where it sets none itself.
    fn in_context<'py>(
        slf: &Bound<'py, Self>,
        measure: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let meter = slf.getattr("parent")?.getattr("lastTimeSignature")?;
        let own = !measure.call_method1("find", (".//time",))?.is_none();
        if meter.is_none() || own {
            return read_measure(slf.as_any(), &written(measure)?);
        }

        // The meter goes in first, and comes out again: music21 keeps it
        // on the part, not in this measure.
        let numerator: i64 = meter.getattr("numerator")?.extract()?;
        let denominator: i64 = meter.getattr("denominator")?.extract()?;
        let attributes = format!(
            "<attributes><time><beats>{numerator}</beats>\
             <beat-type>{denominator}</beat-type></time></attributes>"
        );
        let copied = py.import("copy")?.call_method1("deepcopy", (measure,))?;
        let tree = py.import("xml.etree.ElementTree")?;
        copied.call_method1(
            "insert",
            (0, tree.call_method1("fromstring", (attributes,))?),
        )?;
        let read = read_measure(slf.as_any(), &written(&copied)?)?;
        for added in of_class(&read, "TimeSignature")? {
            read.call_method1("remove", (added,))?;
        }
        Ok(read)
    }

    /// The reading position moved by a `<backup>` or `<forward>`.
    fn moved(slf: &Bound<'_, Self>, element: &Bound<'_, PyAny>, sign: f64) -> PyResult<()> {
        let duration = element.call_method1("find", ("duration",))?;
        if duration.is_none() {
            return Ok(());
        }
        let change = quarters(slf.as_any(), &duration)?;
        let now: f64 = slf.getattr("offsetMeasureNote")?.extract()?;
        slf.setattr("offsetMeasureNote", now + sign * change)
    }

    /// An object put in the measure where the reading stands, under the
    /// staff its element names.
    fn insert_here(
        slf: &Bound<'_, Self>,
        element: &Bound<'_, PyAny>,
        object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let offset = slf.getattr("offsetMeasureNote")?;
        Self::insertCoreAndRef(slf, &offset, element, object)?;
        Ok(())
    }

    /// A `<barline>` read at the end of a measure of its own, the end it
    /// says.
    fn barline_of<'py>(
        slf: &Bound<'py, Self>,
        barline: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let location: Option<String> = barline.call_method1("get", ("location",))?.extract()?;
        let measure = read_measure(slf.as_any(), &in_measure(&[written(barline)?]))?;
        let end = match location.as_deref() {
            Some("left") => "leftBarline",
            _ => "rightBarline",
        };
        measure.getattr(end)
    }
}

/// The module music21 calls `musicxml.xmlToM21`, as music21 has it.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<XMLParserBase>()?;
        m.add_class::<MusicXMLImporter>()?;
        m.add_class::<PartParser>()?;
        m.add_class::<MeasureParser>()?;
        for (name, exception) in [
            (
                "MusicXMLImportException",
                py.get_type::<MusicXMLImportException>(),
            ),
            ("MusicXMLWarning", py.get_type::<MusicXMLWarning>()),
        ] {
            exception.setattr("__module__", "music21.musicxml.xmlObjects")?;
            m.add(name, exception)?;
        }
        m.add_function(wrap_pyfunction!(clean, &m)?)?;
        m.add_function(wrap_pyfunction!(strippedText, &m)?)?;
        m.add_function(wrap_pyfunction!(musicXMLTypeToType, &m)?)?;
        m.add_function(wrap_pyfunction!(float_or_int, &m)?)?;
        m.add("NO_STAFF_ASSIGNED", NO_STAFF_ASSIGNED)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
