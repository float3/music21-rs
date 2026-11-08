//! music21's `converter.subConverters`: one class per format, which reads
//! a document into a stream or writes a stream out.
//!
//! `SubConverter` is music21's base, with its attributes and the methods
//! music21's `converter` calls -- `parseData`, `parseFile`, `write`,
//! `toData` and the rest -- for a Python class of one's own to build on,
//! as music21's is built on. Each of music21's formats is a subclass with
//! music21's registered formats and extensions, reading through the
//! wheel's `converter` and writing through a stream's own `write`, so
//! what music21's `Converter` looks up by format is answered by the crate.
//! A format the crate neither reads nor writes -- Scala, a notebook's
//! display -- has its class and refuses as music21's base does.
//!
//! Nothing here opens another program: `launch`, and so `show` of a
//! format that is rendered, refuse with music21's `SubConverterException`.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyModule, PyString, PyTuple};

use crate::exceptions21::SubConverterException;

/// The module music21 keeps these in.
const MODULE: &str = "music21.converter.subConverters";

static BUILT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The hidden attributes naming the format a class reads and writes
/// through the wheel; `None` on the base.
const READS: &str = "_wheelReads";
const WRITES: &str = "_wheelWrites";

/// Whether `parseData` and `parseFile` answer the stream they read, as
/// music21's base `parseFile` and its MEI class do, or nothing, as most of
/// music21's format classes do.
const DATA_ANSWERS: &str = "_wheelDataAnswers";
const FILE_ANSWERS: &str = "_wheelFileAnswers";

/// A name of music21's, from the provided module where `provide_music21`
/// is answering, else from the wheel itself.
fn music21_name<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(found) = py.import(module).and_then(|module| module.getattr(name)) {
        return Ok(found);
    }
    py.import("music21_rs")?.getattr(name)
}

/// The wheel's `converter`, which reads every format the crate reads.
fn converter(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    py.import("music21_rs.converter")
}

/// music21's `SubConverter`: the base every format's class is built on.
#[pyclass(
    name = "SubConverter",
    module = "music21.converter.subConverters",
    subclass,
    dict
)]
pub struct SubConverter {
    /// What was read, or the empty score a new one starts with.
    stream: Py<PyAny>,
    /// The keywords it was made with.
    #[pyo3(get, set)]
    keywords: Py<PyDict>,
}

impl SubConverter {
    /// A class attribute, read through the instance so that a Python
    /// subclass's own is the one read.
    fn attribute<'py>(slf: &Bound<'py, Self>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        slf.getattr(name)
    }

    /// The format this class reads through the wheel, or `None`.
    fn reads(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::attribute(slf, READS)?.extract()
    }

    /// The format this class writes through a stream's `write`, or `None`.
    fn writes(slf: &Bound<'_, Self>) -> PyResult<Option<String>> {
        Self::attribute(slf, WRITES)?.extract()
    }

    /// The stream read, where the class answers it, else `None`.
    fn answered(slf: &Bound<'_, Self>, flag: &str) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if Self::attribute(slf, flag)?.is_truthy()? {
            return Ok(slf.borrow().stream.clone_ref(py));
        }
        Ok(py.None())
    }

    fn not_implemented() -> PyErr {
        pyo3::exceptions::PyNotImplementedError::new_err(())
    }

    /// The file `write` and `toData` write to, opened for reading or
    /// writing as the class reads: bytes where `readBinary` says so.
    fn binary(slf: &Bound<'_, Self>) -> PyResult<bool> {
        Self::attribute(slf, "readBinary")?.is_truthy()
    }

    /// The encoding a file is opened with: `stringEncoding` where
    /// `codecWrite` says to use one, else Python's default.
    fn encoding<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        if Self::attribute(slf, "codecWrite")?.is_truthy()? {
            return Self::attribute(slf, "stringEncoding");
        }
        Ok(py.None().into_bound(py))
    }
}

#[pymethods]
impl SubConverter {
    fn __traverse__(
        &self,
        visit: pyo3::pyclass::PyVisit<'_>,
    ) -> Result<(), pyo3::pyclass::PyTraverseError> {
        visit.call(&self.stream)?;
        visit.call(&self.keywords)?;
        Ok(())
    }

    #[new]
    #[pyo3(signature = (**keywords))]
    fn new(py: Python<'_>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let keywords = match keywords {
            Some(given) => given.copy()?,
            None => PyDict::new(py),
        };
        let score = music21_name(py, "music21.stream", "Score")?.call0()?;
        Ok(Self {
            stream: score.unbind(),
            keywords: keywords.unbind(),
        })
    }

    /// What was read, or the stream set to be written.
    #[getter]
    fn get_stream(&self, py: Python<'_>) -> Py<PyAny> {
        self.stream.clone_ref(py)
    }

    #[setter]
    fn set_stream(&mut self, value: Py<PyAny>) {
        self.stream = value;
    }

    /// Reads a document's text or bytes into `stream`, and answers it.
    #[pyo3(signature = (dataString, number = None))]
    fn parseData<'py>(
        slf: &Bound<'py, Self>,
        dataString: &Bound<'py, PyAny>,
        number: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        let Some(format) = Self::reads(slf)? else {
            return Err(Self::not_implemented());
        };
        let keywords = PyDict::new(py);
        keywords.set_item("number", number)?;
        keywords.set_item("format", format)?;
        let read = converter(py)?
            .getattr("parseData")?
            .call((dataString,), Some(&keywords))?;
        slf.borrow_mut().stream = read.clone().unbind();
        Self::answered(slf, DATA_ANSWERS)
    }

    /// Reads a file into `stream`, and answers it. A class the wheel reads
    /// has the wheel read the file; one of Python's own reads the file's
    /// text, or its bytes where `readBinary` says so, and hands it to its
    /// `parseData`.
    #[pyo3(signature = (filePath, number = None, **keywords))]
    fn parseFile<'py>(
        slf: &Bound<'py, Self>,
        filePath: &Bound<'py, PyAny>,
        number: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Py<PyAny>> {
        let py = slf.py();
        if let Some(format) = Self::reads(slf)? {
            let given = match keywords {
                Some(keywords) => keywords.copy()?,
                None => PyDict::new(py),
            };
            given.set_item("number", number)?;
            given.set_item("format", format)?;
            let read = converter(py)?
                .getattr("parseFile")?
                .call((filePath,), Some(&given))?;
            slf.borrow_mut().stream = read.unbind();
            return Self::answered(slf, FILE_ANSWERS);
        }
        let open_keywords = PyDict::new(py);
        let mode = if Self::binary(slf)? {
            "rb"
        } else {
            let preferred = py.import("locale")?.call_method0("getpreferredencoding")?;
            open_keywords.set_item("encoding", preferred)?;
            "r"
        };
        let builtins = py.import("builtins")?;
        let handle = builtins
            .getattr("open")?
            .call((filePath, mode), Some(&open_keywords))?;
        let data = handle.call_method0("read");
        handle.call_method0("close")?;
        slf.call_method1("parseData", (data?, number))?;
        Ok(slf.getattr("stream")?.unbind())
    }

    /// Whether this can show what it writes; music21's base always can.
    #[pyo3(signature = (**_keywords))]
    fn checkShowAbility(&self, _keywords: Option<&Bound<'_, PyDict>>) -> bool {
        true
    }

    /// Refused: music21 opens the file in the program its settings name,
    /// and the wheel opens no other program.
    #[pyo3(signature = (filePath, fmt = None, options = "", app = None, launchKey = None))]
    fn launch(
        &self,
        filePath: &Bound<'_, PyAny>,
        fmt: Option<&Bound<'_, PyAny>>,
        options: &str,
        app: Option<&Bound<'_, PyAny>>,
        launchKey: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let _ = (fmt, options, app, launchKey);
        Err(SubConverterException::new_err(format!(
            "music21-rs opens no other program, so it cannot show {}",
            filePath.str()?
        )))
    }

    /// Writes `obj` and opens what was written: refused where it would
    /// open another program, as `launch` is.
    #[pyo3(signature = (obj, fmt, app = None, subformats = None, **keywords))]
    fn show<'py>(
        slf: &Bound<'py, Self>,
        obj: &Bound<'py, PyAny>,
        fmt: Option<&Bound<'py, PyAny>>,
        app: Option<&Bound<'py, PyAny>>,
        subformats: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<()> {
        let py = slf.py();
        let given = PyDict::new(py);
        given.set_item("subformats", subformats)?;
        if let Some(keywords) = keywords {
            given.update(keywords.as_mapping())?;
        }
        let written = slf.call_method("write", (obj, fmt), Some(&given))?;
        let launch = PyDict::new(py);
        launch.set_item("fmt", fmt)?;
        launch.set_item("app", app)?;
        slf.call_method("launch", (written,), Some(&launch))?;
        Ok(())
    }

    /// The extension a file of this format is written with, its dot
    /// first: a subformat's own where one is registered.
    #[pyo3(signature = (subformats = None))]
    fn getExtensionForSubformats(
        slf: &Bound<'_, Self>,
        subformats: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let extensions = Self::attribute(slf, "registerOutputExtensions")?;
        if extensions.len()? == 0 {
            return Err(SubConverterException::new_err(
                "This subConverter cannot show or write: no output extensions are registered for it",
            ));
        }
        let mut extension: String = extensions.get_item(0)?.extract()?;
        let by_subformat = Self::attribute(slf, "registerOutputSubformatExtensions")?;
        if let Some(subformats) = subformats.filter(|given| given.is_truthy().unwrap_or(false)) {
            let mut parts = Vec::new();
            for part in subformats.try_iter()? {
                parts.push(part?.str()?.to_string());
            }
            let joined = parts.join(".");
            if by_subformat.is_truthy()? && by_subformat.contains(&joined)? {
                extension = by_subformat.get_item(&joined)?.extract()?;
            }
        }
        Ok(format!(".{extension}"))
    }

    /// A new temporary file with the extension this format writes.
    #[pyo3(signature = (subformats = None))]
    fn getTemporaryFile<'py>(
        slf: &Bound<'py, Self>,
        subformats: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let extension = Self::getExtensionForSubformats(slf, subformats)?;
        let environment = music21_name(py, "music21.environment", "Environment")?
            .call1(("converter.subConverters",))?;
        let keywords = PyDict::new(py);
        keywords.set_item("returnPathlib", true)?;
        environment.call_method("getTempFile", (extension,), Some(&keywords))
    }

    /// Writes `obj` in `fmt` to `fp`, or to a temporary file, and answers
    /// the path written. A format the wheel writes is written by the
    /// stream's own `write`; music21's base writes the object's `repr`.
    #[pyo3(signature = (obj, fmt, fp = None, subformats = None, **keywords))]
    fn write<'py>(
        slf: &Bound<'py, Self>,
        obj: &Bound<'py, PyAny>,
        fmt: Option<&Bound<'py, PyAny>>,
        fp: Option<&Bound<'py, PyAny>>,
        subformats: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let Some(format) = Self::writes(slf)? else {
            let text = obj.repr()?;
            return slf.call_method1("writeDataStream", (fp, text));
        };
        let mut written = format;
        if let Some(subformats) = subformats.filter(|given| given.is_truthy().unwrap_or(false)) {
            for part in subformats.try_iter()? {
                written.push('.');
                written.push_str(&part?.str()?.to_string());
            }
        }
        let _ = fmt;
        let given = match keywords {
            Some(keywords) => keywords.copy()?,
            None => PyDict::new(py),
        };
        given.set_item("fp", fp)?;
        let path = obj.call_method("write", (written,), Some(&given))?;
        py.import("pathlib")?.getattr("Path")?.call1((path,))
    }

    /// Writes `dataStr` to `fp` -- a path, or an open file, which is
    /// closed after -- or to a temporary file, and answers the path.
    #[pyo3(signature = (fp, dataStr, **_keywords))]
    fn writeDataStream<'py>(
        slf: &Bound<'py, Self>,
        fp: Option<&Bound<'py, PyAny>>,
        dataStr: &Bound<'py, PyAny>,
        _keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let pathlib = py.import("pathlib")?;
        let target = match fp.filter(|fp| !fp.is_none()) {
            Some(fp) => fp.clone(),
            None => Self::getTemporaryFile(slf, None)?,
        };
        let musicxml = Self::writes(slf)?.as_deref() == Some("musicxml");
        if musicxml && !dataStr.is_instance_of::<pyo3::types::PyBytes>() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "{} must be bytes to write to this format",
                dataStr.str()?
            )));
        }
        let mut data = dataStr.clone();
        let codec = Self::attribute(slf, "codecWrite")?.is_truthy()?;
        if !codec && data.is_instance_of::<pyo3::types::PyBytes>() {
            match data.call_method1("decode", ("utf-8",)) {
                Ok(text) => data = text,
                Err(error)
                    if error.is_instance_of::<pyo3::exceptions::PyUnicodeDecodeError>(py) =>
                {
                    slf.setattr("codecWrite", true)?;
                }
                Err(error) => return Err(error),
            }
        }
        let is_path = target.is_instance_of::<PyString>()
            || target.is_instance(&pathlib.getattr("PurePath")?)?;
        if !is_path {
            target.call_method1("write", (&data,))?;
            target.call_method0("close")?;
            return pathlib.getattr("Path")?.call1(("",));
        }
        let mut path = pathlib.getattr("Path")?.call1((target,))?;
        if musicxml {
            path = with_musicxml_suffix(path)?;
        }
        let mode = if Self::binary(slf)? { "wb" } else { "w" };
        let keywords = PyDict::new(py);
        keywords.set_item("mode", mode)?;
        keywords.set_item("encoding", Self::encoding(slf)?)?;
        let handle = py
            .import("builtins")?
            .getattr("open")?
            .call((&path,), Some(&keywords))?;
        let wrote = handle.call_method1("write", (&data,));
        handle.call_method0("close")?;
        wrote?;
        Ok(path)
    }

    /// `obj` written in `fmt` and read back: the text or bytes of the
    /// document, the file it went through removed.
    #[pyo3(signature = (obj, fmt, subformats = None, **keywords))]
    fn toData<'py>(
        slf: &Bound<'py, Self>,
        obj: &Bound<'py, PyAny>,
        fmt: Option<&Bound<'py, PyAny>>,
        subformats: Option<&Bound<'py, PyAny>>,
        keywords: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let given = match keywords {
            Some(keywords) => keywords.copy()?,
            None => PyDict::new(py),
        };
        given.set_item("fmt", fmt)?;
        given.set_item("subformats", subformats)?;
        let path = slf.call_method("write", (obj,), Some(&given))?;
        let mode = if Self::binary(slf)? { "rb" } else { "r" };
        let open = PyDict::new(py);
        open.set_item("mode", mode)?;
        open.set_item("encoding", Self::encoding(slf)?)?;
        let handle = py
            .import("builtins")?
            .getattr("open")?
            .call((&path,), Some(&open))?;
        let data = handle.call_method0("read");
        handle.call_method0("close")?;
        let unlink = PyDict::new(py);
        unlink.set_item("missing_ok", true)?;
        path.call_method("unlink", (), Some(&unlink))?;
        data
    }
}

/// A MusicXML file's path with its suffix: `.musicxml` where there is
/// none, or where it is `.mxl`, the zipped form's.
fn with_musicxml_suffix(path: Bound<'_, PyAny>) -> PyResult<Bound<'_, PyAny>> {
    let suffix: String = path.getattr("suffix")?.extract()?;
    if suffix.is_empty() || suffix == ".mxl" {
        return path.call_method1("with_suffix", (".musicxml",));
    }
    Ok(path)
}

/// `SubConverter.__init__`: the keywords kept, so that a subclass's own
/// `__init__` can hand its keywords up, as it does to music21's.
#[pyfunction]
#[pyo3(signature = (slf, **keywords))]
fn sub_init(slf: &Bound<'_, SubConverter>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    let py = slf.py();
    let kept = match keywords {
        Some(given) => given.copy()?,
        None => PyDict::new(py),
    };
    slf.borrow_mut().keywords = kept.unbind();
    Ok(())
}

/// One of music21's format classes: its name, the formats and extensions
/// it registers, whether it reads bytes and writes with an encoding, and
/// what the wheel reads and writes for it.
struct Format {
    name: &'static str,
    formats: &'static [&'static str],
    inputs: &'static [&'static str],
    outputs: &'static [&'static str],
    subformats: &'static [(&'static str, &'static str)],
    binary: bool,
    codec: bool,
    reads: Option<&'static str>,
    writes: Option<&'static str>,
    /// Whether `parseFile` answers the stream: only where music21's class
    /// keeps its base's `parseFile`, or is MEI's.
    file_answers: bool,
}

/// music21's format classes, in its order.
const FORMATS: &[Format] = &[
    Format {
        name: "ConverterIPython",
        formats: &["ipython", "jupyter"],
        inputs: &[],
        outputs: &[],
        subformats: &[("lilypond", "ly")],
        binary: false,
        codec: false,
        reads: None,
        writes: None,
        file_answers: true,
    },
    Format {
        name: "ConverterLilypond",
        formats: &["lilypond", "lily"],
        inputs: &[],
        outputs: &["ly", "png", "pdf", "svg"],
        subformats: &[("png", "png"), ("pdf", "pdf"), ("svg", "")],
        binary: false,
        codec: true,
        reads: None,
        writes: Some("lilypond"),
        file_answers: true,
    },
    Format {
        name: "ConverterBraille",
        formats: &["braille"],
        inputs: &[],
        outputs: &["txt"],
        subformats: &[],
        binary: false,
        codec: true,
        reads: None,
        writes: Some("braille"),
        file_answers: true,
    },
    Format {
        name: "ConverterText",
        formats: &["text", "txt", "t"],
        inputs: &[],
        outputs: &["txt"],
        subformats: &[],
        binary: false,
        codec: false,
        reads: None,
        writes: Some("text"),
        file_answers: true,
    },
    Format {
        name: "ConverterTextLine",
        formats: &["textline"],
        inputs: &[],
        outputs: &["txt"],
        subformats: &[],
        binary: false,
        codec: false,
        reads: None,
        writes: Some("textline"),
        file_answers: true,
    },
    Format {
        name: "ConverterVolpiano",
        formats: &["volpiano"],
        inputs: &["volpiano", "vp"],
        outputs: &["txt", "vp"],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("volpiano"),
        writes: Some("volpiano"),
        file_answers: true,
    },
    Format {
        name: "ConverterScala",
        formats: &["scala"],
        inputs: &["scl"],
        outputs: &["scl"],
        subformats: &[],
        binary: false,
        codec: false,
        reads: None,
        writes: None,
        file_answers: true,
    },
    Format {
        name: "ConverterHumdrum",
        formats: &["humdrum"],
        inputs: &["krn"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("humdrum"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterTinyNotation",
        formats: &["tinynotation"],
        inputs: &["tntxt", "tinynotation"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("tinynotation"),
        writes: None,
        file_answers: true,
    },
    Format {
        name: "ConverterNoteworthy",
        formats: &["noteworthytext"],
        inputs: &["nwctxt"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("noteworthytext"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterNoteworthyBinary",
        formats: &["noteworthy"],
        inputs: &["nwc"],
        outputs: &[],
        subformats: &[],
        binary: true,
        codec: false,
        reads: Some("noteworthy"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterMusicXML",
        formats: &["musicxml", "xml"],
        inputs: &["xml", "mxl", "musicxml"],
        outputs: &["musicxml", "xml", "mxl"],
        subformats: &[("png", "png"), ("pdf", "pdf")],
        binary: false,
        codec: false,
        reads: Some("musicxml"),
        writes: Some("musicxml"),
        file_answers: false,
    },
    Format {
        name: "ConverterMidi",
        formats: &["midi"],
        inputs: &["mid", "midi"],
        outputs: &["mid"],
        subformats: &[],
        binary: true,
        codec: false,
        reads: Some("midi"),
        writes: Some("midi"),
        file_answers: false,
    },
    Format {
        name: "ConverterABC",
        formats: &["abc"],
        inputs: &["abc"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("abc"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterRomanText",
        formats: &["romantext", "rntext"],
        inputs: &["rntxt", "rntext", "romantext", "rtxt"],
        outputs: &["rntxt"],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("romantext"),
        writes: Some("romantext"),
        file_answers: false,
    },
    Format {
        name: "ConverterClercqTemperley",
        formats: &["cttxt", "har", "clercqTemperley"],
        inputs: &["cttxt", "har", "clercqTemperley"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("cttxt"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterCapella",
        formats: &["capella"],
        inputs: &["capx"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("capella"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterMuseData",
        formats: &["musedata"],
        inputs: &["md", "musedata", "zip"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("musedata"),
        writes: None,
        file_answers: false,
    },
    Format {
        name: "ConverterMEI",
        formats: &["mei"],
        inputs: &["mei"],
        outputs: &[],
        subformats: &[],
        binary: false,
        codec: false,
        reads: Some("mei"),
        writes: None,
        file_answers: true,
    },
];

/// A format's class, built on `SubConverter` as a `class` statement
/// would build it, its registrations as class attributes.
fn format_class<'py>(base: &Bound<'py, PyAny>, format: &Format) -> PyResult<Bound<'py, PyAny>> {
    let py = base.py();
    let members = PyDict::new(py);
    members.set_item("__module__", MODULE)?;
    members.set_item("__qualname__", format.name)?;
    members.set_item("registerFormats", PyTuple::new(py, format.formats)?)?;
    members.set_item("registerInputExtensions", PyTuple::new(py, format.inputs)?)?;
    members.set_item(
        "registerOutputExtensions",
        PyTuple::new(py, format.outputs)?,
    )?;
    let subformats = PyDict::new(py);
    for (subformat, extension) in format.subformats {
        subformats.set_item(subformat, extension)?;
    }
    members.set_item("registerOutputSubformatExtensions", subformats)?;
    if format.binary {
        members.set_item("readBinary", true)?;
    }
    if format.codec {
        members.set_item("codecWrite", true)?;
    }
    members.set_item(READS, format.reads)?;
    members.set_item(WRITES, format.writes)?;
    members.set_item(DATA_ANSWERS, format.name == "ConverterMEI")?;
    members.set_item(FILE_ANSWERS, format.file_answers)?;
    py.import("builtins")?
        .getattr("type")?
        .call1((format.name, PyTuple::new(py, [base])?, members))
}

/// music21's `converter.subConverters`, built.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let held = BUILT.get_or_try_init(py, || {
        let m = PyModule::new(py, MODULE)?;
        m.add_class::<SubConverter>()?;
        let base = m.getattr("SubConverter")?;
        let empty = PyTuple::empty(py);
        for (name, value) in [
            ("readBinary", false.into_pyobject(py)?.to_owned().into_any()),
            (
                "canBePickled",
                true.into_pyobject(py)?.to_owned().into_any(),
            ),
            ("registerFormats", empty.clone().into_any()),
            ("registerShowFormats", empty.clone().into_any()),
            ("registerInputExtensions", empty.clone().into_any()),
            ("registerOutputExtensions", empty.into_any()),
            (
                "registerOutputSubformatExtensions",
                PyDict::new(py).into_any(),
            ),
            ("launchKey", py.None().into_bound(py)),
            ("codecWrite", false.into_pyobject(py)?.to_owned().into_any()),
            ("stringEncoding", PyString::new(py, "utf-8").into_any()),
            (READS, py.None().into_bound(py)),
            (WRITES, py.None().into_bound(py)),
            (DATA_ANSWERS, false.into_pyobject(py)?.to_owned().into_any()),
            (FILE_ANSWERS, true.into_pyobject(py)?.to_owned().into_any()),
        ] {
            base.setattr(name, value)?;
        }
        let init = py
            .import("functools")?
            .getattr("partialmethod")?
            .call1((wrap_pyfunction!(sub_init, &m)?,))?;
        base.setattr("__init__", init)?;
        for format in FORMATS {
            m.add(format.name, format_class(&base, format)?)?;
        }
        m.add(
            "SubConverterException",
            py.get_type::<SubConverterException>(),
        )?;
        Ok::<_, PyErr>(m.unbind())
    })?;
    Ok(held.bind(py).clone())
}
