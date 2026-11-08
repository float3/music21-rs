//! `converter.parse` through the crate's readers, for the formats where
//! what they hand back is what music21's own reader makes.
//!
//! music21 reaches a reader through a `SubConverter` it looks up by format,
//! and lets a program register one of its own over the default. Each class
//! here is music21's own subconverter with `parseData` and `parseFile`
//! reading through the crate, so the score comes back as music21's streams
//! of the installed classes, built by `stream::music21_from_crate`. A call
//! that asks for anything the crate does not read -- a keyword such as
//! MIDI's `quarterLengthDivisors`, or a file it refuses -- goes to music21's
//! reader instead, as it would with nothing installed.
//!
//! Only the formats in [`ROUTED`] are registered by `install_into_music21`:
//! a format goes there once the score it reads is music21's to the last
//! attribute. `route_converters` registers any of the others on request.

use pyo3::prelude::*;
use pyo3::types::PyList;

/// The formats `install_into_music21` routes through the crate.
///
/// Each reads, file for file, as the same score music21's reader makes, to
/// the last attribute, as `tests/converter_parity.py` measures it: ABC 55
/// of 55 tunes, Humdrum 40 of 40 files, MEI 3 of 3 and MIDI 21 of 21. A
/// MusicXML score still lacks where its notes, directions and marks are
/// drawn, in what font and colour, and the score's own appearance and
/// fonts, so `route_converters` routes it only on request.
pub(crate) const ROUTED: &[&str] = &["abc", "midi", "humdrum", "mei"];

/// The Python half: one subconverter per format, and the registering.
const CONVERTERS: &str = r#"
from music21 import converter as _converter
from music21.converter import subConverters as _sub

# The module holding the wheel's readers, set when this one is built.
wheel = None


def _text(data):
    """A document's text, from text or from the bytes of one."""
    if not isinstance(data, bytes):
        return data
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return None


def _read_file(path, binary):
    with open(path, 'rb') as handle:
        data = handle.read()
    return data if binary else _text(data)


class ThroughCrate:
    """What every subconverter here does: read through the crate where
    nothing asks for what only music21's reader does, else as music21."""

    # Whether the last read went through the crate.
    readByCrate = False

    def _crate(self, data, number):
        raise NotImplementedError

    def _routable(self, keywords):
        return not keywords and not self.keywords

    def _through_crate(self, data, number):
        if data is None:
            return False
        try:
            made = self._crate(data, number)
        except Exception:
            # music21 has its own word for what is wrong with a file; it
            # reads it again and says so.
            return False
        self.stream = made
        self.readByCrate = True
        return True

    def parseData(self, dataString, number=None, **keywords):
        self.readByCrate = False
        if self._routable(keywords) and self._through_crate(dataString, number):
            return self.stream
        return super().parseData(dataString, number, **keywords)

    def parseFile(self, filePath, number=None, **keywords):
        self.readByCrate = False
        if self._routable(keywords):
            data = self._file_data(filePath)
            if self._through_crate(data, number):
                self._read_from(filePath)
                return self.stream
        return super().parseFile(filePath, number, **keywords)

    def _file_data(self, filePath):
        return _read_file(filePath, self.readBinary)

    def _read_from(self, filePath):
        """What music21's `parseFile` adds to a score for the file it read."""


class ConverterMusicXML(ThroughCrate, _sub.ConverterMusicXML):
    def _crate(self, data, number):
        return wheel.from_musicxml(_text(data))

    def _file_data(self, filePath):
        archive = _converter.ArchiveManager(filePath)
        if archive.isArchive():
            return _text(archive.getData())
        return _read_file(filePath, False)

    def _read_from(self, filePath):
        if self.stream.metadata.movementName is None:
            self.stream.metadata.movementName = str(filePath).replace('\\', '/').rsplit('/', 1)[-1]


class ConverterABC(ThroughCrate, _sub.ConverterABC):
    def _crate(self, data, number):
        if number is None:
            made = wheel.from_abc(_text(data))
        else:
            made = wheel.from_abc_number(_text(data), number)
        # music21's reader gives each note its length, which leaves how it
        # is written inferred, but for a grace note's; the crate's lengths
        # say what a copy of the beamed tune says.
        for each in made.recurse().notesAndRests:
            if not each.duration.isGrace:
                each.duration.expressionIsInferred = True
        return made


class ConverterMidi(ThroughCrate, _sub.ConverterMidi):
    def _routable(self, keywords):
        # music21 drops an empty `encoding` itself.
        keywords = {key: value for key, value in keywords.items()
                    if not (key == 'encoding' and not value)}
        given = {key: value for key, value in self.keywords.items()
                 if not (key == 'encoding' and not value)}
        return not keywords and not given

    def _crate(self, data, number):
        if isinstance(data, str):
            return None
        return wheel.from_midi(data)


class ConverterHumdrum(ThroughCrate, _sub.ConverterHumdrum):
    def _routable(self, keywords):
        # The crate reads standard Humdrum only; the Josquin Research
        # Project's flavour, whose dots go on the note, is music21's.
        from music21.humdrum import spineParser
        if spineParser.flavors.get('JRP'):
            return False
        return super()._routable(keywords)

    def _crate(self, data, number):
        return wheel.from_humdrum(_text(data))


class ConverterMEI(ThroughCrate, _sub.ConverterMEI):
    def _crate(self, data, number):
        text = _text(data)
        if text.startswith('mei:'):
            text = text[4:]
        return wheel.from_mei(text)


# music21's name for each, so a listing of the converters reads as it does
# with nothing installed.
for _made, _original in ((ConverterMusicXML, _sub.ConverterMusicXML),
                         (ConverterABC, _sub.ConverterABC),
                         (ConverterMidi, _sub.ConverterMidi),
                         (ConverterHumdrum, _sub.ConverterHumdrum),
                         (ConverterMEI, _sub.ConverterMEI)):
    _made.__module__ = _original.__module__
    _made.__qualname__ = _original.__qualname__

by_format = {
    'musicxml': ConverterMusicXML,
    'abc': ConverterABC,
    'midi': ConverterMidi,
    'humdrum': ConverterHumdrum,
    'mei': ConverterMEI,
}


def route(formats):
    """Registers the subconverters for `formats` with music21, and takes
    away those for every other format. Answers the formats now routed."""
    unknown = [name for name in formats if name not in by_format]
    if unknown:
        raise ValueError(f'no reader of the crate is routed for {unknown!r}; '
                         f'it routes {sorted(by_format)!r}')
    for made in by_format.values():
        try:
            _converter.unregisterSubConverter(made)
        except _converter.ConverterException:
            pass
    routed = []
    for name in by_format:
        if name in formats:
            _converter.registerSubConverter(by_format[name])
            routed.append(name)
    return routed
"#;

static CONVERTERS_MODULE: pyo3::sync::PyOnceLock<Py<PyModule>> = pyo3::sync::PyOnceLock::new();

/// The Python half, built once per interpreter, with the wheel's readers.
fn converters(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    CONVERTERS_MODULE
        .get_or_try_init(py, || {
            let module = PyModule::from_code(
                py,
                &std::ffi::CString::new(CONVERTERS)?,
                c"music21_rs_converters.py",
                c"music21_rs_converters",
            )?;
            let readers = PyModule::new(py, "music21_rs")?;
            crate::formats::register(&readers)?;
            module.setattr("wheel", readers)?;
            Ok::<_, PyErr>(module.unbind())
        })
        .map(|module| module.bind(py).clone())
}

/// Registers the formats [`ROUTED`] names; what `install_into_music21`
/// calls.
pub(crate) fn install(py: Python<'_>) -> PyResult<()> {
    converters(py)?
        .getattr("route")?
        .call1((ROUTED.to_vec(),))?;
    Ok(())
}

/// Routes music21's `converter.parse` through this package's readers for
/// `formats`, and back to music21's own for every other one, answering the
/// formats now routed. Call it after `install_into_music21`.
///
/// The formats are `musicxml`, `abc`, `midi`, `humdrum` and `mei`. The
/// score comes back as music21's streams of the installed classes, missing
/// what the matching `from_*` reader leaves out, so a format is worth
/// routing where speed matters more than that; `install_into_music21`
/// routes only those whose scores are music21's in full. A parse given a
/// keyword, such as MIDI's `quarterLengthDivisors`, or a file the crate
/// refuses, is music21's own. `route_converters([])` routes none.
#[pyfunction]
fn route_converters(py: Python<'_>, formats: Vec<String>) -> PyResult<Bound<'_, PyList>> {
    converters(py)?
        .getattr("route")?
        .call1((formats,))?
        .cast_into::<PyList>()
        .map_err(Into::into)
}

pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(route_converters, m)?)?;
    Ok(())
}
