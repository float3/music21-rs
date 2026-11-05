//! Names each chord of a progression as a roman numeral in the key the
//! progression is heard in: `cargo run --example roman_analysis`.

use music21::stream::Analysis;
use music21::{Chord, Key, RomanNumeral, Stream, StreamElement};

fn main() -> music21::Result<()> {
    let mut progression = Stream::new();
    for notes in [
        "C4 E4 G4",
        "A3 C4 F4",
        "D4 F4 A4",
        "G3 B3 D4 F4",
        "C4 E4 G4",
    ] {
        progression.push(Chord::new(notes)?);
    }

    // `Analysis` is non-exhaustive: another method may answer in a way
    // this example does not know.
    let key = match progression.analyze("key")? {
        Some(Analysis::Key(key)) => key,
        _ => Key::from_tonic("C")?,
    };
    println!("in {} {}", key.tonic_pitch_name_with_case(), key.mode());

    for (offset, element) in progression.notes() {
        let StreamElement::Chord(chord) = element else {
            continue;
        };
        let figure = RomanNumeral::analyze(&chord, key.clone())?
            .map_or_else(|| "?".to_string(), |numeral| numeral.figure().to_string());
        println!(
            "{offset:>4}  {:<12} {figure}",
            chord.pitch_names().join(" ")
        );
    }
    Ok(())
}
