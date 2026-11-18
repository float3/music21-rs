//! music21's `noteworthy` package over the crate's NoteWorthy readers:
//! `constants` and `dictionaries` from the crate's tables, and the
//! `translate` and `binaryTranslate` entry points.
//!
//! music21's translator reads a file line by line into streams it keeps
//! on itself; the crate reads the whole file at once. So
//! `NoteworthyTranslator` keeps music21's starting state and its pitch
//! helpers, and reads through the crate; `NWCConverter` keeps the bytes it
//! read. The line-by-line methods (`translateNote`, `createClef`, ...)
//! and the binary parser's objects are not ported.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::PathBuf;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyDict, PyList, PyModule};

use music21::noteworthy::{
    ALTERATION_TEXTS, BAR_STYLES, CLEF_NAMES, DURATION_VALUES, FLAT_MASK, MIDI_INSTRUMENTS,
    OCTAVE_SHIFT_NAMES, SHARP_MASK, staff_positions, step_and_octave,
};

/// The package and its modules, as music21 names them.
const PACKAGE: &str = "music21.noteworthy";
const CONSTANTS: &str = "music21.noteworthy.constants";
const DICTIONARIES: &str = "music21.noteworthy.dictionaries";
const TRANSLATE: &str = "music21.noteworthy.translate";
const BINARY: &str = "music21.noteworthy.binaryTranslate";

/// The clef music21's translator starts in.
const TREBLE: &str = "TREBLE";

/// The version of the binary format music21's converter starts at.
const NWC_VERSION: i64 = 200;

/// The length of each NoteWorthy duration name in quarter notes.
const NOTE_LENGTHS: [f64; 7] = [4.0, 2.0, 1.0, 0.5, 0.25, 0.125, 0.0625];

/// The order sharps and flats are added to a key signature, by count.
const SHARP_ORDER: [&str; 7] = ["F", "C", "G", "D", "A", "E", "B"];
const FLAT_ORDER: [&str; 7] = ["B", "E", "A", "D", "G", "C", "F"];

/// The staves `dictionaries` lays out, by music21's name and clef.
const STAVES: [(&str, &str); 4] = [
    ("dictionaryTreble", "TREBLE"),
    ("dictionaryBass", "BASS"),
    ("dictionaryAlto", "ALTO"),
    ("dictionaryTenor", "TENOR"),
];

static BUILT: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

pyo3::create_exception!(
    music21_rs_facade,
    NoteworthyTranslateException,
    crate::Music21Exception
);
pyo3::create_exception!(
    music21_rs_facade,
    NoteworthyBinaryTranslateException,
    crate::Music21Exception
);

/// The wheel's own module, under either of the names it is built as.
fn wheel(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    py.import("music21_rs")
        .or_else(|_| py.import("music21_rs_facade"))
}

/// A staff's positions as music21's dictionary of them: each position its
/// step, and `octave` the octave of the lowest.
fn staff_dictionary<'py>(py: Python<'py>, clef: &str) -> PyResult<Bound<'py, PyDict>> {
    let (octave, lowest, steps) = staff_positions(clef);
    let made = PyDict::new(py);
    for (position, step) in (lowest..).zip(steps) {
        made.set_item(position, step.to_string())?;
    }
    made.set_item("octave", octave)?;
    Ok(made)
}

/// A key signature's accidentals by count, from one.
fn order_dictionary<'py>(py: Python<'py>, order: &[&str]) -> PyResult<Bound<'py, PyDict>> {
    let made = PyDict::new(py);
    for (count, step) in (1..).zip(order) {
        made.set_item(count, *step)?;
    }
    Ok(made)
}

/// music21's `noteworthy.dictionaries`.
fn dictionaries(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, DICTIONARIES)?;
    let lengths = PyDict::new(py);
    for (name, length) in DURATION_VALUES.iter().zip(NOTE_LENGTHS) {
        lengths.set_item(name, length)?;
    }
    lengths.set_item(0, 0)?;
    let all = PyDict::new(py);
    all.set_item("dictionaryNoteLength", &lengths)?;
    m.add("dictionaryNoteLength", lengths)?;
    for (name, clef) in STAVES {
        let staff = staff_dictionary(py, clef)?;
        all.set_item(name, &staff)?;
        m.add(name, staff)?;
    }
    for (name, order) in [
        ("dictionarySharp", SHARP_ORDER),
        ("dictionaryFlat", FLAT_ORDER),
    ] {
        let made = order_dictionary(py, &order)?;
        all.set_item(name, &made)?;
        m.add(name, made)?;
    }
    m.add("dictionaries", all)?;
    Ok(m)
}

/// music21's `noteworthy.constants`, the crate's tables.
fn constants(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, CONSTANTS)?;
    m.add("MidiInstruments", PyList::new(py, MIDI_INSTRUMENTS)?)?;
    m.add("ClefNames", PyList::new(py, CLEF_NAMES)?)?;
    m.add("OctaveShiftNames", PyList::new(py, OCTAVE_SHIFT_NAMES)?)?;
    for (name, table) in [("FlatMask", FLAT_MASK), ("SharpMask", SHARP_MASK)] {
        let made = PyDict::new(py);
        for (mask, text) in table {
            made.set_item(mask, text)?;
        }
        m.add(name, made)?;
    }
    m.add("AlterationTexts", PyList::new(py, ALTERATION_TEXTS)?)?;
    m.add("BarStyles", PyList::new(py, BAR_STYLES)?)?;
    m.add("DurationValues", PyList::new(py, DURATION_VALUES)?)?;
    Ok(m)
}

/// music21's `NoteworthyTranslator`: reads `.nwctxt` text into a score.
#[pyclass(
    name = "NoteworthyTranslator",
    module = "music21.noteworthy.translate",
    subclass,
    dict
)]
pub struct NoteworthyTranslator;

#[pymethods]
impl NoteworthyTranslator {
    #[new]
    fn new() -> Self {
        Self
    }

    /// music21's starting state: no part yet, an empty measure and score,
    /// the treble clef and no key.
    fn __init__(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let streams = |name: &str| crate::stream::stream_class(py, name)?.call0();
        slf.setattr("currentPart", py.None())?;
        slf.setattr("currentMeasure", streams("Measure")?)?;
        slf.setattr("measureNumber", 0)?;
        slf.setattr("currentEnding", 1)?;
        slf.setattr("repeatedMeasures", PyList::empty(py))?;
        slf.setattr("score", streams("Score")?)?;
        slf.setattr("currentClef", TREBLE)?;
        let key = wheel(py)?.getattr("KeySignature")?.call1((0,))?;
        slf.setattr("currentKey", key)?;
        slf.setattr("openSlurs", PyList::empty(py))?;
        slf.setattr("tiedSteps", PyDict::new(py))?;
        slf.setattr("positionMeasure", py.None())?;
        slf.setattr("measurePosition", 0.0)?;
        slf.setattr("lyricPosition", 0)?;
        slf.setattr("lyrics", PyList::empty(py))?;
        slf.setattr("activeAccidentals", PyDict::new(py))?;
        Ok(())
    }

    /// music21's `parseFile`: the score a `.nwctxt` file holds.
    fn parseFile<'py>(slf: &Bound<'py, Self>, filePath: PathBuf) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let shown = filePath.display().to_string();
        let text = crate::plumbing::filetools::readFileEncodingSafe(py, filePath, "utf-8")
            .map_err(|_| NoteworthyTranslateException::new_err(format!("cannot open {shown}: ")))?;
        Self::read(slf, &text)
    }

    /// music21's `parseString`: the score `.nwctxt` text holds.
    fn parseString<'py>(slf: &Bound<'py, Self>, data: &str) -> PyResult<Bound<'py, PyAny>> {
        Self::read(slf, data)
    }

    /// music21's `parseList`: the score the lines of a `.nwctxt` file
    /// hold.
    fn parseList<'py>(
        slf: &Bound<'py, Self>,
        dataList: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::read(slf, &dataList.join("\n"))
    }

    /// music21's `getStepAndOctaveFromPosition`: the step and octave of a
    /// staff position under `currentClef`.
    fn getStepAndOctaveFromPosition(
        slf: &Bound<'_, Self>,
        positionNote: i32,
    ) -> PyResult<(String, i32)> {
        let clef: String = slf.getattr("currentClef")?.extract()?;
        let (step, octave) = step_and_octave(&clef, positionNote);
        Ok((step.to_string(), octave))
    }

    /// music21's `getOnePitchFromPosition`: the pitch at a position, with
    /// the accidental written before it, else one written earlier in the
    /// measure on that pitch, else the key's.
    fn getOnePitchFromPosition<'py>(
        slf: &Bound<'py, Self>,
        pos: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let (written, position) = written_accidental(pos);
        let position: i32 = position.parse().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err(format!("invalid position: {pos}"))
        })?;
        let (step, octave) = Self::getStepAndOctaveFromPosition(slf, position)?;

        let classes = wheel(py)?;
        let pitch = classes
            .getattr("Pitch")?
            .call1((format!("{step}{octave}"),))?;
        let name: String = pitch.getattr("nameWithOctave")?.extract()?;
        let active = slf.getattr("activeAccidentals")?;
        let accidental = classes.getattr("Accidental")?;
        if let Some(written) = written {
            pitch.setattr("accidental", accidental.call1((written,))?)?;
            active.set_item(&name, written)?;
            return Ok(pitch);
        }
        if let Some(kept) = active.cast::<PyDict>()?.get_item(&name)? {
            pitch.setattr("accidental", accidental.call1((kept,))?)?;
            return Ok(pitch);
        }
        let from_key = slf
            .getattr("currentKey")?
            .call_method1("accidentalByStep", (step,))?;
        if !from_key.is_none() {
            pitch.setattr("accidental", from_key)?;
        }
        Ok(pitch)
    }

    /// music21's `getPitchFromPositionInfo`: a `Pos:` value's pitch, its
    /// tie and notehead marks dropped.
    fn getPitchFromPositionInfo<'py>(
        slf: &Bound<'py, Self>,
        posInfo: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        Self::getOnePitchFromPosition(slf, position_only(posInfo))
    }

    /// music21's `getMultiplePitchesFromPositionInfo`: a chord's `Pos:`
    /// value's pitches.
    fn getMultiplePitchesFromPositionInfo<'py>(
        slf: &Bound<'py, Self>,
        posInfo: &str,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        posInfo
            .split(',')
            .map(|one| Self::getPitchFromPositionInfo(slf, one))
            .collect()
    }
}

impl NoteworthyTranslator {
    /// The score `.nwctxt` text holds, read by the crate, kept as `score`
    /// too.
    fn read<'py>(slf: &Bound<'py, Self>, text: &str) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let score = crate::formats::handed_back(py, music21::noteworthy::from_noteworthy(text))?;
        slf.setattr("score", &score)?;
        Ok(score)
    }
}

/// A position's written accidental, as music21 spells it, and the rest.
fn written_accidental(pos: &str) -> (Option<&'static str>, &str) {
    let spelled = match pos.chars().next() {
        Some('n') => "natural",
        Some('b') => "-",
        Some('#') => "#",
        Some('x') => "##",
        Some('v') => "--",
        _ => return (None, pos),
    };
    (Some(spelled), &pos[1..])
}

/// A `Pos:` value without the tie and notehead marks after it.
fn position_only(posInfo: &str) -> &str {
    posInfo
        .trim_end_matches('^')
        .trim_end_matches('x')
        .trim_end_matches('X')
        .trim_end_matches('z')
}

/// music21's `NWCConverter`: reads a binary `.nwc` file into a score.
#[pyclass(
    name = "NWCConverter",
    module = "music21.noteworthy.binaryTranslate",
    subclass,
    dict
)]
pub struct NWCConverter;

#[pymethods]
impl NWCConverter {
    #[new]
    #[pyo3(signature = (**_keywords))]
    fn new(_keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    /// music21's starting state: no bytes read, version 200, no staves.
    #[pyo3(signature = (**_keywords))]
    fn __init__(slf: &Bound<'_, Self>, _keywords: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let py = slf.py();
        let empty = || PyBytes::new(py, b"");
        slf.setattr("fileContents", empty())?;
        slf.setattr("parsePosition", 0)?;
        slf.setattr("version", NWC_VERSION)?;
        slf.setattr("numberOfStaves", 0)?;
        slf.setattr("titlePageInfo", 0)?;
        slf.setattr("pageNumberStart", 0)?;
        slf.setattr("staves", PyList::empty(py))?;
        slf.setattr("comment", empty())?;
        slf.setattr("fonts", PyList::empty(py))?;
        slf.setattr("lyricist", empty())?;
        slf.setattr("groupVisibility", empty())?;
        slf.setattr("allowLayering", 1)?;
        slf.setattr("margins", empty())?;
        slf.setattr("notationTypeface", PyBytes::new(py, b"Maestro"))?;
        for unset in [
            "extendLastSystem",
            "increaseNoteSpacing",
            "measureStart",
            "measureNumbers",
            "mirrorMargins",
            "staffLabels",
            "sins",
        ] {
            slf.setattr(unset, py.None())?;
        }
        for text in ["copyright1", "copyright2", "author", "title", "user"] {
            slf.setattr(text, empty())?;
        }
        slf.setattr("staffHeight", 0)?;
        slf.setattr("currentAlterations", PyDict::new(py))?;
        Ok(())
    }

    /// music21's `parseFile`: the score a `.nwc` file holds, its bytes
    /// kept as `fileContents`.
    fn parseFile<'py>(slf: &Bound<'py, Self>, fp: PathBuf) -> PyResult<Bound<'py, PyAny>> {
        let bytes = std::fs::read(&fp)?;
        Self::read(slf, PyBytes::new(slf.py(), &bytes))
    }

    /// music21's `readLEShort`: the little-endian short at
    /// `parsePosition`, moving past it unless told `False`.
    #[pyo3(signature = (updateParsePosition = None))]
    fn readLEShort(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<i16> {
        let (bytes, at) = Self::cursor(slf)?;
        let Some(read) = bytes.get(at..at + 2) else {
            return Err(NoteworthyBinaryTranslateException::new_err(
                "unpack requires a buffer of 2 bytes",
            ));
        };
        Self::advance(slf, updateParsePosition.as_ref(), at + 2)?;
        Ok(i16::from_le_bytes([read[0], read[1]]))
    }

    /// music21's `byteToInt`: the byte at `parsePosition`, unsigned.
    #[pyo3(signature = (updateParsePosition = None))]
    fn byteToInt(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<u8> {
        let (bytes, at) = Self::cursor(slf)?;
        let Some(read) = bytes.get(at).copied() else {
            return Err(NoteworthyBinaryTranslateException::new_err(
                "no byte left to read",
            ));
        };
        Self::advance(slf, updateParsePosition.as_ref(), at + 1)?;
        Ok(read)
    }

    /// music21's `byteToSignedInt`: the same, signed.
    #[pyo3(signature = (updateParsePosition = None))]
    fn byteToSignedInt(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<i8> {
        Ok(i8::from_le_bytes([Self::byteToInt(
            slf,
            updateParsePosition,
        )?]))
    }

    /// music21's `readBytes`: the next `bytesToRead` bytes, as many as
    /// there are.
    #[pyo3(signature = (bytesToRead = 1, updateParsePosition = None))]
    fn readBytes<'py>(
        slf: &Bound<'py, Self>,
        bytesToRead: usize,
        updateParsePosition: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let (bytes, at) = Self::cursor(slf)?;
        let start = at.min(bytes.len());
        let end = (at + bytesToRead).min(bytes.len());
        Self::advance(slf, updateParsePosition.as_ref(), at + bytesToRead)?;
        Ok(PyBytes::new(slf.py(), &bytes[start..end]))
    }

    /// music21's `readToNUL`: the bytes up to the next nul, moving past
    /// it; with no nul, the rest, and the position is left at nought.
    #[pyo3(signature = (updateParsePosition = None))]
    fn readToNUL<'py>(
        slf: &Bound<'py, Self>,
        updateParsePosition: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let (bytes, at) = Self::cursor(slf)?;
        let start = at.min(bytes.len());
        let found = bytes[start..].iter().position(|byte| *byte == 0);
        let (end, next) = match found {
            Some(nul) => (start + nul, start + nul + 1),
            None => (bytes.len(), 0),
        };
        Self::advance(slf, updateParsePosition.as_ref(), next)?;
        Ok(PyBytes::new(slf.py(), &bytes[start..end]))
    }

    /// music21's `skipBytes`.
    #[pyo3(signature = (numBytes = 1))]
    fn skipBytes(slf: &Bound<'_, Self>, numBytes: usize) -> PyResult<()> {
        let (_, at) = Self::cursor(slf)?;
        slf.setattr("parsePosition", at + numBytes)
    }

    /// music21's `advanceToNotNUL`: past every `nul` byte from here.
    #[pyo3(signature = (nul = None))]
    fn advanceToNotNUL(slf: &Bound<'_, Self>, nul: Option<Vec<u8>>) -> PyResult<()> {
        let skipped = nul.unwrap_or_else(|| vec![0]);
        let (bytes, mut at) = Self::cursor(slf)?;
        while bytes.get(at..at + skipped.len()) == Some(&skipped[..]) {
            at += skipped.len();
        }
        slf.setattr("parsePosition", at)
    }

    /// music21's `parseString`: the same for bytes.
    #[pyo3(signature = (bytesIn = None))]
    fn parseString<'py>(
        slf: &Bound<'py, Self>,
        bytesIn: Option<Bound<'py, PyBytes>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let bytes = bytesIn.unwrap_or_else(|| PyBytes::new(slf.py(), b""));
        Self::read(slf, bytes)
    }
}

impl NWCConverter {
    /// The score in a file's bytes, kept as `fileContents`, inflated
    /// first where compressed, as music21's `parse` keeps them.
    fn read<'py>(
        slf: &Bound<'py, Self>,
        bytes: Bound<'py, PyBytes>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let bytes = match bytes.as_bytes().strip_prefix(COMPRESSED) {
            Some(packed) => inflated(py, packed)?,
            None => bytes,
        };
        slf.setattr("fileContents", &bytes)?;
        slf.setattr("parsePosition", 0)?;
        crate::formats::from_nwc(py, bytes.as_bytes())
            .map_err(|error| NoteworthyBinaryTranslateException::new_err(error.to_string()))
    }

    /// `fileContents` and `parsePosition`.
    fn cursor(slf: &Bound<'_, Self>) -> PyResult<(Vec<u8>, usize)> {
        Ok((
            slf.getattr("fileContents")?.extract()?,
            slf.getattr("parsePosition")?.extract()?,
        ))
    }

    /// `parsePosition` moved on to `to` unless `update` is given and is
    /// not `True` itself, as music21 asks `is True`.
    fn advance(
        slf: &Bound<'_, Self>,
        update: Option<&Bound<'_, PyAny>>,
        to: usize,
    ) -> PyResult<()> {
        let told = update.is_none_or(|update| update.is(pyo3::types::PyBool::new(slf.py(), true)));
        if told {
            slf.setattr("parsePosition", to)?;
        }
        Ok(())
    }
}

/// What a compressed `.nwc` file starts with; zlib's stream follows.
const COMPRESSED: &[u8] = b"[NWZ]\x00";

/// The most bytes a compressed file may inflate to: music21's
/// `common.fileTools.MAX_UNCOMPRESSED_BYTES`, read where music21 keeps it
/// so that a caller who changes it is heard.
fn inflate_limit(py: Python<'_>) -> PyResult<usize> {
    let module = py
        .import("music21.common.fileTools")
        .or_else(|_| crate::common::module(py, "music21.common.fileTools"))?;
    module.getattr("MAX_UNCOMPRESSED_BYTES")?.extract()
}

/// zlib's stream inflated, refused past the limit.
fn inflated<'py>(py: Python<'py>, packed: &[u8]) -> PyResult<Bound<'py, PyBytes>> {
    let limit = inflate_limit(py)?;
    let decompressor = py.import("zlib")?.call_method0("decompressobj")?;
    let made = decompressor
        .call_method1("decompress", (PyBytes::new(py, packed), limit + 1))?
        .cast_into::<PyBytes>()?;
    let left = decompressor.getattr("unconsumed_tail")?.len()?;
    if made.as_bytes().len() > limit || left > 0 {
        return Err(NoteworthyBinaryTranslateException::new_err(format!(
            "compressed .nwc data uncompresses to more than {limit} bytes"
        )));
    }
    Ok(made)
}

/// music21's `noteworthy.translate`.
fn translate(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, TRANSLATE)?;
    m.add_class::<NoteworthyTranslator>()?;
    let exception = py.get_type::<NoteworthyTranslateException>();
    exception.setattr("__module__", TRANSLATE)?;
    m.add("NoteworthyTranslateException", exception)?;
    Ok(m)
}

/// music21's `noteworthy.binaryTranslate`.
fn binary_translate(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, BINARY)?;
    m.add_class::<NWCConverter>()?;
    let exception = py.get_type::<NoteworthyBinaryTranslateException>();
    exception.setattr("__module__", BINARY)?;
    m.add("NoteworthyBinaryTranslateException", exception)?;
    Ok(m)
}

/// One of the package's modules, built with the others on first asking.
pub(crate) fn module<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    let built = BUILT.get_or_try_init(py, || {
        let made = PyDict::new(py);
        let package = PyModule::new(py, PACKAGE)?;
        let parts = [
            ("binaryTranslate", binary_translate(py)?),
            ("constants", constants(py)?),
            ("dictionaries", dictionaries(py)?),
            ("translate", translate(py)?),
        ];
        let names = PyList::empty(py);
        for (short, part) in parts {
            names.append(short)?;
            package.add(short, &part)?;
            made.set_item(part.name()?, part)?;
        }
        package.add("__all__", names)?;
        made.set_item(PACKAGE, package)?;
        Ok::<_, PyErr>(made.unbind())
    })?;
    let found = built
        .bind(py)
        .get_item(name)?
        .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(name.to_owned()))?;
    Ok(found.cast_into::<PyModule>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_loses_its_marks_and_keeps_its_accidental() {
        assert_eq!(position_only("b3^"), "b3");
        assert_eq!(written_accidental("b3"), (Some("-"), "3"));
        assert_eq!(written_accidental("x0"), (Some("##"), "0"));
        assert_eq!(written_accidental("-4"), (None, "-4"));
    }
}
