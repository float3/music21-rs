//! music21's `common.pathTools`: where music21's files are, and paths made
//! absolute and tidy.
//!
//! The paths are worked out with Python's own `os.path`, which is the
//! platform's: Windows and POSIX disagree on what a path is, and music21's
//! answer is whichever its platform gives.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::{Path, PathBuf};

use pyo3::exceptions::PyFileNotFoundError;
use pyo3::prelude::*;
use pyo3::types::PyList;

use music21::common::pathtools::last_parent_dir;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.pathTools";

/// The variable naming a directory laid out as music21's corpus, which the
/// package's `corpus` reads; its parent is music21's source directory.
const CORPUS_VARIABLE: &str = "MUSIC21_RS_CORPUS";

/// Whether a directory is music21's source: it holds `stream`.
fn is_source(directory: &Path) -> bool {
    directory.join("stream").exists()
}

/// The directory of music21's source files, where `note.py` and the rest
/// are: an installed music21's package, else the parent of the corpus
/// `MUSIC21_RS_CORPUS` names. Raises `FileNotFoundError` where neither is
/// there.
#[pyfunction]
pub(crate) fn getSourceFilePath(py: Python<'_>) -> PyResult<PathBuf> {
    let mut looked = Vec::new();
    let spec = py
        .import("importlib.util")?
        .call_method1("find_spec", ("music21",))
        .ok()
        .filter(|spec| !spec.is_none());
    if let Some(locations) = spec.and_then(|spec| spec.getattr("submodule_search_locations").ok())
        && !locations.is_none()
    {
        for location in locations.try_iter()? {
            let location: PathBuf = location?.extract()?;
            if is_source(&location) {
                return Ok(location);
            }
            looked.push(location);
        }
    }
    let named = py
        .import("os")?
        .getattr("environ")?
        .call_method1("get", (CORPUS_VARIABLE,))?;
    if !named.is_none() {
        let corpus: PathBuf = named.extract()?;
        if let Some(parent) = corpus.parent() {
            if is_source(parent) {
                return Ok(parent.to_path_buf());
            }
            looked.push(parent.to_path_buf());
        }
    }
    let looked = looked
        .first()
        .map_or_else(String::new, |path| path.display().to_string());
    Err(PyFileNotFoundError::new_err(format!(
        "cannot find expected music21 directory: {looked}"
    )))
}

/// music21's corpus metadata cache: `corpus/_metadataCache` in its source.
#[pyfunction]
pub(crate) fn getMetadataCacheFilePath(py: Python<'_>) -> PyResult<PathBuf> {
    Ok(getSourceFilePath(py)?.join("corpus").join("_metadataCache"))
}

/// The directory of music21's core corpus: the one it was told to use, else
/// `corpus` in its source.
#[pyfunction]
pub(crate) fn getCorpusFilePath(py: Python<'_>) -> PyResult<PathBuf> {
    let corpus = py.import("music21.corpus")?;
    if let Ok(corpora) = corpus.getattr("corpora") {
        let manual = corpora
            .getattr("CoreCorpus")?
            .call0()?
            .getattr("manualCoreCorpusPath")?;
        if manual.is_none() {
            return Ok(getSourceFilePath(py)?.join("corpus"));
        }
        return manual.extract();
    }
    // The package's own `corpus`, which reads music21's from where it is.
    if let Ok(found) = corpus.call_method0("getCorpusFilePath") {
        return found.extract();
    }
    Ok(getSourceFilePath(py)?.join("corpus"))
}

/// The corpus directories that hold music, sorted: everything in it but
/// Python files, hidden files, the licence and the caches.
#[pyfunction]
pub(crate) fn getCorpusContentDirs(py: Python<'_>) -> PyResult<Vec<String>> {
    let directory = getCorpusFilePath(py)?;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&directory)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let excluded = name.ends_with(".py")
            || name.ends_with(".pyc")
            || name.starts_with('.')
            || ["license.txt", "_metadataCache", "__pycache__"].contains(&name.as_str());
        if !excluded {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// The directory music21's source directory is in, which holds its
/// documentation and its distribution files beside it.
#[pyfunction]
pub(crate) fn getRootFilePath(py: Python<'_>) -> PyResult<PathBuf> {
    let source = getSourceFilePath(py)?;
    Ok(source.parent().map_or(source.clone(), Path::to_path_buf))
}

/// `os.path.relpath(path, start)`. music21 meant to hand Windows paths back
/// as they are, but its test never matches, so every platform gets the
/// relative path.
#[pyfunction]
#[pyo3(signature = (path, start = None))]
pub(crate) fn relativepath<'py>(
    path: &Bound<'py, PyAny>,
    start: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = path.py();
    py.import("os")?
        .getattr("path")?
        .call_method1("relpath", (path, start))
}

/// A path made absolute and tidy: `~` expanded, `..` and repeated
/// separators collapsed, made absolute against the working directory, and
/// environment variables expanded last. A `..` is collapsed only where the
/// folders up to the last one exist, as the system would need them to.
///
/// A `pathlib.Path` comes back as one and text as text, unless
/// `returnPathlib` says which.
#[pyfunction]
#[pyo3(signature = (path, *, returnPathlib = None))]
pub(crate) fn cleanpath<'py>(
    path: &Bound<'py, PyAny>,
    returnPathlib: Option<bool>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = path.py();
    let pathlib = py.import("pathlib")?;
    let os = py.import("os")?;
    let os_path = os.getattr("path")?;
    let mut path = path.clone();
    let mut as_pathlib = returnPathlib;
    if path.is_instance(&pathlib.getattr("Path")?)? {
        path = path.str()?.into_any();
        as_pathlib = as_pathlib.or(Some(true));
    }
    let as_pathlib = as_pathlib.unwrap_or(false);

    path = os_path.call_method1("expanduser", (path,))?;
    let parts: Vec<String> = pathlib
        .getattr("PurePath")?
        .call1((&path,))?
        .getattr("parts")?
        .extract()?;
    let normalizable = match last_parent_dir(&parts) {
        Some(last) => os_path
            .call_method1(
                "isdir",
                (os_path
                    .getattr("join")?
                    .call1(pyo3::types::PyTuple::new(py, &parts[..=last])?)?,),
            )?
            .is_truthy()?,
        None => true,
    };
    if normalizable {
        path = os_path.call_method1("normpath", (path,))?;
        if !os_path.call_method1("isabs", (&path,))?.is_truthy()? {
            path = os_path.call_method1("abspath", (path,))?;
        }
    } else {
        path = os_path.call_method1("join", (os.call_method0("getcwd")?, path))?;
    }
    path = os_path.call_method1("expandvars", (path,))?;
    if as_pathlib {
        return pathlib.getattr("Path")?.call1((path,));
    }
    Ok(path)
}

/// music21's `common.pathTools`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_function(wrap_pyfunction!(getSourceFilePath, &m)?)?;
    m.add_function(wrap_pyfunction!(getMetadataCacheFilePath, &m)?)?;
    m.add_function(wrap_pyfunction!(getCorpusFilePath, &m)?)?;
    m.add_function(wrap_pyfunction!(getCorpusContentDirs, &m)?)?;
    m.add_function(wrap_pyfunction!(getRootFilePath, &m)?)?;
    m.add_function(wrap_pyfunction!(relativepath, &m)?)?;
    m.add_function(wrap_pyfunction!(cleanpath, &m)?)?;
    let names = [
        "getRootFilePath",
        "getSourceFilePath",
        "getMetadataCacheFilePath",
        "getCorpusFilePath",
        "getCorpusContentDirs",
        "relativepath",
        "cleanpath",
    ];
    m.add("__all__", PyList::new(py, names)?)?;
    Ok(m)
}
