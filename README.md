# music21-rs

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/music21-rs.svg)](https://crates.io/crates/music21-rs)
[![docs.rs](https://docs.rs/music21-rs/badge.svg)](https://docs.rs/music21-rs)

`music21-rs` is a Rust library inspired by selected parts of Python's
[`music21`](https://github.com/cuthbertLab/music21). The current focus is chord
analysis, pitch handling, polyrhythm helpers, and tuning-system utilities.

The crate is still young, so APIs may move while the port fills out.

## Using the crate

Add the crate to your project:

```bash
cargo add music21-rs
```

The default feature set is empty. Enable `serde` when you need serialization
support.

Parse a compact pitch string into a chord and ask for the same common-name
style used by `music21`:

```rust
use music21_rs::Chord;

let chord: Chord = "C E G".parse()?;

assert_eq!(chord.pitched_common_name(), "C-major triad");
assert_eq!(chord.common_name(), "major triad");

# Ok::<(), music21_rs::Error>(())
```

A chord can also report related analytical views:

```rust
use music21_rs::Chord;

let chord = Chord::try_from("C E- G B-")?;

println!("{}", chord.pitched_common_name());
println!("{:?}", chord.normal_form());
println!("{:?}", chord.interval_class_vector());
println!("{:?}", chord.invariance_vector());

# Ok::<(), music21_rs::Error>(())
```

Music21-style spelling helpers are exposed directly on library types:

```rust
use music21_rs::{Interval, Pitch};

let mut pitch = Pitch::from_name("C#4")?;
pitch.get_higher_enharmonic_in_place()?;
assert_eq!(pitch.name_with_octave(), "D-4");

let fifth = Interval::from_name("P5")?;
assert_eq!(fifth.pythagorean_ratio()?.to_string(), "3/2");

# Ok::<(), music21_rs::Error>(())
```

Scala `.scl` scale files can be loaded at runtime, including the several
thousand shipped inside the `music21` reference submodule:

```rust
use music21_rs::ScalaScale;

let scale = ScalaScale::parse("A fifth and an octave
 2
 3/2
 2/1
")?;

assert_eq!(scale.len(), 2);
assert_eq!(scale.ratio_at(1), 1.5);
assert_eq!(scale.frequency_at(440.0, 1), 660.0);

# Ok::<(), music21_rs::Error>(())
```

Degrees keep their written form, so an exact ratio stays exact while a scale
written in cents stays in cents. The crate performs no file IO itself; read the
bytes and hand them to `ScalaScale::parse_bytes`.

## Browser Demos

The live browser demos are published at
[float3.github.io/music21-rs](https://float3.github.io/music21-rs/).

The `examples/` directory contains a small set of interactive tools:

- [Chord Inspector](./examples/web/chord/) names chords from pitch names, MIDI
  numbers, or Web MIDI input; shows Forte/normal-form data; suggests simple
  resolution chords; plays the result; and renders staff notation.
- [Chord Browser](./examples/web/chords/) lists the chord types known to the
  music21-derived chord table, realizes them from a chosen root, and opens root
  position or inversions in the inspector.
- [Polyrhythm Lab](./examples/web/polyrhythm/) lets you enter ratios such as
  `4:5:6`, play the cycle, and compare the rhythm to its equivalent pitch-set
  relationship.
- [Tuning Explorer](./examples/web/tuning/) lists the tuning systems exposed by the
  crate and plays each scale from a chosen root frequency.
- [Audio Polyrhythm Example](./examples/audio/) plays a small polyrhythm
  through the default audio device.

The examples are also wired into the GitHub Pages build, with
[examples/web/index.html](./examples/web/index.html) as the local landing page.

music21's own doctests can be run against the crate: `python-parity` builds a
Python module of music21-shaped classes over `music21-rs` and runs the
docstrings of music21's `pitch.py`, `interval.py`, `chord.py`, `key.py` and
`serial.py` with those in place of music21's, covering better than nine in ten
of their examples. The docstrings that pass are
listed under `python-parity/doctest/`, and
`cargo test --manifest-path python-parity/Cargo.toml --test doctest_pitch`
(or `doctest_interval`, `doctest_chord`, `doctest_key`, `doctest_serial`)
fails when one of them stops passing.

The same site carries [/reports/](https://float3.github.io/music21-rs/reports/):
the library's test coverage, the state of every test suite the repository has —
the workspace, the parity suite, and the Python wheel with its own tests — how
much of music21's own documentation runs against the crate, docstring by
docstring and example by example for each module covered, and every public
method of the music21 classes the crate ports against what has been ported so
far, with the deliberate omissions and their reasons. It is written by `cargo run --release -p xtask -- report` from
[data/feature_map.toml](./data/feature_map.toml).

## Local Development

Use the Rust toolchain pinned in [rust-toolchain.toml](./rust-toolchain.toml).

```bash
cargo test
```

### Running everything

The repository holds four cargo crates, and two of them — `python` and
`python-parity` — sit outside the workspace on purpose, so that a plain
`cargo test` never needs Python. That also means no single cargo invocation
reaches everything. The block below is the whole of what CI gates, in the
order it fails fastest, and can be pasted as one command:

```bash
# 1. the reference submodule, needed by python-parity and the Scala archive
git submodule update --init --recursive

# 2. formatting, for each manifest -- `--all` does not leave the workspace
cargo fmt --all -- --check
cargo fmt --manifest-path python/Cargo.toml --all -- --check
cargo fmt --manifest-path python-parity/Cargo.toml --all -- --check

# 3. lints, and the generated files that must match their source data
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p xtask -- verify-tables
cargo run --release -p xtask -- verify-tuning-tables
cargo run --release -p xtask -- verify-scala-archive
cargo check --workspace --locked

# 4. the library, the examples and xtask
cargo test --workspace --all-targets

# 5. the feature matrix; default features are empty, so this is not redundant
feature_sets=(
  "--no-default-features"
  "--no-default-features --features serde"
  "--all-features"
)
for flags in "${feature_sets[@]}"; do
  cargo check --workspace --all-targets $flags
  cargo clippy --workspace --all-targets $flags -- -D warnings
  cargo test --workspace --all-targets $flags
done

# 6. the parity suite: fixtures, the Scala archive, and music21's own
#    doctests run against the crate through a music21-shaped pyo3 facade
cargo test --manifest-path python-parity/Cargo.toml -- --test-threads=1

# 7. the wheel, its own tests, and a real third-party music21 library run
#    against both music21 and this crate
maturin build --release --manifest-path python/Cargo.toml --out target/wheels
pip install --no-index --find-links target/wheels music21-rs
pytest python/tests -q
python python/downstream/run.py

# 8. what the docs job builds; `report` also fails when the feature map is stale
cargo doc --workspace --no-deps
cargo run --release -p xtask -- report --features-only

# `report` with no flags runs every suite above and records how each one did,
# alongside coverage. `--suites-only` runs just that part; `--no-suites` skips
# it. A suite whose tooling is missing is recorded as skipped, with the reason,
# rather than failing the report.
```

Step 6 needs a Python interpreter with music21's own dependencies, since the
doctest harness imports the whole of music21 rather than the stubbed subset the
chord-table bridge uses:

```bash
uv venv .m21venv --python 3.12
uv pip install --python .m21venv chardet joblib jsonpickle more_itertools numpy requests webcolors
```

Step 7 needs `maturin`, `pytest`, and — for `downstream/run.py` — `git`,
`music21`, `lark` and `numpy`. That script clones a pinned copy of
`harte-library`, a library written for music21 by someone with no knowledge of
this crate, and runs its several-thousand-test suite twice: once on music21 and
once on `music21_rs`. It is a comparison rather than a pass mark, and it fails
if the two sets of failures differ.

**On Windows**, pyo3 links against whichever `python` comes first on `PATH`. If
that is the Microsoft Store build, every step 6 test dies with
`STATUS_DLL_NOT_FOUND` before running a line. Point `PYO3_PYTHON` at a normal
interpreter and put its directory on `PATH` first:

```bash
export PYO3_PYTHON="$(ls -d "$HOME"/AppData/Roaming/uv/python/cpython-3.12*/python.exe | head -1)"
export PATH="$(dirname "$PYO3_PYTHON"):$PATH"
```

Two CI jobs are not in the list because they need tooling this repository does
not otherwise ask for: `nix flake check` with `alejandra --check .`, and the
`wasm-pack` and `tsc` build of [examples/web/](./examples/web/).

Chord table code is committed to the repository so normal builds do not need
Python. To regenerate the table source from upstream `music21`, run:

```bash
cargo run --release -p xtask --features python -- regenerate-tables
```

That command refreshes [data/chord_tables.toml](./data/chord_tables.toml) and
then emits [src/chord/tables/generated.rs](./src/chord/tables/generated.rs).
To emit Rust from the committed TOML without touching Python, run:

```bash
cargo run --release -p xtask -- emit-tables
```

To verify that the committed Rust source matches the TOML, run:

```bash
cargo run --release -p xtask -- verify-tables
```

The tuning-system ratio tables follow the same pattern, sourced from the Scala
archive shipped inside the `music21` submodule rather than from Python:

```bash
cargo run --release -p xtask -- regenerate-tuning-tables
cargo run --release -p xtask -- emit-tuning-tables
cargo run --release -p xtask -- verify-tuning-tables
```

If you use Nix, `nix develop` opens a shell with the Rust and Python pieces used
by the repository's CI setup.

## Project Layout

- [src/chord/](./src/chord/) chord construction, naming, set-class helpers, and
  resolution suggestions
- [src/pitch/](./src/pitch/) pitch spelling, accidentals, and pitch-space helpers
- [src/polyrhythm.rs](./src/polyrhythm.rs) polyrhythm timing and pitch-ratio
  conversion
- [src/tuningsystem/](./src/tuningsystem/) tuning-system ratios and
  frequency helpers
- [examples/web/](./examples/web/) browser tools
- [examples/audio/](./examples/audio/) small polyrhythm sound example
- [music21/](./music21/) optional upstream reference submodule

## Credits and third-party data

`music21-rs` is released under the [AGPL-3.0](./LICENSE). It ports behaviour
from, and bundles data derived from, the projects below. Each remains under its
own licence and copyright; the terms here describe those works, not this crate.

### music21

Chord tables, scale definitions, time-signature behaviour and the expectation
fixtures used to test them are all derived from
[music21](https://github.com/cuthbertLab/music21), the Python library for
computational musicology by Michael Scott Asato Cuthbert and contributors,
licensed [BSD-3-Clause](https://github.com/cuthbertLab/music21/blob/master/LICENSE).

It is pinned here as a git submodule (currently `v10.5.0-319`) and is the source
of truth for `data/chord_tables.toml`, `data/*_expectations.toml`, and the
generated Rust emitted from them. Thanks to Michael Scott Asato Cuthbert and all
`music21` contributors for the original library.

### The Scala scale archive

`data/scala_archive.toml` bundles 3,994 scales. 3,932 of them come from the
[Scala](https://www.huygens-fokker.org/scala/) scale archive as distributed with
music21, which includes it by kind permission of Manuel Op de Coul. music21
states that it "assumes no copyright or change in original licensing" for those
files, and that some may carry restrictions; the complete archive is published by
the [Huygens-Fokker Foundation](https://www.huygens-fokker.org/docs/scales.zip).
Entries carry `source = "music21"`.

### Plainsound Hexatone

The remaining 62 scales in `data/scala_archive.toml` come from
[Plainsound Hexatone](https://github.com/PLAINSOUND/hexatone) — a microtonal
MIDI isomorphic keyboard designed and programmed by
[Marc Sabat](https://www.plainsound.org), licensed
[GPL-3.0](https://www.gnu.org/licenses/gpl-3.0.en.html). They are curated just
intonation, harmonic-series, meantone and equal-division scales, several
credited in their own descriptions to Wilson, Fokker, Vicentino, Farabi and
others. Entries carry `source = "vendored"`.

Those files remain under GPL-3.0 within this work, as AGPL-3.0 section 13
provides for.

### Contributed back

Header-format fixes for five of the Hexatone scale files were sent upstream as
[PLAINSOUND/hexatone#3](https://github.com/PLAINSOUND/hexatone/pull/3) — until
that lands, the `hexatone` submodule tracks a fork branch carrying it — and a
malformed ratio in the Scala archive as
[cuthbertLab/music21#2003](https://github.com/cuthbertLab/music21/pull/2003).
