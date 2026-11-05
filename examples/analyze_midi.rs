//! Reads a MIDI file and says what is in it:
//! `cargo run --example analyze_midi -- song.mid`. Given no file, it writes
//! a short melody as MIDI in memory and reads that back.

use music21::stream::Analysis;
use music21::{Note, Stream, StreamKind, from_midi, midi, to_midi};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = match std::env::args().nth(1) {
        Some(path) => std::fs::read(path)?,
        None => melody()?,
    };

    let score = from_midi(&bytes)?;
    println!(
        "{} parts, {} notes and chords, {} quarter notes long",
        score.parts().len(),
        score.notes().len(),
        score.end_offset()
    );
    if let Some(Analysis::Key(key)) = score.analyze("key")? {
        println!("key: {} {}", key.tonic_pitch_name_with_case(), key.mode());
    }
    if let Some(Analysis::Ambitus(range)) = score.analyze("ambitus")? {
        println!("range: {}", range.name());
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
