//! music21's `common.classTools`: what kind of thing an object is, and
//! attributes set for the length of a `with`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyFrozenSet, PyList, PyString, PyTuple, PyType};

/// An attribute name, which Python requires to be text.
fn attribute_name<'py>(name: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyString>> {
    Ok(name.cast::<PyString>()?.clone())
}

use music21::common::classtools::class_to_class_str;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.classTools";

/// What `tempAttribute` is given when it is given no new value.
static SENTINEL: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn sentinel(py: Python<'_>) -> PyResult<&Py<PyAny>> {
    SENTINEL.get_or_try_init(py, || {
        Ok(py.import("builtins")?.getattr("object")?.call0()?.unbind())
    })
}

/// The default `new_val`, found again for each call.
fn no_new_value() -> Py<PyAny> {
    Python::attach(|py| sentinel(py).map_or_else(|_| py.None(), |found| found.clone_ref(py)))
}

/// Whether `usrData` is an `int`, `True` and `False` not counting.
#[pyfunction]
pub(crate) fn isInt(usrData: &Bound<'_, PyAny>) -> bool {
    usrData.is_instance_of::<pyo3::types::PyInt>() && !is_bool_object(usrData)
}

/// Whether `usrData` is `True` or `False` itself.
fn is_bool_object(object: &Bound<'_, PyAny>) -> bool {
    let py = object.py();
    object.is(pyo3::types::PyBool::new(py, true)) || object.is(pyo3::types::PyBool::new(py, false))
}

/// Whether `usrData` is a number: anything zero can be added to, except
/// `True` and `False`. Whatever adding raises (but not an interrupt) means
/// it is not.
#[pyfunction]
pub(crate) fn isNum(usrData: &Bound<'_, PyAny>) -> PyResult<bool> {
    match usrData.add(0) {
        Ok(_) => Ok(!is_bool_object(usrData)),
        Err(error) if error.is_instance_of::<PyException>(usrData.py()) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Whether `usrData` is a list or a tuple, subclasses included.
#[pyfunction]
pub(crate) fn isListLike(usrData: &Bound<'_, PyAny>) -> bool {
    usrData.is_instance_of::<PyList>() || usrData.is_instance_of::<PyTuple>()
}

/// Whether `usrData` can be iterated over and is not text: it has an
/// `__iter__` and is not a class.
#[pyfunction]
pub(crate) fn isIterable(usrData: &Bound<'_, PyAny>) -> PyResult<bool> {
    if usrData.is_instance_of::<PyString>() || usrData.is_instance_of::<PyBytes>() {
        return Ok(false);
    }
    if !usrData.hasattr("__iter__")? {
        return Ok(false);
    }
    let class = usrData.getattr("__class__")?;
    Ok(!class.is(usrData.py().get_type::<PyType>()))
}

/// Whether `usrData` is a collection of `checkType`, judged by its first
/// item: an empty one holds anything. Reads one item, so is not for
/// something reading uses up.
#[pyfunction]
pub(crate) fn holdsType(
    usrData: &Bound<'_, PyAny>,
    checkType: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if !isIterable(usrData)? {
        return Ok(false);
    }
    match usrData.try_iter()?.next() {
        None => Ok(true),
        Some(first) => first?.is_instance(checkType),
    }
}

/// A class's bare name, cut out of the text `str` gives for it.
#[pyfunction]
pub(crate) fn classToClassStr(classObj: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(class_to_class_str(&classObj.str()?.to_cow()?))
}

/// The set `Music21Object.classSet` is: every class in the instance's
/// class's MRO, by itself, by name and by module-qualified name. Names
/// given in `classNameTuple` stand in for the bare names.
#[pyfunction]
#[pyo3(signature = (instance, classNameTuple = None))]
pub(crate) fn getClassSet<'py>(
    instance: &Bound<'py, PyAny>,
    classNameTuple: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py = instance.py();
    let mro = instance.getattr("__class__")?.call_method0("mro")?;
    let mut members: Vec<Bound<'py, PyAny>> = Vec::new();
    match classNameTuple {
        Some(names) if !names.is_none() => {
            for name in names.try_iter()? {
                members.push(name?);
            }
        }
        _ => {
            for class in mro.try_iter()? {
                members.push(class?.getattr("__name__")?);
            }
        }
    }
    for class in mro.try_iter()? {
        members.push(class?);
    }
    for class in mro.try_iter()? {
        let class = class?;
        let qualified = class
            .getattr("__module__")?
            .add(".")?
            .add(class.getattr("__name__")?)?;
        members.push(qualified);
    }
    PyFrozenSet::new(py, &members)
}

/// What `tempAttribute` hands back: sets the attribute on entering and
/// puts the old value back on leaving, whatever happened in between.
#[pyclass(module = "music21.common.classTools", name = "_TempAttribute")]
pub(crate) struct TempAttribute {
    obj: Py<PyAny>,
    attribute: Py<PyAny>,
    new_val: Py<PyAny>,
    stored: Option<Py<PyAny>>,
}

#[pymethods]
impl TempAttribute {
    fn __enter__(&mut self, py: Python<'_>) -> PyResult<()> {
        let obj = self.obj.bind(py);
        let attribute = attribute_name(self.attribute.bind(py))?;
        self.stored = Some(obj.getattr(&attribute)?.unbind());
        if !self.new_val.is(sentinel(py)?) {
            obj.setattr(&attribute, self.new_val.bind(py))?;
        }
        Ok(())
    }

    #[pyo3(signature = (*exception))]
    fn __exit__(&mut self, py: Python<'_>, exception: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let _ = exception;
        if let Some(stored) = self.stored.take() {
            self.obj
                .bind(py)
                .setattr(attribute_name(self.attribute.bind(py))?, stored.bind(py))?;
        }
        Ok(false)
    }
}

/// Sets `attribute` of `obj` to `new_val` for the length of a `with`, then
/// puts back what it was. With no `new_val`, the attribute is only kept, so
/// whatever the block sets it to is undone.
#[pyfunction]
#[pyo3(signature = (obj, attribute, new_val = no_new_value()))]
pub(crate) fn tempAttribute(
    obj: Py<PyAny>,
    attribute: Py<PyAny>,
    new_val: Py<PyAny>,
) -> TempAttribute {
    TempAttribute {
        obj,
        attribute,
        new_val,
        stored: None,
    }
}

/// What `saveAttributes` hands back.
#[pyclass(module = "music21.common.classTools", name = "_SaveAttributes")]
pub(crate) struct SaveAttributes {
    obj: Py<PyAny>,
    attributes: Vec<Py<PyAny>>,
    stored: Vec<(Py<PyAny>, Py<PyAny>)>,
}

#[pymethods]
impl SaveAttributes {
    fn __enter__(&mut self, py: Python<'_>) -> PyResult<()> {
        let obj = self.obj.bind(py);
        let stored = pyo3::types::PyDict::new(py);
        for attribute in &self.attributes {
            let name = attribute_name(attribute.bind(py))?;
            stored.set_item(&name, obj.getattr(&name)?)?;
        }
        self.stored = stored
            .iter()
            .map(|(key, value)| (key.unbind(), value.unbind()))
            .collect();
        Ok(())
    }

    #[pyo3(signature = (*exception))]
    fn __exit__(&mut self, py: Python<'_>, exception: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let _ = exception;
        let obj = self.obj.bind(py);
        for (attribute, value) in std::mem::take(&mut self.stored) {
            obj.setattr(attribute_name(attribute.bind(py))?, value.bind(py))?;
        }
        Ok(false)
    }
}

/// Keeps the named attributes of `obj` for the length of a `with` and puts
/// them back afterwards, whatever the block set them to.
#[pyfunction]
#[pyo3(signature = (obj, *attributeList))]
pub(crate) fn saveAttributes(obj: Py<PyAny>, attributeList: &Bound<'_, PyTuple>) -> SaveAttributes {
    SaveAttributes {
        obj,
        attributes: attributeList.iter().map(Bound::unbind).collect(),
        stored: Vec::new(),
    }
}

/// music21's `common.classTools`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_function(wrap_pyfunction!(isInt, &m)?)?;
    m.add_function(wrap_pyfunction!(isNum, &m)?)?;
    m.add_function(wrap_pyfunction!(isListLike, &m)?)?;
    m.add_function(wrap_pyfunction!(isIterable, &m)?)?;
    m.add_function(wrap_pyfunction!(holdsType, &m)?)?;
    m.add_function(wrap_pyfunction!(classToClassStr, &m)?)?;
    m.add_function(wrap_pyfunction!(getClassSet, &m)?)?;
    m.add_function(wrap_pyfunction!(tempAttribute, &m)?)?;
    m.add_function(wrap_pyfunction!(saveAttributes, &m)?)?;
    m.add("TEMP_ATTRIBUTE_SENTINEL", sentinel(py)?.bind(py))?;
    m.add(
        "__all__",
        [
            "holdsType",
            "isNum",
            "isInt",
            "isListLike",
            "isIterable",
            "classToClassStr",
            "getClassSet",
            "tempAttribute",
            "saveAttributes",
        ],
    )?;
    Ok(m)
}
