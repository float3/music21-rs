# Open issues

What is known to be wrong, unfinished or unverified. Every figure here was
measured rather than remembered; where a claim came out of a run, the run is
named so it can be repeated. `DECISIONS.md` says which choices were
deliberate.

## Does the wheel depend on music21?

No. `pip install music21-rs` pulls in nothing: the wheel declares no
`Requires-Dist`, and `python/pyproject.toml` declares no `dependencies`. In a
virtualenv with no music21 present the wheel imports in 4ms without loading
music21, builds chords and answers them, and its exception classes resolve —
standing on a plain `Exception` where music21's own `Music21Exception` is not
there to stand on, while still reporting the module music21 keeps them in.

music21 is an *optional* integration, used only when already installed:
`install_into_music21()`, and the exception base so that music21's
`except Music21Exception` catches what these classes raise.

## The direction this is moving in

Two rules, set 2026-09-12, that most of the open work now serves:

- **The crate is meant to be a strict superset of the wheel**, caches aside.
  As little as possible belongs in the facade — only what genuinely cannot
  live in Rust. 29 members answer in the wheel today with no crate function
  behind them; each is either work to move or a documented exception.
- **The crate should have streams.** It has a `Stream` already
  (`src/stream.rs`, 678 lines: elements at offsets, `flatten`, `recurse`,
  parts, measures, voices, `transpose`, and the `*_at` lookups). What it
  lacks is what music21 uses to make an object a *member* of a stream —
  sites and contexts, derivations, `activeSite`, measure-relative offsets —
  and that gap is why the wheel still pays for music21's own object
  machinery. See entry 7.

## Open

1. **The meter partition tree — the crate half is done, the wheel half is
   not.** `TimeSignature` carries all four of music21's `MeterSequence`s and
   builds them by music21's own rules (`431dcbf`), having given up `Copy`
   first (`0e14f8b`); `beat_count` reads the tree rather than deriving the
   count again (`8aa5882`). Eighteen meters are pinned against the strings
   music21 prints, and the 126-meter fixture is unmoved.

   **The wheel half is done too, and the number moved: 33 of 34 docstrings
   and 375 of 387 examples, from 11 and 288.** The wheel hands back the four
   sequences as music21's own two classes, `beatCount` is settable by a
   number or a list, `setDisplay` and `setAccentWeight` are ported,
   `getAccentWeight` honours the level it is given, the `divisions` argument
   partitions the beats, accents and beams alike, `getOffsetFromBeat` reads a
   fractional beat through `addFloatPrecision` and answers through `opFrac`,
   and a meter says how it was written — which is what makes `2/8+3/8` a
   different meter from `3/8+2/8`.

   **One docstring is left: `getBeams`, twelve examples.** It beams a run of
   notes, so it needs the notes. Every piece it reads is already here: the
   beam sequence, `Beams::numbers`, `by_number` and `set_by_number`, the
   three run-walking helpers, and the `BEAMABLE` table that is music21s
   `beamableDurationTypes`. What is missing is the walk itself -- music21
   fixes each note at each of nine beam depths against the span its level
   covers -- and the facade half that reads a duration and whether it sounds
   off the Python objects, which `naiveBeams` already does. What is left: `beatSequence` (17 examples),
   `getBeams` (6), `accentSequence` (6), a settable `beatCount` (5),
   `beamSequence` (4), `displaySequence` (3), `setDisplay` (2), and one
   `getAccentWeight` modulus case. Under the rule above the facade is a
   pass-through and nothing else — what it needs is a pyclass wrapping
   `MeterTerminal`, since a Python caller has to be handed something.

   Two of those want more than a pass-through and are honest exclusions in
   `data/feature_map.toml` until they are written: `getBeams` beams a run of
   notes and so needs the notes, and `setDisplay` is a setter nobody has
   written.

   One thing the tree bought already, beyond the members it unblocks: the
   closed form it replaced was wrong where music21 is not. It cut compound
   threes at a denominator over four, so `3/6` counted as one beat where
   music21 counts three. Two more of the same kind went with it: every beat
   lookup divided the bar evenly, so a bar written additively was counted
   along a beat that was not there, and `beatDuration` answered an average
   where music21 refuses (`861effb`, `cab6c97`, `8aa5882`).

   **`ratioString` is a known divergence.** music21 reads it off the display
   sequence, so after `setDisplay("2/8+2/8+2/8")` a `3/4` calls itself
   `2/8+2/8+2/8`; the crate still writes it from the numerator and the
   denominator. Found while porting `setDisplay` (`861effb`) and left alone
   rather than changed in passing, since `ratioString` is read in a dozen
   places and is one of the failing docstrings in its own right.

2. **Streams in the crate.** Sites, contexts, derivations and
   measure-relative offsets, so that an object can belong to a crate stream
   without music21's half. This is what would let the facade stop building
   `Music21Object`s, and it is also what would let
   `getMeasureOffsetOrMeterModulusOffset` — added to the facade on the old
   rule (`8e455d9`) — move into the crate where it belongs.

3. **The 29 wheel-only members.** Caches (`cachedRealized`,
   `cachedRealizedStr`) stay Python-side by design. The rest — observer
   callbacks (`informClient`, `pitchChanged`), `groups`, `storedInstrument`
   and `getInstrument`, the `AbstractScale` layer, `Sieve`'s settable state,
   `Duration`'s component machinery — are either work to move into the crate
   or exceptions that should say why in `data/feature_map.toml`.

4. **CI had never run any of this** before today's push. The parts it
   exercises first are the new clippy steps for `python` and `python-parity`,
   the wheel crate's test step, and the mixed-layout wheel (`python/pysrc`),
   which has only ever been built on Windows.

5. **The version must go to 0.6.0 before a release.**
   `ChromaticInterval::new` and `notes_to_chromatic` return `Result`
   (`74857d1`). The bump is deliberately not made — nothing is being released
   yet.

   **The semver gate will not catch this.** `cargo semver-checks` runs
   against `--baseline-rev HEAD^` (`.github/workflows/ci.yml`), so on a push
   it compares the tip commit with its parent, not the release with the
   branch. The push that carried the breaking change was green because the
   tip commit was documentation only; anything breaking that lands more than
   one commit before a push goes unnoticed. A baseline of the last released
   tag would catch it.

6. **`sieve` passes 24 of 25 docstrings.** The one that fails,
   `Sieve.segment('cmp', segmentFormat='wid')`, wants the compressed reading
   of a sieve, which the feature map already excludes. Raising the number
   means porting music21's `Sieve.compress`.

7. **An installed chord costs what music21's does.** `Chord('C4 E4 G4')`
   through `install_into_music21` is ~30µs against music21's ~30µs, while the
   bare wheel does it in 8.1µs. One chord builds eight objects through the
   install helper — the chord, three notes, three pitches and a duration —
   and four of them run music21's `Music21Object.__init__` and
   `Sites.__init__` in Python, because music21 will not keep anything else in
   a `Stream` and construction cannot know whether the object will end up in
   one. Two ways out: build fewer objects (lazy notes and pitches, which has
   to keep `chord.pitches[0] is chord.root()` true), or give the crate
   streams so the wheel need not build music21's half at all — entry 2.

   One thing was tried here and measured as no help: deciding the object half
   once per class rather than per object, 30.05µs against 29.45µs, reverted.

## Closed

- **music21's own suite ran on all three sides** and behaves the same on the
  crate: 5,030 tests on music21, 4,526 on each Rust side, one known
  divergence allowed. It also proved the timing rewrite in situ.
- **Collector pauses are no longer counted as slowness.** Of the rows the
  published page showed below a quarter of music21's speed, **14 became 0**;
  below half, 28 became 1. Medians 1.004; 3,260 tests paired.
- **Coverage was being measured against files that no longer exist.** Stale
  objects from before three modules became directories made llvm-cov report
  them as covered by nothing, and its errors made the parity suite's row read
  as a failure. Fixed (`5c6e3e1`): **96.56% of lines where it read 72.67%**,
  parity row green.
- **The report no longer calls a documented divergence a regression**
  (`14e0817`). The page said music21's suite was red while the command it
  names exits green.
- **`getGrace` has a reason** (`a0ca9c3`). No member is missing without one.
- **The benchmark and the report are regenerated** from today's runs, with
  the crate column filled for every case that has a counterpart.

## Smaller things

- The wheel suites (`Python wheel`, `Python wheel tests`) are skipped unless
  `maturin` and `pytest` are on PATH; they live in `.m21venv/Scripts` here.
  The report records the reason rather than failing, which is right, but the
  page then carries two skipped rows.
- music21's suite spawns workers under the embedded interpreter, where
  `sys.executable` is `xtask.exe`; each worker re-execs it with `-c` and
  dies, printing `unknown xtask command "-c"` a few hundred times into the
  log. It costs no tests — both Rust sides match exactly — but it makes the
  log hard to read. `multiprocessing.set_executable` pointed at a real
  interpreter would silence it.

## Deliberate, and not issues

- **Three setters take whatever they are given** — `pitch._client`,
  `key.tonic`, `scale.tonic`. Tightening the first broke twenty-nine of
  music21's own docstrings: the note written into `_client` is not always one
  of ours (`41e1937`).
- **The install helper forgives three exceptions on copy** — `AttributeError`,
  `TypeError` and `RecursionError`, narrowed from a bare `except Exception`
  (`7c8e4de`).
- **`xtask bench` imports the installed wheel on purpose.** It measures the
  artifact a caller installs; the guards against a second copy belong to the
  harnesses that test the linked crate (`1012c5e`, `47cb607`).
