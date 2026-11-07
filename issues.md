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

- **the crate owns the music, the facade owns Python's object protocol.**
  *Rewritten 2026-10-30 after a debate; it was "the crate is a strict
  superset of the wheel".* The crate holds every musical computation and
  any state that survives a copy or a pickle (`barDuration` moved for that
  reason). The facade holds identity and object references, observer
  callbacks, caches, randomness and the clock, music21's global registries,
  Python standard-library types, deprecated aliases, in-place rebuilds and
  loops over a crate function. A facade member is ported into the crate
  when a caller outside Python (the web example, a Rust user, a parity test)
  needs it, and its reason is in `data/feature_map.toml` either way. Facade
  logic drifts where it duplicates the crate, which the drift audit's fixes
  show; that, not the count of wheel-only members, is what to keep down.
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
   on objects. the wheel keeps sites of its own, as music21's are kept
   (DECISIONS.md, reversed 2026-11-07), which is what music21 code reads;
   the crate's streams keep none. this is what would let the facade stop
   building `Music21Object`s.

   **measure-relative offsets and the beat are in.** a `Placed` now answers
   music21's `getMeasureOffsetOrMeterModulusOffset` (`bar_offset`, a
   measure's `paddingLeft` counted as music21 counts it) and the four
   members music21 hangs on every object through its sites -- `beat`,
   `beatStr`, `beatDuration`, `beatStrength` -- from the meter in force,
   with no site anywhere. music21's own examples for them are its tests.
   the wheel's elements answer them through the wheel's own sites, with
   music21's context search (`getContextByClass`, `contextSites`, `next`,
   `previous`) and meters; `Placed` stays the crate's answer. derivations
   are not started, so neither side follows one.

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

   - standalone, a wheel measure is listed as music21 lists it only where a
     stream is asked by class, flattened or walked: `len`, indexing and
     plain iteration leave its barlines out (DECISIONS.md, "a measure's
     barlines are listed, not held"), and `storeAtEnd` keeps only a
     barline.
   - a wheel measure's repr is `<music21.stream.Measure 1>` where
     music21's is `<music21.stream.Measure 1 offset=4.0>`: the offset is
     the measure's in its part, which the wheel's streams answer through
     `placed()` rather than on the measure. Once installed, readers hand
     back music21's own measures, which have it.
   - after `k.mode = 'minor'` a wheel key's `pitches` follow the new mode,
     where music21's keep the old one; a pickle of such a key forgets the
     old mode.

   Settled by debate on 2026-10-30 and recorded in DECISIONS.md: readers
   hand back music21's streams once installed; flags are read for their
   truth; negative lengths stay refused. Settled on 2026-10-31: a key
   follows its tonic edited in place and keeps its signature, as music21's
   does; members that need a site say why they are refused (reversed on
   2026-11-07: the package keeps sites, and they answer).

9. **edits through a handed-out object that may still miss the crate.**
   `data/py_fields.toml` lists them as *value*.

   The four suspected on 2026-11-01 were checked against music21 and
   fixed (below, under closed), and the other rows listed as *value* were
   made to read the object as it stands; none is left.

10. **an uninstalled music21 object's length or priority is read only
    when it goes into a wheel stream.** An edit to it tells only
    music21's own sites, so a wheel stream holding it reads its lengths
    afresh on every append and does not re-sort when its priority
    changes. The package's elements and installed classes tell the
    wheel's streams (DECISIONS.md, "streams hear of element edits").

11. **music21 bugs the seeded fuzzers step around**, not yet fixed on the
    fork:
    - `k = key.Key('G'); k.mode = 'minor'` leaves `k.pitches` listing F#.
    - `n = note.Note(quarterLength=0.25)`, then
      `n.duration.appendTuplet(duration.Tuplet(5, 4))` and
      `n.quarterLength = 1.5` keeps the tuplet, so the note is written as
      1.2.
    - a 3/4 part with `Note('C4', quarterLength=1/3)` at 0 and
      `Note('D4', quarterLength=0.25)` at 1/12 makes
      `GeneralObjectExporter(p).parse()` raise `MeterException: cannot
      access qLenPos 3.0`; makeNotation fuzzer seeds 491, 802, 894, 1061
      and 1962 hit it.

12. **a multi-measure rest cut short is a spanner here and not in
    music21's score.** When the part ends, or another one starts, before
    it has taken all its rests, music21's score holds no spanner; only its
    rests know it, which is how music21's writer still writes it. The
    written files agree; `spannerBundle` does not. The reader also takes a
    count written `3.0`, which music21 refuses.

13. **what the wheel's sites do not keep yet.** derivations are not kept,
    so no context search follows one; a spanner is not a site of the
    notes it spans; the context search's sort leaves out music21's
    `isNotGrace`; `getContextByClass` costs about 100µs a note; a stream
    unpickled may not be a site of what it holds until it is written to;
    `returnSpecial` is ignored.

14. **classes music21 has between the wheel's and its base classes.**
    `Unpitched` is missing, and so are `ChordBase`, `StreamCore`,
    `TempoIndication`, `IntervalBase`, `Expression`, `RepeatMark` and
    others music21 puts between an element and `Music21Object`. The 21
    pairs of classes `issubclass` answers otherwise than music21 for are
    listed in the wheel tests' `_HIERARCHY_GAPS`.

15. **what `provide_music21()` cannot provide yet.** `common`,
    `environment`, `musicxml` and `tinyNotation` are not provided at all.
    Partial, in music21's public names: `converter` 6 of 20 (`parse`,
    `parseFile`, `parseData`, `parseURL`, `toData` and the exceptions; no
    subconverters or archive manager), `corpus` 5 of 16 (no search, no
    `Corpus` classes), `stream` 11 of 21, `note` 10 of 18, `metadata` 12
    of 20, `layout` 6 of 22, `voiceLeading` 8 of 20, `sieve` 8 of 26
    (`generate_homes.py --coverage`, 1,073 of 1,938 in all). A provided
    corpus is read from `MUSIC21_RS_CORPUS`. Five names are placed where
    music21 lacks them: `metadata.MetadataException`,
    `omr.correctors.OmrException`, `tsvConverter.TsvException`,
    `makeNotation.MakeNotationException` and `sieve.Primes`. A provided score's metadata names its software as
    `music21_rs` and its version.

16. **braille segments: what is still music21's or differs.**
    `splitMeasure` is music21's; a grouping split in two does not rebeam
    the objects' `beams`; a standalone object's braille marks go on its
    editorial; on one piano score the parts iterate in another order than
    music21's at equal offsets after transcribing.

17. **`Trigram.makeWords`, `likely` and `lut` are unported in the
    wheel.** They answer at random, and porting them needs the counts kept
    in insertion order.

## closed

- **appending to a wheel stream keeps its end** (was 10). 2,000 appends
  take 3.1ms where music21's take 17.7ms, from 47.8ms: two counts stand
  in for music21's `informSites`. `replace` now puts the replacement
  after what stands at its offset, and a second insert of one object is
  refused with music21's `StreamException`.
- **a priority changed after insertion re-sorts a wheel stream** (was
  13), through the same count; installed classes tell the package's
  streams first.

- **edits through a handed-out object now reach the crate** for an
  accidental's style colour, a scale's tonic, a metronome mark's
  `_tempoText`, and a rest's colour, which the crate's `Rest` now carries.
  A note's or chord's `storedInstrument` edited in place is read too, so
  `unbundleInstruments` and `partitionByInstrument` answer the instrument
  as it now stands. A `FiguredBassScale` counts in its `realizerScale` and
  spells a chromatic bass by its `keySig` as they stand, assigned or edited
  in place, since music21 reads both afresh on every call. A
  `VoiceLeadingQuartet` hears its key as the key stands, the two ways
  music21's edited key reads: `modalOpening` and `clausulaVera` by the
  tonic it has now, `isProperResolution` in the scale its signature
  implies. 3,000 random quartets agree with music21, half with the tonic
  edited. `Chord.inversionName` and `inversionText` name the inversion
  `inversion()` answers, so one fixed through `_overrides` is named
  (`_overrides['inversion'] = 2` on a triad is `64`, `Second Inversion`).
  A deep copy of a pitch keeps its accidental's style, which music21's own
  writer relies on once installed.

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
- **an inversion fixed past every ordinal reads `Unknown Position`.**
  music21's `inversionText` raises `IndexError` off its ordinals list for
  `_overrides['inversion'] = 300`; `inversionName` refuses it by name on
  both sides.
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
