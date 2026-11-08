//! music21's `common.stringTools`, over the crate's
//! `common::stringtools`.

use pyo3::basic::CompareOp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes, PyDict, PyList, PyString, PyTuple};
use pyo3::wrap_pyfunction;

use music21::common::stringtools as tools;

use super::value_error;

/// music21's `stringTools.__all__`.
const ALL: [&str; 12] = [
    "whitespaceEqual",
    "getNumFromStr",
    "hyphenToCamelCase",
    "camelCaseToHyphen",
    "spaceCamelCase",
    "getMd5",
    "formatStr",
    "stripAccents",
    "normalizeFilename",
    "removePunctuation",
    "parenthesesMatch",
    "ParenthesesMatch",
];

/// Whether two strings are equal but for their white space.
#[pyfunction]
#[pyo3(name = "whitespaceEqual")]
fn whitespace_equal(a: &str, b: &str) -> bool {
    tools::whitespace_equal(a, b)
}

/// The characters of a string among `numbers`, and the rest, in order.
#[pyfunction]
#[pyo3(name = "getNumFromStr", signature = (usrStr, numbers = "0123456789"))]
#[allow(non_snake_case)]
fn get_num_from_str(usrStr: &str, numbers: &str) -> (String, String) {
    tools::get_num_from_str(usrStr, numbers)
}

/// A `replacement`-joined name as a camel-cased one.
#[pyfunction]
#[pyo3(name = "hyphenToCamelCase", signature = (usrStr, replacement = "-"))]
#[allow(non_snake_case)]
fn hyphen_to_camel_case(usrStr: &str, replacement: &str) -> PyResult<String> {
    tools::hyphen_to_camel_case(usrStr, replacement).map_err(|error| value_error(&error))
}

/// A camel-cased name as lower-case words joined by `replacement`.
#[pyfunction]
#[pyo3(name = "camelCaseToHyphen", signature = (usrStr, replacement = "-"))]
#[allow(non_snake_case)]
fn camel_case_to_hyphen(usrStr: &str, replacement: &str) -> PyResult<String> {
    tools::camel_case_to_hyphen_checked(usrStr, replacement).map_err(|error| value_error(&error))
}

/// A camel-cased name, or one of letters and numbers, spaced into words.
#[pyfunction]
#[pyo3(
    name = "spaceCamelCase",
    signature = (usrStr, replaceUnderscore = true, fixMeList = None)
)]
#[allow(non_snake_case)]
fn space_camel_case(
    usrStr: &str,
    replaceUnderscore: bool,
    fixMeList: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let kept: Vec<String> = match fixMeList {
        Some(listed) => listed
            .try_iter()?
            .map(|item| item.and_then(|item| item.extract::<String>()))
            .collect::<PyResult<_>>()?,
        None => vec!["PMFC".to_string()],
    };
    let kept: Vec<&str> = kept.iter().map(String::as_str).collect();
    Ok(tools::space_camel_case(usrStr, replaceUnderscore, &kept))
}

/// The MD5 digest of a string's UTF-8 or of bytes, in hexadecimal; of the
/// time and a random number where nothing is given.
#[pyfunction]
#[pyo3(name = "getMd5", signature = (value = None))]
fn get_md5(py: Python<'_>, value: Option<&Bound<'_, PyAny>>) -> PyResult<String> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        let now = py.import("time")?.call_method0("time")?.str()?;
        let draw = py.import("random")?.call_method0("random")?.str()?;
        return Ok(tools::md5_hex(format!("{now}{draw}").as_bytes()));
    };
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(tools::md5_hex(text.to_str()?.as_bytes()));
    }
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(tools::md5_hex(bytes.as_bytes()));
    }
    if let Ok(bytes) = value.cast::<PyByteArray>() {
        return Ok(tools::md5_hex(&bytes.to_vec()));
    }
    let builtins = py.import("builtins")?;
    let viewed = builtins
        .getattr("memoryview")?
        .call1((value,))
        .map_err(|_| PyTypeError::new_err("object supporting the buffer API required"))?;
    let bytes: Vec<u8> = viewed.call_method0("tobytes")?.extract()?;
    Ok(tools::md5_hex(&bytes))
}

/// DEPRECATED: do not use. May be removed at any time.
///
/// The values, strings as they are and anything else as its `repr`,
/// joined by spaces with a line break after.
#[pyfunction]
#[pyo3(name = "formatStr", signature = (msg, *rest_of_message, **keywords))]
fn format_str(
    msg: &Bound<'_, PyAny>,
    rest_of_message: &Bound<'_, PyTuple>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<String> {
    let _ = keywords;
    let mut written = Vec::new();
    for value in std::iter::once(msg.clone()).chain(rest_of_message.iter()) {
        if let Ok(text) = value.cast::<PyString>() {
            written.push(text.to_str()?.to_string());
            continue;
        }
        // music21 decodes bytes first, which can fail, and then writes
        // their `repr` all the same.
        if value.is_instance_of::<PyBytes>() {
            value.call_method1("decode", ("utf-8",))?;
        }
        match value.repr() {
            Ok(repr) => written.push(repr.to_str()?.to_string()),
            Err(error) if error.is_instance_of::<PyTypeError>(value.py()) => {
                match value.call_method1("decode", ("utf-8",)) {
                    Ok(decoded) => written.push(decoded.extract()?),
                    Err(_) => written.push(String::new()),
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(tools::format_str(&written))
}

/// Text with its accents taken off; `ß` becomes `ss` and curly quotes
/// straight ones.
#[pyfunction]
#[pyo3(name = "stripAccents")]
#[allow(non_snake_case)]
fn strip_accents(inputString: &str) -> String {
    tools::strip_accents(inputString)
}

/// A name made a safe POSIX file name, its accents stripped.
#[pyfunction]
#[pyo3(name = "normalizeFilename")]
fn normalize_filename(name: &str) -> String {
    tools::normalize_filename(name)
}

/// Text with all of ASCII's punctuation taken out.
#[pyfunction]
#[pyo3(name = "removePunctuation")]
fn remove_punctuation(s: &str) -> String {
    tools::remove_punctuation(s)
}

/// music21's `ParenthesesMatch` dataclass: where a bracketed stretch of
/// text starts and ends, the text, and the matches nested in it.
#[pyclass(
    name = "ParenthesesMatch",
    module = "music21.common.stringTools",
    get_all,
    set_all
)]
pub(crate) struct ParenthesesMatch {
    start: Py<PyAny>,
    end: Py<PyAny>,
    text: Py<PyAny>,
    nested: Py<PyAny>,
}

#[pymethods]
impl ParenthesesMatch {
    #[new]
    fn new(start: Py<PyAny>, end: Py<PyAny>, text: Py<PyAny>, nested: Py<PyAny>) -> Self {
        Self {
            start,
            end,
            text,
            nested,
        }
    }

    #[classattr]
    fn __match_args__() -> (&'static str, &'static str, &'static str, &'static str) {
        ("start", "end", "text", "nested")
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!(
            "ParenthesesMatch(start={}, end={}, text={}, nested={})",
            self.start.bind(py).repr()?,
            self.end.bind(py).repr()?,
            self.text.bind(py).repr()?,
            self.nested.bind(py).repr()?
        ))
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other) = other.cast::<Self>() else {
            return Ok(py.NotImplemented());
        };
        let fields =
            |found: &Self| PyTuple::new(py, [&found.start, &found.end, &found.text, &found.nested]);
        let mine = fields(self)?;
        let theirs = fields(&other.borrow())?;
        match op {
            CompareOp::Eq => Ok(mine
                .eq(theirs)?
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind()),
            CompareOp::Ne => Ok(mine
                .ne(theirs)?
                .into_pyobject(py)?
                .to_owned()
                .into_any()
                .unbind()),
            _ => Ok(py.NotImplemented()),
        }
    }
}

fn matched(py: Python<'_>, found: tools::ParenthesesMatch) -> PyResult<Py<ParenthesesMatch>> {
    let nested = PyList::empty(py);
    for inner in found.nested {
        nested.append(matched(py, inner)?)?;
    }
    Py::new(
        py,
        ParenthesesMatch {
            start: found.start.into_pyobject(py)?.into_any().unbind(),
            end: found.end.into_pyobject(py)?.into_any().unbind(),
            text: found.text.into_pyobject(py)?.into_any().unbind(),
            nested: nested.into_any().unbind(),
        },
    )
}

/// Every bracketed stretch of a string as a `ParenthesesMatch`, those
/// nested in it inside it. A bracket after a backslash does not count.
#[pyfunction]
#[pyo3(name = "parenthesesMatch", signature = (s, open = "(", close = ")"))]
fn parentheses_match(
    py: Python<'_>,
    s: &str,
    open: &str,
    close: &str,
) -> PyResult<Vec<Py<ParenthesesMatch>>> {
    tools::parentheses_match(s, open, close)
        .map_err(|error| value_error(&error))?
        .into_iter()
        .map(|found| matched(py, found))
        .collect()
}

/// `music21.common.stringTools`, built here.
pub(super) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let module = PyModule::new(py, "music21.common.stringTools")?;
    let re = py.import("re")?;
    module.add("WHITESPACE", re.call_method1("compile", (r"\s+",))?)?;
    module.add("LINEFEED", re.call_method1("compile", ("\n+",))?)?;
    module.add_function(wrap_pyfunction!(whitespace_equal, &module)?)?;
    module.add_function(wrap_pyfunction!(get_num_from_str, &module)?)?;
    module.add_function(wrap_pyfunction!(hyphen_to_camel_case, &module)?)?;
    module.add_function(wrap_pyfunction!(camel_case_to_hyphen, &module)?)?;
    module.add_function(wrap_pyfunction!(space_camel_case, &module)?)?;
    module.add_function(wrap_pyfunction!(get_md5, &module)?)?;
    module.add_function(wrap_pyfunction!(format_str, &module)?)?;
    module.add_function(wrap_pyfunction!(strip_accents, &module)?)?;
    module.add_function(wrap_pyfunction!(normalize_filename, &module)?)?;
    module.add_function(wrap_pyfunction!(remove_punctuation, &module)?)?;
    module.add_function(wrap_pyfunction!(parentheses_match, &module)?)?;
    module.add_class::<ParenthesesMatch>()?;
    module.add("__all__", ALL.to_vec())?;
    Ok(module)
}
