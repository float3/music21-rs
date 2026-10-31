"""The `music21_rs` wheel, checked against music21's own answers.

These are not a second copy of the doctest suite — `python-parity` runs
music21's docstrings verbatim and is the real measure. What is here is the
shape of the package as an importable thing: that every name is exported,
that the classes carry music21's reprs and module strings, and that the
handful of behaviours a caller reaches for first are right.
"""

import gc
import os
import pickle
import copy
import random
import math
import importlib.util
import pathlib
import re
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
    assert not [name for name in exported if name.startswith("_")]
    # Private helpers stay reachable: pickles written by these classes name
    # `_thawed`.
    assert hasattr(m, "_thawed")


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


@pytest.mark.parametrize("copier", [copy.deepcopy, copy.copy])
def test_a_copied_note_keeps_its_volume(copier):
    # music21's `splitAtQuarterLength` makes the second piece by deep copy.
    note = m.Note("C4")
    note.volume.velocity = 64
    copied = copier(note)
    assert copied.hasVolumeInformation()
    assert copied.volume.velocity == 64
    assert copied.volume is not note.volume
    assert copied.volume.client is copied
    assert not copier(m.Note("C4")).hasVolumeInformation()


def test_a_grace_note_is_in_no_tuplet():
    note = m.Note("C4")
    note.duration.quarterLength = 1 / 3
    assert len(note.duration.tuplets) == 1
    assert note.duration.getGraceDuration().tuplets == ()
    assert note.duration.getGraceDuration(appoggiatura=True).tuplets == ()
    grace = note.getGrace()
    assert grace.duration.tuplets == ()
    assert grace.duration.type == "eighth"
    assert len(note.duration.tuplets) == 1
    note.getGrace(inPlace=True)
    assert note.duration.tuplets == ()


def test_an_appoggiatura_makes_time():
    duration = m.Duration(1.0).getGraceDuration(appoggiatura=True)
    assert type(duration).__name__ == "AppoggiaturaDuration"
    assert duration.slash is False
    assert duration.makeTime is True


def test_a_grace_note_read_from_a_file_keeps_its_grace_duration():
    score = m.from_humdrum("**kern\n*M2/4\n=1\n8cq\n4d\n4e\n=2\n*-\n")
    grace = score.recurse().notes[0]
    assert type(grace.duration).__name__ == "GraceDuration"
    assert grace.duration.isGrace
    assert grace.duration.type == "eighth"
    assert grace.duration.quarterLength == 0.0
    assert grace.duration.slash is True


def test_a_length_no_single_value_comes_to_is_a_value_in_a_tuplet():
    duration = m.Duration(5 / 6)
    assert duration.type == "quarter"
    (tuplet,) = duration.tuplets
    assert (tuplet.numberNotesActual, tuplet.numberNotesNormal) == (6, 5)


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


def test_duration_infers_a_tuplet_no_tie_reaches():
    from fractions import Fraction

    assigned = m.Duration()
    assigned.quarterLength = Fraction(5, 6)
    for duration in (assigned, m.Duration(5 / 6)):
        assert duration.type == "quarter"
        (tuplet,) = duration.tuplets
        assert (tuplet.numberNotesActual, tuplet.numberNotesNormal) == (6, 5)
        assert tuplet.durationNormal.type == "32nd"
        assert duration.quarterLength == Fraction(5, 6)
    (triplet,) = m.Duration(1 / 3).tuplets
    assert (triplet.numberNotesActual, triplet.numberNotesNormal) == (3, 2)
    assert m.Duration(1.25).tuplets == ()
    assert m.Duration(5 / 6).fullName == "Quarter Tuplet of 6/5ths (5/6 QL)"
    assert m.Duration(21 / 5).fullName == "Breve Tuplet of 40/21st (4 1/5 QL)"
    assert m.Tuplet(3, 1).fullName == "Tuplet of 3/1st"


def test_sieve_module_functions():
    import itertools

    assert list(itertools.islice(m.eratosthenes(), 6)) == [2, 3, 5, 7, 11, 13]
    assert list(itertools.islice(m.eratosthenes(20), 3)) == [23, 29, 31]
    assert m.rabinMiller(7919) and not m.rabinMiller(561)
    assert m.discreteBinaryPad([3, 4, 6]) == [1, 1, 0, 1]
    assert m.unitNormRange([0, 3, 4]) == [0.0, 0.75, 1.0]
    assert m.unitNormEqual(3) == [0.0, 0.5, 1.0]
    assert m.unitNormStep(0.25) == [0.0, 0.25, 0.5, 0.75, 1.0]


def test_sieve_compression():
    sieve = m.Sieve("(5|2)&4&8")
    assert sieve.segment("cmp", segmentFormat="wid") == [8] * 12
    assert sieve.represent("cmp") == "8@0" and sieve.period() == 40
    sieve.compress()
    assert str(sieve) == "8@0" and sieve.period() == 8
    sieve.expand()
    assert str(sieve) == "{5@0|2@0}&4@0&8@0"
    assert m.Sieve("3@0^4@0").represent("cmp") == "6@3|12@4|12@6|12@8"
    with pytest.raises(m.SieveException):
        m.Sieve("-3@0&5").segment("cmp")


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
    assert m.FigureTuple.fromPitchAndReference(
        m.Pitch("G#4"), m.Key("a"), m.Pitch("E4")
    ) == (3, 1.0, "#")
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


SLENDRO = """! slendro.scl
!
Observed Javanese Slendro scale, Helmholtz/Ellis p. 518, nr.94
 5
!
 228.00000
 484.00000
 728.00000
 960.00000
 2/1
"""


def test_a_rest_is_a_length_with_words_under_it():
    # Each answer read off music21 11.0.0b9.
    rest = m.Rest(1.5)
    assert rest.fullName == "Dotted Quarter Rest"
    assert repr(rest) == "<music21.note.Rest dotted-quarter>"
    assert repr(m.Rest(5.0)) == "<music21.note.Rest 5ql>"
    assert m.Rest("half") == m.Rest(2.0)
    assert m.Rest("half") != m.Note("C4", quarterLength=2.0)
    assert rest.isRest and not rest.isNote and rest.name == "rest"
    assert rest.pitches == ()
    rest.addLyric("hel-")
    rest.addLyric("lo")
    assert rest.lyric == "hel\nlo"
    # Through the duration object, which keeps its dot: a dotted whole.
    rest.duration.type = "whole"
    assert rest.quarterLength == 6.0
    assert rest.fullMeasure == "auto" and m.Rest(fullMeasure=True).fullMeasure is True


def names_of(pitches):
    return [str(each) for each in pitches]


def test_a_chord_symbol_sounds_its_figure():
    # Each answer read off music21 11.0.0b9.
    symbol = m.ChordSymbol("C7/B-")
    assert names_of(symbol.pitches) == ["B-2", "C3", "E3", "G3"]
    assert symbol.chordKind == "dominant-seventh"
    assert symbol.inversion() == 3
    assert repr(symbol) == "<music21.harmony.ChordSymbol C7/B->"
    written = m.ChordSymbol(root="D", bass="F", kind="minor-seventh")
    assert names_of(written.pitches) == ["F3", "A3", "C4", "D4"]
    assert written.figure == "Dm7/F"
    added = m.ChordSymbol("Cm7")
    added.addChordStepModification(m.ChordStepModification("add", 9))
    assert names_of(added.pitches) == ["C3", "E-3", "G3", "B-3", "D4"]
    assert added.findFigure() == "Cm7 add 9"
    assert names_of(m.ChordSymbol("Am").transpose("M2").pitches) == ["B2", "D3", "F#3"]
    assert repr(m.NoChord()) == "<music21.harmony.NoChord N.C.>"
    assert m.NoChord().root() is None
    with pytest.raises(ValueError, match="Invalid chord abbreviation 'junk'"):
        m.ChordSymbol("Cjunk")


def a_three_part_score():
    upper = m.Part([m.Note(name) for name in ["C5", "D5", "E5", "E5", "G5"]], id="upper")
    middle = m.Part([m.Note(name) for name in ["E4", "G4", "G4", "C5", "B4"]], id="middle")
    lower = m.Part(id="lower")
    for name, length in [("C3", 1), ("B2", 2), ("A2", 1), ("G2", 1)]:
        lower.append(m.Note(name, quarterLength=length))
    return m.Score([upper, middle, lower])


def test_consecutive_notes_are_the_objects_held():
    # Each answer read off music21 11.0.0b9.
    line = m.Stream()
    for name in ["C4", "D5", None, "B4", "B5"]:
        line.append(m.Rest() if name is None else m.Note(name))
    found = line.findConsecutiveNotes()
    assert found[0] is line[0] and found[2] is None
    assert [None if each is None else each.nameWithOctave for each in found] == [
        "C4", "D5", None, "B4", "B5"
    ]
    skipped = line.findConsecutiveNotes(skipRests=True, skipOctaves=True, noNone=True)
    assert [each.nameWithOctave for each in skipped] == ["C4", "D5", "B4"]
    steps = line.melodicIntervals()
    assert [step.directedName for step in steps] == ["M9", "P8"]
    assert [steps.elementOffset(step) for step in steps] == [1.0, 4.0]
    assert steps[0].noteStart is line[0]
    rests_skipped = line.melodicIntervals(skipRests=True, skipOctaves=True)
    assert [step.directedName for step in rests_skipped] == ["M9", "m-3"]


def test_a_stream_holds_the_objects_it_is_given():
    # Each answer read off music21 11.0.0b9.
    c, d = m.Note("C"), m.Note("D", quarterLength=2)
    melody = m.Stream([c, d])
    assert [melody.elementOffset(each) for each in melody] == [0.0, 1.0]
    assert melody[0] is c and melody.highestTime == 3.0
    score = a_three_part_score()
    assert [score.elementOffset(part) for part in score] == [0.0, 0.0, 0.0]
    assert len(score.parts) == 3 and len(score.flatten()) == 14
    assert repr(score.parts[0]) == "<music21.stream.Part upper>"


def test_a_stream_finds_and_swaps_what_it_holds():
    """music21's `index`, `replace`, `getElementById` and `clear`."""
    line = m.Stream()
    first, second = m.Note("C4"), m.Note("D4")
    line.append([first, second])
    assert line.index(second) == 1
    with pytest.raises(m.StreamException, match="cannot find object"):
        line.index(m.Note("D4"))

    # A replacement stands where the target stood; one already held is
    # refused, and a target the stream lacks is passed over.
    third = m.Note("E4")
    line.replace(second, third)
    assert line.elements == (first, third)
    assert line.elementOffset(third) == 1.0
    with pytest.raises(m.StreamException):
        line.replace(first, third)
    line.replace(m.Note("F4"), m.Note("G4"))
    assert len(line) == 2

    # With recurse, a stream nested inside is searched too.
    outer, inner = m.Stream(), m.Stream()
    inner.id = "inner"
    inner.append(first)
    outer.insert(0, inner)
    fourth = m.Note("A4")
    outer.replace(first, fourth, recurse=True)
    assert inner.elements == (fourth,)

    assert outer.getElementById("inner") is inner
    assert outer.getElementById("nothing") is None

    line.clear()
    assert len(line) == 0
    bar = m.Measure()
    bar.append(m.Note("C4"))
    bar.rightBarline = "final"
    bar.clear()
    assert bar.rightBarline is None


def test_a_stream_has_a_duration_that_can_be_set_apart():
    """music21's `Stream.duration`: the highest time, unless a duration is
    set, which `quarterLength` then answers; `sort` leaves a stream that
    is always in order as it is."""
    line = m.Stream()
    line.append([m.Note("C4"), m.Note("D4", quarterLength=2)])
    assert line.duration.quarterLength == 3.0
    line.duration = m.Duration(8.0)
    assert line.duration.quarterLength == 8.0
    assert line.quarterLength == 8.0
    assert line.highestTime == 3.0
    assert copy.deepcopy(line).quarterLength == 8.0
    with pytest.raises(m.StreamException):
        line.duration = 4.0
    line.duration = None
    assert line.quarterLength == 3.0
    assert line.sort() is None
    assert line.sort(force=True) is None


def test_a_stream_finds_where_an_element_is_held():
    """music21's `iter`, `containerInHierarchy` and `recurseRepr`."""
    score, part, bar = m.Score(), m.Part(), m.Stream()
    bar.id = "m1"
    held = m.Note("D4")
    bar.append([held, m.Note("E4", quarterLength=1 / 3)])
    part.append(bar)
    score.insert(0, part)
    assert score.containerInHierarchy(held).id == "m1"
    assert score.containerInHierarchy(held, setActiveSite=False) is bar
    assert m.Stream().containerInHierarchy(held) is None

    walked = bar.iter()
    assert type(walked).__name__ == "StreamIterator"
    assert list(walked) == [held, bar[1]]
    assert len(part.iter().notes) == 0

    address = re.compile(r"0x[0-9a-f]+")
    shown = address.sub("ADDR", score.recurseRepr())
    assert shown == (
        "{0.0} <music21.stream.Part ADDR>\n"
        "    {0.0} <music21.stream.Stream m1>\n"
        "        {0.0} <music21.note.Note D>\n"
        "        {1.0} <music21.note.Note E>"
    )
    line = address.sub("ADDR", bar.recurseRepr(addBreaks=False, addEndTimes=True))
    assert line == (
        "{0.0 - 1.0} <music21.note.Note D> / "
        "{1.0 - 1.3333} <music21.note.Note E>"
    )
    mixed = bar.recurseRepr(addEndTimes=True, useMixedNumerals=True)
    assert mixed.splitlines()[1] == "{1 - 1 1/3} <music21.note.Note E>"


def test_a_stream_shows_itself_as_text(capsys):
    """music21's `show('text')`, which prints the `recurseRepr` listing,
    and `show('textline')`, which answers it on one line; the formats
    that render are the caller's to reach."""
    line = m.Stream()
    line.append([m.Note("C4"), m.Note("D4", quarterLength=0.5)])
    assert line.show("text") is None
    assert capsys.readouterr().out == (
        "{0.0} <music21.note.Note C>\n{1.0} <music21.note.Note D>\n"
    )
    line.show("txt", addEndTimes=True)
    assert capsys.readouterr().out == (
        "{0.0 - 1.0} <music21.note.Note C>\n{1.0 - 1.5} <music21.note.Note D>\n"
    )
    assert line.show("textline") == (
        "{0.0} <music21.note.Note C> / {1.0} <music21.note.Note D>"
    )
    with pytest.raises(m.StreamException):
        line.show("musicxml")


def test_a_measures_barlines_are_in_its_listings():
    """A measure keeps its barlines as `leftBarline` and `rightBarline`, and
    the listings that ask what it holds by class, flattened or walked list
    them as music21 does: the left first, the right last at the end."""
    bar = m.Measure()
    bar.append([m.TrebleClef(), m.TimeSignature("2/4"), m.Note("C4"), m.Note("D4")])
    bar.leftBarline = "heavy-light"
    bar.rightBarline = "final"
    part = m.Part()
    part.append(bar)
    named = lambda found: [type(each).__name__ for each in found]
    assert named(bar.getElementsByClass("Barline")) == ["Barline", "Barline"]
    assert named(bar.getElementsNotOfClass("Note")) == [
        "Barline", "TrebleClef", "TimeSignature", "Barline",
    ]
    flat = part.flatten()
    assert named(flat) == [
        "Barline", "TrebleClef", "TimeSignature", "Note", "Note", "Barline",
    ]
    assert [float(flat.elementOffset(each)) for each in flat] == [
        0.0, 0.0, 0.0, 0.0, 1.0, 2.0,
    ]
    assert named(part.recurse()) == [
        "Measure", "Barline", "TrebleClef", "TimeSignature", "Note", "Note", "Barline",
    ]
    assert len(part.recurse().getElementsByClass("Barline")) == 2


def test_a_flattened_parts_barlines_stand_nowhere_again():
    """Flattening lists a measure's barlines, as music21's does; read back
    out of a flat stream they are no barline inside a measure, so music21's
    `makeMeasures` and braille, which pass over them, are matched."""
    bar = m.Measure()
    bar.append(m.Note("C4", quarterLength=4))
    bar.leftBarline = "heavy"
    bar.rightBarline = "double"
    part = m.Part()
    part.append([bar, m.Measure([m.Note("D4", quarterLength=4)])])
    flat = part.flatten()

    remade = flat.makeMeasures().getElementsByClass("Measure")
    # music21 counts [3, 2]: its `len` holds the closing barline.
    assert [len(each) for each in remade] == [3, 1]
    assert [str(each.rightBarline) for each in remade] == [
        "None",
        "<music21.bar.Barline type=final>",
    ]
    # music21: two whole notes, the last measure closed and nothing between.
    assert m.objectToBraille(flat) == "⠀⠀⠀⠼⠙⠲⠀⠀⠀\n⠼⠁⠀⠐⠽⠀⠵⠣⠅"


def test_a_measures_barlines_are_found_where_they_are_listed():
    """What the listings show, the lookups find: music21's answers for a
    measure's own barlines, the left at nought, the right at its end."""
    bar = m.Measure()
    bar.append(m.Note("C4", quarterLength=4))
    bar.rightBarline = m.Barline("final")
    bar.leftBarline = m.Barline("heavy")
    right = bar.rightBarline
    named = lambda found: [type(each).__name__ for each in found]

    assert bar.index(right) == 2
    assert float(bar.elementOffset(right)) == 4.0
    assert right in bar
    assert named(bar.getElementsByOffset(4.0)) == ["Barline"]
    assert named(bar.getElementsByOffset(0.0)) == ["Barline", "Note"]
    assert bar.getElementAtOrBefore(4.0) is right
    assert bar.containerInHierarchy(right) is bar
    assert named(bar.iter()) == ["Barline", "Note", "Barline"]

    bar.remove(right)
    assert bar.rightBarline is None
    assert named(bar.getElementsByClass("Barline")) == ["Barline"]


def test_the_flags_music21_takes_more_than_truth_for():
    """`getTimeSignatures(sortByCreationTime='reverse')` is music21's own
    documented spelling, and `atSoundingPitch` takes what equals True or
    False as music21's `in [True, False, 'unknown']` does."""
    line = m.from_tiny_notation("3/4 c4 d e")
    assert len(line.getTimeSignatures(sortByCreationTime="reverse")) == 1
    line.atSoundingPitch = 1
    assert line.atSoundingPitch is True
    line.atSoundingPitch = 0
    assert line.atSoundingPitch is False
    with pytest.raises(m.StreamException):
        line.atSoundingPitch = 2


def test_a_tempo_marks_referent_is_changed_through_the_one_handed_out():
    """music21: a mark's referent written through the object it hands out
    is the mark's, in its tempo, its repr, a copy and a pickle."""
    mark = m.MetronomeMark(number=60)
    mark.referent.type = "half"
    assert mark.getQuarterBPM() == 120.0
    assert repr(mark) == "<music21.tempo.MetronomeMark larghetto Half=60>"
    assert copy.deepcopy(mark).referent.quarterLength == 2.0
    assert pickle.loads(pickle.dumps(mark)).referent.quarterLength == 2.0


def test_a_rests_full_measure_reaches_what_is_written():
    """music21's `fullMeasure` decides how a rest is written: `False`
    writes no measure rest though the rest fills its bar, and in braille
    `True` is a measure rest though it does not, where `'always'` is not."""
    def part_of(length, said, *more):
        bar = m.Measure()
        bar.append(m.TimeSignature("4/4"))
        rest = m.Rest(quarterLength=length)
        rest.fullMeasure = said
        assert rest.fullMeasure == said
        bar.append([rest, *more])
        part = m.Part()
        part.append(bar)
        return part

    def rests(said):
        written = m.to_musicxml(part_of(4, said))
        return re.findall(r"<rest[^>]*/?>", written)

    assert rests(False) == ["<rest />"]
    assert rests(True) == rests("always") == rests("auto") == ['<rest measure="yes" />']

    short = lambda said: m.objectToBraille(part_of(2, said, m.Note("C4", quarterLength=2)))
    assert short(True) == "⠀⠼⠙⠲⠀⠀\n⠼⠚⠀⠍⠐⠝"
    assert short("always") == short("auto") == "⠀⠼⠙⠲⠀⠀\n⠼⠚⠀⠥⠐⠝"


def test_the_stream_walks_answer_with_the_objects_held():
    # Each answer read off music21 11.0.0b9, which finds the same quartets,
    # the same verticality, the same lengths and the same loudness.
    score = a_three_part_score()
    quartets = m.iterateAllVoiceLeadingQuartets(score)
    assert len(quartets) == 11
    assert repr(quartets[0]) == (
        "<music21.voiceLeading.VoiceLeadingQuartet v1n1=C5, v1n2=D5, v2n1=E4, v2n2=G4>"
    )
    assert quartets[0].v1n1 is score.parts[0][0]
    third = score.parts[0][3]
    heard = m.getVerticalityFromObject(third, score).contentDict
    assert {part: [each.nameWithOctave for each in notes] for part, notes in heard.items()} == {
        0: ["E5"],
        1: ["C5"],
        2: ["A2"],
    }

    lead = m.Score()
    for figure in ["C", "G7", "Am"]:
        lead.append(m.ChordSymbol(figure))
        for _ in range(3):
            lead.append(m.Note("C"))
    lead.append(m.ChordSymbol("C"))
    lead.append(m.Note("D", quarterLength=2))
    flat = m.realizeChordSymbolDurations(lead)
    held = [
        (flat.elementOffset(each), each.quarterLength)
        for each in flat
        if isinstance(each, m.ChordSymbol)
    ]
    assert held == [(0.0, 3.0), (3.0, 3.0), (6.0, 3.0), (9.0, 2.0)]

    loud = m.Stream()
    loud.insert(0, m.Note("C4"))
    loud.insert(0, m.Dynamic("pp"))
    loud.insert(2, m.Dynamic("ff"))
    loud.insert(2, m.Note("D4"))
    m.realizeVolume(loud, setAbsoluteVelocity=True)
    assert [each.volume.velocity for each in loud.notes] == [45, 127]

    first, last = m.Note("C"), m.Note("C")
    between = [m.Note(name) for name in "DEF"]
    source, destination = m.Stream(), m.Stream()
    source.insert(10, first)
    for offset, each in zip([11, 12, 13], between):
        source.insert(offset, each)
    source.insert(14, last)
    destination.insert(20.5, first)
    destination.insert(25.0, last)
    m.interpolateElements(first, last, source, destination)
    assert [destination.elementOffset(each) for each in between] == [21.625, 22.75, 23.875]


def test_scales_stand_on_steps_a_caller_gives():
    # Each answer read off music21 11.0.0b9.
    triad = m.OctaveRepeatingScale("c4", ["m3", "M3"])
    assert [str(p) for p in triad.pitches] == ["C4", "E-4", "G4", "C5"]
    assert triad.getScaleDegreeFromPitch("e-") == 2
    assert triad.type == "Octave Repeating"

    fifths = m.CyclicalScale("c4", ["P5"])
    assert [str(p) for p in fifths.getPitches("g2", "g6")] == [
        "B-2", "F3", "C4", "G4", "D5", "A5", "E6"
    ]
    seconds = m.CyclicalScale("c4", ["m2", "m2"])
    assert seconds.abstract.getDegreeMaxUnique() == 2
    assert str(seconds.pitchFromDegree(1, "c2", "c3")) == "B#1"

    whole = m.SieveScale("d4", "1@0", eld=2)
    assert [str(p) for p in whole.getPitches("c2", "c3")] == [
        "C2", "D2", "F-2", "G-2", "A-2", "B-2", "C3"
    ]


def test_a_scala_scale_is_read_from_the_text_of_its_file():
    slendro = m.ScalaScale("c4", SLENDRO)
    assert [str(p) for p in slendro.getPitches("c3", "c5")] == [
        "C3", "D~3(-22c)", "F3(-16c)", "G~3(-22c)", "B-3(-40c)",
        "C4", "D~4(-22c)", "F4(-16c)", "G~4(-22c)", "A~4(+10c)", "C5",
    ]
    assert slendro.type == "Scala: slendro.scl"


# What music21 11.0.0b14 writes for `MajorScale('C').write('midi')` and
# `Key('G').write('midi')`: no notes, as a scale or a key is put in a stream
# as it is and sounds nothing; the key is a key signature in the conductor
# track.
SCALE_MIDI = (
    b"MThd\x00\x00\x00\x06\x00\x01\x00\x02'`MTrk\x00\x00\x00\x14\x00\xffQ\x03"
    b"\x07\xa1 \x00\xffX\x04\x04\x02\x18\x08\xce`\xff/\x00MTrk\x00\x00\x00\t"
    b"\x00\xff\x03\x00\xce`\xff/\x00"
)
G_MAJOR_MIDI = (
    b"MThd\x00\x00\x00\x06\x00\x01\x00\x02'`MTrk\x00\x00\x00\x1a\x00\xffQ\x03"
    b"\x07\xa1 \x00\xffY\x02\x01\x00\x00\xffX\x04\x04\x02\x18\x08\xce`\xff/\x00"
    b"MTrk\x00\x00\x00\t\x00\xff\x03\x00\xce`\xff/\x00"
)


def test_a_scale_and_a_key_show_themselves_as_text(capsys):
    """Not being streams, a scale and a key show as their `repr`: printed
    for `'text'`, answered for `'textline'`. The formats that render are
    the caller's to open."""
    assert m.MajorScale("C").show("text") is None
    m.Key("G").show("txt")
    assert capsys.readouterr().out == (
        "<music21.scale.MajorScale C major>\n<music21.key.Key of G major>\n"
    )
    assert m.Key("G").show("textline") == "<music21.key.Key of G major>"
    with pytest.raises(m.ScaleException):
        m.MajorScale("C").show("musicxml")
    with pytest.raises(m.ScaleException):
        m.Key("G").show()


def test_a_scale_and_a_key_write_text_scala_and_midi(tmp_path):
    written = m.Key("G").write("text", fp=tmp_path / "key.txt")
    assert written == tmp_path / "key.txt"
    assert written.read_text() == "<music21.key.Key of G major>"

    # Each line after the description as music21 writes it.
    scala = m.MajorScale("C").write("scala", fp=str(tmp_path / "c.scl"))
    lines = scala.read_text().splitlines()
    assert lines[0] == "!" and lines[1].startswith("<music21.scale.Abstract")
    assert lines[2:] == [
        "7", "!", "200.0", "400.0", "500.0", "700.0", "900.0", "1100.0", "1200.0"
    ]
    octatonic = m.OctatonicScale("C").write("scl", fp=tmp_path / "o.scl")
    assert octatonic.read_text().splitlines()[2:5] == ["8", "!", "200.0"]

    assert m.MajorScale("C").write("midi", fp=tmp_path / "c.mid").read_bytes() == SCALE_MIDI
    assert m.Key("G").write(".mid", fp=tmp_path / "g.mid").read_bytes() == G_MAJOR_MIDI

    # With nowhere given, a temporary file with the format's extension.
    temporary = m.Key("G").write("midi")
    assert temporary.suffix == ".mid" and temporary.read_bytes() == G_MAJOR_MIDI
    temporary.unlink()
    with pytest.raises(m.ScaleException):
        m.MajorScale("C").write("lilypond")


def test_a_scale_and_a_key_write_musicxml(tmp_path):
    """A measure of the scale's degrees, the tonic held for a whole note and
    a diatonic scale's dominant for a half, the first under the scale's
    name, as music21's exporter writes one."""
    # music21 puts its own extension on a file named without one.
    written = m.Key("G").write("musicxml", fp=tmp_path / "g")
    assert written == tmp_path / "g.musicxml"
    text = written.read_text()
    assert re.search(r"<encoding-date>\d{4}-\d\d-\d\d</encoding-date>", text)
    notes = list(m.from_musicxml(text).recurse().notes)
    assert [n.nameWithOctave for n in notes] == ["G4", "A4", "B4", "C5", "D5", "E5", "F#5"]
    assert [n.quarterLength for n in notes] == [4, 1, 1, 1, 2, 1, 1]
    assert notes[0].lyric == "G major"

    # musicxml is the format written when none is named.
    octatonic = m.OctatonicScale("C").write(fp=tmp_path / "o.musicxml")
    lengths = [n.quarterLength for n in m.from_musicxml(octatonic.read_text()).recurse().notes]
    assert lengths == [4, 1, 1, 1, 1, 1, 1, 1]


def test_a_scale_and_a_key_write_the_musicxml_music21_writes(tmp_path):
    key = pytest.importorskip("music21.key")
    scale = pytest.importorskip("music21.scale")

    def normalized(path):
        # The date, the software and the part's random id are the volatile
        # parts of a document.
        text = pathlib.Path(path).read_text()
        text = re.sub(r"\s*<(encoding-date|software)>[^<]*</\1>", "", text)
        return re.sub(r'id="P[0-9a-f]*"', 'id="P1"', text)

    cases = [
        (m.MajorScale("C"), scale.MajorScale("C")),
        (m.OctatonicScale("C"), scale.OctatonicScale("C")),
        (m.Key("G"), key.Key("G")),
        (m.Key("b-", "minor"), key.Key("b-", "minor")),
    ]
    for ours, theirs in cases:
        written = ours.write("musicxml", fp=tmp_path / "ours.musicxml")
        assert normalized(written) == normalized(theirs.write("musicxml")), repr(ours)


def test_a_scale_and_a_key_write_and_show_once_installed():
    """Under `install_into_music21` a key's and a scale's `write` and `show`
    are the wheel's rather than blocked as unported."""
    pytest.importorskip("music21.stream")
    # In a process of its own, as installing cannot be undone.
    code = (
        "import music21_rs; music21_rs.install_into_music21()\n"
        "from music21 import key, scale\n"
        "key.Key('G').write('musicxml').unlink()\n"
        "print(scale.MajorScale('C').show('textline'))\n"
        "print(scale.MajorScale('C').write('midi').read_bytes() == %r)\n" % SCALE_MIDI
    )
    done = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    assert done.stdout.split() == ["<music21.scale.MajorScale", "C", "major>", "True"]


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


def _editorial_cycle():
    note = type("Held", (m.Note,), {})("C4")
    note.editorial.back = note
    return note


def _spanner_cycle():
    note = type("HeldNote", (m.Note,), {})("C4")
    slur = type("HeldSlur", (m.Slur,), {})(note)
    note.slur = slur
    return slur


def _song_cycle():
    song = type("HeldSong", (m.CTSong,), {})("Song: x\nVP: I | IV |\nS: [C] $VP\n")
    assert song.rules["VP"].parent is song
    return song


def _chord_editorial_cycle():
    chord = type("HeldChord", (m.Chord,), {})("C4 E4")
    chord.editorial.back = chord
    return chord


def _rest_editorial_cycle():
    rest = type("HeldRest", (m.Rest,), {})()
    rest.editorial.back = rest
    return rest


@pytest.mark.parametrize(
    "made",
    [
        _editorial_cycle,
        _chord_editorial_cycle,
        _rest_editorial_cycle,
        _spanner_cycle,
        _song_cycle,
    ],
)
def test_an_object_pointing_back_through_rust_is_freed(made):
    """A note its editorial points back at, a slur its note points back at,
    and a song whose rules name it as their parent: each is a cycle through
    a Rust field, which Python's collector frees only if it is shown it."""
    held = made()
    freed = weakref.ref(held)
    del held
    gc.collect()
    assert freed() is None


def _untraversed(src: pathlib.Path):
    """Each #[pyclass] struct holding a Py<...> field whose impl blocks
    define no __traverse__."""
    found = {}
    for path in sorted(src.glob("*.rs")):
        text = path.read_text()
        for match in re.finditer(
            r"#\[pyclass[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*pub(?:\(crate\))?\s+struct\s+(\w+)\s*\{(.*?)\n\}",
            text,
            re.S,
        ):
            name, body = match.group(1), match.group(2)
            if not re.search(r":\s*[^,\n]*\bPy<", body):
                continue
            traversed = False
            for block in re.finditer(r"\bimpl\s+%s\s*\{" % re.escape(name), text):
                depth, at = 0, block.end() - 1
                while at < len(text):
                    depth += {"{": 1, "}": -1}.get(text[at], 0)
                    if depth == 0:
                        break
                    at += 1
                if "fn __traverse__" in text[block.end():at]:
                    traversed = True
                    break
            if not traversed:
                found[name] = path.name
    return found


# A class holding Python objects in Rust fields with no `__traverse__`, and
# why no cycle runs through it: Python's collector frees a cycle only where
# each object in it shows what it holds.
_UNTRAVERSED = {
    # Tools built over a stream or a score they analyse, search or correct,
    # holding it and their own results; nothing they hold is given a way
    # back to the tool.
    "ActivityMatch": "an analysis over a stream",
    "Ambitus": "an analysis over a stream",
    "ContiguousSegmentOfNotes": "a search result over a stream",
    "ContiguousSegmentSearcher": "a search over a stream",
    "EnharmonicFixer": "a corrector over two streams",
    "Hasher": "settings and the hashes it made",
    "IndexedLyric": "a search result over a stream",
    "KeyAnalyzer": "an analysis over a stream",
    "KeyWeightKeyAnalysis": "an analysis over a stream",
    "LyricSearcher": "a search over a stream",
    "MeasureHash": "a hash over a measure",
    "MeasureSlice": "a corrector's slice of a score",
    "MelodicIntervalDiversity": "an analysis over a stream",
    "ScoreReduction": "a reduction over a score",
    "SegmentMatcher": "a search over a stream",
    "SinglePart": "a corrector's part of a score",
    "StreamAligner": "an alignment over two streams",
    "StreamSearcher": "a search over a stream",
    "TranspositionChecker": "pitches and the chords made of them",
    "WindowedAnalysis": "an analysis over a stream",
    # Values: numbers, names, pitches, keys, meters and lists of them, which
    # hold nothing of their own.
    "Articulation": "a bend's interval",
    "Barline": "a pause, a fermata or a rest",
    "Contributor": "dates",
    "DataSet": "feature extractor classes",
    "EnharmonicScoreRules": "penalty weights",
    "EnharmonicSimplifier": "pitches and spellings",
    "FiguredBassLine": "a key and a meter",
    "FiguredBassScale": "a scale and a key signature",
    "MeterSequence": "meter terminals and sequences, which hold no parent",
    "Ornament": "accidentals and a size",
    "Realization": "a key and a meter",
    "Rules": "voice-leading limits",
    "ScoreLayout": "page, system and staff layouts",
    "StaffLayout": "a size",
    "TabChord": "the text and numbers of a TSV row",
    "TabChordV2": "the text and numbers of a TSV row",
    "TempoText": "a text expression",
    "TextBox": "a style and a place",
    # Braille's working parts, made and dropped inside one transcription.
    "BrailleElementGrouping": "a transcription's grouping",
    "BrailleText": "lines of braille",
    "NoteGroupingTranscriber": "a transcription's settings",
    # Unpitched notes, which the wheel exposes only once installed, where
    # music21's own collector-visible classes hold them.
    "PercussionChord": "its unpitched notes",
}


def test_every_class_holding_python_objects_is_traversed_or_says_why():
    """A Rust field holding a Python object is invisible to Python's
    collector unless its class says what it holds; before the classes said
    so, every note put into a stream leaked. A new class either says, or
    gives here the reason no cycle runs through it."""
    source = pathlib.Path(__file__).resolve().parents[1] / "src"
    found = _untraversed(source)
    unexplained = sorted(set(found) - set(_UNTRAVERSED))
    stale = sorted(set(_UNTRAVERSED) - set(found))
    assert unexplained == [], f"traverse these or say why: {unexplained}"
    assert stale == [], f"these traverse now; drop them here: {stale}"


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


@pytest.mark.parametrize("value", [math.nan, math.inf, -math.inf])
def test_a_pitch_space_number_that_is_not_finite_is_refused(value):
    """music21 raises here, out of the `int()` its own conversion goes
    through. Answering instead is worse than failing: the cast these classes
    used read every one of these as nought, so a nonsense pitch came back as
    a plain C with nothing to say it was nonsense."""
    with pytest.raises(Exception):
        m.Pitch(ps=value)
    pitch = m.Pitch("A4")
    with pytest.raises(Exception):
        pitch.ps = value
    assert pitch.nameWithOctave == "A4"


def test_a_frequency_that_is_not_finite_is_refused():
    pitch = m.Pitch("A4")
    with pytest.raises(Exception):
        pitch.frequency = math.inf
    with pytest.raises(Exception):
        pitch.frequency = math.nan
    with pytest.raises(Exception):
        pitch.frequency = 0.0
    assert pitch.nameWithOctave == "A4"
    pitch.frequency = 880.0
    assert pitch.nameWithOctave == "A5"


@pytest.mark.parametrize("value", [math.nan, math.inf])
def test_an_interval_of_no_number_of_semitones_is_refused(value):
    with pytest.raises(ValueError):
        m.Interval(value)
    with pytest.raises(ValueError):
        m.ChromaticInterval(value)
    assert m.Interval(7).name == "P5"


def test_a_mark_of_no_tempo_divides_by_no_zero():
    """music21 divides sixty by the number and raises `ZeroDivisionError`.
    Answering an infinity instead put it into whatever arithmetic followed."""
    with pytest.raises(ZeroDivisionError):
        m.MetronomeMark(number=0).secondsPerQuarter()
    assert m.MetronomeMark(number=120).secondsPerQuarter() == 0.5


def test_importing_the_package_imports_no_music21():
    """Every exception here is built on music21's own, so building one
    imports music21 — the whole of it, half a second of it. Nothing asked for
    them at import, and they were built anyway; now they are not."""
    code = "import sys, music21_rs; print('music21' in sys.modules)"
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.strip() == "False"

    # Where music21 cannot be imported at all, as in this suite, an
    # exception stands on a plain `Exception` and importing nothing is no
    # feat. The submodule is where the question has teeth, so ask it there
    # too whenever it is checked out.
    submodule = pathlib.Path(__file__).resolve().parents[2] / "music21"
    if not (submodule / "music21" / "__init__.py").exists():
        return
    environment = dict(os.environ, PYTHONPATH=str(submodule))
    done = subprocess.run(
        [sys.executable, "-c", code],
        capture_output=True,
        text=True,
        check=True,
        env=environment,
    )
    assert done.stdout.strip() == "False", (
        "importing the package pulled music21 in, which it must not do: "
        "something builds an exception, or reaches music21, at import time"
    )


def test_an_exception_is_the_class_music21_keeps_under_that_name():
    # The module string is what `install_into_music21` leaves music21 holding,
    # and what a traceback prints.
    assert m.PitchException.__module__ == "music21.pitch"
    assert m.ChordException.__module__ == "music21.chord"
    assert m.IntervalNetworkException.__module__ == "music21.scale.intervalNetwork"
    assert m.PitchException is m.PitchException
    with pytest.raises(AttributeError):
        m.NoSuchException


def test_a_note_is_drawn_with_a_style_of_the_wheels_own():
    # Where there is no music21 to lend its own, as in this suite.
    note = m.Note("C4")
    note.style.color = "blue"
    assert type(note.style).__name__ == "NoteStyle" and note.hasStyleInformation
    style = m.Style()
    assert (style.units, style.hideObjectOnPrint, style.absoluteY) == ("tenths", False, None)
    style.absoluteY = "below"
    assert style.absoluteY == -70
    with pytest.raises(m.TextFormatException):
        style.enclosure = "parabola"


def test_the_wheels_objects_pickle_without_music21():
    import pickle

    for thing in (m.Note("C4"), m.Chord("C4 E4 G4"), m.Key("a"), m.Rest(1.5), m.Style()):
        # What it *is* comes back, not necessarily the very class: where
        # music21 is there to be held by, `_thawed` answers the subclass
        # music21's own streams will hold -- still one of this package's,
        # and the only kind a score read out of music21's pickle cache can
        # come back as.
        assert isinstance(pickle.loads(pickle.dumps(thing)), type(thing))
    assert pickle.loads(pickle.dumps(m.Chord("C4 E4 G4"))) == m.Chord("C4 E4 G4")


def test_a_metric_modulation_moves_a_number_to_another_note_value():
    # Each answer read off music21 11.0.0b9.
    modulation = m.MetricModulation()
    modulation.oldMetronome = m.MetronomeMark(number=60, referent="quarter")
    modulation.newReferent = "half"
    assert modulation.number == 60
    assert modulation.newMetronome.getQuarterBPM() == 120
    equal = m.MetricModulation()
    equal.newMetronome = m.MetronomeMark(number=60)
    equal.setEqualityByReferent(None, "half")
    assert equal.oldMetronome.number == 30
    with pytest.raises(m.MetricModulationException):
        equal.oldMetronome = "sixty"


def test_an_instrument_starts_out_as_music21s_class_does():
    # Each answer read off music21 11.0.0b9.
    piano = m.Piano()
    assert repr(piano) == "<music21.instrument.Piano 'Piano'>"
    assert piano.midiProgram == 0 and piano.lowestNote.nameWithOctave == "A0"
    assert isinstance(piano, m.KeyboardInstrument) and isinstance(piano, m.Instrument)
    clarinet = m.fromString("Clarinet in A")
    assert isinstance(clarinet, m.Clarinet) and isinstance(clarinet, m.WoodwindInstrument)
    assert clarinet.instrumentName == "Clarinet in A"
    assert clarinet.transposition.directedName == "m-3"
    assert type(m.instrumentFromMidiProgram(56)).__name__ == "Trumpet"
    assert m.Triangle().percMapPitch == 81
    assert m.Violin().autoAssignMidiChannel([0, 1]) == 2
    assert m.ensembleNameBySize(3) == "trio"
    with pytest.raises(m.InstrumentException):
        m.fromString("kazoo concerto")


def test_fresh_instrument_ids_are_counted_not_drawn():
    piano, violin = m.Piano(), m.Violin()
    piano.partIdRandomize()
    violin.partIdRandomize()
    piano.instrumentIdRandomize()
    assert piano.partId != violin.partId
    for given, prefix in ((piano.partId, "P"), (violin.partId, "P"), (piano.instrumentId, "I")):
        assert given[0] == prefix and len(given) == 33
        int(given[1:], 16)
    assert int(violin.partId[1:], 16) == int(piano.partId[1:], 16) + 1

    # Counted after a prefix drawn once per process, so two processes
    # writing a score each do not both name their first part the same.
    code = "import music21_rs as m; p = m.Piano(); p.partIdRandomize(); print(p.partId)"
    firsts = {
        subprocess.run(
            [sys.executable, "-c", code], capture_output=True, text=True, check=True
        ).stdout.strip()
        for _ in range(2)
    }
    assert len(firsts) == 2


def test_a_note_a_scale_stands_twice_is_read_as_its_lowest_degree():
    # Rag Marwa's A is its fifth degree and its seventh, and its D- coming
    # down is its second and its seventh.
    marwa = m.RagMarwa("c4")
    for _ in range(20):
        assert marwa.getScaleDegreeFromPitch("A1", m.Direction.ASCENDING) == 5
        assert marwa.getScaleDegreeFromPitch("D-3", m.Direction.DESCENDING) == 2
        assert str(marwa.nextPitch("A1", m.Direction.ASCENDING)) == "B1"
        assert str(marwa.nextPitch("D-2", m.Direction.DESCENDING)) == "C2"


def test_a_figured_bass_segment_voices_as_music21s_does():
    # Each answer read off music21 11.0.0b9.
    dominant = m.Segment(m.Note("G2"), "7")
    assert dominant.pitchNamesInChord == ["G", "B", "D", "F"]
    voicings = dominant.allCorrectSinglePossibilities()
    assert len(voicings) == 8
    assert [p.nameWithOctave for p in voicings[7]] == ["B5", "F5", "D5", "G2"]
    tonic = m.Segment(m.Note("C3"), "")
    pairs = list(dominant.resolveDominantSeventhSegment(tonic))
    assert len(pairs) == 7
    assert [p.nameWithOctave for p in pairs[0][1]] == ["E3", "C3", "C3", "C3"]
    scale = m.FiguredBassScale("d", "minor")
    assert scale.getPitchNames("C#3", "-7") == ["C#", "E", "G", "B--"]
    crossing = tuple(m.Pitch(name) for name in ("C5", "G5", "E4", "C4"))
    assert m.voiceCrossing(crossing)
    rules = m.Rules()
    rules.partMovementLimits.append((1, 2))
    assert m.Segment(fbRules=rules).fbRules.partMovementLimits == [(1, 2)]


class _Restarting:
    """An iterator that starts again when asked its length, as music21's
    StreamIterator does."""

    def __init__(self, items):
        self.items = list(items)
        self.index = 0

    def __iter__(self):
        self.index = 0
        return self

    def __next__(self):
        if self.index >= len(self.items):
            raise StopIteration
        self.index += 1
        return self.items[self.index - 1]

    def __len__(self):
        self.index = 0
        return len(self.items)


def test_an_iterator_that_restarts_is_read_once():
    notes = [m.Note(name) for name in ("C4", "E4", "G4")]
    chord = m.Chord(_Restarting(notes))
    assert [p.nameWithOctave for p in chord.pitches] == ["C4", "E4", "G4"]


@pytest.mark.parametrize(
    ("tonic", "mode", "scale"),
    [("E-", "major", "MajorScale"), ("c", "minor", "MinorScale"), ("d", "dorian", "DorianScale")],
)
def test_a_key_answers_as_the_scale_of_its_mode(tonic, mode, scale):
    # music21's Key is a DiatonicScale, and answers every scale question
    # as the scale of its mode does.
    key = m.Key(tonic, mode)
    same = getattr(m, scale)(tonic.upper())
    asks = [
        lambda s: s.getPitches(),
        lambda s: s.getPitches("C4", "C6"),
        lambda s: s.getTonic(),
        lambda s: s.getDominant(),
        lambda s: s.getLeadingTone(),
        lambda s: s.nextPitch("F4"),
        lambda s: s.nextPitch("F4", direction="descending"),
        lambda s: s.pitchesFromScaleDegrees([1, 3, 5]),
        lambda s: s.intervalBetweenDegrees(1, 5),
        lambda s: s.getDegreeMaxUnique(),
        lambda s: s.isNext("F4", "E-4"),
        lambda s: s.getChord("C4", "C5"),
        lambda s: s.match(["C", "D", "E-"]),
        lambda s: s.findMissing(["C", "D"]),
        lambda s: s.extractPitchList(["C", "D"]),
        lambda s: s.getRelativeMinor(),
        lambda s: s.getRelativeMajor(),
        lambda s: s.getParallelMinor(),
        lambda s: s.getParallelMajor(),
        lambda s: s.chord,
        lambda s: s.isConcrete,
    ]
    for ask in asks:
        assert repr(ask(key)) == repr(ask(same))


def test_a_key_derives_keys_and_numerals_in_itself():
    minor = m.Key("c")
    assert repr(minor.derive(["C", "D", "E-"])) == "<music21.key.Key of G major>"
    assert [repr(k) for k in minor.deriveAll(["C", "D", "E-"])] == [
        "<music21.key.Key of G major>",
        "<music21.key.Key of C major>",
    ]
    assert repr(minor.deriveRanked(["C", "D", "E-"])[0][1]).startswith("<music21.key.Key of")
    assert repr(m.Key("E-").romanNumeral(5)) == "<music21.roman.RomanNumeral V in E- major>"


def test_a_scale_builds_its_chord_without_music21():
    chord = m.MajorScale("E-").getChord("E-4", "B-4")
    assert repr(chord) == "<music21.chord.Chord E-4 F4 G4 A-4 B-4>"


@pytest.mark.parametrize(
    ("degree", "key_or_scale", "figure"),
    [
        (2, lambda: m.Key("C", "dorian"), "II"),
        (3, lambda: m.Key("C", "lydian"), "III"),
        (2, lambda: m.Key("C"), "ii"),
        (2, lambda: m.Key("c"), "ii"),
        (4, lambda: m.Key("c"), "iv"),
        (2, lambda: m.MajorScale("C"), "ii"),
        (2, lambda: m.MinorScale("C"), "ii"),
        (2, lambda: "C", "ii"),
        (6, lambda: "E-", "vi"),
        (5, lambda: None, "V"),
    ],
)
def test_a_degree_is_written_in_the_case_music21_writes_it(degree, key_or_scale, figure):
    # music21 lowers a degree's numeral in a major or minor key alone; a
    # modal key keeps it upper case, and so a major triad.
    assert m.RomanNumeral(degree, key_or_scale()).figure == figure


def test_an_interval_hands_back_notes_without_music21():
    interval = m.Interval(m.Pitch("C4"), m.Pitch("E4"))
    assert repr(interval.noteStart) == "<music21.note.Note C>"
    assert repr(interval.noteEnd) == "<music21.note.Note E>"
    assert interval.noteStart.pitch is interval.pitchStart


def test_additive_meters_divide_their_beats_as_music21_does():
    meter = m.TimeSignature("2+3/8")
    assert [d.quarterLength for d in meter.beatDivisionDurations] == [0.5, 0.5]
    assert [d.quarterLength for d in meter.beatSubDivisionDurations] == [0.25] * 4
    with pytest.raises(m.TimeSignatureException):
        m.TimeSignature("2/4+3/8").beatDivisionDurations
    with pytest.raises(m.TimeSignatureException):
        m.TimeSignature("2/4+3/8").beatSubDivisionDurations


@pytest.mark.parametrize("figure", ["C#-7", "A-#9", "F#-7/E"])
def test_a_chord_symbol_root_no_pitch_spells_is_an_accidental_error(figure):
    with pytest.raises(m.AccidentalException):
        m.ChordSymbol(figure)


def test_a_chord_symbol_reads_degrees_as_python_reads_numbers():
    # music21 hands `-7` to Python's int, which takes the sign: a flat seventh.
    added = m.ChordSymbol("Cb-7").chordStepModifications
    assert [(mod.degree, mod.interval.semitones) for mod in added] == [(7, -1)]


@pytest.mark.parametrize(
    ("figure", "root", "bass", "inversion"),
    [
        ("A10/C", "A3", "C2", 1),
        ("B-10/D", "B-3", "D2", 1),
        ("Ab10/F#", "A3", "F#2", 6),
        ("C5/G", "C4", "G3", 2),
        ("B-5/E", "B-3", "E2", 5),
    ],
)
def test_a_chord_symbol_keeps_the_root_and_bass_music21_voices(figure, root, bass, inversion):
    # music21's root and bass are its own objects, moved with the chord, and
    # need not be the first note of their name.
    symbol = m.ChordSymbol(figure)
    assert symbol.root().nameWithOctave == root
    assert symbol.bass().nameWithOctave == bass
    assert symbol.inversion() == inversion


MUSICXML = """<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Flute</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>2</divisions>
        <key><fifths>2</fifths><mode>major</mode></key>
        <time><beats>2</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note><pitch><step>D</step><octave>4</octave></pitch>
        <duration>2</duration><voice>1</voice><type>quarter</type></note>
      <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch>
        <duration>1</duration><voice>1</voice><type>eighth</type></note>
      <note><chord/><pitch><step>A</step><octave>4</octave></pitch>
        <duration>1</duration><voice>1</voice><type>eighth</type></note>
      <note><rest/><duration>1</duration><voice>1</voice><type>eighth</type></note>
    </measure>
  </part>
</score-partwise>
"""


def test_musicxml_is_read_into_the_wheels_own_streams():
    score = m.from_musicxml(MUSICXML)
    assert type(score).__name__ == "Score"
    (part,) = score.parts
    assert part.partName == "Flute"
    (measure,) = part.getElementsByClass("Measure")
    assert measure.number == 1
    kinds = [type(element).__name__ for element in measure]
    assert kinds == ["TrebleClef", "Key", "TimeSignature", "Note", "Chord", "Rest"]
    note, chord, rest = list(measure)[3:]
    assert note.nameWithOctave == "D4"
    assert [pitch.nameWithOctave for pitch in chord.pitches] == ["F#4", "A4"]
    assert measure.elementOffset(chord) == 1.0
    assert rest.quarterLength == 0.5


def test_what_is_read_is_written_back_as_musicxml():
    written = m.to_musicxml(m.from_musicxml(MUSICXML), encoding_date="2026-01-01")
    assert "<encoding-date>2026-01-01</encoding-date>" in written
    assert "<part-name>Flute</part-name>" in written
    assert "<fifths>2</fifths>" in written
    assert written.count("<note>") == 4
    assert "<chord />" in written
    # And what was written reads back the same.
    again = m.to_musicxml(m.from_musicxml(written), encoding_date="2026-01-01")
    assert again == written


def test_a_lyric_keeps_how_it_is_drawn_through_musicxml():
    sung = MUSICXML.replace(
        "<duration>2</duration><voice>1</voice><type>quarter</type></note>",
        "<duration>2</duration><voice>1</voice><type>quarter</type>"
        '<lyric number="1" justify="left" placement="below">'
        "<syllabic>single</syllabic><text>la</text></lyric></note>",
    )
    score = m.from_musicxml(sung)
    assert (
        '<lyric justify="left" name="1" number="1" placement="below">'
        in m.to_musicxml(score)
    )
    # The lyric's style says the same, and what is written there is written
    # out.
    note = [e for e in score.parts[0].getElementsByClass("Measure")[0]][3]
    lyric = note.lyrics[0]
    assert lyric.hasStyleInformation
    assert lyric.style.justify == "left"
    assert lyric.style.placement == "below"
    lyric.style.justify = "right"
    assert 'justify="right"' in m.to_musicxml(score)


def test_loose_notes_are_written_once_their_notation_is_made():
    part = m.Part()
    part.insert(0, m.TimeSignature("3/4"))
    for offset, (name, length) in enumerate(
        [("C4", 1), ("D4", 1), ("F#4", 1), ("F4", 1)]
    ):
        part.insert(offset, m.Note(name, quarterLength=length))
    # A half note running past the second barline.
    part.insert(4, m.Note("G4", quarterLength=3))
    part.insert(8, m.Note("A4", quarterLength=0.5))
    part.insert(8.5, m.Note("B4", quarterLength=0.5))

    # As it stands it has no measures, and is refused as music21 refuses it.
    with pytest.raises(Exception):
        m.to_musicxml(part)

    written = m.to_musicxml(part, make_notation=True)
    assert written.count("<measure ") == 3
    # The note across the barline is cut and tied.
    assert written.count('<tie type="start" />') == 1
    assert written.count('<tie type="stop" />') == 1
    # The gap before the last two notes is a rest that is not printed.
    assert 'print-object="no"' in written
    # F natural after F sharp is given its natural, and the eighths a beam.
    assert "<accidental>natural</accidental>" in written
    assert '<beam number="1">begin</beam>' in written
    # The part handed in is left as it was.
    assert len(list(part.getElementsByClass("Measure"))) == 0


def test_making_notation_leaves_a_notated_score_as_it_reads():
    read = m.from_musicxml(MUSICXML)
    plain = m.to_musicxml(read, encoding_date="2026-01-01")
    made = m.to_musicxml(read, encoding_date="2026-01-01", make_notation=True)
    assert made.count("<note>") == plain.count("<note>") == 4
    assert made.count("<measure ") == 1


def test_a_document_that_is_not_musicxml_is_refused():
    with pytest.raises(m.StreamException):
        m.from_musicxml("<html></html>")


def measures_of(stream):
    return list(stream.getElementsByClass("Measure"))


def pitches_of(stream):
    return [
        pitch.nameWithOctave
        for measure in measures_of(stream)
        for element in measure
        if type(element).__name__ in ("Note", "Chord")
        for pitch in element.pitches
    ]


def test_every_reader_is_exported():
    for name in (
        "from_musicxml",
        "from_abc",
        "from_abc_number",
        "from_midi",
        "from_tiny_notation",
        "from_humdrum",
        "from_mei",
        "from_roman_text",
        "from_tsv",
        "from_clercq_temperley",
        "from_noteworthy",
        "from_nwc",
        "from_musedata",
        "from_capella",
        "from_volpiano",
    ):
        assert name in m.__all__


ABC_TUNES = """X:1
T:One
M:2/4
L:1/4
K:G
G A | B c |

X:2
T:Two
M:3/4
L:1/4
K:D
D E F | A3 |
"""


def test_abc_is_read_into_a_score_of_parts_and_measures():
    score = m.from_abc("X:1\nT:Scale\nM:2/4\nL:1/4\nK:G\nG A | B c | d e |\n")
    assert type(score).__name__ == "Score"
    (part,) = score.parts
    measures = measures_of(part)
    assert [measure.number for measure in measures] == [0, 1, 2]
    assert [type(element).__name__ for element in measures[0]] == [
        "TrebleClef",
        "Key",
        "TimeSignature",
        "Note",
        "Note",
    ]
    assert pitches_of(part) == ["G4", "A4", "B4", "C5", "D5", "E5"]


def test_several_abc_tunes_are_an_opus_and_one_can_be_read_by_number():
    opus = m.from_abc(ABC_TUNES)
    assert type(opus).__name__ == "Opus"
    first, second = list(opus)
    assert type(first).__name__ == type(second).__name__ == "Score"
    assert pitches_of(first.parts[0]) == ["G4", "A4", "B4", "C5"]
    assert pitches_of(second.parts[0]) == ["D4", "E4", "F#4", "A4"]
    score = m.from_abc_number(ABC_TUNES, 2)
    assert type(score).__name__ == "Score"
    assert pitches_of(score.parts[0]) == ["D4", "E4", "F#4", "A4"]
    with pytest.raises(m.StreamException):
        m.from_abc_number(ABC_TUNES, 3)


def vlq(number):
    out = [number & 0x7F]
    number >>= 7
    while number:
        out.insert(0, (number & 0x7F) | 0x80)
        number >>= 7
    return bytes(out)


def midi_file(*tracks):
    def track(events):
        body = b"".join(vlq(delta) + event for delta, event in events)
        body += vlq(0) + b"\xff\x2f\x00"
        return b"MTrk" + len(body).to_bytes(4, "big") + body

    header = b"MThd" + (6).to_bytes(4, "big") + (1).to_bytes(2, "big")
    header += len(tracks).to_bytes(2, "big") + (480).to_bytes(2, "big")
    return header + b"".join(track(events) for events in tracks)


def test_midi_is_read_from_bytes_into_measures_of_notes_and_chords():
    conductor = [(0, b"\xff\x58\x04\x03\x02\x18\x08"), (0, b"\xff\x51\x03\x07\xa1\x20")]
    piano = [(0, b"\xff\x03\x05Piano"), (0, b"\xc0\x00")]
    for key in (60, 62, 64, 65):
        piano += [(0, bytes([0x90, key, 80])), (480, bytes([0x80, key, 0]))]
    piano += [(0, b"\x90\x3c\x50"), (0, b"\x90\x40\x50"), (960, b"\x80\x3c\x00"), (0, b"\x80\x40\x00")]
    drums = [(0, b"\xff\x03\x05Drums"), (0, b"\x99\x24\x64"), (480, b"\x89\x24\x00")]
    score = m.from_midi(midi_file(conductor, piano, drums))
    assert type(score).__name__ == "Score"
    keys, strokes = score.parts
    first, second = measures_of(keys)
    kinds = [type(element).__name__ for element in first]
    assert kinds == ["Piano", "TrebleClef", "MetronomeMark", "TimeSignature", "Note", "Note", "Note"]
    assert first.getElementsByClass("MetronomeMark")[0].number == 120
    assert pitches_of(keys) == ["C4", "D4", "E4", "F4", "C4", "E4"]
    assert type(list(second)[1]).__name__ == "Chord"
    # A stroke with no pitch has no class here; its measure keeps the rest
    # after it.
    (drum_measure,) = measures_of(strokes)
    assert [type(element).__name__ for element in drum_measure][-1] == "Rest"
    assert pitches_of(strokes) == []


def test_a_score_is_written_as_abc_and_read_back():
    tune = "X:1\nT:Scale\nM:4/4\nL:1/4\nK:G\nGABc|defg|a4|]\n"
    written = m.to_abc(m.from_abc(tune))
    # The title comes back now that a score carries its metadata; the
    # barline style is still not kept.
    assert written.startswith("X:1\nT:Scale\nM:4/4\nL:1/4\nK:G\n"), written
    assert "G A B c | d e f g | a4 |" in written
    assert m.to_abc(m.from_abc(written)) == written
    assert "L:1/8" in m.to_abc(m.from_abc(tune), unit_length=(1, 8))


def test_noteworthy_text_is_read_into_a_part_per_staff():
    score = m.from_noteworthy(
        "!NoteWorthyComposer(2.0)\n|AddStaff|Name:\"Upper\"\n|Key|Signature:Bb\n"
        "|Note|Dur:Half|Pos:0\n|Note|Dur:Half|Pos:#0\n|Bar\n"
        "|AddStaff|\n|Clef|Type:Bass\n|Chord|Dur:Whole|Pos:-1,1,3\n"
    )
    upper, lower = score.parts
    # A position counts from the middle line: the key's flat, then a sharp
    # written over it.
    assert pitches_of(upper) == ["B-4", "B#4"]
    assert pitches_of(lower) == ["C3", "E3", "G3"]
    # music21 keeps the quotation marks round a staff's name.
    assert upper.partName == '"Upper"'


def test_a_binary_noteworthy_file_is_read_compressed_or_not():
    import pathlib
    import zlib

    nwctxt = pathlib.Path(__file__).parents[2] / "music21" / "music21" / "noteworthy"
    if not nwctxt.is_dir():
        pytest.skip("music21's NoteWorthy files are not here")
    plain = (nwctxt / "cuthbert_test1_uncompressed.nwc").read_bytes()
    score = m.from_nwc(plain)
    assert len(score.parts) == 4
    squeezed = b"[NWZ]\x00" + zlib.compress(plain)
    assert pitches_of(m.from_nwc(squeezed).parts[0]) == pitches_of(score.parts[0])


MUSEDATA = "\n".join([
    "",
    "",
    "",
    "01/01/01 x",
    "WK#:1 MV#:1",
    "source",
    "Work",
    "Movement",
    "Flute",
    "1 0",
    "Group memberships: score",
    "score: part 1 of 1",
    "$  K:-1   Q:2   T:2/4   C:4",
    "C5     2        q     u",
    "Bf4    2        q     u",
    "measure 2",
    "A4     4        h     u",
    "/END",
])


def test_musedata_is_read_from_one_file_or_several():
    score = m.from_musedata(MUSEDATA)
    (part,) = score.parts
    assert part.partName == "Flute"
    assert pitches_of(part) == ["C5", "B-4", "A4"]
    assert len(m.from_musedata([MUSEDATA, MUSEDATA]).parts) == 2


CAPELLA = (
    '<score xmlns="http://www.capella.de/CapXML/2.0"><systems><system><staves>'
    '<staff layout="S"><voices><voice><noteObjects>'
    '<clefSign clef="treble"/><timeSign time="2/4"/>'
    '<chord><duration base="1/4"/><heads><head pitch="C5"/></heads></chord>'
    '<chord><duration base="1/4"/><heads><head pitch="E5"><alter step="-1"/></head>'
    '</heads></chord>'
    '</noteObjects></voice></voices></staff></staves></system></systems></score>'
)


def test_capella_is_read_from_its_document_or_its_archive():
    import io
    import zipfile

    (part,) = m.from_capella(CAPELLA).parts
    # Capella writes its octaves one higher.
    assert pitches_of(part) == ["C4", "E-4"]
    packed = io.BytesIO()
    with zipfile.ZipFile(packed, "w") as archive:
        archive.writestr("score.xml", CAPELLA)
    (again,) = m.from_capella(packed.getvalue()).parts
    assert pitches_of(again) == pitches_of(part)


def test_a_file_that_is_not_noteworthy_is_refused():
    with pytest.raises(m.StreamException):
        m.from_noteworthy("|Note|Dur:4th|Pos:1\n")


def test_volpiano_is_read_and_written_back():
    part = m.from_volpiano("1---c--d---fg---3--wE")
    assert type(part).__name__ == "Part"
    assert pitches_of(part) == ["C4", "D4", "F4", "G4", "E-4"]
    # Written back whole: the barline as `----3` and the neume joining
    # `fg`, which is music21's own answer for the same string. Both were
    # lost before the package had a class for a barline or a spanner.
    assert m.to_volpiano(part) == "1---c-d-fg----3wE-"
    assert [type(s).__name__ for s in part.spannerBundle] == ["Neume"]
    bars = part.getElementsByClass("Measure")
    assert [b.rightBarline.type if b.rightBarline else None for b in bars] == [
        "regular",
        None,
    ]


def test_a_score_is_written_as_midi_and_read_back():
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d e~ e g1")
    written = m.to_midi(part)
    assert written[:4] == b"MThd"
    # A conductor track and the part's own.
    assert int.from_bytes(written[10:12], "big") == 2
    again = m.from_midi(written)
    # The tied notes sound as one.
    (read,) = again.parts
    assert pitches_of(read) == ["C4", "D4", "E4", "G4"]


def test_a_file_that_is_not_midi_is_refused():
    with pytest.raises(m.StreamException):
        m.from_midi(b"not a MIDI file")


def test_tiny_notation_is_read_into_a_part_of_measures():
    part = m.from_tiny_notation("tinyNotation: 3/4 E4 r f# g trip{b-8 a g} c'2.")
    assert type(part).__name__ == "Part"
    measures = measures_of(part)
    assert len(measures) == 3
    first = list(measures[0])
    assert [type(element).__name__ for element in first] == [
        "TrebleClef",
        "TimeSignature",
        "Note",
        "Rest",
        "Note",
    ]
    # Upper case is the octave below middle C; the number is the length.
    assert pitches_of(part) == ["E3", "F#4", "G4", "B-4", "A4", "G4", "C5"]
    triplet = list(measures[1])[1]
    assert triplet.duration.tuplets[0].numberNotesActual == 3
    assert triplet.quarterLength == pytest.approx(1 / 3)


def test_humdrum_is_read_with_a_part_for_each_spine():
    score = m.from_humdrum(
        "**kern\t**kern\n*clefF4\t*clefG2\n*M2/4\t*M2/4\n"
        "=1\t=1\n4C\t4c\n4D\t4d\n=2\t=2\n2E\t2e\n==\t==\n*-\t*-\n"
    )
    upper, lower = score.parts
    # The last spine in the file is the first part.
    assert type(measures_of(upper)[0].getElementsByClass("Clef")[0]).__name__ == "TrebleClef"
    assert type(measures_of(lower)[0].getElementsByClass("Clef")[0]).__name__ == "BassClef"
    assert [measure.number for measure in measures_of(upper)] == [1, 2]
    assert pitches_of(upper) == ["C4", "D4", "E4"]
    assert pitches_of(lower) == ["C3", "D3", "E3"]


MEI = """<mei xmlns="http://www.music-encoding.org/ns/mei"><music><score>
<scoreDef meter.count="2" meter.unit="4">
  <staffGrp>
    <staffDef n="1" clef.shape="G" clef.line="2"/>
    <staffDef n="2" clef.shape="F" clef.line="4"/>
  </staffGrp>
</scoreDef>
<section>
  <measure n="1">
    <staff n="1"><layer n="1"><note pname="c" oct="5" dur="4"/><note pname="e" oct="5" dur="4" accid="f"/></layer></staff>
    <staff n="2"><layer n="1"><chord dur="2"><note pname="c" oct="3"/><note pname="g" oct="3"/></chord></layer></staff>
  </measure>
  <measure n="2">
    <staff n="1"><layer n="1"><rest dur="2"/></layer></staff>
    <staff n="2"><layer n="1"><note pname="c" oct="3" dur="2"/></layer></staff>
  </measure>
</section>
</score></music></mei>"""


def test_mei_is_read_with_a_part_for_each_staff():
    upper, lower = m.from_mei(MEI).parts
    assert [measure.number for measure in measures_of(upper)] == [1, 2]
    # Every layer is a voice.
    (voice,) = measures_of(upper)[0].getElementsByClass("Voice")
    assert [note.nameWithOctave for note in voice.notes] == ["C5", "E-5"]
    (rest,) = measures_of(upper)[1].getElementsByClass("Voice")[0]
    assert type(rest).__name__ == "Rest"
    (chord,) = measures_of(lower)[0].getElementsByClass("Voice")[0]
    assert [pitch.nameWithOctave for pitch in chord.pitches] == ["C3", "G3"]


def test_roman_text_is_read_as_chords_carrying_their_figures():
    score = m.from_roman_text("Title: Test\nTime Signature: 3/4\nm1 G: I b3 V6\nm2 I\nm3 NC\n")
    (part,) = score.parts
    first, second, third = measures_of(part)
    key = first.getElementsByClass("Key")[0]
    assert (key.tonic.name, key.mode) == ("G", "major")
    chords = list(first.getElementsByClass("Chord"))
    assert [chord.lyric for chord in chords] == ["G: I", "V6"]
    assert [first.elementOffset(chord) for chord in chords] == [0.0, 2.0]
    assert [pitch.name for pitch in chords[1].pitches] == ["F#", "A", "D"]
    assert chords[1].quarterLength == 1.0
    assert [pitch.name for pitch in second.getElementsByClass("Chord")[0].pitches] == ["G", "B", "D"]
    assert [type(element).__name__ for element in third] == ["NoChord", "Rest"]
    assert [type(chord).__name__ for chord in chords] == ["RomanNumeral", "RomanNumeral"]
    assert [chord.figure for chord in chords] == ["I", "V6"]


def test_a_dcml_table_is_read_and_written():
    table = (
        "chord\tmeasure\tbeat\ttimesig\tglobal_key\tlocal_key\tnumeral\n"
        "I\t1\t1.0\t4/4\tC\tC\tI\n"
        "V\t1\t3.0\t4/4\tC\tC\tV\n"
        "I\t2\t1.0\t4/4\tC\tC\tI\n"
    )
    score = m.from_tsv(table)
    (part,) = score.parts
    first, second = measures_of(part)
    chords = list(first.getElementsByClass("Chord"))
    assert [chord.figure for chord in chords] == ["I", "V"]
    assert [first.elementOffset(chord) for chord in chords] == [0.0, 2.0]
    assert [chord.quarterLength for chord in chords] == [2.0, 2.0]
    assert [pitch.name for pitch in chords[1].pitches] == ["G", "B", "D"]
    # music21 gives the last chord what is left of the part, which for a
    # table whose last measure holds one chord is nothing at all.
    assert second.getElementsByClass("Chord")[0].quarterLength == 0.0
    written = m.to_tsv(score, dcml_version=1).splitlines()
    assert written[0].split("\t")[:3] == ["chord", "altchord", "measure"]
    assert written[1].split("\t")[:6] == ["I", "", "1", "1.0", "", "4/4"]
    assert len(written) == 4
    with pytest.raises(ValueError):
        m.from_tsv(table, dcml_version=3)
def test_a_rock_corpus_analysis_is_read_as_a_part_of_chords():
    part = m.from_clercq_temperley("% Simple Gifts\nVr: I | I IV |\nS: [A] $Vr\n")
    assert type(part).__name__ == "Part"
    first, second = measures_of(part)
    key = first.getElementsByClass("Key")[0]
    assert (key.tonic.name, key.mode) == ("A", "major")
    assert first.getElementsByClass("TimeSignature")[0].ratioString == "4/4"
    (held,) = first.getElementsByClass("Chord")
    assert [pitch.nameWithOctave for pitch in held.pitches] == ["A4", "C#5", "E5"]
    assert held.lyric == "I\nVr"
    assert held.quarterLength == 4.0
    assert held.tie.type == "start"
    chords = list(second.getElementsByClass("Chord"))
    assert [chord.figure for chord in chords] == ["I", "IV"]
    assert [chord.quarterLength for chord in chords] == [2.0, 2.0]
    assert chords[0].tie.type == "stop"
    assert chords[1].tie is None


def test_roman_numerals_are_written_as_roman_text():
    analysis = "Time Signature: 3/4\nm1 G: I b3 V6\nm2 I\n"
    written = m.to_roman_text(m.from_roman_text(analysis))
    assert written.startswith("Composer: Composer unknown\nTitle: Title unknown\n")
    assert written.endswith(analysis)


def test_a_line_is_written_as_lilypond():
    written = m.to_lilypond(m.from_tiny_notation("4/4 c4 d8 e f#4. g8"))
    assert written.startswith('\\version "2.24"')
    assert "fis' 4." in written
    assert m.to_lilypond(m.from_tiny_notation("c1"), version=(2, 18)).startswith(
        '\\version "2.18"'
    )


def test_a_reader_refuses_what_it_cannot_read():
    with pytest.raises(m.StreamException):
        m.from_mei("<html></html>")


def test_a_score_is_chordified_into_one_line_of_chords():
    upper = m.from_tiny_notation("tinyNotation: 2/4 c'2")
    lower = m.from_tiny_notation("tinyNotation: 2/4 e4 g4")
    score = m.Score()
    score.insert(0, upper)
    score.insert(0, lower)
    chords = score.chordify()
    assert type(chords).__name__ == "Part"
    (measure,) = measures_of(chords)
    made = [element for element in measure if type(element).__name__ == "Chord"]
    assert [[pitch.nameWithOctave for pitch in chord.pitches] for chord in made] == [
        ["E4", "C5"],
        ["G4", "C5"],
    ]
    # The upper note sounds on through both chords, tied.
    assert [note.tie.type if note.tie else None for note in made[0].notes] == [None, "start"]
    assert [note.tie.type if note.tie else None for note in made[1].notes] == [None, "stop"]
    with pytest.raises(m.StreamException):
        score.chordify(addPartIdAsGroup=True)


def _flat_notes(stream):
    flat = stream.flatten()
    return [
        (n.nameWithOctave, flat.elementOffset(n), n.quarterLength) for n in flat.notes
    ]


def test_streams_are_sliced_scaled_and_inverted_as_music21_does():
    # Each answer read off music21 11.0.0b12.
    part = m.from_tiny_notation("tinyNotation: 4/4 c2 d4 e4 f1")
    quarters = [
        ("C4", 0.0, 1.0), ("C4", 1.0, 1.0), ("D4", 2.0, 1.0), ("E4", 3.0, 1.0),
        ("F4", 4.0, 1.0), ("F4", 5.0, 1.0), ("F4", 6.0, 1.0), ("F4", 7.0, 1.0),
    ]
    assert _flat_notes(part.sliceByQuarterLengths(1)) == quarters
    assert _flat_notes(part.sliceByBeat()) == quarters
    assert _flat_notes(part.sliceAtOffsets([1.0, 5.0])) == [
        ("C4", 0.0, 1.0), ("C4", 1.0, 1.0), ("D4", 2.0, 1.0), ("E4", 3.0, 1.0),
        ("F4", 4.0, 1.0), ("F4", 5.0, 3.0),
    ]
    assert _flat_notes(part.sliceByGreatestDivisor()) == quarters[:4] + [("F4", 4.0, 4.0)]
    assert _flat_notes(part.augmentOrDiminish(2)) == [
        ("C4", 0.0, 4.0), ("D4", 4.0, 2.0), ("E4", 6.0, 2.0), ("F4", 8.0, 8.0),
    ]
    assert _flat_notes(part.scaleDurations(0.5)) == [
        ("C4", 0.0, 1.0), ("D4", 2.0, 0.5), ("E4", 3.0, 0.5), ("F4", 4.0, 2.0),
    ]
    stretched = part.scaleOffsets(2)
    assert [stretched.elementOffset(measure) for measure in stretched] == [0.0, 8.0]
    assert [n.nameWithOctave for n in part.invertDiatonic().flatten().notes] == [
        "C4", "B3", "A3", "G3",
    ]
    assert [n.nameWithOctave for n in part.invertDiatonic(m.Note("E4")).flatten().notes] == [
        "G4", "F4", "E4", "D4",
    ]
    assert (part.highestOffset, part.lowestOffset) == (4.0, 0.0)
    assert part.hasMeasures() and not part.hasVoices()
    assert part.isGapless and part.isSequence() and part.isWellFormedNotation()
    with pytest.raises(m.StreamException, match="inPlace=True"):
        part.augmentOrDiminish(2, inPlace=True)


def test_parts_become_voices_and_voices_parts_as_music21_does():
    # Each answer read off music21 11.0.0b12.
    upper = m.from_tiny_notation("tinyNotation: 4/4 e1")
    lower = m.from_tiny_notation("tinyNotation: 4/4 C1")
    score = m.Score([upper, lower])
    joined = score.partsToVoices()
    assert len(joined.parts) == 1
    assert [n.stemDirection for n in joined.flatten().notes] == ["up", "down"]
    apart = joined.voicesToParts()
    assert [[n.nameWithOctave for n in p.flatten().notes] for p in apart.parts] == [
        ["E4"], ["C3"],
    ]
    assert len(joined.explode().parts) == 2
    assert len(score.implode().parts) == 1
    assert len(score.partsToVoices(1, permitOneVoicePerPart=True).parts) == 2
    assert len(score.partsToVoices([[1, 0]]).parts) == 1


def test_metadata_carries_its_names_as_music21_does():
    """Every value here is music21's own answer for the same metadata."""
    md = m.Metadata(title="Maple Leaf Rag")
    md.addContributor("composer", "Joplin, Scott")
    assert md.title == "Maple Leaf Rag"
    # A name is read by its unique name, its work id or its abbreviation.
    assert md.otl == "Maple Leaf Rag"
    assert md.composer == "Joplin, Scott"
    # Every reading of a name answers the primitive that name takes.
    (title,) = md["title"]
    assert isinstance(title, m.Text)
    assert str(title) == "Maple Leaf Rag"
    (composer,) = md["composer"]
    assert isinstance(composer, m.Contributor)
    assert composer.role == "composer"
    assert composer.name == "Joplin, Scott"
    # A new metadata is signed under `software`, as music21 signs one.
    assert len(md.software) == 1
    assert md.all(skipNonContributors=True) == (("composer", "Joplin, Scott"),)

    md.number = "3"
    assert md.number == "3"
    assert md.onm == "3"
    # A name outside music21's vocabulary is no attribute at all.
    with pytest.raises(AttributeError):
        md.madeUp
    # A date is read as the date it says.
    md.dateCreated = "1899/12/01"
    (made,) = md["dateCreated"]
    assert isinstance(made, m.DateSingle)
    assert str(made) == "1899/12/01"
    assert made.relevance == "certain"
    # A copyright is a text with a role.
    md.copyright = "Public domain"
    (rights,) = md["copyright"]
    assert isinstance(rights, m.Copyright)
    assert rights.role is None
    # A plural name writes under the name it stands for.
    md.composers = ["Joplin, Scott", "Hayden, Scott"]
    assert md.composers == ("Joplin, Scott", "Hayden, Scott")
    # Asked for the primitives, `all` answers them.
    kinds = {name: type(value).__name__ for name, value in md.all(returnPrimitives=True)}
    assert kinds["title"] == "Text"
    assert kinds["composer"] == "Contributor"
    assert kinds["dateCreated"] == "DateSingle"
    assert kinds["copyright"] == "Copyright"


def test_metadata_values_are_the_primitives_music21_keeps():
    """A date that may be approximate, a text with a language, somebody
    with a role: music21's `metadata.primitives`, over the crate's."""
    # A date any part of which may be missing and any part of which may say
    # how sure it is.
    date = m.Date(year=1805, month=3, monthError="approximate")
    assert str(date) == "1805/03~/--"
    read = m.Date()
    read.loadStr("1030?/12~/?4")
    assert str(read) == "1030?/12~/04?"
    assert (read.year, read.yearError) == (1030, "uncertain")
    assert read.hasError is True
    assert read.hasTime is False
    # A month no calendar has is refused when a date is loaded.
    with pytest.raises(Exception, match="Month must be between 1 and 12"):
        m.Date().load("1999/14/32")

    # The four classes music21 keeps for a date, each refusing a relevance
    # that is not its own.
    assert str(m.DateSingle("2009/12/31", "approximate")) == "2009/12/31"
    assert str(m.DateRelative("2009/12/31", "prior")) == "prior to 2009/12/31"
    assert str(m.DateBetween(["2009/12/31", "2010/1/28"])) == (
        "2009/12/31 to 2010/01/28"
    )
    assert str(m.DateSelection([1750, "1775/03?"], "and")) == (
        "1750/--/-- and 1775/03?/--"
    )
    for call in (
        lambda: m.DateRelative("2009/12/31", "certain"),
        lambda: m.DateBetween(["2009/12/31"], "certain"),
        lambda: m.DateSelection(["2009/12/31"], "certain"),
    ):
        with pytest.raises(Exception, match="not supported by this object"):
            call()

    # A text keeps what it says of itself.
    text = m.Text("Ale is Dear, The", "en")
    assert str(text) == "Ale is Dear, The"
    assert text.language == "en"
    assert text.getNormalizedArticle() == "The Ale is Dear"
    assert text != m.Text("Ale is Dear, The")
    assert m.Text("some text") == m.Text("some text")

    # A contributor goes under one or more names, and a Humdrum
    # abbreviation is read as the role it stands for.
    who = m.Contributor(names=["Chopin, Fryderyk", "Chopin, Frederick"], role="com")
    assert who.role == "composer"
    assert who.name == "Chopin, Fryderyk"
    assert who.names == ["Chopin, Fryderyk", "Chopin, Frederick"]
    assert m.Contributor.abbreviationToRole("lib") == "librettist"
    assert m.Contributor.roleToAbbreviation("composer") == "com"
    # How long somebody lived, which music21 answers as a timedelta: only a
    # date with all three parts makes one, as Python's own constructor asks.
    lived = m.Contributor(
        name="The Composer", role="composer", birth="1923/07/21", death="2013/11/01"
    )
    assert lived.age().days == 32976
    assert str(lived.birth) == "1923/07/21"
    assert m.Contributor(name="Nobody").age() is None


def test_metadata_is_searched_as_music21_searches_it():
    md = m.Metadata(title="Maple Leaf Rag")
    md.addContributor("composer", "Joplin, Scott")
    assert md.search("joplin", field="composer") == (True, "composer")
    assert md.search("joplin", field="compos") == (True, "composer")
    assert md.search("leaf") == (True, "title")
    assert md.search("leaf", field="composer") == (False, None)
    assert md.search("Wagner") == (False, None)
    assert md.search(composer="Joplin") == (True, "composer")
    assert md.search() == (False, None)


def test_an_opus_looks_up_its_scores_as_music21_does():
    opus = m.Opus()
    for number, title in (("1", "Vrienden, kommt alle gaere"), ("2", "O Venus bant")):
        score = m.Score()
        score.metadata = m.Metadata(title=title, number=number)
        opus.insert(0.0, score)

    assert len(opus.scores) == 2
    assert opus.getNumbers() == ["1", "2"]
    assert opus.getScoreByNumber("2").metadata.title == "O Venus bant"
    assert opus.getScoreByTitle("kommt").metadata.title == "Vrienden, kommt alle gaere"
    assert opus.getScoreByNumber("9") is None
    # The score handed back is the one the opus holds, not a copy.
    assert opus.getScoreByNumber("1") is opus.scores[0]


def test_a_stream_keeps_its_metadata_among_its_elements():
    score = m.Score()
    assert score.metadata is None
    md = m.Metadata(title="Wer nur den lieben Gott")
    score.metadata = md
    assert score.metadata is md
    # music21 keeps it among the elements, at the start.
    assert md in list(score.elements)
    # Writing another takes the place of the first.
    score.metadata = m.Metadata(title="Another")
    assert score.metadata.title == "Another"
    assert len([e for e in score.elements if isinstance(e, m.Metadata)]) == 1
    score.metadata = None
    assert score.metadata is None


def test_a_read_collection_is_looked_up_by_number_and_title():
    """Every answer is music21's own for the same ABC."""
    text = (
        "X:1\nT:One\nM:2/4\nL:1/4\nK:C\nC D | E F |]\n\n"
        "X:2\nT:Two\nM:2/4\nL:1/4\nK:C\nG A | B c |]\n"
    )
    opus = m.from_abc(text)
    assert isinstance(opus, m.Opus)
    # The reader fills the metadata in, so a tune can be asked for.
    assert opus.getNumbers() == ["1", "2"]
    assert [score.metadata.title for score in opus.scores] == ["One", "Two"]
    assert opus.getScoreByTitle("Two").metadata.title == "Two"
    assert opus.getScoreByNumber("1").metadata.title == "One"

    merged = opus.mergeScores()
    assert len(merged.parts) == 2
    assert merged.metadata.title == "One"


def test_a_stream_is_analyzed_as_music21_analyzes_it():
    """Every answer is music21's own for the same stream."""
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 e4 g4 c'4")
    assert str(part.analyze("key")) == "C major"
    # The name need not be exact: music21 reads its own names loosest last.
    assert str(part.analyze("kostka")) == "C major"
    assert str(part.analyze("krumhansl")) == "C major"
    for name in ("ambitus", "span", "range"):
        assert part.analyze(name).name == "P8", name
    assert repr(part.analyze("ambitus")) == "<music21.interval.Interval P8>"

    # A stream with no pitches has no ambitus, and a key analysis of one
    # raises, which is what music21 answers and raises.
    assert m.Stream().analyze("ambitus") is None
    with pytest.raises(m.DiscreteAnalysisException):
        m.Stream().analyze("key")
    with pytest.raises(m.DiscreteAnalysisException):
        part.analyze("nonsense")


def test_elements_are_found_by_offset_as_music21_finds_them():
    """Every answer is music21's own for the same flattened part."""
    flat = m.from_tiny_notation("tinyNotation: 4/4 c2 d4 e4").flatten()

    def offsets(found):
        return [round(flat.elementOffset(e), 4) for e in found.elements]

    # Under music21's defaults an element is found only where it begins,
    # so the half note at nought is not found at 1.
    assert offsets(flat.getElementsByOffset(1.0)) == []
    assert offsets(flat.getElementsByOffset(1.0, mustBeginInSpan=False)) == [0.0]
    assert offsets(flat.getElementsByOffset(0.0, 2.0)) == [0.0, 0.0, 0.0, 2.0]
    assert offsets(flat.getElementsByOffset(0.0, 2.0, includeEndBoundary=False)) == [
        0.0,
        0.0,
        0.0,
    ]
    # A class list narrows it, as music21's does.
    assert offsets(flat.getElementsByOffset(0.0, 4.0, classList="Note")) == [
        0.0,
        2.0,
        3.0,
    ]


@pytest.mark.filterwarnings("ignore:Stream.hasElementOfClass was deprecated")
def test_a_stream_is_asked_what_classes_it_holds():
    flat = m.from_tiny_notation("tinyNotation: 4/4 c2 d4 e4").flatten()
    assert flat.hasElementOfClass("Note")
    assert flat.hasElementOfClass("Clef")
    assert not flat.hasElementOfClass("Dynamic")
    # The clef, the meter and the measure's final barline, as music21.
    assert len(flat.getElementsNotOfClass("Note").elements) == 3
    assert len(flat.getElementsByClass("Note").elements) == 3


def test_a_stream_answers_for_its_pitches_and_its_staff():
    part = m.from_tiny_notation("tinyNotation: 4/4 c2 d4 e4")
    assert part.isTwelveTone()
    # Five lines where no layout says otherwise, as music21 answers.
    assert part.staffLines == 5


def test_elements_are_stretched_to_the_next_as_music21_stretches_them():
    """Every length here is music21's own for the same stream."""
    spread = m.Stream()
    for place in (0.0, 10.0, 20.0):
        spread.insert(place, m.Note("C4"))
    spread.insert(15.0, m.Dynamic("ff"))

    def lengths(stream):
        return [e.quarterLength for e in stream.getElementsByClass("Note")]

    # Ten to the next note, ten again past the dynamic, and the last keeps
    # the length it came with to the end of the stream.
    assert lengths(spread.extendDuration("GeneralNote")) == [10.0, 10.0, 1.0]
    # A class answers as its name does.
    assert lengths(spread.extendDuration(m.Note)) == [10.0, 10.0, 1.0]
    # The answer is a copy: music21 deep-copies unless asked for inPlace.
    assert lengths(spread) == [1.0, 1.0, 1.0]
    with pytest.raises(m.StreamException):
        spread.extendDuration("Note", inPlace=True)


def test_a_stream_takes_in_what_another_holds():
    """Every answer is music21's own for the same streams."""
    spread = m.Stream()
    for place in (0.0, 10.0, 20.0):
        spread.insert(place, m.Note("C4"))
    spread.insert(15.0, m.Dynamic("ff"))

    into = m.Stream()
    assert into.mergeElements(spread) is None
    assert len(into) == 4
    assert [into.elementOffset(e) for e in into] == [0.0, 10.0, 15.0, 20.0]
    # The objects themselves come, as music21's do, so both streams hold
    # the same notes.
    assert all(a is b for a, b in zip(list(into), list(spread)))

    notes_only = m.Stream()
    notes_only.mergeElements(spread, classFilterList=("Note",))
    assert len(notes_only) == 3


def test_an_emptied_copy_keeps_what_music21_keeps():
    """What music21's three `mergeAttributes` copy, of what this keeps."""
    part = m.Part()
    part.id = "hi"
    part.partName = "Flute"
    part.insert(0.0, m.Note("C4"))

    empty = part.cloneEmpty()
    assert isinstance(empty, m.Part)
    assert empty.id == "hi"
    assert empty.partName == "Flute"
    assert len(empty) == 0

    # A measure's number, suffix and padding, which music21's
    # `Measure.mergeAttributes` copies. They are set here rather than given
    # to the constructor, which this package does not read them from.
    measure = m.Measure()
    measure.number = 4
    measure.numberSuffix = "a"
    measure.paddingLeft = 1.0
    measure.insert(0.0, m.Note("C4"))

    empty = measure.cloneEmpty()
    assert (empty.number, empty.numberSuffix, empty.paddingLeft) == (4, "a", 1.0)
    assert len(empty) == 0


def tempo_line():
    """A part of four quarters, at sixty and then at a hundred and twenty."""
    part = m.Part()
    for place in range(4):
        part.insert(float(place), m.Note("C4", quarterLength=1.0))
    part.insert(0.0, m.MetronomeMark(number=60))
    part.insert(2.0, m.MetronomeMark(number=120))
    return part


def test_a_stream_says_how_long_it_lasts_in_seconds():
    """Every answer is music21's own for the same stream."""
    # Two quarters at sixty and two at a hundred and twenty.
    assert tempo_line().seconds == 3.0

    bare = m.Part()
    for place in range(4):
        bare.insert(float(place), m.Note("C4", quarterLength=1.0))
    # music21 answers nan where its own search for a tempo finds nothing,
    # and nought where nothing in the stream lasts.
    assert math.isnan(bare.seconds)
    assert m.Part().seconds == 0.0

    # Only the tempi a stream holds itself count, so a score whose tempo is
    # written on a part has none of its own -- which is nan upstream too.
    score = m.Score()
    score.insert(0.0, tempo_line())
    assert math.isnan(score.seconds)


def test_the_tempo_in_force_through_each_stretch_is_music21s():
    """Every answer is music21's own for the same stream."""
    part = tempo_line()
    boundaries = part.metronomeMarkBoundaries()
    assert [(start, end, mark.number) for start, end, mark in boundaries] == [
        (0.0, 2.0, 60.0),
        (2.0, 4.0, 120.0),
    ]
    # The mark handed back is the stream's own object, as music21's is.
    held = [e for e in part.elements if isinstance(e, m.MetronomeMark)]
    assert all(mark is held[at] for at, (_, _, mark) in enumerate(boundaries))

    # The stream is read flattened, so a tempo written on a part is found
    # from the score.
    score = m.Score()
    score.insert(0.0, part)
    assert [(s, e, mark.number) for s, e, mark in score.metronomeMarkBoundaries()] == [
        (0.0, 2.0, 60.0),
        (2.0, 4.0, 120.0),
    ]

    # A stream that states no tempo is counted at a quarter at 120, which
    # is nobody's mark: music21 builds that one afresh as well.
    bare = m.Part()
    bare.insert(0.0, m.Note("C4", quarterLength=4.0))
    [(start, end, mark)] = bare.metronomeMarkBoundaries()
    assert (start, end, mark.number) == (0.0, 4.0, 120.0)
    assert mark not in list(bare.elements)


def test_the_lyrics_are_gathered_as_music21_gathers_them():
    """Every answer is music21's own for the same part."""

    def sung(name, words):
        note = m.Note(name)
        for number, text in words:
            note.lyrics.append(m.Lyric(text=text, number=number))
        return note

    def shown(line):
        return [
            "-" if each is None else shown(each) if isinstance(each, list) else each.text
            for each in line
        ]

    part = m.Part()
    first = m.Measure()
    first.append(m.TrebleClef())
    first.append(m.Rest(quarterLength=1.0))
    first.append(sung("C4", [(1, "first"), (2, "below")]))
    second = m.Measure()
    second.append(sung("D4", [(1, "second")]))
    second.append(sung("E4", []))
    part.append(first)
    part.append(second)

    found = part.lyrics()
    assert sorted(found) == [1, 2]
    # Every note read keeps a place in each line, so the lines stay level
    # with the notes: the clef and the rest sing nothing and keep none.
    assert shown(found[1]) == ["first", "second", "-"]
    assert shown(found[2]) == ["below", "-", "-"]
    # The lyric handed back is the note's own object, as music21's is.
    assert found[1][0] is first.notes[0].lyrics[0]

    # Asked to recurse rather than to read through the barlines, each
    # measure answers a list of its own, padded within itself.
    recursed = part.lyrics(ignoreBarlines=False, recurse=True)
    assert {n: shown(line) for n, line in recursed.items()} == {
        1: [["first"], ["second", "-"]],
        2: [["below"]],
    }
    # Neither, and the measures are not read at all.
    assert part.lyrics(ignoreBarlines=False) == {}

    tied = m.Part()
    tied.append(sung("C4", [(1, "held")]))
    over = sung("C4", [(1, "over")])
    over.tie = m.Tie("stop")
    tied.append(over)
    assert shown(tied.lyrics()[1]) == ["held", "over"]
    assert shown(tied.lyrics(skipTies=True)[1]) == ["held"]


def outline(stream):
    """Every element of a stream in order, as a line apiece."""
    out = []
    for element in stream.recurse():
        if getattr(element, "isStream", False):
            out.append(f"[{type(element).__name__}]")
        else:
            pitches = " ".join(p.nameWithOctave for p in getattr(element, "pitches", ()))
            length = getattr(element, "quarterLength", 0.0)
            out.append(f"{type(element).__name__} {pitches} ql={length}")
    return out


def test_a_stream_is_transposed_as_music21_transposes_it():
    """Every answer is music21's own for the same part."""
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4")
    assert outline(part.transpose("P5"))[3:] == [
        "Note G4 ql=1.0",
        "Note A4 ql=1.0",
        "Note B4 ql=1.0",
        "Note C5 ql=1.0",
        "Barline  ql=0.0",
    ]
    # A count of semitones, which music21 reads as an interval too.
    assert outline(part.transpose(2))[3:] == [
        "Note D4 ql=1.0",
        "Note E4 ql=1.0",
        "Note F#4 ql=1.0",
        "Note G4 ql=1.0",
        "Barline  ql=0.0",
    ]
    # The answer is a copy, as music21's is unless asked for inPlace.
    assert outline(part)[3] == "Note C4 ql=1.0"
    with pytest.raises(m.StreamException):
        part.transpose("P5", inPlace=True)


def test_tied_notes_are_joined_as_music21_joins_them():
    """Every answer is music21's own for the same part."""
    tied = m.from_tiny_notation("tinyNotation: 4/4 c2~ c2 d4 e4 f4 g4")
    stripped = outline(tied.stripTies())
    # The two halves across the barline are one whole note.
    assert stripped[3] == "Note C4 ql=4.0"
    assert stripped[4] == "[Measure]"
    assert stripped[5:] == [
        "Note D4 ql=1.0",
        "Note E4 ql=1.0",
        "Note F4 ql=1.0",
        "Note G4 ql=1.0",
        "Barline  ql=0.0",
    ]


def test_a_template_keeps_the_skeleton_as_music21_keeps_it():
    """Every answer is music21's own for the same part."""
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4")
    # The clef and the meter stay; the notes become one rest as long as
    # the run they filled.
    assert outline(part.template()) == [
        "[Measure]",
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Rest  ql=4.0",
        "Barline  ql=0.0",
    ]
    assert outline(part.template(removeAll=True)) == ["[Measure]", "Rest  ql=4.0"]
    assert outline(part.template(fillWithRests=False)) == [
        "[Measure]",
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Barline  ql=0.0",
    ]
    # The classes taken out are music21's own default set here, so naming
    # others is refused rather than answered differently.
    with pytest.raises(m.StreamException):
        part.template(removeClasses=["Clef"])


def test_offsets_are_quantized_as_music21_quantizes_them():
    """Every answer is music21's own for the same stream."""
    loose = m.Stream()
    for name, offset, length in (
        ("C4", 0.0, 0.26),
        ("D4", 0.48, 0.52),
        ("E4", 1.01, 0.99),
    ):
        loose.insert(offset, m.Note(name, quarterLength=length))

    quantized = loose.quantize()
    assert [
        (quantized.elementOffset(e), e.quarterLength) for e in quantized.notes
    ] == [(0.0, 0.25), (0.5, 0.5), (1.0, 1.0)]
    # Quantizing part of a stream, or only its offsets, is refused.
    with pytest.raises(m.StreamException):
        loose.quantize(processDurations=False)


def test_a_stream_is_cut_into_measures_as_music21_cuts_it():
    """Every answer is music21's own for the same part."""
    loose = m.Part()
    loose.insert(0.0, m.TimeSignature("3/4"))
    for at, name in enumerate(["C4", "D4", "E4", "F4", "G4", "A4"]):
        loose.insert(float(at), m.Note(name, quarterLength=1.0))

    assert outline(loose.makeMeasures()) == [
        "[Measure]",
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Note C4 ql=1.0",
        "Note D4 ql=1.0",
        "Note E4 ql=1.0",
        "[Measure]",
        "Note F4 ql=1.0",
        "Note G4 ql=1.0",
        "Note A4 ql=1.0",
        "Barline  ql=0.0",
    ]

    # The meters may be given in a stream of their own, as music21 takes
    # them: two-four here rather than the stream's own four-four.
    given = m.Stream()
    given.insert(0.0, m.TimeSignature("2/4"))
    bare = m.Part()
    for at, name in enumerate(["C4", "D4", "E4", "F4"]):
        bare.insert(float(at), m.Note(name, quarterLength=1.0))
    assert outline(bare.makeMeasures(given)) == [
        "[Measure]",
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Note C4 ql=1.0",
        "Note D4 ql=1.0",
        "[Measure]",
        "Note E4 ql=1.0",
        "Note F4 ql=1.0",
        "Barline  ql=0.0",
    ]


def test_what_runs_past_a_barline_is_cut_and_tied():
    """Every answer is music21's own for the same part."""
    over = m.Part()
    over.insert(0.0, m.TimeSignature("4/4"))
    over.insert(0.0, m.Note("C4", quarterLength=6.0))
    over.insert(6.0, m.Note("D4", quarterLength=2.0))

    tied = over.makeMeasures().makeTies()
    notes = [e for e in tied.recurse() if getattr(e, "isNote", False)]
    assert [(n.nameWithOctave, n.quarterLength) for n in notes] == [
        ("C4", 4.0),
        ("C4", 2.0),
        ("D4", 2.0),
    ]
    assert [n.tie.type if n.tie else None for n in notes] == ["start", "stop", None]


def test_notes_are_beamed_and_accidentals_decided_as_music21_does():
    """Every answer is music21's own for the same part."""
    eighths = m.from_tiny_notation("tinyNotation: 4/4 c8 d8 e8 f8 g8 a8 b8 c'8")
    beamed = [e for e in eighths.makeBeams().recurse() if getattr(e, "isNote", False)]
    # Beamed in pairs by the beat, as music21 beams them.
    assert [str(n.beams) for n in beamed[:4]] == [
        "<music21.beam.Beams <music21.beam.Beam 1/start>>",
        "<music21.beam.Beams <music21.beam.Beam 1/stop>>",
        "<music21.beam.Beams <music21.beam.Beam 1/start>>",
        "<music21.beam.Beams <music21.beam.Beam 1/stop>>",
    ]

    # The second sharp in the measure is not written again.
    sharps = m.from_tiny_notation("tinyNotation: 4/4 c#4 c#4 d4 e4")
    decided = [
        e for e in sharps.makeAccidentals().recurse() if getattr(e, "isNote", False)
    ]
    assert [
        (n.nameWithOctave, n.pitch.accidental.displayStatus if n.pitch.accidental else None)
        for n in decided
    ] == [("C#4", True), ("C#4", False), ("D4", None), ("E4", None)]


def test_overlapping_notes_go_into_voices():
    """Every answer is music21's own for the same stream."""
    together = m.Stream()
    together.insert(0.0, m.Note("C4", quarterLength=4.0))
    together.insert(0.0, m.Note("E4", quarterLength=2.0))

    assert outline(together.makeVoices(fillGaps=False)) == [
        "[Voice]",
        "Note C4 ql=4.0",
        "[Voice]",
        "Note E4 ql=2.0",
    ]
    # music21 fills the gaps its voices leave by default; nothing here
    # does, so that is refused rather than answered without the rests.
    with pytest.raises(m.StreamException):
        together.makeVoices()


def test_a_stream_is_notated_as_music21_notates_it():
    """Every answer is music21's own for the same part."""
    raw = m.Part()
    raw.insert(0.0, m.TimeSignature("3/4"))
    for at, name in enumerate(["C4", "D4", "E4", "F4"]):
        raw.insert(float(at), m.Note(name, quarterLength=1.0))

    assert outline(raw.makeNotation()) == [
        "[Measure]",
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Note C4 ql=1.0",
        "Note D4 ql=1.0",
        "Note E4 ql=1.0",
        "[Measure]",
        "Note F4 ql=1.0",
        "Barline  ql=0.0",
    ]
    # Each step is taken at music21's own defaults, so asking for others
    # is refused.
    with pytest.raises(m.StreamException):
        raw.makeNotation(bestClef=True)


def test_a_part_is_turned_to_sounding_and_written_pitch():
    """Every answer is music21's own for the same part."""
    line = m.Part()
    line.insert(0.0, m.Clarinet())
    for at, name in enumerate(["C4", "D4", "E4"]):
        line.insert(float(at), m.Note(name, quarterLength=1.0))
    line.atSoundingPitch = False

    def names(stream):
        return [n.nameWithOctave for n in stream.recurse() if getattr(n, "isNote", False)]

    # A clarinet in B flat sounds a tone below what it reads.
    sounding = line.toSoundingPitch()
    assert names(sounding) == ["B-3", "C4", "D4"]
    assert sounding.atSoundingPitch is True
    assert names(sounding.toWrittenPitch()) == ["C4", "D4", "E4"]
    # The answer is a copy, as music21's is unless asked for inPlace.
    assert names(line) == ["C4", "D4", "E4"]


def test_two_streams_are_asked_where_they_strike_together():
    """Every answer is music21's own for the same streams."""
    first = m.Stream()
    for at in (0.0, 1.0, 2.0):
        first.insert(at, m.Note("C4", quarterLength=1.0))
    second = m.Stream()
    for at in (1.0, 3.0):
        second.insert(at, m.Note("E4", quarterLength=1.0))
    assert first.simultaneousAttacks(second) == [1.0]

    score = m.Stream()
    score.insert(0.0, m.Part())
    score.insert(0.0, m.Part())
    assert score.hasPartLikeStreams()
    assert not first.hasPartLikeStreams()


def test_a_stream_says_whether_its_accidentals_have_been_decided():
    """music21's `haveAccidentalsBeenMade`, which it documents as true
    where any accidental has a display status -- "regardless of if
    makeAccidentals has actually been run" -- and which it works out by
    walking the pitches on first asking. Nothing is remembered here, which
    answers the same thing without a flag to remember wrongly."""
    line = m.Stream()
    line.insert(0.0, m.Note("C#4", quarterLength=1.0))
    line.insert(1.0, m.Note("D4", quarterLength=1.0))
    assert not line.haveAccidentalsBeenMade()

    # Said to be shown, and said *not* to be: either is a decision.
    for shown in (True, False):
        decided = m.Stream()
        note = m.Note("C#4", quarterLength=1.0)
        note.pitch.accidental.displayStatus = shown
        decided.insert(0.0, note)
        assert decided.haveAccidentalsBeenMade()


def test_the_gaps_in_a_stream_are_found_as_music21_finds_them():
    """Every answer is music21's own for the same stream."""
    gappy = m.Stream()
    gappy.insert(1.0, m.Note("E4", quarterLength=2.0))
    gappy.insert(5.0, m.Note("F4", quarterLength=4.0))

    gaps = gappy.findGaps()
    # A quarter before the first note, and two between the two, each a
    # rest here where music21 answers a bare object of the same length.
    assert [(gaps.elementOffset(e), e.quarterLength) for e in gaps] == [
        (0.0, 1.0),
        (3.0, 2.0),
    ]
    assert m.Stream().findGaps() is None
    full = m.Stream()
    for at in (0.0, 1.0):
        full.insert(at, m.Note("C4", quarterLength=1.0))
    assert full.findGaps() is None


def test_a_measure_is_cut_in_two_as_music21_cuts_it():
    """Every answer is music21's own for the same measure."""
    whole = m.from_tiny_notation("tinyNotation: 4/4 c2 d2")
    bar = [e for e in whole if getattr(e, "isStream", False)][0]

    first, second = bar.splitAtQuarterLength(1.0)
    assert outline(first) == [
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Note C4 ql=1.0",
    ]
    # The second piece opens with the clef and meter in force, as
    # music21's searchContext finds them, and the half note is tied.
    assert outline(second) == [
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
        "Note C4 ql=1.0",
        "Note D4 ql=2.0",
        "Barline  ql=0.0",
    ]
    notes = [e for e in first.recurse() if getattr(e, "isNote", False)]
    assert notes[0].tie.type == "start"
    assert [e for e in second.recurse() if getattr(e, "isNote", False)][0].tie.type == (
        "stop"
    )


def test_notes_struck_twice_are_tied_in_place():
    """Every answer is music21's own for the same stream."""
    again = m.Stream()
    again.insert(0.0, m.Note("C4", quarterLength=1.0))
    again.insert(1.0, m.Note("C4", quarterLength=1.0))
    again.insert(2.0, m.Note("D4", quarterLength=1.0))

    # music21 writes the ties onto the notes it was given and answers
    # nothing, and the notes here are the stream's own.
    assert again.extendTies() is None
    assert [n.tie.type if n.tie else None for n in again.notes] == [
        "start",
        "stop",
        None,
    ]
    with pytest.raises(m.StreamException):
        again.extendTies(pitchAttr="name")

    # And across the measures of a part, where the clef and the meter
    # stand among the elements the positions are counted over.
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 c2 c2 g4 g4")
    part.extendTies()
    notes = [e for e in part.recurse() if getattr(e, "isNote", False)]
    assert [n.tie.type if n.tie else None for n in notes] == [
        None,
        None,
        None,
        "start",
        "stop",
        "start",
        "stop",
    ]


def test_elements_are_shifted_and_voices_flattened():
    """Every answer is music21's own for the same streams."""
    shifted = m.Stream()
    for at in (0.0, 1.0, 2.0):
        shifted.insert(at, m.Note("C4", quarterLength=1.0))
    # In place and answering nothing, as music21's does, and only what
    # starts at or after the offset given moves.
    assert shifted.shiftElements(2.0, startOffset=1.0) is None
    assert sorted(shifted.elementOffset(e) for e in shifted) == [0.0, 3.0, 4.0]

    holder = m.Measure()
    inner = m.Voice()
    inner.insert(0.0, m.Note("C4", quarterLength=1.0))
    inner.insert(1.0, m.Note("D4", quarterLength=1.0))
    holder.insert(0.0, inner)
    # One voice in a measure is dissolved into the measure itself.
    assert outline(holder.flattenUnnecessaryVoices()) == [
        "Note C4 ql=1.0",
        "Note D4 ql=1.0",
    ]


def test_what_stands_at_one_offset_is_listed_in_music21s_order():
    """Every order here is music21's own `classSortOrder`."""
    line = m.Stream()
    for at, name in enumerate(["C4", "D4", "E4"]):
        line.insert(float(at), m.Note(name, quarterLength=1.0))
    # Put in last and listed first, the clef sorting before a note.
    line.insert(0.0, m.TrebleClef())
    assert [type(e).__name__ for e in line] == ["TrebleClef", "Note", "Note", "Note"]

    opening = m.Measure()
    for element in (
        m.Note("C4", quarterLength=1.0),
        m.TimeSignature("4/4"),
        m.MetronomeMark(number=120),
        m.KeySignature(2),
        m.TrebleClef(),
        m.Piano(),
    ):
        opening.insert(0.0, element)
    assert [type(e).__name__ for e in opening] == [
        "Piano",
        "TrebleClef",
        "MetronomeMark",
        "KeySignature",
        "TimeSignature",
        "Note",
    ]
    # A voice sorts after its measure's clef, key and meter, where every
    # other stream sorts before them.
    assert m.Voice().classSortOrder == 5
    assert m.Part().classSortOrder == -20


def test_elements_are_taken_out_by_class_and_by_index():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    for at, name in enumerate(["C4", "D4", "E4"]):
        line.insert(float(at), m.Note(name, quarterLength=1.0))
    line.insert(0.0, m.TrebleClef())

    # pop answers the object itself, by its place in `elements`.
    assert type(line.pop(0)).__name__ == "TrebleClef"
    assert line.pop().nameWithOctave == "E4"
    assert type(line.pop(-1)).__name__ == "Note"
    assert len(line) == 1
    with pytest.raises(IndexError):
        m.Stream().pop()

    held = m.Stream()
    held.insert(0.0, m.TrebleClef())
    for at, name in enumerate(["C4", "D4"]):
        held.insert(float(at), m.Note(name, quarterLength=1.0))
    assert held.removeByClass("Note") is None
    assert [type(e).__name__ for e in held] == ["TrebleClef"]

    other = m.Stream()
    other.insert(0.0, m.TrebleClef())
    for at, name in enumerate(["C4", "D4"]):
        other.insert(float(at), m.Note(name, quarterLength=1.0))
    assert other.removeByNotOfClass("Note") is None
    assert [type(e).__name__ for e in other] == ["Note", "Note"]


def test_the_voices_a_stream_holds_are_its_own():
    holder = m.Stream()
    first = m.Voice()
    first.insert(0.0, m.Note("C4", quarterLength=1.0))
    second = m.Voice()
    second.insert(0.0, m.Note("E4", quarterLength=1.0))
    holder.insert(0.0, first)
    holder.insert(0.0, second)

    voices = list(holder.voices)
    assert len(voices) == 2
    # The voices handed back are the stream's own, as music21's are.
    assert voices[0] is first and voices[1] is second


def test_what_a_stream_opens_with_is_read_and_written():
    """Every answer is music21's own for the same measure."""
    bar = m.Measure()
    assert (bar.clef, bar.keySignature, bar.timeSignature) == (None, None, None)

    first = m.TrebleClef()
    bar.clef = first
    bar.keySignature = m.KeySignature(2)
    bar.timeSignature = m.TimeSignature("3/4")
    bar.append(m.Note("C4", quarterLength=1.0))
    assert bar.clef.sign == "G"
    assert bar.keySignature.sharps == 2
    assert bar.timeSignature.ratioString == "3/4"
    # The clef handed back is the measure's own object.
    assert bar.clef is first

    # Writing another takes the first out, as music21's setter does.
    bar.clef = m.BassClef()
    assert bar.clef.sign == "F"
    assert [type(e).__name__ for e in bar] == [
        "BassClef",
        "KeySignature",
        "TimeSignature",
        "Note",
    ]
    bar.clef = None
    assert bar.clef is None
    assert len(bar) == 3

    # Only what stands at the very start is what the stream opens with.
    later = m.Measure()
    later.insert(2.0, m.TrebleClef())
    assert later.clef is None


def test_the_meters_and_instruments_a_stream_holds_are_found():
    """Every answer is music21's own for the same part."""
    line = m.Part()
    first = m.Measure()
    first.insert(0.0, m.TimeSignature("3/4"))
    first.append(m.Note("C4", quarterLength=1.0))
    second = m.Measure()
    second.insert(0.0, m.TimeSignature("2/2"))
    second.append(m.Note("D4", quarterLength=1.0))
    line.insert(0.0, first)
    line.insert(3.0, second)

    found = line.getTimeSignatures()
    assert [(found.elementOffset(t), t.ratioString) for t in found] == [
        (0.0, "3/4"),
        (3.0, "2/2"),
    ]
    # The meters handed back are the measures' own objects.
    assert list(found)[0] is first.timeSignature
    # The part itself states none, so nothing is found without recursing.
    assert len(line.getTimeSignatures(recurse=False, returnDefault=False)) == 0
    # And a four-four is made up where one is asked for, as music21 does.
    assert [t.ratioString for t in line.getTimeSignatures(recurse=False)] == ["4/4"]

    played = m.Part()
    flute = m.Flute()
    played.insert(0.0, flute)
    played.append(m.Note("C4", quarterLength=1.0))
    assert list(played.getInstruments())[0] is flute
    assert played.getInstrument() is flute

    none = m.Part()
    none.append(m.Note("C4", quarterLength=1.0))
    default = none.getInstruments()
    assert [type(i).__name__ for i in default] == ["Instrument"]
    # music21 gives the one it makes up the default part name, which is
    # the empty string.
    assert list(default)[0].partName == ""
    assert len(none.getInstruments(returnDefault=False)) == 0
    assert none.getInstrument(returnDefault=False) is None


def test_an_element_is_found_at_or_before_an_offset():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    for at, name in ((0.0, "C4"), (10.0, "D4"), (20.0, "E4"), (30.0, "F4")):
        line.insert(at, m.Note(name, quarterLength=1.0))
    clef = m.BassClef()
    line.insert(20.0, clef)

    assert line.getElementAtOrBefore(21).nameWithOctave == "E4"
    # Of two things standing at one offset the later in the listing wins,
    # which is the note rather than the clef sorting before it.
    assert line.getElementAtOrBefore(20).nameWithOctave == "E4"
    assert line.getElementAtOrBefore(20, ["Note"]).nameWithOctave == "E4"
    assert line.getElementAtOrBefore(0).nameWithOctave == "C4"
    assert line.getElementAtOrBefore(-1) is None
    # What stands at the offset itself does not count here.
    assert line.getElementBeforeOffset(20).nameWithOctave == "D4"
    assert line.getElementBeforeOffset(20, ["Note"]).nameWithOctave == "D4"

    first = line.notes[0]
    assert line.getElementAfterElement(first).nameWithOctave == "D4"
    assert line.getElementAfterElement(clef).nameWithOctave == "E4"
    assert line.getElementAfterElement(first, ["Clef"]) is clef
    assert line.getElementAfterElement(line.notes[-1]) is None
    with pytest.raises(m.StreamException):
        line.getElementAfterElement(m.Note("C4"))


def test_an_object_is_repeated_into_a_stream_as_copies():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    one = m.Note("C4", quarterLength=2.0)
    assert line.repeatAppend(one, 3) is None
    assert sorted(line.elementOffset(e) for e in line) == [0.0, 2.0, 4.0]
    # Copies, as music21's deepcopy makes them: the object handed in is
    # not one of the three.
    assert not any(e is one for e in line)
    assert [e.nameWithOctave for e in line] == ["C4", "C4", "C4"]

    other = m.Stream()
    other.repeatInsert(m.Note("E4", quarterLength=1.0), [0.0, 4.0, 8.0])
    assert sorted(other.elementOffset(e) for e in other) == [0.0, 4.0, 8.0]
    # And each copy is its own note.
    list(other)[0].pitch.name = "G"
    assert sorted(e.nameWithOctave for e in other) == ["E4", "E4", "G4"]


def test_the_measures_of_a_score_are_gathered_by_offset():
    """Every answer is music21's own for the same score."""
    first = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4 g4 a4 b4 c'4")
    second = m.from_tiny_notation("tinyNotation: 4/4 C4 D4 E4 F4 G4 A4 B4 c4")
    whole = m.Score()
    whole.insert(0.0, first)
    whole.insert(0.0, second)

    def shown(the_map):
        return {at: [bar.number for bar in bars] for at, bars in the_map.items()}

    assert shown(first.measureOffsetMap()) == {0.0: [1], 4.0: [2]}
    # A score answers out of its parts, so the measures standing together
    # in the two parts are listed together.
    assert shown(whole.measureOffsetMap()) == {0.0: [1, 1], 4.0: [2, 2]}
    assert list(whole.measureOffsetMap()) == [0.0, 4.0]
    # The measures are the parts' own objects.
    assert whole.measureOffsetMap()[0.0][0] is first.getElementsByClass("Measure")[0]
    # Gathering by another class is refused rather than answered without
    # the sites it would need.
    with pytest.raises(m.StreamException):
        whole.measureOffsetMap(["Note"])


def test_an_object_is_inserted_and_what_follows_shifted():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    for at, name in ((0.0, "C4"), (1.0, "D4"), (2.0, "E4")):
        line.insert(at, m.Note(name, quarterLength=1.0))

    def shown(stream):
        return sorted((stream.elementOffset(e), e.nameWithOctave) for e in stream)

    # Everything from the element nearest after the offset is moved far
    # enough that it starts where the new note ends.
    assert line.insertAndShift(1.0, m.Note("F4", quarterLength=2.0)) is None
    assert shown(line) == [(0.0, "C4"), (1.0, "F4"), (3.0, "D4"), (4.0, "E4")]

    # Nothing standing after the offset means nothing to move.
    tail = m.Stream()
    tail.insert(0.0, m.Note("C4", quarterLength=1.0))
    tail.insertAndShift(4.0, m.Note("A4", quarterLength=1.0))
    assert shown(tail) == [(0.0, "C4"), (4.0, "A4")]


def test_a_stream_is_divided_by_what_a_function_says():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    for at, name in enumerate(["C4", "G5", "E4", "B5"]):
        line.insert(float(at), m.Note(name, quarterLength=1.0))
    line.insert(0.0, m.TrebleClef())

    high, low = line.splitByClass("Note", lambda n: n.octave > 4)
    assert [e.nameWithOctave for e in high] == ["G5", "B5"]
    assert [e.nameWithOctave for e in low] == ["C4", "E4"]
    # Each keeps the offset it stood at, and the objects are the stream's
    # own, so the clef -- of another class -- is in neither.
    assert [high.elementOffset(e) for e in high] == [1.0, 3.0]
    assert list(low)[0] is line.notes[0]

    # No class at all takes everything the stream holds, as music21 does.
    every, none = line.splitByClass(None, lambda e: True)
    assert len(every) == 5
    assert len(none) == 0


def test_measures_are_taken_from_a_part_as_music21_takes_them():
    """Every answer is music21's own for the same part and score."""
    line = m.from_tiny_notation(
        "tinyNotation: 4/4 c4 d4 e4 f4 g4 a4 b4 c'4 d'4 e'4 f'4 g'4"
    )

    def shown(stream):
        out = []
        for element in stream:
            at = stream.elementOffset(element)
            if getattr(element, "isStream", False):
                out.append(f"[{type(element).__name__} {element.number}@{at}]")
            else:
                out.append(f"{type(element).__name__}@{at}")
        return out

    # The first measure stands at nought, and before it what was in force
    # where it begins: the clef and the meter, which music21 puts first by
    # giving them the measure's priority less one.
    assert shown(line.measures(2, 3)) == [
        "TrebleClef@0.0",
        "TimeSignature@0.0",
        "[Measure 2@0.0]",
        "[Measure 3@4.0]",
    ]
    assert shown(line.measures(3, 3)) == [
        "TrebleClef@0.0",
        "TimeSignature@0.0",
        "[Measure 3@0.0]",
    ]
    # With indices music21 slices, so the end is left out.
    assert shown(line.measures(0, 1, indicesNotNumbers=True)) == ["[Measure 1@0.0]"]
    assert shown(line.measures(9, 10)) == []

    one = line.measure(2)
    assert one.number == 2
    assert [n.nameWithOctave for n in one.notes] == ["G4", "A4", "B4", "C5"]
    # The measure is the part's own object.
    assert one is line.getElementsByClass("Measure")[1]
    assert line.measure(9) is None

    # A score answers a score of each part's measures of that number.
    whole = m.Score()
    whole.insert(0.0, m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4 g4 a4 b4 c'4"))
    whole.insert(0.0, m.from_tiny_notation("tinyNotation: 4/4 C4 D4 E4 F4 G4 A4 B4 c4"))
    cut = whole.measures(2, 2)
    assert isinstance(cut, m.Score)
    assert [shown(part) for part in cut] == [
        ["TrebleClef@0.0", "TimeSignature@0.0", "[Measure 2@0.0]"],
        ["BassClef@0.0", "TimeSignature@0.0", "[Measure 2@0.0]"],
    ]


def test_a_stream_says_where_each_element_begins_and_ends():
    """Every answer is music21's own for the same streams."""
    line = m.Stream()
    line.insert(0.0, m.TrebleClef())
    for at, name in ((0.0, "C4"), (1.0, "D4"), (2.0, "E4")):
        line.insert(at, m.Note(name, quarterLength=1.0))

    def shown(the_map):
        return [
            (type(e.element).__name__, e.offset, e.endTime, e.voiceIndex)
            for e in the_map
        ]

    assert shown(line.offsetMap()) == [
        ("TrebleClef", 0.0, 0.0, None),
        ("Note", 0.0, 1.0, None),
        ("Note", 1.0, 2.0, None),
        ("Note", 2.0, 3.0, None),
    ]
    assert line.offsetMap()[0]._fields == ("element", "offset", "endTime", "voiceIndex")
    # The element is the stream's own object.
    assert line.offsetMap()[0].element is list(line)[0]

    # A measure with voices is read voice by voice, what stands outside
    # them first and with no voice of its own, as music21 reads it.
    bar = m.Measure()
    first = m.Voice()
    first.insert(0.0, m.Note("C4", quarterLength=2.0))
    second = m.Voice()
    second.insert(0.0, m.Note("E4", quarterLength=1.0))
    second.insert(1.0, m.Note("F4", quarterLength=1.0))
    bar.insert(0.0, m.TrebleClef())
    bar.insert(0.0, first)
    bar.insert(0.0, second)
    assert shown(bar.offsetMap()) == [
        ("TrebleClef", 0.0, 0.0, None),
        ("Note", 0.0, 2.0, 0),
        ("Note", 0.0, 1.0, 1),
        ("Note", 1.0, 2.0, 1),
    ]


def test_every_pitch_a_stream_holds_is_gathered():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    line.insert(0.0, m.Key("G"))
    line.insert(0.0, m.Note("C4", quarterLength=1.0))
    line.insert(1.0, m.Chord(["E4", "G4"]))
    line.insert(2.0, m.Rest(quarterLength=1.0))

    # A note's one pitch and a chord's two; the key's are not the music's
    # and music21 passes it over.
    assert [p.nameWithOctave for p in line.pitches] == ["C4", "E4", "G4"]
    # Each is the very object the note holds.
    assert line.pitches[0] is line.notes[0].pitch

    # The pitches of a nested stream come too, so a part answers for its
    # measures without being flattened.
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4")
    assert [p.nameWithOctave for p in part.pitches] == ["C4", "D4", "E4", "F4"]


def test_what_stands_around_an_element_is_extracted():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    for at, name in enumerate(["C4", "D4", "E4", "F4", "G4", "A4", "B4", "c5"]):
        line.insert(float(at), m.Note(name, quarterLength=1.0))
    middle = list(line)[4]

    around = line.extractContext(middle, 2.0, 2.0)
    # From two quarters before it begins to two past where it ends, the
    # elements keeping the offsets they stood at.
    assert [(around.elementOffset(e), e.nameWithOctave) for e in around] == [
        (2.0, "E4"),
        (3.0, "F4"),
        (4.0, "G4"),
        (5.0, "A4"),
        (6.0, "B4"),
    ]
    assert list(around)[0] is list(line)[2]
    assert [e.nameWithOctave for e in line.extractContext(middle, 0.0, 1.0)] == [
        "G4",
        "A4",
    ]
    with pytest.raises(m.StreamException):
        line.extractContext(m.Note("C4"))


def test_a_measure_is_filled_out_with_rests():
    """Every answer is music21's own for the same part."""
    line = m.Part()
    bar = m.Measure()
    bar.insert(0.0, m.TimeSignature("4/4"))
    bar.insert(0.0, m.Note("C4", quarterLength=1.0))
    bar.insert(3.0, m.Note("D4", quarterLength=1.0))
    line.insert(0.0, bar)

    filled = line.makeRests(fillGaps=True, timeRangeFromBarDuration=True)
    assert [
        (type(e).__name__, getattr(e, "quarterLength", 0.0))
        for e in filled.recurse()
        if not getattr(e, "isStream", False)
    ] == [("TimeSignature", 0.0), ("Note", 1.0), ("Rest", 2.0), ("Note", 1.0)]

    # That one combination of music21's keywords is what the crate
    # answers, so its defaults are refused rather than answered as though
    # they had been asked for.
    with pytest.raises(m.StreamException):
        line.makeRests()


def test_what_sounds_while_another_part_sounds_is_found():
    """Every answer is music21's own for the same two streams."""
    upper = m.Stream()
    upper.insert(0.0, m.Note("C5", quarterLength=2.0))
    upper.insert(2.0, m.Note("D5", quarterLength=2.0))
    lower = m.Stream()
    for at, name in enumerate(["C3", "D3", "E3", "F3"]):
        lower.insert(float(at), m.Note(name, quarterLength=1.0))

    held = list(upper)[1]
    found = lower.allPlayingWhileSounding(held, upper)
    # Each at its offset from where the note searched for stands.
    assert [(found.elementOffset(e), e.nameWithOctave) for e in found] == [
        (0.0, "E3"),
        (1.0, "F3"),
    ]
    # And they are the lower stream's own notes.
    assert list(found)[0] is list(lower)[2]

    found = lower.allPlayingWhileSounding(list(upper)[0], upper)
    assert [(found.elementOffset(e), e.nameWithOctave) for e in found] == [
        (0.0, "C3"),
        (1.0, "D3"),
    ]


def test_what_sounds_where_a_note_is_struck_is_found():
    """Every answer is music21's own for the same two streams."""
    upper = m.Stream()
    upper.insert(0.0, m.Note("C5", quarterLength=2.0))
    upper.insert(2.0, m.Note("D5", quarterLength=2.0))
    lower = m.Stream()
    lower.insert(0.0, m.Note("C3", quarterLength=3.0))
    lower.insert(3.0, m.Note("D3", quarterLength=1.0))

    # The long note below is sounding where each of the two above starts.
    assert lower.playingWhenAttacked(list(upper)[0], upper).nameWithOctave == "C3"
    assert lower.playingWhenAttacked(list(upper)[1], upper).nameWithOctave == "C3"
    assert lower.playingWhenAttacked(list(upper)[0], upper) is list(lower)[0]

    # Of several standing there the first of the searched element's class
    # wins, so the note rather than the clef.
    mixed = m.Stream()
    mixed.insert(0.0, m.TrebleClef())
    mixed.insert(0.0, m.Note("E3", quarterLength=1.0))
    assert mixed.playingWhenAttacked(list(upper)[0], upper).nameWithOctave == "E3"
    assert m.Stream().playingWhenAttacked(list(upper)[0], upper) is None


@pytest.mark.filterwarnings("ignore:Stream.hasElementOfClass was deprecated")
def test_a_class_filter_reads_what_music21_says_a_thing_is():
    """Every count is music21's own for the same stream.

    A class of this package's answers to its own name and to the classes
    it extends; what music21 says a thing *is* besides -- a `GeneralNote`,
    a `NotRest`, a `Music21Object` -- is a class nothing here is named
    after, and the crate's hierarchy is what answers for it.
    """
    line = m.Stream()
    for at, element in enumerate(
        (
            m.TrebleClef(),
            m.KeySignature(2),
            m.TimeSignature("4/4"),
            m.MetronomeMark(number=96),
            m.Dynamic("mf"),
            m.Note("C4"),
            m.Chord(["E4", "G4"]),
            m.Rest(quarterLength=1.0),
            m.ChordSymbol("C"),
        )
    ):
        line.insert(float(at), element)

    def count(name):
        return len(line.getElementsByClass(name).elements)

    # The leaves of music21's hierarchy, which the class names alone answer.
    assert (count("Note"), count("Rest"), count("Dynamic")) == (1, 1, 1)
    assert (count("TrebleClef"), count("MetronomeMark")) == (1, 1)
    # And the classes above them, which they do not.
    assert count("GeneralNote") == 4
    assert count("NotRest") == 3
    assert count("ChordBase") == 2
    assert count("Chord") == 2
    assert count("Harmony") == 1
    assert count("TempoIndication") == 1
    assert count("TimeSignatureBase") == 1
    assert count("Music21Object") == 9
    # A list of classes takes anything answering to one of them.
    assert len(line.getElementsByClass(["Clef", "NotRest"]).elements) == 4
    assert sorted(
        type(e).__name__ for e in line.getElementsNotOfClass("GeneralNote")
    ) == ["Dynamic", "KeySignature", "MetronomeMark", "TimeSignature", "TrebleClef"]
    assert line.hasElementOfClass("NotRest")
    assert not line.hasElementOfClass("Unpitched")

    # A stream is asked by its kind, and is a Music21Object like anything
    # else the stream holds.
    holder = m.Part()
    holder.insert(0.0, m.Measure())
    assert len(holder.getElementsByClass("Music21Object").elements) == 1
    assert len(holder.getElementsByClass("Stream").elements) == 1
    assert len(holder.getElementsByClass("Measure").elements) == 1
    assert len(holder.getElementsByClass("Voice").elements) == 0


def test_a_thing_says_what_it_is_as_music21_says_it():
    """Every chain is music21's own `classes` for the same object."""
    assert m.Note("C4").classes == [
        "Note",
        "NotRest",
        "GeneralNote",
        "Music21Object",
        "ProtoM21Object",
        "object",
    ]
    assert m.Rest().classes == [
        "Rest",
        "GeneralNote",
        "Music21Object",
        "ProtoM21Object",
        "object",
    ]
    assert m.Chord(["C4", "E4"]).classes[:3] == ["Chord", "ChordBase", "NotRest"]
    assert m.ChordSymbol("C").classes[:4] == [
        "ChordSymbol",
        "Harmony",
        "Chord",
        "ChordBase",
    ]
    # A clef of a kind, an instrument of a kind and a key carry the chains
    # music21 gives them, which the crate's lineage table says and
    # `class_lineage_parity` holds to music21.
    assert m.TrebleClef().classes[:4] == ["TrebleClef", "GClef", "PitchClef", "Clef"]
    assert m.Ocarina().classes[:4] == [
        "Ocarina",
        "Flute",
        "WoodwindInstrument",
        "Instrument",
    ]
    assert m.Key("G").classes[:5] == [
        "Key",
        "KeySignature",
        "DiatonicScale",
        "ConcreteScale",
        "Scale",
    ]
    # A stream is a `Generic` to music21, being typed by what it holds.
    assert m.Measure().classes == [
        "Measure",
        "Stream",
        "StreamCore",
        "Music21Object",
        "ProtoM21Object",
        "Generic",
        "object",
    ]
    # And what a thing says it is, a filter finds it by.
    line = m.Stream()
    line.insert(0.0, m.Key("G"))
    assert len(line.getElementsByClass("Scale").elements) == 1


def test_the_overlaps_in_a_stream_are_gathered_by_offset():
    """Every answer is music21's own for the same streams."""

    def shown(found):
        return {
            at: [e.nameWithOctave for e in held] for at, held in sorted(found.items())
        }

    line = m.Stream()
    line.insert(0.0, m.Note("C4", quarterLength=2.0))
    line.insert(1.0, m.Note("E4", quarterLength=1.0))
    line.insert(3.0, m.Note("G4", quarterLength=1.0))
    # The one overlapped is listed before the one that found it, which is
    # the order music21's own dictionary takes them in.
    assert shown(line.getOverlaps()) == {0.0: ["E4", "C4"]}

    none = m.Stream()
    for at in (0.0, 1.0, 2.0):
        none.insert(at, m.Note("C4", quarterLength=1.0))
    assert none.getOverlaps() == {}
    assert none.isSequence()

    # The stream is read flattened, so notes overlapping inside a measure
    # are found from the part, and they are the measure's own objects.
    held = m.Part()
    first = m.Measure()
    first.insert(0.0, m.Note("C4", quarterLength=4.0))
    first.insert(0.0, m.Note("E4", quarterLength=1.0))
    first.insert(2.0, m.Note("G4", quarterLength=1.0))
    held.insert(0.0, first)
    assert shown(held.getOverlaps()) == {0.0: ["E4", "G4", "C4"]}
    assert held.getOverlaps()[0.0][0] is list(first)[1]


def test_which_beat_of_which_measure_an_offset_falls_on():
    """Every answer is music21's own for the same part and score."""
    line = m.from_tiny_notation("tinyNotation: 3/4 c4 d4 e4 f4 g4 a4 b4 c'4 d'4")

    def asked(at):
        beat, measure = line.beatAndMeasureFromOffset(at)
        return (beat, measure.number)

    assert [asked(at) for at in (0.0, 1.0, 2.5)] == [(1.0, 1), (2.0, 1), (3.5, 1)]
    assert [asked(at) for at in (3.0, 4.0)] == [(1.0, 2), (2.0, 2)]
    assert [asked(at) for at in (6.0, 8.5)] == [(1.0, 3), (3.5, 3)]
    # Past the end music21 refuses, and so does this.
    with pytest.raises(m.StreamException):
        line.beatAndMeasureFromOffset(99.0)

    # A score is asked through the first of its parts that has measures,
    # and the measure handed back is that part's own object.
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4 g4 a4 b4 c'4")
    whole = m.Score()
    whole.insert(0.0, part)
    assert whole.beatAndMeasureFromOffset(2.0)[0] == 3.0
    assert whole.beatAndMeasureFromOffset(5.0)[1].number == 2
    assert whole.beatAndMeasureFromOffset(0.0)[1] is part.getElementsByClass("Measure")[0]


def test_whether_a_stream_is_at_sounding_pitch_is_said_as_music21_says_it():
    """Every answer, and the refusal, is music21's own."""
    part = m.Part()
    # Nothing has decided, which music21 calls 'unknown'.
    assert part.atSoundingPitch == "unknown"
    part.atSoundingPitch = True
    assert part.atSoundingPitch is True
    part.atSoundingPitch = False
    assert part.atSoundingPitch is False
    part.atSoundingPitch = "unknown"
    assert part.atSoundingPitch == "unknown"
    with pytest.raises(m.StreamException):
        part.atSoundingPitch = "junk"

    # A reader says it, and turning the part to sounding pitch says so.
    read = m.from_musicxml(MUSICXML)
    assert read.parts[0].atSoundingPitch is True
    written = read.parts[0].toWrittenPitch()
    assert written.atSoundingPitch is False


ARTICULATED = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN"
  "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Music</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note>
        <pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type>
        <notations>
          <articulations><staccato/><accent/></articulations>
          <ornaments><trill-mark/></ornaments>
        </notations>
        <lyric number="1"><syllabic>single</syllabic><text>sung</text></lyric>
      </note>
      <note>
        <pitch><step>D</step><octave>4</octave></pitch>
        <duration>3</duration><type>half</type><dot/>
      </note>
    </measure>
  </part>
</score-partwise>
"""


def test_a_note_is_inserted_into_what_already_stands_there():
    """Every answer is music21's own for the same streams."""

    def shown(line):
        return [
            (
                line.elementOffset(e),
                type(e).__name__,
                " ".join(p.nameWithOctave for p in getattr(e, "pitches", ())),
                e.quarterLength,
            )
            for e in line
        ]

    # A note where a note stands makes a chord as long as what stood there.
    line = m.Stream()
    line.insert(0.0, m.Note("C4", quarterLength=2.0))
    line.insertIntoNoteOrChord(0.0, m.Note("E4", quarterLength=1.0))
    assert shown(line) == [(0.0, "Chord", "C4 E4", 2.0)]

    # A chord into a note, and a note into a chord.
    line = m.Stream()
    line.insert(0.0, m.Note("C4", quarterLength=2.0))
    line.insertIntoNoteOrChord(0.0, m.Chord(["E4", "G4"], quarterLength=1.0))
    assert shown(line) == [(0.0, "Chord", "C4 E4 G4", 2.0)]
    line = m.Stream()
    line.insert(0.0, m.Chord(["C4", "E4"], quarterLength=2.0))
    line.insertIntoNoteOrChord(0.0, m.Note("G4", quarterLength=1.0))
    assert shown(line) == [(0.0, "Chord", "C4 E4 G4", 2.0)]

    # A rest gives way, and one pitch is a note unless chords are asked for.
    line = m.Stream()
    line.insert(0.0, m.Rest(quarterLength=2.0))
    line.insertIntoNoteOrChord(0.0, m.Note("G4", quarterLength=1.0))
    assert shown(line) == [(0.0, "Note", "G4", 2.0)]
    line = m.Stream()
    line.insert(0.0, m.Rest(quarterLength=2.0))
    line.insertIntoNoteOrChord(0.0, m.Note("G4", quarterLength=1.0), chordsOnly=True)
    assert shown(line) == [(0.0, "Chord", "G4", 2.0)]

    # Where nothing stands, the object handed in is the one held.
    line = m.Stream()
    given = m.Note("A4", quarterLength=1.0)
    line.insertIntoNoteOrChord(2.0, given)
    assert list(line)[0] is given

    # Two things starting in the span is what music21 refuses.
    line = m.Stream()
    line.insert(0.0, m.Note("C4", quarterLength=1.0))
    line.insert(0.5, m.Note("D4", quarterLength=1.0))
    with pytest.raises(m.StreamException):
        line.insertIntoNoteOrChord(0.0, m.Note("E4", quarterLength=2.0))

    # The merged chord keeps what stood there carried: its articulations
    # and its lyrics.
    line = m.Stream()
    held = m.Note("C4", quarterLength=2.0)
    held.articulations = [m.Staccato()]
    held.lyric = "sung"
    line.insert(0.0, held)
    line.insertIntoNoteOrChord(0.0, m.Note("E4", quarterLength=1.0))
    made = list(line)[0]
    assert [type(a).__name__ for a in made.articulations] == ["Staccato"]
    assert [ly.text for ly in made.lyrics] == ["sung"]


def test_a_reader_carries_the_articulations_and_ornaments_through():
    """music21 answers the same for the same document."""
    read = m.from_musicxml(ARTICULATED)
    notes = [e for e in read.recurse() if getattr(e, "isNote", False)]
    assert [type(a).__name__ for a in notes[0].articulations] == ["Staccato", "Accent"]
    assert [type(e).__name__ for e in notes[0].expressions] == ["Trill"]
    assert notes[1].articulations == []


REPEATED = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN"
  "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Music</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>2</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <barline location="left">
        <bar-style>heavy-light</bar-style>
        <repeat direction="forward"/>
      </barline>
      <note><pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
      <note><pitch><step>D</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
    </measure>
    <measure number="2">
      <note><pitch><step>E</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
      <note><pitch><step>F</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
      <barline location="right">
        <bar-style>light-heavy</bar-style>
        <repeat direction="backward"/>
      </barline>
    </measure>
  </part>
</score-partwise>
"""


def test_a_barline_says_what_music21s_says():
    """Every answer, and every refusal, is music21's own."""
    assert m.Barline().type == "regular"
    assert m.Barline().location is None
    # A name is read as music21's standardizeBarType reads it, MusicXML's
    # two spellings included.
    assert m.Barline("double").type == "double"
    assert m.Barline("light-heavy").type == "final"
    assert m.Barline("double").musicXMLBarStyle() == "light-light"
    assert m.Barline("final").musicXMLBarStyle() == "light-heavy"
    assert repr(m.Barline("double")) == "<music21.bar.Barline type=double>"
    with pytest.raises(m.BarException):
        m.Barline("junk")

    # A repeat is drawn as its direction says: heavy-light where a repeat
    # starts, final where it ends.
    one = m.Repeat()
    assert (one.direction, one.times, one.type) == ("start", None, "heavy-light")
    assert repr(one) == "<music21.bar.Repeat direction=start>"
    other = m.Repeat(direction="end", times=3)
    assert (other.direction, other.times, other.type) == ("end", 3, "final")
    assert repr(other) == "<music21.bar.Repeat direction=end times=3>"
    # Turning it round turns the drawing round, as music21's setter does.
    other.direction = "start"
    assert other.type == "heavy-light"
    with pytest.raises(m.BarException):
        m.Repeat(direction="sideways")
    # And a repeat is a RepeatMark and a Barline, in music21's order,
    # which is what a filter finds it by.
    assert m.Repeat().classes[:3] == ["Repeat", "RepeatMark", "Barline"]
    assert m.Barline().classes[:1] == ["Barline"]


def test_a_measure_keeps_its_two_barlines():
    """Every answer is music21's own for the same measure and part."""
    bar = m.Measure()
    bar.append(m.Note("C4", quarterLength=1.0))
    assert (bar.leftBarline, bar.rightBarline) == (None, None)
    bar.rightBarline = m.Barline("final")
    assert bar.rightBarline.type == "final"
    # A name is taken as the type, as music21's setter takes one.
    bar.rightBarline = "double"
    assert bar.rightBarline.type == "double"
    bar.rightBarline = None
    assert bar.rightBarline is None

    # finalBarline is the last measure's right barline, and a score
    # answers one for each of its parts.
    line = m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4 g4 a4 b4 c'4")
    assert line.finalBarline.type == "final"
    line.finalBarline = "double"
    assert line.finalBarline.type == "double"
    whole = m.Score()
    whole.insert(0.0, m.from_tiny_notation("tinyNotation: 4/4 c4 d4 e4 f4"))
    whole.insert(0.0, m.from_tiny_notation("tinyNotation: 4/4 C4 D4 E4 F4"))
    whole.finalBarline = "double"
    assert [b.type for b in whole.finalBarline] == ["double", "double"]


def test_repeats_are_read_and_played_out():
    """Every answer is music21's own for the same document."""
    read = m.from_musicxml(REPEATED)
    part = read.parts[0]
    bars = list(part.getElementsByClass("Measure"))
    assert [b.number for b in bars] == [1, 2]
    assert bars[0].leftBarline.direction == "start"
    assert bars[0].rightBarline is None
    assert bars[1].rightBarline.direction == "end"

    played = part.expandRepeats()
    assert [
        (measure.number, measure.numberSuffix, [n.nameWithOctave for n in measure.notes])
        for measure in played.getElementsByClass("Measure")
    ] == [
        (1, None, ["C4", "D4"]),
        (2, None, ["E4", "F4"]),
        (1, "a", ["C4", "D4"]),
        (2, "a", ["E4", "F4"]),
    ]


WORDED = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN"
  "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Music</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><key><fifths>0</fifths></key>
        <time><beats>2</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef></attributes>
      <direction placement="above"><direction-type><words>dolce</words></direction-type></direction>
      <note><pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
      <note><pitch><step>D</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"""


def test_words_are_read_and_carried_through():
    """music21 answers the same for the same document."""
    read = m.from_musicxml(WORDED)
    words = [e for e in read.recurse() if isinstance(e, m.TextExpression)]
    assert [(w.content, w.placement) for w in words] == [("dolce", "above")]
    # What music21 says it is, and how it writes it.
    assert words[0].classes[:2] == ["TextExpression", "Expression"]
    assert repr(m.TextExpression("dolce")) == (
        "<music21.expressions.TextExpression 'dolce'>"
    )
    # Long words are cut short in a repr, as music21 cuts them.
    assert repr(m.TextExpression("repeat 3 times")) == (
        "<music21.expressions.TextExpression 'repeat 3 t...'>"
    )

    # A repeat says its count in words: music21's getTextExpression.
    said = m.Repeat(direction="end", times=3).getTextExpression()
    assert said.content == "3x"
    assert m.Repeat(direction="end", times=3).getTextExpression(
        prefix="repeat ", postfix=" times"
    ).content == "repeat 3 times"
    # A repeat with no count has nothing to say; music21 writes the word
    # None into the text there, which is not worth reproducing.
    assert m.Repeat(direction="end").getTextExpression() is None


SLURRED = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN"
  "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <part-list><score-part id="P1"><part-name>Music</part-name></score-part></part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>1</divisions><key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef></attributes>
      <note><pitch><step>C</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type>
        <notations><slur number="1" placement="above" type="start"/></notations></note>
      <note><pitch><step>D</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
      <note><pitch><step>E</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type>
        <notations><slur number="1" type="stop"/></notations></note>
      <note><pitch><step>F</step><octave>4</octave></pitch>
        <duration>1</duration><type>quarter</type></note>
    </measure>
  </part>
</score-partwise>
"""


def test_a_spanner_joins_the_notes_a_stream_holds():
    """Every answer is music21's own for the same document and stream."""
    # A spanner joins this package's own notes whether or not music21 is
    # there to be imported. It keeps its objects in music21's own
    # `SpannerStorage` where music21 can be asked for one, and a stream of
    # music21's holds nothing that is not a `Music21Object` -- so a note of
    # this package's, which is one only where something is installed over
    # music21, was refused and the slur could not be built at all. The
    # wheel was worse for having music21 beside it than without, which is
    # the one thing it must never be.
    joined = m.Slur(m.Note("C4"), m.Note("E4"))
    assert [held.nameWithOctave for held in joined.getSpannedElements()] == ["C4", "E4"]

    read = m.from_musicxml(SLURRED)
    # Asked of the whole tree, which is how music21's own code asks: the
    # crate's readers gather a part's spanners onto the score, where
    # music21's reader leaves them on the part, so `spanners` differs of
    # the two while the bundle does not.
    found = list(read.spannerBundle)
    assert [type(s).__name__ for s in found] == ["Slur"]
    slur = found[0]
    joined = slur.getSpannedElements()
    assert [n.nameWithOctave for n in joined] == ["C4", "E4"]
    # The notes it joins are the score's own.
    notes = [e for e in read.recurse() if getattr(e, "isNote", False)]
    assert joined[0] is notes[0]
    assert (slur.getFirst(), slur.getLast()) == (joined[0], joined[1])
    assert len(slur) == 2
    assert slur.placement == "above"
    assert slur.classes[:2] == ["Slur", "Spanner"]
    assert slur.hasSpannedElement(notes[0])
    assert not slur.hasSpannedElement(notes[1])

    # A slur built here reaches the writer, which is what one is for.
    bar = m.Measure()
    bar.insert(0.0, m.TimeSignature("4/4"))
    notes = [m.Note(name, quarterLength=1.0) for name in ("C4", "D4", "E4", "F4")]
    for at, note in enumerate(notes):
        bar.insert(float(at), note)
    part = m.Part()
    part.insert(0.0, bar)
    part.insert(0.0, m.Slur(notes[0], notes[2]))
    whole = m.Score()
    whole.insert(0.0, part)
    assert len(whole.spannerBundle) == 1
    # Two tags, a start and a stop, which is music21's count for the same
    # score.
    assert m.to_musicxml(whole).count("<slur") == 2

    # The bundle answers getByClass, as music21's does.
    assert len(whole.spannerBundle.getByClass("Slur")) == 1
    assert len(whole.spannerBundle.getByClass("Crescendo")) == 0
    # And every kind music21 has a class for has one here.
    for kind in (
        m.Slur,
        m.Crescendo,
        m.Diminuendo,
        m.Ottava,
        m.Line,
        m.Glissando,
        m.TrillExtension,
        m.TremoloSpanner,
        m.ArpeggioMarkSpanner,
        m.PedalMark,
        m.Neume,
    ):
        one = kind(notes[0], notes[1])
        assert len(one) == 2
        assert one.classes[-4:] == [
            "Spanner",
            "Music21Object",
            "ProtoM21Object",
            "object",
        ]
    # A hairpin is a DynamicWedge to music21 before it is a Spanner.
    assert m.Crescendo().classes[:3] == ["Crescendo", "DynamicWedge", "Spanner"]


def test_a_slur_is_read_from_abc_as_music21_reads_it():
    """music21 answers the same for the same tune."""
    read = m.from_abc("X:1\nT:Slurred\nM:4/4\nL:1/4\nK:C\n(CDE) F |]\n")
    found = list(read.spannerBundle)
    assert [type(s).__name__ for s in found] == ["Slur"]
    assert [n.nameWithOctave for n in found[0].getSpannedElements()] == [
        "C4",
        "D4",
        "E4",
    ]


def test_a_measure_answers_for_its_bar():
    """Every answer is music21's own for the same measure."""
    bar = m.Measure()
    bar.number = 14
    bar.numberSuffix = "a"
    bar.insert(0.0, m.TimeSignature("3/4"))
    bar.insert(0.0, m.Note("C4", quarterLength=1.0))

    assert bar.measureNumberWithSuffix() == "14a"
    assert bar.barDuration.quarterLength == 3.0
    # A third of its bar. music21 answers the Fraction 1/3 where this
    # answers the float, as it does for every length and offset.
    assert bar.barDurationProportion() == pytest.approx(1 / 3)
    # The meter its one quarter note best fits.
    assert bar.bestTimeSignature().ratioString == "1/4"

    # A pickup is padded on the left by what it lacks of its bar.
    pickup = m.Measure()
    pickup.insert(0.0, m.TimeSignature("4/4"))
    pickup.insert(0.0, m.Note("C4", quarterLength=1.0))
    padded = pickup.padAsAnacrusis()
    assert padded.paddingLeft == 3.0
    # music21 changes the measure it is given; the answer here is a new
    # one, as every other edit in this package is.
    assert pickup.paddingLeft == 0.0

    # With useInitialRests the rests it starts with go, and what follows
    # moves back, which is music21's own answer for the same measure.
    resty = m.Measure()
    resty.insert(0.0, m.TimeSignature("4/4"))
    resty.insert(0.0, m.Rest(quarterLength=2.0))
    resty.insert(2.0, m.Note("C4", quarterLength=1.0))
    used = resty.padAsAnacrusis(useInitialRests=True)
    assert used.paddingLeft == 3.0
    assert [type(e).__name__ for e in used] == ["TimeSignature", "Note"]


def test_a_measure_is_numbered_and_padded_as_music21_does():
    """Every answer is music21's own for the same measure."""
    bar = m.Measure()
    # What music21's own `Measure.__init__` gives every measure.
    assert (bar.number, bar.numberSuffix) == (0, None)
    assert (bar.paddingLeft, bar.paddingRight) == (0.0, 0.0)

    # The constructor reads a name as music21 reads one.
    assert (m.Measure(number="12b").number, m.Measure(number="12b").numberSuffix) == (
        12,
        "b",
    )
    assert (m.Measure(number=7).number, m.Measure(number=7).numberSuffix) == (7, None)

    # A number leaves the suffix as it stands, as music21's attribute
    # does. Assigning a *name* splits it here, where music21 would keep
    # the whole string as the number -- the number is a number here.
    bar.number = 14
    bar.numberSuffix = "a"
    assert bar.measureNumberWithSuffix() == "14a"
    bar.number = 15
    assert (bar.number, bar.numberSuffix) == (15, "a")
    bar.number = "16c"
    assert (bar.number, bar.numberSuffix) == (16, "c")


def test_instruments_rearrange_a_stream_as_music21s_do():
    """Every answer is music21's own for the same part."""
    line = m.Part()
    line.insert(0.0, m.Flute())
    line.insert(0.0, m.Note("C4", quarterLength=1.0))
    line.insert(1.0, m.Note("D4", quarterLength=1.0))
    line.insert(2.0, m.Violin())
    line.insert(2.0, m.Note("E4", quarterLength=1.0))
    line.insert(3.0, m.Note("F4", quarterLength=1.0))

    # A part per instrument, each holding what that instrument plays.
    score = m.partitionByInstrument(line)
    assert [
        (
            [type(i).__name__ for i in part.recurse() if hasattr(i, "instrumentName")],
            [n.nameWithOctave for n in part.recurse() if getattr(n, "isNote", False)],
        )
        for part in score
    ] == [(["Flute"], ["C4", "D4"]), (["Violin"], ["E4", "F4"])]

    # Bundling puts the instrument onto the notes that sound it, so
    # nothing but notes is left standing in the stream.
    bundled = m.bundleInstruments(line)
    assert [type(e).__name__ for e in bundled] == ["Note"] * 4

    # Unbundling takes each note's own instrument back out beside it.
    stored = m.Part()
    for at, (name, kind) in enumerate((("C4", m.Flute), ("D4", m.Violin))):
        one = m.Note(name, quarterLength=1.0)
        one.storedInstrument = kind()
        stored.insert(float(at), one)
    assert [type(e).__name__ for e in m.unbundleInstruments(stored)] == [
        "Flute",
        "Note",
        "Violin",
        "Note",
    ]

    # Two instruments of a kind at one offset settle into one.
    twice = m.Part()
    twice.insert(0.0, m.Flute())
    twice.insert(0.0, m.Flute())
    twice.insert(0.0, m.Note("C4", quarterLength=1.0))
    assert [type(e).__name__ for e in m.deduplicate(twice)] == ["Flute", "Note"]

    # Each answers a new stream, as every edit in this package does, so
    # music21's inPlace is refused rather than answered differently.
    with pytest.raises(m.StreamException):
        m.deduplicate(twice, inPlace=True)


def test_a_line_is_measured_as_music21_measures_it():
    """Every answer is music21's own for the same stream."""
    line = m.Stream()
    at = 0.0
    for name, length in (
        ("C4", 1.0),
        ("D4", 0.5),
        ("E4", 0.5),
        ("F4", 2.0),
        ("G4", 1.0),
        ("A4", 1.0),
        ("B4", 2.0),
    ):
        line.insert(at, m.Note(name, quarterLength=length))
        at += length

    # To nine places, which is as far as music21's own answer goes.
    assert round(m.nPVI(line), 9) == 53.333333333
    assert round(m.melodicIntervalVariability(line), 9) == 22.268088571
    # The keywords are findConsecutiveNotes', which music21 passes on.
    assert round(m.melodicIntervalVariability(line, skipUnisons=True), 9) == (
        22.268088571
    )

    # nPVI counts every element, clefs and the final barline among them,
    # as music21's docstring says: 3.2 over nine elements.
    part = m.from_tiny_notation("tinyNotation: 4/4 c4 d8 e8 f2 g4 a4 b2")
    assert round(m.nPVI(part.flatten()), 6) == 35.555556

    # A lyric under each note saying how deep in the beat it falls.
    labelled = m.labelBeatDepth(
        m.from_tiny_notation("tinyNotation: 4/4 c8 d8 e8 f8 g8 a8 b8 c'8")
    )
    assert [
        "".join(ly.text for ly in n.lyrics)
        for n in labelled.recurse()
        if getattr(n, "isNote", False)
    ] == ["****", "*", "**", "*", "***", "*", "**", "*"]

    # Thomassen's melodic accent goes onto each note's editorial, which is
    # where music21 writes it, and answers nothing.
    notes = [m.Note(name, quarterLength=1.0) for name in ("C4", "E4", "G4", "C5")]
    holder = m.Stream()
    for at, note in enumerate(notes):
        holder.insert(float(at), note)
    assert m.thomassenMelodicAccent(holder) is None
    assert [round(n.editorial.melodicAccent, 6) for n in notes] == [
        1.0,
        0.33,
        0.2211,
        0.67,
    ]


def test_a_notes_editorial_holds_what_an_analysis_found():
    """music21's own `Editorial` behaves this way."""
    note = m.Note("C4")
    # The same object every time, so what is written on it stays.
    assert note.editorial is note.editorial
    note.editorial.melodicAccent = 0.5
    assert note.editorial.melodicAccent == 0.5
    assert "melodicAccent" in note.editorial
    assert note.editorial["melodicAccent"] == 0.5

    # Three names answer None before anything writes them and two answer
    # an empty list, which is music21's own predefined set.
    assert m.Editorial().harmonicInterval is None
    assert m.Editorial().melodicInterval is None
    assert m.Editorial().ficta is None
    assert m.Editorial().footnotes == []
    assert m.Editorial().comments == []
    # Anything else raises until written, as music21's does.
    with pytest.raises(AttributeError):
        m.Editorial().nonsense


def test_the_interval_against_another_part_is_written_on_each_note():
    """Every answer is music21's own for the same two parts."""
    upper = m.from_tiny_notation("tinyNotation: 4/4 e'2 d'2 c'2 b2").flatten()
    lower = m.from_tiny_notation("tinyNotation: 4/4 c1 g1").flatten()
    assert upper.attachIntervalsBetweenStreams(lower) is None

    notes = [n for n in upper if getattr(n, "isNote", False)]
    assert [
        (n.nameWithOctave, n.editorial.harmonicInterval.name) for n in notes
    ] == [("E5", "M10"), ("D5", "M9"), ("C5", "P4"), ("B4", "M3")]

    # Asked again against a part of rests, every note's answer is cleared
    # rather than kept, which is what music21 does first thing.
    rests = m.from_tiny_notation("tinyNotation: 4/4 r1 r1").flatten()
    upper.attachIntervalsBetweenStreams(rests)
    assert [n.editorial.harmonicInterval for n in notes] == [None] * 4


def test_a_part_is_named_as_music21_names_it():
    """Every answer is music21's own for the same part."""
    bare = m.Part()
    assert (bare.partName, bare.partAbbreviation) == (None, None)

    named = m.Part()
    named.partName = "Flauto"
    assert named.partName == "Flauto"

    # Unsaid, a part answers for the first instrument it holds: its own
    # partName, else its instrumentName, and the abbreviation likewise.
    played = m.Part()
    played.insert(0.0, m.Flute())
    assert (played.partName, played.partAbbreviation) == ("Flute", "Fl")

    # music21 looks into the substreams too, a part's instrument often
    # standing in its first measure.
    nested = m.Part()
    bar = m.Measure()
    bar.insert(0.0, m.Flute())
    bar.insert(0.0, m.Note("C4", quarterLength=1.0))
    nested.insert(0.0, bar)
    assert nested.partName == "Flute"


def test_a_line_is_cut_into_its_runs_of_notes():
    """Every answer is music21's own for the same line."""
    line = m.from_tiny_notation("4/4 C4 r D E r r F r G r A B r c")
    runs = m.Segmenter.getSegmentsList(line)
    assert [[n.nameWithOctave for n in run] for run in runs] == [
        ["C3"],
        ["D3", "E3"],
        ["F3"],
        ["G3"],
        ["A3", "B3"],
        ["C4"],
    ]

    # A rest standing after another gives an empty run, as music21's does.
    kept = m.Segmenter.getSegmentsList(line, removeEmptyLists=False)
    assert [len(run) for run in kept] == [0, 1, 2, 0, 1, 1, 2, 1]

    # A run is the stream's own notes, and an interval is built on two of
    # them, so it keeps the very notes it was measured between.
    notes = [n for n in line.recurse() if getattr(n, "isNote", False)]
    assert runs[1][0] is notes[1]
    intervals = m.Segmenter.getIntervalList(line)
    assert [i.name for i in intervals] == ["M2", "M2"]
    assert intervals[0].noteStart is notes[1]
    assert intervals[0].noteEnd is notes[2]


def test_a_property_of_every_pitch_is_counted():
    """Every answer is music21's own for the same line."""
    line = m.from_tiny_notation("4/4 c4 d c e f# g a b-")
    assert sorted(m.pitchAttributeCount(line).items()) == [
        ("A", 1),
        ("B-", 1),
        ("C", 2),
        ("D", 1),
        ("E", 1),
        ("F#", 1),
        ("G", 1),
    ]
    assert sorted(m.pitchAttributeCount(line, "pitchClass").items()) == [
        (0, 2),
        (2, 1),
        (4, 1),
        (6, 1),
        (7, 1),
        (9, 1),
        (10, 1),
    ]
    # A Counter, as music21 answers, so ties order by what was met first.
    counted = m.pitchAttributeCount(m.from_tiny_notation("4/4 c4 c d d d e"))
    assert counted.most_common(2) == [("D", 3), ("C", 2)]


def test_a_chord_diagram_says_what_it_sounds():
    """Every answer is music21's own for the same diagram."""
    board = m.GuitarFretBoard(
        [m.FretNote(6, 3, 3), m.FretNote(5, 2, 2), m.FretNote(4, 0)]
    )
    assert repr(board) == "<music21.tablature.GuitarFretBoard 6 strings, 3 notes, 4 frets>"
    assert board.numStrings == 6
    assert [n.string for n in board.fretNotesLowestFirst()] == [6, 5, 4]
    assert board.getFretNoteByString(5).fret == 2
    assert board.getFretNoteByString(1) is None
    # Each string's open pitch raised by the fret stopping it, and nothing
    # for a string nobody stops.
    assert [p.nameWithOctave if p else None for p in board.getPitches()] == [
        "G2",
        "B2",
        "D3",
        None,
        None,
        None,
    ]
    assert repr(m.FretNote()) == "<music21.tablature.FretNote>"
    assert repr(m.FretNote(3, 2, 1)) == "<music21.tablature.FretNote 3rd string, 2nd fret, 1st finger>"


def test_words_are_read_off_a_score_and_stood_on_its_page():
    """Every answer is music21's own for the same words."""
    line = m.from_tiny_notation("4/4 c4_hel-   d_lo e_there f_now")
    assert m.assembleLyrics(line) == "hello there now"
    assert m.assembleAllLyrics(line) == "hello there now"
    assert m.prependArticle("Ale is Dear, The") == "The Ale is Dear"
    assert m.postpendArticle("The Ale is Dear") == "Ale is Dear, The"
    # A text box stands where it is told and keeps the style it is drawn
    # with, so writing into that style is writing into the box's.
    box = m.TextBox("Hello there, this is long", 250, 1000)
    assert repr(box) == '<music21.text.TextBox "Hello ther...">'
    assert (box.page, box.style.absoluteX, box.style.absoluteY) == (1, 250, 1000)
    assert (box.style.alignVertical, box.style.alignHorizontal) == ("top", "center")
    box.style.fontSize = 40
    assert box.style.fontSize == 40
    assert box.hasStyleInformation
    box.page = 3
    assert box.page == 3
    try:
        box.page = 0
    except Exception as error:  # noqa: BLE001 - the class is music21's
        assert "positive integer" in str(error)
    else:
        raise AssertionError("page zero was allowed")


def test_a_repeat_mark_says_what_it_is_written_with():
    """Every answer is music21's own for the same marks."""
    import copy

    segno = m.Segno()
    assert (segno.getText(), segno.useSymbol) == ("Segno", True)
    coda = m.Coda()
    assert (coda.getText(), coda.useSymbol) == ("Coda", True)
    # A coda given words it may be written with is written, not drawn.
    told = m.Coda("to Coda")
    assert (told.getText(), told.useSymbol) == ("to Coda", False)
    # And words it may not are passed over, as music21 passes them over.
    other = m.Coda("D.C. al fine")
    assert (other.getText(), other.useSymbol) == ("Coda", True)
    assert m.DaCapoAlFine().getText() == "Da Capo al fine"
    assert m.DalSegnoAlCoda().isValidText("d.s. al coda")
    assert not m.DalSegnoAlCoda().isValidText("Fine")
    # What music21 says each is, down to `Music21Object`.
    assert m.DaCapo().classes[:3] == ["DaCapo", "RepeatExpressionCommand", "RepeatExpression"]
    assert m.Segno().classes[1] == "RepeatExpressionMarker"
    copied = copy.deepcopy(told)
    assert (copied.getText(), copied.useSymbol) == ("to Coda", False)


def test_a_barline_is_copied_and_frozen():
    """Copying one rebuilt it by calling the class with the object itself,
    which is what Python's `copyreg` does for a class it knows nothing
    else about, and a barline read that as the type it is drawn with."""
    import copy
    import pickle

    line = m.Repeat(direction="end", times=3)
    copied = copy.deepcopy(line)
    assert copied is not line
    assert (copied.direction, copied.times, copied.type) == ("end", 3, "final")
    thawed = pickle.loads(pickle.dumps(line))
    assert (thawed.direction, thawed.times) == ("end", 3)
    # music21 refuses a count on a repeat that starts a passage, and one
    # below nought, each in its own words.
    start = m.Repeat(direction="start")
    for value, said in ((3, "start Repeat"), (-3, "less than zero")):
        try:
            start.times = value
        except Exception as error:  # noqa: BLE001 - the class is music21's
            assert said in str(error)
        else:
            raise AssertionError(f"times = {value} was allowed")


def test_a_pitch_is_read_from_its_name_in_another_language():
    """Every answer is music21's own for the same names."""
    assert m.toPitch("Es", "de").name == "E-"
    assert m.toPitch("H", "de").name == "B"
    assert [m.toPitch(name, "de").name for name in ("As", "A", "Ais")] == ["A-", "A", "A#"]
    assert m.toPitch("si", "it").name == "B"
    assert m.toPitch("ré", "fr").name == "D"
    # A name the language calls nothing is a C, as music21 answers.
    assert m.toPitch("Es", "it").name == "C"
    assert m.toPitch("Es", "la").name == "C"
    # The note is built on the pitch read, which carries no octave, as
    # music21's does: `<music21.note.Note E->`.
    assert m.toNote("Es", "de").nameWithOctave == "E-"
    assert [p.name for p in m.toChord(["Es", "Gis", "B"], "de").pitches] == ["E-", "G#", "B-"]
    # The dictionary music21 builds by writing every name out.
    german = m.generateLanguageDictionary("de")
    assert len(german) == 63
    assert german["H"] == "B"
    assert m.generateLanguageDictionary("la") == {}


def test_a_walk_answers_what_music21_s_walk_answers():
    """music21's `recurse` answers a StreamIterator, which its own code
    filters again; a list could not be filtered and a stream would sort
    what the walk met in order."""
    score = m.from_tiny_notation("4/4 c4 d8 e8 f2 g1")
    walked = score.recurse()
    assert type(walked).__name__ == "StreamIterator"
    # The walk's order: each measure before what it holds.
    assert [type(e).__name__ for e in walked][:3] == ["Measure", "TrebleClef", "TimeSignature"]
    assert len(walked) == len([e for e in walked])
    assert walked[0] is walked.first()
    assert walked[-1] is walked.last()
    # A stretch of a walk is a list of what it found, as music21's
    # iterator answers and as music21's own search docstrings ask for.
    notes = walked.notes
    assert [n.nameWithOctave for n in notes[1:3]] == ["D4", "E4"]
    assert [n.nameWithOctave for n in notes[::2]] == ["C4", "E4", "G4"]
    assert notes[100:] == []
    # Filtered again, as music21's own code filters it.
    assert [n.nameWithOctave for n in walked.notes] == ["C4", "D4", "E4", "F4", "G4"]
    assert len(walked.notesAndRests) == 5
    assert len(walked.getElementsByClass("Measure")) == 2
    assert len(walked.getElementsNotOfClass("Measure")) == len(walked) - 2
    # Gathered back into a stream, each where it stood.
    gathered = walked.notes.stream()
    assert [gathered.elementOffset(n) for n in gathered] == [0.0, 1.0, 1.5, 2.0, 4.0]


def test_a_property_of_every_element_is_counted():
    """Every answer music21 gives for the same line, as far as the wheel's
    elements carry the property: a clef or a meter here has no length of
    its own, where music21's is counted as a nought."""
    line = m.from_tiny_notation("4/4 c4 d8 e8 f2")
    measure = line.getElementsByClass("Measure")[0]
    assert sorted(m.attributeCount(measure).items()) == [(0.5, 2), (1.0, 1), (2.0, 1)]
    # A clef answers to a name as well as a note does, as music21's does.
    assert sorted(m.attributeCount(measure, "name").items()) == [
        ("C", 1),
        ("D", 1),
        ("E", 1),
        ("F", 1),
        ("treble", 1),
    ]
    # A Counter, as music21 answers, so ties order by what was met first.
    counted = m.attributeCount(m.from_tiny_notation("4/4 c4 c4 d4 d4").flatten(), "name")
    assert counted.most_common(2) == [("C", 2), ("D", 2)]


def test_an_alteration_is_written_as_a_chord_symbol_writes_it():
    """Every answer is music21's own."""
    assert [m.alterToChordSymbolString(n) for n in (-2, -1, 0, 1, 2)] == [
        "bb",
        "b",
        "",
        "#",
        "##",
    ]


def test_a_key_is_read_for_every_measure():
    """Every answer is music21's own for the same line."""
    assert [m.divide(4.0, -1), m.divide(3.0, 0), m.divide(6.0, 2)] == [2.0, 3.0, 2.0]

    with pytest.raises(m.FloatingKeyException):
        m.KeyAnalyzer()
    bare = m.Part()
    bare.insert(0.0, m.Note("C4"))
    with pytest.raises(m.FloatingKeyException):
        m.KeyAnalyzer(bare)

    line = m.from_tiny_notation(
        "4/4 c4 e g c' c e g c' g b d' g d f# a d' e g# b e' a c# e a'"
    )
    analyzer = m.KeyAnalyzer(line)
    assert (analyzer.numMeasures, analyzer.windowSize) == (6, 4)
    assert analyzer.stream is line

    # Each measure on its own, by the Aarden-Essen weighting music21's
    # analyze('key') reads with, each key carrying how well it fitted and
    # the twenty-three that fitted less well.
    raw = analyzer.getRawKeyByMeasure()
    assert [str(k) for k in raw] == [
        "C major",
        "C major",
        "G major",
        "D major",
        "E major",
        "A major",
    ]
    assert len(raw[0].alternateInterpretations) == 23
    assert round(raw[0].correlationCoefficient, 6) == 0.71926
    assert len(analyzer.rawKeyByMeasure) == 6

    # The readings of one measure, spelled as music21 spells a key.
    readings = analyzer.getInterpretationByMeasure(0)
    assert sorted((n, round(v, 6)) for n, v in readings.items())[:4] == [
        ("A", -0.174943),
        ("A-", 0.18564),
        ("B", -0.351549),
        ("B-", 0.081055),
    ]

    # Smoothed by the measures around each, through the window said.
    assert [str(k) for k in analyzer.run()] == [
        "C major",
        "C major",
        "C major",
        "G major",
        "a minor",
        "A major",
    ]
    analyzer.windowSize = 1
    assert [str(k) for k in analyzer.run()] == [
        "C major",
        "C major",
        "G major",
        "D major",
        "A major",
        "A major",
    ]

    # The weighting is a function, as music21's is, so a caller may weigh
    # the window however it likes.
    analyzer.windowSize = 2
    analyzer.weightAlgorithm = lambda coefficient, distance: coefficient / (
        (abs(distance) + 1) ** 2
    )
    assert [str(k) for k in analyzer.run()] == [
        "C major",
        "C major",
        "G major",
        "D major",
        "E major",
        "A major",
    ]

    # A measure with no notes has no reading and no key of its own, so the
    # keys answered are one short, as music21's are.
    quiet = m.from_tiny_notation("4/4 c4 e g c' r1 d f a d'")
    hushed = m.KeyAnalyzer(quiet)
    hushed.windowSize = 1
    assert [None if k is None else str(k) for k in hushed.getRawKeyByMeasure()] == [
        "C major",
        None,
        "G major",
        "B- major",
        "D major",
        "G major",
    ]
    assert hushed.getInterpretationByMeasure(1) is None
    assert [str(k) for k in hushed.run()] == [
        "C major",
        "B- major",
        "d minor",
        "d minor",
        "d minor",
    ]


def test_the_span_of_what_a_stream_sounds():
    """Every answer is music21's own for the same stream."""
    ambitus = m.Ambitus()
    assert (ambitus.name, list(ambitus.identifiers)) == (
        "Ambitus Analysis",
        ["ambitus", "span"],
    )

    held = m.Chord(["a2", "b4", "c8"])
    stream = m.Stream()
    stream.append(held)
    low, high = ambitus.getPitchSpan(stream)
    assert (str(low), str(high)) == ("A2", "C8")
    # The stream's own pitch objects, not copies of them.
    assert low is held.pitches[0] and high is held.pitches[2]

    span = ambitus.getSolution(stream)
    assert span.name == "m38"
    assert span.pitchStart is held.pitches[0]

    # Nothing where the stream sounds nothing, and a chord symbol is
    # passed over, as music21 passes it over.
    quiet = m.Stream()
    quiet.append(m.Rest())
    assert ambitus.getPitchSpan(quiet) is None
    assert ambitus.getSolution(quiet) is None
    symbols = m.Stream()
    symbols.append(m.ChordSymbol("C6"))
    assert ambitus.getPitchSpan(symbols) is None

    # `process` answers a solution and a colour upstream, and the colour is
    # a keyscape's; the package leaves it out rather than answer half.
    assert not hasattr(ambitus, "process")


def test_the_key_a_stream_is_most_likely_in():
    """Every answer is music21's own for the same line."""
    line = m.from_tiny_notation("4/4 c4 e g c' c e g c' g b d' g")

    weighings = {
        "KeyWeightKeyAnalysis": ("KeyWeightKeyAnalysis Base Class", 0.873542),
        "KrumhanslSchmuckler": (
            "Krumhansl Schmuckler/Kessler Key Analysis",
            0.873542,
        ),
        "AardenEssen": ("Aarden Essen Key Analysis", 0.821518),
        "SimpleWeights": ("Simple Weight Key Analysis", 0.862911),
        "BellmanBudge": ("Bellman Budge Key Analysis", 0.836727),
        "TemperleyKostkaPayne": ("Temperley Kostka Payne Key Analysis", 0.843534),
    }
    for kind, (name, coefficient) in weighings.items():
        weigher = getattr(m, kind)()
        assert weigher.name == name
        assert len(weigher.getWeights("major")) == 12
        assert weigher.getWeights("MAJOR") == weigher.getWeights("major")
        with pytest.raises(m.DiscreteAnalysisException):
            weigher.getWeights("lydian")
        found = weigher.getSolution(line)
        assert str(found) == "C major"
        assert round(found.correlationCoefficient, 6) == coefficient
        assert len(found.alternateInterpretations) == 23
        # The base class weighs as Krumhansl-Schmuckler does, as music21's
        # does, and the subclasses by their own profile.
        assert not hasattr(weigher, "process")

    # The weights themselves are music21's, Krumhansl-Schmuckler for the
    # base class.
    assert m.KeyWeightKeyAnalysis().getWeights("major") == [
        6.35,
        2.23,
        3.48,
        2.33,
        4.38,
        4.09,
        2.52,
        5.19,
        2.39,
        3.66,
        2.29,
        2.88,
    ]
    assert m.SimpleWeights().getWeights("minor")[:3] == [2.0, 0.0, 1.0]
    assert [str(k) for k in m.AardenEssen().getSolution(line).alternateInterpretations[:3]] == [
        "G major",
        "c minor",
        "a minor",
    ]

    nothing = m.Stream()
    nothing.append(m.Rest())
    with pytest.raises(m.DiscreteAnalysisException):
        m.KrumhanslSchmuckler().getSolution(nothing)


def test_how_many_melodic_intervals_a_stream_uses():
    """Every count is music21's own for the same line."""
    diversity = m.MelodicIntervalDiversity()
    assert (diversity.name, list(diversity.identifiers)) == (
        "Interval Diversity",
        ["interval.diversity", "intervals"],
    )
    line = m.from_tiny_notation("4/4 c4 e g c' c e g c' g b d' g")
    found = diversity.countMelodicIntervals(line)
    assert sorted((k, v[1]) for k, v in found.items()) == [
        ("M3", 3),
        ("P4", 3),
        ("P5", 1),
        ("P8", 1),
        ("m3", 3),
    ]
    # An interval is built on the notes the stream holds, as music21 builds
    # it, so the count carries the score's own notes.
    notes = [n for n in line.recurse() if getattr(n, "isNote", False)]
    assert found["M3"][0].noteStart is notes[0]

    # Direction counts where music21 counts it, and a unison is passed over
    # unless it is asked for.
    assert sorted(
        (k, v[1]) for k, v in diversity.countMelodicIntervals(line, ignoreDirection=False).items()
    ) == [("M3", 3), ("P-4", 1), ("P-5", 1), ("P-8", 1), ("P4", 2), ("m3", 3)]
    unisons = diversity.countMelodicIntervals(
        m.from_tiny_notation("4/4 c4 c d d e e"), ignoreUnison=False
    )
    assert sorted((k, v[1]) for k, v in unisons.items()) == [("M2", 2), ("P1", 3)]

    # A dictionary given is counted into and handed back.
    again = diversity.countMelodicIntervals(line, found=dict(found))
    assert sorted((k, v[1]) for k, v in again.items()) == [
        ("M3", 6),
        ("P4", 6),
        ("P5", 2),
        ("P8", 2),
        ("m3", 6),
    ]

    # music21's own `process` passes `ignoreDirection` where `found` goes,
    # so it raises for any line of two notes; this does what its docstring
    # says, and its colour is the one white string music21 answers.
    assert diversity.process(line) == (5, "#ffffff")
    assert diversity.getSolution(line) == 5
    assert diversity.solutionToColor(5) == "#ffffff"


def outline_of(score):
    """Every part of a score, and in each the elements measure by measure."""
    out = []
    for part in score.getElementsByClass("Part"):
        written = []
        for measure in part.getElementsByClass("Measure"):
            held = []
            for element in measure:
                kind = type(element).__name__
                if getattr(element, "pitches", None):
                    names = "+".join(p.nameWithOctave for p in element.pitches)
                    held.append(f"{kind}({names},{element.quarterLength})")
                else:
                    held.append(kind)
            written.append(f"m{measure.number}[" + " ".join(held) + "]")
        out.append(" ".join(written))
    return out


def test_a_figured_bass_line_is_written_out():
    """Every answer is music21's own for the same line."""
    line = m.FiguredBassLine(m.Key("B"), m.TimeSignature("3/4"))
    assert (str(line.inKey), str(line.inTime)) == (
        "B major",
        "<music21.meter.TimeSignature 3/4>",
    )

    # As music21 does, the figures are written onto the note handed in.
    bass = m.Note("B2")
    line.addElement(bass)
    line.addElement(m.Note("C#3"), "6")
    line.addElement(m.Note("D#3"), "6")
    assert [lyric.text for lyric in bass.lyrics] == [""]
    assert bass.editorial.notationString is None

    written = line.generateBassLine()
    score = m.Score()
    score.insert(0.0, written)
    assert outline_of(score) == [
        "m1[BassClef KeySignature TimeSignature "
        "Note(B2,1.0) Note(C#3,1.0) Note(D#3,1.0)]"
    ]
    # The crate keeps a barline on the measure rather than among its
    # elements, which is why the outline above shows none; music21's own
    # measure holds a Barline there, and says the same thing.
    assert [
        str(bar.rightBarline) for bar in written.getElementsByClass("Measure")
    ] == ["<music21.bar.Barline type=final>"]
    assert [
        [lyric.text for lyric in note.lyrics]
        for note in written.recurse()
        if getattr(note, "isNote", False)
    ] == [[""], ["6"], ["6"]]

    # Anything that is not a bass object is refused where music21 refuses
    # it: on reading the editorial it writes the figures onto.
    with pytest.raises(AttributeError):
        m.FiguredBassLine().addElement("C3")


def test_a_figured_bass_line_is_realized():
    """Every count and every voicing is music21's own for the same line."""
    line = m.FiguredBassLine()
    line.addElement(m.Note("C3", quarterLength=2.0))
    line.addElement(m.Note("G2", quarterLength=2.0), "7")
    line.addElement(m.Note("C3", quarterLength=4.0))

    segments = line.retrieveSegments()
    assert [
        (str(s.bassNote.pitch), s.bassNote.quarterLength, list(s.pitchNamesInChord))
        for s in segments
    ] == [
        ("C3", 2.0, ["C", "E", "G"]),
        ("G2", 2.0, ["G", "B", "D", "F"]),
        ("C3", 4.0, ["C", "E", "G"]),
    ]
    assert [(s.numParts, str(s._maxPitch)) for s in segments] == [(4, "B5")] * 3

    realization = line.realize()
    assert realization.getNumSolutions() == 20
    progressions = realization.getAllPossibilityProgressions()
    assert len(progressions) == 20
    assert [tuple(str(p) for p in voicing) for voicing in progressions[0]] == [
        ("G3", "E3", "C3", "C3"),
        ("B3", "F3", "D3", "G2"),
        ("C4", "E3", "C3", "C3"),
    ]

    # Keyboard style to begin with, as music21's is: the upper parts as
    # chords over the bass.
    assert realization.keyboardStyleOutput
    score = realization.generateRealizationFromPossibilityProgression(progressions[0])
    assert outline_of(score) == [
        "m1[TrebleClef KeySignature TimeSignature "
        "Chord(G3+E3+C3,2.0) Chord(B3+F3+D3,2.0)] m2[Chord(C4+E3+C3,4.0)]",
        "m1[BassClef KeySignature TimeSignature Note(C3,2.0) Note(G2,2.0)] "
        "m2[Note(C3,4.0)]",
    ]
    realization.keyboardStyleOutput = False
    chorale = realization.generateRealizationFromPossibilityProgression(progressions[0])
    assert len(outline_of(chorale)) == 4
    realization.keyboardStyleOutput = True

    # The rules and the parts are the caller's to say.
    loose = m.Rules()
    loose.forbidVoiceCrossing = False
    assert line.realize(loose).getNumSolutions() == 20
    assert line.realize(None, 3).getNumSolutions() == 0
    assert line.realize(None, 4, "F5").getNumSolutions() == 18

    # A line of one segment, and a line of chord symbols.
    short = m.FiguredBassLine()
    short.addElement(m.Note("C3", quarterLength=4.0))
    assert short.realize().getNumSolutions() == 21
    chords = m.FiguredBassLine()
    chords.addElement(m.ChordSymbol("C"))
    chords.addElement(m.ChordSymbol("G"))
    assert chords.realize().getNumSolutions() == 31

    # music21's own retrieveSegments raises an IndexError for a line with
    # nothing in it; this answers what realize's own docstring says.
    with pytest.raises(m.FiguredBassLineException):
        m.FiguredBassLine().realize()


def test_a_realization_chosen_at_random():
    """Chosen through Python's own random, which is where music21 chooses."""
    line = m.FiguredBassLine()
    line.addElement(m.Note("C3", quarterLength=2.0))
    line.addElement(m.Note("G2", quarterLength=2.0), "7")
    realization = line.realize()
    every = [
        [[p.nameWithOctave for p in voicing] for voicing in progression]
        for progression in realization.getAllPossibilityProgressions()
    ]

    random.seed(7)
    chosen = realization.getRandomPossibilityProgression()
    names = [[p.nameWithOctave for p in voicing] for voicing in chosen]
    assert names in every
    # A caller who seeds random gets the same realization twice.
    random.seed(7)
    again = realization.getRandomPossibilityProgression()
    assert [[p.nameWithOctave for p in voicing] for voicing in again] == names

    random.seed(7)
    assert len(outline_of(realization.generateRandomRealization())) == 2
    random.seed(7)
    assert len(outline_of(realization.generateRandomRealizations(2))) == 2
    # Asked for more than there are, it answers every one of them.
    asked = outline_of(realization.generateRandomRealizations(1000))
    assert len(asked) == len(outline_of(realization.generateAllRealizations()))


def test_a_part_laid_over_a_figured_bass_line():
    """Every answer is music21's own for the same line."""

    def overlaid():
        # A fresh line each time: music21's own retrieveSegments appends
        # the bass line to its overlaid parts, so asking it twice is not
        # asking the same thing twice. This one reads the line and leaves
        # it as it was.
        line = m.FiguredBassLine()
        line.addElement(m.Note("C3", quarterLength=1.0))
        line.addElement(m.Note("D3", quarterLength=1.0), "6")
        line.addElement(m.Note("E3", quarterLength=2.0), "6")
        top = m.Part()
        for name in ("E5", "F5", "G5", "C5"):
            top.append(m.Note(name, quarterLength=1.0))
        line.overlayPart(top)
        return line

    segments = overlaid().retrieveSegments()
    # Four segments for three bass notes: where the melody moves and the
    # bass holds, music21 makes a segment of no length whose other parts
    # must stay where they are.
    assert [float(s.quarterLength) for s in segments] == [1.0, 1.0, 2.0, 0.0]
    assert [
        [(part, str(held)) for part, held in s.fbRules._partPitchLimits]
        for s in segments
    ] == [
        [],
        [(1, "<music21.note.Note F>")],
        [(1, "<music21.note.Note G>")],
        [(1, "<music21.note.Note C>")],
    ]
    assert [list(s.fbRules._partsToCheck) for s in segments] == [[], [], [2, 3, 4], []]

    realization = overlaid().realize()
    assert realization.getNumSolutions() == 9
    score = realization.generateRealizationFromPossibilityProgression(
        realization.getAllPossibilityProgressions()[0]
    )
    assert outline_of(score) == [
        "m1[TrebleClef KeySignature TimeSignature "
        "Chord(E5+C5+G4,1.0) Chord(F5+B4+B4,1.0) Chord(G5+C5+G4,2.0)]",
        "m1[BassClef KeySignature TimeSignature "
        "Note(C3,1.0) Note(D3,1.0) Note(E3,2.0)] m2[Note(E3,2.0)]",
    ]


class _Counter:
    """A processor of the shape music21's WindowedAnalysis asks for."""

    def process(self, window):
        names = [
            n.nameWithOctave for n in window.recurse() if getattr(n, "isNote", False)
        ]
        if not names:
            raise m.DiscreteAnalysisException("nothing to analyse")
        return "+".join(names), "#%02x0000" % min(len(names), 255)


def test_an_analysis_run_over_windows_of_a_stream():
    """Every answer is music21's own for the same line and the same processor.

    The processor is the test's own, which is how the colour comes to be
    answered at all: a window's answer is (data, colour), and nothing in
    the package knows the colour a key or a pitch span is drawn in.
    """
    line = m.from_tiny_notation("4/4 c4 e g c' c e g c'")
    analysis = m.WindowedAnalysis(line, _Counter())

    # The stream cut into measures a quarter note long, and into any other
    # length the caller says.
    assert len(analysis.getMinimumWindowStream().getElementsByClass("Measure")) == 8
    assert len(analysis.getMinimumWindowStream("1/2").getElementsByClass("Measure")) == 4

    # One window at a time, then two, overlapping as music21 overlaps them.
    data, colours = analysis.analyze(1)
    assert data == ["C4", "E4", "G4", "C5", "C4", "E4", "G4", "C5"]
    assert colours == ["#010000"] * 8
    data, colours = analysis.analyze(2)
    assert data == ["C4+E4", "E4+G4", "G4+C5", "C5+C4", "C4+E4", "E4+G4", "G4+C5"]

    # Side by side, with music21's own warning where the windows do not
    # divide evenly, and the last window short.
    with pytest.warns(UserWarning, match="not divisible by windowSize"):
        data, colours = analysis.analyze(3, windowType="noOverlap")
    assert data == ["C4+E4+G4", "C5+C4+E4", "G4+C5"]

    # Averaged: for each quarter, every overlapping window holding it, put
    # end to end.
    data, colours = analysis.analyze(2, windowType="adjacentAverage")
    assert data == [
        "C4+E4",
        "C4+E4+E4+G4",
        "E4+G4+G4+C5",
        "G4+C5+C5+C4",
        "C5+C4+C4+E4",
        "C4+E4+E4+G4",
        "E4+G4+G4+C5",
        "G4+C5",
    ]

    # A window type nobody answers to is refused, as music21 refuses it.
    with pytest.raises(Exception):
        analysis.analyze(1, windowType="sideways")

    # Every window size, each with its answers, its colours and a note of
    # the size.
    solutions, colours, meta = analysis.process(1, 3, includeTotalWindow=False)
    assert [len(row) for row in solutions] == [8, 7, 6]
    assert [len(row) for row in colours] == [8, 7, 6]
    assert meta == [{"windowSize": 1}, {"windowSize": 2}, {"windowSize": 3}]
    # The whole stream is one of the sizes unless it is asked not to be.
    assert analysis.process(1, 2)[2] == [
        {"windowSize": 1},
        {"windowSize": 2},
        {"windowSize": 8},
    ]
    # A step written as a string doubles the size, as music21 reads it.
    assert analysis.process(1, 8, "2x", includeTotalWindow=False)[2] == [
        {"windowSize": 1},
        {"windowSize": 2},
        {"windowSize": 4},
    ]
    # `process` reads the window type loosely where `analyze` reads it
    # exactly, as music21 reads them.
    assert analysis.process(2, 2, 1, "nonoverlapping")[2] == [
        {"windowSize": 2},
        {"windowSize": 8},
    ]

    # A window the processor cannot read answers music21's own pair.
    quiet = m.WindowedAnalysis(m.from_tiny_notation("4/4 c4 r r e"), _Counter())
    data, colours = quiet.analyze(1)
    assert data == ["C4", (None, None, 0), (None, None, 0), "E4"]
    assert colours == ["#010000", "#ffffff", "#ffffff", "#010000"]

    # A score is flattened, as music21 flattens it: part-like substreams
    # are not supported there either.
    scored = m.Score()
    scored.insert(0.0, m.from_tiny_notation("4/4 c4 e g c'"))
    scored.insert(0.0, m.from_tiny_notation("4/4 e4 g c' e'"))
    assert m.WindowedAnalysis(scored, _Counter()).analyze(1)[0] == [
        "C4+E4",
        "E4+G4",
        "G4+C5",
        "C5+E5",
    ]

    with pytest.raises(m.WindowedAnalysisException):
        m.WindowedAnalysis("C4", _Counter())


def test_a_dynamic_says_what_it_is():
    """music21 writes a dynamic as its mark, and the class alone for none."""
    assert repr(m.Dynamic("mf")) == "<music21.dynamics.Dynamic mf>"
    assert repr(m.Dynamic("")) == "<music21.dynamics.Dynamic>"
    # Nothing said is music21's `n`, as the value itself says.
    assert repr(m.Dynamic()) == "<music21.dynamics.Dynamic n>"


def test_a_percussion_chord_says_what_it_strikes():
    """Several drums struck at once, which is not a chord by pitch."""
    struck = m.PercussionChord([m.Note("E5"), m.Note("C4")])
    # music21 writes what is struck, in the order it was given.
    assert repr(struck) == "<music21.percussion.PercussionChord [E5 C4]>"
    assert struck.isChord is False
    assert len(struck) == 2
    assert [pitch.nameWithOctave for pitch in struck.pitches] == ["E5", "C4"]
    # One holding nothing says where it sits instead, as music21's does.
    assert repr(m.PercussionChord()).startswith(
        "<music21.percussion.PercussionChord object at 0x"
    )

    # The same objects every time, and the ones a caller handed over.
    note = m.Note("D4")
    struck.notes = [note]
    assert struck.notes[0] is note

    # Writing the pitches replaces everything struck with notes of them,
    # which is music21's own setter.
    struck.pitches = [60]
    assert [held.pitch.nameWithOctave for held in struck.notes] == ["C4"]

    # Only a note or a stroke may be struck; a rest sounds nothing at all.
    with pytest.raises(TypeError):
        struck.notes = [m.Rest()]

    # Two are equal where they strike the same things in the same order,
    # and a copy strikes copies.
    copied = copy.deepcopy(struck)
    assert copied == struck
    assert copied.notes[0] is not struck.notes[0]
    assert m.PercussionChord([m.Note("C4"), m.Note("E4")]) != m.PercussionChord(
        [m.Note("E4"), m.Note("C4")]
    )


def test_a_midi_key_names_the_drum_it_plays():
    """The General MIDI percussion map, read either way."""
    mapper = m.PercussionMapper()
    # A key says which drum *and how it is played*: 36 is a bass drum
    # played `midi-1` and 35 the same drum played `acoustic`.
    played = mapper.midiPitchToInstrument(36)
    assert (played.instrumentName, played.modifier) == ("Bass Drum", "midi-1")
    assert mapper.midiPitchToInstrument(35).modifier == "acoustic"
    # A pitch is read as the key it is struck on.
    assert mapper.midiPitchToInstrument(m.Pitch(57)).instrumentName == "Crash Cymbals"

    # And back: the key a drum is struck on.
    assert mapper.midiInstrumentToPitch(played).nameWithOctave == "C2"

    # The key is the drum struck, not the class's own: a muted conga is
    # key 62 with the high drum's modifier, where 63 is it struck open.
    muted = mapper.midiPitchToInstrument(62)
    assert (muted.modifier, muted.percMapPitch, muted.muted) == ("high", 62, True)
    assert mapper.midiPitchToInstrument(63).muted is False

    # Every key the map names a drum for, against the class of that drum.
    mapping = mapper.reverseInstrumentMapping
    assert len(mapping) == 52
    assert mapping[36].__name__ == "BassDrum"

    # Nine of the map's sounds are no instrument, and each is a bare
    # unpitched percussion instrument under the sound's own name.
    sounds = mapper.namedSoundMapping
    assert len(sounds) == 9
    assert sounds[39] == "Hand Clap"
    clap = mapper.midiPitchToInstrument(39)
    assert (clap.instrumentName, clap.percMapPitch) == ("Hand Clap", 39)

    # A key the map names nothing for, and an instrument it has no key
    # for, are refused in music21's own words.
    with pytest.raises(m.MIDIPercussionException):
        mapper.midiPitchToInstrument(0)
    with pytest.raises(m.MIDIPercussionException):
        mapper.midiInstrumentToPitch(m.Piano())

    # Unless the caller asks for one anyway, which keeps the key.
    other = mapper.midiPitchToInstrument(22, raiseIfUnmapped=False)
    assert (other.instrumentName, other.percMapPitch) == ("Percussion", 22)


def test_lyrics_are_found_in_a_stream_as_text():
    """Each verse read as one text, and the notes a stretch is sung to."""
    line = m.from_tiny_notation("4/4 c4_Et d_in f_terra a_pax")
    searcher = m.LyricSearcher(line)
    assert searcher.indexText == "Et in terra pax"

    # Every lyric with where its text sits, on the stream's own notes.
    indexed = searcher.index()
    assert len(indexed) == 4
    first = indexed[0]
    assert (first.start, first.end, first.measure, first.text) == (0, 2, 1, "Et")
    assert first.el is line.recurse().notes[0]
    assert first.lyric is first.el.lyrics[0]
    assert repr(first) == (
        "IndexedLyric(el=<music21.note.Note C>, start=0, end=2, measure=1, "
        "lyric=<music21.note.Lyric number=1 syllabic=single text='Et'>, "
        "text='Et', identifier=1)"
    )
    # music21's is a namedtuple, so it is read as a tuple too, and one part
    # may be written afresh.
    assert len(first) == 9
    assert tuple(first)[1:4] == (0, 2, 1)
    assert first.modify(text="ET").text == "ET"
    assert first.text == "Et"

    # A stretch of text across a word break is sung to both its notes.
    found = searcher.search("in ter")
    assert len(found) == 1
    assert found[0].matchText == "in ter"
    assert (found[0].mStart, found[0].mEnd, found[0].identifier) == (1, 1, 1)
    sung = line.recurse().notes
    # A walk answers a list for a stretch of it, as music21's iterator
    # does and as music21's own code asks for.
    assert found[0].els == tuple(sung[1:3])
    # A match is music21's own `SearchMatch` where music21 is installed and
    # a plain named tuple of the same parts otherwise, holding nothing of
    # the crate's either way.
    assert [index.text for index in found[0].indices] == ["in", "terra"]
    assert type(found[0]).__name__ == "SearchMatch"

    # A plain string is read as itself, where a compiled pattern is matched
    # as a pattern: `.` is a character in the first and anything in the
    # second.
    assert searcher.search("t.rra") == []
    assert len(searcher.search(re.compile("t.rra"))) == 1

    # Nothing to search, and something that is neither a string nor a
    # pattern, are refused in music21's words.
    with pytest.raises(m.LyricSearcherException):
        m.LyricSearcher().index()
    with pytest.raises(m.LyricSearcherException):
        searcher.search(5)


def test_a_part_is_cut_into_overlapping_segments():
    """A part written as text and cut into stretches, and the stretches of
    some scores compared with one another."""
    line = m.from_tiny_notation("4/4 c4 d e f g a b c' d' e' f' g'")
    segments, measures = m.translateMonophonicPartToSegments(
        line, segmentLengths=4, overlap=2
    )
    assert segments == ["<>@A", "@ACE", "CEGH", "GHJL", "JLMO", "MO"]
    # Each stretch's first and last measure, as music21 reads them off the
    # translation.
    assert measures == [(1, 1), (1, 2), (2, 2), (2, 3), (3, 3), (3, 3)]

    # A score is indexed part by part, each as music21's pair of lists.
    score = m.Score()
    score.insert(0.0, line)
    indexed = m.indexScoreParts(score, segmentLengths=4, overlap=2)
    assert indexed == [{"segmentList": segments, "measureList": measures}]

    # Every stretch of one score against every stretch of the next, by
    # difflib's ratio; a score is never compared with itself.
    alike = m.scoreSimilarity(
        {"first": indexed, "second": indexed}, minimumLength=2, forceDifflib=True
    )
    assert alike[0] == ("first", 0, 0, (1, 1), "second", 0, 0, (1, 1), 1.0)
    assert len(alike) == len(segments) ** 2
    assert len(m.scoreSimilarity({"only": indexed}, minimumLength=2, forceDifflib=True)) == 0

    # What the crate cannot do honestly is said rather than answered
    # another way: a jitter moves each start at random.
    with pytest.raises(m.StreamException):
        m.translateMonophonicPartToSegments(line, jitter=2)
    # And a comparison by Levenshtein is a different measure from
    # difflib's, which is the only one here -- so it is refused wherever
    # music21 would reach for that library, and answered where music21
    # would fall back to difflib itself.
    if importlib.util.find_spec("Levenshtein") is not None:
        with pytest.raises(m.StreamException):
            m.scoreSimilarity({"first": indexed}, minimumLength=2)
    else:
        assert m.scoreSimilarity({"first": indexed}, minimumLength=2) == []


def test_a_stream_is_searched_for_a_run_of_notes():
    """Runs of notes found by name and by rhythm, and the text a line is
    written as so that two lines can be compared."""
    line = m.from_tiny_notation("3/4 c4. d8 e4 g4. a8 f4 c'8 d'4.")
    notes = line.recurse().notes

    # The text of a line: a character for each note's pitch and one for its
    # length, and the shorter readings beside it.
    assert m.translateStreamToString(notes) == "<U>F@PCUEFAPHFJU"
    assert m.translateStreamToStringNoRhythm(notes) == "<>@CEAHJ"
    assert m.translateStreamToStringOnlyRhythm(notes) == "UFPUFPFU"
    assert m.translateDiatonicStreamToString(notes) == "CRLNOMQK"
    # One note at a time, which is what the text is made of.
    first = notes.first()
    assert m.translateNoteToByte(first) == "<"
    assert m.translateDurationToBytes(first) == "U"
    assert m.translateNoteWithDurationToBytes(first) == "<U"
    assert m.translateNoteTieToByte(first) == ""

    # A dotted quarter followed by an eighth, twice over.
    dotted = [m.Note("C4"), m.Note("C4")]
    dotted[0].duration.quarterLength = 1.5
    dotted[1].duration.quarterLength = 0.5
    assert m.rhythmicSearch(notes, dotted) == [0, 3]
    assert m.noteNameSearch(notes, [m.Note("D4"), m.Note("E4")]) == [1]

    # The searcher itself, whose algorithms are a list of functions a
    # caller adds to, as music21's are.
    searcher = m.StreamSearcher(line, dotted)
    searcher.recurse = True
    searcher.filterNotes = True
    assert [function.__name__ for function in searcher.algorithms] == [
        "wildcardAlgorithm"
    ]
    searcher.algorithms.append(m.StreamSearcher.rhythmAlgorithm)
    found = searcher.run()
    assert [match.index for match in found] == [0, 3]
    assert found[0].elStart is notes.first()
    assert len(found[0].els) == 2
    # Each algorithm answers that a pair matches, that it does not, or
    # nothing at all, leaving the next to say.
    assert searcher.wildcardAlgorithm(m.Note("C"), m.Note("D")) is None
    assert searcher.rhythmAlgorithm(m.Note("C"), m.Note("D")) is None

    # An algorithm of one's own is said rather than run another way.
    searcher.algorithms.append(lambda me, streamEl, searchEl: True)
    with pytest.raises(m.SearchException):
        searcher.run()

    # How alike other lines are, each told what it scored.
    others = [
        m.from_tiny_notation("3/4 c4. d8 e4"),
        m.from_tiny_notation("3/4 g4 a b"),
    ]
    ranked = m.approximateNoteSearch(line, others)
    assert ranked[0] is others[0]
    assert ranked[0].matchProbability > ranked[1].matchProbability

    # The rhythms the measures share, most common first, each with a copy
    # of the first measure moved to start on C5.
    rhythms = m.mostCommonMeasureRhythms(line)
    assert [(found["number"], found["rhythmString"]) for found in rhythms] == [
        (2, "UFP"),
        (1, "FU"),
    ]
    assert [
        held.pitch.nameWithOctave for held in rhythms[0]["rhythm"].recurse().notes
    ] == ["C5", "D5", "E5"]
    assert rhythms[0]["measures"][0] is line.recurse().getElementsByClass("Measure").first()


def test_a_harm_token_is_read_as_a_roman_numeral():
    """`**harm`'s notation is not music21's: an inversion letter stands for
    the figured bass, so `V7b` is a `V65`."""
    assert m.convertHarmToRoman("V7b") == "V65"
    assert m.convertHarmToRoman("Gnc") == "Ger43"
    # A token that says no chord says nothing.
    assert m.convertHarmToRoman("zzz") is None

    # The parts a token is read into, as music21's own groups come out.
    assert m.HarmParser().parse("V7b") == {
        "accidental": None,
        "root": "V",
        "attribute": None,
        "intervals": "7",
        "inversion": "b",
        "alternative": None,
        "secondary": None,
        "implied": False,
    }
    # A token in brackets is implied harmony, and one that cannot be read
    # at all is nothing read.
    assert m.HarmParser().parse("(I)")["implied"] is True
    assert m.HarmParser().parse("zzz") == {}


def test_what_sounds_under_each_dynamic():
    """Every pitch against the dynamic it sounds under, which is where it
    stands among music21's short names."""
    line = m.from_tiny_notation("4/4 c4 d e f")
    line.insert(0.0, m.Dynamic("p"))
    line.insert(2.0, m.Dynamic("f"))
    matched = m.ActivityMatch(line)
    # `e` starts just where the piano ends and the forte begins, so it
    # sounds under both, as music21 reads it.
    assert matched.pitchToDynamic() == [
        (60.0, 5),
        (62.0, 5),
        (64.0, 5),
        (64.0, 8),
        (65.0, 8),
    ]
    # Asked for coordinates instead, each pair comes once with how many
    # times it was matched.
    assert matched.pitchToDynamic(False) == [
        (60.0, 5, 1),
        (62.0, 5, 1),
        (64.0, 5, 1),
        (64.0, 8, 1),
        (65.0, 8, 1),
    ]

    # Anything but a stream, and a stream with no dynamic in it, are
    # refused in music21's words.
    with pytest.raises(m.CorrelateException):
        m.ActivityMatch("not a stream")
    with pytest.raises(m.CorrelateException):
        m.ActivityMatch(m.from_tiny_notation("4/4 c4 d e f")).pitchToDynamic()


def test_a_stream_is_searched_for_a_row_of_pitch_classes():
    """Runs of notes read as pitch classes, matched against segments and
    marked where they are found."""
    line = m.from_tiny_notation("4/4 c4 d e f g a b c'")

    # Every run of three notes, as the searcher reads repetitions.
    searcher = m.ContiguousSegmentSearcher(line, "skipConsecutive", False)
    runs = searcher.byLength(3)
    assert len(runs) == 6
    assert repr(runs[0]) == (
        "<music21.search.serial.ContiguousSegmentOfNotes ['C4', 'D4', 'E4']>"
    )
    assert runs[0].readPitchClassesFromBottom() == [0, 2, 4]
    assert runs[0].getDistinctPitchClasses() == [0, 2, 4]
    # A run's notes are the stream's own.
    assert list(runs[0].segment) == list(line.recurse().notes)[:3]

    # The runs that are the segments searched for, each told what it was
    # read as and what it was matched to.
    matcher = m.SegmentMatcher(line, includeChords=False)
    found = matcher.find([[0, 2, 4], [7, 9, 11]])
    assert [run.activeSegment for run in found] == [[0, 2, 4], [7, 9, 11]]
    assert [run.matchedSegment for run in found] == [[0, 2, 4], [7, 9, 11]]
    assert found[0].containerStream is line

    # A transposed matcher reads a run as the intervals it makes, and a
    # transformed one as the row it is.
    assert m.TransposedSegmentMatcher.normalize([3, 4, 5]) == "11"
    assert m.SegmentMatcher.normalize(["B", -24, "1"]) == [11, 0, 1]
    assert repr(m.TransformedSegmentMatcher.normalize([3, 4, 5])) == (
        "<music21.serial.ToneRow 345>"
    )
    transformed = m.TransformedSegmentMatcher
    assert transformed.getTransformations(
        transformed.normalize([3, 4, 5]), transformed.normalize([4, 5, 6])
    ) == [("P", 3), ("RI", 5)]

    # A segment looked for already is passed over, which for a transformed
    # matcher means one looked for in another form.
    asked = m.SegmentMatcher(None)
    assert asked.checkSearchedAlready([4, 5, 6]) is False
    assert asked.checkSearchedAlready([1, 2, 3]) is False
    assert asked.checkSearchedAlready([4, 5, 6]) is True
    rows = m.TransformedSegmentMatcher(None)
    assert rows.checkSearchedAlready([0, 1, 3]) is False
    # The retrograde inversion of the same row.
    assert rows.checkSearchedAlready([3, 5, 6]) is True

    # Labelling marks a copy: a lyric on the first note of each run found
    # and a line joining its ends.
    labelled = m.labelSegments(line, {"CDE": [0, 2, 4]}, includeChords=False)
    assert [
        [lyric.text for lyric in note.lyrics] for note in labelled.recurse().notes
    ][:3] == [["CDE"], [], []]
    assert len(labelled.recurse().getElementsByClass("Line")) == 1
    # The stream it was given is left as it was.
    assert [lyric.text for lyric in line.recurse().notes.first().lyrics] == []


def test_music_is_written_in_braille():
    """One element at a time, as music21's `braille.basic` writes it."""
    note = m.Note("C4")
    assert m.noteToBraille(note) == "⠐⠹"
    # Without its octave mark, which a segment decides.
    assert m.noteToBraille(note, showOctave=False) == "⠹"
    # What each sign says is written onto the object, as music21 writes it.
    assert note.editorial.brailleEnglish == ["C quarter ⠹"]

    chord = m.Chord(["G4", "B4", "D5", "G5"])
    chord.quarterLength = 4.0
    assert m.chordToBraille(chord, descending=True) == "⠨⠷⠼⠴⠤"
    assert m.restToBraille(m.Rest()) == "⠧"
    assert m.timeSigToBraille(m.TimeSignature("4/8")) == "⠼⠙⠦"
    assert m.keySigToBraille(m.KeySignature(3)) == "⠩⠩⠩"
    # A mark that says no number is written as nothing at all.
    assert m.metronomeMarkToBraille(m.MetronomeMark(numberSounding=80)) == ""

    # The octave mark alone, from a pitch or from the octave itself.
    assert m.pitchToOctave(m.Pitch("C4")) == m.pitchToOctave(4)
    # A note within a third of the one before carries no octave mark.
    assert m.showOctaveWithNote(m.Note("C4"), m.Note("E4")) is False
    assert m.showOctaveWithNote(None, m.Note("E4")) is True

    # Words, numbers and the two writings of braille itself.
    assert m.wordToBraille("Andante") == "⠠⠁⠝⠙⠁⠝⠞⠑"
    assert m.numberToBraille(37) == "⠼⠉⠛"
    assert m.brailleUnicodeToBrailleAscii("⠜⠋⠂") == ">F1"
    assert m.brailleAsciiToBrailleUnicode("_>") == "⠸⠜"
    assert list(m.yieldDots("⠇")) == ["⠄", "⠄", "⠄"]


def test_braille_is_laid_out_in_lines():
    """Lines of so many cells, as music21's `braille.text` lays them out."""
    text = m.BrailleText(lineLength=10, showHand="right")
    assert repr(text) == "<music21.braille.text.BrailleText 1 line, 0 headings, 10 cols>"
    assert (text.lineLength, text.rightHandSymbol, text.leftHandSymbol) == (
        10,
        True,
        False,
    )
    assert text.allHeadings == []

    text.addHeading(m.timeSigToBraille(m.TimeSignature("4/8")))
    text.addMeasureNumber(7)
    assert len(text.allLines) == 2
    assert text.allLines[0].isHeading is True
    assert str(text.allLines[0]) == "⠼⠙⠦"
    assert text.allHeadings == [(0, 1)]

    # A caller holding a line is holding the text's: writing through it
    # reaches the text.
    line = text.currentLine
    line.append("⠽", addSpace=False)
    assert text.currentLine is line
    assert str(text).splitlines()[-1].endswith("⠽")

    # The two hands at a keyboard are the text's own lines.
    keyboard = m.BrailleKeyboard(lineLength=10)
    keyboard.highestMeasureNumberLength = 2
    keyboard.makeNewLines()
    assert keyboard.rightHandLine is keyboard.allLines[0]
    assert keyboard.leftHandLine is keyboard.allLines[1]

    # One line on its own, which is what a text is made of.
    bare = m.BrailleTextLine(5)
    assert repr(bare) == "<music21.braille.text.BrailleTextLine '⠀⠀⠀⠀⠀'>"
    assert bare.allChars == 5 * ["⠀"]
    assert bare.canAppend("⠹", addSpace=False) is True
    bare.append("⠹", addSpace=False)
    assert str(bare) == "⠹"
    assert bare.textLocation == 1


def test_a_score_is_written_in_braille():
    """A part, a measure or a whole score, as music21's `braille.translate`
    writes them."""
    line = m.from_tiny_notation("4/4 c1")
    assert m.objectToBraille(line) == "⠀⠀⠼⠙⠲⠀⠀\n⠼⠁⠀⠐⠽⠣⠅"
    # A measure on its own is written with no heading and no measure
    # number, as music21 writes one.
    measure = line.getElementsByClass("Measure")[0]
    assert m.measureToBraille(measure) == "⠼⠙⠲⠀⠐⠽⠣⠅"
    # The same part through the other two names music21 gives.
    assert m.streamToBraille(line) == m.objectToBraille(line)
    assert m.partToBraille(line) == m.objectToBraille(line)

    # One element on its own, which is what a stream is written out of.
    assert m.objectToBraille(m.Note("C4")) == "⠐⠹"

    # The English listing of each segment, which music21 writes instead of
    # braille when asked to debug.
    listing = m.objectToBraille(line, debug=True)
    assert "---begin segment---" in listing
    assert "Time Signature 4/4" in listing

    # What the crate cannot do honestly is said: music21's segmentBreaks
    # says where to break a segment by hand.
    with pytest.raises(m.BrailleTranslateException):
        m.objectToBraille(line, segmentBreaks=[(1, 2.0)])


def test_music21s_segment_division_divides_braille():
    """music21's `BrailleSegmentDivision` in a measure starts a new
    segment there, as its `getRawSegments` example shows."""
    objects = pytest.importorskip("music21.braille.objects")
    part = m.from_tiny_notation("3/4 c4 c c e e e g g g c'2.").makeNotation()
    part.measure(2).insert(1.0, objects.BrailleSegmentDivision())
    assert m.objectToBraille(part) == (
        "⠀⠀⠀⠼⠉⠲⠀⠀⠀⠀\n⠼⠁⠀⠐⠹⠹⠹⠀⠫⠐\n⠼⠃⠄⠀⠐⠫⠫⠀⠳⠳⠳⠀⠨⠝⠄⠣⠅"
    )


def test_a_grouping_of_elements_is_written_in_braille():
    """The elements braille writes together, and the transcriber that
    writes them one at a time."""
    grouping = m.BrailleElementGrouping()
    # music21's mimics a list, so this one hands `append` on to the list it
    # holds.
    grouping.append(m.Note("C4"))
    grouping.append(m.Note("D4"))
    grouping.append(m.Rest())
    assert len(grouping) == 3
    assert repr(grouping) == (
        "<music21.braille.segment.BrailleElementGrouping ["
        "<music21.note.Note C>, <music21.note.Note D>, "
        "<music21.note.Rest quarter>]>"
    )
    # The defaults music21 starts a grouping with.
    assert repr(grouping.keySignature) == (
        "<music21.key.KeySignature of no sharps or flats>"
    )
    assert repr(grouping.timeSignature) == "<music21.meter.TimeSignature 4/4>"
    assert (grouping.descendingChords, grouping.showClefSigns) == (True, False)
    assert (grouping.upperFirstInNoteFingering, grouping.withHyphen) == (True, False)
    assert grouping.numRepeats == 0

    transcriber = m.NoteGroupingTranscriber()
    # What a caller says wins over the grouping, which wins over the
    # default.
    assert transcriber.showClefSigns is False
    transcriber.brailleElementGrouping = grouping
    grouping.showClefSigns = True
    assert transcriber.showClefSigns is True
    transcriber.showClefSigns = False
    assert transcriber.showClefSigns is False
    grouping.showClefSigns = False

    assert transcriber.transcribeGroup(grouping) == "⠐⠹⠱⠧"
    # What each sign says is written onto the element, as music21 writes it.
    assert grouping[0].editorial.brailleEnglish == ["Octave 4 ⠐", "C quarter ⠹"]
    assert str(grouping).splitlines()[-1] == "Rest quarter ⠧"

    # One element at a time, which is how music21 writes a grouping and
    # what its own examples ask for.
    one = m.NoteGroupingTranscriber()
    note = m.Note("C4")
    one.transcribeOneElement(note)
    assert "".join(one.trans) == "⠐⠹"
    assert one.previousElement is note

    # A copy shares the elements and holds a list of its own, which is what
    # music21's own splitting of a grouping asks for.
    shallow = copy.copy(grouping)
    assert shallow[0] is grouping[0]
    assert shallow.internalList is not grouping.internalList
    assert copy.deepcopy(grouping)[0] is not grouping[0]


def test_a_streams_notes_are_hashed():
    """What of each note is hashed, and the hashing itself, as music21's
    `alpha.analysis.hasher` does it."""
    hasher = m.Hasher()
    # music21's defaults: a note is its MIDI pitch, its length and its
    # offset, each rounded to a 32nd of a quarter.
    assert (hasher.hashMIDI, hasher.granularity) == (True, 32)
    assert (hasher.includeReference, hasher.hashChordsAsNotes) == (False, True)

    line = m.from_tiny_notation("4/4 c4 d e f")
    hashes = hasher.hashStream(line)
    assert hasher.tupleList == ["Pitch", "Duration", "Offset"]
    assert len(hashes) == 4
    assert tuple(hashes[0]) == (60, 1.0, 0.0)

    # Asked for it, each hash says what it was made from -- the score's own
    # note.
    hasher.includeReference = True
    referenced = hasher.hashStream(line)
    assert referenced[0].reference is line.recurse().notes.first()

    # What is remembered while hashing, by the flags that ask for it.
    counting = m.Hasher()
    counting.hashIntervalFromLastNote = True
    counting.setupValidTypesAndStateVars()
    assert counting.stateVars == {"IntervalFromLastNote": None}

    # One value at a time, which is how music21 documents its own hashing.
    assert counting._hashOctave(m.Note("C5")) == 5
    assert counting._hashMIDIPitchName(m.Note(72)) == 72
    assert counting._hashRoundedDuration(m.Note("C4")) == 1.0

    # Each measure on its own.
    assert len(hasher.hashMeasures(line)) == 1

def test_two_streams_are_aligned_note_by_note():
    """Two streams aligned as the cheapest edit from one to the other, as
    music21's `alpha.analysis.aligner` aligns them. The matrix is numpy's,
    which is what music21 works in, so without numpy there is nothing to
    align in."""
    pytest.importorskip("numpy")
    target = m.from_tiny_notation("4/4 c4 d e f")
    source = m.from_tiny_notation("4/4 c4 d e")
    aligner = m.StreamAligner(target, source)
    # music21's default hasher: no offset hashed, and every hash says what
    # it was made from.
    assert (aligner.hasher.hashOffset, aligner.hasher.includeReference) == (False, True)

    aligner.align()
    assert (aligner.n, aligner.m) == (4, 3)
    assert aligner.distanceMatrix.tolist() == [
        [0, 2, 4, 6],
        [2, 0, 2, 4],
        [4, 2, 0, 2],
        [6, 4, 2, 0],
        [8, 6, 4, 2],
    ]
    assert aligner.similarityScore == 0.75
    assert aligner.changesCount[m.ChangeOps.NoChange] == 3
    assert aligner.changesCount[m.ChangeOps.Insertion] == 1

    # Every change is between the score's own notes, and the last of these
    # is the note the source has not got.
    notes = list(target.recurse().notes)
    assert aligner.changes[0][0] is notes[0]
    assert aligner.changes[-1][0] is notes[-1]
    assert aligner.changes[-1][2] == m.ChangeOps.Insertion
    assert aligner.changes[-1][2].color == "green"
    assert repr(m.ChangeOps.NoChange) == "<ChangeOps.NoChange: 3>"
    assert m.ChangeOps.NoChange.color is None

    # One step at a time, which is how music21 documents the alignment.
    assert aligner.getPossibleMovesFromLocation(0, 0) == [None, None, None]
    assert aligner.getPossibleMovesFromLocation(2, 2) == [2, 2, 0]
    assert aligner.getOpFromLocation(2, 2) == m.ChangeOps.NoChange
    assert aligner.getOpFromLocation(4, 3) == m.ChangeOps.Insertion
    first = aligner.hashedTargetStream[0]
    assert (aligner.insertCost(first), aligner.deleteCost(first)) == (2, 2)
    assert aligner.substitutionCost(first, aligner.hashedSourceStream[0]) == 0
    assert aligner.tupleEqualityWithoutReference(first, aligner.hashedSourceStream[0])
    assert aligner.calculateNumSimilarities(first, aligner.hashedSourceStream[2]) == 1

def test_a_note_read_wrong_is_respelled_against_the_performance():
    """Where the reading and the performance spell one sound differently,
    the performance's spelling wins, as music21's `EnharmonicFixer` has
    it. A change names two notes and no streams, which is how music21's
    own examples build one."""
    # music21's own cases, as its docstring prints them: a played note, a
    # note read, and what the reading becomes.
    for played, read, fixed in [
        ("A4", "An4", None),
        ("G#4", "An4", "flat"),
        ("G-4", "Fn4", "sharp"),
        ("G#4", "Gn4", "sharp"),
        ("G#4", "G-4", "sharp"),
        ("G#4", "A#4", "flat"),
        ("A-4", "G-4", "sharp"),
        # Two spellings of one sound on different letters: nothing is
        # written, so the reading keeps the sharp it was read with.
        ("B-4", "A#4", "sharp"),
    ]:
        midi, omr = m.Note(played), m.Note(read)
        fixer = m.EnharmonicFixer([(midi, omr, m.ChangeOps.Substitution)], None, None)
        fixer.fix()
        written = omr.pitch.accidental
        assert (written.name if written is not None else None) == fixed, f"{played}/{read}"
        # Every note a change names is coloured black.
        assert omr.style.color == "black"

    # Nothing is written where the alignment found no change, nor where the
    # two are too far apart to be two spellings of one sound.
    for op, played, read in [
        (m.ChangeOps.NoChange, "A4", "An4"),
        (m.ChangeOps.Substitution, "C4", "F#4"),
    ]:
        omr = m.Note(read)
        m.EnharmonicFixer([(m.Note(played), omr, op)], None, None).fix()
        assert omr.pitch.accidental.name == m.Note(read).pitch.accidental.name

    # A reading that missed a key signature takes the pitch played whole.
    midi, omr = m.Note("B-4"), m.Note("B4")
    m.EnharmonicFixer([(midi, omr, m.ChangeOps.Substitution)], None, None).fix()
    assert omr.pitch.nameWithOctave == "B-4"

    # The predicates music21 reads its cases with, each the crate's.
    fixer = m.EnharmonicFixer([], None, None)
    sharp, flat, natural = m.Note("A#4"), m.Note("B-4"), m.Note("An4")
    assert fixer.checkIfNoteInstance(sharp, flat)
    assert not fixer.checkIfNoteInstance(sharp, m.Rest())
    assert fixer.isEnharmonic(sharp, flat)
    assert fixer.hasAcc(sharp) and fixer.hasAcc(natural)
    assert fixer.hasNatAcc(natural) and not fixer.hasNatAcc(sharp)
    assert fixer.hasSharpFlatAcc(sharp) and not fixer.hasSharpFlatAcc(natural)
    assert fixer.stepEq(sharp, natural) and fixer.stepNotEq(sharp, flat)
    assert not fixer.intervalTooBig(m.Note("C4"), m.Note("F4"))
    assert fixer.intervalTooBig(m.Note("C4"), m.Note("F#4"))
    assert fixer.intervalTooBig(m.Note("C4"), m.Note("F4"), setInt=4)

def test_a_score_read_by_an_optical_reader_is_mended():
    """A measure whose notes do not fill its bar is flagged, and its
    rhythm compared with another's, as music21's `omr.correctors` does
    it."""
    whole = m.from_tiny_notation("4/4 c4 d e f  g a b c'  d'4 e' f' g'")
    short = m.from_tiny_notation("4/4 c4 d e f  g a b c'  d'4 e' f'")

    part = m.SinglePart(whole, 0)
    assert part.hashedNotes == ["PPPP", "PPPP", "PPPP"]
    assert part.incorrectMeasures == []
    assert len(part.measureStream) == 3
    flagged = m.SinglePart(short, 1)
    assert flagged.incorrectMeasures == [2]
    assert flagged.hashedNotes[2] == "PPP"
    # Each measure's own bar, rather than the first measure's.
    assert flagged.getIncorrectMeasureIndices(runFast=False) == [2]
    # How likely a measure that far from a flagged one corrects it: one
    # entry for every distance either way, as music21 sizes it.
    distribution = flagged.horizontalProbabilityDist()
    assert len(distribution) == len(flagged.hashedNotes) * 2 - 1
    assert all(0.0 <= odds <= 1.0 for odds in distribution)
    # No distance at all is the measure itself, which music21 counts as
    # certain so that the indices line up.
    assert distribution[len(flagged.hashedNotes) - 1] == 1.0
    assert flagged.indexArray[len(flagged.hashedNotes) - 1] == 0

    # One measure's rhythm as text: music21's own `VFUF` example.
    line = m.from_tiny_notation("4/4 c4. c8 r4. b8")
    hashed = m.MeasureHash(line.getElementsByClass("Measure")[0])
    assert hashed.hashString == "VFUF"
    assert hashed.getHashString() == "VFUF"
    hashed.setSequenceMatcher()
    assert hashed.getMeasureDifference("VGUF") == 0.25
    assert hashed.getMeasureDifference("VFUF") == 1.0
    assert hashed.getOpCodes("VGUF") == [
        ("equal", 0, 1, 0, 1),
        ("replace", 1, 2, 1, 2),
        ("equal", 2, 4, 2, 4),
    ]

    # The characters a note, a rest and a grace note are written as.
    assert hashed.hashQuarterLength(1.0) == 80
    assert hashed.hashNote(m.Note("C4")) == "P"
    assert hashed.hashRest(m.Rest()) == "Q"

    # And how likely each of music21's four kinds of change is.
    assert hashed.getProbabilityOnEquality() == 0.9675
    assert hashed.getProbabilityOnOmission() == 0.009
    assert hashed.getProbabilityOnAddition() == 0.004
    assert hashed.getProbabilityFromOneCharSub("P", "P") == 1.0
    assert hashed.getProbabilityFromOneCharSub("Z", "P") == 0.0165
    assert (
        hashed.differenceProbabilityForOneOpCode(
            ("equal", 0, 4, 0, 4), "PFPFFF", "PFPFGF"
        )
        == 0.8762013031640626
    )
    assert (
        hashed.differenceProbabilityForOneOpCode(("delete", 0, 5, 0, 0), "e", "GFPGF")
        == 1.024e-12
    )
    bare = m.MeasureHash()
    bare.hashString = "GFPGF"
    assert bare.getProbabilityBasedOnChanges("e") == 2.9472832125e-14

def test_a_recording_is_turned_into_notes():
    """A recorded signal read as frequencies, those as pitches, and the
    pitches as notes, as music21's `audioSearch` does it."""
    # music21's own `histogram` example: the first edge is the least of the
    # data as it was given, and the rest are shares of the span.
    counts, edges = m.histogram([1, 1, 4, 5, 6, 0, 8, 8, 8, 8, 8], 8)
    assert counts == [3, 0, 0, 1, 1, 1, 0, 5]
    assert edges == [0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    assert m.histogram([1, 1, 4, 5, 6, 0, 8, 8, 8, 8, 8], 4)[0] == [3, 1, 2, 5]

    # The frequency a signal sounds, by correlating it with itself.
    rate = 44100.0
    signal = [
        math.sin(2 * math.pi * 441.0 * i / rate) * 10000.0 for i in range(1024)
    ]
    # music21 finds 442.2033 for this window: the interpolated peak sits a
    # little early.
    assert abs(m.autocorrelationFunction(signal, rate) - 442.2033) < 1e-3

    # The thresholds between a scale's pitches, and the pitch a frequency
    # falls nearest -- music21's own 441.72 example.
    thresholds, pitches = m.prepareThresholds()
    assert len(thresholds) == len(pitches) - 1
    assert [p.nameWithOctave for p in pitches][:3] == ["C4", "C#4", "D4"]
    frequency, pitch = m.normalizeInputFrequency(441.72)
    assert (round(frequency, 4), pitch.nameWithOctave) == (440.0, "A4")
    assert m.normalizeInputFrequency(441.72, thresholds, pitches)[1].nameWithOctave == "A4"

    # music21's own `detectPitchFrequencies` example, in C major.
    heard = [143.627689055, 99.0835452019, 211.004784689, 4700.31347962, 2197.9431119]
    found = m.detectPitchFrequencies(heard, useScale=m.MajorScale("C4"))
    assert [int(round(hertz)) for hertz in found] == [147, 98, 220, 4699, 2093]

    # One bad reading smoothed away, and music21's two refusals.
    inputPitches = [
        440, 220, 440, 440, 442, 443, 441, 470, 440, 441, 440, 442, 440, 440,
        440, 397, 440, 440, 440, 442, 443, 441, 440, 440, 440, 440, 440, 442,
        443, 441, 440, 440,
    ]
    smoothed = m.smoothFrequencies(inputPitches)
    assert smoothed[:4] == [409, 409, 409, 428]
    assert inputPitches[1] == 220, "the list given is left alone"
    assert m.smoothFrequencies(list(inputPitches), inPlace=True) is None
    for levels in (0, 40):
        with pytest.raises(ValueError):
            m.smoothFrequencies(inputPitches, smoothLevels=levels)

    # A length snapped to one music21 writes, and the quarter estimated.
    assert m.quantizeDuration(0.24) == 0.25
    assert m.quantizeDuration(1.1) == 1.0
    assert m.quarterLengthEstimation([20, 19, 10, 30, 6, 21]) > 0.0

    # Runs of one pitch joined into notes, each note keeping the very pitch
    # object its run ended on.
    heard = [m.Pitch("C4") for _ in range(8)] + [m.Pitch("D4") for _ in range(8)]
    notes, lengths = m.joinConsecutiveIdenticalPitches(heard)
    assert [note.name for note in notes] == ["C", "D"]
    # music21 writes the rest frequency onto the first pitch it is given
    # and passes over the first pitch after every run, so the two runs of
    # eight count six and seven.
    assert lengths == [6, 7]
    # The rest frequency is written onto the first pitch, which music21
    # spells as the nearest note it can and so answers a hair above.
    assert abs(heard[0].frequency - 10.0) < 1e-6
    assert notes[1].pitch is heard[-1]

    # music21's own transcription example: the notes it is given, each as
    # long as its share of the estimated quarter.
    durations = [20, 19, 10, 30, 6, 21]
    written = [m.Note(name) for name in ("C#4", "D5", "B4", "F#5", "C5")] + [m.Rest()]
    score, count = m.notesAndDurationsToStream(written, durations)
    assert count == 6
    assert score.metadata.title == "Automatic Music21 Transcription"
    part = score.getElementsByClass("Part")[0]
    assert [part.elementOffset(note) for note in part] == [
        0.0,
        1.0,
        2.0,
        2.5,
        4.0,
        4.25,
    ]
    # The notes in it are the caller's own.
    assert part[0] is written[0]

def test_a_tabular_analysis_is_read_as_roman_numerals():
    """The Digital and Cognitive Musicology Lab's tables read as roman
    numerals, as music21's `romanText.tsvConverter` reads them."""
    # music21's own documented answers for the seven readings.
    assert (m.isMinor("F"), m.isMinor("f")) == (False, True)
    assert m.characterSwaps("ii%", minor=False, direction="DCML-m21") == "iiø"
    assert m.getLocalKey("V", "G") == "D"
    assert m.getLocalKey("ii", "C") == "d"
    assert m.getLocalKey("#vii", "a") == "g#"
    assert m.getLocalKey("vii", "a", convertDCMLToM21=True) == "g"
    assert m.localKeyAsRn(m.Key("C"), m.Key("e")) == "VI"
    assert m.localKeyAsRn(m.Key("C#"), m.Key("e")) == "#VI"
    assert m.localKeyAsRn(m.Key("e"), m.Key("C")) == "iii"
    assert m.getSecondaryKey("V/vi", "C") == "a"
    assert m.getForm(m.RomanNumeral("viio7")) == "o"
    assert m.getForm(m.RomanNumeral("IVM7")) == "M"
    assert m.handleAddedTones("V(64)") == "Cad64"
    assert m.handleAddedTones("i(4+2)") == "i[no3][add4][add2]"
    assert m.handleAddedTones("ii6(11#7b6)") == "ii6[no8][no5][add11][add#7][addb6]"

    # A row of the lab's second standard, spelled the lab's way and then
    # music21's -- music21's own `_changeRepresentation` example.
    row = m.TabChordV2()
    row.global_key = "F"
    row.local_key = "vi"
    row.numeral = "ii"
    row.chord = "ii%7(6)"
    row.representationType = "DCML"
    assert row.combinedChord == "ii%7(6)"
    row._changeRepresentation()
    assert row.representationType == "m21"
    assert row.chord == "iiø7[no5][add6]"

    # The lab renamed four columns, and a version 2 row answers the older
    # names by reading the newer ones.
    row.mn_onset = 0.0
    assert row.beat == 1.0
    row.mn_onset = 0.5
    assert row.beat == 3.0
    row.beat = 1.5
    assert (row.beat, row.mn_onset) == (1.5, 0.125)
    row.measure = 4
    assert (row.measure, row.mn) == (4, 4)
    assert (row.local_key, row.global_key) == (row.localkey, row.globalkey)

    # A row of the first standard as the numeral it stands for: music21's
    # own `tabToM21` example.
    first = m.TabChord()
    first.numeral = "vii"
    first.global_key = "F"
    first.local_key = "V"
    first.representationType = "m21"
    numeral = first.tabToM21()
    assert numeral.figure == "vii"
    assert numeral.key.tonicPitchNameWithCase == "C"
    assert numeral.quarterLength == 0.0

def test_the_notation_a_score_leaves_unsaid_is_worked_out():
    """Beams, stem directions and tuplet brackets written onto the notes
    they belong to, as music21's `stream.makeNotation` writes them."""
    line = m.from_tiny_notation("4/4 c16 c c c c c c c c c c c c c c c")
    measure = line.getElementsByClass("Measure")[0]

    # Beamed in place: the notes are the ones that were there.
    notes = list(measure.recurse().notes)
    assert m.makeBeams(measure, inPlace=True) is None
    assert measure.recurse().notes[0] is notes[0]
    said = [str(note.beams) for note in measure.recurse().notes[:4]]
    assert said[0].endswith("1/start>/<music21.beam.Beam 2/start>>")
    assert said[1].endswith("1/continue>/<music21.beam.Beam 2/stop>>")
    assert said[3].endswith("1/stop>/<music21.beam.Beam 2/stop>>")
    # And the stems of each beamed group point one way.
    assert {note.stemDirection for note in measure.recurse().notes} == {"up"}

    # Asked not to, it beams a copy and leaves the stream alone.
    plain = m.from_tiny_notation("4/4 c8 c c c c c c c")
    bare = plain.getElementsByClass("Measure")[0]
    beamed = m.makeBeams(bare, inPlace=False)
    assert beamed is not None
    assert str(bare.recurse().notes[0].beams) == "<music21.beam.Beams>"
    assert str(beamed.recurse().notes[0].beams).endswith("1/start>>")

    # The groups those beams make, which music21 yields one at a time.
    groups = list(m.iterateBeamGroups(beamed))
    assert [len(group) for group in groups] == [2, 2, 2, 2]
    assert groups[0][0] is beamed.recurse().notes[0]

    # What a note or chord ties forward, and what ties nothing.
    first, second, third = m.Note("C4"), m.Note("D4"), m.Note("E4")
    second.tie = m.Tie("start")
    third.tie = m.Tie("stop")
    assert m.getTiePitchSet(m.Chord([first, second, third])) == {"D4"}
    assert m.getTiePitchSet(m.Note("F#5")) == set()
    assert m.getTiePitchSet(m.Rest()) is None

    # A tuplet drawn with the bracket its place in its run asks for.
    triplet = m.from_tiny_notation("4/4 trip{c8 c c} c4 c c")
    held = triplet.getElementsByClass("Measure")[0]
    assert m.makeTupletBrackets(held, inPlace=True) is None
    tuplets = [note.duration.tuplets for note in held.recurse().notes]
    assert tuplets[0] and tuplets[0][0].type == "start"
    assert tuplets[2] and tuplets[2][0].type == "stop"


def test_a_rock_corpus_analysis_is_read_into_a_part():
    """A de Clercq-Temperley analysis read as its text says it, and played
    through to a part, as music21's `romanText.clercqTemperley` reads
    one."""
    text = """% Brown-Eyed Girl
VP: I | IV | I | V |
In: $VP*2
Vr: $VP*4 IV | V | I | vi | IV | V | I | V |     % Second half
Ch: V | | $VP*2 I |*4
S: [G] $In $Vr $Ch"""

    song = m.CTSong(text)
    assert song.title == "Brown-Eyed Girl"
    assert song.lines[0] == "% Brown-Eyed Girl"
    assert song.lines[-1] == "S: [G] $In $Vr $Ch"
    assert song.comments == [["Vr:", "Second half"]]
    assert song.homeKey.tonicPitchNameWithCase == "G"
    assert song.homeTimeSig.ratioString == "4/4"
    assert list(song.rules) == ["VP", "In", "Vr", "Ch", "S"]
    assert repr(song) == (
        "<music21.romanText.clercqTemperley.CTSong "
        "title='Brown-Eyed Girl' year=None>"
    )

    # A rule says its name, its chords, its comment and the section it
    # stands for.
    verse = song.rules["Vr"]
    assert verse.LHS == "Vr"
    assert verse.sectionName == "Verse"
    assert verse.comment == "Second half"
    assert verse.musicText.startswith("$VP*4 IV | V |")
    assert m.CTRule.fixupChordAtom("iix") == "iio"
    assert [group[1] for group in m.CTRule("In: $VP*2")._measureGroups()] == ["$"]

    # The song played through to a part, each chord carrying its figure.
    part = song.toPart()
    numerals = list(part.recurse().notes)
    assert numerals[0].figure == "I"
    assert numerals[0].lyrics[0].text == "I"
    # The part holds every measure the S: rule plays through: eight for
    # the introduction, twenty-four for the verse and fourteen for the
    # chorus. (It is music21's own `Part` wherever music21 can be asked
    # for one, and this package's otherwise, which is what the doctest
    # suite checks.)
    assert len(part.getElementsByClass("Measure")) == 8 + 24 + 14
    # The name of every rule that brought a section stands on its first
    # chord, both as a lyric under the figure and in its editorial -- the
    # innermost rule first, as music21 writes them.
    assert numerals[0].lyric == "I" + chr(10) + "VP" + chr(10) + "In"
    assert numerals[0].editorial.formalDivision == ["VP", "In"]
    assert numerals[1].lyric == "IV"
    assert numerals[1].editorial.formalDivision == []

    # A rule with a song behind it is expanded by the song, which keeps the
    # state music21 keeps on the rule.
    with pytest.raises(Exception):
        verse.expand()
    assert len(m.CTRule("In: I | IV |").expand()) == 2


def test_a_score_is_reduced_to_the_notes_an_analyst_marked():
    """The notes a lyric marks gathered into a part of their own, as
    music21's `analysis.reduction` gathers them."""
    part = m.from_tiny_notation("4/4 c4 d e f  g a b c'")
    notes = list(part.recurse().notes)
    notes[0].addLyric("::/o:5/g:Ursatz/tb:I")
    notes[4].addLyric("::/o:5/g:Ursatz/tb:V")

    reduction = m.ScoreReduction()
    reduction.score = part
    # The score it keeps is a copy, in a score of its own since the part
    # has no parts under it.
    assert len(reduction.score.getElementsByClass("Part")) == 1
    assert reduction.score.getElementsByClass("Part")[0] is not part

    reduced = reduction.reduce()
    parts = list(reduced.getElementsByClass("Part"))
    # music21 names the part it gathers and leaves the other's id as it
    # found it -- which for a stream of music21's own is `id(obj)`, a
    # number, where a stream of this package's answers nothing. Asked of
    # music21's own `ScoreReduction` the pair reads `['Ursatz', 2738...]`
    # too, so only the name is pinned here.
    assert len(parts) == 2
    assert parts[0].id == "Ursatz"
    marked = list(parts[0].recurse().notes)
    assert [note.name for note in marked] == ["C", "G"]
    assert [note.lyric for note in marked] == ["I", "V"]
    # The part the marks were read off is still there.
    assert len(list(parts[1].recurse().notes)) == 8

    # Nothing to reduce is refused, as music21 refuses it.
    with pytest.raises(Exception):
        m.ScoreReduction().reduce()
    with pytest.raises(Exception):
        m.ScoreReduction().score = m.Note("C4")


def test_two_readings_of_a_stream_are_merged_as_variants():
    """Where two readings of one stream differ, what differs is kept as a
    variant, as music21's `variant` module keeps it.

    A variant is music21's own `variant.Variant`, which this package has
    no class for, so each of these says outright that it needs music21
    beside it rather than answering a stream with no variants in it."""
    one = m.from_tiny_notation("4/4 a4 b c d  e2 f2")
    two = m.from_tiny_notation("4/4 a4 b c e  e2 f2")
    try:
        from music21 import variant as m21variant
    except ImportError:
        for call in (
            lambda: m.mergeVariants(one, two, variantName="london"),
            lambda: m.mergeVariantMeasureStreams(one, two, variantName="london"),
            lambda: m.mergeVariantsEqualDuration([one, two], ["london"]),
            lambda: m.addVariant(one, 0.0, two, variantName="london"),
            lambda: m.makeAllVariantsReplacements(one),
        ):
            with pytest.raises(Exception, match="no class for"):
                call()
        return

    from music21 import converter

    one = converter.parse("tinyNotation: 4/4 a4 b c d  e2 f2")
    two = converter.parse("tinyNotation: 4/4 a4 b c e  e2 f2")

    merged = m.mergeVariants(one, two, variantName="london")
    variants = list(merged.getElementsByClass(m21variant.Variant))
    assert len(variants) == 1
    # `list(...)`, because music21's own `Groups` is a list subclass whose
    # `__eq__` answers `False` against a plain list -- on its own
    # `mergeVariants` output as much as on this package's.
    assert list(variants[0].groups) == ["london"]
    # The whole measure that differs, which is what music21's own
    # `mergeVariants` keeps: it compares measure by measure, so the variant
    # is the measure the two readings disagree in and not the one note.
    assert [note.name for note in variants[0].containedSite.recurse().notes] == [
        "A",
        "B",
        "C",
        "E",
    ]
    # The reading it was merged into is unchanged.
    assert not list(one.getElementsByClass(m21variant.Variant))

    # Asked for in place, the variants go into the stream it was given and
    # nothing else of it moves.
    notes = list(one.recurse().notes)
    assert m.mergeVariants(one, two, variantName="london", inPlace=True) is None
    assert len(list(one.getElementsByClass(m21variant.Variant))) == 1
    assert list(one.recurse().notes)[0] is notes[0]

    # One reading added by hand, and said to stand in for four quarters.
    plain = converter.parse("tinyNotation: 4/4 a4 b c d  e2 f2")
    other = converter.parse("tinyNotation: 4/4 a4 b c e")
    m.addVariant(
        plain,
        0.0,
        other,
        variantName="draft",
        replacementQuarterLength=4.0,
    )
    added = list(plain.getElementsByClass(m21variant.Variant))
    assert len(added) == 1
    assert list(added[0].groups) == ["draft"]
    assert added[0].replacementQuarterLength == 4.0

    # music21 leaves every variant exactly as it is where they are named,
    # both branches of its own test skipping, so the stream comes back
    # unchanged.
    before = [v.lengthType for v in plain.getElementsByClass(m21variant.Variant)]
    named = m.makeAllVariantsReplacements(plain, variantNames=["draft"])
    assert [v.lengthType for v in named.getElementsByClass(m21variant.Variant)] == before


def test_a_piece_is_measured_by_every_feature_extractor():
    """What each extractor reads of a piece, and the table a set of them
    makes, as music21's `features` module makes it.

    An extractor class is music21's, so `extractorsById` needs music21
    beside it; an extractor's bare id does instead, which is how a wheel
    on its own names one."""
    line = m.from_tiny_notation("4/4 c4 d e2")

    # Where in its library the extractor of a name stands.
    assert m.getIndex("Range") == (61, "jsymbolic")
    assert m.getIndex("Tonal Certainty", "native") == (1, "native")
    assert m.getIndex("aBrandNewFeature!") is None

    # Every feature of the piece, each as a list of its own.
    every = m.allFeaturesAsList(line)
    assert every[2:5] == [[2], [2], [1.0]]
    assert len(every) > 85

    # One extractor over a stream of this package's own.
    one = m.Stream()
    one.append(m.Note("A4"))
    assert m.vectorById(one, "p20") == [1.0] + [0.0] * 11
    assert m.vectorById(one, "not an extractor") is None

    # A table of two extractors over one piece.
    ds = m.DataSet(classLabel="Composer")
    ds.addFeatureExtractors(["P20", "R31"])
    assert ds.getClassLabel() == "Composer"
    assert ds.getAttributeLabels()[-3:] == [
        "Initial_Time_Signature_0",
        "Initial_Time_Signature_1",
        "Composer",
    ]
    # The id's column says nothing of itself and the class's is discrete.
    assert ds.getDiscreteLabels()[:2] == [None, False]
    assert ds.getDiscreteLabels()[-2:] == [True, True]
    assert ds.getClassPositionLabels()[-2:] == [False, True]

    ds.addData(line, classValue="Bach", id="a line")
    ds.process()
    (row,) = ds.getFeaturesAsList()
    # A space in an id is written as an underscore, as music21 writes it.
    assert row[0] == "a_line"
    assert row[-1] == "Bach"
    assert row[-3:-1] == [4, 4]
    assert ds.getUniqueClassValues() == ["Bach"]
    # Asked for unconcatenated, each feature is a list of its own.
    (shaped,) = ds.getFeaturesAsList(concatenateLists=False)
    assert shaped[-2] == [4, 4]
    assert ds._getOutputFormatFromFilePath("junk") is None
    # A wheel on its own writes the table with the crate's own writer,
    # which `features_parity` holds to music21's byte for byte.
    assert ds.getString("csv").splitlines() == [
        "Identifier,Pitch_Class_Distribution_0,Pitch_Class_Distribution_1,"
        "Pitch_Class_Distribution_2,Pitch_Class_Distribution_3,"
        "Pitch_Class_Distribution_4,Pitch_Class_Distribution_5,"
        "Pitch_Class_Distribution_6,Pitch_Class_Distribution_7,"
        "Pitch_Class_Distribution_8,Pitch_Class_Distribution_9,"
        "Pitch_Class_Distribution_10,Pitch_Class_Distribution_11,"
        "Initial_Time_Signature_0,Initial_Time_Signature_1,Composer",
        "a_line,0.3333333333333333,0.0,0.3333333333333333,0.0,"
        "0.3333333333333333,0.0,0.0,0.0,0.0,0.0,0.0,0.0,4,4,Bach",
    ]
    with pytest.raises(Exception, match="no output format"):
        ds.getString("nonsense")

    # Nothing can be added to a set with no class label at all.
    with pytest.raises(Exception, match="class label"):
        m.DataSet().addData(line)

    try:
        from music21 import features as m21features
    except ImportError:
        # An extractor class is music21's own, and so is the writer a
        # format names.
        for call in (
            lambda: m.extractorsById("p20"),
            lambda: m.extractorById("p20"),
            lambda: m.DataSet(classLabel="Composer").addData("bwv66.6"),
            lambda: m.allFeaturesAsList("bwv66.6"),
        ):
            with pytest.raises(Exception, match="needs music21 beside it"):
                call()
        return

    # With music21 beside it, an id names music21's own class -- which is
    # what a caller then calls with a stream.
    found = m.extractorsById(["p-20", "p 21"])
    assert [fe.id for fe in found] == ["P20", "P21"]
    assert m.extractorById("p20") is found[0]
    assert m.extractorsById("nothing of the sort") == []
    assert m.extractorById("nothing of the sort") is None

    # A set given one of music21's own extractor classes reads it as the
    # crate's row of the same id.
    named = m.DataSet(classLabel="Composer")
    named.addFeatureExtractors(m21features.jSymbolic.ChangesOfMeterFeature)
    assert named.getAttributeLabels() == ["Identifier", "Changes_of_Meter", "Composer"]
    # A music21 stream added to it is read by `install_into_music21`'s
    # classes, which this suite does not install; `doctest_features` is
    # where that half runs.


def test_a_feature_extractor_is_a_class_of_its_own():
    """Each of music21's feature extractors as its own class, over the
    crate's row for it.

    What the class says of the feature -- its id, its name, how many values
    it has -- is read off that row, and a wheel on its own answers all of
    it. The feature object `extract` hands back is music21's own `Feature`,
    which this package does not stand in for, so that one member says it
    needs music21 beside it."""
    assert m.PitchClassDistributionFeature.id == "P20"
    assert m.PitchClassDistributionFeature.name == "Pitch Class Distribution"
    assert m.PitchClassDistributionFeature.dimensions == 12
    assert m.PitchClassDistributionFeature.discrete is False
    assert m.PitchClassDistributionFeature.normalize is True
    assert m.PitchClassDistributionFeature.isSequential is True
    # The one extractor music21 reads out of order.
    assert m.DurationFeature.isSequential is False
    # A native extractor is a class here too.
    assert m.TonalCertainty.id == "K1"

    line = m.from_tiny_notation("4/4 c4 d e2")
    fe = m.ChangesOfMeterFeature(line)
    assert fe.getAttributeLabels() == ["Changes_of_Meter"]
    assert fe.data is line
    assert fe.stream is line

    labels = m.FifthsPitchHistogramFeature().getAttributeLabels()
    assert labels[0] == "Fifths_Pitch_Histogram_0"
    assert len(labels) == 12

    try:
        from music21 import features as m21features
    except ImportError:
        # The feature an extractor hands back is music21's own object.
        with pytest.raises(Exception, match="needs music21 beside it"):
            fe.extract()
        return

    assert fe.extract().vector == [0]
    assert isinstance(fe.extract(), m21features.base.Feature)


def test_a_score_is_reduced_to_the_chords_that_carry_it():
    """A score thinned out to the few chords that carry each measure, as
    music21's `analysis.reduceChords` thins it.

    The steps that take music21's own mutable timespan tree and walk it
    while editing it are refused with that reason: the crate reads a score
    as a list of spans of its own and runs every step over that list in
    music21's order, which is what `run` answers with."""
    cr = m.ChordReducer()
    assert cr.positionInMeasure == 0
    assert cr.numberOfElementsInMeasure == 0

    # How much each chord of a measure weighs, keyed by the classes it
    # sounds -- a chord struck twice weighing as one.
    first = m.Chord("C4 E4 G4 C5")
    first.quarterLength = 2.0
    second = m.Chord("C4 E4 F4 B4")
    third = m.Chord("C4 E4 G4 C5")
    chords = [first, second, third]
    weights = cr.computeMeasureChordWeights(chords)
    assert weights == {(0, 4, 7): 3.0, (0, 11, 4, 5): 1.0}
    # Consonance counts for a lot, and the last chord of the measure
    # counts as a full beat.
    scored = cr.computeMeasureChordWeights(chords, weightAlgorithm=cr.qlbsmpConsonance)
    assert scored[(0, 4, 7)] == 3.0
    assert round(scored[(0, 11, 4, 5)], 1) == 0.1
    # Each of music21's four algorithms weighs one chord on its own.
    assert cr.quarterLengthOnly(first) == 2.0
    assert cr.quarterLengthBeatStrength(first) == 2.0
    assert cr.quarterLengthBeatStrengthMeasurePosition(first) == 2.0
    assert cr.qlbsmpConsonance(first) == 2.0

    # A step that wants music21's mutable timespan tree says so.
    for call in (
        lambda: cr.collapseArpeggios(None),
        lambda: cr.splitByBass(None),
        lambda: cr.fillBassGaps(None, None),
        lambda: cr.removeNonChordTones(None),
    ):
        with pytest.raises(Exception, match="mutable timespan tree"):
            call()

    # The reduction itself is of a score, as music21 refuses anything else.
    with pytest.raises(Exception, match="stream.Score"):
        cr.run(m.from_tiny_notation("4/4 c4 d e2"))


def _live_notes():
    return sum(isinstance(held, m.Note) for held in gc.get_objects())


def test_a_chords_pitches_are_handed_out_without_its_notes():
    """Asking a chord for its pitches, or its root, builds no notes: under
    `install_into_music21` each note runs music21's own object machinery,
    which is most of what a chord costs. The pitch objects are still the
    chord's own, and the notes built later carry the same ones."""
    gc.collect()
    before = _live_notes()
    chord = m.Chord("C4 E4 G4")
    pitches = chord.pitches
    assert chord.root() is pitches[0]
    assert chord.pitches[1] is pitches[1]
    assert _live_notes() == before

    # An edit through one reaches the chord.
    pitches[1].name = "E-"
    assert chord.commonName == "minor triad"

    # The notes, once built, hold the pitches already handed out, and the
    # pitch knows its note.
    notes = chord.notes
    assert [note.pitch for note in notes] == list(pitches)
    assert all(note.pitch is pitch for note, pitch in zip(notes, pitches))
    assert pitches[0]._client is notes[0]
    pitches[2].name = "G#"
    assert chord.pitchNames == ["C", "E-", "G#"]


def test_a_loose_pitch_reaches_its_note_when_asked_for_its_client():
    """music21 reads `pitch._client` to find the note around a pitch; a
    pitch handed out before its note was built answers the note."""
    chord = m.Chord("D4 F#4 A4")
    third = chord.pitches[1]
    client = third._client
    assert client.pitch is third
    assert chord.notes[1] is client


def test_has_element_of_class_warns_it_is_deprecated_once():
    """music21 deprecated `hasElementOfClass` in v11 and warns on the first
    call in a process, as its `common.deprecated` does. Run apart, since
    the warning is spent once given."""
    code = """
import warnings
import music21_rs as m
line = m.Stream()
line.append(m.Note("C4"))
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    first = line.hasElementOfClass("Note")
    second = line.hasElementOfClass("Rest")
print(first, second, len(caught))
print(caught[0].category.__name__)
print(caught[0].message)
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    lines = done.stdout.splitlines()
    assert lines[0] == "True False 1"
    # music21's own class where music21 is there, Python's where not.
    assert lines[1] in ("Music21DeprecationWarning", "DeprecationWarning")
    assert lines[2] == (
        "Stream.hasElementOfClass was deprecated on v11 and will disappear at "
        "or after v12. use `bool(s.getElementsByClass(className))` instead"
    )


def test_readers_hand_back_music21_streams_once_installed():
    """Once the wheel is installed into music21 its readers hand back
    music21's own streams, so a note answers its offset and the score can
    be written and nested as music21's; before that, the wheel's own."""
    pytest.importorskip("music21.stream")
    code = """
import music21_rs as m
from music21 import stream
before = m.from_tiny_notation("3/4 c4 d e f g a")
print(isinstance(before, stream.Stream))
m.install_into_music21()
part = m.from_tiny_notation("3/4 c4 d e f g a")
print(isinstance(part, stream.Stream))
print([float(n.offset) for n in part.recurse().notes])
print(part.recurse().notes[4].measureNumber, part.recurse().notes[4].beat)
print(len(stream.Score([part]).parts))
print("<score-partwise" in m.to_musicxml(part))
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.splitlines() == [
        "False",
        "True",
        "[0.0, 1.0, 2.0, 0.0, 1.0, 2.0]",
        "2 2.0",
        "1",
        "True",
    ]


def test_a_blocked_member_says_it_is_not_ported():
    """A member music21 has and the wheel has not ported stays absent once
    installed, and says why, so it is not taken for a typo."""
    pytest.importorskip("music21.stream")
    code = """
import music21_rs as m
m.install_into_music21()
from music21 import roman
numeral = roman.RomanNumeral("V7")
print(hasattr(numeral, "findFigure"))
try:
    numeral.findFigure
except AttributeError as error:
    print(error)
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.splitlines() == [
        "False",
        "'RomanNumeral' object has no attribute 'findFigure': "
        "not ported to music21-rs",
    ]


def test_a_slur_read_once_installed_joins_its_own_notes():
    """music21's measures hold their barlines among their elements, so a
    slur's notes are found past them; and the slur is one music21's
    streams hold."""
    pytest.importorskip("music21.stream")
    code = """
import music21_rs as m
from music21 import bar, note, spanner, stream
part = stream.Part()
for number, names in enumerate([["C4", "D4", "E4", "F4"], ["G4", "A4", "B4", "C5"]]):
    measure = stream.Measure(number=number + 1)
    measure.append([note.Note(name) for name in names])
    measure.rightBarline = bar.Barline("double")
    part.append(measure)
notes = list(part.recurse().notes)
part.insert(0, spanner.Slur(notes[2], notes[5]))
written = m.to_musicxml(stream.Score([part]))
m.install_into_music21()
read = m.from_musicxml(written)
slur = read.recurse().getElementsByClass("Slur").first()
print([n.nameWithOctave for n in slur.getSpannedElements()])
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.splitlines() == ["['E4', 'A4']"]


def test_words_read_once_installed_stand_in_music21s_streams():
    """Words over the staff read after installing go into music21's own
    streams, which hold nothing that is not a `Music21Object`."""
    pytest.importorskip("music21.stream")
    code = """
import music21_rs as m
from music21 import expressions, note, stream
measure = stream.Measure(number=1)
measure.append(note.Note("C4", quarterLength=4))
measure.insert(0, expressions.TextExpression("cresc."))
written = m.to_musicxml(stream.Score([stream.Part([measure])]))
m.install_into_music21()
read = m.from_musicxml(written)
print([t.content for t in read.recurse().getElementsByClass("TextExpression")])
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.splitlines() == ["['cresc.']"]


def test_a_flag_is_read_for_its_truth_as_music21_reads_it():
    """music21 asks `if inPlace:`, so 1, 0 and None do for True and False;
    a string is refused, `'False'` being true in Python."""
    chord = m.Chord("G4 E4 C4")
    assert chord.sortDiatonicAscending(inPlace=1) is None
    assert [p.nameWithOctave for p in chord.pitches] == ["C4", "E4", "G4"]

    pitch = m.Pitch("C4")
    moved = pitch.transpose(2, inPlace=0)
    assert (moved.nameWithOctave, pitch.nameWithOctave) == ("D4", "C4")
    assert pitch.transpose(2, inPlace=None).nameWithOctave == "D4"

    with pytest.raises(TypeError):
        pitch.transpose(2, inPlace="False")


def test_a_score_read_once_installed_keeps_its_metadata():
    """music21's `metadata` setter takes only music21's own `Metadata`, so
    one the wheel built was dropped without a word once installed."""
    pytest.importorskip("music21.stream")
    code = """
import music21_rs as m
m.install_into_music21()
read = m.from_abc("X:1\\nT:Little Tune\\nC:Somebody\\nM:2/4\\nL:1/4\\nK:C\\nC D|E F|]\\n")
print(read.metadata.title, read.metadata.composer)
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.splitlines() == ["Little Tune Somebody"]


@pytest.mark.parametrize("extra", ["RehearsalMark", "Segno", "SystemLayout"])
def test_a_slur_joins_its_notes_past_what_the_wheel_drops(extra):
    """Something the wheel has no class for, standing before a slur, is
    left out of what a reader hands back; the slur still joins its own
    notes rather than the ones a place further on."""
    music21_stream = pytest.importorskip("music21.stream")
    from music21 import expressions, layout, note, repeat, spanner
    from music21.musicxml.m21ToXml import GeneralObjectExporter

    made = {
        "RehearsalMark": lambda: expressions.RehearsalMark("A"),
        "Segno": repeat.Segno,
        "SystemLayout": lambda: layout.SystemLayout(isNew=True),
    }[extra]
    first = music21_stream.Measure(number=1)
    first.insert(0, made())
    first.append([note.Note(name) for name in ("C4", "E4", "G4", "B4")])
    second = music21_stream.Measure(number=2)
    second.append([note.Note(name) for name in ("D5", "F5", "A5", "C6")])
    part = music21_stream.Part([first, second])
    part.insert(0, spanner.Slur(first.notes[1], second.notes[1]))
    written = GeneralObjectExporter(music21_stream.Score([part])).parse().decode()

    read = m.from_musicxml(written)
    slurs = [e for e in read.recurse() if type(e).__name__ == "Slur"]
    assert [n.nameWithOctave for n in slurs[0].getSpannedElements()] == ["E4", "F5"]


def test_a_humdrum_comment_read_once_installed_is_music21s():
    """A `!!` line read once installed stands in the score as music21's
    own `GlobalComment`, where music21's reader puts it."""
    pytest.importorskip("music21.stream")
    code = """
import music21_rs as m
m.install_into_music21()
read = m.from_humdrum("!!first\\n**kern\\n*M2/4\\n=1\\n4c\\n4d\\n==\\n*-\\n")
said = [e for e in read if type(e).__name__ == "GlobalComment"]
print([(e.comment, float(e.offset), e.priority) for e in said])
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    assert done.stdout.splitlines() == ["[('first', 0.0, -37)]"]


def test_a_key_signatures_count_and_flag_are_set_apart():
    """music21 14acfa0f2: `sharps` is always an int and `isNonTraditional`
    a flag of its own, so writing pitches over a signature leaves it
    traditional, and a flagged one still names a key and transposes."""
    two = m.KeySignature(2)
    two.alteredPitches = ["E-"]
    assert (two.sharps, two.isNonTraditional) == (2, False)
    assert repr(two) == "<music21.key.KeySignature of 2 sharps>"

    flagged = m.KeySignature(2)
    flagged.isNonTraditional = True
    flagged.alteredPitches = ["B-", "F#"]
    assert repr(flagged) == "<music21.key.KeySignature of pitches: [B-, F#]>"
    assert flagged.asKey().tonic.name == "D"
    moved = flagged.transpose("M2")
    assert (moved.sharps, moved.isNonTraditional) == (4, True)
    assert flagged != m.KeySignature(2)

    with pytest.raises(TypeError):
        m.sharpsToPitch(None)


def test_a_chords_per_note_accessors_warn_they_are_deprecated():
    """music21 2e5a51ae7 deprecated ten per-note accessors of `Chord` in
    v11, each warning once per process. Run apart, since a warning is
    spent once given."""
    code = """
import warnings
import music21_rs as m
c = m.Chord("C4 E4 G4")
p = c.pitches[0]
calls = [
    lambda: c.getColor(p), lambda: c.getNotehead(p),
    lambda: c.getNoteheadFill(p), lambda: c.getStemDirection(p),
    lambda: c.getTie(p), lambda: c.setColor("red", p),
    lambda: c.setNotehead("x", p), lambda: c.setNoteheadFill(True, p),
    lambda: c.setStemDirection("up", p), lambda: c.setTie("start", p),
]
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    for call in calls:
        call()
        call()
for warning in caught:
    print(warning.message)
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    lines = done.stdout.splitlines()
    assert len(lines) == 10
    assert lines[0] == (
        "Chord.getColor was deprecated on v11 and will disappear at or after "
        "v12. read a component color via c[query].style.color"
    )
    assert lines[9] == (
        "Chord.setTie was deprecated on v11 and will disappear at or after "
        "v12. index the component and set .tie, e.g. c[query].tie = ..."
    )


def test_a_chord_is_indexed_by_int_name_or_pitch_only():
    """music21 f9f96d9a1: a chord's key is an int, a pitch name with its
    octave, or a Pitch. A Note, or anything else, is a KeyError."""
    chord = m.Chord("C#4 D-4 G4")
    assert chord[1].pitch.name == "D-"
    assert chord["g4"].pitch.name == "G"
    assert chord[m.Pitch("C#4")].pitch.name == "C#"
    for key in (m.Note("C#4"), None):
        with pytest.raises(KeyError):
            chord[key]
    with pytest.raises(KeyError):
        chord[m.Note("G4")] = "A4"
    chord[m.Pitch("G4")] = "A4"
    assert chord.pitchNames == ["C#", "D-", "A"]


def test_a_chord_symbol_in_root_position_drops_a_stored_inversion():
    """music21 c02ed8926: re-realizing a symbol whose bass is its root
    clears a stored inversion, which `inversion(None)` meant to do since
    2012 and never did."""
    symbol = m.ChordSymbol("C")
    symbol.setInversion(2, transpose=False, inPlace=True)
    symbol.addChordStepModification(m.ChordStepModification("add", 9))
    assert symbol.inversion() == 0


def test_a_length_that_is_no_number_is_refused_as_music21_refuses_it():
    """music21 71c384b69: an unlinked duration refuses a length that is not
    finite with a ValueError; a linked one fails turning it into a ratio."""
    for value, linked, raised in (
        (math.inf, False, ValueError),
        (math.nan, False, ValueError),
        (-math.inf, False, ValueError),
        (math.inf, True, OverflowError),
        (math.nan, True, ValueError),
    ):
        length = m.Duration()
        length.linked = linked
        with pytest.raises(raised):
            length.quarterLength = value


def test_an_abstract_scale_says_its_mode_as_music21_does():
    """music21 a0358751c: every abstract scale has a `mode`. A diatonic one
    names its mode, an octatonic one keeps the number it was built with,
    and the rest have none."""
    assert m.DorianScale("C4").abstract.mode == "dorian"
    assert m.OctatonicScale("C4", mode=1).abstract.mode == 1
    assert m.OctatonicScale("C4", mode=2).abstract.mode == 2
    for made in (
        m.OctatonicScale("C4"),
        m.HarmonicMinorScale("C4"),
        m.WholeToneScale("C4"),
        m.RagMarwa("C4"),
    ):
        assert made.abstract.mode is None


@pytest.mark.parametrize(
    "tonic, pitches, ninth",
    [
        ("C", "C4 D-4 E-4 F-4 G-4 G4 A4 B-4 C5", "C4"),
        ("F#", "F#4 G4 A4 B-4 C5 D-5 E-5 F-5 G-5", "F#4"),
        ("E-", "E-4 F-4 G-4 G4 A4 B-4 C5 D-5 E-5", "E-4"),
    ],
)
def test_the_first_octatonic_mode_closes_on_its_octave(tonic, pitches, ninth):
    """music21's mode 1 starts with the half step and stacks its steps to
    the octave; its ranged realization starts on C whatever the tonic."""
    scale = m.OctatonicScale(tonic, mode=1)
    assert " ".join(str(p) for p in scale.pitches) == pitches
    ranged = " ".join(str(p) for p in scale.getPitches("C2", "C3"))
    assert ranged == "C2 D-2 E-2 F-2 G-2 G2 A2 B-2 C3"
    assert str(scale.pitchFromDegree(9)) == ninth


def test_add_variant_warns_for_its_old_keyword():
    """music21 v11: `addVariant`'s old `replacementDuration` warns on every
    call, and giving it beside `replacementQuarterLength` is a TypeError.
    Both are checked before music21 is needed."""
    one = m.Stream()
    with pytest.warns(Warning, match="'replacementDuration' is deprecated"):
        with pytest.raises(TypeError, match="received both"):
            m.addVariant(
                one, 0.0, None, replacementQuarterLength=2.0, replacementDuration=1.0
            )


def test_figure_tuples_are_named_as_music21_names_them():
    """music21 v11 made a figure a `FigureTuple` (degFromRefPitch, alter,
    prefix), built by `FigureTuple.fromPitchAndReference`; `figureTupleSolo`
    is its deprecated alias."""
    made = m.FigureTuple.fromPitchAndReference(
        m.Pitch("A-3"), m.Key("C"), m.Pitch("F#2")
    )
    assert repr(made) == "FigureTuple(degFromRefPitch=3, alter=-1.0, prefix='b')"
    assert made.degFromRefPitch == 3

    first = m.figureTuples(m.Chord(["F#2", "D3"]), m.Key("C"))[0]
    assert type(first).__name__ == "PitchFigureTuple"
    assert (first.degFromRefPitch, first.prefix, first.pitch.name) == (1, "#", "F#")

    flat_six = m.FigureTuple(degFromRefPitch=6, alter=-1, prefix="")
    corrected = m.correctRNAlterationForMinor(flat_six, m.Key("c"))
    assert repr(corrected) == "FigureTuple(degFromRefPitch=6, alter=-1, prefix='b')"
    major = m.FigureTuple(2, 0.0, "")
    assert m.correctRNAlterationForMinor(major, m.Key("C")) is major


def test_figure_tuple_solo_warns_it_is_deprecated():
    code = """
import warnings
import music21_rs as m
with warnings.catch_warnings(record=True) as caught:
    warnings.simplefilter("always")
    made = m.figureTupleSolo(m.Pitch("A-4"), m.Key("C"), m.Pitch("C4"))
    m.figureTupleSolo(m.Pitch("A-4"), m.Key("C"), m.Pitch("C4"))
print(repr(made))
print(len(caught), caught[0].message)
"""
    done = subprocess.run(
        [sys.executable, "-c", code], capture_output=True, text=True, check=True
    )
    lines = done.stdout.splitlines()
    assert lines[0] == "FigureTuple(degFromRefPitch=6, alter=-1.0, prefix='b')"
    assert lines[1] == (
        "1 figureTupleSolo was deprecated on v11 and will disappear at or after "
        "v12. use FigureTuple.fromPitchAndReference() instead"
    )


def test_beat_progress_is_a_fraction_where_music21s_is():
    from fractions import Fraction

    six_eight = m.TimeSignature("6/8")
    assert six_eight.getBeatProgress(Fraction(5, 3)) == (2, Fraction(1, 6))
    assert six_eight.getBeatProgress(1.0) == (1, 1.0)


def test_meter_spans_are_fractions_where_music21s_are():
    from fractions import Fraction

    thirds = m.MeterSequence("1/12+1/12+1/12")
    assert thirds.offsetToSpan(0.4) == (Fraction(1, 3), Fraction(2, 3))
    assert isinstance(thirds.offsetToSpan(0.4)[0], Fraction)
    spans = thirds.getLevelSpan(0)
    assert spans[1] == (Fraction(1, 3), Fraction(2, 3))
    assert all(isinstance(end, Fraction) for _, end in spans[:2])
    assert spans[2][1] == 1.0


def test_clercq_temperley_text_that_is_no_song_is_refused():
    """music21 b30248a6a: the converter refuses data with no barline or no
    `S:` rule, as a `CTSongException`, rather than opening it as a file."""
    with pytest.raises(Exception, match='must contain "\\|" and "S:"') as raised:
        m.from_clercq_temperley("S: [C] I IV V I\n")
    assert type(raised.value).__name__ == "CTSongException"


def test_a_quartets_key_given_as_a_name_is_handed_back_as_a_key():
    """music21 reads `VoiceLeadingQuartet(..., key='C')` through its key
    setter, so the name comes back as the Key it names."""
    notes = [m.Note(name) for name in ("C4", "D4", "A3", "B3")]
    for given in ({"key": "C"}, {"analyticKey": "C"}):
        quartet = m.VoiceLeadingQuartet(*notes, **given)
        assert type(quartet.key).__name__ == "Key"
        assert quartet.key.tonic.name == "C"


def test_an_accidental_is_not_named_in_the_crates_own_flats():
    """The crate writes a double flat `bb`; music21's table has `b` for a
    flat and no `bb`, so the wheel refuses it as music21 does."""
    assert m.Accidental("b").name == "flat"
    for name in ("bb", "BB"):
        with pytest.raises(Exception, match="not a supported accidental type"):
            m.Accidental(name)


def test_a_duration_is_set_only_from_a_duration():
    """music21's `duration` setter reads `quarterLength` off what it is
    given and raises TypeError for anything that has none: a bare number
    is not a duration."""
    for made in (m.Note("C4"), m.Chord("C4 E4"), m.Rest()):
        with pytest.raises(TypeError, match="this must be a Duration object, not 3"):
            made.duration = 3
        made.duration = m.Duration(2.0)
        assert made.quarterLength == 2.0


def test_set_inversion_reads_its_number_through_int():
    """music21's `setInversion` reads the inversion through `int()`, so a
    numeral written as text works, and a copy says it came from it."""
    triad = m.Chord("C4 E4 G4")
    turned = triad.setInversion("1")
    assert [p.nameWithOctave for p in turned.pitches] == ["E4", "G4", "C5"]
    with pytest.raises(Exception, match="Inversion must be an integer"):
        triad.setInversion("first")


def test_a_streams_offsets_are_fractions_where_music21s_are():
    """music21 answers offsets through `opFrac`: a triplet note stands at
    `Fraction(1, 3)`, not at the float that only nearly says it."""
    from fractions import Fraction

    line = m.from_tiny_notation("4/4 trip{c8 d e} f4")
    measure = line.getElementsByClass("Measure")[0]
    notes = list(measure.notes)
    assert measure.elementOffset(notes[1]) == Fraction(1, 3)
    assert isinstance(measure.elementOffset(notes[1]), Fraction)
    assert measure.elementOffset(notes[3]) == 1.0
    assert isinstance(measure.highestOffset, float)
    loose = m.Stream()
    loose.insert(Fraction(1, 3), m.Note("C4"))
    assert isinstance(loose.lowestOffset, Fraction)
    assert isinstance(loose.highestTime, Fraction)
