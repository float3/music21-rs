//! music21's `musicxml` package: the package itself, `xmlObjects`, `helpers`
//! and `partStaffExporter`. The writer, `m21ToXml`, is in [`crate::m21toxml`].
//!
//! The rules (which ids are valid, how measure numbers sort, which marks a
//! tag stands for) are the crate's `musicxml` module's; this is the glue that
//! gives them music21's names and works on `xml.etree.ElementTree` elements,
//! which is what music21's exporter and reader pass about.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyUserWarning;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyModule, PyString, PyTuple};

/// The package.
const PACKAGE: &str = "music21.musicxml";

/// The modules music21 keeps these in.
const OBJECTS: &str = "music21.musicxml.xmlObjects";
const HELPERS: &str = "music21.musicxml.helpers";
const PART_STAFF: &str = "music21.musicxml.partStaffExporter";

static BUILT_PACKAGE: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static BUILT_OBJECTS: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static BUILT_HELPERS: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static BUILT_PART_STAFF: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    MusicXMLException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    MusicXMLExportException,
    MusicXMLException
);
pyo3::create_exception!(
    music21_rs_facade,
    MusicXMLImportException,
    MusicXMLException
);
pyo3::create_exception!(music21_rs_facade, MusicXMLWarning, PyUserWarning);

/// `xml.etree.ElementTree`.
pub(crate) fn element_tree(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    py.import("xml.etree.ElementTree")
}

/// A refusal of the writer's, as music21's `MusicXMLExportException`.
pub(crate) fn export_error(error: &music21::Error) -> PyErr {
    MusicXMLExportException::new_err(crate::pitch::message(error))
}

// ---------------------------------------------------------------- package

/// `music21.musicxml`: its `__all__` names the modules of it the package
/// builds, as music21's names the ones it imports.
pub(crate) fn package(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_PACKAGE.get_or_try_init(py, || {
        let m = PyModule::new(py, PACKAGE)?;
        let prefix = format!("{PACKAGE}.");
        let children: Vec<&str> = crate::BUILT_MODULES
            .iter()
            .filter_map(|name| name.strip_prefix(&prefix))
            .collect();
        m.add("__all__", children)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

// ------------------------------------------------------------- xmlObjects

/// music21's `xmlObjects.ARTICULATION_MARKS`: each MusicXML tag beside the
/// articulation class it reads as.
const ARTICULATION_MARKS: [(&str, &str); 16] = [
    ("accent", "Accent"),
    ("strong-accent", "StrongAccent"),
    ("staccato", "Staccato"),
    ("staccatissimo", "Staccatissimo"),
    ("spiccato", "Spiccato"),
    ("tenuto", "Tenuto"),
    ("detached-legato", "DetachedLegato"),
    ("scoop", "Scoop"),
    ("plop", "Plop"),
    ("doit", "Doit"),
    ("falloff", "Falloff"),
    ("breath-mark", "BreathMark"),
    ("caesura", "Caesura"),
    ("stress", "Stress"),
    ("unstress", "Unstress"),
    ("other-articulation", "Articulation"),
];

/// music21's `xmlObjects.TECHNICAL_MARKS`. Hammer-ons and pull-offs are
/// spanners, written by the exporter's spanner code instead.
const TECHNICAL_MARKS: [(&str, &str); 21] = [
    ("up-bow", "UpBow"),
    ("down-bow", "DownBow"),
    ("harmonic", "StringHarmonic"),
    ("open-string", "OpenString"),
    ("thumb-position", "StringThumbPosition"),
    ("fingering", "Fingering"),
    ("pluck", "FrettedPluck"),
    ("double-tongue", "DoubleTongue"),
    ("triple-tongue", "TripleTongue"),
    ("stopped", "Stopped"),
    ("snap-pizzicato", "SnapPizzicato"),
    ("string", "StringIndication"),
    ("bend", "FretBend"),
    ("tap", "FretTap"),
    ("fret", "FretIndication"),
    ("heel", "OrganHeel"),
    ("toe", "OrganToe"),
    ("fingernails", "HarpFingerNails"),
    ("handbell", "HandbellIndication"),
    ("other-technical", "TechnicalIndication"),
    // Not music21's forward table's: the reversed one adds it.
    ("harmonic", "Harmonic"),
];

/// music21's `xmlObjects.ORNAMENT_MARKS`: the ornaments that are not
/// spanners, by tag.
const ORNAMENT_MARKS: [(&str, &str); 10] = [
    ("trill-mark", "Trill"),
    ("turn", "Turn"),
    ("delayed-turn", "Turn"),
    ("inverted-turn", "InvertedTurn"),
    ("delayed-inverted-turn", "InvertedTurn"),
    ("shake", "Shake"),
    ("mordent", "Mordent"),
    ("inverted-mordent", "InvertedMordent"),
    ("schleifer", "Schleifer"),
    ("other-ornament", "Ornament"),
];

/// The class of music21's a mark table names.
fn marked<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    crate::class_of(py, module, name)
}

/// An `OrderedDict` of tag to class, as music21's tables are.
fn ordered<'py>(
    py: Python<'py>,
    module: &str,
    marks: &[(&str, &str)],
) -> PyResult<Bound<'py, PyAny>> {
    let table = py.import("collections")?.getattr("OrderedDict")?.call0()?;
    for (tag, name) in marks {
        table.set_item(*tag, marked(py, module, name)?)?;
    }
    Ok(table)
}

/// The reversed table, class to tag, leaving out the classes music21's
/// leaves out and moving to the end the ones its moves there, so that a
/// subclass is looked for before its base.
fn reversed<'py>(
    py: Python<'py>,
    module: &str,
    marks: &[(&str, &str)],
    dropped: &[&str],
    moved: &[&str],
) -> PyResult<Bound<'py, PyAny>> {
    let table = py.import("collections")?.getattr("OrderedDict")?.call0()?;
    for (tag, name) in marks {
        if dropped.contains(name) || moved.contains(name) {
            continue;
        }
        table.set_item(marked(py, module, name)?, *tag)?;
    }
    for name in moved {
        let tag = marks
            .iter()
            .find(|(_, known)| known == name)
            .map(|(tag, _)| *tag)
            .unwrap_or_default();
        table.set_item(marked(py, module, name)?, tag)?;
    }
    Ok(table)
}

/// music21's `MusicXMLException.__init__`: the message, and an empty
/// measure number and part name for whoever catches it to fill in.
#[pyfunction]
fn exception_init(slf: &Bound<'_, PyAny>, message: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    py.get_type::<pyo3::exceptions::PyException>()
        .getattr("__init__")?
        .call1((slf, message))?;
    slf.setattr("measureNumber", "")?;
    slf.setattr("partName", "")?;
    Ok(())
}

/// music21's `MusicXMLException.__str__`: the message, after the part and
/// measure it was raised in where those are known.
#[pyfunction]
fn exception_str(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = slf.py();
    let message: String = py
        .get_type::<pyo3::exceptions::PyException>()
        .getattr("__str__")?
        .call1((slf,))?
        .extract()?;
    let measure = slf.getattr("measureNumber")?.str()?.to_string();
    let part = slf.getattr("partName")?.str()?.to_string();
    if measure.is_empty() && part.is_empty() {
        return Ok(message);
    }
    Ok(format!("In part ({part}), measure ({measure}): {message}"))
}

/// music21's `yesNoToBoolean`: `'yes'` or `True` is true, all else false.
#[pyfunction]
fn yesNoToBoolean(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(text.to_str()? == "yes");
    }
    Ok(value.is(pyo3::types::PyBool::new(value.py(), true).as_any()))
}

/// music21's `booleanToYesNo`: `'yes'` for anything true, `'no'` else.
#[pyfunction]
fn booleanToYesNo(value: &Bound<'_, PyAny>) -> PyResult<&'static str> {
    Ok(if value.is_truthy()? { "yes" } else { "no" })
}

/// music21's `fractionToPercent`: a share as a whole percent, truncated.
#[pyfunction]
fn fractionToPercent(value: f64) -> String {
    music21::musicxml::fraction_to_percent(value)
}

/// music21's `isValidXSDID`: whether text may be an element's `id`.
#[pyfunction]
fn isValidXSDID(text: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Ok(text) = text.cast::<PyString>() else {
        return Ok(false);
    };
    Ok(music21::musicxml::is_valid_xsd_id(text.to_str()?))
}

/// `music21.musicxml.xmlObjects`.
pub(crate) fn objects_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_OBJECTS.get_or_try_init(py, || {
        let m = PyModule::new(py, OBJECTS)?;

        // The exceptions, with music21's message that names where it was.
        for (exception, name) in [
            (py.get_type::<MusicXMLException>(), "MusicXMLException"),
            (
                py.get_type::<MusicXMLExportException>(),
                "MusicXMLExportException",
            ),
            (
                py.get_type::<MusicXMLImportException>(),
                "MusicXMLImportException",
            ),
            (py.get_type::<MusicXMLWarning>(), "MusicXMLWarning"),
        ] {
            exception.setattr("__module__", OBJECTS)?;
            m.add(name, exception)?;
        }
        let partial = py.import("functools")?.getattr("partialmethod")?;
        let base = py.get_type::<MusicXMLException>();
        base.setattr(
            "__init__",
            partial.call1((wrap_pyfunction!(exception_init, &m)?,))?,
        )?;
        base.setattr(
            "__str__",
            partial.call1((wrap_pyfunction!(exception_str, &m)?,))?,
        )?;

        // The tables of marks.
        const ARTICULATIONS: &str = "music21.articulations";
        const EXPRESSIONS: &str = "music21.expressions";
        m.add(
            "DYNAMIC_MARKS",
            PyList::new(py, music21::musicxml::DYNAMIC_MARKS)?,
        )?;
        m.add(
            "ARTICULATION_MARKS",
            ordered(py, ARTICULATIONS, &ARTICULATION_MARKS)?,
        )?;
        m.add(
            "ARTICULATION_MARKS_REV",
            reversed(
                py,
                ARTICULATIONS,
                &ARTICULATION_MARKS,
                &["Articulation"],
                &["Staccato", "Accent"],
            )?,
        )?;
        let forward = &TECHNICAL_MARKS[..TECHNICAL_MARKS.len() - 1];
        m.add("TECHNICAL_MARKS", ordered(py, ARTICULATIONS, forward)?)?;
        m.add(
            "TECHNICAL_MARKS_REV",
            reversed(
                py,
                ARTICULATIONS,
                &TECHNICAL_MARKS,
                &["TechnicalIndication"],
                &[],
            )?,
        )?;
        let ornaments = PyDict::new(py);
        for (tag, name) in ORNAMENT_MARKS {
            ornaments.set_item(tag, marked(py, EXPRESSIONS, name)?)?;
        }
        m.add("ORNAMENT_MARKS", ornaments)?;
        m.add(
            "STYLE_ATTRIBUTES_YES_NO_TO_BOOL",
            PyTuple::new(py, ["hideObjectOnPrint"])?,
        )?;

        m.add_function(wrap_pyfunction!(yesNoToBoolean, &m)?)?;
        m.add_function(wrap_pyfunction!(booleanToYesNo, &m)?)?;
        m.add_function(wrap_pyfunction!(fractionToPercent, &m)?)?;
        m.add_function(wrap_pyfunction!(isValidXSDID, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

// ---------------------------------------------------------------- helpers

/// How deep `indent` indents each level.
const INDENT: &str = "  ";

/// Whether an element's text or tail is missing or all whitespace.
fn blank(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if value.is_none() {
        return Ok(true);
    }
    Ok(value.str()?.to_str()?.trim().is_empty())
}

/// music21's `indent`: whitespace put into an element's text and tails, in
/// place, so that it prints a child to a line, two spaces a level.
#[pyfunction]
#[pyo3(signature = (elem, level = 0))]
pub(crate) fn indent(elem: &Bound<'_, PyAny>, level: usize) -> PyResult<()> {
    let here = format!("\n{}", INDENT.repeat(level));
    let children: Vec<Bound<'_, PyAny>> = elem.try_iter()?.collect::<PyResult<_>>()?;

    // A leaf: only its tail, and only below the top.
    if children.is_empty() {
        if level > 0 && blank(&elem.getattr("tail")?)? {
            elem.setattr("tail", &here)?;
        }
        return Ok(());
    }

    // An element with children: its text indents the first child, each
    // child's tail the next, and the last child's tail closes it.
    if blank(&elem.getattr("text")?)? {
        elem.setattr("text", format!("{here}{INDENT}"))?;
    }
    if blank(&elem.getattr("tail")?)? {
        elem.setattr("tail", &here)?;
    }
    for child in &children {
        indent(child, level + 1)?;
    }
    if let Some(last) = children.last() {
        last.setattr("tail", &here)?;
    }
    Ok(())
}

/// music21's `dumpString`: an element as indented text, attributes sorted
/// where there are several, with no trailing whitespace. The element is
/// copied first unless `noCopy` is true.
#[pyfunction]
#[pyo3(signature = (obj, *, noCopy = false))]
pub(crate) fn dumpString(obj: &Bound<'_, PyAny>, noCopy: bool) -> PyResult<String> {
    let py = obj.py();
    let element = if noCopy {
        obj.clone()
    } else {
        py.import("copy")?.getattr("deepcopy")?.call1((obj,))?
    };
    indent(&element, 0)?;

    // music21 sorts the attributes of each element that has several.
    for each in element.call_method0("iter")?.try_iter()? {
        let attributes = each?.getattr("attrib")?;
        if attributes.len()? < 2 {
            continue;
        }
        let sorted = py
            .import("builtins")?
            .getattr("sorted")?
            .call1((attributes.call_method0("items")?,))?;
        attributes.call_method0("clear")?;
        attributes.call_method1("update", (sorted,))?;
    }
    let keywords = PyDict::new(py);
    keywords.set_item("encoding", "unicode")?;
    let text: String = element_tree(py)?
        .getattr("tostring")?
        .call((element,), Some(&keywords))?
        .extract()?;
    Ok(text.trim_end().to_string())
}

/// music21's `dump`: prints what `dumpString` gives.
#[pyfunction]
pub(crate) fn dump(obj: &Bound<'_, PyAny>) -> PyResult<()> {
    let text = dumpString(obj, false)?;
    obj.py()
        .import("builtins")?
        .getattr("print")?
        .call1((text,))?;
    Ok(())
}

/// The indices of `root`'s children whose tag is one of `tags`.
fn tagged(root: &Bound<'_, PyAny>, tags: &Bound<'_, PyAny>) -> PyResult<Vec<usize>> {
    let mut found = Vec::new();
    for (index, child) in root
        .call_method1("findall", ("*",))?
        .try_iter()?
        .enumerate()
    {
        if tags.contains(child?.getattr("tag")?)? {
            found.push(index);
        }
    }
    Ok(found)
}

/// music21's `insertBeforeElements`: `insert` put into `root` before the
/// first child tagged one of `tagList`, or at the end.
#[pyfunction]
#[pyo3(signature = (root, insert, tagList = None))]
fn insertBeforeElements(
    root: &Bound<'_, PyAny>,
    insert: &Bound<'_, PyAny>,
    tagList: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let tags = match tagList {
        Some(tags) if tags.is_truthy()? => tags,
        _ => {
            root.call_method1("append", (insert,))?;
            return Ok(());
        }
    };
    let at = tagged(root, tags)?
        .into_iter()
        .next()
        .unwrap_or(root.len()?);
    root.call_method1("insert", (at, insert))?;
    Ok(())
}

/// music21's `insertAfterElements`: `insert` put into `root` after the
/// last child tagged one of `tagList`, or first.
#[pyfunction]
#[pyo3(signature = (root, insert, tagList = None))]
fn insertAfterElements(
    root: &Bound<'_, PyAny>,
    insert: &Bound<'_, PyAny>,
    tagList: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let mut at = 0;
    if let Some(tags) = tagList.filter(|tags| tags.is_truthy().unwrap_or(false)) {
        at = tagged(root, tags)?.last().map_or(0, |last| last + 1);
    }
    root.call_method1("insert", (at, insert))?;
    Ok(())
}

/// music21's `measureNumberComesBefore`.
#[pyfunction]
fn measureNumberComesBefore(mNum1: &str, mNum2: &str) -> bool {
    music21::musicxml::measure_number_comes_before(mNum1, mNum2)
}

/// music21's `isFullMeasureRest`: a rest said to fill its measure, or left
/// to decide and as long as the bar of the meter in force.
#[pyfunction]
fn isFullMeasureRest(r: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = r.py();
    let full = r.getattr("fullMeasure")?;
    if full.is(pyo3::types::PyBool::new(py, true).as_any()) || full.eq("always")? {
        return Ok(true);
    }
    if !full.eq("auto")? {
        return Ok(false);
    }
    let meter = crate::class_of(py, "music21.meter", "TimeSignature")?;
    let context = r.call_method1("getContextByClass", (meter,))?;
    if context.is_none() || !context.is_truthy()? {
        return Ok(false);
    }
    let bar: f64 = context
        .getattr("barDuration")?
        .getattr("quarterLength")?
        .extract()?;
    let own: f64 = r.getattr("duration")?.getattr("quarterLength")?.extract()?;
    Ok(bar == own)
}

/// music21's `synchronizeIdsToM21`: an element's `id` attribute, where it
/// has one, made the object's `id`.
#[pyfunction]
pub(crate) fn synchronizeIdsToM21(
    element: &Bound<'_, PyAny>,
    m21Object: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let found = element.call_method1("get", ("id",))?;
    if found.is_none() || !found.is_truthy()? {
        return Ok(());
    }
    m21Object.setattr("id", found)
}

/// music21's `synchronizeIdsToXML`: an object's `id` written as the
/// element's `id` attribute, where it is one MusicXML allows.
#[pyfunction]
pub(crate) fn synchronizeIdsToXML(
    element: &Bound<'_, PyAny>,
    m21Object: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = element.py();
    let proto = crate::class_of(py, "music21.prebase", "ProtoM21Object")?;
    if !m21Object.is_instance(&proto)? || !m21Object.hasattr("id")? {
        return Ok(());
    }
    let id = m21Object.getattr("id")?;
    let Ok(text) = id.cast::<PyString>() else {
        return Ok(());
    };
    if !music21::musicxml::is_valid_xsd_id(text.to_str()?) {
        return Ok(());
    }
    element.call_method1("set", ("id", text))?;
    Ok(())
}

/// The attribute name music21 reads for a hyphenated MusicXML name:
/// `page-number` is `pageNumber`.
fn attribute_name(xml: &str, given: Option<String>) -> PyResult<String> {
    match given {
        Some(name) => Ok(name),
        None => music21::common::stringtools::hyphen_to_camel_case(xml, "-")
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string())),
    }
}

/// music21's `setM21AttributeFromAttribute`: an attribute of the element,
/// where it has it, set on the object, through `transform` if given.
#[pyfunction]
#[pyo3(signature = (m21El, xmlEl, xmlAttributeName, attributeName = None, transform = None))]
pub(crate) fn setM21AttributeFromAttribute(
    m21El: &Bound<'_, PyAny>,
    xmlEl: &Bound<'_, PyAny>,
    xmlAttributeName: &str,
    attributeName: Option<String>,
    transform: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let mut value = xmlEl.call_method1("get", (xmlAttributeName,))?;
    if value.is_none() {
        return Ok(());
    }
    if let Some(transform) = transform.filter(|given| !given.is_none()) {
        value = transform.call1((value,))?;
    }
    let name = attribute_name(xmlAttributeName, attributeName)?;
    m21El.setattr(name.as_str(), value)
}

/// music21's `setXMLAttributeFromAttribute`: an attribute of the object,
/// where it is set, written on the element as text, through `transform`
/// if given.
#[pyfunction]
#[pyo3(signature = (m21El, xmlEl, xmlAttributeName, attributeName = None, transform = None))]
pub(crate) fn setXMLAttributeFromAttribute(
    m21El: &Bound<'_, PyAny>,
    xmlEl: &Bound<'_, PyAny>,
    xmlAttributeName: &str,
    attributeName: Option<String>,
    transform: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let name = attribute_name(xmlAttributeName, attributeName)?;
    let mut value = match m21El.getattr(name.as_str()) {
        Ok(value) if !value.is_none() => value,
        _ => return Ok(()),
    };
    if let Some(transform) = transform.filter(|given| !given.is_none()) {
        value = transform.call1((value,))?;
    }
    xmlEl.call_method1("set", (xmlAttributeName, value.str()?))?;
    Ok(())
}

/// `music21.musicxml.helpers`.
pub(crate) fn helpers_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_HELPERS.get_or_try_init(py, || {
        let m = PyModule::new(py, HELPERS)?;
        macro_rules! add {
            ($($name:ident),* $(,)?) => {
                $(m.add_function(wrap_pyfunction!($name, &m)?)?;)*
            };
        }
        add!(
            dumpString,
            dump,
            indent,
            insertBeforeElements,
            insertAfterElements,
            measureNumberComesBefore,
            isFullMeasureRest,
            synchronizeIdsToM21,
            synchronizeIdsToXML,
            setM21AttributeFromAttribute,
            setXMLAttributeFromAttribute,
        );
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

// ------------------------------------------------------ partStaffExporter

/// music21's `PartStaffExporterMixin`, which `ScoreExporter` is built on,
/// with no layout of its own so that it can be a second base. Joining
/// several staves into one part is done by the crate's writer as it writes
/// the score, so the mixin's steps are not members of it.
pub(crate) fn part_staff_mixin(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    part_staff_module(py)?.getattr("PartStaffExporterMixin")
}

/// `music21.musicxml.partStaffExporter`.
pub(crate) fn part_staff_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT_PART_STAFF.get_or_try_init(py, || {
        let m = PyModule::new(py, PART_STAFF)?;
        let namespace = PyDict::new(py);
        namespace.set_item("__slots__", PyTuple::empty(py))?;
        namespace.set_item("__module__", PART_STAFF)?;
        namespace.set_item(
            "__doc__",
            "What joins a score's staves into parts; the writer does it.",
        )?;
        let mixin = py.get_type::<pyo3::types::PyType>().call1((
            "PartStaffExporterMixin",
            (py.get_type::<pyo3::types::PyAny>(),),
            namespace,
        ))?;
        m.add("PartStaffExporterMixin", mixin)?;
        m.add_function(wrap_pyfunction!(addStaffTags, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

// ------------------------------------------------------ ElementTree trees

/// An `xml.etree.ElementTree` element, as a tree the crate's staff-joining
/// steps work on. Each child it hands back is the element itself, so what
/// the steps change is changed in the tree Python holds.
pub(crate) struct Et(pub(crate) Py<PyAny>);

/// A Python error raised inside a step, as the crate's.
fn step_error(error: PyErr) -> music21::Error {
    use music21::error::{FormatError, FormatErrorKind};
    music21::Error::MusicXml(Box::new(FormatError::new(
        FormatErrorKind::Invalid,
        error.to_string(),
    )))
}

impl Et {
    /// Runs a call on the element with the interpreter held.
    fn call<R>(&self, run: impl FnOnce(&Bound<'_, PyAny>) -> PyResult<R>) -> music21::Result<R> {
        Python::attach(|py| run(self.0.bind(py))).map_err(step_error)
    }
}

impl music21::musicxml::XmlTree for Et {
    fn tag(&self) -> Option<String> {
        // A comment's tag is `ElementTree.Comment`, a function.
        self.call(|element| Ok(element.getattr("tag")?.extract::<String>().ok()))
            .ok()
            .flatten()
    }

    fn attribute(&self, name: &str) -> Option<String> {
        self.call(|element| element.call_method1("get", (name,))?.extract())
            .ok()
            .flatten()
    }

    fn set_attribute(&mut self, name: &str, value: &str) -> music21::Result<()> {
        self.call(|element| element.call_method1("set", (name, value)).map(|_| ()))
    }

    fn text(&self) -> Option<String> {
        self.call(|element| element.getattr("text")?.extract())
            .ok()
            .flatten()
    }

    fn set_text(&mut self, text: &str) -> music21::Result<()> {
        self.call(|element| element.setattr("text", text))
    }

    fn len(&self) -> usize {
        self.call(|element| element.len()).unwrap_or(0)
    }

    fn child(&self, index: usize) -> Option<Self> {
        self.call(|element| Ok(Et(element.get_item(index)?.unbind())))
            .ok()
    }

    fn with_child<R>(&mut self, index: usize, change: impl FnOnce(&mut Self) -> R) -> Option<R> {
        let mut child = self.child(index)?;
        Some(change(&mut child))
    }

    fn insert(&mut self, index: usize, child: Self) -> music21::Result<()> {
        self.call(|element| element.call_method1("insert", (index, child.0)).map(|_| ()))
    }

    fn remove(&mut self, index: usize) -> music21::Result<()> {
        self.call(|element| {
            let child = element.get_item(index)?;
            element.call_method1("remove", (child,)).map(|_| ())
        })
    }

    fn element(&self, tag: &str) -> music21::Result<Self> {
        Python::attach(|py| {
            Ok(Et(element_tree(py)?
                .getattr("Element")?
                .call1((tag,))?
                .unbind()))
        })
        .map_err(step_error)
    }

    fn comment(&self, text: &str) -> music21::Result<Self> {
        Python::attach(|py| {
            Ok(Et(element_tree(py)?
                .getattr("Comment")?
                .call1((text,))?
                .unbind()))
        })
        .map_err(step_error)
    }

    fn deep_copy(&self) -> music21::Result<Self> {
        self.call(|element| {
            let copied = element
                .py()
                .import("copy")?
                .getattr("deepcopy")?
                .call1((element,))?;
            Ok(Et(copied.unbind()))
        })
    }
}

/// A step's refusal, as music21's `MusicXMLExportException`.
pub(crate) fn step_refused(error: &music21::Error) -> PyErr {
    export_error(error)
}

/// music21's `addStaffTags`: a `<staff>` in each child of a `<measure>`
/// whose tag is in `tagList`, before its beams, notations, lyrics and
/// sound.
#[pyfunction]
fn addStaffTags(
    measure: &Bound<'_, PyAny>,
    staffNumber: usize,
    tagList: Vec<String>,
) -> PyResult<()> {
    let tags: Vec<&str> = tagList.iter().map(String::as_str).collect();
    let mut tree = Et(measure.clone().unbind());
    music21::musicxml::add_staff_tags(&mut tree, staffNumber, &tags).map_err(|e| step_refused(&e))
}
