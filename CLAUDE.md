# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`music21-rs` is a Rust library inspired by parts of Python's `music21`
(chord analysis, pitch handling, polyrhythm, tuning systems). Use the
Rust toolchain pinned in `rust-toolchain.toml`. `nix develop` opens a
shell with the Rust and Python pieces used by CI, if you use Nix.

## Repository guidance (from AGENTS.md)

- Keep `music21-rs` library code (`src/`) application-agnostic.
  Functionality that exists only to render, lay out, or adapt data for
  `examples/web` belongs in the web example layer, not under `src/`.

## Commands

Main library tests (no Python required):

```bash
cargo test
```

Run a single test (works like any Rust crate — substring match, module path
prefix, or full path):

```bash
cargo test tuningsystem::tests::twelve_tone_systems_keep_chromatic_ratios_ascending
cargo test --lib tuningsystem   # all tests in a module
```

Lint / format, matching CI exactly:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace --all-targets
```

Full workspace test run (`examples/audio`, `examples/web`, `xtask` — but
*not* `python-parity`, which is excluded from the workspace):

```bash
cargo test --workspace --all-targets
```

Chord-table smoke-check binary (hardcoded expectations from the Python
reference behavior):

```bash
cargo run --bin test
```

Python parity tests, run separately against the upstream `music21`
reference submodule (init it first):

```bash
git submodule update --init --recursive
cargo test --manifest-path python-parity/Cargo.toml -- --test-threads=1
```

Chord table pipeline. The generated table is committed, so normal builds
never need Python:

```bash
cargo run -p xtask --features python -- regenerate-tables  # music21 (py) -> data/chord_tables.toml -> src/chord/tables/generated.rs
cargo run -p xtask -- emit-tables                           # data/chord_tables.toml -> generated.rs only, no Python
cargo run -p xtask -- verify-tables                          # assert generated.rs matches the TOML (CI runs this in lint)
```

## What CI gates

`.github/workflows/ci.yml` is the only workflow. A change must survive:

- **lint** — `cargo fmt --all -- --check`, clippy with `-D warnings`,
  `xtask verify-tables`, `cargo check --workspace --all-targets`.
- **feature-matrix** — check + clippy + test under `--no-default-features`,
  `--no-default-features --features serde`, and `--all-features`. A change
  that only compiles with default features fails here.
- **test** — `cargo test --workspace --all-targets` on ubuntu *and* windows.
- **python-parity** — the excluded crate, run single-threaded against the
  submodule.
- **stable-check** — `cargo check --workspace --locked`, so a change that
  touches dependencies must commit the updated `Cargo.lock`.
- **nix** — `nix flake check` plus `alejandra --check .` on the Nix files.

`docs` builds rustdoc and the wasm-pack web examples and `pages` publishes
them; neither is a correctness gate.

`release` runs on pushes to master and compares `package.version` in
`Cargo.toml` against the previous commit's. If it changed, it tags
`v<version>` and cuts a GitHub release with generated notes — so **merging a
version bump to master publishes a release**. It does not run `cargo
publish`; crates.io is still a manual step. When bumping, add the matching
section at the top of `RELEASE_NOTES.md` and refresh both `Cargo.lock` and
`python-parity/Cargo.lock` (that crate is outside the workspace and pins
`music21-rs` by path, so its lockfile carries the version too).

Note that `fraction` is a *public* dependency — `FractionType` is re-exported
from the crate root and returned by `Interval::pythagorean_ratio` — so
bumping it is a breaking change for downstreams and needs a minor bump,
not a patch.

Running `cargo fmt --all`, the clippy line, `cargo test --workspace
--all-targets`, and `xtask verify-tables` locally covers everything except
the feature matrix and the Nix job.

## Architecture

**Workspace layout**: the root crate (`music21-rs`) plus `utils`,
`examples/audio`, `examples/web`, and `xtask` are cargo workspace members.
`python-parity` is a separate, non-member crate (own `Cargo.toml`, depends
on `music21-rs` by path) so its Python/pyo3-only test suite never taints
normal `cargo test`/`cargo check` runs on the library.

**Python bridge (`shared.rs`, `utils`)**: `shared.rs` is compiled into
both `xtask` and `python-parity` (via the `utils` crate's `python`
feature) and holds all the pyo3 plumbing — cloning the `music21` submodule,
optionally creating a venv, and injecting dummy `music21.environment`/
`music21.exceptions21` modules so `music21/music21/chord/tables.py` can be
imported standalone without pulling in all of music21's dependencies. This
is how `xtask regenerate-tables` and the python-parity tests reach the
Python reference implementation.

**Chord table generation**: `data/chord_tables.toml` is the checked-in
source of truth, derived from upstream music21's `chord/tables.py`.
`src/chord/tables/generated.rs` (~7k lines) is deterministically emitted
from that TOML by `xtask`. Never hand-edit `generated.rs` — edit the
pipeline or the TOML and re-emit. `xtask verify-tables` (run in CI's
`lint` job) checks the two are in sync.

**Shared numeric types (`src/defaults.rs`)**: all public APIs use crate-wide
type aliases rather than raw primitives — `FloatType` (f64), `IntegerType`
(i32), `UnsignedIntegerType` (u32), `FractionType`
(`fraction::GenericFraction<IntegerType>`), `Octave`
(`Option<IntegerType>`, matching music21's absent-octave representation).
Match these aliases rather than introducing new primitive types when
extending public APIs.

**Module structure (`src/lib.rs`)**: each top-level module (`chord`,
`pitch`, `interval`, `key`, `roman`, `tuningsystem`, `polyrhythm`,
`chordsymbol`, `midi`, `abc`, `duration`, `note`, `scale`, `stream`,
`analysis`, `error`) is public and re-exports its key types at the crate
root in `lib.rs`. `common`, `defaults`, `display`, `fraction_pow`, and
`stepname` are crate-private support modules. When adding a public type,
add the re-export in `lib.rs` alongside the existing ones.

**Errors**: a single crate-wide `Error`/`Result` (`src/error.rs`) is used
throughout instead of per-module error types.

**Note/Chord layering**: `GeneralNote` (holds a `Option<Duration>`) →
`NotRest` → `Note` (adds `_pitch`) mirrors music21's class hierarchy;
`GeneralNoteTrait` is the shared duration accessor. `Chord` is *not* part
of that chain — it holds `_notes: Vec<Note>` and its own `duration`
directly. `IntoNotes` (in `src/chord/mod.rs`) is the single conversion
trait for chord inputs (strings, slices, MIDI integers, `Option<T>`);
`IntoNote` is its per-note counterpart. Add new chord input types there,
not in a parallel trait.

**Tuning systems (`src/tuningsystem/`)**: `TuningSystem` covers fixed
ratio-table/equal-temperament systems; `adaptive::AdaptiveTuningSystem`
covers systems whose frequency depends on harmonic context (e.g. recursive
just intonation); `AnyTuningSystem` unifies the two. Ratio tables (Just
Intonation, Pythagorean, Partch's 43-tone scale, etc.) are hand-transcribed
constant arrays — when adding or editing one, verify it against the
canonical source (e.g. `music21/music21/scale/scala/scl/*.scl`), since
`ALL_TUNING_SYSTEMS`-driven tests only catch a bad entry if it breaks
strict ascending order within the octave.

**Feature flags**: default features are empty. `serde` (on `music21-rs`)
adds `Serialize`/`Deserialize` to public types. `python` (on `utils`,
consumed by `xtask` and `python-parity`) gates all pyo3 code — never make
the main library depend on it.

**Examples**: `examples/web` is a wasm-pack/TypeScript app (Chord
Inspector, Chord Browser, Polyrhythm Lab, Tuning Explorer) built by CI's
`docs` job and published to GitHub Pages alongside rustdoc; `examples/audio`
is a small cpal-based polyrhythm playback example. Both are workspace
members with their own `Cargo.toml`.

## Conventions

**Tests live in the module they test.** There is no `tests/` directory —
every test is a `#[cfg(test)] mod tests` block at the bottom of its own
source file, and `cargo test` runs ~210 of them in well under a second.
Put new tests next to the code, and keep them fast enough that the whole
suite stays instant.

**Porting fidelity has a limit.** Mirroring music21's *data model* (the
`GeneralNote`/`NotRest`/`Note` chain, absent octaves as `Option`,
music21's naming and output strings) is deliberate — parity tests depend
on it. Mirroring music21's *runtime machinery* is not: Python's
memoization dicts, `_client` back-references, and observer callbacks were
transliterated here once and became dead weight that hid a real bug (a
`Chord`→`NotRest`→`Arc<ChordBase>` cycle that leaked every chord and made
`Chord::set_duration` a silent no-op). Before porting a Python attribute,
check whether anything in the Rust translation would ever read it.

**Don't launder structured data through strings.** The crate has a
recurring pull toward Python-style stringly-typed plumbing — building an
interval by formatting `"P5"` and re-parsing it, recovering a scale degree
by scraping ASCII digits out of an interval name, reading a pitch letter
out of a `#[derive(Debug)]` impl. Several of these have been removed; when
you have the structured value in hand, pass it through rather than
re-deriving it downstream. Note that `Interval::new`/`Interval::from_name`
still parse from strings — that is the current constructor, but prefer
hoisting a repeated parse into a `LazyLock` static over re-parsing in a
loop.

**Constructors**: `Pitch::new` is a nine-positional-argument transliteration
of music21's keyword-argument `__init__`, and is private to `src/pitch/` for
that reason. Build pitches with the named helpers (`Pitch::from_name`,
`from_number`, `from_midi`, `from_pitch_class`, …) or `Pitch::builder()` /
`PitchOptions`. When adding a construction path, extend `PitchOptions`
rather than giving `Pitch::new` a tenth argument.

**Prefer borrowing intervals.** `Interval` is not `Copy` and carries a
diatonic + chromatic pair, so a by-value parameter that only reads fields
forces every caller to clone. `Interval::transpose_pitch`,
`transpose_pitch_with_options`, and `Pitch::transpose` all take `&Interval`;
keep new interval-consuming APIs the same way unless they genuinely need
ownership.

**Parsers return `Result`, so let them.** `Interval::from_name` used to
`.expect()` on the interval number it scraped out of the input, which turned
every malformed name into a panic out of a `Result`-returning public API.
When a public entry point already returns `Result`, propagate rather than
asserting the input is well-formed. Reserve `.expect()` for crate-internal
constants that cannot fail (`Duration::whole`, the `LazyLock` interval
statics), and give it a message saying why.
