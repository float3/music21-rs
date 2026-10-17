# Unreleased

## added

- **music21's own metadata doctests run against the crate**, 285 of its
  370 examples, where none of them ran before. Getting there took five
  things, each a real gap in the wheel's `Metadata`:
  **`__setattr__` refused every name outside the metadata vocabulary**,
  which is the only call music21 makes for an attribute -- its own
  comment says so -- so `Music21Object.__init__` could not even write
  `_id` and nothing of music21's half was built. Anything the vocabulary
  does not name now goes into the object's own dictionary, as
  `super().__setattr__` would put it, and the class carries a `dict` to
  hold it, as the package's `Stream` does.
  **A frozen score could not be thawed** while the class stood in for
  music21's: `Music21Object.__setstate__` writes the whole dictionary with
  `object.__setattr__`, which CPython refuses for an object whose class
  defines `__setattr__` in C. The class pickles itself now, as every other
  facade here does.
  And five members music21's own code and docstrings ask for are carried:
  `add`, `getCustom`, `addCustom`, `setCustom`,
  `uniqueNameToNamespaceName`, `namespaceNameToUniqueName`,
  `isContributorUniqueName`, `isStandardName`, `bestTitle`,
  `getContributorsByRole`, `contributors`, and music21's four plural
  names (`composers`, `librettists`, `lyricists`, `software`), each
  answering every value under the name it stands for.
  `addContributor` takes music21's own `Contributor` -- anything answering
  to a role and a name -- as well as the pair this package took before.
  The crate gained the three vocabulary lookups behind them:
  `metadata::unique_name_from_namespace`, `is_standard_name` and
  `unique_name_of_plural`.
  What the remaining 85 examples want is music21's metadata *primitives*:
  the `Text`, `Date` and `Contributor` objects it answers with where this
  package answers the strings they say (`convertValue`, a value's
  `language`), and `_contents`, the dictionary music21's own private
  methods read its values out of.

- **A pitch read from what it is called in German, French, Italian or
  Spanish** (`pitch::from_language_name`, `pitch::language_dictionary`):
  music21's `languageExcerpts.naturalLanguageObjects`, the other direction
  of `Pitch::german` and its three sisters, which the crate had only the
  writing half of. The dictionary is built as music21 builds its own, by
  writing every one of the seven letters with up to four flats and four
  sharps out in the language and turning the pairs round, so there is one
  table and not two; a name the language calls no pitch by is a C natural,
  as music21 answers. The wheel has `toPitch`, `toNote`, `toChord` and
  `generateLanguageDictionary` beside it, and music21's own doctests for
  the module run against them (3 of 3 docstrings, 7 of 7 examples).
  `language_names_parity` compares every pair of all four dictionaries
  with music21's -- 63 names each -- and reads every one of them back,
  with eight names no language uses.

- **A walk of the wheel's streams answers what music21's answers**
  (`music21.stream.iterator.StreamIterator`): `recurse` handed back a
  plain list, so music21's own code could not filter it again --
  `analysis.discrete` asks a window for `recurse().notes`, and the wheel's
  own `bestClef` and `romanNumeralFromChord` ask the same of whatever
  stream they are given. The class holds what the walk found, each with
  where it stood, in the walk's own order, and answers `len`, truth,
  indexing, iteration, `first`, `last`, `notes`, `notesAndRests`,
  `getElementsByClass`, `getElementsNotOfClass` and `stream`. A stream
  cannot stand in for it, as the order of a walk is not a sort: `recurse`
  meets one part's notes before the next part's, where a stream would put
  every offset nought together -- which is what made the first attempt at
  this, handing back a stream, read a chorale in an order music21 never
  does. With it, `analysis.windowed` passes 34 of its 35 doctests rather
  than 24, the one left being a measure's `repr`, which music21 writes
  with its number and its offset and this package writes with its id.

- **music21's own doctests for seven of its analysis modules now run
  against the crate**, which takes the harness from 36 modules to 43:
  `analysis.elements` (4 of 4 examples), `analysis.patel` (14 of 14),
  `analysis.metrical` (13 of 13), `analysis.pitchAnalysis` (7 of 7),
  `analysis.segmentByRests` (6 of 6), `analysis.floatingKey` (14 of 14)
  and `analysis.windowed` (24 of 35). Each suite swaps the classes
  music21's own examples build -- a pitch, a note, a duration, a chord, a
  tie, a volume, the beams, and for the two that read a whole chorale its
  clefs, keys, meters, instruments, dynamics, expressions, articulations
  and tempo marks -- so music21's readers build the crate's objects and
  its own analysis docstrings run over them.
  Four things it found, each fixed. `labelBeatDepth` answered a new stream
  where music21 writes the lyrics onto the part it is given, so a caller
  reading its own notes afterwards saw none; it writes them in place now,
  as its sibling `thomassenMelodicAccent` always did.
  `melodicIntervalVariability` raised `StreamException` for a line too
  short to have a deviation where music21 has raised `ValueError` since
  v9. `getElementsByClass` found nothing when asked with a class rather
  than a name -- music21's own code asks `getElementsByClass(stream.
  Measure)`, and this package's classes do not inherit music21's, so a
  class is read as its name now. And the wheel's `Stream` has `first` and
  `last`, which music21's own code asks of whatever a filter hands back.
  What `analysis.windowed` still fails is one shape of thing: the wheel's
  `recurse` answers a list, so music21's own `analysis.discrete`
  processors cannot walk a window with `.recurse().notes`, and a measure's
  `repr` does not carry its number and offset.
- The doctest totals are regenerated, so the report counts the
  documentation of the 44 modules mapped since they were last written --
  `stream.makeNotation`, `search.serial`, the braille modules, the tree
  readers, `romanText.tsvConverter`, `romanText.clercqTemperley` and the
  rest. A module the harness does not cover shows as nought of what
  music21 documents rather than being left off the page, which is what
  that file is for.

- The feature map tracks `stream/makeNotation.py` and the two functions
  beside music21's ornament classes in `expressions.py`, neither of which
  it listed anywhere -- so the notation steps the crate has had since
  0.8.0, and which `musicxml_make_notation_parity` holds to music21 byte
  for byte on 257 subjects, were tracked only as `Stream`'s methods and
  music21's module-level half of them was tracked not at all. Nine of
  makeNotation's seventeen functions are ported, six are left out with
  their reasons -- the halves of a step the crate's own step takes itself,
  and a context manager there is no `with` here to hang on -- and
  `splitElementsToCompleteTuplets` and `consolidateCompletedTuplets` are
  public now, where they were private ports of two functions music21
  publishes. `makeAccidentals` and `makeAccidentalsInMeasureStream` are
  one function here, which the entry says. What the entry turns up is two
  real gaps, both about the accidentals an ornament implies:
  `makeOrnamentalAccidentals`, which `make_accidentals` says outright it
  does not consider, and the stream-level `realizeOrnaments` -- a
  different function from the `expressions.realizeOrnaments` the crate
  does have, as that one's doc comment now says. (The second of those is
  ported below; the first is still the to-do list.)
- **The accidentals an ornament's own notes need, decided with the rest**
  (`makenotation::make_ornamental_accidentals`): music21's
  `makeOrnamentalAccidentals`, the last gap the notation map turned up.
  `make_accidentals` now decides them where music21 decides them -- after
  the note itself and after each note of a chord, against everything heard
  so far including that note -- and counts them among the pitches already
  heard, so a note the ornament before it has already sounded shows no
  accidental of its own and the measure after one is decided against what
  its ornaments played, as music21's `makeAccidentalsInMeasureStream`
  counts them. What is decided is written onto the ornament, which is
  where the MusicXML writer reads the mark from; an ornament of a fixed
  size carries no accidental of its own and is left as it stands, music21
  keeping that on a resolved pitch a value type does not hold.
  `musicxml_make_notation_parity` gained the eight subjects that reach it:
  a note written `name@offset+length!ornament!` carries an ornament, which
  is the only way a subject can, since every accidental of a score read
  from a document has been decided by its reader and the step is then
  skipped. Two of them write an `<accidental-mark>` music21 writes and the
  crate did not, and the test was run with the step switched off to prove
  they catch it.
- **Every ornament a stream carries played out, in a stream of its own**
  (`makenotation::realize_ornaments`): music21's
  `stream.makeNotation.realizeOrnaments`, which the crate had no
  counterpart for -- `expressions::realize_ornaments` is music21's other
  function of that name, over the ornaments of one note. The answer is the
  stream's `clone_empty` with each element appended after the one before
  and every nested stream realized the same way, so a gap between two
  elements closes, as music21's does. A note carrying ornaments is
  replaced by the notes each of them plays, read in the key signature in
  force where the note stands -- and music21 hands every ornament the note
  *as it stands* rather than what the ornament before it left, so a note
  carrying two is heard twice, once for each, which is kept. An ornament
  on anything but a note is refused, as music21 refuses one by asking
  whatever it is given for its single pitch.
  `realize_ornaments_parity` holds it to music21 by the shared outline:
  thirteen subjects built for it -- mordents, trills, turns with their
  accidentals written, a tremolo's tied pieces, an ornament too long for
  its note, ornaments in voices, a key change between two trills -- and a
  sweep of every corpus score carrying an ornament, of which 26 compare
  and three are refused by both sides with the same sentence, a trill on a
  grace note having no time to steal.
- **A score reduced to the chords it is made of** (`analysis::reduce_chords`):
  music21's `analysis.reduceChords.ChordReducer.run`, which the crate had
  no counterpart for at all. The notes are read as spans of time and
  thinned out -- spans of no length go, each part's are fused where they
  sound alike in one measure, they are cut where the bass moves, the notes
  above the bass of a dissonant moment go, the gaps are filled, and spans
  shorter than half a note and then a note go unless they fill their
  measure -- and what is left is chordified against the score, each measure
  keeping the three chords weighing most in it (how long each sounds, times
  how strong its beat is, a tenth of that where it is dissonant).
  `reduce_chords_parity` holds it to music21 on twelve corpus scores --
  chorales, string quartets, a trio sonata, madrigals, early music whose
  meters change, a Palestrina Agnus -- every chord of every measure the
  same, with its offset and its length.
  Two things about music21's own reduction are worth knowing, since both
  decide what a measure keeps. Its weights are keyed by
  `tuple({p.pitchClass for p in chord})`, whose order is CPython's for a
  set of small integers, so two voicings of the same classes weigh as one
  chord where their hash tables came out alike and as two where they did
  not; `python_set_order` reproduces that order. And it takes the last of
  `allParts()` as the bass, sorting a set of part objects by the id of each
  part's instrument -- so where two parts share an instrument, as a piano's
  two staves do, which one music21 calls the bass falls out of where those
  objects sit in memory. The crate keeps the score's own order there, so
  the lower staff is the bass, and the parity test passes such a score over
  rather than pretend to compare it.
- The feature map tracks `search/serial.py`, music21's serial search,
  which the crate had ported with a parity test over corpus scores and the
  map listed nowhere: 41 members over five entries, of which the crate
  answers 19 and every one left out carries its reason -- the five matcher
  subclasses are `Matching`'s kinds rather than classes of their own, the
  twelve methods music21 dispatches between for each way of reading
  repetitions are one walk here, and the six `label*` functions are
  `SegmentMatcher::label` given the `Matching` each name says. The map's
  header now also accounts for the three files it is right to list as
  nothing: `features/jSymbolic.py`'s 72 extractors and
  `features/native.py`'s 24, which are rows of a generated table rather
  than a class each; `midi/percussion.py`, whose mapping between a
  percussion key and the instrument that plays it lives inside the MIDI
  port; and `figuredBass/rules.py`, whose `Rules` is nineteen switches with
  no member a scan can see. So the map now really does name every music21
  file the crate ports.
- A `**harm` spine is read as the roman numerals it annotates:
  `humdrum::harm`, music21's `humdrum.harmParser`, and `from_humdrum`
  putting each numeral where the staff's element of that line stands. A
  token is a degree with what is written round it -- `V7b`, `#IVo7`,
  `Gnd`, `V/V/V`, `(I)`, `I[V]` -- and the two notations are not the same:
  the inversion letter stands for the figured bass, so `V7b` is a `V65`,
  and a German or French sixth is read as a seventh chord whatever
  intervals are written. music21 reads a token with a regular expression
  and the crate has none, so `Harm::parse` reads it by hand in the same
  order; every one of 120 tokens agrees with music21, its own tests' and
  docstrings' among them. No corpus file has such a spine, so
  `humdrum_parity` reads one written for it, outline and MusicXML both,
  and the Python package reads it too, since `from_humdrum` hands back the
  numerals as `RomanNumeral`s.
- A chord carrying a roman numeral sorts where music21 sorts a
  `RomanNumeral`, which is a `Harmony`: at 19, before the notes standing
  at the same offset. Nothing but a `**harm` spine had ever put one beside
  a note.
- **The lab's tabular analyses, read and written** (`romantext::tsv`):
  music21's `romanText.tsvConverter`, the format the Digital and Cognitive
  Musicology Lab annotates its corpora in. `from_tsv` reads a table into a
  score of `RomanNumeral`s -- the measures prepared from the table's own
  numbers and meters, each numeral inserted at its beat and lasting until
  the next, and a repeat bracket over the measures a volta marks -- and
  `to_tsv` writes a score back out as a table, column for column. Both of
  the lab's standards are read: version 1 as the Annotated Beethoven Corpus
  was first released, and version 2, whose columns are named differently and
  whose labels carry the added tones in parentheses -- `V(64)` is music21's
  `Cad64` and `i(4+2)` its `i[no3][add4][add2]`. The cells music21 keeps on
  a numeral's `editorial`, which are the columns its reader did not read,
  are the caller's here: `to_tsv_with_extra` takes back what
  `tsv_to_chords` read them into, since a chord of the crate's keeps
  nothing of the kind. `tsv_parity` holds both halves to music21 -- the four
  tables music21 ships read identically, every measure, offset, length and
  pitch of them and the repeat brackets besides, and 140 tables written back
  byte for byte: those four and all 68 RomanText analyses of music21's
  corpus, each in both standards. The wheel reads and writes one too,
  `music21_rs.from_tsv` and `music21_rs.to_tsv`, each taking the
  `dcml_version` music21 takes and defaulting as music21 defaults.
- **The rock corpus's analyses, read** (`romantext::clercq_temperley`):
  music21's `romanText.clercqTemperley`, the de Clercq-Temperley flavour of
  RomanText. `from_clercq_temperley` reads a song -- a handful of rules,
  each a line naming a section's chords a measure at a time, and an `S:`
  rule playing them through, `S: [G] $In $Vr $Vr $Ch` -- into a part of
  `RomanNumeral`s: a measure for each the rules name, its chords sharing
  the bar equally, each figure written under its chord as a lyric, and the
  name of the rule that brought it under the first chord of its section. A
  rule may expand another (`$VP*2`), repeat a measure of its own (`|*4`),
  say the key or the meter in brackets, repeat the chord before it with a
  dot, and rest a bar with an `R`; a chord repeated across a barline is
  tied rather than struck again. `CtSong` and `CtRule` are music21's two
  classes, the three passes of its `_measureGroups` included.
  `clercq_temperley_parity` holds the reader to music21 on 207 songs --
  the six music21 carries in the module, eleven written for the test and
  200 made up out of the whole grammar -- every one agreeing as an outline
  *and* as MusicXML byte for byte, with ten more that both sides refuse.
  The wheel reads one too, as `music21_rs.from_clercq_temperley`.
- **The feature map now tracks every music21 file the crate ports**, where
  it tracked 44 of them: `percussion.py`, `tablature.py`, `text.py`,
  `layout.py`, `style.py`, `spanner.py`, `repeat.py`, `variant.py`,
  `metadata/__init__.py`, the `search` package, the `tree` package,
  `analysis/reduction.py`, `analysis/correlate.py`, `features/base.py`, the
  `braille` package, `alpha/analysis`, `omr` and `audioSearch`. The map is
  the report's ported table and the crate's to-do list, and all of that was
  ported and untracked -- so the report said nothing about any of it and
  nothing said what was left in it. It is 76 classes and 533 members more,
  of which the crate answers 365, and **every member left out carries its
  reason**, so nothing in the whole map is missing with no reason given.
  What the widening says: `variant.py` is ported whole, all ten of its
  functions, as are `search/base.py`'s eighteen, `analysis/correlate.py`,
  `Horizontality` and `LyricSearcher`; the braille transcription, the OMR
  correction and the audio search are ported nearly whole; `Spanner`
  answers 3 of music21's 14 members, the other eleven being object and
  site machinery a spanner naming positions needs none of; and
  `Verticality` answers 13 of 14. The 72 jSymbolic extractors and the 24
  native ones are rows of a generated table rather than a class each, which
  `features_parity` checks against music21's live classes. The map's header
  now also says what it deliberately leaves out and why -- the readers and
  the writers, which are one function in and one out and are held to
  music21 by parity tests over whole documents; music21's small helper
  modules, whose content the crate ports where it uses it; and what the
  crate does not model at all.
- **The General MIDI percussion map is `instrument::for_percussion_key`**,
  music21's `midi.percussion.PercussionMapper.midiPitchToInstrument`: the
  instrument a drum key plays, played the way that key says -- key 36 is a
  bass drum played `1` where key 35 is the same drum played `acoustic` --
  with music21's own refusal for a key its map names no instrument for.
  `Instrument::percussion_pitch` is the other direction, its
  `midiInstrumentToPitch`. `percussion_parity` holds both to music21: all
  128 keys and all 116 instrument classes agree, down to the modifier and
  the drum it plays.
- The Python package's `Dynamic` says what it is -- `<music21.dynamics
  .Dynamic mf>`, and the class alone where it says nothing, which is what
  music21's own `__repr__` writes of `_reprInternal`. It was the last class
  of the package whose repr differed from music21's with music21 absent;
  every other one already answers what music21 answers, the module each
  class names being music21's own.
- `Metadata::best_title` is music21's `bestTitle` -- the first of `title`,
  `popularTitle`, `alternativeTitle` and `movementName` that says anything
  -- and `Metadata::contributors_by_role` is its `getContributorsByRole`,
  which answers the contributors with no role of their own when asked for
  none, as a MusicXML file may name one. Both were found by widening the
  map, which is what the map is for.
- The report writes a note or a reason the way the map writes it: a name
  in backticks is `<code>` on the page, as the suite notes already were.
  There were 500-odd literal backticks on the published page, and the
  widened map would have added hundreds more.
- **Three fixes to how the map reads a class, each found by the new
  entries.** A name in a comment is not a port: the match was the word
  anywhere in the Rust, and every file here opens by naming the music21
  class it ports, so `repeat.Expander` and the four `*FretBoard`s read as
  ported where the crate merely mentioned them; the source is read with its
  line comments taken out now, and no entry already in the map changes,
  which says the map was honest before. A class body ends at the next
  function written against the left margin: a module-level function
  standing after a class gave the class the functions it defines *inside
  itself*, so music21's `prepareBeamedNotes` lent three filters to the
  `BrailleGrandSegment` above it and `StaffGroup` and `StreamSearcher` each
  carried one such ghost -- three members music21 never had are off the
  map. And a public field is a definition, since a value type answers with
  one where a method would be ceremony: `TextBox.content` is `pub content:
  String` here.
- **How often each value of some property of a stream's elements comes**
  (`analysis::elements::attribute_count`): music21's
  `analysis.elements.attributeCount`, the one function of a module the
  feature map listed nowhere. The property is a function of the element
  rather than the name of one, answering nothing where the element has
  none -- which is music21's `hasattr` test -- and the elements are
  whatever reading the caller counts over, a stream's own or a recursive
  or filtered one, as music21 takes a stream or a stream iterator. The
  counts come in the order each value was first met, which is the order
  Python's `Counter` keeps, so `most_common` leaves ties as they stand.
  `analysis_parity` holds it to music21 on 46 corpus scores and swept 49
  more -- every note counted by name, and every note and rest by length,
  in the same order on both sides. The wheel has it too, counting a named
  property as music21 does.

## fixed

- **A drum struck on a key other than its instrument's own default was read
  as that default**, and so written back as the wrong key: the MIDI reader
  built the instrument of a percussion key without the modifier the key
  says, so a bass drum on key 36 came back as key 35, a closed hi-hat (42)
  and an open one (46) both as the pedal (44), every tom-tom as the low
  floor tom (41), and the low bongo, woodblock and timbale as the high one.
  The reader goes through `instrument::for_percussion_key` now, as
  music21's goes through its mapper, and a stroke read from a file comes
  back played the way the key says.
  Nothing caught it: the outline the reader tests compare named the
  instrument's class and not how it is played, and `demos/drum_sample.xml`,
  the one drum subject either test had, strikes only default keys. The
  outline says the modifier now; `written:percussion.mid`, a file written
  for the test holding all 35 mapped keys and four music21 has no
  instrument for, is a subject of `midi_read_parity` and catches it; and
  `built:percussion` in `midi_write_parity` is a stroke of each of fifteen
  drums played its own way, whose keys both writers write alike.

# music21-rs 0.10.0

The Python package stops being a stream of notes and becomes a package: it
holds what a score says. Barlines and repeats, the words over the staff,
the eleven spanners music21 has classes for and the bundle beside them, a
note's and a chord's editorial, a score's metadata -- and every analysis
member the crate had ported and the package did not answer, which is
`analysis.patel`, `analysis.metrical`, `analysis.discrete`,
`analysis.floatingKey`, `analysis.windowed`, `analysis.segmentByRests`,
`analysis.pitchAnalysis`, the four instrument stream functions and the
figured-bass realizer. Its `Stream` answers 99 of music21's 131 members,
every one the crate has ported but five, and each of those five is left
out with its reason said. The readers carry all of it through now: a score
read with `from_musicxml` had no repeats and no slurs a caller could see.

Four classes of defect were found and fixed on the way, each by a test
that now holds it: `clone_empty` dropped a measure's number and a part's
name, so a template lost both; two gaps in what a class answers to (`Key`
and `Instrument`) meant `getElementsByClass('KeySignature')` found nothing
where music21 finds the signature; one `OffsetSpan` existed twice with two
different readings of music21's own rule; and the package's `Editorial`
stood in for music21's, which cost 34 of music21's own tests until
music21's class was given the job back wherever music21 is there to ask.

## added

- The Python package's `Stream` answers `analyze`, music21's own: the
  ambitus as the interval from the lowest pitch sounding to the highest, or
  the key one of music21's five weightings settles on, the method name read
  as music21 reads it. A method nobody answers to, and a key analysis of a
  stream with no pitches, raise `DiscreteAnalysisException`, which is the
  class music21 raises.
- The Python package's `Stream` answers `getElementsByOffset` with
  music21's four keywords and a `classList`, `getElementsNotOfClass`,
  `hasElementOfClass`, `isTwelveTone` and `staffLines`. The offset filter
  asks the crate's `OffsetSpan` of each element the stream holds rather
  than keeping a second copy of music21's rules.
- `stream::OffsetSpan::takes_at` is public: whether something standing at
  an offset and lasting so long is in the span, which is all the decision
  turns on.
- The Python package gains `Metadata`, music21's `metadata.Metadata`: a
  name is read and written as music21's `__getattr__` reads one, by its
  unique name, by one of the four grandfathered work ids or by one of the
  twenty Humdrum abbreviations, so `md.otl` and `md.title` are the same
  title. `search`, `all` and `md['title']` answer as music21's do. What a
  name answers is a **string**, where music21 answers its own `Text` and
  `Contributor`, so `all(returnPrimitives=True)` is refused; that is also
  why nothing here is installed over music21's own `metadata`, where being
  silently different would be worse than being absent.
- The package's `Stream` carries `metadata`, which music21 keeps among the
  elements rather than beside them, and its `Opus` gains `scores`,
  `getNumbers`, `getScoreByNumber`, `getScoreByTitle` and `mergeScores`.
  A lookup hands back the score the opus holds, and asks the metadata
  itself, so music21's own answers where music21 is installed and the
  package's own where it is not.
- The Python package's `Stream` stretches, takes in and empties itself as
  music21's does: `extendDuration`, `mergeElements` -- which takes in the
  other stream's own objects, as music21 does, so both hold the same notes
  -- and `cloneEmpty`. It answers `seconds` and `metronomeMarkBoundaries`
  too, handing back the stream's own mark wherever it holds one.
- The Python package's `Stream` answers `playingWhenAttacked` and
  `allPlayingWhileSounding`: what it holds sounding where an element of
  another stream starts, and everything it holds sounding while that
  element sounds, each at its offset from where the element stands.
- The Python package's `Stream` answers `extractContext`, what it holds
  around one of its elements, the elements being its own.
- The Python package's `Stream` answers `pitches`, every pitch of
  everything it holds -- a note's one, a chord's several, a nested
  stream's own, each the very object that note holds -- with a key passed
  over as music21 passes it over.
- The Python package's `Stream` answers `offsetMap`, music21's list of
  `OffsetMap` named tuples -- the element, where it begins, where it ends
  and which voice it is in -- a stream with voices read voice by voice as
  music21 reads it.
- The Python package's `Stream` answers `measures` and `measure`: the
  measures of a number range, the first at nought and before it what was
  in force where it begins -- the clef, meter, instrument, key signature
  and tempo mark music21's `collect` names -- and a score answers a score
  of each part's. The package's streams sort by music21's `priority`
  before `classSortOrder` now, which is what puts those collected marks
  before the measure they head.
- The Python package's `Stream` answers `splitByClass`, dividing the
  elements of a class in two by what a function says of each.
- The Python package's `Stream` answers `insertAndShift`, moving
  everything from the element nearest after the offset far enough that it
  starts where the new one ends, as music21's does.
- The Python package's `Stream` answers `measureOffsetMap`, a score's
  built out of its parts as music21's is, so the measures standing
  together in several parts are listed together.
- The Python package's `Stream` repeats an object into itself:
  `repeatAppend` and `repeatInsert`, each putting in copies as music21's
  `deepcopy` makes them.
- The Python package's `Stream` looks elements up around an offset:
  `getElementAtOrBefore`, `getElementBeforeOffset` and
  `getElementAfterElement`, each handing back the stream's own object.
  Of two things standing at one offset the later in the listing is the
  answer, which is music21's `max` over the sort tuples.
- The Python package's `Stream` answers `getTimeSignatures`,
  `getInstruments` and `getInstrument`, each handing back the stream's own
  objects, and making up music21's default -- a four-four, a bare
  `Instrument` -- where the stream states none and one is asked for.
- The Python package's `Stream` reads and writes what it opens with:
  `clef`, `keySignature` and `timeSignature`, each the one of its class
  standing at the very start, and writing one takes out the one it had, as
  music21's setters do. The crate answers the clef *in force* instead
  (`in_force_before`), a stream of values having no object to hand back;
  these are the package's own.
- The Python package's `Stream` takes elements out by class and by index
  (`removeByClass`, `removeByNotOfClass`, `pop`) and answers `voices`,
  the voices it holds being its own objects.
- The Python package's `Stream` cuts itself in two at a length
  (`splitAtQuarterLength`), ties each run of one note together
  (`extendTies`), moves what it holds (`shiftElements`) and dissolves a
  measure's lone voice (`flattenUnnecessaryVoices`). `extendTies` and
  `shiftElements` change the stream and answer nothing, as music21's do,
  and the ties are written onto the stream's own notes.
- The Python package's `Stream` turns itself to sounding and to written
  pitch (`toSoundingPitch`, `toWrittenPitch`), and answers
  `simultaneousAttacks`, `hasPartLikeStreams` and `findGaps` -- whose
  gaps are rests here, where music21 answers bare objects carrying the
  same offset and length.
- The Python package's `Stream` notates itself as music21's does:
  `makeMeasures` -- taking a `meterStream`, as music21 does --
  `makeRests`, `makeTies`, `makeBeams`, `makeAccidentals`, `makeVoices`
  and `makeNotation`. Each step is taken at music21's own defaults and a
  keyword asking for anything else is refused rather than answered
  differently; `makeVoices` fills no gaps, which is why music21's own
  `fillGaps=True` is refused.
- The Python package's `Stream` answers `transpose`, `stripTies`,
  `quantize` and `template` as well, each handing back a new stream as the
  package's other edits do. `transpose` takes an `Interval`, a name or a
  count of semitones; `template`'s `removeClasses` and `exemptFromRemove`
  are refused, the classes taken out being music21's own default set, and
  `removeAll=True` takes out everything.
- The Python package's `Stream` answers `lyrics`, music21's own: every
  line gathered by the number it is sung under, with a place for each note
  read so the lines stay level with the notes, and the note's own `Lyric`
  object in it. `ignoreBarlines`, `recurse` and `skipTies` answer as
  music21's do.
- `Stream::lyric_positions` is the by-position twin of `Stream::lyrics`,
  answering `stream::SungAt` and `stream::SungPlace`: where each lyric is
  sung, as a position among `Stream::leaves` and an index among that
  note's lyrics.
- `Stream::metronome_mark_positions` is the by-position twin of
  `metronome_mark_boundaries`: each stretch with the position among
  `Stream::leaves` of the mark it was read off, which is how a caller
  keeping an object per element finds the very mark music21 hands back.
  `stream::TempoSpan` is its answer.
- `metadata::unique_name_of` is public: the unique name an attribute name
  stands for, which music21 reads through `__getattr__`.

- `Stream` answers music21's questions of its layout: `find_gaps` and
  `is_gapless`, `overlaps` and `is_sequence` (music21's `getOverlaps`,
  which gathers only the overlaps found looking on from each element until
  the first it does not overlap), `simultaneous_attacks`,
  `playing_when_attacked`, `all_playing_while_sounding`, `offset_map`,
  `measure_offset_map`, `highest_offset`, `lowest_offset`, `has_measures`,
  `has_voices` and `is_well_formed_notation`. each answers as music21's
  does on every MusicXML score of music21's corpus.
- `Stream` cuts its notes and rests as music21's slicing does:
  `slice_by_quarter_lengths`, `slice_by_greatest_divisor` (by measure, and
  across a score's parts at each measure), `slice_at_offsets`,
  `slice_by_beat` and `split_by` (music21's `splitByClass`). Grace notes
  are left whole and a measure with nothing of its own to cut is left as
  it is; a note in a voice is cut in its voice. each cuts every MusicXML
  score of music21's corpus as music21 does.
- `Stream` edits itself as music21's editing methods do: `extend_ties`,
  `scale_offsets` (from a `stream::Anchor`), `scale_durations`,
  `augment_or_diminish`, `invert_diatonic`, `shift_elements`,
  `repeat_append`, `repeat_insert`, `insert_and_shift`, and for a measure
  `bar_duration_proportion` and `pad_as_anacrusis`. each edits every
  MusicXML score of music21's corpus as music21 does.
- `Stream` moves music between voices and parts as music21 does:
  `voices_to_parts` (by place, or with `separate_by_id` by voice id) and
  `explode` make a part of each voice, each with its part's instruments
  and the clef that best fits its notes; `parts_to_voices` (grouped by a
  `stream::VoiceAllocation`) and `implode` make the parts of each group
  the voices of one part, stems set up and down if asked. as in music21,
  `parts_to_voices` takes only the notes a measure holds itself, not those
  already in its voices. each moves every MusicXML score of music21's
  corpus as music21 does.
- `Stream` finds elements by where they stand as music21 does:
  `element_at_or_before`, `element_before_offset` and `element_after`
  (music21's `getElementAtOrBefore`, `getElementBeforeOffset` and
  `getElementAfterElement`), each of the music21 classes named;
  `beat_and_measure_from_offset`, which counts a partial measure's beats
  on from the measure before it; and `time_signatures`, `instruments` and
  `instrument` (music21's `getTimeSignatures`, `getInstruments` and
  `getInstrument`), which look within the stream. each answers as
  music21's does on every MusicXML score of music21's corpus.
- `Stream` keeps and takes out elements as music21 does:
  `extract_context` (the elements from some way before one to some way
  after it, bounded as music21 bounds them), `remove_by_class` and
  `remove_by_not_of_class`; and `harmonic_intervals` gives each note the
  interval to the first note of another stream sounding with it, which
  music21's `attachIntervalsBetweenStreams` attaches. each answers as
  music21's does on every MusicXML score of music21's corpus.
- `Stream::split_at_quarter_length` cuts a stream in two, as music21's
  `splitAtQuarterLength` does: what sounds across the point is cut and
  tied, what follows moves to the second half, a voice is cut as a stream
  is, and the second half of a measure gets its time signature, the key
  and clef in force at the point, and its right barline. what is in force
  before a measure, which music21 finds in its context, is given as a
  `stream::InForce`, which `Stream::in_force_before` reads off a part.
  Every measure of every MusicXML score of music21's corpus is cut as
  music21 cuts it.
- the Python package's `Stream` gains music21's `sliceByQuarterLengths`,
  `sliceAtOffsets`, `sliceByBeat`, `sliceByGreatestDivisor`,
  `scaleOffsets`, `scaleDurations`, `augmentOrDiminish`, `invertDiatonic`,
  `voicesToParts`, `explode`, `partsToVoices` and `implode`, each handing
  back a new stream, and `highestOffset`, `lowestOffset`, `hasMeasures`,
  `hasVoices`, `isGapless`, `isSequence` and `isWellFormedNotation`.
- `Metadata::search` answers music21's `search`: a query, and a field that
  may be a unique name, one of music21's four grandfathered work ids or
  twenty Humdrum abbreviations (`OLD_MUSIC21_NAMES`), or any piece of a
  name -- `compos` finds `composer`. a `MetadataQuery::Text` matches by
  containment with either case ignored, as music21's plain string query
  does, and a `MetadataQuery::Matcher` stands in for the regular
  expression music21 reads out of a query holding `*.|+?{}`, since the
  crate compiles no pattern itself. `Metadata::all_sorted` reads the names
  in the order music21's `all` reads them.
- `Stream` looks a score up in an opus as music21's `stream.Opus` does:
  `scores`, `score_numbers`, `score_by_number`, `score_by_title` and
  `merge_scores`. each answers as music21's does on every multi-tune ABC
  file of music21's corpus.
- `Stream::elements_in_span` is music21's `getElementsByOffset`, taking a
  `stream::OffsetSpan`: one offset or a stretch, with music21's four flags
  -- `include_end_boundary`, `must_finish_in_span`, `must_begin_in_span`
  and `include_elements_that_end_at_start` -- starting at music21's own
  defaults, under which an element is found only where it *begins*. a span
  of no length takes the snapped start as its end, as the fork's music21
  does, so a float a hair off the fraction an element stands at still
  finds it. `elements_of_class`, `elements_not_of_class` and
  `has_element_of_class` are music21's `getElementsByClass`,
  `getElementsNotOfClass` and `hasElementOfClass`. every measure of every
  MusicXML score of music21's corpus answers all thirty-eight spans as
  music21 answers them.
- `Stream::seconds` is music21's `seconds`: how long a stream lasts at the
  tempi it holds, each holding until the next and the last to the end.
  Where the stream states no tempo, or none from its start, the tempo in
  force is given to it, as music21 searches its context for one. a stream
  holding nothing that lasts is nought seconds long and one holding
  something is `NaN`, which is music21's answer rather than a refusal.
  `Stream::metronome_mark_boundaries` is music21's
  `metronomeMarkBoundaries`, moved from `features::DataInstance` to the
  class music21 keeps it on.
- `Stream::final_barline`, `final_barlines` and `set_final_barline` are
  music21's `finalBarline`: the right barline of the last measure a stream
  holds, or a measure's own, and nothing for a stream holding no measures.
  A score answers one barline per part, which `final_barlines` gives, and
  writing one reaches the last measure of every part.
- `Stream::staff_lines` is music21's `staffLines`: the lines named by the
  first staff layout standing at the start of the stream, however deep,
  and five where none names any. only the staff layouts are read, so a
  note or a measure standing later does not stop the search.
- `Stream::extend_duration` is music21's `extendDuration`: a copy with
  each element of a class stretched to where the next one starts, the last
  to the end of the stream. only what the crate gives a length to is
  stretched -- a note, a rest, a chord, a chord symbol -- where music21
  stretches any element, everything there carrying a duration; the marks
  the crate keeps timeless have nothing to write.
- `Stream::merge_elements` is music21's `mergeElements`, taking in what
  another stream holds at the offsets it holds them at, filtered by class
  where asked. music21 stores the other stream's own objects in both
  streams; a crate stream owns what it holds, so they are copied.
- `Stream::clone_empty` is music21's `cloneEmpty`: the same kind and id,
  holding nothing.
- `Stream::analyze` is music21's `analyze`, answering a `stream::Analysis`:
  `ambitus`, `span` and `range` give the interval from the lowest pitch
  sounding to the highest, and `key` and the names of music21's five key
  weightings give the key the stream is most likely in. the name is read as
  music21 reads it, three searches over and loosest last, so `kostka` finds
  Temperley's profile and a bare `key` finds Aarden's. a stream with no
  pitches has no ambitus and answers nothing, where a key analysis of one
  refuses, which is what music21 answers and raises.
- `Stream::is_twelve_tone` is music21's `isTwelveTone`, and
  `Stream::notes_and_rests` is its `flatten().notesAndRests`, where
  `Stream::notes` leaves the rests out. what the stream holds itself is
  `elements_of_class` for `GeneralNote`.
- `Stream::lyrics` is music21's `lyrics`, gathering the syllables sung in
  a stream by the number each is sung under, taking a
  `stream::LyricsOptions`. each number's line keeps one place for every
  note read -- the lyric sung there, or `SungLyric::Unsung` where that
  note sings nothing under that number -- so the lines stay level with the
  notes, and a number first met partway through is padded with the notes
  before it. as in music21, the notes of a measure are read through the
  barlines by default, every other nested stream only where asked, and
  each then answers a `SungLyric::Nested` list of its own.
- `StreamElement::is_instance` and `StreamElement::music21_classes` are
  public: music21's `isinstance` by class name, and the class names an
  element answers to.
- `makenotation::op_frac` is public: music21's `opFrac`.
- `audiosearch` ports music21's `audioSearch`: frequencies found by
  autocorrelation (`autocorrelation_frequency`, `frequencies_from_wav`),
  snapped to a scale (`prepare_thresholds`, `normalize_input_frequency`,
  `detect_pitch_frequencies`), smoothed (`smooth_frequencies`), made
  pitches (`pitch_frequencies_to_objects`) and notes and rests
  (`join_consecutive_identical_pitches`), quantized into a stream
  (`quantize_duration`, `quarter_length_estimation`, `histogram`,
  `notes_and_durations_to_stream`), and all of it at once as music21's
  transcriber does (`monophonic_stream_from_wav`); and the score
  follower's `decision_process`. every stage gives music21's answers, bit
  for bit, on music21's own test recording and on tones made for the
  test; the frequencies, which music21 finds by FFT, agree to within a
  millionth.
- the `language-detection` feature brings music21's language feature,
  `TX1`, which names the language of a piece's lyrics, last among
  `features::native::NATIVE` as it is in music21, and
  `features::language`, music21's `LanguageDetector` and `Trigram`:
  `most_likely_language` and `most_likely_language_numeric` compare a text
  with excerpts in English, French, Italian, German, Chinese, Latin and
  Dutch by the letters that follow each pair of letters. the excerpts are
  music21's, from Project Gutenberg, about 1.9 MB, so the feature is off by
  default. `TX1` gives music21's value on every MusicXML score of
  music21's corpus.
- The Python package's `Stream` answers
  `attachIntervalsBetweenStreams`: the interval to what sounds against
  each of its notes in another stream, written onto the note's
  `editorial.harmonicInterval` where music21 writes it, and cleared
  first, so a note asked twice never keeps the older answer.
- The Python package answers music21's `analysis.patel` and
  `analysis.metrical`: `nPVI` and `melodicIntervalVariability` -- which
  take `findConsecutiveNotes`' keywords, as music21 passes them on --
  `labelBeatDepth` and `thomassenMelodicAccent`.
- The Python package answers `analysis.segmentByRests.Segmenter`,
  `analysis.pitchAnalysis.pitchAttributeCount` and
  `harmony.alterToChordSymbolString`. A segment is the stream's own notes
  and an interval is built on two of them, so `noteStart` is one of the
  score's notes; a count is a `collections.Counter` of whatever property
  of a pitch was named, as music21 counts one.
- The Python package gains `analysis.windowed.WindowedAnalysis`:
  `getMinimumWindowStream`, `analyze` and `process`. The windows are the
  crate's -- the stream cut into quarter-note measures with what runs past
  a barline tied, and the windows of each size put end to end -- and each
  is handed to the processor's own `process` as a stream of the package's.
  The colour is the processor's, which is why this carries `analyze` and
  `process` where `discrete`'s own classes do not: music21's processors
  work unchanged where music21 is installed, and so does a processor a
  caller wrote. A window the processor cannot read answers
  `((None, None, 0), '#ffffff')`, music21 warns where the windows do not
  divide evenly, and a score is flattened first -- all three as music21
  does them.
- A note's and a chord's `editorial` is **music21's own** wherever music21
  can be imported, and the package's own `Editorial` only for a caller who
  has no music21 at all. music21's is a dict subclass with the whole dict
  API -- its `tsvConverter` calls `update` on one -- and music21 keeps it
  where its copying carries it along; the package's stand-in had neither,
  so a copied note lost what an analysis had written on it and music21's
  own figured-bass realizer could not read back the `notationString` it
  had just written. music21's own suite is what found it: 34 of its tests
  failed under the crate and none on music21. Same bargain `style.Style`
  strikes, and for the same reason.
- `Chord::is_sorted_diatonic_ascending` is music21's
  `isSortedDiatonicAscending`, new in music21 11: whether the notes
  already stand lowest to highest on the staff, which is where its own
  sort asks before it copies anything. Staff position decides first, so
  `B#3 C4` is sorted and `C4 B#3` is not. The facade answered it with a
  walk of its own and the crate did not answer it at all; it is the
  crate's now, and `Repeat::repeat_text_expression` is credited in the
  feature map, which leaves nothing in the whole map missing with no
  reason given.
- The Python package gains the figured-bass realizer: `FiguredBassLine`
  and `Realization`, music21's `figuredBass.realizer`. A line is bass notes
  with their figures in a key and a meter -- `addElement`,
  `generateBassLine`, `overlayPart`, `retrieveSegments`, `realize` -- and a
  realization is every voicing that keeps the rules: `getNumSolutions`,
  `getAllPossibilityProgressions`, `getRandomPossibilityProgression`,
  `generateRealizationFromPossibilityProgression`,
  `generateAllRealizations`, `generateRandomRealization` and
  `generateRandomRealizations`, in keyboard or chorale style.
  Where music21 chooses at random this chooses through Python's own
  `random`, which is what music21 draws from, so a caller who seeds it gets
  the same realization twice. A voicing is a tuple of pitches, as music21's
  is, and a score is this package's own streams. music21's own
  `retrieveSegments` appends the bass line to its overlaid parts, so asking
  it twice is not asking the same thing twice; this reads the line and
  leaves it as it was. `addLyricsToBassNote` is here too, and a chord now
  carries an `editorial`, which is where the realizer writes the figures it
  was added with.
- The Python package's `TimeSignature` says what it is: `<music21.meter
  .TimeSignature 3/4>`. Installed over music21's, a class answers through
  music21's own `Music21Object.__repr__`; this one is installed over
  nothing, so without it a meter read as the object it is at an address.
- The Python package gains the `analysis.discrete` classes: `Ambitus`, the
  five key weightings (`KeyWeightKeyAnalysis` and the `KrumhanslSchmuckler`,
  `AardenEssen`, `SimpleWeights`, `BellmanBudge` and
  `TemperleyKostkaPayne` subclasses) and `MelodicIntervalDiversity`. A span
  is the stream's own pitch objects and the interval built on them, a key
  carries how well it fitted and the twenty-three that fitted less well,
  and a counted interval is built on the two notes it was measured
  between. `process` is left out for `Ambitus` and for a key weighting,
  since it answers a solution *and* a colour out of music21's colour
  dictionaries, which are presentation and no part of the crate;
  `MelodicIntervalDiversity.process` is answered, its colour being the one
  white string music21 has not worked out yet -- and music21's own raises
  there, passing `ignoreDirection` where `found` goes.
- `analysis::melodic_line_positions` is the by-position twin
  `analysis::melodic_lines` now reads its own answer off: the note a count
  was taken from is the note the stream holds, so the package can hand back
  its own. `KeyProfile::music21_name` and `identifiers` are public, being
  the table `Stream::analyze` reads a method name against.
- The Python package gains `analysis.floatingKey`'s `KeyAnalyzer` and
  `divide`: the key of every measure, read one measure at a time by the
  Aarden-Essen weighting and then smoothed by the measures around it. Each
  raw key carries how well it fitted and the keys that fitted less well,
  as music21's does, and `weightAlgorithm` is a Python function, so a
  caller may weigh the window however it likes. music21 keeps each
  measure's readings once asked for; the package reads them again, so a
  stream edited between two questions is answered as it stands.
- The Python package gains `Editorial`, music21's `editorial.Editorial`:
  the dictionary with attribute access where an analysis leaves what it
  worked out, which a note answers as its `editorial` -- the same object
  every time, so what is written on it stays. `ficta`,
  `harmonicInterval` and `melodicInterval` answer `None` before anything
  writes them and `footnotes` and `comments` an empty list, which is
  music21's own predefined set, and anything else raises until written.
  `thomassenMelodicAccent` writes its answer there, where music21 writes
  it.
- The Python package answers music21's four instrument stream functions:
  `partitionByInstrument`, `bundleInstruments`, `unbundleInstruments` and
  `deduplicate`, each handing back a new stream as every edit here does.
- The Python package's `Measure` answers everything the crate has ported
  for one: `measureNumberWithSuffix`, `barDuration`,
  `barDurationProportion`, `bestTimeSignature` and `padAsAnacrusis`. It
  carries `number`, `numberSuffix`, `paddingLeft` and `paddingRight` as
  music21's own properties, with music21's defaults, and its constructor
  reads `Measure(number='14a')` as music21 reads it -- all four were
  attributes the readers wrote and nothing declared, so a measure built by
  hand had none of them.
- The Python package's `Part` carries `partName` and `partAbbreviation`,
  music21's own properties: the name said of the part, and where nothing
  is said the first instrument standing anywhere in it -- its `partName`,
  else its `instrumentName`, and the abbreviation likewise -- which is
  where music21 looks too, a part's instrument often standing in its
  first measure. music21 keeps what it found; the package looks again,
  so a part given an instrument afterwards answers for it.
- The Python package gains the spanners: `Spanner` and the eleven kinds
  music21 has classes for -- `Slur`, `Crescendo`, `Diminuendo`, `Ottava`,
  `Line`, `Glissando`, `TrillExtension`, `TremoloSpanner`,
  `ArpeggioMarkSpanner`, `PedalMark` and volpiano's `Neume` -- with
  `getSpannedElements`, `addSpannedElements`, `getFirst`, `getLast`,
  `hasSpannedElement`, `replaceSpannedElement` and `placement`. A stream
  answers `spanners` and `spannerBundle`, and `SpannerBundle` answers
  `getByClass`. The readers carry them through now, so a slur read with
  `from_musicxml` or `from_abc` names the score's own notes, and a slur
  built here reaches `to_musicxml`. The crate's readers gather a part's
  spanners onto the score where music21's reader leaves them on the part,
  so `spanners` answers differently of those two while `spannerBundle`,
  which asks the whole tree, answers the same.
- The Python package gains `Barline`, `Repeat` and `TextExpression`,
  music21's `bar.Barline`, `bar.Repeat` and
  `expressions.TextExpression`. A measure carries its `leftBarline` and
  `rightBarline`, a stream answers `finalBarline` (a list, for one holding
  parts) and `expandRepeats`, and a repeat says its count in words with
  `getTextExpression`. The readers carry barlines, repeats and words
  through now, where before they were read into the crate and left out of
  what the package handed back -- so a score read with `from_musicxml`
  kept its repeats only as far as writing it again.
- `Barline::repeat_text_expression` is music21's
  `Repeat.getTextExpression`: the count written between a prefix and a
  postfix, `3x` by default. A repeat that says no count answers nothing,
  where music21 writes the word `None` into the text.
- `data/feature_map.toml` tracks music21's `bar` module: `Barline` 2 of 2
  and `Repeat` 2 of 2 ported, all of them in the package.
- The Python package's `Stream` answers `insertIntoNoteOrChord`: a note
  or chord put in where something already starts within its length, the
  two becoming one chord, the merging being the crate's. Where nothing
  stands there the object handed in is the one the stream then holds, as
  music21 does.
- The Python package's `Stream` carries `atSoundingPitch` as music21's
  own property: `True`, `False` or `'unknown'`, `'unknown'` until
  something says, and anything else refused with music21's sentence. It
  was an attribute the readers wrote and nothing declared, so a stream
  nobody had told had none at all.
- The Python package's `Stream` answers `beatAndMeasureFromOffset`: the
  beat is the crate's arithmetic over the meter in force, and the measure
  is the stream's own object, found where music21 looks for it -- the last
  one standing at or before the offset, in the stream itself or in the
  first of its parts that has measures.
- `Stream::overlap_positions` is the by-position twin of
  `Stream::overlaps`, each group's members by their position among
  `Stream::leaves`, and the Python package's `Stream` answers
  `getOverlaps` on it -- the overlaps gathered by the offset each begins
  at, holding the stream's own elements, the one overlapped before the one
  that found it as music21's dictionary takes them.
- The Python package's classes answer `classes`, music21's own chain of
  what a thing is: a note is a `NotRest`, a `GeneralNote`, a
  `Music21Object`, a `ProtoM21Object` and an `object`; an ocarina is a
  `Flute` and a `WoodwindInstrument`; a measure is a `Generic`, music21's
  streams being typed by what they hold. The crate's lineage is what
  answers, which `class_lineage_parity` holds to music21's.
- `class_lineage_parity` holds `StreamElement::music21_classes` -- what
  the crate says a thing *is*, which every class filter in the crate and
  in the Python package reads -- to music21's own `classes`, for all 34
  kinds a stream can hold, music21's chain cut at `Music21Object` where
  the crate's stops. The table had been transcribed by hand and never
  checked.

## fixed

- `StreamElement::music21_classes` said a repeat barline was a `Barline`
  and no more, where music21's `Repeat` is a `RepeatMark` and a `Barline`
  in that order. `class_lineage_parity` covers both barlines now.
- the Python package's readers dropped a note's articulations and
  ornaments: the crate read them and the writers wrote them, so a score
  read and written again carried them, but `note.articulations` and
  `note.expressions` were empty on the objects handed back. Every object
  the package builds from a crate value carries them now. A fermata and an
  arpeggio are still left behind, the package having no class for either.
- `OffsetSpan` was two types of the same name and the same meaning: a
  private one in `stream::queries` with a predicate comparing raw floats,
  and the public `stream::span` one whose predicate compares through
  `Exact`, which is how music21 holds an offset -- a float or a
  `Fraction`. Both were music21's `isElementOffsetInRange`, line for line
  the same otherwise, and the six searches going through the private one
  (the consecutive walk, `shift_elements`, `playing_when_attacked`,
  `all_playing_while_sounding` and the two in `split_at_quarter_length`)
  could in principle answer a triplet offset the other way. The private
  type is gone; there is one span and one predicate.
- `StreamElement::music21_classes` said a `Key` was a `Key` and a
  `KeySignature` and no more, where music21's is a `DiatonicScale`, a
  `ConcreteScale` and a `Scale` besides -- which is where everything it
  answers about degrees comes from -- and said every instrument was an
  `Instrument` and nothing else, where music21 gives each its families: a
  flute is a `WoodwindInstrument`, an ocarina a `Flute`. The crate had the
  family chain all along (`Instrument::families`), unread by the lineage.
  `Instrument::class_name` is music21's own name for a kind, which is what
  the chain starts with.
- a class filter in the Python package matched a class by its own name
  and by the classes it extends, which are the ones music21 itself has a
  class for -- so `getElementsByClass('GeneralNote')` found nothing where
  music21 finds every note, and `('Music21Object')` nothing at all. What
  music21 says a thing *is* besides is answered by the crate's hierarchy
  (`StreamElement::music21_classes`) now, which every class filter goes
  through: `getElementsByClass`, `getElementsNotOfClass`,
  `hasElementOfClass`, `getElementsByOffset`'s `classList`,
  `removeByClass`, `splitByClass`, `mergeElements` and
  `measureOffsetMap`. A stream is asked by its kind, which costs nothing;
  anything else is read as the value it is, and only once every name a
  class is named after has failed -- the readers themselves do not come
  through it, so nothing reads a value to read a value.
- the report counted nothing of `Measure`, `Part`, `Score` and `Opus` as
  carried by the wheel, because it looked for each member in a wheel class
  of that name and the package puts every stream method on one `Stream`
  class the kinds extend. The four say `facade = "Stream"` now, so what an
  `Opus` really does answer -- `getNumbers`, `getScoreByNumber`,
  `getScoreByTitle`, `scores`, `mergeScores` -- is counted. Only members
  the `Stream` body actually defines are credited, which is what an
  instance of those classes answers.
- The Python package's `Clef`, `KeySignature`, `TimeSignature`,
  `Instrument`, `Dynamic`, `MetronomeMark`, `Metadata` and `Voice` carry
  music21's `classSortOrder`, which they were missing: what stands at one
  offset is listed in the order music21 lists it in, so a clef written
  after a note at the same offset is still listed first. Everything that
  reads a stream in order was affected -- `elements`, `getElementsByOffset`
  and the new `pop` among them -- and a stream built by one of the readers
  was not, its elements arriving in the crate's order already.
- `Stream::clone_empty` kept a stream's id alone, where music21's
  `cloneEmpty` merges the attributes of the class as well: a part's name
  and abbreviation (`Part.mergeAttributes`), a measure's number, suffix and
  padding (`Measure.mergeAttributes`), and whether the stream is at
  sounding pitch (`Stream.mergeAttributes`). `extractContext` had the right
  list in a copy of its own, and the two are one implementation now.
- the feature map pointed music21's `Interval.name` at the crate's `name`,
  which answers the long spelling, `Perfect Octave`; `short_name` is the
  one that answers `P8`, and the map now says so. the count is unchanged:
  the member was already read as ported, by the function that does not
  implement it.
- `MetronomeMark::seconds_per_quarter` reads the speed a mark is *played*
  at, as music21's `secondsPerQuarter` does through a `getQuarterBPM()`
  that takes `useNumberSounding=True`. a mark that says no number but
  carries a sounding one -- what a MusicXML `<sound tempo="96">` reads as,
  which much of music21's corpus carries -- was refused where music21
  answers, and so were `duration_to_seconds`, `quarter_length_to_seconds`
  and `seconds_to_duration`.

- `Stream::has_part_like_streams` counts a `PartStaff` as a part and a
  stream of chord symbols or strokes as holding music, as music21 does.
- an offset worked out to be negative is snapped to the fraction it stands
  for, as a positive one is, as music21's `opFrac` snaps it.
- merging variants finds an element where an offset worked out in floats
  falls a hair short of the fraction the element stands at, as music21
  does with its search for one offset fixed on the fork the crate is held
  to.
- an ABC measure cut in two because it overflows its bar keeps its
  closing barline at the end of the second measure, where it was put at
  the start, as music21 does with its `splitAtQuarterLength` fixed on the
  fork the crate is held to.
- `Stream::end_offset` is snapped to the fraction it stands for, as
  music21's `highestTime` is, so a stream ending on a triplet is as long
  as music21 says to the last bit.
- `TimeSignature::beat_proportion` snaps the beat to the fraction it
  stands for, as music21's `getBeatProportion` does, so a third of a beat
  in is music21's to the last bit.
- `Duration::augment_or_diminish` snaps the scaled length to the fraction
  it stands for, as music21 does; an unlinked duration stays unlinked,
  keeps its written values, and has its length scaled as it is, unsnapped.
- the MusicXML reader pads a pickup measure by the share of its bar it
  lacks, as music21's `padAsAnacrusis` works it out, and a measure cut
  short by what its exact length lacks, so the padding of a measure whose
  length is no binary fraction is music21's to the last bit.

# music21-rs 0.9.0

every score format music21 reads is read -- Capella, MuseData and
NoteWorthy join the others -- and every one it writes is written: MusicXML,
MIDI, RomanText, Volpiano, LilyPond and braille, and ABC besides, which
music21 does not write. most of music21's analysis beyond the chord is the crate's as well:
`chordify`, windowed and floating key analysis, the jSymbolic and native
features and their data sets, the `search`, `tree`, `text`, `layout`,
`variant` and `omr` modules, score reductions, `alpha.analysis`, and
tablature. each is held to music21 on its corpus, usually every MusicXML
score of it, output for output.

ABC is written as well as read. music21 writes no ABC, so `abc::to_abc` is
held to the two readers instead: on every tenth tune of music21's corpus,
1,294 of 1,298 are read back by the crate as the score they were written
from, and by music21 as the score it reads from the original. the other four
are tunes whose own reading cannot be written back.

## breaking changes

- `StreamElement` has a `Break` variant, a manuscript's line, page or column
  break, and `SpannerKind` a `Neume` variant, so a `match` over either needs
  arms for them.
- `StreamElement` has four more variants, `Barline`, `PedalObject`,
  `RehearsalMark` and `MetricModulation`, and `SpannerKind` two more,
  `Glissando` and `TremoloSpanner`, so a `match` over either needs arms for
  them.
- `from_musicxml` refuses what music21 raises on and it used to pass over
  or bend: a `<glissando>` or `<slide>` whose `line-type` is none of
  `solid`, `dashed`, `dotted` and `wavy`, a `<lyric>` whose `justify` is
  none of `left`, `center`, `right` and `full`, a `<rehearsal>` enclosed in
  nothing music21 names, and a `<tremolo>` of more than eight strokes,
  which was read as eight.
- `musicxml::ExportOptions` has a new public field, `make_notation`. code
  building one with every field named needs it; code using
  `..ExportOptions::default()` does not.
- `StreamElement` has a `Layout` variant, a page, system, staff or score
  layout, and a `TextBox` variant, so a `match` over it needs arms for them.
- `TimespanTree::maximum_overlap` returns an `Option`, nothing for an empty
  tree, as music21's `maximumOverlap` returns `None`.
- `StreamElement` has a `Variant` variant, another reading of the passage
  it stands at, so a `match` over it needs an arm for it.
- `DataSet::class_position_labels` takes `include_class_label` after
  `include_id`, as music21's `getClassPositionLabels` does, and leaves the
  class's column out without it.

## added

- `features` is music21's `features` package, begun: a `DataInstance`
  prepares a piece once -- ties stripped, each part and voice on its own --
  and works out its pitch, interval and contour histograms, and when each
  note sounds in seconds at the piece's tempi (`seconds_map`,
  `metronome_mark_boundaries`), as they are asked for.
  `features::jsymbolic::JSYMBOLIC` holds all 72 extractors music21
  implements from jSymbolic -- melodic, instrumental, rhythmic, texture and
  pitch --, each answering a `Feature`
  vector, and `features::native::NATIVE` the 19 music21 adds of its own:
  the key's mode and tonal certainty, note lengths, the chords of the piece
  chordified, chord-symbol bass motion and the Landini cadence. music21's
  language feature is not ported. each value is a `features::Value`, a
  whole number or a float as music21's is. every one gives music21's
  values on every MusicXML score of music21's corpus. a `features::DataSet`
  reads features of many pieces into one table, each row labelled with its
  piece's class, and writes it as tab-separated, CSV or ARFF text exactly
  as music21's `DataSet` does; `extractors_by_id`, `index_of`,
  `vector_by_id` and `all_features_as_list` are music21's helpers of those
  names.
- `text` is music21's `text` module: `assemble_lyrics` and
  `assemble_all_lyrics` join a stream's lyrics into words and lines, and
  `prepend_article` and `postpend_article` move a title's article. with it
  `Metadata::string_value` reads any attribute as music21's does --
  contributors summarised, titles with their articles put back in front --
  and `Metadata::title` the title. every MusicXML score of music21's corpus
  gives music21's lyrics and metadata text.
- `lily::to_lilypond` writes a stream as LilyPond, as music21's
  `LilypondConverter` writes it with `textFromMusic21Object`: parts on
  staves, voices, measures and their barlines, pickups, clefs, keys,
  meters, tempi, line and page breaks, notes, rests and chords with their
  beams, stems, ties, fermatas, tuplets and colours, and lyrics.
  `LilyOptions::version` is the LilyPond version the file says it is for,
  which music21 asks of the LilyPond installed. of the 654 MusicXML scores
  in music21's corpus, 602 come out the same text as music21's, space for
  space, and the crate refuses the other 52 as music21 does: notes of no
  length, such as chord symbols and grace chords, and lengths no note value
  writes. running it does not need LilyPond.
- `search::LyricSearcher` is music21's `LyricSearcher`: a stream's lyrics
  read as text, verse by verse, and searched for a string, each match
  giving the lyrics, notes and measures it spans. every MusicXML score of
  music21's corpus indexes and searches as music21's does.
- the rest of `search` is music21's `search.base`: `StreamSearcher` finds
  runs of elements by a list of terms and algorithms -- wildcards, rhythm,
  note names, or one's own -- and `rhythmic_search`, `note_name_search`
  and `note_name_rhythmic_search` do the same for the common cases. the
  `translate_*` functions turn notes and rests into text as music21's do,
  and the approximate searches rank streams by how alike that text is,
  with Python's `difflib` ratio; `most_common_measure_rhythms` groups
  measures by rhythm. every MusicXML score of music21's corpus searches,
  translates and ranks as music21's does.
- `search::index_score_parts` cuts each part of a score into overlapping
  stretches of its translated notes, as music21's `search.segment` does,
  and `score_similarity` compares every stretch of some scores with every
  stretch of the scores after it by `difflib`'s ratio. every MusicXML
  score of music21's corpus segments and compares as music21's does.
- `analysis::correlate::pitch_to_dynamic` pairs each pitch of a stream's
  notes and chords with each dynamic it sounds under, as music21's
  `ActivityMatch.pitchToDynamic` does, and `pitch_to_dynamic_counts`
  counts the pairs. every MusicXML score of music21's corpus pairs as
  music21's does.
- `search::ContiguousSegmentSearcher` finds every run of a stream's notes
  and chords holding a number of pitch classes, reading repetitions in each
  of music21's four ways, and `search::SegmentMatcher` finds the runs that
  are rows or sets of pitch classes searched for -- exactly, transposed,
  transformed, or as multisets, transposed or inverted -- and labels them
  in a copy of the stream with a line and a lyric, as music21's
  `search.serial` does.
- `Stream::template` is music21's `template`: the stream and every stream
  in it without its notes, chords, rests, dynamics and expressions, a rest
  standing for each stretch of them unless asked not to, with
  `TemplateOptions` choosing what goes, whether voices stay and what is
  kept whatever else says. `Stream::insert_into_note_or_chord` adds a note
  or chord into what starts within its length, as music21's
  `insertIntoNoteOrChord` does, and `Stream::flatten_unnecessary_voices`
  takes out empty voices and the voice left alone, as music21's
  `flattenUnnecessaryVoices` does. every MusicXML score of music21's corpus
  gives music21's templates, merged notes and flattened measures.
- `analysis::reduction` is music21's `analysis.reduction`: a lyric starting
  `::` marks its note for a reduction -- its pitch from a chord, octave,
  notehead fill, stem, group, voice and texts above and below, read as a
  `ReductiveNote` -- and `ScoreReduction` draws the marked notes of a score,
  and of a chord reduction, into a part for each group, their voices filled
  with hidden rests, above the score's parts with the marks taken out of
  their lyrics. `PartReduction` answers when, and how loudly, each part or
  group of parts plays, by measure or by run of notes, weighed by its
  dynamics, as music21's `PartReduction` does. `Duration::set_dots` is
  music21's `dots` setter. every MusicXML score of music21's corpus, its
  notes marked, is reduced and weighed as music21 reduces and weighs it.
- `braille` begins music21's `braille` package: `braille::lookup` holds
  the signs of braille music, and `braille::basic` writes one element at a
  time -- notes with their slurs, tuplets, articulations, accidentals,
  octaves, fingerings, fermatas and ties, chords as intervals, rests,
  clefs, key and time signatures, barlines, dynamics, text expressions,
  tempo texts, metronome marks, instruments, headings, words and numbers --
  each with the English music21 writes beside it, and converts braille to
  braille ASCII and dots and back. every element of every MusicXML score of
  music21's corpus is written as music21 writes it.
- `braille::translate` writes whole scores, parts and measures in braille
  as music21's `objectToBraille` does: `braille::segment` cuts a part into
  segments of groupings -- signatures and tempo for the heading, notes,
  voices in accord, long texts -- with slurs, beamed groups, repeated
  measures, doubled articulations and groupings split to fit a line, a
  piano's two staves side by side, and `braille::text` lays them out in
  lines with headings and measure numbers. `BrailleOptions` holds
  music21's keyword arguments, and its `debug` gives music21's English
  listing of each segment instead. every MusicXML score of music21's corpus
  is written as music21 writes it, in braille and in English.
- `alpha` begins music21's experimental `alpha` package:
  `alpha::analysis::hasher::Hasher` hashes each note, rest and chord of a
  stream by what music21's `Hasher` is set to hash -- pitch, MIDI number,
  name with and without octave, duration and offset (rounded or not), the
  interval from the note before, accidentals, ties, chords as notes or as
  chords with their normal order and prime form -- and
  `alpha::analysis::aligner::StreamAligner` aligns two streams by those
  hashes, with music21's edit distance, the insertions, deletions and
  substitutions it takes, and how alike the streams are. every MusicXML
  score of music21's corpus is hashed and aligned as music21 hashes and
  aligns it; the interval from the note before is compared on the scores
  of up to 600 notes and rests with no part split into staves, where
  music21 answers in good time and the same however the score was read.
- `layout` holds music21's layout objects: a `ScoreLayout` with the scaling
  and the page, system and staff layouts a score starts with, and the
  `PageLayout`, `SystemLayout` and `StaffLayout` a measure changes them
  with or starts a new page or system by, each margin and distance kept as
  the whole or fractional number of tenths the file gave. `from_musicxml`
  reads them from `<defaults>`, `<print>` and `<staff-details>`, and
  `to_musicxml` writes them back as music21 does, with a measure's
  `<measure-numbering>` (`Stream::measure_numbering`). every MusicXML score
  of music21's corpus music21 can write is read and written with its
  layouts as music21 reads and writes it.
- `text::TextBox` is music21's `TextBox`, text standing anywhere on a page,
  drawn as a `style::TextStyle` says: where it stands, its fonts, size,
  weight and colour, and how it is aligned and justified, each value an
  integer, a decimal or text as music21's `numToIntOrFloat` reads it
  (`style::StyleValue`). `from_musicxml` reads each `<credit>` into one
  and `to_musicxml` writes it back as music21 does; a style music21 does
  not name is refused, as music21 refuses it.
- `alpha::analysis` is music21's whole `alpha.analysis` package:
  `ornament_recognizer` answers the trill or turn a run of notes plays, from
  the note it is written on or alone, with its note length, nachschlag and
  accidental; `search::find_consecutive_scale` finds the runs of notes going
  one way through a scale, by name, name and octave, pitch class or letter;
  and `fixer` corrects a score read by optical music recognition by a
  performance of it aligned to it, respelling what it misspelled, taking out
  the measures it read wrong, and writing the trills and turns played. each
  answers as music21's does on thousands of runs, searches and readings
  built for it. `Stream::remove_event` is music21's `remove`.
- `omr` is music21's `omr` package. `omr::correctors::ScoreCorrector`
  flags the measures of a score read by optical music recognition whose
  notes do not fill their bar, and puts in their place the rhythm most
  likely meant -- from a measure of the same part a likely distance away,
  or from the measure at the same place in a part that tends to move with
  it -- keeping their pitches; `measure_hash` writes a measure's rhythm as
  the text both models compare. `omr::evaluators` measures a corrected
  score against its ground truth (`evaluate_correcting_model`,
  `OmrGroundTruthPair`) and counts the measures whose rhythm comes again
  (`auto_correlation_best_measure`). every MusicXML score of music21's
  corpus is flagged, corrected and counted as music21 does it, measure for
  measure, and music21's own Mozart K. 525 pair evaluates the same.
  `Error::Omr` is what it refuses with.
- `variant` is music21's `variant` module. a `Variant` holds another
  reading of a passage beside it, in named groups, standing in for as much
  of the stream as its replacement length says. `merge_variants` merges
  another version of a score, a stream of measures or a stretch of notes
  into it as variants where they differ (`merge_variant_scores`,
  `merge_variant_measure_streams`, `merge_variants_equal_duration`), and
  `merge_part_as_ossia` an ossia part; `add_variant`, `replaced_elements`,
  `refine_variant`, `make_all_variants_replacements` and
  `make_variant_blocks` are music21's functions of those names, and
  `Stream::activate_variants` and `Stream::show_variant_as_ossialike_part`
  music21's methods. they keep music21's slips: measures are found by
  number where music21 finds them so, `refine_variant` works in place only,
  as music21's does, and `make_variant_blocks` fails where music21's does,
  after moving the first variant. on every MusicXML score of music21's
  corpus, merged with a version with a measure transposed, one taken out
  and one played twice, every function leaves the same measures, notes and
  variants as music21's, offsets and lengths bit for bit.
- `Rest::full_measure` is music21's `fullMeasure`, which the MusicXML reader
  sets where music21's does.
- `ChordSymbol` carries lyrics, as music21's does, with `lyrics`,
  `lyrics_mut` and `add_lyric`.
- `tablature` is music21's `tablature`: `FretNote`s on a `FretBoard` --
  bare, or tuned as a guitar, ukulele, bass guitar or mandolin -- ordered
  lowest string first and sounded as pitches, with `FirstFret` and
  `ChordWithFretBoard`.
- `tree` begins music21's `tree` package: `as_timespans` reads every
  element of a stream, flat or semi-flat, as an `ElementTimespan`, and a
  `TimespanTree` answers what starts, stops or sounds across an offset and
  walks the `Verticality` at each offset something starts, with its
  pitches, pitch classes, bass and the time to the next. every MusicXML
  score of music21's corpus is read as music21 reads it.
  Each `Verticality` pairs its parts' motion and gathers voice-leading
  quartets, music21's `getPairedMotion` and `getAllVoiceLeadingQuartets`;
  the tree walks verticalities n at a time, `unwrap`s them into each
  part's `Horizontality` with its passing and neighbour tones, finds the
  runs between consonances and splits at offsets. `Verticality::make_element`
  makes the chord or rest sounding there, as music21's `makeElement` does
  for chordify.
- `analysis::windowed::WindowedAnalysis` cuts a stream into windows of a
  quarter note and runs any analysis over windows of each size, overlapping,
  side by side or averaged, as music21's `WindowedAnalysis` does; and
  `analysis::floating_key::KeyAnalyzer` reads a key for every measure and
  smooths each by its neighbours, as music21's `floatingKey` does. every
  MusicXML score of music21's corpus gives music21's windows and keys.
- `Stream::find_consecutive_notes` and `melodic_intervals` find the notes
  of a stream that follow one another and the intervals between them, as
  music21's `findConsecutiveNotes` and `melodicIntervals` do, with its
  options in `ConsecutiveOptions`. four of music21's small analyses are
  built on them: `analysis::patel` (the rhythm's normalized pairwise
  variability and the melodic interval variability), `analysis::metrical`
  (the depth of the metrical hierarchy at each note, which
  `label_beat_depth` marks with stars as music21 does, and Thomassen's
  melodic accent), `analysis::segment_by_rests` and
  `analysis::pitch_analysis`. every MusicXML score of music21's corpus
  gives music21's answers. the wheel's `Stream` has `findConsecutiveNotes`
  and `melodicIntervals` too, answering with the objects it holds.
- `Stream::chordify` and `chordify_with` make a score's parts one line of
  chords, as music21's `chordify` does: laid out as the first part is, with
  a chord of every pitch sounding wherever any part starts or stops a note,
  a held note tied across, a pitch two parts share one note, the notes'
  articulations and expressions gathered onto the chord, and rests standing
  together joined. `ChordifyOptions` carries music21's `addTies`,
  `removeRedundantPitches` and `toSoundingPitch`. every MusicXML score of
  music21's corpus chordifies as music21 chordifies it, byte for byte as
  MusicXML where music21 can write the result. the wheel's `Stream` has
  `chordify` too.
- `romantext::to_roman_text` writes a score's roman numerals as a RomanText
  analysis, as music21's `romanText.writeRoman.RnWriter` does: the header,
  `Time Signature:` lines, and each measure's numerals at their beats with
  the key where it changes, repeats included. every analysis of music21's
  corpus, read and written on both sides, comes out the same text. a chord
  now says which roman numeral it stands for (`Chord::numeral`,
  `set_numeral`, `with_numeral`), as music21's `RomanNumeral` is a chord;
  `from_roman_text` gives each chord its numeral, and transposing a chord
  moves its numeral's key. the wheel has `to_roman_text`, and its
  `from_roman_text` hands back `RomanNumeral`s rather than plain `Chord`s.
- `capella::from_capella` reads Capella's CapXML, the `score.xml` a `.capx`
  holds, as music21's `capella.fromCapellaXML` does: system by system, each
  staff's share put end to end into a part, clefs and key signatures that
  restate the one before dropped, and each part cut into measures. clefs,
  keys, meters, notes and chords with their accidentals, ties, tuplets and
  lyrics, rests and barlines are read. held to music21 byte for byte as
  MusicXML on the `.capx` music21 carries and documents written for the
  test. the wheel has `from_capella`, which unpacks a `.capx` itself.
- `musedata::from_musedata` reads a MuseData work, its files' texts in
  order, as music21's `musedata` and `musedata.translate` do, in both of the
  encoding's stages: a part for each part, measures with their barlines and
  repeats, notes, chords and rests, ties, voices, beams, the accidentals the
  encoding shows, articulations, fermatas, ornaments, dynamics, lyrics, the
  clef, key, meter and tempo word, a transposing part turned to the pitch
  it sounds, and the work's title, number and movement. held to music21 byte
  for byte as MusicXML on the work music21 carries, read whole and file by
  file, and on parts written for the test in both stages. the wheel has
  `from_musedata` too.
- `noteworthy::from_nwc` reads NoteWorthy Composer's binary `.nwc`, as
  music21's `noteworthy.binaryTranslate` does: each staff and object written
  out as `.nwctxt` lines (`noteworthy::nwc_lines`, music21's
  `dumpToNWCText`) and those read by the text reader. a compressed file
  must be inflated first, as the crate unpacks nothing; the wheel's
  `from_nwc` inflates one itself. held to music21 on its four `.nwc` files,
  line for line and as MusicXML.
- `noteworthy::from_noteworthy` reads NoteWorthy Composer's text format,
  `.nwctxt`, as music21's `noteworthy.translate` does: a part per staff,
  notes, chords and rests spelled from their places under the clef, the
  key and the accidentals written before them, slurs, ties, triplets, grace
  notes, lyrics, barlines, repeats and endings, clefs, keys, meters, tempos,
  dynamics, words, *Coda* and *Segno* marks, instruments and the title.
  Every file music21 carries beside its reader is read into the score
  music21 makes, held to it byte for byte as MusicXML. the wheel has
  `from_noteworthy` too.
- `volpiano::from_volpiano` and `volpiano::to_volpiano` read and write
  Volpiano, the notation of the chant font the CANTUS database is written
  in, as music21's `volpiano.toPart` and `fromStream` do: clefs, notes
  liquescent or not, flats and naturals on B and E, barlines, neumes and
  line, page and column breaks. held to music21 on music21's own examples
  and 300 generated strings, read both ways and written back, and on every
  MusicXML score of its corpus written as Volpiano. the wheel has
  `from_volpiano` and `to_volpiano` too.
- `Stream::to_sounding_pitch` and `Stream::to_written_pitch` move the notes
  under an octave line, as music21's `Ottava.performTransposition` and
  `undoTransposition` do, and turn the line's `OctaveShift::transposing`
  to say where they now are. `to_written_pitch_by` takes music21's
  `ottavasToSounding`. `to_musicxml` writes the notes under a transposing
  line at the pitch they sound, as music21's writer does.
  `OctaveShift::interval` and `set_transposing` are new.
- the wheel has `to_abc`, writing one of its streams or one of music21's as
  ABC, which music21 has no writer for, with `unit_length` and
  `measures_per_line` as the crate's `abc::ExportOptions` has them.
- `midi::to_midi` and `midi::ExportOptions` write a score as a standard MIDI
  file, as music21's `streamToMidiFile(...).writestr()` writes it, byte for
  byte: a conductor track with the tempos, meters and keys, a track per part
  named after its instrument, a channel per program, tied notes sounding
  once, each note's velocity from the dynamic in force and its
  articulations, lyrics, chord symbols sounding, strokes on the drum
  channel, and a pitch between the keys bent on a channel of its own, its
  repeats played out first. the wheel has `to_midi` too, taking music21's
  streams as readily as its own and handing back `bytes`.
- `Stream::expand_repeats` plays a score's or a part's repeats out, as
  music21's `expandRepeats` does: passages between repeat barlines written
  out as often as they say, first and second endings in turn, and a *da
  capo* or *dal segno* taken to its *fine* or through its *coda*, numbered as
  music21 numbers them and laid out where music21 lays them. repeats music21
  finds badly formed are refused.
- `Stream::strip_ties` folds each run of tied notes into its first, as
  music21's `stripTies` does.
- `Error::Stream`, music21's `StreamException`.
- `abc::to_abc` and `abc::ExportOptions` write a score, a part or an opus as
  ABC: the header (title, composer, origin, meter, unit length, tempo, key
  and mode), a voice for each part and for each voice a measure holds,
  measures with their barlines, repeats and endings, notes, chords and rests
  with their lengths, accidentals and octaves, ties, tuplets, grace notes
  with their decorations, slurs and hairpins, chord symbols, lyrics,
  dynamics, articulations, ornaments, fermatas and text, and changes of
  meter, key and tempo. a note music21 cut at a barline when reading is
  written whole, so a tune reads back into the measures it was read into.
  What ABC cannot say is refused with `Error::Abc`: a microtone, a tuplet
  inside a tuplet, unpitched strokes, pedals, octave lines, glissandi,
  tremolos between notes, rehearsal marks and metric modulations.
- the wheel reads every format the crate reads: `from_abc(text)`,
  `from_abc_number(text, number)`, `from_midi(data)` taking the file's
  `bytes`, `from_tiny_notation(text)`, `from_humdrum(text)`,
  `from_mei(text)` and `from_roman_text(text)`, beside `from_musicxml`. each
  hands back the wheel's own streams: a `Score`, an `Opus` of scores for an
  ABC, Humdrum or RomanText file holding several, and a `Part` for
  TinyNotation. a RomanText score's chords carry their numerals as lyrics.
  What the wheel has no class for -- unpitched strokes, words, barlines,
  spanners, metadata -- is read and left out. a reader that cannot read its
  input raises `StreamException`, which the wheel now exports.
- the web score editor opens MIDI (`.mid`, `.midi`), Humdrum (`.krn`), MEI
  (`.mei`), RomanText (`.rntxt`, each numeral written under its chord) and
  TinyNotation (`.tntxt`) through the crate's readers, by the same
  conversion its MusicXML import uses. a MIDI file now keeps its meters,
  triplets, voices, tracks and instruments; the notes of a track that also
  plays the drums are kept and its strokes left out.
- a figured bass is written out as a score, as music21's realizer writes
  one. `FiguredBassLine::generate_bass_line` is the bass with its figures
  under it as lyrics, in measures, and a `Realization` writes a way of
  voicing the line with `generate_realization_from_possibility_progression`
  -- in keyboard style, the upper parts as chords over the bass, or with
  `set_keyboard_style_output(false)` in chorale style, a staff for each
  part -- every way with `generate_all_realizations`, and ways the caller
  chooses with `generate_random_realization` and
  `generate_random_realizations`. `FiguredBassLine::overlay_part` lays a
  written part over the line for every realization to keep, and
  `in_key_and_time` gives a line its meter. nine lines are written by both,
  112 scores in all, and each comes out the same MusicXML byte for byte.
- what instruments do to a stream they stand in, music21's four functions:
  `instrument::unbundle_instruments` puts the instrument each note keeps as
  its own into the stream beside it, `bundle_instruments` takes them back,
  `deduplicate` folds together the instruments a part says more than once,
  and `partition_by_instrument` splits a score into a part for each
  instrument playing in it. fifteen streams built both as music21's and as
  the crate's come out the same from each.
- `volume::dynamic_context` is `Volume.getDynamicContext`: the dynamic a
  note is read against, searched for outwards from the stream holding the
  note as music21's `getContextByClass` searches. 5,167 notes of three
  corpus scores are given the dynamic music21 gives them.
- `Note::grace_note`, `Chord::grace_note` and `Rest::grace_note` are
  `getGrace`: the same note written as a grace note or an appoggiatura.
- `Beams::naive_beams` is `Beams.naiveBeams`, over the elements of a stream,
  and `Beams::naive` its rule for one element.
- `Ornament::split_client` is `Trill.splitClient`: it carries a trill onto
  the first of the pieces a trilled note was split into and answers the
  trill extension joining them. `Expression::tie_attach` says which piece
  of a split note keeps any expression.
- `musescore::from_mscx` reads MuseScore's own uncompressed `.mscx` format,
  as MuseScore 4 saves it and MuseScore 3 where the two agree, into a score.
  It needs no feature and no MuseScore installed, and builds for wasm. it
  takes the document's text: a `.mscz` is a zip archive holding a `.mscx`,
  and unzipping it is the caller's. it reads parts and staves, measures and
  their numbers, voices, notes as the file spells them, chords, rests, ties,
  dots, tuplets, grace notes, clefs, keys, meters, tempo marks, dynamics,
  text, lyrics, chord symbols, articulations, fingerings, fermatas,
  ornaments, arpeggios, barlines, repeats, endings, segno and coda signs and
  the jumps to them, slurs, hairpins, pedal lines, instruments and the
  file's metadata. beams, tuplet brackets and the clef and key a staff
  starts with are worked out as MuseScore works them out where the file
  leaves them to its layout. percussion and tablature staves, files older
  than MuseScore 3's, tremolos, trill, octave, glissando and text lines,
  measure repeats, fret diagrams and figured bass are refused with
  `Error::MuseScore` rather than dropped.
- the reader is held to MuseScore itself, music21 having no reader of the
  format: `mscx_parity` reads sixteen `.mscx` files and the MusicXML
  MuseScore 4.7.5 exported from each, writes both with `to_musicxml`, and
  compares the texts. thirteen agree; the three that differ are places
  where MuseScore's export says something other than its own file does, and
  the reader follows the file. the test lists each with its reason.
- MusicXML, written and read as music21 writes and reads it, for what the
  writer refused and the reader passed over:
  - glissandi and slides: `SpannerKind::Glissando`, `Spanner::glissando`,
    with `Glissando` saying how it is played (`SlideType`) and the words
    along it. One played `Continuous` is written `<slide>`.
  - tremolos between notes: `SpannerKind::TremoloSpanner`,
    `Spanner::tremolo`, `number_of_marks`.
  - a pedal drawn as a sign and a line (`PedalForm::SymbolLine`), which
    `to_musicxml` refused: its line resumes straight after the sign, at
    `Spanner::offset`, where the pedal mark itself stands.
  - the bounces and gaps inside a held pedal: `PedalObject` and
    `PedalObjectKind`, standing in the stream as
    `StreamElement::PedalObject` and joined by their pedal mark.
  - a barline inside a measure, `StreamElement::Barline`, which music21
    keeps where it appends it and writes nothing for.
  - how a lyric is drawn: `Lyric::justify` (`Justification`), `placement`
    and `is_hidden`.
  - rehearsal marks: `expressions::RehearsalMark`, with its placement and
    what is drawn round it, as `StreamElement::RehearsalMark`.
  - a time signature left unprinted: `TimeSignature::is_hidden`.
  - a metric modulation, which `from_musicxml` refused:
    `StreamElement::MetricModulation`, with `MetricModulation::placement`.
    A file gives its two note values and no numbers; the writer gives each
    side its number from the tempo in force before it in the part, as
    music21's exporter does, and writes the speed the new side is played at.
- the wheel's `to_musicxml` reads music21's own `Glissando`,
  `TremoloSpanner`, `PedalBounce`, `PedalGapStart`, `PedalGapEnd`,
  `RehearsalMark` and `MetricModulation` and a barline inside a measure,
  where it refused a pedal object, a rehearsal mark and a metric modulation
  and left the rest out; a wheel `Lyric`'s `style` carries the
  justification, placement and `hideObjectOnPrint` the crate read.
- `musicxml::ExportOptions::make_notation` writes a score as music21's
  exporter writes one by default, with `makeNotation=True`: the notation the
  score leaves unsaid is worked out before it is written. gaps are filled
  with rests that are not printed, a part of loose notes is cut into
  measures and its overlapping notes put in voices, notes running past a
  barline are cut and tied, accidentals are decided, tuplets a cut left
  unfinished are completed, notes are beamed and tuplets bracketed where the
  part has none yet, and lengths no single note value writes are cut into
  tied values. `to_musicxml` then takes a part or a plain stream as readily
  as a score, and the score handed in is not changed. off, which is the
  default, the score is written as it stands, as before.
  It is held to music21 byte for byte on 244 subjects, each read by the
  crate's own reader and written by both: 15 TinyNotation lines and the same
  15 as loose notes, 9 parts built a note at a time, 29 MIDI files, 134 ABC
  tunes — every one of the 118 in music21's corpus that it reads into no
  measures, which could not be compared before — and 42 corpus scores kept
  as MusicXML. beyond those, 600 generated TinyNotation subjects, every
  fortieth ABC tune of the corpus, 215 further corpus scores and every
  corpus score tried as MIDI that music21 can write agree.
- `makenotation::make_notation` is music21's `makeNotation` on a stream:
  the same steps for a caller that wants the notated score rather than its
  MusicXML. `makenotation::make_tuplet_brackets` and
  `makenotation::split_at_durations` are music21's `makeTupletBrackets` and
  `splitAtDurations`.
- `Duration::expression_is_inferred` and `set_expression_is_inferred`:
  music21's `expressionIsInferred`. a duration made from a length may be
  written another way when notation is made; one made from a note value is
  left as written.
- `Stream::padding_left` and `padding_right`: music21's `paddingLeft` and
  `paddingRight`, how much of its bar a pickup or a measure cut short leaves
  unfilled. the MusicXML and ABC readers set them as music21's do.
- `TimeSignature::is_restated`: whether a meter is an earlier measure's
  standing again, as music21's ABC reader leaves one object in two measures.
- the wheel's `to_musicxml` takes `make_notation=True`.
- `to_musicxml` with `make_notation` writes a measure or a voice standing
  alone, as music21's `fromMeasure` and `fromVoice` do: the measure is
  filled out to its bar, given a meter and a clef its notes fit where it
  states none, its accidentals decided and its notes beamed; a voice is
  written as measure one.
- `Stream::at_sounding_pitch` and `set_at_sounding_pitch` say whether a
  stream's notes are written at the pitch they sound, at the pitch a
  transposing instrument reads, or neither has been said: music21's
  `atSoundingPitch`. `Stream::to_written_pitch` and `to_sounding_pitch`
  transpose each part by its instruments' transpositions, each instrument
  moving the notes, chords, chord symbols and keys from where it stands to
  where the next one does, as music21's `toWrittenPitch` and
  `toSoundingPitch` do. the wheel reads and sets `atSoundingPitch`.

## changed

- the crate is held to music21 11.0.0b11 with the fixes sent upstream from
  here, and stops copying the music21 bugs those fix:
  - `capella::from_capella` reads a `single` barline as a regular one, where
    it refused it, and reads a tuplet's `prolong`.
  - `noteworthy::from_nwc` reads tenor and percussion clefs, the notes of a
    chord in a file of version 2 or later and of version 1.70 or earlier,
    and a rest chord as its rest and the notes sounding with it.
    `from_noteworthy` reads a chord's `Dur2` and `Pos2` as notes sounding
    with the chord, where it took them for a rest, and a `RestChord` as a
    rest and a chord starting together, each in a voice of its own.
  - `volpiano::from_volpiano` puts a break in the measure being read, not the
    first measure, and makes a neume of every note of its run, not just the
    last pair.
  - `tinynotation::from_tiny_notation` counts only notes and rests towards a
    tie or a bracket, and a `}` closes the last bracket opened rather than a
    tie.
  - `abc::from_abc` keeps a measure's overflow that no meter fits, where it
    lost it.
  - `instrument::deduplicate`, and the MIDI reader with it, settle only the
    instruments standing at one offset together, so a program change later
    in a part is kept.
  - a roman numeral past seven, `VIII`, is refused, where it read as `VII`.
  - a trill takes its accidental's showing only where that is decided.
  - `to_musicxml` writes a chord's notes lowest first on the staff, its
    fingerings moving with their notes, and a chord whose fingered notes
    would not stay first as it stands; it joins a piano's staves where the
    upper lacks a measure the lower has, and the reader keys an arpeggio
    across chords by its number and its offset in the part.
  - `from_musicxml` hides a lyric only where its `print-object` is `no`,
    where it hid one marked `yes`.
  - `Stream::find_consecutive_notes` with `skip_unisons` passes over a chord
    sounding the pitches of the chord before, in any order, and what it
    passes over still sounds until it ends, so no break follows it.
  - `KeyAnalyzer::smooth_interpretation_by_measure` smooths each measure by
    its neighbours' own readings, where a measure read after its neighbour
    was smoothed took the smoothed reading.
  - jSymbolic's independent voices (`T1` to `T3`) count a part sounding a
    chord among the parts sounding, where only single notes counted.
    Native `CS12` passes over a chord symbol that sounds nothing, where it
    refused the piece, and `P22` analyses a piece whose stated keys agree
    in a mode other than major or minor, where it refused it.
  - `PartReduction::weighted_spans` puts a part in a group where its id
    holds one of the group's matches as a whole word, weighs a stretch by
    the mean loudness of its dynamics, where it divided their sum by seven,
    keeps each group's stretches apart where groups of one id shared them,
    keeps the stretch before a stretch's first dynamic, as loud as the last
    dynamic before it, and leaves a dynamic on a stretch's end to the next.
  - `ReductiveNote::note_and_text_expression` gives a chord's highest note
    where no pitch is said, where it looked for a C.
  - `Sieve` reads a modulus of nought as the empty class, and its
    complement as every integer, where it refused it.
  - `StepScale::octave_repeating` respells the last step of steps a whole
    octave wide spelled as something else, so `M3 M3 M3` closes on the
    tonic as `M3 M3 d4` rather than on `B#`.
  - the Python package's `Accidental` reads a value as music21's lookup
    table does -- a name, modifier, alternate name or alter, a string tried
    again lowercased -- refusing `''` and `None`. With
    `allowNonStandardValue`, `set` sets the name or the alter alone, and a
    standard `set` tells its `_client`. `Pitch(step=...)` refuses what is
    not one letter of the scale, and is passed over where a name is given.
- `abc::to_abc` says each voice's opening clef on its `V:` line, where it
  used to name it in the `K:` field, and says every clef but a treble clef
  the notes fit, where it used to leave unsaid any clef the notes fit. a
  renderer drawing the tune, whose default is treble, now draws a bass part
  in the bass clef. a tune of one voice names its part on a `V:1` line, and
  a part opening with an instrument that has a General MIDI program says so
  with `%%MIDI program`.
- the score editor writes an imported score as ABC with `abc::to_abc`, its
  own ABC writer gone: what it reads is turned into the crate's score and
  written by the crate.
- `to_musicxml` writes a part at written pitch, as music21's exporter
  does: a stream holding anything at sounding pitch is turned to written
  pitch before it is written. `from_musicxml` says a part is at sounding
  pitch unless the file gave it a `<transpose>`, and `from_mei` says its
  parts are at written pitch, as music21's readers do, so a part whose
  instrument transposes is written as music21 writes it. with
  `make_notation` the part is turned to written pitch after the score's
  notation is made, and its accidentals are decided again at written pitch.
- the score editor's own MIDI reader, `midi_to_abc`, is gone: MIDI is read
  by the crate's `from_midi` like the other formats.
- the `musescore` module is always there, since reading a `.mscx` runs no
  program. `musescore::MuseScore`, which finds and runs an installed
  MuseScore, is still behind the `musescore` feature and still left out of
  a wasm build.
- `makenotation::make_rests` fills a measure only as far as its padding
  leaves, and takes a measure no meter has been stated for as long as the
  meter standing in the part before it, or failing that as the meter that
  fits what it holds, where it took `4/4`.
- `makenotation::make_accidentals` treats a chord symbol as the chord it is
  in music21: the notes it stands for count among those already heard, and
  a natural one of them is given is written on the symbol's root.
- `makenotation::make_beams` starts a measure padded on the left where its
  padding says.
- `Duration::from_type` and the named values (`Duration::quarter()` and the
  rest) say how they are written, so `expression_is_inferred` is false for
  them, as it is for music21's `Duration('quarter')`.
- the places `Scale::places_of` counts are numbered by the degree each
  stands on, lowest first, and `next_pitch_below` and `next_pitch_above`
  read a note the scale stands twice as the first of them: coming down from
  Rag Marwa's `D-`, its second degree, the next note is `C`, where it was
  `B`, as from the seventh.
- the wheel answers the same on every run where music21 picks at random.
  A pitch a scale stands on more than one degree is the lowest of them, in
  `getScaleDegreeFromPitch` and in where `nextPitch` steps from: rag
  Marwa's `A` is its fifth degree, not its fifth or its seventh by chance.
  `partIdRandomize` and `instrumentIdRandomize` count their ids up from one
  through the process, in the same `P` or `I` and thirty-two hex digits,
  where they drew them at random.

## fixed

- `from_musicxml` puts each measure at the offset music21 does, the sum of
  the lengths before it snapped to a fraction: a measure after a run of
  triplet-length bars was a hair early.
- a scale steps from a pitch on one of its degrees however the pitch is
  spelled, as music21 finds the degree by pitch space: told to start below
  `E#4` in C major, `next_pitch_beside` stepped up from `E4` to `F4`, where
  music21 steps from `F4` to `G4`.
- a pitch built from a number keeps the natural music21 writes on it:
  `Pitch(60)` has a natural, where only setting `ps` or `midi` leaves
  none.
- transposing a pitch moves its fundamental with it, as music21 does: a
  diatonic interval dropped the fundamental, and one counted in semitones
  left it where it was.
- a negative harmonic shift is written with music21's ordinal suffix:
  `-2th`, as Python takes the last digit of -2 modulo ten to be eight.
- `Stream::flatten`, `recurse` and `leaves` snap each offset they add up
  as music21's `opFrac` does, so a note after three triplets stands on the
  beat rather than a hair before it, and lands in the measure it starts.
- the key a stream is analysed in counts chord symbols and percussion
  chords among its notes, as music21's does, so a stretch holding only
  chord symbols lasting nothing is read, at nought, rather than not at all.
- transposing a `Key` or a `KeySignature` keeps the colour it is drawn in,
  as music21's does; a score turned to sounding pitch lost it.
- `from_roman_text` records a pickup's `padding_left`, and the last
  measure's `padding_right`, as music21 does; it moved the pickup's chords
  to the start of the measure and said nothing of what was left out, so
  their beats were read a beat or more early.
- the figured-bass realizer's scores bracket and complete their tuplets and
  join tied tuplet notes back up, as music21's `makeNotation` does when the
  realizer writes a part. a line holding a triplet was written with its
  tuplets unbracketed.
- `abc::to_abc` no longer writes a bass clef that music21's reader, and the
  crate's, read as lowering every note two octaves: music21 takes a bass
  clef named in the header's `K:` field that way. it also gives every voice
  of a measure the measure's clef, key and meter, where only the first voice
  had them and the others were written with no meter.
- `midi::to_midi` plays a chord whose notes each carry a volume as music21
  does: each note's velocity realized under the dynamic the chord's context
  finds -- in its own staff, or in the other staves of the part it was read
  from -- and with the chord's articulations.
- `volume::dynamics_in_force` finds the dynamic music21's `realizeVolume`
  finds: offsets are held as music21 holds them, a binary fraction as a
  float and anything else as an exact fraction, so where music21 adds a
  float span to a fractional start and the span ends a hair past the next
  dynamic, the earlier dynamic is in force there, as it is in music21.
- a chord symbol of a kind music21 has no notation for, such as MusicXML's
  `minor-major`, sounds its root and its bass alone, as music21's does,
  rather than the chord the crate read its letters as.
- `TimeSignature::average_beat_strength` weighs the stream's own elements
  only, as music21's `averageBeatStrength` does, and with `notes_only` its
  notes and chords without its rests. it used to weigh everything inside
  the measures and voices the stream held, so `best_time_signature` gave a
  measure whose notes stand in a voice three-four where music21 gives
  six-eight.
- `from_musicxml` gives a hairpin, bracket, octave line or pedal started
  after the last note of a part the next part's first note, as music21
  does. the note was taken from the queue and then lost, leaving the
  spanner on the one note it ended on.
- the score editor read a flat as a natural wherever it spelled a note
  itself -- an imported score, a note dragged or moved by the arrow keys,
  the comping part -- and decided which accidentals its MusicXML export
  shows the same way, since pitch names came to write flats as `b`. a B flat
  in F major was written `=B`.
- a figured bass line is realized as it is written. a bass note running
  past a barline is a segment for each of the two notes it is written as,
  as it is in music21, where it used to be one; a line in a meter its notes
  cross the barlines of now has the voicings music21 finds for it.
- a note cut at a barline shares out its expressions as music21's
  `splitAtQuarterLength` does: a trill stays on the first piece, a fermata
  goes to the last, and an ornament goes where its `tie_attach` says. both
  pieces used to keep them all, so `make_ties` and the ABC reader's
  rebarring wrote a trill and a fermata twice.
- `from_musicxml` reads words naming a coda or a segno as music21 does: the
  mark is drawn as its sign, which has no side of the staff, where the
  words' placement was kept and written on the sign.
- a `MetronomeMark`'s number within a millionth of a whole number is that
  whole number, as music21 makes it: a dotted quarter at 110 counted in
  quarters is `165`, where `equivalent_by_referent` answered
  `165.00000000000003` and everything worked out from it carried the hair.
- `abc::from_abc` transposes a tune whose key says `-8va` by semitones, as
  music21 does, which spells each note afresh and leaves how its accidental
  is shown undecided. it kept each spelling and showing.
- `musicxml::from_musicxml` reads a Finale `<forward>` of no length as a
  quarter rest that is not printed, as music21 does.
- `musicxml::to_musicxml` passes over an unprinted rest whose length no note
  value writes, leaving a `<forward>`, where it refused the score.

# music21-rs 0.8.0

four more of music21's analysis modules are the crate's—
`analysis.enharmonics`, `analysis.harmonicFunction`,
`analysis.neoRiemannian` and `analysis.transposition`—and pass every
example: 40 modules, 1,028 docstrings and 9,174 examples. asking music21 and
the crate the same questions over thousands of random inputs found eleven
places where they answered differently; they agree now.

MusicXML, both ways. `musicxml::to_musicxml` writes a score as music21's
`ScoreExporter` writes it with `makeNotation=False`, and 41 scores from
music21's corpus come out byte for byte the same as music21's own export.
`musicxml::from_musicxml` reads a file as music21's reader reads it: the
same scores read by the crate and written back give the text music21 gives
after reading them itself.

ABC, MIDI and TinyNotation are read as music21 reads them. `abc::from_abc`
was run over all 12,978 tunes of music21's corpus beside music21's own
reader: 12,860 come out the same score, and the other 118 music21 reads into
no measures and cannot be compared. `midi::from_midi` reads every MIDI file
music21 carries, and corpus scores music21 wrote as MIDI, into the score
music21 reads. Humdrum, MEI and RomanText follow: `humdrum::from_humdrum`,
`mei::from_mei` and `romantext::from_roman_text`, each held to music21's
reader the same way, on scores from music21's corpus and its own tests.

## breaking changes

- Harte chord notation is the `harte` crate, a workspace member beside this
  one that depends on it, rather than the `harte` module: `harte::Harte`,
  `HarteInterval`, `SHORTHAND_DEGREES` and `convert_interval` moved there
  as they were, and it refuses a label with its own `harte::Error`, so
  `Error::Harte` is gone from this crate. it is harte-library's notation,
  not music21's, and music21's author asks for a small core with add-ons
  around it.
- flats are written `b`: `Pitch::name` is `Bb`, `name_with_octave`
  `Bb4`, a flat's `Accidental::modifier` `b` and a double flat's `bb`, and
  so for every name built from them: key, scale and chord-symbol names,
  `CHROMATIC_PITCH_CLASS_NAMES`. `-` is still read everywhere a name is.
  music21 writes `-`, and the wheel still does: it respells every name it
  hands Python. Cuthbert's "Music21's Mistakes" regrets borrowing `-`.
  A pitch name and octave no longer collide: B-flat 1 is `Bb1`, B in octave
  -1 `B-1`.
- every pitch has an accidental. `Pitch::accidental` answers
  `&Accidental`, a natural where none is written, so arithmetic on it
  needs no `None` case. what was `accidental` is `written_accidental`,
  and `accidental_mut` is `written_accidental_mut`: music21's view, where
  `D` has none and `Dn` a natural, which the wheel keeps.
  `set_accidental` takes an `Accidental`; `set_written_accidental` takes
  the `Option` music21's setter does. Cuthbert's "Music21's Mistakes"
  lists the natural-or-`None` split as more trouble than it is worth.
- `Polyrhythm::ratio_pitches` and `ratio_chord` sound the ratios in tune:
  each pitch carries a microtone for the part of its ratio past the nearest
  semitone, so a 4:5:6 rhythm on `C4` is `C4 E4(-14c) G4(+2c)`.
- `Polyrhythm::to_chord` and `to_polypitch` are gone. they mapped where each
  beat falls in the bar onto twelve semitones, which is no chord the rhythm
  implies; `ratio_chord` is the chord its ratios make.
- `Interval::pythagorean_ratio` returns an error instead of panicking on an
  interval too far along the chain of fifths for a ratio of two `i32`s, and
  answers for up to nineteen fifths where it stopped at eighteen.
- `Pitch::frequency_hz_in` sounds the degree nearest each key in a system
  without twelve degrees to the octave, where it read the pitch-space number
  as a degree: a4 in 19-EDO is 436.0 Hz, where it was degree 69, 101.3 Hz.
- `TuningSystem::label` names a degree by the pitch it sounds: `C-1` for
  `CN1`, `C♯/D♭4` for `C#/Db4`, `G0(+2c)` for Bohlen-Pierce's `T0O0`.
  `TWELVE_TONE_NAMES`, `TWELVE_TONE_NAMES_SHARP`, `TWELVE_TONE_NAMES_FLAT`
  and `WHOLE_TONE_NAMES` are gone.
- `tuningsystem::Fraction` is `tuningsystem::Ratio`, an enum of `Rational`
  and `Root` in place of a struct whose `base` of nought meant rational.
  `Ratio::root(base, numerator, denominator)` replaces
  `Fraction::new_with_base(numerator, denominator, base)`, `value` replaces
  `ratio`, and `to_string` replaces `label`. `TuningSystem::fraction` is
  `exact_ratio`, `ScalaDegree::as_fraction` is `as_ratio`, and
  `equal_temperament_12` and `equal_temperament_default` are gone for
  `equal_temperament`.

- `StreamElement` has eight more variants and `expressions::Expression` is
  new, so a `match` over either needs arms for them.
## added

- `abc::from_abc` and `abc::from_abc_number`, music21's `abcFormat` and its
  translator: a tune as a score, a file of tunes as an opus, voices as
  parts, keys and modes, meters, tempos, default lengths, broken rhythm,
  tuplets, grace notes, chords, ties, slurs, hairpins, bowings and accents,
  chord symbols, lyrics, repeats and endings, a pickup measure, and a
  measure holding more than its bar cut in two as `reBar` cuts it.
- `makenotation::make_beams` and `set_stem_direction_for_beam_groups`,
  music21's `makeBeams`: the notes of each measure beamed by the meter in
  force, and each beamed group's stems pointed one way.
- `musescore::MuseScore`, behind the `musescore` feature: finds an installed
  MuseScore and runs it to convert a file, so anything MuseScore opens
  (`.mscz`, `.mscx`, Guitar Pro, MIDI, Capella...) can be read as a score
  and a score written as anything it saves. it is the one part of the crate
  that touches files and starts a process, and is not built for wasm.
- `midi::from_midi`, music21's `midi.translate.midiFileToStream`: each
  track with notes a part, the tracks without the conductor whose meters,
  keys and tempos the parts take; notes gathered into chords, moved onto
  sixteenths and triplets, cut into measures, put in voices where they
  overlap, tied across barlines and filled out with rests; instruments from
  program changes and track names, lyrics, velocities, and channel 10 as
  unpitched percussion.
- `tinynotation::from_tiny_notation`, music21's TinyNotation: meters, notes
  by letter and octave mark, lengths, dots, accidentals, rests, ties,
  `trip{}` and `quad{}`, lyrics and note names.
- `humdrum::from_humdrum`, music21's `humdrum.spineParser`: each `**kern`
  spine a part and a spine that splits its voices; notes with their
  lengths, tuplets, ties, beams, articulations and ornaments, chords,
  rests, grace notes, barlines and repeats, clefs, keys, meters, tempos and
  instruments; `**dynam` spines as dynamics and `**text` spines as lyrics;
  and a file of several scores as an opus.
- `mei::from_mei`, music21's `mei.base`: staves as parts and layers as
  voices; the meter, key, clef, instrument and transposition of each
  `<staffDef>`; notes, chords, rests, tuplets, articulations and ornaments;
  and the slurs, ties, beams and tuplets written on the notes or apart from
  them by `xml:id`; with the document's metadata.
- `romantext::from_roman_text`, music21's `romanText`: measures of roman
  numerals in their keys, each numeral realized as a chord standing on its
  beat and carrying its figure as a lyric; measures copied from earlier
  ones; repeats and endings; and the analysis's metadata.
- `makenotation::make_measures` and `make_measures_by`, `make_ties`,
  `make_rests`, `make_voices` and `quantize`: music21's `makeMeasures`,
  `makeTies`, `makeRests`, `makeVoices` and `quantize`.
- a note may be named (`Note::id`), and MusicXML export writes the name.
- a `Rest` may carry a tie, as music21's may.
- `musicxml::to_musicxml` and `ExportOptions`: parts, measures, voices,
  notes, chords, rests, grace notes, ties, beams, tuplet brackets,
  accidentals, lyrics, fermatas, ornaments, articulations, slurs, hairpins,
  octave lines, line brackets, pedal marks, arpeggios across staves, cue
  sizes, dynamics, tempo marks and text, segno, coda and the other repeat
  marks, chord symbols, barlines,
  repeats and endings, staff groups, a piano's staves joined into one part,
  instruments, and the score's metadata.
- `musicxml::from_musicxml`, music21's `musicxml.xmlToM21`: parts, a part
  on several staves split into one part for each, measures and their
  numbers, voices, notes, chords, rests, unpitched strokes, grace notes,
  tuplets nested or not, ties, beams, lyrics, articulations, ornaments,
  fermatas, arpeggios, slurs, hairpins, octave lines, brackets, pedals,
  wavy lines, dynamics, tempo marks, words and repeat marks, chord symbols,
  barlines, repeats and endings, instruments and transpositions, staff
  groups and metadata. it takes the document's text; unpacking a `.mxl`
  is the caller's.
- `SpannerKind::TrillExtension`, the wavy line carrying a trill on.
- the wheel has `from_musicxml(text)` and `to_musicxml(score,
  encoding_date=None, software=None)`. the writer takes the wheel's streams
  or music21's; the reader hands back the wheel's own `Score`, holding what
  the wheel has classes for -- notes, chords, rests, clefs, keys, meters,
  tempo marks, dynamics, chord symbols, instruments -- with measure numbers
  and part names as attributes. a wheel stream now takes attributes.
- `metadata::Metadata`, music21's `Metadata`, with its 131-property
  vocabulary; a `Stream` carries one.
- a `Stream` now carries what a score says of its measures and parts:
  measure numbers, part names, ids, staff groups (`StaffGroup`), barlines
  (`bar::Barline`), alternative endings (`bar::Ending`) and spanners.
- `spanner::Spanner`: slurs, crescendos, diminuendos, octave lines
  (`OctaveShift`), lines (`LineEnds`), pedal marks (`Pedal`) and arpeggios
  across several chords, naming what they join by position in
  `Stream::leaves`.
- `repeat::RepeatExpression`, music21's `Segno`, `Coda`, `Fine`, `DaCapo`
  and the rest, with the words each may be written in; a
  `StreamElement::RepeatExpression` holds one.
- `notation::NoteSize` on a note or rest, and a lyric's colour.
- `percussion::Unpitched` and `PercussionChord`, music21's `note.Unpitched`
  and `percussion.PercussionChord`: strokes with no pitch, displayed at a
  step and octave, and chords of them. `StreamElement::Unpitched` and
  `PercussionChord` hold them, and MusicXML export writes them.
- MusicXML export writes a part that changes instrument, naming on each
  note the instrument that plays it.
- `StreamElement::Clef`, `Instrument`, `Key`, `TempoText` and
  `TextExpression`.
- a note, chord or rest carries its expressions (`expressions::Expression`:
  ornaments, `Fermata`, arpeggios) and articulations; a note or rest may be
  hidden from print, and a rest shifted off the middle line.
- `Duration::grace` and `Grace`, what music21's `GraceDuration` says; a
  tuplet says how it is bracketed (`TupletType`, `TupletBracket`,
  `TupletShow`); a time signature its symbol; a key, signature, meter and
  clef their colour; dynamics, tempo marks and chord symbols where they are
  placed; `ChordSymbol::no_chord` for music21's `NoChord`.
- `musicxml::css_color_hex` and `CSS_COLORS`, the CSS3 colour names
  music21 writes a named colour with.
- `Instrument::part_id` and `instrument_id`.
- `chordsymbol::voice_chord_notation` and `ChordVoicing`: a chord of one
  of music21's kinds as music21 voices it, with the root and bass music21
  is left holding.
- `analysis::enharmonics::best_spelling`, music21's
  `EnharmonicSimplifier.bestPitches`, built on `spelling_options` and
  `best_choice`, with its three penalties and scores on `EnharmonicRules`.
  The wheel installs `EnharmonicSimplifier` and both rule classes over
  music21's.
- `analysis::neoriemannian`: `Transform` (L, P, R), `lrp_chain` and
  `lrp_combination`, `complete_hexatonic`, `hexatonic_system`,
  `chromatic_mediant`, `disjunct_mediant`, `slide`, `nebenverwandt`,
  `is_neo_r` and `is_chromatic_mediant`. the wheel installs them over
  music21's `analysis.neoRiemannian`.
- `analysis::harmonic_function::HarmonicFunction`, the eighteen function
  labels, with `to_roman` and `from_roman`: music21's `functionToRoman`
  and `romanToFunction`. the wheel installs them and the enum over
  music21's `analysis.harmonicFunction`.
- `analysis::transposition::TranspositionChecker`, a set of pitches in all
  twelve transpositions and how many differ. the wheel installs it over
  music21's.
- `chordsymbol::alter_to_chord_symbol_string`, music21's
  `alterToChordSymbolString`: `b` or `#` once per semitone.
- `TimeSignature::offset_in_bar`, the arithmetic of music21's
  `getMeasureOffsetOrMeterModulusOffset`: where an element falls in the
  bar, given its offset and the meter's. the wheel's member reads the two
  offsets off music21's stream and hands them over.
- analysis of a whole `Stream`: `analysis::stream_distribution` and
  `estimate_key_of_stream`, music21's `KeyWeightKeyAnalysis` over a
  stream's duration-weighted pitch classes; `stream_pitch_span`, its
  `Ambitus`; and `melodic_lines`, the lines `MelodicIntervalDiversity`
  counts, one per part with ties struck once. `Stream::has_part_like_streams`
  is music21's `hasPartLikeStreams`, which `melodic_lines` reads.
- `Stream::placed`: every element with its offset, its offset in its
  measure, and the key signature, metre and tempo in force, each part
  reading its own. music21 stores sites on each object to answer this;
  the crate answers with a composite `Placed` value instead.
- `Pitch`, `Accidental`, `Microtone` and `Duration` are `Eq` and `Hash`,
  hashing what their equality compares, so they key maps and fill sets.
  music21's are mutable and cannot; its author lists that first among
  the mistakes he would not repeat.
- `Pitch::frequency_hz_tuned` tunes a pitch by its spelling:
  `tuningsystem::Tuning` says how wide an interval sounds, and `Just`,
  `EqualDivision` and `Temperament` are tunings, measured from a
  `tuningsystem::Reference` (`A4` at 440 Hz by default). `C#` and `Db` are
  two notes in a meantone or in 19-EDO, and one in 12-EDO.
- `Interval::pythagorean_monzo`, a spelled interval as octaves and fifths,
  and `Monzo::pythagorean` to build one from staff steps and semitones.
- `Polyrhythm::ratio_frequencies`, the subdivision ratios as frequencies,
  and `PolyrhythmRatioTone::cents`, each ratio's exact width.
- `Polyrhythm::to_score`: one measure of the rhythm as a score, a part for
  each subdivision in tuplets where it does not divide the bar, each playing
  its ratio's pitch, with the tempo as a metronome mark.
- `TuningSystem::nearest_degree`, the degree nearest a frequency;
  `TuningSystem::pitch_at`, the pitch a degree sounds, spelled with a
  microtone; `TuningSystem::is_keyboard`; and `Pitch::from_frequency_in`, a
  frequency snapped to a tuning system and spelled.
- `tuningsystem::Ratio::to_fraction`, `monzo` and `cents`.

## changed
- MusicXML export writes a chord's notes in the order the chord holds them,
  as music21 does, rather than lowest first.
- MusicXML export names a part with no name after the first instrument it
  holds anywhere, inside its measures too, as music21's `partName` does.

- the web score editor reads and writes MusicXML through the crate. its
  export builds a score of the crate's and `to_musicxml` writes it, so the
  file is laid out as music21 lays one out; its import is `from_musicxml`,
  with the tablature tuning, rehearsal marks and drum channel the editor
  wants read off the document beside it.
- `Stream::parts` includes a `PartStaff`, as music21's `.parts` does, and
  the wheel reads music21's `PartStaff` as one rather than as a `Part`.
- the wheel keeps a tuplet's bracketing, a time signature's symbol and a
  metronome mark's placement and parentheses in the crate's value.
- a chord symbol's figure writes its added and omitted notes as degrees
  above the root, in order, with the bass last, as music21 now does:
  `G7addb9add11addb13`, `Am7add11/C`, `Dsusadd#3omit4`, where it wrote
  note names (`CaddD-`) that no chord-symbol parser reads back.
  `ChordSymbolFigure::additions` is a list of `ChordAlteration`s and
  `omissions` of degrees. a figure read as music21 reads one takes any run
  of `b`, `-` and `#` after `add` or `omit` (`add-9`, `addbb9`), and the
  wheel's `findFigure` writes the bass after the modifications
  (`C add 2/B-`).
- MusicXML is behind the `musicxml` feature, which `musescore` turns on.## Fixed

- The music21 submodule follows the fork's master to `5151b5189`, which
  fixes a ninth music21 bug found porting the opus lookups:
  `Metadata.search` compiled any query holding one of `*.|+?{}` as a
  regular expression, so a query carrying that punctuation without being a
  pattern raised `re.error`. Ten titles of the Essen collections -- an
  unbalanced bracket, or a `(?)` -- therefore made `Opus.getScoreByTitle`
  raise when asked for one of music21's own titles. `opus_parity` no
  longer has to count them.

- a lyric written as a hyphen alone is the middle of a word with nothing
  sung to it, as music21 reads it, not the end of one; and a lyric of no
  text at all says nothing of where it falls in a word.

- MusicXML export places a rest that is shifted off the middle line by the
  clef in force where the rest stands, not by the last clef written; and the
  wheel carries a chord's notes hidden one by one.
- the voice-leading walk sorts a text expression ahead of everything at its
  offset, as music21's `classSortOrder` of -30 does; it was sorted with the
  notes.

- `Accidental`'s ordering agrees with its equality. equality compares
  names and ordering alterations, so a renamed natural ordered `Equal` to
  `natural` while `!=` it, which `PartialOrd` forbids. two accidentals of
  one alteration and different names are now unordered. the wheel keeps
  music21's `<`, which compares alterations alone.
- a chord symbol is voiced as music21 voices it, root and bass included.
  music21 puts its own root and bass objects in the list it voices, so an
  octave pass moves them with the chord, a bass held twice moves twice,
  and the inversion reads a bass already moved: `C5/G` is `G3 G3 C4`,
  `B-5/E` `E2 E2 B-3 F4`, and `Ab10/F#` roots on an A3 it does not sound.
  `voice_chord_notation` hands back that voicing with its root and bass.
  9,972 figures agree with music21 on eight properties.
- `Chord::pitched_common_name` names the root `root()` gives and the bass
  `bass()` gives, a root or bass set by hand included, where it read the
  ones it found; a table of spellings pinned seven augmented sixths to one
  root and is gone.
- `ChordSymbol::parse_music21` reads a degree as music21 does, through
  what Python's `int` accepts once sharps and flats are off, so `Cb-7`
  adds a flat seventh where it was refused, and takes the first run of the
  digits one to nine, so `C10` adds a one. a root no pitch spells, `C#-7`,
  raises music21's `AccidentalException`, where the wheel raised
  `ValueError`.
- the report counted a wheel class that inherits its members, such as
  `Trill` from `Ornament`, as lacking them. the wheel figure reads 94%
  where it read 89%.
- a chord respelled by dissonance, as one built from pitch-class numbers
  is, could be spelled otherwise than music21 spells it. the Pythagorean
  penalty gave a twelfth down 1/6 where music21 gives 1/3, and took the
  log of each prime apart where music21 takes one of the whole
  denominator, and a tolerance for ties then hid a difference music21's
  `min` keeps. pitch classes 3, 6 and 9 are `E- G- A` now, as in music21;
  every set of pitch classes and 6,000 respellings of named pitches agree.
- an augmented sixth is named by its root and inversion, as music21 names
  it, for every voicing: a table of spellings had pinned `A- C D F#` and
  six others to one inversion whatever note was in the bass, so nine of
  ten voicings tried were misnamed. and a seventh built from pitch-class
  numbers is an enharmonic equivalent where music21 says so, since the
  fifth above its inferred root is respelled on the way.
- `RomanNumeral`: `frontAlterationString` is what was written, so `It6`
  has none and `-VI` keeps its `-`; `It53`, `Ger7` and `Sw7` root on their
  raised bass; `functionalityScore` looks `V+` up as written; and a
  numeral that is neither a triad nor a seventh is never mixture. 1,185
  numerals now answer eighteen questions as music21's do.
- the wheel's `Key` answers the scale questions music21's `Key` inherits
  from `DiatonicScale`—`getPitches`, `getDominant`, `nextPitch`,
  `derive`, `romanNumeral` and the rest—as the scale of its mode does,
  where it raised `AttributeError`, installed or not. `derive` hands back
  keys, as music21's does.
- a scale's `getChord` and an interval's `noteStart` and `noteEnd` built
  their objects from music21's classes even with no music21 installed,
  and failed there; they use the wheel's own. scales
  have music21's `chord` and `usePitchDegreeCache`.
- a roman numeral made from a degree is written in capitals in a modal
  key, as music21 writes it: 2 in C dorian is `II`, a major triad, where
  it was `ii`.
- `TimeSignature::beat_division_durations` and its subdivisions are read
  off the beat sequence as music21 reads them: the divisions of every beat
  must agree, not the beats, so `2+3/8` divides into eighths where it was
  refused. the wheel raises music21's `TimeSignatureException` where the
  divisions do differ. 110 meters agree with music21 on fifteen
  properties.
- `Chord::closed_position` measured from the note sounding lowest where
  music21 measures from the note written lowest, so `E#4 F-4` closed to
  `F-4 E#5` where music21 keeps `E#4 F-4`. 31 of 4,000 random chords
  differed; none do now.
- `Pitch::frequency_hz` rounds as music21 rounds: it raises the twelfth
  root of two to the distance from A4, where it took two to the distance
  over twelve, and so differed from music21 in the last bits of nearly
  every pitch.
- the report's coverage run instrumented only the workspace since
  cargo-llvm-cov 0.9, whose default wrapper skips `python-parity`: its
  suites ran and wrote no profile. it passes `--no-rustc-wrapper` now, and
  reads 95.2% of lines where it read 90.6%. a suite that did not pass is
  named beside the figure, since it leaves the figure short.

# music21-rs 0.7.1

the wheel read the first item of a music21 stream iterator twice, so with it
installed `chord.Chord(stream.notes)` held its first note twice; that is
fixed. besides, music21's key analysis is in the crate, weighted by duration
and ranked, spelled and scored as music21 ranks, spells and scores it, and
music21's own `analysis.discrete`, `figuredBass.checker` and
`figuredBass.harmony` pass every example with the crate beneath them: 36
modules, 1,004 docstrings and 9,008 examples. a figured-bass segment finds its
voicings about eighteen times faster than it did in 0.7.0.

## added

- `analysis::pitch_class_distribution`, how long each pitch class sounds,
  and `analysis::estimate_key_from_distribution`, every key ranked against
  it: music21's key-weight analysis, checked against music21's rankings and
  coefficients on three chorales and two made-up lines by
  `key_analysis_parity`.
- `analysis::pitch_span`, music21's `Ambitus.getPitchSpan`, and
  `analysis::melodic_interval_counts`, its
  `MelodicIntervalDiversity.countMelodicIntervals`.

## changed

- `estimate_key_from_pitches` and the rest rank keys as music21 does, ties
  and all, and spell them as music21 does: a minor key on the eighth pitch
  class is G-sharp minor, where it was A-flat minor.
- a figured-bass segment walks its voicings as indices over each pitch's
  place and name, worked out once, and builds only the voicings it keeps: a
  seventh chord's 2,744 voicings are looked at in about 75 microseconds,
  where they took 1.3 milliseconds.

## fixed

- the wheel read a music21 stream iterator's first item twice wherever it
  took a whole iterator at once, so `chord.Chord(stream.notes)` with the
  wheel installed held its first note twice. collecting asks an iterator for
  its length, and music21's answers by walking itself from the start; the
  wheel no longer asks.
- a figured-bass segment the wheel builds is made of the wheel's own rules,
  scale, notes and chords wherever music21's are not replaced, not of
  music21's.

# music21-rs 0.7.0

`Tuplet` is music21's now in full, and installed over music21's own, and so
are the instruments and `AbstractScale`. `clef`, `articulations`,
`TempoText` and music21's ornaments are ported and installed, and so is
figured-bass realization: a bass line and its figures voiced in every way the
rules allow. music21's own doctests pass every example of the modules that
came with it: 33 modules, 963 docstrings and 8,760 examples in all. a note
and a chord know the instrument they are played on. the `Tuplet` changes
break its API, which is why this is a minor release.

## breaking changes

- a `Tuplet`'s written values may be unsaid, as music21's may:
  `duration_actual`, `duration_normal`, `tuplet_actual` and `tuplet_normal`
  answer an `Option`, and `Tuplet::ratio` is music21's `Tuplet(3, 2)`, with
  neither side counted in a note value. `duration_type`, `dots`,
  `normal_duration_type` and `normal_dots` still answer, with the eighth
  music21 assumes where nothing is said.
- `Tuplet::multiplier` is music21's `tupletMultiplier`: how long the normal
  side lasts over how long the actual notes would last unaltered. it used to
  be the ratio of the counts alone, which is wrong wherever the two sides are
  counted in different values: three eighths in the time of one quarter
  multiplies each eighth by 2/3, not 1/3.

## added

- `Tuplet::set_duration_actual`, `set_duration_normal`, and `augmented`,
  music21's `augmentOrDiminish`, which scales both written values and keeps
  the ratio.
- `Note::stored_instrument`, `set_stored_instrument` and `instrument`, and the
  same on `Chord`: music21's `storedInstrument` and `getInstrument`, as far as
  a note can answer without the stream around it.
- `Instrument`, `SearchLanguage`, `ensemble_name_by_size` and
  `Simplification` are named at the crate root.
- `clef`: music21's clefs as `Clef` and `ClefKind` -- sign, line, octave
  change and the note on the lowest line for each of its 24 classes;
  `stem_direction_for_pitches`, `from_string` (`clefFromString`) and
  `best_for` (`bestClef`). checked against music21 by `clef_parity`, 796
  cases.
- `articulations`: music21's 52 articulation classes as `Articulation` and
  `ArticulationKind`, each with its volume shift, length shift, where it
  attaches after a tie split and the fields its class carries -- a
  fingering's finger, a string or fret number, a bend. checked by
  `articulation_parity`.
- `tempo::TempoText`, a tempo said in words, with the metronome mark it
  implies and `is_common_tempo_text`.
- `Instrument::set_percussion_pitch` and `set_in_percussion_map`.
- figured-bass realization, music21's `figuredBass` realizer:
  `figuredbass::scale::FiguredBassScale` (`realizerScale`), the voicing rules
  in `figuredbass::possibility`, the resolutions of dominant sevenths,
  diminished sevenths and augmented sixths in `figuredbass::resolution`,
  `figuredbass::rules::Rules`, `figuredbass::segment::Segment` and
  `figuredbass::realizer::{FiguredBassLine, Realization}`, which counts,
  lists and picks from every way of voicing a line. music21's orders are
  kept, so the n-th voicing is music21's n-th.
- `scale::fix_default_octave_for_pitch_list`, music21's
  `fixDefaultOctaveForPitchList`: the octaves a collection's notes leave out,
  filled in so that it rises.
- `Articulation::finger` answers a `Finger`, a number or the text written for
  it, since a MusicXML fingering may be a word.
- `expressions`: music21's 24 ornament classes as `Ornament` and
  `OrnamentKind` -- mordents, trills, turns, appoggiaturas and tremolos, with
  their ornamental pitches in a key and the notes each is played as, a
  nachschlag, a turn's delay and a tremolo's tied repeats included -- and
  `realize_ornaments`, and `Ornament::update_accidental_display`, which
  decides whether each ornamental pitch shows its accidental. checked against
  music21 by `ornament_parity`, 1,368 cases over notes, lengths, keys and
  accidentals.

## changed

- the wheel's `Tuplet` takes music21's keyword-only arguments, leaves a
  written value unset until something says it, refuses a change once frozen
  with music21's `TupletException`, and is installed over music21's, with
  that exception. music21's `duration` docstrings pass all 588 examples with
  it swapped in, where they lost 44 when it was last tried.
- a note's or chord's `storedInstrument` refuses anything that is not an
  instrument with music21's `TypeError`.
- the wheel installs its instruments, clefs, articulations, ornaments,
  `AbstractScale` and `TempoText` over music21's, with `ExpressionException`
  and `TremoloException`, and figured bass's `possibility`, `resolution`,
  `realizerScale`, `rules` and `segment`, which music21's own realizer then
  drives. `AbstractScale` builds its pattern from pitches
  (`buildNetworkFromPitches`) and fixes their octaves in place
  (`fixDefaultOctaveForPitchList`), which is all it lacked: music21's `scale`
  doctests pass all 344 examples with it installed.
  An instrument or a clef the wheel builds is the installed class, as
  music21 holds nothing else in a stream. with every one of them installed,
  music21's suite fails the same fourteen tests as without.
- `Scale::from_pitches` closes a collection as music21 does: not at all when
  the last note already carries the tonic's name, and otherwise on the first
  octave of the tonic past the last note in whichever direction the notes
  went. it used to close only upward, and only by pitch space, so a
  collection falling to its last note, or ending on `B#` above a `C`, closed
  somewhere music21 does not.
- a tuplet frozen into a pickle keeps its bracket, its placement, how its
  numbers are shown, its nesting and whether it is frozen. music21 reads its
  corpus back out of pickles, so a score read from the cache had lost every
  tuplet bracket.
- a tuplet's `bracket` takes `'slur'` as music21's does, and its written
  values are music21's own `DurationTuple` wherever music21 is there, since
  music21's code asks for that class.
- installing a class replaces it in the tables music21 keeps of its classes,
  not only where a module names it, so music21 pickles and looks up the
  installed class wherever it goes by the table.
- an accidental of music21's own handed to the wheel keeps whether it is
  shown, where the wheel used to read its name alone.
- a quarter length whose float is written over more than 64 bits -- anything
  as small as 1/10080 -- comes back as the `Fraction` music21's `opFrac`
  makes of it rather than as a float. a fret bend's `release` read from
  MusicXML is one.

# music21-rs 0.6.0

the stream learns two more things to hold, and with them the last of
music21's stream walks the crate had left out. every one of music21's own
examples now passes in all twenty-three modules under the harness — `sieve`,
both halves of `meter`, `dynamics`, `scale.scala` and `instrument` included,
802 docstrings and 7,467 examples — and 644 of the 691 members the feature map
tracks are ported, up from 565 of 617. every one of the 47 that are not has a
reason written beside it, so the map no longer has an unexplained gap in it.

music21's instruments and metric modulations are ported, and the wheel
installs nine more classes over music21's own, with the exceptions they
raise: `ChordSymbol`, `NoChord`, `ChordStepModification`, `Rest`,
`MetricModulation`, `Sieve`, `TimeSignature`, `MeterTerminal` and
`MeterSequence`. with every one of them installed, music21's own suite of
5,038 tests fails the same fourteen as it does on its own.

it is also the release that takes the breaking changes earlier ones held
back: a tuning system that can repeat at any period, a pitch's accidental
kept as music21 keeps it, one scale type where there were two, and no
`#[non_exhaustive]` where it only hid new variants from a caller's `match`.

## breaking changes

- `TuningSystem::EqualTemperament { octave_size }`, `WholeTone` and
  `QuarterTone` are one variant, `TuningSystem::Equal(EqualDivision)`, so an
  equal temperament may divide any period: `13edt`, Bohlen-Pierce, is a
  tuning system like the rest. `EQUAL_TEMPERAMENT`, `WHOLE_TONE` and
  `QUARTER_TONE` are constants and `TuningSystem::edo(n)` names any other.
  - `octave_size` is `degrees_per_period` and `octave` is `period_of`;
    `period_cents` and `repeats_at_the_octave` are new.
  - `fraction` returns an `Option`, `None` only for an irrational period.
  - `id`, `display_name` and `description` return `String`, and an equal
    system's id is its division, `19edo`. `EqualTemperament`, `WholeTone`
    and `QuarterTone` still parse.
  - the `size` argument is gone from `AnyTuningSystem` and
    `AdaptiveTuningSystem`, and so are the free functions `get_ratio`,
    `get_fraction`, `get_label`, `get_frequency`, `get_frequency_at`,
    `get_cents` and `get_cents_at`; the methods remain.
  - `TuningSystem`, `AdaptiveTuningSystem` and `AnyTuningSystem` are
    `PartialEq` but no longer `Eq`.
- `Pitch::accidental` returns `Option<&Accidental>`: `None` for a bare `D`
  and a natural for `Dn`, as music21's `.accidental` does. it used to answer
  a natural for both. `explicit_accidental`, `explicit_accidental_mut` and
  `has_accidental` are gone; `accidental_mut` edits in place, and
  `Pitch::alter` is the alteration either way. serialized pitches drop the
  `has_accidental` field.
- `Key::scale` returns a `Scale`, read off the key's signature so every key
  has one, and `Key::as_scale` is gone. `DiatonicScale` is gone.
- `StreamElement` has two new variants, `Dynamic` and `ChordSymbol`.
  `StreamElement`, `DurationType` and `ScaleType` are not
  `#[non_exhaustive]`, so a new variant is a compile error in a caller's
  `match` rather than something a wildcard arm swallows.
- `ChordSymbol::duration` returns `&Duration`.
- `MeterTerminal::division_options` is `partition_options` and
  `set_weights_at_level` is `set_level_weight`, music21's names for them.
- `Pitch::from_name`, `Note::from_name` and `Interval::from_name` take
  `impl AsRef<str>`. a `&str`,
  a `String` or a `&String` passes as before; only a type that could be
  turned into a `String` without being one has to say so now.
- `tuningsystem::FORTYTHREE_TONE`, an alias of `FORTY_THREE_TONE`, is gone.
- `Duration::add_duration_tuple` keeps the values it is given. a quarter with
  a half and a quarter tied on is three written values, and has no single
  type, until `consolidate` is called; it used only to lengthen the duration,
  so `components` read back one whole note. this is what music21 does.

## added

- `meter::MeterTerminal::from_partition_string` and `from_parts_given`, which
  read a span written as `2/4+2/4` or built out of the parts it holds;
  `offset_to_address`, every index down to the terminal sounding at an
  offset; `subdivide_nested_hierarchy`, a span nested to a depth; and
  `meter::partition_name`, what a partition of so many parts is called.
- the rest of what music21 asks a span: `ratio_equal`, `flatten` and
  `flat_weights`, `level_weight`, `partition_options`, `set_level_weight`,
  `subdivide_by_other` and `partition_by_other`. a span flattens to what its
  leaves come to, so a bar of four quarters divided into eighths flattens to
  `8/8`.
- a meter sequence now weighs what its parts weigh, and sharing a weight out
  among them follows how long each part is, as music21 does. a span used to
  keep one weight of its own and leave every part weighing one.
- music21's own `meter.core` docstrings run against the crate, as
  `doctest_metercore`: all 386 of their examples and all 39 docstrings pass,
  from 39 examples when the harness was first pointed at them. a
  `MeterSequence` holds its parts as the objects music21 hands back, adopts
  the spans it is built from, and carries music21's `_levelListCache`.
- `ScalaScale` answers a Scala file the way music21's `ScalaData` does:
  `written_cents` and `written_degrees` for the lines the file writes,
  `adjacent_cents` and `interval_sequence` for the width of each step,
  `from_adjacent_cents`, `from_interval_sequence` and `from_written_cents` for
  a scale built out of those, and `file_string` for the `.scl` text.
  `ScalaArchive::find` is the searching half of music21's `parse`: the one
  scale a name names. a `ScalaDegree` reads and writes its own line,
  through `FromStr` and `written`. a degree read as a
  ratio is written back as that ratio, so a file read and written is the
  scale it was; music21 keeps cents alone and cannot.
- `dynamics::Dynamic`: a dynamic mark and the loudness it stands for, with
  music21's tables for the marks, their Italian and English names and their
  scalars, and `dynamic_str_from_decimal` for the mark a loudness falls
  under. a mark music21 has no loudness for is read without its `s` and its
  closing `z`, so `sfz` is as loud as `f`. checked against music21 by
  `table_parity`.
- `volume::realize_volume`: sets how loud every note and chord of a stream
  sounds from the dynamics in force, each until the next, and writes the
  answer back as an absolute velocity. `DynamicContext` is music21's
  `useDynamicContext`: the stream's own dynamics, none, or one for
  everything. the expectations are music21's own `testRealizeVolumeA`.
- `realize_chord_symbol_durations`: every chord symbol in a stream holds
  until the next one, wherever in the nesting that is, and the last to the
  end. `ChordSymbol` has a `duration`, nought until something says.
- `Stream::for_each_mut`: every element with its offset from the stream's
  start, nested streams walked into, for an edit that has to land in the
  score itself rather than in a flattened copy.
- `voiceleading::iterate_all_voice_leading_quartets`, with `QuartetOptions`:
  every `VoiceLeadingQuartet` in a score, as music21's walk over a timespan
  tree finds them. held to 4,451 quartets over three corpus scores by
  `voice_leading_parity`. `verticality_at` is what is sounding in each part
  at an offset.
- `tempo::interpolate_elements`: carries what lies between two points of one
  stream into another, keeping each thing's place between them.
- `Sieve::compressed`: the compressed reading of a sieve, a union of residual
  classes with the same members. a sieve of nothing but `&` and `|` is
  compressed by intersecting its classes; one with a group, a `^` or a
  complemented group from its members over a range.
- `instrument`: music21's 116 instrument classes as `Instrument::of_kind`,
  each with the families above it (`is_a`), its name and abbreviation, its
  General MIDI program and sound, its range and its transposition.
  `Instrument::from_name` is music21's `fromString`, reading a part name in
  six languages and their abbreviations along with the key a transposing
  instrument is in; `from_midi_program`, `all_names`,
  `auto_assign_midi_channel` and `ensemble_name_by_size` are the rest. every
  kind, all 128 programs and 2,054 lookups are checked against music21 by
  `instrument_parity`. `string_pitches` is a string instrument's open strings
  and `set_modifier` says how unpitched percussion is played, choosing the
  drum it plays. music21's own `instrument` docstrings run against the crate,
  as `doctest_instrument`, and all 150 of their examples pass.
- `tempo::MetricModulation`: a change of tempo written as an equation
  between two metronome marks, with music21's ways of setting one side from
  the other. `update_from` fills in what the marks leave unsaid from the mark
  in force before it.
- `Sieve::parse` reads every spelling music21 reads: `[...]` groups, `M,N`
  and `MsubN` residuals, `and` and `*`, `or` and `+`, and `not`.
  `Sieve::from_segment` builds a sieve from integers as music21's
  `CompressionSegment` does.
- scales that take their steps as an argument: `Scale::from_steps`, an
  octave-repeating scale given by the steps between its notes;
  `Scale::from_cycle`, a cycle walked from its tonic both ways, for a cycle
  that does not come back at the octave; `Scale::from_scala`, a Scala file's
  steps walked the same way; and `StepScale::sieve_by`, a sieve counted in
  any unit. `pitch_on_degree_between` reads a degree off a range realized,
  and `Simplification` with `with_simplification` says how a scale spells.
- `ChordSymbol::parse_music21` reads a figure as music21's `_parseFigure`
  does, and `chordsymbol::sound_chord_notation` sounds it as `_updatePitches`
  does, octaves, inversion and modifications included. all 524 figures of
  the parity fixture agree note for note.
- every stream walk has a twin answering in positions in `Stream::leaves`:
  `volume::dynamics_in_force`, `chordsymbol::chord_symbol_durations`,
  `voiceleading::voice_leading_quartet_positions`,
  `voiceleading::verticality_positions_at` and `tempo::interpolated_offset`.
- a `Rest` carries the words sung under it; the verse rules a note and a
  rest share are `notation::verses`.
- `TimeSignature::summed_numerator` and `MeterTerminal::summed_numerator`:
  whether a meter was written `3+2/8`, which beams in the parts it was
  written in.
- a score editor at `/score/` on the site: ABC typed on the left is engraved
  and played by abcjs, and everything musical about it — keys, harmony at
  every onset, parallel and hidden fifths and octaves, MIDI and MusicXML
  export, cents in any tuning system — is answered by the crate.
- `Duration` can have its written values said rather than read off its
  length: `consolidate`, `slice_component_at_position`, `set_dot_groups`,
  `split_dot_groups`, `linked` and `set_linked`, `grace_duration` and
  `written_values`, with `DurationTuple` for one written value and how long
  it lasts. the two can disagree: a grace note is an eighth lasting nothing.

## changed

- **a chord builds no Python object until something asks for one.** a chord
  holds a `Note` and a `Pitch` object per note so that an edit through one of
  them sticks, and it built every one of them on construction whether or not
  anything ever looked. they are built a note at a time now, when that note is
  wanted, and `inner` is the chord meanwhile -- which is what it always was.
  Against music21 through the wheel, the median of the benchmark's 21 cases
  goes from 14.3x to 24.1x on one machine: `commonName` 36.9us to 18.9us,
  `forteClass` 30.9 to 14.1, `inversion` 28.1 to 11.4, `orderedPitchClasses`
  29.0 to 11.9. nothing a caller can see changes -- the same object still
  comes back every time, and music21's own 7,315 examples pass as they did.
- **everything that reads a name or asks a chord a question is faster**, by
  between two and thirty times, with no change to any answer -- music21's own
  7,315 examples pass as they did. what it cost was allocation:

  | | before | after |
  | --- | --- | --- |
  | `Interval::from_name("P5")` | 1151ns | 33ns |
  | `Pitch::from_name("C#4")` | 584ns | 136ns |
  | `Pitch::clone` (with an accidental) | 127ns | 18ns |
  | `Chord::root`, `inversion`, `third` | ~450ns | ~45ns |
  | `Chord::is_dominant_seventh` | 457ns | 15ns |
  | `Chord::common_name` | 5025ns | 976ns |
  | `Chord::new("C4 E4 G4")` | 1340ns | 708ns |

  an accidental borrows its name and its modifier from the table rather than
  owning two strings, so copying a pitch no longer allocates. an ordinary
  pitch name and an ordinary interval name are read where they stand. the
  root search fits in an array of seven, there being seven letters, instead
  of a set, a map and three vectors. a chord counts its distinct pitch names
  without writing any of them. a pitch class is written as a character rather
  than through the formatting machinery. and a specifier is compared without
  lowercasing eleven prefixes and eleven names to do it. naming a chord keeps
  the names a set class carries borrowed from the table until one of them is
  the answer, reads the spellings it checks for off the notes rather than out
  of a set of their names, and counts pitch classes in a bitmask.
- `Pitch::from_name` and `Note::from_name` take `impl AsRef<str>` rather than
  `impl Into<String>`, so a name is read where the caller has it instead of
  being copied first. everything that could be passed before still can.
- the report's headline speedup is the benchmark's median, with the median
  over music21's own suite beside it, and its tests tile no longer counts the
  tests music21 fails on its own as failing.
- music21's own `dynamics` and `scale.scala` docstrings run against the
  crate, as `doctest_dynamics` and `doctest_scala`, and every docstring is now
  read the way music21's own runner reads it -- its `fixDoctests` rewrites an
  object's address to `0x...` before comparing anything, and forty-two of its
  docstrings are written with an address in full. all 7,317 examples of the
  twenty-two modules pass, with nothing left out.
- `MeterSequence.offsetToDepth` takes music21's `index`, the hint its v7 added
  for which part of the finest level an offset falls in.
- music21's own suite runs in the environment its tests need. 109 of them
  failed on music21 itself in CI -- 71 for want of matplotlib, 36 of
  LilyPond, 2 of a `~/Desktop` -- and the last one failed because the runner
  here ignored every warning, including the one music21's `deprecated`
  doctest exists to show. with all of that provided, music21 fails none of its
  own tests, so every test in the comparison now measures something. the
  `dynamics` classes are installed by `install_into_music21` on the strength
  of it.
- the report tracks three modules it could not see before: `meter.core`,
  whose two classes the whole meter tree is made of; `tie`, whose entire API
  is the names in its `__slots__`, which no scan for methods could find; and
  `scale.scala`. a feature-map entry can now say `members = "slots"`.
- **a scale is realized over a range as music21 walks it.** going up, from
  the tonic moved by whole periods to the closest at or below the range;
  coming down, down the descending pattern from the closest at or above it,
  each walk stopping at the first note past its end. a scale that respells
  as it goes spells its descent differently, so Rag Asawari on B- comes down
  through F#4 and C#4. the whole-tone and chromatic scales are cycles, as
  music21 builds them. `derive_ranked` realizes each candidate between the
  lowest and highest pitch it is given, as music21 does; the divergence this
  file used to record there was never shown by any input. 630 realizations
  over ten scale types, nine tonics and seven ranges agree with music21.
- a time signature's counting word is read anywhere and in any case, so
  `"6/8 fast"` is fast 6/8, and `subdivide_nested_hierarchy_by` loads a
  sequence's own uneven parts first, as music21 does for 11/8.
- the wheel carries `Stream`, `Score`, `Part`, `PartStaff`, `Measure`,
  `Voice` and `Opus`, with music21's five stream walks over them;
  `OctaveRepeatingScale`, `CyclicalScale`, `SieveScale` and `ScalaScale`;
  a class for every instrument kind; and a `Style` and `NoteStyle` of its
  own where music21 cannot be imported, so a wheel pickle reads back without
  music21.
- an installed class compares and hashes equal to the class it replaces, so
  `classSet` holds one where it held two, and an exception is installed as
  the facade's own class, so music21's `except` clauses catch what the
  facade raises. music21's tests are gathered before anything is installed,
  and every side runs the same 5,038 under the same names.
- coverage is measured on nightly with cargo-llvm-cov's `--doctests`, so the
  rustdoc examples count; the report also clears the profiles earlier runs
  left, which it had been merging in.
- the `music21` submodule points at `float3/music21` master, upstream with
  the fixes sent from here merged in, until they land upstream; every
  generated fixture is stamped with its commit. rustls is at 0.23.45 for
  RUSTSEC-2026-0285, and every GitHub Action is pinned to a commit.

# music21-rs 0.5.0

the release that finishes what 0.4.0 started: seventeen of music21's modules
now pass every one of their own doctests, `harmony` among them, and 565 of
the 617 members the feature map tracks are ported, up from 516. nothing a
caller had changes shape; this is a minor bump for what was added and for
the modules that moved into directories, every public path of which is
re-exported where it was.

## added

- `chordsymbol::ChordSymbolFigure`: a chord named as a lead-sheet symbol in
  the parts music21 writes it from — root, kind, abbreviation, bass,
  additions and omissions — with `written_with` for another abbreviation.
  It is the one writer behind `Chord::chord_symbol`,
  `chord_symbol_figure_from_chord` and the Python facade, and it follows
  music21's rules: a chord that leaves out a note of its kind is still that
  kind and says nothing about it, a bass the kind does not carry is never an
  addition, and a suspended second in inversion is a suspended fourth on its
  bass. `ChordSymbol::kind` names the music21 kind a shorthand is, and a
  shorthand that is one of music21's abbreviations — `N6`, `It+6`, `+11`,
  `tristan` — is realized from that kind's notation.
- `TimeSignature` derives music21's default accent hierarchy from the
  numerator and denominator: `accent_weights`, `accent_weight`,
  `accent_weight_with`, `accent`, `accent_partition_quarter_length`,
  `beat_depth` and `average_beat_strength` over a stream, checked against
  music21 across the 126 meters the fixture sweeps. `meter::best_time_signature`
  fits a signature to what a measure holds.
- `roman::FigureTuple` and `PitchFigureTuple`, with `figure_tuples`,
  `correct_rn_alteration_for_minor`, `correct_suffix_for_chord_quality` and
  `adjust_minor_vi_and_vii_by_quality`; `Scale::roman_numeral` and
  `Scale::scala_data`; `Scale::tune` over a stream.
- `duration`: `quarter_length_to_tuplet`, `quarter_length_to_non_power_of_2_tuplet`
  and `quarter_conversion` as functions of their own, `Duration::is_complex`,
  `component_index_at_qtr_position`, `component_start_time`, `clear`,
  `add_duration_tuple` and `dot_groups`, and both sides of a `Tuplet`
  (`duration_actual`, `duration_normal`, `tuplet_actual`, `tuplet_normal`,
  `set_duration_type`, `set_ratio`).
- `sieve`: `eratosthenes`, `rabin_miller`, `discrete_binary_pad`,
  `unit_norm_range`, `unit_norm_equal` and `unit_norm_step`.
- `Modifier::modify_pitch_name` and `figuredbass::convert_to_pitch`;
  `DiatonicInterval::specifier_abbreviation`; `Lyric::set_text_and_syllabic`;
  `Rest::full_name`; `MetronomeMark::text_expression`;
  `Volume::merge_attributes`; `ToneRow::make_twelve_tone_row`;
  `VoiceLeadingQuartet::opens_incorrectly` and `closes_incorrectly`;
  `ScalaScale::new`; `Stream::events_mut` and `StreamEvent::element_mut`.

## changed

- `chord`, `roman`, `chordsymbol`, `pitch`, `scale` and `serial` are
  directory modules split by concern; every public item is re-exported from
  where it was.
- the Python wheel installs `harmony.chordSymbolFigureFromChord` and
  `chordSymbolFromChord` over music21's own, honours `changeAbbreviationFor`,
  and reads a `ChordSymbol` by its pitches and the root its figure fixed.
  `duration` has a facade file of its own.
- comments and docs describe the behaviour as it stands rather than the
  history behind it.

## internal

- the unit tests reach every public function of the crate; line coverage of
  `src` by the unit tests alone is 95%, up from 90%.
- the report generator is four files rather than one; the nineteen doctest
  harness files are one `doctest_suite!` invocation each; the facade's error
  mappers are one macro; gcd and lcm come from the `num` crate.

# music21-rs 0.4.0

the release that makes the crate something music21 itself can be run on.
The classes ship as a Python wheel, `music21_rs`, and
`install_into_music21()` puts them into an installed music21 in place of its
own, so a program written against music21 runs on the crate without a line
changed — checked against music21's whole test suite and against a library
written by someone who had never heard of this crate. fifteen of music21's
modules pass every one of their own doctests, 5,460 examples in all.

beside that: roman numerals are figured bass over a scale rather than a
stack of intervals, figured bass is a module of its own, durations carry
what they are written as, notes and chords carry beams and lyrics, and the
tuning half grew regular temperament theory and all 95 of the Xenharmonic
Wiki's temperaments. several signatures change, so this is a minor bump.

## added

- `Interval` gains music21's compact names (`short_name`, `simple_name`,
  `semi_simple_name`, `directed_name`), the `is_step`, `is_diatonic_step`,
  `is_chromatic_step`, `is_skip` and `is_consonant` predicates,
  `complement`, `interval_class`, and `Interval::sum` / `Interval::difference`
  for `interval.add` and `interval.subtract`.
- the halves of an interval are public: `GenericInterval`, `DiatonicInterval`,
  `ChromaticInterval` and `Specifier`, with music21's full method set —
  `simple_directed`, `semi_simple_undirected`, `octaves`, `staff_distance`,
  `mod7`, `mod7_inversion`, `complement`, `reverse`, `nice_name` and the
  directed and simple name variants, `get_diatonic`/`get_chromatic`,
  `cents`, `interval_class`, and `DiatonicInterval::try_new`, which rejects
  a "Major Fifth" or a descending perfect unison the way music21 does.
  `Interval` exposes them as `generic()`, `diatonic()`, `chromatic()` and
  `specifier()`, and can be built from either half with `from_diatonic` and
  `from_chromatic`.
- `GenericInterval::transpose_pitch` is music21's generic transposition —
  it moves the staff position and keeps the accidental, so a third above
  `C#4` is `E#4` — and `transpose_pitch_key_aware` carries a pitch's offset
  from a key signature across, so `F` natural in G major steps up to `G-`.
  `GenericInterval::from_name` reads `"Third"`, `"3rd"` and
  `"Descending Fifth"`.
- accidentals carry music21's display state: `Pitch::update_accidental_display`
  decides whether an accidental should be shown from the pitches before it,
  the key signature's altered pitches and the cautionary switches
  (`AccidentalDisplayOptions`), adding a cautionary natural where music21
  adds one. `Accidental::set_attribute_independently` writes one part of an
  accidental without the other two following, which is how a `sori` is built.
- a pitch now knows whether it carries an accidental object at all:
  `Pitch::has_accidental`, `explicit_accidental`, `explicit_accidental_mut`
  and `set_accidental`. music21 keeps no accidental on a bare `D` but does
  keep an explicit `Dn`, and the two are no longer equal.
  `Pitch::set_accidental_alter` splits a fractional alteration into an
  accidental and a microtone, and `set_microtone_cents` sets or clears the
  microtone. `Pitch::name_in_key_signature` and `step_in_key_signature` are
  music21's key-signature helpers.
- Non-traditional key signatures: `KeySignature::from_altered_pitches` takes
  the pitches a signature alters where no count of sharps can express it
  (`E-` with `G#`), with `is_non_traditional`, `set_altered_pitches`,
  `set_sharps`, `accidentals_apply_only_to_octave` and a `Display` that
  writes music21's `KeySignature of pitches: [E-, G#]`.
- `Chord` gains music21's mutating half: `add`, `remove`, `remove_named`,
  `set_root`, `set_bass` and `set_inversion`, with `found_root`,
  `found_bass`, `overridden_root` and `overridden_bass` to tell an inferred
  answer from one a caller fixed. `chord_step_with_root`,
  `semitones_from_chord_step_with_root` and `inversion_from_root` are
  music21's `testRoot` forms, and `note_pitch_classes` is music21's
  `pitchClasses`, in chord order with repeats kept.
- `format_vector_string` writes a pitch-class list the way music21's
  `Chord.formatVectorString` does, with ten and eleven as `A` and `B`.
- notes and chords carry the notation music21 puts on them, in two new
  modules. `notation` holds `Tie` (with `TieType`, `TieStyle` and
  `Placement`), `Notehead`, `StemDirection` and `Lyric` (with `Syllabic`);
  `volume` holds `Volume`, which is a MIDI velocity and its 0-to-1 scalar
  together with music21's rule for reading them against the dynamics around
  them. `Note` gains `tie`, `notehead`, `notehead_fill`,
  `notehead_parenthesis`, `stem_direction`, `color`, `volume`, `lyrics`,
  `lyric` and `add_lyric`; `Accidental` gains `color`.
- `Chord` gains the same notation through its notes — `notes_mut`,
  `note_for_pitch`, `note_for_pitch_mut` — plus `tie`, `color`,
  `color_of_pitch`, `volume`, `set_volumes`, `has_component_volumes`,
  `lyrics` and `add_lyric`. `annotate_intervals` names the interval from the
  lowest pitch up to each of the others, and `annotated_with_intervals`
  writes those names on as lyrics.
- `Chord::string_harmonic` reads a written string harmonic: a chord whose
  second note has a diamond head comes back with the pitch it actually
  sounds added on top. this is music21's `Pitch.getStringHarmonic`, which
  was previously left out because it depends on noteheads.
- `Chord::remove_redundant_pitches_reporting` and its name and pitch-class
  variants hand back the pitches they dropped, as music21's do in place.
- `Accidental::set_attribute_independently` takes an `AccidentalAttribute`,
  so only the three parts music21 allows can be named.
- the module functions `notes_to_generic`, `notes_to_chromatic`,
  `intervals_to_diatonic`, `convert_diatonic_number_to_step`,
  `convert_semitone_to_specifier_generic`,
  `convert_semitone_to_specifier_generic_microtone`, `convert_generic` and
  `parse_specifier`, and `Interval::transpose_pitch_with_options` with
  music21's `reverse` and `maxAccidental` arguments, are public.
- `Chord` gains `root`, `bass`, `chord_step`, `third`, `fifth`, `seventh`,
  `semitones_from_chord_step`, `has_repeated_chord_step`,
  `has_any_enharmonic_spelled_pitches`, the `is_triad` / `is_major_triad` /
  `is_minor_triad` / `is_diminished_triad` / `is_augmented_triad` /
  `is_seventh` / `is_dominant_seventh` / `is_half_diminished_seventh` /
  `is_diminished_seventh` / `is_incomplete_major_triad` /
  `is_incomplete_minor_triad` / `contains_triad` / `contains_seventh` /
  `is_consonant` predicates, `quality` returning the new `TriadQuality`, and
  `closed_position`, `remove_redundant_pitches`,
  `remove_redundant_pitch_names`, `remove_redundant_pitch_classes` and
  `sort_ascending`.
- `voiceleading::VoiceLeadingQuartet` and `MotionType`, a port of music21's
  two-voice motion classification with parallel and hidden fifth and octave
  checks, voice crossing and voice overlap.
- `analysis::KeyProfile` with the five key-finding weight sets music21 ships
  (Krumhansl-Schmuckler, Aarden-Essen, Simple, Bellman-Budge,
  Temperley-Kostka-Payne), verified against music21 by the parity fixture,
  and `estimate_key_from_pitches_with` / `estimate_key_from_chords_with` to
  choose one.
- `Chord::transpose`, the augmented-sixth family (`is_augmented_sixth`,
  `is_italian_augmented_sixth`, `is_french_augmented_sixth`,
  `is_german_augmented_sixth`, `is_swiss_augmented_sixth`), `is_ninth`,
  `is_transpositionally_symmetrical`, `can_be_dominant_v`, `can_be_tonic`,
  `has_any_repeated_diatonic_note`, `prime_form`, `prime_form_string`,
  `forte_class_tni`, `pitch_class_cardinality` and
  `ordered_pitch_classes_string`.
- `Interval::directed_simple_name`.
- `Scale::degree_of`, `degree_of_pitch_class`, `next_pitch_above` and
  `next_pitch_below`, the realized-scale counterparts of music21's
  `getScaleDegreeFromPitch` and `nextPitch`.
- `tempo::MetronomeMark`, `convert_tempo_by_referent` and the
  `DEFAULT_TEMPO_VALUES` word table, verified against music21 by the parity
  fixture.
- `Error::Duration`, `Error::Key`, `Error::Scale` and `Error::Tempo`
  variants. duration, mode and scale-degree errors used to be reported as
  `Error::Ordinal`.
- `Pitch::from_frequency`, `harmonic`, `harmonic_from_fundamental`,
  `harmonic_string`, `is_enharmonic`, `all_common_enharmonics`,
  `transpose_below_target`, `transpose_above_target`, `is_twelve_tone` and
  `diatonic_note_number`.
- `KeySignature::altered_pitches`, `accidental_by_step`, `transpose` and
  `scale`; `Key::tonic_pitch_name_with_case`.
- `Duration::type_and_dots`, `dots` and `full_name`,
  `DurationType::type_number`, and `quarter_length_to_closest_type`.
- `ScaleType::derive`, `derive_ranked` and `derive_all` find the tonics on
  which a scale type contains a set of pitches; `Scale::degree_and_accidental_of`
  and `Chord::scale_degrees` report degrees with the accidental that departs
  from the scale's spelling.
- `Pitch::german`, `italian`, `french`, `spanish`, `unicode_name` and
  `unicode_name_with_octave`; `Pitch::transpose` is public and returns
  `Result` instead of swallowing failures.
- `serial::ToneRow`, a port of music21's `serial` module: zero- and
  original-centered `P`/`I`/`R`/`RI` transformations and their inverse
  search, `matrix` and `row_to_matrix`, `intervals_as_string`,
  `is_all_interval`, Link chord classification and hexachordal
  combinatoriality, plus the 71 historical rows as `HISTORICAL_ROWS` and
  `historical_row_by_name`. both tables are verified against music21 by a
  new `serial_expectations.toml` fixture.
- `Error::Serial`.
- `Chord::normal_order` and `normal_order_string` (music21's `normalOrder`,
  on the chord's own pitch classes), `interval_vector_string`,
  `forte_class_number`, `forte_class_tn`, `multiset_cardinality`,
  `is_prime_form_inversion`, `has_z_relation`, `are_z_relations`,
  `is_false_diminished_seventh`, `inversion_text`,
  `interval_from_chord_step`, `semi_closed_position`,
  `sort_chromatic_ascending`, `sort_diatonic_ascending`,
  `sort_frequency_ascending`, `pitch_names` and `full_name`.
- `Interval::simple_nice_name`, `semi_simple_nice_name`,
  `directed_nice_name`, `directed_simple_nice_name`,
  `directed_semi_simple_nice_name`, `specific_name`, `cents`, `is_unison`,
  `is_perfectable`, `staff_distance`, `mod7`, `mod7_inversion` and `mod12`.
- `Pitch::full_name`, music21's `E-flat in octave 4 (+20c)` spelling.
- `TimeSignature::offset_from_beat`, `beat_progress`, `beat_proportion`,
  `beat_proportion_string`, `beat_division_quarter_lengths`,
  `beat_division_durations`, `beat_sub_division_durations`,
  `beat_division_count_name`, `ratio_equal` and the two beat-to-quarter
  ratios; the division lengths are checked against music21 by the meter
  fixture.
- `Scale::derive_by_degree` and `Key::derive_by_degree`,
  `KeySignature::transpose_pitch_from_c`, and `analysis::tonal_certainty`
  over a ranked list of key estimates.
- `Pitch::get_enharmonic`, `implicit_octave`, `pitch_class_string`,
  `cent_shift_from_midi`, `convert_quarter_tones_to_microtones`,
  `convert_microtones_to_quarter_tones`,
  `harmonic_and_fundamental_from_pitch` and
  `harmonic_and_fundamental_string_from_pitch`.
- `Scale::degree_count`, `transpose`, `chord`, `pitches_from_scale_degrees`,
  `interval_between_degrees`, `is_next`, `match_pitches`, `find_missing` and
  `solfeg` with `SolfegVariant`; the two syllable tables are pinned against
  music21 by the table fixture.
- `RomanNumeral::roman_numeral`, `figure_and_key`,
  `scale_degree_with_alteration`, `functionality_score` (with the
  `FUNCTIONALITY_SCORES` table, pinned against music21), `is_neapolitan`,
  `is_mixture` and `transpose`.
- `VoiceLeadingQuartet::with_key`, `key`, `is_proper_resolution`,
  `leap_not_set_with_step` and `clausula_vera`.
- `Note::step`, `octave`, `pitches` and `full_name`; `Duration::ordinal`,
  `augment_or_diminish` and `DurationType::ordinal`;
  `MetronomeMark::equivalent_by_referent` and
  `maintained_number_with_referent`; `Chord::geometric_normal_form`.
- `pitch::simplify_multiple_enharmonics` and `dissonance_score`, and
  `chordsymbol::chord_symbol_figure_from_chord` and `chord_symbol_from_chord`,
  the last three music21 members the feature map listed as missing.
- `ChordSymbol::find_figure`, `transpose` and `inversion_is_valid`;
  `Music21ChordType` now carries every abbreviation music21 accepts, with
  `abbreviations_for_kind`, `notation_for_kind` and
  `current_abbreviation_for_kind` to read them, and the parity fixture
  checks the whole list.
- `Chord::from_forte_class`, `from_forte_address` and `from_interval_vector`;
  `roman::roman_inversion_name` and `identify_as_tonic_or_dominant`;
  `key::convert_key_string_to_music21_key_string`;
  `DurationType::next_larger` and `next_smaller`; `convert_pitch_class_to_str`
  is public.
- `Interval::from_generic_and_chromatic`, `pitch_start`, `pitch_end`,
  `note_start`, `note_end`, and the free functions
  `written_higher_pitch`, `written_lower_pitch`, `absolute_higher_pitch`,
  `absolute_lower_pitch` and `staff_distance_to_generic_number`.
- `Note::transpose`, `Key::transpose` and `Key::as_scale`.
- `Interval` implements `Display` as music21's interval `repr` does — the
  directed name plus the cent shift past its diatonic spelling, so the
  interval from `C1` to a `C1` half-sharp reads `A1 (-50c)` — and
  `Interval::diatonic_interval_cent_shift` is that shift on its own.
- `ChromaticInterval::from_int` and `whole_semitones`, and
  `Interval::whole_semitones`, for the whole-semitone count where the
  fractional one is not wanted.
- the crate's classes ship as a Python wheel, `music21_rs`, built from
  `python/` by maturin — music21-shaped `Pitch`, `Note`, `Chord`,
  `Duration`, `Interval`, `Key`, `RomanNumeral`, `MetronomeMark`, `ToneRow`
  and the rest, with music21's names, keyword arguments, properties and
  `repr`. `install_into_music21()` puts them into an installed music21 in
  place of its own, so an existing program runs on the crate without a line
  changed.
- `chord::tables` is public, and is music21's own `chord.tables` module: a
  caller holding a cardinality and a Forte class number reads a prime form,
  an interval vector, a Forte name or the common names without building a
  chord to ask.
- regular temperament theory, the Xenharmonic Wiki's half of the subject, in
  four new types under `tuningsystem`. `Monzo` is an interval as the
  exponents of the primes making up its ratio and `Val` is how many steps of
  a tuning each prime is worth, both reading and writing the wiki's bracket
  notation; `Mos` is a moment-of-symmetry pattern and `MosScale` a generator
  and a period in cents; `Temperament` is a period, some generators and a
  mapping onto a subgroup, built from the wiki's own infobox line by
  `from_mapping` and `from_published`; `EqualDivision` is an equal division
  of any interval, which is where Bohlen-Pierce and Carlos Alpha, Beta and
  Gamma live. rank is not fixed at two and the equave need not be the octave.
- all 95 of the wiki's `Infobox regtemp` temperaments are carried as data, in
  `data/temperaments.toml` and the `WIKI_TEMPERAMENTS` table emitted from it.
  Nothing in it is trusted: a test builds every entry, checks that each of the
  277 commas the wiki lists really does vanish under the mapping, and checks
  that each published MOS pattern really does come out of the generator.
- the tuning collections are checked for holding the same scale twice, within
  each of them and across them: nothing in `ALL_TUNING_SYSTEMS` or in
  `WIKI_TEMPERAMENTS` is a second name for anything else in it, 23 of the 28
  ratio tables are in the bundled Scala archive as well, and the archive holds
  44 groups covering 54 redundant files out of 3,994. the comparison is by
  sound rather than by spelling, and by multiset rather than by set. it is
  compiled for the tests alone and is not public API — a caller wants a scale
  it can play, not a fingerprint it can only compare — so exporting it later,
  if anyone asks for it, stays an addition rather than a break.
- `RomanNumeral` is figured bass over a scale, which is what music21's
  `_updatePitches` is: the bass is the scale degree the inversion figure
  names, every number of the column is that many scale steps above it, and
  only then is the result respelled to the quality the numeral asked for.
  That is what makes `V7b5` alter one note rather than name a different
  chord, and what gives `[no3]`, `[add4]` and `[#7]` something to mean.
  `over_scale` reads a numeral against a `scale::Scale` rather than a key, so
  a figure means something in a collection no key signature can write; an
  augmented sixth is read as music21 reads it, as figured bass over an
  altered degree, so `Ger6/5` and `It+6` land in the right inversion in any
  key. `written_accidental` and `accidental` are kept apart.
- `figuredbass` is a new module: a column of numbers under a bass note, read
  both as written and expanded to every note the shorthand stands for.
  `Modifier` keeps the mark as written beside the accidental it means,
  because figured bass has marks no accidental name covers. `RomanNumeral`
  reads its own digits with it, so there is one shorthand table and not two.
- scales gained the whole of music21's `ConcreteScale` behaviour that needed
  more than a table: a scale given by its notes (`Scale::from_pitches`,
  filling in the octaves a caller left out), a scale that comes down a
  different way from the way it goes up, `derive_by_degree`,
  `next_pitch_beside` for music21's `getNeighbor`, and the degree machinery a
  gapped scale needs — `named_degrees`, `pitch_on_degree`, `degrees_of_by`
  and `places_of`, since Rag Asawari's five notes stand on degrees 1, 2, 4, 5
  and 6 and Rag Marwa stands one note in two places. `is_realizable` says
  when a collection cannot be walked over a range at all.
- durations carry what they are written as, not only how long they sound:
  components, tuplets, `appendTuplet` and `aggregateTupletMultiplier`,
  `quarterLengthNoTuplets`, the grace durations, and music21's full names for
  a tied, a tuplet and an unwritable length.
- notes and chords carry beams and lyrics as music21 models them.
  `notation::Beam` and `notation::Beams` are music21's `beam` module,
  including the unsaid beam type its `naiveBeams` builds and the list helpers
  that walk a run of notes; a lyric may have several components and may have
  nothing sung to it at all.
- `Volume::realized_with` realizes a loudness against the dynamic in force
  and the articulations' shift, with `realized_str` and `realized_dynamic`
  for music21's `getRealizedStr` and `dynamicStrFromDecimal`.
- `MetronomeMark` says how fast it is played apart from what it says:
  `number_sounding` and `sounding_quarter_bpm` are music21's
  `numberSounding`, which is a `<sound tempo=…>` with no mark beside it or a
  score marked *Allegro* played at a hundred and forty-four.
- `stream` grew enough to hold streams within streams and to answer what is
  in force at an offset.
- a chord, a tone row and a stream are walked the way Rust walks a
  collection: each implements `IntoIterator`, by value and by reference.
- the bundled Scala archive merges two submodules, music21's 3,932 scales and
  hexatone's 62, every one of which parses. a zero-degree file is legal
  rather than malformed, and `bundled_with_failures()` now returns nothing.
- a sieve says more than whether an integer is in it. `Sieve` gains a
  `Display` that writes the expression back out the way music21 does — each
  residual normalized, the groups it was written with kept, so `3@11` reads
  `3@2` and `(5|2)&4&8` reads `{5@0|2@0}&4@0&8@0` — together with music21's
  segment formats (`segment_binary`, `segment_widths`, `segment_unit`),
  `collect` for the first so many members from a starting point, `shifted`,
  and `intersection`, `union` and `symmetric_difference`.
- `chordsymbol::chord_symbol_kind_from_chord` names the kind a figure is
  written with, which is what music21 answers beside the figure when
  `chordSymbolFigureFromChord` is asked to include the chord type.
- the absent octave is readable both of music21 v11's ways —
  `Pitch::implicit_octave` and `octave_is_implicit` — and the default octave
  is read live from music21's `defaults.pitchOctave` where there is a music21
  to ask. `crate::Given` is the argument type that tells a deprecated `None`
  from an argument nobody wrote.

## bug fixes

- pitches built from a name no longer count as having inferred spelling, so
  `Pitch::transpose` keeps `D-` a flat and `KeySignature::new(-7)` is C-flat
  major rather than B major.
- `tuningsystem::C4` is `440 * 2^(-9/12)` to full precision rather than the
  rounded `261.6256`.
- transposing with no accidental limit errors past quadruple accidentals
  instead of silently respelling, as music21 does.
- microtonal intervals survive. `ChromaticInterval` counts fractional
  semitones as music21's does, and `notes_to_chromatic` no longer rounds the
  pitch-space distance away, so the interval between `C1` and a `C1`
  half-sharp is half a semitone rather than one, transposing by it lands on
  `C~1` rather than `C#1`, and `Interval::pythagorean_ratio` reports that a
  quarter tone has no Pythagorean ratio instead of answering `2187/2048`.
- a `PitchOptions` with both a name and an accidental keeps the accidental
  (`name("D").accidental("--")` is `D--`, not `D`), and a pitch built from a
  number stays marked as having inferred spelling, so transposing it
  respells (`Pitch::from_number(6.0)` down a minor second is `F`, not `E#`).
- `Pitch::midi` folds out-of-range pitches into 0 to 127 by octaves and
  rounds half up, as music21 does; `frequency_hz` is computed from A4 = 440
  exactly, so `A4` reads `440.0` rather than `439.99999999999994`.
- transposing keeps the pitch's fundamental, transposed with it, and can
  reach octave -1 (`D2` down a minor 23rd is `C#-1`).
- the pitch-language errors, `transposeAboveTarget`/`transposeBelowTarget`
  and the unknown-accidental errors use music21's wording.
- doubled, tripled and quadrupled specifiers are spelled `Doubly-Diminished`,
  `Triply-Augmented` and so on in interval names, as music21 spells them,
  rather than `Double Diminished`. the specifier fixture now checks the
  names as well as the semitone counts.
- a minor key raises its sixth and seventh where the figure asks, which is
  music21's `_adjustMinorVIandVIIByQuality` at its default setting: `viio7`
  in A minor is `G# B D F`, where it used to keep the accidental as written
  and come out `G B- D- F-`. `VI` and `VII` ask for the natural degrees and
  still get them.
- a microtonal interval says the cents itself. transposition no longer
  carries the cents a pitch came with across an interval measured in
  fractions of a semitone — the interval already says how far from its
  written value the answer sits — so a sieve scale realized in quarter tones
  comes out as pitches rather than as one note two octaves flat.
- a scale realized from an octave-less tonic keeps its octaves:
  `Scale::derive_by_degree` reads the interval to the requested degree off a
  realization in octave 4, where realizing from the octave-less tonic gave
  the degree pitch no octave, made the interval descending and put the tonic
  an octave high.
- `ScaleType::pitch_on_degree` answers `None` for a degree a gapped scale has
  not got, rather than the note that happens to be that many notes along.
- a pitch's chosen spelling is recorded on both halves of it at once, so a
  respelling no longer half-applies.
- a major seventh over a triad that is not major is a major seventh.
  `ChordSymbol::parse` read the `M` of `mM7`, `+M9`, `minmaj11` and `m#7` as
  part of the triad and gave the seventh the triad's own quality, so `F#mM7`
  came out `F# A C# E` instead of `F# A C# E#` and `F+M7` lost its seventh
  altogether. an altered degree now stands in place of the one the chord's
  quality would give rather than sounding beside it. found by running
  music21's own `harmony` doctests against the crate.
- a zero-degree Scala file is read as a scale with no pitches rather than
  rejected, which made `ScalaScale::is_empty` dead by construction;
  `ratio_at` answers `1.0` for one instead of dividing by a zero degree
  count.

## breaking changes

- `ChromaticInterval::semitones`, `directed`, `undirected` and
  `Interval::semitones` return `FloatType` rather than `IntegerType`, and
  `ChromaticInterval::new` takes a `FloatType`. `ChromaticInterval` is no
  longer `Eq` or `Hash`. `mod12`, `simple_directed`, `simple_undirected` and
  `interval_class` still return `IntegerType`, off the rounded count.
- `Chord::inversion` now follows music21: it is the chord step the bass
  occupies above the music21 root, so `C F G` is second inversion and
  `A- C F#` is first inversion with root `F#`, and it is `None` only for an
  empty chord. `Chord::bass` picks the written-lowest pitch, so `B#3` is
  below `C4`.
- `Chord::empty` returns `Chord` rather than `Result<Chord>`; it cannot fail.
- `Note::from_pitch` returns `Note` rather than `Result<Note>`, and
  `From<Pitch>` / `From<&Pitch>` replace the `TryFrom` impls that could not
  fail. `Note::from_number` is new and takes a pitch-space number.
- under the `serde` feature, `Pitch`, `Accidental`, `Microtone`, `Chord` and
  `Note` no longer serialize their fields with a leading underscore
  (`"step"` rather than `"_step"`). data written by 0.3.0 will not
  deserialize unchanged.
- the `test` binary is gone. it was auto-discovered from `src/bin/test.rs`
  and shipped with the crate; its checks are now a unit test.
- `Interval::direction` is the direction of the chromatic interval, as
  music21's is: a diminished unison is `Descending` and an augmented one
  `Ascending`, where both used to read `Oblique` off the generic interval.
- `transpose_pitch_with_options(pitch, true, max_accidental)` honours
  `max_accidental` on the reversed transposition instead of resetting it to
  four.
- `KeySignature::sharps` returns `Option<IntegerType>`, since a
  non-traditional signature has no count of sharps. `altered_pitches`,
  `transpose_pitch_from_c` and `try_as_key` are errors on one.
- `Chord::inversion_name` is music21's `inversionName`: the figured-bass
  number (`53`, `6`, `64`, `7`, `65`, `43`, `42`) as
  `Result<Option<IntegerType>>`, an error for a chord that is neither a triad
  nor carries a seventh. the words it used to return are `inversion_text`.
- `Chord::closed_position` and `semi_closed_position` take
  `leave_redundant_pitches`, and `is_italian_augmented_sixth` takes
  `restrict_doublings`, matching music21's keyword arguments.
- `Duration::duration_type` reports the type a dotted value is dotted from,
  as music21 does: a dotted half is a half with one dot, where it used to be
  no type at all. `Chord::full_name` names the quarter every chord without a
  duration of its own implicitly has.
- `Chord::common_name` names the augmented sixths that share a set class with
  a seventh chord, and prefixes `enharmonic equivalent to` where the spelling
  does not match the name, both as music21 does.
- pitch classes round half to even, as Python does, so the half-sharp `C~` is
  pitch class `0` rather than `1`.
- `Chord::remove` reports a pitch that is not there with music21's message,
  which does not name the pitch.
- `Specifier::parse` reports an unknown quality as
  `Cannot find a match for value: 'x'`, and a zero generic interval as
  `The Zeroth is not an interval`, music21's texts.
- two misnamed tuning systems are renamed, and one duplicate is gone.
  `TuningSystem::JustIntonation` held Wendy Carlos's Harmonic scale, reaching
  the 19-limit, and is now `CarlosHarmonic`; `FiveLimit` is the classical
  12-tone just scale. `Indian` held Ptolemy's intense diatonic, a European
  scale, and is now `PtolemyIntenseDiatonic`. `IndianFull` was a second name
  for the same twenty-two ratios as `Indian22` and is removed.
- `Duration::full_name` returns `String` rather than `Option<String>`: every
  duration has a name, including the tied and tuplet ones it could not write
  before.
- `Volume::realized_str` returns `String` rather than `&'static str`, since
  it is music21's `getRealizedStr` — the number written to two places, not a
  mark from a fixed list. `realized_dynamic` is the mark.
- `Lyric::text` returns `String` and `Lyric::set_number` takes an
  `IntegerType` without a `Result`; `explicit_text` is music21's `.text`,
  which may be `None`, and `clear_text` is the assignment of it.
  `Note::add_lyric` and `Chord::add_lyric` take music21's `number` argument
  beside the text.
- `StemDirection::NoneDirection` is gone and `StemDirection::ALL` holds five
  directions rather than six. `"none"` is an input spelling of `"noStem"`
  that music21's setter rewrites, so no note ever reads back as `"none"`;
  `StemDirection::NAMES` still lists all six spellings the setter takes.
- `ScaleType::pitch_at_degree` takes an `IntegerType` rather than a `usize`,
  matching the degree numbering everything else in `scale` uses.
- `MetronomeMark::set_number` takes an `Option<FloatType>`, since a mark may
  say a word and no number.

## internal

- `python-parity` now builds a pyo3 module, `music21_rs_facade`, of
  music21-shaped `Pitch`, `Accidental` and `Microtone` classes, and a test
  runs the doctests of music21's own `pitch.py` with those classes swapped
  in: 890 of the 920 examples pass, and `python-parity/doctest/pitch.toml`
  pins the passing docstrings. running them turned up the accidental,
  inferred-spelling, MIDI-folding and negative-octave fixes above.
- the same harness covers `interval.py` (759 of 771 examples), `chord.py`
  (916 of 971), `key.py` (230 of 253) and `serial.py` (148 of 149), each with
  a facade module and an expectation file under `python-parity/doctest/`. the
  interval facade is what made the interval halves public and found the
  reversed `maxAccidental` and key-aware transposition fixes;
  `IntervalBaseTrait`, which it replaced, is gone. the chord facade brought
  `note.Note`, `duration.Duration`, `tie.Tie`, `note.Lyric`, `volume.Volume`
  and a colour-only `style.Style` with it, and found the `commonName`,
  `inversionName`, dotted-duration and pitch-class rounding fixes above.
- the facade chord holds its notes as Python objects rather than values, so
  `chord[1].notehead = 'diamond'` sticks and `chord[1] is chord.notes[1]`
  holds. pitches are still values, which is what most of the remaining
  failures come down to: `chord.pitches[0].getEnharmonic(inPlace=True)` does
  not reach the chord. the rest are music21 internals (`_overrides`,
  `chordTablesAddress`, `client`) and stream context.
- `cargo run -p xtask -- report` writes `target/reports`: the library's test
  coverage from `cargo llvm-cov`, and every public method of the music21
  classes named in `data/feature_map.toml` against the crate's `pub fn`s,
  with the deliberate omissions and their reasons. CI publishes it on the
  Pages site under `/reports/` and fails when the map goes stale.
- `Pitch::from_options` is the constructor; the nine-argument positional
  `Pitch::new` and the `IntoAccidental`, `IntoCentShift`, `IntoPitchName` and
  `IntoPitch` traits, half of whose impls were `panic!()` stubs, are gone.
- `Interval::from_name` and `from_semitones` build intervals directly rather
  than through an `IntervalArgument` enum. `IntervalBaseTrait` borrows its
  interval and pitch instead of consuming them, and `Specifier` is `Copy`.
- `interval_to_pythagorean_ratio` no longer keeps a process-wide
  `Mutex<HashMap>` of every ratio it has computed.
- the chord root-finding walk lives once in `chord::root`; `Chord`,
  `chordsymbol` and `roman` share it and its `pitch_class` helper.
- private fields drop their `_` prefix, and `spelling_is_infered` is spelled
  correctly.
- `Note` is a `Pitch` plus an `Option<Duration>`. the `GeneralNote` and
  `NotRest` wrapper structs and `GeneralNoteTrait`, mirrored from music21's
  class hierarchy, are gone; nothing dispatched on them. under `serde` a
  note now serializes as `{"pitch", "duration"}` rather than nesting the
  duration two wrappers deep.
- the doctest harness now covers fifteen of music21's modules — `pitch`,
  `interval`, `chord`, `note`, `duration`, `key`, `serial`, `scale`, `roman`,
  `beam`, `volume`, `tie`, `figuredBass.notation`, `tempo` and
  `chord.tables` — and every one of them passes every one of music21's own
  examples: 590 docstrings and 5,460 examples against music21 11.0.0b9.
  `voiceLeading` is under the harness too and is not there yet.
- music21's own test suite runs against the crate, in CI and in the report.
  `xtask music21-suite` runs every `Test` class and every docstring in every
  music21 module twice — once on music21, once with `install_into_music21()`
  in front of it — and diffs the two, so only a test that fails under the
  crate and not under music21 counts. two divergences are listed by name with
  the reason for each, and a listed one that stops failing fails the run too,
  so the list cannot go stale. `xtask downstream` is the same idea against a
  library written by someone who had never heard of this crate: harte-library
  at a pinned commit, 8,116 tests, run both ways with the failures required
  to match exactly.
- three more of music21's modules are under the doctest harness:
  `music21.sieve` at 24 of 25 docstrings and 106 of 107 examples,
  `music21.harmony` at 22 of 25 and 346 of 361, and `music21.meter.base` at
  6 of 34 and 239 of 387. each replaces less than the whole module on
  purpose, so that music21's own `PitchSieve`, `ChordSymbol` and
  `bestTimeSignature` run over the crate's objects; none of the three is
  installed by `install_into_music21`, since music21's `Sieve` and
  `TimeSignature` carry machinery the crate does not model. what the numbers
  say is not modelled: sieve compression, `MeterSequence`, and the
  augmented-sixth and Neapolitan chord figures.
- nothing that drives Python is written in Python any more. the benchmark,
  music21's suite and the harte-library comparison were `bench.py`,
  `music21_suite.py` and `run.py`; each is now an `xtask` command driving
  music21 through pyo3, ported answer-for-answer and checked against the
  Python it replaces on the same wheel and the same interpreter before that
  was deleted.
- `xtask report` writes the Pages report: coverage, every test suite and
  whether it ran or was skipped and why, the doctest scoreboard, the unit
  tests music21's own suite runs per module, and the ported feature set. its
  layout is a stylesheet and a TypeScript file beside it, compiled and
  verified the way every other generated file here is.
- the Xenharmonic Wiki temperaments, the Scala archive and the tuning tables
  each have the three-stage `regenerate` / `emit` / `verify` shape the chord
  tables have, and `xtask regenerate-all` rebuilds every generated file from
  the submodules and stamps each with the version and commit it read.
- the `music21` submodule moves to 11.0.0b9, which carries this repository's
  own fix to music21's Scala reader (cuthbertLab/music21#2026): 21 archive
  files raised on read and 2 more parsed to the wrong cents, because a
  comment after a degree's value had its digits spliced onto the number.

# music21-rs 0.3.0

this release removes a layer of music21's Python runtime that had been
transliterated into Rust without ever being connected to anything, and fixes
two bugs that were hiding inside it. the minor bump is required because
`fraction` is a public dependency: `FractionType` is re-exported from the
crate root and returned by `Interval::pythagorean_ratio`, so moving to
`fraction` 0.16 is a breaking change for callers who name those types.

## breaking changes

- updated `fraction` to 0.16 and `itertools` to 0.15. `FractionType`
  (`fraction::GenericFraction<IntegerType>`) appears in the public API, so
  downstream crates that mention it must move to `fraction` 0.16 as well.
- `Interval::from_name` and `Interval::new` now return `Error::Interval` for
  a name with no interval number in it, instead of panicking. code that
  relied on the panic (`""`, `"X"`, `"perfect"`) now sees an `Err`.

## bug fixes

- fixed `Chord::set_duration` and `Chord::with_duration`, which silently did
  nothing on any chord that had notes in it. the chord's duration lived
  behind an `Arc<ChordBase>` that every note in the chord also referenced, so
  the `Arc::get_mut` in the setter never succeeded. `Chord::new("C E G")
  .with_duration(Duration::whole())` returned a chord with no duration; only
  the empty chord worked. the same reference cycle leaked each chord's
  internal note list.
- fixed `Interval::from_name` panicking out of a `Result`-returning API on
  malformed interval names.
- `PitchOptions::fundamental` now does something. the value was stored on
  `Pitch` and had no reader anywhere in the crate, so it could be set but
  never observed. added `Pitch::fundamental()`.

## highlights

- added `Pitch::fundamental()`.
- pitch names are no longer formatted out of a `#[derive(Debug)]` impl.
  `Pitch::name` and the ABC and scale helpers now spell the step letter
  explicitly, so the output no longer depends on `StepName`'s variant names.
- transposition no longer clones the interval. `Interval::transpose_pitch`
  and `Pitch::transpose` read the interval through a reference internally
  instead of taking it by value, so a `Chord` or scale walk no longer copies
  a diatonic/chromatic pair per note. public signatures are unchanged.
- `interval_to_pythagorean_ratio` no longer holds its cache lock across the
  circle-of-fifths walk, so concurrent callers queue only on the map access,
  and no longer re-parses `"P5"`/`"-P5"` from strings on every call.
- removed roughly 600 lines of unreachable internals: the `_client` observer
  chains on `Pitch` and `Accidental` (only ever assigned `None`), the
  `HashMap<String, String>` caches on `Note` and `ChordBase` (never read),
  `ChordBase._overrides`, `Pitch._overriden_freq440`, and the `ChordBase` /
  `IntoNotRests` layer that duplicated the note list `Chord` already held.

# music21-rs 0.2.2

this release reworks the tuning-system API around context-dependent tunings
and moves the Python parity suite out of the main workspace.

## breaking changes

- removed the `TuningSystem::StepMethod` and
  `TuningSystem::RecursiveEqualTemperament` variants, both of which computed
  plain equal temperament. `ALL_TUNING_SYSTEMS` is now 15 entries rather
  than 17.

## highlights

- added the `tuningsystem::adaptive` module and `AdaptiveTuningSystem` for
  tunings whose frequencies depend on harmonic context, plus the
  `AnyTuningSystem` enum unifying those with the fixed ratio-table systems,
  with `frequency_at`, `cents_at`, and `is_adaptive`.
- added `TWELVE_TONE_NAMES_SHARP` and `TWELVE_TONE_NAMES_FLAT`.
- improved just intonation ratio accuracy.
- moved the Python parity tests into a separate `python-parity` package
  outside the cargo workspace, so `cargo test` and `cargo check` on the
  library no longer pull in pyo3.
- added `xtask verify-tables`, which CI runs to assert the committed chord
  table source still matches `data/chord_tables.toml`.
- removed the dormant internal `base` and `prebase` modules.

# music21-rs 0.2.1

this patch release fixes a Pythagorean tuning table ordering issue and improves
the Tuning Explorer browser workflow.

## highlights

- fixed the Pythagorean tuning ratios so the twelve-tone chromatic degrees stay
  in ascending frequency order, including `Bb` below `B`.
- added shareable URLs to the Tuning Explorer, preserving the selected tuning
  system, root frequency, and selected degree.
- added a major-scale playback button for twelve-tone tuning systems, with
  nearest-degree suggestions for non-twelve-tone systems.
- updated twelve-tone Tuning Explorer labels to use unambiguous flat spellings
  such as `Bb4`.

# music21-rs 0.2.0

this release continues the browser-demo work from `0.1.x` and cleans up several
pre-1.0 pitch APIs so the Rust surface more closely resembles Python
`music21`.

## breaking API changes

- removed the legacy `PitchAccidental` and `PitchMicrotone` builder wrapper
  types.
- added public `Accidental`, `Microtone`, and `PitchClass` structs with
  companion `AccidentalSpecifier`, `MicrotoneSpecifier`, and
  `PitchClassSpecifier` input enums.
- updated `PitchOptions` and pitch builders to use those specifier types
  directly.

## highlights

- added `Pitch::accidental()`, `Pitch::microtone()`, and
  `Pitch::pitch_class()` accessors.
- added public accidental helpers for names, modifiers, unicode display,
  non-standard values, and display metadata.
- added public microtone helpers for cents, harmonic shifts, and music21-style
  formatting.
- added normalized public pitch-class values with music21-style `A`/`B`
  display for pitch classes 10 and 11.
- added immediate playback when selecting a suggested resolution in the Chord
  Inspector.
- added per-resolution preview buttons that play the current chord followed by
  the suggested resolution without changing the page.
- added hover/focus notation previews for suggested resolutions, showing the
  current chord and hovered resolution side by side on one ABCJS staff.
- added a CI TypeScript build step for the web demos and included generated web
  JavaScript in the GitHub Pages artifact checks.

# music21-rs 0.1.1

this patch release adds a few browser-facing theory workflow improvements on
top of `0.1.0`.

## highlights

- added MIDI-number input support to the Chord Inspector. inputs like
  `60 64 67`, `60,64,67`, and `midi: 60 64 67` analyze as MIDI notes.
- added web MIDI support to the Chord Inspector so a connected MIDI device can
  feed the currently held notes into the analyzer.
- added a MIDI column to the pitch table and changed pitch display spelling from
  music21 flats such as `A-5` to browser-facing names such as `Ab5`.
- added a `Class` help widget in the Chord Inspector pitch table.
- added a Chord Browser at `/chords` listing all 351 unpitched entries in the
  music21-derived chord table, with links back into the inspector.
- expanded the Chord Browser with a root selector, realized pitches, and
  per-inversion inspector links.
- added range filtering to the Chord Browser note-count control.
- listed directed dyad inversions such as major second and minor seventh as
  separate Chord Browser rows, with interval-class labels kept as aliases.
- moved the Chord Browser frontend source to TypeScript, with browser-served
  JavaScript generated from that source.
- added resolution-chord links to Chord Browser rows when the realized chord has
  suggestions.

# music21-rs 0.1.0

this release expands `music21-rs` from a chord-name port into a broader set of
interactive music-theory tools and supporting Rust APIs.

## highlights

- added a browser demo suite published from `examples/web`:
  - chord inspector at `/chord`
  - Polyrhythm Lab at `/polyrhythm`
  - Tuning Explorer at `/tuning`
  - a root index page linking the demos and docs
- added simple chord-resolution suggestions to the Rust `Chord` API and the
  Chord Inspector.
- added chord playback, ABC staff notation, random chord generation, clickable
  keyboard toggles, history controls, shareable URLs, and an "open as
  polyrhythm" bridge to the Chord Inspector.
- added a Polyrhythm Lab with playback, random rhythm settings, shareable URLs,
  history controls, track mute/edit/remove controls, ABC rhythm notation, and
  chord-equivalence links.
- added a Tuning Explorer for the tuning systems exposed by the crate, including
  scale playback and per-degree ratio/frequency/cents data.
- updated CI to build and smoke-check the full browser demo site on every run.
- cleaned up the README and package description to reflect the current crate
  scope.

## notes

- the browser demos share one WASM crate at `examples/web` rather than keeping
  the Rust glue under the chord demo.
- APIs are still pre-1.0 and may continue to change as more of `music21` is
  ported.
