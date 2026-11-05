"""harte-library's `HarteInterval`: an interval read from a Harte degree."""

from music21_rs import Interval, IntervalException

from harte.utils import convert_interval


class HarteInterval(Interval):
    """A music21-rs `Interval` read from a Harte degree: `b6` is a minor
    sixth. A degree that is not one raises `IntervalException`."""

    def __init__(self, harte_interval: str, **keywords) -> None:
        self._harte_interval = harte_interval
        try:
            self._converted_interval = convert_interval(harte_interval)
        except ValueError as value_error:
            raise IntervalException("Harte Interval cannot be converted") from value_error
        super().__init__(self._converted_interval, **keywords)

    def __eq__(self, other) -> bool:
        if isinstance(other, HarteInterval):
            return self._harte_interval == other._harte_interval
        return self._harte_interval == other

    __hash__ = None

    def __repr__(self) -> str:
        return f"HarteInterval({self._harte_interval})"

    def __str__(self) -> str:
        return self._harte_interval
