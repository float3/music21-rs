//! MusicXML, partwise, plain or compressed (`.mxl`), read by the crate's own
//! reader -- music21's, ported -- and turned into what the editor writes ABC
//! from. A part written on several staves, a piano's, comes back as a part
//! per staff, and a tablature staff doubling the notes of a staff above it
//! is left out.
//!
//! Three things the editor wants are no part of the score music21 models,
//! and are read off the document beside it: a tablature staff's tuning, a
//! rehearsal mark, and whether a part plays on the drum channel.

use super::{
    Beat, Imported, KeySig, Measure, Note, Part, TICKS, decode_text, written_ticks, zip::Archive,
};
use music21_rs::musicxml::from_musicxml;
use music21_rs::{
    ChordSymbol, Clef, Duration, DurationType, Instrument, NoteSize, Stream, StreamElement,
    StreamKind, TieType,
};
use roxmltree::{Document, Node, ParsingOptions};
use std::collections::{HashMap, HashSet};

pub(super) fn read_compressed(archive: &Archive<'_>) -> Result<Imported, String> {
    // The container names the score; failing that, the first XML file in the
    // archive is it.
    let named = match archive.read("META-INF/container.xml")? {
        Some(container) => {
            let text = decode_text(&container);
            let options = ParsingOptions {
                allow_dtd: true,
                ..ParsingOptions::default()
            };
            let document =
                Document::parse_with_options(&text, options).map_err(|err| err.to_string())?;
            document
                .descendants()
                .find(|node| node.has_tag_name("rootfile"))
                .and_then(|node| node.attribute("full-path"))
                .map(str::to_string)
        }
        None => None,
    };
    let name = named
        .or_else(|| {
            archive
                .names()
                .find(|name| {
                    !name.starts_with("META-INF")
                        && (name.ends_with(".xml") || name.ends_with(".musicxml"))
                })
                .map(str::to_string)
        })
        .ok_or("the archive holds no score")?;
    let score = archive.read(&name)?.ok_or("the archive holds no score")?;
    read(&decode_text(&score))
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|child| child.has_tag_name(name))
}

fn text_at<'a>(node: Node<'a, '_>, names: &[&str]) -> Option<&'a str> {
    let node = names
        .iter()
        .try_fold(node, |node, name| child(node, name))?;
    node.text().map(str::trim).filter(|text| !text.is_empty())
}

fn number<T: std::str::FromStr>(node: Node<'_, '_>, names: &[&str]) -> Option<T> {
    text_at(node, names)?.parse().ok()
}

fn step_class(step: &str) -> Option<i32> {
    Some(match step {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => return None,
    })
}

/// What the editor reads off the document itself, by part id.
#[derive(Default)]
struct Extras {
    /// Parts playing on the General MIDI drum channel.
    drums: HashSet<String>,
    /// Open strings of a staff, lowest first, by part and staff number.
    tuning: HashMap<(String, usize), Vec<i32>>,
    /// A rehearsal mark, by part and the measure's place in it.
    markers: HashMap<(String, usize), String>,
}

fn extras(text: &str) -> Extras {
    let mut extras = Extras::default();
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let Ok(document) = Document::parse_with_options(text, options) else {
        return extras;
    };
    let root = document.root_element();
    for part in root
        .descendants()
        .filter(|node| node.has_tag_name("score-part"))
    {
        if let Some(id) = part.attribute("id")
            && number::<u8>(part, &["midi-instrument", "midi-channel"]) == Some(10)
        {
            extras.drums.insert(id.to_string());
        }
    }
    for part in root.children().filter(|node| node.has_tag_name("part")) {
        let Some(id) = part.attribute("id") else {
            continue;
        };
        let measures = part.children().filter(|node| node.has_tag_name("measure"));
        for (index, measure) in measures.enumerate() {
            for details in measure
                .descendants()
                .filter(|node| node.has_tag_name("staff-details"))
            {
                let staff = details
                    .attribute("number")
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(1);
                let mut strings: Vec<(i32, i32)> = details
                    .children()
                    .filter(|node| node.has_tag_name("staff-tuning"))
                    .filter_map(|tuning| {
                        let line = tuning.attribute("line")?.parse().ok()?;
                        let class = step_class(text_at(tuning, &["tuning-step"])?)?;
                        let alter = number::<f64>(tuning, &["tuning-alter"])
                            .unwrap_or(0.0)
                            .round() as i32;
                        let octave: i32 = number(tuning, &["tuning-octave"])?;
                        Some((line, (octave + 1) * 12 + class + alter))
                    })
                    .collect();
                strings.sort();
                if !strings.is_empty() {
                    extras.tuning.insert(
                        (id.to_string(), staff.max(1)),
                        strings.into_iter().map(|(_, midi)| midi).collect(),
                    );
                }
            }
            if let Some(mark) = measure
                .descendants()
                .find(|node| node.has_tag_name("rehearsal"))
                .and_then(|node| node.text())
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                extras
                    .markers
                    .insert((id.to_string(), index), mark.to_string());
            }
        }
    }
    extras
}

/// The ABC clef a clef reads as, and whether it is a tablature staff's.
fn clef_name(clef: &Clef) -> Option<(String, bool)> {
    let down = clef.octave_change() < 0;
    let name = match clef.sign()? {
        "G" if down => "treble-8",
        "G" => "treble",
        "F" if down => "bass-8",
        "F" => "bass",
        "C" if clef.line() == Some(4) => "tenor",
        "C" => "alto",
        "TAB" => return Some(("treble-8".to_string(), true)),
        _ => return None,
    };
    Some((name.to_string(), false))
}

/// A chord symbol as the editor writes one: root, kind and bass.
fn chord_text(symbol: &ChordSymbol) -> Option<String> {
    if symbol.is_no_chord() {
        return None;
    }
    let flat = |name: String| name.replace('-', "b");
    let mut text = flat(symbol.root().name());
    text.push_str(match symbol.kind_text() {
        Some(written) => written,
        None => match symbol.kind().unwrap_or("") {
            "minor" => "m",
            "augmented" => "+",
            "diminished" => "dim",
            "dominant" | "dominant-seventh" => "7",
            "major-seventh" => "maj7",
            "minor-seventh" => "m7",
            "diminished-seventh" => "dim7",
            "augmented-seventh" => "+7",
            "half-diminished" | "half-diminished-seventh" => "m7b5",
            "major-minor" | "minor-major-seventh" => "m(maj7)",
            "major-sixth" => "6",
            "minor-sixth" => "m6",
            "dominant-ninth" => "9",
            "major-ninth" => "maj9",
            "minor-ninth" => "m9",
            "dominant-11th" => "11",
            "dominant-13th" => "13",
            "suspended-second" => "sus2",
            "suspended-fourth" => "sus4",
            "power" => "5",
            _ => "",
        },
    });
    if let Some(bass) = symbol
        .bass()
        .filter(|bass| bass.name() != symbol.root().name())
    {
        text.push('/');
        text.push_str(&flat(bass.name()));
    }
    Some(text)
}

fn ticks(quarters: f64) -> i64 {
    (quarters * TICKS as f64).round() as i64
}

/// One note, chord or rest as a beat, at concert pitch.
fn beat_of(element: &StreamElement, transpose: i32) -> Option<Beat> {
    let duration: Duration = element.duration().cloned()?;
    let tied = |tie: Option<&music21_rs::Tie>| {
        tie.is_some_and(|tie| {
            matches!(
                tie.tie_type(),
                TieType::Stop | TieType::Continue | TieType::ContinueLetRing
            )
        })
    };
    let sounded = |note: &music21_rs::Note| {
        // A cue note shows another part's line and is not played.
        (note.size() != Some(NoteSize::Cue)).then(|| Note {
            midi: note.pitch().midi() + transpose,
            tied: tied(note.tie()),
        })
    };
    let notes: Vec<Note> = match element {
        StreamElement::Note(note) => sounded(note).into_iter().collect(),
        StreamElement::Chord(chord) => chord.notes().iter().filter_map(sounded).collect(),
        StreamElement::Rest(_) => Vec::new(),
        _ => return None,
    };
    let grace = duration.is_grace();
    let length = ticks(duration.quarter_length());
    let components = duration.components();
    let mut beat = Beat {
        grace,
        notes,
        ..Beat::default()
    };
    beat.written = match components.as_slice() {
        [(DurationType::Breve, _)] => TICKS * 8,
        [(kind, dots)] => match kind.type_number() {
            Some(denominator) if denominator >= 1.0 => written_ticks(denominator as u32, *dots),
            _ => ticks(kind.quarter_length_with_dots(*dots)),
        },
        [] => length,
        several => several
            .iter()
            .map(|(kind, dots)| ticks(kind.quarter_length_with_dots(*dots)))
            .sum(),
    };
    // Every tuplet the note is written inside, as one ratio.
    let (actual, normal) = duration
        .tuplets()
        .iter()
        .fold((1u32, 1u32), |(actual, normal), tuplet| {
            (actual * tuplet.actual(), normal * tuplet.normal())
        });
    beat.tuplet = (actual > 0 && normal > 0 && actual != normal).then_some((actual, normal));
    if !grace && (beat.length() - length).abs() > 1 {
        beat.written = length;
        beat.tuplet = None;
    }
    Some(beat)
}

/// A staff as it is read: its measures, the voices it has met in order, and
/// its clef.
#[derive(Default)]
struct Staff {
    measures: Vec<Measure>,
    clef: Option<String>,
    tablature: bool,
}

/// Reads one part of the score -- one staff -- into its measures, or says
/// why it cannot be.
fn read_staff(
    part: &Stream,
    part_id: &str,
    extras: &Extras,
    imported: &mut Imported,
) -> Result<Staff, String> {
    let mut staff = Staff::default();
    // The instruments in force, which say how far the part sounds from
    // where it is written.
    let instruments: Vec<(f64, &Instrument)> = part
        .events()
        .iter()
        .filter_map(|event| match event.element() {
            StreamElement::Instrument(instrument) => Some((event.offset(), &**instrument)),
            _ => None,
        })
        .collect();
    let mut voices: Vec<String> = Vec::new();
    let mut index = 0usize;

    for event in part.events() {
        let StreamElement::Stream(measure) = event.element() else {
            continue;
        };
        if measure.kind() != StreamKind::Measure {
            continue;
        }
        let transpose = instruments
            .iter()
            .rev()
            .find(|(offset, _)| *offset <= event.offset())
            .and_then(|(_, instrument)| instrument.transposition())
            .map_or(0, |interval| interval.semitones().round() as i32);

        let mut frame = Measure::default();
        if let Some(ending) = measure.ending() {
            frame.ending = ending
                .numbers()
                .iter()
                .copied()
                .filter(|number| *number > 0)
                .collect();
        }
        if let Some(barline) = measure.left_barline()
            && barline.repeat_direction() == Some(music21_rs::RepeatDirection::Start)
        {
            frame.repeat_start = true;
        }
        if let Some(barline) = measure.right_barline()
            && barline.repeat_direction() == Some(music21_rs::RepeatDirection::End)
        {
            frame.repeat_end = barline.repeat_times().unwrap_or(2).max(2);
        }
        frame.marker = extras.markers.get(&(part_id.to_string(), index)).cloned();

        // The voices of this measure: its voice streams, or itself where it
        // has one line.
        let mut lines: Vec<(usize, &Stream)> = Vec::new();
        for held in measure.events() {
            if let StreamElement::Stream(voice) = held.element()
                && voice.kind() == StreamKind::Voice
            {
                let name = voice.id().unwrap_or("1").to_string();
                let place = match voices.iter().position(|known| *known == name) {
                    Some(place) => place,
                    None => {
                        voices.push(name);
                        voices.len() - 1
                    }
                };
                lines.push((place, voice));
            }
        }
        if lines.is_empty() {
            if voices.is_empty() {
                voices.push("1".to_string());
            }
            lines.push((0, measure));
        }

        let mut symbols: Vec<(f64, String)> = Vec::new();
        for held in measure.events() {
            match held.element() {
                StreamElement::TimeSignature(meter) => {
                    frame.meter = Some((meter.numerator().max(1), meter.denominator().max(1)));
                }
                StreamElement::KeySignature(_) | StreamElement::Key(_) => {
                    let (fifths, minor) = match held.element() {
                        StreamElement::Key(key) => (key.sharps(), key.mode() == "minor"),
                        StreamElement::KeySignature(signature) => {
                            (signature.sharps().unwrap_or(0), false)
                        }
                        _ => continue,
                    };
                    // The key sounds where the notes do, so a transposing
                    // instrument's is moved to concert pitch.
                    let mut sharps = fifths + (transpose * 7).rem_euclid(12);
                    if sharps > 7 {
                        sharps -= 12;
                    }
                    frame.key = Some(KeySig { sharps, minor });
                }
                StreamElement::Clef(clef) => {
                    if let Some((name, tablature)) = clef_name(clef) {
                        staff.clef = Some(name);
                        staff.tablature = tablature;
                    }
                }
                StreamElement::ChordSymbol(symbol) => {
                    if let Some(text) = chord_text(symbol) {
                        symbols.push((held.offset(), text));
                    }
                }
                StreamElement::MetronomeMark(mark) if imported.tempo.is_none() => {
                    imported.tempo = mark.sounding_quarter_bpm().or_else(|| mark.quarter_bpm());
                }
                StreamElement::Unpitched(_) | StreamElement::PercussionChord(_) => {
                    return Err("unpitched percussion".to_string());
                }
                _ => {}
            }
        }

        let mut read: Vec<Vec<Beat>> = vec![Vec::new(); voices.len()];
        // Where each beat starts, to hang the chord symbols on.
        let mut starts: Vec<(f64, usize, usize)> = Vec::new();
        for (place, line) in lines {
            let mut clock = 0i64;
            for held in line.events() {
                if matches!(
                    held.element(),
                    StreamElement::Unpitched(_) | StreamElement::PercussionChord(_)
                ) {
                    return Err("unpitched percussion".to_string());
                }
                let Some(beat) = beat_of(held.element(), transpose) else {
                    continue;
                };
                let beats = &mut read[place];
                if !beat.grace {
                    let at = ticks(held.offset());
                    if at > clock {
                        beats.push(Beat {
                            written: at - clock,
                            ..Beat::default()
                        });
                    }
                    clock = at + beat.length();
                    starts.push((held.offset(), place, beats.len()));
                }
                beats.push(beat);
            }
        }
        // A chord symbol is written over the next note to start, in whichever
        // voice that is.
        starts.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        for (offset, text) in symbols {
            if let Some((_, place, beat)) = starts
                .iter()
                .find(|(start, place, beat)| {
                    *start >= offset - 1e-9 && read[*place][*beat].chord.is_none()
                })
                .copied()
            {
                read[place][beat].chord = Some(text);
            }
        }
        frame.voices = read;
        staff.measures.push(frame);
        index += 1;
    }
    // A voice first met late is empty in the measures before it.
    for measure in &mut staff.measures {
        measure.voices.resize_with(voices.len(), Vec::new);
    }
    Ok(staff)
}

pub(super) fn read(text: &str) -> Result<Imported, String> {
    let score = from_musicxml(text).map_err(|err| format!("the MusicXML does not read: {err}"))?;
    let extras = extras(text);
    let metadata = score.metadata();
    let mut imported = Imported {
        title: metadata
            .and_then(|metadata| {
                metadata
                    .first_text("title")
                    .or_else(|| metadata.first_text("movementName"))
            })
            .unwrap_or_default()
            .trim()
            .to_string(),
        ..Imported::default()
    };

    // The score's parts, a part written on several staves being several
    // that name one part of the document between them.
    let mut groups: Vec<(String, Vec<&Stream>)> = Vec::new();
    for part in score.parts() {
        let first = part
            .events()
            .iter()
            .find_map(|event| match event.element() {
                StreamElement::Instrument(instrument) => Some(&**instrument),
                _ => None,
            });
        let id = first
            .and_then(Instrument::part_id)
            .unwrap_or_default()
            .to_string();
        match groups.last_mut() {
            Some((known, staves)) if *known == id && part.kind() == StreamKind::PartStaff => {
                staves.push(part);
            }
            _ => groups.push((id, vec![part])),
        }
    }

    for (id, staves) in groups {
        let first = staves[0];
        let name = first
            .name()
            .filter(|name| !name.is_empty())
            .unwrap_or(if id.is_empty() { "Part" } else { &id })
            .to_string();
        let program = first
            .events()
            .iter()
            .find_map(|event| match event.element() {
                StreamElement::Instrument(instrument) => instrument.midi_program(),
                _ => None,
            })
            .map(|program| program.min(127));
        if extras.drums.contains(&id) {
            imported.skipped.push(format!("{name}: drums"));
            continue;
        }
        let mut read = Vec::new();
        let mut refused = None;
        for (place, part) in staves.iter().enumerate() {
            // A staff of a part written on several says which in its id.
            let number = part
                .id()
                .and_then(|id| id.rsplit_once("-Staff"))
                .and_then(|(_, number)| number.parse::<usize>().ok())
                .unwrap_or(place + 1);
            match read_staff(part, &id, &extras, &mut imported) {
                Ok(staff) => read.push((number, staff)),
                Err(why) => {
                    refused = Some(why);
                    break;
                }
            }
        }
        if let Some(why) = refused {
            imported.skipped.push(format!("{name}: {why}"));
            continue;
        }
        let count = read.iter().filter(|(_, staff)| !staff.tablature).count();
        let doubled = count > 0;
        let mut index = 0;
        for (number, staff) in read {
            if staff.tablature && doubled {
                imported.skipped.push(format!(
                    "{name}: its tablature staff, which doubles the notes"
                ));
                continue;
            }
            index += 1;
            imported.parts.push(Part {
                name: if count > 1 {
                    format!("{name} ({index})")
                } else {
                    name.clone()
                },
                program,
                tuning: extras
                    .tuning
                    .get(&(id.clone(), number))
                    .cloned()
                    .unwrap_or_default(),
                clef: staff.clef,
                measures: staff.measures,
            });
        }
    }
    Ok(imported)
}
