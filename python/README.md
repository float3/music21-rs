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

## Building

```bash
uvx maturin build --release --manifest-path python/Cargo.toml
uvx maturin develop --manifest-path python/Cargo.toml   # into the active venv
```

## Licence

AGPL-3.0-only, as the crate is.
