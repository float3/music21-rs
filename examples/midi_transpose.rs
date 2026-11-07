//! Reads a MIDI file, transposes it by an interval and writes it back:
//! `cargo run --example midi_transpose -- song.mid M2 out.mid`. The
//! interval is named as music21 names one (`M2`, `-P4`), and the output
//! defaults to `transposed.mid`. Given no file, it transposes a short
//! melody of its own and says what came out, writing nothing.
//!
//! In music21 this is `s = converter.parse(path)`,
//! `s.transpose('M2', inPlace=True)` and `s.write('midi', fp=out)`.

use music21::{Interval, Note, Stream, StreamKind, from_midi, midi, to_midi};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next();
    let interval = Interval::from_name(args.next().unwrap_or_else(|| "M2".to_string()))?;
    let output = args.next().unwrap_or_else(|| "transposed.mid".to_string());

    let bytes = match &input {
        Some(path) => std::fs::read(path)?,
        None => melody()?,
    };
    let score = from_midi(&bytes)?;
    let transposed = score.transpose(&interval)?;
    let written = to_midi(&transposed, &midi::ExportOptions::default())?;

    let names = |stream: &Stream| -> Vec<String> {
        let pitches = stream.pitches();
        pitches
            .iter()
            .take(8)
            .map(|p| p.name_with_octave())
            .collect()
    };
    eprintln!("before: {}", names(&score).join(" "));
    eprintln!("after:  {}", names(&transposed).join(" "));
    // MIDI keeps no spelling, so what is read back is respelled.
    eprintln!("read:   {}", names(&from_midi(&written)?).join(" "));

    if input.is_some() {
        std::fs::write(&output, written)?;
        eprintln!("wrote {output}");
    }
    Ok(())
}

/// A D major arpeggio and back, as a MIDI file.
fn melody() -> music21::Result<Vec<u8>> {
    let mut part = Stream::with_kind(StreamKind::Part);
    for name in ["D4", "F#4", "A4", "D5", "C#5", "A4", "F#4", "D4"] {
        part.push(Note::from_name(name)?);
    }
    to_midi(&part, &midi::ExportOptions::default())
}
