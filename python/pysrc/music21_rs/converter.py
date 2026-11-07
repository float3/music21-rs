"""music21's `converter`, over the crate's readers and writers.

`parse` takes what music21's `converter.parse` takes -- a path, the text or
bytes of a document, a `(path, number)` pair, a URL -- tells the format from
the file's extension or the document's first characters as music21 does,
and reads it with the wheel's reader for that format: MusicXML (a `.mxl`
unzipped first), ABC, MIDI, Humdrum, MEI, RomanText, de Clercq-Temperley,
TinyNotation, Volpiano, Capella, NoteWorthy (text and binary), MuseData
(a file, a directory or a zip of a work's parts), and MuseScore's `.mscx`
and `.mscz`, which music21 does not read itself.

On its own, the wheel hands back its own streams, and needs no music21.
Once `install_into_music21` has run, these are music21's own `parse`,
`parseFile`, `parseData`, `parseURL` and `toData`, reading what
`install_into_music21` routes through the crate and the rest with
music21's readers, its pickles included; MuseScore's files, which music21
has no reader for, are still read by the crate, into music21's streams.

There is no cache: every parse reads the file. `forceSource` and
`storePickle` are taken and mean nothing. A keyword the crate's reader
cannot honour -- MIDI's `quarterLengthDivisors`, TinyNotation's
`makeNotation=False` -- raises `ConverterException` rather than being
ignored.

`ConverterException` and `ConverterFileException` are music21's own where
music21 can be imported, as the wheel's other exceptions are built on
music21's, and stand-ins of their own where it cannot. They are made on
first asking, so importing this module imports no music21.
"""

import io
import os
import pathlib
import re
import sys
import tempfile
import zipfile

from . import music21_rs as _wheel

__all__ = [
    "ConverterException",
    "ConverterFileException",
    "parse",
    "parseData",
    "parseFile",
    "parseURL",
    "toData",
]


# The keywords every reader takes, for the cache the wheel does not keep.
_CACHE_KEYWORDS = ("forceSource", "storePickle")


class _Format:
    """One format the wheel reads: music21's names for it, the extensions
    of its files, whether its files are read as bytes, the keywords taken
    at the value the crate's reader always reads with, and the reader."""

    def __init__(self, formats, extensions, read, *, binary=False, defaults=None):
        self.formats = formats
        self.extensions = extensions
        self.read = read
        self.binary = binary
        self.defaults = defaults or {}

    @property
    def name(self):
        return self.formats[0]


def _text(data):
    """A document's text, from text or from the bytes of one."""
    if not isinstance(data, bytes):
        return data
    if data.startswith((b"\xff\xfe", b"\xfe\xff")):
        return data.decode("utf-16")
    try:
        return data.decode("utf-8-sig")
    except UnicodeDecodeError:
        return data.decode("latin-1")


def _read_abc(data, number):
    if number is None:
        return _wheel.from_abc(_text(data))
    return _wheel.from_abc_number(_text(data), number)


def _read_midi(data, number):
    if isinstance(data, str):
        data = data.encode("latin-1")
    return _wheel.from_midi(data)


def _read_mei(data, number):
    text = _text(data)
    if text[:4].lower() == "mei:":
        text = text[4:]
    return _wheel.from_mei(text)


def _read_musedata(data, number):
    if isinstance(data, (list, tuple)):
        return _wheel.from_musedata([_text(each) for each in data])
    return _wheel.from_musedata(_text(data))


def _read_mscx(data, number):
    if isinstance(data, bytes) and data.startswith(b"PK"):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            data = archive.read(_mscx_member(archive.namelist()))
    return _wheel.from_mscx(_text(data))


def _read_musicxml(data, number):
    if isinstance(data, bytes) and data.startswith(b"PK"):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            data = _musicxml_of_zip(archive)
    return _wheel.from_musicxml(_text(data))


def _reader(function):
    return lambda data, number: function(_text(data))


# In the order music21 asks its subconverters, which is by class name, so a
# header such as `xml:` finds the format music21 finds.
_FORMATS = (
    _Format(("abc",), ("abc",), _read_abc),
    _Format(("capella",), ("capx",), lambda data, number: _wheel.from_capella(data),
            binary=True),
    _Format(("cttxt", "har", "clercqtemperley"), ("cttxt", "har", "clercqtemperley"),
            _reader(_wheel.from_clercq_temperley)),
    _Format(("humdrum",), ("krn",), _reader(_wheel.from_humdrum)),
    _Format(("mei",), ("mei",), _read_mei),
    _Format(("midi",), ("mid", "midi"), _read_midi, binary=True,
            defaults={"quantizePost": True, "encoding": ""}),
    _Format(("musedata",), ("md", "musedata", "zip"), _read_musedata),
    _Format(("musescore",), ("mscx", "mscz"), _read_mscx, binary=True),
    _Format(("musicxml", "xml"), ("xml", "mxl", "musicxml"), _read_musicxml,
            binary=True),
    _Format(("noteworthytext",), ("nwctxt",), _reader(_wheel.from_noteworthy)),
    _Format(("noteworthy",), ("nwc",), lambda data, number: _wheel.from_nwc(data),
            binary=True),
    _Format(("romantext", "rntext"), ("rntxt", "rntext", "romantext", "rtxt"),
            _reader(_wheel.from_roman_text)),
    _Format(("tinynotation",), ("tntxt", "tinynotation"), _reader(_wheel.from_tiny_notation),
            defaults={"makeNotation": True}),
    _Format(("volpiano",), ("volpiano", "vp"), _reader(_wheel.from_volpiano)),
)

# The formats music21 has no reader for, which the crate reads into
# music21's streams even once installed.
_WHEEL_ONLY = ("musescore",)


def _format_named(name):
    """The format music21 would choose for `name`, a format or an
    extension, as its `Converter.regularizeFormat` chooses one; or None."""
    name = name.lower().strip().lstrip(".").split(".")[0]
    for each in _FORMATS:
        if name in each.formats:
            return each
    for each in _FORMATS:
        if name in each.extensions:
            return each
    return None


def _format_given(name):
    found = _format_named(name)
    if found is None:
        raise _exception("ConverterException")(f"no converter available for format: {name}")
    return found


def _format_from_header(data):
    """The format a header such as `tinyNotation:` names, and the data
    after it, as music21's `Converter.formatFromHeader` finds them."""
    if isinstance(data, bytes):
        start = data[:20].decode("utf-8", "ignore").lower()
    elif isinstance(data, str):
        start = data[:20].lower()
    else:
        raise ValueError(f"Cannot parse a format from {type(data)}.")
    for each in _FORMATS:
        for name in each.formats:
            if start.startswith(name + ":"):
                return each, data[len(name) + 1:].lstrip()
    return None, data


def _format_from_content(data):
    """The format of a document by its first characters, as music21's
    `Converter.parseData` tells one; with a zip's and NoteWorthy's, which
    music21 does not tell."""
    if isinstance(data, bytes) and data.startswith(b"PK"):
        return _format_of_zip(data)
    if isinstance(data, bytes) and data.startswith(b"[NWZ]"):
        return _format_named("noteworthy")
    text = data.decode("utf-8", "ignore") if isinstance(data, bytes) else data
    lowered = text.lower()
    if text.startswith("<?xml") or text.startswith("<museScore"):
        if "<museScore" in text[:1000]:
            return _format_named("musescore")
        if "<mei" in text:
            return _format_named("mei")
        return _format_named("musicxml")
    if lowered.startswith("mei:"):
        return _format_named("mei")
    if lowered.startswith("musicxml:"):
        return _format_named("musicxml")
    if text.startswith("MThd") or lowered.startswith("midi:"):
        return _format_named("midi")
    if text.startswith(("!!!", "**")) or lowered.startswith("humdrum:"):
        return _format_named("humdrum")
    if lowered.startswith("tinynotation:"):
        return _format_named("tinynotation")
    if text.startswith("!NoteWorthyComposer"):
        return _format_named("noteworthytext")
    # MuseData has to state a meter and a key.
    if "WK#:" in text and "measure" in text:
        return _format_named("musedata")
    if "M:" in text and "K:" in text:
        return _format_named("abc")
    if "Time Signature:" in text and "m1" in text:
        return _format_named("romantext")
    raise _exception("ConverterException")(
        f"File not found or no such format found for: {text}"
    )


def _format_of_zip(data):
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        names = archive.namelist()
    if "score.xml" in names:
        return _format_named("capella")
    if any(name.endswith(".mscx") for name in names):
        return _format_named("musescore")
    if any(name.startswith("META-INF") or name.endswith((".xml", ".musicxml")) for name in names):
        return _format_named("musicxml")
    return _format_named("musedata")


def _mscx_member(names):
    return next(name for name in names if name.endswith(".mscx"))


def _musicxml_of_zip(archive):
    """The document a `.mxl` holds, as music21's `ArchiveManager` finds it:
    the first `.xml` or `.musicxml` outside `META-INF`."""
    for name in archive.namelist():
        if "META-INF" in name:
            continue
        if pathlib.PurePosixPath(name).suffix not in (".musicxml", ".xml", ".mxl") and name != ".xml":
            continue
        data = archive.read(name)
        found = re.match(rb"encoding=['\"](\S*?)['\"]", data[:1000])
        encoding = found.group(1).decode("ascii") if found else "utf-8"
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            return data.decode("utf-16-le")
    raise _exception("ConverterFileException")("no MusicXML document in the archive")


def _base_name(name):
    """The last part of a path in a directory or a zip, whichever its
    separator."""
    return name.replace("\\", "/").rstrip("/").rsplit("/", 1)[-1]


def _is_musedata_name(name):
    """Whether a file in a directory or a zip is one of a MuseData work's
    parts, as music21's `MuseDataDirectory` tells one."""
    stem = _base_name(name)
    if not name.endswith(".md"):
        if stem.startswith("mchan") or name.endswith(("/", ".py", ".svn-base")):
            return False
        if stem.startswith("."):
            return False
    return any(character.isdigit() for character in stem)


def _musedata_parts(names):
    """The parts of a MuseData work among `names`, without the score file
    where there are parts, sorted, as music21's `MuseDataDirectory` finds
    them."""
    parts = [name for name in names if _is_musedata_name(name)]
    if len(parts) > 1:
        parts = [
            name for name in parts
            if not "".join(c for c in _base_name(name) if not c.isdigit()).startswith("s")
        ]
    return sorted(parts)


def _file_data(path, found):
    """What the reader of `found` reads out of the file at `path`."""
    if found.name == "musedata":
        return _musedata_data(path)
    data = path.read_bytes()
    if found.binary:
        return data
    return _text(data)


def _musedata_data(path):
    if path.is_dir():
        names = [str(path / name) for name in sorted(os.listdir(path))]
        return [_text(pathlib.Path(name).read_bytes()) for name in _musedata_parts(names)]
    if path.suffix == ".zip" or (path.suffix == ".md" and zipfile.is_zipfile(path)):
        with zipfile.ZipFile(path) as archive:
            return [_text(archive.read(name)) for name in _musedata_parts(archive.namelist())]
    return [_text(path.read_bytes())]


def _checked(found, keywords):
    """Refuses a keyword the crate's reader of `found` does not honour."""
    refused = sorted(
        name for name, value in keywords.items()
        if name not in _CACHE_KEYWORDS and found.defaults.get(name, _MISSING) != value
    )
    if refused:
        raise _exception("ConverterException")(
            f"the wheel's {found.name} reader takes no keyword {', '.join(refused)} "
            "at that value; install the wheel into music21 for music21's reader"
        )


_MISSING = object()


def _installed():
    """Whether the wheel has been installed into music21: music21's `Note`
    is then the wheel's."""
    note = sys.modules.get("music21.note")
    return note is not None and isinstance(getattr(note, "Note", None), type) and issubclass(
        note.Note, _wheel.Note
    )


def _music21_converter():
    from music21 import converter

    return converter


def _found_or_none(name):
    return _format_named(name) if name is not None else None


def parseFile(fp, number=None, format=None, forceSource=False, **keywords):
    """Reads the file at `fp` into a stream, in the format its extension
    names unless `format` names one, as music21's `converter.parseFile`
    does. A directory is read as the parts of a MuseData work."""
    path = pathlib.Path(os.path.expanduser(str(fp)))
    if _installed() and not _wheel_only(_found_or_none(format) or _found_or_none(path.suffix)):
        return _music21_converter().parseFile(
            fp, number=number, format=format, forceSource=forceSource, **keywords
        )
    if not path.exists():
        raise _exception("ConverterFileException")(f"no such file exists: {path}")
    if format is not None:
        found = _format_given(format)
    elif path.is_dir():
        found = _format_named("musedata")
    else:
        found = _format_named(path.suffix) if path.suffix else None
        if found is None:
            raise _exception("ConverterFileException")(
                f"cannot find format from file extensions for: {path}"
            )
    _checked(found, keywords)
    return found.read(_file_data(path, found), number)


def parseData(dataStr, number=None, format=None, **keywords):
    """Reads a document given as text or bytes into a stream, in the format
    `format` names, else the one a header such as `tinyNotation:` names or
    the document's first characters tell, as music21's
    `converter.parseData` does."""
    if _installed() and not _wheel_only(_data_format(dataStr, format, quiet=True)):
        return _music21_converter().parseData(dataStr, number=number, format=format, **keywords)
    if format is None:
        dataStr = dataStr.lstrip()
        found, dataStr = _format_from_header(dataStr)
        if found is None:
            found = _format_from_content(dataStr)
    else:
        found = _format_given(format)
    _checked(found, keywords)
    return found.read(dataStr, number)


def _data_format(data, format, quiet=False):
    try:
        if format is not None:
            return _format_given(format)
        stripped = data.lstrip()
        found, stripped = _format_from_header(stripped)
        return found or _format_from_content(stripped)
    except Exception:
        if quiet:
            return None
        raise


def _wheel_only(found):
    return found is not None and found.name in _WHEEL_ONLY


def parseURL(url, *, format=None, number=None, forceSource=False, **keywords):
    """Downloads the document at `url` and reads it, in the format its
    extension names, else as `parseData` tells one, as music21's
    `converter.parseURL` does -- without the `autoDownload` setting music21
    asks first, which the wheel does not keep."""
    if _installed():
        return _music21_converter().parseURL(
            url, format=format, number=number, forceSource=forceSource, **keywords
        )
    import urllib.parse
    import urllib.request

    with urllib.request.urlopen(url) as response:
        data = response.read()
    if format is None:
        found = _format_named(pathlib.PurePosixPath(urllib.parse.urlparse(url).path).suffix)
        if found is not None:
            format = found.name
    if format == "musicxml" and data.startswith(b"PK"):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            data = _musicxml_of_zip(archive)
    return parseData(data, number=number, format=format, **keywords)


def parse(value, *, forceSource=False, number=None, format=None, **keywords):
    r"""Reads a file path, the text or bytes of a document, a `(path,
    number)` pair or a URL into a stream, as music21's `converter.parse`
    does.

    A string is a file where one exists by that name, else a URL where it
    starts `http://` or `https://`, else a document. A document starting
    with a header such as `tinyNotation:` is a document whatever else it
    holds.

    >>> from music21_rs import converter
    >>> part = converter.parse("tinyNotation: 3/4 E4 r f# g")
    >>> part.getElementsByClass('Measure').first().timeSignature
    <music21.meter.TimeSignature 3/4>
    """
    if _installed() and not isinstance(value, (str, bytes, pathlib.PurePath, list, tuple)):
        return _music21_converter().parse(
            value, forceSource=forceSource, number=number, format=format, **keywords
        )
    if isinstance(value, bytes):
        text = value.decode("utf-8", "ignore")
    elif isinstance(value, (str, pathlib.PurePath)):
        text = str(value)
    else:
        text = ""

    if isinstance(value, (list, tuple)) and len(value) == 2 and _can_load(str(value[0])):
        return _parse_pair(value, format, keywords)
    if not isinstance(value, bytes) and text.startswith("MThd"):
        return parseData(value, number=number, format=format, **keywords)
    if isinstance(value, str) and _format_from_header(value.lstrip())[0] is not None:
        return parseData(value, number=number, format=format, **keywords)
    if not isinstance(value, bytes) and _can_load(text):
        return parseFile(text, number=number, format=format, forceSource=forceSource, **keywords)
    if not isinstance(value, bytes) and _can_load(os.path.expanduser(text)):
        return parseFile(os.path.expanduser(text), number=number, format=format,
                         forceSource=forceSource, **keywords)
    if not isinstance(value, bytes) and text.startswith(("http://", "https://")):
        return parseURL(value, number=number, format=format, forceSource=forceSource, **keywords)
    if isinstance(value, pathlib.PurePath):
        raise FileNotFoundError(f"Cannot find file in {text}")
    if isinstance(value, str) and _format_named(pathlib.PurePath(value).suffix or "-") is not None:
        # A file path mistyped, rather than a document.
        raise FileNotFoundError(f"Cannot find file in {text}")
    return parseData(value, number=number, format=format, **keywords)


def _parse_pair(value, format, keywords):
    """A `(path, number)` pair, as `corpus.search` hands them out: the
    path read, and the score numbered `number` where it holds several."""
    path, number = value
    if number is None:
        return parseFile(path, format=format, **keywords)
    if not isinstance(path, str):
        raise _exception("ConverterException")(
            f"If using a two-element list, the first value must be a string, not {path!r}"
        )
    if not isinstance(number, int):
        raise _exception("ConverterException")(
            "If using a two-element list, the second value must be an integer number, "
            f"not {number!r}"
        )
    read = parseFile(path, format=format, **keywords)
    if type(read).__name__ == "Opus":
        return read.getScoreByNumber(number)
    return read


def _can_load(path):
    try:
        return os.path.exists(path)
    except ValueError:
        return False


# What a stream can be written as, by the name and the extensions music21
# gives each format, and whether what is written is bytes.
_WRITTEN = {
    "musicxml": False, "xml": False, "midi": True, "mid": True,
    "text": False, "txt": False, "t": False, "textline": False,
    "lilypond": False, "lily": False, "braille": False,
    "romantext": False, "rntxt": False, "volpiano": False, "abc": False,
}


def toData(obj, fmt, **keywords):
    """`obj` written in `fmt`, handed back as the text or bytes of the
    document, as music21's `converter.toData` hands it back.

    The formats are those `Stream.write` writes -- MusicXML, MIDI, text,
    LilyPond, braille, RomanText and Volpiano -- and ABC, which music21
    does not write.
    """
    name = fmt.lower().lstrip(".")
    if name == "abc":
        return _wheel.to_abc(obj)
    if _installed():
        return _music21_converter().toData(obj, fmt, **keywords)
    if name.split(".")[0] not in _WRITTEN:
        raise _exception("ConverterException")(f"cannot support output in this format yet: {fmt}")
    binary = _WRITTEN[name.split(".")[0]]
    handle, path = tempfile.mkstemp()
    os.close(handle)
    try:
        written = pathlib.Path(str(obj.write(fmt, fp=path, **keywords)))
        return written.read_bytes() if binary else written.read_text(encoding="utf-8")
    finally:
        pathlib.Path(path).unlink(missing_ok=True)


# The exceptions, made on first asking: music21's own where it can be
# imported, so that code catching music21's catches these, and plain
# exceptions under music21's names where it cannot.
_EXCEPTIONS = ("ConverterException", "ConverterFileException")
_made = {}


def _exception(name):
    if name not in _made:
        try:
            from music21 import converter

            _made[name] = getattr(converter, name)
        except Exception:
            _made[name] = type(name, (Exception,), {"__module__": "music21.converter"})
    return _made[name]


def __getattr__(name):
    if name in _EXCEPTIONS:
        return _exception(name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
