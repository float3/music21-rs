//! Scores written by other programs, read into ABC for the editor: Guitar
//! Pro 3 to 8, the per-part JSON Songsterr's player loads, and every format
//! the crate reads -- MusicXML, MIDI, Humdrum, MEI, RomanText and
//! TinyNotation -- which [`stream`] turns from the crate's score. Every
//! reader fills the one [`Imported`] shape below, and [`abc`] turns that
//! into the crate's score for the crate's ABC writer, so a repeat, a tuplet
//! or a tie comes out the same whichever program wrote it.

mod abc;
mod formats;
mod gpif;
mod guitarpro;
mod musicxml;
mod songsterr;
mod stream;
mod zip;

use super::js_error;
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Ticks to a quarter note: every note value down to a 128th and every
/// tuplet of up to thirteen lands on a whole number of them.
const TICKS: i64 = 4_324_320;

/// A score as the readers leave it: parts of measures of voices of beats.
#[derive(Debug, Default)]
struct Imported {
    title: String,
    /// Quarter notes a minute at the start.
    tempo: Option<f64>,
    parts: Vec<Part>,
    /// What was left out, and why.
    skipped: Vec<String>,
}

#[derive(Debug, Default)]
struct Part {
    name: String,
    /// The General MIDI program, from 0.
    program: Option<u8>,
    /// Open strings as MIDI numbers, lowest first; empty for an instrument
    /// without them.
    tuning: Vec<i32>,
    /// The ABC clef the source names, where it names one.
    clef: Option<String>,
    measures: Vec<Measure>,
}

#[derive(Debug, Default, Clone)]
struct Measure {
    /// The time signature, where the source states one.
    meter: Option<(u32, u32)>,
    /// The key signature, where the source states one.
    key: Option<KeySig>,
    repeat_start: bool,
    /// How many times the repeat closing here is played; 0 where none does.
    repeat_end: u32,
    /// The endings of a repeat this measure belongs to.
    ending: Vec<u32>,
    /// A section name: "Intro", "Verse".
    marker: Option<String>,
    voices: Vec<Vec<Beat>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct KeySig {
    sharps: i32,
    minor: bool,
}

/// Notes struck together, or a rest.
#[derive(Debug, Default, Clone)]
struct Beat {
    /// The written length in ticks, dots included and any tuplet not.
    written: i64,
    /// `(actual, normal)`: `actual` of these in the time of `normal`.
    tuplet: Option<(u32, u32)>,
    /// A grace note, which takes no time of its own.
    grace: bool,
    /// None for a rest.
    notes: Vec<Note>,
    chord: Option<String>,
    text: Option<String>,
    /// An analysis written under the beat: a RomanText numeral.
    figure: Option<String>,
}

impl Beat {
    /// How long the beat sounds, in ticks.
    fn length(&self) -> i64 {
        if self.grace {
            return 0;
        }
        match self.tuplet {
            Some((actual, normal)) if actual > 0 => self.written * normal as i64 / actual as i64,
            _ => self.written,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Note {
    midi: i32,
    /// Held over from the same pitch in the beat before.
    tied: bool,
}

/// The ticks of a note value written as its denominator (4 for a quarter,
/// 1 for a whole) with `dots` dots.
fn written_ticks(denominator: u32, dots: u32) -> i64 {
    let base = TICKS * 4 / denominator.max(1) as i64;
    let mut total = base;
    let mut added = base;
    for _ in 0..dots.min(4) {
        added /= 2;
        total += added;
    }
    total
}

/// The ratio a tuplet written with one number stands for: that many notes in
/// the time of the largest power of two below it, and a duplet in the time of
/// three.
fn tuplet_ratio(actual: u32) -> Option<(u32, u32)> {
    match actual {
        0 | 1 => None,
        2 => Some((2, 3)),
        4 => Some((4, 3)),
        _ => Some((actual, 1 << (31 - (actual - 1).leading_zeros()))),
    }
}

/// Text decoded the way the Windows programs of the day wrote it: each byte a
/// character of Latin-1.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| byte as char).collect()
}

/// A text file as UTF-8, or as UTF-16 where it starts with that byte order mark.
fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |little: bool| {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                if little {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xff, 0xfe, ..] => utf16(true),
        [0xfe, 0xff, ..] => utf16(false),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Reads one file, whichever of the formats it is in, by what it holds or,
/// for the plain-text formats that say nothing of themselves, by its name.
fn read_file(name: &str, bytes: &[u8]) -> Result<Imported, String> {
    let lower = name.to_ascii_lowercase();
    let extension = lower
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension);
    if bytes.starts_with(b"MThd") {
        return formats::midi(bytes);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        let archive = zip::Archive::new(bytes)?;
        if let Some(gpif) = archive.read("Content/score.gpif")? {
            return gpif::read_xml(&decode_text(&gpif));
        }
        return musicxml::read_compressed(&archive);
    }
    if bytes.starts_with(b"BCFZ") || bytes.starts_with(b"BCFS") {
        return gpif::read_gpx(bytes);
    }
    if bytes.get(1..19) == Some(b"FICHIER GUITAR PRO") {
        return guitarpro::read(bytes);
    }
    let text = decode_text(bytes);
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        return songsterr::read(trimmed);
    }
    if text.contains("<score-partwise") {
        return musicxml::read(&text);
    }
    if text.contains("<score-timewise") {
        return Err(format!(
            "{name} is timewise MusicXML; save it partwise, as nearly every program does"
        ));
    }
    if text.contains("<GPIF") {
        return gpif::read_xml(&text);
    }
    if text.contains("<mei") {
        return formats::mei(&text);
    }
    if extension == "krn" || text.lines().any(|line| line.starts_with("**")) {
        return formats::humdrum(&text);
    }
    if matches!(extension, "rntxt" | "rntext" | "romantext" | "rtxt") {
        return formats::roman_text(&text);
    }
    if matches!(extension, "tntxt" | "tinynotation")
        || trimmed
            .get(..13)
            .is_some_and(|head| head.eq_ignore_ascii_case("tinynotation:"))
    {
        return formats::tiny_notation(&text);
    }
    Err(format!(
        "{name} is not a Guitar Pro, MusicXML, MIDI, Humdrum, MEI, RomanText, \
         TinyNotation or Songsterr file this editor can read"
    ))
}

// ---------------------------------------------------------------- bindings

/// What an import hands the editor.
#[derive(Serialize)]
struct Import {
    abc: String,
    title: String,
    /// What was left out, and why.
    skipped: Vec<String>,
    /// The open strings of the first part that has them, lowest first.
    tuning: Option<Vec<i32>>,
}

#[wasm_bindgen]
#[derive(Debug, Default)]
/// A score read from one file or more, written out as ABC. A Songsterr track
/// is one part to a file, so the files of one song are added together.
pub struct ScoreImport {
    imported: Imported,
}

#[wasm_bindgen]
impl ScoreImport {
    #[wasm_bindgen(constructor)]
    /// An import with nothing read yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a Guitar Pro (`.gp3`, `.gp4`, `.gp5`, `.gpx`, `.gp`), MusicXML
    /// (`.musicxml`, `.xml`, `.mxl`), MIDI (`.mid`, `.midi`), Humdrum
    /// (`.krn`), MEI (`.mei`), RomanText (`.rntxt`), TinyNotation (`.tntxt`)
    /// or Songsterr JSON file into the score, adding its parts after those
    /// already read.
    pub fn add(&mut self, name: &str, bytes: &[u8]) -> Result<(), JsValue> {
        let read = read_file(name, bytes).map_err(|err| js_error(format!("{name}: {err}")))?;
        if self.imported.title.is_empty() {
            self.imported.title = read.title;
        }
        self.imported.tempo = self.imported.tempo.or(read.tempo);
        self.imported.parts.extend(read.parts);
        self.imported.skipped.extend(read.skipped);
        Ok(())
    }

    /// The score read so far, as ABC.
    pub fn finish(&self) -> Result<JsValue, JsValue> {
        let abc = abc::to_abc(&self.imported).map_err(js_error)?;
        let import = Import {
            abc,
            title: self.imported.title.clone(),
            skipped: self.imported.skipped.clone(),
            tuning: self
                .imported
                .parts
                .iter()
                .find(|part| !part.tuning.is_empty())
                .map(|part| part.tuning.clone()),
        };
        serde_wasm_bindgen::to_value(&import).map_err(js_error)
    }
}

#[cfg(test)]
mod tests;
