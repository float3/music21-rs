//! music21's `metadata.caching`: parsing scores for a bundle's metadata.
//!
//! A `MetadataCachingJob` parses one file and makes a `MetadataEntry` of
//! each score in it; `JobProcessor` runs jobs and reports on each. Jobs
//! run one after another here: music21 runs them in worker processes
//! where asked, which pickles each job between processes, and these
//! classes are not built for that.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyTuple};

use music21::metadata::bundles::MetadataEntry as RsEntry;
use music21::metadata::rich::RichMetadata as RsRich;

use crate::bundles::MetadataEntry;

/// The module's dotted name.
const MODULE: &str = "music21.metadata.caching";

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<MetadataCachingJob>()?;
        m.add_class::<JobProcessor>()?;
        m.add_function(wrap_pyfunction!(cache_metadata, &m)?)?;
        m.add("environLocal", environment(py)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

fn environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    crate::environment::module(py)?
        .getattr("Environment")?
        .call1(("metadata.caching",))
}

/// One file to parse for metadata, and what came of it.
pub(crate) struct Job {
    file_path: Py<PyAny>,
    parse_using_corpus: bool,
    corpus_name: Option<String>,
}

impl Job {
    pub(crate) fn new(
        py: Python<'_>,
        path: &Bound<'_, PyAny>,
        parse_using_corpus: bool,
        corpus_name: Option<String>,
    ) -> PyResult<Self> {
        let file_path = py.import("pathlib")?.getattr("Path")?.call1((path,))?;
        Ok(Self {
            file_path: file_path.unbind(),
            parse_using_corpus,
            corpus_name,
        })
    }

    /// The path relative to the corpus, where it lies inside it.
    fn clean_path(&self, py: Python<'_>) -> PyResult<String> {
        let path = self.file_path.bind(py);
        if let Ok(root) = crate::corpus::corpus_file_path(py)
            && let Ok(relative) = path.call_method1("relative_to", (root,))
        {
            return Ok(relative.str()?.to_string());
        }
        Ok(path.str()?.to_string())
    }

    /// Parses the file, answering its entries and the paths that would
    /// not parse: music21's `run`.
    pub(crate) fn run(&self, py: Python<'_>) -> PyResult<(Vec<RsEntry>, Vec<Py<PyAny>>)> {
        let path = self.file_path.bind(py);
        let kwargs = PyDict::new(py);
        kwargs.set_item("forceSource", true)?;
        let parsed = if self.parse_using_corpus {
            crate::corpus::module(py)?
                .getattr("parse")?
                .call((path.str()?,), Some(&kwargs))
        } else {
            crate::converter::module(py)?
                .getattr("parse")?
                .call((path,), Some(&kwargs))
        };
        let Ok(parsed) = parsed else {
            return Ok((Vec::new(), vec![path.clone().unbind()]));
        };
        let clean = self.clean_path(py)?;
        let corpus_name = self.corpus_name.clone().unwrap_or_default();
        let mut entries = Vec::new();

        // An opus files each numbered score, then a stand-in for itself.
        let classes: Vec<String> = parsed.getattr("classes")?.extract()?;
        if classes.iter().any(|class| class == "Opus") {
            for score in parsed.getattr("scores")?.try_iter()? {
                let score = score?;
                let metadata = score.getattr("metadata")?;
                let number = if metadata.is_none() {
                    None
                } else {
                    Some(metadata.getattr("number")?).filter(|number| !number.is_none())
                };
                let (Some(number), Some(rich)) = (number, rich_of(&score)?) else {
                    continue;
                };
                entries.push(RsEntry::new(
                    clean.clone(),
                    Some(number.str()?.to_string()),
                    Some(rich),
                ));
            }
            entries.push(RsEntry::new(clean, None, None));
            return Ok((entries, Vec::new()));
        }
        let mut entry = RsEntry::new(clean, None, rich_of(&parsed)?);
        entry.corpus_name = corpus_name;
        entries.push(entry);
        Ok((entries, Vec::new()))
    }
}

/// A score's metadata with what is read off the score, or nothing for a
/// score with no metadata.
fn rich_of(score: &Bound<'_, PyAny>) -> PyResult<Option<RsRich>> {
    let py = score.py();
    let metadata = score.getattr("metadata")?;
    if metadata.is_none() {
        return Ok(None);
    }
    let mut rich = crate::richmetadata::rich_value(&metadata)?;
    let (stream, _) = crate::stream::read_with_leaves(score)?;
    let corpus = crate::corpus::corpus_directory(py);
    rich.update(&stream, corpus.as_deref())
        .map_err(|error| crate::stream::stream_error(&error))?;
    Ok(Some(rich))
}

/// music21's `MetadataCachingJob`.
#[pyclass(name = "MetadataCachingJob", module = "music21.metadata.caching")]
pub struct MetadataCachingJob {
    job: Job,
    #[pyo3(get)]
    jobNumber: i64,
    #[pyo3(get)]
    filePathErrors: Py<PyList>,
    #[pyo3(get)]
    results: Py<PyList>,
}

#[pymethods]
impl MetadataCachingJob {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.filePathErrors)?;
        visit.call(&self.results)
    }

    #[new]
    #[pyo3(signature = (filePath, jobNumber = 0, parseUsingCorpus = true, corpusName = None))]
    fn new(
        py: Python<'_>,
        filePath: &Bound<'_, PyAny>,
        jobNumber: i64,
        parseUsingCorpus: bool,
        corpusName: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self {
            job: Job::new(py, filePath, parseUsingCorpus, corpusName)?,
            jobNumber,
            filePathErrors: PyList::empty(py).unbind(),
            results: PyList::empty(py).unbind(),
        })
    }

    #[getter]
    fn filePath(&self, py: Python<'_>) -> Py<PyAny> {
        self.job.file_path.clone_ref(py)
    }

    #[getter]
    fn parseUsingCorpus(&self) -> bool {
        self.job.parse_using_corpus
    }

    #[getter]
    fn corpusName(&self) -> Option<String> {
        self.job.corpus_name.clone()
    }

    /// The path relative to the corpus, where it lies inside it.
    #[getter]
    fn cleanFilePath<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        py.import("pathlib")?
            .getattr("Path")?
            .call1((self.job.clean_path(py)?,))
    }

    /// Parses the file, answering its entries and its errors.
    fn run<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let (entries, errors) = self.job.run(py)?;
        let results = self.results.bind(py);
        results.call_method0("clear")?;
        for entry in entries {
            results.append(MetadataEntry::object(py, &std::sync::Arc::new(entry))?)?;
        }
        for error in errors {
            self.filePathErrors.bind(py).append(error)?;
        }
        PyTuple::new(
            py,
            [
                self.getResults(py)?.into_any(),
                self.getErrors(py)?.into_any(),
            ],
        )
    }

    fn getErrors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.filePathErrors
            .bind(py)
            .to_tuple()
            .into_pyobject(py)
            .map_err(Into::into)
    }

    fn getResults<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.results
            .bind(py)
            .to_tuple()
            .into_pyobject(py)
            .map_err(Into::into)
    }
}

/// music21's `JobProcessor`: runs jobs, reporting on each.
#[pyclass(name = "JobProcessor", module = "music21.metadata.caching")]
pub struct JobProcessor;

#[pymethods]
impl JobProcessor {
    /// A line on how far the jobs have got.
    #[staticmethod]
    fn _report(
        totalJobs: i64,
        remainingJobs: i64,
        filePath: &Bound<'_, PyAny>,
        filePathErrorCount: i64,
    ) -> PyResult<String> {
        Ok(format!(
            "updated {} of {totalJobs} files; total errors: {filePathErrorCount} ... last file: {}",
            totalJobs - remainingJobs,
            filePath.str()?
        ))
    }

    /// Runs the jobs one after another, as music21's `process_serial`
    /// does: an iterator of a dictionary for each job.
    #[staticmethod]
    fn process_serial<'py>(
        py: Python<'py>,
        jobs: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let jobs: Vec<Bound<'py, PyAny>> = jobs.try_iter()?.collect::<PyResult<_>>()?;
        let mut remaining = jobs.len();
        let reports = PyList::empty(py);
        for job in jobs {
            let ran = job.call_method0("run")?;
            remaining -= 1;
            let report = PyDict::new(py);
            report.set_item("metadataEntries", ran.get_item(0)?)?;
            report.set_item("errors", ran.get_item(1)?)?;
            report.set_item("filePath", job.getattr("filePath")?)?;
            report.set_item("remainingJobs", remaining)?;
            reports.append(report)?;
        }
        reports.try_iter().map(Bound::into_any)
    }

    /// The same as `process_serial`: the jobs run in this process.
    #[staticmethod]
    #[pyo3(signature = (jobs, processCount = None))]
    fn process_parallel<'py>(
        py: Python<'py>,
        jobs: &Bound<'py, PyAny>,
        processCount: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let _ = processCount;
        Self::process_serial(py, jobs)
    }
}

/// Caches the metadata of each corpus named, all of them where none is.
#[pyfunction]
#[pyo3(name = "cacheMetadata", signature = (corpusNames = None, useMultiprocessing = true, verbose = false))]
fn cache_metadata(
    py: Python<'_>,
    corpusNames: Option<&Bound<'_, PyAny>>,
    useMultiprocessing: bool,
    verbose: bool,
) -> PyResult<()> {
    let names: Vec<Option<String>> = match corpusNames.filter(|names| !names.is_none()) {
        Some(names) if names.extract::<String>().is_ok() => vec![Some(names.extract()?)],
        Some(names) => names.extract()?,
        None => {
            let mut names: Vec<Option<String>> = crate::corpora::local_names(py)?
                .into_iter()
                .map(Some)
                .collect();
            names.push(Some("local".to_string()));
            names.push(Some("core".to_string()));
            names
        }
    };
    for name in names {
        let corpus = crate::corpora::from_name(py, name.as_deref())?;
        corpus.call_method1("cacheMetadata", (useMultiprocessing, verbose))?;
    }
    Ok(())
}
