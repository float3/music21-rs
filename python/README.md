# music21-rs for Python

[![PyPI](https://img.shields.io/pypi/v/music21-rs.svg)](https://pypi.org/project/music21-rs/)
[![music21 members in the wheel](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-wheel.json)](https://hilll.dev/music21-rs/reports/#ported)
[![music21 doctests passing](https://img.shields.io/endpoint?url=https%3A%2F%2Fhilll.dev%2Fmusic21-rs%2Freports%2Fbadge-doctests.json)](https://hilll.dev/music21-rs/reports/#doctests)

`music21_rs` is [music21](https://github.com/cuthbertLab/music21)'s analysis
classes, `Pitch`, `Interval`, `Chord`, `Note`, `Duration`, `Key`, `Scale`,
`RomanNumeral`, `TimeSignature`, `ToneRow` and others, implemented in Rust
and packaged for Python. the classes have music21's names, arguments,
properties and `repr`, and give music21's answers. the rust half is the
[music21-rs](https://crates.io/crates/music21-rs) crate; the wheel needs no
Rust toolchain and no music21 to run on its own. music21's stream
machinery, its `converter` and the corpus are not included; those remain
music21's. the package does read scores in seven formats into streams of its
own, and writes MusicXML: see [Reading and writing
scores](#reading-and-writing-scores).

the [reports page](https://hilll.dev/music21-rs/reports/) has the full
numbers: every music21 method and whether it is ported, the doctest and test
suite results, benchmarks and sizes, refreshed on every push.

```bash
pip install music21-rs
```

```python
import music21_rs as m

chord = m.Chord("C4 E4 G4 B-4")
chord.commonName            # 'dominant seventh chord'
chord.root()                # <music21.pitch.Pitch C4>
chord.forteClass            # '4-27B'
chord.transpose("M3")       # <music21.chord.Chord E4 G#4 B4 D5>

m.RomanNumeral("viio7", m.Key("c")).pitches
# (<music21.pitch.Pitch B4>, <music21.pitch.Pitch D5>, <music21.pitch.Pitch F5>, <music21.pitch.Pitch A-5>)

m.TimeSignature("6/8").getAccentWeight(1.5)   # 0.5
```

## reading and writing scores

```python
import music21_rs as m

score = m.from_musicxml(open("tune.musicxml", encoding="utf-8").read())
for part in score.parts:
    print(part.partName, len(part.getElementsByClass("Measure")))

text = m.to_musicxml(score, encoding_date="2026-01-01")

song = m.from_midi(open("song.mid", "rb").read())
line = m.from_tiny_notation("tinyNotation: 3/4 E4 r f# g trip{b-8 a g} c'2.")
```

every format music21-rs reads is read as music21's `converter.parse` reads
it, into this package's own streams:

| function | format | hands back |
| --- | --- | --- |
| `from_musicxml(text)` | MusicXML, partwise | `Score` |
| `from_abc(text)` | ABC | `Score`, or an `Opus` of them for several tunes |
| `from_abc_number(text, number)` | one tune of an ABC file, by its `X:` | `Score` |
| `from_midi(data)` | a standard MIDI file's `bytes` | `Score` |
| `from_tiny_notation(text)` | TinyNotation | `Part` |
| `from_humdrum(text)` | Humdrum `**kern` | `Score`, or an `Opus` for several tables |
| `from_mei(text)` | MEI | `Score` |
| `from_roman_text(text)` | RomanText | `Score`, or an `Opus` for several movements |
| `from_noteworthy(text)` | NoteWorthy Composer text, `.nwctxt` | `Score` |
| `from_musedata(texts)` | MuseData, one file's text or a list of a work's | `Score` |
| `from_capella(data)` | a Capella `.capx`'s `bytes`, or its `score.xml` | `Score` |
| `from_nwc(data)` | a NoteWorthy Composer `.nwc` file's `bytes`, compressed or not | `Score` |
| `from_volpiano(text)` | Volpiano, the chant font's notation | `Part` |

the streams hold measures of notes, chords, rests, clefs, keys, meters,
tempo marks, dynamics, chord symbols and instruments, and a voice apiece
where a measure has more than one. a RomanText score's chords are
`RomanNumeral`s, carrying their figures as lyrics. a score's metadata comes
with it, as a `Metadata`, so a collection read from one file can be asked
for a tune by its number or its title. what the package has no class for --
repeat marks, unpitched percussion, a fermata -- is read and left out. a
note's articulations and its ornaments come with it, and so do barlines,
repeats, words and the spanners: a measure carries its `leftBarline` and
`rightBarline`, its `number`, `numberSuffix` and padding, and answers
`barDuration`, `barDurationProportion`, `bestTimeSignature` and
`padAsAnacrusis`; a part carries its `partName` and `partAbbreviation`,
answering for the first instrument it holds where nothing says them; a stream answers `finalBarline`, `spanners` and
`spannerBundle` and plays its repeats out with `expandRepeats`; and a
slur names the score's own notes. input a reader cannot read raises `StreamException`. a
compressed `.mxl` is a zip holding the document: unpack it first.

the package's streams slice, scale, invert and regroup themselves as
music21's do -- `sliceByQuarterLengths`, `sliceAtOffsets`, `sliceByBeat`,
`sliceByGreatestDivisor`, `scaleOffsets`, `scaleDurations`,
`augmentOrDiminish`, `invertDiatonic`, `voicesToParts`, `explode`,
`partsToVoices`, `implode`, `transpose`, `stripTies`, `quantize` and
`template` -- each handing back a new stream, so `inPlace=True` and a
`target` are refused. they notate themselves too -- `makeMeasures`,
`makeRests`, `makeTies`, `makeBeams`, `makeAccidentals`, `makeVoices` and
`makeNotation`, each step at music21's own defaults, a keyword asking for
anything else refused rather than answered differently.
`splitAtQuarterLength` cuts a stream in two at a length from its start,
and `extendTies` and `shiftElements` change the stream and answer
nothing, as music21's do -- the ties written onto its own notes. and they answer
`highestOffset`,
`lowestOffset`, `hasMeasures`, `hasVoices`, `isGapless`, `isSequence` and
`isWellFormedNotation`, `hasPartLikeStreams`, `findGaps`, `getOverlaps`
and `simultaneousAttacks`, and they take elements out by class or by index
(`removeByClass`, `removeByNotOfClass`, `pop`), repeat an object into
themselves as copies (`repeatAppend`, `repeatInsert`), make room for one
(`insertAndShift`), merge a note into what already stands at an offset
(`insertIntoNoteOrChord`), divide themselves by what a function says of
each element (`splitByClass`) and answer `voices` and
what they open with -- `clef`, `keySignature` and `timeSignature`, read
and written as music21's are -- and the meters and instruments they hold,
`getTimeSignatures`, `getInstruments` and `getInstrument`, which make up
music21's default where a stream states none -- `measures` and `measure` -- the measures of a number range, the first at
nought and before it what was in force where it begins -- and
`measureOffsetMap`, which a score builds out of its parts, and
`offsetMap`, music21's named tuple per element, `pitches`, `extractContext`, `playingWhenAttacked` and `allPlayingWhileSounding` --
besides `chordify`, `findConsecutiveNotes` and
`melodicIntervals`. a part written for a transposing instrument is turned
to sounding pitch and back with `toSoundingPitch` and `toWrittenPitch`,
and says which it is with `atSoundingPitch` -- `True`, `False` or
`'unknown'`, as music21's does. they are asked for elements by class and by offset -- a class being read
as music21 reads it, by its own name or by anything it is a kind of, so
`getElementsByClass('GeneralNote')` finds the notes, the chords and the
rests -- which is what every object's `classes` says of itself, music21's
own chain down to `object` --
`getElementsByClass`, `getElementsNotOfClass`, `hasElementOfClass` and
`getElementsByOffset` with music21's four keywords, under which an element
is found only where it *begins*, `getElementAtOrBefore`,
`getElementBeforeOffset`, `getElementAfterElement` and
`beatAndMeasureFromOffset` -- and they answer
`isTwelveTone` and
`staffLines`. they stretch, take in and empty themselves as well:
`extendDuration` gives each element of a class the time up to the next one,
`mergeElements` takes in everything another stream holds, each at the offset
it stands at there and the object itself, and `cloneEmpty` is a stream of
the same class holding nothing. `seconds` says how long a stream lasts and
`metronomeMarkBoundaries` which tempo holds through each stretch of it,
handing back the stream's own mark wherever it holds one; where a stream
states no tempo a quarter at 120 is counted, as upstream, and `seconds` is
`nan`, which is music21's answer for a stream whose own search finds no
tempo. `lyrics` gathers every line sung, by the number it is sung under,
with a place for each note read so the lines stay level with the notes and
the note's own `Lyric` in it.

music21's `analysis.patel` and `analysis.metrical` are here -- `nPVI`,
`melodicIntervalVariability`, `labelBeatDepth` and
`thomassenMelodicAccent` -- and a stream writes the interval against
another part onto its notes with `attachIntervalsBetweenStreams`. each
leaves its answer on the note's `editorial`, music21's dictionary-with-attributes for what an analysis
found.

`analysis.segmentByRests.Segmenter` cuts a line into the runs of notes
between its rests, each run the stream's own notes and each interval built
on two of them; `analysis.pitchAnalysis.pitchAttributeCount` counts a named
property of every pitch into a `Counter`, and
`analysis.elements.attributeCount` counts a named property of every
element a stream or an iterator hands over, passing over an element that
has none. `analysis.floatingKey.KeyAnalyzer`
reads a key for every measure and smooths each by the measures around it,
through a window and a weighting the caller may say.
the figured-bass realizer is here as `FiguredBassLine` and `Realization`:
a line of bass notes and figures in a key and a meter, written out as a
part, and every voicing of it that keeps the rules -- counted, listed,
chosen from and written out as a score in keyboard or chorale style, with
any part laid over the line kept as it stands. where music21 chooses at
random this chooses through Python's own `random`, so a caller who seeds it
gets the same realization twice.

`analysis.windowed.WindowedAnalysis` runs a processor's own analysis over
windows of a stream, a quarter note long and up, each window handed over as
a stream of the package's; the answer and the colour are the processor's.

`analysis.discrete`'s classes are here too -- `Ambitus`, the five key
weightings and `MelodicIntervalDiversity` -- each handing back what the
stream holds: a span is its own pitch objects, and a counted interval is
built on the two notes it was measured between. `process` is left out for
`Ambitus` and for a key weighting, answering a colour out of music21's
colour tables, which the crate does not carry.

music21's four instrument stream functions are here too --
`partitionByInstrument`, `bundleInstruments`, `unbundleInstruments` and
`deduplicate` -- each handing back a new stream.

a score carries its `metadata`, music21's `Metadata`, whose names are read
and written as music21 reads them -- by a unique name, by one of the four
grandfathered work ids, or by one of the twenty Humdrum abbreviations, so
`md.otl` and `md.title` are the same title -- and which answers `search`,
`all` and `md['title']`. what a name answers is a string, where music21
answers its own `Text` and `Contributor`, so `all(returnPrimitives=True)` is
refused and nothing here is installed over music21's own `metadata`. an
`Opus` -- what a multi-tune ABC or Essen file reads as -- answers `scores`,
`getNumbers`, `getScoreByNumber`, `getScoreByTitle` and `mergeScores`.

`analyze` asks a stream one question, as music21's does: `ambitus`, `span`
and `range` give the interval from the lowest pitch sounding to the
highest, and `key` and the names of music21's five key weightings give the
key the stream is most likely in. the name need not be exact -- `kostka`
finds Temperley's profile, a bare `key` finds Aarden's -- and a stream with
no pitches has no ambitus, answering `None` as music21 does.

`to_musicxml` takes one of this package's streams or one of music21's and
writes what music21's exporter writes with `makeNotation=False`. with
`make_notation=True` it makes the notation first, as music21's exporter does
by default, so a part of loose notes is written in measures, with its ties,
rests, accidentals and beams.

`to_midi` takes the same and hands back the `bytes` of the MIDI file
music21's `streamToMidiFile` writes, its repeats played out first, and
`to_abc` writes it as ABC, which music21 has no writer for: a tune music21's
reader and this package's both read back as the same score. `to_volpiano`
writes it as Volpiano, as music21's `volpiano.fromStream` does, and
`to_roman_text` writes its roman numerals as a RomanText analysis, as
music21's `RnWriter` does. `to_lilypond` writes it as LilyPond, as music21's
`LilypondConverter` does, with no LilyPond installed: `version` is the
LilyPond version the file says it is for, 2.24 unless given.

## using it inside music21

`install_into_music21()` replaces the classes of an installed music21 with
these, so existing music21 code runs on the Rust implementation unchanged:

```python
import music21_rs
music21_rs.install_into_music21()   # before anything imports the classes

from music21 import chord, roman, key
chord.Chord("C4 E4 G4").commonName          # 'major triad', out of Rust
roman.RomanNumeral("V7", key.Key("G")).pitches
```

it patches the live modules for the whole process, so call it before any
`from music21.chord import Chord`; for a test suite, `conftest.py` is early
enough. the installed classes are `Music21Object` subclasses, so music21 can
hold them in streams, find them by class, pickle them and export them to
MusicXML.

## coverage of music21

- 94% of the public methods of the ported music21 classes are reachable from
  this wheel.
- all 40 music21 modules whose doctests run against the port pass every
  example, 9,185 of them: `pitch`, `interval`, `chord`, `chord.tables`,
  `note`, `duration`, `key`, `scale`, `scale.scala`, `roman`, `harmony`,
  `serial`, `sieve`, `meter.base`, `meter.core`, `beam`, `tie`, `volume`,
  `dynamics`, `instrument`, `clef`, `articulations`, `expressions`, `tempo`,
  `voiceLeading`, `analysis.discrete`, `analysis.enharmonics`,
  `analysis.harmonicFunction`, `analysis.neoRiemannian`,
  `analysis.transposition`, and ten of `figuredBass`: `notation`,
  `possibility`, `realizerScale`, `rules`, `resolution`, `segment`,
  `harmony`, `checker`, and the `realizer` and `examples` that realize whole
  lines over them.
- music21's own test suite gives the same results with this wheel installed
  over music21 as with music21 alone.
- [harte-library](https://github.com/andreamust/harte-library), a third-party
  chord parser built on music21, gives identical results for its 8,116 tests
  on both.

`style.Style` is provided but not installed over music21's, whose
version does more. missing behaviour raises
rather than falling back to music21.

## speed and size

times per call, from the
[benchmark](https://hilll.dev/music21-rs/reports/#speedups) against music21
11 on Python 3.13.

| | music21 | music21_rs | speedup |
| --- | ---: | ---: | ---: |
| `Pitch('C#4')` | 1.55 us | 0.37 us | 4x |
| `Note('C#4')` | 4.4 us | 1.3 us | 3.4x |
| `Interval('P5')` | 7.5 us | 0.47 us | 16x |
| `Chord('C4 E4 G4')` | 16.2 us | 8.9 us | 1.8x |
| `Chord.commonName` | 526 us | 60 us | 8.8x |
| `Chord.forteClass` | 244 us | 53 us | 4.6x |
| `Pitch.transpose('M3')` | 28 us | 1.0 us | 28x |
| `Pitch.getEnharmonic()` | 22.7 us | 0.63 us | 36x |
| `ToneRow.zeroCenteredTransformation` | 274 us | 0.37 us | 741x |
| `pcToToneRow(...).matrix()` | 2.71 ms | 4.9 us | 547x |

repeated queries on the same chord are cached in both and cost the same. over
music21's own test suite the median test runs at the same speed, since most
of a music21 test is music21's own code.

the wheel is 1.7 MB, one compiled module with the chord tables and scale
definitions inside it and no dependencies. music21 installs 105 MB.

## building

```bash
uvx maturin build --release --manifest-path python/Cargo.toml
uvx maturin develop --manifest-path python/Cargo.toml   # into the active venv
pytest python/tests -q
```

the wheel shares the crate's version number. two further checks run it
against music21 itself, and need music21, `lark` and `pytest` installed
beside it:

```bash
# music21's own test suite, on music21, on the crate and on the wheel
cargo run --release -p xtask --features python -- music21-suite
# harte-library's test suite, on music21 and on the wheel
cargo run --release -p xtask -- downstream
```

## credits and third-party data

`music21_rs` is released under the
[AGPL-3.0](https://github.com/float3/music21-rs/blob/master/LICENSE). it
ports behaviour from, and compiles in data from, the projects below, each
under its own licence.

### music21

the chord tables, scale definitions, time-signature behaviour and
chord-symbol kinds are derived from
[music21](https://github.com/cuthbertLab/music21), the Python library for
computational musicology by Michael Scott Asato Cuthbert and contributors,
licensed [BSD-3-Clause](https://github.com/cuthbertLab/music21/blob/master/LICENSE).
The wheel is built against music21 11.0.0b9, and its classes are checked
against that version's own doctests and test suite. thanks to Michael Scott
Asato Cuthbert and all music21 contributors for the original library.

### harte-library

the `harte` crate beside this one, which `xtask downstream` runs
harte-library's own tests against, is a port of
[harte-library](https://github.com/andreamust/harte-library) by Andrea
Poltronieri, licensed
[MIT](https://github.com/andreamust/harte-library/blob/main/LICENSE).

### the Scala scale archive

thirteen of the tuning tables compiled in are transcribed from the
[Scala](https://www.huygens-fokker.org/scala/) scale archive as distributed
with music21, which includes it by kind permission of Manuel Op de Coul. the
archive itself, 3,994 scales in the crate's `scala-archive` feature, and the
62 [Plainsound Hexatone](https://github.com/PLAINSOUND/hexatone) scales
beside it are not in the wheel.

### the Xenharmonic Wiki

the mapping, generators, commas and scales of 95 regular temperaments from
the [Xenharmonic Wiki](https://en.xen.wiki), which is CC BY-SA, are compiled
in. only the numbers are used.

### contributed back

fixes found while porting went upstream:

- [cuthbertLab/music21#1746](https://github.com/cuthbertLab/music21/pull/1746):
  the `Pitch` constructor's type annotation admits a `Pitch`.
- [cuthbertLab/music21#2003](https://github.com/cuthbertLab/music21/pull/2003):
  a malformed ratio in the Scala archive's `sparschuh-stanhope.scl`.
- [cuthbertLab/music21#2004](https://github.com/cuthbertLab/music21/pull/2004):
  a `type: ignore` left over from a closed mypy issue.
- [cuthbertLab/music21#2026](https://github.com/cuthbertLab/music21/pull/2026):
  Scala files with text after a pitch value, which 23 archive files have.
- [cuthbertLab/music21#2027](https://github.com/cuthbertLab/music21/pull/2027):
  `removeRedundantPitches` confusing a flat with a negative octave.
- [cuthbertLab/music21#2028](https://github.com/cuthbertLab/music21/pull/2028):
  `getPitchFromNodeDegree` handing back a pitch owned by the scale's cache.
- [cuthbertLab/music21#2038](https://github.com/cuthbertLab/music21/pull/2038):
  `OctaveRepeatingScale` appending to the list of steps it was given.
- [cuthbertLab/music21#2043](https://github.com/cuthbertLab/music21/pull/2043):
  a descending realization checking its range against the unaltered pitch.
- [cuthbertLab/music21#2044](https://github.com/cuthbertLab/music21/pull/2044):
  `derive` leaving out a scale's altered degrees, so a harmonic minor derived
  as a natural minor.
- [cuthbertLab/music21#2045](https://github.com/cuthbertLab/music21/pull/2045):
  an octave-repeating scale wider than an octave starting an octave off its
  tonic.
- [cuthbertLab/music21#2046](https://github.com/cuthbertLab/music21/pull/2046):
  timespan offsets added as floats, which gave a voice-leading quartet that is
  not in the music.
- [cuthbertLab/music21#2048](https://github.com/cuthbertLab/music21/pull/2048):
  two deepcopies in `AbstractScale` of values that are already copies.
- [cuthbertLab/music21#2054](https://github.com/cuthbertLab/music21/pull/2054):
  a realization cached without its altered degrees, so a harmonic minor lost
  its raised seventh after `nextPitch`.
- [cuthbertLab/music21#2056](https://github.com/cuthbertLab/music21/pull/2056):
  `getAllNamesForInstrument` finding names only for an instrument whose name
  is spelled like its class.
- [cuthbertLab/music21#2057](https://github.com/cuthbertLab/music21/pull/2057):
  `nextPitch` on a cyclical scale answering below the pitch it was given:
  F#3 after C4 on a scale of major thirds from D3.
- [PLAINSOUND/hexatone#3](https://github.com/PLAINSOUND/hexatone/pull/3):
  Scala headers in five Hexatone scale files (open).
- corrections to the Xenharmonic Wiki's temperament pages, found while
  checking its infoboxes: [special:contributions/hill](https://en.xen.wiki/w/Special:Contributions/hill).
