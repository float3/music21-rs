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

Create a chord from a compact pitch string and ask for the same common-name
style used by `music21`:

```rust
use music21_rs::Chord;

let chord = Chord::new("C E G")?;

assert_eq!(chord.pitched_common_name(), "C-major triad");
assert_eq!(chord.common_name(), "major triad");

# Ok::<(), music21_rs::Exception>(())
```

A chord can also report related analytical views:

```rust
use music21_rs::Chord;

let chord = Chord::new("C E- G B-")?;

println!("{}", chord.pitched_common_name());
println!("{:?}", chord.normal_form());
println!("{:?}", chord.interval_vector());

# Ok::<(), music21_rs::Exception>(())
```

## Browser Demos

The live browser demos are published at
[float3.github.io/music21-rs](https://float3.github.io/music21-rs/).

The `examples/` directory contains a small set of interactive tools:

- [Chord Inspector](./examples/chord/) names chords, shows Forte/normal-form
  data, suggests simple resolution chords, plays the result, and renders staff
  notation.
- [Polyrhythm Lab](./examples/polyrhythm/) lets you enter ratios such as
  `4:5:6`, play the cycle, and compare the rhythm to its equivalent pitch-set
  relationship.
- [Tuning Explorer](./examples/tuning/) lists the tuning systems exposed by the
  crate and plays each scale from a chosen root frequency.

The examples are also wired into the GitHub Pages build, with
[examples/index.html](./examples/index.html) as the local landing page.

## Local Development

Use the Rust toolchain pinned in [rust-toolchain.toml](./rust-toolchain.toml).

```bash
cargo test
```

For parity work against upstream `music21`, initialize the reference submodule
and run the Python-backed checks:

```bash
git submodule update --init --recursive
cargo test --features python
```

If you use Nix, `nix develop` opens a shell with the Rust and Python pieces used
by the repository's CI setup.

## Project Layout

- [src/chord/](./src/chord/) chord construction, naming, set-class helpers, and
  resolution suggestions
- [src/pitch/](./src/pitch/) pitch spelling, accidentals, and pitch-space helpers
- [src/polyrhythm.rs](./src/polyrhythm.rs) polyrhythm timing and pitch-ratio
  conversion
- [src/tuningsystem.rs](./src/tuningsystem.rs) tuning-system ratios and
  frequency helpers
- [examples/](./examples/) browser tools and the small polyrhythm sound example
- [music21/](./music21/) optional upstream reference submodule

## Credits

Thanks to Michael Scott Asato Cuthbert and all `music21` contributors for their
work in computational musicology and for the original Python library.
