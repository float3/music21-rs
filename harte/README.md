# harte

[![CI](https://github.com/float3/music21-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/music21-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/harte.svg)](https://crates.io/crates/harte)
[![docs.rs](https://docs.rs/harte/badge.svg)](https://docs.rs/harte)

Harte chord notation, `C:maj7/3`, `Bb:(b3,5,b7,9)`, `F:sus4(*3,9)`, `N`,
read into [music21-rs](https://crates.io/crates/music21-rs) chords. it is a
port of [harte-library](https://github.com/andreamust/harte-library), and
builds the chord harte-library builds for every label in that library's
coverage set.

a label is a root, an optional shorthand, an optional list of degrees in
parentheses and an optional bass degree after a slash. `*3` in the degrees
removes the third. a degree is written the Harte way, `b3`, `#11`, `bb7`.
`N` and `X` name no chord.

## coverage of harte-library

- every one of the 8,064 labels in harte-library's coverage set gives the
  pitches, root, bass, sounding degrees and prettified label the library
  gives, and a label the library rejects is an error here too.
- the 48 shorthands harte-library knows, each with the degrees it stands
  for (`SHORTHAND_DEGREES`).
- not ported: harte-library's chord is a subclass of music21's `Chord`.
  `Harte` holds a music21-rs `Chord` instead, and `into_chord` hands it
  over.

## using the crate

```bash
cargo add harte
```

default features are empty. `serde` adds `Serialize` and `Deserialize` to
`Harte` and `HarteInterval`, and turns on music21-rs's `serde`.

```rust
use harte::Harte;

let harte = Harte::new("Bb:min7/b3")?;
assert_eq!(harte.chord().pitch_names(), ["Bb", "Db", "F", "Ab"]);
assert_eq!(harte.chord().bass().map(|p| p.name_with_octave()), Some("Db3".to_string()));
assert_eq!(harte.root_name(), Some("Bb"));
assert_eq!(harte.shorthand(), Some("min7"));
assert_eq!(harte.bass_degree(), Some("b3"));
assert!(!harte.bass_is_root());

// the degrees a label sounds, its shorthand unwrapped and its removals applied
let sus = Harte::new("F:sus4(*5,9)")?;
assert_eq!(sus.sounding_degrees(), ["1", "4", "9"]);

// the shortest label for the same chord
assert_eq!(Harte::new("C:(b3,5)")?.prettify(), "C:min");

// as MIDI numbers, and as the twelve pitch classes counted from C
let g7 = Harte::new("G:7")?;
assert_eq!(g7.midi_pitches(), [67, 71, 74, 77]);
assert_eq!(g7.multi_hot_encoding(false), [0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1]);

// a single degree, as the music21 interval it stands for
assert_eq!(harte::convert_interval("b7")?, "m7");

// no chord
assert!(Harte::new("N")?.is_empty());
# Ok::<(), harte::Error>(())
```

the full API is on [docs.rs](https://docs.rs/harte).

## browser demo

[hilll.dev/music21-rs/harte](https://hilll.dev/music21-rs/harte/) reads a
Harte label in the browser via wasm and shows the chord it builds, from
[examples/web/harte/](https://github.com/float3/music21-rs/tree/master/examples/web/harte).

## development

harte lives in the [music21-rs](https://github.com/float3/music21-rs)
repository and is released from it, at a version of its own. each harte
release names the music21-rs release it builds against.

```bash
# the crate's tests, including the comparison with harte-library's coverage set
cargo test -p harte --all-features
# regenerate data/harte_expectations.toml from the pinned harte-library checkout
cargo run --release -p xtask --features python -- regenerate-fixtures
```

the comparison reads `data/harte_expectations.toml`, so the tests need
neither Python nor harte-library to run.

## credits

`harte` is released under the
[AGPL-3.0](https://github.com/float3/music21-rs/blob/master/LICENSE). it is
a port of [harte-library](https://github.com/andreamust/harte-library) by
Andrea Poltronieri, licensed
[MIT](https://github.com/andreamust/harte-library/blob/main/LICENSE), and
`data/harte_expectations.toml` is generated from that library's test data.
