A music21 user's guide to the crate: the Python idioms you know, and the
calls that do the same here.

The names follow music21's, in Rust's case: `pitchedCommonName` is
[`Chord::pitched_common_name`](crate::Chord::pitched_common_name),
`makeNotation` is [`make_notation`](crate::makenotation::make_notation).
Every fallible call returns [`Result`](crate::Result), so the examples end
in `?`. A flat may be written `b` or, as music21 writes it, `-`; the crate
answers with `b`.

# Pitches, notes and chords

| music21 | music21 crate |
| --- | --- |
| `pitch.Pitch('C#4')` | `Pitch::from_name("C#4")?` |
| `p.nameWithOctave`, `p.midi` | `p.name_with_octave()`, `p.midi()` |
| `p.transpose('M3')` | `p.transpose(&Interval::from_name("M3")?)?` |
| `note.Note('E-4', quarterLength=2)` | `Note::from_name("Eb4")?.with_duration(Duration::half())` |
| `chord.Chord('C E G')` | `Chord::new("C E G")?` or `"C E G".parse::<Chord>()?` |
| `c.commonName`, `c.root()` | `c.common_name()`, `c.root()` |
| `interval.Interval('P5')` | `Interval::from_name("P5")?` |

```
use music21::{Chord, Duration, Interval, Note, Pitch};

let pitch = Pitch::from_name("C#4")?;
assert_eq!(pitch.midi(), 61);
let third = Interval::from_name("M3")?;
assert_eq!(pitch.transpose(&third)?.name_with_octave(), "E#4");

let note = Note::from_name("E-4")?.with_duration(Duration::half());
assert_eq!(note.pitch().name(), "Eb");
assert_eq!(note.duration().map(|length| length.quarter_length()), Some(2.0));

let chord = Chord::new("G3 B3 D4 F4")?;
assert_eq!(chord.common_name(), "dominant seventh chord");
assert_eq!(chord.root().map(|root| root.name()), Some("G".to_string()));
assert!(chord.is_dominant_seventh());
assert_eq!(chord.inversion(), Some(0));
# Ok::<(), music21::Error>(())
```

# Streams

A [`Stream`](crate::Stream) is music21's stream: elements at offsets,
nested as scores, parts, measures and voices. Its kind is a
[`StreamKind`](crate::StreamKind) rather than a subclass.

| music21 | music21 crate |
| --- | --- |
| `stream.Stream()`, `stream.Part()` | `Stream::new()`, `Stream::with_kind(StreamKind::Part)` |
| `s.append(n)` | `s.push(n)` |
| `s.insert(2.0, n)` | `s.insert(2.0, n)` |
| `s.flatten()`, `s.recurse()` | `s.flatten()`, `s.recurse()` |
| `s.notes`, `s.pitches` | `s.notes()`, `s.pitches()` |
| `s.parts`, `s.getElementsByClass(stream.Measure)` | `s.parts()`, `s.measures()` |
| `s.highestTime` | `s.end_offset()` |
| `s.chordify()` | `s.chordify()?` |
| `s.transpose('P4')` | `s.transpose(&Interval::from_name("P4")?)?` |
| `converter.parse('tinyNotation: 4/4 c4 d e f')` | `tinynotation::from_tiny_notation("4/4 c4 d e f")?` |

```
use music21::{Interval, Note, Stream, StreamElement, StreamKind};

let mut upper = Stream::with_kind(StreamKind::Part);
let mut lower = Stream::with_kind(StreamKind::Part);
for (high, low) in [("E4", "C3"), ("F4", "D3"), ("G4", "E3")] {
    upper.push(Note::from_name(high)?);
    lower.push(Note::from_name(low)?);
}
let mut score = Stream::with_kind(StreamKind::Score);
score.insert(0.0, upper);
score.insert(0.0, lower);

assert_eq!(score.parts().len(), 2);
assert_eq!(score.flatten().notes().len(), 6);
assert_eq!(score.end_offset(), 3.0);

let chords = score.chordify()?;
let (offset, first) = &chords.flatten().notes()[0];
assert_eq!(*offset, 0.0);
let StreamElement::Chord(chord) = first else {
    panic!("chordify makes chords");
};
assert_eq!(chord.pitch_names(), ["C", "E"]);

let up = score.transpose(&Interval::from_name("P4")?)?;
assert_eq!(up.parts()[0].pitches()[0].name_with_octave(), "A4");
# Ok::<(), music21::Error>(())
```

Elements come back as [`StreamElement`](crate::StreamElement), an enum to
match on where music21 would check `isinstance`. It is
`#[non_exhaustive]`, so a `match` needs a wildcard arm.

# Reading and writing files

`converter.parse` is one function per format, each taking the file's
contents rather than its path; `s.write(fmt)` likewise. Reading needs no
feature except for MusicXML.

| format | read | write |
| --- | --- | --- |
| MusicXML (`musicxml` feature) | `musicxml::from_musicxml` | `musicxml::to_musicxml` |
| MIDI | [`midi::from_midi`](crate::midi::from_midi) | [`midi::to_midi`](crate::midi::to_midi) |
| ABC | [`abc::from_abc`](crate::abc::from_abc) | [`abc::to_abc`](crate::abc::to_abc) |
| MEI | [`mei::from_mei`](crate::mei::from_mei) | |
| Humdrum | [`humdrum::from_humdrum`](crate::humdrum::from_humdrum) | |
| RomanText | [`romantext::from_roman_text`](crate::romantext::from_roman_text) | [`romantext::to_roman_text`](crate::romantext::to_roman_text) |
| LilyPond | | [`lily::to_lilypond`](crate::lily::to_lilypond) |

```
use music21::{from_abc, from_midi, midi, to_midi};

let tune = from_abc("X:1\nT:Scale\nM:4/4\nL:1/4\nK:C\nCDEF|GABc|]\n")?;
assert_eq!(tune.flatten().notes().len(), 8);

let bytes = to_midi(&tune, &midi::ExportOptions::default())?;
let back = from_midi(&bytes)?;
assert_eq!(back.pitches()[7].name_with_octave(), "C5");
# Ok::<(), music21::Error>(())
```

A file on disk is read with `std::fs` first:

```no_run
# #[cfg(feature = "musicxml")] {
use music21::musicxml::{ExportOptions, from_musicxml, to_musicxml};

let text = std::fs::read_to_string("bwv66.6.musicxml")?;
let score = from_musicxml(&text)?;
std::fs::write("copy.musicxml", to_musicxml(&score, &ExportOptions::default())?)?;
# }
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Keys and roman numerals

| music21 | music21 crate |
| --- | --- |
| `key.Key('c')` | `Key::from_tonic("c")?` |
| `key.KeySignature(-3)` | `KeySignature::new(-3)` |
| `s.analyze('key')` | `s.analyze("key")?`, a [`stream::Analysis`](crate::stream::Analysis) |
| `roman.RomanNumeral('V7', k)` | `RomanNumeral::new("V7", k)?` |
| `roman.romanNumeralFromChord(c, k)` | `RomanNumeral::analyze(&c, k)?` |

```
use music21::stream::Analysis;
use music21::{Chord, Key, RomanNumeral, Stream};

let mut progression = Stream::new();
for notes in ["C4 E4 G4", "F4 A4 C5", "G3 B3 D4 F4", "C4 E4 G4"] {
    progression.push(Chord::new(notes)?);
}
let Some(Analysis::Key(key)) = progression.analyze("key")? else {
    panic!("a key analysis names a key");
};
assert_eq!(key.tonic_pitch_name_with_case(), "C");

let numeral = RomanNumeral::analyze(&Chord::new("G3 B3 D4 F4")?, key.clone())?;
assert_eq!(numeral.map(|n| n.figure().to_string()), Some("V7".to_string()));

let realized = RomanNumeral::new("ii6", Key::from_tonic("C")?)?.to_chord()?;
assert_eq!(realized.pitch_names(), ["F", "A", "D"]);
# Ok::<(), music21::Error>(())
```

Windowed and floating key analysis are in
[`analysis::windowed`](crate::analysis::windowed) and
[`analysis::floating_key`](crate::analysis::floating_key).

# Notation a score leaves unsaid

music21's `makeNotation`, which its writers run by default, is
[`makenotation::make_notation`](crate::makenotation::make_notation), in
place. Its pieces (`make_measures`, `make_ties`, `make_rests`,
`make_beams`, `make_accidentals`) are in the same module. The MusicXML
writer runs it only when asked: set `ExportOptions::make_notation`.

```
use music21::makenotation::make_notation;
use music21::{Duration, Note, Stream, StreamKind, TimeSignature};

let mut part = Stream::with_kind(StreamKind::Part);
part.insert(0.0, TimeSignature::new(3, 4)?);
for name in ["C4", "D4", "E4", "F4"] {
    part.push(Note::from_name(name)?.with_duration(Duration::half()));
}
make_notation(&mut part)?;
assert_eq!(part.measures().len(), 3);
# Ok::<(), music21::Error>(())
```

# Features

No feature is on by default. Each one adds code or data the rest of the
crate never needs.

| feature | adds | costs |
| --- | --- | --- |
| `musicxml` | MusicXML reading and writing | about 9,000 lines to compile |
| `scala-archive` | `ScalaArchive::bundled()`, the Scala archive's ~4,000 scales | about 1 MB of data in the binary |
| `language-detection` | `features::language` and music21's `TX1` feature | about 1.9 MB of excerpts in the binary |
| `musescore` | converting through an installed MuseScore; turns on `musicxml` | starts a program and touches files |
| `serde` | `Serialize` and `Deserialize` on the public types | the `serde` dependency |

MuseScore's `.mscx` is read without any feature, by
[`musescore::from_mscx`](crate::musescore::from_mscx).

# Where the crate differs from music21

- **A stream holds values, and nothing knows where it stands.** There are
  no sites: a note has no `offset`, `activeSite`, `beat` or
  `measureNumber`, and the same note pushed into two streams is two copies.
  Where an element stands is the stream's to say:
  [`Stream::placed`](crate::Stream::placed) gives every element with its
  offset, measure, beat and the key, meter and tempo in force there, which
  is what music21 finds by `getContextByClass`.
- **Nothing is cached.** Asking a chord its root twice works it out twice;
  each answer is cheap enough that this costs less than music21's cache.
- **What cannot be answered is refused.** A bad name, a negative length or
  a construct a reader does not handle comes back as an
  [`Error`](crate::Error) rather than a guess, and nothing panics on bad
  input.
- **Nothing answers at random.** Where music21 picks at random, the crate
  follows a fixed rule, or takes a seed or a chooser.
- **Spelling.** A flat is written `b` (`Bb4`), and every pitch has an
  accidental, a natural where none is written.
- **The caller brings the regular expression engine** for metadata
  searches: `MetadataQuery::Matcher` takes a predicate.
- **Not ported:** the corpus, plotting, live score following and
  recording from a microphone.

```
use music21::{Note, Stream, StreamElement, StreamKind, TimeSignature};

let mut part = Stream::with_kind(StreamKind::Part);
part.insert(0.0, TimeSignature::new(3, 4)?);
for name in ["C4", "D4", "E4", "F4"] {
    part.push(Note::from_name(name)?);
}
let placed = part.placed();
let fourth = placed
    .iter()
    .filter(|placed| matches!(placed.element(), StreamElement::Note(_)))
    .nth(3)
    .unwrap();
assert_eq!(fourth.offset(), 3.0);
assert_eq!(fourth.beat()?, Some(1.0));

assert!(Note::from_name("H4").is_err());
# Ok::<(), music21::Error>(())
```
