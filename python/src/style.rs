//! music21's `style` beyond `Style` and `NoteStyle`, which `notation`
//! keeps: the text, placement, bezier, line, stream and beam styles, and
//! the `Enclosure` enum.
//!
//! As `Style` does, each keeps what it is given in its own dictionary and
//! has music21's defaults on the class. What music21 checks on the way in
//! is checked here with the crate's `style::TextStyle`, and refused with
//! music21's `TextFormatException` and music21's wording. Where music21 is
//! installed its own classes are used, so none of this goes over them.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::PyClass;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use music21::style::TextStyle as RsTextStyle;

use crate::base::Footing;
use crate::notation::{Style, TextFormatException};
use crate::prebase::ProtoM21Object;

/// The module music21 keeps these in.
const MODULE: &str = "music21.style";

/// music21's `Enclosure` members, by name and value, in its order.
const ENCLOSURES: [(&str, &str); 15] = [
    ("RECTANGLE", "rectangle"),
    ("SQUARE", "square"),
    ("OVAL", "oval"),
    ("CIRCLE", "circle"),
    ("BRACKET", "bracket"),
    ("TRIANGLE", "triangle"),
    ("DIAMOND", "diamond"),
    ("PENTAGON", "pentagon"),
    ("HEXAGON", "hexagon"),
    ("HEPTAGON", "heptagon"),
    ("OCTAGON", "octagon"),
    ("NONAGON", "nonagon"),
    ("DECAGON", "decagon"),
    ("INVERTED_BRACKET", "inverted-bracket"),
    ("NO_ENCLOSURE", "none"),
];

/// The enum, built once.
static ENCLOSURE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

impl Footing for Style {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        ProtoM21Object::under(Style).add_subclass(value)
    }
}

/// music21's `style.Enclosure`: a string enum of the shapes text can be
/// drawn inside, built on music21's `common.StrEnum`, whose repr leaves
/// the value out.
pub(crate) fn enclosure_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let built = ENCLOSURE.get_or_try_init(py, || -> PyResult<Py<PyAny>> {
        let keywords = PyDict::new(py);
        keywords.set_item("module", MODULE)?;
        keywords.set_item("qualname", "Enclosure")?;
        let class = crate::plumbing::str_enum(py)?
            .call(("Enclosure", ENCLOSURES.to_vec()), Some(&keywords))?;
        class.setattr(
            "__doc__",
            "The shapes text can be drawn inside; NO_ENCLOSURE says there is none.",
        )?;
        Ok(class.unbind())
    })?;
    Ok(built.bind(py).clone())
}

/// The member an enclosure's name gives, read without regard to case, as
/// music21's setter reads it; `None` for no enclosure music21 names.
pub(crate) fn enclosure_named<'py>(
    py: Python<'py>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let lowered = name.to_lowercase();
    if !ENCLOSURES.iter().any(|(_, value)| *value == lowered) {
        return Ok(None);
    }
    enclosure_class(py)?.call1((lowered,)).map(Some)
}

/// What an instance keeps under `name`, or `None`.
fn stored<'py>(slf: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let dict = slf.getattr("__dict__")?;
    Ok(dict
        .call_method1("get", (name,))
        .unwrap_or_else(|_| py.None().into_bound(py)))
}

fn store(slf: &Bound<'_, PyAny>, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
    slf.getattr("__dict__")?.set_item(name, value)
}

/// The list kept under `name`, made empty the first time it is asked for:
/// music21 makes a new one for every instance.
fn stored_list<'py>(slf: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let found = stored(slf, name)?;
    if !found.is_none() {
        return Ok(found);
    }
    let made = PyList::empty(slf.py()).into_any();
    store(slf, name, made.clone())?;
    Ok(made)
}

/// A refusal in music21's words: `what` and the value as Python shows it.
fn refused(what: &str, value: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    Ok(TextFormatException::new_err(format!(
        "{what}: {}",
        value.repr()?
    )))
}

/// music21's `numToIntOrFloat`: a whole number where the value is within a
/// millionth of one, a float otherwise, read from text where it is text.
fn int_or_float<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = value.py();
    let builtins = py.import("builtins")?;
    let rounded = builtins.getattr("round")?.call1((value,));
    let (value, whole) = match rounded {
        Ok(whole) => (value.clone(), whole),
        Err(error)
            if error.is_instance_of::<pyo3::exceptions::PyValueError>(py)
                || error.is_instance_of::<pyo3::exceptions::PyTypeError>(py) =>
        {
            let float = builtins.getattr("float")?.call1((value,))?;
            let whole = builtins.getattr("round")?.call1((&float,))?;
            (float, whole)
        }
        Err(error) => return Err(error),
    };
    let keywords = PyDict::new(py);
    keywords.set_item("abs_tol", 1e-6)?;
    let close = py
        .import("math")?
        .getattr("isclose")?
        .call((&whole, &value), Some(&keywords))?;
    if close.is_truthy()? {
        return Ok(whole);
    }
    value.call_method1("__add__", (0.0,))
}

/// A value music21 checks with `value.lower()` against the ones it names,
/// checked by `set` on the crate's style; `Ok(None)` for one refused.
fn lowered<'py>(
    value: &Bound<'py, PyAny>,
    set: impl Fn(&mut RsTextStyle, &str) -> bool,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let lower = value.call_method0("lower")?;
    let text: String = lower.extract()?;
    let mut scratch = RsTextStyle::default();
    Ok(set(&mut scratch, &text).then_some(lower))
}

/// A value music21 checks with `value in (None, ...)`: the value itself
/// where the crate's style takes it as written.
fn as_written(value: &Bound<'_, PyAny>, set: impl Fn(&mut RsTextStyle, &str) -> bool) -> bool {
    if !value.is_instance_of::<PyString>() {
        return false;
    }
    let Ok(text) = value.extract::<String>() else {
        return false;
    };
    let mut scratch = RsTextStyle::default();
    set(&mut scratch, &text)
}

/// music21's `style.TextStyle`: a style that also says how text is set --
/// its font, size, weight, spacing, alignment and justification.
#[pyclass(name = "TextStyle", module = "music21.style", extends = Style, subclass)]
pub struct TextStyle;

impl Footing for TextStyle {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        Style::under(TextStyle).add_subclass(value)
    }
}

#[pymethods]
impl TextStyle {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        crate::base::built(TextStyle)
    }

    /// `top`, `middle`, `bottom`, `baseline`, or `None`.
    #[getter]
    fn get_alignVertical<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_alignVertical")
    }

    #[setter]
    fn set_alignVertical(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !value.is_none()
            && !as_written(&value, |style, text| {
                style.set_align_vertical(Some(text)).is_ok()
            })
        {
            return Err(refused("Invalid vertical align", &value)?);
        }
        store(slf.as_any(), "_alignVertical", value)
    }

    /// `left`, `right`, `center`, or `None`.
    #[getter]
    fn get_alignHorizontal<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_alignHorizontal")
    }

    #[setter]
    fn set_alignHorizontal(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        if !value.is_none()
            && !as_written(&value, |style, text| {
                style.set_align_horizontal(Some(text)).is_ok()
            })
        {
            return Err(refused("Invalid horizontal align", &value)?);
        }
        store(slf.as_any(), "_alignHorizontal", value)
    }

    /// `left`, `center`, `right`, `full`, or `None`, kept in lower case.
    #[getter]
    fn get_justify<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_justify")
    }

    #[setter]
    fn set_justify(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            return store(slf.as_any(), "_justify", value);
        }
        match lowered(&value, |style, text| style.set_justify(Some(text)).is_ok())? {
            Some(lower) => store(slf.as_any(), "_justify", lower),
            None => Err(refused("Not a supported justification", &value)?),
        }
    }

    /// `normal`, `italic`, `bold`, `bolditalic`, or `None`.
    #[getter]
    fn get_fontStyle<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_fontStyle")
    }

    #[setter]
    fn set_fontStyle(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            return store(slf.as_any(), "_fontStyle", value);
        }
        match lowered(&value, |style, text| {
            style.set_font_style(Some(text)).is_ok()
        })? {
            Some(lower) => store(slf.as_any(), "_fontStyle", lower),
            None => Err(refused("Not a supported fontStyle", &value)?),
        }
    }

    /// `normal`, `bold`, or `None`.
    #[getter]
    fn get_fontWeight<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_fontWeight")
    }

    #[setter]
    fn set_fontWeight(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        if value.is_none() {
            return store(slf.as_any(), "_fontWeight", value);
        }
        match lowered(&value, |style, text| {
            style.set_font_weight(Some(text)).is_ok()
        })? {
            Some(lower) => store(slf.as_any(), "_fontWeight", lower),
            // music21 shows the value as text here, not as its repr.
            None => Err(TextFormatException::new_err(format!(
                "Not a supported fontWeight: {}",
                value.str()?
            ))),
        }
    }

    /// The size: a number where it is one, else as given, since MusicXML's
    /// font sizes may be CSS sizes.
    #[getter]
    fn get_fontSize<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_fontSize")
    }

    #[setter]
    fn set_fontSize(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        if value.is_none() {
            return store(slf.as_any(), "_fontSize", value);
        }
        let read = match int_or_float(&value) {
            Ok(number) => number,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => value,
            Err(error) => return Err(error),
        };
        store(slf.as_any(), "_fontSize", read)
    }

    /// The spacing between letters, as a float, or `'normal'` or `None`.
    #[getter]
    fn get_letterSpacing<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored(slf.as_any(), "_letterSpacing")
    }

    #[setter]
    fn set_letterSpacing(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let normal = value.extract::<String>().is_ok_and(|text| text == "normal");
        if normal || value.is_none() {
            return store(slf.as_any(), "_letterSpacing", value);
        }
        match py.import("builtins")?.getattr("float")?.call1((&value,)) {
            Ok(number) => store(slf.as_any(), "_letterSpacing", number),
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => {
                Err(refused("Not a supported letterSpacing", &value)?)
            }
            Err(error) => Err(error),
        }
    }

    /// The font families, first choice first: a list, made empty the first
    /// time it is asked for. Text given is split at its commas.
    #[getter]
    fn get_fontFamily<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "_fontFamily")
    }

    #[setter]
    fn set_fontFamily(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let text =
            value.is_instance_of::<PyString>() || value.is_instance_of::<pyo3::types::PyBytes>();
        let iterable = !text
            && value.hasattr("__iter__")?
            && !value.get_type().is(py.get_type::<pyo3::types::PyType>());
        if iterable {
            return store(slf.as_any(), "_fontFamily", value);
        }
        let families = PyList::empty(py);
        for family in value.call_method1("split", (",",))?.try_iter()? {
            families.append(family?.call_method0("strip")?)?;
        }
        store(slf.as_any(), "_fontFamily", families.into_any())
    }
}

/// music21's `style.TextStylePlacement`: a text style that also says
/// whether the text goes above or below.
#[pyclass(name = "TextStylePlacement", module = "music21.style", extends = TextStyle, subclass)]
pub struct TextStylePlacement;

#[pymethods]
impl TextStylePlacement {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        crate::base::built(TextStylePlacement)
    }
}

/// A style class with nothing of its own but its defaults, on `Style`.
macro_rules! plain_style {
    ($rust:ident, $name:literal, $doc:literal) => {
        #[doc = $doc]
        #[pyclass(name = $name, module = "music21.style", extends = Style, subclass)]
        pub struct $rust;

        #[pymethods]
        impl $rust {
            #[new]
            #[pyo3(signature = (*_arguments, **_keywords))]
            fn new(
                _arguments: &Bound<'_, PyTuple>,
                _keywords: Option<&Bound<'_, PyDict>>,
            ) -> PyClassInitializer<Self> {
                crate::base::built($rust)
            }
        }
    };
}

plain_style!(
    BezierStyle,
    "BezierStyle",
    "music21's `style.BezierStyle`: the bezier points of a curve, as MusicXML gives them."
);
plain_style!(
    LineStyle,
    "LineStyle",
    "music21's `style.LineStyle`: the shape and type of a line, and its dashes."
);
plain_style!(
    BeamStyle,
    "BeamStyle",
    "music21's `style.BeamStyle`: how a beam is drawn, which is whether it fans."
);

/// music21's `style.StreamStyle`: the appearance a score says for itself
/// -- line widths, note sizes, distances, fonts and part names.
#[pyclass(name = "StreamStyle", module = "music21.style", extends = Style, subclass)]
pub struct StreamStyle;

#[pymethods]
impl StreamStyle {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(
        _arguments: &Bound<'_, PyTuple>,
        _keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyClassInitializer<Self> {
        crate::base::built(StreamStyle)
    }

    /// Pairs of a line's type and its width in tenths.
    #[getter]
    fn get_lineWidths<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "lineWidths")
    }

    #[setter]
    fn set_lineWidths(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        store(slf.as_any(), "lineWidths", value)
    }

    /// Pairs of a note's type and its size against the normal one.
    #[getter]
    fn get_noteSizes<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "noteSizes")
    }

    #[setter]
    fn set_noteSizes(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        store(slf.as_any(), "noteSizes", value)
    }

    /// Pairs of `beam` or `hyphen` and a distance in tenths.
    #[getter]
    fn get_distances<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "distances")
    }

    #[setter]
    fn set_distances(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        store(slf.as_any(), "distances", value)
    }

    /// Pairs of any other appearance's type and its tenths.
    #[getter]
    fn get_otherAppearances<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "otherAppearances")
    }

    #[setter]
    fn set_otherAppearances(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        store(slf.as_any(), "otherAppearances", value)
    }

    /// The text styles lyrics are set in.
    #[getter]
    fn get_lyricFonts<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "lyricFonts")
    }

    #[setter]
    fn set_lyricFonts(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        store(slf.as_any(), "lyricFonts", value)
    }

    /// The languages lyrics are written in.
    #[getter]
    fn get_lyricLanguages<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        stored_list(slf.as_any(), "lyricLanguages")
    }

    #[setter]
    fn set_lyricLanguages(slf: &Bound<'_, Self>, value: Bound<'_, PyAny>) -> PyResult<()> {
        store(slf.as_any(), "lyricLanguages", value)
    }
}

/// Each class's own defaults, as music21's `__init__` sets them.
const DEFAULTS: [(&str, &[&str]); 5] = [
    (
        "TextStyle",
        &[
            "lineHeight",
            "textDirection",
            "textRotation",
            "language",
            "textDecoration",
        ],
    ),
    ("TextStylePlacement", &["placement"]),
    (
        "BezierStyle",
        &[
            "bezierOffset",
            "bezierOffset2",
            "bezierX",
            "bezierY",
            "bezierX2",
            "bezierY2",
        ],
    ),
    ("LineStyle", &["lineShape", "lineType"]),
    (
        "StreamStyle",
        &[
            "musicFont",
            "wordFont",
            "measureNumbering",
            "measureNumberStyle",
        ],
    ),
];

/// `self._style`, or `None` where it has none.
fn own_style<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    match slf.getattr("_style") {
        Ok(found) => Ok(found),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {
            Ok(py.None().into_bound(py))
        }
        Err(error) => Err(error),
    }
}

/// `StyleMixin.hasStyleInformation`: whether a style has been made yet.
#[pyfunction]
fn has_style_information(slf: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!own_style(slf)?.is_none())
}

/// `StyleMixin.style`: the style, made of the class's `_styleClass` the
/// first time it is asked for.
#[pyfunction]
fn mixin_style<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let found = own_style(slf)?;
    if !found.is_none() {
        return Ok(found);
    }
    let made = slf.getattr("_styleClass")?.call0()?;
    slf.setattr("_style", &made)?;
    Ok(made)
}

#[pyfunction]
fn set_mixin_style(slf: &Bound<'_, PyAny>, value: Bound<'_, PyAny>) -> PyResult<()> {
    slf.setattr("_style", value)
}

/// `self._editorial`, or `None` where it has none.
fn own_editorial<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    match slf.getattr("_editorial") {
        Ok(found) => Ok(found),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {
            Ok(py.None().into_bound(py))
        }
        Err(error) => Err(error),
    }
}

/// `StyleMixin.hasEditorialInformation`.
#[pyfunction]
fn has_editorial_information(slf: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!own_editorial(slf)?.is_none())
}

/// `StyleMixin.editorial`: the editorial, made the first time it is asked
/// for.
#[pyfunction]
fn mixin_editorial<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let found = own_editorial(slf)?;
    if !found.is_none() {
        return Ok(found);
    }
    let module = py
        .import("music21.editorial")
        .or_else(|_| py.import("music21_rs"))?;
    let made = module.getattr("Editorial")?.call0()?;
    slf.setattr("_editorial", &made)?;
    Ok(made)
}

#[pyfunction]
fn set_mixin_editorial(slf: &Bound<'_, PyAny>, value: Bound<'_, PyAny>) -> PyResult<()> {
    slf.setattr("_editorial", value)
}

/// music21's `style.StyleMixin`: what gives an object that is no
/// `Music21Object` -- a lyric, an accidental -- a style and an editorial,
/// each made on first asking.
///
/// It is mixed into classes on another base, which one with a Rust layout
/// cannot be, so it is a Python class built with `types.new_class`, its
/// members Rust. Its `__slots__` are empty where music21's hold `_style`
/// and `_editorial`: slots of its own would not mix with the wheel's
/// `ProtoM21Object`, so the two are kept wherever the class it is mixed
/// into keeps its attributes.
fn style_mixin(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let m = PyModule::new(py, MODULE)?;
    let builtins = py.import("builtins")?;
    let property = builtins.getattr("property")?;
    let members = PyDict::new(py);
    members.set_item("__module__", MODULE)?;
    members.set_item("__qualname__", "StyleMixin")?;
    members.set_item(
        "__doc__",
        "Gives a class that is no Music21Object a style and an editorial, each \
         made the first time it is asked for.",
    )?;
    members.set_item("__slots__", PyTuple::empty(py))?;
    members.set_item("_styleClass", py.get_type::<Style>())?;
    members.set_item(
        "hasStyleInformation",
        property.call1((wrap_pyfunction!(has_style_information, &m)?,))?,
    )?;
    members.set_item(
        "style",
        property.call1((
            wrap_pyfunction!(mixin_style, &m)?,
            wrap_pyfunction!(set_mixin_style, &m)?,
        ))?,
    )?;
    members.set_item(
        "hasEditorialInformation",
        property.call1((wrap_pyfunction!(has_editorial_information, &m)?,))?,
    )?;
    members.set_item(
        "editorial",
        property.call1((
            wrap_pyfunction!(mixin_editorial, &m)?,
            wrap_pyfunction!(set_mixin_editorial, &m)?,
        ))?,
    )?;
    builtins.getattr("type")?.call1((
        "StyleMixin",
        PyTuple::new(py, [builtins.getattr("object")?])?,
        members,
    ))
}

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The names of music21's `style` that are no Rust class: `StyleMixin`.
/// The classes are put there by their own `__module__`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add("StyleMixin", style_mixin(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add_class::<TextStyle>()?;
    m.add_class::<TextStylePlacement>()?;
    m.add_class::<BezierStyle>()?;
    m.add_class::<LineStyle>()?;
    m.add_class::<StreamStyle>()?;
    m.add_class::<BeamStyle>()?;
    for (class, names) in DEFAULTS {
        let class = m.getattr(class)?;
        for name in names {
            class.setattr(*name, py.None())?;
        }
    }
    let stream_style = m.getattr("StreamStyle")?;
    stream_style.setattr("printPartName", true)?;
    stream_style.setattr("printPartAbbreviation", true)?;
    m.getattr("BeamStyle")?.setattr("fan", py.None())?;
    m.add("Enclosure", enclosure_class(py)?)?;
    Ok(())
}
