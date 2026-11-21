//! music21's `corpus.virtual`: works kept elsewhere, each a class with the
//! addresses it can be downloaded from. The works and the choosing of an
//! address by format are the crate's `corpus::virtual_corpus`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyList, PyTuple};

use music21::corpus::virtual_corpus::{VIRTUAL_WORKS, urls_by_ext};

use crate::plumbing::built::new_class;

/// The module's dotted name.
const MODULE: &str = "music21.corpus.virtual";

/// The class attribute naming which of the crate's works a class is.
const WORK: &str = "_work";

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let environment = crate::environment::module(py)?
            .getattr("Environment")?
            .call1(("corpus.virtual",))?;
        m.add("environLocal", environment)?;
        m.add_class::<VirtualWork>()?;

        // Each work is a class of its own, as music21's is, which fills
        // itself in from the crate's table.
        let base = py.get_type::<VirtualWork>().into_any();
        for (at, work) in VIRTUAL_WORKS.iter().enumerate() {
            let class = new_class(
                py,
                MODULE,
                work.class,
                work.title,
                vec![base.clone()],
                vec![(WORK, at.into_pyobject(py)?.into_any())],
            )?;
            m.add(work.class, class)?;
        }
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// music21's `VirtualWork`: a work kept elsewhere, its composer, title,
/// path in the corpus and addresses.
#[pyclass(
    name = "VirtualWork",
    module = "music21.corpus.virtual",
    subclass,
    dict
)]
pub struct VirtualWork;

#[pymethods]
impl VirtualWork {
    #[new]
    #[pyo3(signature = (*_arguments))]
    fn new(_arguments: &Bound<'_, PyTuple>) -> Self {
        Self
    }

    /// Fills in the work its class is, or leaves it empty, as music21's
    /// base class is.
    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let work = slf
            .get_type()
            .getattr(WORK)
            .ok()
            .and_then(|at| at.extract::<usize>().ok())
            .and_then(|at| VIRTUAL_WORKS.get(at));
        let Some(work) = work else {
            slf.setattr("composer", py.None())?;
            slf.setattr("title", py.None())?;
            slf.setattr("corpusPath", py.None())?;
            return slf.setattr("urlList", PyList::empty(py));
        };
        slf.setattr("composer", work.composer)?;
        slf.setattr("title", work.title)?;
        slf.setattr("corpusPath", work.corpus_path)?;
        slf.setattr("urlList", PyList::new(py, work.urls)?)
    }

    /// The addresses whose format has one of the extensions given, or the
    /// first address where none is: music21's `getUrlByExt`.
    #[pyo3(signature = (extList = None))]
    fn getUrlByExt<'py>(
        slf: &Bound<'py, Self>,
        extList: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let urls: Vec<String> = slf.getattr("urlList")?.extract()?;

        // One extension or a list of them; none at all asks for the best.
        let extensions: Vec<Option<String>> = match extList {
            Some(given)
                if given.is_instance_of::<PyList>() || given.is_instance_of::<PyTuple>() =>
            {
                given.extract()?
            }
            Some(given) => vec![given.extract()?],
            None => vec![None],
        };
        if extensions == [None] {
            let first = urls.first().ok_or_else(|| {
                pyo3::exceptions::PyIndexError::new_err("list index out of range")
            })?;
            return PyList::new(py, [first]);
        }
        let wanted: Vec<&str> = extensions.iter().flatten().map(String::as_str).collect();
        let held: Vec<&str> = urls.iter().map(String::as_str).collect();
        PyList::new(py, urls_by_ext(&held, &wanted))
    }
}
