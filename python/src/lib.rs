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
pub mod interval;
pub mod key;
pub mod notation;
pub mod note;
pub mod pitch;
pub mod scale;
pub mod serial;

pub use pitch::{Accidental, Microtone, Pitch};

/// The names each music21 module has a counterpart for here, which is what
/// [`install_into_music21`] replaces and what `python-parity`'s doctest
/// harness swaps one module at a time.
const MUSIC21_MODULES: [(&str, &[&str]); 10] = [
    ("music21.pitch", pitch::NAMES),
    ("music21.interval", interval::NAMES),
    ("music21.note", note::NAMES),
    ("music21.duration", note::DURATION_NAMES),
    ("music21.chord", chord::NAMES),
    ("music21.key", key::NAMES),
    ("music21.serial", serial::NAMES),
    ("music21.tie", notation::TIE_NAMES),
    ("music21.volume", notation::VOLUME_NAMES),
    ("music21.style", notation::STYLE_NAMES),
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

/// The Python half of installing a facade class into music21.
///
/// music21 keeps a score as `Stream`s of `Music21Object`s, and refuses to
/// hold anything else: an object with no offset, no sites and no place in a
/// class hierarchy is not something it can put at a point in time. That
/// machinery — sites, contexts, derivations, offsets relative to a container
/// — is exactly what this crate does not model and does not want to, so a
/// facade class is installed as a Python subclass of the facade *and* of
/// music21's own `Music21Object`, which is where all of it comes from.
///
/// Only `Music21Object` is inherited, and never the music21 class the facade
/// replaces: a member the facade has not ported still raises `AttributeError`
/// rather than quietly falling through to music21's own implementation, so
/// the doctest scores keep meaning what they say. What the subclass does
/// carry across is *classification* — `classes` and `classSet`, so that
/// `getElementsByClass` and `.notes` find our objects where they would find
/// music21's — which is a statement about what the object is, not a borrowed
/// implementation of it.
const INSTALL_HELPER: &str = r#"
from music21 import base as _base


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
    namespace = {
        '__init__': __init__,
        '__deepcopy__': __deepcopy__,
        '__copy__': __copy__,
        '__module__': original.__module__,
        '__qualname__': original.__qualname__,
        'classes': tuple(a.__name__ for a in original.__mro__ if a is not object),
    }
    for name in ('isNote', 'isRest', 'isChord', 'classSortOrder', 'equalityAttributes'):
        if hasattr(original, name):
            namespace[name] = getattr(original, name)
    installed = type(original.__name__, (facade, _base.Music21Object), namespace)
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
                module.setattr(*name, value)?;
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
    serial::register(m)?;
    key::register(m)?;
    interval::register(m)?;
    scale::register(m)?;
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
