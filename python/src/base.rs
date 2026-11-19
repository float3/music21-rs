//! music21's `base.Music21Object`, `note.GeneralNote` and `note.NotRest`:
//! the classes every element of the package's is built on, as music21's
//! are.
//!
//! They carry no music. Each element class keeps everything it answers in
//! its own value; these are here so that `isinstance(n, note.GeneralNote)`
//! and `issubclass(Note, base.Music21Object)` say what they say in music21,
//! and so that code written for music21 can subclass them. What they hold
//! is music21's `groups` and `derivation`, which every `Music21Object` has.
//!
//! An installed class stands on music21's own classes as well; its chain
//! has these first and music21's after, and music21's `groups` is the one
//! it keeps.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::{PyClass, PyClassInitializer, PyTypeInfo};

use crate::prebase::ProtoM21Object;

pyo3::create_exception!(music21_rs_facade, ElementException, crate::Music21Exception);
pyo3::create_exception!(music21_rs_facade, GroupException, crate::Music21Exception);

/// The names this module hands out.
pub const NAMES: &[&str] = &[
    "Music21Object",
    "GeneralNote",
    "NotRest",
    "Groups",
    "ElementWrapper",
];

/// music21's `Music21Object`: what a stream holds.
#[pyclass(name = "Music21Object", module = "music21.base", extends = ProtoM21Object, subclass)]
#[derive(Default)]
pub struct Music21Object {
    /// music21's `groups`, made on first asking.
    groups: Option<Py<PyAny>>,
    /// music21's `derivation`: what this was made from, made on first
    /// asking or by what made it.
    derivation: Option<Py<PyAny>>,
    /// music21's `_style`: made of the class's `_styleClass` on first
    /// asking, where the class keeps no style of its own.
    style: Option<Py<PyAny>>,
    /// music21's `_duration`, where the class keeps no duration of its
    /// own: a `Duration` of nothing, made on first asking, until one is
    /// set.
    duration: Option<Py<PyAny>>,
    /// music21's `id`, where one has been given.
    id: Option<Py<PyAny>>,
}

#[pymethods]
impl Music21Object {
    /// A bare one, as `base.Music21Object()` builds, taking music21's
    /// `groups` keyword. Whatever else is given is for a subclass.
    #[new]
    #[pyo3(signature = (*_arguments, **keywords))]
    pub(crate) fn new(
        _arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let given = |name: &str| -> PyResult<Option<Bound<'_, PyAny>>> {
            Ok(match keywords {
                Some(keywords) => keywords.get_item(name)?.filter(|value| !value.is_none()),
                None => None,
            })
        };
        let groups = given("groups")?.map(Bound::unbind);
        let mut duration = given("duration")?.map(Bound::unbind);
        if let Some(length) = given("quarterLength")? {
            let py = length.py();
            let made = crate::duration::Duration::object(
                py,
                music21::Duration::new(0.0)
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?,
            )?;
            made.bind(py).setattr("quarterLength", length)?;
            duration = Some(made);
        }
        Ok(
            PyClassInitializer::from(ProtoM21Object::default()).add_subclass(Self {
                groups,
                derivation: None,
                style: None,
                duration,
                id: given("id")?.filter(|id| !id.is_none()).map(Bound::unbind),
            }),
        )
    }

    /// music21's `groups`: the names of the groups this belongs to, as a
    /// `Groups`, a list of strings.
    #[getter]
    fn get_groups(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(groups) = &self.groups {
            return Ok(groups.clone_ref(py));
        }
        let made = groups_class(py)?.call0()?.unbind();
        self.groups = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_groups(&mut self, value: Py<PyAny>) {
        self.groups = Some(value);
    }

    /// music21's `id`: the one given, or else Python's `id()` of the
    /// object, a number of its own.
    #[getter]
    fn get_id(slf: &Bound<'_, Self>) -> Py<PyAny> {
        let py = slf.py();
        match &slf.borrow().id {
            Some(given) => given.clone_ref(py),
            None => (slf.as_ptr() as usize)
                .into_pyobject(py)
                .map(|id| id.into_any().unbind())
                .unwrap_or_else(|_| py.None()),
        }
    }

    #[setter]
    fn set_id(&mut self, value: Option<Py<PyAny>>) {
        self.id = value;
    }

    /// Where among things at one offset this class sorts: music21's
    /// `classSortOrder`, 20 for a class that says nothing, as a note does.
    #[classattr]
    fn classSortOrder() -> i64 {
        20
    }

    /// music21's `derivation`: what this was made from and how, a
    /// `Derivation` of nothing until something made it.
    #[getter]
    fn get_derivation(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(derivation) = &slf.borrow().derivation {
            return Ok(derivation.clone_ref(py));
        }
        let made = Py::new(
            py,
            crate::derivation::Derivation::of(slf.as_any(), None, None),
        )?;
        slf.borrow_mut().derivation = Some(made.clone_ref(py).into_any());
        Ok(made.into_any())
    }

    #[setter]
    fn set_derivation(&mut self, value: Option<Py<PyAny>>) {
        self.derivation = value;
    }

    /// music21's `style`: how this is drawn, an object of the class's
    /// `_styleClass` made the first time it is asked for, and the same
    /// object after that.
    #[getter]
    fn get_style(slf: &Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(style) = &slf.borrow().style {
            return Ok(style.clone_ref(py));
        }
        let made = style_class(slf.as_any())?.call0()?.unbind();
        slf.borrow_mut().style = Some(made.clone_ref(py));
        Ok(made)
    }

    #[setter]
    fn set_style(&mut self, value: Py<PyAny>) {
        self.style = Some(value);
    }

    /// Whether a `style` has been made yet.
    #[getter]
    fn hasStyleInformation(&self) -> bool {
        self.style.is_some()
    }

    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.groups)?;
        visit.call(&self.derivation)?;
        visit.call(&self.style)?;
        visit.call(&self.duration)?;
        visit.call(&self.id)
    }

    fn __clear__(&mut self) {
        self.groups = None;
        self.derivation = None;
        self.id = None;
        self.style = None;
        self.duration = None;
    }
}

/// music21's `GeneralNote`: a note, a rest or a chord.
#[pyclass(name = "GeneralNote", module = "music21.note", extends = Music21Object, subclass)]
#[derive(Default)]
pub struct GeneralNote;

#[pymethods]
impl GeneralNote {
    #[new]
    #[pyo3(signature = (*arguments, **keywords))]
    fn new(
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Music21Object::new(arguments, keywords)?.add_subclass(Self))
    }
}

/// music21's `NotRest`: a note or a chord, anything sounding.
#[pyclass(name = "NotRest", module = "music21.note", extends = GeneralNote, subclass)]
#[derive(Default)]
pub struct NotRest;

#[pymethods]
impl NotRest {
    #[new]
    #[pyo3(signature = (*arguments, **keywords))]
    fn new(
        arguments: &Bound<'_, PyTuple>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(GeneralNote::new(arguments, keywords)?.add_subclass(Self))
    }
}

/// music21's `ElementWrapper`: an object that is not a `Music21Object`,
/// carried as one so a stream can hold it.
///
/// The wrapped object is `obj`. An attribute the wrapper does not have is
/// read off it, and one it has is set on it, unless `Music21Object` has an
/// attribute of that name: an `offset` is always the wrapper's own.
#[pyclass(name = "ElementWrapper", module = "music21.base", extends = Music21Object, subclass, dict, weakref)]
pub struct ElementWrapper {
    /// The object carried.
    obj: Py<PyAny>,
    /// The `id` given, where one has been.
    #[pyo3(get, set)]
    _id: Option<Py<PyAny>>,
}

#[pymethods]
impl ElementWrapper {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.obj)?;
        visit.call(&self._id)
    }

    fn __clear__(&mut self) {
        Python::attach(|py| self.obj = py.None());
        self._id = None;
    }

    #[new]
    #[pyo3(signature = (obj=None, **keywords))]
    fn new(
        py: Python<'_>,
        obj: Option<Py<PyAny>>,
        keywords: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let wrapper = Self {
            obj: obj.unwrap_or_else(|| py.None()),
            _id: None,
        };
        Ok(Music21Object::new(&PyTuple::empty(py), keywords)?.add_subclass(wrapper))
    }

    #[classattr]
    fn equalityAttributes() -> (&'static str,) {
        ("obj",)
    }

    #[classattr]
    fn _DOC_ORDER() -> Vec<&'static str> {
        vec!["obj"]
    }

    #[classattr]
    fn _DOC_ATTR(py: Python<'_>) -> PyResult<Py<PyDict>> {
        let made = PyDict::new(py);
        made.set_item(
            "obj",
            "The object this wraps. It should not be a Music21Object, which a stream \
             can hold as it is.",
        )?;
        Ok(made.unbind())
    }

    /// The object carried.
    #[getter]
    fn get_obj(&self, py: Python<'_>) -> Py<PyAny> {
        self.obj.clone_ref(py)
    }

    #[setter]
    fn set_obj(&mut self, value: Py<PyAny>) {
        self.obj = value;
    }

    /// The `id` given, or Python's `id()` of the wrapper where none has
    /// been, as music21's `Music21Object.id` answers.
    #[getter]
    fn get_id(slf: &Bound<'_, Self>) -> Py<PyAny> {
        let py = slf.py();
        match &slf.borrow()._id {
            Some(given) => given.clone_ref(py),
            None => (slf.as_ptr() as usize)
                .into_pyobject(py)
                .map(|id| id.into_any().unbind())
                .unwrap_or_else(|_| py.None()),
        }
    }

    #[setter]
    fn set_id(&mut self, value: Option<Py<PyAny>>) {
        self._id = value;
    }

    /// `id=... offset=... obj='...'`, the object shown in thirty
    /// characters at most.
    fn _reprInternal(slf: &Bound<'_, Self>) -> PyResult<String> {
        let py = slf.py();
        let shown: String = slf.borrow().obj.bind(py).str()?.to_string();
        let mut short: String = shown.chars().take(30).collect();
        if shown.chars().count() > 30 {
            short.push_str("...");
            if short.starts_with('<') {
                short.push('>');
            }
        }
        let short = short.into_pyobject(py)?.repr()?.to_string();
        let offset = slf.getattr("offset")?.str()?.to_string();
        if slf.borrow()._id.is_some() {
            let id = slf.getattr("id")?.str()?.to_string();
            return Ok(format!("id={id} offset={offset} obj={short}"));
        }
        Ok(format!("offset={offset} obj={short}"))
    }

    /// Sets `obj` on the wrapper, and anything else on the object carried
    /// where that has the attribute and `Music21Object` has not.
    fn __setattr__(slf: &Bound<'_, Self>, name: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let own = |value: &Bound<'_, PyAny>| crate::plain_setattr(slf, name, value);
        if name == "obj" {
            slf.borrow_mut().obj = value.unbind();
            return Ok(());
        }
        if slf.getattr("__dict__")?.contains(name)? {
            own(&value)?;
        }
        let stored = slf.borrow().obj.clone_ref(py).into_bound(py);
        let reserved = music21_object_keys(py)?.contains(name)?;
        if !reserved && !stored.is_none() && stored.hasattr(name)? {
            return stored.setattr(name, value);
        }
        own(&value)
    }

    /// An attribute the wrapper has not, read off the object carried.
    fn __getattr__<'py>(slf: &Bound<'py, Self>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let stored = slf.borrow().obj.clone_ref(py).into_bound(py);
        if stored.is_none() {
            return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                "Could not get attribute {name:?} in an object-less element"
            )));
        }
        py.import("builtins")?
            .getattr("object")?
            .call_method1("__getattribute__", (stored, name))
    }

    /// Equal where the other is of this class and every attribute named in
    /// an `equalityAttributes` along the class's chain is equal.
    fn __eq__(slf: &Bound<'_, Self>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        if !other.is_instance(&slf.get_type())? {
            return Ok(false);
        }
        for name in equality_attributes(&slf.get_type())? {
            let mine = slf.getattr(name.as_str()).ok();
            let theirs = other.getattr(name.as_str()).ok();
            match (mine, theirs) {
                (Some(mine), Some(theirs)) => {
                    if !mine.eq(theirs)? {
                        return Ok(false);
                    }
                }
                // Neither has it: the wheel's `Music21Object` has no
                // `duration` to compare.
                (None, None) => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn __hash__(slf: &Bound<'_, Self>) -> isize {
        (slf.as_ptr() as usize >> 4) as isize
    }
}

/// The names music21 collects from every `equalityAttributes` along a
/// class's chain.
fn equality_attributes(class: &Bound<'_, pyo3::types::PyType>) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for ancestor in class.mro().iter() {
        let Ok(named) = ancestor.getattr("equalityAttributes") else {
            continue;
        };
        let named: Vec<String> = match named.extract::<String>() {
            Ok(one) => vec![one],
            Err(_) => named.extract()?,
        };
        for name in named {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Ok(names)
}

static MUSIC21_OBJECT_KEYS: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// Every attribute a bare `Music21Object` has: music21's
/// `_m21ObjDefaultDefinedKeys`, which an `ElementWrapper` keeps for itself.
fn music21_object_keys(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let held = MUSIC21_OBJECT_KEYS.get_or_try_init(py, || {
        let bare = Bound::new(py, ProtoM21Object::under(Music21Object::default()))?;
        let names = py.import("builtins")?.call_method1("dir", (bare,))?;
        Ok::<_, PyErr>(
            pyo3::types::PyFrozenSet::new(py, names.try_iter()?.collect::<PyResult<Vec<_>>>()?)?
                .into_any()
                .unbind(),
        )
    })?;
    Ok(held.bind(py).clone())
}

/// Gives a copy the groups of what it was copied from, as music21's
/// `deepcopy` does: its own list, holding the same names.
pub(crate) fn carry_groups(source: &Bound<'_, PyAny>, copy: &Bound<'_, PyAny>) -> PyResult<()> {
    let (Ok(source), Ok(copy)) = (source.cast::<Music21Object>(), copy.cast::<Music21Object>())
    else {
        return Ok(());
    };
    let Some(groups) = source
        .borrow()
        .groups
        .as_ref()
        .map(|groups| groups.clone_ref(source.py()))
    else {
        return Ok(());
    };
    let groups = groups.into_bound(source.py());
    let copied = groups.get_type().call1((&groups,))?;
    copy.borrow_mut().groups = Some(copied.unbind());
    Ok(())
}

/// A copy's own copies of the style and the duration kept here, as
/// music21's deep copy makes them; a class keeping either elsewhere
/// copies it itself.
fn carry_style(source: &Bound<'_, PyAny>, copy: &Bound<'_, PyAny>) -> PyResult<()> {
    let (Ok(source), Ok(copy)) = (source.cast::<Music21Object>(), copy.cast::<Music21Object>())
    else {
        return Ok(());
    };
    let py = source.py();
    let deep = |held: &Option<Py<PyAny>>| -> PyResult<Option<Py<PyAny>>> {
        match held {
            Some(held) => Ok(Some(
                py.import("copy")?
                    .call_method1("deepcopy", (held.bind(py),))?
                    .unbind(),
            )),
            None => Ok(None),
        }
    };
    let (style, duration) = {
        let source = source.borrow();
        (deep(&source.style)?, deep(&source.duration)?)
    };
    if let Some(style) = style {
        copy.borrow_mut().style = Some(style);
    }
    if let Some(duration) = duration {
        crate::duration::adopt_duration(py, &duration, copy.as_any())?;
        copy.borrow_mut().duration = Some(duration);
    }
    Ok(())
}

/// music21's `Music21Object.duration`, read: the duration set, or a
/// `Duration` of nothing made the first time it is asked for.
#[pyfunction]
fn duration_of(slf: &Bound<'_, Music21Object>) -> PyResult<Py<PyAny>> {
    let py = slf.py();
    if let Some(duration) = &slf.borrow().duration {
        return Ok(duration.clone_ref(py));
    }
    let empty = music21::Duration::new(0.0)
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    let made = crate::duration::Duration::object(py, empty)?;
    slf.borrow_mut().duration = Some(made.clone_ref(py));
    Ok(made)
}

/// music21's `Music21Object.duration`, written: the old duration let go,
/// the new one told what holds it, and the sites told of the change where
/// there was a duration before. Anything with no `quarterLength` is no
/// duration.
#[pyfunction]
fn set_duration_of(slf: &Bound<'_, Music21Object>, value: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    let old = slf.borrow().duration.as_ref().map(|old| old.clone_ref(py));
    if let Some(old) = &old {
        let old = old.bind(py);
        if !crate::duration::is_frozen(old) {
            old.setattr("client", py.None())?;
        }
    }
    let Ok(length) = value.getattr("quarterLength") else {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "this must be a Duration object, not {}",
            value.str()?
        )));
    };
    let value = value.clone().unbind();
    slf.borrow_mut().duration = Some(value.clone_ref(py));
    crate::duration::adopt_duration(py, &value, slf.as_any())?;
    if old.is_some() {
        crate::duration::told_sites(slf.as_any(), &length)?;
    }
    Ok(())
}

/// The class an object's `style` is made of: the `_styleClass` its class
/// names, as a Python subclass of music21's names it, or else the one
/// music21 gives the wheel's class -- a `TextStyle` for an expression, a
/// plain `Style` for anything else.
///
/// Not a class attribute here, since the wheel reads one as a class's own
/// choice: a note with none of its own is drawn with a note's style.
fn style_class<'py>(object: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = object.py();
    if let Ok(named) = object.get_type().getattr("_styleClass") {
        return Ok(named);
    }
    if object.is_instance_of::<crate::expressions::Expression>() {
        return Ok(py.get_type::<crate::style::TextStyle>().into_any());
    }
    Ok(py.get_type::<crate::notation::Style>().into_any())
}

/// Gives `Music21Object` its `duration`, once the module is made.
///
/// The `duration` is a Python `property`, as music21's is, rather than an
/// attribute of the class's own: music21's `Chord` takes the getter of the
/// one it inherits and puts its own beside the setter.
pub(crate) fn add_late_members(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    let duration = py.import("builtins")?.getattr("property")?.call1((
        wrap_pyfunction!(duration_of, m)?,
        wrap_pyfunction!(set_duration_of, m)?,
        py.None(),
        "Get and set the duration of this object as a Duration object.",
    ))?;
    m.getattr("Music21Object")?.setattr("duration", duration)?;
    Ok(())
}

/// The derivation an object has, without making one for an object that has
/// none: the package's own, or music21's `_derivation`.
pub(crate) fn derivation_held<'py>(object: &Bound<'py, PyAny>) -> Option<Bound<'py, PyAny>> {
    let py = object.py();
    if let Ok(ours) = object.cast::<Music21Object>()
        && let Some(held) = &ours.borrow().derivation
    {
        return Some(held.bind(py).clone());
    }
    object
        .getattr(pyo3::intern!(py, "_derivation"))
        .ok()
        .filter(|held| !held.is_none())
}

/// Says of a copy that it was copied from `source`, as music21's
/// `deepcopy` does: a derivation via `'__deepcopy__'`, and the groups.
pub(crate) fn copied_from(source: &Bound<'_, PyAny>, copy: &Bound<'_, PyAny>) -> PyResult<()> {
    carry_groups(source, copy)?;
    carry_style(source, copy)?;
    crate::derived_from(copy, source, "__deepcopy__")
}

/// A copy as `copy.copy` leaves it: music21 records a derivation only on
/// `deepcopy`, so this forgets the one the copy was given.
pub(crate) fn shallow(copy: Bound<'_, PyAny>) -> Bound<'_, PyAny> {
    if let Ok(ours) = copy.cast::<Music21Object>() {
        ours.borrow_mut().derivation = None;
    }
    copy
}

/// What a class of the package's is built on, and how to build one on it.
pub(crate) trait Footing: PyTypeInfo {
    /// `value` as a whole object, its bases built bare beneath it.
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T>;
}

impl Footing for PyAny {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        value.into()
    }
}

impl Footing for ProtoM21Object {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        PyClassInitializer::from(ProtoM21Object::default()).add_subclass(value)
    }
}

impl Footing for Music21Object {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        ProtoM21Object::under(Music21Object::default()).add_subclass(value)
    }
}

impl Footing for GeneralNote {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        Music21Object::under(GeneralNote).add_subclass(value)
    }
}

impl Footing for NotRest {
    fn under<T: PyClass<BaseType = Self>>(value: T) -> PyClassInitializer<T> {
        GeneralNote::under(NotRest).add_subclass(value)
    }
}

/// A class's value with whatever it extends built bare beneath it: what
/// `Py::new` and a `#[new]` take.
pub(crate) fn built<T>(value: T) -> PyClassInitializer<T>
where
    T: PyClass,
    T::BaseType: Footing,
{
    <T::BaseType as Footing>::under(value)
}

/// The classes music21 gives more than one base, with all of them, in
/// music21's order.
const MULTIPLE_BASES: &[(&str, &[&str])] = &[
    ("Spiccato", &["Staccato", "Accent"]),
    ("Stress", &["DynamicArticulation", "LengthArticulation"]),
    ("StringHarmonic", &["Bowing", "Harmonic"]),
    ("StringFingering", &["StringIndication", "Fingering"]),
    ("FrettedPluck", &["FretIndication", "Fingering"]),
    ("RepeatExpression", &["RepeatMark", "Expression"]),
    ("Repeat", &["RepeatMark", "Barline"]),
    ("ChordWithFretBoard", &["ChordSymbol", "FretBoard"]),
];

/// Gives the classes music21 builds on more than one base the rest of
/// theirs: `articulations.Spiccato` is a `Staccato` and an `Accent`. A pyo3
/// class extends one class; Python takes the others once the classes are
/// made, where they add nothing to an instance's layout -- a class with no
/// value of its own, or a mixin with no slots -- which is all these are.
/// Where both bases keep a value Python refuses, and the wheel tests'
/// `_HIERARCHY_GAPS` lists the class instead.
pub(crate) fn add_bases(m: &Bound<'_, PyModule>) -> PyResult<()> {
    for (name, bases) in MULTIPLE_BASES {
        let class = m.getattr(*name)?;
        let mut all = Vec::new();
        for base in *bases {
            all.push(m.getattr(*base)?);
        }
        class.setattr("__bases__", PyTuple::new(m.py(), all)?)?;
    }
    Ok(())
}

/// music21's `Groups`, as Python: a list of distinct strings that compares
/// as a set, ignoring case.
const GROUPS: &str = r#"
class Groups(list):
    """A list of strings naming the groups an element belongs to.

    The same name is not held twice, and two are equal when they hold the
    same names, in any order and of any case.
    """

    __slots__ = ()

    def _validName(self, value):
        if not isinstance(value, str):
            raise group_exception(
                f'Only strings can be used as group names, not {value!r}'
            )

    def append(self, value):
        self._validName(value)
        if not list.__contains__(self, value):
            list.append(self, value)

    def __setitem__(self, i, y):
        self._validName(y)
        super().__setitem__(i, y)

    def __eq__(self, other):
        if not isinstance(other, Groups):
            return False
        if len(self) != len(other):
            return False
        for mine, theirs in zip(sorted(self), sorted(other)):
            if mine.lower() != theirs.lower():
                return False
        return True

    def __reduce__(self):
        # Rebuilt from its names by the package's `_groups`, which every
        # build can import; the class's own name is music21's.
        return (_groups, (list(self),))


Groups.__module__ = 'music21.base'
"#;

/// A new `_SplitTuple` of what is given, as a tuple.
#[pyfunction]
fn split_tuple_new<'py>(
    cls: &Bound<'py, PyAny>,
    tupEls: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let items = PyTuple::new(py, tupEls.try_iter()?.collect::<PyResult<Vec<_>>>()?)?;
    py.get_type::<PyTuple>()
        .call_method1("__new__", (cls, items))
}

/// Gives a new `_SplitTuple` its own empty `spannerList`.
#[pyfunction]
fn split_tuple_init(slf: &Bound<'_, PyAny>, tupEls: &Bound<'_, PyAny>) -> PyResult<()> {
    let _ = tupEls;
    slf.setattr("spannerList", pyo3::types::PyList::empty(slf.py()))
}

/// Two are equal where both are `_SplitTuple`s holding equal spanners and
/// equal items.
#[pyfunction]
fn split_tuple_eq(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
    if !other.is_instance(&slf.get_type())? {
        return Ok(false);
    }
    if !slf
        .getattr("spannerList")?
        .eq(other.getattr("spannerList")?)?
    {
        return Ok(false);
    }
    let tuple = slf.py().get_type::<PyTuple>();
    tuple.getattr("__eq__")?.call1((slf, other))?.is_truthy()
}

/// The hash of the items and of which spanner list it holds, so that two
/// holding equal items and different lists hash apart.
#[pyfunction]
fn split_tuple_hash(slf: &Bound<'_, PyAny>) -> PyResult<isize> {
    let py = slf.py();
    let items = py
        .get_type::<PyTuple>()
        .getattr("__hash__")?
        .call1((slf,))?;
    let list = slf.getattr("spannerList")?.as_ptr() as usize;
    PyTuple::new(py, [items, list.into_pyobject(py)?.into_any()])?.hash()
}

static SPLIT_TUPLE: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// music21's `_SplitTuple`: the tuple of pieces `splitAtQuarterLength`
/// hands back, a tuple with a `spannerList` of the spanners the split made.
pub(crate) fn split_tuple<'py>(m: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyAny>> {
    use crate::plumbing::built::{method, new_class, static_method};

    let py = m.py();
    let class = SPLIT_TUPLE.get_or_try_init(py, || {
        let as_method = |function: Bound<'py, PyAny>, name: &str| {
            method(function, "music21.base", "_SplitTuple", name)
        };
        let members = vec![
            (
                "__new__",
                static_method(
                    wrap_pyfunction!(split_tuple_new, m)?.into_any(),
                    "music21.base",
                    "_SplitTuple",
                    "__new__",
                )?,
            ),
            (
                "__init__",
                as_method(
                    wrap_pyfunction!(split_tuple_init, m)?.into_any(),
                    "__init__",
                )?,
            ),
            (
                "__eq__",
                as_method(wrap_pyfunction!(split_tuple_eq, m)?.into_any(), "__eq__")?,
            ),
            (
                "__hash__",
                as_method(
                    wrap_pyfunction!(split_tuple_hash, m)?.into_any(),
                    "__hash__",
                )?,
            ),
        ];
        let made = new_class(
            py,
            "music21.base",
            "_SplitTuple",
            "The pieces a split hands back, as a tuple, with the spanners the split \
             made in its `spannerList`.",
            vec![py.get_type::<PyTuple>().into_any()],
            members,
        )?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    Ok(class.bind(py).clone())
}

/// The `GroupException` music21 raises for a name that is not a string.
#[pyfunction]
fn group_exception(py: Python<'_>, message: String) -> PyResult<PyErr> {
    let class = crate::exception_type(py, "GroupException")?
        .expect("GroupException is in the exception table");
    Ok(PyErr::from_value(class.call1((message,))?))
}

static GROUPS_CLASS: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// The package's `Groups` class, built once per interpreter.
pub(crate) fn own_groups(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    GROUPS_CLASS
        .get_or_try_init(py, || {
            let module = PyModule::from_code(
                py,
                &std::ffi::CString::new(GROUPS)?,
                c"music21_rs_groups.py",
                c"music21_rs_groups",
            )?;
            module.add_function(wrap_pyfunction!(group_exception, &module)?)?;
            module.add("_groups", groups_rebuilder(py)?)?;
            Ok::<_, PyErr>(module.getattr("Groups")?.unbind())
        })
        .map(|class| class.bind(py).clone())
}

/// A `Groups` holding these names: what a pickled one is read back with.
#[pyfunction]
#[pyo3(name = "_groups")]
fn rebuilt_groups<'py>(py: Python<'py>, names: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    let made = own_groups(py)?.call0()?;
    for name in names {
        made.call_method1("append", (name,))?;
    }
    Ok(made)
}

static GROUPS_REBUILDER: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

/// `_groups`, made once, and named by the package it is reached from.
pub(crate) fn groups_rebuilder(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    GROUPS_REBUILDER
        .get_or_try_init(py, || {
            let made = wrap_pyfunction!(rebuilt_groups, py)?.into_any();
            made.setattr("__module__", crate::package_name(py))?;
            Ok::<_, PyErr>(made.unbind())
        })
        .map(|made| made.bind(py).clone())
}

/// The `Groups` a new element is given: music21's own where music21 has
/// been imported, which is the class music21's code compares against, and
/// the package's otherwise. Nothing imports music21 to ask.
fn groups_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let loaded = py
        .import("sys")?
        .getattr("modules")?
        .get_item("music21.base");
    if let Ok(base) = loaded
        && !crate::is_provided(&base)
        && let Ok(class) = base.getattr("Groups")
    {
        return Ok(class);
    }
    own_groups(py)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Music21Object>()?;
    m.add_class::<GeneralNote>()?;
    m.add_class::<NotRest>()?;
    m.add_class::<ElementWrapper>()?;
    m.add("Groups", own_groups(m.py())?)?;
    m.add("_groups", groups_rebuilder(m.py())?)?;
    m.add("_SplitTuple", split_tuple(m)?)?;
    Ok(())
}

/// Hands a class built on one of these to Python by value, as pyo3 does
/// for a class built on nothing: a method can answer with the value itself.
macro_rules! into_python {
    ($($class:path),* $(,)?) => {
        $(
            impl<'py> pyo3::IntoPyObject<'py> for $class {
                type Target = $class;
                type Output = pyo3::Bound<'py, $class>;
                type Error = pyo3::PyErr;

                fn into_pyobject(self, py: pyo3::Python<'py>) -> pyo3::PyResult<Self::Output> {
                    pyo3::Bound::new(py, built(self))
                }
            }
        )*
    };
}

into_python!(
    crate::articulations::Articulation,
    crate::bar::Barline,
    crate::chord::Chord,
    crate::clef::Clef,
    crate::dynamics::Dynamic,
    crate::expressions::Ornament,
    crate::expressions::TextExpression,
    crate::spineparser::GlobalComment,
    crate::spineparser::MiscTandem,
    crate::instrument::Instrument,
    crate::interval::ChromaticInterval,
    crate::interval::DiatonicInterval,
    crate::interval::GenericInterval,
    crate::interval::Interval,
    crate::key::KeySignature,
    crate::layout::LayoutBase,
    crate::layout::PageLayout,
    crate::layout::ScoreLayout,
    crate::layout::StaffLayout,
    crate::layout::SystemLayout,
    crate::metadata::Metadata,
    crate::meter::TimeSignature,
    crate::note::Note,
    crate::rest::Rest,
    crate::percussion::PercussionChord,
    crate::repeat::RepeatExpression,
    crate::scale::Scale,
    crate::searchserial::ContiguousSegmentOfNotes,
    crate::serial::ToneRow,
    crate::serial::TwelveToneMatrix,
    crate::spanner::Spanner,
    crate::stream::Stream,
    crate::tempo::MetricModulation,
    crate::tempo::MetronomeMark,
    crate::tempo::TempoText,
    crate::text::TextBox,
    crate::stream::Verticality,
    crate::voiceleading::VoiceLeadingQuartet,
);
