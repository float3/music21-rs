# music21-rs

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/music21-rs.svg)](https://crates.io/crates/music21-rs)
[![docs.rs](https://docs.rs/music21-rs/badge.svg)](https://docs.rs/music21-rs)

`music21-rs` is a work-in-progress Rust port of selected parts of [music21](https://github.com/cuthbertLab/music21), currently focused on chord naming and supporting pitch/chord infrastructure.

## Status

- The project is under active development.
- APIs are still evolving and may change between releases.
- The Python `music21` repository is used as a reference source and is included as a git submodule at [`music21/`](./music21).

## Quick Start

```rust
use music21_rs::chord::Chord;

let chord = Chord::new("C E G")?;
assert_eq!(chord.pitched_common_name(), "C-major triad");

let augmented = Chord::new("C E G#")?;
assert_eq!(
    augmented.pitched_common_names(),
    ["C-augmented triad", "C-equal 3-part octave division"]
);

let empty = Chord::new("")?;
assert_eq!(empty.pitched_common_name(), "empty chord");
# Ok::<(), music21_rs::exception::Exception>(())
```

## Examples

- [Chord Inspector web demo](./examples/chord/) builds with `wasm-pack` and is published with the generated docs on GitHub Pages.
- [Polyrhythm Lab web demo](./examples/polyrhythm/) maps rhythm cycles to chord/polypitch sets and links them back to the chord inspector.
- [Tuning Explorer web demo](./examples/tuning/) compares music21-rs tuning systems and plays their scales in the browser.
- [Polyrhythm sound example](./examples/polyrhythmsound.rs) demonstrates the polyrhythm helpers.

## Development

### Prerequisites

- Rust (stable toolchain; see [`rust-toolchain.toml`](./rust-toolchain.toml))
- Git

### Clone

```bash
git clone https://github.com/float3/music21-rs.git
cd music21-rs
git submodule update --init --recursive
```

### Nix Development Shell (Optional)

If you use Nix, the repository includes a flake-based development shell with Rust
tooling, Python, and bindgen dependencies:

```bash
nix develop
```

Inside the shell, `PYO3_PYTHON` is set automatically to the Nix-provided Python
interpreter for Python-backed build/test flows.

### Optional Python-Backed Validation

Some generation and parity checks can use Python via the `python` feature:

```bash
cargo test --features python
```

## Project Layout

- `src/`: main Rust library
- `src/bin/test.rs`: regression-style executable checks for chord naming
- `utils/`: shared helper crate for build/test support
- `music21/`: Python reference submodule
- `build.rs`: optional table generation logic

## Credits

Thanks to Michael Scott Asato Cuthbert and all `music21` contributors for their work in computational musicology and for the `music21` library.
