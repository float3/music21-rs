//! music21's `corpus.corpora` and `corpus.manager`: the core corpus and
//! local corpora, each with its metadata bundle.
//!
//! The core corpus is music21's own scores, found where the wheel's
//! `corpus` finds them; its metadata bundle is the cache music21 ships
//! beside them. A local corpus is a list of directories in music21's user
//! settings, cached in the scratch directory. `corpus.manager` keeps each
//! corpus's bundle once read, so every `metadataBundle` of one corpus is
//! the same object, as in music21.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};

use crate::bundles::MetadataBundle;

/// The two modules' dotted names.
const CORPORA: &str = "music21.corpus.corpora";
const MANAGER: &str = "music21.corpus.manager";

/// The core corpus's name.
const CORE: &str = "core";
/// The unnamed local corpus's name.
const LOCAL: &str = "local";
/// Where the core corpus keeps its metadata cache, under its directory.
const CACHE_DIRECTORY: &str = "_metadataCache";

/// music21's `corpus.corpora`, built once.
pub(crate) fn corpora_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, CORPORA)?;
        m.add_class::<Corpus>()?;
        m.add_class::<CoreCorpus>()?;
        m.add_class::<LocalCorpus>()?;
        m.add("CorpusException", crate::corpus::CORPUS_EXCEPTION.get(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `corpus.manager`, built once.
pub(crate) fn manager_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MANAGER)?;
        m.add_function(wrap_pyfunction!(from_name_py, &m)?)?;
        m.add_function(wrap_pyfunction!(bundle_by_corpus, &m)?)?;
        m.add_function(wrap_pyfunction!(cache_bundle_from_disk, &m)?)?;
        m.add_function(wrap_pyfunction!(read_all_bundles, &m)?)?;
        m.add_function(wrap_pyfunction!(iterate_corpora, &m)?)?;
        m.add_function(wrap_pyfunction!(list_local_names, &m)?)?;
        m.add_function(wrap_pyfunction!(list_search_fields, &m)?)?;
        m.add_function(wrap_pyfunction!(search, &m)?)?;
        m.add("CorpusException", crate::corpus::CORPUS_EXCEPTION.get(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

fn corpus_error(py: Python<'_>, message: String) -> PyErr {
    crate::corpus::CORPUS_EXCEPTION.err(py, message)
}

/// Each corpus's bundle, once read.
fn bundles(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    static HELD: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
    let held = HELD.get_or_try_init(py, || Ok::<_, PyErr>(PyDict::new(py).unbind()))?;
    Ok(held.bind(py).clone())
}

/// The corpus a name names: the core corpus, the unnamed local corpus for
/// `local` or none, or a named local corpus.
pub(crate) fn from_name<'py>(py: Python<'py>, name: Option<&str>) -> PyResult<Bound<'py, PyAny>> {
    if name == Some(CORE) {
        let made = PyClassInitializer::from(Corpus).add_subclass(CoreCorpus);
        return Ok(Bound::new(py, made)?.into_any());
    }
    let name = name.filter(|name| *name != LOCAL).map(str::to_string);
    let made = PyClassInitializer::from(Corpus).add_subclass(LocalCorpus { name });
    Ok(Bound::new(py, made)?.into_any())
}

/// The names of the local corpora in the user's settings.
pub(crate) fn local_names(py: Python<'_>) -> PyResult<Vec<String>> {
    let settings = match user_setting(py, "localCorporaSettings") {
        Some(settings) => settings,
        None => return Ok(Vec::new()),
    };
    settings.try_iter()?.map(|name| name?.extract()).collect()
}

/// One of music21's user settings, or nothing where it cannot be read.
fn user_setting<'py>(py: Python<'py>, key: &str) -> Option<Bound<'py, PyAny>> {
    crate::environment::module(py)
        .and_then(|module| module.getattr("UserSettings")?.call0())
        .and_then(|settings| settings.get_item(key))
        .ok()
        .filter(|found| !found.is_none())
}

/// A list of paths as `pathlib.Path`s.
fn path_list<'py>(py: Python<'py>, paths: &[PathBuf]) -> PyResult<Bound<'py, PyList>> {
    let class = py.import("pathlib")?.getattr("Path")?;
    let made = paths
        .iter()
        .map(|path| class.call1((path.as_os_str(),)))
        .collect::<PyResult<Vec<_>>>()?;
    PyList::new(py, made)
}

/// music21's `Corpus`: what every corpus shares.
#[pyclass(name = "Corpus", module = "music21.corpus.corpora", subclass)]
pub struct Corpus;

#[pymethods]
impl Corpus {
    /// Whether a corpus's works are parsed through the corpus.
    #[classattr]
    fn parseUsingCorpus() -> bool {
        true
    }

    /// The extensions of the formats asked for, every format's where none
    /// is, widened to each format's every extension unless
    /// `expandExtensions` is false.
    #[staticmethod]
    #[pyo3(signature = (fileExtensions = None, *, expandExtensions = true))]
    fn translateExtensions<'py>(
        py: Python<'py>,
        fileExtensions: Option<&Bound<'py, PyAny>>,
        expandExtensions: bool,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let given = fileExtensions.filter(|given| given.is_truthy().unwrap_or(false));
        let extensions = match given {
            Some(given) if !expandExtensions => given.extract::<Vec<String>>()?,
            other => crate::corpus::extensions(other)?,
        };
        PyTuple::new(py, extensions)
    }

    /// The paths of the works matching a name and, where asked, a movement.
    #[pyo3(signature = (workName, movementNumber = None, *, fileExtensions = None))]
    fn getWorkList<'py>(
        slf: &Bound<'py, Self>,
        workName: &Bound<'py, PyAny>,
        movementNumber: Option<&Bound<'py, PyAny>>,
        fileExtensions: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let kwargs = PyDict::new(py);
        kwargs.set_item("fileExtensions", fileExtensions)?;
        let listed: Vec<String> = slf
            .call_method("getPaths", (), Some(&kwargs))?
            .try_iter()?
            .map(|path| Ok(path?.str()?.to_string()))
            .collect::<PyResult<_>>()?;
        let paths = listed.into_iter().map(PathBuf::from).collect();
        let movement = movementNumber.filter(|movement| !movement.is_none());
        let found = crate::corpus::work_list_in(paths, workName, movement)?;
        path_list(py, &found)
    }

    /// The entries of the corpus's bundle a query matches.
    #[pyo3(signature = (query = None, field = None, *, fileExtensions = None, **keywords))]
    fn search<'py>(
        slf: &Bound<'py, Self>,
        query: Option<&Bound<'py, PyAny>>,
        field: Option<&Bound<'py, PyAny>>,
        fileExtensions: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let kwargs = match keywords {
            Some(keywords) => keywords.copy()?,
            None => PyDict::new(py),
        };
        kwargs.set_item("field", field)?;
        kwargs.set_item("fileExtensions", fileExtensions)?;
        slf.getattr("metadataBundle")?
            .call_method("search", (query,), Some(&kwargs))
    }

    /// The paths of the corpus's works that belong to a composer or a
    /// collection, sorted.
    #[pyo3(signature = (composerName, *, fileExtensions = None))]
    fn getComposer<'py>(
        slf: &Bound<'py, Self>,
        composerName: &str,
        fileExtensions: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        crate::corpuswork::composer_paths(slf.as_any(), composerName, fileExtensions)
    }

    /// Each of the corpus's directories with its works: the core corpus's
    /// directories, and none for a local corpus.
    #[getter]
    fn directoryInformation<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyTuple>> {
        if slf.is_instance_of::<CoreCorpus>() {
            return crate::corpuswork::core_directories(slf.as_any());
        }
        Ok(PyTuple::empty(slf.py()))
    }

    /// The directories with their works, as a list.
    fn getWorkReferences<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(slf.py(), Self::directoryInformation(slf)?)
    }

    /// The corpus's metadata bundle, read once and kept.
    #[getter]
    fn metadataBundle<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let bundle = bundle_by_corpus(slf.as_any())?;
        bundle.setattr("corpus", slf)?;
        Ok(bundle)
    }

    /// The corpus's metadata bundle.
    fn all<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        Self::metadataBundle(slf)
    }

    /// Parses every work of the corpus not cached since it changed into
    /// its bundle and writes it, answering the paths that would not parse.
    #[pyo3(signature = (useMultiprocessing = true, verbose = true, timer = None))]
    fn cacheMetadata<'py>(
        slf: &Bound<'py, Self>,
        useMultiprocessing: bool,
        verbose: bool,
        timer: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = timer;
        let py = slf.py();
        let bundle = MetadataBundle::new(Some(slf.as_any()))?;
        let bundle = Bound::new(py, bundle)?;
        MetadataBundle::read(&bundle, None)?;
        let paths = slf.call_method0("getPaths")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("parseUsingCorpus", slf.getattr("parseUsingCorpus")?)?;
        kwargs.set_item("useMultiprocessing", useMultiprocessing)?;
        kwargs.set_item("verbose", verbose)?;
        let failed = bundle.call_method("addFromPaths", (paths,), Some(&kwargs))?;
        bundles(py)?.set_item(slf.getattr("name")?, &bundle)?;
        Ok(failed)
    }

    /// Deletes the corpus's cache and builds it again.
    #[pyo3(signature = (useMultiprocessing = true, verbose = true))]
    fn rebuildMetadataCache<'py>(
        slf: &Bound<'py, Self>,
        useMultiprocessing: bool,
        verbose: bool,
    ) -> PyResult<Bound<'py, PyAny>> {
        let path = slf.getattr("cacheFilePath")?;
        if path.call_method0("exists")?.is_truthy()? {
            path.call_method0("unlink")?;
        }
        bundles(slf.py())?.del_item(slf.getattr("name")?).ok();
        Self::cacheMetadata(slf, useMultiprocessing, verbose, None)
    }
}

/// music21's `CoreCorpus`: the scores music21 ships.
#[pyclass(name = "CoreCorpus", module = "music21.corpus.corpora", extends = Corpus)]
pub struct CoreCorpus;

#[pymethods]
impl CoreCorpus {
    #[new]
    fn new() -> PyClassInitializer<Self> {
        PyClassInitializer::from(Corpus).add_subclass(Self)
    }

    fn __repr__(&self) -> &'static str {
        "<music21.corpus.corpora.CoreCorpus>"
    }

    #[getter]
    fn name(&self) -> &'static str {
        CORE
    }

    /// The metadata cache music21 ships beside the scores.
    #[getter]
    fn cacheFilePath<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::corpus::corpus_file_path(py)?
            .div(CACHE_DIRECTORY)?
            .div("core.p.gz")
    }

    /// The corpus's works in the formats asked for, sorted.
    #[pyo3(signature = (fileExtensions = None, *, expandExtensions = true))]
    fn getPaths<'py>(
        &self,
        py: Python<'py>,
        fileExtensions: Option<&Bound<'py, PyAny>>,
        expandExtensions: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let _ = expandExtensions;
        let (_, listed) = crate::corpus::listing(py, fileExtensions)?;
        Ok(listed)
    }

    /// The directory the user set as the core corpus, or None.
    #[getter]
    fn get_manualCoreCorpusPath<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        user_setting(py, "manualCoreCorpusPath").unwrap_or_else(|| py.None().into_bound(py))
    }

    /// Sets the directory of the core corpus in the user's settings, or
    /// unsets it with None.
    #[setter]
    fn set_manualCoreCorpusPath(
        &self,
        py: Python<'_>,
        expr: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let settings = crate::environment::module(py)?
            .getattr("UserSettings")?
            .call0()?;
        let Some(expr) = expr.filter(|expr| !expr.is_none()) else {
            settings.set_item("manualCoreCorpusPath", py.None())?;
            return Ok(());
        };
        let path = py
            .import("pathlib")?
            .getattr("Path")?
            .call1((expr,))?
            .call_method0("expanduser")?;
        if !path.call_method0("is_dir")?.is_truthy()? {
            return Err(corpus_error(
                py,
                "path needs to be a path to an existing directory".to_string(),
            ));
        }
        settings.set_item("manualCoreCorpusPath", path)?;
        Ok(())
    }

    /// Whether there is no corpus to read.
    #[getter]
    fn noCorpus(&self, py: Python<'_>) -> bool {
        crate::corpus::corpus_directory(py).is_none()
    }
}

/// music21's `LocalCorpus`: directories of the user's own, by name.
#[pyclass(name = "LocalCorpus", module = "music21.corpus.corpora", extends = Corpus)]
pub struct LocalCorpus {
    name: Option<String>,
}

impl LocalCorpus {
    fn settings<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        match &self.name {
            None => user_setting(py, "localCorpusSettings"),
            Some(name) => user_setting(py, "localCorporaSettings")?
                .call_method1("get", (name,))
                .ok()
                .filter(|found| !found.is_none()),
        }
    }

    fn directories(&self, py: Python<'_>) -> PyResult<Vec<PathBuf>> {
        let Some(settings) = self.settings(py) else {
            return Ok(Vec::new());
        };
        let mut found: Vec<PathBuf> = settings
            .try_iter()?
            .map(|path| Ok(PathBuf::from(path?.str()?.to_string())))
            .collect::<PyResult<_>>()?;
        found.sort();
        found.dedup();
        Ok(found)
    }
}

#[pymethods]
impl LocalCorpus {
    #[new]
    #[pyo3(signature = (name = None))]
    fn new(py: Python<'_>, name: Option<&Bound<'_, PyAny>>) -> PyResult<PyClassInitializer<Self>> {
        let name = match name.filter(|name| !name.is_none()) {
            None => None,
            Some(name) => {
                let Ok(name) = name.extract::<String>() else {
                    return Err(corpus_error(
                        py,
                        "Name must be a string or None".to_string(),
                    ));
                };
                if name.is_empty() {
                    return Err(corpus_error(py, "Name cannot be blank".to_string()));
                }
                if name == CORE || name == "virtual" {
                    return Err(corpus_error(py, format!("The name '{name}' is reserved.")));
                }
                Some(name).filter(|name| name != LOCAL)
            }
        };
        Ok(PyClassInitializer::from(Corpus).add_subclass(Self { name }))
    }

    #[classattr]
    fn parseUsingCorpus() -> bool {
        false
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let name = PyString::new(py, self.name.as_deref().unwrap_or(LOCAL));
        Ok(format!(
            "<music21.corpus.corpora.LocalCorpus: {}>",
            name.repr()?
        ))
    }

    /// `local` for the unnamed corpus, else its name.
    #[getter]
    fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| LOCAL.to_string())
    }

    /// The cache file its settings name, else one in the scratch
    /// directory: `local.p.gz` or `local-<name>.p.gz`.
    #[getter]
    fn cacheFilePath<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if let Some(settings) = self.settings(py)
            && let Ok(path) = settings.getattr("cacheFilePath")
            && !path.is_none()
        {
            return Ok(path);
        }
        let file = match &self.name {
            None => format!("{LOCAL}.p.gz"),
            Some(name) => format!("{LOCAL}-{name}.p.gz"),
        };
        crate::environment::module(py)?
            .getattr("Environment")?
            .call0()?
            .call_method0("getRootTempDir")?
            .div(file)
    }

    /// The directories in its settings.
    #[getter]
    fn directoryPaths<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let found = self.directories(py)?;
        PyTuple::new(py, path_list(py, &found)?)
    }

    /// Whether its settings have it.
    #[getter]
    fn existsInSettings(&self, py: Python<'_>) -> bool {
        self.name.is_none() || self.settings(py).is_some()
    }

    /// The works in its directories in the formats asked for.
    #[pyo3(signature = (*, fileExtensions = None, expandExtensions = true))]
    fn getPaths<'py>(
        &self,
        py: Python<'py>,
        fileExtensions: Option<&Bound<'py, PyAny>>,
        expandExtensions: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let extensions = Corpus::translateExtensions(py, fileExtensions, expandExtensions)?
            .extract::<Vec<String>>()?;
        let mut found = Vec::new();
        for directory in self.directories(py)? {
            let mut paths = Vec::new();
            crate::corpus::walk(Path::new(&directory), &mut paths);
            paths.retain(|path| crate::corpus::is_work(path, &extensions));
            paths.sort();
            found.extend(paths);
        }
        path_list(py, &found)
    }
}

#[pyfunction]
#[pyo3(name = "fromName")]
fn from_name_py<'py>(py: Python<'py>, name: Option<&str>) -> PyResult<Bound<'py, PyAny>> {
    from_name(py, name)
}

/// The bundle of a corpus, read from its cache once and kept: music21's
/// `getMetadataBundleByCorpus`.
#[pyfunction]
#[pyo3(name = "getMetadataBundleByCorpus")]
fn bundle_by_corpus<'py>(corpusObject: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = corpusObject.py();
    cache_bundle_from_disk(corpusObject)?;
    let name = corpusObject.getattr("name")?;
    match bundles(py)?.get_item(&name)? {
        Some(bundle) => Ok(bundle),
        None => Err(corpus_error(
            py,
            format!("No metadata bundle found for corpus {}", name.str()?),
        )),
    }
}

/// Reads a corpus's bundle from its cache, unless already read.
#[pyfunction]
#[pyo3(name = "cacheMetadataBundleFromDisk")]
fn cache_bundle_from_disk(corpusObject: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = corpusObject.py();
    let name = corpusObject.getattr("name")?;
    if bundles(py)?.contains(&name)? {
        return Ok(());
    }
    let bundle = Bound::new(py, MetadataBundle::new(Some(&name))?)?;
    MetadataBundle::read(&bundle, None)?;
    MetadataBundle::validate(&bundle)?;
    bundles(py)?.set_item(name, bundle)?;
    Ok(())
}

/// Every corpus: the core corpus, the unnamed local corpus and each named
/// one.
#[pyfunction]
#[pyo3(name = "iterateCorpora", signature = (returnObjects = true))]
fn iterate_corpora<'py>(py: Python<'py>, returnObjects: bool) -> PyResult<Bound<'py, PyList>> {
    let mut names = vec![CORE.to_string(), LOCAL.to_string()];
    names.extend(local_names(py)?);
    let found = PyList::empty(py);
    for name in names {
        if returnObjects {
            found.append(from_name(py, Some(&name))?)?;
        } else {
            found.append(name)?;
        }
    }
    Ok(found)
}

/// Reads every corpus's bundle.
#[pyfunction]
#[pyo3(name = "readAllMetadataBundlesFromDisk")]
fn read_all_bundles(py: Python<'_>) -> PyResult<()> {
    for corpus in iterate_corpora(py, true)?.iter() {
        cache_bundle_from_disk(&corpus)?;
    }
    Ok(())
}

/// The local corpora's names, None for the unnamed one first unless
/// `skipNone`.
#[pyfunction]
#[pyo3(name = "listLocalCorporaNames", signature = (skipNone = false))]
fn list_local_names(py: Python<'_>, skipNone: bool) -> PyResult<Bound<'_, PyList>> {
    let found = PyList::empty(py);
    if !skipNone {
        found.append(py.None())?;
    }
    for name in local_names(py)? {
        found.append(name)?;
    }
    Ok(found)
}

/// Every field a corpus can be searched by.
#[pyfunction]
#[pyo3(name = "listSearchFields")]
fn list_search_fields(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    py.get_type::<MetadataBundle>()
        .call_method0("listSearchFields")
}

/// Searches every corpus named, all of them where none is, and joins what
/// they find.
#[pyfunction]
#[pyo3(signature = (query = None, field = None, *, corpusNames = None, fileExtensions = None, **keywords))]
pub(crate) fn search<'py>(
    py: Python<'py>,
    query: Option<&Bound<'py, PyAny>>,
    field: Option<&Bound<'py, PyAny>>,
    corpusNames: Option<&Bound<'py, PyAny>>,
    fileExtensions: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let names: Vec<Option<String>> = match corpusNames.filter(|names| !names.is_none()) {
        Some(names) => names.extract()?,
        None => {
            let mut names = vec![Some(CORE.to_string()), Some(LOCAL.to_string())];
            names.extend(local_names(py)?.into_iter().map(Some));
            names
        }
    };
    let mut joined = Bound::new(py, MetadataBundle::new(None)?)?.into_any();
    for name in names {
        let corpus = from_name(py, name.as_deref())?;
        let kwargs = match keywords {
            Some(keywords) => keywords.copy()?,
            None => PyDict::new(py),
        };
        kwargs.set_item("field", field)?;
        kwargs.set_item("fileExtensions", fileExtensions)?;
        let found = corpus.call_method("search", (query,), Some(&kwargs))?;
        joined = joined.call_method1("union", (found,))?;
    }
    Ok(joined)
}
