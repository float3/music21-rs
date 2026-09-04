# Unreleased

Cleanup of the Python-shaped plumbing that remained in the pitch, note and
chord constructors. Nothing musical changes; a handful of signatures do, so
the next release needs a minor bump.

## Breaking Changes

- `Chord::empty` returns `Chord` rather than `Result<Chord>`; it cannot fail.
- `Note::from_pitch` returns `Note` rather than `Result<Note>`, and
  `From<Pitch>` / `From<&Pitch>` replace the `TryFrom` impls that could not
  fail. `Note::from_number` is new and takes a pitch-space number.
- Under the `serde` feature, `Pitch`, `Accidental`, `Microtone`, `Chord` and
  `Note` no longer serialize their fields with a leading underscore
  (`"step"` rather than `"_step"`). Data written by 0.3.0 will not
  deserialize unchanged.
- The `test` binary is gone. It was auto-discovered from `src/bin/test.rs`
  and shipped with the crate; its checks are now a unit test.

## Internal

- `Pitch::from_options` is the constructor; the nine-argument positional
  `Pitch::new` and the `IntoAccidental`, `IntoCentShift`, `IntoPitchName` and
  `IntoPitch` traits, half of whose impls were `panic!()` stubs, are gone.
- `Interval::from_name` and `from_semitones` build intervals directly rather
  than through an `IntervalArgument` enum. `IntervalBaseTrait` borrows its
  interval and pitch instead of consuming them, and `Specifier` is `Copy`.
- `interval_to_pythagorean_ratio` no longer keeps a process-wide
  `Mutex<HashMap>` of every ratio it has computed.
- The chord root-finding walk lives once in `chord::root`; `Chord`,
  `chordsymbol` and `roman` share it and its `pitch_class` helper.
- Private fields drop their `_` prefix, and `spelling_is_infered` is spelled
  correctly.

# music21-rs 0.3.0

This release removes a layer of music21's Python runtime that had been
transliterated into Rust without ever being connected to anything, and fixes
two bugs that were hiding inside it. The minor bump is required because
`fraction` is a public dependency: `FractionType` is re-exported from the
crate root and returned by `Interval::pythagorean_ratio`, so moving to
`fraction` 0.16 is a breaking change for callers who name those types.

## Breaking Changes

- Updated `fraction` to 0.16 and `itertools` to 0.15. `FractionType`
  (`fraction::GenericFraction<IntegerType>`) appears in the public API, so
  downstream crates that mention it must move to `fraction` 0.16 as well.
- `Interval::from_name` and `Interval::new` now return `Error::Interval` for
  a name with no interval number in it, instead of panicking. Code that
  relied on the panic (`""`, `"X"`, `"perfect"`) now sees an `Err`.

## Bug Fixes

- Fixed `Chord::set_duration` and `Chord::with_duration`, which silently did
  nothing on any chord that had notes in it. The chord's duration lived
  behind an `Arc<ChordBase>` that every note in the chord also referenced, so
  the `Arc::get_mut` in the setter never succeeded. `Chord::new("C E G")
  .with_duration(Duration::whole())` returned a chord with no duration; only
  the empty chord worked. The same reference cycle leaked each chord's
  internal note list.
- Fixed `Interval::from_name` panicking out of a `Result`-returning API on
  malformed interval names.
- `PitchOptions::fundamental` now does something. The value was stored on
  `Pitch` and had no reader anywhere in the crate, so it could be set but
  never observed. Added `Pitch::fundamental()`.

## Highlights

- Added `Pitch::fundamental()`.
- Pitch names are no longer formatted out of a `#[derive(Debug)]` impl.
  `Pitch::name` and the ABC and scale helpers now spell the step letter
  explicitly, so the output no longer depends on `StepName`'s variant names.
- Transposition no longer clones the interval. `Interval::transpose_pitch`
  and `Pitch::transpose` read the interval through a reference internally
  instead of taking it by value, so a `Chord` or scale walk no longer copies
  a diatonic/chromatic pair per note. Public signatures are unchanged.
- `interval_to_pythagorean_ratio` no longer holds its cache lock across the
  circle-of-fifths walk, so concurrent callers queue only on the map access,
  and no longer re-parses `"P5"`/`"-P5"` from strings on every call.
- Removed roughly 600 lines of unreachable internals: the `_client` observer
  chains on `Pitch` and `Accidental` (only ever assigned `None`), the
  `HashMap<String, String>` caches on `Note` and `ChordBase` (never read),
  `ChordBase._overrides`, `Pitch._overriden_freq440`, and the `ChordBase` /
  `IntoNotRests` layer that duplicated the note list `Chord` already held.

# music21-rs 0.2.2

This release reworks the tuning-system API around context-dependent tunings
and moves the Python parity suite out of the main workspace.

## Breaking Changes

- Removed the `TuningSystem::StepMethod` and
  `TuningSystem::RecursiveEqualTemperament` variants, both of which computed
  plain equal temperament. `ALL_TUNING_SYSTEMS` is now 15 entries rather
  than 17.

## Highlights

- Added the `tuningsystem::adaptive` module and `AdaptiveTuningSystem` for
  tunings whose frequencies depend on harmonic context, plus the
  `AnyTuningSystem` enum unifying those with the fixed ratio-table systems,
  with `frequency_at`, `cents_at`, and `is_adaptive`.
- Added `TWELVE_TONE_NAMES_SHARP` and `TWELVE_TONE_NAMES_FLAT`.
- Improved Just Intonation ratio accuracy.
- Moved the Python parity tests into a separate `python-parity` package
  outside the cargo workspace, so `cargo test` and `cargo check` on the
  library no longer pull in pyo3.
- Added `xtask verify-tables`, which CI runs to assert the committed chord
  table source still matches `data/chord_tables.toml`.
- Removed the dormant internal `base` and `prebase` modules.

# music21-rs 0.2.1

This patch release fixes a Pythagorean tuning table ordering issue and improves
the Tuning Explorer browser workflow.

## Highlights

- Fixed the Pythagorean tuning ratios so the twelve-tone chromatic degrees stay
  in ascending frequency order, including `Bb` below `B`.
- Added shareable URLs to the Tuning Explorer, preserving the selected tuning
  system, root frequency, and selected degree.
- Added a major-scale playback button for twelve-tone tuning systems, with
  nearest-degree suggestions for non-twelve-tone systems.
- Updated twelve-tone Tuning Explorer labels to use unambiguous flat spellings
  such as `Bb4`.

# music21-rs 0.2.0

This release continues the browser-demo work from `0.1.x` and cleans up several
pre-1.0 pitch APIs so the Rust surface more closely resembles Python
`music21`.

## Breaking API Changes

- Removed the legacy `PitchAccidental` and `PitchMicrotone` builder wrapper
  types.
- Added public `Accidental`, `Microtone`, and `PitchClass` structs with
  companion `AccidentalSpecifier`, `MicrotoneSpecifier`, and
  `PitchClassSpecifier` input enums.
- Updated `PitchOptions` and pitch builders to use those specifier types
  directly.

## Highlights

- Added `Pitch::accidental()`, `Pitch::microtone()`, and
  `Pitch::pitch_class()` accessors.
- Added public accidental helpers for names, modifiers, unicode display,
  non-standard values, and display metadata.
- Added public microtone helpers for cents, harmonic shifts, and music21-style
  formatting.
- Added normalized public pitch-class values with music21-style `A`/`B`
  display for pitch classes 10 and 11.
- Added immediate playback when selecting a suggested resolution in the Chord
  Inspector.
- Added per-resolution preview buttons that play the current chord followed by
  the suggested resolution without changing the page.
- Added hover/focus notation previews for suggested resolutions, showing the
  current chord and hovered resolution side by side on one ABCJS staff.
- Added a CI TypeScript build step for the web demos and included generated web
  JavaScript in the GitHub Pages artifact checks.

# music21-rs 0.1.1

This patch release adds a few browser-facing theory workflow improvements on
top of `0.1.0`.

## Highlights

- Added MIDI-number input support to the Chord Inspector. Inputs like
  `60 64 67`, `60,64,67`, and `midi: 60 64 67` analyze as MIDI notes.
- Added Web MIDI support to the Chord Inspector so a connected MIDI device can
  feed the currently held notes into the analyzer.
- Added a MIDI column to the pitch table and changed pitch display spelling from
  music21 flats such as `A-5` to browser-facing names such as `Ab5`.
- Added a `Class` help widget in the Chord Inspector pitch table.
- Added a Chord Browser at `/chords` listing all 351 unpitched entries in the
  music21-derived chord table, with links back into the inspector.
- Expanded the Chord Browser with a root selector, realized pitches, and
  per-inversion inspector links.
- Added range filtering to the Chord Browser note-count control.
- Listed directed dyad inversions such as major second and minor seventh as
  separate Chord Browser rows, with interval-class labels kept as aliases.
- Moved the Chord Browser frontend source to TypeScript, with browser-served
  JavaScript generated from that source.
- Added resolution-chord links to Chord Browser rows when the realized chord has
  suggestions.

# music21-rs 0.1.0

This release expands `music21-rs` from a chord-name port into a broader set of
interactive music-theory tools and supporting Rust APIs.

## Highlights

- Added a browser demo suite published from `examples/web`:
  - Chord Inspector at `/chord`
  - Polyrhythm Lab at `/polyrhythm`
  - Tuning Explorer at `/tuning`
  - a root index page linking the demos and docs
- Added simple chord-resolution suggestions to the Rust `Chord` API and the
  Chord Inspector.
- Added chord playback, ABC staff notation, random chord generation, clickable
  keyboard toggles, history controls, shareable URLs, and an "open as
  polyrhythm" bridge to the Chord Inspector.
- Added a Polyrhythm Lab with playback, random rhythm settings, shareable URLs,
  history controls, track mute/edit/remove controls, ABC rhythm notation, and
  chord-equivalence links.
- Added a Tuning Explorer for the tuning systems exposed by the crate, including
  scale playback and per-degree ratio/frequency/cents data.
- Updated CI to build and smoke-check the full browser demo site on every run.
- Cleaned up the README and package description to reflect the current crate
  scope.

## Notes

- The browser demos share one WASM crate at `examples/web` rather than keeping
  the Rust glue under the chord demo.
- APIs are still pre-1.0 and may continue to change as more of `music21` is
  ported.
