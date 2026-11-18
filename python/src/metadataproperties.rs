//! music21's `metadata.properties`: the vocabulary of metadata names, as
//! the crate's `metadata::property_descriptions` holds it, with the
//! lookup tables music21 builds from it.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{IntoPyDict, PyDict, PyList, PyModule, PyTuple};

use music21::metadata::{PropertyDescription as RsDescription, ValueKind, property_descriptions};

/// The module music21 keeps these in.
const MODULE: &str = "music21.metadata.properties";

/// Where music21 keeps the value classes.
const PRIMITIVES: &str = "music21.metadata.primitives";

/// music21's plural attributes, each answering every value of its name.
const PLURAL_ATTRIBUTES: [&str; 4] = ["composers", "librettists", "lyricists", "software"];

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The class a value kind is held as.
fn value_class<'py>(py: Python<'py>, kind: ValueKind) -> PyResult<Bound<'py, PyAny>> {
    match kind {
        ValueKind::Text => crate::class_of(py, PRIMITIVES, "Text"),
        ValueKind::Copyright => crate::class_of(py, PRIMITIVES, "Copyright"),
        ValueKind::Contributor => crate::class_of(py, PRIMITIVES, "Contributor"),
        ValueKind::Date => crate::class_of(py, PRIMITIVES, "DatePrimitive"),
        ValueKind::Number => Ok(py.get_type::<pyo3::types::PyInt>().into_any()),
    }
}

/// music21's `PropertyDescription`, a dataclass as music21's is.
fn description_class(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let text = crate::class_of(py, PRIMITIVES, "Text")?;
    let none = py.None();
    let fields = PyList::new(
        py,
        [
            ("uniqueName", none.clone_ref(py).into_bound(py)),
            ("name", "".into_pyobject(py)?.into_any()),
            ("namespace", "".into_pyobject(py)?.into_any()),
            ("oldMusic21Abbrev", none.clone_ref(py).into_bound(py)),
            ("oldMusic21WorkId", none.clone_ref(py).into_bound(py)),
            ("valueType", text),
            (
                "needsArticleNormalization",
                false.into_pyobject(py)?.to_owned().into_any(),
            ),
            (
                "isContributor",
                false.into_pyobject(py)?.to_owned().into_any(),
            ),
        ]
        .into_iter()
        .map(|(name, default)| {
            let field = py
                .import("dataclasses")?
                .getattr("field")?
                .call((), Some(&[("default", default)].into_py_dict(py)?))?;
            PyTuple::new(
                py,
                [
                    name.into_pyobject(py)?.into_any(),
                    py.None().into_bound(py),
                    field,
                ],
            )
        })
        .collect::<PyResult<Vec<_>>>()?,
    )?;
    let keywords = PyDict::new(py);
    keywords.set_item("module", MODULE)?;
    py.import("dataclasses")?
        .getattr("make_dataclass")?
        .call(("PropertyDescription", fields), Some(&keywords))
}

/// One description as music21's: the unique name set only where it is
/// not the name itself.
fn description<'py>(
    class: &Bound<'py, PyAny>,
    property: &RsDescription,
) -> PyResult<Bound<'py, PyAny>> {
    let py = class.py();
    let keywords = PyDict::new(py);
    if property.unique_name != property.name {
        keywords.set_item("uniqueName", property.unique_name)?;
    }
    keywords.set_item("name", property.name)?;
    keywords.set_item("namespace", property.namespace)?;
    keywords.set_item("oldMusic21Abbrev", property.old_abbreviation)?;
    keywords.set_item("oldMusic21WorkId", property.old_work_id)?;
    keywords.set_item("valueType", value_class(py, property.value_kind)?)?;
    keywords.set_item(
        "needsArticleNormalization",
        property.needs_article_normalization,
    )?;
    keywords.set_item("isContributor", property.is_contributor)?;
    class.call((), Some(&keywords))
}

/// `music21.metadata.properties`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let class = description_class(py)?;
        m.add("PropertyDescription", &class)?;
        let mut kinds = Vec::new();
        for name in ["DatePrimitive", "Text", "Contributor", "Copyright"] {
            let kind = crate::class_of(py, PRIMITIVES, name)?;
            m.add(name, &kind)?;
            kinds.push(kind);
        }
        // music21's `ValueType`: what a metadata value may be.
        kinds.push(py.get_type::<pyo3::types::PyInt>().into_any());
        let union = py
            .import("typing")?
            .getattr("Union")?
            .get_item(PyTuple::new(py, kinds)?)?;
        m.add("ValueType", union)?;

        // The vocabulary, and the tables music21 builds from it.
        let properties: Vec<RsDescription> = property_descriptions().collect();
        let mut described = Vec::with_capacity(properties.len());
        for property in &properties {
            described.push(description(&class, property)?);
        }
        m.add(
            "STANDARD_PROPERTY_DESCRIPTIONS",
            PyTuple::new(py, &described)?,
        )?;

        let namespaced =
            |property: &RsDescription| format!("{}:{}", property.namespace, property.name);
        let ns_to_description = PyDict::new(py);
        let ns_to_unique = PyDict::new(py);
        let unique_to_ns = PyDict::new(py);
        let unique_to_description = PyDict::new(py);
        let unique_to_type = PyDict::new(py);
        let abbreviation_to_ns = PyDict::new(py);
        let work_id_to_ns = PyDict::new(py);
        let work_id_to_unique = PyDict::new(py);
        let unique_to_work_id = PyDict::new(py);
        for (property, object) in properties.iter().zip(&described) {
            let ns = namespaced(property);
            ns_to_description.set_item(&ns, object)?;
            ns_to_unique.set_item(&ns, property.unique_name)?;
            unique_to_ns.set_item(property.unique_name, &ns)?;
            unique_to_description.set_item(property.unique_name, object)?;
            unique_to_type.set_item(property.unique_name, value_class(py, property.value_kind)?)?;
            if let Some(abbreviation) = property.old_abbreviation {
                abbreviation_to_ns.set_item(abbreviation, &ns)?;
            }
            if let Some(work_id) = property.old_work_id {
                work_id_to_ns.set_item(work_id, &ns)?;
                work_id_to_unique.set_item(work_id, property.unique_name)?;
                unique_to_work_id.set_item(property.unique_name, work_id)?;
            }
        }
        m.add("NAMESPACE_NAME_TO_PROPERTY_DESCRIPTION", &ns_to_description)?;
        m.add("NAMESPACE_NAME_TO_UNIQUE_NAME", &ns_to_unique)?;
        m.add("UNIQUE_NAME_TO_NAMESPACE_NAME", &unique_to_ns)?;
        m.add(
            "UNIQUE_NAME_TO_PROPERTY_DESCRIPTION",
            &unique_to_description,
        )?;
        m.add("UNIQUE_NAME_TO_VALUE_TYPE", &unique_to_type)?;
        m.add(
            "MUSIC21_ABBREVIATION_TO_NAMESPACE_NAME",
            &abbreviation_to_ns,
        )?;
        m.add("MUSIC21_WORK_ID_TO_NAMESPACE_NAME", &work_id_to_ns)?;
        m.add("MUSIC21_WORK_ID_TO_UNIQUE_NAME", &work_id_to_unique)?;
        m.add("UNIQUE_NAME_TO_MUSIC21_WORK_ID", &unique_to_work_id)?;

        // The name lists, in the tables' order.
        let keys = |table: &Bound<'_, PyDict>| -> Vec<String> {
            table
                .keys()
                .iter()
                .filter_map(|key| key.extract().ok())
                .collect()
        };
        let unique_names = keys(&unique_to_ns);
        let work_ids = keys(&work_id_to_ns);
        let abbreviations = keys(&abbreviation_to_ns);
        m.add("ALL_UNIQUE_NAMES", &unique_names)?;
        m.add("ALL_MUSIC21_WORK_IDS", &work_ids)?;
        m.add("ALL_MUSIC21_ABBREVIATIONS", &abbreviations)?;
        m.add("ALL_NAMESPACE_NAMES", keys(&ns_to_description))?;
        // `software` is only ever plural. music21 adds the three file
        // names as one string, joined, and so does this.
        let mut single: Vec<String> = unique_names
            .iter()
            .filter(|name| name.as_str() != "software")
            .cloned()
            .chain(work_ids)
            .chain(abbreviations)
            .collect();
        single.push("fileFormatfilePathfileNumber".to_string());
        let mut legal = single.clone();
        legal.extend(PLURAL_ATTRIBUTES.iter().map(|name| (*name).to_string()));
        m.add("ALL_SINGLE_ATTRIBUTE_NAMES", single)?;
        m.add("ALL_PLURAL_ATTRIBUTE_NAMES", PLURAL_ATTRIBUTES.to_vec())?;
        m.add("ALL_LEGAL_ATTRIBUTES", legal)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
