//! Guitar Pro 6, 7 and 8. The score is one XML document, `score.gpif`: Guitar
//! Pro 7 and 8 keep it in a zip archive (`.gp`), and Guitar Pro 6 in a file
//! system of its own (`.gpx`), usually compressed. The document keeps every
//! bar, voice, beat, note and rhythm once, by id, and the master bars list
//! which bar of each staff sounds together.

use super::{Beat, Imported, KeySig, Measure, Note, Part, TICKS, latin1, written_ticks};
use roxmltree::{Document, Node, ParsingOptions};
use std::collections::HashMap;

const DAMAGED: &str = "the Guitar Pro 6 file is damaged";

/// Reads bits most significant first, as Guitar Pro 6 packs them.
struct Bits<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u8,
}

impl Bits<'_> {
    fn bit(&mut self) -> Option<u32> {
        let byte = *self.data.get(self.byte)?;
        let value = (byte >> (7 - self.bit)) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.byte += 1;
        }
        Some(u32::from(value))
    }

    fn bits(&mut self, count: u32) -> Option<u32> {
        (0..count).try_fold(0, |value, _| Some(value << 1 | self.bit()?))
    }

    /// `count` bits read least significant first.
    fn bits_reversed(&mut self, count: u32) -> Option<u32> {
        (0..count).try_fold(0, |value, index| Some(value | self.bit()? << index))
    }
}

fn i32_at(data: &[u8], at: usize) -> Option<i32> {
    let quad = data.get(at..at + 4)?;
    Some(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]))
}

/// Undoes Guitar Pro 6's compression: runs of literal bytes, and copies of
/// what was already written, each copy an offset back and a length.
fn decompress(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let expected = i32_at(bytes, 4).ok_or(DAMAGED)?.clamp(0, 1 << 26) as usize;
    let mut bits = Bits {
        data: &bytes[8..],
        byte: 0,
        bit: 0,
    };
    let mut out = Vec::with_capacity(expected);
    while out.len() < expected {
        let Some(flag) = bits.bit() else { break };
        if flag == 1 {
            let Some(width) = bits.bits(4) else { break };
            let (Some(offset), Some(length)) =
                (bits.bits_reversed(width), bits.bits_reversed(width))
            else {
                break;
            };
            let offset = offset as usize;
            if offset == 0 || offset > out.len() {
                return Err(DAMAGED.to_string());
            }
            let from = out.len() - offset;
            for index in 0..offset.min(length as usize) {
                out.push(out[from + index]);
            }
        } else {
            let Some(length) = bits.bits_reversed(2) else {
                break;
            };
            for _ in 0..length {
                let Some(byte) = bits.bits(8) else { break };
                out.push(byte as u8);
            }
        }
    }
    Ok(out)
}

/// The file called `wanted` in a Guitar Pro 6 file system: sectors of 4 KiB,
/// each file an entry naming the sectors that hold it.
fn gpx_file(bytes: &[u8], wanted: &str) -> Result<Vec<u8>, String> {
    const SECTOR: usize = 0x1000;
    let unpacked;
    let data = if bytes.starts_with(b"BCFZ") {
        unpacked = decompress(bytes)?;
        unpacked.as_slice()
    } else {
        bytes
    };
    let data = data.strip_prefix(b"BCFS").ok_or(DAMAGED)?;
    let mut offset = SECTOR;
    while offset + 3 < data.len() {
        if i32_at(data, offset) == Some(2) {
            let name = data.get(offset + 4..offset + 4 + 127).ok_or(DAMAGED)?;
            let name = latin1(name.split(|&byte| byte == 0).next().unwrap_or_default());
            let size = i32_at(data, offset + 0x8c).ok_or(DAMAGED)?.max(0) as usize;
            let mut file = Vec::with_capacity(size);
            let mut pointer = offset + 0x94;
            while let Some(sector) = i32_at(data, pointer).filter(|&sector| sector > 0) {
                offset = sector as usize * SECTOR;
                file.extend_from_slice(
                    data.get(offset..(offset + SECTOR).min(data.len()))
                        .unwrap_or_default(),
                );
                pointer += 4;
            }
            if name == wanted {
                file.truncate(size);
                return Ok(file);
            }
        }
        offset += SECTOR;
    }
    Err("the Guitar Pro 6 file holds no score".to_string())
}

pub(super) fn read_gpx(bytes: &[u8]) -> Result<Imported, String> {
    let score = gpx_file(bytes, "score.gpif")?;
    read_xml(&String::from_utf8_lossy(&score))
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|child| child.has_tag_name(name))
}

fn path<'a, 'input>(node: Node<'a, 'input>, names: &[&str]) -> Option<Node<'a, 'input>> {
    names.iter().try_fold(node, |node, name| child(node, name))
}

fn text_at<'a>(node: Node<'a, '_>, names: &[&str]) -> Option<&'a str> {
    path(node, names)?
        .text()
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

fn numbers<T: std::str::FromStr>(text: Option<&str>) -> Vec<T> {
    text.unwrap_or_default()
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect()
}

/// A `<Property name="…">` among the children of a `<Properties>`.
fn property<'a, 'input>(
    properties: Option<Node<'a, 'input>>,
    name: &str,
) -> Option<Node<'a, 'input>> {
    properties?
        .children()
        .find(|node| node.has_tag_name("Property") && node.attribute("name") == Some(name))
}

/// Every element of one kind, by its `id`.
fn by_id<'a, 'input>(
    root: Node<'a, 'input>,
    list: &str,
    item: &str,
) -> HashMap<&'a str, Node<'a, 'input>> {
    child(root, list)
        .into_iter()
        .flat_map(|list| list.children())
        .filter(|node| node.has_tag_name(item))
        .filter_map(|node| Some((node.attribute("id")?, node)))
        .collect()
}

struct Staff {
    part: usize,
    /// Open strings, lowest first.
    tuning: Vec<i32>,
    chords: HashMap<String, String>,
}

fn is_drums(track: Node<'_, '_>) -> bool {
    let channel = |names: &[&str]| text_at(track, names).and_then(|text| text.parse::<i32>().ok());
    text_at(track, &["InstrumentSet", "Type"]) == Some("drumKit")
        || child(track, "GeneralMidi").and_then(|midi| midi.attribute("table"))
            == Some("Percussion")
        || channel(&["GeneralMidi", "PrimaryChannel"]) == Some(9)
        || channel(&["MidiConnection", "PrimaryChannel"]) == Some(9)
}

fn pitch_of(note: Node<'_, '_>, tuning: &[i32]) -> Option<i32> {
    let properties = child(note, "Properties");
    let number = |name: &str, field: &str| {
        property(properties, name)
            .and_then(|property| text_at(property, &[field]))
            .and_then(|text| text.parse::<i32>().ok())
    };
    if let Some(midi) = number("Midi", "Number") {
        return Some(midi);
    }
    if let (Some(string), Some(fret)) = (number("String", "String"), number("Fret", "Fret"))
        && let Some(open) = tuning.get(string as usize)
    {
        return Some(open + fret);
    }
    // Guitar Pro numbers octaves from C0 at MIDI 0, one below the usual.
    let pitch =
        property(properties, "ConcertPitch").and_then(|property| child(property, "Pitch"))?;
    let class = match text_at(pitch, &["Step"])? {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => return None,
    };
    let alter = match text_at(pitch, &["Accidental"]).unwrap_or_default() {
        "#" => 1,
        "##" | "x" => 2,
        "b" => -1,
        "bb" => -2,
        _ => 0,
    };
    let octave: i32 = text_at(pitch, &["Octave"])?.parse().ok()?;
    Some(octave * 12 + class + alter)
}

fn note_value(name: &str) -> i64 {
    match name {
        "DoubleWhole" => TICKS * 8,
        "Whole" => written_ticks(1, 0),
        "Half" => written_ticks(2, 0),
        "Quarter" => written_ticks(4, 0),
        "Eighth" => written_ticks(8, 0),
        "16th" => written_ticks(16, 0),
        "32nd" => written_ticks(32, 0),
        "64th" => written_ticks(64, 0),
        "128th" => written_ticks(128, 0),
        "256th" => written_ticks(256, 0),
        _ => TICKS,
    }
}

pub(super) fn read_xml(text: &str) -> Result<Imported, String> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(text, options)
        .map_err(|err| format!("the score does not parse: {err}"))?;
    let root = document.root_element();
    if !root.has_tag_name("GPIF") {
        return Err("this is not a Guitar Pro score".to_string());
    }
    let mut imported = Imported {
        title: text_at(root, &["Score", "Title"])
            .unwrap_or_default()
            .to_string(),
        tempo: child(root, "MasterTrack")
            .and_then(|master| child(master, "Automations"))
            .into_iter()
            .flat_map(|automations| automations.children())
            .find(|automation| text_at(*automation, &["Type"]) == Some("Tempo"))
            .and_then(|automation| text_at(automation, &["Value"]))
            .and_then(|value| value.split_whitespace().next()?.parse().ok()),
        ..Imported::default()
    };

    // Each staff of each track in order: the master bars list one bar for each.
    let mut staves: Vec<Option<Staff>> = Vec::new();
    for track in child(root, "Tracks")
        .into_iter()
        .flat_map(|tracks| tracks.children())
        .filter(|node| node.has_tag_name("Track"))
    {
        let name = text_at(track, &["Name"]).unwrap_or("Track").to_string();
        let drums = is_drums(track);
        let program = text_at(track, &["GeneralMidi", "Program"])
            .or_else(|| text_at(track, &["Sounds", "Sound", "MIDI", "Program"]))
            .and_then(|text| text.parse::<u8>().ok())
            .filter(|program| *program < 128);
        let staff_properties: Vec<Option<Node<'_, '_>>> = match child(track, "Staves") {
            Some(list) => list
                .children()
                .filter(|node| node.has_tag_name("Staff"))
                .map(|staff| child(staff, "Properties"))
                .collect(),
            None => vec![child(track, "Properties")],
        };
        let chords: HashMap<String, String> = track
            .descendants()
            .filter(|node| node.has_tag_name("Item"))
            .filter_map(|item| {
                Some((
                    item.attribute("id")?.to_string(),
                    item.attribute("name")?.to_string(),
                ))
            })
            .filter(|(_, name)| !name.trim().is_empty())
            .collect();
        if drums {
            imported.skipped.push(format!("{name}: drums"));
        }
        let count = staff_properties.len();
        for (index, properties) in staff_properties.into_iter().enumerate() {
            if drums {
                staves.push(None);
                continue;
            }
            staves.push(Some(Staff {
                part: imported.parts.len(),
                tuning: numbers(
                    property(properties, "Tuning").and_then(|tuning| text_at(tuning, &["Pitches"])),
                ),
                chords: chords.clone(),
            }));
            imported.parts.push(Part {
                name: if count > 1 {
                    format!("{name} ({})", index + 1)
                } else {
                    name.clone()
                },
                program,
                ..Part::default()
            });
        }
    }
    for staff in staves.iter().flatten() {
        imported.parts[staff.part].tuning = staff.tuning.clone();
    }

    let bars = by_id(root, "Bars", "Bar");
    let voices = by_id(root, "Voices", "Voice");
    let beats = by_id(root, "Beats", "Beat");
    let notes = by_id(root, "Notes", "Note");
    let rhythms = by_id(root, "Rhythms", "Rhythm");

    let master_bars = child(root, "MasterBars")
        .into_iter()
        .flat_map(|list| list.children())
        .filter(|node| node.has_tag_name("MasterBar"));
    for master in master_bars {
        let repeat = child(master, "Repeat");
        let frame = Measure {
            meter: text_at(master, &["Time"]).and_then(|time| {
                let (numerator, denominator) = time.split_once('/')?;
                Some((
                    numerator.trim().parse().ok()?,
                    denominator.trim().parse().ok()?,
                ))
            }),
            key: text_at(master, &["Key", "AccidentalCount"])
                .and_then(|count| count.parse().ok())
                .map(|sharps| KeySig {
                    sharps,
                    minor: text_at(master, &["Key", "Mode"]) == Some("Minor"),
                }),
            repeat_start: repeat.and_then(|repeat| repeat.attribute("start")) == Some("true"),
            repeat_end: if repeat.and_then(|repeat| repeat.attribute("end")) == Some("true") {
                repeat
                    .and_then(|repeat| repeat.attribute("count")?.parse().ok())
                    .unwrap_or(2)
                    .max(2)
            } else {
                0
            },
            ending: numbers(text_at(master, &["AlternateEndings"])),
            marker: text_at(master, &["Section", "Text"]).map(str::to_string),
            voices: Vec::new(),
        };
        let bar_ids: Vec<&str> = text_at(master, &["Bars"])
            .unwrap_or_default()
            .split_whitespace()
            .collect();
        for (index, staff) in staves.iter().enumerate() {
            let Some(staff) = staff else { continue };
            let mut measure = frame.clone();
            let bar = bar_ids.get(index).and_then(|id| bars.get(id));
            for voice_id in bar
                .map(|bar| numbers::<i64>(text_at(*bar, &["Voices"])))
                .unwrap_or_default()
            {
                let Some(voice) = voices.get(voice_id.to_string().as_str()) else {
                    continue;
                };
                let beat_ids: Vec<&str> = text_at(*voice, &["Beats"])
                    .unwrap_or_default()
                    .split_whitespace()
                    .collect();
                let read = beat_ids
                    .iter()
                    .filter_map(|id| beats.get(id))
                    .map(|beat| read_beat(*beat, &rhythms, &notes, staff))
                    .collect();
                measure.voices.push(read);
            }
            imported.parts[staff.part].measures.push(measure);
        }
    }
    Ok(imported)
}

fn read_beat(
    beat: Node<'_, '_>,
    rhythms: &HashMap<&str, Node<'_, '_>>,
    notes: &HashMap<&str, Node<'_, '_>>,
    staff: &Staff,
) -> Beat {
    let rhythm = child(beat, "Rhythm").and_then(|rhythm| rhythms.get(rhythm.attribute("ref")?));
    let mut read = Beat {
        grace: child(beat, "GraceNotes").is_some(),
        chord: text_at(beat, &["Chord"]).and_then(|id| staff.chords.get(id).cloned()),
        text: text_at(beat, &["FreeText"]).map(str::to_string),
        ..Beat::default()
    };
    if let Some(rhythm) = rhythm {
        let base = note_value(text_at(*rhythm, &["NoteValue"]).unwrap_or("Quarter"));
        let dots = child(*rhythm, "AugmentationDot")
            .and_then(|dot| dot.attribute("count")?.parse::<u32>().ok())
            .unwrap_or(0);
        let mut added = base;
        read.written = base;
        for _ in 0..dots.min(4) {
            added /= 2;
            read.written += added;
        }
        let ratio = |name: &str| {
            let tuplet = child(*rhythm, name)?;
            Some((
                tuplet.attribute("num")?.parse::<u32>().ok()?,
                tuplet.attribute("den")?.parse::<u32>().ok()?,
            ))
        };
        read.tuplet = match (ratio("PrimaryTuplet"), ratio("SecondaryTuplet")) {
            (Some((a, n)), Some((b, m))) => Some((a * b, n * m)),
            (Some(tuplet), None) | (None, Some(tuplet)) => Some(tuplet),
            (None, None) => None,
        }
        .filter(|&(actual, normal)| actual > 0 && normal > 0 && actual != normal);
    } else {
        read.written = TICKS;
    }
    for id in text_at(beat, &["Notes"])
        .unwrap_or_default()
        .split_whitespace()
    {
        let Some(note) = notes.get(id) else { continue };
        let muted = property(child(*note, "Properties"), "Muted").is_some();
        if muted {
            continue;
        }
        if let Some(midi) = pitch_of(*note, &staff.tuning) {
            let tied =
                child(*note, "Tie").and_then(|tie| tie.attribute("destination")) == Some("true");
            read.notes.push(Note { midi, tied });
        }
    }
    read
}
