# music21-rs for Python

[![PyPI](https://img.shields.io/pypi/v/music21-rs.svg)](https://pypi.org/project/music21-rs/)
[![music21 members in the wheel](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-wheel.json)](https://hilll.dev/music21-rs/reports/#ported)
[![music21 doctests passing](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-doctests.json)](https://hilll.dev/music21-rs/reports/#doctests)

`music21_rs` is [music21](https://github.com/cuthbertLab/music21)'s analysis
classes, `Pitch`, `Interval`, `Chord`, `Note`, `Duration`, `Key`, `Scale`,
`RomanNumeral`, `TimeSignature`, `ToneRow` and others, implemented in Rust
and packaged for Python. The classes have music21's names, arguments,
properties and `repr`, and give music21's answers. The Rust half is the
[music21-rs](https://crates.io/crates/music21-rs) crate; the wheel needs no
Rust toolchain and no music21 to run on its own. Streams, parsing, notation
output and the corpus are not included; those remain music21's.

The [reports page](https://hilll.dev/music21-rs/reports/) has the full
numbers: every music21 method and whether it is ported, the doctest and test
suite results, benchmarks and sizes, refreshed on every push.

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

## Using it inside music21

`install_into_music21()` replaces the classes of an installed music21 with
these, so existing music21 code runs on the Rust implementation unchanged:

```python
import music21_rs
music21_rs.install_into_music21()   # before anything imports the classes

from music21 import chord, roman, key
chord.Chord("C4 E4 G4").commonName          # 'major triad', out of Rust
roman.RomanNumeral("V7", key.Key("G")).pitches
```

It patches the live modules for the whole process, so call it before any
`from music21.chord import Chord`; for a test suite, `conftest.py` is early
enough. The installed classes are `Music21Object` subclasses, so music21 can
hold them in streams, find them by class, pickle them and export them to
MusicXML.

## Coverage of music21

- 94% of the public methods of the ported music21 classes are reachable from
  this wheel.
- 17 of the 19 music21 modules whose doctests run against the port pass all
  of them: `pitch`, `interval`, `chord`, `chord.tables`, `note`, `duration`,
  `key`, `scale`, `roman`, `harmony`, `serial`, `beam`, `tie`, `volume`,
  `figuredBass.notation`, `tempo` and `voiceLeading`. `sieve` and
  `meter.base` are partial and are not installed over music21's.
- music21's own test suite gives the same results with this wheel installed
  over music21 as with music21 alone, apart from two documented differences.
- [harte-library](https://github.com/andreamust/harte-library), a third-party
  chord parser built on music21, gives identical results for its 8,116 tests
  on both.

`Tuplet`, `AbstractScale`, `Sieve` and `style.Style` are provided but not
installed over music21's, whose versions do more. Missing behaviour raises
rather than falling back to music21.

## Speed and size

Times per call, from the
[benchmark](https://hilll.dev/music21-rs/reports/#speedups) against music21
11 on Python 3.13.

| | music21 | music21_rs | speedup |
| --- | ---: | ---: | ---: |
| `Pitch('C#4')` | 1.55 us | 0.37 us | 4x |
| `Note('C#4')` | 4.4 us | 1.3 us | 3.4x |
| `Interval('P5')` | 7.5 us | 0.47 us | 16x |
| `Chord('C4 E4 G4')` | 16.2 us | 8.9 us | 1.8x |
| `Chord.commonName` | 526 us | 60 us | 8.8x |
| `Chord.forteClass` | 244 us | 53 us | 4.6x |
| `Pitch.transpose('M3')` | 28 us | 1.0 us | 28x |
| `Pitch.getEnharmonic()` | 22.7 us | 0.63 us | 36x |
| `ToneRow.zeroCenteredTransformation` | 274 us | 0.37 us | 741x |
| `pcToToneRow(...).matrix()` | 2.71 ms | 4.9 us | 547x |

Repeated queries on the same chord are cached in both and cost the same. Over
music21's own test suite the median test runs at the same speed, since most
of a music21 test is music21's own code.

The wheel is 1.7 MB, one compiled module with the chord tables and scale
definitions inside it and no dependencies. music21 installs 105 MB.

## Building

```bash
uvx maturin build --release --manifest-path python/Cargo.toml
uvx maturin develop --manifest-path python/Cargo.toml   # into the active venv
pytest python/tests -q
```

The wheel shares the crate's version number. Two further checks run it
against music21 itself, and need music21, `lark` and `pytest` installed
beside it:

```bash
# music21's own test suite, on music21, on the crate and on the wheel
cargo run --release -p xtask --features python -- music21-suite
# harte-library's test suite, on music21 and on the wheel
cargo run --release -p xtask -- downstream
```

## Credits and third-party data

`music21_rs` is released under the
[AGPL-3.0](https://github.com/float3/music21-rs/blob/master/LICENSE). It
ports behaviour from, and compiles in data from, the projects below, each
under its own licence.

### music21

The chord tables, scale definitions, time-signature behaviour and
chord-symbol kinds are derived from
[music21](https://github.com/cuthbertLab/music21), the Python library for
computational musicology by Michael Scott Asato Cuthbert and contributors,
licensed [BSD-3-Clause](https://github.com/cuthbertLab/music21/blob/master/LICENSE).
The wheel is built against music21 11.0.0b9, and its classes are checked
against that version's own doctests and test suite. Thanks to Michael Scott
Asato Cuthbert and all music21 contributors for the original library.

### harte-library

The crate's `harte` module, compiled into the wheel, is a port of
[harte-library](https://github.com/andreamust/harte-library) by Andrea
Poltronieri, licensed
[MIT](https://github.com/andreamust/harte-library/blob/main/LICENSE).

### The Scala scale archive

Thirteen of the tuning tables compiled in are transcribed from the
[Scala](https://www.huygens-fokker.org/scala/) scale archive as distributed
with music21, which includes it by kind permission of Manuel Op de Coul. The
archive itself, 3,994 scales in the crate's `scala-archive` feature, and the
62 [Plainsound Hexatone](https://github.com/PLAINSOUND/hexatone) scales
beside it are not in the wheel.

### The Xenharmonic Wiki

The mapping, generators, commas and scales of 95 regular temperaments from
the [Xenharmonic Wiki](https://en.xen.wiki), which is CC BY-SA, are compiled
in. Only the numbers are used.

### Contributed back

Fixes found while porting went upstream:

- [cuthbertLab/music21#1746](https://github.com/cuthbertLab/music21/pull/1746):
  the `Pitch` constructor's type annotation admits a `Pitch`.
- [cuthbertLab/music21#2003](https://github.com/cuthbertLab/music21/pull/2003):
  a malformed ratio in the Scala archive's `sparschuh-stanhope.scl`.
- [cuthbertLab/music21#2004](https://github.com/cuthbertLab/music21/pull/2004):
  a `type: ignore` left over from a closed mypy issue.
- [cuthbertLab/music21#2026](https://github.com/cuthbertLab/music21/pull/2026):
  Scala files with text after a pitch value, which 23 archive files have.
- [cuthbertLab/music21#2027](https://github.com/cuthbertLab/music21/pull/2027):
  `removeRedundantPitches` confusing a flat with a negative octave.
- [cuthbertLab/music21#2028](https://github.com/cuthbertLab/music21/pull/2028):
  `getPitchFromNodeDegree` handing back a pitch owned by the scale's cache
  (open).
- [PLAINSOUND/hexatone#3](https://github.com/PLAINSOUND/hexatone/pull/3):
  Scala headers in five Hexatone scale files (open).
- Corrections to the Xenharmonic Wiki's temperament pages, found while
  checking its infoboxes: [Special:Contributions/hill](https://en.xen.wiki/w/Special:Contributions/hill).
