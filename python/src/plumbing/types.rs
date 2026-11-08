//! music21's `common.types`: the type aliases and type variables its
//! annotations use, as the `typing` objects music21's own module makes.
//!
//! On Python 3.12 and later each alias is a `typing.TypeAliasType`, which is
//! what music21's `type` statements make; before that, there being no such
//! type, an alias is the union or literal it stands for.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyInt, PyList, PyString, PyTuple, PyType};

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.types";

/// The class music21's stream type variables are bound by.
const STREAM: &str = "music21.stream.Stream";

/// The class music21's object type variables are bound by.
const MUSIC21_OBJECT: &str = "music21.base.Music21Object";

/// The union of these types, as `a | b | c` makes it.
fn union<'py>(types: &[Bound<'py, PyAny>]) -> PyResult<Bound<'py, PyAny>> {
    let mut made = types[0].clone();
    for next in &types[1..] {
        made = made.bitor(next)?;
    }
    Ok(made)
}

/// A `TypeVar` named `name`, bound by the class `bound` names. One in a
/// `type` statement's brackets infers its variance, which Python before
/// 3.12 cannot say.
fn type_var<'py>(
    py: Python<'py>,
    name: &str,
    bound: &str,
    infers_variance: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords = PyDict::new(py);
    keywords.set_item("bound", bound)?;
    if infers_variance && py.import("typing")?.hasattr("TypeAliasType")? {
        keywords.set_item("infer_variance", true)?;
    }
    let made = py
        .import("typing")?
        .getattr("TypeVar")?
        .call((name,), Some(&keywords))?;
    made.setattr("__module__", MODULE)?;
    Ok(made)
}

/// An alias named `name` for `value`, generic over `parameters`.
fn alias<'py>(
    py: Python<'py>,
    name: &str,
    value: Bound<'py, PyAny>,
    parameters: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let Ok(alias_type) = py.import("typing")?.getattr("TypeAliasType") else {
        return Ok(value);
    };
    let keywords = PyDict::new(py);
    keywords.set_item("type_params", PyTuple::new(py, parameters)?)?;
    let made = alias_type.call((name, value), Some(&keywords))?;
    // Where music21's are, rather than in `typing`.
    let _ = made.setattr("__module__", MODULE);
    Ok(made)
}

/// music21's `common.types`, built; `enums` is the built `common.enums`,
/// whose `OffsetSpecial` one alias names.
pub(crate) fn module<'py>(
    py: Python<'py>,
    enums: &Bound<'py, PyModule>,
) -> PyResult<Bound<'py, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    let abc = py.import("collections.abc")?;
    let callable = abc.getattr("Callable")?;
    let iterable = abc.getattr("Iterable")?;
    let fraction = py.import("fractions")?.getattr("Fraction")?;
    let text = py.get_type::<PyString>().into_any();
    let float = py.get_type::<PyFloat>().into_any();
    let int = py.get_type::<PyInt>().into_any();
    let offset_special = enums.getattr("OffsetSpecial")?;

    let doc_order = py
        .get_type::<PyList>()
        .get_item(union(&[text.clone(), callable])?)?;
    m.add("DocOrder", alias(py, "DocOrder", doc_order, &[])?)?;
    m.add(
        "OffsetQL",
        alias(
            py,
            "OffsetQL",
            union(&[float.clone(), fraction.clone()])?,
            &[],
        )?,
    )?;
    m.add(
        "OffsetQLSpecial",
        alias(
            py,
            "OffsetQLSpecial",
            union(&[float.clone(), fraction.clone(), offset_special])?,
            &[],
        )?,
    )?;
    m.add(
        "OffsetQLIn",
        alias(py, "OffsetQLIn", union(&[int, float, fraction])?, &[])?,
    )?;

    for name in ["StreamType", "StreamType2"] {
        m.add(name, type_var(py, name, STREAM, false)?)?;
    }
    for name in ["M21ObjType", "M21ObjType2", "ChangedM21ObjType"] {
        m.add(name, type_var(py, name, MUSIC21_OBJECT, false)?)?;
    }

    // `type ClassListType[M21ObjType: ...]` has a type variable of its own,
    // not the module's.
    let parameter = type_var(py, "M21ObjType", MUSIC21_OBJECT, true)?;
    let class_of = py.get_type::<PyType>().get_item(&parameter)?;
    let class_list = union(&[
        text.clone(),
        iterable.get_item(&text)?,
        class_of.clone(),
        iterable.get_item(&class_of)?,
    ])?;
    m.add(
        "ClassListType",
        alias(py, "ClassListType", class_list, &[parameter])?,
    )?;

    let steps = PyTuple::new(py, ["C", "D", "E", "F", "G", "A", "B"])?;
    let step_name = py.import("typing")?.getattr("Literal")?.get_item(steps)?;
    m.add("StepName", alias(py, "StepName", step_name, &[])?)?;
    Ok(m)
}
