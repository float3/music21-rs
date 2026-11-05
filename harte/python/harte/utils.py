"""harte-library's helpers for Harte degrees."""

from typing import List

from music21_rs.music21_rs import _HarteLabel

from harte.mappings import SHORTHAND_DEGREES


def convert_interval(harte_interval: str) -> str:
    """The music21 interval a Harte degree stands for: `b3` is `m3`, `#11` is
    `A4`. A degree that is not one raises `ValueError`."""
    return _HarteLabel.convert_interval(harte_interval)


def degree_to_sort_key(degree: str) -> float:
    """Where a degree sorts among the others: by its number, a flat just
    below it and a sharp just above."""
    return _HarteLabel.degree_sort_key(degree)


def unwrap_shorthand(harte_shorthand: str, harte_degrees: list) -> List[str]:
    """The degrees of a shorthand together with `harte_degrees`, ordered by
    their number. Deprecated in harte-library; `Harte.unwrap_shorthand` is
    what to use."""
    assert harte_shorthand in SHORTHAND_DEGREES, "The Harte shorthand is not valid. "
    degrees = list(set(SHORTHAND_DEGREES[harte_shorthand] + harte_degrees))
    degrees.sort(key=lambda x: [k for k in x if k.isdigit()][0])
    return degrees
