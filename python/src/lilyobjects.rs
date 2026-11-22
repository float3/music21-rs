//! music21's `lily.lilyObjects`: LilyPond's grammar as Python objects.
//!
//! Each class is one rule of LilyPond's grammar, holding its parts as
//! attributes and writing itself out with `stringOutput`. An object put in
//! an attribute takes the object it was put in as its parent, and one put
//! in a list takes it where it has none yet; a line of output is indented
//! by a space for each parent above it, which is why the text depends on
//! the order a tree was built in.
//!
//! The classes are built here as Python classes, as music21's are plain
//! classes users subclass, with every method Rust: one `__init__` reads
//! each class's attributes from [`CLASSES`], and one `stringOutput` writes
//! the class it finds first along an object's method resolution order.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyList, PyString, PyTuple};

use crate::plumbing::built::{method, new_class};

/// The module's dotted name.
const MODULE: &str = "music21.lily.lilyObjects";

/// The base class's name.
const BASE: &str = "LyObject";

/// What a LilyPond backslash is written as.
const BACKSLASH: &str = "\\";

/// An attribute's value where `__init__` is not given one.
#[derive(Clone, Copy)]
enum Default {
    None,
    /// A new empty list each time.
    List,
    False,
    True,
    Empty,
    Int(i64),
    Text(&'static str),
}

/// Each class under `LyObject`, in music21's order, with the attributes
/// its `__init__` takes, in order, and their defaults.
#[rustfmt::skip]
const CLASSES: &[(&str, &[(&str, Default)])] = &[
    ("LyMock", &[]),
    ("LyLilypondTop", &[("contents", Default::List)]),
    ("LyTopLevelExpression", &[("lilypondHeader", Default::None), ("bookBlock", Default::None), ("bookPartBlock", Default::None), ("scoreBlock", Default::None), ("compositeMusic", Default::None), ("fullMarkup", Default::None), ("fullMarkupList", Default::None), ("outputDef", Default::None)]),
    ("LyLilypondHeader", &[("lilypondHeaderBody", Default::None)]),
    ("LyEmbeddedScm", &[("content", Default::Empty)]),
    ("LyLilypondHeaderBody", &[("assignments", Default::List)]),
    ("LyAssignmentId", &[("content", Default::Empty), ("isLyricString", Default::False)]),
    ("LyAssignment", &[("assignmentId", Default::None), ("identifierInit", Default::None), ("propertyPath", Default::None), ("embeddedScm", Default::None)]),
    ("LyIdentifierInit", &[("scoreBlock", Default::None), ("bookBlock", Default::None), ("bookPartBlock", Default::None), ("outputDef", Default::None), ("contextDefSpecBlock", Default::None), ("music", Default::None), ("postEvent", Default::None), ("numberExpression", Default::None), ("string", Default::None), ("embeddedScm", Default::None), ("fullMarkup", Default::None), ("fullMarkupList", Default::None), ("digit", Default::None), ("contextModification", Default::None)]),
    ("LyContextDefSpecBlock", &[("contextDefSpecBody", Default::None)]),
    ("LyContextDefSpecBody", &[("contextDefIdentifier", Default::None), ("contextDefSpecBody", Default::None), ("embeddedScm", Default::None), ("contextMod", Default::None), ("contextModification", Default::None)]),
    ("LyBookBlock", &[("bookBody", Default::None)]),
    ("LyBookBody", &[("contents", Default::List), ("bookIdentifier", Default::None)]),
    ("LyBookpartBlock", &[("bookpartBody", Default::None)]),
    ("LyBookpartBody", &[("contents", Default::List), ("bookIdentifier", Default::None)]),
    ("LyScoreBlock", &[("scoreBody", Default::None)]),
    ("LyScoreBody", &[("music", Default::None), ("scoreIdentifier", Default::None), ("scoreBody", Default::None), ("lilypondHeader", Default::None), ("outputDef", Default::None), ("error", Default::None)]),
    ("LyPaperBlock", &[("outputDef", Default::None)]),
    ("LyLayout", &[]),
    ("LyOutputDef", &[("outputDefBody", Default::None)]),
    ("LyOutputDefHead", &[("defType", Default::None)]),
    ("LyOutputDefBody", &[("outputDefHead", Default::None), ("outputDefIdentifier", Default::None), ("outputDefBody", Default::None), ("assignment", Default::None), ("contextDefSpecBlock", Default::None), ("error", Default::None)]),
    ("LyTempoEvent", &[("tempoRange", Default::None), ("stenoDuration", Default::None), ("scalar", Default::None)]),
    ("LyMusicList", &[("contents", Default::List)]),
    ("LyMusic", &[("simpleMusic", Default::None), ("compositeMusic", Default::None)]),
    ("LyAlternativeMusic", &[("musicList", Default::None)]),
    ("LyRepeatedMusic", &[("simpleString", Default::None), ("unsignedNumber", Default::None), ("music", Default::None), ("alternativeMusic", Default::None)]),
    ("LySequentialMusic", &[("musicList", Default::None), ("displayTag", Default::False), ("beforeMatter", Default::None)]),
    ("LyOssiaMusic", &[("musicList", Default::None), ("startstop", Default::True)]),
    ("LySimultaneousMusic", &[("musicList", Default::None), ("displayTag", Default::False)]),
    ("LySimpleMusic", &[("eventChord", Default::None), ("musicIdentifier", Default::None), ("musicPropertyDef", Default::None), ("contextChange", Default::None)]),
    ("LyContextModification", &[("contextModList", Default::None), ("contextModIdentifier", Default::None), ("displayWith", Default::True)]),
    ("LyContextModList", &[("contents", Default::List), ("contextModIdentifier", Default::None)]),
    ("LyCompositeMusic", &[("prefixCompositeMusic", Default::None), ("groupedMusicList", Default::None), ("newLyrics", Default::None)]),
    ("LyGroupedMusicList", &[("simultaneousMusic", Default::None), ("sequentialMusic", Default::None)]),
    ("LySchemeFunction", &[("content", Default::None)]),
    ("LyOptionalId", &[("content", Default::None)]),
    ("LyPrefixCompositeMusic", &[("type", Default::None), ("genericPrefixMusicScm", Default::None), ("simpleString", Default::None), ("optionalId", Default::None), ("optionalContextMod", Default::None), ("music", Default::None), ("fraction", Default::None), ("repeatedMusic", Default::None), ("pitchAlsoInChords1", Default::None), ("pitchAlsoInChords2", Default::None), ("modeChangingHead", Default::None), ("groupedMusicList", Default::None), ("modeChangingHeadWithContext", Default::None), ("relativeMusic", Default::None), ("reRhythmedMusic", Default::None)]),
    ("LyModeChangingHead", &[("hasContext", Default::False), ("mode", Default::None)]),
    ("LyRelativeMusic", &[("content", Default::None)]),
    ("LyNewLyrics", &[("groupedMusicLists", Default::List)]),
    ("LyReRhythmedMusic", &[("groupedMusic", Default::None), ("newLyrics", Default::None)]),
    ("LyContextChange", &[("before", Default::Empty), ("after", Default::Empty)]),
    ("LyPropertyPath", &[("embeddedScheme", Default::List)]),
    ("LyPropertyOperation", &[("mode", Default::None), ("value1", Default::Empty), ("value2", Default::Empty), ("value3", Default::Empty)]),
    ("LyContextDefMod", &[("contextDef", Default::Empty)]),
    ("LyContextMod", &[("contextDefOrProperty", Default::None), ("scalar", Default::None)]),
    ("LyMusicPropertyDef", &[("isOnce", Default::False), ("propertyDef", Default::None)]),
    ("LyEventChord", &[("simpleChordElements", Default::None), ("postEvents", Default::None), ("chordRepetition", Default::None), ("multiMeasureRest", Default::None), ("duration", Default::None), ("commandElement", Default::None), ("noteChordElement", Default::None)]),
    ("LyNoteChordElement", &[("chordBody", Default::None), ("optionalNoteModeDuration", Default::None), ("postEvents", Default::List)]),
    ("LyChordBody", &[("chordBodyElements", Default::List)]),
    ("LyChordBodyElement", &[("parts", Default::List)]),
    ("LyCommandElement", &[("commandType", Default::None), ("argument", Default::None)]),
    ("LyCommandEvent", &[("commandType", Default::None), ("argument1", Default::Empty), ("argument2", Default::Empty)]),
    ("LyPostEvents", &[("eventList", Default::List)]),
    ("LyPostEvent", &[("arg1", Default::None), ("arg2", Default::None)]),
    ("LyDirectionLessEvent", &[("event", Default::None)]),
    ("LyDirectionReqdEvent", &[("event", Default::None)]),
    ("LyOctaveCheck", &[("equalOrQuotesOrNone", Default::None)]),
    ("LyPitch", &[("noteNamePitch", Default::Empty), ("quotes", Default::Empty)]),
    ("LyGenTextDef", &[("value", Default::None)]),
    ("LyScriptAbbreviation", &[("value", Default::Empty)]),
    ("LyScriptDir", &[("value", Default::Empty)]),
    ("LyStenoDuration", &[("durationNumber", Default::None), ("numDots", Default::Int(0))]),
    ("LyMultipliedDuration", &[("stenoDur", Default::None), ("multiply", Default::List)]),
    ("LyTremoloType", &[("tremTypeOrNone", Default::None)]),
    ("LyOptionalRest", &[("rest", Default::False)]),
    ("LySimpleElement", &[("parts", Default::List)]),
    ("LyLyricElement", &[("lyMarkupOrString", Default::None)]),
    ("LyTempoRange", &[("lowestOrOnlyTempo", Default::None), ("highestTempoOrNone", Default::None)]),
    ("LyNumberExpression", &[("numberAndSepList", Default::List)]),
    ("LyNumberTerm", &[("numberAndSepList", Default::List)]),
    ("LyLyricMarkup", &[("lyricMarkupOrIdentifier", Default::None), ("markupTop", Default::None)]),
    ("LyFullMarkupList", &[("markupListOrIdentifier", Default::None)]),
    ("LyFullMarkup", &[("markupTopOrIdentifier", Default::None)]),
    ("LyMarkupTop", &[("argument1", Default::Empty), ("argument2", Default::None)]),
    ("LyMarkupList", &[("markupIdentifierOrList", Default::None)]),
    ("LyMarkupComposedList", &[("markupHeadList", Default::Empty), ("markupBracedList", Default::Empty)]),
    ("LyMarkupBracedList", &[("listBody", Default::Empty)]),
    ("LyMarkupBracedListBody", &[("markupOrMarkupList", Default::List)]),
    ("LySimpleMarkup", &[("simpleType", Default::Text("string")), ("argument1", Default::Empty), ("argument2", Default::None)]),
    ("LyMarkup", &[("simpleMarkup", Default::None), ("optionalMarkupHeadList", Default::None)]),
];

pyo3::create_exception!(
    music21_rs_facade,
    LilyObjectsException,
    crate::Music21Exception
);

fn ly_error(message: impl Into<String>) -> PyErr {
    LilyObjectsException::new_err(message.into())
}

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<LilyObjectsException>();
        exception.setattr("__module__", MODULE)?;
        m.add("LilyObjectsException", exception)?;
        let base = base_class(py)?;
        m.add(BASE, &base)?;
        for (name, _) in CLASSES {
            m.add(*name, subclass(py, name, &base)?)?;
        }
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// One of the module's classes by name.
pub(crate) fn class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    module(py)?.getattr(name)
}

/// Builds an object of one of the module's classes.
pub(crate) fn make<'py>(
    py: Python<'py>,
    name: &str,
    keywords: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    for (key, value) in keywords {
        kwargs.set_item(*key, value)?;
    }
    class(py, name)?.call((), Some(&kwargs))
}

fn wrapped<'py>(
    function: Bound<'py, PyAny>,
    class: &str,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    method(function, MODULE, class, name)
}

/// `LyObject`, on `ProtoM21Object` as music21's is.
fn base_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let probe = PyModule::new(py, MODULE)?;
    let property = py.import("builtins")?.getattr("property")?;
    let mut members: Vec<(&str, Bound<'_, PyAny>)> = vec![
        ("supportedClasses", PyList::empty(py).into_any()),
        ("m21toLy", PyDict::new(py).into_any()),
        ("defaultAttributes", PyDict::new(py).into_any()),
        ("backslash", PyString::new(py, BACKSLASH).into_any()),
    ];
    let methods = [
        ("__init__", wrap_pyfunction!(init, &probe)?),
        ("__setattr__", wrap_pyfunction!(set_attribute, &probe)?),
        ("getParent", wrap_pyfunction!(get_parent, &probe)?),
        ("setParent", wrap_pyfunction!(set_parent, &probe)?),
        ("ancestorList", wrap_pyfunction!(ancestor_list, &probe)?),
        (
            "getAncestorByClass",
            wrap_pyfunction!(ancestor_by_class, &probe)?,
        ),
        ("setAttributes", wrap_pyfunction!(set_attributes, &probe)?),
        (
            "setAttributesFromClassObject",
            wrap_pyfunction!(attributes_from_class, &probe)?,
        ),
        ("_reprInternal", wrap_pyfunction!(repr_internal, &probe)?),
        ("__str__", wrap_pyfunction!(to_str, &probe)?),
        ("stringOutput", wrap_pyfunction!(string_output, &probe)?),
        (
            "getFirstNonNoneAttribute",
            wrap_pyfunction!(first_not_none, &probe)?,
        ),
        (
            "newlineSeparateStringOutputIfNotNone",
            wrap_pyfunction!(newline_separate, &probe)?,
        ),
        ("encloseCurly", wrap_pyfunction!(enclose_curly, &probe)?),
        ("quoteString", wrap_pyfunction!(quote_string, &probe)?),
        ("comment", wrap_pyfunction!(comment, &probe)?),
    ];
    for (name, function) in methods {
        members.push((name, wrapped(function.into_any(), BASE, name)?));
    }
    let indent = wrapped(
        wrap_pyfunction!(newline_indent, &probe)?.into_any(),
        BASE,
        "newlineIndent",
    )?;
    members.push(("newlineIndent", property.call1((indent,))?));
    let proto = py.get_type::<crate::prebase::ProtoM21Object>().into_any();
    new_class(
        py,
        MODULE,
        BASE,
        "LyObject is the base class of all other Lily Objects.",
        vec![proto],
        members,
    )
}

/// One class under `LyObject`, with the class attributes music21 gives it.
fn subclass<'py>(
    py: Python<'py>,
    name: &str,
    base: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut members: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    match name {
        "LyMock" => {
            members.push((
                "supportedClasses",
                PyList::new(py, ["Mock", "Mocker"])?.into_any(),
            ));
            let mock = PyDict::new(py);
            mock.set_item("mockAttribute", "mock-attribute")?;
            mock.set_item("mockAttribute2", "mock-attribute-2")?;
            let mocker = PyDict::new(py);
            mocker.set_item("mockerAttribute", "mock-attribute")?;
            mocker.set_item("greg", "mock-attribute-2")?;
            let table = PyDict::new(py);
            table.set_item("Mock", mock)?;
            table.set_item("Mocker", mocker)?;
            members.push(("m21toLy", table.into_any()));
            let defaults = PyDict::new(py);
            defaults.set_item("mockAttribute2", 7)?;
            members.push(("defaultAttributes", defaults.into_any()));
        }
        "LyLilypondTop" => {
            let can_contain = PyList::new(
                py,
                [
                    py.None(),
                    "TopLevelExpression".into_pyobject(py)?.into_any().unbind(),
                    "Assignment".into_pyobject(py)?.into_any().unbind(),
                ],
            )?;
            members.push(("canContain", can_contain.into_any()));
        }
        "LyModeChangingHead" => {
            members.push(("allowableModes", PyList::new(py, MODES)?.into_any()));
        }
        "LyCompositeMusic" => {
            let probe = PyModule::new(py, MODULE)?;
            let contents = wrapped(
                wrap_pyfunction!(composite_contents, &probe)?.into_any(),
                name,
                "contents",
            )?;
            let property = py.import("builtins")?.getattr("property")?;
            members.push(("contents", property.call1((contents,))?));
        }
        _ => {}
    }
    new_class(py, MODULE, name, "", vec![base.clone()], members)
}

/// LilyPond's input modes a mode-changing head may name.
const MODES: [&str; 5] = ["note", "drum", "figure", "chord", "lyric"];

/// The attributes of the first class along an object's method resolution
/// order that `CLASSES` has, with its name.
fn spec_of(
    object: &Bound<'_, PyAny>,
) -> PyResult<(&'static str, &'static [(&'static str, Default)])> {
    for class in object.get_type().mro().iter() {
        let name: String = class.getattr("__name__")?.extract()?;
        if let Some((found, fields)) = CLASSES.iter().find(|(known, _)| *known == name) {
            return Ok((found, fields));
        }
        if name == BASE {
            break;
        }
    }
    Ok((BASE, &[]))
}

/// `object.__setattr__`, below `LyObject`'s own.
fn plain_set(object: &Bound<'_, PyAny>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = object.py();
    py.get_type::<PyAny>()
        .getattr("__setattr__")?
        .call1((object, name, value))?;
    Ok(())
}

fn is_ly(object: &Bound<'_, PyAny>) -> PyResult<bool> {
    object.is_instance(&class(object.py(), BASE)?)
}

/// Sets the attributes every object has, then the class's own from what
/// was given, as each class's `__init__` does.
#[pyfunction]
#[pyo3(signature = (slf, *args, **kwargs))]
fn init(
    slf: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<()> {
    let py = slf.py();
    let none = py.None().into_bound(py);
    slf.setattr("lilyAttributes", PyDict::new(py))?;
    plain_set(slf, "_parent", &none)?;
    slf.setattr("thisIndent", 0)?;
    for name in [
        "markupTop",
        "lyricMarkupOrIdentifier",
        "markupListOrIdentifier",
        "markupTopOrIdentifier",
    ] {
        slf.setattr(name, &none)?;
    }

    let (class, fields) = spec_of(slf)?;
    if args.len() > fields.len() {
        return Err(PyTypeError::new_err(format!(
            "{class}.__init__() takes {} positional arguments but {} were given",
            fields.len() + 1,
            args.len() + 1
        )));
    }
    let mut given: Vec<Option<Bound<'_, PyAny>>> = vec![None; fields.len()];
    for (at, value) in args.iter().enumerate() {
        given[at] = Some(value);
    }
    if let Some(kwargs) = kwargs {
        for (key, value) in kwargs.iter() {
            let key: String = key.extract()?;
            let Some(at) = fields.iter().position(|(field, _)| *field == key) else {
                return Err(PyTypeError::new_err(format!(
                    "{class}.__init__() got an unexpected keyword argument '{key}'"
                )));
            };
            given[at] = Some(value);
        }
    }

    // Each attribute in order: a list default is a new list, and a None
    // given where a list is the default is one too.
    for ((field, default), value) in fields.iter().zip(given) {
        let value = match (
            value.filter(|value| !value.is_none() || !matches!(default, Default::List)),
            default,
        ) {
            (Some(value), _) => value,
            (None, Default::None) => py.None().into_bound(py),
            (None, Default::List) => PyList::empty(py).into_any(),
            (None, Default::False) => false.into_pyobject(py)?.to_owned().into_any(),
            (None, Default::True) => true.into_pyobject(py)?.to_owned().into_any(),
            (None, Default::Empty) => PyString::new(py, "").into_any(),
            (None, Default::Int(number)) => number.into_pyobject(py)?.into_any(),
            (None, Default::Text(text)) => PyString::new(py, text).into_any(),
        };
        slf.setattr(*field, value)?;
    }
    Ok(())
}

/// An object set as an attribute takes this one as its parent, and one in
/// a list or tuple takes it where it has none yet.
#[pyfunction]
fn set_attribute(slf: &Bound<'_, PyAny>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
    if is_ly(value)? {
        value.call_method1("setParent", (slf,))?;
    } else if iterable(value)? {
        for member in value.try_iter()? {
            let member = member?;
            if is_ly(&member)? && member.getattr("_parent")?.is_none() {
                member.call_method1("setParent", (slf,))?;
            }
        }
    }
    plain_set(slf, name, value)
}

/// music21's `common.isIterable`: anything with `__iter__` but text.
fn iterable(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if value.is_instance_of::<PyString>() || value.is_instance_of::<PyBytes>() {
        return Ok(false);
    }
    if value.is_instance_of::<pyo3::types::PyType>() {
        return Ok(false);
    }
    value.hasattr("__iter__")
}

#[pyfunction]
fn get_parent<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let parent = slf.getattr("_parent")?;
    if parent.is_none() {
        return Ok(parent);
    }
    parent.call0()
}

#[pyfunction]
fn set_parent(slf: &Bound<'_, PyAny>, parentObject: &Bound<'_, PyAny>) -> PyResult<()> {
    let reference = slf
        .py()
        .import("weakref")?
        .getattr("ref")?
        .call1((parentObject,))?;
    plain_set(slf, "_parent", &reference)
}

/// Every parent above, nearest first.
#[pyfunction]
fn ancestor_list<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let found = PyList::empty(slf.py());
    let mut current = slf.call_method0("getParent")?;
    while !current.is_none() {
        found.append(&current)?;
        current = current.call_method0("getParent")?;
    }
    Ok(found)
}

/// The `getAncestorNumber`th parent above of a class.
#[pyfunction]
#[pyo3(signature = (slf, classObj, getAncestorNumber = 1))]
fn ancestor_by_class<'py>(
    slf: &Bound<'py, PyAny>,
    classObj: &Bound<'py, PyAny>,
    getAncestorNumber: usize,
) -> PyResult<Bound<'py, PyAny>> {
    let mut seen = 0;
    for ancestor in ancestor_list(slf)?.iter() {
        if ancestor.is_instance(classObj)? {
            seen += 1;
            if seen == getAncestorNumber {
                return Ok(ancestor);
            }
        }
    }
    Ok(slf.py().None().into_bound(slf.py()))
}

/// A newline and a space for each parent above.
#[pyfunction]
fn newline_indent(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let depth = ancestor_list(slf)?.len();
    Ok(format!("\n{}", " ".repeat(depth)))
}

/// `lilyAttributes` from a music21 object, by the first of the supported
/// classes it is.
#[pyfunction]
fn set_attributes<'py>(
    slf: &Bound<'py, PyAny>,
    m21Object: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let classes = m21Object.getattr("classes")?;
    let supported = slf.getattr("supportedClasses")?;
    for tried in supported.try_iter()? {
        let tried = tried?;
        if classes.contains(&tried)? || tried.extract::<String>().is_ok_and(|name| name == "*") {
            return slf.call_method1("setAttributesFromClassObject", (tried, m21Object));
        }
    }
    Err(ly_error(format!(
        "Could not support setting attributes from {}: supported classes: {}",
        m21Object.str()?,
        supported.str()?
    )))
}

/// `lilyAttributes` from a music21 object by `m21toLy`'s table for a
/// class, a missing attribute taking its default or None.
#[pyfunction]
fn attributes_from_class<'py>(
    slf: &Bound<'py, PyAny>,
    classLookup: &Bound<'py, PyAny>,
    m21Object: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let table = slf.getattr("m21toLy")?;
    if !table.contains(classLookup)? {
        return Err(ly_error(format!(
            "Could not support setting attributes from {} error in self.m21toLy, \
             missing class definitions and no \"*\"",
            m21Object.str()?
        )));
    }
    let class_table = table.get_item(classLookup)?;
    let defaults = slf.getattr("defaultAttributes")?;
    let attributes = slf.getattr("lilyAttributes")?;
    for m21_attribute in class_table.try_iter()? {
        let m21_attribute = m21_attribute?;
        let name: String = m21_attribute.extract()?;
        let value = match m21Object.getattr(name.as_str()) {
            Ok(value) => value,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(slf.py()) => {
                match defaults.get_item(&m21_attribute) {
                    Ok(value) => value,
                    Err(_) => slf.py().None().into_bound(slf.py()),
                }
            }
            Err(error) => return Err(error),
        };
        attributes.set_item(class_table.get_item(&m21_attribute)?, value)?;
    }
    Ok(attributes)
}

/// The text, on one line, cut after ten characters.
#[pyfunction]
fn repr_internal(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let text = slf.str()?.to_string().replace('\n', "  ");
    if text.chars().count() >= 13 {
        return Ok(format!("{}...", text.chars().take(10).collect::<String>()));
    }
    Ok(text)
}

/// The output with blank lines closed up.
#[pyfunction]
fn to_str(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let output: String = slf.call_method0("stringOutput")?.extract()?;
    Ok(output.replace("\n\n", "\n"))
}

#[pyfunction]
fn first_not_none<'py>(
    slf: &Bound<'py, PyAny>,
    attributeList: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    for name in attributeList.try_iter()? {
        let value = slf.getattr(name?.extract::<String>()?.as_str())?;
        if !value.is_none() {
            return Ok(value);
        }
    }
    Ok(slf.py().None().into_bound(slf.py()))
}

/// Each item but None, as text, each followed by a newline and indent.
#[pyfunction]
fn newline_separate(slf: &Bound<'_, PyAny>, contents: &Bound<'_, PyAny>) -> PyResult<String> {
    let indent = newline_indent(slf)?;
    let mut out = String::new();
    for item in contents.try_iter()? {
        let item = item?;
        if item.is_none() {
            continue;
        }
        out.push_str(&item.str()?.to_string());
        out.push_str(&indent);
    }
    Ok(out)
}

/// Text in braces, each on its own indented line.
#[pyfunction]
fn enclose_curly(slf: &Bound<'_, PyAny>, arg: &Bound<'_, PyAny>) -> PyResult<String> {
    if arg.is_none() {
        return Ok(" { } ".to_string());
    }
    let indent = newline_indent(slf)?;
    let inner = if arg.is_instance_of::<PyList>() {
        let parts: Vec<String> = arg.extract()?;
        parts.join(&indent)
    } else {
        arg.str()?.to_string()
    };
    Ok(format!(" {{ {indent}{inner}{indent} }} {indent}"))
}

/// Text in double quotes, its quotes and backslashes escaped, and a space.
#[pyfunction]
fn quote_string(slf: &Bound<'_, PyAny>, stringIn: &str) -> String {
    let _ = slf;
    format!(
        "\"{}\" ",
        stringIn.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

/// A LilyPond comment.
#[pyfunction]
fn comment(slf: &Bound<'_, PyAny>, stringIn: &str) -> String {
    let _ = slf;
    format!(" %{{ {} %}} ", stringIn.trim())
}

/// `LyCompositeMusic.contents`: whichever music it holds.
#[pyfunction]
fn composite_contents<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let prefix = slf.getattr("prefixCompositeMusic")?;
    if !prefix.is_none() {
        return Ok(prefix);
    }
    slf.getattr("groupedMusicList")
}

/// An object's attributes read for its output.
struct Ly<'a, 'py> {
    object: &'a Bound<'py, PyAny>,
}

impl<'py> Ly<'_, 'py> {
    fn get(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.object.getattr(name)
    }

    fn has(&self, name: &str) -> PyResult<bool> {
        Ok(!self.get(name)?.is_none())
    }

    /// `self.name.stringOutput()`.
    fn so(&self, name: &str) -> PyResult<String> {
        output_of(&self.get(name)?)
    }

    /// `str(self.name)`.
    fn s(&self, name: &str) -> PyResult<String> {
        Ok(self.get(name)?.str()?.to_string())
    }

    fn indent(&self) -> PyResult<String> {
        newline_indent(self.object)
    }

    fn enclose(&self, arg: &Bound<'py, PyAny>) -> PyResult<String> {
        enclose_curly(self.object, arg)
    }

    fn enclose_text(&self, text: &str) -> PyResult<String> {
        self.enclose(&PyString::new(self.object.py(), text).into_any())
    }

    fn lines(&self, name: &str) -> PyResult<String> {
        newline_separate(self.object, &self.get(name)?)
    }

    fn first(&self, names: &[&str]) -> PyResult<Option<Bound<'py, PyAny>>> {
        for name in names {
            let value = self.get(name)?;
            if !value.is_none() {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    /// `str(x)` of each member, joined.
    fn joined(&self, name: &str, separator: &str) -> PyResult<String> {
        let mut parts = Vec::new();
        for item in self.get(name)?.try_iter()? {
            parts.push(item?.str()?.to_string());
        }
        Ok(parts.join(separator))
    }

    fn is_text(&self, name: &str) -> PyResult<bool> {
        Ok(self.get(name)?.is_instance_of::<PyString>())
    }

    fn truthy(&self, name: &str) -> PyResult<bool> {
        self.get(name)?.is_truthy()
    }
}

fn output_of(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.call_method0("stringOutput")?.str()?.to_string())
}

/// What the object writes as LilyPond: the output of the first class
/// along its method resolution order this module defines.
#[pyfunction]
fn string_output(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let (class, _) = spec_of(slf)?;
    output(class, &Ly { object: slf })
}

fn output(class: &str, ly: &Ly<'_, '_>) -> PyResult<String> {
    let b = BACKSLASH;
    Ok(match class {
        "LyLilypondTop" | "LyMusicList" | "LyLilypondHeaderBody" => {
            let list = if class == "LyLilypondHeaderBody" {
                "assignments"
            } else {
                "contents"
            };
            ly.lines(list)?
        }
        "LyTopLevelExpression" => {
            let names = [
                "lilypondHeader",
                "bookBlock",
                "bookPartBlock",
                "scoreBlock",
                "compositeMusic",
                "fullMarkup",
                "fullMarkupList",
                "outputDef",
            ];
            match ly.first(&names)? {
                Some(found) => output_of(&found)?,
                None => return Err(ly_error("Need an outputObject to report")),
            }
        }
        "LyLilypondHeader" => format!("{b}header{}", ly.enclose(&ly.get("lilypondHeaderBody")?)?),
        "LyEmbeddedScm" | "LyAssignmentId" => ly.s("content")?,
        "LyAssignment" => assignment(ly)?,
        "LyIdentifierInit" => identifier_init(ly)?,
        "LyContextDefSpecBlock" => {
            format!("{b}context {}", ly.enclose(&ly.get("contextDefSpecBody")?)?)
        }
        "LyContextDefSpecBody" => context_def_spec_body(ly)?,
        "LyBookBlock" => format!("{b}book {}", ly.enclose(&ly.get("bookBody")?)?),
        "LyBookBody" | "LyBookpartBody" => {
            if ly.has("bookIdentifier")? {
                ly.s("bookIdentifier")?
            } else if !ly.truthy("contents")? {
                String::new()
            } else {
                ly.lines("contents")?
            }
        }
        "LyBookpartBlock" => {
            let inner = if ly.has("bookpartBody")? {
                ly.so("bookpartBody")?
            } else {
                String::new()
            };
            format!("{b}bookpart {}", ly.enclose_text(&inner)?)
        }
        "LyScoreBlock" => {
            if !ly.has("scoreBody")? {
                return Err(ly_error("scoreBody object cannot be empty!"));
            }
            format!("{b}score {}", ly.enclose(&ly.get("scoreBody")?)?)
        }
        "LyScoreBody" => score_body(ly)?,
        "LyPaperBlock" => {
            if ly.has("outputDef")? {
                ly.so("outputDef")?
            } else {
                String::new()
            }
        }
        "LyLayout" => {
            let lines = [
                format!("{b}layout {{"),
                format!(" {b}context {{"),
                format!("   {b}RemoveEmptyStaves"),
                format!("   {b}override VerticalAxisGroup.remove-first = ##t"),
                " }".to_string(),
                "}".to_string(),
            ];
            let list = PyList::new(ly.object.py(), lines)?;
            newline_separate(ly.object, &list.into_any())?
        }
        "LyOutputDef" => {
            if !ly.has("outputDefBody")? {
                return Err(ly_error("Need outputDefBody to be set"));
            }
            format!("{}}}", ly.so("outputDefBody")?)
        }
        "LyOutputDefHead" => {
            let kind = ly.s("defType")?;
            if !["paper", "midi", "layout"].contains(&kind.as_str()) {
                return Err(ly_error(
                    "self.defType must be one of 'paper', 'midi', or 'layout'",
                ));
            }
            format!("{b}{kind}")
        }
        "LyOutputDefBody" => output_def_body(ly)?,
        "LyTempoEvent" => tempo_event(ly)?,
        "LyMusic" => match ly.first(&["simpleMusic", "compositeMusic"])? {
            Some(found) => output_of(&found)?,
            None => {
                return Err(ly_error(
                    "Need to define one of simpleMusic or compositeMusic",
                ));
            }
        },
        "LyAlternativeMusic" => {
            if ly.has("musicList")? {
                format!("{b}alternative{}", ly.enclose(&ly.get("musicList")?)?)
            } else {
                String::new()
            }
        }
        "LyRepeatedMusic" => {
            if !(ly.has("simpleString")? && ly.has("unsignedNumber")? && ly.has("music")?) {
                return Err(ly_error(
                    "need simpleString, unsignedNumber, and music to output repeated music",
                ));
            }
            let out = format!(
                "{b}repeat {}{}{}",
                ly.so("simpleString")?,
                ly.so("unsignedNumber")?,
                ly.so("music")?
            );
            if ly.has("alternativeMusic")? {
                format!("{out} {}", ly.so("alternativeMusic")?)
            } else {
                out
            }
        }
        "LySequentialMusic" => {
            let list = if ly.has("musicList")? {
                ly.so("musicList")?
            } else {
                String::new()
            };
            let tag = if ly.truthy("displayTag")? {
                format!("{b}sequential ")
            } else {
                String::new()
            };
            let before = if ly
                .get("beforeMatter")?
                .extract::<String>()
                .is_ok_and(|matter| matter == "startStaff")
            {
                format!("{b}startStaff ")
            } else {
                String::new()
            };
            format!("{tag}{{ {before}{list} }} {}", ly.indent()?)
        }
        "LyOssiaMusic" => {
            let (start, stop) = if ly.truthy("startstop")? {
                (format!("{b}startStaff "), format!("{b}stopStaff"))
            } else {
                (String::new(), String::new())
            };
            let list = if ly.has("musicList")? {
                ly.so("musicList")?
            } else {
                String::new()
            };
            format!("{{{start}{list} {stop}}}{}", ly.indent()?)
        }
        "LySimultaneousMusic" => {
            let list = if ly.has("musicList")? {
                ly.so("musicList")?
            } else {
                String::new()
            };
            if ly.get("displayTag")?.is_truthy()?
                && ly.get("displayTag")?.extract::<bool>().unwrap_or(false)
            {
                format!("{b}simultaneous {}", ly.enclose_text(&list)?)
            } else {
                let indent = ly.indent()?;
                format!("{indent}<< {list} >>{indent}")
            }
        }
        "LySimpleMusic" => match ly.first(&[
            "eventChord",
            "musicIdentifier",
            "musicPropertyDef",
            "contextChange",
        ])? {
            Some(found) => output_of(&found)?,
            None => return Err(ly_error("need one attribute set")),
        },
        "LyContextModification" => {
            if ly.has("contextModList")? {
                format!("{b}with {}", ly.enclose(&ly.get("contextModList")?)?)
            } else if ly.has("contextModIdentifier")? {
                format!("{b}with {}", ly.s("contextModIdentifier")?)
            } else {
                String::new()
            }
        }
        "LyContextModList" => {
            let out = ly.lines("contents")?;
            if ly.has("contextModIdentifier")? {
                format!("{out} {}", ly.s("contextModIdentifier")?)
            } else {
                out
            }
        }
        "LyCompositeMusic" => {
            let lyrics = if ly.has("newLyrics")? {
                ly.s("newLyrics")?
            } else {
                String::new()
            };
            match ly.first(&["prefixCompositeMusic", "groupedMusicList"])? {
                Some(found) => format!("{}\n{lyrics}", found.str()?),
                None => {
                    return Err(ly_error(
                        "Need to define either prefixCompositeMusic or groupedMusicList",
                    ));
                }
            }
        }
        "LyGroupedMusicList" => match ly.first(&["simultaneousMusic", "sequentialMusic"])? {
            Some(found) => found.str()?.to_string(),
            None => {
                return Err(ly_error(
                    "Need to define either simultaneousMusic or sequentialMusic",
                ));
            }
        },
        "LySchemeFunction" => {
            if ly.has("content")? {
                ly.s("content")?
            } else {
                String::new()
            }
        }
        "LyOptionalId" => {
            if ly.has("content")? {
                format!(" = {}", ly.s("content")?)
            } else {
                String::new()
            }
        }
        "LyPrefixCompositeMusic" => prefix_composite(ly)?,
        "LyModeChangingHead" => {
            if !ly.has("mode")? {
                return Err(ly_error("Mode must be set"));
            }
            let mode = ly.s("mode")?;
            if !MODES.contains(&mode.as_str()) {
                return Err(ly_error(format!("Not an allowable mode {mode}")));
            }
            if ly.truthy("hasContext")? || mode == "note" {
                format!("{b}{mode}mode")
            } else {
                format!("{b}{mode}s")
            }
        }
        "LyRelativeMusic" => {
            if !ly.has("content")? {
                return Err(ly_error("need content for relative music"));
            }
            format!("{b}relative {}", ly.so("content")?)
        }
        "LyNewLyrics" => {
            let mut out = String::new();
            for item in ly.get("groupedMusicLists")?.try_iter()? {
                let item = item?;
                out.push_str(&format!("{b}addlyrics "));
                if is_ly(&item)? {
                    out.push_str(&output_of(&item)?);
                } else {
                    out.push_str(&format!("{} ", item.str()?));
                }
            }
            out
        }
        "LyReRhythmedMusic" => {
            let music = ly.get("groupedMusic")?;
            let mut out = if is_ly(&music)? {
                output_of(&music)?
            } else if !music.is_none() {
                format!("{} ", music.str()?)
            } else {
                return Err(ly_error("need groupedMusic for re-rhythmed music"));
            };
            if ly.has("newLyrics")? {
                out.push_str(&ly.so("newLyrics")?);
            }
            out
        }
        "LyContextChange" => format!("{b}change {} = {} ", ly.s("before")?, ly.s("after")?),
        "LyPropertyPath" => {
            let mut parts = Vec::new();
            for item in ly.get("embeddedScheme")?.try_iter()? {
                parts.push(output_of(&item?)?);
            }
            parts.join(" ")
        }
        "LyPropertyOperation" => {
            let (one, two, three) = (ly.s("value1")?, ly.s("value2")?, ly.s("value3")?);
            match ly
                .get("mode")?
                .extract::<String>()
                .unwrap_or_default()
                .as_str()
            {
                "set" => format!("{b}set {one} = {two} "),
                "unset" => format!("{b}unset {one} "),
                "override" => format!("{b}override {one}.{two} = {three} "),
                "revert" => format!("{b}revert {one}.{two} "),
                _ => return Err(ly_error(format!("invalid mode {}", ly.s("mode")?))),
            }
        }
        "LyContextDefMod" => format!("{b}{} ", ly.s("contextDef")?),
        "LyContextMod" => {
            if !ly.has("contextDefOrProperty")? {
                return Err(ly_error(
                    "need a contextDef or property to modify a context",
                ));
            }
            let out = ly.so("contextDefOrProperty")?;
            if ly.has("scalar")? {
                format!("{out} {} ", ly.s("scalar")?)
            } else {
                out
            }
        }
        "LyMusicPropertyDef" => {
            if !ly.has("propertyDef")? {
                return Err(ly_error(
                    "need a propertyDef for a music property definition",
                ));
            }
            let once = if ly.truthy("isOnce")? {
                format!("{b}once ")
            } else {
                String::new()
            };
            format!("{once}{}", ly.so("propertyDef")?)
        }
        "LyEventChord" => event_chord(ly)?,
        "LyNoteChordElement" => {
            let mut out = ly.s("chordBody")?;
            if ly.has("optionalNoteModeDuration")? {
                out.push_str(&format!("{} ", ly.s("optionalNoteModeDuration")?));
            }
            for event in ly.get("postEvents")?.try_iter()? {
                out.push_str(&format!("{} ", event?.str()?));
            }
            out
        }
        "LyChordBody" => format!("< {} > ", ly.joined("chordBodyElements", " ")?),
        "LyChordBodyElement" | "LyNumberExpression" | "LyNumberTerm" => {
            let list = match class {
                "LyChordBodyElement" => "parts",
                _ => "numberAndSepList",
            };
            let joined = ly.joined(list, " ")?;
            if class == "LyChordBodyElement" {
                joined
            } else {
                format!("{joined} ")
            }
        }
        "LyCommandElement" => command_element(ly)?,
        "LyCommandEvent" => command_event(ly)?,
        "LyPostEvents" => {
            let mut parts = Vec::new();
            for event in ly.get("eventList")?.try_iter()? {
                parts.push(output_of(&event?)?);
            }
            parts.join(" ")
        }
        "LyPostEvent" => {
            let mut out = ly.s("arg1")?;
            if ly.has("arg2")? {
                out.push_str(&format!(" {}", ly.s("arg2")?));
            }
            format!("{out} ")
        }
        "LyDirectionLessEvent" | "LyDirectionReqdEvent" => format!("{} ", ly.s("event")?),
        "LyOctaveCheck" => {
            let check = ly.get("equalOrQuotesOrNone")?;
            if check.is_none() {
                String::new()
            } else if check.extract::<String>().is_ok_and(|check| check == "=") {
                "= ".to_string()
            } else {
                format!("= {} ", check.str()?)
            }
        }
        "LyPitch" => format!("{}{} ", ly.s("noteNamePitch")?, ly.s("quotes")?),
        "LyGenTextDef" | "LyScriptAbbreviation" | "LyScriptDir" => format!("{} ", ly.s("value")?),
        "LyStenoDuration" => {
            let dots: usize = ly.get("numDots")?.extract()?;
            format!("{}{} ", ly.s("durationNumber")?, ".".repeat(dots))
        }
        "LyMultipliedDuration" => {
            if !ly.has("stenoDur")? {
                String::new()
            } else {
                let mut out = ly.s("stenoDur")?;
                for factor in ly.get("multiply")?.try_iter()? {
                    out.push_str(&format!(" * {}", factor?.str()?));
                }
                out
            }
        }
        "LyTremoloType" => {
            if ly.has("tremTypeOrNone")? {
                format!(":{} ", ly.s("tremTypeOrNone")?)
            } else {
                ": ".to_string()
            }
        }
        "LyOptionalRest" => {
            let rest = ly.get("rest")?;
            if rest.extract::<bool>().is_ok_and(|rest| !rest) {
                String::new()
            } else {
                format!("{b}rest ")
            }
        }
        "LySimpleElement" => ly.joined("parts", "")?,
        "LyLyricElement" => format!("{} ", ly.s("lyMarkupOrString")?),
        "LyTempoRange" => {
            if ly.has("highestTempoOrNone")? {
                format!(
                    "{}-{} ",
                    ly.s("lowestOrOnlyTempo")?,
                    ly.s("highestTempoOrNone")?
                )
            } else {
                format!("{} ", ly.s("lowestOrOnlyTempo")?)
            }
        }
        "LyLyricMarkup" => {
            if ly.has("markupTop")? {
                format!(
                    "{} {}",
                    ly.s("lyricMarkupOrIdentifier")?,
                    ly.s("markupTop")?
                )
            } else {
                format!("{} ", ly.s("lyricMarkupOrIdentifier")?)
            }
        }
        "LyFullMarkupList" | "LyFullMarkup" => {
            let (name, word) = if class == "LyFullMarkup" {
                ("markupTopOrIdentifier", "markup")
            } else {
                ("markupListOrIdentifier", "markuplist")
            };
            if ly.is_text(name)? {
                format!("{} ", ly.s(name)?)
            } else if !ly.has(name)? {
                return Err(ly_error(format!("need a {word} or identifier")));
            } else {
                format!("{b}{word} {}", ly.so(name)?)
            }
        }
        "LyMarkupTop" => {
            if ly.has("argument2")? {
                format!("{} {}", ly.s("argument1")?, ly.s("argument2")?)
            } else {
                ly.s("argument1")?
            }
        }
        "LyMarkupList" => ly.s("markupIdentifierOrList")?,
        "LyMarkupComposedList" => {
            format!("{} {}", ly.s("markupHeadList")?, ly.s("markupBracedList")?)
        }
        "LyMarkupBracedList" => format!("{{ {} }}", ly.s("listBody")?),
        "LyMarkupBracedListBody" => {
            let mut out = String::new();
            for item in ly.get("markupOrMarkupList")?.try_iter()? {
                out.push_str(&format!("{} ", item?.str()?));
            }
            out
        }
        "LySimpleMarkup" => match ly.s("simpleType")?.as_str() {
            "string" => format!("{} ", ly.s("argument1")?),
            "score-body" => format!("{b}score {{ {} }} ", ly.s("argument1")?),
            "markup-function" => format!("{} {} ", ly.s("argument1")?, ly.s("argument2")?),
            other => return Err(ly_error(format!("unknown simpleType {other}"))),
        },
        "LyMarkup" => {
            let head = if ly.has("optionalMarkupHeadList")? {
                format!("{} ", ly.s("optionalMarkupHeadList")?)
            } else {
                String::new()
            };
            format!("{head}{}", ly.s("simpleMarkup")?)
        }
        _ => String::new(),
    })
}

fn assignment(ly: &Ly<'_, '_>) -> PyResult<String> {
    if ly.has("embeddedScm")? {
        return ly.so("embeddedScm");
    }
    if !ly.has("assignmentId")? || !ly.has("identifierInit")? {
        return Err(ly_error("need an assignmentId or identifierInit"));
    }
    if ly.has("propertyPath")? {
        return Ok(format!(
            "{} {} = {} ",
            ly.s("assignmentId")?,
            ly.so("propertyPath")?,
            ly.so("identifierInit")?
        ));
    }
    Ok(format!(
        "{} = {}  ",
        ly.s("assignmentId")?,
        ly.so("identifierInit")?
    ))
}

fn identifier_init(ly: &Ly<'_, '_>) -> PyResult<String> {
    let names = [
        "scoreBlock",
        "bookBlock",
        "bookPartBlock",
        "outputDef",
        "contextDefSpecBlock",
        "music",
        "postEvent",
        "numberExpression",
        "string",
        "embeddedScm",
        "fullMarkup",
        "fullMarkupList",
        "digit",
        "contextModification",
    ];
    for name in names {
        let value = ly.get(name)?;
        if value.is_none() {
            continue;
        }
        return match name {
            "digit" => Ok(value.str()?.to_string()),
            "string" => Ok(quote_string(ly.object, &value.str()?.to_string())),
            _ => output_of(&value),
        };
    }
    Err(ly_error("need an outputObject"))
}

fn context_def_spec_body(ly: &Ly<'_, '_>) -> PyResult<String> {
    let body = if ly.has("contextDefSpecBody")? {
        Some(ly.s("contextDefSpecBody")?)
    } else {
        None
    };
    if ly.has("contextDefIdentifier")? {
        return ly.s("contextDefIdentifier");
    }
    if ly.has("embeddedScm")? {
        let head = body
            .map(|body| format!("{body} {BACKSLASH}grobdescriptions "))
            .unwrap_or_default();
        return Ok(format!("{head}{}", ly.so("embeddedScm")?));
    }
    for name in ["contextMod", "contextModification"] {
        if ly.has(name)? {
            let inner = ly.so(name)?;
            return Ok(match &body {
                Some(body) => format!("{body} {inner}"),
                None => inner,
            });
        }
    }
    Ok(String::new())
}

fn score_body(ly: &Ly<'_, '_>) -> PyResult<String> {
    if ly.has("music")? {
        return ly.so("music");
    }
    if ly.has("scoreIdentifier")? {
        return ly.s("scoreIdentifier");
    }
    if !ly.has("scoreBody")? {
        return Err(ly_error(
            "scoreBody cannot be None if music and scoreIdentifier are None",
        ));
    }
    for name in ["lilypondHeader", "outputDef", "error"] {
        if ly.has(name)? {
            return Ok(format!("{} {}", ly.so("scoreBody")?, ly.so(name)?));
        }
    }
    Err(ly_error(
        "one of music, scoreIdentifier, lilypondHeader, outputDef, or error must not be None",
    ))
}

fn output_def_body(ly: &Ly<'_, '_>) -> PyResult<String> {
    if ly.has("outputDefHead")? {
        let out = format!("{} {{ ", ly.s("outputDefHead")?);
        if ly.has("outputDefIdentifier")? {
            return Ok(out + &ly.s("outputDefIdentifier")?);
        }
        return Ok(out);
    }
    if !ly.has("outputDefBody")? {
        return Err(ly_error(
            "Need embedded outputDefBody if outputDefIdentifier or outputDefHead are not defined",
        ));
    }
    for name in ["assignment", "contextDefSpecBlock", "error"] {
        if ly.has(name)? {
            return Ok(format!("{} {}", ly.so("outputDefBody")?, ly.so(name)?));
        }
    }
    Err(ly_error(
        "Need to define at least one of assignment, contextDefSpecBlock, or error if outputDefHead is None",
    ))
}

fn tempo_event(ly: &Ly<'_, '_>) -> PyResult<String> {
    let base = format!("{BACKSLASH}tempo");
    if ly.has("tempoRange")? {
        if !ly.has("stenoDuration")? {
            return Err(ly_error(
                "If tempoRange is defined then need a stenoDuration",
            ));
        }
        let range = ly.so("tempoRange")?;
        let steno = ly.so("stenoDuration")?;
        if ly.has("scalar")? {
            return Ok(format!("{base} {} {steno} = {range}", ly.s("scalar")?));
        }
        return Ok(format!("{base} {steno} = {range}"));
    }
    if ly.has("stenoDuration")? {
        if !ly.has("scalar")? {
            return Err(ly_error(
                "If tempoRange is not defined but stenoDuration is, need a scalar",
            ));
        }
        return Ok(format!(
            "{base} {} = {}",
            ly.so("stenoDuration")?,
            ly.s("scalar")?
        ));
    }
    if !ly.has("scalar")? {
        return Err(ly_error("If tempoRange is not defined then need scalar"));
    }
    Ok(format!("{base} {}", ly.s("scalar")?))
}

fn prefix_composite(ly: &Ly<'_, '_>) -> PyResult<String> {
    let kind = ly.get("type")?.extract::<String>().unwrap_or_default();
    let b = BACKSLASH;
    Ok(match kind.as_str() {
        "scheme" => ly.s("genericPrefixMusicScm")?,
        "context" | "new" => {
            let mut out = format!("{b}{kind} {} ", ly.s("simpleString")?);
            if ly.has("optionalId")? {
                out.push_str(&format!("{} ", ly.s("optionalId")?));
            }
            if ly.has("optionalContextMod")? {
                out.push_str(&format!("{} ", ly.s("optionalContextMod")?));
            }
            out.push_str(&format!("{} ", ly.s("music")?));
            out
        }
        "tuplet" => format!("{b}tuplet {} {} ", ly.s("fraction")?, ly.s("music")?),
        "repeated" => ly.s("repeatedMusic")?,
        "transpose" => format!(
            "{b}transpose {} {} {} ",
            ly.s("pitchAlsoInChords1")?,
            ly.s("pitchAlsoInChords2")?,
            ly.s("music")?
        ),
        "modeChanging" => format!(
            "{} {}",
            ly.s("modeChangingHead")?,
            ly.s("groupedMusicList")?
        ),
        "modeChangingWith" => {
            let mut out = format!("{} ", ly.s("modeChangingHeadWithContext")?);
            if ly.has("optionalContextMod")? {
                out.push_str(&format!("{} ", ly.s("optionalContextMod")?));
            }
            out.push_str(&format!("{} ", ly.s("groupedMusicList")?));
            out
        }
        "relative" => ly.s("relativeMusic")?,
        "rhythmed" => ly.s("reRhythmedMusic")?,
        _ => {
            return Err(ly_error(format!(
                "unknown self.type or None: {}",
                ly.s("type")?
            )));
        }
    })
}

fn event_chord(ly: &Ly<'_, '_>) -> PyResult<String> {
    if ly.has("noteChordElement")? {
        return Ok(format!("{} ", ly.s("noteChordElement")?));
    }
    if ly.has("commandElement")? {
        return Ok(format!("{} ", ly.s("commandElement")?));
    }
    let events = ly.get("postEvents")?;
    let (head, joiner) = if ly.has("multiMeasureRest")? {
        ("multiMeasureRest", "")
    } else if ly.has("chordRepetition")? {
        ("chordRepetition", " ")
    } else {
        ("simpleChordElements", " ")
    };
    let mut out = ly.s(head)?;
    if head != "simpleChordElements" && ly.has("duration")? {
        out.push_str(&format!(" {}", ly.s("duration")?));
    }
    if !events.is_none() {
        for event in events.try_iter()? {
            out.push_str(joiner);
            out.push_str(&event?.str()?.to_string());
        }
    }
    out.push(' ');
    Ok(out)
}

fn command_element(ly: &Ly<'_, '_>) -> PyResult<String> {
    let kind = ly.get("commandType")?;
    let b = BACKSLASH;
    let Ok(kind) = kind.extract::<String>() else {
        if kind.is_none() {
            return Err(ly_error("need a commandType to output a command element"));
        }
        return output_of(&kind);
    };
    let argument = ly.get("argument")?;
    Ok(match kind.as_str() {
        "skip" | "partial" => {
            let written = if is_ly(&argument)? {
                output_of(&argument)?
            } else {
                argument.str()?.to_string()
            };
            format!("{b}{kind} {written}")
        }
        "[" => format!("{b}[ "),
        "]" => format!("{b}] "),
        BACKSLASH => format!("{kind} "),
        "time" | "mark" => format!("{b}{kind} {} ", argument.str()?),
        _ => return Err(ly_error(format!("unknown commandType {kind}"))),
    })
}

fn command_event(ly: &Ly<'_, '_>) -> PyResult<String> {
    let kind = ly.get("commandType")?;
    let b = BACKSLASH;
    if let Ok(text) = kind.extract::<String>() {
        return Ok(match text.as_str() {
            "~" => format!("{b}~ "),
            "mark-default" => format!("{b}mark {b}default "),
            "key-default" => format!("{b}key {b}default "),
            "key" => format!("{b}key {} {} ", ly.s("argument1")?, ly.s("argument2")?),
            _ => return Err(ly_error(format!("unknown commandType {text}"))),
        });
    }
    if kind.is_instance(&class(kind.py(), "LyTempoEvent")?)? {
        return output_of(&kind);
    }
    Err(ly_error(format!("unknown commandType {}", kind.str()?)))
}
