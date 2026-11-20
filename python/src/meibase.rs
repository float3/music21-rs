//! music21's `mei` package: `MeiToM21Converter` and the per-element steps
//! of `mei.base`, each under music21's name.
//!
//! The steps read ElementTree elements and make this package's objects, as
//! music21's do, and call one another through the module as imported: a
//! caller that replaces `noteFromElement` or `music21.note.Note` on its
//! module, as music21's tests do, has that replacement called by every step
//! that would call it. What an attribute value stands for is decided by the
//! crate's `mei::step`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use music21::mei::step;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyCFunction, PyDict, PyInt, PyList, PyModule, PyString, PyTuple};

/// The module music21 keeps these in.
const MODULE: &str = "music21.mei.base";

/// The package around it.
const PACKAGE: &str = "music21.mei";

/// MEI's namespace, as ElementTree writes it before a tag.
const MEI_NS: &str = "{http://www.music-encoding.org/ns/mei}";

/// The `xml:id` attribute, as ElementTree names it.
const XMLID: &str = "{http://www.w3.org/XML/1998/namespace}id";

/// The modules of music21's `mei.base` imports, and that its steps reach
/// through it.
const IMPORTS: [&str; 18] = [
    "articulations",
    "bar",
    "chord",
    "clef",
    "duration",
    "environment",
    "exceptions21",
    "expressions",
    "instrument",
    "interval",
    "key",
    "metadata",
    "meter",
    "note",
    "pitch",
    "stream",
    "spanner",
    "tie",
];

/// The tags music21 passes over unread without saying so.
const IGNORE_UNPROCESSED: [&str; 8] = [
    "sb",
    "lb",
    "pb",
    "slur",
    "tie",
    "tupletSpan",
    "beamSpan",
    "instrDef",
];

/// music21's messages, by the names it keeps them under.
const TEST_FAILS: &str =
    "MEI module had {} failures and {} errors; run music21/mei/base.py to find out more.";
const INVALID_XML_DOC: &str = "MEI document is not valid XML.";
const WRONG_ROOT_ELEMENT: &str = "Root element should be <mei> in the MEI namespace, not <{}>.";
const UNKNOWN_TAG: &str = "Found unexpected tag while parsing MEI: <{}>.";
const UNEXPECTED_ATTR_VALUE: &str = "Unexpected value for \"{}\" attribute: {}";
const SEEMINGLY_NO_PARTS: &str = "There appear to be no <staffDef> tags in this score.";
const MISSING_VOICE_ID: &str = "Found a <layer> without @n attribute and no override.";
const CANNOT_FIND_XMLID: &str = "Could not find the @{} so we could not create the {}.";
const MISSING_TUPLET_DATA: &str =
    "Both @num and @numbase attributes are required on <tuplet> tags.";
const UNIMPLEMENTED_IMPORT: &str = "Importing {} without {} is not yet supported.";
const UNPROCESSED_SUBELEMENT: &str = "Found an unprocessed <{}> element in a <{}>.";
const MISSED_DATE: &str = "Unable to decipher the composition date \"{}\"";
const BAD_VERSE_NUMBER: &str = "Verse number must be an int (got \"{}\")";

/// The keys of what `scoreDefFromElement` returns that are no part's.
const ALL_PARTS: &str = "all-part objects";
const WHOLE_SCORE: &str = "whole-score objects";

/// Where `_makeBarlines` leaves the left barline of the measure after.
const NEXT_LEFT: &str = "next @left";

/// The duration type number past which a note takes beams.
const QUARTER_NUMBER: i32 = 4;

pyo3::create_exception!(music21_rs_facade, MeiValidityError, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, MeiValueError, crate::Music21Exception);
pyo3::create_exception!(
    music21_rs_facade,
    MeiAttributeError,
    crate::Music21Exception
);
pyo3::create_exception!(music21_rs_facade, MeiElementError, crate::Music21Exception);

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static BUILT_PACKAGE: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

// ------------------------------------------------------------ reaching out

/// The module as imported, which is what a caller replaces names on, or
/// the one built here where none is.
fn here(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let imported = py
        .import("sys")?
        .getattr("modules")?
        .call_method1("get", (MODULE,))?;
    if !imported.is_none() {
        return Ok(imported);
    }
    Ok(module(py)?.into_any())
}

/// One of this module's names, as it stands now.
fn named<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    here(py)?.getattr(name)
}

/// One of music21's modules, as this module holds it.
fn m21<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let here = here(py)?;
    if let Some(held) = here
        .getattr("__dict__")?
        .cast_into::<PyDict>()?
        .get_item(name)?
    {
        return Ok(held);
    }
    let imported = py.import(format!("music21.{name}"))?.into_any();
    here.setattr(name, &imported)?;
    Ok(imported)
}

/// `music21.<module>.<name>`, as it stands now.
fn class<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    m21(py, module)?.getattr(name)
}

/// `environLocal.warn(message)`.
fn warn(py: Python<'_>, message: String) -> PyResult<()> {
    named(py, "environLocal")?.call_method1("warn", (message,))?;
    Ok(())
}

/// `environLocal.printDebug(message)`.
fn debug(py: Python<'_>, message: String) -> PyResult<()> {
    named(py, "environLocal")?.call_method1("printDebug", (message,))?;
    Ok(())
}

/// A message of music21's with its `{}` filled in order.
fn filled(template: &str, values: &[String]) -> String {
    let mut out = String::new();
    let mut rest = template;
    for value in values {
        let Some((before, after)) = rest.split_once("{}") else {
            break;
        };
        out.push_str(before);
        out.push_str(value);
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A value as Python's `str()` writes it.
fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string())
}

/// `elem.get(name)`.
fn get<'py>(elem: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    elem.call_method1("get", (name,))
}

/// `elem.get(name, default)`.
fn get_or<'py>(
    elem: &Bound<'py, PyAny>,
    name: &str,
    default: impl IntoPyObject<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    elem.call_method1("get", (name, default))
}

/// `elem.get(name) is not None`.
fn has(elem: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    Ok(!get(elem, name)?.is_none())
}

/// An attribute read as text, none where it is missing.
fn get_text(elem: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    let value = get(elem, name)?;
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// Python's `int(value)`.
fn int<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    value.py().get_type::<PyInt>().call1((value,))
}

/// `copy.deepcopy(value)`.
fn deepcopy<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    value
        .py()
        .import("copy")?
        .getattr("deepcopy")?
        .call1((value,))
}

/// `xml.etree.ElementTree.Element(tag, **attributes)`.
fn element<'py>(
    py: Python<'py>,
    tag: &str,
    attributes: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    crate::musicxml::element_tree(py)?
        .getattr("Element")?
        .call((tag,), Some(attributes))
}

/// A tag in MEI's namespace.
fn tag(name: &str) -> String {
    format!("{MEI_NS}{name}")
}

/// Keyword arguments from pairs.
fn keywords<'py>(
    py: Python<'py>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    for (key, value) in pairs {
        dict.set_item(key, value)?;
    }
    Ok(dict)
}

/// A crate refusal as music21's `MeiValueError`.
fn value_error(error: &music21::Error) -> PyErr {
    MeiValueError::new_err(crate::pitch::message(error))
}

/// `isinstance(value, (note.Note, note.Rest, chord.Chord))`.
fn has_duration(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = value.py();
    let classes = PyTuple::new(
        py,
        [
            class(py, "note", "Note")?,
            class(py, "note", "Rest")?,
            class(py, "chord", "Chord")?,
        ],
    )?;
    value.is_instance(&classes)
}

/// The tag-to-step mapping a step hands `_processEmbeddedElements`, each
/// step looked up as it stands now.
fn steps<'py>(py: Python<'py>, pairs: &[(&str, &str)]) -> PyResult<Bound<'py, PyDict>> {
    let mapping = PyDict::new(py);
    for (element, step) in pairs {
        mapping.set_item(tag(element), named(py, step)?)?;
    }
    Ok(mapping)
}

/// `_processEmbeddedElements(elem.findall('*'), mapping, elem.tag, slurBundle)`.
fn embedded<'py>(
    elem: &Bound<'py, PyAny>,
    mapping: &Bound<'py, PyDict>,
    slurBundle: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    named(py, "_processEmbeddedElements")?.call1((
        elem.call_method1("findall", ("*",))?,
        mapping,
        elem.getattr("tag")?,
        slurBundle,
    ))
}

/// `makeDuration(_qlDurationFromAttr(elem.get('dur')), int(elem.get('dots', 0)))`.
fn written_duration<'py>(elem: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let length = named(py, "_qlDurationFromAttr")?.call1((get(elem, "dur")?,))?;
    let dots = int(&get_or(elem, "dots", 0)?)?;
    named(py, "makeDuration")?.call1((length, dots))
}

/// Beams filled on a note or chord whose `m21Beam` names one, where its
/// duration takes beams.
fn fill_beams(elem: &Bound<'_, PyAny>, thing: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = elem.py();
    let beam = get(elem, "m21Beam")?;
    if beam.is_none() {
        return Ok(());
    }
    let kind = thing.getattr("duration")?.getattr("type")?;
    let number = class(py, "duration", "convertTypeToNumber")?.call1((&kind,))?;
    if number.gt(QUARTER_NUMBER)? {
        thing.getattr("beams")?.call_method1("fill", (kind, beam))?;
    }
    Ok(())
}

/// What a note and a chord share once made: slurs, `xml:id`, `artic`,
/// `tie`.
fn finish_event(
    elem: &Bound<'_, PyAny>,
    thing: &Bound<'_, PyAny>,
    slurBundle: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = elem.py();
    if !slurBundle.is_none() {
        named(py, "addSlurs")?.call1((elem, thing, slurBundle))?;
    }
    if has(elem, XMLID)? {
        thing.setattr("id", get(elem, XMLID)?)?;
    }
    if has(elem, "artic")? {
        let marks = named(py, "_makeArticList")?.call1((get(elem, "artic")?,))?;
        thing
            .getattr("articulations")?
            .call_method1("extend", (marks,))?;
    }
    if has(elem, "tie")? {
        let made = named(py, "_tieFromAttr")?.call1((get(elem, "tie")?,))?;
        thing.setattr("tie", made)?;
    }
    Ok(())
}

/// A grace note's duration put in place of its written one.
fn grace(elem: &Bound<'_, PyAny>, thing: &Bound<'_, PyAny>) -> PyResult<()> {
    if !has(elem, "grace")? {
        return Ok(());
    }
    let py = elem.py();
    let length = thing.getattr("duration")?.getattr("quarterLength")?;
    thing.setattr(
        "duration",
        class(py, "duration", "GraceDuration")?.call1((length,))?,
    )
}

/// The thing scaled into its tuplet where its element names one.
fn tupleted<'py>(
    elem: &Bound<'py, PyAny>,
    thing: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !has(elem, "m21TupletNum")? {
        return Ok(thing);
    }
    named(elem.py(), "scaleToTuplet")?.call1((thing, elem))
}

// ------------------------------------------------------------ the converter

/// music21's `MeiToM21Converter`: one MEI document, prepared and read.
#[pyclass(
    module = "music21.mei.base",
    name = "MeiToM21Converter",
    subclass,
    dict
)]
pub(crate) struct MeiToM21Converter {
    /// The document's root element.
    #[pyo3(get, set)]
    documentRoot: Py<PyAny>,
    /// Attributes to add to elements, by their `xml:id`.
    #[pyo3(get, set)]
    m21Attr: Py<PyAny>,
    /// The slurs made before the notes are read.
    #[pyo3(get, set)]
    slurBundle: Py<PyAny>,
}

#[pymethods]
impl MeiToM21Converter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.documentRoot)?;
        visit.call(&self.m21Attr)?;
        visit.call(&self.slurBundle)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (theDocument=None))]
    fn new(py: Python<'_>, theDocument: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        debug(py, "*** initializing MeiToM21Converter".to_owned())?;
        let tree = crate::musicxml::element_tree(py)?;
        let documentRoot = match theDocument {
            None => tree.getattr("Element")?.call1((tag("mei"),))?,
            Some(document) => Self::root_of(py, &tree, document)?,
        };
        let m21Attr = py
            .import("collections")?
            .getattr("defaultdict")?
            .call1((py.get_type::<PyDict>(),))?;
        let slurBundle = class(py, "spanner", "SpannerBundle")?.call0()?;
        Ok(Self {
            documentRoot: documentRoot.unbind(),
            m21Attr: m21Attr.unbind(),
            slurBundle: slurBundle.unbind(),
        })
    }

    /// Reads the document into a stream: the slurs, ties, beams and tuplets
    /// written apart first, then the score, then its metadata.
    fn run<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        debug(py, "*** pre-processing spanning elements".to_owned())?;
        for step in [
            "_ppSlurs",
            "_ppTies",
            "_ppBeams",
            "_ppTuplets",
            "_ppConclude",
        ] {
            named(py, step)?.call1((slf,))?;
        }

        debug(py, "*** processing <score> elements".to_owned())?;
        let root = slf.getattr("documentRoot")?;
        let query = format!(".//{MEI_NS}music//{MEI_NS}score");
        let score = named(py, "scoreFromElement")?.call1((
            root.call_method1("find", (query,))?,
            slf.getattr("slurBundle")?,
        ))?;

        debug(py, "*** preparing metadata".to_owned())?;
        score.setattr("metadata", named(py, "makeMetadata")?.call1((root,))?)?;
        Ok(score)
    }
}

impl MeiToM21Converter {
    /// The root of a document's text, refused where it is no XML or no MEI.
    fn root_of<'py>(
        py: Python<'py>,
        tree: &Bound<'py, PyModule>,
        document: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let root = match tree.getattr("fromstring")?.call1((document,)) {
            Ok(root) => root,
            Err(error) if error.is_instance(py, &tree.getattr("ParseError")?) => {
                debug(
                    py,
                    "\n\nERROR: Parsing the MEI document with ElementTree failed.".to_owned(),
                )?;
                debug(
                    py,
                    format!("We got the following error:\n{}", error.value(py)),
                )?;
                return Err(MeiValidityError::new_err(INVALID_XML_DOC));
            }
            Err(error) => return Err(error),
        };
        let found = text(&root.getattr("tag")?)?;
        if found != tag("mei") {
            return Err(MeiElementError::new_err(filled(
                WRONG_ROOT_ELEMENT,
                &[found],
            )));
        }
        Ok(root)
    }
}

// ------------------------------------------------------------ utilities

/// music21's `safePitch`: a pitch, a default one for an empty name.
#[pyfunction]
#[pyo3(signature = (name, accidental=None, octave=None))]
fn safePitch<'py>(
    py: Python<'py>,
    name: &Bound<'py, PyAny>,
    accidental: Option<Bound<'py, PyAny>>,
    octave: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let pitch = class(py, "pitch", "Pitch")?;
    if !name.is_truthy()? {
        return pitch.call0();
    }
    let octave = octave.filter(|octave| octave.is_truthy().unwrap_or(false));
    let accidental = accidental.filter(|accidental| !accidental.is_none());
    let mut pairs = Vec::new();
    if let Some(octave) = octave {
        pairs.push(("octave", int(&octave)?));
    }
    if let Some(accidental) = accidental {
        pairs.push(("accidental", accidental));
    }
    pitch.call((name,), Some(&keywords(py, &pairs)?))
}

/// music21's `makeDuration`: a duration of `base` with `dots` dots.
#[pyfunction]
#[pyo3(signature = (base=None, dots=None))]
fn makeDuration<'py>(
    py: Python<'py>,
    base: Option<Bound<'py, PyAny>>,
    dots: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let base = match base {
        Some(base) => base,
        None => 0.0_f64.into_pyobject(py)?.into_any(),
    };
    let made = class(py, "duration", "Duration")?.call1((base,))?;
    let dots = match dots {
        Some(dots) => dots,
        None => 0_i32.into_pyobject(py)?.into_any(),
    };
    made.setattr("dots", dots)?;
    Ok(made)
}

/// music21's `allPartsPresent`: the `n` of every `<staffDef>`, once each.
#[pyfunction]
fn allPartsPresent<'py>(scoreElem: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py = scoreElem.py();
    let parts = PyList::empty(py);
    let query = format!(".//{MEI_NS}staffDef");
    for staffDef in scoreElem.call_method1("findall", (query,))?.try_iter()? {
        let n = get(&staffDef?, "n")?;
        if !parts.contains(&n)? {
            parts.append(n)?;
        }
    }
    if parts.is_empty() {
        return Err(MeiValidityError::new_err(SEEMINGLY_NO_PARTS));
    }
    Ok(parts.to_tuple())
}

/// music21's `_attrTranslator`: what `attr` stands for in `mapping`.
#[pyfunction]
#[pyo3(name = "_attrTranslator")]
fn attr_translator<'py>(
    attr: &Bound<'py, PyAny>,
    name: &Bound<'py, PyAny>,
    mapping: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = attr.py();
    match mapping.get_item(attr) {
        Ok(found) => Ok(found),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyKeyError>(py) => Err(
            MeiValueError::new_err(filled(UNEXPECTED_ATTR_VALUE, &[text(name)?, text(attr)?])),
        ),
        Err(error) => Err(error),
    }
}

/// `_attrTranslator(attr, name, <the module's table>)`.
fn translated<'py>(
    attr: &Bound<'py, PyAny>,
    name: &str,
    table: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = attr.py();
    named(py, "_attrTranslator")?.call1((attr, name, named(py, table)?))
}

/// music21's `_accidentalFromAttr`.
#[pyfunction]
#[pyo3(name = "_accidentalFromAttr")]
fn accidental_from_attr<'py>(attr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    translated(attr, "accid", "_ACCID_ATTR_DICT")
}

/// music21's `_accidGesFromAttr`.
#[pyfunction]
#[pyo3(name = "_accidGesFromAttr")]
fn accid_ges_from_attr<'py>(attr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    translated(attr, "accid.ges", "_ACCID_GES_ATTR_DICT")
}

/// music21's `_qlDurationFromAttr`.
#[pyfunction]
#[pyo3(name = "_qlDurationFromAttr")]
fn ql_duration_from_attr<'py>(attr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    translated(attr, "dur", "_DUR_ATTR_DICT")
}

/// music21's `_articulationFromAttr`: a tuple of one or two marks.
#[pyfunction]
#[pyo3(name = "_articulationFromAttr")]
fn articulation_from_attr<'py>(attr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py = attr.py();
    if let Ok(written) = attr.extract::<String>()
        && let Some((_, pair)) = step::COMPOUND_ARTICULATIONS
            .iter()
            .find(|(name, _)| *name == written)
    {
        let marks = pair
            .iter()
            .map(|name| class(py, "articulations", name)?.call0())
            .collect::<PyResult<Vec<_>>>()?;
        return PyTuple::new(py, marks);
    }
    let made = translated(attr, "artic", "_ARTIC_ATTR_DICT")?.call0()?;
    PyTuple::new(py, [made])
}

/// music21's `_makeArticList`: the marks of every `artic` value.
#[pyfunction]
#[pyo3(name = "_makeArticList")]
fn make_artic_list<'py>(attr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = attr.py();
    let marks = PyList::empty(py);
    let each = named(py, "_articulationFromAttr")?;
    for name in attr.call_method1("split", (" ",))?.try_iter()? {
        for mark in each.call1((name?,))?.try_iter()? {
            marks.append(mark?)?;
        }
    }
    Ok(marks)
}

/// music21's `_getOctaveShift`: the octaves a clef is moved by.
#[pyfunction]
#[pyo3(name = "_getOctaveShift")]
fn get_octave_shift(dis: Option<String>, disPlace: Option<String>) -> PyResult<i32> {
    step::octave_shift(dis.as_deref(), disPlace.as_deref())
        .map_err(|_| pyo3::exceptions::PyKeyError::new_err(dis.unwrap_or_default()))
}

/// music21's `_sharpsFromAttr`: the sharps of a `key.sig`, flats negative.
#[pyfunction]
#[pyo3(name = "_sharpsFromAttr")]
fn sharps_from_attr(signature: &str) -> PyResult<i32> {
    step::sharps_of(signature)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(crate::pitch::message(&error)))
}

// ------------------------------------------------------------ preprocessing

/// Each element of a tag under the document's `<score>`.
fn spanning<'py>(converter: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let query = format!(".//{MEI_NS}music//{MEI_NS}score//{MEI_NS}{name}");
    converter
        .getattr("documentRoot")?
        .call_method1("iterfind", (query,))?
        .try_iter()?
        .collect()
}

/// `theConverter.m21Attr[removeOctothorpe(xmlid)][name] = value`.
fn mark<'py>(
    converter: &Bound<'py, PyAny>,
    xmlid: &Bound<'py, PyAny>,
    name: &str,
    value: impl IntoPyObject<'py>,
) -> PyResult<()> {
    let py = converter.py();
    let id = named(py, "removeOctothorpe")?.call1((xmlid,))?;
    converter
        .getattr("m21Attr")?
        .get_item(id)?
        .set_item(name, value)
}

/// Whether an element has both `startid` and `endid`.
fn has_ends(elem: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(has(elem, "startid")? && has(elem, "endid")?)
}

/// music21's `_ppSlurs`: a slur in the bundle for every `<slur>`, and its
/// ends marked on the notes it joins.
#[pyfunction]
#[pyo3(name = "_ppSlurs")]
fn pp_slurs(theConverter: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = theConverter.py();
    debug(py, "*** pre-processing slurs".to_owned())?;
    for (count, slur) in spanning(theConverter, "slur")?.into_iter().enumerate() {
        if !has_ends(&slur)? {
            warn(
                py,
                filled(
                    UNIMPLEMENTED_IMPORT,
                    &["<slur>".into(), "@startid and @endid".into()],
                ),
            )?;
            continue;
        }
        let id = step::fresh_uuid(count);
        let made = class(py, "spanner", "Slur")?.call0()?;
        made.setattr("idLocal", &id)?;
        theConverter
            .getattr("slurBundle")?
            .call_method1("append", (made,))?;
        mark(theConverter, &get(&slur, "startid")?, "m21SlurStart", &id)?;
        mark(theConverter, &get(&slur, "endid")?, "m21SlurEnd", &id)?;
    }
    Ok(())
}

/// music21's `_ppTies`: `tie` marked on the notes every `<tie>` joins.
#[pyfunction]
#[pyo3(name = "_ppTies")]
fn pp_ties(theConverter: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = theConverter.py();
    debug(py, "*** pre-processing ties".to_owned())?;
    for tie in spanning(theConverter, "tie")? {
        if !has_ends(&tie)? {
            warn(
                py,
                filled(
                    UNIMPLEMENTED_IMPORT,
                    &["<tie>".into(), "@startid and @endid".into()],
                ),
            )?;
            continue;
        }
        mark(theConverter, &get(&tie, "startid")?, "tie", "i")?;
        mark(theConverter, &get(&tie, "endid")?, "tie", "t")?;
    }
    Ok(())
}

/// music21's `_ppBeams`: `m21Beam` marked on the notes every `<beamSpan>`
/// joins.
#[pyfunction]
#[pyo3(name = "_ppBeams")]
fn pp_beams(theConverter: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = theConverter.py();
    debug(py, "*** pre-processing beams".to_owned())?;
    let attributes = theConverter.getattr("m21Attr")?;
    for beam in spanning(theConverter, "beamSpan")? {
        if !has_ends(&beam)? {
            warn(
                py,
                filled(
                    UNIMPLEMENTED_IMPORT,
                    &["<beamSpan>".into(), "@startid and @endid".into()],
                ),
            )?;
            continue;
        }
        mark(theConverter, &get(&beam, "startid")?, "m21Beam", "start")?;
        mark(theConverter, &get(&beam, "endid")?, "m21Beam", "stop")?;

        // The notes between, unless already marked as an end.
        let listed = get_or(&beam, "plist", "")?;
        for xmlid in listed.call_method1("split", (" ",))?.try_iter()? {
            let id = named(py, "removeOctothorpe")?.call1((xmlid?,))?;
            let held = attributes.get_item(&id)?;
            if !held.contains("m21Beam")? {
                held.set_item("m21Beam", "continue")?;
            }
        }
    }
    Ok(())
}

/// music21's `_ppTuplets`: `m21TupletNum` and `m21TupletNumbase` marked on
/// the notes every `<tupletSpan>` lists, or on its ends to be searched
/// between later.
#[pyfunction]
#[pyo3(name = "_ppTuplets")]
fn pp_tuplets(theConverter: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = theConverter.py();
    debug(py, "*** pre-processing tuplets".to_owned())?;
    for tuplet in spanning(theConverter, "tupletSpan")? {
        let num = get(&tuplet, "num")?;
        let numbase = get(&tuplet, "numbase")?;
        let listed = get(&tuplet, "plist")?;
        if !has_ends(&tuplet)? && listed.is_none() {
            warn(
                py,
                filled(
                    UNIMPLEMENTED_IMPORT,
                    &[
                        "<tupletSpan>".into(),
                        "@startid and @endid or @plist".into(),
                    ],
                ),
            )?;
            continue;
        }

        // Every note listed, where the span lists them.
        if !listed.is_none() {
            for xmlid in get_or(&tuplet, "plist", "")?
                .call_method1("split", (" ",))?
                .try_iter()?
            {
                let xmlid = xmlid?;
                let id = named(py, "removeOctothorpe")?.call1((&xmlid,))?;
                if !id.is_truthy()? {
                    continue;
                }
                mark(theConverter, &xmlid, "m21TupletNum", &num)?;
                mark(theConverter, &xmlid, "m21TupletNumbase", &numbase)?;
            }
            continue;
        }

        // Its ends, to be searched between once the layer is read.
        for (end, search) in [("startid", "start"), ("endid", "end")] {
            let xmlid = get(&tuplet, end)?;
            mark(theConverter, &xmlid, "m21TupletSearch", search)?;
            mark(theConverter, &xmlid, "m21TupletNum", &num)?;
            mark(theConverter, &xmlid, "m21TupletNumbase", &numbase)?;
        }
    }
    Ok(())
}

/// music21's `_ppConclude`: the marks of the steps before added to their
/// elements, after what each already has.
#[pyfunction]
#[pyo3(name = "_ppConclude")]
fn pp_conclude(theConverter: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = theConverter.py();
    debug(py, "*** concluding pre-processing".to_owned())?;
    let attributes = theConverter.getattr("m21Attr")?;
    let root = theConverter.getattr("documentRoot")?;
    for held in root.call_method1("iterfind", ("*//*",))?.try_iter()? {
        let held = held?;
        let id = get(&held, XMLID)?;
        if !attributes.contains(&id)? {
            continue;
        }
        let marks = attributes.get_item(&id)?;
        for name in marks.try_iter()? {
            let name = name?;
            let before = held.call_method1("get", (&name, ""))?;
            held.call_method1("set", (&name, before.add(marks.get_item(&name)?)?))?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------ helpers

/// music21's `_processEmbeddedElements`: each element made by the step
/// its tag maps to, what a step returns as a list or tuple spread out.
#[pyfunction]
#[pyo3(name = "_processEmbeddedElements", signature = (elements, mapping, callerTag=None, slurBundle=None))]
fn process_embedded<'py>(
    py: Python<'py>,
    elements: &Bound<'py, PyAny>,
    mapping: &Bound<'py, PyAny>,
    callerTag: Option<Bound<'py, PyAny>>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let processed = PyList::empty(py);
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let ignored = named(py, "_IGNORE_UNPROCESSED")?;
    for held in elements.try_iter()? {
        let held = held?;
        let found = held.getattr("tag")?;
        if mapping.contains(&found)? {
            let result = mapping.get_item(&found)?.call1((&held, &slurBundle))?;
            if result.is_instance_of::<PyTuple>() || result.is_instance_of::<PyList>() {
                for each in result.try_iter()? {
                    processed.append(each?)?;
                }
            } else {
                processed.append(result)?;
            }
            continue;
        }
        if !ignored.contains(&found)? {
            let caller = match &callerTag {
                Some(caller) => text(caller)?,
                None => "None".to_owned(),
            };
            debug(py, filled(UNPROCESSED_SUBELEMENT, &[text(&found)?, caller]))?;
        }
    }
    Ok(processed)
}

/// music21's `_timeSigFromAttrs`: the meter of `meter.count` and
/// `meter.unit`.
#[pyfunction]
#[pyo3(name = "_timeSigFromAttrs")]
fn time_sig_from_attrs<'py>(elem: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let ratio = format!(
        "{}/{}",
        text(&get(elem, "meter.count")?)?,
        text(&get(elem, "meter.unit")?)?
    );
    class(py, "meter", "TimeSignature")?.call1((ratio,))
}

/// music21's `_keySigFromAttrs`: a key named by its tonic, or a key
/// signature by its sharps or flats.
#[pyfunction]
#[pyo3(name = "_keySigFromAttrs")]
fn key_sig_from_attrs<'py>(elem: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let step = get(elem, "key.pname")?;
    if !step.is_none() {
        let mode = get_or(elem, "key.mode", "")?;
        let accidental = named(py, "_accidentalFromAttr")?.call1((get(elem, "key.accid")?,))?;
        let tonic = if accidental.is_none() {
            step
        } else {
            step.add(accidental)?
        };
        let pairs = [("tonic", tonic), ("mode", mode)];
        return class(py, "key", "Key")?.call((), Some(&keywords(py, &pairs)?));
    }
    let sharps = named(py, "_sharpsFromAttr")?.call1((get(elem, "key.sig")?,))?;
    let signature =
        class(py, "key", "KeySignature")?.call((), Some(&keywords(py, &[("sharps", sharps)])?))?;
    let mode = get_or(elem, "key.mode", "major")?;
    signature.call_method("asKey", (), Some(&keywords(py, &[("mode", mode)])?))
}

/// music21's `_transpositionFromAttrs`: the interval from written to
/// sounding pitch.
#[pyfunction]
#[pyo3(name = "_transpositionFromAttrs")]
fn transposition_from_attrs<'py>(elem: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let diatonic: i64 = int(&get_or(elem, "trans.diat", 0)?)?.extract()?;
    let semitones: i64 = int(&get_or(elem, "trans.semi", 0)?)?.extract()?;
    let (generic, chromatic) =
        step::transposition_steps(diatonic, semitones).map_err(|error| value_error(&error))?;
    let interval = m21(py, "interval")?;
    interval.getattr("intervalFromGenericAndChromatic")?.call1((
        interval.getattr("GenericInterval")?.call1((generic,))?,
        interval.getattr("ChromaticInterval")?.call1((chromatic,))?,
    ))
}

/// music21's `_barlineFromAttr`: a barline, a repeat, or for `rptboth`
/// the two repeats either side of the line.
#[pyfunction]
#[pyo3(name = "_barlineFromAttr")]
fn barline_from_attr<'py>(attr: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = attr.py();
    let written = text(attr)?;
    if !written.starts_with(step::REPEAT_PREFIX) {
        let style = translated(attr, "right", "_BAR_ATTR_DICT")?;
        return class(py, "bar", "Barline")?.call1((style,));
    }
    let repeat = class(py, "bar", "Repeat")?;
    match written.as_str() {
        "rptboth" => {
            let each = named(py, "_barlineFromAttr")?;
            Ok(
                PyTuple::new(py, [each.call1(("rptend",))?, each.call1(("rptstart",))?])?
                    .into_any(),
            )
        }
        "rptend" => {
            let times = keywords(py, &[("times", 2_i32.into_pyobject(py)?.into_any())])?;
            repeat.call(("end",), Some(&times))
        }
        _ => repeat.call1(("start",)),
    }
}

/// music21's `_tieFromAttr`: the tie a `tie` names.
#[pyfunction]
#[pyo3(name = "_tieFromAttr")]
fn tie_from_attr<'py>(py: Python<'py>, attr: &str) -> PyResult<Bound<'py, PyAny>> {
    class(py, "tie", "Tie")?.call1((step::tie_type(attr).as_str(),))
}

/// music21's `addSlurs`: the object put in the slurs its element starts,
/// ends or names; whether it was put in any.
#[pyfunction]
fn addSlurs(
    elem: &Bound<'_, PyAny>,
    obj: &Bound<'_, PyAny>,
    slurBundle: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let py = elem.py();

    // A slur found by its id, nothing where there is none.
    let into_slur = |id: &Bound<'_, PyAny>| -> PyResult<bool> {
        let found = slurBundle.call_method1("getByIdLocal", (id,));
        let first = found.and_then(|found| found.get_item(0));
        match first {
            Ok(slur) => {
                slur.call_method1("addSpannedElements", (obj,))?;
                Ok(true)
            }
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyIndexError>(py) => Ok(false),
            Err(error) => Err(error),
        }
    };

    let mut added = false;
    for end in ["m21SlurStart", "m21SlurEnd"] {
        if has(elem, end)? {
            added = into_slur(&get(elem, end)?)?;
        }
    }
    let Some(slurs) = get_text(elem, "slur")? else {
        return Ok(added);
    };
    for each in slurs.split(' ') {
        let marks: Vec<char> = each.chars().collect();
        let [number, kind] = marks[..] else {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "expected 2 values to unpack, got {}",
                marks.len()
            )));
        };
        let number = number.to_string();
        if kind == 'i' {
            let made = class(py, "spanner", "Slur")?.call0()?;
            made.setattr("idLocal", &number)?;
            slurBundle.call_method1("append", (&made,))?;
            made.call_method1("addSpannedElements", (obj,))?;
            added = true;
        } else if kind == 't' {
            added = into_slur(&PyString::new(py, &number).into_any())?;
        }
    }
    Ok(added)
}

/// music21's `beamTogether`: everything with beams beamed in one group,
/// what a nested `<beam>` beamed left alone.
#[pyfunction]
fn beamTogether<'py>(someThings: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = someThings.py();
    let to_number = class(py, "duration", "convertTypeToNumber")?;
    let mut last: isize = -1;
    for (place, thing) in someThings.try_iter()?.enumerate() {
        let thing = thing?;
        if !thing.hasattr("beams")? {
            continue;
        }
        let kind = if last == -1 { "start" } else { "continue" };
        let duration_type = thing.getattr("duration")?.getattr("type")?;
        let beamable = to_number.call1((&duration_type,))?.gt(QUARTER_NUMBER)?;
        let beams = thing.getattr("beams")?;
        if beamable && !beams.is_truthy()? {
            beams.call_method1("fill", (duration_type, kind))?;
            last = place as isize;
        }
    }
    someThings
        .get_item(last)?
        .getattr("beams")?
        .call_method1("setAll", ("stop",))?;
    Ok(someThings.clone())
}

/// music21's `removeOctothorpe`.
#[pyfunction]
fn removeOctothorpe(xmlid: &str) -> String {
    step::without_hash(xmlid).to_owned()
}

// ------------------------------------------------------------ metadata

/// music21's `makeMetadata`: the title, composer and date of the first
/// `<work>`.
#[pyfunction]
fn makeMetadata<'py>(documentRoot: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = documentRoot.py();
    let mut meta = class(py, "metadata", "Metadata")?.call0()?;
    let work = documentRoot.call_method1("find", (format!(".//{MEI_NS}work"),))?;
    if work.is_none() {
        return Ok(meta);
    }
    for step in ["metaSetTitle", "metaSetComposer", "metaSetDate"] {
        meta = named(py, step)?.call1((&work, meta))?;
    }
    Ok(meta)
}

/// `work.findall(path)` with the namespace put before each step of it.
fn found_all<'py>(work: &Bound<'py, PyAny>, path: &[&str]) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let query = path_of(path);
    work.call_method1("findall", (query,))?
        .try_iter()?
        .collect()
}

/// A relative path with MEI's namespace before each tag.
fn path_of(path: &[&str]) -> String {
    let mut query = String::from(".");
    for each in path {
        query.push('/');
        query.push_str(&tag(each));
    }
    query
}

/// music21's `metaSetTitle`: the title, with the subtitle after it, and
/// the movement's name from `<tempo>`.
#[pyfunction]
fn metaSetTitle<'py>(
    work: &Bound<'py, PyAny>,
    meta: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut subtitle = None;
    for title in found_all(work, &["titleStmt", "title"])? {
        if get_or(&title, "type", "")?.eq("subtitle")? {
            subtitle = Some(title.getattr("text")?);
        } else if meta.getattr("title")?.is_none() {
            meta.setattr("title", title.getattr("text")?)?;
        }
    }
    if let Some(subtitle) = subtitle.filter(|subtitle| subtitle.is_truthy().unwrap_or(false)) {
        let joined = format!("{} ({})", text(&meta.getattr("title")?)?, text(&subtitle)?);
        meta.setattr("title", joined)?;
    }
    let tempo = work.call_method1("find", (path_of(&["tempo"]),))?;
    if !tempo.is_none() {
        meta.setattr("movementName", tempo.getattr("text")?)?;
    }
    Ok(meta)
}

/// music21's `metaSetComposer`: the composer, or the composers where there
/// are more than one.
#[pyfunction]
fn metaSetComposer<'py>(
    work: &Bound<'py, PyAny>,
    meta: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = work.py();
    let composers = PyList::empty(py);
    for name in found_all(work, &["titleStmt", "respStmt", "persName"])? {
        let named_text = name.getattr("text")?;
        if get(&name, "role")?.eq("composer")? && named_text.is_truthy()? {
            composers.append(named_text)?;
        }
    }
    for composer in found_all(work, &["titleStmt", "composer"])? {
        let written = composer.getattr("text")?;
        if written.is_truthy()? {
            composers.append(written)?;
            continue;
        }
        let name = composer.call_method1("find", (path_of(&["persName"]),))?;
        let written = name.getattr("text")?;
        if written.is_truthy()? {
            composers.append(written)?;
        }
    }
    if composers.len() == 1 {
        meta.setattr("composer", composers.get_item(0)?)?;
    } else if composers.len() > 1 {
        meta.setattr("composers", composers)?;
    }
    Ok(meta)
}

/// music21's `metaSetDate`: the date of composition, or the range it fell
/// in.
#[pyfunction]
fn metaSetDate<'py>(
    work: &Bound<'py, PyAny>,
    meta: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = work.py();
    let date = work.call_method1("find", (path_of(&["history", "creation", "date"]),))?;
    if date.is_none() {
        return Ok(meta);
    }

    // One date, written as text or as `isodate`.
    let iso = get(&date, "isodate")?;
    let written = date.getattr("text")?;
    if written.is_truthy()? || iso.is_truthy()? {
        let given = if iso.is_truthy()? { iso } else { written };
        let made = class(py, "metadata", "Date")?.call0()?;
        let slashed = given.call_method1("replace", ("-", "/"))?;
        match made.call_method1("loadStr", (slashed,)) {
            Ok(_) => meta.setattr("dateCreated", made)?,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
                warn(py, filled(MISSED_DATE, &[text(&given)?]))?;
            }
            Err(error) => return Err(error),
        }
        return Ok(meta);
    }

    // A range, by either pair of names.
    let either = |first: &str, second: &str| -> PyResult<Bound<'py, PyAny>> {
        let value = get(&date, first)?;
        if value.is_truthy()? {
            return Ok(value);
        }
        get(&date, second)
    };
    let start = either("notbefore", "startdate")?;
    let end = either("notafter", "enddate")?;
    if start.is_truthy()? && end.is_truthy()? {
        let range = PyTuple::new(py, [start, end])?;
        meta.setattr(
            "dateCreated",
            class(py, "metadata", "DateBetween")?.call1((range,))?,
        )?;
    }
    Ok(meta)
}

/// music21's `getVoiceId`: the id of the one voice among `fromThese`.
#[pyfunction]
fn getVoiceId<'py>(fromThese: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = fromThese.py();
    let voice = class(py, "stream", "Voice")?;
    let mut voices = Vec::new();
    for item in fromThese.try_iter()? {
        let item = item?;
        if item.is_instance(&voice)? {
            voices.push(item);
        }
    }
    match &voices[..] {
        [only] => only.getattr("id"),
        _ => Err(pyo3::exceptions::PyRuntimeError::new_err(
            "getVoiceId: found too few or too many Voice objects",
        )),
    }
}

// ------------------------------------------------------------ tuplets

/// music21's `scaleToTuplet`: each note, rest and chord scaled into the
/// tuplet `elem` names, or marked to be searched for later.
#[pyfunction]
fn scaleToTuplet<'py>(
    objs: &Bound<'py, PyAny>,
    elem: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = objs.py();
    let many = ["list", "set", "tuple"]
        .iter()
        .map(|name| py.import("builtins")?.getattr(*name))
        .collect::<PyResult<Vec<_>>>()?;
    let was_list = objs.is_instance(&PyTuple::new(py, many)?.into_any())?;
    let things: Vec<Bound<'py, PyAny>> = if was_list {
        objs.try_iter()?.collect::<PyResult<_>>()?
    } else {
        vec![objs.clone()]
    };

    for obj in &things {
        if !has_duration(obj)? {
            continue;
        }
        if has(elem, "m21TupletSearch")? {
            for name in ["m21TupletSearch", "m21TupletNum", "m21TupletNumbase"] {
                obj.setattr(name, get(elem, name)?)?;
            }
            continue;
        }

        // The tuplet, of the note's own type.
        let duration = obj.getattr("duration")?;
        let kind = duration.getattr("type")?;
        let pairs = [
            ("numberNotesActual", int(&get(elem, "m21TupletNum")?)?),
            ("numberNotesNormal", int(&get(elem, "m21TupletNumbase")?)?),
            ("durationNormal", kind.clone()),
            ("durationActual", kind),
        ];
        let tuplet = class(py, "duration", "Tuplet")?.call((), Some(&keywords(py, &pairs)?))?;
        duration.call_method1("appendTuplet", (tuplet,))?;

        // Its bracket, from `m21TupletType` or else `tuplet`.
        let first = || -> PyResult<Bound<'py, PyAny>> {
            obj.getattr("duration")?.getattr("tuplets")?.get_item(0)
        };
        let given = get(elem, "m21TupletType")?;
        let written = text(&get_or(elem, "tuplet", "")?)?;
        if !given.is_none() {
            first()?.setattr("type", given)?;
        } else if written.starts_with('i') {
            first()?.setattr("type", "start")?;
        } else if written.starts_with('t') {
            first()?.setattr("type", "stop")?;
        }
    }

    if was_list {
        return Ok(objs.clone());
    }
    Ok(things.into_iter().next().unwrap_or_else(|| objs.clone()))
}

/// Whether a note carries `m21TupletSearch` of `value`.
fn searching(thing: &Bound<'_, PyAny>, value: &str) -> PyResult<bool> {
    if !thing.hasattr("m21TupletSearch")? {
        return Ok(false);
    }
    thing.getattr("m21TupletSearch")?.eq(value)
}

/// The tuplet-search marks taken off a note.
fn unmark(thing: &Bound<'_, PyAny>) -> PyResult<()> {
    for name in ["m21TupletSearch", "m21TupletNum", "m21TupletNumbase"] {
        thing.delattr(name)?;
    }
    Ok(())
}

/// music21's `_guessTuplets`: the notes from each marked start to its
/// marked end scaled into the tuplet the start names.
#[pyfunction]
#[pyo3(name = "_guessTuplets")]
fn guess_tuplets<'py>(theLayer: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = theLayer.py();
    let mut tuplet: Option<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = None;
    for thing in theLayer.try_iter()? {
        let thing = thing?;
        if !has_duration(&thing)? {
            continue;
        }
        if searching(&thing, "start")? {
            let num = int(&thing.getattr("m21TupletNum")?)?.str()?.into_any();
            let numbase = int(&thing.getattr("m21TupletNumbase")?)?.str()?.into_any();
            tuplet = Some((num, numbase));
            unmark(&thing)?;
        }
        let Some((num, numbase)) = &tuplet else {
            continue;
        };
        let pairs = [
            ("m21TupletNum", num.clone()),
            ("m21TupletNumbase", numbase.clone()),
        ];
        let marked = element(py, "", &keywords(py, &pairs)?)?;
        named(py, "scaleToTuplet")?.call1((&thing, marked))?;
        if searching(&thing, "end")? {
            thing
                .getattr("duration")?
                .getattr("tuplets")?
                .get_item(0)?
                .setattr("type", "stop")?;
            unmark(&thing)?;
            tuplet = None;
        }
    }
    Ok(theLayer.clone())
}

// ------------------------------------------------------------ elements

/// music21's `scoreDefFromElement`: what a `<scoreDef>` says of the whole
/// score, of every part, and of each part by its `n`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn scoreDefFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let every = PyList::empty(py);
    let post = PyDict::new(py);
    post.set_item(ALL_PARTS, &every)?;
    post.set_item(WHOLE_SCORE, PyList::empty(py))?;

    // What holds for every part.
    if has(elem, "meter.count")? {
        every.append(named(py, "_timeSigFromAttrs")?.call1((elem,))?)?;
    }
    if has(elem, "key.pname")? || has(elem, "key.sig")? {
        every.append(named(py, "_keySigFromAttrs")?.call1((elem,))?)?;
    }

    // Each part's, from its staff group.
    for group in elem
        .call_method1("iterfind", (tag("staffGrp"),))?
        .try_iter()?
    {
        let parts = named(py, "staffGrpFromElement")?.call1((group?, &slurBundle))?;
        post.call_method1("update", (parts,))?;
    }
    Ok(post)
}

/// music21's `staffGrpFromElement`: each `<staffDef>` in a group and the
/// groups inside it, by its `n`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None, staffDefDict=None))]
fn staffGrpFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
    staffDefDict: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let defs = match staffDefDict {
        Some(given) if !given.is_none() => given,
        _ => PyDict::new(py).into_any(),
    };
    let staff_def = tag("staffDef");
    let staff_group = tag("staffGrp");
    for held in elem.call_method1("findall", ("*",))?.try_iter()? {
        let held = held?;
        let found = held.getattr("tag")?;
        if found.eq(&staff_def)? {
            let made = named(py, "staffDefFromElement")?.call1((&held, &slurBundle))?;
            defs.set_item(get(&held, "n")?, made)?;
        } else if found.eq(&staff_group)? {
            named(py, "staffGrpFromElement")?.call1((&held, &slurBundle, &defs))?;
        }
    }
    Ok(defs)
}

/// The instrument a `<staffDef>` names by its `<instrDef>` or its label,
/// or nothing.
fn staff_instrument<'py>(elem: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py = elem.py();
    let definition = elem.call_method1("find", (tag("instrDef"),))?;
    if !definition.is_none() {
        return Ok(Some(
            named(py, "instrDefFromElement")?.call1((definition,))?,
        ));
    }
    let instrument = m21(py, "instrument")?;
    match instrument
        .getattr("fromString")?
        .call1((get_or(elem, "label", "")?,))
    {
        Ok(found) => Ok(Some(found)),
        Err(error) if error.is_instance(py, &instrument.getattr("InstrumentException")?) => {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// music21's `staffDefFromElement`: a part's instrument, meter, key and
/// clef, each by its key.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn staffDefFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let mapping = steps(py, &[("clef", "clefFromElement")])?;
    let post = PyDict::new(py);

    // The instrument, named as the part.
    if let Some(made) = staff_instrument(elem)? {
        made.setattr("partName", get(elem, "label")?)?;
        made.setattr("partAbbreviation", get(elem, "label.abbr")?)?;
        made.setattr("partId", get(elem, "n")?)?;
        post.set_item("instrument", made)?;
    }
    if has(elem, "trans.semi")? {
        if !post.contains("instrument")? {
            post.set_item(
                "instrument",
                class(py, "instrument", "Instrument")?.call0()?,
            )?;
        }
        let interval = named(py, "_transpositionFromAttrs")?.call1((elem,))?;
        post.get_item("instrument")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("instrument"))?
            .setattr("transposition", interval)?;
    }

    // The meter, key and clef written as attributes.
    if has(elem, "meter.count")? {
        post.set_item("meter", named(py, "_timeSigFromAttrs")?.call1((elem,))?)?;
    }
    if has(elem, "key.pname")? || has(elem, "key.sig")? {
        post.set_item("key", named(py, "_keySigFromAttrs")?.call1((elem,))?)?;
    }
    if has(elem, "clef.shape")? {
        let attributes = PyDict::new(py);
        for (name, from) in [
            ("shape", "clef.shape"),
            ("line", "clef.line"),
            ("dis", "clef.dis"),
            ("dis.place", "clef.dis.place"),
        ] {
            attributes.set_item(name, get(elem, from)?)?;
        }
        let made = crate::musicxml::element_tree(py)?
            .getattr("Element")?
            .call1(("clef", attributes))?;
        post.set_item("clef", named(py, "clefFromElement")?.call1((made,))?)?;
    }

    // A `<clef>` held within over the attributes'.
    let clef = class(py, "clef", "Clef")?;
    for item in embedded(elem, &mapping, &slurBundle)?.try_iter()? {
        let item = item?;
        if item.is_instance(&clef)? {
            post.set_item("clef", item)?;
        }
    }
    Ok(post)
}

/// music21's `dotFromElement`: one dot.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn dotFromElement(elem: &Bound<'_, PyAny>, slurBundle: Option<Bound<'_, PyAny>>) -> i32 {
    let _ = (elem, slurBundle);
    1
}

/// music21's `articFromElement`: the marks of an `<artic>`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn articFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    let py = elem.py();
    let marks = get(elem, "artic")?;
    if marks.is_none() {
        return Ok(PyList::empty(py).into_any());
    }
    named(py, "_makeArticList")?.call1((marks,))
}

/// music21's `accidFromElement`: the accidental an `<accid>` names, its
/// sounding one first.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn accidFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    let py = elem.py();
    if has(elem, "accid.ges")? {
        return named(py, "_accidGesFromAttr")?.call1((get_or(elem, "accid.ges", "")?,));
    }
    named(py, "_accidentalFromAttr")?.call1((get(elem, "accid")?,))
}

/// music21's `sylFromElement`: a syllable as a lyric.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn sylFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    let py = elem.py();
    let lyric = class(py, "note", "Lyric")?;
    let written = elem.getattr("text")?;
    let position = get_text(elem, "wordpos")?;
    let connector = get_text(elem, "con")?;
    if written.is_none() && position.is_none() {
        return lyric.call((), Some(&keywords(py, &[("text", written)])?));
    }
    let (joined, syllabic) = step::syllable(
        &written.extract::<String>()?,
        position.as_deref(),
        connector.as_deref(),
    )
    .map_err(|error| pyo3::exceptions::PyKeyError::new_err(crate::pitch::message(&error)))?;
    let joined = PyString::new(py, &joined).into_any();
    let Some(syllabic) = syllabic else {
        return lyric.call((), Some(&keywords(py, &[("text", joined)])?));
    };
    let pairs = [
        ("text", joined),
        ("syllabic", PyString::new(py, syllabic.as_str()).into_any()),
        ("applyRaw", true.into_pyobject(py)?.to_owned().into_any()),
    ];
    lyric.call((), Some(&keywords(py, &pairs)?))
}

/// music21's `verseFromElement`: a verse's syllables, numbered by its `n`
/// or else `backupN`.
#[pyfunction]
#[pyo3(signature = (elem, backupN=None, slurBundle=None))]
fn verseFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    backupN: Option<Bound<'py, PyAny>>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let _ = slurBundle;
    let py = elem.py();
    let backupN = backupN.unwrap_or_else(|| py.None().into_bound(py));
    let syllables = PyList::empty(py);
    let each = named(py, "sylFromElement")?;
    for syllable in elem
        .call_method1("findall", (path_of(&["syl"]),))?
        .try_iter()?
    {
        syllables.append(each.call1((syllable?,))?)?;
    }
    for syllable in syllables.iter() {
        let number = get_or(elem, "n", &backupN)?;
        match int(&number) {
            Ok(number) => syllable.setattr("number", number)?,
            Err(error)
                if error.is_instance_of::<pyo3::exceptions::PyTypeError>(py)
                    || error.is_instance_of::<pyo3::exceptions::PyValueError>(py) =>
            {
                warn(py, filled(BAD_VERSE_NUMBER, &[text(&number)?]))?;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(syllables)
}

/// music21's `noteFromElement`: a `<note>` with what it holds.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn noteFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let mapping = steps(
        py,
        &[
            ("dot", "dotFromElement"),
            ("artic", "articFromElement"),
            ("accid", "accidFromElement"),
            ("syl", "sylFromElement"),
        ],
    )?;

    // The pitch and written duration.
    let accidental = named(py, "_accidentalFromAttr")?.call1((get(elem, "accid")?,))?;
    let pitch = named(py, "safePitch")?.call1((
        get_or(elem, "pname", "")?,
        accidental,
        get_or(elem, "oct", "")?,
    ))?;
    let mut made = class(py, "note", "Note")?.call1((pitch,))?;
    made.setattr("duration", written_duration(elem)?)?;

    // What it holds: dots, marks, an accidental, a syllable.
    let mut dots: i64 = 0;
    let articulation = class(py, "articulations", "Articulation")?;
    let lyric = class(py, "note", "Lyric")?;
    for held in embedded(elem, &mapping, &slurBundle)?.try_iter()? {
        let held = held?;
        if held.is_instance_of::<PyInt>() {
            dots += held.extract::<i64>()?;
        } else if held.is_instance(&articulation)? {
            made.getattr("articulations")?
                .call_method1("append", (held,))?;
        } else if held.is_instance_of::<PyString>() {
            let accidental = class(py, "pitch", "Accidental")?.call1((held,))?;
            made.getattr("pitch")?.setattr("accidental", accidental)?;
        } else if held.is_instance(&lyric)? {
            made.setattr("lyrics", PyList::new(py, [held])?)?;
        }
    }

    // The attributes after.
    if has(elem, "accid.ges")? {
        let sounding = named(py, "_accidGesFromAttr")?.call1((get_or(elem, "accid.ges", "")?,))?;
        let accidental = class(py, "pitch", "Accidental")?.call1((sounding,))?;
        made.getattr("pitch")?.setattr("accidental", accidental)?;
    }
    finish_event(elem, &made, &slurBundle)?;
    if dots > 0 {
        let length = named(py, "_qlDurationFromAttr")?.call1((get(elem, "dur")?,))?;
        made.setattr(
            "duration",
            named(py, "makeDuration")?.call1((length, dots))?,
        )?;
    }
    grace(elem, &made)?;
    fill_beams(elem, &made)?;
    made = tupleted(elem, made)?;

    // The verses held within, numbered from one.
    if !elem.call_method1("find", (path_of(&["verse"]),))?.is_none() {
        let lyrics = PyList::empty(py);
        let each = named(py, "verseFromElement")?;
        let verses = elem.call_method1("findall", (path_of(&["verse"]),))?;
        for (place, verse) in verses.try_iter()?.enumerate() {
            let numbered = keywords(
                py,
                &[("backupN", (place + 1).into_pyobject(py)?.into_any())],
            )?;
            for syllable in each.call((verse?,), Some(&numbered))?.try_iter()? {
                lyrics.append(syllable?)?;
            }
        }
        made.setattr("lyrics", lyrics)?;
    }
    Ok(made)
}

/// A rest of an element's written duration, with its id and tuplet;
/// hidden for a space.
fn rest_of<'py>(elem: &Bound<'py, PyAny>, shown: Shown) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let duration = written_duration(elem)?;
    let made =
        class(py, "note", "Rest")?.call((), Some(&keywords(py, &[("duration", duration)])?))?;
    if shown == Shown::Hidden {
        made.getattr("style")?.setattr("hideObjectOnPrint", true)?;
    }
    if has(elem, XMLID)? {
        made.setattr("id", get(elem, XMLID)?)?;
    }
    tupleted(elem, made)
}

/// Whether a rest is printed.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    Printed,
    Hidden,
}

/// music21's `restFromElement`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn restFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    rest_of(elem, Shown::Printed)
}

/// music21's `spaceFromElement`: a rest not printed.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn spaceFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    rest_of(elem, Shown::Hidden)
}

/// A whole-measure rest or space: `step`'s, marked to be given its
/// measure's length where it has no `dur`.
fn measure_long<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
    step: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let made = named(py, step)?.call1((elem, slurBundle))?;
    if !has(elem, "dur")? {
        made.setattr("m21wasMRest", true)?;
    }
    Ok(made)
}

/// music21's `mRestFromElement`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn mRestFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    measure_long(elem, slurBundle, "restFromElement")
}

/// music21's `mSpaceFromElement`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn mSpaceFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    measure_long(elem, slurBundle, "spaceFromElement")
}

/// music21's `chordFromElement`: a `<chord>` of its `<note>`s.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn chordFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let mapping = steps(py, &[("artic", "articFromElement")])?;
    let nothing = PyCFunction::new_closure(
        py,
        None,
        None,
        |_: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> { Ok(()) },
    )?;
    mapping.set_item(tag("note"), nothing)?;

    // The notes, then the chord's own duration.
    let notes = PyList::empty(py);
    let each = named(py, "noteFromElement")?;
    for held in elem.call_method1("iterfind", (tag("note"),))?.try_iter()? {
        notes.append(each.call1((held?, &slurBundle))?)?;
    }
    let mut made = class(py, "chord", "Chord")?
        .call((), Some(&keywords(py, &[("notes", notes.into_any())])?))?;
    made.setattr("duration", written_duration(elem)?)?;

    let articulation = class(py, "articulations", "Articulation")?;
    for held in embedded(elem, &mapping, &slurBundle)?.try_iter()? {
        let held = held?;
        if held.is_instance(&articulation)? {
            made.getattr("articulations")?
                .call_method1("append", (held,))?;
        }
    }
    finish_event(elem, &made, &slurBundle)?;
    grace(elem, &made)?;
    fill_beams(elem, &made)?;
    made = tupleted(elem, made)?;
    Ok(made)
}

/// music21's `clefFromElement`.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn clefFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    let py = elem.py();
    let made = if get(elem, "shape")?.eq("perc")? {
        class(py, "clef", "PercussionClef")?.call0()?
    } else if get(elem, "shape")?.eq("TAB")? {
        class(py, "clef", "TabClef")?.call0()?
    } else {
        let name = get(elem, "shape")?.add(get(elem, "line")?)?;
        let shift =
            named(py, "_getOctaveShift")?.call1((get(elem, "dis")?, get(elem, "dis.place")?))?;
        class(py, "clef", "clefFromString")?
            .call((name,), Some(&keywords(py, &[("octaveShift", shift)])?))?
    };
    if has(elem, XMLID)? {
        made.setattr("id", get(elem, XMLID)?)?;
    }
    Ok(made)
}

/// music21's `instrDefFromElement`: the instrument an `<instrDef>` names
/// by MIDI program or by name.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn instrDefFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    let py = elem.py();
    let instrument = m21(py, "instrument")?;
    if has(elem, "midi.instrnum")? {
        let program = int(&get(elem, "midi.instrnum")?)?;
        return instrument
            .getattr("instrumentFromMidiProgram")?
            .call1((program,));
    }
    let refused = instrument.getattr("InstrumentException")?;
    match instrument
        .getattr("fromString")?
        .call1((get(elem, "midi.instrname")?,))
    {
        Ok(found) => Ok(found),
        Err(error)
            if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py)
                || error.is_instance(py, &refused) =>
        {
            let made = instrument.getattr("Instrument")?.call0()?;
            made.setattr("partName", get_or(elem, "midi.instrname", "")?)?;
            Ok(made)
        }
        Err(error) => Err(error),
    }
}

/// What a `<beam>` and a `<tuplet>` may hold, and the step for each.
const BEAM_CONTENTS: [(&str, &str); 9] = [
    ("clef", "clefFromElement"),
    ("chord", "chordFromElement"),
    ("note", "noteFromElement"),
    ("rest", "restFromElement"),
    ("tuplet", "tupletFromElement"),
    ("beam", "beamFromElement"),
    ("bTrem", "bTremFromElement"),
    ("space", "spaceFromElement"),
    ("barLine", "barLineFromElement"),
];

/// music21's `beamFromElement`: what a `<beam>` holds, beamed together.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn beamFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let held = embedded(elem, &steps(py, &BEAM_CONTENTS)?, &slurBundle)?;
    named(py, "beamTogether")?.call1((held,))
}

/// music21's `barLineFromElement`: the barline a `<barLine>`'s `rend`
/// names.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn barLineFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = slurBundle;
    let py = elem.py();
    named(py, "_barlineFromAttr")?.call1((get_or(elem, "rend", "single")?,))
}

/// music21's `tupletFromElement`: what a `<tuplet>` holds, scaled into it,
/// bracketed from its first note to its last, and beamed.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn tupletFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    if !has(elem, "num")? || !has(elem, "numbase")? {
        return Err(MeiAttributeError::new_err(MISSING_TUPLET_DATA));
    }
    let mapping = steps(
        py,
        &[
            ("tuplet", "tupletFromElement"),
            ("beam", "beamFromElement"),
            ("note", "noteFromElement"),
            ("rest", "restFromElement"),
            ("chord", "chordFromElement"),
            ("clef", "clefFromElement"),
            ("bTrem", "bTremFromElement"),
            ("space", "spaceFromElement"),
            ("barLine", "barLineFromElement"),
        ],
    )?;
    let members = embedded(elem, &mapping, &slurBundle)?;
    let pairs = [
        ("m21TupletNum", get(elem, "num")?),
        ("m21TupletNumbase", get(elem, "numbase")?),
    ];
    let marked = element(py, "c", &keywords(py, &pairs)?)?;
    let members = named(py, "scaleToTuplet")?.call1((members, marked))?;

    // The bracket from the first note to the last.
    let general = class(py, "note", "GeneralNote")?;
    let mut first = None;
    let mut last = None;
    for (place, each) in members.try_iter()?.enumerate() {
        if !each?.is_instance(&general)? {
            continue;
        }
        if first.is_none() {
            first = Some(place);
        } else {
            last = Some(place);
        }
    }
    let bracket = |place: Option<usize>, kind: &str| -> PyResult<()> {
        let place: Bound<'py, PyAny> = match place {
            Some(place) => place.into_pyobject(py)?.into_any(),
            None => py.None().into_bound(py),
        };
        members
            .get_item(place)?
            .getattr("duration")?
            .getattr("tuplets")?
            .get_item(0)?
            .setattr("type", kind)
    };
    bracket(first, "start")?;
    bracket(last.or(first), "stop")?;

    let members = named(py, "beamTogether")?.call1((members,))?;
    PyTuple::new(py, members.try_iter()?.collect::<PyResult<Vec<_>>>()?)
}

/// music21's `bTremFromElement`: what a `<bTrem>` holds, each note and
/// chord with the tremolo its `unitdur` names.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn bTremFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let mapping = steps(
        py,
        &[("chord", "chordFromElement"), ("note", "noteFromElement")],
    )?;
    let held = embedded(elem, &mapping, &slurBundle)?;
    let unit = text(&get_or(elem, "unitdur", "")?)?;
    let Some(marks) = step::tremolo_marks(&unit) else {
        return Ok(held);
    };
    let not_rest = class(py, "note", "NotRest")?;
    for each in held.try_iter()? {
        let each = each?;
        if !each.is_instance(&not_rest)? {
            continue;
        }
        let tremolo = class(py, "expressions", "Tremolo")?.call0()?;
        tremolo.setattr("numberOfMarks", marks)?;
        each.getattr("expressions")?
            .call_method1("append", (tremolo,))?;
    }
    Ok(held)
}

/// music21's `layerFromElement`: a `<layer>` as a voice, its id from
/// `overrideN` or its `n`.
#[pyfunction]
#[pyo3(signature = (elem, overrideN=None, slurBundle=None))]
fn layerFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    overrideN: Option<Bound<'py, PyAny>>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let mapping = steps(
        py,
        &[
            ("clef", "clefFromElement"),
            ("chord", "chordFromElement"),
            ("note", "noteFromElement"),
            ("rest", "restFromElement"),
            ("mRest", "mRestFromElement"),
            ("beam", "beamFromElement"),
            ("tuplet", "tupletFromElement"),
            ("bTrem", "bTremFromElement"),
            ("space", "spaceFromElement"),
            ("mSpace", "mSpaceFromElement"),
            ("barLine", "barLineFromElement"),
        ],
    )?;
    let layer = named(py, "_processEmbeddedElements")?.call1((
        elem.call_method1("iterfind", ("*",))?,
        mapping,
        elem.getattr("tag")?,
        &slurBundle,
    ))?;
    let layer = named(py, "_guessTuplets")?.call1((layer,))?;

    let voice = class(py, "stream", "Voice")?.call0()?;
    for each in layer.try_iter()? {
        voice.call_method1("coreAppend", (each?,))?;
    }
    voice.call_method0("coreElementsChanged")?;

    // Its id.
    if let Some(given) = overrideN.filter(|given| given.is_truthy().unwrap_or(false)) {
        voice.setattr("id", given)?;
    } else if has(elem, "n")? {
        voice.setattr("id", get(elem, "n")?)?;
    } else {
        return Err(MeiAttributeError::new_err(MISSING_VOICE_ID));
    }
    Ok(voice)
}

/// music21's `staffFromElement`: each `<layer>` of a `<staff>` as a voice,
/// numbered from one.
#[pyfunction]
#[pyo3(signature = (elem, slurBundle=None))]
fn staffFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let layers = PyList::empty(py);
    let layer_tag = tag("layer");
    let ignored = named(py, "_IGNORE_UNPROCESSED")?;
    let mut number: u32 = 1;
    for held in elem.call_method1("iterfind", ("*",))?.try_iter()? {
        let held = held?;
        let found = held.getattr("tag")?;
        if found.eq(&layer_tag)? {
            let bundle = keywords(py, &[("slurBundle", slurBundle.clone())])?;
            let voice =
                named(py, "layerFromElement")?.call((&held, number.to_string()), Some(&bundle))?;
            layers.append(voice)?;
            number += 1;
        } else if !ignored.contains(&found)? {
            debug(
                py,
                filled(
                    UNPROCESSED_SUBELEMENT,
                    &[text(&found)?, text(&elem.getattr("tag")?)?],
                ),
            )?;
        }
    }
    Ok(layers)
}

/// music21's `_correctMRestDurs`: every whole-measure rest given the
/// measure's length, and its voice and measure with it.
#[pyfunction]
#[pyo3(name = "_correctMRestDurs")]
fn correct_m_rest_durs(staves: &Bound<'_, PyAny>, targetLength: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = staves.py();
    let streams = class(py, "stream", "Stream")?;
    let duration = class(py, "duration", "Duration")?;
    for measure in staves.call_method0("values")?.try_iter()? {
        let measure = measure?;
        for voice in measure.try_iter()? {
            let voice = voice?;
            if !voice.is_instance(&streams)? {
                continue;
            }
            let held: Vec<_> = voice.try_iter()?.collect::<PyResult<_>>()?;
            for each in held {
                if !each.hasattr("m21wasMRest")? {
                    continue;
                }
                each.setattr("quarterLength", targetLength)?;
                voice.setattr("duration", duration.call1((targetLength,))?)?;
                measure.setattr("duration", duration.call1((targetLength,))?)?;
                each.delattr("m21wasMRest")?;
            }
        }
    }
    Ok(())
}

/// The barline on every measure in `staves`, each its own copy.
fn set_barlines(staves: &Bound<'_, PyAny>, side: &str, barline: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = staves.py();
    let measure = class(py, "stream", "Measure")?;
    for each in staves.call_method0("values")?.try_iter()? {
        let each = each?;
        if each.is_instance(&measure)? {
            each.setattr(side, deepcopy(barline)?)?;
        }
    }
    Ok(())
}

/// music21's `_makeBarlines`: a measure's `left` and `right` put on each
/// part's measure; the opening half of a two-way repeat kept for the next.
#[pyfunction]
#[pyo3(name = "_makeBarlines")]
fn make_barlines<'py>(
    elem: &Bound<'py, PyAny>,
    staves: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let from_attr = named(py, "_barlineFromAttr")?;
    if has(elem, "left")? {
        let mut bars = from_attr.call1((get(elem, "left")?,))?;
        if bars.hasattr("__len__")? {
            bars = bars.get_item(1)?;
        }
        set_barlines(&staves, "leftBarline", &bars)?;
    }
    if has(elem, "right")? {
        let mut bars = from_attr.call1((get(elem, "right")?,))?;
        if bars.hasattr("__len__")? {
            staves.set_item(NEXT_LEFT, bars.get_item(1)?)?;
            bars = bars.get_item(0)?;
        }
        set_barlines(&staves, "rightBarline", &bars)?;
    }
    Ok(staves)
}

/// music21's `measureFromElement`: a `<measure>` as one measure per part,
/// by part; a part it lacks given a rest through it.
#[pyfunction]
#[pyo3(signature = (elem, backupNum, expectedNs, slurBundle=None, activeMeter=None))]
fn measureFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    backupNum: &Bound<'py, PyAny>,
    expectedNs: &Bound<'py, PyAny>,
    slurBundle: Option<Bound<'py, PyAny>>,
    activeMeter: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    let slurBundle = slurBundle.unwrap_or_else(|| py.None().into_bound(py));
    let staves = PyDict::new(py);
    let waiting = PyDict::new(py);
    let staff_tag = tag("staff");
    let staff_def_tag = tag("staffDef");
    let ignored = named(py, "_IGNORE_UNPROCESSED")?;
    let measure = class(py, "stream", "Measure")?;
    let number = || -> PyResult<Bound<'py, PyAny>> { int(&get_or(elem, "n", backupNum)?) };
    let mut longest: Option<Bound<'py, PyAny>> = None;

    // Each staff as a measure, and each staff definition kept for it.
    for held in elem.call_method1("iterfind", ("*",))?.try_iter()? {
        let held = held?;
        let found = held.getattr("tag")?;
        if found.eq(&staff_tag)? {
            let bundle = keywords(py, &[("slurBundle", slurBundle.clone())])?;
            let voices = named(py, "staffFromElement")?.call((&held,), Some(&bundle))?;
            let numbered = keywords(py, &[("number", number()?)])?;
            let made = measure.call((voices,), Some(&numbered))?;
            let length = made.getattr("duration")?.getattr("quarterLength")?;
            staves.set_item(get(&held, "n")?, made)?;
            if longest
                .as_ref()
                .is_none_or(|before| before.lt(&length).unwrap_or(false))
            {
                longest = Some(length);
            }
        } else if found.eq(&staff_def_tag)? {
            let which = get(&held, "n")?;
            if which.is_none() {
                warn(
                    py,
                    filled(UNIMPLEMENTED_IMPORT, &["<staffDef>".into(), "@n".into()]),
                )?;
            } else {
                let made = named(py, "staffDefFromElement")?.call1((&held, &slurBundle))?;
                waiting.set_item(which, made)?;
            }
        } else if !ignored.contains(&found)? {
            debug(
                py,
                filled(
                    UNPROCESSED_SUBELEMENT,
                    &[text(&found)?, text(&elem.getattr("tag")?)?],
                ),
            )?;
        }
    }
    for (which, defined) in waiting.iter() {
        for each in defined.call_method0("values")?.try_iter()? {
            staves
                .get_item(&which)?
                .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(which.clone().unbind()))?
                .call_method1("insert", (0, each?))?;
        }
    }

    // A rest through the measure for each part with no staff in it.
    let longest = longest.unwrap_or_else(|| py.None().into_bound(py));
    for n in expectedNs.try_iter()? {
        let n = n?;
        if staves.contains(&n)? {
            continue;
        }
        let rest = class(py, "note", "Rest")?.call(
            (),
            Some(&keywords(py, &[("quarterLength", longest.clone())])?),
        )?;
        rest.setattr("m21wasMRest", true)?;
        let voice = class(py, "stream", "Voice")?.call1((PyList::new(py, [rest])?,))?;
        voice.setattr("id", "1")?;
        let numbered = keywords(py, &[("number", number()?)])?;
        staves.set_item(
            n,
            measure.call((PyList::new(py, [voice])?,), Some(&numbered))?,
        )?;
    }

    // Whole-measure rests, by the meter where nothing else says how long.
    let correct = named(py, "_correctMRestDurs")?;
    let metered = match &activeMeter {
        Some(meter) if !meter.is_none() && longest.eq(step::NO_DURATION)? => {
            let bar = meter.getattr("barDuration")?.getattr("quarterLength")?;
            (!longest.eq(&bar)?).then_some(bar)
        }
        _ => None,
    };
    correct.call1((&staves, metered.unwrap_or(longest)))?;
    named(py, "_makeBarlines")?.call1((elem, staves))
}

/// What `sectionScoreCore` carries from one element to the next.
struct Core<'py> {
    active_meter: Bound<'py, PyAny>,
    next_left: Bound<'py, PyAny>,
    backup: Bound<'py, PyAny>,
}

/// music21's `sectionScoreCore`: a `<section>` or `<score>` read part by
/// part, with the meter, left barline and measure number it leaves.
#[pyfunction]
#[pyo3(signature = (elem, allPartNs, slurBundle, *, activeMeter=None, nextMeasureLeft=None, backupMeasureNum=None))]
fn sectionScoreCore<'py>(
    elem: &Bound<'py, PyAny>,
    allPartNs: &Bound<'py, PyAny>,
    slurBundle: &Bound<'py, PyAny>,
    activeMeter: Option<Bound<'py, PyAny>>,
    nextMeasureLeft: Option<Bound<'py, PyAny>>,
    backupMeasureNum: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = elem.py();
    let none = || py.None().into_bound(py);
    let mut core = Core {
        active_meter: activeMeter.unwrap_or_else(none),
        next_left: nextMeasureLeft.unwrap_or_else(none),
        backup: match backupMeasureNum {
            Some(given) => given,
            None => 0_i32.into_pyobject(py)?.into_any(),
        },
    };
    let parts: Vec<Bound<'py, PyAny>> = allPartNs.try_iter()?.collect::<PyResult<_>>()?;
    let parsed = PyDict::new(py);
    let next_thing = PyDict::new(py);
    for n in &parts {
        parsed.set_item(n, PyList::empty(py))?;
        next_thing.set_item(n, PyList::empty(py))?;
    }
    let ignored = named(py, "_IGNORE_UNPROCESSED")?;
    let own = elem.getattr("tag")?;
    let in_section = own.eq(tag("section"))?;

    for held in elem.call_method1("iterfind", ("*",))?.try_iter()? {
        let held = held?;
        let found = text(&held.getattr("tag")?)?;
        if found == tag("measure") && in_section {
            core_measure(
                &held,
                allPartNs,
                &parts,
                slurBundle,
                &parsed,
                &next_thing,
                &mut core,
            )?;
        } else if found == tag("scoreDef") {
            core_score_def(&held, &parts, slurBundle, &next_thing, &mut core)?;
        } else if found == tag("staffDef") {
            core_staff_def(&held, slurBundle, &next_thing, &mut core)?;
        } else if found == tag("section") {
            core_section(
                &held,
                allPartNs,
                slurBundle,
                &own,
                &parsed,
                &next_thing,
                &mut core,
            )?;
        } else if !ignored.contains(&found)? {
            debug(py, filled(UNPROCESSED_SUBELEMENT, &[found, text(&own)?]))?;
        }
    }
    PyTuple::new(
        py,
        [
            parsed.into_any(),
            core.active_meter,
            core.next_left,
            core.backup,
        ],
    )
}

/// The list a dict holds under `key`.
fn list_at<'py>(dict: &Bound<'py, PyDict>, key: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    dict.as_any().get_item(key)
}

/// A `<measure>` in a section: each part's measure, with what waited for
/// it put at its start.
fn core_measure<'py>(
    held: &Bound<'py, PyAny>,
    allPartNs: &Bound<'py, PyAny>,
    parts: &[Bound<'py, PyAny>],
    slurBundle: &Bound<'py, PyAny>,
    parsed: &Bound<'py, PyDict>,
    next_thing: &Bound<'py, PyDict>,
    core: &mut Core<'py>,
) -> PyResult<()> {
    let py = held.py();
    core.backup = core.backup.add(1)?;
    let pairs = [
        ("slurBundle", slurBundle.clone()),
        ("activeMeter", core.active_meter.clone()),
    ];
    let result = named(py, "measureFromElement")?.call(
        (held, &core.backup, allPartNs),
        Some(&keywords(py, &pairs)?),
    )?;
    for n in parts {
        let measure = result.get_item(n)?;
        for thing in list_at(next_thing, n)?.try_iter()? {
            measure.call_method1("insert", (0, thing?))?;
        }
        next_thing.set_item(n, PyList::empty(py))?;
        if !core.next_left.is_none() {
            measure.setattr("leftBarline", deepcopy(&core.next_left)?)?;
        }
        list_at(parsed, n)?.call_method1("append", (measure,))?;
    }
    core.next_left = if result.contains(NEXT_LEFT)? {
        result.get_item(NEXT_LEFT)?
    } else {
        py.None().into_bound(py)
    };
    Ok(())
}

/// A `<scoreDef>`: what holds for every part waits for each part's next
/// measure, a copy for each part after the first, and each part's own.
fn core_score_def<'py>(
    held: &Bound<'py, PyAny>,
    parts: &[Bound<'py, PyAny>],
    slurBundle: &Bound<'py, PyAny>,
    next_thing: &Bound<'py, PyDict>,
    core: &mut Core<'py>,
) -> PyResult<()> {
    let py = held.py();
    let local = named(py, "scoreDefFromElement")?.call1((held, slurBundle))?;
    let meter = class(py, "meter", "TimeSignature")?;
    for every in local.get_item(ALL_PARTS)?.try_iter()? {
        let every = every?;
        if every.is_instance(&meter)? {
            core.active_meter = every.clone();
        }
        for (place, n) in parts.iter().enumerate() {
            let inserted = if place == 0 {
                every.clone()
            } else {
                deepcopy(&every)?
            };
            list_at(next_thing, n)?.call_method1("append", (inserted,))?;
        }
    }
    for n in parts {
        if !local.contains(n)? {
            continue;
        }
        for each in local.get_item(n)?.call_method0("values")?.try_iter()? {
            list_at(next_thing, n)?.call_method1("append", (each?,))?;
        }
    }
    Ok(())
}

/// A `<staffDef>`: what it says waits for its part's next measure.
fn core_staff_def<'py>(
    held: &Bound<'py, PyAny>,
    slurBundle: &Bound<'py, PyAny>,
    next_thing: &Bound<'py, PyDict>,
    core: &mut Core<'py>,
) -> PyResult<()> {
    let py = held.py();
    let n = get(held, "n")?;
    if n.is_none() {
        return warn(
            py,
            filled(UNIMPLEMENTED_IMPORT, &["<staffDef>".into(), "@n".into()]),
        );
    }
    let meter = class(py, "meter", "TimeSignature")?;
    let defined = named(py, "staffDefFromElement")?.call1((held, slurBundle))?;
    for each in defined.call_method0("values")?.try_iter()? {
        let each = each?;
        if each.is_instance(&meter)? {
            core.active_meter = each.clone();
        }
        list_at(next_thing, &n)?.call_method1("append", (each,))?;
    }
    Ok(())
}

/// A `<section>` inside: read, then what waited put at the start of each
/// part's first measure, its instrument before the measure.
fn core_section<'py>(
    held: &Bound<'py, PyAny>,
    allPartNs: &Bound<'py, PyAny>,
    slurBundle: &Bound<'py, PyAny>,
    own: &Bound<'py, PyAny>,
    parsed: &Bound<'py, PyDict>,
    next_thing: &Bound<'py, PyDict>,
    core: &mut Core<'py>,
) -> PyResult<()> {
    let py = held.py();
    let pairs = [
        ("activeMeter", core.active_meter.clone()),
        ("nextMeasureLeft", core.next_left.clone()),
        ("backupMeasureNum", core.backup.clone()),
        ("slurBundle", slurBundle.clone()),
    ];
    let result =
        named(py, "sectionFromElement")?.call((held, allPartNs), Some(&keywords(py, &pairs)?))?;
    let local = result.get_item(0)?;
    core.active_meter = result.get_item(1)?;
    core.next_left = result.get_item(2)?;
    core.backup = result.get_item(3)?;

    let instrument = class(py, "instrument", "Instrument")?;
    let streams = class(py, "stream", "Stream")?;
    for pair in local.call_method0("items")?.try_iter()? {
        let pair = pair?;
        let n = pair.get_item(0)?;
        let each_list = pair.get_item(1)?;
        let waiting = list_at(next_thing, &n)?;
        if waiting.is_truthy()? {
            // The instrument first, before the first measure.
            let mut found = None;
            for (place, each) in waiting.try_iter()?.enumerate() {
                let each = each?;
                if each.is_instance(&instrument)? {
                    found = Some((place, each));
                    break;
                }
            }
            if let Some((place, made)) = found.filter(|(_, made)| made.is_truthy().unwrap_or(false))
            {
                each_list.call_method1("insert", (0, made))?;
                waiting.del_item(place)?;
            }
            for each in each_list.try_iter()? {
                let each = each?;
                if !each.is_instance(&streams)? {
                    continue;
                }
                for thing in waiting.try_iter()? {
                    each.call_method1("insert", (0.0, thing?))?;
                }
                break;
            }
            next_thing.set_item(&n, PyList::empty(py))?;
        }
        if own.eq(tag("section"))? {
            for each in each_list.try_iter()? {
                list_at(parsed, &n)?.call_method1("append", (each?,))?;
            }
        } else if own.eq(tag("score"))? {
            list_at(parsed, &n)?.call_method1("append", (each_list,))?;
        }
    }
    Ok(())
}

/// music21's `sectionFromElement`.
#[pyfunction]
fn sectionFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    allPartNs: &Bound<'py, PyAny>,
    activeMeter: &Bound<'py, PyAny>,
    nextMeasureLeft: &Bound<'py, PyAny>,
    backupMeasureNum: &Bound<'py, PyAny>,
    slurBundle: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    debug(py, "*** processing a <section>".to_owned())?;
    let pairs = [
        ("activeMeter", activeMeter.clone()),
        ("nextMeasureLeft", nextMeasureLeft.clone()),
        ("backupMeasureNum", backupMeasureNum.clone()),
    ];
    named(py, "sectionScoreCore")?.call((elem, allPartNs, slurBundle), Some(&keywords(py, &pairs)?))
}

/// music21's `scoreFromElement`: a `<score>` as a score of parts, its
/// slurs after them.
#[pyfunction]
fn scoreFromElement<'py>(
    elem: &Bound<'py, PyAny>,
    slurBundle: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = elem.py();
    debug(py, "*** processing a <score>".to_owned())?;
    let all_parts = named(py, "allPartsPresent")?.call1((elem,))?;
    let bundle = keywords(py, &[("slurBundle", slurBundle.clone())])?;
    let parsed = named(py, "sectionScoreCore")?
        .call((elem, &all_parts), Some(&bundle))?
        .get_item(0)?;

    debug(py, "*** making the Score".to_owned())?;
    let part = class(py, "stream", "Part")?;
    let mut made = Vec::new();
    for n in all_parts.try_iter()? {
        let n = n?;
        let each = part.call0()?;
        each.setattr("atSoundingPitch", false)?;
        for thing in parsed.get_item(&n)?.try_iter()? {
            each.call_method1("append", (thing?,))?;
        }
        made.push(each);
    }
    let score = class(py, "stream", "Score")?.call1((PyList::new(py, made)?,))?;
    let slurs = PyList::new(py, slurBundle.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
    score.call_method1("append", (slurs,))?;
    Ok(score)
}

// ------------------------------------------------------------ the module

/// A music21 class of the package's own, for a table that names it.
fn articulation_class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Some(class) = crate::installed_class(py, "music21.articulations", name) {
        return Ok(class);
    }
    crate::provide::extension(py)?.getattr(name)
}

/// A table of the crate's as a dict.
fn table<'py, T>(py: Python<'py>, rows: &[(&str, T)]) -> PyResult<Bound<'py, PyDict>>
where
    T: for<'a> IntoPyObject<'a> + Copy,
{
    let dict = PyDict::new(py);
    for (key, value) in rows {
        dict.set_item(*key, *value)?;
    }
    Ok(dict)
}

/// [`table`] with what an attribute left out stands for under `None`.
fn table_or<'py, T>(
    py: Python<'py>,
    rows: &[(&str, T)],
    missing: Option<T>,
) -> PyResult<Bound<'py, PyDict>>
where
    T: for<'a> IntoPyObject<'a> + Copy,
{
    let dict = table(py, rows)?;
    dict.set_item(py.None(), missing)?;
    Ok(dict)
}

/// music21's `mei.base`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<MeiToM21Converter>()?;
        for (exception, name) in [
            (py.get_type::<MeiValidityError>(), "MeiValidityError"),
            (py.get_type::<MeiValueError>(), "MeiValueError"),
            (py.get_type::<MeiAttributeError>(), "MeiAttributeError"),
            (py.get_type::<MeiElementError>(), "MeiElementError"),
        ] {
            exception.setattr("__module__", MODULE)?;
            m.add(name, exception)?;
        }
        m.add(
            "environLocal",
            crate::environment::module(py)?
                .getattr("Environment")?
                .call1(("mei.base",))?,
        )?;

        // The constants and tables.
        m.add("_XMLID", XMLID)?;
        m.add("MEI_NS", MEI_NS)?;
        let ignored: Vec<String> = IGNORE_UNPROCESSED.iter().map(|name| tag(name)).collect();
        m.add("_IGNORE_UNPROCESSED", PyTuple::new(py, ignored)?)?;
        for (name, message) in [
            ("_TEST_FAILS", TEST_FAILS),
            ("_INVALID_XML_DOC", INVALID_XML_DOC),
            ("_WRONG_ROOT_ELEMENT", WRONG_ROOT_ELEMENT),
            ("_UNKNOWN_TAG", UNKNOWN_TAG),
            ("_UNEXPECTED_ATTR_VALUE", UNEXPECTED_ATTR_VALUE),
            ("_SEEMINGLY_NO_PARTS", SEEMINGLY_NO_PARTS),
            ("_MISSING_VOICE_ID", MISSING_VOICE_ID),
            ("_CANNOT_FIND_XMLID", CANNOT_FIND_XMLID),
            ("_MISSING_TUPLET_DATA", MISSING_TUPLET_DATA),
            ("_UNIMPLEMENTED_IMPORT", UNIMPLEMENTED_IMPORT),
            ("_UNPROCESSED_SUBELEMENT", UNPROCESSED_SUBELEMENT),
            ("_MISSED_DATE", MISSED_DATE),
            ("_BAD_VERSE_NUMBER", BAD_VERSE_NUMBER),
        ] {
            m.add(name, message)?;
        }
        m.add(
            "_ACCID_ATTR_DICT",
            table_or(py, &step::WRITTEN_ACCIDENTALS, None)?,
        )?;
        m.add(
            "_ACCID_GES_ATTR_DICT",
            table_or(py, &step::GESTURAL_ACCIDENTALS, None)?,
        )?;
        m.add(
            "_DUR_ATTR_DICT",
            table_or(py, &step::DURATIONS, Some(step::NO_DURATION))?,
        )?;
        m.add("_BAR_ATTR_DICT", table(py, &step::BARLINES)?)?;
        m.add(
            "_MEI_UNITDUR_TO_TREMOLO_MARKS",
            table(py, &step::TREMOLO_MARKS)?,
        )?;
        let articulations = PyDict::new(py);
        for (name, class) in step::ARTICULATIONS {
            articulations.set_item(name, articulation_class(py, class)?)?;
        }
        m.add("_ARTIC_ATTR_DICT", articulations)?;
        m.add("_MUSIC21_IMPORTS", PyTuple::new(py, IMPORTS)?)?;

        // The steps.
        macro_rules! add {
            ($($function:ident),* $(,)?) => {
                $(m.add_function(wrap_pyfunction!($function, &m)?)?;)*
            };
        }
        add!(
            safePitch,
            makeDuration,
            allPartsPresent,
            attr_translator,
            accidental_from_attr,
            accid_ges_from_attr,
            ql_duration_from_attr,
            articulation_from_attr,
            make_artic_list,
            get_octave_shift,
            sharps_from_attr,
            pp_slurs,
            pp_ties,
            pp_beams,
            pp_tuplets,
            pp_conclude,
            process_embedded,
            time_sig_from_attrs,
            key_sig_from_attrs,
            transposition_from_attrs,
            barline_from_attr,
            tie_from_attr,
            addSlurs,
            beamTogether,
            removeOctothorpe,
            makeMetadata,
            metaSetTitle,
            metaSetComposer,
            metaSetDate,
            getVoiceId,
            scaleToTuplet,
            guess_tuplets,
            scoreDefFromElement,
            staffGrpFromElement,
            staffDefFromElement,
            dotFromElement,
            articFromElement,
            accidFromElement,
            sylFromElement,
            verseFromElement,
            noteFromElement,
            restFromElement,
            mRestFromElement,
            spaceFromElement,
            mSpaceFromElement,
            chordFromElement,
            clefFromElement,
            instrDefFromElement,
            beamFromElement,
            barLineFromElement,
            tupletFromElement,
            bTremFromElement,
            layerFromElement,
            staffFromElement,
            correct_m_rest_durs,
            make_barlines,
            measureFromElement,
            sectionScoreCore,
            sectionFromElement,
            scoreFromElement,
        );
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `mei` package: the converter, which is all it names besides
/// `base`.
pub(crate) fn package(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_PACKAGE.get_or_try_init(py, || {
        let m = PyModule::new(py, PACKAGE)?;
        m.add(
            "MeiToM21Converter",
            module(py)?.getattr("MeiToM21Converter")?,
        )?;
        m.add("__all__", vec!["MeiToM21Converter", "base"])?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

#[cfg(test)]
mod tests {
    use super::filled;

    #[test]
    fn a_message_is_filled_in_order() {
        assert_eq!(
            filled(
                "Importing {} without {} is not yet supported.",
                &["<tie>".into(), "@n".into()]
            ),
            "Importing <tie> without @n is not yet supported."
        );
    }
}
