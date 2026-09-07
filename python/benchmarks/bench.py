"""Time `music21_rs` against `music21`, operation by operation.

Both are called through the same Python API with the same inputs, so what is
measured is the whole cost a caller pays: the interpreter's, the FFI's and
the implementation's. Every case is checked for agreement before it is timed
— a case whose two sides answer differently is reported and not timed, since
timing two different computations says nothing.

music21 caches analysis on the object that was asked, so most cases build a
fresh object each iteration: that is the cost of *doing* the analysis. The
cases marked "cached" reuse one object on purpose, to show what music21's
memoization buys once it is warm.

    python python/benchmarks/bench.py
    python python/benchmarks/bench.py --json out.json
"""

from __future__ import annotations

import argparse
import json
import platform
import sys
import time
from dataclasses import dataclass, field
from typing import Any, Callable

import music21
import music21_rs

from music21 import chord as m21chord
from music21 import interval as m21interval
from music21 import note as m21note
from music21 import pitch as m21pitch
from music21 import serial as m21serial


@dataclass
class Case:
    name: str
    group: str
    music21: Callable[[], Any]
    music21_rs: Callable[[], Any]
    #: How to compare the two answers when they are not plainly equal.
    normalise: Callable[[Any], Any] = lambda value: value
    notes: str = ""


CHORDS = ["C4 E4 G4", "D3 F#3 A3 C4", "B-2 D3 F3 A-3", "F#4 A4 C#5 E5", "C4 E-4 G-4 A4"]


def cases() -> list[Case]:
    """Every operation timed, paired between the two implementations."""
    out: list[Case] = []
    add = out.append

    # ---- construction ----------------------------------------------------
    add(Case("Pitch('C#4')", "construction",
             lambda: m21pitch.Pitch("C#4").nameWithOctave,
             lambda: music21_rs.Pitch("C#4").nameWithOctave))
    add(Case("Note('C#4')", "construction",
             lambda: m21note.Note("C#4").nameWithOctave,
             lambda: music21_rs.Note("C#4").nameWithOctave))
    add(Case("Interval('P5')", "construction",
             lambda: m21interval.Interval("P5").directedName,
             lambda: music21_rs.Interval("P5").directedName))
    add(Case("Chord('C4 E4 G4')", "construction",
             lambda: len(m21chord.Chord("C4 E4 G4").pitches),
             lambda: len(music21_rs.Chord("C4 E4 G4").pitches)))

    # ---- chord analysis, fresh object each time --------------------------
    for label, expression in [
        ("commonName", lambda c: c.commonName),
        ("root()", lambda c: c.root().nameWithOctave),
        ("inversion()", lambda c: c.inversion()),
        ("forteClass", lambda c: c.forteClass),
        ("primeForm", lambda c: list(c.primeForm)),
        ("intervalVector", lambda c: list(c.intervalVector)),
        ("isDominantSeventh()", lambda c: c.isDominantSeventh()),
        ("orderedPitchClasses", lambda c: list(c.orderedPitchClasses)),
    ]:
        add(Case(f"Chord.{label}", "chord analysis",
                 (lambda e=expression: [e(m21chord.Chord(text)) for text in CHORDS]),
                 (lambda e=expression: [e(music21_rs.Chord(text)) for text in CHORDS]),
                 notes="5 chords"))

    # The pattern a memo is for: several set-class questions of one chord,
    # each of which would otherwise repeat the same table search.
    def three_questions(builder):
        def ask():
            chord = builder("D3 F#3 A3 C4")
            return (chord.forteClass, list(chord.primeForm), list(chord.intervalVector))
        return ask

    add(Case("Chord: forteClass + primeForm + intervalVector", "chord analysis",
             three_questions(m21chord.Chord),
             three_questions(music21_rs.Chord),
             notes="one chord, three questions"))

    # ---- pitch and interval work -----------------------------------------
    add(Case("Pitch.transpose('M3')", "pitch",
             lambda: m21pitch.Pitch("C#4").transpose("M3").nameWithOctave,
             lambda: music21_rs.Pitch("C#4").transpose("M3").nameWithOctave))
    add(Case("Pitch.frequency", "pitch",
             lambda: round(m21pitch.Pitch("A4").frequency, 6),
             lambda: round(music21_rs.Pitch("A4").frequency, 6)))
    add(Case("Pitch.getEnharmonic()", "pitch",
             lambda: m21pitch.Pitch("C#4").getEnharmonic().nameWithOctave,
             lambda: music21_rs.Pitch("C#4").getEnharmonic().nameWithOctave))
    add(Case("Interval(p1, p2)", "pitch",
             lambda: m21interval.Interval(m21pitch.Pitch("C4"),
                                          m21pitch.Pitch("A-5")).directedName,
             lambda: music21_rs.Interval(music21_rs.Pitch("C4"),
                                         music21_rs.Pitch("A-5")).directedName))

    # ---- twelve-tone rows -------------------------------------------------
    row = list(range(12))
    add(Case("pcToToneRow(...).matrix()", "serial",
             lambda: str(m21serial.pcToToneRow(row).matrix())[:40],
             lambda: str(music21_rs.pcToToneRow(row).matrix())[:40]))
    add(Case("ToneRow.zeroCenteredTransformation", "serial",
             lambda: m21serial.pcToToneRow(row).zeroCenteredTransformation("I", 3).pitchClasses(),
             lambda: music21_rs.pcToToneRow(row).zeroCenteredTransformation("I", 3).pitchClasses()))

    # ---- the same questions on an object that has already answered them ---
    warm_m21 = m21chord.Chord("D3 F#3 A3 C4")
    warm_rs = music21_rs.Chord("D3 F#3 A3 C4")
    for label, expression in [
        ("commonName", lambda c: c.commonName),
        ("forteClass", lambda c: c.forteClass),
    ]:
        expression(warm_m21)
        expression(warm_rs)
        add(Case(f"Chord.{label} (cached)", "warm object",
                 (lambda e=expression: e(warm_m21)),
                 (lambda e=expression: e(warm_rs)),
                 notes="music21 memoizes"))

    return out


def measure(function: Callable[[], Any], *, seconds: float, repeats: int) -> float:
    """Nanoseconds per call, taking the best of `repeats` timed batches.

    The best batch is used rather than the mean: the true cost is bounded
    below, and everything above it is noise from the machine.
    """
    # calibrate: grow the batch until one takes long enough to time well
    batch = 1
    while True:
        start = time.perf_counter()
        for _ in range(batch):
            function()
        elapsed = time.perf_counter() - start
        if elapsed >= seconds / repeats or batch >= 1 << 22:
            break
        batch = max(batch * 2, int(batch * (seconds / repeats) / max(elapsed, 1e-9)))

    best = float("inf")
    for _ in range(repeats):
        start = time.perf_counter_ns()
        for _ in range(batch):
            function()
        best = min(best, (time.perf_counter_ns() - start) / batch)
    return best


def humanise(nanoseconds: float) -> str:
    if nanoseconds < 1_000:
        return f"{nanoseconds:,.0f} ns"
    if nanoseconds < 1_000_000:
        return f"{nanoseconds / 1_000:,.1f} us"
    return f"{nanoseconds / 1_000_000:,.2f} ms"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, default=0.4,
                        help="rough time to spend on each side of each case")
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--json", type=str, default=None)
    arguments = parser.parse_args()

    print(f"python {platform.python_version()} on {platform.platform()}")
    print(f"music21 {music21.__version__}  vs  music21_rs (release build)\n")

    results: list[dict[str, Any]] = []
    disagreed: list[tuple[str, Any, Any]] = []
    group = None
    for case in cases():
        left = case.normalise(case.music21())
        right = case.normalise(case.music21_rs())
        if left != right:
            disagreed.append((case.name, left, right))
            continue
        slow = measure(case.music21, seconds=arguments.seconds, repeats=arguments.repeats)
        fast = measure(case.music21_rs, seconds=arguments.seconds, repeats=arguments.repeats)
        if group != case.group:
            group = case.group
            print(f"{group.upper():<38} {'music21':>11} {'music21_rs':>11} {'speedup':>9}")
        print(f"  {case.name:<36} {humanise(slow):>11} {humanise(fast):>11} "
              f"{slow / fast:>8.1f}x")
        results.append({"case": case.name, "group": case.group, "notes": case.notes,
                        "music21_ns": slow, "music21_rs_ns": fast, "speedup": slow / fast})

    if results:
        speedups = sorted(r["speedup"] for r in results)
        middle = speedups[len(speedups) // 2]
        print(f"\n{len(results)} cases: median {middle:.1f}x, "
              f"range {speedups[0]:.1f}x to {speedups[-1]:.1f}x")
    if disagreed:
        print(f"\n{len(disagreed)} cases not timed, the two sides disagreeing:")
        for name, left, right in disagreed:
            print(f"  {name}: music21 {left!r} vs music21_rs {right!r}")

    if arguments.json:
        with open(arguments.json, "w", encoding="utf-8") as handle:
            json.dump({"music21": music21.__version__,
                       "python": platform.python_version(),
                       "platform": platform.platform(),
                       "results": results}, handle, indent=2)
        print(f"\nwrote {arguments.json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
