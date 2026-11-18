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

use crate::musicxml::MusicXMLImportException;
use crate::subconverters::music21_name;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{IntoPyDict, PyDict, PyList, PyModule, PyString, PyTuple};

/// The module music21 keeps these in.
const MODULE: &str = "music21.musicxml.xmlToM21";

/// music21's `NO_STAFF_ASSIGNED`: the staff of an element that names none.
const NO_STAFF_ASSIGNED: i32 = 0;

/// The tag joining a note to the chord of the note before it.
const CHORD: &str = "chord";

/// How many tuplets deep music21 keeps track of at once.
const TUPLET_DEPTH: usize = 7;

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// A crate error as music21's reader raises it.
fn import_error(error: music21::Error) -> PyErr {
    MusicXMLImportException::new_err(crate::pitch::message(&error))
}

/// An ElementTree element written back as text.
fn written(element: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = element.py();
    let keywords = PyDict::new(py);
    keywords.set_item("encoding", "unicode")?;
    crate::musicxml::element_tree(py)?
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

/// A measure's text read with the parser's divisions, inside the tuplets
/// the parser is in, as this package's `Measure`. The tuplets the measure
/// leaves open are the parser's from then on, as music21's `activeTuplets`
/// carry from one note to the next.
fn read_measure<'py>(slf: &Bound<'py, PyAny>, text: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let active = slf.getattr("activeTuplets")?;
    let mut context = step::MeasureContext {
        divisions: divisions_of(slf)?,
        ..step::MeasureContext::default()
    };
    for (place, held) in context.tuplets.iter_mut().enumerate() {
        let Ok(given) = active.get_item(place) else {
            break;
        };
        if let Ok(tuplet) = given.extract::<PyRef<'_, crate::duration::Tuplet>>() {
            *held = Some(tuplet.value());
        }
    }
    let read = step::read_measure(text, &mut context).map_err(import_error)?;

    // What is left open, written back into the parser's own list.
    for (place, held) in context.tuplets.iter().enumerate() {
        let object = match held {
            Some(tuplet) => crate::installed_new(
                py,
                "music21.duration",
                "Tuplet",
                crate::duration::Tuplet::wrap(*tuplet),
            )?
            .into_any(),
            None => py.None(),
        };
        if active.set_item(place, object).is_err() {
            break;
        }
    }
    crate::stream::from_crate(py, &read)
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

    /// music21's `setStyleAttributes`: each of the element's attributes in
    /// `musicXMLNames` written to the object's style under the name in
    /// `m21Names` beside it, or its own name in camel case; a number as a
    /// number, `yes` and `no` as booleans where music21 reads them so.
    #[staticmethod]
    #[pyo3(signature = (mxObject, m21Object, musicXMLNames, m21Names = None))]
    fn setStyleAttributes(
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
        musicXMLNames: &Bound<'_, PyAny>,
        m21Names: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let xml_names = names_of(musicXMLNames)?;
        let m21_names = match m21Names.filter(|names| !names.is_none()) {
            Some(given) => names_of(given)?,
            None => xml_names
                .iter()
                .map(|name| music21::common::stringtools::hyphen_to_camel_case(name, ""))
                .collect::<music21::Result<_>>()
                .map_err(import_error)?,
        };
        set_style(mxObject, m21Object, &xml_names, &m21_names)
    }

    /// music21's `setTextFormatting`: justification, decoration, rotation,
    /// spacing, language, direction and enclosure, then the print style.
    #[staticmethod]
    fn setTextFormatting(
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        set_style(mxObject, m21Object, TEXT_FORMATTING.0, TEXT_FORMATTING.1)?;
        Self::setPrintStyleAlign(mxObject, m21Object)
    }

    /// music21's `setLineStyle`: a line's shape, type and dashes.
    #[staticmethod]
    fn setLineStyle(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        let line_type = mxObject.call_method1("get", ("line-type",))?;
        if m21Object.hasattr("lineType")? && !line_type.is_none() {
            m21Object.setattr("lineType", line_type)?;
        }
        set_style(mxObject, m21Object, LINE_STYLE.0, LINE_STYLE.1)
    }

    /// music21's `setPrintObject`: `print-object="no"` hides the object.
    #[staticmethod]
    fn setPrintObject(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        let said: Option<String> = mxObject.call_method1("get", ("print-object",))?.extract()?;
        if said.as_deref() != Some("no") {
            return Ok(());
        }
        if m21Object.hasattr("style")? {
            return m21Object
                .getattr("style")?
                .setattr("hideObjectOnPrint", true);
        }
        // A slotted object takes none.
        let _ = m21Object.setattr("hideObjectOnPrint", true);
        Ok(())
    }

    /// music21's `setPrintStyleAlign`: the print style, then the alignment.
    #[staticmethod]
    fn setPrintStyleAlign(
        mxObject: &Bound<'_, PyAny>,
        m21Object: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        Self::setPrintStyle(mxObject, m21Object)?;
        set_style(mxObject, m21Object, ALIGN.0, ALIGN.1)
    }

    /// music21's `setPrintStyle`: position, font and colour.
    #[staticmethod]
    fn setPrintStyle(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::setPosition(mxObject, m21Object)?;
        Self::setFont(mxObject, m21Object)?;
        Self::setColor(mxObject, m21Object)
    }

    /// music21's `setColor`.
    #[staticmethod]
    fn setColor(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        set_style(mxObject, m21Object, &["color"], &["color"])
    }

    /// music21's `setFont`: family, style, size and weight.
    #[staticmethod]
    fn setFont(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        set_style(mxObject, m21Object, FONT.0, FONT.1)
    }

    /// music21's `setPosition`: the default and relative positions.
    #[staticmethod]
    fn setPosition(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        set_style(mxObject, m21Object, POSITION.0, POSITION.1)
    }

    /// music21's `setPlacement`: on the object where it has a
    /// `placement`, else on its style.
    #[staticmethod]
    fn setPlacement(mxObject: &Bound<'_, PyAny>, m21Object: &Bound<'_, PyAny>) -> PyResult<()> {
        let placement = mxObject.call_method1("get", ("placement",))?;
        if placement.is_none() {
            return Ok(());
        }
        if m21Object.hasattr("placement")? {
            return m21Object.setattr("placement", placement);
        }
        m21Object.getattr("style")?.setattr("placement", placement)
    }

    /// music21's `setEditorial`: a `<footnote>` or `<level>` kept as an
    /// editorial comment, a footnote where there is one.
    #[staticmethod]
    fn setEditorial(mxObj: &Bound<'_, PyAny>, m21Obj: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = mxObj.py();
        let footnote = mxObj.call_method1("find", ("footnote",))?;
        let level = mxObj.call_method1("find", ("level",))?;
        if footnote.is_none() && level.is_none() {
            return Ok(());
        }
        let comment = music21_name(py, "music21.editorial", "Comment")?.call0()?;
        if !footnote.is_none() {
            comment.setattr("text", footnote.getattr("text")?)?;
            comment.setattr("isFootnote", true)?;
            Self::setTextFormatting(&footnote, &comment)?;
        }
        if !level.is_none() {
            comment.setattr("levelInformation", level.getattr("text")?)?;
            let reference: Option<String> = level.call_method1("get", ("reference",))?.extract()?;
            if reference.as_deref() == Some("yes") {
                comment.setattr("isReference", true)?;
            }
        }
        let list = match comment.getattr("isFootnote")?.is_truthy()? {
            true => "footnotes",
            false => "comments",
        };
        m21Obj
            .getattr("editorial")?
            .getattr(list)?
            .call_method1("append", (comment,))?;
        Ok(())
    }

    /// music21's `xmlPrintToPageLayout`: the page a `<print>` starts.
    #[staticmethod]
    #[pyo3(signature = (mxPrint, inputM21 = None))]
    fn xmlPrintToPageLayout<'py>(
        mxPrint: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let read = step::read_print_page(&written(mxPrint)?).map_err(import_error)?;
        layout_into(
            mxPrint.py(),
            music21::layout::Layout::Page(read),
            inputM21,
            PAGE_LAYOUT,
        )
    }

    /// music21's `xmlPageLayoutToPageLayout`: a `<page-layout>`.
    #[staticmethod]
    #[pyo3(signature = (mxPageLayout, inputM21 = None))]
    fn xmlPageLayoutToPageLayout<'py>(
        mxPageLayout: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let read = step::read_page_layout(&written(mxPageLayout)?).map_err(import_error)?;
        layout_into(
            mxPageLayout.py(),
            music21::layout::Layout::Page(read),
            inputM21,
            PAGE_LAYOUT,
        )
    }

    /// music21's `xmlPrintToSystemLayout`: the system a `<print>` starts.
    #[staticmethod]
    #[pyo3(signature = (mxPrint, inputM21 = None))]
    fn xmlPrintToSystemLayout<'py>(
        mxPrint: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let read = step::read_print_system(&written(mxPrint)?).map_err(import_error)?;
        layout_into(
            mxPrint.py(),
            music21::layout::Layout::System(read),
            inputM21,
            SYSTEM_LAYOUT,
        )
    }

    /// music21's `xmlSystemLayoutToSystemLayout`: a `<system-layout>`.
    #[staticmethod]
    #[pyo3(signature = (mxSystemLayout, inputM21 = None))]
    fn xmlSystemLayoutToSystemLayout<'py>(
        mxSystemLayout: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let read = step::read_system_layout(&written(mxSystemLayout)?).map_err(import_error)?;
        layout_into(
            mxSystemLayout.py(),
            music21::layout::Layout::System(read),
            inputM21,
            SYSTEM_LAYOUT,
        )
    }

    /// music21's `xmlStaffLayoutToStaffLayout`: a `<staff-layout>`, kept
    /// too under its staff and place where the parser keeps them.
    #[pyo3(signature = (mxStaffLayout, inputM21 = None))]
    fn xmlStaffLayoutToStaffLayout<'py>(
        slf: &Bound<'py, Self>,
        mxStaffLayout: &Bound<'py, PyAny>,
        inputM21: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let read = step::read_staff_layout(&written(mxStaffLayout)?).map_err(import_error)?;
        let number = read.staff_number;
        let made = layout_into(
            py,
            music21::layout::Layout::Staff(read),
            inputM21,
            STAFF_LAYOUT,
        )?;
        let target = match inputM21.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => made.clone(),
        };
        if slf.hasattr("staffLayoutObjects")? && slf.hasattr("offsetMeasureNote")? {
            let key = (number.unwrap_or(1), slf.getattr("offsetMeasureNote")?);
            slf.getattr("staffLayoutObjects")?.set_item(key, target)?;
        }
        Ok(made)
    }
}

/// The names music21 maps from MusicXML attributes onto a style, as
/// (MusicXML's, music21's), for each group of attributes.
type StyleNames = (&'static [&'static str], &'static [&'static str]);

/// MusicXML's `%text-formatting`, less the print style.
const TEXT_FORMATTING: StyleNames = (
    &[
        "justify",
        "text-decoration",
        "text-rotation",
        "letter-spacing",
        "line-height",
        "lang",
        "text-direction",
        "enclosure",
    ],
    &[
        "justify",
        "textDecoration",
        "textRotation",
        "letterSpacing",
        "lineHeight",
        "language",
        "textDirection",
        "enclosure",
    ],
);

/// MusicXML's `%line-shape`, `%line-type` and `%dashed-formatting`.
const LINE_STYLE: StyleNames = (
    &["line-shape", "line-type", "dash-length", "space-length"],
    &["lineShape", "lineType", "dashLength", "spaceLength"],
);

/// MusicXML's `%halign` and `%valign`.
const ALIGN: StyleNames = (&["valign", "halign"], &["alignVertical", "alignHorizontal"]);

/// MusicXML's `%font`.
const FONT: StyleNames = (
    &["font-family", "font-style", "font-size", "font-weight"],
    &["fontFamily", "fontStyle", "fontSize", "fontWeight"],
);

/// MusicXML's `%position`.
const POSITION: StyleNames = (
    &["default-x", "default-y", "relative-x", "relative-y"],
    &["absoluteX", "absoluteY", "relativeX", "relativeY"],
);

/// The style members music21 reads `yes` and `no` into: its
/// `xmlObjects.STYLE_ATTRIBUTES_YES_NO_TO_BOOL`.
const YES_NO_STYLE: &[&str] = &["hideObjectOnPrint"];

/// What a page, system and staff layout read put on one given to fill.
const PAGE_LAYOUT: &[&str] = &[
    "isNew",
    "pageNumber",
    "pageHeight",
    "pageWidth",
    "topMargin",
    "bottomMargin",
    "leftMargin",
    "rightMargin",
];
const SYSTEM_LAYOUT: &[&str] = &[
    "isNew",
    "distance",
    "topDistance",
    "topMargin",
    "bottomMargin",
    "leftMargin",
    "rightMargin",
];
const STAFF_LAYOUT: &[&str] = &["distance", "staffNumber"];

/// What the reader warns of, each as a `MusicXMLWarning`, as music21
/// warns.
fn warn_all(py: Python<'_>, warnings: &[music21::Error]) -> PyResult<()> {
    let warn = py.import("warnings")?.getattr("warn")?;
    let category = py.get_type::<crate::musicxml::MusicXMLWarning>();
    for warning in warnings {
        warn.call1((crate::pitch::message(warning), &category, 2))?;
    }
    Ok(())
}

/// Each of `members` copied from one object to another where the first
/// has it set.
fn copy_members(from: &Bound<'_, PyAny>, to: &Bound<'_, PyAny>, members: &[&str]) -> PyResult<()> {
    for member in members {
        let value = from.getattr(*member)?;
        if !value.is_none() {
            to.setattr(*member, value)?;
        }
    }
    Ok(())
}

/// A name or names given, as a list.
fn names_of(given: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Ok(name) = given.extract::<String>() {
        return Ok(vec![name]);
    }
    given.try_iter()?.map(|name| name?.extract()).collect()
}

/// music21's `setStyleAttributes` over names already paired.
fn set_style<S: AsRef<str>>(
    mxObject: &Bound<'_, PyAny>,
    m21Object: &Bound<'_, PyAny>,
    xml_names: &[S],
    m21_names: &[S],
) -> PyResult<()> {
    let py = mxObject.py();
    if !mxObject.getattr("attrib")?.is_truthy()? {
        return Ok(());
    }
    let style_class = music21_name(py, "music21.style", "Style")?;
    let mut style = match m21Object.is_instance(&style_class)? {
        true => Some(m21Object.clone()),
        false => None,
    };
    for (xml_name, m21_name) in xml_names.iter().zip(m21_names) {
        let (xml_name, m21_name) = (xml_name.as_ref(), m21_name.as_ref());
        let value: Option<String> = mxObject.call_method1("get", (xml_name,))?.extract()?;
        let Some(value) = value else {
            continue;
        };
        let read = style_value(py, m21_name, &value)?;
        if style.is_none() {
            style = Some(m21Object.getattr("style")?);
        }
        if let Some(style) = &style {
            style.setattr(m21_name, read)?;
        }
    }
    Ok(())
}

/// An attribute's text as music21 writes it to a style: `yes` or `no` as
/// a boolean where music21 reads one, a number as an `int` where whole,
/// anything else as written.
fn style_value<'py>(py: Python<'py>, name: &str, text: &str) -> PyResult<Bound<'py, PyAny>> {
    if YES_NO_STYLE.contains(&name) {
        return Ok(pyo3::types::PyBool::new(py, text == "yes")
            .to_owned()
            .into_any());
    }
    let Ok(number) = text.trim().parse::<f64>() else {
        return Ok(PyString::new(py, text).into_any());
    };
    match music21::common::numbertools::num_to_int_or_float(number) {
        Ok(music21::common::numbertools::IntOrFloat::Int(whole)) => {
            Ok(whole.into_pyobject(py)?.into_any())
        }
        _ => Ok(number.into_pyobject(py)?.into_any()),
    }
}

/// A layout read, handed back as a new object, or written into the one
/// given: `members` are copied where the reading gave them.
fn layout_into<'py>(
    py: Python<'py>,
    read: music21::layout::Layout,
    inputM21: Option<&Bound<'py, PyAny>>,
    members: &[&str],
) -> PyResult<Bound<'py, PyAny>> {
    let classes = crate::layout::classes_from(py, false)?;
    let made = crate::layout::layout_object(&classes, &read)?;
    let Some(given) = inputM21.filter(|given| !given.is_none()) else {
        return Ok(made);
    };
    copy_members(&made, given, members)?;
    Ok(py.None().into_bound(py))
}

/// The spanners a part's measure parsers are building, kept between one
/// direction and the next as music21's `spannerBundle` keeps them: the
/// crate's state, with the objects made for what it builds.
#[pyclass(name = "_SpannerState", module = "music21.musicxml.xmlToM21")]
pub struct SpannerHolder {
    state: step::SpannerState,
    /// Each object a spanner may join, by the uid the state knows it by.
    joinable: Vec<(usize, Py<PyAny>)>,
    /// The object made for each spanner, with how many of what it joins
    /// the object has been given.
    made: Vec<(step::SpannerId, Py<PyAny>, usize)>,
}

impl SpannerHolder {
    /// The uid an object is known by, given one where it has none.
    fn uid_of(&mut self, py: Python<'_>, object: &Bound<'_, PyAny>) -> usize {
        if let Some((uid, _)) = self
            .joinable
            .iter()
            .find(|(_, held)| held.bind(py).is(object))
        {
            return *uid;
        }
        let uid = self.state.new_uid();
        self.joinable.push((uid, object.clone().unbind()));
        uid
    }

    /// Each object made brought up to what the state now says of its
    /// spanner: what it says of itself, whether it is complete, and what
    /// it has joined since.
    fn sync(&mut self, py: Python<'_>) -> PyResult<()> {
        for (id, object, given) in &mut self.made {
            let object = object.bind(py);
            let Some(read) = self.state.spanner(*id) else {
                continue;
            };
            let complete = read.complete();
            object.cast::<crate::spanner::Spanner>()?.borrow_mut().inner = read;
            object.setattr("completeStatus", complete)?;
            let joined = self.state.joined(*id);
            for uid in &joined[*given..] {
                if let Some((_, held)) = self.joinable.iter().find(|(known, _)| known == uid) {
                    object.call_method1("addSpannedElements", (held.bind(py),))?;
                }
            }
            *given = joined.len();
        }
        Ok(())
    }
}

#[pymethods]
impl SpannerHolder {
    #[new]
    fn new() -> Self {
        Self {
            state: step::SpannerState::default(),
            joinable: Vec::new(),
            made: Vec::new(),
        }
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        for (_, held) in &self.joinable {
            visit.call(held)?;
        }
        for (_, held, _) in &self.made {
            visit.call(held)?;
        }
        Ok(())
    }

    fn __clear__(&mut self) {
        self.joinable.clear();
        self.made.clear();
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

    /// music21's `processEncoding`: the programs an `<encoding>` names
    /// added to `md`'s software, and what it says the document marks.
    fn processEncoding(
        slf: &Bound<'_, Self>,
        encoding: &Bound<'_, PyAny>,
        md: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let read = step::read_encoding(&written(encoding)?).map_err(import_error)?;
        for software in &read.software {
            md.call_method1("add", ("software", software))?;
        }
        if read.finale {
            slf.setattr("applyFinaleWorkarounds", true)?;
        }
        if read.system_breaks {
            slf.setattr("definesExplicitSystemBreaks", true)?;
        }
        if read.page_breaks {
            slf.setattr("definesExplicitPageBreaks", true)?;
        }
        Ok(())
    }

    /// music21's `isRecognizableMetadataKey`: whether a `<miscellaneous-field>`
    /// names one of music21's metadata properties.
    #[staticmethod]
    fn isRecognizableMetadataKey(miscFieldName: &str) -> bool {
        music21::metadata::is_standard_name(miscFieldName)
            || music21::metadata::is_music21_work_id(miscFieldName)
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
        // No clef is assumed for any staff, as music21 now has it.
        let clefs = PyDict::new(py);
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
            ("arpeggioSpanners", PyDict::new(py).into_any()),
            (
                "_spannerState",
                Bound::new(py, SpannerHolder::new())?.into_any(),
            ),
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
            .map_err(|error| Self::placed(slf, &error).unwrap_or_else(|failed| failed))?;
        warn_all(py, &read.warnings)?;
        let mut made = Vec::new();
        for staff in &read.parts {
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

    /// music21's `setLastMeasureInfo`: the measure's number, suffix and
    /// meter kept for the next; a 4/4 where no meter has been met.
    fn setLastMeasureInfo(slf: &Bound<'_, Self>, m: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        // A measure numbered as the last keeps the last suffix, so that
        // Finale's X1, X2 do not pile up.
        let number = m.getattr("number")?;
        if !number.eq(slf.getattr("lastMeasureNumber")?)? {
            slf.setattr("lastMeasureNumber", number)?;
            slf.setattr("lastNumberSuffix", m.getattr("numberSuffix")?)?;
        }
        let meter = m.getattr("timeSignature")?;
        if !meter.is_none() {
            return slf.setattr("lastTimeSignature", meter);
        }
        if slf.getattr("lastTimeSignature")?.is_none() {
            let common = music21_name(py, "music21.meter", "TimeSignature")?.call1(("4/4",))?;
            slf.setattr("lastTimeSignature", common)?;
        }
        Ok(())
    }

    /// music21's `adjustTimeAttributesFromMeasure`: the measure padded as
    /// a pickup or a measure cut short, an empty one filled with a rest,
    /// and the next measure's offset moved on past it.
    fn adjustTimeAttributesFromMeasure(
        slf: &Bound<'_, Self>,
        m: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let meter = slf.getattr("lastTimeSignature")?;
        let bar_length: f64 = match meter.is_none() {
            true => 4.0,
            false => meter
                .getattr("barDuration")?
                .getattr("quarterLength")?
                .extract()?,
        };
        let mut has_notes = false;
        for element in m
            .call_method0("recurse")?
            .getattr("notesAndRests")?
            .try_iter()?
        {
            if !element?.getattr("classes")?.contains("Harmony")? {
                has_notes = true;
                break;
            }
        }
        let last_offset: f64 = slf.getattr("lastMeasureOffset")?.extract()?;
        let fit = step::measure_fit(&step::FitAsked {
            highest: m.getattr("highestTime")?.extract()?,
            bar_length,
            has_notes,
            at_start: last_offset == 0.0,
            last_was_short: slf.getattr("lastMeasureWasShort")?.is_truthy()?,
        });

        if fit.overfull_ignored {
            let message = format!(
                "Warning: measure {} in part {}is overfull: {} > {bar_length},assuming {} is correct.",
                m.getattr("number")?,
                slf.getattr("stream")?.getattr("partName")?,
                m.getattr("highestTime")?,
                fit.shift,
            );
            py.import("warnings")?.call_method1(
                "warn",
                (
                    message,
                    py.get_type::<crate::musicxml::MusicXMLWarning>(),
                    2,
                ),
            )?;
        }
        if fit.fill_with_rest {
            let keywords = PyDict::new(py);
            keywords.set_item("quarterLength", bar_length)?;
            let rest = music21_name(py, "music21.note", "Rest")?.call((), Some(&keywords))?;
            m.call_method1("insert", (0.0, rest))?;
        }
        if let Some(padding) = fit.padding_left {
            m.setattr("paddingLeft", padding)?;
        }
        if let Some(padding) = fit.padding_right {
            m.setattr("paddingRight", padding)?;
        }
        slf.setattr("lastMeasureWasShort", fit.last_was_short)?;
        slf.setattr("lastMeasureOffset", last_offset + fit.shift)
    }

    /// music21's `getDefaultInstrument`: the instrument a `<score-part>`
    /// names, read as the reader reads it for a part.
    #[pyo3(signature = (mxScorePart = None))]
    fn getDefaultInstrument<'py>(
        slf: &Bound<'py, Self>,
        mxScorePart: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let score_part = match mxScorePart.filter(|given| !given.is_none()) {
            Some(given) => given.clone(),
            None => slf.getattr("mxScorePart")?,
        };
        let score_part = match score_part.is_none() {
            true => None,
            false => Some(written(&score_part)?),
        };
        // Under the part's id, which names the instrument read.
        let id: String = slf.getattr("partId")?.str()?.extract()?;
        let part = crate::musicxml::element_tree(py)?
            .getattr("Element")?
            .call1(("part",))?;
        part.call_method1("set", ("id", id))?;
        part.call_method1(
            "append",
            (crate::musicxml::element_tree(py)?
                .getattr("Element")?
                .call1(("measure",))?,),
        )?;
        let read =
            step::read_part(&written(&part)?, score_part.as_deref()).map_err(import_error)?;
        warn_all(py, &read.warnings)?;
        for part in &read.parts {
            let made = crate::stream::from_crate(py, part)?;
            let found = first_of(&made, "Instrument")?;
            if !found.is_none() {
                return Ok(found);
            }
        }
        music21_name(py, "music21.instrument", "Instrument")?.call0()
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
            let chord = crate::musicxml::element_tree(py)?
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
        let read = step::read_accidental(&written(mxAccidental)?).map_err(import_error)?;
        let made = crate::installed_new(
            py,
            "music21.pitch",
            "Accidental",
            crate::pitch::Accidental::from_inner(read),
        )?
        .into_any();
        let Some(given) = inputM21.filter(|given| !given.is_none()) else {
            return Ok(made);
        };

        // Written into the accidental given, which music21 also hands back.
        let made = made.bind(py);
        given.call_method(
            "set",
            (made.getattr("name")?,),
            Some(&[("allowNonStandardValue", true)].into_py_dict(py)?),
        )?;
        copy_members(made, given, &["displayStyle", "displayType"])?;
        Ok(given.clone().unbind())
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
        let read = step::read_lyric(&written(mxLyric)?).map_err(import_error)?;
        let made = crate::installed_new(
            py,
            "music21.note",
            "Lyric",
            crate::notation::Lyric::wrap(read),
        )?
        .into_any();
        let Some(given) = inputM21.filter(|given| !given.is_none()) else {
            return Ok(made);
        };

        // Written into the lyric given, which music21 hands nothing back for.
        let made = made.bind(py);
        copy_members(made, given, &["text", "syllabic", "number", "identifier"])?;
        let components = made.getattr("components")?;
        if components.len()? > 0 {
            given.setattr("components", components)?;
        }
        copy_members(
            &made.getattr("style")?,
            &given.getattr("style")?,
            &["color"],
        )?;
        Ok(py.None())
    }

    /// music21's `updateLyricsFromList`: the `<lyric>`s of a note, each
    /// numbered nought taking its place among them, as its lyrics.
    fn updateLyricsFromList(
        slf: &Bound<'_, Self>,
        n: &Bound<'_, PyAny>,
        lyricList: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = slf.py();
        let mut texts = Vec::new();
        for lyric in lyricList.try_iter()? {
            texts.push(written(&lyric?)?);
        }
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let read = step::read_lyrics(&texts).map_err(import_error)?;
        let held = n.getattr("lyrics")?;
        for lyric in read {
            let made = crate::installed_new(
                py,
                "music21.note",
                "Lyric",
                crate::notation::Lyric::wrap(lyric),
            )?;
            held.call_method1("append", (made,))?;
        }
        Ok(())
    }

    /// music21's `xmlToArticulation`: one mark of `<articulations>`.
    fn xmlToArticulation(slf: &Bound<'_, Self>, mxObj: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        match step::read_articulation(&written(mxObj)?).map_err(import_error)? {
            Some(read) => crate::articulations::Articulation::object(py, read),
            None => Ok(py.None()),
        }
    }

    /// music21's `xmlOrnamentToExpression`: one mark of `<ornaments>`, as
    /// read on a note of its own; `None` for one music21 does not know.
    #[pyo3(signature = (mxObj, *, taggedNote = None))]
    fn xmlOrnamentToExpression<'py>(
        slf: &Bound<'py, Self>,
        mxObj: &Bound<'py, PyAny>,
        taggedNote: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let _ = taggedNote;
        let notations = format!(
            "<notations><ornaments>{}</ornaments></notations>",
            written(mxObj)?
        );
        let note = Self::marked_note(slf, &notations)?;
        let expressions = note.getattr("expressions")?;
        if expressions.len()? == 0 {
            return Ok(py.None().into_bound(py));
        }
        expressions.get_item(0)
    }

    /// music21's `xmlNotations`: what a `<notations>` marks -- the
    /// articulations, ornaments and other expressions -- put on `n`.
    fn xmlNotations(
        slf: &Bound<'_, Self>,
        mxNotations: &Bound<'_, PyAny>,
        n: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let note = Self::marked_note(slf, &written(mxNotations)?)?;
        for member in ["articulations", "expressions"] {
            let read = note.getattr(member)?;
            if read.len()? == 0 {
                continue;
            }
            let held = n.getattr(member)?;
            for mark in read.try_iter()? {
                held.call_method1("append", (mark?,))?;
            }
        }
        Ok(())
    }

    /// music21's `xmlDirectionTypeToSpanners`: a `<wedge>`, `<bracket>`,
    /// `<dashes>`, `<octave-shift>` or `<pedal>` read into the part's
    /// spanners, and the spanners it begins handed back. One that ends a
    /// spanner joins `nLast` to it; a pedal's bounces and gaps go in the
    /// measure at `totalOffset` on `staffKey`.
    #[pyo3(signature = (mxObj, staffKey = None, totalOffset = None))]
    fn xmlDirectionTypeToSpanners<'py>(
        slf: &Bound<'py, Self>,
        mxObj: &Bound<'py, PyAny>,
        staffKey: Option<i32>,
        totalOffset: Option<f64>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let holder = slf.getattr("parent")?.getattr("_spannerState")?;
        let holder = holder.cast::<SpannerHolder>()?;
        let last = slf.getattr("nLast")?;
        let staff = staffKey.unwrap_or(NO_STAFF_ASSIGNED);
        let offset = totalOffset.unwrap_or(0.0);
        let read = {
            let mut held = holder.borrow_mut();
            let last = match last.is_none() {
                true => None,
                false => Some(held.uid_of(py, &last)),
            };
            step::read_direction_type(&written(mxObj)?, staff, offset, last, &mut held.state)
                .map_err(import_error)?
        };

        // The spanners begun, into the part's bundle.
        let begun = PyList::empty(py);
        let bundle = slf.getattr("spannerBundle")?;
        for id in read.started {
            let made = {
                let held = holder.borrow();
                let Some(spanner) = held.state.spanner(id) else {
                    continue;
                };
                crate::spanner::Spanner::object(py, &spanner, Vec::new())?
            };
            holder.borrow_mut().made.push((id, made.clone_ref(py), 0));
            bundle.call_method1("append", (made.bind(py),))?;
            begun.append(made)?;
        }

        // The pedal's bounces and gaps, into the measure.
        for (uid, at, mark) in read.marks {
            let made = crate::expressions::PedalObject::object(py, mark)?;
            holder.borrow_mut().joinable.push((uid, made.clone_ref(py)));
            Self::insertCoreAndRef(
                slf,
                &at.into_pyobject(py)?.into_any(),
                &staff.into_pyobject(py)?.into_any(),
                made.bind(py),
            )?;
        }
        holder.borrow_mut().sync(py)?;
        Ok(begun)
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
        let made = first_of(&measure, "TextExpression")?;
        if !made.is_none() {
            XMLParserBase::setTextFormatting(mxWords, &made)?;
        }
        Ok(made)
    }

    /// music21's `xmlToRehearsalMark`: a `<rehearsal>`.
    fn xmlToRehearsalMark<'py>(
        slf: &Bound<'py, Self>,
        mxRehearsal: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let direction = format!(
            "<direction><direction-type>{}</direction-type></direction>",
            written(mxRehearsal)?
        );
        let measure = read_measure(slf.as_any(), &in_measure(&[direction]))?;
        first_of(&measure, "RehearsalMark")
    }

    /// music21's `xmlNotehead`: a `<notehead>` put on `n`.
    fn xmlNotehead(
        slf: &Bound<'_, Self>,
        n: &Bound<'_, PyAny>,
        mxNotehead: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let read = Self::marked_note(slf, &written(mxNotehead)?)?;
        copy_members(
            &read,
            n,
            &["notehead", "noteheadFill", "noteheadParenthesis"],
        )?;
        copy_members(&read.getattr("style")?, &n.getattr("style")?, &["color"])
    }

    /// music21's `xmlNoteToGeneralNoteHelper`: what any `<note>` says of a
    /// note or rest besides its pitch -- its print style, whether it is
    /// shown, its tie and its notations -- put on `n`.
    #[pyo3(signature = (n, mxNote, freeSpanners = true))]
    fn xmlNoteToGeneralNoteHelper<'py>(
        slf: &Bound<'py, Self>,
        n: &Bound<'py, PyAny>,
        mxNote: &Bound<'py, PyAny>,
        freeSpanners: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = freeSpanners;
        XMLParserBase::setPrintStyle(mxNote, n)?;
        XMLParserBase::setPrintObject(mxNote, n)?;
        if !mxNote.call_method1("find", ("tie",))?.is_none() {
            n.setattr("tie", Self::xmlToTie(slf, mxNote)?)?;
        }
        for notations in mxNote.call_method1("findall", ("notations",))?.try_iter()? {
            Self::xmlNotations(slf, &notations?, n)?;
        }
        Ok(n.clone())
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
    fn placed(slf: &Bound<'_, Self>, error: &music21::Error) -> PyResult<PyErr> {
        let raised = MusicXMLImportException::new_err(crate::pitch::message(error));
        let measure = error
            .position()
            .and_then(|position| position.measure.clone())
            .filter(|measure| !measure.is_empty())
            .unwrap_or_else(|| "0".to_string());
        let value = raised.value(slf.py());
        value.setattr("partName", Self::part_name(slf)?)?;
        value.setattr("measureNumber", measure)?;
        Ok(raised)
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
    /// A quarter note on C4 carrying `notations`, read as the reader reads
    /// the note they are written on.
    fn marked_note<'py>(slf: &Bound<'py, Self>, notations: &str) -> PyResult<Bound<'py, PyAny>> {
        let note = format!(
            "<attributes><divisions>1</divisions></attributes><note><pitch><step>C</step>\
             <octave>4</octave></pitch><duration>1</duration><type>quarter</type>{notations}</note>"
        );
        let measure = read_measure(slf.as_any(), &in_measure(&[note]))?;
        first_of(&measure, "GeneralNote")
    }

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
        let tree = crate::musicxml::element_tree(py)?;
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
        // A floor of nought, as music21 keeps one against rounding.
        slf.setattr("offsetMeasureNote", (now + sign * change).max(0.0))
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
        let py = slf.py();
        let location: Option<String> = barline.call_method1("get", ("location",))?.extract()?;
        let read = step::read_barline(&written(barline)?).map_err(import_error)?;
        let made = crate::bar::Barline::object(py, read)?.into_bound(py);
        // music21 takes MusicXML 3.0's default end, the right.
        made.setattr("location", location.as_deref().unwrap_or("right"))?;
        Ok(made)
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
        m.add_class::<SpannerHolder>()?;
        // The exceptions are `xmlObjects`' own, which that module finishes.
        let objects = crate::musicxml::objects_module(py)?;
        for name in ["MusicXMLImportException", "MusicXMLWarning"] {
            m.add(name, objects.getattr(name)?)?;
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
