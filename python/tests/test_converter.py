"""`music21_rs.converter` and `music21_rs.corpus`: music21's `converter.parse`
and `corpus.parse` over the crate's readers.

The readers themselves are held to music21 elsewhere (`converter_parity.py`,
the parity suites); what is here is the choosing: that a file or a document
reaches the right reader by its extension or its first characters, as
music21's converter sends it, with no music21 to import, and that once the
wheel is installed into music21 the same calls are music21's own.
"""

import importlib.util
import os
import pathlib
import subprocess
import sys

import pytest

import music21_rs as m
from music21_rs import converter, corpus

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "music21" / "music21"
MSCX = ROOT / "python-parity" / "tests" / "mscx"

# Run first in a subprocess standing on its own: music21 is not to be found,
# however it is installed, as for a user who never installed it.
WITHOUT_MUSIC21 = """
import sys

class _NoMusic21:
    def find_spec(self, name, path=None, target=None):
        if name == 'music21' or name.startswith('music21.'):
            raise ModuleNotFoundError(name)
        return None

sys.meta_path.insert(0, _NoMusic21())
"""

BLITZKRIEG_BOP = """
% Blitzkrieg Bop

BP: I | IV V | %THIS IS A COMMENT
In: $BP*3 I IV | I | $BP*3 I IV | I | R |*4 I |*4
Vr: $BP*3 I IV | I |
Br: IV | | I | IV I | IV | | ii | IV V |
Co: R |*4 I |*4
S: [A] $In $Vr $Vr $Br $Vr $Vr $Br $Vr $Vr $Co
"""


def _run(code, **environment):
    env = {key: value for key, value in os.environ.items() if key != "PYTHONPATH"}
    env.update(environment)
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, env=env, cwd=ROOT / "python"
    )
    assert done.returncode == 0, done.stderr
    return done.stdout.splitlines()


@pytest.fixture
def fixtures():
    if not SOURCE.is_dir():
        pytest.skip("music21's submodule is not checked out")
    return SOURCE


def test_parse_reads_every_format_with_no_music21(fixtures, tmp_path):
    """Each format music21's converter reads, and MuseScore's, from a file
    of it, standing on its own: the wheel's streams, and no music21."""
    (tmp_path / "bop.cttxt").write_text(BLITZKRIEG_BOP)
    paths = [
        fixtures / "corpus" / "bach" / "bwv66.6.mxl",
        fixtures / "corpus" / "demos" / "two-parts.xml",
        fixtures / "corpus" / "essenFolksong" / "han1.abc",
        fixtures / "midi" / "testPrimitive" / "test03.mid",
        fixtures / "humdrum" / "Missa_Sine_nomine-Kyrie.krn",
        fixtures / "mei" / "test" / "notes_in_utf16.mei",
        fixtures / "corpus" / "monteverdi" / "madrigal.3.1.rntxt",
        tmp_path / "bop.cttxt",
        fixtures / "capella" / "Nu_rue_mit_sorgen.capx",
        fixtures / "noteworthy" / "cuthbert_test1.nwctxt",
        fixtures / "noteworthy" / "cuthbert_test1.nwc",
        fixtures / "musedata" / "testPrimitive" / "test01",
        fixtures / "musedata" / "testPrimitive" / "test01" / "01.md",
        MSCX / "two-staves.mscx",
    ]
    code = WITHOUT_MUSIC21 + f"""
import music21_rs as m
from music21_rs import converter
for path in {[str(path) for path in paths]!r} + [
    'tinyNotation: 3/4 c4 d e f g a',
    'volpiano: 1---c--d---f--d---ed--c--d---f---g--h--j---hgf--g--h---4',
]:
    read = converter.parse(path)
    print(type(read).__name__, isinstance(read, m.Stream), len(read.recurse().notesAndRests) > 0)
print('music21' in sys.modules)
"""
    assert _run(code) == [
        "Score True True",  # mxl
        "Score True True",  # xml
        "Opus True True",  # abc
        "Score True True",  # mid
        "Score True True",  # krn
        "Score True True",  # mei
        "Score True True",  # rntxt
        "Part True True",  # cttxt, as music21's `CTSong.toPart`
        "Score True True",  # capx
        "Score True True",  # nwctxt
        "Score True True",  # nwc
        "Score True True",  # a MuseData directory
        "Score True True",  # one MuseData file
        "Score True True",  # mscx
        "Part True True",  # tinyNotation
        "Part True True",  # volpiano
        "False",
    ]


def test_a_document_is_read_by_its_first_characters(fixtures):
    """With no file name to go by, a document is told by its header or its
    first characters, as music21's `parseData` tells it; a zip by what it
    holds."""
    read = {
        "midi": (fixtures / "midi" / "testPrimitive" / "test03.mid").read_bytes(),
        "mxl": (fixtures / "corpus" / "bach" / "bwv66.6.mxl").read_bytes(),
        "musicxml": (fixtures / "corpus" / "demos" / "two-parts.xml").read_text(),
        "mei": (fixtures / "mei" / "test" / "notes_in_utf8.mei").read_text(),
        "humdrum": (fixtures / "humdrum" / "Missa_Sine_nomine-Kyrie.krn").read_text(),
        "abc": (fixtures / "corpus" / "essenFolksong" / "han1.abc").read_text(),
        "romantext": (fixtures / "corpus" / "monteverdi" / "madrigal.3.1.rntxt").read_text(),
        "nwc": (fixtures / "noteworthy" / "cuthbert_test1.nwc").read_bytes(),
        "mscx": (MSCX / "two-staves.mscx").read_text(),
    }
    for name, data in read.items():
        assert converter._data_format(data, None).name == {
            "mxl": "musicxml", "nwc": "noteworthy", "mscx": "musescore",
        }.get(name, name), name
        assert type(converter.parse(data)).__name__ in ("Score", "Opus"), name
    # A header says the format, whatever follows, and is taken off.
    part = converter.parse("abc: X:1\nM:3/4\nL:1/4\nK:G\nGAB|c3|]")
    assert [str(p) for p in part.pitches] == ["G4", "A4", "B4", "C5"]
    with pytest.raises(converter.ConverterException, match="no such format"):
        converter.parse("nothing a reader reads")


def test_a_file_is_read_by_its_extension_unless_a_format_is_given(fixtures, tmp_path):
    """A file's extension names its format, as music21's
    `getFormatFromFileExtension` names it; `format=` names it instead; an
    extension nothing reads raises `ConverterFileException`."""
    unnamed = tmp_path / "chorale"
    unnamed.write_bytes((fixtures / "corpus" / "bach" / "bwv66.6.mxl").read_bytes())
    with pytest.raises(converter.ConverterFileException, match="cannot find format"):
        converter.parseFile(unnamed)
    assert len(converter.parse(unnamed, format="musicxml").parts) == 4
    assert len(converter.parse(str(unnamed), format="mxl").parts) == 4
    with pytest.raises(converter.ConverterException, match="no converter available"):
        converter.parse(unnamed, format="png")
    with pytest.raises(FileNotFoundError):
        converter.parse(tmp_path / "missing.xml")
    with pytest.raises(FileNotFoundError):
        converter.parse(str(tmp_path / "missing.krn"))


def test_a_number_picks_a_tune_and_a_keyword_the_reader_cannot_honour_raises(fixtures):
    path = fixtures / "corpus" / "essenFolksong" / "han1.abc"
    assert type(converter.parse(path, number=3)).__name__ == "Score"
    assert type(converter.parse((str(path), 3))).__name__ == "Score"
    # What the crate always reads with is taken; anything else is refused
    # rather than ignored.
    converter.parse("tinyNotation: c4 d", makeNotation=True, forceSource=True)
    with pytest.raises(converter.ConverterException, match="makeNotation"):
        converter.parse("tinyNotation: c4 d", makeNotation=False)
    with pytest.raises(converter.ConverterException, match="quarterLengthDivisors"):
        converter.parse(fixtures / "midi" / "testPrimitive" / "test03.mid",
                        quarterLengthDivisors=(4,))


def test_parse_url_reads_what_it_downloads(fixtures):
    url = (fixtures / "corpus" / "bach" / "bwv66.6.mxl").as_uri()
    assert len(converter.parseURL(url).parts) == 4


def test_to_data_writes_a_stream_in_a_format():
    part = converter.parse("tinyNotation: 4/4 c4 d e f")
    assert converter.toData(part, "musicxml").startswith("<?xml")
    assert converter.toData(part, "midi").startswith(b"MThd")
    assert "C D E F" in converter.toData(part, "abc")
    with pytest.raises(converter.ConverterException, match="cannot support output"):
        converter.toData(part, "png")


def test_the_package_hands_out_the_modules():
    assert m.converter is converter
    assert m.corpus is corpus


def test_corpus_reads_from_the_directory_the_variable_names(fixtures):
    """With no music21 to find, the corpus is the directory
    `MUSIC21_RS_CORPUS` names; with neither, asking raises."""
    code = WITHOUT_MUSIC21 + """
from music21_rs import corpus
try:
    corpus.getWork('bach/bwv66.6')
except corpus.CorpusException as error:
    print('no corpus' if 'MUSIC21_RS_CORPUS' in str(error) else error)
"""
    assert _run(code) == ["no corpus"]
    code = WITHOUT_MUSIC21 + """
from music21_rs import corpus
score = corpus.parse('bach/bwv66.6')
print(type(score).__name__, len(score.parts), score.metadata.corpusFilePath)
print(corpus.getWork('beethoven/opus59no1', 2).name)
print(len(corpus.getWork('trecento')) > 100)
print(corpus.getWork('bach/bwv66.6.xml').name)
try:
    corpus.parse('no/such/work')
except corpus.CorpusException as error:
    print(str(error).startswith('Could not find a work'))
print('music21' in sys.modules)
"""
    assert _run(code, MUSIC21_RS_CORPUS=str(fixtures / "corpus")) == [
        "Score 4 bach/bwv66.6.mxl",
        "movement2.mxl",
        "True",
        "bwv66.6.mxl",
        "True",
        "False",
    ]


def test_corpus_finds_an_installed_music21s_corpus_without_importing_it():
    spec = importlib.util.find_spec("music21")
    locations = spec.submodule_search_locations if spec else None
    if not any(pathlib.Path(each, "corpus").is_dir() for each in locations or ()):
        pytest.skip("music21 is not installed")
    code = """
import sys
from music21_rs import corpus
score = corpus.parse('bach/bwv66.6')
print(type(score).__name__, len(score.parts), score.metadata.corpusFilePath)
print('music21' in sys.modules)
"""
    assert _run(code, PYTHONPATH=os.environ.get("PYTHONPATH", "")) == [
        "Score 4 bach/bwv66.6.mxl",
        "False",
    ]


def test_installed_converter_and_corpus_are_music21s():
    """Installed, the calls are music21's own and hand back its streams;
    MuseScore's files, which music21 does not read, are read by the crate
    into music21's streams."""
    pytest.importorskip("music21.stream")
    code = f"""
import music21_rs as m
from music21_rs import converter, corpus
m.install_into_music21()
from music21 import converter as m21converter, stream
score = corpus.parse('bach/bwv66.6')
print(isinstance(score, stream.Score), score.metadata.corpusFilePath)
print(isinstance(converter.parse({str(MSCX / "two-staves.mscx")!r}), stream.Score))
print(isinstance(converter.parse('tinyNotation: c4 d', makeNotation=False), stream.Part))
print(converter.ConverterException is m21converter.ConverterException)
"""
    assert _run(code, PYTHONPATH=os.environ.get("PYTHONPATH", "")) == [
        "True bach/bwv66.6.mxl",
        "True",
        "True",
        "True",
    ]
