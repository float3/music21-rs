# harte-rs

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)
[![PyPI](https://img.shields.io/pypi/v/harte-rs.svg)](https://pypi.org/project/harte-rs/)

Harte chord notation, `C:maj7/3`, `Bb:(b3,5,b7,9)`, `F:sus4(*3,9)`, `N`,
read into [music21-rs](https://pypi.org/project/music21-rs/) chords. it is
a replacement for [harte-library](https://github.com/andreamust/harte-library):
it is imported as `harte`, has the same modules and names, and builds the
chord harte-library builds for every label in that library's coverage set.
the label is read by the [harte-rs](https://crates.io/crates/harte-rs) Rust crate,
and the chord is a music21-rs `Chord`, so neither music21 nor lark is
needed.

```bash
pip install harte-rs
```

harte-rs and harte-library both install the `harte` package, so uninstall
harte-library first.

```python
from harte.harte import Harte
from harte.interval import HarteInterval
from harte.utils import convert_interval

chord = Harte("Bb:min7/b3")
chord.pitches         # (<music21.pitch.Pitch B-4>, <music21.pitch.Pitch D-3>, <music21.pitch.Pitch F5>, <music21.pitch.Pitch A-5>)
chord.bass()          # <music21.pitch.Pitch D-3>
chord.commonName      # 'minor seventh chord'
chord.get_root()      # 'Bb'
chord.get_shorthand() # 'min7'
chord.get_bass()      # 'b3'

Harte("C:(b3,5)").prettify()            # 'C:min'
Harte("G:7").get_midi_pitches()         # [67, 71, 74, 77]
Harte("G:7").multi_hot_encoding()       # [0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1]
Harte("F:sus4(*5,9)").unwrap_shorthand() # ['1', '4', '9']

HarteInterval("b6").name                # 'm6'
convert_interval("#11")                 # 'A4'
```

`Harte` is a music21-rs `Chord` and `HarteInterval` a music21-rs
`Interval`, so every member those have works on them. with music21
installed and `music21_rs.install_into_music21()` called before `harte` is
imported, a `Harte` is a music21 `Chord` too, and goes in music21's streams
as harte-library's does.

## differences from harte-library

- a label that is not Harte notation raises music21-rs's `ChordException`,
  where harte-library raises lark's parse errors. `harte.parse_harte`, the
  lark grammar, is not included.
- `str()` and `repr()` give the label in its canonical spelling,
  `Harte(C:maj7(9)/3)`. harte-library's raise `TypeError` for every label
  but `N`.
- two labels are equal when they sound the same chord: `C:maj` equals
  `C:(3,5)`. harte-library's compares the degrees as written.
- `bass_is_root()` answers whether the bass is the root. harte-library's
  compares the root's name with the bass degree, and answers `False` for
  every chord.
- `N` and `X` have no shorthand: `get_shorthand()` is `None` and
  `contains_shorthand()` is `False`.

## development

harte-rs lives in the [music21-rs](https://github.com/float3/music21-rs)
repository, in `harte/python`, and is released with the music21-rs wheel at
the same version. it builds on that wheel's `_HarteLabel`, so it needs the
wheel built from the same checkout:

```bash
maturin build --release --manifest-path python/Cargo.toml --out target/wheels
pip install target/wheels/music21_rs-*.whl ./harte/python pytest
python -m pytest harte/python/tests
```

the tests check every label in harte-library's coverage set against
`harte/data/harte_expectations.toml`, which records what harte-library makes
of each.

## credits

harte-rs is released under the
[AGPL-3.0](https://github.com/float3/music21-rs/blob/master/LICENSE). it is
a port of [harte-library](https://github.com/andreamust/harte-library) by
Andrea Poltronieri, licensed MIT; its licence is included as
`LICENSE-harte-library`.
