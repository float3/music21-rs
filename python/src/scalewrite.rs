//! music21's `write` and `show` for a scale and a key, which are not
//! streams but which music21 writes as one, and `write` for the wheel's
//! own streams.
//!
//! music21 hands either to its converters: MusicXML is a measure of the
//! scale's degrees, MIDI a stream holding the object as it is, the text
//! formats its `repr`, and Scala the file of its steps. Each is written here
//! by the crate's own writers, so nothing reaches music21's converters; what
//! has to be opened in another program to be shown is refused, as the
//! wheel's `Stream.show` refuses it.

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyString};

use music21_rs_crate::musicxml::ExportOptions;

use crate::scale::{ConcreteScale, ScaleException};

/// What the object being written is, which decides how its degrees are
/// laid out in MusicXML.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// The tonic held for a whole note and the dominant for a half: music21's
    /// `fromDiatonicScale`, which a key takes too.
    Diatonic,
    /// The tonic held for a whole note: music21's `fromScale`.
    Concrete,
    /// A pattern with no tonic, which has no pitches to write.
    Abstract,
}

/// The formats a scale is written in, under music21's names.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    MusicXml,
    Midi,
    Text,
    TextLine,
    Scala,
    LilyPond,
    Braille,
    RomanText,
    Volpiano,
}

impl Format {
    /// The extension music21 gives a temporary file of this format.
    fn extension(self) -> &'static str {
        match self {
            Self::MusicXml => ".musicxml",
            Self::Midi => ".mid",
            Self::Text | Self::TextLine => ".txt",
            Self::Scala => ".scl",
            Self::LilyPond => ".ly",
            Self::RomanText => ".rntxt",
            Self::Braille | Self::Volpiano => ".txt",
        }
    }
}

/// A document written, as text or as bytes.
enum Written {
    Text(String),
    Bytes(Vec<u8>),
}

/// The format a `fmt` names, as music21's `common.findFormat` reads it;
/// no format at all is MusicXML, music21's default.
fn format_of(fmt: Option<&str>) -> PyResult<Format> {
    let given = fmt.unwrap_or("musicxml");
    let name = given.trim_start_matches('.').to_lowercase();
    let format = match name.as_str() {
        "musicxml" | "xml" => Format::MusicXml,
        "midi" | "mid" => Format::Midi,
        "text" | "txt" | "t" => Format::Text,
        "textline" => Format::TextLine,
        "scala" | "scl" => Format::Scala,
        // Among them a subformat (`musicxml.png`), which is something
        // rendered, and the crate renders nothing.
        _ => {
            return Err(ScaleException::new_err(format!(
                "cannot write {given:?} here: a scale or a key is written as \
                 'musicxml', 'midi', 'text', 'textline' or 'scala'"
            )));
        }
    };
    Ok(format)
}

/// The format a stream's `fmt` names: those music21 writes a stream in that
/// the wheel writes too. No format at all is MusicXML, music21's default.
fn stream_format_of(fmt: Option<&str>) -> PyResult<Format> {
    let given = fmt.unwrap_or("musicxml");
    let name = given.trim_start_matches('.').to_lowercase();
    let format = match name.as_str() {
        "musicxml" | "xml" => Format::MusicXml,
        "midi" | "mid" => Format::Midi,
        "text" | "txt" | "t" => Format::Text,
        "textline" => Format::TextLine,
        "lily" | "lilypond" => Format::LilyPond,
        "braille" => Format::Braille,
        "romantext" | "rntxt" => Format::RomanText,
        "volpiano" => Format::Volpiano,
        // A compressed file, or something rendered: the crate renders
        // nothing and zips nothing.
        _ => {
            return Err(crate::stream::StreamException::new_err(format!(
                "cannot write {given:?} here: a stream is written as 'musicxml', \
                 'midi', 'text', 'textline', 'lilypond', 'braille', 'romantext' \
                 or 'volpiano'"
            )));
        }
    };
    Ok(format)
}

/// music21's `Stream.write`: the stream written in `fmt` by the wheel's own
/// writers, to `fp` or a temporary file, and the path written. MusicXML is
/// made notation first unless `makeNotation=False`, as music21 makes it.
pub(crate) fn write_stream(
    stream: &Bound<'_, PyAny>,
    fmt: Option<&str>,
    fp: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py = stream.py();
    let format = stream_format_of(fmt)?;
    let package = py
        .import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))?;
    let said = |name: &str, default: bool| -> PyResult<bool> {
        match keywords
            .map(|given| given.get_item(name))
            .transpose()?
            .flatten()
        {
            Some(value) => value.is_truthy(),
            None => Ok(default),
        }
    };
    let text = |name: &str| -> PyResult<Written> {
        Ok(Written::Text(
            package.getattr(name)?.call1((stream,))?.extract()?,
        ))
    };

    let written = match format {
        Format::MusicXml => {
            let options = PyDict::new(py);
            options.set_item("make_notation", said("makeNotation", true)?)?;
            Written::Text(
                package
                    .getattr("to_musicxml")?
                    .call((stream,), Some(&options))?
                    .extract()?,
            )
        }
        Format::Midi => {
            let options = PyDict::new(py);
            options.set_item("add_start_delay", said("addStartDelay", false)?)?;
            options.set_item("add_end_delay", said("addEndDelay", true)?)?;
            Written::Bytes(
                package
                    .getattr("to_midi")?
                    .call((stream,), Some(&options))?
                    .extract()?,
            )
        }
        Format::Text => Written::Text(stream.call_method0("recurseRepr")?.extract()?),
        Format::TextLine => Written::Text(stream.call_method1("show", ("textline",))?.extract()?),
        Format::LilyPond => text("to_lilypond")?,
        Format::Braille => text("objectToBraille")?,
        Format::RomanText => text("to_roman_text")?,
        Format::Volpiano => text("to_volpiano")?,
        Format::Scala => {
            return Err(crate::stream::StreamException::new_err(
                "a stream is not written as a Scala file",
            ));
        }
    };
    written_to(py, fp, &written, format)
}

/// music21's `write` for a scale or a key: `obj` written in `fmt` to `fp`,
/// or to a temporary file where `fp` is not given, and the path written.
///
/// `scale` is the concrete scale whose steps a Scala file lists: `obj`
/// itself, the scale a key stands on, or an abstract pattern stood on C4;
/// `None` for a pattern with no steps.
pub(crate) fn write_scale(
    obj: &Bound<'_, PyAny>,
    scale: Option<&Bound<'_, PyAny>>,
    kind: Kind,
    fmt: Option<&str>,
    fp: Option<&Bound<'_, PyAny>>,
    direction: Option<&Bound<'_, PyAny>>,
    keywords: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let format = format_of(fmt)?;
    let written = match format {
        Format::MusicXml => Written::Text(musicxml_of(obj, kind)?),
        Format::Midi => Written::Bytes(midi_of(obj, keywords)?),
        Format::Text | Format::TextLine => Written::Text(obj.repr()?.to_string()),
        Format::Scala => Written::Text(scala_of(obj, scale, kind, direction)?),
        Format::LilyPond | Format::Braille | Format::RomanText | Format::Volpiano => {
            return Err(ScaleException::new_err(
                "a scale or a key is written as 'musicxml', 'midi', 'text', \
                 'textline' or 'scala'",
            ));
        }
    };
    written_to(obj.py(), fp, &written, format)
}

/// music21's `show` for a scale or a key: `'text'` prints its `repr` and
/// `'textline'` answers it. Every other format is opened in a program of
/// its own, and is refused.
pub(crate) fn show_scale(obj: &Bound<'_, PyAny>, fmt: Option<&str>) -> PyResult<Option<String>> {
    let py = obj.py();
    let format = format_of(fmt)?;
    match format {
        Format::Text => {
            py.import("builtins")?
                .getattr("print")?
                .call1((obj.repr()?,))?;
            Ok(None)
        }
        Format::TextLine => Ok(Some(obj.repr()?.to_string())),
        _ => Err(ScaleException::new_err(format!(
            "cannot show {:?} here: only 'text' and 'textline' are shown; \
             write the document and open it instead",
            fmt.unwrap_or("musicxml")
        ))),
    }
}

/// The MusicXML document of the scale's degrees, as music21's exporter
/// writes a scale.
fn musicxml_of(obj: &Bound<'_, PyAny>, kind: Kind) -> PyResult<String> {
    let py = obj.py();
    if kind == Kind::Abstract {
        return Err(ScaleException::new_err(
            "an abstract scale has no pitches to write; write a concrete scale on a tonic",
        ));
    }

    // music21 signs the document with the day it was written.
    let today: String = py
        .import("datetime")?
        .getattr("date")?
        .call_method0("today")?
        .call_method0("isoformat")?
        .extract()?;
    let options = ExportOptions {
        encoding_date: Some(today),
        make_notation: true,
        ..ExportOptions::default()
    };
    let measure = degree_measure(obj, kind)?;
    let stream = crate::stream::crate_stream(&measure)?;
    music21_rs_crate::musicxml::to_musicxml(&stream, &options)
        .map_err(|error| ScaleException::new_err(crate::pitch::message(&error)))
}

/// A measure of a note on each degree, the first under the scale's name and
/// the tonic (and a diatonic scale's dominant) held longer, metered to fit:
/// music21's `fromScale` and `fromDiatonicScale`.
fn degree_measure<'py>(obj: &Bound<'py, PyAny>, kind: Kind) -> PyResult<Bound<'py, PyAny>> {
    let py = obj.py();
    let numbered = PyDict::new(py);
    numbered.set_item("number", 1)?;
    let measure = py
        .get_type::<crate::stream::Measure>()
        .call((), Some(&numbered))?;
    let tonic = obj.call_method0("getTonic")?.getattr("name")?;
    let dominant = match kind {
        Kind::Diatonic => Some(obj.call_method0("getDominant")?.getattr("name")?),
        _ => None,
    };

    let degrees: usize = obj.call_method0("getDegreeMaxUnique")?.extract()?;
    for degree in 1..=degrees {
        let pitch = obj.call_method1("pitchFromDegree", (degree,))?;
        let note = py.get_type::<crate::note::Note>().call0()?;
        note.setattr("pitch", &pitch)?;
        if degree == 1 {
            note.call_method1("addLyric", (obj.getattr("name")?,))?;
        }

        let name = pitch.getattr("name")?;
        let length = if name.eq(&tonic)? {
            4
        } else if let Some(dominant) = &dominant
            && name.eq(dominant)?
        {
            2
        } else {
            1
        };
        note.setattr("quarterLength", length)?;
        measure.call_method1("append", (note,))?;
    }

    let meter = measure.call_method0("bestTimeSignature")?;
    measure.setattr("timeSignature", meter)?;
    Ok(measure)
}

/// The MIDI file of a stream holding a copy of the object, as music21's
/// `music21ObjectToMidiFile` writes anything that is not a stream: a scale
/// sounds nothing, and a key is a key signature.
fn midi_of(obj: &Bound<'_, PyAny>, keywords: Option<&Bound<'_, PyDict>>) -> PyResult<Vec<u8>> {
    let py = obj.py();
    let flag = |name: &str, default: bool| -> PyResult<bool> {
        match keywords
            .map(|given| given.get_item(name))
            .transpose()?
            .flatten()
        {
            Some(value) => value.is_truthy(),
            None => Ok(default),
        }
    };
    let options = music21_rs_crate::midi::ExportOptions {
        add_start_delay: flag("addStartDelay", false)?,
        add_end_delay: flag("addEndDelay", true)?,
        acceptable_channels: None,
    };

    // A copy, as music21 makes, so the object is put in no stream.
    let copied = py.import("copy")?.call_method1("deepcopy", (obj,))?;
    let holder = py.get_type::<crate::stream::Stream>().call0()?;
    holder.call_method1("insert", (0, copied))?;
    let stream = crate::stream::crate_stream(&holder)?;
    music21_rs_crate::midi::to_midi(&stream, &options)
        .map_err(|error| ScaleException::new_err(crate::pitch::message(&error)))
}

/// The Scala file of the scale's steps. music21 writes it from the scale's
/// abstract pattern, and so describes it by that pattern's `repr`.
fn scala_of(
    obj: &Bound<'_, PyAny>,
    scale: Option<&Bound<'_, PyAny>>,
    kind: Kind,
    direction: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let py = obj.py();
    let Some(scale) = scale else {
        // music21's words for an abstract scale with nothing to realize.
        return Err(ScaleException::new_err("no network is defined."));
    };
    let pattern = match kind {
        Kind::Abstract => obj.clone(),
        _ => {
            // A key holds no pattern of its own until one is put there; the
            // scale it stands on does.
            let own = obj.getattr("abstract")?;
            match own.is_none() {
                true => scale.getattr("abstract")?,
                false => own,
            }
        }
    };

    let steps = scale
        .cast::<ConcreteScale>()?
        .borrow()
        .scala_steps(direction)?;
    let data = py.get_type::<crate::scala::ScalaData>().call0()?;
    data.call_method1("setIntervalSequence", (steps,))?;
    data.setattr("description", pattern.repr()?)?;
    data.call_method0("getFileString")?.extract()
}

/// Writes a document to `fp` — a path, or an open file — or to a temporary
/// file, and hands back where it went: the path, or the file as given.
fn written_to(
    py: Python<'_>,
    fp: Option<&Bound<'_, PyAny>>,
    written: &Written,
    format: Format,
) -> PyResult<Py<PyAny>> {
    let fp = fp.filter(|given| !given.is_none());

    // An open file is written to as it is, text or bytes as the format is.
    if let Some(file) = fp
        && !file.is_instance_of::<PyString>()
        && file.hasattr("write")?
    {
        match written {
            Written::Text(text) => file.call_method1("write", (text,))?,
            Written::Bytes(bytes) => file.call_method1("write", (PyBytes::new(py, bytes),))?,
        };
        return Ok(file.clone().unbind());
    }

    let path: String = match fp {
        Some(given) => {
            let named: String = py
                .import("os")?
                .call_method1("fspath", (given,))?
                .extract()?;
            suffixed(named, format)
        }
        None => temporary_file(py, format)?,
    };
    let bytes = match written {
        Written::Text(text) => text.as_bytes(),
        Written::Bytes(bytes) => bytes.as_slice(),
    };
    std::fs::write(&path, bytes)
        .map_err(|error| pyo3::exceptions::PyOSError::new_err(format!("{path}: {error}")))?;
    Ok(py
        .import("pathlib")?
        .getattr("Path")?
        .call1((path,))?
        .unbind())
}

/// A MusicXML file named with no extension is given music21's, as music21's
/// converter gives it.
fn suffixed(path: String, format: Format) -> String {
    if format != Format::MusicXml {
        return path;
    }
    if std::path::Path::new(&path).extension().is_some() {
        return path;
    }
    path + format.extension()
}

/// A new, empty temporary file for a format, made where Python makes them.
fn temporary_file(py: Python<'_>, format: Format) -> PyResult<String> {
    let made = py
        .import("tempfile")?
        .call_method1("mkstemp", (format.extension(),))?;
    let (handle, path): (i64, String) = made.extract()?;
    py.import("os")?.call_method1("close", (handle,))?;
    Ok(path)
}
