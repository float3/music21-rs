# music21-rs

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/music21-rs.svg)](https://crates.io/crates/music21-rs)
[![docs.rs](https://docs.rs/music21-rs/badge.svg)](https://docs.rs/music21-rs)
[![music21 members in the crate](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-crate.json)](https://hilll.dev/music21-rs/reports/#ported)
[![music21 doctests passing](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-doctests.json)](https://hilll.dev/music21-rs/reports/#doctests)

A Rust port of the analysis half of
[music21](https://github.com/cuthbertLab/music21): pitches, intervals, chords,
keys, scales, roman numerals, chord symbols, figured bass, durations, meters,
tone rows, sieves, tempo marks and voice leading. It keeps music21's data
model and answers, so a chord is named, a numeral realized and a scale spelled
as music21 does it. The same code is also published as a Python package; see
[python/README.md](./python/README.md).

The [reports page](https://hilll.dev/music21-rs/reports/) has the full
numbers: every music21 method and whether it is ported, the doctest and test
suite results, coverage, benchmarks and sizes, refreshed on every push.

## Coverage of music21

- 92% of the public methods of the ported music21 classes are in the crate.
- The crate's own tests cover 96% of its lines and 97% of its functions.
- 17 of the 19 music21 modules whose doctests run against the port pass all
  of them: `pitch`, `interval`, `chord`, `chord.tables`, `note`, `duration`,
  `key`, `scale`, `roman`, `harmony`, `serial`, `beam`, `tie`, `volume`,
  `figuredBass.notation`, `tempo` and `voiceLeading`. `sieve` and
  `meter.base` are partial: sieve compression and `MeterSequence` are not
  ported.
- music21's own test suite gives the same results with the crate's classes
  standing in for music21's as with music21 alone, apart from two documented
  differences.

Not ported: streams, parsing, notation output and the corpus.

## Speed and size

Times per call, from the
[benchmark](https://hilll.dev/music21-rs/reports/#speedups) against music21
11 on Python 3.13.

| | music21 | music21-rs | speedup |
| --- | ---: | ---: | ---: |
| `Pitch::from_name("C#4")` | 1.55 us | 0.22 us | 7x |
| `Interval::from_name("P5")` | 7.5 us | 0.21 us | 35x |
| `Chord::new("C4 E4 G4")` | 16.2 us | 0.79 us | 21x |
| `Chord::common_name` | 526 us | 14.8 us | 36x |
| `Chord::forte_class` | 244 us | 7.4 us | 33x |
| `Chord::root` | 115 us | 6.4 us | 18x |
| `Pitch::transpose` by a third | 28 us | 0.58 us | 49x |
| `Pitch::get_enharmonic` | 22.7 us | 0.37 us | 62x |
| `Interval::new(p1, p2)` | 9.7 us | 0.47 us | 21x |

The published crate is 0.7 MB of source, with the chord tables and scale
definitions compiled in and five small dependencies. A release binary that
names chords and transposes pitches, the benchmark example, is 441 KB.
music21 installs 105 MB.

## Using the crate

```bash
cargo add music21-rs
```

Default features are empty. `serde` adds `Serialize` and `Deserialize` to the
public types; `scala-archive` bundles the Scala scale archive described below.

Name a chord and read its set class:

```rust
use music21_rs::Chord;

let chord: Chord = "C E- G B-".parse()?;
assert_eq!(chord.pitched_common_name(), "C-minor seventh chord");
assert_eq!(chord.forte_class().as_deref(), Some("4-26"));
assert_eq!(chord.chord_symbol().as_deref(), Some("Cm7"));
# Ok::<(), music21_rs::Error>(())
```

Spell and transpose pitches:

```rust
use music21_rs::{Interval, Pitch};

let mut pitch = Pitch::from_name("C#4")?;
pitch.get_higher_enharmonic_in_place()?;
assert_eq!(pitch.name_with_octave(), "D-4");

let fifth = Interval::from_name("P5")?;
assert_eq!(fifth.transpose_pitch(&pitch)?.name_with_octave(), "A-4");
assert_eq!(fifth.pythagorean_ratio()?.to_string(), "3/2");
# Ok::<(), music21_rs::Error>(())
```

Realize a roman numeral in a key, and analyse a chord as one:

```rust
use music21_rs::{Chord, Key, RomanNumeral};

let key = Key::from_tonic("c")?;
let numeral = RomanNumeral::new("viio7", key.clone())?;
assert_eq!(numeral.to_chord()?.pitch_names(), ["B", "D", "F", "A-"]);

let analysed = RomanNumeral::analyze(&Chord::new("F A- C")?, key)?;
assert_eq!(analysed.map(|n| n.figure().to_string()), Some("iv".to_string()));
# Ok::<(), music21_rs::Error>(())
```

Beats and accent weights of a time signature:

```rust
use music21_rs::TimeSignature;

let six_eight = TimeSignature::new(6, 8)?;
assert_eq!(six_eight.beat_count(), 2);
assert_eq!(six_eight.accent_weight(1.5)?, 0.5);
# Ok::<(), music21_rs::Error>(())
```

Read a chord label in Harte notation:

```rust
use music21_rs::Harte;

let harte = Harte::new("Bb:min7/b3")?;
assert_eq!(harte.chord().pitch_names(), ["B-", "D-", "F", "A-"]);
assert_eq!(harte.chord().bass().map(|p| p.name_with_octave()), Some("D-3".to_string()));
assert_eq!(Harte::new("C:(b3,5)")?.prettify(), "C:min");
# Ok::<(), music21_rs::Error>(())
```

Full API documentation is on [docs.rs](https://docs.rs/music21-rs).

## Beyond music21

The tuning code has no counterpart in music21:

- 28 tuning systems as exact ratio tables: just intonation, meantone,
  Pythagorean, Bohlen-Pierce, the Carlos scales and 14 historical keyboard
  temperaments.
- The complete Scala scale archive, 3,994 scales, bundled and searchable with
  the `scala-archive` feature.
- Regular temperament theory from the Xenharmonic Wiki: monzos, vals, 95
  temperaments with their commas and mappings, moments of symmetry, and equal
  divisions of any interval.
- Adaptive tuning, where a pitch's frequency depends on the chord around it.
- Polyrhythms, including the chord a rhythm's ratios give as pitches.
- A check for the same scale filed under two names. Scales are compared by
  their cents, so one written as ratios and one written as cents still match.
  The Scala archive has 44 such groups.
- Harte chord notation, `C:maj7/3` or `Bb:(b3,5,b7,9)`, ported from
  [harte-library](https://github.com/andreamust/harte-library) and checked
  against every label in that library's 8,064-chord coverage set.

## Browser demos

[hilll.dev/music21-rs](https://hilll.dev/music21-rs/) runs the crate in the
browser via wasm, from [examples/web/](./examples/web/): a chord inspector, a
chord browser, a polyrhythm lab, a Harte chord reader, a roman numeral
realizer, a scale finder, a tone row matrix, a chord listener that names what
a microphone hears, whether its notes sound together or one after another, and
a tuning explorer that plays every tuning system, regular temperament, equal
division and Scala scale from the computer keyboard or a MIDI device.
[examples/audio/](./examples/audio/) plays a polyrhythm through the default
audio device.

## Development

Use the toolchain pinned in [rust-toolchain.toml](./rust-toolchain.toml).
`cargo test` runs the crate's tests and needs nothing else. The full check
is several commands, because the parity suite lives in a crate outside the
workspace:

```bash
# the reference submodule, needed by the parity suite and the Scala archive
git submodule update --init --recursive

# formatting, for each manifest; `--all` does not leave the workspace
cargo fmt --all -- --check
cargo fmt --manifest-path python/Cargo.toml --all -- --check
cargo fmt --manifest-path python-parity/Cargo.toml --all -- --check

# lints, and the generated files that must match their source data
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p xtask -- verify-tables
cargo run --release -p xtask -- verify-tuning-tables
cargo run --release -p xtask -- verify-scala-archive
cargo run --release -p xtask -- verify-temperaments
cargo run --release -p xtask -- verify-report-script
cargo check --workspace --locked

# the library, the examples and xtask, under each feature set
cargo test --workspace --all-targets
for flags in "--no-default-features" "--no-default-features --features serde" "--all-features"; do
  cargo clippy --workspace --all-targets $flags -- -D warnings
  cargo test --workspace --all-targets $flags
done

# the parity suite: fixtures, the Scala archive, and music21's doctests
cargo test --manifest-path python-parity/Cargo.toml -- --test-threads=1

# music21's own test suite, on music21 and on the crate
cargo run --release -p xtask --features python -- music21-suite

# the docs job; `report` fails when the feature map is stale
cargo doc --workspace --no-deps
cargo run --release -p xtask --features python -- report --features-only
```

The parity suite runs music21's doctests against the crate through an
embedded interpreter, so it needs music21's dependencies:

```bash
uv venv .m21venv --python 3.12
uv pip install --python .m21venv chardet joblib jsonpickle lark more_itertools numpy requests webcolors
```

On Windows, pyo3 links against the first `python` on `PATH`. The Microsoft
Store build does not work; set `PYO3_PYTHON` to another interpreter and put
its directory first on `PATH`.

Generated files are committed, so normal builds need only Rust. Each has
`regenerate-*`, `emit-*` and `verify-*` commands in `xtask`;
`regenerate-all` rebuilds all of them after a submodule bump. `nix develop`
opens a shell with everything CI uses.

## Credits and third-party data

`music21-rs` is released under the [AGPL-3.0](./LICENSE). It ports behaviour
from, and bundles data from, the projects below, each under its own licence.

### music21

The chord tables, scale definitions, time-signature behaviour, chord-symbol
kinds and every expectation fixture are derived from
[music21](https://github.com/cuthbertLab/music21), the Python library for
computational musicology by Michael Scott Asato Cuthbert and contributors,
licensed [BSD-3-Clause](https://github.com/cuthbertLab/music21/blob/master/LICENSE).
It is pinned as a git submodule, currently at 11.0.0b9, and is the source of
truth for `data/chord_tables.toml`, `data/*_expectations.toml` and the Rust
emitted from them. Thanks to Michael Scott Asato Cuthbert and all music21
contributors for the original library.

### harte-library

The `harte` module is a port of
[harte-library](https://github.com/andreamust/harte-library) by Andrea
Poltronieri, licensed
[MIT](https://github.com/andreamust/harte-library/blob/main/LICENSE), and
`data/harte_expectations.toml` is generated from its test data.

### The Scala scale archive

`data/scala_archive.toml` bundles 3,994 scales. 3,932 of them come from the
[Scala](https://www.huygens-fokker.org/scala/) scale archive as distributed
with music21, which includes it by kind permission of Manuel Op de Coul.
music21 states that it "assumes no copyright or change in original licensing"
for those files, and that some may carry restrictions; the complete archive is
published by the
[Huygens-Fokker Foundation](https://www.huygens-fokker.org/docs/scales.zip).
Entries carry `source = "music21"`.

### Plainsound Hexatone

The remaining 62 scales come from
[Plainsound Hexatone](https://github.com/PLAINSOUND/hexatone), a microtonal
MIDI isomorphic keyboard designed and programmed by
[Marc Sabat](https://www.plainsound.org), licensed
[GPL-3.0](https://www.gnu.org/licenses/gpl-3.0.en.html). They are curated
just intonation, harmonic-series, meantone and equal-division scales, several
credited in their own descriptions to Wilson, Fokker, Vicentino, Farabi and
others. Entries carry `source = "vendored"`. Those files remain under GPL-3.0
within this work, as AGPL-3.0 section 13 provides for.

### The Xenharmonic Wiki

`data/temperaments.toml` records the mapping, generators, commas and scales
of 95 regular temperaments from the [Xenharmonic Wiki](https://en.xen.wiki),
which is CC BY-SA. Only the numbers are used.

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
