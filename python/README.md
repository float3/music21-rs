# music21-rs for Python

[![PyPI](https://img.shields.io/pypi/v/music21-rs.svg)](https://pypi.org/project/music21-rs/)
[![music21 members in the wheel](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-wheel.json)](https://hilll.dev/music21-rs/reports/#ported)
[![music21 doctests passing](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-doctests.json)](https://hilll.dev/music21-rs/reports/#doctests)

`music21_rs` is a Rust port of
[music21](https://github.com/cuthbertLab/music21)'s analysis classes:
`Pitch`, `Interval`, `Chord`, `Note`, `Duration`, `Key`, `Scale`,
`RomanNumeral`, `TimeSignature`, `ToneRow` and others, with music21's names,
arguments, properties and `repr`. Streams, parsing, notation output and the
corpus are not included; those remain music21's.

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

The [reports page](https://hilll.dev/music21-rs/reports/) lists every music21
method as ported, missing, or excluded with a reason.

- 74% of the public methods of the ported music21 classes are reachable from
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

Not included: streams, scores, parsing, writing, the corpus and `show()`.
`Tuplet`, `AbstractScale`, `Sieve` and `style.Style` are provided but not
installed over music21's, whose versions do more. Missing behaviour raises
rather than falling back to music21.

## Speed

Times per call, from the [benchmark](https://hilll.dev/music21-rs/reports/#speedups)
against music21 11 on Python 3.13.

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
| `ToneRow.zeroCenteredTransformation` | | | 741x |

Repeated queries on the same chord are cached in both and cost the same. Over
music21's own test suite the median test runs at the same speed, since most
of a music21 test is music21's own code.

## Building

```bash
uvx maturin build --release --manifest-path python/Cargo.toml
uvx maturin develop --manifest-path python/Cargo.toml   # into the active venv
pytest python/tests -q
```

The wheel is built from the `music21-rs` crate and shares its version number.

## Licence

AGPL-3.0-only, as the crate is.
