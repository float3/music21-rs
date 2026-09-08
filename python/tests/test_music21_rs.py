"""The `music21_rs` wheel, checked against music21's own answers.

These are not a second copy of the doctest suite — `python-parity` runs
music21's docstrings verbatim and is the real measure. What is here is the
shape of the package as an importable thing: that every name is exported,
that the classes carry music21's reprs and module strings, and that the
handful of behaviours a caller reaches for first are right.
"""

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
