# open issues

what is known to be wrong, unfinished or unverified. every figure here was
measured rather than remembered; where a claim came out of a run, the run is
named so it can be repeated. `DECISIONS.md` says which choices were
deliberate.

## does the wheel depend on music21?

no. `pip install music21-rs` pulls in nothing: the wheel declares no
`Requires-Dist`, and `python/pyproject.toml` declares no `dependencies`. in a
virtualenv with no music21 present the wheel imports in 4ms without loading
music21, builds chords and answers them, and its exception classes resolve —
standing on a plain `Exception` where music21's own `Music21Exception` is not
there to stand on, while still reporting the module music21 keeps them in.

music21 is an *optional* integration, used only when already installed:
`install_into_music21()`, and the exception base so that music21's
`except Music21Exception` catches what these classes raise.

## the direction this is moving in

two rules, set 2026-09-12, that most of the open work now serves:

- **the crate is meant to be a strict superset of the wheel**, caches aside.
  As little as possible belongs in the facade — only what genuinely cannot
  live in Rust. 124 members answer in the wheel today with no crate
  function behind them, and each carries its reason in
  `data/feature_map.toml`.
- **the crate should have streams.** it has a `Stream` already
  (`src/stream.rs`, 678 lines: elements at offsets, `flatten`, `recurse`,
  parts, measures, voices, `transpose`, and the `*_at` lookups). what it
  lacks is what music21 uses to make an object a *member* of a stream —
  sites and contexts, derivations, `activeSite`, measure-relative offsets —
  and that gap is why the wheel still pays for music21's own object
  machinery. see entry 7.

## open

1. **the meter partition tree — done.** `meter` passes **34 of 34 docstrings
   and 387 of 387 examples**, from 11 and 288 when this started.

   `TimeSignature` carries all four of music21's `MeterSequence`s and builds
   them by music21's own rules (`431dcbf`), having given up `Copy` first
   (`0e14f8b`); `beat_count` reads the tree rather than working the count out
   again (`8aa5882`). the wheel hands the four sequences back as music21's own
   two classes (`9b43d3d`), `beatCount` takes a number or a list, `setDisplay`
   and `setAccentWeight` are ported, `getAccentWeight` honours its level, the
   `divisions` argument partitions the beats, accents and beams alike
   (`444fca5`), `getOffsetFromBeat` reads a fractional beat through
   `addFloatPrecision` and answers through `opFrac`, a meter says how it was
   written (`4941983`), and `getBeams` beams a run of notes against the beam
   sequence (`189b8c9`).

   The beaming walk is the crate's, at nine depths against the span each
   level covers; the wheel reads four things off each object — where it sits,
   how long it lasts, what it is written as, and whether it sounds — and hands
   them over. where it sits needed care: music21 reads `el.offset` and gets
   away with it for a bare list because it appends that list to a fresh
   `Measure` first, so the wheel lays a list end to end and reads a stream's
   own offsets.

   `TimeSignature` is 31 of 33 members ported, the two left being exceptions
   with reasons in `data/feature_map.toml`: `resetValues` and `load` rebuild
   a meter in place where a new one is built instead.

   Eighteen meters are pinned against the strings music21 prints for their
   beat and beam sequences, and the 126-meter fixture has not moved through
   any of it.

   One thing the tree bought beyond the members it unblocks: the closed forms
   it replaced were wrong where music21 is not, three times over. compound
   threes were cut at a denominator over four, so `3/6` counted as one beat
   where music21 counts three; every beat lookup divided the bar evenly, so a
   bar written additively was counted along a beat that was not there; and
   `beatDuration` answered an average where music21 refuses
   (`8aa5882`, `86c637d`, `cab6c97`). the 126-meter fixture could not catch
   any of them, because every meter in it divides evenly.

2. **streams in the crate.** contexts, derivations and measure-relative
   offsets, so that an object can belong to a crate stream without
   music21's half. not sites: the crate keeps its stream a tree of plain
   values and answers where an element sits with `Stream::placed`, the
   composite "Music21's Mistakes" (point 2) recommends over storing sites
   on objects. the wheel still keeps music21's sites for compatibility. this is what would let the facade stop building
   `Music21Object`s.

   **measure-relative offsets and the beat are in.** a `Placed` now answers
   music21's `getMeasureOffsetOrMeterModulusOffset` (`bar_offset`, a
   measure's `paddingLeft` counted as music21 counts it) and the four
   members music21 hangs on every object through its sites -- `beat`,
   `beatStr`, `beatDuration`, `beatStrength` -- from the meter in force,
   with no site anywhere. music21's own examples for them are its tests.
   the wheel does not read them yet: its elements still answer through
   music21's half, which is the step left. derivations are not started.

   It is also what the wheel waits on to carry `FiguredBassLine` and
   `Realization`, which build their bass line and realizations as music21
   parts. music21's own classes run on the crate's objects there meanwhile.

   **It is not what the analyses waited on.** `KeyWeightKeyAnalysis`,
   `Ambitus` and `MelodicIntervalDiversity` only read a stream, and the
   wheel has read one into the crate since `chordify`: the package's
   `Stream.analyze` answers both the key and the ambitus over that bridge,
   with no contexts anywhere. What is left of them in the wheel is the
   classes themselves and `MelodicIntervalDiversity`'s counts.

3. **the 124 wheel-only members.** every one is an exclusion with a
   reason in `data/feature_map.toml`; none is left undecided. caches
   (`cachedRealized`, `cachedRealizedStr`) stay Python-side by design, and
   most of the rest are observer callbacks (`informClient`,
   `pitchChanged`), music21's own object machinery (`Spanner`'s by-object
   members, `Stream.elementOffset`), or steps the crate runs inside one
   function (`ChordReducer`, `Hasher`). the count is read off the page
   `xtask report --features-only` writes; it grew from 29 because the map
   now lists far more classes, not because members moved out.

   **`barDuration` no longer keeps state in the facade.** its setter kept
   music21's `_overriddenBarDuration` as a Python object in the wheel; the
   crate's `TimeSignature` now carries it (`set_bar_duration`), so a copy,
   a pickle and every crate reader of `bar_duration` see it. the one thing
   given up is identity: `ts.barDuration is d` is false, as no music21
   example asks.

   **What the count still cannot see.** a member can be ported and keep
   state only in the facade. 146 fields of the wheel's classes hold Python
   objects. many hold identity rather than value — the `Duration` a note
   hands back, the chord a note sits in — which Python callers rely on and
   the crate cannot carry; which of them hold only a value, as
   `barDuration` did, is not yet sorted.

4. **CI has now run all of this green**, music21's own suite and the docs
   build included. two things it caught that no local gate did: `cargo fmt`
   reaches only workspace members, so a hand-widened line in `python-parity`
   went unformatted through every local check and failed lint — formatting
   has to be asked of all three manifests — and a run is cancelled when
   another push supersedes it, so a green tick has to be read against the
   commit it actually covers.

5. **releases and the semver gate — done.** the breaking changes this entry
   listed shipped in 0.6.0, whose notes say so under "Breaking Changes",
   and the crate is at 0.7.1. `cargo semver-checks` now takes the last
   release tag as its baseline (`.github/workflows/ci.yml`), falling back to
   the previous commit only where no tag exists, so a breaking change fails
   there until the version is bumped rather than slipping past a
   documentation-only tip.

6. **`sieve` — done.** it passes 25 of 25 docstrings and 107 of 107
   examples, `Sieve.compressed` having brought in the compressed reading the
   last one wanted.

7. **an installed chord costs what music21's does only once its notes
   are asked for.** measured on one Linux machine, best of five runs of
   20,000 (`Chord('C4 E4 G4')` through `install_into_music21`):

   | asked for          | before | now    | music21 |
   |--------------------|--------|--------|---------|
   | the chord alone    | 3.3µs  | 3.3µs  | 21.2µs  |
   | its `pitches`      | 26.8µs | 8.9µs  | --      |
   | its `root()`       | 11.8µs | 6.9µs  | --      |
   | its `notes`        | 25.6µs | 25.8µs | --      |

   music21 builds notes and pitches with the chord, so its one figure
   stands for every row.

   the notes were already lazy; what was left was that `pitches` built all
   three and `root()` the one it named, and each note runs music21's
   `Music21Object.__init__` and `Sites.__init__` in Python. a chord now
   hands out pitch objects on their own, and a note built later takes the
   one already out, so `chord.pitches[0] is chord.root()` and
   `chord.pitches[0] is chord[0].pitch` both hold whichever is asked first.
   an edit through such a pitch is written into the chord directly.

   what is left is the notes themselves: music21 will keep nothing but a
   `Music21Object` in a `Stream`, so a note has to pay for its half of
   that. giving the crate streams so the wheel need not build music21's
   half is entry 2.

   One thing was tried here and measured as no help: deciding the object half
   once per class rather than per object, 30.05µs against 29.45µs, reverted.

8. **older mismatches the drift audit turned up.** music21's 346
   non-owner commits since porting began (7e77707) were checked against
   the port on 2026-10-30; the drifts found are fixed. These differ from
   music21 too, but music21 already behaved this way at 7e77707, so they
   were never followed rather than left behind:

   - the wheel's hand-back loses humdrum tandems and `!!` comments. Its
     measures keep their barlines beside their elements, so a Humdrum or
     RomanText measure's barline or repeat is its `rightBarline` but not
     one of its elements, as music21's is (stored at the end). Its readers hand back the wheel's own
     streams even with music21 installed, so an installed note in one
     answers music21's `offset` as 0.0: it has no site. Handing back
     music21's streams there is a design call, not yet made.
   - wheel argument handling: negative quarter lengths refused, non-bool
     truthy flags refused, `Stream.isAtEnd` and `storeAtEnd` missing.
   - a wheel measure's repr is `<music21.stream.Measure m1>` where
     music21's is `<music21.stream.Measure 1 offset=4.0>`: the offset is
     the measure's in its part, which needs the sites the wheel's streams
     do not keep.

## closed

- **random comparison with music21 found what the fixtures could not.**
  Chords, pitches, intervals, keys, Roman numerals, meters and the
  analysis modules were asked the same questions as music21 over thousands
  of random inputs. eleven differences came out and are fixed: dissonance
  respelling (`[3, 6, 9]`), `closedPosition`'s bass, pitch frequency to the
  bit, augmented-sixth names in every voicing, four `RomanNumeral`
  properties, a key's scale members, a degree's case in modal keys,
  additive meters' divisions, and three wheel members that needed music21
  installed. then `ChordSymbol`: degrees read through Python's `int`, the
  voicing's root and bass objects moving with the chord, and
  `pitchedCommonName` asking the root a chord answers; 9,972 figures agree.
  One difference stays, deliberately: a scale's `nextPitch` takes
  `direction` as a string where music21 insists on its enum.

- **music21's own suite ran on all three sides** and behaves the same on the
  crate: 5,030 tests on music21, 4,526 on each Rust side, one known
  divergence allowed. it also proved the timing rewrite in situ.
- **collector pauses are no longer counted as slowness.** of the rows the
  published page showed below a quarter of music21's speed, **14 became 0**;
  below half, 28 became 1. medians 1.004; 3,260 tests paired.
- **coverage was being measured against files that no longer exist.** stale
  objects from before three modules became directories made llvm-cov report
  them as covered by nothing, and its errors made the parity suite's row read
  as a failure. fixed (`5c6e3e1`): **96.56% of lines where it read 72.67%**,
  parity row green.
- **the report no longer calls a documented divergence a regression**
  (`14e0817`). the page said music21's suite was red while the command it
  names exits green.
- **`getGrace` has a reason** (`a0ca9c3`). no member is missing without one.
- **the benchmark and the report are regenerated** from today's runs, with
  the crate column filled for every case that has a counterpart.

## smaller things

- the wheel suites (`Python wheel`, `Python wheel tests`) are skipped unless
  `maturin` and `pytest` are on PATH; they live in `.m21venv/Scripts` here.
  The report records the reason rather than failing, which is right, but the
  page then carries two skipped rows.
- **the suite's workers start Python now — done.** they were started by
  running `sys.executable`, which under the embedded interpreter is
  `xtask.exe`, so each worker ran xtask with `-c`, xtask read `-c` as a
  command it does not have, and the worker died: **1,295** refusals in one
  run's log. `multiprocessing.set_executable`, pointed at the interpreter
  `sys._base_executable` or `sys.base_prefix` names, brings that to **0**.
  It cost no tests either way — 5,154 run on each side, with the same
  failures before and after — so it was a log that could not be read
  rather than a gap.

## deliberate, and not issues

- **three setters take whatever they are given** — `pitch._client`,
  `key.tonic`, `scale.tonic`. tightening the first broke twenty-nine of
  music21's own docstrings: the note written into `_client` is not always one
  of ours (`41e1937`).
- **the install helper forgives three exceptions on copy** — `AttributeError`,
  `TypeError` and `RecursionError`, narrowed from a bare `except Exception`
  (`7c8e4de`).
- **a `<transpose>` with no `<chromatic>` reads as a unison.** music21
  reads a generic interval, but MusicXML requires `<chromatic>`, the
  crate's instrument holds a full interval, and music21's own writer
  cannot write a generic one back.
- **`xtask bench` imports the installed wheel on purpose.** it measures the
  artifact a caller installs; the guards against a second copy belong to the
  harnesses that test the linked crate (`1012c5e`, `47cb607`).
