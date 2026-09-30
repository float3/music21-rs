//! Scores written by other programs, read into ABC for the editor: Guitar
//! Pro 3 to 8, MusicXML and the per-part JSON Songsterr's player loads.
//! Every reader fills the one [`Imported`] shape below and one writer turns
//! it into ABC, so a repeat, a tuplet or a tie comes out the same whichever
//! program wrote it.

mod gpif;
mod guitarpro;
mod musicxml;
mod songsterr;
mod zip;

use super::{abc_key_name, abc_pitch, js_error, key_alters, spell_in_key};
use music21_rs::{Key, KeySignature, Pitch, abc_duration, estimate_key_from_pitches};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};
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

/// Reads one file, whichever of the formats it is in, by what it holds.
fn read_file(name: &str, bytes: &[u8]) -> Result<Imported, String> {
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
    Err(format!(
        "{name} is not a Guitar Pro, MusicXML or Songsterr file this editor can read"
    ))
}

// ------------------------------------------------------------------ writer

/// A part's voice as it is written: which part and voice it is, its id and
/// its staff.
struct AbcVoice<'a> {
    part: &'a Part,
    voice: usize,
    id: String,
    clef: String,
    /// The part's first voice, which carries its name.
    primary: bool,
}

/// What every voice's measure shares: the meter and key in force and how the
/// bar is closed.
struct Frame {
    meter: (u32, u32),
    key: KeySig,
    meter_changed: bool,
    key_changed: bool,
    /// The length every voice is filled to, in ticks.
    target: i64,
}

fn has_notes(part: &Part, voice: usize) -> bool {
    part.measures.iter().any(|measure| {
        measure
            .voices
            .get(voice)
            .is_some_and(|beats| beats.iter().any(|beat| !beat.notes.is_empty()))
    })
}

/// The clef a part is written in: the one its source names, a guitar's or a
/// bass's an octave above where they sound, otherwise by where it lies.
fn clef_of(part: &Part) -> String {
    if let Some(clef) = &part.clef {
        return clef.clone();
    }
    let fretted = match part.program {
        Some(program) => (24..=39).contains(&program),
        None => !part.tuning.is_empty(),
    };
    if fretted {
        let bass = part
            .program
            .is_some_and(|program| (32..=39).contains(&program))
            || part.tuning.first().is_some_and(|&lowest| lowest < 36);
        return if bass { "bass-8" } else { "treble-8" }.to_string();
    }
    let pitches: Vec<i32> = part
        .measures
        .iter()
        .flat_map(|measure| measure.voices.iter().flatten())
        .flat_map(|beat| beat.notes.iter().map(|note| note.midi))
        .collect();
    let mean = pitches.iter().sum::<i32>() as f64 / pitches.len().max(1) as f64;
    if mean < 57.0 { "bass" } else { "treble" }.to_string()
}

fn key_of(key: KeySig) -> Result<Key, String> {
    KeySignature::new(key.sharps.clamp(-7, 7))
        .try_as_key(Some(if key.minor { "minor" } else { "major" }), None)
        .map_err(|err| err.to_string())
}

/// A length in ticks as an ABC length against `L:1/8`.
fn abc_length(ticks: i64) -> Result<String, String> {
    let eighth = TICKS / 2;
    let divisor = gcd(ticks.max(1), eighth);
    abc_duration((ticks.max(1) / divisor) as u32, (eighth / divisor) as u32)
        .map_err(|err| err.to_string())
}

fn gcd(mut left: i64, mut right: i64) -> i64 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left.abs().max(1)
}

/// Rests filling `ticks`, each a plain note value from a whole to a 64th;
/// what is left under a 64th, the crumbs of an irregular tuplet, is dropped,
/// since no value could be written for it.
fn rests(ticks: i64, symbol: char) -> Result<String, String> {
    let mut left = ticks;
    let mut out = String::new();
    while let Some(piece) = (0..7)
        .map(|shift| (TICKS * 4) >> shift)
        .find(|&value| value <= left)
    {
        let _ = write!(out, "{symbol}{} ", abc_length(piece)?);
        left -= piece;
    }
    Ok(out)
}

/// Text safe inside an ABC annotation or field.
fn quoted(text: &str) -> String {
    text.replace(['"', '\n', '\r'], " ").trim().to_string()
}

/// The endings that open at `index`, written as ABC's `[1,2`.
fn opening_ending(frames: &[Measure], index: usize) -> Option<String> {
    let ending = &frames.get(index)?.ending;
    if ending.is_empty() || (index > 0 && frames[index - 1].ending == *ending) {
        return None;
    }
    let numbers: Vec<String> = ending.iter().map(u32::to_string).collect();
    Some(format!("[{}", numbers.join(",")))
}

/// The barline closing measure `index`, with the ending the next one opens.
fn closing_bar(frames: &[Measure], index: usize) -> String {
    let this = &frames[index];
    let next = frames.get(index + 1);
    let opens = next.is_some_and(|next| next.repeat_start);
    let mut bar = match (this.repeat_end > 0, opens) {
        (true, true) => "::".to_string(),
        (true, false) => ":|".to_string(),
        (false, true) => "|:".to_string(),
        (false, false) if next.is_none() => "|]".to_string(),
        (false, false)
            if !this.ending.is_empty() && next.is_some_and(|next| next.ending.is_empty()) =>
        {
            "||".to_string()
        }
        _ => "|".to_string(),
    };
    if let Some(ending) = opening_ending(frames, index + 1) {
        bar.push_str(&ending);
    }
    bar
}

/// The notes of each beat of a voice tied on into the next beat, by measure,
/// beat and note index.
fn ties_out(part: &Part, voice: usize) -> BTreeSet<(usize, usize, usize)> {
    let mut sounding: Vec<(usize, usize)> = Vec::new();
    for (m, measure) in part.measures.iter().enumerate() {
        if let Some(beats) = measure.voices.get(voice) {
            for (b, beat) in beats.iter().enumerate() {
                if !beat.grace {
                    sounding.push((m, b));
                }
            }
        }
    }
    let beat = |(m, b): (usize, usize)| &part.measures[m].voices[voice][b];
    let mut ties = BTreeSet::new();
    for pair in sounding.windows(2) {
        let (from, to) = (beat(pair[0]), beat(pair[1]));
        for (n, note) in from.notes.iter().enumerate() {
            if to
                .notes
                .iter()
                .any(|next| next.tied && next.midi == note.midi)
            {
                ties.insert((pair[0].0, pair[0].1, n));
            }
        }
    }
    ties
}

struct Speller {
    key: Key,
    scale: Vec<Pitch>,
    alters: BTreeMap<char, i32>,
}

impl Speller {
    fn new(key: KeySig) -> Result<Self, String> {
        let key = key_of(key)?;
        let scale = key.pitches().map_err(|err| err.to_string())?;
        let alters = key_alters(key.sharps());
        Ok(Self { key, scale, alters })
    }

    fn write(
        &self,
        midi: i32,
        in_force: &mut BTreeMap<(char, i32), i32>,
    ) -> Result<String, String> {
        let pitch = spell_in_key(midi, &self.key, &self.scale).map_err(|err| format!("{err:?}"))?;
        abc_pitch(&pitch, &self.alters, in_force).map_err(|err| format!("{err:?}"))
    }
}

/// How a note of the beat at `position` in the bar begins: beamed to the one
/// before it or not. Notes shorter than a quarter beam within each beat.
fn beam_breaks(position: i64, beat: &Beat, previous: Option<&Beat>, beat_unit: i64) -> bool {
    let short = |beat: &Beat| !beat.notes.is_empty() && beat.written < TICKS;
    position % beat_unit == 0 || !short(beat) || !previous.is_some_and(short)
}

/// Writes one voice's measure.
#[allow(clippy::too_many_arguments)]
fn write_measure(
    out: &mut String,
    voice: &AbcVoice<'_>,
    index: usize,
    frames: &[Measure],
    frame: &Frame,
    speller: &Speller,
    ties: &BTreeSet<(usize, usize, usize)>,
    lead: bool,
) -> Result<(), String> {
    let beats: &[Beat] = voice
        .part
        .measures
        .get(index)
        .and_then(|measure| measure.voices.get(voice.voice))
        .map_or(&[], Vec::as_slice);
    let (numerator, denominator) = frame.meter;
    let beat_unit = if denominator == 8 && numerator % 3 == 0 {
        TICKS * 3 / 2
    } else {
        TICKS * 4 / denominator.max(1) as i64
    };
    if frame.meter_changed && index > 0 {
        let _ = write!(out, " [M:{numerator}/{denominator}]");
    }
    if frame.key_changed && index > 0 {
        let _ = write!(
            out,
            " [K:{} clef={}]",
            abc_key_name(&speller.key),
            voice.clef
        );
    }
    let mut labels = Vec::new();
    if lead {
        if let Some(marker) = &frames[index].marker {
            labels.push(format!("\"^{}\"", quoted(marker)));
        }
        let plays = frames[index].repeat_end;
        if plays > 2 {
            labels.push(format!("\"^×{plays}\""));
        }
    }

    let mut in_force = BTreeMap::new();
    let mut position = 0_i64;
    let mut tuplet_left = 0_usize;
    let mut previous: Option<&Beat> = None;
    let mut graces = String::new();
    for (b, beat) in beats.iter().enumerate() {
        if beat.grace {
            if let Some(note) = beat.notes.iter().max_by_key(|note| note.midi) {
                graces.push_str(&speller.write(note.midi, &mut in_force)?);
                graces.push_str(&abc_length(beat.written)?);
            }
            continue;
        }
        if beam_breaks(position, beat, previous, beat_unit)
            || tuplet_left == 0 && beat.tuplet.is_some()
        {
            out.push(' ');
        }
        if tuplet_left == 0
            && let Some((actual, normal)) = beat.tuplet
        {
            let count = tuplet_group(&beats[b..], (actual, normal));
            tuplet_left = count;
            let _ = write!(out, "({actual}:{normal}:{count}");
        }
        for label in labels.drain(..) {
            out.push_str(&label);
        }
        if let Some(chord) = &beat.chord {
            let _ = write!(out, "\"{}\"", quoted(chord));
        }
        if let Some(text) = &beat.text {
            let _ = write!(out, "\"^{}\"", quoted(text));
        }
        if !graces.is_empty() {
            let _ = write!(out, "{{{graces}}}");
            graces.clear();
        }
        let length = abc_length(beat.written)?;
        if beat.notes.is_empty() {
            let _ = write!(out, "z{length}");
        } else {
            let mut notes: Vec<(usize, i32)> = beat
                .notes
                .iter()
                .map(|note| note.midi)
                .enumerate()
                .collect();
            notes.sort_by_key(|&(_, midi)| midi);
            notes.dedup_by_key(|(_, midi)| *midi);
            let tied: Vec<bool> = notes
                .iter()
                .map(|&(n, _)| ties.contains(&(index, b, n)))
                .collect();
            let written = notes
                .iter()
                .map(|&(_, midi)| speller.write(midi, &mut in_force))
                .collect::<Result<Vec<_>, _>>()?;
            if written.len() == 1 {
                let _ = write!(
                    out,
                    "{}{length}{}",
                    written[0],
                    if tied[0] { "-" } else { "" }
                );
            } else if tied.iter().all(|&tie| tie) {
                let _ = write!(out, "[{}]{length}-", written.concat());
            } else {
                out.push('[');
                for (note, tie) in written.iter().zip(&tied) {
                    let _ = write!(out, "{note}{}", if *tie { "-" } else { "" });
                }
                let _ = write!(out, "]{length}");
            }
        }
        tuplet_left = tuplet_left.saturating_sub(1);
        position += beat.length();
        previous = Some(beat);
    }
    if position < frame.target {
        out.push(' ');
        for label in labels.drain(..) {
            out.push_str(&label);
        }
        out.push_str(
            rests(
                frame.target - position,
                if voice.primary { 'z' } else { 'x' },
            )?
            .trim_end(),
        );
    }
    Ok(())
}

/// How many beats from the start of `beats` one tuplet bracket holds: those
/// with the same ratio, until they add up to whole groups of the shortest.
fn tuplet_group(beats: &[Beat], ratio: (u32, u32)) -> usize {
    let mut sum = 0_i64;
    let mut shortest = i64::MAX;
    let mut count = 0;
    for beat in beats.iter().filter(|beat| !beat.grace) {
        if beat.tuplet != Some(ratio) {
            break;
        }
        sum += beat.written;
        shortest = shortest.min(beat.written);
        count += 1;
        if sum % (ratio.0 as i64 * shortest) == 0 {
            break;
        }
    }
    count.max(1)
}

/// The key the notes suggest, for a score that states none.
fn estimated_key(parts: &[&Part]) -> KeySig {
    let pitches: Vec<Pitch> = parts
        .iter()
        .flat_map(|part| part.measures.iter())
        .flat_map(|measure| measure.voices.iter().flatten())
        .flat_map(|beat| beat.notes.iter())
        .filter_map(|note| Pitch::from_midi(note.midi).ok())
        .collect();
    estimate_key_from_pitches(&pitches)
        .ok()
        .and_then(|estimates| estimates.first().map(|estimate| estimate.key().clone()))
        .map_or(
            KeySig {
                sharps: 0,
                minor: false,
            },
            |key| KeySig {
                sharps: key.sharps(),
                minor: key.mode() == "minor",
            },
        )
}

fn to_abc(imported: &Imported) -> Result<String, String> {
    let parts: Vec<&Part> = imported
        .parts
        .iter()
        .filter(|part| {
            (0..part
                .measures
                .iter()
                .map(|m| m.voices.len())
                .max()
                .unwrap_or(0))
                .any(|v| has_notes(part, v))
        })
        .collect();
    let Some(lead) = parts.first() else {
        return Err(if imported.skipped.is_empty() {
            "there are no notes to import".to_string()
        } else {
            format!(
                "there are no notes to import; left out: {}",
                imported.skipped.join(", ")
            )
        });
    };
    let count = parts
        .iter()
        .map(|part| part.measures.len())
        .max()
        .unwrap_or(0);
    let frames: Vec<Measure> = (0..count)
        .map(|index| {
            let mut frame = lead.measures.get(index).cloned().unwrap_or_default();
            frame.voices.clear();
            frame
        })
        .collect();

    let mut voices = Vec::new();
    for (p, part) in parts.iter().enumerate() {
        let clef = clef_of(part);
        let widest = part
            .measures
            .iter()
            .map(|measure| measure.voices.len())
            .max()
            .unwrap_or(0);
        let mut first = true;
        for voice in (0..widest).filter(|&voice| has_notes(part, voice)) {
            let id = if first {
                format!("P{}", p + 1)
            } else {
                format!("P{}v{}", p + 1, voice + 1)
            };
            voices.push(AbcVoice {
                part,
                voice,
                id,
                clef: clef.clone(),
                primary: first,
            });
            first = false;
        }
    }

    let first_key = frames
        .first()
        .and_then(|frame| frame.key)
        .unwrap_or_else(|| estimated_key(&parts));
    let mut layout = Vec::with_capacity(count);
    let (mut meter, mut key) = ((4, 4), first_key);
    for (index, frame) in frames.iter().enumerate() {
        let next_meter = frame.meter.unwrap_or(meter);
        let next_key = frame.key.unwrap_or(key);
        let bar = next_meter.0 as i64 * 4 * TICKS / next_meter.1.max(1) as i64;
        let content = voices
            .iter()
            .filter_map(|voice| voice.part.measures.get(index)?.voices.get(voice.voice))
            .map(|beats| beats.iter().map(Beat::length).sum::<i64>())
            .max()
            .unwrap_or(0);
        let partial = content > 0 && content < bar && (index == 0 || index + 1 == count);
        layout.push(Frame {
            meter: next_meter,
            key: next_key,
            meter_changed: index == 0 || next_meter != meter,
            key_changed: index > 0 && next_key != key,
            target: if partial { content } else { bar.max(content) },
        });
        (meter, key) = (next_meter, next_key);
    }

    let mut abc = String::new();
    let title = if imported.title.trim().is_empty() {
        "Imported score"
    } else {
        imported.title.trim()
    };
    let _ = writeln!(abc, "X:1\nT:{}", quoted(title));
    let (numerator, denominator) = layout.first().map_or((4, 4), |frame| frame.meter);
    let _ = writeln!(abc, "M:{numerator}/{denominator}\nL:1/8");
    let _ = writeln!(
        abc,
        "Q:1/4={}",
        imported.tempo.unwrap_or(120.0).round().max(1.0)
    );
    let _ = writeln!(abc, "K:{}", abc_key_name(&key_of(first_key)?));
    let shared = parts.iter().any(|part| {
        voices
            .iter()
            .filter(|voice| std::ptr::eq(voice.part, *part))
            .count()
            > 1
    });
    if shared {
        let groups: Vec<String> = parts
            .iter()
            .map(|part| {
                let ids: Vec<&str> = voices
                    .iter()
                    .filter(|voice| std::ptr::eq(voice.part, *part))
                    .map(|voice| voice.id.as_str())
                    .collect();
                if ids.len() > 1 {
                    format!("({})", ids.join(" "))
                } else {
                    ids.concat()
                }
            })
            .collect();
        let _ = writeln!(abc, "%%score {}", groups.join(" "));
    }
    for voice in &voices {
        let name = quoted(&voice.part.name);
        if name.is_empty() || !voice.primary {
            let _ = writeln!(abc, "V:{} clef={}", voice.id, voice.clef);
        } else {
            let _ = writeln!(abc, "V:{} clef={} name=\"{name}\"", voice.id, voice.clef);
        }
        if let Some(program) = voice.part.program {
            let _ = writeln!(abc, "%%MIDI program {program}");
        }
    }

    let spellers = layout
        .iter()
        .map(|frame| frame.key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| Ok(((key.sharps, key.minor), Speller::new(key)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let ties: Vec<_> = voices
        .iter()
        .map(|voice| ties_out(voice.part, voice.voice))
        .collect();

    // Four bars to a line, never breaking before a bar that opens an ending:
    // the ending is written against the barline before it.
    let mut lines: Vec<std::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    for index in 0..count {
        let full = index + 1 - start >= 4;
        let ends = index + 1 == count;
        if ends || full && opening_ending(&frames, index + 1).is_none() {
            lines.push(start..index + 1);
            start = index + 1;
        }
    }
    for range in lines {
        for (v, voice) in voices.iter().enumerate() {
            let mut line = format!("[V:{}]", voice.id);
            if range.start == 0 {
                if frames[0].repeat_start {
                    line.push_str(" |:");
                }
                if let Some(ending) = opening_ending(&frames, 0) {
                    line.push_str(&ending);
                }
            }
            for index in range.clone() {
                let frame = &layout[index];
                let speller = &spellers[&(frame.key.sharps, frame.key.minor)];
                write_measure(
                    &mut line,
                    voice,
                    index,
                    &frames,
                    frame,
                    speller,
                    &ties[v],
                    v == 0,
                )?;
                let _ = write!(line, " {}", closing_bar(&frames, index));
            }
            abc.push_str(line.trim_end());
            abc.push('\n');
        }
    }
    Ok(abc)
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
    /// (`.musicxml`, `.xml`, `.mxl`) or Songsterr JSON file into the score,
    /// adding its parts after those already read.
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
        let abc = to_abc(&self.imported).map_err(js_error)?;
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
