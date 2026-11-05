"""harte-library's `Harte`: a chord read from a Harte label."""

import sys
from typing import List, Optional

import music21_rs
from music21_rs.music21_rs import _HarteLabel


def _chord_class():
    """music21's `Chord` where `music21_rs.install_into_music21()` has put
    music21-rs's in its place, so a `Harte` goes in music21's streams; the
    wheel's own `Chord` otherwise."""
    music21_chord = sys.modules.get("music21.chord")
    installed = getattr(music21_chord, "Chord", None)
    if isinstance(installed, type) and issubclass(installed, music21_rs.Chord):
        return installed
    return music21_rs.Chord


class Harte(_chord_class()):
    """A music21-rs `Chord` read from a Harte label, such as `C:maj7/3`.

    The pitches stand in degree order above the root in octave 4, the bass an
    octave lower where it is not the root. `N` and `X` are an empty chord.
    A label that is not Harte notation raises `ChordException`.
    """

    def __init__(self, chord: str, **keywords):
        label = _HarteLabel(chord)
        super().__init__(**keywords)
        label.fill(self)
        self.chord = chord
        self._label = label

    def __deepcopy__(self, *args, **kwargs):
        return Harte(self.chord)

    def __reduce__(self):
        return (Harte, (self.chord,))

    def get_degrees(self) -> Optional[List[str]]:
        """The degrees written in parentheses, or `None` when there are none."""
        return self._label.degrees

    def get_midi_pitches(self) -> List[int]:
        """The MIDI numbers of the pitches, lowest first."""
        return sorted(pitch.midi for pitch in self.pitches)

    def multi_hot_encoding(self, transpose: bool = False) -> List[int]:
        """Which of the twelve pitch classes the chord sounds, one flag each,
        counted from C, or from the root with `transpose`."""
        pitch_classes = [pitch.pitchClass for pitch in self.pitches]
        if transpose:
            root = self.root().pitchClass
            pitch_classes = [(pitch_class - root) % 12 for pitch_class in pitch_classes]
        return [1 if i in pitch_classes else 0 for i in range(12)]

    def get_root(self) -> Optional[str]:
        """The root as written, `C#`, or `None` for no chord."""
        return self._label.root

    def get_bass(self) -> Optional[str]:
        """The bass as a degree above the root, `1` when none was written."""
        return self._label.bass

    def bass_is_root(self) -> bool:
        """Whether the bass is the root."""
        return self._label.bass_is_root()

    def contains_shorthand(self) -> bool:
        """Whether the label carries a shorthand."""
        return self._label.contains_shorthand()

    def get_shorthand(self) -> Optional[str]:
        """The shorthand, `maj7`, or `None` when there is none."""
        return self._label.shorthand

    def prettify(self) -> str:
        """The label rewritten with the largest shorthand its degrees contain:
        `C:(b3,5)` is `C:min`. A label no shorthand fits comes back as written."""
        return self._label.prettify()

    def unwrap_shorthand(self) -> Optional[List[str]]:
        """Every degree the chord sounds where the label has a shorthand, the
        written degrees where it has none, and `None` where it has neither."""
        return self._label.unwrapped

    def __eq__(self, other):
        """Two labels are equal when they sound the same chord: `C:maj` equals
        `C:(3,5)`."""
        if isinstance(other, Harte):
            return self._label == other._label
        return False

    __hash__ = None

    def __repr__(self):
        return f"Harte({self})"

    def __str__(self):
        return str(self._label)
