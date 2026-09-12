"""The `music21_rs` wheel, checked against music21's own answers.

These are not a second copy of the doctest suite — `python-parity` runs
music21's docstrings verbatim and is the real measure. What is here is the
shape of the package as an importable thing: that every name is exported,
that the classes carry music21's reprs and module strings, and that the
handful of behaviours a caller reaches for first are right.
"""

import gc
import math
import subprocess
import sys
import weakref

import pytest

import music21_rs as m


def test_every_public_name_is_exported():
    exported = set(m.__all__)
    assert "music21_rs" not in exported, "the extension submodule is not part of the API"
    for name in ("Pitch", "Interval", "Chord", "Note", "Key", "ToneRow", "Duration"):
        assert name in exported
    for name in exported:
        assert hasattr(m, name)


@pytest.mark.parametrize(
    ("cls", "module"),
    [
        (m.Pitch, "music21.pitch"),
        (m.Accidental, "music21.pitch"),
        (m.Interval, "music21.interval"),
        (m.Chord, "music21.chord"),
        (m.Note, "music21.note"),
        (m.Duration, "music21.duration"),
        (m.Key, "music21.key"),
        (m.Volume, "music21.volume"),
    ],
)
def test_classes_report_music21s_module(cls, module):
    # The module strings are what let python-parity swap these classes into
    # music21 and run its docstrings unchanged.
    assert cls.__module__ == module


def test_pitch_reads_a_name_and_transposes():
    assert repr(m.Pitch("C4")) == "<music21.pitch.Pitch C4>"
    assert m.Pitch("C4").transpose("M3").nameWithOctave == "E4"
    # An octave digit may sit anywhere after the name, as music21 reads it.
    assert m.Pitch("e4-").nameWithOctave == "E-4"
    assert m.Pitch(midi=61).nameWithOctave == "C#4"


def test_pitch_ordering_and_equality():
    assert m.Pitch("C4") == m.Pitch("C4")
    assert m.Pitch("D#4") != m.Pitch("E-4"), "enharmonics are not equal"
    assert m.Pitch("C4") < m.Pitch("D4")
    # music21's `__le__` is `<` or `==`, so two pitches of the same height
    # that are spelled differently are neither.
    assert not m.Pitch("E-4") <= m.Pitch("D#4")
    assert m.Pitch("D#4") <= m.Pitch("D#4")


def test_chord_analysis():
    chord = m.Chord("C4 E4 G4")
    assert repr(chord) == "<music21.chord.Chord C4 E4 G4>"
    assert chord.commonName == "major triad"
    assert chord.isMajorTriad()
    assert chord.inversion() == 0
    assert chord.root().nameWithOctave == "C4"
    assert chord.bass().nameWithOctave == "C4"
    assert [p.nameWithOctave for p in chord.transpose("M3").pitches] == [
        "E4",
        "G#4",
        "B4",
    ]


def test_a_chord_hands_out_one_object_per_note():
    chord = m.Chord("C4 E4 G4")
    assert chord[1] is chord.notes[1]
    assert chord.pitches[0] is chord[0].pitch
    # And so an edit through either reaches the chord.
    chord[0].octave = 3
    assert repr(chord) == "<music21.chord.Chord C3 E4 G4>"
    chord.pitches[1].getEnharmonic(inPlace=True)
    assert repr(chord) == "<music21.chord.Chord C3 F-4 G4>"


def test_note_keywords_build_the_pitch_and_the_duration():
    note = m.Note(step="C", accidental="sharp", octave=2, type="eighth", dots=2)
    assert note.nameWithOctave == "C#2"
    assert note.duration.quarterLength == 0.875
    assert m.Note("C4", type="half") != m.Note("C4", type="quarter")


def test_a_note_keeps_one_duration_object():
    note = m.Note("C4")
    duration = note.duration
    duration.type = "half"
    assert note.duration is duration
    assert note.quarterLength == 2.0


def test_interval_arithmetic():
    interval = m.Interval("P5")
    assert interval.directedName == "P5"
    assert interval.semitones == 7
    assert interval.complement.directedName == "P4"
    assert m.Interval(noteStart=m.Note("C4"), noteEnd=m.Note("E4")).name == "M3"


def test_key_and_signature():
    key = m.Key("f#")
    assert key.sharps == 3
    assert key.mode == "minor"
    assert key.tonic.name == "F#"
    assert [p.name for p in m.KeySignature(2).alteredPitches] == ["F#", "C#"]


def test_tone_row_operations():
    row = m.pcToToneRow([0, 2, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11])
    assert row.pitchClasses() == [0, 2, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    assert row.isTwelveToneRow()
    assert row.zeroCenteredTransformation("I", 0).pitchClasses()[:3] == [0, 10, 11]
    webern = m.getHistoricalRowByName("RowWebernOp29")
    assert webern.pitchClasses() == [3, 11, 2, 1, 5, 4, 7, 6, 10, 9, 0, 8]
    assert webern.composer == "Webern"


def test_errors_are_music21s_exception_classes():
    with pytest.raises(m.PitchException):
        m.Pitch("Q")
    # music21 reads an octave written before the name as a bad argument
    # rather than as a musical failure, and raises Python's own ValueError.
    with pytest.raises(ValueError):
        m.Pitch("4c")
    with pytest.raises(m.NotRestException):
        m.Note("C4").notehead = "junk"


def test_duration_module_functions():
    assert m.nextLargerType("quarter") == "half"
    assert m.nextSmallerType("quarter") == "eighth"
    with pytest.raises(m.DurationException):
        m.nextLargerType("duplex-maxima")
    assert m.quarterLengthToClosestType(1.5) == ("quarter", False)
    assert m.convertQuarterLengthToType(2.0) == "half"
    assert m.dottedMatch(1.5) == (1, "quarter")
    assert m.dottedMatch(0.4) == (False, False)
    tuplet, component = m.quarterLengthToNonPowerOf2Tuplet(0.4)
    assert (tuplet.numberNotesActual, tuplet.numberNotesNormal) == (5, 4)
    assert component.type == "eighth"
    assert [(t.numberNotesActual, t.numberNotesNormal) for t in m.quarterLengthToTuplet(1 / 3)] == [(3, 2), (3, 1)]
    components, tuplet = m.quarterConversion(5.0)
    assert [c.type for c in components] == ["whole", "quarter"] and tuplet is None
    assert m.convertTypeToQuarterLength("quarter", 1) == 1.5
    assert m.convertTypeToQuarterLength("eighth", 0, [m.Tuplet(3, 2)]) == m.Duration(1 / 3).quarterLength
    assert m.convertTypeToNumber("eighth") == 8.0


def test_sieve_module_functions():
    import itertools

    assert list(itertools.islice(m.eratosthenes(), 6)) == [2, 3, 5, 7, 11, 13]
    assert list(itertools.islice(m.eratosthenes(20), 3)) == [23, 29, 31]
    assert m.rabinMiller(7919) and not m.rabinMiller(561)
    assert m.discreteBinaryPad([3, 4, 6]) == [1, 1, 0, 1]
    assert m.unitNormRange([0, 3, 4]) == [0.0, 0.75, 1.0]
    assert m.unitNormEqual(3) == [0.0, 0.5, 1.0]
    assert m.unitNormStep(0.25) == [0.0, 0.25, 0.5, 0.75, 1.0]


def test_harmony_module_functions():
    assert m.getAbbreviationListGivenChordType("dominant-seventh") == ["7", "dom7"]
    assert m.getCurrentAbbreviationFor("major") == ""
    assert m.getNotationStringGivenChordType("minor-seventh") == "1,-3,5,-7"
    with pytest.raises(KeyError):
        m.getCurrentAbbreviationFor("nonsense")


def test_chord_module_functions():
    assert [p.name for p in m.fromForteClass("3-11").pitches] == ["C", "E-", "G"]
    assert [p.name for p in m.fromForteClass([3, 11]).pitches] == ["C", "E-", "G"]
    assert m.fromForteClass("4-z15").forteClass == "4-15A"
    assert m.fromIntervalVector([1, 1, 1, 1, 1, 1]).forteClass == "4-15A"
    assert m.fromIntervalVector([1, 1, 1, 1, 1, 1], True).forteClass == "4-29A"
    assert m.fromIntervalVector([0, 0, 0, 0, 0, 0]).forteClass == "1-1"
    assert m.fromIntervalVector([9, 9, 9, 9, 9, 9]) is None


def test_roman_module_functions():
    assert m.expandShortHand("65") == ["6", "5"]
    assert m.romanInversionName(m.Chord("E4 G4 C5")) == "6"
    assert m.romanInversionName(m.Chord("E G C")) == ""
    assert m.correctSuffixForChordQuality(m.Chord("B D F"), "6") == "o6"
    chord = m.Chord("G3 B3 D4")
    assert m.identifyAsTonicOrDominant(chord, m.Key("C")) == "V"
    assert chord.root().name == "G"
    assert m.identifyAsTonicOrDominant(m.Chord("G B D"), m.Key("C")) == "V64"
    assert m.identifyAsTonicOrDominant(["F", "A", "C"], m.Key("C")) == "I"
    assert m.identifyAsTonicOrDominant(["D", "F#", "A"], m.Key("C")) == "V43"
    assert m.identifyAsTonicOrDominant(["D", "F#", "A", "C"], m.Key("E")) == "V"
    assert m.identifyAsTonicOrDominant(["E", "F"], m.Key("C")) is False
    assert m.romanNumeralFromChord(m.Chord("G3 B3 D4 F4"), m.Key("C")).figure == "V7"
    assert m.romanNumeralFromChord(m.Chord("G B D F"), m.Key("C")).figure == "V43"
    assert m.romanNumeralFromChord(m.Chord("C4 E-4 G-4"), m.Key("C")).figure == "io5b3"
    assert m.romanNumeralFromChord(m.Chord("A-3 C4 E-4 F#4"), m.Key("c")).figure == "Ger65"
    assert m.romanNumeralFromChord(m.Chord("D F A")).figure == "i"
    assert m.romanNumeralFromChord(m.Chord("D F# A"), "G").figure == "V"
    tuples = m.figureTuples(m.Chord("B3 D4 F4 A4"), m.Key("C"))
    assert [t[:3] for t in tuples] == [(1, 0.0, ""), (3, 0.0, ""), (5, 0.0, ""), (7, 0.0, "")]
    assert tuples[0][3].name == "B"
    assert m.figureTupleSolo(m.Pitch("G#4"), m.Key("a"), m.Pitch("E4")) == (3, 1.0, "#")
    assert m.correctRNAlterationForMinor((7, 0.0, ""), m.Key("a")) == (7, 0.0, "b")
    assert m.correctRNAlterationForMinor((7, 1.0, "#"), m.Key("a")) == (7, 0.0, "")


class _Placed:
    """A note or chord at an offset, as a stream would hold it."""

    def __init__(self, element, offset):
        self.element = element
        self.offset = offset

    def __getattr__(self, name):
        return getattr(self.element, name)

    @property
    def pitches(self):
        return self.element.pitches

    @pitches.setter
    def pitches(self, value):
        self.element.pitches = value


class _Stream:
    """The little of a music21 stream the meter and scale helpers read."""

    def __init__(self, elements):
        self.elements = []
        offset = 0.0
        for element in elements:
            self.elements.append(_Placed(element, offset))
            offset += float(element.duration.quarterLength)

    def __iter__(self):
        return iter(self.elements)

    def __len__(self):
        return len(self.elements)

    @property
    def notes(self):
        return [e for e in self.elements if not e.isRest]

    @property
    def notesAndRests(self):
        return list(self.elements)

    def flatten(self):
        return self

    def recurse(self):
        return self


def test_meter_functions_over_a_stream():
    notes = [m.Note("C4", quarterLength=1.0), m.Note("D4", quarterLength=1.0), m.Note("E4", quarterLength=1.0), m.Note("F4", quarterLength=1.0)]
    assert m.TimeSignature("4/4").averageBeatStrength(_Stream(notes)) == 0.5
    assert m.TimeSignature("3/4").averageBeatStrength(_Stream(notes)) == 0.75
    assert m.TimeSignature("4/4").averageBeatStrength(_Stream([])) == 0.0
    assert m.bestTimeSignature(_Stream(notes)).ratioString == "4/4"
    waltz = [m.Note("C4", quarterLength=1.5), m.Note("D4", quarterLength=0.5), m.Note("E4", quarterLength=1.0)]
    assert m.bestTimeSignature(_Stream(waltz)).ratioString == "6/8"
    eighths = [m.Note("C4", quarterLength=0.5) for _ in range(6)]
    assert m.bestTimeSignature(_Stream(eighths)).ratioString == "3/4"


def test_a_scale_tunes_a_stream():
    # An equal-tempered scale carries an enharmonic of every spelling it is
    # handed, so tuning keeps the names and moves nothing, as music21's does.
    notes = [m.Note(name) for name in ["G-4", "D-4", "E4", "B-3", "C5"]]
    chord = m.Chord(["G-4", "D-5", "A4"])
    m.MajorScale("D").tune(_Stream(notes + [chord]))
    assert [n.pitch.nameWithOctave for n in notes] == ["G-4", "D-4", "E4", "B-3", "C5"]
    assert [p.nameWithOctave for p in chord.pitches] == ["G-4", "D-5", "A4"]


@pytest.mark.parametrize(
    ("cls", "argument", "linked"),
    [
        (m.Note, "C4", "pitch"),
        (m.Note, "C4", "duration"),
        (m.Note, "C4", "volume"),
        (m.Pitch, "C#4", "accidental"),
        (m.Chord, "C4 E4 G4", "notes"),
    ],
)
def test_an_object_and_what_it_hands_out_are_freed_together(cls, argument, linked):
    """What a note hands out points back at the note, through Rust.

    A subclass, because that is what `install_into_music21` makes and because
    a bare facade class cannot be weakly referenced.
    """
    held = type("Held", (cls,), {})(argument)
    getattr(held, linked)
    freed = weakref.ref(held)
    del held
    gc.collect()
    assert freed() is None


def test_what_a_facade_hands_out_is_built_without_an_import(monkeypatch):
    """Each object a facade builds is made as the class music21 has installed
    under its name, where there is one. Nothing is installed here, and asking
    must not send Python searching `sys.path` for music21 on every note."""
    m.Chord("C4 E4 G4").pitches
    asked = []

    class Watch:
        def find_spec(self, name, path=None, target=None):
            asked.append(name)

    monkeypatch.setattr(sys, "meta_path", [Watch(), *sys.meta_path])
    chord = m.Chord("C4 E4 G4")
    chord.pitches
    chord.notes
    m.Note("D4").duration
    m.Pitch("C#4").transpose("M3")
    assert asked == []
