"""The `music21_rs` wheel, checked against music21's own answers.

These are not a second copy of the doctest suite — `python-parity` runs
music21's docstrings verbatim and is the real measure. What is here is the
shape of the package as an importable thing: that every name is exported,
that the classes carry music21's reprs and module strings, and that the
handful of behaviours a caller reaches for first are right.
"""

import gc
import os
import copy
import math
import pathlib
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
        assert type(pickle.loads(pickle.dumps(thing))) is type(thing)
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
    # This package's streams keep no neumes or barlines to write.
    assert m.to_volpiano(part) == "1---c-d-f-g-wE-"


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
    assert md["title"] == ("Maple Leaf Rag",)
    assert md.all() == (("composer", "Joplin, Scott"), ("title", "Maple Leaf Rag"))
    assert md.all(skipContributors=True) == (("title", "Maple Leaf Rag"),)

    md.number = "3"
    assert md.number == "3"
    assert md.onm == "3"
    # A name outside music21's vocabulary is no attribute at all.
    with pytest.raises(AttributeError):
        md.madeUp
    # The primitives music21 hands back are not here to hand back.
    with pytest.raises(Exception):
        md.all(returnPrimitives=True)


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


def test_a_stream_is_asked_what_classes_it_holds():
    flat = m.from_tiny_notation("tinyNotation: 4/4 c2 d4 e4").flatten()
    assert flat.hasElementOfClass("Note")
    assert flat.hasElementOfClass("Clef")
    assert not flat.hasElementOfClass("Dynamic")
    # The clef and the meter, where music21 also holds the final barline
    # it keeps among the elements and this package keeps on the measure.
    assert len(flat.getElementsNotOfClass("Note").elements) == 2
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
    ]
    # A count of semitones, which music21 reads as an interval too.
    assert outline(part.transpose(2))[3:] == [
        "Note D4 ql=1.0",
        "Note E4 ql=1.0",
        "Note F#4 ql=1.0",
        "Note G4 ql=1.0",
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
    ]
    assert outline(part.template(removeAll=True)) == ["[Measure]", "Rest  ql=4.0"]
    assert outline(part.template(fillWithRests=False)) == [
        "[Measure]",
        "TrebleClef  ql=0.0",
        "TimeSignature  ql=0.0",
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
