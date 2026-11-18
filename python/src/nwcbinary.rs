//! music21's `noteworthy.binaryTranslate`: `NWCConverter`, `NWCStaff` and
//! `NWCObject` over the crate's `noteworthy::NwcReader`.
//!
//! music21's objects read the file through the converter they belong to,
//! moving its `parsePosition` and `currentAlterations` on as they go, and
//! keep what they read as attributes. Each `parse` here hands the
//! converter's bytes, position, version and alterations to the crate's
//! reader, writes back where it got to, and keeps what it read as
//! music21's attributes. A staff's `dump` and the converter's
//! `dumpToNWCText` are written from those attributes, as music21's are.

// music21's own names, kept as music21 spells them.
#![allow(non_snake_case)]

use std::path::PathBuf;

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyBytes, PyDict, PyList, PyTuple};

use music21::noteworthy::{MIDI_INSTRUMENTS, Member, NwcObject, NwcReader, NwcStaff, Object};

use crate::Walkable;

/// The module music21 keeps these in.
pub(crate) const MODULE: &str = "music21.noteworthy.binaryTranslate";

/// What a compressed `.nwc` file starts with; zlib's stream follows.
const COMPRESSED: &[u8] = b"[NWZ]\x00";

/// The version of the binary format music21's converter starts at.
const NWC_VERSION: u32 = 200;

/// The first and fourth strings of a valid file.
const ARTWARE: &[u8] = b"[NoteWorthy ArtWare]";
const COMPOSER: &[u8] = b"[NoteWorthy Composer]";

/// Where a file says its version.
const VERSION_AT: usize = 45;

/// music21's `NWCConverter.versionFromHex`.
const VERSIONS: [(i32, u32); 8] = [
    (0x0114, 120),
    (0x011E, 130),
    (0x0132, 150),
    (0x0137, 155),
    (0x0146, 170),
    (0x014B, 175),
    (0x0200, 200),
    (0x0201, 201),
];

/// The version music21 takes a file of an unknown one to be.
const NEWEST: u32 = 201;

/// The instrument a staff that names none is written with.
const DEFAULT_INSTRUMENT: &str = "Acoustic Grand Piano";

pyo3::create_exception!(
    music21_rs_facade,
    NoteworthyBinaryTranslateException,
    crate::Music21Exception
);

fn binary_error(error: music21::Error) -> PyErr {
    NoteworthyBinaryTranslateException::new_err(crate::pitch::message(&error))
}

/// Bytes as music21 decodes the file's strings.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

/// `parsePosition` moved on to `to` unless `update` is given and is not
/// `True` itself, as music21 asks `is True`.
fn advance(
    parser: &Bound<'_, PyAny>,
    update: Option<&Bound<'_, PyAny>>,
    to: usize,
) -> PyResult<()> {
    let told = update.is_none_or(|update| update.is(PyBool::new(parser.py(), true)));
    if told {
        parser.setattr("parsePosition", to)?;
    }
    Ok(())
}

/// A converter's `fileContents` and `parsePosition`.
fn cursor(parser: &Bound<'_, PyAny>) -> PyResult<(Vec<u8>, usize)> {
    Ok((
        parser.getattr("fileContents")?.extract()?,
        parser.getattr("parsePosition")?.extract()?,
    ))
}

/// Runs the crate's reader over a converter's bytes from its position,
/// with its version and alterations, and writes back where it got to.
fn reading<T>(
    parser: &Bound<'_, PyAny>,
    read: impl FnOnce(&mut NwcReader<'_>) -> music21::Result<T>,
) -> PyResult<T> {
    let (bytes, at) = cursor(parser)?;
    let mut reader = NwcReader::new(&bytes, parser.getattr("version")?.extract()?);
    reader.at = at;
    let held = parser.getattr("currentAlterations")?;
    for pair in held.call_method0("items")?.walk()? {
        reader.alterations.push(pair?.extract()?);
    }
    let read = read(&mut reader).map_err(binary_error)?;

    parser.setattr("parsePosition", reader.at)?;
    parser.setattr("version", reader.version)?;
    let alterations = PyDict::new(parser.py());
    for (place, text) in reader.alterations {
        alterations.set_item(place, text)?;
    }
    parser.setattr("currentAlterations", alterations)?;
    Ok(read)
}

/// music21's `NWCConverter`: a binary `.nwc` file read into staves and
/// objects, then into a score through the text translator.
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

    /// music21's `parseFile`: a `.nwc` file's bytes read, parsed and made
    /// a score.
    fn parseFile<'py>(slf: &Bound<'py, Self>, fp: PathBuf) -> PyResult<Bound<'py, PyAny>> {
        let bytes = std::fs::read(&fp)?;
        slf.setattr("fileContents", PyBytes::new(slf.py(), &bytes))?;
        Self::parse(slf)?;
        Self::toStream(slf)
    }

    /// music21's `parseString`: the same for bytes.
    #[pyo3(signature = (bytesIn = None))]
    fn parseString<'py>(
        slf: &Bound<'py, Self>,
        bytesIn: Option<Bound<'py, PyBytes>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let bytes = bytesIn.unwrap_or_else(|| PyBytes::new(slf.py(), b""));
        slf.setattr("fileContents", bytes)?;
        Self::parse(slf)?;
        Self::toStream(slf)
    }

    /// music21's `readLEShort`: the little-endian short at
    /// `parsePosition`, moving past it unless told `False`.
    #[pyo3(signature = (updateParsePosition = None))]
    fn readLEShort(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<i16> {
        let (bytes, at) = cursor(slf.as_any())?;
        let Some(read) = bytes.get(at..at + 2) else {
            return Err(NoteworthyBinaryTranslateException::new_err(
                "unpack requires a buffer of 2 bytes",
            ));
        };
        advance(slf.as_any(), updateParsePosition.as_ref(), at + 2)?;
        Ok(i16::from_le_bytes([read[0], read[1]]))
    }

    /// music21's `byteToInt`: the byte at `parsePosition`, unsigned.
    #[pyo3(signature = (updateParsePosition = None))]
    fn byteToInt(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<u8> {
        let (bytes, at) = cursor(slf.as_any())?;
        let Some(read) = bytes.get(at).copied() else {
            return Err(NoteworthyBinaryTranslateException::new_err(
                "no byte left to read",
            ));
        };
        advance(slf.as_any(), updateParsePosition.as_ref(), at + 1)?;
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
        let (bytes, at) = cursor(slf.as_any())?;
        let start = at.min(bytes.len());
        let end = (at + bytesToRead).min(bytes.len());
        advance(slf.as_any(), updateParsePosition.as_ref(), at + bytesToRead)?;
        Ok(PyBytes::new(slf.py(), &bytes[start..end]))
    }

    /// music21's `readToNUL`: the bytes up to the next nul, moving past
    /// it; with no nul, the rest, and the position is left at nought.
    #[pyo3(signature = (updateParsePosition = None))]
    fn readToNUL<'py>(
        slf: &Bound<'py, Self>,
        updateParsePosition: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let (bytes, at) = cursor(slf.as_any())?;
        let start = at.min(bytes.len());
        let found = bytes[start..].iter().position(|byte| *byte == 0);
        let (end, next) = match found {
            Some(nul) => (start + nul, start + nul + 1),
            None => (bytes.len(), 0),
        };
        advance(slf.as_any(), updateParsePosition.as_ref(), next)?;
        Ok(PyBytes::new(slf.py(), &bytes[start..end]))
    }

    /// music21's `isValidNWCFile`: whether the file opens as NoteWorthy's
    /// files do. Read from the start, and the position kept where told
    /// `False`.
    #[pyo3(signature = (updateParsePosition = None))]
    fn isValidNWCFile(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let stored: usize = slf.getattr("parsePosition")?.extract()?;
        slf.setattr("parsePosition", 0)?;
        let first = Self::readToNUL(slf, None)?;
        if first.as_bytes() != ARTWARE {
            return Ok(false);
        }
        Self::readToNUL(slf, None)?;
        Self::readToNUL(slf, None)?;
        let fourth = Self::readToNUL(slf, None)?;
        if fourth.as_bytes() != COMPOSER {
            return Ok(false);
        }
        let keep =
            updateParsePosition.is_some_and(|update| update.is(PyBool::new(slf.py(), false)));
        if keep && stored != 0 {
            slf.setattr("parsePosition", stored)?;
        }
        Ok(true)
    }

    /// music21's `fileVersion`: the version the file says at byte 45, as
    /// music21 numbers versions, kept as `version`; an unknown one is taken
    /// to be the newest.
    #[pyo3(signature = (updateParsePosition = None))]
    fn fileVersion(
        slf: &Bound<'_, Self>,
        updateParsePosition: Option<Bound<'_, PyAny>>,
    ) -> PyResult<u32> {
        let stored: usize = slf.getattr("parsePosition")?.extract()?;
        slf.setattr("parsePosition", VERSION_AT)?;
        let raw = Self::readLEShort(slf, updateParsePosition.clone())?;
        if updateParsePosition.is_some_and(|update| update.is(PyBool::new(slf.py(), false))) {
            slf.setattr("parsePosition", stored)?;
        }
        let version = VERSIONS
            .iter()
            .find(|(hex, _)| *hex == i32::from(raw))
            .map_or(NEWEST, |(_, version)| *version);
        slf.setattr("version", version)?;
        Ok(version)
    }

    /// music21's `skipBytes`.
    #[pyo3(signature = (numBytes = 1))]
    fn skipBytes(slf: &Bound<'_, Self>, numBytes: usize) -> PyResult<()> {
        let (_, at) = cursor(slf.as_any())?;
        slf.setattr("parsePosition", at + numBytes)
    }

    /// music21's `advanceToNotNUL`: past every `nul` byte from here.
    #[pyo3(signature = (nul = None))]
    fn advanceToNotNUL(slf: &Bound<'_, Self>, nul: Option<Vec<u8>>) -> PyResult<()> {
        let skipped = nul.unwrap_or_else(|| vec![0]);
        let (bytes, mut at) = cursor(slf.as_any())?;
        while !skipped.is_empty() && bytes.get(at..at + skipped.len()) == Some(&skipped[..]) {
            at += skipped.len();
        }
        slf.setattr("parsePosition", at)
    }

    /// music21's `parse`: the file inflated where compressed, its header
    /// read, then each staff.
    fn parse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let contents = slf.getattr("fileContents")?.cast_into::<PyBytes>()?;
        if let Some(packed) = contents.as_bytes().strip_prefix(COMPRESSED) {
            let packed = packed.to_vec();
            slf.setattr("fileContents", inflated(py, &packed)?)?;
        }
        slf.setattr("parsePosition", 0)?;
        Self::parseHeader(slf)?;
        let staves = PyList::empty(py);
        let count: usize = slf.getattr("numberOfStaves")?.extract()?;
        for _ in 0..count {
            let staff = Bound::new(py, NWCStaff)?;
            NWCStaff::__init__(&staff, slf.as_any())?;
            NWCStaff::parse(&staff)?;
            staves.append(staff)?;
        }
        slf.setattr("staves", staves)
    }

    /// music21's `parseHeader`: what the header says, kept as attributes,
    /// and the position moved past it.
    fn parseHeader(slf: &Bound<'_, Self>) -> PyResult<()> {
        let py = slf.py();
        let header = reading(slf.as_any(), |reader| reader.header())?;
        let bytes = |value: &[u8]| PyBytes::new(py, value);
        slf.setattr("user", bytes(&header.user))?;
        slf.setattr("title", bytes(&header.title))?;
        slf.setattr("author", bytes(&header.author))?;
        if header.version >= 200 {
            slf.setattr("lyricist", bytes(&header.lyricist))?;
        }
        slf.setattr("copyright1", bytes(&header.copyright1))?;
        slf.setattr("copyright2", bytes(&header.copyright2))?;
        slf.setattr("comment", bytes(&header.comment))?;
        slf.setattr("extendLastSystem", header.extend_last_system)?;
        slf.setattr("increaseNoteSpacing", header.increase_note_spacing)?;
        slf.setattr("measureNumbers", header.measure_numbers)?;
        slf.setattr("measureStart", header.measure_start)?;
        slf.setattr("margins", bytes(&header.margins))?;
        if header.version >= 130 {
            slf.setattr("groupVisibility", bytes(&header.group_visibility))?;
            slf.setattr("allowLayering", header.allow_layering)?;
        }
        if header.version >= 200 {
            slf.setattr("notationTypeface", bytes(&header.notation_typeface))?;
        }
        slf.setattr("staffHeight", header.staff_height)?;
        let fonts = PyList::empty(py);
        for font in &header.fonts {
            let made = PyDict::new(py);
            made.set_item("name", bytes(&font.name))?;
            made.set_item("style", font.style)?;
            made.set_item("size", font.size)?;
            made.set_item("charset", font.charset)?;
            fonts.append(made)?;
        }
        slf.setattr("fonts", fonts)?;
        slf.setattr("titlePageInfo", header.title_page_info)?;
        slf.setattr("staffLabels", header.staff_labels)?;
        slf.setattr("pageNumberStart", header.page_number_start)?;
        slf.setattr("numberOfStaves", header.number_of_staves)
    }

    /// music21's `dumpToNWCText`: the title and author, then each staff's
    /// lines.
    fn dumpToNWCText(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        let mut info = String::new();
        let title: Vec<u8> = slf.getattr("title")?.extract()?;
        let author: Vec<u8> = slf.getattr("author")?.extract()?;
        if !title.is_empty() {
            info.push_str("|SongInfo|Title:");
            info.push_str(&latin1(&title));
        }
        if !author.is_empty() {
            info.push_str("|Author:");
            info.push_str(&latin1(&author));
        }
        let mut lines = vec![info];
        for staff in slf.getattr("staves")?.walk()? {
            let dumped: Vec<String> = staff?.call_method0("dump")?.extract()?;
            lines.extend(dumped);
        }
        Ok(lines)
    }

    /// music21's `toStream`: the text lines read by the translator.
    fn toStream<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        let py = slf.py();
        let lines = PyList::new(py, Self::dumpToNWCText(slf)?)?;
        let translator = crate::nwctranslate::module(py)?
            .getattr("NoteworthyTranslator")?
            .call0()?;
        translator.call_method1("parseList", (lines,))
    }
}

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

/// music21's `NWCStaff`: a staff's settings, verses and objects.
#[pyclass(
    name = "NWCStaff",
    module = "music21.noteworthy.binaryTranslate",
    subclass,
    dict
)]
pub struct NWCStaff;

#[pymethods]
impl NWCStaff {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(_arguments: &Bound<'_, PyTuple>, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    /// music21's starting state, read from `parent`.
    fn __init__(slf: &Bound<'_, Self>, parent: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("parent", parent)?;
        slf.setattr("lyrics", PyList::empty(py))?;
        slf.setattr("objects", PyList::empty(py))?;
        slf.setattr("instrumentName", PyBytes::new(py, b""))?;
        for unset in [
            "group",
            "layerWithNextStaff",
            "transposition",
            "partVolume",
            "stereoPan",
            "alignSyllable",
            "name",
        ] {
            slf.setattr(unset, py.None())?;
        }
        for zero in [
            "color",
            "numberOfLyrics",
            "numberOfObjects",
            "lines",
            "staffOffset",
            "lyricAlignment",
        ] {
            slf.setattr(zero, 0)?;
        }
        slf.setattr("label", PyBytes::new(py, b""))
    }

    /// music21's `parse`: the staff's header, verses and objects.
    fn parse(slf: &Bound<'_, Self>) -> PyResult<()> {
        Self::parseHeader(slf)?;
        Self::parseLyrics(slf)?;
        Self::parseObjects(slf)?;
        Ok(())
    }

    /// music21's `parseHeader`.
    fn parseHeader(slf: &Bound<'_, Self>) -> PyResult<()> {
        let parent = slf.getattr("parent")?;
        let version: u32 = parent.getattr("version")?.extract()?;
        let staff = reading(&parent, |reader| reader.staff_header())?;
        Self::keep_header(slf, &staff, version)
    }

    /// music21's `parseLyrics`: each verse's syllables.
    fn parseLyrics<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let parent = slf.getattr("parent")?;
        let verses: i32 = slf.getattr("numberOfLyrics")?.extract()?;
        let read = reading(&parent, |reader| reader.staff_lyrics(verses))?;
        let lyrics = PyList::empty(py);
        for verse in read {
            let syllables: Vec<Bound<'_, PyBytes>> = verse
                .iter()
                .map(|syllable| PyBytes::new(py, syllable))
                .collect();
            lyrics.append(syllables)?;
        }
        slf.setattr("lyrics", &lyrics)?;
        Ok(lyrics)
    }

    /// music21's `parseObjects`: the staff's objects.
    fn parseObjects<'py>(slf: &Bound<'py, Self>) -> PyResult<Bound<'py, PyList>> {
        let py = slf.py();
        let parent = slf.getattr("parent")?;
        let (count, read) = reading(&parent, |reader| reader.staff_objects())?;
        slf.setattr("numberOfObjects", count)?;
        let objects = PyList::empty(py);
        for object in &read {
            objects.append(NWCObject::made(&parent, object)?)?;
        }
        slf.setattr("objects", &objects)?;
        Ok(objects)
    }

    /// music21's `dump`: the staff and its instrument as text lines, then
    /// each object that writes one.
    fn dump(slf: &Bound<'_, Self>) -> PyResult<Vec<String>> {
        let named: Vec<u8> = slf.getattr("instrumentName")?.extract()?;
        let instrument = if named.is_empty() {
            DEFAULT_INSTRUMENT.to_string()
        } else {
            latin1(&named)
        };
        let label: Vec<u8> = slf.getattr("label")?.extract()?;
        let label = if label.is_empty() {
            instrument.clone()
        } else {
            latin1(&label)
        };
        let mut lines = vec![format!("|AddStaff|Name:{label}")];
        let mut staff_instrument = format!("|StaffInstrument|Name:{instrument}");
        if let Some(patch) = MIDI_INSTRUMENTS.iter().position(|name| *name == instrument) {
            staff_instrument.push_str(&format!("|Patch:{patch}"));
        }
        staff_instrument.push_str(&format!("|Trans:{}", slf.getattr("transposition")?.str()?));
        lines.push(staff_instrument);
        for object in slf.getattr("objects")?.walk()? {
            let object = object?;
            let dumped: String = object.getattr("dumpMethod")?.call1((&object,))?.extract()?;
            if !dumped.is_empty() {
                lines.push(dumped);
            }
        }
        Ok(lines)
    }
}

impl NWCStaff {
    /// What a staff's header said, as music21's attributes.
    fn keep_header(slf: &Bound<'_, Self>, staff: &NwcStaff, version: u32) -> PyResult<()> {
        let py = slf.py();
        let bytes = |value: &[u8]| PyBytes::new(py, value);
        slf.setattr("name", bytes(&staff.name))?;
        if version >= 200 {
            slf.setattr("label", bytes(&staff.label))?;
        }
        if version >= 200 || version == 175 {
            slf.setattr("instrumentName", bytes(&staff.instrument_name))?;
        }
        slf.setattr("group", bytes(&staff.group))?;
        if version >= 200 {
            slf.setattr("lines", staff.lines)?;
            slf.setattr("layerWithNextStaff", staff.layer_with_next_staff)?;
            slf.setattr("partVolume", staff.part_volume)?;
            slf.setattr("stereoPan", staff.stereo_pan)?;
            slf.setattr("color", staff.color)?;
        }
        slf.setattr("transposition", staff.transposition)?;
        slf.setattr("alignSyllable", staff.align_syllable)?;
        slf.setattr("numberOfLyrics", staff.number_of_lyrics)?;
        slf.setattr("lyricAlignment", staff.lyric_alignment)?;
        slf.setattr("staffOffset", staff.staff_offset)
    }
}

/// music21's `NWCObject`: one object of a staff, as music21's attributes,
/// with `dumpMethod` writing its text line.
#[pyclass(
    name = "NWCObject",
    module = "music21.noteworthy.binaryTranslate",
    subclass,
    dict
)]
pub struct NWCObject;

/// The function every object's `dumpMethod` is: the line it was read as.
static DUMP: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// music21's `dumpMethod`: an object's text line, or nothing.
#[pyfunction]
#[pyo3(name = "_dumped")]
fn dumped(inner_self: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    match inner_self.getattr("_line") {
        Ok(line) => Ok(line.unbind()),
        Err(_) => Ok(pyo3::types::PyString::new(inner_self.py(), "")
            .into_any()
            .unbind()),
    }
}

fn dump_method(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    Ok(DUMP
        .get_or_try_init(py, || {
            Ok::<_, PyErr>(wrap_pyfunction!(dumped, py)?.into_any().unbind())
        })?
        .bind(py)
        .clone())
}

/// music21's attributes of an object and their starting values.
const OBJECT_DEFAULTS: [(&str, i64); 21] = [
    ("placement", 0),
    ("pos", 0),
    ("style", 0),
    ("localRepeatCount", 0),
    ("data", 0),
    ("delay", 0),
    ("clefType", 0),
    ("offset", 0),
    ("visible", 0),
    ("duration", 0),
    ("font", 0),
    ("sharps", 0),
    ("octaveShift", 0),
    ("attribute2", 0),
    ("stemLength", 0),
    ("dots", 0),
    ("bits", 0),
    ("denominator", 0),
    ("volume", 0),
    ("base", 0),
    ("velocity", 0),
];

#[pymethods]
impl NWCObject {
    #[new]
    #[pyo3(signature = (*_arguments, **_keywords))]
    fn new(_arguments: &Bound<'_, PyTuple>, _keywords: Option<&Bound<'_, PyDict>>) -> Self {
        Self
    }

    /// music21's starting state, read through `parserParent`.
    fn __init__(slf: &Bound<'_, Self>, parserParent: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::blank(slf.as_any(), parserParent)
    }

    /// music21's `parse`: the object at the converter's position.
    fn parse(slf: &Bound<'_, Self>) -> PyResult<()> {
        let parent = slf.getattr("parserParent")?;
        let read = reading(&parent, |reader| reader.object())?;
        Self::keep(slf.as_any(), &read)
    }
}

impl NWCObject {
    /// music21's attributes as an object starts out.
    fn blank(slf: &Bound<'_, PyAny>, parent: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("parserParent", parent)?;
        for (name, value) in OBJECT_DEFAULTS {
            slf.setattr(name, value)?;
        }
        for unset in [
            "type",
            "data1",
            "data2",
            "data3",
            "durationStr",
            "octaveShiftName",
            "clefName",
            "attribute1",
            "name",
            "dotAttribute",
            "text",
        ] {
            slf.setattr(unset, py.None())?;
        }
        for (name, value) in [("count", 0), ("value", 0), ("flats", 0), ("numerator", 0)] {
            slf.setattr(name, value)?;
        }
        for text in ["tieInfo", "keyString", "alterationStr"] {
            slf.setattr(text, "")?;
        }
        slf.setattr("dumpMethod", dump_method(py)?)
    }

    /// A new object holding what the crate read.
    fn made<'py>(parent: &Bound<'py, PyAny>, read: &NwcObject) -> PyResult<Bound<'py, PyAny>> {
        let made = Bound::new(parent.py(), Self)?.into_any();
        Self::blank(&made, parent)?;
        Self::keep(&made, read)?;
        Ok(made)
    }

    /// What the crate read, as music21's attributes.
    fn keep(slf: &Bound<'_, PyAny>, read: &NwcObject) -> PyResult<()> {
        let py = slf.py();
        slf.setattr("type", read.type_name())?;
        slf.setattr("visible", read.visible)?;
        slf.setattr("_line", read.line())?;
        match &read.body {
            Object::Clef { name, shift } => {
                slf.setattr("clefName", *name)?;
                slf.setattr("octaveShiftName", *shift)?;
            }
            Object::Key(signature) => slf.setattr("keyString", *signature)?,
            Object::Bar(style) | Object::Ending(style) => slf.setattr("style", *style)?,
            Object::TimeSig(numerator, denominator) => {
                slf.setattr("numerator", *numerator)?;
                slf.setattr("denominator", *denominator)?;
            }
            Object::Tempo(value) => slf.setattr("value", *value)?,
            Object::Note(member) => Self::keep_member(slf, member)?,
            Object::Rest(duration) => slf.setattr("durationStr", duration)?,
            Object::Chord {
                members,
                stem_length,
            } => {
                slf.setattr("stemLength", *stem_length)?;
                slf.setattr("data2", Self::members(slf, members)?)?;
            }
            Object::RestChord { members, rest } => {
                let held = Self::members(slf, members)?;
                let resting = Self::member_object(slf, None)?;
                resting.setattr("type", "Rest")?;
                resting.setattr("durationStr", rest)?;
                held.append(resting)?;
                slf.setattr("data2", held)?;
            }
            Object::Text(text, position) => {
                slf.setattr("text", PyBytes::new(py, text))?;
                slf.setattr("pos", *position)?;
            }
            Object::Silent(_) => {}
        }
        Ok(())
    }

    /// A note's attributes.
    fn keep_member(slf: &Bound<'_, PyAny>, member: &Member) -> PyResult<()> {
        slf.setattr("durationStr", &member.duration)?;
        slf.setattr("pos", member.position)?;
        slf.setattr("alterationStr", &member.alteration)?;
        slf.setattr("tieInfo", member.tie)?;
        slf.setattr("stemLength", member.stem_length)
    }

    /// A chord's notes as objects of their own.
    fn members<'py>(slf: &Bound<'py, PyAny>, members: &[Member]) -> PyResult<Bound<'py, PyList>> {
        let made = PyList::empty(slf.py());
        for member in members {
            made.append(Self::member_object(slf, Some(member))?)?;
        }
        Ok(made)
    }

    fn member_object<'py>(
        slf: &Bound<'py, PyAny>,
        member: Option<&Member>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let parent = slf.getattr("parserParent")?;
        let made = Bound::new(slf.py(), Self)?.into_any();
        Self::blank(&made, &parent)?;
        if let Some(member) = member {
            made.setattr("type", "Note")?;
            Self::keep_member(&made, member)?;
            let note = Object::Note(member.clone());
            made.setattr(
                "_line",
                NwcObject {
                    visible: 0,
                    body: note,
                }
                .line(),
            )?;
        }
        Ok(made)
    }
}

/// music21's `noteworthy.binaryTranslate`.
pub(crate) fn module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let m = PyModule::new(py, MODULE)?;
    m.add_class::<NWCConverter>()?;
    m.add_class::<NWCStaff>()?;
    m.add_class::<NWCObject>()?;
    let exception = py.get_type::<NoteworthyBinaryTranslateException>();
    exception.setattr("__module__", MODULE)?;
    m.add("NoteworthyBinaryTranslateException", exception)?;
    m.add(
        "environLocal",
        crate::environment::module(py)?
            .getattr("Environment")?
            .call1(("noteworthy.binaryTranslate",))?,
    )?;
    Ok(m)
}
