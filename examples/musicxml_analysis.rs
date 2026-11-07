//! Reads a MusicXML score, names its key and the roman numeral of each
//! chord, and writes the score back with the analysis as a part of its own:
//! `cargo run --features musicxml --example musicxml_analysis -- score.musicxml`.
//! Given no file, it analyses a short cadence of its own.
//!
//! In music21 this is `s = converter.parse(path)`, `k = s.analyze('key')`,
//! `chords = s.chordify()`, a `romanNumeralFromChord` lyric on each chord,
//! `s.insert(0, chords)` and `s.write('musicxml')`.

use music21::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use music21::stream::Analysis;
use music21::{Key, RomanNumeral, StreamElement};

/// I IV V7 I in C major: chords over a bass line, in half notes.
const CADENCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0">
  <part-list>
    <score-part id="P1"><part-name>Upper</part-name></score-part>
    <score-part id="P2"><part-name>Bass</part-name></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>G</sign><line>2</line></clef>
      </attributes>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><type>half</type></note>
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><type>half</type></note>
    </measure>
    <measure number="2">
      <note><pitch><step>F</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>D</step><octave>5</octave></pitch><duration>2</duration><type>half</type></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note>
      <note><chord/><pitch><step>C</step><octave>5</octave></pitch><duration>2</duration><type>half</type></note>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes>
        <divisions>1</divisions>
        <key><fifths>0</fifths></key>
        <time><beats>4</beats><beat-type>4</beat-type></time>
        <clef><sign>F</sign><line>4</line></clef>
      </attributes>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>2</duration><type>half</type></note>
      <note><pitch><step>F</step><octave>3</octave></pitch><duration>2</duration><type>half</type></note>
    </measure>
    <measure number="2">
      <note><pitch><step>G</step><octave>3</octave></pitch><duration>2</duration><type>half</type></note>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>2</duration><type>half</type></note>
    </measure>
  </part>
</score-partwise>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(path)?,
        None => CADENCE.to_string(),
    };
    let mut score = from_musicxml(&text)?;

    // `Analysis` is non-exhaustive: another method may answer in a way
    // this example does not know.
    let key = match score.analyze("key")? {
        Some(Analysis::Key(key)) => key,
        _ => Key::from_tonic("C")?,
    };
    eprintln!("key: {} {}", key.tonic_pitch_name_with_case(), key.mode());

    let mut chords = score.chordify()?;
    let mut failure = None;
    chords.for_each_mut(&mut |_, element| {
        let StreamElement::Chord(chord) = element else {
            return;
        };
        let figure = match RomanNumeral::analyze(chord, key.clone()) {
            Ok(Some(numeral)) => numeral.figure().to_string(),
            Ok(None) => "?".to_string(),
            Err(error) => {
                failure.get_or_insert(error);
                return;
            }
        };
        eprintln!("{:<16} {figure}", chord.pitch_names().join(" "));
        if let Err(error) = chord.add_lyric(&figure, None, false) {
            failure.get_or_insert(error);
        }
    });
    if let Some(error) = failure {
        return Err(error.into());
    }

    chords.set_name(Some("Analysis".to_string()));
    score.insert(0.0, chords);
    print!("{}", to_musicxml(&score, &ExportOptions::default())?);
    Ok(())
}
