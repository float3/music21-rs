# music21-rs

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/music21-rs.svg)](https://crates.io/crates/music21-rs)
[![docs.rs](https://docs.rs/music21-rs/badge.svg)](https://docs.rs/music21-rs)
[![PyPI](https://img.shields.io/pypi/v/music21-rs.svg)](https://pypi.org/project/music21-rs/)

[![music21 members in the crate](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-crate.json)](https://hilll.dev/music21-rs/reports/#features)
[![music21 members in the wheel](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-wheel.json)](https://hilll.dev/music21-rs/reports/#features)
[![music21 doctests passing](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-doctests.json)](https://hilll.dev/music21-rs/reports/#doctests)

A Rust port of the parts of [music21](https://github.com/cuthbertLab/music21)
that analyse and spell music: pitches, intervals, chords, keys, scales, roman
numerals, chord symbols, figured bass, durations, meters, tone rows, sieves,
tempo marks and voice leading. It comes two ways:

- the Rust crate, `music21-rs`, with no Python anywhere in it;
- a Python wheel, `music21_rs`, whose classes have music21's names, keyword
  arguments, properties and `repr`, and which can be installed over an
  existing music21 so a program written for music21 runs on the Rust code
  without a line changed. See [python/README.md](./python/README.md).

Whether it is a faithful port is measured rather than claimed. music21's own
doctests run against the crate, module by module, and music21's whole test
suite runs with the wheel installed over it. The badges above are written by
that measurement on every push; the details are on the
[reports page](https://hilll.dev/music21-rs/reports/).

## How much of music21 is here

Three numbers say it, and the reports page keeps all three current.

**Members ported.** [data/feature_map.toml](./data/feature_map.toml) names the
music21 classes the crate ports. The report reads every public method of those
classes out of music21 itself and looks for the crate's function of the same
name, so a method music21 adds shows up as missing until it is ported. Members
left out on purpose are listed with a reason each. The first badge is the
crate's share; the second is the share that also reaches Python through the
wheel.

**Doctests passing.** Nineteen music21 modules are run through a harness that
imports the real music21, swaps its classes for the wheel's, and runs the
module's docstrings unchanged. Seventeen of them pass every example:
`pitch`, `interval`, `chord`, `chord.tables`, `note`, `duration`, `key`,
`scale`, `roman`, `harmony`, `serial`, `beam`, `tie`, `volume`,
`figuredBass.notation`, `tempo` and `voiceLeading`. The two that do not are
`sieve`, which lacks music21's compressed reading of a sieve, and
`meter.base`, which lacks the `MeterSequence` partition tree; both are
deliberate and are explained in the report.

**music21's own suite.** All 5,000-odd of music21's own tests run three ways
in CI: on music21, on music21 with the crate installed over it, and on music21
with the published wheel installed over it. Anything that fails only with the
crate installed fails the build. Two divergences are expected and listed by
name.

A fourth check is written by someone else:
[harte-library](https://github.com/andreamust/harte-library), a chord grammar
built on music21 by an author who had never heard of this crate, runs its
8,116 tests on both and the failures have to match.

## Using the crate

```bash
cargo add music21-rs
```

Default features are empty. `serde` adds `Serialize` and `Deserialize` to the
public types; `scala-archive` bundles the Scala scale archive described below.

Name a chord, and read the set-class views music21 gives it:

```rust
use music21_rs::Chord;

let chord: Chord = "C E- G B-".parse()?;
assert_eq!(chord.pitched_common_name(), "C-minor seventh chord");
assert_eq!(chord.forte_class().as_deref(), Some("4-26"));
assert_eq!(chord.chord_symbol().as_deref(), Some("Cm7"));
# Ok::<(), music21_rs::Error>(())
```

Spell and transpose pitches the way music21 does:

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

Read a roman numeral as figured bass over a key, and a chord as a numeral:

```rust
use music21_rs::{Chord, Key, RomanNumeral};

let key = Key::from_tonic("c")?;
let numeral = RomanNumeral::new("viio7", key.clone())?;
assert_eq!(numeral.to_chord()?.pitch_names(), ["B", "D", "F", "A-"]);

let analysed = RomanNumeral::analyze(&Chord::new("F A- C")?, key)?;
assert_eq!(analysed.map(|n| n.figure().to_string()), Some("iv".to_string()));
# Ok::<(), music21_rs::Error>(())
```

Ask a time signature where its beats fall and how strong each is:

```rust
use music21_rs::TimeSignature;

let six_eight = TimeSignature::new(6, 8)?;
assert_eq!(six_eight.beat_count(), 2);
assert_eq!(six_eight.accent_weight(1.5)?, 0.5);
# Ok::<(), music21_rs::Error>(())
```

Every module is documented on [docs.rs](https://docs.rs/music21-rs), and every
type is re-exported at the crate root.

## What music21 does not have

The tuning half of the crate goes past music21, which tunes a scale only by
reading a Scala file:

- 28 tuning systems as exact ratio tables, just intonation, meantone,
  Pythagorean, Bohlen-Pierce and the Carlos scales among them, with fourteen
  historical keyboard temperaments from Salinas to Lehman;
- the whole Scala scale archive, 3,994 scales, parsed once at build time and
  searchable without any file IO (`scala-archive` feature);
- regular temperament theory from the Xenharmonic Wiki: monzos and vals, 95
  temperaments with their commas and mappings, moments of symmetry, and equal
  divisions of any interval;
- adaptive tuning, where a pitch's frequency depends on the chord it sounds in;
- polyrhythms as a first-class type, with the chord a cycle maps onto when its
  ratios are read as pitches;
- duplicate detection across all of the above, by sound rather than spelling.

Every table transcribed from somewhere else is checked against its source in
CI, so a mistyped ratio fails the build.

## Browser demos

[hilll.dev/music21-rs](https://hilll.dev/music21-rs/) runs the crate in the
browser through wasm: a chord inspector, a chord browser, a polyrhythm lab and
a tuning explorer, all under [examples/web/](./examples/web/).
[examples/audio/](./examples/audio/) plays a polyrhythm through the default
audio device.

## Working on it

Use the toolchain pinned in [rust-toolchain.toml](./rust-toolchain.toml).
`cargo test` runs the crate's own tests and needs no Python. The `python` and
`python-parity` crates sit outside the workspace on purpose, so what CI gates
takes more than one command:

```bash
# the reference submodule, needed by python-parity and the Scala archive
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

# the wheel, its tests, music21's suite, and harte-library on both
maturin build --release --manifest-path python/Cargo.toml --out target/wheels
pip install --no-index --find-links target/wheels music21-rs
pytest python/tests -q
cargo run --release -p xtask --features python -- music21-suite
cargo run --release -p xtask -- downstream

# the docs job; `report` fails when the feature map is stale
cargo doc --workspace --no-deps
cargo run --release -p xtask --features python -- report --features-only
```

The parity suite imports the whole of music21, so it needs an interpreter
with music21's dependencies:

```bash
uv venv .m21venv --python 3.12
uv pip install --python .m21venv chardet joblib jsonpickle more_itertools numpy requests webcolors
```

On Windows, pyo3 links against whichever `python` is first on `PATH`; the
Microsoft Store build cannot be loaded, so point `PYO3_PYTHON` at another
interpreter and put its directory first.

Generated files are committed so normal builds need nothing but Rust. Each has
a `regenerate-*`, `emit-*` and `verify-*` command in `xtask`:
`regenerate-tables` reads music21's chord tables through Python,
`regenerate-tuning-tables` and `regenerate-scala-archive` read the Scala
archive from the submodules, and `regenerate-fixtures` rewrites every
expectation file under [data/](./data/). `regenerate-all` does all of it,
which is what a submodule bump needs. `nix develop` opens a shell with the
Rust and Python pieces CI uses.

## Credits and third-party data

`music21-rs` is released under the [AGPL-3.0](./LICENSE). It ports behaviour
from, and bundles data derived from, the projects below. Each remains under its
own licence and copyright.

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
of 95 regular temperaments as the [Xenharmonic Wiki](https://en.xen.wiki)
publishes them. The wiki is CC BY-SA; only the numbers are taken.

### Contributed back

Header fixes for five Hexatone scale files went upstream as
[PLAINSOUND/hexatone#3](https://github.com/PLAINSOUND/hexatone/pull/3), a
malformed ratio in the Scala archive as
[cuthbertLab/music21#2003](https://github.com/cuthbertLab/music21/pull/2003),
and music21's reading of Scala files with trailing comments as
[cuthbertLab/music21#2026](https://github.com/cuthbertLab/music21/pull/2026).
