//! The JSON Songsterr's player loads for one part of a song: its tuning and
//! instrument, then measures of voices of beats of notes on strings and frets.
//! The field names were read off the files themselves; nothing documents them.

use super::{Beat, Imported, KeySig, Measure, Note, Part, TICKS, tuplet_ratio, written_ticks};
use serde_json::Value;

/// Songsterr's instrument number for a drum kit, which is not a MIDI program.
const DRUMS: i64 = 1024;

fn number(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|float| float.round() as i64))
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

pub(super) fn read(json: &str) -> Result<Imported, String> {
    let track: Value =
        serde_json::from_str(json).map_err(|err| format!("the JSON does not parse: {err}"))?;
    let measures = track["measures"]
        .as_array()
        .ok_or("this JSON has no measures, so it is not a Songsterr track")?;
    let name = text(&track["name"])
        .or_else(|| text(&track["instrument"]))
        .unwrap_or_else(|| "Track".to_string());
    let mut imported = Imported {
        title: text(&track["title"]).unwrap_or_default(),
        tempo: track["automations"]["tempo"]
            .as_array()
            .and_then(|changes| changes.first())
            .and_then(|change| change["bpm"].as_f64()),
        ..Imported::default()
    };
    let instrument = number(&track["instrumentId"]);
    if instrument == Some(DRUMS) {
        imported.skipped.push(format!("{name}: drums"));
        return Ok(imported);
    }
    // Strings are numbered from the highest, as the tuning lists them.
    let strings: Vec<i32> = track["tuning"]
        .as_array()
        .map(|tuning| {
            tuning
                .iter()
                .filter_map(number)
                .map(|midi| midi as i32)
                .collect()
        })
        .unwrap_or_default();

    let mut part = Part {
        name,
        program: instrument
            .filter(|id| (0..128).contains(id))
            .map(|id| id as u8),
        tuning: strings.iter().rev().copied().collect(),
        ..Part::default()
    };
    for source in measures {
        let mut measure = Measure {
            meter: source["signature"]
                .as_array()
                .filter(|pair| pair.len() == 2)
                .and_then(|pair| Some((number(&pair[0])? as u32, number(&pair[1])? as u32))),
            key: source["keySignature"].as_object().map(|key| {
                let count = key.get("accidentalCount").and_then(number).unwrap_or(0) as i32;
                let flats = key.get("transposeAs").and_then(Value::as_str) == Some("b");
                KeySig {
                    sharps: if flats { -count.abs() } else { count },
                    minor: key.get("mode").and_then(Value::as_str) == Some("minor"),
                }
            }),
            repeat_start: source["repeatStart"].as_bool().unwrap_or(false),
            repeat_end: number(&source["repeat"]).map_or(0, |plays| plays.max(2) as u32),
            ending: source["alternateEnding"]
                .as_array()
                .map(|endings| {
                    endings
                        .iter()
                        .filter_map(number)
                        .map(|n| n as u32)
                        .collect()
                })
                .unwrap_or_default(),
            marker: text(&source["marker"]["text"]),
            voices: Vec::new(),
        };
        for voice in source["voices"].as_array().into_iter().flatten() {
            let beats = voice["beats"].as_array().into_iter().flatten();
            measure
                .voices
                .push(beats.map(|beat| read_beat(beat, &strings)).collect());
        }
        part.measures.push(measure);
    }
    carry_endings(&mut part.measures);
    imported.parts.push(part);
    Ok(imported)
}

/// Songsterr marks an ending on its first bar only; the ending runs on to
/// the bar that closes the repeat, where there is one before the next ending
/// or repeat begins.
fn carry_endings(measures: &mut [Measure]) {
    for index in 0..measures.len() {
        if measures[index].ending.is_empty() || measures[index].repeat_end > 0 {
            continue;
        }
        let closing = (index + 1..measures.len())
            .take_while(|&later| measures[later].ending.is_empty() && !measures[later].repeat_start)
            .find(|&later| measures[later].repeat_end > 0);
        if let Some(closing) = closing {
            let ending = measures[index].ending.clone();
            for measure in &mut measures[index + 1..=closing] {
                measure.ending = ending.clone();
            }
        }
    }
}

fn read_beat(beat: &Value, strings: &[i32]) -> Beat {
    // `duration` is the length played, dots and tuplets counted; `type` and
    // `dots` are how it is written.
    let played = beat["duration"]
        .as_array()
        .filter(|pair| pair.len() == 2)
        .and_then(|pair| Some((number(&pair[0])?, number(&pair[1])?)))
        .filter(|&(_, denominator)| denominator > 0)
        .map(|(numerator, denominator)| numerator * 4 * TICKS / denominator);
    let mut read = Beat {
        written: number(&beat["type"]).map_or(TICKS, |kind| {
            written_ticks(
                kind.max(1) as u32,
                number(&beat["dots"]).unwrap_or(0) as u32,
            )
        }),
        tuplet: number(&beat["tuplet"]).and_then(|actual| tuplet_ratio(actual as u32)),
        grace: beat["graceNote"].is_string(),
        chord: text(&beat["chord"]["text"]),
        text: text(&beat["text"]["text"]),
        ..Beat::default()
    };
    if let Some(played) = played
        && !read.grace
        && (read.length() - played).abs() > 1
    {
        read.written = played;
        read.tuplet = None;
    }
    if beat["rest"].as_bool() == Some(true) {
        return read;
    }
    for note in beat["notes"].as_array().into_iter().flatten() {
        if note["rest"].as_bool() == Some(true) || note["dead"].as_bool() == Some(true) {
            continue;
        }
        let (Some(string), Some(fret)) = (number(&note["string"]), number(&note["fret"])) else {
            continue;
        };
        if let Some(open) = strings.get(string as usize) {
            read.notes.push(Note {
                midi: open + fret as i32,
                tied: note["tie"].as_bool() == Some(true),
            });
        }
    }
    read
}
