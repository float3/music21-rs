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

/// music21's own coercion of a number written some other way.
///
/// Its setters are `int(value)` and `float(value)`, so a score reader that
/// hands an octave over as the string it read from the file gets a number.
/// Refusing one where music21 accepts it is a difference nobody asked for.
pub(crate) fn as_int(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    if let Ok(number) = value.extract::<i32>() {
        return Ok(number);
    }
    value
        .py()
        .import("builtins")?
        .getattr("int")?
        .call1((value,))?
        .extract()
}

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
    // Through the helper, so that the copy gets the music21 half started as
    // a fresh object of that class would: `__new__` alone leaves an object
    // with no sites for a stream to hold it by.
    let copy = blank_installed(class.as_any())?;
    *copy.cast::<T>()?.borrow_mut() = value;
    Ok(copy)
}

/// Records where a new object came from, as music21's own methods do.
///
/// `n.transpose('P5')` hands back a copy, and music21 writes on the copy
/// what it was made from and how; its own streams read that back — a
/// deepcopy of a stream moves its spanners onto the copies by it. A thing no
/// stream holds keeps no derivation, and nothing is written on one.
pub(crate) fn derived_from(
    made: &Bound<'_, PyAny>,
    origin: &Bound<'_, PyAny>,
    method: &str,
) -> PyResult<()> {
    let Ok(derivation) = made.getattr("derivation") else {
        return Ok(());
    };
    if derivation.is_none() {
        return Ok(());
    }
    derivation.setattr("origin", origin)?;
    derivation.setattr("method", method)?;
    Ok(())
}

/// Whether these classes have been installed into music21 at all.
fn anything_installed(py: Python<'_>) -> bool {
    install_helper(py)
        .and_then(|helper| helper.getattr("installed"))
        .map(|installed| installed.len().unwrap_or(0) > 0)
        .unwrap_or(false)
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

/// music21's own `Music21Exception`, as a type these classes can be built on.
///
/// Every exception music21 raises is one of these, and its own code catches
/// that base all over: its MusicXML reader tries the mode a score names for
/// its key signature and lets a bad one go by catching it. An exception of
/// ours that was not one of these would not be caught, and the reader would
/// fail on a score it can read. Where music21 is not there to be imported —
/// the wheel used on its own — a plain `Exception` stands in.
pub struct Music21Exception;

#[allow(deprecated)]
unsafe impl pyo3::type_object::PyTypeInfo for Music21Exception {
    const NAME: &'static str = "Music21Exception";
    const MODULE: Option<&'static str> = Some("music21.exceptions21");

    fn type_object_raw(py: Python<'_>) -> *mut pyo3::ffi::PyTypeObject {
        static TYPE_OBJECT: pyo3::sync::PyOnceLock<Py<pyo3::types::PyType>> =
            pyo3::sync::PyOnceLock::new();
        TYPE_OBJECT
            .get_or_init(py, || {
                py.import("music21.exceptions21")
                    .and_then(|module| module.getattr("Music21Exception"))
                    .and_then(|class| Ok(class.cast_into::<pyo3::types::PyType>()?))
                    .map(pyo3::Bound::unbind)
                    .unwrap_or_else(|_| py.get_type::<pyo3::exceptions::PyException>().unbind())
            })
            .as_ptr()
            .cast()
    }
}

/// What `__reduce__` hands back: the function that makes a blank object of
/// the right class, the module and name to make it under, and the state to
/// write into it.
pub(crate) type Pickled = (Py<PyAny>, (String, String), Py<PyAny>);

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
pub(crate) fn pickled<T, V>(slf: &Bound<'_, T>, value: &V) -> PyResult<Pickled>
where
    T: pyo3::PyClass,
    V: serde::Serialize,
{
    pickled_extra(slf, value, None)
}

/// The same, for a facade that holds Python objects of its own.
///
/// A note's ornaments and marks are plain Python objects the crate does not
/// model, kept in the facade rather than in the object's `__dict__`, so a
/// pickle that carried only the two halves would thaw a note with no
/// articulations on it — and music21 freezes every score it parses.
pub(crate) fn pickled_extra<T, V>(
    slf: &Bound<'_, T>,
    value: &V,
    extra: Option<&Bound<'_, pyo3::types::PyDict>>,
) -> PyResult<Pickled>
where
    T: pyo3::PyClass,
    V: serde::Serialize,
{
    let py = slf.py();
    // Not the class itself: a pickle carrying a class carries its module
    // and its name, and in another process — joblib starts one to measure a
    // score's features — that name is still music21's own class, which
    // cannot read what this wrote. So the pickle carries the name and a
    // function of ours that puts the class there before reading it.
    let class = slf.as_any().get_type();
    let module = class
        .getattr("__module__")
        .and_then(|module| module.extract::<String>())
        .unwrap_or_default();
    let name = class
        .getattr("__qualname__")
        .and_then(|name| name.extract::<String>())
        .unwrap_or_default();
    let thaw = py
        .import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?
        .getattr("_thawed")?
        .unbind();
    let written = written_state(value)?;
    // The Python half is music21's own: everything it keeps on the object,
    // with the two fields it never freezes emptied, as `Music21Object`
    // freezes it.
    let carried = match slf.as_any().getattr("__dict__") {
        Ok(carried) => {
            let copied = carried.call_method0("copy")?;
            copied.set_item("_derivation", py.None())?;
            copied.set_item("_activeSite", py.None())?;
            copied.unbind()
        }
        Err(_) => py.None(),
    };
    let extra = match extra {
        Some(extra) => extra.clone().into_any().unbind(),
        None => py.None(),
    };
    let state = (written, carried, extra)
        .into_pyobject(py)?
        .into_any()
        .unbind();
    Ok((thaw, (module, name), state))
}

/// The other end of a pickle: a blank object of the class music21 has under
/// that name, with these classes put in place first if they are not there.
///
/// A score frozen with the crate installed can only be read back with the
/// crate installed — the objects in it are these classes — so a process that
/// has not installed them installs them now. That is what lets a score cross
/// into a worker process, which is how music21's own feature extraction runs.
#[pyfunction]
#[pyo3(name = "_thawed")]
fn thawed(py: Python<'_>, module: &str, name: &str) -> PyResult<Py<PyAny>> {
    // Only where *nothing* is installed: a process that has never installed
    // these classes — joblib starts one to measure a score's features — has
    // to install them to read a score at all. Installing again where they
    // are already in place would build every class a second time, and every
    // object made before that would stop being of the class its module now
    // names, which is a far worse thing than a name this cannot resolve.
    if !anything_installed(py) {
        install_into_music21(py)?;
    }
    // A class of ours that was registered but never installed — music21
    // does something with it that this does not — is still the class the
    // module has under that name, and still what was written out.
    let Some(class) = installed_class(py, module, name).or_else(|| {
        py.import(module)
            .and_then(|module| module.getattr(name))
            .ok()
    }) else {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "{module}.{name} is not one of these classes"
        )));
    };
    Ok(blank_installed(&class)?.unbind())
}

/// The other half of that: what the object was, read back, with the Python
/// half put where it was.
pub(crate) fn unpickled<T, V>(slf: &Bound<'_, T>, state: &Bound<'_, PyAny>) -> PyResult<Option<V>>
where
    T: pyo3::PyClass,
    V: serde::de::DeserializeOwned,
{
    unpickled_extra(slf, state).map(|(value, _)| value)
}

/// The same, handing back whatever Python objects the facade froze beside
/// its two halves.
#[allow(clippy::type_complexity)]
pub(crate) fn unpickled_extra<T, V>(
    slf: &Bound<'_, T>,
    state: &Bound<'_, PyAny>,
) -> PyResult<(Option<V>, Option<Py<PyAny>>)>
where
    T: pyo3::PyClass,
    V: serde::de::DeserializeOwned,
{
    let py = slf.py();
    let read = state
        .extract::<(String, Py<PyAny>, Py<PyAny>)>()
        .or_else(|_| {
            state
                .extract::<(String, Py<PyAny>)>()
                .map(|(written, carried)| (written, carried, py.None()))
        });
    let Ok((written, carried, extra)) = read else {
        // Not one of ours. music21 freezes its own half as a plain dictionary
        // of attributes and hands it straight back, so that is what this is.
        if let Ok(own) = slf.as_any().getattr("__dict__") {
            own.call_method1("update", (state,))?;
        }
        return Ok((None, None));
    };
    if !carried.is_none(py)
        && let Ok(own) = slf.as_any().getattr("__dict__")
    {
        own.call_method1("update", (carried,))?;
    }
    let extra = (!extra.is_none(py)).then_some(extra);
    read_state(&written).map(|value| (Some(value), extra))
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
import copy as _copy
import sys

from music21 import base as _base
from music21 import derivation as _derivation
from music21 import style as _style

# What `Music21Object._deepcopySubclassable` leaves behind: where the object
# sat, what it was derived from, and what it had worked out about both.
_NOT_COPIED = frozenset(('_derivation', '_activeSite', '_sites', '_cache'))

# music21's own plumbing, which every one of its classes is built on and
# none of which says anything musical: what an object reports itself as,
# where it is drawn, what an editor wrote about it, and the slots machinery
# underneath both. Inheriting it is the point of installing a class at all —
# it is `Music21Object` for the things a stream does not hold.
_MACHINERY = frozenset((
    'ProtoM21Object',
    'StyleMixin',
    'SlottedObjectMixin',
    'EqualSlottedObjectMixin',
))


def machinery_of(original, facade):
    """The plumbing classes `original` is built on, in its own order.

    One at a time, because not all of them can be a base of a pyo3 class:
    `StyleMixin` has real `__slots__` and will not share an instance layout
    with one. Taking the rest anyway is the difference between a beam that
    knows its own slots and one that knows nothing.
    """
    taken = (facade,)
    for ancestor in original.__mro__:
        if ancestor.__name__ not in _MACHINERY:
            continue
        if not getattr(ancestor, '__module__', '').startswith('music21'):
            continue
        try:
            type('probe', taken + (ancestor,), {})
        except TypeError:
            continue
        taken += (ancestor,)
    return taken


def rebind(original, installed):
    """Replace a class everywhere music21 has already bound it.

    music21 is fully imported before any of this runs, and a module that
    wrote `from music21.duration import Duration` at the top holds the class
    it had then — `music21.base` is one of them, and every object it builds
    would go on being the old class. Those bindings are replaced too.
    """
    for module in list(sys.modules.values()):
        if module is None:
            continue
        if not getattr(module, '__name__', '').startswith('music21'):
            continue
        for name, value in list(vars(module).items()):
            if value is original:
                setattr(module, name, installed)


# The classes installed over music21's, by module and name. A facade that
# builds a new object of its own kind looks here first: music21 will only
# hold the installed class, so an object built as the bare facade is one no
# stream can take and no pickle can find.
installed = {}

# music21's own class for each one installed over it.
replaced = {}

# The classes of ours that an installed class is built on. They are the
# other half of a class music21 has, not a kind of thing in their own right,
# so `classes` and `classSet` do not name them.
_facades = set()

# What `classes` and `classSet` answer for a class, worked out once.
_lineages = {}


def refresh_class_sets():
    """Forget what was worked out, since there is a new class in the world."""
    _lineages.clear()


def lineage(cls):
    """What `cls` is, and everything it is a kind of, in music21's order.

    An installed class stands in for one of music21's own, so the chain it
    reports is that class's chain and not the arrangement of bases that put
    it there: an accidental of ours is still a `StyleMixin` as far as
    anything reading `classes` is concerned, and a `ChordSymbol` built on an
    installed `Chord` is still a `Chord`.
    """
    known = _lineages.get(cls)
    if known is not None:
        return known
    walked = []
    for ancestor in (cls,) + cls.__mro__:
        stands_for = ancestor.__dict__.get('_replaces')
        if stands_for is None and ancestor in _facades:
            # The facade under an installed class is that class's other
            # half, not a kind of thing music21 knows about.
            continue
        for member in (stands_for.__mro__ if stands_for is not None else (ancestor,)):
            if member not in walked:
                walked.append(member)
    names = []
    for member in walked:
        if member.__name__ not in names:
            names.append(member.__name__)
    members = set(walked)
    for member in walked:
        members.add(member.__name__)
        qualified = member.__module__ + '.' + member.__name__
        members.add(qualified)
        if qualified.startswith('music21.'):
            members.add(qualified[len('music21.'):])
        # `getElementsByClass` matches on the classes themselves, and the
        # name it is given now means the installed class. Both are listed:
        # music21's own default arguments hold the class it had when the
        # module was read, which is the one being replaced, while anything
        # asking by name today gets the one standing in for it.
        if member in replaced:
            members.add(replaced[member])
    known = (tuple(names), frozenset(members), tuple(walked))
    _lineages[cls] = known
    return known


def slots_of(cls):
    """The slots music21 keeps this kind of object's values in.

    They live in Rust here rather than in slots, but music21's own code asks
    what the slots are — its pickling of a slotted object does — and the
    answer is the one the class being replaced would give.
    """
    names = set()
    for ancestor in lineage(cls)[2]:
        names.update(getattr(ancestor, '__slots__', ()))
    return names


def style_and_editorial(original):
    """What `StyleMixin` gives a class that a stream does not hold.

    Its `__slots__` cannot share an instance layout with a pyo3 class, so
    what it defines is put on the installed class rather than inherited. The
    style half is the facade's own — the crate models the colour — and the
    editorial half is music21's, which the crate models nothing of.
    """
    if not issubclass(original, _style.StyleMixin):
        return {}
    members = {
        name: getattr(_style.StyleMixin, name)
        for name in ('editorial', 'hasEditorialInformation')
    }
    members['_styleClass'] = original._styleClass
    return members


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
    elif hasattr(cls, 'editorial'):
        made._editorial = None
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
        if ancestor is _base.Music21Object or ancestor is object:
            continue
        if ancestor.__name__ in _MACHINERY:
            continue
        for name in vars(ancestor):
            if name.startswith('_') or name in blocked:
                continue
            if hasattr(facade, name):
                continue
            blocked[name] = NotPorted(original.__name__, name)
    return blocked


def make_class(facade, original):
    def __init__(self, *arguments, **keywords):
        if issubclass(original, _base.Music21Object):
            _base.Music21Object.__init__(self)
        elif hasattr(type(self), 'editorial'):
            self._editorial = None
        if facade.__init__ is not object.__init__:
            facade.__init__(self, *arguments, **keywords)

    def __deepcopy__(self, memo=None):
        # The facade copies as whatever class it was asked on, so this is
        # already one of these; it just has no music21 half yet.
        copier = getattr(facade, '__deepcopy__', None)
        if copier is not None:
            copied = copier(self, memo)
        else:
            # A facade with no copy of its own is copied the way a pickle of
            # it is read back — but into a blank of *this* class rather than
            # through the name the pickle carries, since looking a class up
            # by name would install the classes afresh if a test had reloaded
            # the module underneath, and every object made before that would
            # stop being of the class its module now names.
            copied = blank(type(self))
            copied.__setstate__(self.__reduce__()[2])
        if isinstance(copied, _base.Music21Object):
            _base.Music21Object.__init__(copied)
        elif hasattr(type(copied), 'editorial'):
            copied._editorial = None
        # The music21 half comes across as music21 copies it — the offset,
        # the editorial, the labels — and what tied the object to where it
        # sat does not: a copy belongs to no stream until something puts it
        # in one.
        for name, value in vars(self).items():
            if name in _NOT_COPIED:
                continue
            try:
                setattr(copied, name, _copy.deepcopy(value, memo if memo is not None else {}))
            except Exception:  # pragma: no cover - music21 keeps odd things here
                pass
        # music21 records where a copy came from, and a stream's own deepcopy
        # reads that back to move its spanners onto the copies. Without it a
        # crescendo would still point at the notes of the original and would
        # be written out over nothing. Only the things a stream holds have
        # any of that; a pitch or an accidental is not one of them.
        if isinstance(copied, _base.Music21Object):
            derived = _derivation.Derivation(client=copied)
            derived.origin = self
            derived.method = '__deepcopy__'
            copied._derivation = derived
            copied.purgeOrphans()
        return copied

    def __copy__(self):
        return __deepcopy__(self)

    _facades.add(facade)
    namespace = dict(unported_members(facade, original))
    namespace.update(style_and_editorial(original))
    namespace.update({
        '__init__': __init__,
        '__deepcopy__': __deepcopy__,
        '__copy__': __copy__,
        '__module__': original.__module__,
        '__qualname__': original.__qualname__,
        # Worked out per class rather than fixed, so that a Python subclass
        # of one of these reports itself and not the class it was built on.
        'classes': property(lambda self: lineage(type(self))[0]),
        'classSet': property(lambda self: lineage(type(self))[1]),
        '_getSlotsRecursive': lambda self: slots_of(type(self)),
        '_replaces': original,
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
            # The facade counts too: a method of one facade that hands back
            # another builds the plain class, and that is the same object
            # this is, without the half music21 keeps.
            return (type.__instancecheck__(cls, instance)
                    or isinstance(instance, (original, facade)))

        def __subclasscheck__(cls, subclass):
            return (type.__subclasscheck__(cls, subclass)
                    or issubclass(subclass, (original, facade)))

    if issubclass(original, _base.Music21Object):
        bases = (facade, original)
        fallback = (facade, _base.Music21Object)
    else:
        # Not something a stream holds — an accidental, a duration, a beam.
        # music21's own class is still never a base, but the plumbing it is
        # built on is: that is where `editorial`, `style` and the slots
        # machinery live, and none of it says anything musical.
        bases = machinery_of(original, facade)
        fallback = (facade,)
    try:
        installed = Stands(original.__name__, bases, namespace)
    except TypeError:
        # Some pairs cannot share a layout; those keep the old arrangement,
        # where only the container machinery is inherited.
        try:
            installed = Stands(original.__name__, fallback, namespace)
        except TypeError:
            # A facade nothing can be built on at all stands as it is.
            return facade
    replaced[original] = installed
    return installed


def wants_installing(facade, original):
    """Whether this pair is one to build an installed class for.

    Everything music21 has a class for is, except a Stream: a stream is a
    container rather than an element, and none of these are one.
    """
    if not isinstance(facade, type) or not isinstance(original, type):
        return False
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
    let helper = install_helper(py)?;
    let built = match module.getattr(name) {
        Ok(original)
            if helper
                .getattr("wants_installing")?
                .call1((facade, &original))?
                .extract::<bool>()? =>
        {
            helper.getattr("make_class")?.call1((facade, &original))?
        }
        _ => facade.clone(),
    };
    // Whatever was built is what music21 now has under that name, and what
    // everything here must build to match it — so it is recorded before it
    // is handed back, for `installed_new` to find.
    helper
        .getattr("installed")?
        .set_item((module.name()?, name), &built)?;
    // Each class installed makes every other one a little more itself: what
    // `getElementsByClass` matches on has to name the installed classes, not
    // only the ones they replaced.
    helper.getattr("refresh_class_sets")?.call0()?;
    Ok(built)
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
                let original = module.getattr(*name).ok();
                let value = class_to_install(py, &module, name, &value)?;
                module.setattr(*name, &value)?;
                let helper = install_helper(py)?;
                if let Some(original) = original {
                    helper.getattr("rebind")?.call1((original, &value))?;
                }
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
    m.add_function(wrap_pyfunction!(thawed, m)?)?;
    // maturin's generated package does `from .music21_rs import *`, which
    // without this would pull the extension module in under its own name.
    let mut names: Vec<String> = m
        .dict()
        .keys()
        .iter()
        .filter_map(|key| key.extract::<String>().ok())
        // Everything but the dunders, since the package the wheel installs
        // takes its contents from this list: a pickle written by one of
        // these classes names `_thawed`, and music21's own `key` doctest
        // reads `_sharpsToPitchCache` back.
        .filter(|name| !name.starts_with("__"))
        .collect();
    names.sort();
    m.add("__all__", names)?;
    Ok(())
}
