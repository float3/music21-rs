# Opinionated decisions

Choices in this repository that could reasonably have gone the other way, why
they went the way they did, and what each one costs. A decision is here if
reversing it would be a design change rather than a bug fix.

Read this beside `CLAUDE.md`, which says how the pieces work; this says why
they are shaped that way. `issues.md` says what is still wrong.

## The crate

**The crate models music, not scores.** `Stream`, parsing, notation output
and the corpus stay music21's. Everything here answers a question about a
pitch, an interval, a chord, a meter or a tuning, and answers it without
knowing where in a piece the object sits.

*Costs:* every member needing an object's place in a stream —
`getMeasureOffsetOrMeterModulusOffset`, `Volume.getDynamicContext`,
`realizeVolume` — can only be answered in the Python facade, which is why 29
members are carried by the wheel and not the crate.

**Nothing is cached.** A value that answers the same question twice does the
arithmetic twice. music21 memoizes on the object; this does not.

*Costs:* `Chord.forteClass` asked twice costs twice, where music21's second
answer is free. The benchmark shows it: warm-object cases are the only two
where music21 is within reach. The wheel does cache, because a facade object
is asked the same question repeatedly by music21's own code.

**Errors are values.** Every fallible operation returns `Result`; nothing
panics on bad input. The `panic!`s and `expect`s that exist are on constant
literals — `Interval::from_name("P5")` — where failure would be a bug in this
file rather than in a caller's data.

*Costs:* signatures are noisier, and a constructor that could be infallible
often is not. It also means a value nobody can answer is refused rather than
answered wrongly — which is why `ChromaticInterval::new` became fallible and
why non-finite pitch space, frequencies and tempi are errors.

**`#![forbid(unsafe_code)]`.** The crate contains no unsafe Rust, and cannot
grow any without the attribute coming off. The wheel denies rather than
forbids it, with exactly one allow, because a pyo3 exception type standing on
music21's own base needs a hand-written `PyTypeInfo`.

**Generated tables are committed.** The chord tables, the tuning tables and
the Scala archive are emitted into Rust by `xtask` and checked in, so a
normal build needs no Python and no submodule. Each generated file carries
the command that regenerates it, and CI verifies the file still matches the
TOML it came from.

*Costs:* ~7k lines of generated Rust in the tree, and a regeneration step
whenever a submodule moves.

**`TimeSignature` is an immutable ratio.** music21's owns four mutable
partition trees; this one is a `Copy` pair of integers that derives its
beats, accents and divisions from the ratio.

*Costs:* `beatSequence`, `beamSequence`, `accentSequence`, `displaySequence`,
`getBeams` and a settable `beatCount` cannot be answered, which is 23 of
meter's 34 docstrings. See `issues.md`; the plan is a separate
`MeterSequence` type with the mutable sequences owned by the facade, so the
ratio type stays as it is.

## The Python facade

**Installed classes are Python subclasses of the facade and of the class they
replace.** `install_into_music21()` builds, for each name, a class deriving
from the pyo3 class and from music21's own, so music21 will keep it in a
`Stream` — it has the offset, the sites and the context machinery — while
every musical question is answered in Rust.

*Costs:* a Python `__init__` on the hot path. An installed `Chord('C4 E4 G4')`
costs about what music21's does, because building one makes eight objects
that each start music21's half. The bare wheel does the same chord in 8.1µs
against music21's 30µs.

**Anything not ported is blocked, never inherited.** A member the facade
lacks raises `AttributeError` rather than falling through to music21's
implementation. A partial port that silently answered from music21 would make
the parity numbers meaningless.

**Exceptions stand on music21's own base, and are built on first asking.**
music21 catches `Music21Exception` throughout, so an exception of ours that
was not one would escape its own reader. Building one imports music21, so
they are built when first named rather than at import: `import music21_rs`
costs 4ms rather than 519ms, and the wheel needs no music21 to run.

*Costs:* a hand-written `unsafe impl PyTypeInfo`, and a wheel used standalone
reports `music21_rs_facade` as an exception's module until something installs
or names it. Installing stamps all 27 first, because music21's doctests read
those module strings.

**The wheel depends on nothing.** No `Requires-Dist`. music21 is an optional
integration, present or not.

**Facade objects carry the Python objects music21 hands them.** A note keeps
its `Duration`, `Volume`, `Beams`, `Tie`, expressions and articulations as
the Python objects they were given, so `chord.duration is d` and notation
written through one of them sticks.

*Costs:* reference cycles that Python's collector cannot see through Rust
fields — every such class implements `__traverse__`/`__clear__` for exactly
this reason, and before that every note that went into a stream leaked.

**Some setters take whatever they are given.** `pitch._client`, `key.tonic`
and `scale.tonic` keep a facade object where the value is one and record
nothing where it is not, rather than raising. The note written into
`_client` is not always one of ours: a harness that swaps one module's
classes and not another's hands it music21's own `Note`. Tightening it broke
29 of music21's docstrings.

**A frozen score carries its musical half as text.** Pickling writes the Rust
value out as JSON beside the Python half, and the pickle names a function of
ours that puts the class back before reading. That is what lets a score cross
into a worker process, which is how music21's feature extraction runs.

## The harnesses

**Parity is measured against music21's own docstrings, run verbatim.**
`python-parity` collects the docstrings of a music21 module, swaps that
module's names for the facades, and runs the examples as they are. What
passes is what the crate reproduces to the letter.

**What passes is listed, not counted.** `python-parity/doctest/<module>.toml`
names every docstring that passes; the test fails when a listed one stops.
There is no "known divergence" list — a docstring that cannot pass is simply
absent, so the file only ever grows.

*Costs:* a docstring that fails for a deliberate reason looks the same as one
that fails by accident. `sieve` is 24/25 because the compressed reading of a
sieve is not modelled.

**music21's own suite is a comparison, not a pass mark.** It has failures of
its own in any environment, so only a test failing under `music21_rs` that
does not fail under music21 counts. One divergence is listed with its reason
and must keep failing, so the list cannot go stale.

**Three sides, not two.** music21, the crate linked into the test binary, and
the installed wheel. A difference between the last two is a packaging fault
and worth seeing rather than averaging away.

**Only one copy of these classes may exist in a harness process.** The
virtualenv on `sys.path` holds music21's dependencies and also the installed
wheel, and freezing a score imports `music21_rs` by name. Both harnesses
claim that name for the linked classes first; the wheel side of the suite is
the one place that must import the wheel.

**Nothing that drives Python is written in Python.** The benchmark, the suite
runner and the downstream comparison are Rust calling pyo3. The exceptions
are the wheel's own pytest suite, which must run under a plain interpreter to
test what `pip install` produced, and a `conftest.py` written into somebody
else's project.

## Measurement

**The collector's time is not the test's time.** music21's suite pauses to
collect garbage, and the pause lands on whichever test is running. Each test
is timed once, so a test that caught one read as many times slower than it
is — fourteen rows of the published comparison were collector pauses on tests
that are at parity. The pause is recorded against the test it landed on and
taken off before the sides are compared; the clock time stays in the JSON.

**A benchmark case is timed only where every side agrees on the answer.**
Timing two different computations says nothing. A case whose crate-side
answer disagrees is now named rather than silently blanked, so an empty
column means "nobody wrote this" and nothing else.

**The report distinguishes the crate from the wheel.** 29 members answer in
the wheel with no crate function, and the page counts them, so that "the
crate is a superset of the wheel" is not assumed from a single ported figure.

## Packaging and tooling

**The wheel is a mixed layout.** A checked-in `python/pysrc/music21_rs/__init__.py`
beside the extension, rather than the one maturin writes, because the
generated package imports every name — which would build all 27 exceptions
and import music21 at import time.

**The dependency is taken under another name.** This crate's library and the
crate it wraps are both `music21_rs`, and rustdoc refuses to build a doctest
against two externs of one name, so `cargo test` there could never run them.

**`xtask` always runs in release.** It parses ~4,000 Scala files and walks the
whole chord table; a debug build spends minutes on what release does in
seconds.

**Documentation that builds should reach the reader.** `pages` waits only on
lint and docs, not on every test job, so one stale fixture cannot keep the
whole site unpublished. A *release* still waits on everything.

## Decisions made and then unmade

**Deciding an installed object's half once per class.** The install helper
asked `issubclass` on every object it built; deciding it when the class is
installed measured 30.05µs against 29.45µs — nothing — so it was reverted.
The cost is the number of objects a chord builds, not the branching on each.

**Refusing a client that is not our note.** See above: it broke 29 docstrings.
The permissive setter is deliberate and now says so.
