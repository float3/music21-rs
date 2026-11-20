//! music21's `metadata.bundles`, over the crate's `metadata::bundles`.
//!
//! A `MetadataBundle` is the metadata of many scores, searched and combined
//! as sets; each corpus has one, cached on disk as the gzipped pickle
//! music21 writes. The crate reads and writes that pickle; this module
//! gzips it with Python's `gzip`, finds where it lives through the corpus,
//! and hands out music21's classes over the crate's values.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PySlice, PyString, PyTuple};

use music21::metadata::bundles::{
    KeySource, MetadataBundle as RsBundle, MetadataEntry as RsEntry, SetOperation,
    corpus_path_to_key,
};

use crate::richmetadata::{Query, RichMetadata, last_item, rich_value};

/// The module's dotted name.
const MODULE: &str = "music21.metadata.bundles";

pyo3::create_exception!(
    music21_rs_facade,
    MetadataBundleException,
    crate::Music21Exception
);

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let exception = py.get_type::<MetadataBundleException>();
        exception.setattr("__module__", MODULE)?;
        m.add("MetadataBundleException", exception)?;
        m.add_class::<MetadataEntry>()?;
        m.add_class::<MetadataBundle>()?;
        m.add_function(wrap_pyfunction!(demo_bundle, &m)?)?;
        m.add(
            "__all__",
            ["MetadataEntry", "MetadataBundle", "MetadataBundleException"],
        )?;
        m.add("environLocal", environment(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `environLocal` for this module.
fn environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    crate::environment::module(py)?
        .getattr("Environment")?
        .call1(("metadata.bundles",))
}

fn bundle_error(message: &str) -> PyErr {
    MetadataBundleException::new_err(message.to_string())
}

/// music21's `common.getSourceFilePath() / 'corpus'`: where a corpus path
/// is read relative to.
fn corpus_root(py: Python<'_>) -> Option<PathBuf> {
    crate::corpus::corpus_directory(py)
}

/// A path as a `pathlib.Path`.
fn path_object<'py>(py: Python<'py>, path: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import("pathlib")?.getattr("Path")?.call1((path,))
}

/// music21's `MetadataEntry`: one score of a bundle.
#[pyclass(name = "MetadataEntry", module = "music21.metadata.bundles", frozen)]
pub struct MetadataEntry {
    pub(crate) inner: Arc<RsEntry>,
}

impl MetadataEntry {
    /// The source path, under the corpus where it is relative.
    pub(crate) fn absolute_path(&self, py: Python<'_>) -> PyResult<PathBuf> {
        let path = PathBuf::from(&self.inner.source_path);
        if path.is_absolute() {
            return Ok(path);
        }
        Ok(PathBuf::from(crate::corpus::corpus_file_path(py)?.str()?.to_string()).join(path))
    }

    pub(crate) fn object(py: Python<'_>, inner: &Arc<RsEntry>) -> PyResult<Py<Self>> {
        Py::new(
            py,
            Self {
                inner: Arc::clone(inner),
            },
        )
    }
}

#[pymethods]
impl MetadataEntry {
    #[new]
    #[pyo3(signature = (sourcePath = None, number = None, metadataPayload = None, corpusName = ""))]
    fn new(
        sourcePath: Option<&Bound<'_, PyAny>>,
        number: Option<&Bound<'_, PyAny>>,
        metadataPayload: Option<&Bound<'_, PyAny>>,
        corpusName: &str,
    ) -> PyResult<Self> {
        let source_path = match sourcePath {
            Some(path) => path.str()?.to_string(),
            None => String::new(),
        };
        let number = match number.filter(|number| !number.is_none()) {
            Some(number) => Some(number.str()?.to_string()),
            None => None,
        };
        let metadata = match metadataPayload.filter(|payload| !payload.is_none()) {
            Some(payload) => Some(rich_value(payload)?),
            None => None,
        };
        let mut inner = RsEntry::new(source_path, number, metadata);
        inner.corpus_name = corpusName.to_string();
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let key = self.inner.corpus_path(corpus_root(py).as_deref());
        Ok(format!(
            "<music21.metadata.bundles.MetadataEntry {}>",
            PyString::new(py, &key).repr()?
        ))
    }

    /// The source path as a string, so an entry opens as a file does.
    fn __fspath__(&self) -> String {
        self.inner.source_path.clone()
    }

    /// The score, read from the corpus.
    fn parse<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let path = path_object(py, &self.inner.source_path)?;
        let number = match &self.inner.number {
            Some(number) => Some(PyString::new(py, number).into_any()),
            None => None,
        };
        crate::corpus::parse(py, &path, None, number.as_ref(), None, None, None)
    }

    /// Shows the score.
    #[pyo3(signature = (showFormat = None))]
    fn show(&self, py: Python<'_>, showFormat: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        self.parse(py)?.call_method1("show", (showFormat,))?;
        Ok(())
    }

    /// Searches the entry's metadata.
    #[pyo3(signature = (query = None, field = None, **keywords))]
    fn search<'py>(
        &self,
        py: Python<'py>,
        query: Option<&Bound<'py, PyAny>>,
        field: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.metadata(py)?
            .call_method("search", (query, field), keywords)
    }

    /// The key it is filed under.
    #[getter]
    fn corpusPath(&self, py: Python<'_>) -> String {
        self.inner.corpus_path(corpus_root(py).as_deref())
    }

    /// Its `RichMetadata`, or None for an opus's stand-in.
    #[getter]
    fn metadata<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.inner.metadata {
            Some(rich) => Ok(RichMetadata::object(py, rich)?.into_any()),
            None => Ok(py.None().into_bound(py)),
        }
    }

    #[getter]
    fn number(&self) -> Option<String> {
        self.inner.number.clone()
    }

    #[getter]
    fn sourcePath<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        path_object(py, &self.inner.source_path)
    }

    #[getter]
    fn corpusName(&self) -> String {
        self.inner.corpus_name.clone()
    }
}

/// What a bundle was made from: a corpus, a corpus's name, or nothing.
fn bundle_name(expr: Option<&Bound<'_, PyAny>>) -> PyResult<(Option<String>, Option<Py<PyAny>>)> {
    let Some(expr) = expr.filter(|expr| !expr.is_none()) else {
        return Ok((None, None));
    };
    if let Ok(name) = expr.extract::<String>() {
        return Ok((Some(name), None));
    }
    if expr.is_instance_of::<crate::corpora::Corpus>() {
        let name = expr.getattr("name")?.extract()?;
        return Ok((name, Some(expr.clone().unbind())));
    }
    Err(bundle_error(
        "Need to take a string, corpus, or None as expression",
    ))
}

/// music21's `MetadataBundle`.
#[pyclass(name = "MetadataBundle", module = "music21.metadata.bundles", subclass)]
pub struct MetadataBundle {
    pub(crate) inner: RsBundle,
    corpus: Option<Py<PyAny>>,
}

impl MetadataBundle {
    pub(crate) fn wrap(inner: RsBundle) -> Self {
        Self {
            inner,
            corpus: None,
        }
    }

    /// A new bundle of the same class as `slf` over `inner`.
    fn sibling<'py>(slf: &Bound<'py, Self>, inner: RsBundle) -> PyResult<Bound<'py, PyAny>> {
        let made = slf.get_type().call0()?;
        made.cast::<Self>()?.borrow_mut().inner = inner;
        Ok(made)
    }

    fn other<'py>(other: &Bound<'py, PyAny>) -> PyResult<PyRef<'py, Self>> {
        other
            .cast::<Self>()
            .map(Bound::borrow)
            .map_err(|_| bundle_error("metadataBundle must be a MetadataBundle"))
    }

    fn combine<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
        operation: SetOperation,
    ) -> PyResult<Bound<'py, PyAny>> {
        let combined = slf
            .borrow()
            .inner
            .combine(&Self::other(other)?.inner, operation);
        Self::sibling(slf, combined)
    }

    fn keys_compare(&self, other: &Bound<'_, PyAny>) -> PyResult<Option<Ordering>> {
        Ok(self.inner.compare_keys(&Self::other(other)?.inner))
    }

    /// The cache file of the bundle's corpus.
    fn cache_path<'py>(slf: &Bound<'py, Self>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let path = slf.getattr("filePath")?;
        if path.is_none() {
            return Ok(None);
        }
        Ok(Some(path))
    }
}

#[pymethods]
impl MetadataBundle {
    #[new]
    #[pyo3(signature = (expr = None))]
    pub(crate) fn new(expr: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let (name, corpus) = bundle_name(expr)?;
        Ok(Self {
            inner: RsBundle::new(name),
            corpus,
        })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// An entry by place, or a list of them by slice.
    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        i: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let entries = PyList::empty(py);
        if let Ok(slice) = i.cast::<PySlice>() {
            let indices = slice.indices(self.inner.len() as isize)?;
            let mut at = indices.start;
            for _ in 0..indices.slicelength {
                let (_, entry) = self.inner.get(at as usize).expect("a place in the slice");
                entries.append(MetadataEntry::object(py, entry)?)?;
                at += indices.step;
            }
            return Ok(entries.into_any());
        }
        let mut at: isize = i.extract()?;
        if at < 0 {
            at += self.inner.len() as isize;
        }
        match usize::try_from(at).ok().and_then(|at| self.inner.get(at)) {
            Some((_, entry)) => Ok(MetadataEntry::object(py, entry)?.into_bound(py).into_any()),
            None => Err(pyo3::exceptions::PyIndexError::new_err(
                "list index out of range",
            )),
        }
    }

    fn __repr__(&self) -> String {
        let count = self.inner.len();
        let status = if count == 1 {
            "{1 entry}".to_string()
        } else {
            format!("{{{count} entries}}")
        };
        match self.inner.name() {
            Some(name) => format!("<music21.metadata.bundles.MetadataBundle '{name}': {status}>"),
            None => format!("<music21.metadata.bundles.MetadataBundle {status}>"),
        }
    }

    /// The same entries in the same order.
    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        match other.cast::<Self>() {
            Ok(other) => self.inner == other.borrow().inner,
            Err(_) => false,
        }
    }

    fn __and__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, other, SetOperation::Intersection)
    }

    fn __or__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, other, SetOperation::Union)
    }

    fn __sub__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, other, SetOperation::Difference)
    }

    fn __xor__<'py>(
        slf: &Bound<'py, Self>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, other, SetOperation::SymmetricDifference)
    }

    fn __lt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.keys_compare(other)? == Some(Ordering::Less))
    }

    fn __le__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(matches!(
            self.keys_compare(other)?,
            Some(Ordering::Less | Ordering::Equal)
        ))
    }

    fn __gt__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.keys_compare(other)? == Some(Ordering::Greater))
    }

    fn __ge__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(matches!(
            self.keys_compare(other)?,
            Some(Ordering::Greater | Ordering::Equal)
        ))
    }

    fn difference<'py>(
        slf: &Bound<'py, Self>,
        metadataBundle: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, metadataBundle, SetOperation::Difference)
    }

    fn intersection<'py>(
        slf: &Bound<'py, Self>,
        metadataBundle: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, metadataBundle, SetOperation::Intersection)
    }

    fn symmetric_difference<'py>(
        slf: &Bound<'py, Self>,
        metadataBundle: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, metadataBundle, SetOperation::SymmetricDifference)
    }

    fn union<'py>(
        slf: &Bound<'py, Self>,
        metadataBundle: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::combine(slf, metadataBundle, SetOperation::Union)
    }

    fn isdisjoint(&self, metadataBundle: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(self.inner.is_disjoint(&Self::other(metadataBundle)?.inner))
    }

    fn issubset(&self, metadataBundle: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.__le__(metadataBundle)
    }

    fn issuperset(&self, metadataBundle: &Bound<'_, PyAny>) -> PyResult<bool> {
        self.__ge__(metadataBundle)
    }

    /// The entries by key, as music21 keeps them.
    #[getter]
    fn _metadataEntries<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let ordered = py.import("collections")?.getattr("OrderedDict")?.call0()?;
        for (key, entry) in self.inner.iter() {
            ordered.set_item(key, MetadataEntry::object(py, entry)?)?;
        }
        Ok(ordered)
    }

    /// The corpus the bundle belongs to, found by its name where it was
    /// not made from one.
    #[getter]
    fn get_corpus<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        if let Some(corpus) = &self.corpus {
            return Ok(corpus.bind(py).clone());
        }
        match self.inner.name() {
            Some(name) => crate::corpora::from_name(py, Some(name)),
            None => Ok(py.None().into_bound(py)),
        }
    }

    #[setter]
    fn set_corpus(&mut self, newCorpus: Option<Bound<'_, PyAny>>) {
        self.corpus = newCorpus
            .filter(|corpus| !corpus.is_none())
            .map(Bound::unbind);
    }

    /// Where the bundle's corpus caches it, or None for a bundle of no
    /// corpus.
    #[getter]
    fn filePath<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let corpus = slf.getattr("corpus")?;
        if corpus.is_none() {
            return Ok(py.None().into_bound(py));
        }
        let path = corpus.getattr("cacheFilePath")?;
        path_object(py, &path.str()?.to_string())
    }

    #[getter]
    fn name(&self) -> Option<String> {
        self.inner.name().map(str::to_string)
    }

    /// Parses each path and files its metadata, answering the paths that
    /// would not parse. A path already filed is read again only where it
    /// changed after the cache. The jobs run one after another.
    #[pyo3(signature = (paths, parseUsingCorpus = false, useMultiprocessing = true, storeOnDisk = true, verbose = false))]
    fn addFromPaths<'py>(
        slf: &Bound<'py, Self>,
        paths: &Bound<'py, PyAny>,
        parseUsingCorpus: bool,
        useMultiprocessing: bool,
        storeOnDisk: bool,
        verbose: bool,
    ) -> PyResult<Bound<'py, PyList>> {
        let _ = (useMultiprocessing, verbose);
        let py = slf.py();
        let cache_time = match Self::cache_path(slf)? {
            Some(path) if path.call_method0("exists")?.is_truthy()? => Some(
                path.call_method0("stat")?
                    .getattr("st_ctime")?
                    .extract::<f64>()?,
            ),
            _ => None,
        };
        let corpus_name = match slf.borrow().inner.name() {
            None => "core".to_string(),
            Some(name) => name.strip_prefix("local-").unwrap_or(name).to_string(),
        };

        // A job for each path not filed since it last changed.
        let errors = PyList::empty(py);
        let root = corpus_root(py);
        for path in paths.try_iter()? {
            let path = path?;
            let key = path_key(&path, root.as_deref())?;
            if slf.borrow().inner.entry(&key).is_some() {
                let changed = path
                    .call_method0("stat")?
                    .getattr("st_ctime")?
                    .extract::<f64>()?;
                if cache_time.is_some_and(|cached| changed < cached) {
                    continue;
                }
            }
            let job =
                crate::caching::Job::new(py, &path, parseUsingCorpus, Some(corpus_name.clone()))?;
            let (results, failed) = job.run(py)?;
            for entry in results {
                let key = entry.corpus_path(root.as_deref());
                slf.borrow_mut().inner.insert_shared(key, Arc::new(entry));
            }
            for path in failed {
                errors.append(path)?;
            }
        }
        Self::validate(slf)?;
        if storeOnDisk {
            Self::write(slf, None)?;
        }
        Ok(errors)
    }

    /// Removes every entry.
    fn clear(&mut self) {
        self.inner.clear();
    }

    /// The key a file path or corpus path is filed under.
    #[staticmethod]
    #[pyo3(signature = (filePath, number = None))]
    fn corpusPathToKey(
        py: Python<'_>,
        filePath: &Bound<'_, PyAny>,
        number: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let number = match number.filter(|number| !number.is_none()) {
            Some(number) => Some(number.str()?.to_string()),
            None => None,
        };
        let key = path_key(filePath, corpus_root(py).as_deref())?;
        Ok(match number {
            Some(number) => format!("{key}_{number}"),
            None => key,
        })
    }

    /// Deletes the cache file of the bundle's corpus, keeping the bundle.
    fn delete(slf: &Bound<'_, Self>) -> PyResult<()> {
        if let Some(path) = Self::cache_path(slf)?
            && path.call_method0("exists")?.is_truthy()?
        {
            path.call_method0("unlink")?;
        }
        Ok(())
    }

    /// Every field a bundle can be searched by.
    #[staticmethod]
    fn listSearchFields(py: Python<'_>) -> PyResult<Bound<'_, PyTuple>> {
        let mut fields: Vec<String> = music21::metadata::property_descriptions()
            .flat_map(|description| {
                let mut names = vec![description.unique_name.to_string()];
                names.extend(description.old_work_id.map(str::to_string));
                names
            })
            .collect();
        fields.extend(
            music21::metadata::rich::RICH_ATTRIBUTES
                .iter()
                .map(|name| (*name).to_string()),
        );
        fields.sort();
        PyTuple::new(py, fields)
    }

    /// Reads the cache of the bundle's corpus, or of `filePath`; nothing
    /// changes where there is no such file.
    #[pyo3(signature = (filePath = None))]
    pub(crate) fn read<'py>(
        slf: &Bound<'py, Self>,
        filePath: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Self>> {
        let py = slf.py();
        let path = match filePath.filter(|path| !path.is_none()) {
            Some(path) => Some(path.clone()),
            None => Self::cache_path(slf)?,
        };
        let Some(path) = path else {
            return Err(crate::metadata::MetadataException::new_err(
                "Unnamed MetadataBundles have no default file path to read from.",
            ));
        };
        let path = path_object(py, &path.str()?.to_string())?;
        if !path.call_method0("exists")?.is_truthy()? {
            return Ok(slf.clone());
        }
        let bytes: Vec<u8> = py
            .import("gzip")?
            .call_method1("open", (&path, "rb"))?
            .call_method0("read")?
            .extract()?;
        let read =
            RsBundle::from_cache(&bytes).map_err(|error| bundle_error(&error.to_string()))?;

        // The entries alone: the bundle keeps its own name.
        let mut me = slf.borrow_mut();
        me.inner.clear();
        for (key, entry) in read.iter() {
            me.inner.insert_shared(key.to_string(), Arc::clone(entry));
        }
        Ok(slf.clone())
    }

    /// The entries a query matches, sorted by source path: music21's
    /// `search`. Further keywords narrow the search one after another.
    #[pyo3(signature = (query = None, field = None, *, fileExtensions = None, **keywords))]
    fn search<'py>(
        slf: &Bound<'py, Self>,
        query: Option<&Bound<'py, PyAny>>,
        field: Option<String>,
        fileExtensions: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let extensions = match fileExtensions.filter(|given| given.is_truthy().unwrap_or(false)) {
            Some(given) => crate::corpus::extensions(Some(given))?,
            None => Vec::new(),
        };
        let keywords = match keywords {
            Some(keywords) => keywords.copy()?,
            None => PyDict::new(py),
        };
        let (query, field) = match (query.filter(|query| !query.is_none()), field) {
            (Some(query), field) => (query.clone(), field),
            (None, Some(field)) => (py.None().into_bound(py), Some(field)),
            (None, None) => {
                let Some((key, value)) = last_item(&keywords) else {
                    return Err(bundle_error("Query cannot be empty"));
                };
                keywords.del_item(&key)?;
                (value, Some(key))
            }
        };
        let asked = Query::read(&query)?;
        let found = asked.run(|asked| {
            slf.borrow()
                .inner
                .search(asked, field.as_deref(), &extensions)
        })?;
        let found = Bound::new(py, Self::wrap(found))?;
        if keywords.is_empty() {
            return Ok(found.into_any());
        }
        Self::search(&found, None, None, None, Some(&keywords))
    }

    /// Drops the entries whose score is not on disk, answering how many.
    pub(crate) fn validate(slf: &Bound<'_, Self>) -> PyResult<usize> {
        let root = corpus_root(slf.py());
        let exists = |path: &str| {
            let path = Path::new(path);
            match (&root, path.is_absolute()) {
                (Some(root), false) => root.join(path).exists(),
                _ => path.exists(),
            }
        };
        Ok(slf.borrow_mut().inner.validate(exists))
    }

    /// Writes the bundle as music21's gzipped pickle to the cache of its
    /// corpus. As in music21, a bundle of no corpus is not written, even
    /// with a `filePath`.
    #[pyo3(signature = (filePath = None))]
    fn write<'py>(
        slf: &Bound<'py, Self>,
        filePath: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, Self>> {
        let _ = filePath;
        let py = slf.py();
        let Some(path) = Self::cache_path(slf)? else {
            return Ok(slf.clone());
        };
        let bytes = pyo3::types::PyBytes::new(py, &slf.borrow().inner.to_cache());
        let out = py.import("gzip")?.call_method1("open", (path, "wb"))?;
        out.call_method1("write", (bytes,))?;
        out.call_method0("close")?;
        Ok(slf.clone())
    }

    /// Bundles are compared as sets of keys, not hashed.
    #[classattr]
    const __hash__: Option<Py<PyAny>> = None;
}

/// The key a path is filed under, a `pathlib.Path` read by its parts and a
/// string by its text, as music21 reads them.
fn path_key(path: &Bound<'_, PyAny>, root: Option<&Path>) -> PyResult<String> {
    let py = path.py();
    if path.is_instance(&py.import("pathlib")?.getattr("PurePath")?)? {
        let text = PathBuf::from(path.str()?.to_string());
        let source = KeySource::Path {
            path: &text,
            corpus: root,
        };
        return Ok(corpus_path_to_key(source, None));
    }
    match path.extract::<String>() {
        Ok(text) => Ok(corpus_path_to_key(KeySource::Text(&text), None)),
        Err(_) => Err(PyTypeError::new_err(
            "a path must be a string or a pathlib.Path",
        )),
    }
}

/// The bundles music21's examples share, made once: the core corpus's,
/// and its searches for `bach`, `corelli` (both as composer) and `3/4`.
#[pyfunction]
fn demo_bundle<'py>(py: Python<'py>, which: &str) -> PyResult<Bound<'py, PyAny>> {
    static MADE: PyOnceLock<Py<PyDict>> = PyOnceLock::new();
    let made = MADE
        .get_or_try_init(py, || Ok::<_, PyErr>(PyDict::new(py).unbind()))?
        .bind(py);
    if let Some(found) = made.get_item(which)? {
        return Ok(found);
    }
    let core = || demo_bundle(py, "core");
    let found = match which {
        "core" => crate::corpora::from_name(py, Some("core"))?.getattr("metadataBundle")?,
        "bach" => core()?.call_method1("search", ("bach", "composer"))?,
        "corelli" => core()?.call_method1("search", ("corelli", "composer"))?,
        "3/4" => core()?.call_method1("search", ("3/4",))?,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "no demo bundle called {}",
                PyString::new(py, which).repr()?
            )));
        }
    };
    made.set_item(which, &found)?;
    Ok(found)
}
