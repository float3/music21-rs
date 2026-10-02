# harte

Harte chord notation, read into [music21-rs](https://crates.io/crates/music21-rs)
chords: `C:maj7/3`, `Bb:(b3,5,b7,9)`, `F:sus4(*3,9)`, `N`.

A label is a root, an optional shorthand, an optional list of degrees in
parentheses and an optional bass degree after a slash. `*3` in the degrees
removes the third. A degree is written the Harte way, `b3`, `#11`, `bb7`;
`convert_interval` gives the music21 interval it stands for, and `Harte`
builds the chord the label sounds, with the root fixed as the label names it.

```rust
use harte::Harte;

let harte = Harte::new("Bb:min7/b3")?;
assert_eq!(harte.chord().pitch_names(), ["Bb", "Db", "F", "Ab"]);
assert_eq!(harte.chord().bass().map(|p| p.name_with_octave()), Some("Db3".to_string()));
assert_eq!(Harte::new("C:(b3,5)")?.prettify(), "C:min");
# Ok::<(), harte::Error>(())
```

It is a port of [harte-library](https://github.com/andreamust/harte-library)
by Andrea Poltronieri, licensed
[MIT](https://github.com/andreamust/harte-library/blob/main/LICENSE), and is
checked against every label in that library's 8,064-chord coverage set
(`data/harte_expectations.toml`, generated from the library's test data; the
crate's own tests run the comparison). It lives in the music21-rs repository
and is released from it.
