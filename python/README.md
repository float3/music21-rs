# music21-rs for Python

music21-shaped Python classes — `Pitch`, `Interval`, `Chord`, `Note`,
`Key`, `ToneRow` and the rest — carrying music21's names, constructor
keywords, properties and `repr`, backed by the Rust
[`music21-rs`](https://github.com/float3/music21-rs) crate.

```python
import music21_rs as m

chord = m.Chord("C4 E4 G4")
chord.commonName          # 'major triad'
chord.root()              # <music21.pitch.Pitch C4>
chord.transpose("M3")     # <music21.chord.Chord E4 G#4 B4>
```

The classes report music21's own module strings (`<music21.pitch.Pitch C4>`),
which is what lets `python-parity` run music21's own doctests against them:
the real music21 is imported, its classes are swapped for these, and its
docstrings are executed unchanged. The passing counts in the repository's
`python-parity/doctest/` are the measure of how much of music21 this
reproduces.

Anything music21 does that the Rust crate does not is left to fail rather
than reimplemented in Python-shaped Rust.

## Pointing an existing music21 program at it

`install_into_music21()` replaces music21's own classes with these, in the
music21 that is installed, so a program written for music21 runs on the Rust
implementation without a line changed:

```python
import music21_rs
music21_rs.install_into_music21()   # before anything imports the classes

from music21.chord import Chord
Chord("C4 E4 G4").commonName        # 'major triad', out of Rust
```

It patches a live module, so it changes music21 for everything in the
process; nothing calls it for you. Call it before the program does
`from music21.chord import Chord`, since that binds whatever it finds at
import time — a `conftest.py` is early enough for a test suite.

`cargo run --release -p xtask -- downstream` is that idea as a test: it checks out
[harte-library](https://github.com/andreamust/harte-library) — a Harte chord
notation parser that subclasses `chord.Chord` and `interval.Interval` — runs
its 8,116-test suite twice, once each way, and requires the two sets of
failures to match exactly. They do.

## Building

```bash
uvx maturin build --release --manifest-path python/Cargo.toml
uvx maturin develop --manifest-path python/Cargo.toml   # into the active venv
```

## Licence

AGPL-3.0-only, as the crate is.
