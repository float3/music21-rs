//! music21's `common.fileTools`: reading files whose encoding is not known,
//! compressed members with a size limit, gzipped pickles, a `with` that
//! changes directory, and a program run with its errors kept.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::io::Read;
use std::path::PathBuf;

use pyo3::exceptions::{PyOSError, PyUnicodeDecodeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyString, PyTuple};

use music21::common::filetools::{MAX_UNCOMPRESSED_BYTES, read_limited};

use super::built::with_context;
use crate::Music21Exception;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.common.fileTools";

/// `pathlib`'s two path classes as they were when the module was built,
/// which unpickling swaps and puts back.
static STORED_PATHLIB: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

fn stored_pathlib(py: Python<'_>) -> PyResult<&Py<PyDict>> {
    STORED_PATHLIB.get_or_try_init(py, || {
        let pathlib = py.import("pathlib")?;
        let stored = PyDict::new(py);
        stored.set_item("posixPath", pathlib.getattr("PosixPath")?)?;
        stored.set_item("windowsPath", pathlib.getattr("WindowsPath")?)?;
        Ok(stored.unbind())
    })
}

/// A Python file object read as a Rust reader; an error it raises is
/// carried through and given back.
struct PythonReader<'py> {
    file: Bound<'py, PyAny>,
    error: Option<PyErr>,
}

impl Read for PythonReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self
            .file
            .call_method1("read", (buffer.len(),))
            .and_then(|chunk| chunk.extract::<Vec<u8>>());
        match read {
            Ok(chunk) => {
                let count = chunk.len().min(buffer.len());
                buffer[..count].copy_from_slice(&chunk[..count]);
                Ok(count)
            }
            Err(error) => {
                self.error = Some(error);
                Err(std::io::Error::other("the file could not be read"))
            }
        }
    }
}

/// The uncompressed bytes of the member `name` of a `zipfile.ZipFile`,
/// raising `exceptionClass` where it holds more than
/// `MAX_UNCOMPRESSED_BYTES`, whatever its header claims.
#[pyfunction]
#[pyo3(signature = (zipFile, name, exceptionClass = None))]
pub(crate) fn readZipMember<'py>(
    zipFile: &Bound<'py, PyAny>,
    name: &Bound<'py, PyAny>,
    exceptionClass: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyBytes>> {
    let py = zipFile.py();
    let exception = match exceptionClass {
        Some(class) => class.clone(),
        None => py.get_type::<Music21Exception>().into_any(),
    };
    let too_large = || -> PyResult<PyErr> {
        let message = format!(
            "{} uncompresses to more than {MAX_UNCOMPRESSED_BYTES} bytes",
            name.repr()?
        );
        Ok(PyErr::from_value(exception.call1((message,))?))
    };
    let info = zipFile.call_method1("getinfo", (name,))?;
    if info.getattr("file_size")?.gt(MAX_UNCOMPRESSED_BYTES)? {
        return Err(too_large()?);
    }
    let member = zipFile.call_method1("open", (info,))?;
    let read = with_context(&member, |opened| {
        let mut reader = PythonReader {
            file: opened.clone(),
            error: None,
        };
        match read_limited(&mut reader, MAX_UNCOMPRESSED_BYTES) {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err(too_large()?),
            Err(error) => Err(reader.error.take().unwrap_or_else(|| error.into())),
        }
    })?;
    Ok(PyBytes::new(py, &read.unwrap_or_default()))
}

/// What `cd` hands back: in the directory on entering, back where it was
/// on leaving, whatever happened in between.
#[pyclass(module = "music21.common.fileTools", name = "_ChangedDirectory")]
pub(crate) struct ChangedDirectory {
    target: PathBuf,
    previous: Option<PathBuf>,
}

#[pymethods]
impl ChangedDirectory {
    fn __enter__(&mut self) -> PyResult<()> {
        let previous = std::env::current_dir()?;
        self.previous = Some(previous.clone());
        if let Err(error) = std::env::set_current_dir(&self.target) {
            std::env::set_current_dir(previous)?;
            self.previous = None;
            return Err(error.into());
        }
        Ok(())
    }

    #[pyo3(signature = (*exception))]
    fn __exit__(&mut self, exception: &Bound<'_, PyTuple>) -> PyResult<bool> {
        let _ = exception;
        if let Some(previous) = self.previous.take() {
            std::env::set_current_dir(previous)?;
        }
        Ok(false)
    }
}

/// Changes to `targetDir` for the length of a `with`, then back.
#[pyfunction]
pub(crate) fn cd(targetDir: PathBuf) -> ChangedDirectory {
    ChangedDirectory {
        target: targetDir,
        previous: None,
    }
}

/// The contents of a gzipped pickle. A pickle that cannot be read is a
/// `Music21Exception` naming the file.
#[pyfunction]
pub(crate) fn readPickleGzip<'py>(filePath: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = filePath.py();
    preparePathClassesForUnpickling(py)?;
    let file = py.import("gzip")?.call_method1("open", (filePath, "rb"))?;
    let read = with_context(&file, |opened| {
        let loaded = opened
            .call_method0("read")
            .and_then(|bytes| py.import("pickle")?.call_method1("loads", (bytes,)));
        match loaded {
            Ok(loaded) => Ok(loaded),
            Err(error) => {
                restorePathClassesAfterUnpickling(py)?;
                let message = format!("Cannot load file {}", filePath.str()?);
                let raised =
                    PyErr::from_value(py.get_type::<Music21Exception>().call1((message,))?);
                raised.set_cause(py, Some(error));
                Err(raised)
            }
        }
    })?;
    restorePathClassesAfterUnpickling(py)?;
    Ok(read.unwrap_or_else(|| py.None().into_bound(py)))
}

/// Text decoded with universal newlines, as a file opened for text gives it.
fn universal_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// A file's text in the encoding `firstGuess` names, or, where it cannot be
/// read so, in whatever encoding `chardet` takes it to be in. Slow when the
/// guess is wrong, so the guess should be a likely one.
#[pyfunction]
#[pyo3(signature = (filePath, firstGuess = "utf-8"))]
pub(crate) fn readFileEncodingSafe(
    py: Python<'_>,
    filePath: PathBuf,
    firstGuess: &str,
) -> PyResult<String> {
    let bytes = std::fs::read(&filePath)?;
    let raw = PyBytes::new(py, &bytes);
    let codecs = py.import("codecs")?;
    match codecs.call_method1("decode", (&raw, firstGuess)) {
        Ok(text) => return Ok(universal_newlines(&text.extract::<String>()?)),
        Err(error) if error.is_instance_of::<PyUnicodeDecodeError>(py) => {}
        Err(error) => return Err(error),
    }
    let detected = py.import("chardet")?.call_method1("detect", (&raw,))?;
    let encoding = detected.get_item("encoding")?;
    let encoding = if encoding.is_truthy()? {
        encoding
    } else {
        PyString::new(py, "ascii").into_any()
    };
    codecs.call_method1("decode", (raw, encoding))?.extract()
}

/// Which of `pathlib`'s path classes stands for the other while a pickle
/// from another platform is read: the platform's own.
fn on_windows() -> bool {
    cfg!(windows)
}

/// Makes `pathlib`'s path class of the other platform this platform's, so
/// that a pickle written on either reads here.
#[pyfunction]
pub(crate) fn preparePathClassesForUnpickling(py: Python<'_>) -> PyResult<()> {
    let pathlib = py.import("pathlib")?;
    stored_pathlib(py)?;
    if on_windows() {
        pathlib.setattr("PosixPath", pathlib.getattr("WindowsPath")?)?;
    } else {
        pathlib.setattr("WindowsPath", pathlib.getattr("PosixPath")?)?;
    }
    Ok(())
}

/// Puts back the class `preparePathClassesForUnpickling` replaced.
#[pyfunction]
pub(crate) fn restorePathClassesAfterUnpickling(py: Python<'_>) -> PyResult<()> {
    let pathlib = py.import("pathlib")?;
    let stored = stored_pathlib(py)?.bind(py);
    if on_windows() {
        pathlib.setattr("PosixPath", stored.get_item("posixPath")?)?;
    } else {
        pathlib.setattr("WindowsPath", stored.get_item("windowsPath")?)?;
    }
    Ok(())
}

/// Runs a program, keeping what it writes to standard error, and raises
/// `OSError` with that text if it fails.
#[pyfunction]
pub(crate) fn runSubprocessCapturingStderr(
    py: Python<'_>,
    subprocessCommand: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let mut command: Vec<PathBuf> = Vec::new();
    for part in subprocessCommand.try_iter()? {
        command.push(part?.extract()?);
    }
    let Some((program, arguments)) = command.split_first() else {
        return Err(pyo3::exceptions::PyIndexError::new_err(
            "list index out of range",
        ));
    };
    let finished = py.detach(|| {
        std::process::Command::new(program)
            .args(arguments)
            .stdin(std::process::Stdio::inherit())
            .output()
    })?;
    if finished.status.success() {
        return Ok(());
    }
    let encoding = py
        .import("locale")?
        .getattr("getpreferredencoding")?
        .call1((false,))?;
    let errors = PyBytes::new(py, &finished.stderr);
    let text = match errors.call_method1("decode", (encoding,)) {
        Ok(text) => text,
        Err(error) if error.is_instance_of::<PyUnicodeDecodeError>(py) => {
            errors.call_method1("decode", ("ascii", "ignore"))?
        }
        Err(error) => return Err(error),
    };
    Err(PyOSError::new_err(text.unbind()))
}

/// music21's `common.fileTools`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add("MAX_UNCOMPRESSED_BYTES", MAX_UNCOMPRESSED_BYTES)?;
    m.add_function(wrap_pyfunction!(readFileEncodingSafe, &m)?)?;
    m.add_function(wrap_pyfunction!(readZipMember, &m)?)?;
    m.add_function(wrap_pyfunction!(readPickleGzip, &m)?)?;
    m.add_function(wrap_pyfunction!(cd, &m)?)?;
    m.add_function(wrap_pyfunction!(preparePathClassesForUnpickling, &m)?)?;
    m.add_function(wrap_pyfunction!(restorePathClassesAfterUnpickling, &m)?)?;
    m.add_function(wrap_pyfunction!(runSubprocessCapturingStderr, &m)?)?;
    m.add("_storedPathlibClasses", stored_pathlib(py)?.bind(py))?;
    m.add(
        "__all__",
        [
            "MAX_UNCOMPRESSED_BYTES",
            "readFileEncodingSafe",
            "readZipMember",
            "readPickleGzip",
            "cd",
            "preparePathClassesForUnpickling",
            "restorePathClassesAfterUnpickling",
            "runSubprocessCapturingStderr",
        ],
    )?;
    Ok(m)
}
