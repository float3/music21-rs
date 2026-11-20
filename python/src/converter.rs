//! music21's `converter`, over the crate's readers and writers.
//!
//! `parse` takes what music21's `converter.parse` takes -- a path, the text or
//! bytes of a document, a `(path, number)` pair, a URL -- tells the format from
//! the file's extension or the document's first characters as music21 does,
//! and reads it with the wheel's reader for that format: MusicXML (a `.mxl`
//! unzipped first), ABC, MIDI, Humdrum, MEI, RomanText, de Clercq-Temperley,
//! TinyNotation, Volpiano, Capella, NoteWorthy (text and binary), MuseData
//! (a file, a directory or a zip of a work's parts), and MuseScore's `.mscx`
//! and `.mscz`, which music21 does not read itself.
//!
//! On its own, the wheel hands back its own streams, and needs no music21.
//! Once `install_into_music21` has run, these are music21's own `parse`,
//! `parseFile`, `parseData`, `parseURL` and `toData`, reading what
//! `install_into_music21` routes through the crate and the rest with
//! music21's readers, its pickles included; MuseScore's files, which music21
//! has no reader for, are still read by the crate, into music21's streams.
//!
//! There is no cache: every parse reads the file. `forceSource` and
//! `storePickle` are taken and mean nothing. A keyword the crate's reader
//! cannot honour -- MIDI's `quarterLengthDivisors`, TinyNotation's
//! `makeNotation=False` -- raises `ConverterException` rather than being
//! ignored.
//!
//! `ConverterException` and `ConverterFileException` are music21's own where
//! music21 can be imported, as the wheel's other exceptions are built on
//! music21's, and stand-ins of their own where it cannot. They are made on
//! first asking, so importing this module imports no music21.
//!
//! The files, archives and downloads are read with Python's own `pathlib`,
//! `zipfile` and `urllib`, so that a path, an archive or a URL means what it
//! means to the caller's Python.

use pyo3::exceptions::{PyAttributeError, PyFileNotFoundError, PyUnicodeDecodeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyCFunction, PyDict, PyInt, PyList, PySlice, PyString, PyTuple};

use crate::plumbing::built::with_context;
use crate::provide;

/// The module's dotted name.
const MODULE: &str = "music21_rs.converter";

/// Where music21 keeps the module's exceptions.
const MUSIC21_MODULE: &str = "music21.converter";

/// The keywords every reader takes, for the cache the wheel does not keep.
const CACHE_KEYWORDS: [&str; 2] = ["forceSource", "storePickle"];

/// The module's public names.
const ALL: [&str; 11] = [
    "ConverterException",
    "ConverterFileException",
    "freeze",
    "freezeStr",
    "parse",
    "parseData",
    "parseFile",
    "parseURL",
    "thaw",
    "thawStr",
    "toData",
];

/// A zip archive's first bytes.
const ZIP_MAGIC: &[u8] = b"PK";

/// A binary NoteWorthy file's first bytes.
const NWC_MAGIC: &[u8] = b"[NWZ]";

/// How many characters a format header is looked for in.
const HEADER_LENGTH: isize = 20;

/// How many characters a MuseScore document's root is looked for in.
const MUSESCORE_LENGTH: usize = 1000;

/// How many bytes of a `.mxl`'s document its encoding is looked for in.
const ENCODING_LENGTH: usize = 1000;

/// The crate's reader a format is read with.
#[derive(Clone, Copy, PartialEq)]
enum Reader {
    Abc,
    Capella,
    ClercqTemperley,
    Humdrum,
    Mei,
    Midi,
    MuseData,
    MuseScore,
    MusicXml,
    NoteworthyText,
    Noteworthy,
    RomanText,
    TinyNotation,
    Volpiano,
}

/// Whether a format's files are read as bytes or as text.
#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    Binary,
    Text,
}

/// A keyword's value the crate's reader always reads with.
#[derive(Clone, Copy)]
enum Fixed {
    Bool(bool),
    Str(&'static str),
}

/// One format the wheel reads: music21's names for it, the extensions of
/// its files, the reader, whether its files are read as bytes, and the
/// keywords taken at the value the crate's reader always reads with.
struct Format {
    formats: &'static [&'static str],
    extensions: &'static [&'static str],
    reader: Reader,
    encoding: Encoding,
    defaults: &'static [(&'static str, Fixed)],
}

impl Format {
    /// The format's own name, music21's first for it.
    fn name(&self) -> &'static str {
        self.formats[0]
    }
}

/// The formats, in the order music21 asks its subconverters, which is by
/// class name, so a header such as `xml:` finds the format music21 finds.
const FORMATS: [Format; 14] = [
    text_format(&["abc"], &["abc"], Reader::Abc),
    binary_format(&["capella"], &["capx"], Reader::Capella),
    text_format(
        &["cttxt", "har", "clercqtemperley"],
        &["cttxt", "har", "clercqtemperley"],
        Reader::ClercqTemperley,
    ),
    text_format(&["humdrum"], &["krn"], Reader::Humdrum),
    text_format(&["mei"], &["mei"], Reader::Mei),
    Format {
        formats: &["midi"],
        extensions: &["mid", "midi"],
        reader: Reader::Midi,
        encoding: Encoding::Binary,
        defaults: &[
            ("quantizePost", Fixed::Bool(true)),
            ("encoding", Fixed::Str("")),
        ],
    },
    text_format(&["musedata"], &["md", "musedata", "zip"], Reader::MuseData),
    binary_format(&["musescore"], &["mscx", "mscz"], Reader::MuseScore),
    binary_format(
        &["musicxml", "xml"],
        &["xml", "mxl", "musicxml"],
        Reader::MusicXml,
    ),
    text_format(&["noteworthytext"], &["nwctxt"], Reader::NoteworthyText),
    binary_format(&["noteworthy"], &["nwc"], Reader::Noteworthy),
    text_format(
        &["romantext", "rntext"],
        &["rntxt", "rntext", "romantext", "rtxt"],
        Reader::RomanText,
    ),
    Format {
        formats: &["tinynotation"],
        extensions: &["tntxt", "tinynotation"],
        reader: Reader::TinyNotation,
        encoding: Encoding::Text,
        defaults: &[("makeNotation", Fixed::Bool(true))],
    },
    text_format(&["volpiano"], &["volpiano", "vp"], Reader::Volpiano),
];

/// The formats music21 has no reader for, which the crate reads into
/// music21's streams even once installed.
const WHEEL_ONLY: [Reader; 1] = [Reader::MuseScore];

/// What a stream can be written as, by the name and the extensions music21
/// gives each format, and how what is written is read back.
const WRITTEN: [(&str, Encoding); 15] = [
    ("musicxml", Encoding::Text),
    ("xml", Encoding::Text),
    ("midi", Encoding::Binary),
    ("mid", Encoding::Binary),
    ("text", Encoding::Text),
    ("txt", Encoding::Text),
    ("t", Encoding::Text),
    ("textline", Encoding::Text),
    ("lilypond", Encoding::Text),
    ("lily", Encoding::Text),
    ("braille", Encoding::Text),
    ("romantext", Encoding::Text),
    ("rntxt", Encoding::Text),
    ("volpiano", Encoding::Text),
    ("abc", Encoding::Text),
];

const fn text_format(
    formats: &'static [&'static str],
    extensions: &'static [&'static str],
    reader: Reader,
) -> Format {
    Format {
        formats,
        extensions,
        reader,
        encoding: Encoding::Text,
        defaults: &[],
    }
}

const fn binary_format(
    formats: &'static [&'static str],
    extensions: &'static [&'static str],
    reader: Reader,
) -> Format {
    Format {
        formats,
        extensions,
        reader,
        encoding: Encoding::Binary,
        defaults: &[],
    }
}

/// One of the formats, as `_data_format` hands it out.
#[pyclass(frozen, name = "_Format", module = "music21_rs.converter")]
struct FormatInfo {
    format: &'static Format,
}

#[pymethods]
impl FormatInfo {
    /// The format's own name, music21's first for it.
    #[getter]
    fn name(&self) -> &'static str {
        self.format.name()
    }

    /// music21's names for the format.
    #[getter]
    fn formats(&self) -> Vec<&'static str> {
        self.format.formats.to_vec()
    }

    /// The extensions of the format's files.
    #[getter]
    fn extensions(&self) -> Vec<&'static str> {
        self.format.extensions.to_vec()
    }

    /// Whether the format's files are read as bytes.
    #[getter]
    fn binary(&self) -> bool {
        self.format.encoding == Encoding::Binary
    }
}

/// The module, built once.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.setattr("__doc__", DOC)?;
        m.add("__all__", ALL.to_vec())?;
        m.add_function(wrap_pyfunction!(parse_file, &m)?)?;
        m.add_function(wrap_pyfunction!(parse_data, &m)?)?;
        m.add_function(wrap_pyfunction!(parse_url, &m)?)?;
        m.add_function(wrap_pyfunction!(parse, &m)?)?;
        m.add_function(wrap_pyfunction!(to_data, &m)?)?;
        m.add_function(wrap_pyfunction!(data_format, &m)?)?;
        m.add_function(wrap_pyfunction!(freeze, &m)?)?;
        m.add_function(wrap_pyfunction!(thaw, &m)?)?;
        m.add_function(wrap_pyfunction!(freeze_str, &m)?)?;
        m.add_function(wrap_pyfunction!(thaw_str, &m)?)?;
        m.add_class::<FormatInfo>()?;
        m.add(
            "__getattr__",
            exception_getattr(py, MODULE, &CONVERTER_EXCEPTIONS)?,
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}

/// The module's docstring: the first paragraph of this file's.
const DOC: &str = "music21's `converter`, over the crate's readers and writers.\n\n\
`parse` takes what music21's `converter.parse` takes -- a path, the text or \
bytes of a document, a `(path, number)` pair, a URL -- tells the format from \
the file's extension or the document's first characters as music21 does, and \
reads it with the wheel's reader for that format. Once `install_into_music21` \
has run, these are music21's own `parse`, `parseFile`, `parseData`, \
`parseURL` and `toData`.";

// --- Exceptions -------------------------------------------------------------

/// An exception made on first asking: music21's own where it can be
/// imported, so that code catching music21's catches it, and a plain
/// exception under music21's name where it cannot.
pub(crate) struct LazyException {
    name: &'static str,
    module: &'static str,
    made: PyOnceLock<Py<PyAny>>,
}

impl LazyException {
    pub(crate) const fn new(name: &'static str, module: &'static str) -> Self {
        Self {
            name,
            module,
            made: PyOnceLock::new(),
        }
    }

    /// The exception, made the first time it is asked for.
    pub(crate) fn get<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let made = self.made.get_or_try_init(py, || {
            music21_exception(py, self.name, self.module).map(Bound::unbind)
        })?;
        Ok(made.bind(py).clone())
    }

    /// The exception raised with `message`.
    pub(crate) fn err(&self, py: Python<'_>, message: String) -> PyErr {
        match self.get(py) {
            Ok(class) => match class.call1((message,)) {
                Ok(raised) => PyErr::from_value(raised),
                Err(error) => error,
            },
            Err(error) => error,
        }
    }
}

static CONVERTER_EXCEPTION: LazyException =
    LazyException::new("ConverterException", MUSIC21_MODULE);
static CONVERTER_FILE_EXCEPTION: LazyException =
    LazyException::new("ConverterFileException", MUSIC21_MODULE);
static CONVERTER_EXCEPTIONS: [&LazyException; 2] =
    [&CONVERTER_EXCEPTION, &CONVERTER_FILE_EXCEPTION];

/// music21's exception `name` from `module`, or a stand-in under its name:
/// built on the provided `Music21Exception` where `provide_music21()`
/// answers for music21, as music21's own are built on it.
fn music21_exception<'py>(
    py: Python<'py>,
    name: &str,
    module: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if provide::providing(py)? {
        return new_exception(py, name, module, provide::music21_exception(py)?);
    }
    let found = py
        .import("importlib")?
        .call_method1("import_module", (module,))
        .and_then(|imported| imported.getattr(name));
    match found {
        Ok(found) => Ok(found),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyException>(py) => new_exception(
            py,
            name,
            module,
            py.get_type::<pyo3::exceptions::PyException>().into_any(),
        ),
        Err(error) => Err(error),
    }
}

/// `type(name, (base,), {"__module__": module})`.
fn new_exception<'py>(
    py: Python<'py>,
    name: &str,
    module: &str,
    base: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let namespace = PyDict::new(py);
    namespace.set_item("__module__", module)?;
    py.import("builtins")?
        .getattr("type")?
        .call1((name, PyTuple::new(py, [base])?, namespace))
}

/// A module's `__getattr__` handing out its exceptions on first asking.
pub(crate) fn exception_getattr<'py>(
    py: Python<'py>,
    module: &'static str,
    exceptions: &'static [&'static LazyException],
) -> PyResult<Bound<'py, PyCFunction>> {
    PyCFunction::new_closure(
        py,
        Some(c"__getattr__"),
        None,
        move |args: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<Py<PyAny>> {
            let py = args.py();
            let name: String = args.get_item(0)?.extract()?;
            if let Some(exception) = exceptions.iter().find(|each| each.name == name) {
                return Ok(exception.get(py)?.unbind());
            }
            Err(PyAttributeError::new_err(format!(
                "module {} has no attribute {}",
                PyString::new(py, module).repr()?,
                PyString::new(py, &name).repr()?
            )))
        },
    )
}

// --- Python interop ---------------------------------------------------------

/// A function of the wheel's, by name.
fn wheel<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    provide::extension(py)?.getattr(name)
}

/// The bytes of `data`, where it is bytes.
fn bytes_of<'a>(data: &'a Bound<'_, PyAny>) -> Option<&'a [u8]> {
    data.cast::<PyBytes>().ok().map(|bytes| bytes.as_bytes())
}

/// `data[start:]`.
fn tail<'py>(data: &Bound<'py, PyAny>, start: isize) -> PyResult<Bound<'py, PyAny>> {
    data.get_item(PySlice::new(data.py(), start, isize::MAX, 1))
}

/// `data[:end]`.
fn head<'py>(data: &Bound<'py, PyAny>, end: isize) -> PyResult<Bound<'py, PyAny>> {
    data.get_item(PySlice::new(data.py(), 0, end, 1))
}

/// `bytes.decode(encoding, errors)`.
fn decode<'py>(
    data: &Bound<'py, PyAny>,
    encoding: &str,
    errors: &str,
) -> PyResult<Bound<'py, PyAny>> {
    data.call_method1("decode", (encoding, errors))
}

/// A document's text, from text or from the bytes of one.
fn text<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let Some(bytes) = bytes_of(data) else {
        return Ok(data.clone());
    };
    if bytes.starts_with(b"\xff\xfe") || bytes.starts_with(b"\xfe\xff") {
        return decode(data, "utf-16", "strict");
    }
    match decode(data, "utf-8-sig", "strict") {
        Err(error) if error.is_instance_of::<PyUnicodeDecodeError>(data.py()) => {
            decode(data, "latin-1", "strict")
        }
        decoded => decoded,
    }
}

/// Runs `body` on the zip archive `source` holds, a path or a file, and
/// closes it, as `with zipfile.ZipFile(source)` does.
fn with_zip<'py, T>(
    source: &Bound<'py, PyAny>,
    body: impl FnOnce(&Bound<'py, PyAny>) -> PyResult<T>,
) -> PyResult<T> {
    let py = source.py();
    let archive = py.import("zipfile")?.getattr("ZipFile")?.call1((source,))?;
    let done = with_context(&archive, body)?;
    done.ok_or_else(|| {
        pyo3::exceptions::PyRuntimeError::new_err("the archive's exit swallowed an error")
    })
}

/// The bytes of a document as a file Python can read a zip from.
fn bytes_file<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    data.py().import("io")?.getattr("BytesIO")?.call1((data,))
}

/// The names in a zip archive, in its order.
fn zip_names(archive: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    archive.call_method0("namelist")?.extract()
}

/// A `pathlib.Path`.
fn path_of<'py>(py: Python<'py>, value: impl IntoPyObject<'py>) -> PyResult<Bound<'py, PyAny>> {
    py.import("pathlib")?.getattr("Path")?.call1((value,))
}

/// `os.path.expanduser`.
fn expand_user<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    value
        .py()
        .import("os.path")?
        .call_method1("expanduser", (value,))
}

/// Whether a path names something that exists; a path Python cannot ask
/// about does not.
fn can_load(path: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = path.py();
    match py.import("os.path")?.call_method1("exists", (path,)) {
        Ok(exists) => exists.is_truthy(),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyValueError>(py) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Whether music21's `Note` is the wheel's: the wheel has then been
/// installed into music21.
pub(crate) fn installed(py: Python<'_>) -> PyResult<bool> {
    if provide::providing(py)? {
        return Ok(false);
    }
    let note = py
        .import("sys")?
        .getattr("modules")?
        .call_method1("get", ("music21.note",))?;
    if note.is_none() {
        return Ok(false);
    }
    let Some(class) = provide::attribute(&note, "Note")? else {
        return Ok(false);
    };
    let Ok(class) = class.cast_into::<pyo3::types::PyType>() else {
        return Ok(false);
    };
    class.is_subclass(&wheel(py, "Note")?)
}

/// music21's own `converter`.
fn music21_converter(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    py.import(MUSIC21_MODULE)
}

/// The keywords with those the caller gave added.
fn with_keywords<'py>(
    py: Python<'py>,
    named: &[(&str, Option<&Bound<'py, PyAny>>)],
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let all = PyDict::new(py);
    for (name, value) in named {
        match value {
            Some(value) => all.set_item(name, value)?,
            None => all.set_item(name, py.None())?,
        }
    }
    if let Some(keywords) = keywords {
        all.update(keywords.as_mapping())?;
    }
    Ok(all)
}

// --- Formats ----------------------------------------------------------------

/// The format music21 would choose for `name`, a format or an extension,
/// as its `Converter.regularizeFormat` chooses one.
fn format_named(name: &Bound<'_, PyAny>) -> PyResult<Option<&'static Format>> {
    let cleaned: String = name
        .call_method0("lower")?
        .call_method0("strip")?
        .call_method1("lstrip", (".",))?
        .extract()?;
    let cleaned = cleaned.split('.').next().unwrap_or("");
    if let Some(found) = FORMATS.iter().find(|each| each.formats.contains(&cleaned)) {
        return Ok(Some(found));
    }
    Ok(FORMATS
        .iter()
        .find(|each| each.extensions.contains(&cleaned)))
}

/// The format named, by its own name.
fn format_called(name: &str) -> &'static Format {
    FORMATS
        .iter()
        .find(|each| each.name() == name)
        .expect("a format of the table")
}

/// [`format_named`] of a name that may be absent.
fn found_or_none(name: Option<&Bound<'_, PyAny>>) -> PyResult<Option<&'static Format>> {
    match name {
        Some(name) => format_named(name),
        None => Ok(None),
    }
}

/// The format `name` names, or `ConverterException`.
fn format_given(name: &Bound<'_, PyAny>) -> PyResult<&'static Format> {
    if let Some(found) = format_named(name)? {
        return Ok(found);
    }
    Err(CONVERTER_EXCEPTION.err(
        name.py(),
        format!("no converter available for format: {}", name.str()?),
    ))
}

/// The format a header such as `tinyNotation:` names, and the data after
/// it, as music21's `Converter.formatFromHeader` finds them.
fn format_from_header<'py>(
    data: &Bound<'py, PyAny>,
) -> PyResult<(Option<&'static Format>, Bound<'py, PyAny>)> {
    let start: String = if bytes_of(data).is_some() {
        decode(&head(data, HEADER_LENGTH)?, "utf-8", "ignore")?
            .call_method0("lower")?
            .extract()?
    } else if data.is_instance_of::<PyString>() {
        head(data, HEADER_LENGTH)?
            .call_method0("lower")?
            .extract()?
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Cannot parse a format from {}.",
            data.get_type().str()?
        )));
    };
    for each in &FORMATS {
        for name in each.formats {
            if start.starts_with(&format!("{name}:")) {
                let after = tail(data, name.chars().count() as isize + 1)?;
                return Ok((Some(each), after.call_method0("lstrip")?));
            }
        }
    }
    Ok((None, data.clone()))
}

/// The format of a document by its first characters, as music21's
/// `Converter.parseData` tells one; with a zip's and NoteWorthy's, which
/// music21 does not tell.
fn format_from_content(data: &Bound<'_, PyAny>) -> PyResult<&'static Format> {
    let py = data.py();
    if let Some(bytes) = bytes_of(data) {
        if bytes.starts_with(ZIP_MAGIC) {
            return format_of_zip(data);
        }
        if bytes.starts_with(NWC_MAGIC) {
            return Ok(format_called("noteworthy"));
        }
    }
    let text: String = match bytes_of(data) {
        Some(_) => decode(data, "utf-8", "ignore")?.extract()?,
        None => data.extract()?,
    };
    let lowered = text.to_lowercase();
    if text.starts_with("<?xml") || text.starts_with("<museScore") {
        let opening: String = text.chars().take(MUSESCORE_LENGTH).collect();
        if opening.contains("<museScore") {
            return Ok(format_called("musescore"));
        }
        if text.contains("<mei") {
            return Ok(format_called("mei"));
        }
        return Ok(format_called("musicxml"));
    }
    if lowered.starts_with("mei:") {
        return Ok(format_called("mei"));
    }
    if lowered.starts_with("musicxml:") {
        return Ok(format_called("musicxml"));
    }
    if text.starts_with("MThd") || lowered.starts_with("midi:") {
        return Ok(format_called("midi"));
    }
    if text.starts_with("!!!") || text.starts_with("**") || lowered.starts_with("humdrum:") {
        return Ok(format_called("humdrum"));
    }
    if lowered.starts_with("tinynotation:") {
        return Ok(format_called("tinynotation"));
    }
    if text.starts_with("!NoteWorthyComposer") {
        return Ok(format_called("noteworthytext"));
    }
    // MuseData has to state a meter and a key.
    if text.contains("WK#:") && text.contains("measure") {
        return Ok(format_called("musedata"));
    }
    if text.contains("M:") && text.contains("K:") {
        return Ok(format_called("abc"));
    }
    if text.contains("Time Signature:") && text.contains("m1") {
        return Ok(format_called("romantext"));
    }
    Err(CONVERTER_EXCEPTION.err(
        py,
        format!("File not found or no such format found for: {text}"),
    ))
}

/// The format of a zip by what it holds.
fn format_of_zip(data: &Bound<'_, PyAny>) -> PyResult<&'static Format> {
    let names = with_zip(&bytes_file(data)?, zip_names)?;
    if names.iter().any(|name| name == "score.xml") {
        return Ok(format_called("capella"));
    }
    if names.iter().any(|name| name.ends_with(".mscx")) {
        return Ok(format_called("musescore"));
    }
    let musicxml = names.iter().any(|name| {
        name.starts_with("META-INF") || name.ends_with(".xml") || name.ends_with(".musicxml")
    });
    if musicxml {
        return Ok(format_called("musicxml"));
    }
    Ok(format_called("musedata"))
}

/// The format of a document, told as `parseData` tells it.
fn format_of_data(
    data: &Bound<'_, PyAny>,
    format: Option<&Bound<'_, PyAny>>,
) -> PyResult<&'static Format> {
    if let Some(format) = format {
        return format_given(format);
    }
    let stripped = data.call_method0("lstrip")?;
    let (found, stripped) = format_from_header(&stripped)?;
    match found {
        Some(found) => Ok(found),
        None => format_from_content(&stripped),
    }
}

/// The format `parseData` would read `data` in, or None where it would
/// raise and `quiet` is set.
#[pyfunction]
#[pyo3(name = "_data_format", signature = (data, format, quiet=false))]
fn data_format(
    data: &Bound<'_, PyAny>,
    format: Option<&Bound<'_, PyAny>>,
    quiet: bool,
) -> PyResult<Option<FormatInfo>> {
    let quiet = if quiet {
        Failure::Quiet
    } else {
        Failure::Raised
    };
    Ok(told_format(data, format, quiet)?.map(|format| FormatInfo { format }))
}

/// What a failure to tell a format does.
#[derive(Clone, Copy, PartialEq)]
enum Failure {
    Quiet,
    Raised,
}

/// [`format_of_data`], an `Exception` raised or answered with None.
fn told_format(
    data: &Bound<'_, PyAny>,
    format: Option<&Bound<'_, PyAny>>,
    failure: Failure,
) -> PyResult<Option<&'static Format>> {
    let py = data.py();
    match format_of_data(data, format) {
        Ok(found) => Ok(Some(found)),
        Err(error)
            if failure == Failure::Quiet
                && error.is_instance_of::<pyo3::exceptions::PyException>(py) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Whether the format is one music21 has no reader for.
fn wheel_only(found: Option<&Format>) -> bool {
    found.is_some_and(|found| WHEEL_ONLY.contains(&found.reader))
}

/// Refuses a keyword the crate's reader of `found` does not honour.
fn checked(found: &Format, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let Some(keywords) = keywords else {
        return Ok(());
    };
    let py = keywords.py();
    let mut refused = Vec::new();
    for (name, value) in keywords.iter() {
        let name: String = name.extract()?;
        if CACHE_KEYWORDS.contains(&name.as_str()) {
            continue;
        }
        let fixed = found.defaults.iter().find(|(each, _)| *each == name);
        let honoured = match fixed {
            Some((_, Fixed::Bool(fixed))) => value.eq(*fixed)?,
            Some((_, Fixed::Str(fixed))) => value.eq(*fixed)?,
            None => false,
        };
        if !honoured {
            refused.push(name);
        }
    }
    if refused.is_empty() {
        return Ok(());
    }
    refused.sort();
    Err(CONVERTER_EXCEPTION.err(
        py,
        format!(
            "the wheel's {} reader takes no keyword {} at that value; install the wheel \
             into music21 for music21's reader",
            found.name(),
            refused.join(", ")
        ),
    ))
}

// --- Reading ----------------------------------------------------------------

/// `data` read by the format's reader; `number` picks a tune where the
/// format holds several.
fn read<'py>(
    format: &Format,
    data: &Bound<'py, PyAny>,
    number: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    match format.reader {
        Reader::Abc => match number {
            None => wheel(py, "from_abc")?.call1((text(data)?,)),
            Some(number) => wheel(py, "from_abc_number")?.call1((text(data)?, number)),
        },
        Reader::Capella => wheel(py, "from_capella")?.call1((data,)),
        Reader::ClercqTemperley => wheel(py, "from_clercq_temperley")?.call1((text(data)?,)),
        Reader::Humdrum => wheel(py, "from_humdrum")?.call1((text(data)?,)),
        Reader::Mei => read_mei(data),
        Reader::Midi => read_midi(data),
        Reader::MuseData => read_musedata(data),
        Reader::MuseScore => read_mscx(data),
        Reader::MusicXml => read_musicxml(data),
        Reader::NoteworthyText => wheel(py, "from_noteworthy")?.call1((text(data)?,)),
        Reader::Noteworthy => wheel(py, "from_nwc")?.call1((data,)),
        Reader::RomanText => wheel(py, "from_roman_text")?.call1((text(data)?,)),
        Reader::TinyNotation => wheel(py, "from_tiny_notation")?.call1((text(data)?,)),
        Reader::Volpiano => wheel(py, "from_volpiano")?.call1((text(data)?,)),
    }
}

fn read_midi<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    let data = if data.is_instance_of::<PyString>() {
        data.call_method1("encode", ("latin-1",))?
    } else {
        data.clone()
    };
    wheel(py, "from_midi")?.call1((data,))
}

fn read_mei<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    let mut text = text(data)?;
    let prefix: String = head(&text, 4)?.call_method0("lower")?.extract()?;
    if prefix == "mei:" {
        text = tail(&text, 4)?;
    }
    wheel(py, "from_mei")?.call1((text,))
}

fn read_musedata<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    if data.is_instance_of::<PyList>() || data.is_instance_of::<PyTuple>() {
        let parts = data
            .try_iter()?
            .map(|each| text(&each?))
            .collect::<PyResult<Vec<_>>>()?;
        return wheel(py, "from_musedata")?.call1((PyList::new(py, parts)?,));
    }
    wheel(py, "from_musedata")?.call1((text(data)?,))
}

fn read_mscx<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    let mut data = data.clone();
    if bytes_of(&data).is_some_and(|bytes| bytes.starts_with(ZIP_MAGIC)) {
        data = with_zip(&bytes_file(&data)?, |archive| {
            let names = zip_names(archive)?;
            let member = names
                .iter()
                .find(|name| name.ends_with(".mscx"))
                .ok_or_else(|| pyo3::exceptions::PyStopIteration::new_err(()))?;
            archive.call_method1("read", (member,))
        })?;
    }
    wheel(py, "from_mscx")?.call1((text(&data)?,))
}

fn read_musicxml<'py>(data: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    let mut data = data.clone();
    if bytes_of(&data).is_some_and(|bytes| bytes.starts_with(ZIP_MAGIC)) {
        data = with_zip(&bytes_file(&data)?, musicxml_of_zip)?;
    }
    wheel(py, "from_musicxml")?.call1((text(&data)?,))
}

/// The document a `.mxl` holds, as music21's `ArchiveManager` finds it: the
/// first `.xml` or `.musicxml` outside `META-INF`.
fn musicxml_of_zip<'py>(archive: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = archive.py();
    for name in zip_names(archive)? {
        if name.contains("META-INF") {
            continue;
        }
        let suffix = posix_suffix(&name);
        if ![".musicxml", ".xml", ".mxl"].contains(&suffix) && name != ".xml" {
            continue;
        }
        let data = archive.call_method1("read", (&name,))?;
        let bytes = bytes_of(&data).unwrap_or_default();
        let opening = &bytes[..bytes.len().min(ENCODING_LENGTH)];
        let encoding = match declared_encoding(opening) {
            Some(found) => PyBytes::new(py, found).call_method1("decode", ("ascii",))?,
            None => PyString::new(py, "utf-8").into_any(),
        };
        return match data.call_method1("decode", (encoding,)) {
            Err(error) if error.is_instance_of::<PyUnicodeDecodeError>(py) => {
                decode(&data, "utf-16-le", "strict")
            }
            decoded => decoded,
        };
    }
    Err(CONVERTER_FILE_EXCEPTION.err(py, "no MusicXML document in the archive".to_owned()))
}

/// The encoding `encoding="..."` names at the very start of a document,
/// as `re.match(rb"encoding=['\"](\S*?)['\"]", ...)` finds it.
fn declared_encoding(opening: &[u8]) -> Option<&[u8]> {
    let rest = opening.strip_prefix(b"encoding=")?;
    let (quote, rest) = rest.split_first()?;
    if *quote != b'\'' && *quote != b'"' {
        return None;
    }
    for (index, byte) in rest.iter().enumerate() {
        if *byte == b'\'' || *byte == b'"' {
            return Some(&rest[..index]);
        }
        if b" \t\n\r\x0b\x0c".contains(byte) {
            return None;
        }
    }
    None
}

/// `PurePosixPath(name).suffix`.
fn posix_suffix(name: &str) -> &str {
    let last = name
        .split('/')
        .rfind(|part| !part.is_empty() && *part != ".")
        .unwrap_or("");
    match last.rfind('.') {
        Some(index) if index > 0 && index < last.len() - 1 => &last[index..],
        _ => "",
    }
}

/// What the reader of `found` reads out of the file at `path`.
fn file_data<'py>(path: &Bound<'py, PyAny>, found: &Format) -> PyResult<Bound<'py, PyAny>> {
    if found.reader == Reader::MuseData {
        return musedata_data(path);
    }
    let data = path.call_method0("read_bytes")?;
    if found.encoding == Encoding::Binary {
        return Ok(data);
    }
    text(&data)
}

/// The texts of a MuseData work's parts: a directory's, a zip's, or one
/// file's.
fn musedata_data<'py>(path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = path.py();
    if path.call_method0("is_dir")?.is_truthy()? {
        let mut listed: Vec<String> = py
            .import("os")?
            .call_method1("listdir", (path,))?
            .extract()?;
        listed.sort();
        let mut names = Vec::new();
        for name in listed {
            names.push(path.div(name)?.str()?.to_string());
        }
        let mut texts = Vec::new();
        for name in music21::musedata::part_paths(&names) {
            texts.push(text(&path_of(py, name)?.call_method0("read_bytes")?)?);
        }
        return Ok(PyList::new(py, texts)?.into_any());
    }

    let suffix: String = path.getattr("suffix")?.extract()?;
    let zipped = suffix == ".zip"
        || (suffix == ".md"
            && py
                .import("zipfile")?
                .call_method1("is_zipfile", (path,))?
                .is_truthy()?);
    if zipped {
        let texts = with_zip(path, |archive| {
            let mut texts = Vec::new();
            for name in music21::musedata::part_paths(&zip_names(archive)?) {
                texts.push(text(&archive.call_method1("read", (name,))?)?);
            }
            Ok(texts)
        })?;
        return Ok(PyList::new(py, texts)?.into_any());
    }
    let one = text(&path.call_method0("read_bytes")?)?;
    Ok(PyList::new(py, [one])?.into_any())
}

// --- music21's functions ----------------------------------------------------

/// Reads the file at `fp` into a stream, in the format its extension
/// names unless `format` names one, as music21's `converter.parseFile`
/// does. A directory is read as the parts of a MuseData work.
#[pyfunction]
#[pyo3(name = "parseFile", signature = (fp, number=None, format=None, forceSource=None, **keywords))]
#[allow(non_snake_case)]
fn parse_file<'py>(
    py: Python<'py>,
    fp: &Bound<'py, PyAny>,
    number: Option<&Bound<'py, PyAny>>,
    format: Option<&Bound<'py, PyAny>>,
    forceSource: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let path = path_of(py, expand_user(&fp.str()?.into_any())?)?;
    let suffix = path.getattr("suffix")?;
    if installed(py)? {
        let found = match found_or_none(format)? {
            Some(found) => Some(found),
            None => found_or_none(Some(&suffix))?,
        };
        if !wheel_only(found) {
            let force = force_source(py, forceSource);
            let all = with_keywords(
                py,
                &[
                    ("number", number),
                    ("format", format),
                    ("forceSource", Some(&force)),
                ],
                keywords,
            )?;
            return music21_converter(py)?.call_method("parseFile", (fp,), Some(&all));
        }
    }
    if !path.call_method0("exists")?.is_truthy()? {
        return Err(
            CONVERTER_FILE_EXCEPTION.err(py, format!("no such file exists: {}", path.str()?))
        );
    }

    // The format: the one asked for, a directory's, or the extension's.
    let found = if let Some(format) = format {
        format_given(format)?
    } else if path.call_method0("is_dir")?.is_truthy()? {
        format_called("musedata")
    } else {
        let named = if suffix.is_truthy()? {
            format_named(&suffix)?
        } else {
            None
        };
        named.ok_or_else(|| {
            CONVERTER_FILE_EXCEPTION.err(
                py,
                format!(
                    "cannot find format from file extensions for: {}",
                    path.str().map(|text| text.to_string()).unwrap_or_default()
                ),
            )
        })?
    };
    checked(found, keywords)?;
    read(found, &file_data(&path, found)?, number)
}

/// `forceSource` as given, False where it was not.
fn force_source<'py>(py: Python<'py>, given: Option<&Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    match given {
        Some(given) => given.clone(),
        None => pyo3::types::PyBool::new(py, false).to_owned().into_any(),
    }
}

/// Reads a document given as text or bytes into a stream, in the format
/// `format` names, else the one a header such as `tinyNotation:` names or
/// the document's first characters tell, as music21's
/// `converter.parseData` does.
#[pyfunction]
#[pyo3(name = "parseData", signature = (dataStr, number=None, format=None, **keywords))]
#[allow(non_snake_case)]
fn parse_data<'py>(
    py: Python<'py>,
    dataStr: &Bound<'py, PyAny>,
    number: Option<&Bound<'py, PyAny>>,
    format: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    if installed(py)? && !wheel_only(told_format(dataStr, format, Failure::Quiet)?) {
        let all = with_keywords(py, &[("number", number), ("format", format)], keywords)?;
        return music21_converter(py)?.call_method("parseData", (dataStr,), Some(&all));
    }
    let (found, data) = match format {
        Some(format) => (format_given(format)?, dataStr.clone()),
        None => {
            let stripped = dataStr.call_method0("lstrip")?;
            let (found, stripped) = format_from_header(&stripped)?;
            match found {
                Some(found) => (found, stripped),
                None => (format_from_content(&stripped)?, stripped),
            }
        }
    };
    checked(found, keywords)?;
    read(found, &data, number)
}

/// Downloads the document at `url` and reads it, in the format its
/// extension names, else as `parseData` tells one, as music21's
/// `converter.parseURL` does -- without the `autoDownload` setting music21
/// asks first, which the wheel does not keep.
#[pyfunction]
#[pyo3(name = "parseURL", signature = (url, *, format=None, number=None, forceSource=None, **keywords))]
#[allow(non_snake_case)]
fn parse_url<'py>(
    py: Python<'py>,
    url: &Bound<'py, PyAny>,
    format: Option<&Bound<'py, PyAny>>,
    number: Option<&Bound<'py, PyAny>>,
    forceSource: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    if installed(py)? {
        let force = force_source(py, forceSource);
        let all = with_keywords(
            py,
            &[
                ("format", format),
                ("number", number),
                ("forceSource", Some(&force)),
            ],
            keywords,
        )?;
        return music21_converter(py)?.call_method("parseURL", (url,), Some(&all));
    }

    // The download, then the format its path's extension names.
    let response = py
        .import("urllib.request")?
        .call_method1("urlopen", (url,))?;
    let mut data = with_context(&response, |response| response.call_method0("read"))?
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("the download was swallowed"))?;
    let mut format = format.cloned();
    if format.is_none() {
        let path = py
            .import("urllib.parse")?
            .call_method1("urlparse", (url,))?
            .getattr("path")?;
        let suffix = py
            .import("pathlib")?
            .getattr("PurePosixPath")?
            .call1((path,))?
            .getattr("suffix")?;
        if let Some(found) = format_named(&suffix)? {
            format = Some(PyString::new(py, found.name()).into_any());
        }
    }
    let musicxml = match &format {
        Some(format) => format.eq("musicxml")?,
        None => false,
    };
    if musicxml && bytes_of(&data).is_some_and(|bytes| bytes.starts_with(ZIP_MAGIC)) {
        data = with_zip(&bytes_file(&data)?, musicxml_of_zip)?;
    }
    parse_data(py, &data, number, format.as_ref(), keywords)
}

/// Reads a file path, the text or bytes of a document, a `(path, number)`
/// pair or a URL into a stream, as music21's `converter.parse` does.
///
/// A string is a file where one exists by that name, else a URL where it
/// starts `http://` or `https://`, else a document. A document starting
/// with a header such as `tinyNotation:` is a document whatever else it
/// holds.
#[pyfunction]
#[pyo3(signature = (value, *, forceSource=None, number=None, format=None, **keywords))]
#[allow(non_snake_case)]
fn parse<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
    forceSource: Option<&Bound<'py, PyAny>>,
    number: Option<&Bound<'py, PyAny>>,
    format: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let pure_path = py.import("pathlib")?.getattr("PurePath")?;
    let is_bytes = bytes_of(value).is_some();
    let is_str = value.is_instance_of::<PyString>();
    let is_path = value.is_instance(&pure_path)?;
    let is_sequence = value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>();
    let force = force_source(py, forceSource);

    // Anything but a path or a document, installed: music21's to read.
    if installed(py)? && !(is_str || is_bytes || is_path || is_sequence) {
        let all = with_keywords(
            py,
            &[
                ("forceSource", Some(&force)),
                ("number", number),
                ("format", format),
            ],
            keywords,
        )?;
        return music21_converter(py)?.call_method("parse", (value,), Some(&all));
    }
    let text: String = if is_bytes {
        decode(value, "utf-8", "ignore")?.extract()?
    } else if is_str || is_path {
        value.str()?.to_string()
    } else {
        String::new()
    };

    // A `(path, number)` pair, a MIDI file's bytes as text, a header.
    if is_sequence && value.len()? == 2 && can_load(&value.get_item(0)?.str()?.into_any())? {
        return parse_pair(py, value, format, keywords);
    }
    if !is_bytes && text.starts_with("MThd") {
        return parse_data(py, value, number, format, keywords);
    }
    if is_str
        && format_from_header(&value.call_method0("lstrip")?)?
            .0
            .is_some()
    {
        return parse_data(py, value, number, format, keywords);
    }

    // A file, by its name as given or with `~` expanded, or a URL.
    let named = PyString::new(py, &text).into_any();
    if !is_bytes && can_load(&named)? {
        return parse_file(py, &named, number, format, Some(&force), keywords);
    }
    let expanded = expand_user(&named)?;
    if !is_bytes && can_load(&expanded)? {
        return parse_file(py, &expanded, number, format, Some(&force), keywords);
    }
    if !is_bytes && (text.starts_with("http://") || text.starts_with("https://")) {
        return parse_url(py, value, format, number, Some(&force), keywords);
    }

    // A file path mistyped, rather than a document.
    if is_path {
        return Err(PyFileNotFoundError::new_err(format!(
            "Cannot find file in {text}"
        )));
    }
    if is_str {
        let suffix = pure_path.call1((value,))?.getattr("suffix")?;
        let suffix = if suffix.is_truthy()? {
            suffix
        } else {
            PyString::new(py, "-").into_any()
        };
        if format_named(&suffix)?.is_some() {
            return Err(PyFileNotFoundError::new_err(format!(
                "Cannot find file in {text}"
            )));
        }
    }
    parse_data(py, value, number, format, keywords)
}

/// A `(path, number)` pair, as `corpus.search` hands them out: the path
/// read, and the score numbered `number` where it holds several.
fn parse_pair<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
    format: Option<&Bound<'py, PyAny>>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let path = value.get_item(0)?;
    let number = value.get_item(1)?;
    if number.is_none() {
        return parse_file(py, &path, None, format, None, keywords);
    }
    if !path.is_instance_of::<PyString>() {
        return Err(CONVERTER_EXCEPTION.err(
            py,
            format!(
                "If using a two-element list, the first value must be a string, not {}",
                path.repr()?
            ),
        ));
    }
    if !number.is_instance_of::<PyInt>() {
        return Err(CONVERTER_EXCEPTION.err(
            py,
            format!(
                "If using a two-element list, the second value must be an integer number, \
                 not {}",
                number.repr()?
            ),
        ));
    }
    let read = parse_file(py, &path, None, format, None, keywords)?;
    if read.get_type().name()? == "Opus" {
        return read.call_method1("getScoreByNumber", (number,));
    }
    Ok(read)
}

/// `obj` written in `fmt`, handed back as the text or bytes of the
/// document, as music21's `converter.toData` hands it back.
///
/// The formats are those `Stream.write` writes -- MusicXML, MIDI, text,
/// LilyPond, braille, RomanText and Volpiano -- and ABC, which music21
/// does not write.
#[pyfunction]
#[pyo3(name = "toData", signature = (obj, fmt, **keywords))]
fn to_data<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    fmt: &Bound<'py, PyAny>,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let name: String = fmt
        .call_method0("lower")?
        .call_method1("lstrip", (".",))?
        .extract()?;
    if name == "abc" {
        return wheel(py, "to_abc")?.call1((obj,));
    }
    if installed(py)? {
        return music21_converter(py)?.call_method("toData", (obj, fmt), keywords);
    }
    let first = name.split('.').next().unwrap_or("");
    let Some((_, encoding)) = WRITTEN.iter().find(|(written, _)| *written == first) else {
        return Err(CONVERTER_EXCEPTION.err(
            py,
            format!("cannot support output in this format yet: {}", fmt.str()?),
        ));
    };

    // Written to a temporary file, read back, and the file removed.
    let made = py.import("tempfile")?.call_method0("mkstemp")?;
    py.import("os")?
        .call_method1("close", (made.get_item(0)?,))?;
    let path = made.get_item(1)?;
    let written = write_and_read(obj, fmt, &path, *encoding, keywords);
    let kwargs = PyDict::new(py);
    kwargs.set_item("missing_ok", true)?;
    path_of(py, &path)?.call_method("unlink", (), Some(&kwargs))?;
    written
}

/// `obj` written in `fmt` to `path`, and what was written read back.
fn write_and_read<'py>(
    obj: &Bound<'py, PyAny>,
    fmt: &Bound<'py, PyAny>,
    path: &Bound<'py, PyAny>,
    encoding: Encoding,
    keywords: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = obj.py();
    let all = with_keywords(py, &[("fp", Some(path))], keywords)?;
    let written = obj.call_method("write", (fmt,), Some(&all))?;
    let written = path_of(py, written.str()?)?;
    if encoding == Encoding::Binary {
        return written.call_method0("read_bytes");
    }
    let kwargs = PyDict::new(py);
    kwargs.set_item("encoding", "utf-8")?;
    written.call_method("read_text", (), Some(&kwargs))
}

/// music21's `freezeThaw`, which the wheel builds.
fn freeze_thaw(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    crate::music21_module(py, "music21.freezeThaw")
}

/// music21's `freeze`: the stream written to `fp`, or to a new file in
/// music21's temporary directory, and the path written.
#[pyfunction]
#[pyo3(signature = (streamObj, fmt=None, fp=None, fastButUnsafe=None, zipType=None))]
#[allow(non_snake_case)]
fn freeze<'py>(
    py: Python<'py>,
    streamObj: &Bound<'py, PyAny>,
    fmt: Option<&Bound<'py, PyAny>>,
    fp: Option<&Bound<'py, PyAny>>,
    fastButUnsafe: Option<&Bound<'py, PyAny>>,
    zipType: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let fast = force_source(py, fastButUnsafe);
    let made = PyDict::new(py);
    made.set_item("fastButUnsafe", fast)?;
    let freezer = freeze_thaw(py)?
        .getattr("StreamFreezer")?
        .call((streamObj,), Some(&made))?;
    let zip_type = zip_type_of(py, zipType);
    let written = with_keywords(
        py,
        &[("fmt", fmt), ("fp", fp), ("zipType", Some(&zip_type))],
        None,
    )?;
    freezer.call_method("write", (), Some(&written))
}

/// `zipType` as given, `zlib` where it was not.
fn zip_type_of<'py>(py: Python<'py>, given: Option<&Bound<'py, PyAny>>) -> Bound<'py, PyAny> {
    match given {
        Some(given) => given.clone(),
        None => PyString::new(py, "zlib").into_any(),
    }
}

/// music21's `thaw`: the stream `freeze` wrote to `fp`.
#[pyfunction]
#[pyo3(signature = (fp, zipType=None))]
#[allow(non_snake_case)]
fn thaw<'py>(
    py: Python<'py>,
    fp: &Bound<'py, PyAny>,
    zipType: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let thawer = freeze_thaw(py)?.getattr("StreamThawer")?.call0()?;
    let zip_type = zip_type_of(py, zipType);
    let opened = with_keywords(py, &[("zipType", Some(&zip_type))], None)?;
    thawer.call_method("open", (fp,), Some(&opened))?;
    thawer.getattr("stream")
}

/// music21's `freezeStr`: the stream written out, as bytes for a pickle
/// and as text for jsonpickle.
#[pyfunction]
#[pyo3(name = "freezeStr", signature = (streamObj, fmt=None))]
#[allow(non_snake_case)]
fn freeze_str<'py>(
    py: Python<'py>,
    streamObj: &Bound<'py, PyAny>,
    fmt: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let freezer = freeze_thaw(py)?
        .getattr("StreamFreezer")?
        .call1((streamObj,))?;
    let written = with_keywords(py, &[("fmt", fmt)], None)?;
    freezer.call_method("writeStr", (), Some(&written))
}

/// music21's `thawStr`: the stream `freezeStr` wrote.
#[pyfunction]
#[pyo3(name = "thawStr")]
#[allow(non_snake_case)]
fn thaw_str<'py>(py: Python<'py>, strData: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let thawer = freeze_thaw(py)?.getattr("StreamThawer")?.call0()?;
    thawer.call_method1("openStr", (strData,))?;
    thawer.getattr("stream")
}

// --- For the corpus -----------------------------------------------------------

/// Every extension of the format `name` names, each with its dot; none
/// where it names no format.
pub(crate) fn extensions_of(name: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    Ok(format_named(name)?
        .map(|found| {
            found
                .extensions
                .iter()
                .map(|each| format!(".{each}"))
                .collect()
        })
        .unwrap_or_default())
}

/// `parse(path, number=number, format=format)`.
pub(crate) fn parse_path<'py>(
    path: &Bound<'py, PyAny>,
    number: Option<&Bound<'py, PyAny>>,
    format: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    parse(path.py(), path, None, number, format, None)
}

#[cfg(test)]
mod tests {
    use super::{declared_encoding, posix_suffix};

    #[test]
    fn an_encoding_is_read_only_where_the_document_starts_with_it() {
        assert_eq!(
            declared_encoding(b"encoding='UTF-16'?>"),
            Some(&b"UTF-16"[..])
        );
        assert_eq!(declared_encoding(b"encoding=\"x y\""), None);
        assert_eq!(declared_encoding(b"<?xml encoding='UTF-8'"), None);
    }

    #[test]
    fn a_suffix_is_pathlib_s() {
        assert_eq!(posix_suffix("a/b.xml"), ".xml");
        assert_eq!(posix_suffix(".xml"), "");
        assert_eq!(posix_suffix("a/b."), "");
        assert_eq!(posix_suffix("a/b.mxl/"), ".mxl");
    }
}
