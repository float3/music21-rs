# music21-rs for Python

[![PyPI](https://img.shields.io/pypi/v/music21-rs.svg)](https://pypi.org/project/music21-rs/)
[![music21 members in the wheel](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-wheel.json)](https://hilll.dev/music21-rs/reports/#features)
[![music21 doctests passing](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-doctests.json)](https://hilll.dev/music21-rs/reports/#doctests)

`music21_rs` is [music21](https://github.com/cuthbertLab/music21)'s analysis
classes, `Pitch`, `Interval`, `Chord`, `Note`, `Duration`, `Key`, `Scale`,
`RomanNumeral`, `TimeSignature`, `ToneRow` and the rest, ported to Rust and
handed back to Python with music21's names, keyword arguments, properties and
`repr`. It is not a rewrite of music21: streams, parsing, notation output and
the corpus stay music21's. It is the part that answers musical questions, made
faster and checked against the original.

```bash
pip install music21-rs
```

```python
import music21_rs as m

chord = m.Chord("C4 E4 G4 B-4")
chord.commonName            # 'dominant seventh chord'
chord.root()                # <music21.pitch.Pitch C4>
chord.forteClass            # '4-27B'
chord.transpose("M3")       # <music21.chord.Chord E4 G#4 B4 D5>

m.RomanNumeral("viio7", m.Key("c")).pitches
# (<music21.pitch.Pitch B4>, <music21.pitch.Pitch D5>, <music21.pitch.Pitch F5>, <music21.pitch.Pitch A-5>)

m.TimeSignature("6/8").getAccentWeight(1.5)   # 0.5
```

The classes report music21's own module strings, `<music21.pitch.Pitch C4>`,
because they are meant to stand in for music21's.

## Running a music21 program on it

`install_into_music21()` replaces music21's classes with these, in the music21
that is installed, so a program written for music21 runs on the Rust code
without a line changed:

```python
import music21_rs
music21_rs.install_into_music21()   # before anything imports the classes

from music21 import chord, roman, key
chord.Chord("C4 E4 G4").commonName          # 'major triad', out of Rust
roman.RomanNumeral("V7", key.Key("G")).pitches
```

It patches live modules, so it changes music21 for the whole process, and
nothing calls it for you. Call it before the program does
`from music21.chord import Chord`, since that binds whatever it finds at
import time. For a test suite, a `conftest.py` is early enough.

The installed classes are real `Music21Object`s: music21 puts them in
streams, finds them with `getElementsByClass`, freezes and thaws them, and
exports them to MusicXML as it would its own. What music21 does with them
that the crate does not model, an offset in a stream, editorial marks, a
style, stays music21's.

## How much of music21 this is

Three measurements, each redone on every push and published on the
[reports page](https://hilll.dev/music21-rs/reports/):

- **Members.** Every public method of the music21 classes the crate ports is
  read out of music21 itself and matched against the port. The first badge is
  the share reachable from this wheel; the crate itself ports more, since
  some of it has no Python-side use yet.
- **Doctests.** Nineteen music21 modules run their own docstrings with these
  classes swapped in. Seventeen pass every example: `pitch`, `interval`,
  `chord`, `chord.tables`, `note`, `duration`, `key`, `scale`, `roman`,
  `harmony`, `serial`, `beam`, `tie`, `volume`, `figuredBass.notation`,
  `tempo` and `voiceLeading`. `sieve` and `meter.base` do not, for want of
  sieve compression and the `MeterSequence` tree, and neither is installed.
- **music21's own suite.** All of music21's tests run on music21, then with
  the crate installed over it, then with this wheel installed over it. A test
  that fails only under the port fails the build. Two divergences are expected
  and named.

A fourth: [harte-library](https://github.com/andreamust/harte-library), a
chord grammar built on music21 by someone who had never heard of this
project, runs its 8,116 tests on music21 and on this and the failures have to
match. They do.

## Speed

The reports page carries the benchmark. Constructing a `Pitch`, `Note`,
`Interval` or `Chord` is two to sixteen times faster than music21's; chord
analysis, `commonName`, `forteClass` and the rest on a fresh chord, is about
five times faster. On a chord that has already answered, music21's memoised
result and this wheel's cache cost the same. Across music21's whole test
suite the median test runs at the same speed either way, because most of what
a music21 test does is music21's own code.

## What is not here

- Anything music21 does with a stream, a score or a file: parsing, writing,
  the corpus, `show()`.
- `Tuplet`, `AbstractScale`, `Sieve`, `TimeSignature`'s `MeterSequence` and
  `style.Style` are provided but not installed over music21's, because
  music21's own are richer.
- Behaviour the crate does not have is left to fail rather than patched in
  Python. The reports page lists every missing member with the reason where
  one was decided.

## Building

```bash
uvx maturin build --release --manifest-path python/Cargo.toml
uvx maturin develop --manifest-path python/Cargo.toml   # into the active venv
pytest python/tests -q
```

The wheel's Rust half is the `music21-rs` crate; its version and the crate's
are pinned to each other by a test.

## Licence

AGPL-3.0-only, as the crate is.
