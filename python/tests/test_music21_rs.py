"""The `music21_rs` wheel, checked against music21's own answers.

These are not a second copy of the doctest suite — `python-parity` runs
music21's docstrings verbatim and is the real measure. What is here is the
shape of the package as an importable thing: that every name is exported,
that the classes carry music21's reprs and module strings, and that the
handful of behaviours a caller reaches for first are right.
"""

import gc
import copy
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


def test_a_reader_refuses_what_it_cannot_read():
    with pytest.raises(m.StreamException):
        m.from_mei("<html></html>")
