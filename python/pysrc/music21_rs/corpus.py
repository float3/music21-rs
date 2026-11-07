"""music21's `corpus`, found on disk and read with the wheel's `converter`.

The scores are music21's own, in the layout its `corpus` directory has
(`bach/bwv66.6.mxl`, `essenFolksong/han1.abc`, ...); the wheel ships none.
They are looked for in

1. the directory the `MUSIC21_RS_CORPUS` environment variable names, where
   it is set;
2. the `corpus` directory of an installed music21, found without importing
   it;

and where neither is there, asking for a work raises `CorpusException`.

`getWork` finds a work by part of its path, as music21's does, and `parse`
reads it. There is no cache of parsed scores and no metadata bundle to
search: every parse reads the file.

Once `install_into_music21` has run, these are music21's own `getWork` and
`parse`, with music21's caches and its local corpora.
"""

import importlib.util
import os
import pathlib

from . import converter as _converter

__all__ = [
    "CorpusException",
    "getCorePaths",
    "getCorpusFilePath",
    "getPaths",
    "getWork",
    "parse",
]

# The variable naming a directory laid out as music21's corpus.
CORPUS_VARIABLE = "MUSIC21_RS_CORPUS"

# The extensions of the formats music21's corpus holds, in its order.
_EXTENSIONS = (
    ".abc", ".capx", ".mid", ".midi", ".xml", ".mxl", ".musicxml", ".md",
    ".musedata", ".zip", ".krn", ".rntxt", ".rntext", ".romantext", ".rtxt",
    ".nwctxt", ".nwc",
)

# The files under each corpus directory, by directory and extensions.
_paths = {}


def getCorpusFilePath():
    """The directory the corpus is read from: the one `MUSIC21_RS_CORPUS`
    names, else an installed music21's `corpus`. Raises `CorpusException`
    where there is neither."""
    named = os.environ.get(CORPUS_VARIABLE)
    if named:
        path = pathlib.Path(named).expanduser()
        if not path.is_dir():
            raise _exception()(f"{CORPUS_VARIABLE} names no directory: {path}")
        return path
    found = _music21_corpus()
    if found is None:
        raise _exception()(
            "no corpus to read: install music21, whose corpus this reads, or set "
            f"{CORPUS_VARIABLE} to a directory laid out as music21's corpus"
        )
    return found


def _music21_corpus():
    """music21's `corpus` directory, found by where the package is rather
    than by importing it."""
    try:
        spec = importlib.util.find_spec("music21")
    except (ImportError, ValueError):
        return None
    if spec is None:
        return None
    for location in spec.submodule_search_locations or ():
        corpus = pathlib.Path(location) / "corpus"
        if corpus.is_dir():
            return corpus
    return None


def _extensions(fileExtensions):
    """Every extension of the formats `fileExtensions` name, as music21's
    `Corpus.translateExtensions` widens them; all of them for none."""
    if not fileExtensions:
        return _EXTENSIONS
    if isinstance(fileExtensions, str):
        fileExtensions = [fileExtensions]
    widened = []
    for extension in fileExtensions:
        found = _converter._format_named(extension)
        if found is not None:
            widened += ["." + each for each in found.extensions]
    return tuple(widened)


def getPaths(fileExtensions=()):
    """The paths of every work in the corpus, or of those in the formats
    `fileExtensions` name, sorted, as music21's `corpus.getPaths` lists
    them."""
    root = getCorpusFilePath()
    extensions = _extensions(fileExtensions)
    key = (str(root), extensions)
    if key not in _paths:
        _paths[key] = [
            path for path in sorted(root.rglob("*"))
            if not path.name.startswith(("__", "."))
            and any(path.suffix.endswith(extension) for extension in extensions)
        ]
    return _paths[key]


getCorePaths = getPaths


def _work_list(workName, movementNumber, fileExtensions):
    """The paths matching `workName`, as music21's `Corpus.getWorkList`
    matches them: a path holding the name, those with a `/` before it
    where there are any, then those of the movement asked for."""
    if str(workName).startswith("schumann/"):
        workName = str(workName).replace("schumann/", "schumann_robert/")
    wanted = pathlib.PurePath(workName).as_posix().lower()
    results = [path for path in getPaths(fileExtensions) if wanted in path.as_posix().lower()]
    stricter = [path for path in results if "/" + wanted in path.as_posix().lower()]
    if stricter:
        results = stricter
    if movementNumber is None or not results:
        return sorted(set(results))
    return sorted(set(_movements(results, movementNumber)))


def _movement_names(movementNumber):
    if isinstance(movementNumber, (list, tuple)):
        numbers = [str(number) for number in movementNumber]
        return [
            "".join(numbers),
            "-".join(numbers),
            "movement" + "-".join(numbers),
            "movement" + "-0".join(numbers),
        ]
    return [f"0{movementNumber}", str(movementNumber), f"movement{movementNumber}"]


def _movements(results, movementNumber):
    names = _movement_names(movementNumber)
    found = []
    for path in sorted(results):
        partial = True
        if path.suffix:
            for name in names:
                if path.stem.lower() == name.lower():
                    found.append(path)
                    partial = False
        if found:
            continue
        if partial:
            found += [path for name in names if path.name.startswith(name.lower())]
    return found


def getWork(workName, movementNumber=None, *, fileExtensions=()):
    """The path of the work whose path holds `workName`, or a list of the
    paths where several do, as music21's `corpus.getWork` finds it. A name
    ending `.xml` finds the `.mxl` of that name too."""
    if _installed():
        from music21 import corpus

        return corpus.getWork(workName, movementNumber, fileExtensions=fileExtensions)
    if not workName:
        raise _exception()("a work name must be provided as an argument")
    name = str(workName)
    found = _work_list(workName, movementNumber, fileExtensions)
    compressed = name.endswith((".xml", ".musicxml"))
    if not found and compressed:
        found = _work_list(os.path.splitext(name)[0] + ".mxl", movementNumber, fileExtensions)
    if not found:
        kind = "n xml, musicxml, or mxl" if compressed else ""
        raise _exception()(
            f"Could not find a{kind} work that met this criterion: {workName}; "
            'if you are searching for a file on disk, use "converter" instead of "corpus".'
        )
    if len(found) == 1:
        return found[0]
    return found


def parse(workName, movementNumber=None, *, number=None, fileExtensions=(),
          forceSource=False, format=None):
    """The work `getWork` finds, read by the wheel's `converter.parse`, its
    metadata's `corpusFilePath` saying where in the corpus it is, as
    music21's `corpus.parse` reads it.

    Where `getWork` finds several, the first is read.
    """
    if _installed():
        from music21 import corpus

        return corpus.parse(workName, movementNumber, number=number,
                            fileExtensions=fileExtensions, forceSource=forceSource,
                            format=format)
    found = getWork(workName, movementNumber, fileExtensions=fileExtensions)
    path = found[0] if isinstance(found, list) else found
    read = _converter.parse(path, number=number, format=format)
    _say_where(read, path)
    return read


def _say_where(read, path):
    if read.metadata is None:
        from . import Metadata

        read.metadata = Metadata()
    try:
        read.metadata.corpusFilePath = path.relative_to(getCorpusFilePath()).as_posix()
    except ValueError:
        read.metadata.corpusFilePath = str(path)


def _installed():
    return _converter._installed()


_made = []


def _exception():
    """`CorpusException`: music21's own where it can be imported, a plain
    exception under its name where it cannot."""
    if not _made:
        _made.append(_converter._music21_exception("CorpusException", "music21.exceptions21"))
    return _made[0]


def __getattr__(name):
    if name == "CorpusException":
        return _exception()
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
