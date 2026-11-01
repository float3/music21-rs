//! Reads an ABC tune and writes it as MusicXML:
//! `cargo run --features musicxml --example abc_to_musicxml -- tune.abc`.
//! Given no file, it converts a short tune of its own.

use music21_rs::from_abc;
use music21_rs::musicxml::{ExportOptions, to_musicxml};

const TUNE: &str = "X:1
T:A Short Tune
M:3/4
L:1/8
K:G
GA B2 d2 | c2 A2 F2 | G6 |]
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(path)?,
        None => TUNE.to_string(),
    };

    let score = from_abc(&text)?;
    let xml = to_musicxml(&score, &ExportOptions::default())?;
    print!("{xml}");
    Ok(())
}
