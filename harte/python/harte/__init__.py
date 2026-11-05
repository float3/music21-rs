"""Harte chord notation, read into music21-rs chords.

A replacement for harte-library: the same modules and names, with the label
read by the `harte` Rust crate and the chord a music21-rs `Chord`.

>>> from harte.harte import Harte
>>> Harte("Bb:min7/b3").pitches
(<music21.pitch.Pitch B-4>, <music21.pitch.Pitch D-3>, <music21.pitch.Pitch F5>, <music21.pitch.Pitch A-5>)
"""

from harte.harte import Harte
from harte.interval import HarteInterval

__all__ = ["Harte", "HarteInterval"]
