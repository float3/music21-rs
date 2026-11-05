"""harte-rs against harte-library.

`harte/data/harte_expectations.toml` records what harte-library, on music21,
makes of every label in its own coverage set; each is checked here. The other
cases are harte-library's own tests.
"""

import copy
import pathlib
import pickle

import pytest

from harte.harte import Harte
from harte.interval import HarteInterval
from harte.mappings import DEGREE_SHORTHAND_MAP, SHORTHAND_DEGREES
from harte.utils import convert_interval, degree_to_sort_key, unwrap_shorthand
from music21_rs import Chord, ChordException, Interval, IntervalException

EXPECTATIONS = pathlib.Path(__file__).resolve().parents[2] / "data" / "harte_expectations.toml"


def _cases():
    """The fixture's labels, where it can be read: it needs tomllib, Python
    3.11+, and the repository, which the sdist does not carry."""
    try:
        import tomllib
    except ImportError:
        return []
    if not EXPECTATIONS.is_file():
        return []
    return tomllib.loads(EXPECTATIONS.read_text(encoding="utf-8"))["chord"]


CASES = _cases()


def _sort_parenthesised(label):
    """`C:maj7(#11,9)` with the degrees in its parentheses sorted, as the
    fixture writes them."""
    head, _, rest = label.partition("(")
    inside, closed, tail = rest.partition(")")
    if not closed:
        return label
    return f"{head}({','.join(sorted(inside.split(',')))}){tail}"


def _in_degree_order(degrees, pitches):
    """Degrees and their pitches in harte-library's degree order, its ties
    broken by spelling: the library breaks them by set order, which is not
    fixed from one run to the next."""
    return sorted(zip(degrees, pitches), key=lambda pair: (degree_to_sort_key(pair[0]), pair[0]))


@pytest.mark.skipif(not CASES, reason="the fixture needs tomllib and the repository")
def test_the_fixture_covers_harte_librarys_coverage_set():
    assert len(CASES) == 8064


@pytest.mark.parametrize("case", CASES, ids=[case["label"] for case in CASES])
def test_every_label_builds_the_chord_harte_library_builds(case):
    if "error" in case:
        with pytest.raises(ChordException):
            Harte(case["label"])
        return
    chord = Harte(case["label"])
    pitches = [pitch.nameWithOctave for pitch in chord.pitches]
    sounding = chord._label.sounding_degrees
    assert _in_degree_order(sounding, pitches) == _in_degree_order(
        case["degrees"], case["pitches"]
    )
    if case.get("root") is None:
        assert chord.get_root() is None
        return
    assert chord.root().nameWithOctave == case["root"]
    assert chord.bass().nameWithOctave == case["bass"]
    assert _sort_parenthesised(chord.prettify()) == case["pretty"]


@pytest.mark.parametrize(
    "label,intervals",
    [
        ("C", ["P5", "M3"]),
        ("A", ["P5", "M3"]),
        ("C:maj", ["P5", "M3"]),
        ("C:min", ["P5", "m3"]),
        ("C:dim", ["d5", "m3"]),
        ("C:aug", ["A5", "M3"]),
        ("N", []),
    ],
)
def test_interval_extraction(label, intervals):
    annotated = Harte(label).annotateIntervals(
        inPlace=False, returnList=True, stripSpecifiers=False
    )
    assert set(intervals) == set(annotated)


@pytest.mark.parametrize(
    "label,pitches",
    [
        ("F:(b3, 5, b7, 11)", ["F", "A-", "C", "E-", "B-"]),
        ("F:(b3, 11, b7, 5)", ["F", "A-", "C", "E-", "B-"]),
        ("F:maj7(#11)", ["F", "A", "C", "E", "B"]),
    ],
)
def test_ordering_of_degrees(label, pitches):
    assert [pitch.name for pitch in Harte(label).pitches] == pitches


@pytest.mark.parametrize(
    "harte_interval,music21_interval",
    [
        (f"{accidental}{number}", f"{quality}{(number - 1) % 7 + 1}")
        for number in range(1, 15)
        for accidental, quality in zip(
            ("b", "", "#"),
            ("dPA" if (number - 1) % 7 + 1 in (1, 4, 5) else "mMA"),
        )
    ],
)
def test_harte_interval(harte_interval, music21_interval):
    assert convert_interval(harte_interval) == music21_interval


def test_the_parts_of_a_label():
    chord = Harte("Bb:min7/b3")
    assert isinstance(chord, Chord)
    assert chord.chord == "Bb:min7/b3"
    assert chord.get_root() == "Bb"
    assert chord.get_shorthand() == "min7"
    assert chord.contains_shorthand()
    assert chord.get_bass() == "b3"
    assert not chord.bass_is_root()
    assert chord.get_degrees() is None
    assert chord.unwrap_shorthand() == ["1", "b3", "5", "b7"]
    assert chord.root().nameWithOctave == "B-4"
    assert chord.bass().nameWithOctave == "D-3"
    assert chord.commonName == "minor seventh chord"
    assert str(chord) == "Bb:min7/b3"
    assert repr(chord) == "Harte(Bb:min7/b3)"


def test_midi_pitches_and_multi_hot_encoding():
    chord = Harte("G:7")
    assert chord.get_midi_pitches() == [67, 71, 74, 77]
    assert chord.multi_hot_encoding() == [0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1]
    assert chord.multi_hot_encoding(True) == [1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0]


def test_degrees_and_unwrapping():
    chord = Harte("F:sus4(*5,9)")
    assert chord.get_degrees() == ["*5", "9"]
    assert chord.unwrap_shorthand() == ["1", "4", "9"]
    assert Harte("C:(b3,5)").unwrap_shorthand() == ["b3", "5"]
    assert Harte("C").unwrap_shorthand() is None
    assert Harte("C").bass_is_root()


def test_prettify():
    assert Harte("C:(b3,5)").prettify() == "C:min"
    assert Harte("D:(b3,5,7,9)/b3").prettify() == "D:minmaj7(9)/b3"


def test_no_chord():
    for label in ("N", "X"):
        chord = Harte(label)
        assert chord.pitches == ()
        assert chord.get_root() is None
        assert chord.get_shorthand() is None
        assert not chord.contains_shorthand()
        assert str(chord) == "N"


def test_equality_is_the_chord_sounded():
    assert Harte("C:maj") == Harte("C:(3,5)")
    assert Harte("C:maj") != Harte("C:min")
    assert Harte("C:maj") != Chord("C4 E4 G4")


def test_a_label_that_is_not_harte_notation():
    with pytest.raises(ChordException):
        Harte("C:foo")


def test_keywords_reach_the_chord():
    assert Harte("C", quarterLength=2).quarterLength == 2.0


def test_copies_keep_the_label():
    chord = Harte("Bb:min7/b3")
    for copied in (copy.deepcopy(chord), pickle.loads(pickle.dumps(chord))):
        assert type(copied) is Harte
        assert str(copied) == "Bb:min7/b3"
        assert copied.pitches == chord.pitches
    transposed = chord.transpose("P5")
    assert type(transposed) is Harte
    assert transposed.get_root() == "Bb"
    assert [pitch.nameWithOctave for pitch in transposed.pitches] == ["F5", "A-3", "C6", "E-6"]


def test_harte_interval_class():
    interval = HarteInterval("b6")
    assert isinstance(interval, Interval)
    assert interval.name == "m6"
    assert interval.isConsonant()
    assert str(interval) == "b6"
    assert repr(interval) == "HarteInterval(b6)"
    assert interval == "b6"
    assert interval == HarteInterval("b6")
    with pytest.raises(IntervalException):
        HarteInterval("x")
    with pytest.raises(ValueError):
        convert_interval("x")


def test_mappings():
    assert len(SHORTHAND_DEGREES) == 48
    assert SHORTHAND_DEGREES["hdim7"] == ["1", "b3", "b5", "b7"]
    assert next(iter(DEGREE_SHORTHAND_MAP.items())) == (("3", "5", "b7", "9"), "9")
    assert unwrap_shorthand("maj", ["9"]) == ["1", "3", "5", "9"]


def test_a_harte_goes_in_music21s_streams_once_installed():
    """With `install_into_music21()` called before `harte` is imported, a
    `Harte` is music21's `Chord`, as harte-library's is."""
    pytest.importorskip("music21.stream")
    import subprocess
    import sys

    script = """
import music21_rs
music21_rs.install_into_music21()
from music21 import base, chord, stream
from harte.harte import Harte
h = Harte('C:maj7')
assert isinstance(h, chord.Chord) and isinstance(h, base.Music21Object)
s = stream.Stream()
s.append(h)
s.append(Harte('G:7/3'))
assert [p.nameWithOctave for p in s.pitches][-1] == 'F5'
copied = h.transpose(2)
assert copied.get_root() == 'C' and copied.pitches[0].nameWithOctave == 'D4'
"""
    subprocess.run([sys.executable, "-c", script], check=True)
