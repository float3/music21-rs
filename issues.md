# Open issues

What is known to be wrong, unfinished or unverified, as of the
`fix-leaks-and-validation` branch (19 commits on `6ee78e7`, nothing pushed).
Every figure here was measured rather than remembered; where a claim came out
of a run, the run is named so it can be repeated.

## Does the wheel depend on music21?

No. `pip install music21-rs` pulls in nothing: the wheel declares no
`Requires-Dist` at all, and `python/pyproject.toml` declares no
`dependencies`. In a virtualenv with no music21 present the wheel imports in
4ms without loading music21, builds chords and answers them, and its
exception classes resolve — standing on a plain `Exception` where music21's
own `Music21Exception` is not there to stand on, while still reporting the
module music21 keeps them in.

music21 is an *optional* integration, used in two places and only when it is
already installed: `install_into_music21()`, which swaps these classes into a
running music21, and the exception base, which prefers music21's own so that
music21's `except Music21Exception` catches what these classes raise.

## Unverified work

1. **The three-sided `music21-suite` has not run since its timing code was
   rewritten.** `target/music21-suite/*.json` is from 2026-09-11 05:02;
   `xtask/src/music21_suite.rs` last changed 2026-09-12 09:37 (`35f6e99`).
   That leaves two changes with only unit tests behind them: taking the
   collector's time off each test, and the guard that stops the linked side
   importing the wheel (`47cb607`). Both are exercised only by a full run:

   ```
   cargo run --release -p xtask --features python -- music21-suite
   ```

2. **None of this has run in CI.** Nineteen commits, nothing pushed. The
   parts CI would exercise first are the new clippy steps for `python` and
   `python-parity`, the wheel crate's test step, and the mixed-layout wheel
   (`python/pysrc`), which has only ever been built on Windows. CI builds it
   on ubuntu and macOS too.

3. **`target/benchmarks.json` is from 2026-09-08.** The benchmark has been
   run since, but to a scratch file; the report would read the stale one.

## Before a release

4. **The version must go to 0.6.0.** `ChromaticInterval::new` and
   `notes_to_chromatic` now return `Result` (`74857d1`), which
   `cargo semver-checks` fails without a minor bump. The bump was written and
   deliberately dropped; `RELEASE_NOTES.md` needs its section back with it.

5. **Regenerate the report.** `target/reports/report.json` carries no
   coverage, no suites and no benchmark figures — the last run was
   `--features-only` — and is stamped `4a4d0f8`, mid-branch.

## Functional gaps

6. **`meter` passes 11 of 34 docstrings, 286 of 387 examples.** Everything
   still failing wants music21's partition tree: `beatSequence` (17
   examples), `getBeams` (6), `accentSequence` (6), a settable `beatCount`
   (5), `beamSequence` (4), `displaySequence` (3), `setDisplay` (2), and one
   `getAccentWeight` modulus case.

   music21 hangs four mutable `MeterSequence`s off every `TimeSignature`;
   this crate's `TimeSignature` is a `Copy` pair of integers, and `slow 6/8`
   differs from `6/8` only in how its beat sequence is partitioned. The
   cheaper shape is a standalone `MeterSequence` type in the crate with the
   four mutable sequences owned by the facade, where music21's mutability
   lives anyway — that keeps `TimeSignature` and its three call sites intact.
   The parsing this needs is done (`96ea594`): `TimeSignature::parts` reads
   `3/8+2/8`, `3+2/8` and `slow 6/8` as music21 writes them.

7. **`sieve` passes 24 of 25 docstrings, and the one that fails is a feature
   this crate does not have.** `Sieve.segment('cmp', segmentFormat='wid')`
   wants the compressed reading of a sieve, which the feature map already
   excludes (`expand`, `compress`, `represent`). The harness records a
   permanently failing docstring by leaving it out of
   `python-parity/doctest/sieve.toml`, which is already the case, so nothing
   here is mislabelled — raising the number means porting music21's
   `Sieve.compress`.

8. **`GeneralNote.getGrace` is answered by the wheel and unlabelled in the
   map.** The facade implements it (`python/src/note.rs:1474`, which swaps in
   the unlinked duration music21 hands back); the crate has no grace concept
   at all, exactly as its sibling `getGraceDuration` says. It was the only
   one of the 52 missing members saying nothing about why, so it now carries
   a reason of its own in `data/feature_map.toml`.

9. Every other module passes its docstrings in full: chord 105/105, pitch
   83/83, interval 107/107, roman 34/34, scale 62/62, duration 53/53,
   voiceLeading 61/61, harmony 25/25, key 18/18, note 33/33, tempo 27/27,
   serial 22/22, beam 13/13, figuredBass 12/12, chord.tables 10/10, volume
   9/9, tie 2/2.

## Performance

10. **An installed chord costs what music21's does.** `Chord('C4 E4 G4')`
    through `install_into_music21` is ~30µs against music21's ~30µs, while
    the bare wheel does it in 8.1µs. The profile says why: one chord builds
    eight objects through the install helper — the chord, three notes, three
    pitches and a duration — and four of them pay music21's own
    `Music21Object.__init__` and `Sites.__init__` in Python. The lever is
    building notes and pitches on demand rather than up front, which has to
    keep `chord.pitches[0] is chord.root()` true, so it is a refactor of the
    facade rather than a tweak.

    One thing was tried here and measured as no help: deciding the object
    half once per class rather than per object, which came to 30.05µs against
    29.45µs and was reverted. The cost is the count of objects, not the
    branching on each one.

## Deliberate, and not issues

- **Three setters take whatever they are given** — `pitch._client`,
  `key.tonic`, `scale.tonic`. Tightening the first to a note or nothing broke
  twenty-nine of music21's own docstrings, because the note written there is
  not always one of ours: a harness that swaps one module's classes and not
  another's hands it music21's own `Note`. The other two validate on the line
  above, and the `.ok()` only records that there is no facade object to hand
  back (`41e1937`).

- **The install helper forgives three exceptions on copy** — `AttributeError`,
  `TypeError` and `RecursionError`, narrowed from a bare `except Exception`
  (`7c8e4de`). A read-only attribute, something a copy cannot reach, and a
  structure deep enough to run the stack out.

- **`xtask bench` imports the installed wheel on purpose.** It measures the
  artifact a caller installs. The guards against loading a second copy of
  these classes belong to the harnesses that test the *linked* crate
  (`1012c5e`, `47cb607`), not to it.

- **29 members answer in the wheel and have no crate function.** Caches,
  observer callbacks, instruments, and anything needing the stream an object
  sits in. The report counts them and says so, so that "the crate is a
  superset of the wheel" is not assumed.
