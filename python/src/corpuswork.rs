//! music21's `corpus.work`: the works in each of a corpus's directories,
//! found by the crate's `corpus::work`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::PathBuf;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21::corpus::work::{self as rs_work, Holder};

/// The module's dotted name.
const MODULE: &str = "music21.corpus.work";

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let named = py.import("collections")?.getattr("namedtuple")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("module", MODULE)?;
        let work = named.call(("CorpusWork", ["title", "files", "virtual"]), Some(&kwargs))?;
        let file = named.call(
            ("CorpusFile", ["path", "title", "filename", "format", "ext"]),
            Some(&kwargs),
        )?;
        m.add("CorpusWork", work)?;
        m.add("CorpusFile", file)?;
        m.add_class::<DirectoryInformation>()?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// A directory of a corpus and the works in it.
#[pyclass(name = "DirectoryInformation", module = "music21.corpus.work")]
pub struct DirectoryInformation {
    #[pyo3(get, set)]
    directoryName: String,
    #[pyo3(get, set)]
    directoryTitle: String,
    #[pyo3(get, set)]
    isComposer: bool,
    #[pyo3(get)]
    works: Py<PyAny>,
    #[pyo3(get, set)]
    corpusObject: Option<Py<PyAny>>,
}

#[pymethods]
impl DirectoryInformation {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.works)?;
        visit.call(&self.corpusObject)
    }

    fn __clear__(&mut self) {
        self.corpusObject = None;
    }

    #[new]
    #[pyo3(signature = (dirName = String::new(), dirTitle = String::new(), isComposer = true, corpusObject = None))]
    fn new(
        py: Python<'_>,
        dirName: String,
        dirTitle: String,
        isComposer: bool,
        corpusObject: Option<Py<PyAny>>,
    ) -> PyResult<Py<Self>> {
        let works = py.import("collections")?.getattr("OrderedDict")?.call0()?;
        let made = Py::new(
            py,
            Self {
                directoryName: dirName,
                directoryTitle: dirTitle,
                isComposer,
                works: works.unbind(),
                corpusObject,
            },
        )?;
        Self::findWorks(made.bind(py))?;
        Ok(made)
    }

    fn __repr__(&self) -> String {
        format!("<{MODULE}.DirectoryInformation {}>", self.directoryName)
    }

    /// Fills `works` from the directory's files in the corpus, and answers
    /// it.
    fn findWorks<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let me = slf.borrow();
        let works = me.works.bind(py).clone();
        works.call_method0("clear")?;
        let corpus = match &me.corpusObject {
            Some(corpus) => corpus.bind(py).clone(),
            None => py.None().into_bound(py),
        };
        let paths: Vec<PathBuf> = corpus
            .call_method1("getComposer", (&me.directoryName,))?
            .try_iter()?
            .map(|path| Ok(PathBuf::from(path?.str()?.to_string())))
            .collect::<PyResult<_>>()?;

        // Each work as music21's named tuples.
        let module = module(py)?;
        let work_class = module.getattr("CorpusWork")?;
        let file_class = module.getattr("CorpusFile")?;
        for work in rs_work::find_works(&me.directoryName, &paths) {
            let files = PyList::empty(py);
            for file in work.files {
                files.append(file_class.call1((
                    file.path,
                    file.title,
                    file.filename,
                    file.format,
                    file.ext,
                ))?)?;
            }
            works.set_item(work.stub, work_class.call1((work.title, files, false))?)?;
        }
        Ok(works)
    }
}

/// The paths of a corpus's works that belong to a composer or collection.
pub(crate) fn composer_paths<'py>(
    corpus: &Bound<'py, PyAny>,
    composerName: &str,
    fileExtensions: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyList>> {
    let py = corpus.py();
    let kwargs = PyDict::new(py);
    kwargs.set_item("fileExtensions", fileExtensions)?;
    let paths: Vec<PathBuf> = corpus
        .call_method("getPaths", (), Some(&kwargs))?
        .try_iter()?
        .map(|path| Ok(PathBuf::from(path?.str()?.to_string())))
        .collect::<PyResult<_>>()?;
    let class = py.import("pathlib")?.getattr("Path")?;
    let found = rs_work::composer_paths(composerName, &paths)
        .into_iter()
        .map(|path| class.call1((path,)))
        .collect::<PyResult<Vec<_>>>()?;
    PyList::new(py, found)
}

/// The core corpus's directories, each with its works.
pub(crate) fn core_directories<'py>(corpus: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let py = corpus.py();
    let mut made = Vec::new();
    for directory in rs_work::CORE_DIRECTORIES {
        let information = DirectoryInformation::new(
            py,
            directory.name.to_string(),
            directory.title.to_string(),
            directory.holds == Holder::Composer,
            Some(corpus.clone().unbind()),
        )?;
        made.push(information);
    }
    PyTuple::new(py, made)
}
