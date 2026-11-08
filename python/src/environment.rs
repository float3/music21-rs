//! music21's `environment`: the user's preferences, the singleton every
//! module's `environLocal` reads them through, and its debugging and
//! warning output.
//!
//! The preferences, their defaults, the settings file's layout and where it
//! lives are the crate's (`music21::environment`). What is here holds them
//! as music21 does, in a dictionary of Python values that its callers read
//! and change in place, and asks Python for what Python answers: the
//! temporary directory `tempfile` names, `os.access`, `os.environ`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::{Path, PathBuf};

use music21::environment::{self as settings, LocalCorpus, Platform, Value};
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict, PyList, PyModule, PyString, PyTuple};

pyo3::create_exception!(
    music21_rs_facade,
    EnvironmentException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    UserSettingsException,
    EnvironmentException
);

/// The module music21 calls `environment`, built on first asking: its
/// exceptions are built on music21's `Music21Exception`, which is not to
/// be reached for while the wheel is imported.
static MODULE: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// `_environStorage`: the singleton and the platform it was made for.
static STORAGE: pyo3::sync::PyOnceLock<Py<PyDict>> = pyo3::sync::PyOnceLock::new();

/// `LocalCorpusSettings`, a `list` subclass, built once.
static LOCAL_CORPUS_SETTINGS: pyo3::sync::PyOnceLock<Py<PyAny>> = pyo3::sync::PyOnceLock::new();

fn environment_error(message: impl Into<String>) -> PyErr {
    EnvironmentException::new_err(message.into())
}

fn user_settings_error(message: impl Into<String>) -> PyErr {
    UserSettingsException::new_err(message.into())
}

/// A crate error as music21 raises it: an `EnvironmentException`, or a
/// `ValueError` for a number that is not one.
fn raised(error: music21::Error) -> PyErr {
    match error {
        music21::Error::Environment(message) => environment_error(message),
        other => pyo3::exceptions::PyValueError::new_err(other.to_string()),
    }
}

fn storage(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let held = STORAGE.get_or_try_init(py, || {
        let made = PyDict::new(py);
        made.set_item("forcePlatform", py.None())?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `envSingleton`: the one `_EnvironmentCore` every
/// `Environment` reads and writes, made on first asking.
#[pyfunction]
fn envSingleton(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let stored = storage(py)?;
    if let Some(instance) = stored.get_item("instance")? {
        return Ok(instance);
    }
    let made = Bound::new(py, EnvironmentCore::new(py, None)?)?.into_any();
    stored.set_item("instance", &made)?;
    Ok(made)
}

/// The platform music21 is told to act as, or the one it is on.
fn platform_of(forced: Option<&str>) -> Platform {
    forced
        .and_then(Platform::from_name)
        .unwrap_or_else(Platform::current)
}

/// The running macOS's version, `(0, 0, 0)` off a Mac: music21's
/// `common.macOSVersion`.
fn mac_version(py: Python<'_>) -> PyResult<(u32, u32, u32)> {
    if Platform::current() != Platform::Darwin {
        return Ok((0, 0, 0));
    }
    let release: String = py
        .import("platform")?
        .call_method0("mac_ver")?
        .get_item(0)?
        .extract()?;
    let mut parts = release.split('.').map(|part| part.parse().unwrap_or(0));
    Ok((
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ))
}

fn pathlib_path<'py>(py: Python<'py>, path: &Path) -> PyResult<Bound<'py, PyAny>> {
    py.import("pathlib")?
        .getattr("Path")?
        .call1((path.to_string_lossy().into_owned(),))
}

/// Text a path-like value stands for, as `os.fspath` gives it.
fn path_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(text) = value.extract::<String>() {
        return Ok(text);
    }
    value
        .py()
        .import("os")?
        .call_method1("fspath", (value,))?
        .extract()
}

/// The user's id where the system has one: what music21 checks a
/// temporary directory's owner against.
fn user_id(py: Python<'_>) -> PyResult<Option<u32>> {
    let platform: String = py.import("sys")?.getattr("platform")?.extract()?;
    if platform == "win32" {
        return Ok(None);
    }
    Ok(Some(py.import("os")?.call_method0("getuid")?.extract()?))
}

/// Whether Python's `os.access` lets this user read and write a path, and
/// it is there: music21's `_checkAccessibility`.
fn accessible(py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<bool> {
    let is_path = path.is_instance(&py.import("pathlib")?.getattr("Path")?)?;
    if !is_path && !path.is_instance_of::<PyString>() {
        return Ok(false);
    }
    let os = py.import("os")?;
    let exists = os
        .getattr("path")?
        .call_method1("exists", (path,))?
        .is_truthy()?;
    let readable = os
        .call_method1("access", (path, os.getattr("R_OK")?))?
        .is_truthy()?;
    let writable = os
        .call_method1("access", (path, os.getattr("W_OK")?))?
        .is_truthy()?;
    Ok(exists && readable && writable)
}

/// The same for a path the crate holds; a Python error reads as no.
fn accessible_path(py: Python<'_>, path: &Path) -> bool {
    pathlib_path(py, path)
        .and_then(|path| accessible(py, &path))
        .unwrap_or(false)
}

/// music21's `LocalCorpusSettings`: a list of directories, with a `name`
/// and a `cacheFilePath`.
///
/// A `list` subclass, as music21's is, so it is a class made with `type`
/// over these functions: pyo3 cannot build a class on `list`.
fn local_corpus_settings(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let class = LOCAL_CORPUS_SETTINGS.get_or_try_init(py, || {
        let partial = py.import("functools")?.getattr("partialmethod")?;
        let namespace = PyDict::new(py);
        let module = PyModule::new(py, "music21_rs_environment")?;
        namespace.set_item(
            "__init__",
            partial.call1((wrap_pyfunction!(local_corpus_init, &module)?,))?,
        )?;
        namespace.set_item(
            "__repr__",
            partial.call1((wrap_pyfunction!(local_corpus_repr, &module)?,))?,
        )?;
        namespace.set_item("__module__", "music21.environment")?;
        namespace.set_item("__qualname__", "LocalCorpusSettings")?;
        namespace.set_item(
            "__doc__",
            "The directories of a local corpus, as a list, with the corpus's \
             `name` (None for the unnamed one) and the `cacheFilePath` its \
             metadata is cached in (None for the temporary directory).",
        )?;
        let bases = PyTuple::new(py, [py.get_type::<PyList>()])?;
        let made = py.import("builtins")?.getattr("type")?.call1((
            "LocalCorpusSettings",
            bases,
            namespace,
        ))?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    Ok(class.bind(py).clone())
}

#[pyfunction]
#[pyo3(signature = (slf, paths=None, name=None, cacheFilePath=None))]
fn local_corpus_init(
    slf: &Bound<'_, PyAny>,
    paths: Option<&Bound<'_, PyAny>>,
    name: Option<&Bound<'_, PyAny>>,
    cacheFilePath: Option<&Bound<'_, PyAny>>,
) -> PyResult<()> {
    let py = slf.py();
    let paths = match paths {
        Some(paths) if !paths.is_none() => paths.clone(),
        _ => PyList::empty(py).into_any(),
    };
    py.get_type::<PyList>()
        .call_method1("__init__", (slf, paths))?;
    let none = py.None().into_bound(py);
    slf.setattr("name", name.unwrap_or(&none))?;
    slf.setattr("cacheFilePath", cacheFilePath.unwrap_or(&none))?;
    Ok(())
}

#[pyfunction]
fn local_corpus_repr(slf: &Bound<'_, PyAny>) -> PyResult<String> {
    let py = slf.py();
    let listed: String = py
        .get_type::<PyList>()
        .call_method1("__repr__", (slf,))?
        .extract()?;
    let mut shown = format!("LocalCorpusSettings({listed}");
    for attribute in ["name", "cacheFilePath"] {
        let value = slf.getattr(attribute)?;
        if !value.is_none() {
            shown.push_str(&format!(", {attribute}={}", value.repr()?));
        }
    }
    shown.push(')');
    Ok(shown)
}

/// A crate corpus as a `LocalCorpusSettings`.
fn corpus_object<'py>(py: Python<'py>, corpus: &LocalCorpus) -> PyResult<Bound<'py, PyAny>> {
    local_corpus_settings(py)?.call1((
        corpus.paths.clone(),
        corpus.name.clone(),
        corpus.cache_file_path.clone(),
    ))
}

/// A `LocalCorpusSettings` as the crate's corpus.
fn corpus_value(object: &Bound<'_, PyAny>, name: Option<String>) -> PyResult<LocalCorpus> {
    let mut paths = Vec::new();
    for path in object.try_iter()? {
        let path = path?;
        if !path.is_none() {
            paths.push(path.str()?.to_string());
        }
    }
    let cache = object
        .getattr("cacheFilePath")
        .ok()
        .filter(|cache| !cache.is_none())
        .map(|cache| cache.str().map(|text| text.to_string()))
        .transpose()?;
    Ok(LocalCorpus {
        name,
        paths,
        cache_file_path: cache,
    })
}

/// A preference's Python value for the crate to write out.
fn setting_value(key: &str, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    if key == "localCorpusSettings" {
        return Ok(Value::LocalCorpus(corpus_value(value, None)?));
    }
    if key == "localCorporaSettings" {
        let mut corpora = Vec::new();
        for (name, corpus) in value.cast::<PyDict>()?.iter() {
            corpora.push(corpus_value(&corpus, Some(name.str()?.to_string()))?);
        }
        return Ok(Value::LocalCorpora(corpora));
    }
    if value.is_none() {
        return Ok(Value::Unset);
    }
    Ok(Value::Text(value.str()?.to_string()))
}

/// The crate's value as music21 holds it.
fn python_value<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    match value {
        Value::Unset => Ok(py.None().into_bound(py)),
        Value::Text(text) => Ok(PyString::new(py, text).into_any()),
        Value::Number(number) => Ok(number.into_pyobject(py)?.into_any()),
        Value::LocalCorpus(corpus) => corpus_object(py, corpus),
        Value::LocalCorpora(corpora) => {
            let made = PyDict::new(py);
            for corpus in corpora {
                made.set_item(corpus.name.clone(), corpus_object(py, corpus)?)?;
            }
            Ok(made.into_any())
        }
    }
}

/// What a settings file says, put into `reference` as music21's
/// `_fromSettings` puts it: the local corpora always, and a preference only
/// where the dictionary already has the key.
fn apply_settings(reference: &Bound<'_, PyDict>, read: &[(String, Value)]) -> PyResult<()> {
    let py = reference.py();
    for (key, value) in read {
        let special = matches!(value, Value::LocalCorpus(_) | Value::LocalCorpora(_));
        if special || reference.contains(key)? {
            reference.set_item(key, python_value(py, value)?)?;
        }
    }
    Ok(())
}

/// music21's `etIndent`: indents an ElementTree element in place for
/// writing out, `spaces` a level.
#[pyfunction]
#[pyo3(signature = (elem, level=0, spaces=2))]
fn etIndent(elem: &Bound<'_, PyAny>, level: usize, spaces: usize) -> PyResult<()> {
    let blank = |text: Bound<'_, PyAny>| -> PyResult<bool> {
        if text.is_none() {
            return Ok(true);
        }
        Ok(text.str()?.to_string().trim().is_empty())
    };
    let indent = format!("\n{}", " ".repeat(level * spaces));
    if elem.len()? > 0 {
        if blank(elem.getattr("text")?)? {
            elem.setattr("text", format!("{indent}{}", " ".repeat(spaces)))?;
        }
        if blank(elem.getattr("tail")?)? {
            elem.setattr("tail", &indent)?;
        }
        for child in elem.try_iter()? {
            // music21 indents the children two spaces a level, whatever
            // `spaces` is.
            etIndent(&child?, level + 1, 2)?;
        }
        if blank(elem.getattr("tail")?)? {
            elem.setattr("tail", &indent)?;
        }
    } else if level > 0 && blank(elem.getattr("tail")?)? {
        elem.setattr("tail", &indent)?;
    }
    Ok(())
}

/// music21's `_EnvironmentCore`: the preferences themselves, which every
/// `Environment` shares.
#[pyclass(
    name = "_EnvironmentCore",
    module = "music21.environment",
    dict,
    subclass
)]
pub struct EnvironmentCore {
    /// The preferences by name.
    #[pyo3(get, set, name = "_ref")]
    reference: Py<PyDict>,
    /// The preferences that name a path.
    #[pyo3(get, set, name = "_keysToPaths")]
    keys_to_paths: Py<PyList>,
    /// The temporary directory found last.
    #[pyo3(get, set)]
    defaultRootTempDir: Py<PyAny>,
}

impl EnvironmentCore {
    /// The defaults for the platform, the settings file not read.
    fn new(py: Python<'_>, forcePlatform: Option<String>) -> PyResult<Self> {
        let made = Self {
            reference: PyDict::new(py).unbind(),
            keys_to_paths: PyList::new(py, settings::PATH_KEYS)?.unbind(),
            defaultRootTempDir: py.None(),
        };
        load_defaults(made.reference.bind(py), forcePlatform.as_deref())?;
        Ok(made)
    }

    /// What the dictionary holds, as the crate writes it out.
    fn entries(reference: &Bound<'_, PyDict>) -> PyResult<Vec<(String, Value)>> {
        let mut found = Vec::new();
        for (key, value) in reference.iter() {
            let key: String = key.extract()?;
            let value = setting_value(&key, &value)?;
            found.push((key, value));
        }
        Ok(found)
    }
}

/// Every default for the platform, put into the dictionary.
fn load_defaults(reference: &Bound<'_, PyDict>, forced: Option<&str>) -> PyResult<()> {
    let py = reference.py();
    let platform = platform_of(forced);
    for (key, value) in settings::defaults(platform, mac_version(py)?) {
        reference.set_item(key, python_value(py, &value)?)?;
    }
    Ok(())
}

#[pymethods]
impl EnvironmentCore {
    #[new]
    #[pyo3(signature = (forcePlatform=None))]
    fn py_new(py: Python<'_>, forcePlatform: Option<String>) -> PyResult<Self> {
        let reading = forcePlatform.is_none();
        let core = Self::new(py, forcePlatform)?;
        if reading {
            read_into(py, core.reference.bind(py), None, &core)?;
        }
        Ok(core)
    }

    fn __getitem__<'py>(slf: &Bound<'py, Self>, key: &str) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let reference = slf.borrow().reference.clone_ref(py).into_bound(py);
        let mut value = if key == "localCorpusPath" {
            let corpus = reference
                .get_item("localCorpusSettings")?
                .ok_or_else(|| environment_error("no preference: localCorpusSettings"))?;
            match corpus.try_iter()?.next() {
                Some(first) => first?,
                None => PyString::new(py, "").into_any(),
            }
        } else {
            reference
                .get_item(key)?
                .ok_or_else(|| environment_error(format!("no preference: {key}")))?
        };
        if let Ok(bytes) = value.cast::<pyo3::types::PyBytes>() {
            let decoded = String::from_utf8_lossy(bytes.as_bytes()).into_owned();
            value = PyString::new(py, &decoded).into_any();
        }
        if key == "debug" {
            let level = settings::debug_level(&value.str()?.to_string()).map_err(raised)?;
            value = level.into_pyobject(py)?.into_any();
        }
        let unset = value.is_none() || value.eq("")?;
        if unset {
            return Ok(py.None().into_bound(py));
        }
        let paths = slf.borrow().keys_to_paths.clone_ref(py).into_bound(py);
        if paths.contains(key)? {
            return py.import("pathlib")?.getattr("Path")?.call1((value,));
        }
        Ok(value)
    }

    fn __setitem__(slf: &Bound<'_, Self>, key: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        let reference = slf.borrow().reference.clone_ref(py).into_bound(py);
        let mut value = value;
        if key.to_lowercase().contains("path") && !value.is_none() {
            let cleaned = settings::clean_path(&path_text(&value)?);
            value = PyString::new(py, &cleaned).into_any();
        }
        if !reference.contains(key)? && key != "localCorpusPath" {
            return Err(environment_error(format!("no preference: {key}")));
        }
        if !value.is_none() && value.eq("")? {
            value = py.None().into_bound(py);
        }
        if matches!(key, "showFormat" | "writeFormat" | "autoDownload") {
            let given: String = value.call_method0("lower")?.extract()?;
            let taken = settings::accepted(key, &given).map_err(raised)?;
            value = PyString::new(py, &taken).into_any();
        }
        if key == "localCorpusSettings" && !value.is_instance(&local_corpus_settings(py)?)? {
            return Err(environment_error(format!(
                "{} is not an acceptable value for preference: {key}",
                value.str()?
            )));
        }
        if key != "localCorpusPath" {
            return reference.set_item(key, value);
        }
        let corpus = reference
            .get_item("localCorpusSettings")?
            .ok_or_else(|| environment_error("no preference: localCorpusSettings"))?;
        let written = value.str()?;
        if !corpus.contains(&written)? {
            corpus.call_method1("append", (written,))?;
        }
        Ok(())
    }

    fn __repr__(&self) -> &'static str {
        "<Environment>"
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self.reference.bind(py).repr()?.to_string())
    }

    /// Reads an ElementTree of settings into `ref` (a new dictionary where
    /// none is given): music21's `_fromSettings`.
    #[pyo3(signature = (settingsTree, r#ref=None))]
    fn _fromSettings(
        &self,
        py: Python<'_>,
        settingsTree: &Bound<'_, PyAny>,
        r#ref: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let reference = r#ref.unwrap_or_else(|| PyDict::new(py));
        let written: String = py
            .import("xml.etree.ElementTree")?
            .call_method(
                "tostring",
                (settingsTree.call_method0("getroot")?,),
                Some(&[("encoding", "unicode")].into_py_dict(py)?),
            )?
            .extract()?;
        let read = settings::read_settings(&written).map_err(raised)?;
        apply_settings(&reference, &read)
    }

    /// Puts every default into the preferences.
    #[pyo3(signature = (forcePlatform=None))]
    fn _loadDefaults(&self, py: Python<'_>, forcePlatform: Option<String>) -> PyResult<()> {
        load_defaults(self.reference.bind(py), forcePlatform.as_deref())
    }

    /// Whether a path exists and this user can read and write it.
    fn _checkAccessibility(&self, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<bool> {
        accessible(py, path)
    }

    /// Whether a directory is accessible and no other user can change it.
    fn _isSafeTempDir(&self, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<bool> {
        let held = PathBuf::from(path_text(path)?);
        Ok(settings::is_safe_temp_dir(&held, user_id(py)?, |path| {
            accessible_path(py, path)
        }))
    }

    /// The preferences as an ElementTree, as music21 writes them out.
    #[pyo3(signature = (r#ref=None))]
    fn toSettingsXML<'py>(
        &self,
        py: Python<'py>,
        r#ref: Option<Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let reference = r#ref.unwrap_or_else(|| self.reference.bind(py).clone());
        let written = settings::settings_xml(&Self::entries(&reference)?, false);
        let tree = py.import("xml.etree.ElementTree")?;
        let root = tree.call_method1("fromstring", (written,))?;
        tree.call_method1("ElementTree", (root,))
    }

    /// The directory music21 keeps temporary files in, found as the crate
    /// finds it and made if need be.
    fn getDefaultRootTempDir<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let held = slf.borrow().defaultRootTempDir.clone_ref(py).into_bound(py);
        if accessible(py, &held)? {
            return Ok(held);
        }
        let system: String = py
            .import("tempfile")?
            .call_method0("gettempdir")?
            .extract()?;
        let found = settings::default_root_temp_dir(Path::new(&system), user_id(py)?, |path| {
            accessible_path(py, path)
        })?;
        let found = pathlib_path(py, &found)?;
        slf.borrow_mut().defaultRootTempDir = found.clone().unbind();
        Ok(found)
    }

    fn getKeysToPaths(&self, py: Python<'_>) -> Py<PyList> {
        self.keys_to_paths.clone_ref(py)
    }

    fn getRefKeys<'py>(&self, py: Python<'py>) -> Bound<'py, PyList> {
        self.reference.bind(py).keys()
    }

    /// `directoryScratch` where it is set, which must exist, and the
    /// default temporary directory otherwise.
    fn getRootTempDir<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let scratch = slf
            .borrow()
            .reference
            .bind(py)
            .get_item("directoryScratch")?;
        let Some(scratch) = scratch.filter(|scratch| !scratch.is_none()) else {
            return Self::getDefaultRootTempDir(slf);
        };
        let directory = py.import("pathlib")?.getattr("Path")?.call1((scratch,))?;
        if !directory.call_method0("exists")?.is_truthy()? {
            return Err(environment_error(format!(
                "user-specified scratch directory ({}) does not exist; remove preference \
                 file or reset Environment",
                directory.str()?
            )));
        }
        Ok(directory)
    }

    /// Where the settings file is on this platform.
    fn getSettingsPath<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let os = py.import("os")?;
        let environ = os.getattr("environ")?;
        let variable = |name: &str| -> Option<String> {
            environ
                .call_method1("get", (name,))
                .ok()
                .and_then(|value| value.extract().ok())
        };
        let writable = |path: &Path| -> bool {
            os.getattr("W_OK")
                .and_then(|mode| {
                    os.call_method1("access", (path.to_string_lossy().into_owned(), mode))
                })
                .and_then(|answer| answer.is_truthy())
                .unwrap_or(false)
        };
        let mut failed = None;
        let temporary = || match Self::getDefaultRootTempDir(slf).and_then(|dir| path_text(&dir)) {
            Ok(dir) => PathBuf::from(dir),
            Err(error) => {
                failed = Some(error);
                PathBuf::new()
            }
        };
        let found = settings::settings_path(Platform::current(), variable, writable, temporary);
        if let Some(error) = failed {
            return Err(error);
        }
        pathlib_path(py, &found)
    }

    /// A new temporary file with `suffix`, closed and left in place.
    #[pyo3(signature = (suffix="", returnPathlib=true))]
    fn getTempFile<'py>(
        slf: &Bound<'py, Self>,
        suffix: &str,
        returnPathlib: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        temp_file(slf.py(), &Self::getRootTempDir(slf)?, suffix, returnPathlib)
    }

    fn keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let found = self.reference.bind(py).keys();
        found.append("localCorpusPath")?;
        Ok(found)
    }

    fn formatToKey(&self, m21Format: &str) -> Option<&'static str> {
        settings::format_to_key(m21Format)
    }

    fn formatToApp<'py>(&self, py: Python<'py>, m21Format: &str) -> PyResult<Bound<'py, PyAny>> {
        let Some(key) = settings::format_to_key(m21Format) else {
            return Ok(py.None().into_bound(py));
        };
        match self.reference.bind(py).get_item(key)? {
            Some(app) => Ok(app),
            None => Err(environment_error(format!(
                "{key} is not set in UserSettings. "
            ))),
        }
    }

    /// Reads the settings file, where there is one, over the preferences.
    #[pyo3(signature = (filePath=None))]
    fn read(slf: &Bound<'_, Self>, filePath: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let reference = slf.borrow().reference.clone_ref(py).into_bound(py);
        let path = match filePath.filter(|path| !path.is_none()) {
            Some(path) => path,
            None => Self::getSettingsPath(slf)?,
        };
        read_into(py, &reference, Some(path), &slf.borrow())
    }

    fn restoreDefaults(&mut self, py: Python<'_>) -> PyResult<()> {
        self.reference = PyDict::new(py).unbind();
        load_defaults(self.reference.bind(py), None)
    }

    /// Writes the preferences out to the settings file, as music21 lays
    /// it out.
    #[pyo3(signature = (filePath=None))]
    fn write(slf: &Bound<'_, Self>, filePath: Option<Bound<'_, PyAny>>) -> PyResult<()> {
        let py = slf.py();
        let path = match filePath.filter(|path| !path.is_none()) {
            Some(path) => path,
            None => Self::getSettingsPath(slf)?,
        };
        let path = PathBuf::from(path_text(&path)?);
        let parent_exists = path
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty() || parent.exists());
        if !parent_exists {
            return Err(environment_error(format!(
                "bad file path for .music21rc: {}",
                path.display()
            )));
        }
        let reference = slf.borrow().reference.clone_ref(py).into_bound(py);
        let written = settings::settings_xml(&Self::entries(&reference)?, true);
        std::fs::write(&path, written)?;
        Ok(())
    }
}

/// Reads a settings file into the dictionary, where the file is there.
fn read_into(
    py: Python<'_>,
    reference: &Bound<'_, PyDict>,
    path: Option<Bound<'_, PyAny>>,
    core: &EnvironmentCore,
) -> PyResult<()> {
    let path = match path {
        Some(path) => PathBuf::from(path_text(&path)?),
        None => {
            // The settings path, worked out without a core to ask: the
            // one being made is not a Python object yet.
            let made = Bound::new(
                py,
                EnvironmentCore {
                    reference: PyDict::new(py).unbind(),
                    keys_to_paths: core.keys_to_paths.clone_ref(py),
                    defaultRootTempDir: py.None(),
                },
            )?;
            PathBuf::from(path_text(&EnvironmentCore::getSettingsPath(&made)?)?)
        }
    };
    if !path.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path)?;
    let read = settings::read_settings(&text).map_err(|error| {
        let message = match error {
            music21::Error::Environment(message) => message,
            other => other.to_string(),
        };
        environment_error(format!("Cannot parse file {}: {message}", path.display()))
    })?;
    apply_settings(reference, &read)
}

/// A new temporary file in `directory`, made by Python's `tempfile`.
fn temp_file<'py>(
    py: Python<'py>,
    directory: &Bound<'py, PyAny>,
    suffix: &str,
    as_path: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let suffix = if !suffix.is_empty() && !suffix.starts_with('.') {
        format!(".{suffix}")
    } else {
        suffix.to_owned()
    };
    let keywords = PyDict::new(py);
    keywords.set_item("dir", directory)?;
    keywords.set_item("suffix", suffix)?;
    keywords.set_item("delete", false)?;
    let file = py
        .import("tempfile")?
        .call_method("NamedTemporaryFile", (), Some(&keywords))?;
    let name = file.getattr("name")?;
    file.call_method0("close")?;
    if as_path {
        return py.import("pathlib")?.getattr("Path")?.call1((name,));
    }
    Ok(name)
}

/// The parts of a message as text: music21's `common.formatStr`, which
/// gives anything not text as its `repr`.
fn message_parts(msg: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut parts = Vec::new();
    for part in msg.try_iter()? {
        let part = part?;
        if let Ok(bytes) = part.cast::<pyo3::types::PyBytes>() {
            parts.push(String::from_utf8_lossy(bytes.as_bytes()).into_owned());
        } else if let Ok(text) = part.extract::<String>() {
            parts.push(text);
        } else {
            parts.push(part.repr()?.to_string());
        }
    }
    Ok(parts)
}

fn write_stderr(py: Python<'_>, text: &str) -> PyResult<()> {
    py.import("sys")?
        .getattr("stderr")?
        .call_method1("write", (text,))?;
    Ok(())
}

/// music21's `Environment`: the preferences as a dictionary, temporary
/// files, and the debugging and warning output of the module that made it.
#[pyclass(name = "Environment", module = "music21.environment", dict, subclass)]
pub struct Environment {
    /// The name of the module this belongs to, which its messages start
    /// with.
    #[pyo3(get, set)]
    modNameParent: Py<PyAny>,
}

#[pymethods]
impl Environment {
    /// One for the module named `modName`; with `forcePlatform`, the
    /// shared preferences are made again as they would be on that platform.
    #[new]
    #[pyo3(signature = (modName=None, forcePlatform=None))]
    fn new(
        py: Python<'_>,
        modName: Option<Py<PyAny>>,
        forcePlatform: Option<Py<PyAny>>,
    ) -> PyResult<Self> {
        let stored = storage(py)?;
        let forced = forcePlatform.unwrap_or_else(|| py.None());
        let previous = stored.get_item("forcePlatform")?;
        let same = match previous {
            Some(previous) => previous.eq(forced.bind(py))?,
            None => forced.is_none(py),
        };
        if !same {
            let platform: Option<String> = forced.extract(py)?;
            let made = Bound::new(py, EnvironmentCore::new(py, platform)?)?;
            stored.set_item("instance", made)?;
        }
        Ok(Self {
            modNameParent: modName.unwrap_or_else(|| py.None()),
        })
    }

    #[classattr]
    fn _DOC_ORDER() -> Vec<&'static str> {
        vec!["read", "write", "getSettingsPath"]
    }

    #[classattr]
    fn _DOC_ATTR(py: Python<'_>) -> PyResult<Py<PyDict>> {
        let made = PyDict::new(py);
        made.set_item(
            "modNameParent",
            "The name of the module that holds this Environment.",
        )?;
        Ok(made.unbind())
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.get_item(key)
    }

    fn __setitem__(
        &self,
        py: Python<'_>,
        key: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        envSingleton(py)?.set_item(key, value)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(envSingleton(py)?.repr()?.to_string())
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(envSingleton(py)?.str()?.to_string())
    }

    /// The system's temporary directory with `music21` in it, or another
    /// only this user can change.
    fn getDefaultRootTempDir<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let found = envSingleton(py)?.call_method0("getDefaultRootTempDir")?;
        let message = PyList::new(
            py,
            [
                "environment".into_pyobject(py)?.into_any(),
                "using temporary directory:".into_pyobject(py)?.into_any(),
                found.str()?.into_any(),
            ],
        )?;
        slf.call_method1("printDebug", (message,))?;
        Ok(found)
    }

    fn getKeysToPaths<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method0("getKeysToPaths")
    }

    fn getRefKeys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method0("getRefKeys")
    }

    fn getRootTempDir<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method0("getRootTempDir")
    }

    fn getSettingsPath<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method0("getSettingsPath")
    }

    /// A temporary file with `suffix`, as a `pathlib.Path` or as text.
    #[pyo3(signature = (suffix="", returnPathlib=true))]
    fn getTempFile<'py>(
        slf: &Bound<'py, Self>,
        suffix: &str,
        returnPathlib: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let made = envSingleton(py)?.call_method1("getTempFile", (suffix, returnPathlib))?;
        let message = PyList::new(
            py,
            [
                "environment".into_pyobject(py)?.into_any(),
                "temporary file:".into_pyobject(py)?.into_any(),
                made.clone(),
            ],
        )?;
        slf.call_method1("printDebug", (message,))?;
        Ok(made)
    }

    fn keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method0("keys")
    }

    fn formatToApp<'py>(
        &self,
        py: Python<'py>,
        m21Format: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method1("formatToApp", (m21Format,))
    }

    fn formatToKey<'py>(
        &self,
        py: Python<'py>,
        m21Format: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method1("formatToKey", (m21Format,))
    }

    /// Writes a message to standard error where the `debug` preference is
    /// at `statusLevel` or more, after the module's name.
    #[pyo3(signature = (msg, statusLevel=settings::DEBUG_USER))]
    fn printDebug(&self, py: Python<'_>, msg: &Bound<'_, PyAny>, statusLevel: i64) -> PyResult<()> {
        let level: i64 = envSingleton(py)?.get_item("debug")?.extract()?;
        if level < statusLevel {
            return Ok(());
        }
        let mut parts = if msg.is_instance_of::<PyString>() {
            vec![msg.extract::<String>()?]
        } else {
            message_parts(msg)?
        };
        self.prefixed(py, &mut parts, ":")?;
        write_stderr(py, &settings::format_message(&parts))
    }

    #[pyo3(signature = (filePath=None))]
    fn read<'py>(
        &self,
        py: Python<'py>,
        filePath: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method1("read", (filePath,))
    }

    fn restoreDefaults(&self, py: Python<'_>) -> PyResult<()> {
        envSingleton(py)?.call_method0("restoreDefaults")?;
        Ok(())
    }

    /// Writes a warning to standard error, whatever `debug` says, after the
    /// module's name or `header`.
    #[pyo3(signature = (msg, header=None))]
    fn warn(&self, py: Python<'_>, msg: &Bound<'_, PyAny>, header: Option<String>) -> PyResult<()> {
        let is_number = (msg.is_instance_of::<pyo3::types::PyInt>()
            || msg.is_instance_of::<pyo3::types::PyFloat>())
            && !msg.is_instance_of::<pyo3::types::PyBool>();
        let mut parts = if msg.is_instance_of::<PyString>() {
            vec![msg.extract::<String>()?]
        } else if msg.is_instance_of::<PyDict>() {
            vec![msg.repr()?.to_string()]
        } else if is_number || msg.is_instance_of::<pyo3::exceptions::PyException>() {
            vec![msg.str()?.to_string()]
        } else {
            message_parts(msg)?
        };
        match header {
            Some(header) => parts.insert(0, header),
            None => self.prefixed(py, &mut parts, ": WARNING:")?,
        }
        write_stderr(py, &settings::format_message(&parts))
    }

    #[pyo3(signature = (filePath=None))]
    fn write<'py>(
        &self,
        py: Python<'py>,
        filePath: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        envSingleton(py)?.call_method1("write", (filePath,))
    }

    /// Which MusicXML reader `musicxmlPath` names, or the PNG one in a
    /// notebook.
    fn xmlReaderType(slf: &Bound<'_, Self>) -> PyResult<Option<&'static str>> {
        let py = slf.py();
        let key = if running_in_notebook(py)? {
            "musescoreDirectPNGPath"
        } else {
            "musicxmlPath"
        };
        let path = slf.get_item(key)?;
        if !path.is_truthy()? {
            return Ok(None);
        }
        Ok(settings::xml_reader_type(&path.str()?.to_string()))
    }
}

impl Environment {
    /// The module's name put before a message that does not start with it.
    fn prefixed(&self, py: Python<'_>, parts: &mut Vec<String>, after: &str) -> PyResult<()> {
        let name = self.modNameParent.bind(py);
        if name.is_none() {
            return Ok(());
        }
        let name = name.str()?.to_string();
        if parts.first() != Some(&name) {
            parts.insert(0, format!("{name}{after}"));
        }
        Ok(())
    }
}

/// Whether this runs in Jupyter, Colab or Marimo: music21's
/// `common.runningInNotebook`.
fn running_in_notebook(py: Python<'_>) -> PyResult<bool> {
    let sys = py.import("sys")?;
    let stream: String = sys.getattr("stderr")?.get_type().name()?.extract()?;
    if stream == "OutStream" {
        return Ok(true);
    }
    if !sys.getattr("modules")?.contains("marimo")? {
        return Ok(false);
    }
    py.import("marimo")?
        .call_method0("running_in_notebook")?
        .is_truthy()
}

/// music21's `UserSettings`: the preferences, each change written straight
/// to the settings file.
#[pyclass(name = "UserSettings", module = "music21.environment", dict, subclass)]
pub struct UserSettings {
    #[pyo3(get, set)]
    _environment: Py<PyAny>,
}

#[pymethods]
impl UserSettings {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        let environment = Bound::new(py, Environment::new(py, None, None)?)?;
        Ok(Self {
            _environment: environment.into_any().unbind(),
        })
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self._environment.bind(py).get_item(key)
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self._environment.bind(py).repr()?.to_string())
    }

    fn __str__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(self._environment.bind(py).str()?.to_string())
    }

    /// Sets a preference and writes the settings file. A path must exist.
    fn __setitem__(&self, py: Python<'_>, key: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
        let environment = self._environment.bind(py);
        let paths = environment.call_method0("getKeysToPaths")?;
        let mut value = value;
        if paths.contains(key)? {
            let skipped = value.is_none() || value.str()?.to_string().starts_with("/skip");
            if !skipped {
                let cleaned = settings::clean_path(&path_text(&value)?);
                if !Path::new(&cleaned).exists() {
                    return Err(user_settings_error(format!(
                        "attempting to set a value to a path that does not exist: {cleaned}"
                    )));
                }
                value = PyString::new(py, &cleaned).into_any();
            }
        } else if key == "localCorpusSettings" {
            if !is_list_like(&value) {
                return Err(user_settings_error(
                    "localCorpusSettings must be provided as a list.",
                ));
            }
        } else if key == "localCorporaSettings" {
            let Ok(corpora) = value.cast::<PyDict>() else {
                return Err(user_settings_error(
                    "localCorporaSettings must be provided as a dict.",
                ));
            };
            for (name, paths) in corpora.iter() {
                if !name.is_instance_of::<PyString>() {
                    return Err(user_settings_error(
                        "Each key in localCorporaSettings must be a string.",
                    ));
                }
                if !is_list_like(&paths) {
                    return Err(user_settings_error(
                        "Each entry in localCorporaSettings must point to a list.",
                    ));
                }
            }
        }
        environment.set_item(key, value)?;
        environment.call_method0("write")?;
        Ok(())
    }

    /// Writes a settings file of the defaults, where there is none yet.
    fn create(&self, py: Python<'_>) -> PyResult<()> {
        let environment = self._environment.bind(py);
        let path = environment.call_method0("getSettingsPath")?;
        if path.call_method0("exists")?.is_truthy()? {
            return Err(user_settings_error(
                "An environment configuration file already exists; simply set values to modify.",
            ));
        }
        environment.call_method0("write")?;
        Ok(())
    }

    /// Removes the settings file.
    fn delete(&self, py: Python<'_>) -> PyResult<()> {
        let path = self._environment.bind(py).call_method0("getSettingsPath")?;
        if !path.call_method0("exists")?.is_truthy()? {
            return Err(user_settings_error(
                "An environment configuration file does not exist.",
            ));
        }
        path.call_method0("unlink")?;
        Ok(())
    }

    fn getSettingsPath<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self._environment.bind(py).call_method0("getSettingsPath")
    }

    /// The preferences' names, one at a time.
    fn keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let found = self._environment.bind(py).call_method0("getRefKeys")?;
        let found = found.cast_into::<PyList>()?;
        found.append("localCorpusPath")?;
        Ok(found.try_iter()?.into_any())
    }

    /// Each preference's name and value, one at a time.
    fn items<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let pairs = PyList::empty(py);
        for key in Self::keys(&slf.borrow(), py)?.try_iter()? {
            let key = key?;
            let value = slf.get_item(&key)?;
            pairs.append((key, value))?;
        }
        Ok(pairs.try_iter()?.into_any())
    }

    /// Each preference's value, one at a time.
    fn values<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let values = PyList::empty(py);
        for pair in Self::items(slf)?.try_iter()? {
            values.append(pair?.get_item(1)?)?;
        }
        Ok(values.try_iter()?.into_any())
    }

    /// Restores the platform's defaults and writes them out.
    fn restoreDefaults(&self, py: Python<'_>) -> PyResult<()> {
        let environment = self._environment.bind(py);
        environment.call_method0("restoreDefaults")?;
        environment.call_method0("write")?;
        Ok(())
    }
}

/// A list or a tuple: music21's `common.isListLike`.
fn is_list_like(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()
}

/// Every preference's name: music21's `environment.keys()`.
#[pyfunction]
fn keys(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let settings = Bound::new(py, UserSettings::new(py)?)?;
    UserSettings::keys(&settings.borrow(), py)
}

/// Sets one preference, making the settings file if need be.
#[pyfunction]
#[pyo3(name = "set")]
fn set_preference(py: Python<'_>, key: &str, value: Bound<'_, PyAny>) -> PyResult<()> {
    let settings = UserSettings::new(py)?;
    if let Err(error) = settings.create(py)
        && !error.is_instance_of::<UserSettingsException>(py)
    {
        return Err(error);
    }
    settings.__setitem__(py, key, value)
}

/// One preference's value.
#[pyfunction]
#[pyo3(name = "get")]
fn get_preference<'py>(py: Python<'py>, key: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    UserSettings::new(py)?.__getitem__(py, key)
}

/// The module music21 calls `environment`, as music21 has it.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = MODULE.get_or_try_init(py, || {
        let m = PyModule::new(py, "music21.environment")?;
        m.add_class::<EnvironmentCore>()?;
        m.add_class::<Environment>()?;
        m.add_class::<UserSettings>()?;
        m.add("LocalCorpusSettings", local_corpus_settings(py)?)?;
        for (name, exception) in [
            (
                "EnvironmentException",
                py.get_type::<EnvironmentException>(),
            ),
            (
                "UserSettingsException",
                py.get_type::<UserSettingsException>(),
            ),
        ] {
            exception.setattr("__module__", "music21.environment")?;
            m.add(name, exception)?;
        }
        m.add_function(wrap_pyfunction!(envSingleton, &m)?)?;
        m.add_function(wrap_pyfunction!(etIndent, &m)?)?;
        m.add_function(wrap_pyfunction!(keys, &m)?)?;
        m.add_function(wrap_pyfunction!(set_preference, &m)?)?;
        m.add_function(wrap_pyfunction!(get_preference, &m)?)?;
        m.add("_environStorage", storage(py)?)?;
        m.add("_MOD", "environment")?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
