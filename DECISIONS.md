# opinionated decisions

choices in this repository that could reasonably have gone the other way, why
they went the way they did, and what each one costs. a decision is here if
reversing it would be a design change rather than a bug fix.

read this beside `CLAUDE.md`, which says how the pieces work; this says why
they are shaped that way. `issues.md` says what is still wrong.

## the crate

**the crate models music, and is meant to model scores too.** *changed
2026-09-12.* it used to answer only questions about a pitch, an interval, a
chord, a meter or a tuning, leaving `Stream`, parsing and notation to
music21 — which is why members are carried by the wheel and not the crate.
The direction now is the opposite: the crate is meant to be a strict superset
of the wheel, caches aside, and as little as possible belongs in the facade.

the crate already has a `Stream` (`src/stream.rs`): elements at offsets,
`flatten`, `recurse`, parts, measures, voices, `transpose`, and lookups for
the key, meter and tempo in force. what it does not have is what music21 uses
to make an object a *member* of a stream — sites and contexts, derivations,
`activeSite`, measure-relative offsets.

*costs:* until it does, a facade object that music21 may put in a stream has
to carry music21's own half: an installed chord's notes each run
`Music21Object.__init__` once they are asked for. see `issues.md`.

**nothing answers at random.** *changed 2026-10-07.* where music21 picks
at random -- a note a scale stands on twice, the way Rag Marwa comes down
from its `D-`, a fresh part or instrument id -- the crate and the wheel
follow a fixed rule instead: the lowest degree, the first place in order of
degree, ids counted up through the process. a choice music21 makes by
chance and a caller may want to make is an argument or a seed
(`next_pitch_below_from`, `WeightedHexatonicBlues::sample`), never a
random number generator.

*costs:* upstream music21 still picks at random, so its `testRagMarwaB`
and `testRagMarwaC` count both answers. the fork the submodule pins
answers by the same rule (`b68a661af`), so `music21-suite` runs with no
expected divergences; following upstream instead would bring both back.

*debated 2026-10-30, kept, with one exception.* a lookup that answers
differently each call cannot be compared, and where a caller wants chance
the crate takes a chooser or a seed. but ids counted from one in every
process named two workers' first parts alike, so an id's first half is now
drawn once per process and only its second half counts. the fork commit
behind Rag Marwa's rule (`b68a661af`) has not been offered upstream.

**where music21's author names a mistake, the crate does not repeat it.**
*Changed 2026-09-25.* Cuthbert's "Music21's Mistakes"
(`music21/documentation/source/developerReference/startingOver.ipynb`) lists
what he would do differently. the crate does it differently: a flat is
written `b` (`Bb4`), and `-` is still read; every pitch has an accidental, a
natural where none is written, with music21's written-or-not kept as
`written_accidental`; pitches, accidentals and durations are `Eq` and
`Hash`; and where an element sits is asked for with `Stream::placed`, not
stored on it as a site.

*costs:* the wheel translates. every name it hands Python is respelled with
`-` (`python/src/spelling.rs`), and a name built anywhere else in the wheel
that skips that module leaks `b` into music21's output. the parity suites
catch it: they compare in music21's spelling.

*debated 2026-10-30, kept.* each change is an item of Cuthbert's own list,
and `-` would make B flat 1 (`B-1`) and B in octave -1 the same name; going
back would break some 500 call sites a second time. the cost the other side
found was real but local: the parity tests held seven copies of the
respelling, now one (`utils::music21_name`), and `flat_spelling_parity`
asks some thousand names of the wheel and music21 alike and fails on a `b`
music21 does not write itself.

**nothing is cached.** a value that answers the same question twice does the
arithmetic twice. music21 memoizes on the object; this does not.

*costs:* `Chord.forteClass` asked twice costs twice, where music21's second
answer is free. the benchmark shows it: warm-object cases are the only two
where music21 is within reach. the wheel does cache, because a facade object
is asked the same question repeatedly by music21's own code.

*debated 2026-10-30, kept.* recomputing in Rust (0.4-1µs for a forte class)
beats music21's *cached* answer (2µs); what loses to a warm music21 object is
the Python call, which the wheel's cache already removes. a cache on `Chord`
would go stale through `notes_mut`, and music21 itself fixed five
cache-correctness bugs in six weeks of 2026. what was wasteful was an
algorithm asking the same chord for its root four times, so the analyses
find it once and hand it on.

**errors are values.** every fallible operation returns `Result`; nothing
panics on bad input. the `panic!`s and `expect`s that exist are on constant
literals — `Interval::from_name("P5")` — where failure would be a bug in this
file rather than in a caller's data.

*costs:* signatures are noisier, and a constructor that could be infallible
often is not. it also means a value nobody can answer is refused rather than
answered wrongly — which is why `ChromaticInterval::new` became fallible and
why non-finite pitch space, frequencies and tempi are errors.

**`#![forbid(unsafe_code)]`.** the crate contains no unsafe Rust, and cannot
grow any without the attribute coming off. the wheel denies rather than
forbids it, with exactly one allow, because a pyo3 exception type standing on
music21's own base needs a hand-written `PyTypeInfo`.

**the caller brings the regular expression engine.** music21's
`Metadata.search` reads a pattern out of any query holding `*.|+?{}` and
compiles it with Python's `re`. the crate carries no engine and takes no
dependency for one: `MetadataQuery::Text` is music21's plain-string reading,
containment with either case ignored, and `MetadataQuery::Matcher` takes a
predicate, so a caller who wants the pattern hands over whichever engine
they already have. the parity test builds that predicate with `regex`, a
dependency of the test crate alone, for exactly the queries music21 would
read as a pattern.

*costs:* a Rust caller searching by pattern writes two lines rather than
one, and the facade keeps music21's own `re` for its own search.

**generated tables are committed.** the chord tables, the tuning tables and
the Scala archive are emitted into Rust by `xtask` and checked in, so a
normal build needs no Python and no submodule. each generated file carries
the command that regenerates it, and CI verifies the file still matches the
TOML it came from.

*costs:* ~7k lines of generated Rust in the tree, and a regeneration step
whenever a submodule moves.

**`TimeSignature` carries the tree it is divided into.** *changed
2026-09-12.* it used to be a `Copy` pair of integers deriving its beats,
accents and divisions from the ratio by closed-form rules. it now owns
music21's four `MeterSequence`s — display, beats, beams and accents — built
by music21's own rules, and `beat_count` reads how many parts the beat
sequence has rather than working the count out again.

the earlier plan, a crate type with the mutable sequences held by the facade,
was rejected: it put Rust-able logic in Python.

*costs:* the type gives up `Copy`, `Eq` and `Hash`, since its parts carry
float weights. that reached 36 by-value methods, the `StreamElement` variant
and `Stream::time_signature_at`. building a meter now builds four trees
rather than none.

*what it bought, beyond the members it unblocks:* the closed form was wrong
where music21 is not. it cut compound threes at a denominator over four, so
`3/6` counted as one beat where music21 counts three; music21 cuts at an
eighth. the tree and the old rule agree across all 126 meters of the
fixture, which is what says the tree is built right, and they disagree only
where the old rule was wrong.

*how it is checked:* eighteen meters are pinned against the strings music21
prints for `beatSequence` and `beamSequence`, because three of the rules are
not guessable from the outside — a sequence nobody has divided still has one
part, the word in `slow 6/8` makes it a different meter counted in six, and
a meter written additively is beamed by its numerator (`3/8+2/8` beams
`{2/8+3/8}`) but counted in the parts it was written in, each keeping its own
note (`2/4+3/8` counts `{{1/4+1/4}+{1/8+1/8+1/8}}`).

## the Python facade

**the package keeps no sites, as the crate keeps none.** a stream of the
package's holds objects, so it *could* write itself onto each of them and let
an element answer `offset`, `activeSite` and `getContextByClass` as music21's
does. it does not: where a thing stands is the stream's own to say, which is
the crate's rule carried all the way out, and the members that need a site are
refused or left out with the reason said -- `beatDuration`, and the context an
element would walk outwards through. what a stream can answer *about* where
something stands it does answer, by asking the crate and handing back its own
objects: `beatAndMeasureFromOffset`, `getElementAtOrBefore`, `measures`
collecting the clef and meter in force.

*costs:* code written against music21 that reads `n.offset` or walks contexts
has to ask the stream instead. the gain is that nothing holds a reference back
to what holds it, so there are no cycles for the garbage collector to break
and no object that is in two streams at two offsets.

*debated 2026-10-30, kept.* the harm was narrower than the rule: an installed
note in one of these streams read `offset` as 0.0 and `beat` as nan without
complaint. so once the wheel is installed into music21 its readers hand back
music21's own streams, whose sites answer, and standalone they hand back the
package's own; music21 merely being importable decides nothing. building
music21's containers costs about 3.8 times the read on a large score, still
under music21's own parser. weak sites in the package's streams were tried
on paper and stall on music21's tree walk, which a context search needs.

**installed classes are Python subclasses of the facade and of the class they
replace.** `install_into_music21()` builds, for each name, a class deriving
from the pyo3 class and from music21's own, so music21 will keep it in a
`Stream` — it has the offset, the sites and the context machinery — while
every musical question is answered in Rust.

*costs:* a Python `__init__` on the hot path. an installed `Chord('C4 E4 G4')`
builds only itself (3.3µs against music21's 21.2µs), and its pitches on
their own (8.9µs); asking for its notes costs about what music21's chord
does (25.8µs), each note starting music21's half.

**the facade shows two classes where the crate has one.** music21 calls a
span of a bar that nothing divides a `MeterTerminal` and one that is divided a
`MeterSequence`, and its own docstrings compare both reprs to the letter — 57
of the first and 83 of the second. the crate models both as one type, since
the difference is whether a list is empty. so the wheel picks the class by
whether the span has parts.

*costs:* a caller sees a class boundary the crate does not have, and
`isinstance` against music21's own two classes is false for ours, as it is
for every other facade class.

**a sequence read off a meter is a view of it, not a copy.** music21 divides a
bar by reaching into the sequence it carries — `ts.beatSequence.partition(2)`
— assigns back through it, and its *own* code asks our object
`self.beatSequence.isUniformPartition()`. a wrapper holding a copy would take
all of that silently. so one holds the meter it came from, which of the four
sequences it is, and the path down to the span; reads and writes go to the
meter itself. what `subdivide` hands back is detached instead, because
music21's `subdivide` does not happen in place.

*costs:* a view outlives nothing — a span whose parent has been repartitioned
under it raises rather than answering from a stale copy.

**anything not ported is blocked, never inherited.** a member the facade
lacks raises `AttributeError` rather than falling through to music21's
implementation. a partial port that silently answered from music21 would make
the parity numbers meaningless.

*debated 2026-10-30, kept.* falling through was tried: music21's `Chord.add`
and `remove` and its pitch `octave` and `name` setters run on a facade object
and change nothing, raising nothing, because they write state the facade does
not keep. the message now ends "not ported to music21-rs", so a blocked member
is not taken for a typo; `show` and `write` on keys and scales, most of what
was blocked, are ported.

**a measure's barlines are listed, not held.** music21 holds a measure's left
barline at nought and its right one at the end among its elements. the
package keeps them as `leftBarline` and `rightBarline` and lists them where a
stream is asked what it holds by class, flattened or walked, the right one
last. *debated 2026-10-30.* holding them would shift the places spanners name
their notes by, silently, across forty sites that touch what a stream holds,
for a pattern music21's own code barely uses; listing them closes what callers
see. *costs:* `len(measure)`, indexing and plain iteration leave them out
where music21's count them.

**a flag is read for its truth; a string is refused.** music21 asks
`if inPlace:`, so `inPlace=1` and a forwarded `None` work there, and every
flag the wheel takes reads its argument's truth. a string is a `TypeError`
where music21 takes it: `'False'` is true. *debated 2026-10-30;* no caller in
music21 or its corpus passes either, so this is drop-in compatibility, not
repair.

**a negative length is refused.** music21 takes `Duration(-1)` and calls it
`zero`; a note of that length makes the next one overlap it, and MusicXML
export of it fails. the crate's arithmetic assumes lengths of nought or more,
so a negative one is refused rather than answered wrongly. *debated
2026-10-30, kept;* nothing in music21 makes one on purpose.

**exceptions stand on music21's own base, and are built on first asking.**
music21 catches `Music21Exception` throughout, so an exception of ours that
was not one would escape its own reader. building one imports music21, so
they are built when first named rather than at import: `import music21_rs`
costs 4ms rather than 519ms, and the wheel needs no music21 to run.

*costs:* a hand-written `unsafe impl PyTypeInfo`, and a wheel used standalone
reports `music21_rs_facade` as an exception's module until something installs
or names it. installing stamps all 27 first, because music21's doctests read
those module strings.

**the wheel depends on nothing.** no `Requires-Dist`. music21 is an optional
integration, present or not.

**facade objects carry the Python objects music21 hands them.** a note keeps
its `Duration`, `Volume`, `Beams`, `Tie`, expressions and articulations as
the Python objects they were given, so `chord.duration is d` and notation
written through one of them sticks.

*costs:* reference cycles that Python's collector cannot see through Rust
fields — every such class implements `__traverse__`/`__clear__` for exactly
this reason, and before that every note that went into a stream leaked.

*debated 2026-10-30, kept.* sixty of music21's passing docstrings change a
note's duration, volume or beams through the object it hands back, and
seventy-seven check `is`; values would lose those writes silently. what the
debate found instead were cycles nobody had shown the collector: a note,
chord or rest whose editorial points back at it, a slur its note keeps, a
Clercq-Temperley song and its rules. those traverse now, and a test scans
every class holding a Python object: each traverses it or gives the reason
no cycle runs through it.

**where music21's class is the better one, music21's is the one that
answers.** `style.Style` is the long-standing example -- a note's `style` is
music21's own object inside music21 and the package's only without it,
because music21's exporters read a page off it and its subclasses carry more
than one class of ours would. `editorial.Editorial` is the same bargain, and
it was learnt the hard way: the package's stand-in is a dictionary with
attribute access, music21's is a `dict` subclass with the whole dict API that
music21's own copying carries along, and standing in for it cost 34 of
music21's own tests -- a copied note lost what an analysis had written on it,
and music21's figured-bass realizer could not read back the `notationString`
it had just written. so a note's and a chord's `editorial` is music21's
wherever music21 can be imported, and the package's `Editorial` answers only
for a caller who has no music21 at all.

*costs:* two behaviours for one member, told apart by whether music21 is
importable, and a class of the package's that music21's own suite never
exercises. the gain is that nothing quietly worse than music21 stands in
music21's place.

**some setters take whatever they are given.** `pitch._client`, `key.tonic`
and `scale.tonic` keep a facade object where the value is one and record
nothing where it is not, rather than raising. the note written into
`_client` is not always one of ours: a harness that swaps one module's
classes and not another's hands it music21's own `Note`. tightening it broke
29 of music21's docstrings.

**a frozen score carries its musical half as text.** pickling writes the Rust
value out as JSON beside the Python half, and the pickle names a function of
ours that puts the class back before reading. that is what lets a score cross
into a worker process, which is how music21's feature extraction runs.

## the harnesses

**parity is measured against music21's own docstrings, run verbatim.**
`python-parity` collects the docstrings of a music21 module, swaps that
module's names for the facades, and runs the examples as they are. what
passes is what the crate reproduces to the letter.

**what passes is listed, not counted.** `python-parity/doctest/<module>.toml`
names every docstring that passes; the test fails when a listed one stops.
There is no "known divergence" list — a docstring that cannot pass is simply
absent, so the file only ever grows.

*costs:* a docstring that fails for a deliberate reason looks the same as one
that fails by accident. `sieve` is 24/25 because the compressed reading of a
sieve is not modelled.

*debated 2026-10-30, changed.* a newly passing docstring only printed a line
that nextest hides, so the list fell behind what passes. the gate now fails
both ways: a listed docstring that fails, and a passing one that is not
listed, which `UPDATE_DOCTEST_EXPECTATIONS=1` records. an optional
`[not_passing]` table names deliberate failures with a reason
(`collapseArpeggios` refused, `Stream.plot` without matplotlib), and the
rewrite keeps it; an unexplained failure still needs no entry. `sieve` now
passes 25/25. `xtask report` reads summaries only for suites that exist, and
shows the reasons.

**music21's own suite is a comparison, not a pass mark.** it has failures of
its own in any environment, so only a test failing under `music21_rs` that
does not fail under music21 counts. one divergence is listed with its reason
and must keep failing, so the list cannot go stale.

**three sides, not two.** music21, the crate linked into the test binary, and
the installed wheel. a difference between the last two is a packaging fault
and worth seeing rather than averaging away.

**only one copy of these classes may exist in a harness process.** the
virtualenv on `sys.path` holds music21's dependencies and also the installed
wheel, and freezing a score imports `music21_rs` by name. both harnesses
claim that name for the linked classes first; the wheel side of the suite is
the one place that must import the wheel.

**nothing that drives Python is written in Python.** the benchmark, the suite
runner and the downstream comparison are Rust calling pyo3. the exceptions
are the wheel's own pytest suite, which must run under a plain interpreter to
test what `pip install` produced, and a `conftest.py` written into somebody
else's project.

## measurement

**the collector's time is not the test's time.** music21's suite pauses to
collect garbage, and the pause lands on whichever test is running. each test
is timed once, so a test that caught one read as many times slower than it
is — fourteen rows of the published comparison were collector pauses on tests
that are at parity. the pause is recorded against the test it landed on and
taken off before the sides are compared; the clock time stays in the JSON.

**a benchmark case is timed only where every side agrees on the answer.**
Timing two different computations says nothing. a case whose crate-side
answer disagrees is now named rather than silently blanked, so an empty
column means "nobody wrote this" and nothing else.

**the report distinguishes the crate from the wheel.** 29 members answer in
the wheel with no crate function, and the page counts them, so that "the
crate is a superset of the wheel" is not assumed from a single ported figure.

## packaging and tooling

**the wheel is a mixed layout.** a checked-in `python/pysrc/music21_rs/__init__.py`
beside the extension, rather than the one maturin writes, because the
generated package imports every name — which would build all 27 exceptions
and import music21 at import time.

**the dependency is taken under another name.** this crate's library and the
crate it wraps are both `music21_rs`, and rustdoc refuses to build a doctest
against two externs of one name, so `cargo test` there could never run them.

**`xtask` always runs in release.** it parses ~4,000 Scala files and walks the
whole chord table; a debug build spends minutes on what release does in
seconds.

**documentation that builds should reach the reader.** `pages` waits only on
lint and docs, not on every test job, so one stale fixture cannot keep the
whole site unpublished. a *release* still waits on everything.

## decisions made and then unmade

**deciding an installed object's half once per class.** the install helper
asked `issubclass` on every object it built; deciding it when the class is
installed measured 30.05µs against 29.45µs — nothing — so it was reverted.
The cost is the number of objects a chord builds, not the branching on each.

**refusing a client that is not our note.** see above: it broke 29 docstrings.
The permissive setter is deliberate and now says so.
