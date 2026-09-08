//! `music21_rs`: music21-shaped Python classes over the `music21-rs` crate.
//!
//! Each class here carries music21's name, module string, constructor
//! keywords, property names and `repr`, and holds a `music21-rs` value
//! underneath. That makes the module two things at once: a Python package for
//! callers who want the crate's analysis without a Rust toolchain, and the
//! subject of `python-parity`, which imports the real music21 and runs its
//! own doctests against these classes.
//!
//! The classes are deliberately thin. Anything music21 does that the crate
//! does not is left to fail rather than reimplemented here in Python-shaped
//! Rust — the point is to expose the crate, not to build a second music21.

use pyo3::prelude::*;

pub mod chord;
pub mod figuredbass;
pub mod interval;
pub mod key;
pub mod notation;
pub mod note;
pub mod pitch;
pub mod roman;
pub mod scale;
pub mod serial;
pub mod tempo;
pub mod voiceleading;

pub use pitch::{Accidental, Microtone, Pitch};

/// The names each music21 module has a counterpart for here, which is what
/// [`install_into_music21`] replaces and what `python-parity`'s doctest
/// harness swaps one module at a time.
const MUSIC21_MODULES: [(&str, &[&str]); 15] = [
    ("music21.pitch", pitch::NAMES),
    ("music21.interval", interval::NAMES),
    ("music21.note", note::NAMES),
    ("music21.duration", note::DURATION_NAMES),
    ("music21.chord", chord::NAMES),
    ("music21.key", key::NAMES),
    ("music21.serial", serial::NAMES),
    ("music21.tie", notation::TIE_NAMES),
    ("music21.volume", notation::VOLUME_NAMES),
    ("music21.beam", notation::BEAM_NAMES),
    ("music21.scale", scale::NAMES),
    ("music21.roman", roman::NAMES),
    ("music21.figuredBass.notation", figuredbass::NAMES),
    ("music21.tempo", tempo::NAMES),
    ("music21.voiceLeading", voiceleading::NAMES),
];

/// A copy of a facade value as an instance of the class it was asked on,
/// so that a Python subclass of a facade class copies as itself.
///
/// A pyo3 method returning `Self` builds the base class, which for an
/// installed facade would drop the music21 half of the object and leave a
/// copy no stream would take. Allocating through `cls.__new__(cls)` keeps
/// the class, and the state is written into it afterwards.
pub(crate) fn copy_as_same_type<'py, T>(
    slf: &Bound<'py, T>,
    value: T,
) -> PyResult<Bound<'py, PyAny>>
where
    T: pyo3::PyClass<Frozen = pyo3::pyclass::boolean_struct::False>,
{
    let class = slf.as_any().get_type();
    let copy = class.call_method1("__new__", (&class,))?;
    *copy.cast::<T>()?.borrow_mut() = value;
    Ok(copy)
}

/// The class music21 now has under a name, where one of ours was installed
/// there.
///
/// A facade that builds a new object of its own kind has to build one of
/// these: music21 will only hold the installed class, and a pickle looking
/// the class up by name finds it and not the bare facade.
pub(crate) fn installed_class<'py>(
    py: Python<'py>,
    module: &str,
    name: &str,
) -> Option<Bound<'py, PyAny>> {
    let helper = install_helper(py).ok()?;
    let installed = helper.getattr("installed").ok()?;
    installed.get_item((module, name)).ok()
}

/// A facade object's musical half, written out as text a pickle can carry.
///
/// music21 freezes a score by pickling it, and an object of one of these
/// classes carries its state in Rust where a pickle cannot see it. Writing
/// it out is how it survives the round trip; the Python half of an installed
/// object, which is the offset and the sites, pickles itself as it always
/// did.
pub(crate) fn written_state<T>(value: &T) -> PyResult<String>
where
    T: serde::Serialize,
{
    serde_json::to_string(value).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("cannot write this out: {error}"))
    })
}

/// The same read back.
pub(crate) fn read_state<T>(text: &str) -> PyResult<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(text).map_err(|error| {
        pyo3::exceptions::PyValueError::new_err(format!("cannot read this back: {error}"))
    })
}

/// How a facade object is pickled: rebuilt by calling its own class with no
/// arguments, then told what it was.
///
/// The musical half lives in Rust where a pickle cannot see it, so it is
/// written out as text; the Python half of an installed object — the offset,
/// the sites, everything music21 keeps — goes along as its own dictionary.
pub(crate) fn pickled<T, V>(slf: &Bound<'_, T>, value: &V) -> PyResult<(Py<PyAny>, (), Py<PyAny>)>
where
    T: pyo3::PyClass,
    V: serde::Serialize,
{
    let py = slf.py();
    let class = slf.as_any().get_type().into_any().unbind();
    let written = written_state(value)?;
    let carried = match slf.as_any().getattr("__dict__") {
        Ok(carried) => carried.unbind(),
        Err(_) => py.None(),
    };
    let state = (written, carried).into_pyobject(py)?.into_any().unbind();
    Ok((class, (), state))
}

/// The other half of that: what the object was, read back, with the Python
/// half put where it was.
pub(crate) fn unpickled<T, V>(slf: &Bound<'_, T>, state: &Bound<'_, PyAny>) -> PyResult<V>
where
    T: pyo3::PyClass,
    V: serde::de::DeserializeOwned,
{
    let (written, carried): (String, Py<PyAny>) = state.extract()?;
    let py = slf.py();
    if !carried.is_none(py)
        && let Ok(own) = slf.as_any().getattr("__dict__")
    {
        own.call_method1("update", (carried,))?;
    }
    read_state(&written)
}

/// A new facade object, as the class music21 now has under that name.
///
/// Every object a facade builds has to be one of the installed classes where
/// there is one: music21 will hold nothing else, `isinstance` reads it as
/// what it replaced, and a pickle looking the class up by module and name
/// finds the installed one and refuses anything else. Where nothing has been
/// installed — the wheel on its own — the bare facade is what there is.
pub(crate) fn installed_new<T>(
    py: Python<'_>,
    module: &str,
    name: &str,
    value: T,
) -> PyResult<Py<T>>
where
    T: pyo3::PyClass<Frozen = pyo3::pyclass::boolean_struct::False>,
    T: Into<pyo3::PyClassInitializer<T>>,
{
    let Some(class) = installed_class(py, module, name) else {
        return Py::new(py, value);
    };
    let object = blank_installed(&class)?;
    let cell = object.cast::<T>()?;
    *cell.borrow_mut() = value;
    Ok(cell.clone().unbind())
}

/// A blank instance of an installed class, with its music21 half started.
pub(crate) fn blank_installed<'py>(class: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = class.py();
    install_helper(py)?.getattr("blank")?.call1((class,))
}

/// The Python half of installing a facade class into music21.
///
/// music21 keeps a score as `Stream`s of `Music21Object`s, and refuses to
/// hold anything else: an object with no offset, no sites and no place in a
/// class hierarchy is not something it can put at a point in time. That
/// machinery — sites, contexts, derivations, offsets relative to a container
/// — is exactly what this crate does not model and does not want to, so a
/// facade class is installed as a Python subclass of the facade *and* of the
/// music21 class it replaces.
///
/// Inheriting the class being replaced is what makes music21's own
/// `isinstance` checks work — `Stream.pitches` asks whether each element is a
/// `GeneralNote` — but it would also let a member the facade has not ported
/// fall through to music21's implementation of it, and the doctest scores
/// would stop meaning what they say. So it does not: every public member
/// music21 defines below `Music21Object` that the facade does not define is
/// *blocked* on the installed class, and raises `AttributeError` as it did
/// when the facade replaced the class outright.
///
/// `Music21Object` itself is never blocked. Sites, contexts, derivations and
/// offsets are the container machinery, and using music21's is the whole
/// point of inheriting from it.
const INSTALL_HELPER: &str = r#"
from music21 import base as _base


# The classes installed over music21's, by module and name. A facade that
# builds a new object of its own kind looks here first: music21 will only
# hold the installed class, so an object built as the bare facade is one no
# stream can take and no pickle can find.
installed = {}


def blank(cls):
    """An instance of an installed class with nothing said about it yet.

    `__new__` alone leaves the music21 half unstarted, which is where the
    offset and the sites live, so that half is started here. The caller
    writes the musical half in afterwards.
    """
    made = cls.__new__(cls)
    # Only the classes music21 keeps in a stream have a music21 half to
    # start; a pitch or a duration is not one of those.
    if issubclass(cls, _base.Music21Object):
        _base.Music21Object.__init__(made)
    return made


class NotPorted:
    """A member music21 has and the facade does not.

    It reads as absent, so nothing can quietly reach music21's own
    implementation of something this crate has not ported.
    """

    def __init__(self, owner, name):
        self.owner = owner
        self.name = name

    def __get__(self, instance, owner=None):
        raise AttributeError(
            f'{self.owner!r} object has no attribute {self.name!r}'
        )

    def __set__(self, instance, value):
        raise AttributeError(
            f'{self.owner!r} object has no attribute {self.name!r}'
        )


def unported_members(facade, original):
    """Every public member music21 defines below `Music21Object` that the
    facade does not."""
    blocked = {}
    for ancestor in original.__mro__:
        if ancestor is _base.Music21Object:
            break
        for name in vars(ancestor):
            if name.startswith('_') or name in blocked:
                continue
            if hasattr(facade, name):
                continue
            blocked[name] = NotPorted(original.__name__, name)
    return blocked


def make_class(facade, original):
    def __init__(self, *arguments, **keywords):
        _base.Music21Object.__init__(self)
        if facade.__init__ is not object.__init__:
            facade.__init__(self, *arguments, **keywords)

    def __deepcopy__(self, memo=None):
        # The facade copies as whatever class it was asked on, so this is
        # already one of these; it just has no music21 half yet, and a copy
        # belongs to no stream until something puts it in one.
        copied = facade.__deepcopy__(self, memo)
        _base.Music21Object.__init__(copied)
        return copied

    def __copy__(self):
        return __deepcopy__(self)

    classified = set()
    for ancestor in original.__mro__:
        classified.add(ancestor)
        classified.add(ancestor.__name__)
        classified.add(ancestor.__module__ + '.' + ancestor.__name__)
    namespace = dict(unported_members(facade, original))
    namespace.update({
        '__init__': __init__,
        '__deepcopy__': __deepcopy__,
        '__copy__': __copy__,
        '__module__': original.__module__,
        '__qualname__': original.__qualname__,
        'classes': tuple(a.__name__ for a in original.__mro__ if a is not object),
    })
    for name in ('isNote', 'isRest', 'isChord', 'classSortOrder', 'equalityAttributes'):
        if hasattr(original, name):
            namespace[name] = getattr(original, name)
    class Stands(type):
        """Metaclass under which the class replaced still counts as this one.

        music21's own subclasses — `harmony.ChordSymbol` and the rest — were
        built on the class this replaces and go on inheriting from it, so a
        plain `isinstance(chordSymbol, chord.Chord)` inside music21 would say
        no once `chord.Chord` is this class instead. It says yes: anything
        the old class would have accepted, the new one accepts.
        """

        def __instancecheck__(cls, instance):
            return (type.__instancecheck__(cls, instance)
                    or isinstance(instance, original))

        def __subclasscheck__(cls, subclass):
            return (type.__subclasscheck__(cls, subclass)
                    or issubclass(subclass, original))

    try:
        installed = Stands(original.__name__, (facade, original), namespace)
    except TypeError:
        # Some pairs cannot share a layout; those keep the old arrangement,
        # where only the container machinery is inherited.
        installed = Stands(original.__name__, (facade, _base.Music21Object), namespace)
    # A caller who asks a stream for `note.Note` is asking for this class now.
    installed.classSet = frozenset(classified | {installed})
    return installed


def wants_music21_object(facade, original):
    if not isinstance(facade, type) or not isinstance(original, type):
        return False
    if not issubclass(original, _base.Music21Object):
        return False
    # A Stream is a container, not an element, and none of these are one.
    return not getattr(original, 'isStream', False)
"#;

/// Builds the class to install for `name`: the facade itself, or a subclass
/// of it that music21 will accept as an element of a stream.
pub fn class_to_install<'py>(
    py: Python<'py>,
    module: &Bound<'py, PyModule>,
    name: &str,
    facade: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let Ok(original) = module.getattr(name) else {
        return Ok(facade.clone());
    };
    let helper = install_helper(py)?;
    if !helper
        .getattr("wants_music21_object")?
        .call1((facade, &original))?
        .extract::<bool>()?
    {
        return Ok(facade.clone());
    }
    helper.getattr("make_class")?.call1((facade, &original))
}

/// The helper module, compiled once per interpreter.
fn install_helper<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyModule>> {
    if let Ok(existing) = py.import("music21_rs_install") {
        return Ok(existing);
    }
    let helper = PyModule::from_code(
        py,
        &std::ffi::CString::new(INSTALL_HELPER)?,
        c"music21_rs_install.py",
        c"music21_rs_install",
    )?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("music21_rs_install", &helper)?;
    Ok(helper)
}

/// Replaces music21's own classes with these, in the music21 that is
/// installed, and answers how many names were replaced.
///
/// This is a testing aid: it is how you point an existing music21 program at
/// the Rust implementation without changing a line of it. Call it before the
/// program imports the classes — in a `conftest.py`, or at the top of
/// `__main__` — since `from music21.chord import Chord` binds the class it
/// finds at import time.
///
/// It patches a live module, so it changes music21 for everything in the
/// process. Nothing in this module calls it for you.
#[pyfunction]
fn install_into_music21(py: Python<'_>) -> PyResult<usize> {
    let ours = PyModule::new(py, "music21_rs")?;
    register_all(&ours)?;
    let mut replaced = 0;
    for (module_name, names) in MUSIC21_MODULES {
        let module = py.import(module_name)?;
        for name in names {
            if let Ok(value) = ours.getattr(*name) {
                let value = class_to_install(py, &module, name, &value)?;
                module.setattr(*name, &value)?;
                install_helper(py)?
                    .getattr("installed")?
                    .set_item((module_name, *name), &value)?;
                replaced += 1;
            }
        }
    }
    Ok(replaced)
}

/// Adds every class and function to a module. `python-parity` builds its own
/// module around this one, so the registration is separate from the
/// `#[pymodule]` below.
pub fn register_all(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pitch::register(m)?;
    figuredbass::register(m)?;
    tempo::register(m)?;
    voiceleading::register(m)?;
    serial::register(m)?;
    key::register(m)?;
    interval::register(m)?;
    scale::register(m)?;
    roman::register(m)?;
    notation::register(m)?;
    note::register(m)?;
    chord::register(m)?;
    Ok(())
}

/// The Python module: `import music21_rs`. One flat namespace holding every
/// class, since music21's own module layout is what the classes carry in
/// their `__module__` strings rather than where they are imported from.
#[pymodule]
pub fn music21_rs(m: &Bound<'_, PyModule>) -> PyResult<()> {
    register_all(m)?;
    m.add_function(wrap_pyfunction!(install_into_music21, m)?)?;
    // maturin's generated package does `from .music21_rs import *`, which
    // without this would pull the extension module in under its own name.
    let mut names: Vec<String> = m
        .dict()
        .keys()
        .iter()
        .filter_map(|key| key.extract::<String>().ok())
        .filter(|name| !name.starts_with('_'))
        .collect();
    names.sort();
    m.add("__all__", names)?;
    Ok(())
}
