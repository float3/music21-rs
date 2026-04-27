# music21-rs

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)

`music21-rs` is a work-in-progress Rust port of selected parts of [music21](https://github.com/cuthbertLab/music21), currently focused on chord naming and supporting pitch/chord infrastructure.

## Status

- The project is under active development.
- APIs are still evolving and may change between releases.
- The Python `music21` repository is used as a reference source and is included as a git submodule at [`music21/`](./music21).

## Quick Start

```rust
use music21_rs::chord::Chord;

let chord = Chord::new(Some("C E G"))?;
assert_eq!(chord.pitched_common_name(), "C-major triad");
# Ok::<(), music21_rs::exception::Exception>(())
```

## Website-Facing APIs

The crate now includes high-level helpers in [`src/web/mod.rs`](./src/web/mod.rs) intended for
backend/API use in music websites and tools:

- `analyze_chord`: rich chord metadata (common names, forte class, inversion, pitch classes)
- `analyze_key`: mode/key signature summary, scale tones, harmonized triads/sevenths, relative/parallel
- `analyze_progression`: degree + roman numeral labeling per chord in a key context
- `suggest_scales_for_chord`: candidate scales/modes that contain all chord tones
- `analyze_polyrhythm`: timeline events, beat timings, coincidence ticks, and derived harmonic color

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

### Common Commands

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo run --bin test
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
