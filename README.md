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
support, or `regex` when you want interval parsing to use the regex-backed path.

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

## Local Development

Use the Rust toolchain pinned in [rust-toolchain.toml](./rust-toolchain.toml).

```bash
cargo test
```

For parity work against upstream `music21`, initialize the reference submodule.
Normal library tests do not require Python.

```bash
git submodule update --init --recursive
```

Chord table code is committed to the repository so normal builds do not need
Python. To regenerate the table source from upstream `music21`, run:

```bash
cargo run -p xtask --features python -- regenerate-tables
```

That command refreshes [data/chord_tables.toml](./data/chord_tables.toml) and
then emits [src/chord/tables/generated.rs](./src/chord/tables/generated.rs).
To emit Rust from the committed TOML without touching Python, run:

```bash
cargo run -p xtask -- emit-tables
```

To verify that the committed Rust source matches the TOML, run:

```bash
cargo run -p xtask -- verify-tables
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

## Credits

Thanks to Michael Scott Asato Cuthbert and all `music21` contributors for their
work in computational musicology and for the original Python library.
