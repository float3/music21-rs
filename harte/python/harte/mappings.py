"""harte-library's tables of Harte shorthands and the degrees they stand for."""

from collections import OrderedDict

from music21_rs.music21_rs import _HarteLabel

# Each shorthand and the degrees it stands for, the root included.
SHORTHAND_DEGREES = {
    name: list(degrees) for name, degrees in _HarteLabel.SHORTHAND_DEGREES
}

# The degrees `Harte.prettify` folds into each shorthand, in the order tried.
DEGREE_SHORTHAND_MAP = OrderedDict(
    (tuple(degrees), name) for degrees, name in _HarteLabel.DEGREE_SHORTHANDS
)

# The common shorthands and their degrees above the root.
SHORTHAND_DEGREE_MAP = {
    "maj": ("3", "5"),
    "min": ("b3", "5"),
    "aug": ("3", "#5"),
    "dim": ("b3", "b5"),
    "maj7": ("3", "5", "7"),
    "min7": ("b3", "5", "b7"),
    "7": ("3", "5", "b7"),
    "minmaj7": ("b3", "5", "7"),
    "dim7": ("b3", "b5", "bb7"),
    "hdim7": ("b3", "b5", "b7"),
    "maj6": ("3", "5", "6"),
    "min6": ("b3", "5", "6"),
    "9": ("3", "5", "b7", "9"),
    "maj9": ("3", "5", "7", "9"),
    "min9": ("b3", "5", "b7", "9"),
    "sus4": ("4", "5"),
}
