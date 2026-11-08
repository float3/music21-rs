//! music21's `common.objects`: `RelativeCounter`, `defaultlist`,
//! `SingletonCounter`, the slotted-object mixins, `FrozenObject` and
//! `Timer`.
//!
//! The mixins and the two containers are real Python classes, built with
//! methods written in Rust: music21's classes mix them in beside other
//! bases and add slots of their own, which a class with a Rust layout
//! would not allow.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::sync::atomic::{AtomicU64, Ordering};

use pyo3::exceptions::{PyAttributeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{
    PyDict, PyFrozenSet, PyInt, PyList, PySet, PyString, PyTuple, PyWeakrefReference,
};

use super::built::{class_name, method, new_class, static_method, super_of};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.objects";

/// The classes the methods here call `super()` on, once built.
static RELATIVE_COUNTER: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static FROZEN_OBJECT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Each class's slots, all the way up, as `_getSlotsRecursive` found them.
static SLOTS_BY_CLASS: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

/// The next number any `SingletonCounter` hands out.
static SINGLETON_COUNTER: AtomicU64 = AtomicU64::new(0);

fn built<'py>(py: Python<'py>, cell: &PyOnceLock<Py<PyAny>>) -> PyResult<Bound<'py, PyAny>> {
    cell.get(py)
        .map(|class| class.bind(py).clone())
        .ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("music21.common.objects is not built yet")
        })
}

fn slots_by_class(py: Python<'_>) -> &Py<PyDict> {
    SLOTS_BY_CLASS.get_or_init(py, || PyDict::new(py).unbind())
}

// ---------------------------------------------------------------- RelativeCounter

/// The counter's keys, most common first, ties in the order they came.
#[pyfunction]
#[pyo3(name = "__iter__")]
fn counter_iter<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let keys = super_of(&built(py, &RELATIVE_COUNTER)?, slf)?.call_method0("__iter__")?;
    let keywords = PyDict::new(py);
    keywords.set_item("key", slf.getattr("__getitem__")?)?;
    keywords.set_item("reverse", true)?;
    let sorted = py
        .import("builtins")?
        .getattr("sorted")?
        .call((keys,), Some(&keywords))?;
    sorted.try_iter().map(Bound::into_any)
}

/// Each key with its count, most common first.
#[pyfunction]
#[pyo3(name = "items")]
fn counter_items<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let pairs = PyList::empty(py);
    for key in slf.try_iter()? {
        let key = key?;
        let count = slf.get_item(&key)?;
        pairs.append((key, count))?;
    }
    pairs.try_iter().map(Bound::into_any)
}

/// A counter of the same class with each count divided by the total and
/// multiplied by `scale`.
fn counter_scaled<'py>(slf: &Bound<'py, PyAny>, scale: Option<i64>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let mut total = PyInt::new(py, 0).into_any();
    for key in slf.try_iter()? {
        total = total.add(slf.get_item(key?)?)?;
    }
    let scaled = PyDict::new(py);
    for key in slf.try_iter()? {
        let key = key?;
        let mut count = slf.get_item(&key)?;
        if let Some(scale) = scale {
            count = count.mul(scale)?;
        }
        scaled.set_item(&key, count.div(&total)?)?;
    }
    slf.getattr("__class__")?.call1((scaled,))
}

/// A counter of the same class whose counts are proportions of the whole.
#[pyfunction]
fn asProportion<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    counter_scaled(slf, None)
}

/// A counter of the same class whose counts are percentages of the whole.
#[pyfunction]
fn asPercentage<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    counter_scaled(slf, Some(100))
}

// ---------------------------------------------------------------- defaultlist

/// Keeps the function that fills a `defaultlist`'s gaps.
#[pyfunction]
#[pyo3(name = "__init__")]
fn defaultlist_init(slf: &Bound<'_, PyAny>, fx: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    py.get_type::<PyList>().call_method1("__init__", (slf,))?;
    slf.setattr("_fx", fx)
}

/// Appends what `_fx` makes until `index` is in the list.
#[pyfunction]
#[pyo3(name = "_fill")]
fn defaultlist_fill(slf: &Bound<'_, PyAny>, index: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    while PyInt::new(py, slf.len()?).le(index)? {
        let made = slf.getattr("_fx")?.call0()?;
        slf.call_method1("append", (made,))?;
    }
    Ok(())
}

#[pyfunction]
#[pyo3(name = "__setitem__")]
fn defaultlist_setitem(
    slf: &Bound<'_, PyAny>,
    index: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    slf.call_method1("_fill", (index,))?;
    slf.py()
        .get_type::<PyList>()
        .call_method1("__setitem__", (slf, index, value))?;
    Ok(())
}

#[pyfunction]
#[pyo3(name = "__getitem__")]
fn defaultlist_getitem<'py>(
    slf: &Bound<'py, PyAny>,
    index: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    slf.call_method1("_fill", (index,))?;
    slf.py()
        .get_type::<PyList>()
        .call_method1("__getitem__", (slf, index))
}

// ---------------------------------------------------------------- SingletonCounter

/// Hands out numbers in ascending order, shared by every instance: no two
/// calls on any of them give the same one.
#[pyclass(subclass, module = "music21.common.objects")]
pub(crate) struct SingletonCounter;

#[pymethods]
impl SingletonCounter {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __call__(&self) -> u64 {
        SINGLETON_COUNTER.fetch_add(1, Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------- SlottedObjectMixin

/// Every slot of the object's class and the classes above it, but
/// `_frozenHash`, kept per class.
#[pyfunction]
#[pyo3(name = "_getSlotsRecursive")]
fn slots_recursive<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let cache = slots_by_class(py).bind(py);
    let class = slf.get_type();
    if let Some(found) = cache.get_item(&class)? {
        return Ok(found);
    }
    let slots = PySet::empty(py)?;
    for ancestor in slf.getattr("__class__")?.call_method0("mro")?.try_iter()? {
        let named = match ancestor?.getattr("__slots__") {
            Ok(named) => named,
            Err(error) if error.is_instance_of::<PyAttributeError>(py) => {
                PyTuple::empty(py).into_any()
            }
            Err(error) => return Err(error),
        };
        slots.call_method1("update", (named,))?;
    }
    slots.discard("_frozenHash")?;
    let frozen = PyFrozenSet::new(py, slots.iter())?.into_any();
    cache.set_item(&class, &frozen)?;
    Ok(frozen)
}

/// An attribute, or `None` where there is none to read.
fn attribute_or_none<'py>(
    object: &Bound<'py, PyAny>,
    name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    match object.getattr(name.cast::<PyString>()?) {
        Ok(found) => Ok(found),
        Err(error) if error.is_instance_of::<PyAttributeError>(object.py()) => {
            Ok(object.py().None().into_bound(object.py()))
        }
        Err(error) => Err(error),
    }
}

/// The object's `__dict__`, if it has one, and every slot, a weak
/// reference in one replaced by what it refers to, with a warning printed.
#[pyfunction]
#[pyo3(name = "__getstate__")]
fn slotted_getstate<'py>(slf: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = slf.py();
    let name = pyo3::intern!(py, "__dict__").clone().into_any();
    let own = attribute_or_none(slf, &name)?;
    let state = if own.is_none() {
        PyDict::new(py).into_any()
    } else {
        own.call_method0("copy")?
    };
    for slot in slf.call_method0("_getSlotsRecursive")?.try_iter()? {
        let slot = slot?;
        let mut value = attribute_or_none(slf, &slot)?;
        if value.is_instance_of::<PyWeakrefReference>() {
            value = value.call0()?;
            let warning = format!(
                "Warning: uncaught weakref found in {} - {}, will not be wrapped again",
                slf.repr()?,
                slot.str()?
            );
            py.import("builtins")?.getattr("print")?.call1((warning,))?;
        }
        state.set_item(slot, value)?;
    }
    Ok(state)
}

/// Sets every attribute the state names.
#[pyfunction]
#[pyo3(name = "__setstate__")]
fn slotted_setstate(slf: &Bound<'_, PyAny>, state: &Bound<'_, PyAny>) -> PyResult<()> {
    for pair in state.call_method0("items")?.try_iter()? {
        let (slot, value): (Bound<'_, PyAny>, Bound<'_, PyAny>) = pair?.extract()?;
        slf.setattr(slot.cast::<PyString>()?, value)?;
    }
    Ok(())
}

/// Equal to an object of the very same class whose slots, `id` aside,
/// all hold equal values; anything else is left to the other object.
#[pyfunction]
#[pyo3(name = "__eq__")]
fn slotted_eq(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = slf.py();
    if !slf.get_type().is(other.get_type()) {
        return Ok(py.NotImplemented());
    }
    for slot in slf.call_method0("_getSlotsRecursive")?.try_iter()? {
        let slot = slot?;
        if slot.eq("id")? {
            continue;
        }
        let slot = slot.cast::<PyString>()?;
        if slf.getattr(slot)?.ne(other.getattr(slot)?)? {
            return Ok(false.into_pyobject(py)?.to_owned().into_any().unbind());
        }
    }
    Ok(true.into_pyobject(py)?.to_owned().into_any().unbind())
}

#[pyfunction]
#[pyo3(name = "__ne__")]
fn slotted_ne(slf: &Bound<'_, PyAny>, other: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!slf.eq(other)?)
}

// ---------------------------------------------------------------- FrozenObject

#[pyfunction]
#[pyo3(name = "__new__", signature = (cls, *arguments, **keywords))]
fn frozen_new<'py>(
    cls: &Bound<'py, PyAny>,
    arguments: &Bound<'py, PyTuple>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let _ = (arguments, keywords);
    let py = cls.py();
    let made = super_of(&built(py, &FROZEN_OBJECT)?, cls)?.call_method1("__new__", (cls,))?;
    set_frozen_hash(&made, py.None().bind(py))?;
    Ok(made)
}

/// Sets `_frozenHash` past the object's own `__setattr__`.
fn set_frozen_hash(object: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
    object
        .py()
        .import("builtins")?
        .getattr("object")?
        .call_method1("__setattr__", (object, "_frozenHash", value))?;
    Ok(())
}

#[pyfunction]
#[pyo3(name = "__setstate__")]
fn frozen_setstate(slf: &Bound<'_, PyAny>, state: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = slf.py();
    set_frozen_hash(slf, py.None().bind(py))?;
    super_of(&built(py, &FROZEN_OBJECT)?, slf)?.call_method1("__setstate__", (state,))?;
    Ok(())
}

/// Lets an attribute be set only by an `__init__`, `__new__` or
/// `__setstate__` of the object's own class somewhere up the stack, or on
/// an object whose `frozen` is false; raises `TypeError` otherwise.
#[pyfunction]
#[pyo3(name = "_check_init", signature = (slf, key = None))]
fn frozen_check_init(slf: &Bound<'_, PyAny>, key: Option<&Bound<'_, PyAny>>) -> PyResult<bool> {
    let py = slf.py();
    if let Some(key) = key
        && key.eq("__class__")?
    {
        return Ok(true);
    }
    let frozen = match slf.getattr("frozen") {
        Ok(frozen) => frozen.is_truthy()?,
        Err(error) if error.is_instance_of::<PyAttributeError>(py) => true,
        Err(error) => return Err(error),
    };
    if !frozen {
        return Ok(true);
    }
    let class = slf.getattr("__class__")?;
    let mut frame = py.import("sys")?.call_method1("_getframe", (0,)).ok();
    while let Some(current) = frame.filter(|current| !current.is_none()) {
        let name = current.getattr("f_code")?.getattr("co_name")?;
        let builds = ["__init__", "__new__", "__setstate__"]
            .iter()
            .try_fold(false, |found, wanted| {
                Ok::<_, PyErr>(found || name.eq(*wanted)?)
            })?;
        if builds {
            let locals = current.getattr("f_locals")?;
            if locals.contains("self")?
                && locals.get_item("self")?.getattr("__class__")?.eq(&class)?
            {
                return Ok(true);
            }
        }
        frame = Some(current.getattr("f_back")?);
    }
    Err(PyTypeError::new_err(format!(
        "This {} instance is immutable.",
        class_name(slf)?
    )))
}

#[pyfunction]
#[pyo3(name = "__setattr__")]
fn frozen_setattr(
    slf: &Bound<'_, PyAny>,
    key: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    slf.call_method1("_check_init", (key,))?;
    super_of(&built(slf.py(), &FROZEN_OBJECT)?, slf)?.call_method1("__setattr__", (key, value))?;
    Ok(())
}

#[pyfunction]
#[pyo3(name = "__delattr__")]
fn frozen_delattr(slf: &Bound<'_, PyAny>, key: &Bound<'_, PyAny>) -> PyResult<()> {
    slf.call_method1("_check_init", (key,))?;
    super_of(&built(slf.py(), &FROZEN_OBJECT)?, slf)?.call_method1("__delattr__", (key,))?;
    Ok(())
}

/// What `FrozenObject` says of item assignment, which it never allows,
/// whatever the class above it would do.
fn not_subscriptable(slf: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    Ok(PyTypeError::new_err(format!(
        "{} object is not subscriptable",
        slf.getattr("__class__")?.str()?
    )))
}

#[pyfunction]
#[pyo3(name = "__setitem__")]
fn frozen_setitem(
    slf: &Bound<'_, PyAny>,
    key: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let above = super_of(&built(slf.py(), &FROZEN_OBJECT)?, slf)?;
    if above.hasattr("__setitem__")? {
        slf.call_method0("_check_init")?;
        above.call_method1("__setitem__", (key, value))?;
    }
    Err(not_subscriptable(slf)?)
}

#[pyfunction]
#[pyo3(name = "__delitem__")]
fn frozen_delitem(slf: &Bound<'_, PyAny>, key: &Bound<'_, PyAny>) -> PyResult<()> {
    let above = super_of(&built(slf.py(), &FROZEN_OBJECT)?, slf)?;
    if above.hasattr("__delitem__")? {
        slf.call_method0("_check_init")?;
        above.call_method1("__delitem__", (key,))?;
    }
    Err(not_subscriptable(slf)?)
}

/// The hash of every slot's name and value, worked out once and kept in
/// `_frozenHash`.
#[pyfunction]
#[pyo3(name = "__hash__")]
fn frozen_hash(slf: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let py = slf.py();
    let kept = slf.getattr("_frozenHash")?;
    if !kept.is_none() {
        return Ok(kept.unbind());
    }
    let mut pairs = Vec::new();
    for slot in slf.call_method0("_getSlotsRecursive")?.try_iter()? {
        let slot = slot?;
        let value = slf.getattr(slot.cast::<PyString>()?)?;
        pairs.push(PyTuple::new(py, [slot, value])?);
    }
    let hashed = PyInt::new(py, PyTuple::new(py, pairs)?.hash()?).into_any();
    set_frozen_hash(slf, &hashed)?;
    Ok(hashed.unbind())
}

// ---------------------------------------------------------------- Timer

/// Times from when it is made: called, it gives the seconds since, or once
/// stopped the seconds it ran.
#[pyclass(subclass, module = "music21.common.objects")]
pub(crate) struct Timer(music21::common::objects::Timer);

#[pymethods]
impl Timer {
    #[new]
    fn new() -> Self {
        Self(music21::common::objects::Timer::new())
    }

    /// Starts again from now; a timer starts when it is made anyway.
    fn start(&mut self) {
        self.0.start();
    }

    /// Stops, keeping how long it ran.
    fn stop(&mut self) {
        self.0.stop();
    }

    /// The same as `start`.
    fn clear(&mut self) {
        self.0.clear();
    }

    fn __call__(&self) -> f64 {
        self.0.elapsed()
    }

    /// The seconds, to three places.
    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        let rounded = py
            .import("builtins")?
            .getattr("round")?
            .call1((self.0.elapsed(), 3))?;
        Ok(rounded.str()?.to_string())
    }
}

// ---------------------------------------------------------------- the module

/// Rust functions as the methods of one class.
fn methods_of<'py>(
    m: &Bound<'py, PyModule>,
    class: &str,
    functions: Vec<(&'static str, Bound<'py, PyAny>)>,
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    functions
        .into_iter()
        .map(|(name, function)| Ok((name, method(function, m.name()?.to_str()?, class, name)?)))
        .collect()
}

/// music21's `common.objects`, built; `decorators` is the built
/// `common.decorators`, whose `deprecated` marks `defaultlist`.
pub(crate) fn module<'py>(
    py: Python<'py>,
    decorators: &Bound<'py, PyModule>,
) -> PyResult<Bound<'py, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    let collections = py.import("collections")?;
    let empty = || PyTuple::empty(py).into_any();

    let counter = new_class(
        py,
        MODULE,
        "RelativeCounter",
        "A `Counter` that iterates from the most common key to the least, and \
         gives its counts as proportions or percentages of the whole.",
        vec![collections.getattr("Counter")?],
        methods_of(
            &m,
            "RelativeCounter",
            vec![
                ("__iter__", wrap_pyfunction!(counter_iter, &m)?.into_any()),
                ("items", wrap_pyfunction!(counter_items, &m)?.into_any()),
                (
                    "asProportion",
                    wrap_pyfunction!(asProportion, &m)?.into_any(),
                ),
                (
                    "asPercentage",
                    wrap_pyfunction!(asPercentage, &m)?.into_any(),
                ),
            ],
        )?,
    )?;
    let _ = RELATIVE_COUNTER.set(py, counter.clone().unbind());

    let mut list_methods = methods_of(
        &m,
        "defaultlist",
        vec![
            ("_fill", wrap_pyfunction!(defaultlist_fill, &m)?.into_any()),
            (
                "__setitem__",
                wrap_pyfunction!(defaultlist_setitem, &m)?.into_any(),
            ),
            (
                "__getitem__",
                wrap_pyfunction!(defaultlist_getitem, &m)?.into_any(),
            ),
        ],
    )?;
    let init = method(
        wrap_pyfunction!(defaultlist_init, &m)?.into_any(),
        MODULE,
        "defaultlist",
        "__init__",
    )?;
    let deprecate = super::decorators::deprecated_with(
        decorators,
        "v11",
        "v12",
        "use a plain list or a dict instead",
    )?;
    list_methods.push(("__init__", deprecate.call1((init,))?));
    let default_list = new_class(
        py,
        MODULE,
        "defaultlist",
        "A list that fills itself up to an index it is asked for, with what a \
         function makes, rather than raising `IndexError`.\n\n\
         * Deprecated in v11: use a plain list, checking whether the value is \
         there, or a dict.",
        vec![py.get_type::<PyList>().into_any()],
        list_methods,
    )?;

    let mut slotted_members = vec![("__slots__", empty())];
    slotted_members.extend(methods_of(
        &m,
        "SlottedObjectMixin",
        vec![
            (
                "__getstate__",
                wrap_pyfunction!(slotted_getstate, &m)?.into_any(),
            ),
            (
                "__setstate__",
                wrap_pyfunction!(slotted_setstate, &m)?.into_any(),
            ),
            (
                "_getSlotsRecursive",
                wrap_pyfunction!(slots_recursive, &m)?.into_any(),
            ),
        ],
    )?);
    let slotted = new_class(
        py,
        MODULE,
        "SlottedObjectMixin",
        "A base for classes with `__slots__` that pickles them, even holding \
         weak references or subclassed by a class without slots.",
        vec![py.import("builtins")?.getattr("object")?],
        slotted_members,
    )?;

    let mut equal_members = vec![("__slots__", empty())];
    equal_members.extend(methods_of(
        &m,
        "EqualSlottedObjectMixin",
        vec![
            ("__eq__", wrap_pyfunction!(slotted_eq, &m)?.into_any()),
            ("__ne__", wrap_pyfunction!(slotted_ne, &m)?.into_any()),
        ],
    )?);
    let equal = new_class(
        py,
        MODULE,
        "EqualSlottedObjectMixin",
        "A `SlottedObjectMixin` equal to another of its class whose slots, \
         `id` aside, hold equal values.",
        vec![slotted.clone()],
        equal_members,
    )?;

    let mut frozen_members = vec![
        ("__slots__", empty()),
        (
            "__new__",
            static_method(
                wrap_pyfunction!(frozen_new, &m)?.into_any(),
                MODULE,
                "FrozenObject",
                "__new__",
            )?,
        ),
    ];
    frozen_members.extend(methods_of(
        &m,
        "FrozenObject",
        vec![
            (
                "__setstate__",
                wrap_pyfunction!(frozen_setstate, &m)?.into_any(),
            ),
            (
                "_check_init",
                wrap_pyfunction!(frozen_check_init, &m)?.into_any(),
            ),
            (
                "__setattr__",
                wrap_pyfunction!(frozen_setattr, &m)?.into_any(),
            ),
            (
                "__delattr__",
                wrap_pyfunction!(frozen_delattr, &m)?.into_any(),
            ),
            (
                "__setitem__",
                wrap_pyfunction!(frozen_setitem, &m)?.into_any(),
            ),
            (
                "__delitem__",
                wrap_pyfunction!(frozen_delitem, &m)?.into_any(),
            ),
            ("__hash__", wrap_pyfunction!(frozen_hash, &m)?.into_any()),
        ],
    )?);
    let frozen = new_class(
        py,
        MODULE,
        "FrozenObject",
        "An object whose slots cannot be set once `__init__`, `__new__` or \
         `__setstate__` is done, hashed by their values. The hash is worked out \
         when first asked for and kept in `_frozenHash`, which a subclass's \
         `__slots__` includes.",
        vec![equal.clone()],
        frozen_members,
    )?;
    let _ = FROZEN_OBJECT.set(py, frozen.clone().unbind());

    m.add("RelativeCounter", counter)?;
    m.add("defaultlist", default_list)?;
    m.add_class::<SingletonCounter>()?;
    m.add("SlottedObjectMixin", slotted)?;
    m.add("EqualSlottedObjectMixin", equal)?;
    m.add("FrozenObject", frozen)?;
    m.add_class::<Timer>()?;
    m.add("_slotsByClass", slots_by_class(py).bind(py))?;
    m.add(
        "__all__",
        [
            "defaultlist",
            "SingletonCounter",
            "RelativeCounter",
            "SlottedObjectMixin",
            "EqualSlottedObjectMixin",
            "FrozenObject",
            "Timer",
        ],
    )?;
    Ok(m)
}
