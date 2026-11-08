//! music21's `common.misc`, over the crate's `common::misc`.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple};
use pyo3::wrap_pyfunction;

use music21::common::misc as tools;

/// music21's `misc.__all__`.
const ALL: [&str; 10] = [
    "cleanedFlatNotation",
    "defaultDeepcopy",
    "flattenList",
    "getMissingImportStr",
    "getPlatform",
    "macOSVersion",
    "pitchList",
    "runningInNotebook",
    "sortModules",
    "unique",
];

/// A list of lists flattened one level into a list.
#[pyfunction]
#[pyo3(name = "flattenList")]
#[allow(non_snake_case)]
fn flatten_list<'py>(originalList: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let flat = PyList::empty(originalList.py());
    for inner in originalList.try_iter()? {
        for item in inner?.try_iter()? {
            flat.append(item?)?;
        }
    }
    Ok(flat)
}

/// The items of an iterable each once, in order; `key` says what makes
/// two the same.
#[pyfunction]
#[pyo3(name = "unique", signature = (originalList, *, key = None))]
#[allow(non_snake_case)]
fn unique<'py>(
    originalList: &Bound<'py, PyAny>,
    key: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let py = originalList.py();
    let key = match key {
        Some(key) if key.is_truthy()? => Some(key),
        _ => None,
    };
    let seen = PySet::empty(py)?;
    let found = PyList::empty(py);
    for item in originalList.try_iter()? {
        let item = item?;
        let identity = match key {
            Some(key) => key.call1((&item,))?,
            None => item.clone(),
        };
        if seen.contains(&identity)? {
            continue;
        }
        seen.add(identity)?;
        found.append(item)?;
    }
    Ok(found)
}

/// How to install the optional packages named, or None for none.
#[pyfunction]
#[pyo3(name = "getMissingImportStr")]
#[allow(non_snake_case)]
fn get_missing_import_str(modNameList: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if !modNameList.is_truthy()? {
        return Ok(None);
    }
    let names: Vec<String> = if modNameList.len()? == 1 {
        vec![modNameList.get_item(0)?.str()?.to_str()?.to_string()]
    } else {
        modNameList
            .try_iter()?
            .map(|name| name.and_then(|name| name.extract::<String>()))
            .collect::<PyResult<_>>()?
    };
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    Ok(tools::missing_import_message(&names))
}

/// `win`, `darwin` or `nix`: the platform this runs on.
#[pyfunction]
#[pyo3(name = "getPlatform")]
fn get_platform() -> &'static str {
    tools::platform_name()
}

/// The version of macOS this runs on, as three ints; (0, 0, 0) on any
/// other platform.
#[pyfunction]
#[pyo3(name = "macOSVersion")]
fn mac_os_version(py: Python<'_>) -> PyResult<(i64, i64, i64)> {
    if tools::platform_name() != "darwin" {
        return Ok((0, 0, 0));
    }
    let release: String = py
        .import("platform")?
        .call_method0("mac_ver")?
        .get_item(0)?
        .extract()?;
    let parts: Vec<i64> = release
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    let part = |index: usize| parts.get(index).copied().unwrap_or(0);
    Ok((part(0), part(1), part(2)))
}

/// Modules sorted most recently modified first, by last access then name
/// in a tie.
#[pyfunction]
#[pyo3(name = "sortModules")]
#[allow(non_snake_case)]
fn sort_modules<'py>(moduleList: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py = moduleList.py();
    let os = py.import("os")?;
    let time = py.import("time")?;
    let by_name = PyDict::new(py);
    let sorted = PyList::empty(py);
    for module in moduleList.try_iter()? {
        let module = module?;
        let name = module.getattr("__name__")?;
        by_name.set_item(&name, &module)?;
        let modified = os
            .call_method1("stat", (module.getattr("__file__")?,))?
            .get_item(8)?;
        let local = time.call_method1("localtime", (modified,))?;
        let written = time.call_method1("asctime", (&local,))?;
        sorted.append(PyTuple::new(py, [local, written, name])?)?;
    }
    sorted.sort()?;
    sorted.reverse()?;
    let modules = PyList::empty(py);
    for entry in sorted.iter() {
        modules.append(by_name.get_item(entry.get_item(2)?)?)?;
    }
    Ok(modules)
}

/// Pitches written as music21 once wrote a list of them: `[C4, E4]`.
#[pyfunction]
#[pyo3(name = "pitchList")]
#[allow(non_snake_case)]
fn pitch_list(pitchL: &Bound<'_, PyAny>) -> PyResult<String> {
    let names: Vec<String> = pitchL
        .try_iter()?
        .map(|pitch| pitch.and_then(|pitch| pitch.getattr("nameWithOctave")?.extract()))
        .collect::<PyResult<_>>()?;
    Ok(format!("[{}]", names.join(", ")))
}

/// Whether this runs in Jupyter or Colab, whose `sys.stderr` is an
/// `OutStream`.
#[pyfunction]
#[pyo3(name = "runningInJupyterOrColab")]
fn running_in_jupyter_or_colab(py: Python<'_>) -> PyResult<bool> {
    let stderr = py.import("sys")?.getattr("stderr")?;
    let name = stderr.get_type().getattr("__name__")?;
    name.eq("OutStream")
}

/// Whether this runs in a marimo notebook, asked only where marimo is
/// loaded already.
#[pyfunction]
#[pyo3(name = "runningInMarimo")]
fn running_in_marimo(py: Python<'_>) -> PyResult<bool> {
    let modules = py.import("sys")?.getattr("modules")?;
    if !modules.contains("marimo")? {
        return Ok(false);
    }
    py.import("marimo")?
        .call_method0("running_in_notebook")?
        .is_truthy()
}

/// Whether this runs in a notebook: Jupyter, Colab or marimo.
#[pyfunction]
#[pyo3(name = "runningInNotebook")]
fn running_in_notebook(py: Python<'_>) -> PyResult<bool> {
    Ok(running_in_jupyter_or_colab(py)? || running_in_marimo(py)?)
}

/// The types `defaultDeepcopy` copies by reference, as Python's `copy`
/// does.
fn immutable_types(py: Python<'_>) -> PyResult<Bound<'_, PySet>> {
    let builtins = py.import("builtins")?;
    let types = py.import("types")?;
    let weakref = py.import("weakref")?;
    let found = PySet::empty(py)?;
    found.add(py.None().bind(py).get_type())?;
    found.add(builtins.getattr("Ellipsis")?.get_type())?;
    found.add(builtins.getattr("NotImplemented")?.get_type())?;
    for name in [
        "int", "float", "bool", "complex", "bytes", "str", "type", "range", "property",
    ] {
        found.add(builtins.getattr(name)?)?;
    }
    for name in ["CodeType", "BuiltinFunctionType", "FunctionType"] {
        found.add(types.getattr(name)?)?;
    }
    found.add(weakref.getattr("ref")?)?;
    Ok(found)
}

/// A deep copy of an object from the state its protocol-4 `__reduce_ex__`
/// gives, the attributes in `ignoreAttributes` set to None instead.
#[pyfunction]
#[pyo3(
    name = "defaultDeepcopy",
    signature = (obj, memo = None, *, ignoreAttributes = None)
)]
#[allow(non_snake_case)]
fn default_deepcopy<'py>(
    obj: &Bound<'py, PyAny>,
    memo: Option<Bound<'py, PyAny>>,
    ignoreAttributes: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = obj.py();
    let memo = match memo.filter(|memo| !memo.is_none()) {
        Some(memo) => memo,
        None => PyDict::new(py).into_any(),
    };
    let ignored = match ignoreAttributes {
        Some(ignored) => ignored.clone(),
        None => PyTuple::empty(py).into_any(),
    };
    let reduced = obj.call_method1("__reduce_ex__", (4,))?;
    let function = reduced.get_item(0)?;
    let arguments = reduced.get_item(1)?.cast_into::<PyTuple>()?;
    let state = reduced.get_item(2)?;
    let copied = function.call1(arguments)?;
    let identity = py.import("builtins")?.call_method1("id", (obj,))?;
    memo.set_item(identity, &copied)?;
    let immutable = immutable_types(py)?;
    let deepcopy = py.import("copy")?.getattr("deepcopy")?;
    for pair in state.call_method0("items")?.try_iter()? {
        let pair = pair?;
        let attribute = pair.get_item(0)?;
        let value = pair.get_item(1)?;
        if ignored.contains(&attribute)? {
            copied.setattr(attribute.cast::<PyString>()?, py.None())?;
        } else if immutable.contains(value.get_type())? {
            copied.setattr(attribute.cast::<PyString>()?, value)?;
        } else {
            let value = deepcopy.call1((value, &memo))?;
            copied.setattr(attribute.cast::<PyString>()?, value)?;
        }
    }
    Ok(copied)
}

/// A note name with each flat written `b` written `-` instead.
#[pyfunction]
#[pyo3(name = "cleanedFlatNotation")]
fn cleaned_flat_notation(music_str: &str) -> String {
    tools::cleaned_flat_notation(music_str)
}

/// `music21.common.misc`, built here.
pub(super) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "music21.common.misc")?;
    module.add_function(wrap_pyfunction!(flatten_list, &module)?)?;
    module.add_function(wrap_pyfunction!(unique, &module)?)?;
    module.add_function(wrap_pyfunction!(get_missing_import_str, &module)?)?;
    module.add_function(wrap_pyfunction!(get_platform, &module)?)?;
    module.add_function(wrap_pyfunction!(mac_os_version, &module)?)?;
    module.add_function(wrap_pyfunction!(sort_modules, &module)?)?;
    module.add_function(wrap_pyfunction!(pitch_list, &module)?)?;
    module.add_function(wrap_pyfunction!(running_in_jupyter_or_colab, &module)?)?;
    module.add_function(wrap_pyfunction!(running_in_marimo, &module)?)?;
    // music21 remembers the answer, with `functools.cache`.
    let notebook = wrap_pyfunction!(running_in_notebook, &module)?;
    let cached = py.import("functools")?.call_method1("cache", (notebook,))?;
    module.add("runningInNotebook", cached)?;
    module.add_function(wrap_pyfunction!(default_deepcopy, &module)?)?;
    module.add_function(wrap_pyfunction!(cleaned_flat_notation, &module)?)?;
    module.add("_IMMUTABLE_DEEPCOPY_TYPES", immutable_types(py)?)?;
    module.add("__all__", ALL.to_vec())?;
    Ok(module)
}
