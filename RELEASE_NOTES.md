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
