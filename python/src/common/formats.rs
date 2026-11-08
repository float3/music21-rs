//! music21's `common.formats`, over the crate's `common::formats`.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use pyo3::wrap_pyfunction;

use music21::common::formats as tools;

/// music21's `formats.__all__`.
const ALL: [&str; 9] = [
    "findSubConverterForFormat",
    "findFormat",
    "findInputExtension",
    "findFormatFile",
    "findFormatExtFile",
    "findFormatExtURL",
    "VALID_SHOW_FORMATS",
    "VALID_WRITE_FORMATS",
    "VALID_AUTO_DOWNLOAD",
];

/// The `music21.converter.subConverters` class for a music21 format name,
/// not a file extension, or None.
#[pyfunction]
#[pyo3(name = "findSubConverterForFormat")]
fn find_sub_converter_for_format<'py>(
    py: Python<'py>,
    fmt: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(found) = tools::sub_converter_for_format(fmt) else {
        return Ok(None);
    };
    let classes = py.import("music21.converter.subConverters")?;
    classes.getattr(found.class_name).map(Some)
}

/// The format a name, an abbreviation or an extension stands for, and the
/// extension it is written with.
#[pyfunction]
#[pyo3(name = "findFormat")]
fn find_format(fmt: &str) -> (Option<&'static str>, Option<String>) {
    tools::find_format(fmt)
}

/// Every extension, dotted, a format reads, given by name or extension.
#[pyfunction]
#[pyo3(name = "findInputExtension")]
fn find_input_extension<'py>(py: Python<'py>, fmt: &str) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, tools::find_input_extension(fmt))
}

/// A path's suffix, as `pathlib.Path` reads it.
fn suffix(fp: &Bound<'_, PyAny>) -> PyResult<String> {
    let pathlib = fp.py().import("pathlib")?;
    let path_class = pathlib.getattr("Path")?;
    let path = if fp.is_instance(&path_class)? {
        fp.clone()
    } else {
        path_class.call1((fp,))?
    };
    path.getattr("suffix")?.extract()
}

/// The format of a file, by its extension, or None.
#[pyfunction]
#[pyo3(name = "findFormatFile")]
fn find_format_file(fp: &Bound<'_, PyAny>) -> PyResult<Option<&'static str>> {
    Ok(tools::find_format(&suffix(fp)?).0)
}

/// The format of a file and the extension it has, or two Nones.
#[pyfunction]
#[pyo3(name = "findFormatExtFile")]
fn find_format_ext_file(fp: &Bound<'_, PyAny>) -> PyResult<(Option<&'static str>, Option<String>)> {
    let suffix = suffix(fp)?;
    match tools::find_format(&suffix).0 {
        Some(format) => Ok((Some(format), Some(suffix))),
        None => Ok((None, None)),
    }
}

/// The format and extension a URL names, or two Nones.
#[pyfunction]
#[pyo3(name = "findFormatExtURL")]
fn find_format_ext_url(url: &str) -> (Option<&'static str>, Option<String>) {
    tools::find_format_ext_url(url)
}

/// `music21.common.formats`, built here.
pub(super) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "music21.common.formats")?;
    module.add(
        "VALID_SHOW_FORMATS",
        PyList::new(py, tools::VALID_SHOW_FORMATS)?,
    )?;
    module.add(
        "VALID_WRITE_FORMATS",
        PyList::new(py, tools::VALID_WRITE_FORMATS)?,
    )?;
    module.add(
        "VALID_AUTO_DOWNLOAD",
        PyList::new(py, tools::VALID_AUTO_DOWNLOAD)?,
    )?;
    module.add_function(wrap_pyfunction!(find_sub_converter_for_format, &module)?)?;
    module.add_function(wrap_pyfunction!(find_format, &module)?)?;
    module.add_function(wrap_pyfunction!(find_input_extension, &module)?)?;
    module.add_function(wrap_pyfunction!(find_format_file, &module)?)?;
    module.add_function(wrap_pyfunction!(find_format_ext_file, &module)?)?;
    module.add_function(wrap_pyfunction!(find_format_ext_url, &module)?)?;
    module.add("__all__", ALL.to_vec())?;
    Ok(module)
}
