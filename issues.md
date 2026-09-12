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

1. **The meter partition tree.** `meter` passes 11 of 34 docstrings, 286 of
   387 examples. What still fails wants music21's four `MeterSequence`s:
   `beatSequence` (17 examples), `getBeams` (6), `accentSequence` (6), a
   settable `beatCount` (5), `beamSequence` (4), `displaySequence` (3),
   `setDisplay` (2), and one `getAccentWeight` modulus case.

   Under the rule above this goes in the crate, not the facade: the sequences
   and the partitioning are Rust, and the facade becomes a pass-through.
   `TimeSignature` therefore stops being a `Copy` pair of integers and owns
   its partitions, which reaches 41 by-value method signatures, the
   `StreamElement::TimeSignature` variant, `Stream::time_signature_at` and
   the copy in `Stream`'s transposer. The parsing it needs is done
   (`96ea594`): `TimeSignature::parts` reads `3/8+2/8`, `3+2/8` and
   `slow 6/8` as music21 writes them.

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
   (`74857d1`), which `cargo semver-checks` fails without a minor bump. The
   bump is deliberately not made — nothing is being released yet — so that
   job is expected to be red until it is.

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
