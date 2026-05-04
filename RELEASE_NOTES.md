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
