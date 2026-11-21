//! music21's `converter.museScore`: finding MuseScore, running it to make
//! images of a score, and finding the numbered pages it writes. Where to
//! look and what the pages are called is the crate's `musescore::locate`.
//!
//! Each function reaches the others, `environLocal` and
//! `environment.set` through their modules, as music21's reach them
//! through its globals, so that what a caller puts in their place is used.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::{Path, PathBuf};

use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyTuple};

use music21::environment::Platform;
use music21::musescore::locate::{self, ImageRequest, PageError};

use crate::exceptions21::SubConverterException;
use crate::subconverters::music21_name;

/// The module's dotted name.
const MODULE: &str = "music21.converter.museScore";

/// The environment key naming the MuseScore that makes images.
const DIRECT_PNG_KEY: &str = "musescoreDirectPNGPath";

/// The extension MuseScore is asked for where no subformat names one.
const PNG: &str = "png";

/// The variables MuseScore needs to run headless in a notebook on Linux,
/// set only for the run where they are not set already.
const QT_PLATFORM: &str = "QT_QPA_PLATFORM";
const XDG_RUNTIME: &str = "XDG_RUNTIME_DIR";

/// What music21 says when MuseScore is not installed.
const NOT_INSTALLED: &str = "To show notation, music21 needs MuseScore (free from https://musescore.org), even if you use other notation software.  Install it and then try again.";

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        let environment = crate::environment::module(py)?
            .getattr("Environment")?
            .call1(("converter.museScore",))?;
        m.add("environLocal", environment)?;

        // music21's pattern, kept for callers who read it.
        let re = py.import("re")?;
        let pattern =
            re.call_method1("compile", (r"MuseScore ?[12]\b", re.getattr("IGNORECASE")?))?;
        m.add("reMuseScoreBefore3", pattern)?;
        m.add(
            "SubConverterException",
            py.get_type::<SubConverterException>(),
        )?;

        m.add_function(wrap_pyfunction!(findMuseScore, &m)?)?;
        m.add_function(wrap_pyfunction!(getOrFindMuseScoreDirectPNGPath, &m)?)?;
        m.add_function(wrap_pyfunction!(runThroughMuseScore, &m)?)?;
        m.add_function(wrap_pyfunction!(findNumberedPNGPath, &m)?)?;
        m.add_function(wrap_pyfunction!(findLastPNGPath, &m)?)?;
        m.add_function(wrap_pyfunction!(findPNGRange, &m)?)?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// A path as a `pathlib.Path`.
fn path_object<'py>(py: Python<'py>, path: &Path) -> PyResult<Bound<'py, PyAny>> {
    py.import("pathlib")?.getattr("Path")?.call1((path,))
}

/// The platform `common.getPlatform` names, asked each time so that a
/// caller who replaces it is heard.
fn platform(py: Python<'_>) -> PyResult<Platform> {
    let name: String = music21_name(py, "music21.common", "getPlatform")?
        .call0()?
        .extract()?;
    Ok(Platform::from_name(&name).unwrap_or(Platform::Nix))
}

/// A page refusal as the exception music21 raises for it.
fn page_error(error: PageError) -> PyErr {
    match error {
        PageError::NotPng(said) | PageError::NoNumber(said) => PyValueError::new_err(said),
        PageError::Unnumbered(said) => SubConverterException::new_err(said),
        PageError::Missing(said) => PyOSError::new_err(said),
    }
}

/// The MuseScore program, version 3 or newer, that can make images of
/// notation, or `None`: the newest found in `searchDirs`, else where this
/// platform installs it.
#[pyfunction]
#[pyo3(signature = (searchDirs = None))]
fn findMuseScore<'py>(
    py: Python<'py>,
    searchDirs: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let platform = platform(py)?;
    let directories: Vec<PathBuf> = match searchDirs.filter(|given| !given.is_none()) {
        Some(given) => given
            .try_iter()?
            .map(|directory| directory?.extract())
            .collect::<PyResult<_>>()?,
        None => {
            let home: PathBuf = py
                .import("pathlib")?
                .getattr("Path")?
                .call_method0("home")?
                .extract()?;
            locate::default_directories(platform, &home)
        }
    };
    locate::find(platform, &directories)
        .map(|found| path_object(py, &found))
        .transpose()
}

/// The MuseScore set as `musescoreDirectPNGPath`, else the one found,
/// which is saved there. Raises `SubConverterException` where there is
/// none.
#[pyfunction]
fn getOrFindMuseScoreDirectPNGPath(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let module = py.import(MODULE)?;
    let set = module.getattr("environLocal")?.get_item(DIRECT_PNG_KEY)?;
    if !set.is_none() && set.call_method0("exists")?.is_truthy()? {
        return Ok(set);
    }
    let found = module.getattr("findMuseScore")?.call0()?;
    if found.is_none() {
        return Err(SubConverterException::new_err(NOT_INSTALLED));
    }
    py.import("music21.environment")?
        .getattr("set")?
        .call1((DIRECT_PNG_KEY, &found))?;
    Ok(found)
}

/// Runs a file through MuseScore to make an image of it, a PNG unless the
/// first subformat names another kind, and answers the file made: for a
/// PNG, its first page.
#[pyfunction]
#[pyo3(signature = (fp, subformats = None, *, dpi = None, trimEdges = true, leaveMargin = 0, **_keywords))]
fn runThroughMuseScore<'py>(
    py: Python<'py>,
    fp: &Bound<'py, PyAny>,
    subformats: Option<&Bound<'py, PyAny>>,
    dpi: Option<u32>,
    #[pyo3(from_py_with = crate::truthy)] trimEdges: bool,
    leaveMargin: u32,
    _keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let module = py.import(MODULE)?;
    let executable: PathBuf = module
        .getattr("getOrFindMuseScoreDirectPNGPath")?
        .call0()?
        .extract()?;

    // The first subformat names the kind of file; none, or an empty one, is
    // a PNG.
    let mut extension = PNG.to_string();
    if let Some(first) = subformats.filter(|given| given.is_truthy().unwrap_or(false)) {
        let named = first.get_item(0)?;
        if named.is_truthy()? {
            extension = named.extract()?;
        }
    }
    let output = fp.call_method1("with_suffix", (format!(".{extension}"),))?;

    // In a notebook music21 sets its own resolution, and on Linux lets
    // MuseScore run without a display.
    let notebook = music21_name(py, "music21.common", "runningInNotebook")?
        .call0()?
        .is_truthy()?;
    let headless = notebook && platform(py)? == Platform::Nix;
    let notebook_dpi: u32 = crate::defaults::defaults_module(py)?
        .getattr("jupyterImageDpi")?
        .extract()?;
    let request = ImageRequest {
        dpi: dpi.or(notebook.then_some(notebook_dpi)),
        trim: trimEdges.then_some(leaveMargin),
    };
    let input: PathBuf = fp.extract()?;
    let output_path: PathBuf = output.extract()?;
    let command = locate::image_command(&executable, &input, &output_path, &request);

    let environ = py.import("os")?.getattr("environ")?;
    let mut added = Vec::new();
    if headless {
        let runtime = crate::environment::module(py)?
            .getattr("Environment")?
            .call0()?
            .call_method0("getRootTempDir")?
            .str()?;
        for (variable, value) in [
            (QT_PLATFORM, "offscreen".to_string()),
            (XDG_RUNTIME, runtime.to_string()),
        ] {
            if !environ.contains(variable)? {
                environ.set_item(variable, value)?;
                added.push(variable);
            }
        }
    }
    let ran = music21_name(
        py,
        "music21.common.fileTools",
        "runSubprocessCapturingStderr",
    )?
    .call1((command,));

    // The environment is left as it was found.
    for variable in added {
        environ.del_item(variable)?;
    }
    ran?;

    if extension == PNG {
        return findNumberedPNGPath(py, output_path);
    }
    Ok(output)
}

/// The first numbered page MuseScore wrote for a path ending in `.png`.
#[pyfunction]
fn findNumberedPNGPath<'py>(py: Python<'py>, inputFp: PathBuf) -> PyResult<Bound<'py, PyAny>> {
    let found = locate::first_page(&inputFp).map_err(page_error)?;
    path_object(py, &found)
}

/// The last numbered page beside a numbered one.
#[pyfunction]
fn findLastPNGPath<'py>(py: Python<'py>, inputFp: PathBuf) -> PyResult<Bound<'py, PyAny>> {
    let found = locate::last_page(&inputFp).map_err(page_error)?;
    path_object(py, &found)
}

/// The number of the last page and how many digits number the pages.
#[pyfunction]
#[pyo3(signature = (firstFp, lastFp = None))]
fn findPNGRange<'py>(
    py: Python<'py>,
    firstFp: PathBuf,
    lastFp: Option<PathBuf>,
) -> PyResult<Bound<'py, PyTuple>> {
    // A first page not named `.png` is refused before the last is looked
    // for, as music21 refuses it.
    let png = firstFp.to_string_lossy().ends_with(".png");
    let last = match lastFp {
        Some(last) => last,
        None if png => locate::last_page(&firstFp).map_err(page_error)?,
        None => firstFp.clone(),
    };
    let range = locate::page_range(&firstFp, &last).map_err(|error| match error {
        PageError::Unnumbered(said) => PyValueError::new_err(said),
        other => page_error(other),
    })?;
    PyTuple::new(py, [range.last as usize, range.digits])
}
