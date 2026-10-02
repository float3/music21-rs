//! A score the crate read, turned into what the editor writes ABC from.
//! Every format read by one of the crate's readers -- music21's, ported --
//! comes through here: MusicXML, MIDI, Humdrum, MEI, RomanText and
//! TinyNotation. A part written on several staves, a piano's, comes back as
//! a part per staff, a tablature staff doubling the notes of a staff above
//! it is left out, and so is a part of unpitched percussion.

use super::{Beat, Imported, KeySig, Measure, Note, Part, TICKS, written_ticks};
use music21_rs::{
    ChordSymbol, Clef, Duration, DurationType, Instrument, Lyric, NoteSize, Stream, StreamElement,
    StreamKind, TieType,
};
use std::collections::{HashMap, HashSet};

/// What the editor wants that is no part of the score music21 models, read
/// off the document beside it where its format says it, by part id.
#[derive(Default)]
pub(super) struct Extras {
    /// Parts playing on the General MIDI drum channel.
    pub(super) drums: HashSet<String>,
    /// Open strings of a staff, lowest first, by part and staff number.
    pub(super) tuning: HashMap<(String, usize), Vec<i32>>,
    /// A rehearsal mark, by part and the measure's place in it.
    pub(super) markers: HashMap<(String, usize), String>,
    /// Whether each chord's lyric is written under it: a RomanText score's
    /// lyrics are its numerals.
    pub(super) figures: bool,
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

/// One note, chord or rest as a beat, at concert pitch, with its first
/// lyric as its figure where `figures` asks for it.
fn beat_of(element: &StreamElement, transpose: i32, figures: bool) -> Option<Beat> {
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
    let (notes, lyrics): (Vec<Note>, &[Lyric]) = match element {
        StreamElement::Note(note) => (sounded(note).into_iter().collect(), note.lyrics()),
        StreamElement::Chord(chord) => (
            chord.notes().iter().filter_map(sounded).collect(),
            chord.lyrics(),
        ),
        StreamElement::Rest(rest) => (Vec::new(), rest.lyrics()),
        // The notes struck with a drum keep their pitches; the strokes are
        // left out.
        StreamElement::PercussionChord(chord) => {
            let written = chord.written();
            let pitched: Vec<Note> = written
                .notes()
                .iter()
                .enumerate()
                .filter(|(index, _)| !chord.is_unpitched(*index))
                .filter_map(|(_, note)| sounded(note))
                .collect();
            if pitched.is_empty() {
                return None;
            }
            (pitched, written.lyrics())
        }
        _ => return None,
    };
    let grace = duration.is_grace();
    let length = ticks(duration.quarter_length());
    let components = duration.components();
    let mut beat = Beat {
        grace,
        notes,
        figure: lyrics
            .first()
            .filter(|_| figures)
            .map(Lyric::text)
            .filter(|text| !text.is_empty()),
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
    /// Whether it held strokes with no pitch, which are left out: a MIDI
    /// track may play the drums beside its notes.
    strokes: bool,
}

/// The instruments of a part with where each starts playing, in order:
/// those standing in the part itself, where MusicXML puts them, and those
/// standing in its measures, where a MIDI track's is.
fn instruments_of(part: &Stream) -> Vec<(f64, &Instrument)> {
    let mut found: Vec<(f64, &Instrument)> = Vec::new();
    for event in part.events() {
        match event.element() {
            StreamElement::Instrument(instrument) => found.push((event.offset(), instrument)),
            StreamElement::Stream(measure) => {
                for held in measure.events() {
                    if let StreamElement::Instrument(instrument) = held.element() {
                        found.push((event.offset() + held.offset(), instrument));
                    }
                }
            }
            _ => {}
        }
    }
    found.sort_by(|left, right| left.0.total_cmp(&right.0));
    found
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
    let instruments = instruments_of(part);
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
                    staff.strokes = true;
                }
                let Some(beat) = beat_of(held.element(), transpose, extras.figures) else {
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
    let pitched = staff
        .measures
        .iter()
        .flat_map(|measure| measure.voices.iter().flatten())
        .any(|beat| !beat.notes.is_empty());
    if staff.strokes && !pitched {
        return Err("unpitched percussion".to_string());
    }
    Ok(staff)
}

/// The score to import out of what a reader read: the score itself, the
/// first of an opus's, or a lone part standing for a score of one.
fn first_score(read: &Stream, imported: &mut Imported) -> Vec<Stream> {
    match read.kind() {
        StreamKind::Opus => {
            let scores = read.streams_of_kind(StreamKind::Score);
            if scores.len() > 1 {
                imported.skipped.push(format!(
                    "the {} scores after the first in the file",
                    scores.len() - 1
                ));
            }
            scores
                .first()
                .map(|score| vec![(*score).clone()])
                .unwrap_or_default()
        }
        StreamKind::Part | StreamKind::PartStaff => vec![read.clone()],
        _ => Vec::new(),
    }
}

fn title_of(score: &Stream) -> Option<String> {
    let metadata = score.metadata()?;
    metadata
        .first_text("title")
        .or_else(|| metadata.first_text("movementName"))
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
}

/// Turns a score a reader read into what the editor writes ABC from, with
/// what the document said beside it.
pub(super) fn read(stream: &Stream, extras: &Extras) -> Result<Imported, String> {
    let mut imported = Imported {
        title: title_of(stream).unwrap_or_default(),
        ..Imported::default()
    };
    let held = first_score(stream, &mut imported);
    let score = held.first().unwrap_or(stream);
    if let Some(title) = title_of(score) {
        imported.title = title;
    }
    let parts = match score.kind() {
        StreamKind::Part | StreamKind::PartStaff => vec![score],
        _ => score.parts(),
    };

    // The score's parts, a part written on several staves being several
    // that name one part of the document between them.
    let mut groups: Vec<(String, Vec<&Stream>)> = Vec::new();
    for part in parts {
        let id = instruments_of(part)
            .first()
            .and_then(|(_, instrument)| instrument.part_id())
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
        let instruments = instruments_of(first);
        // A part is called what the score calls it, or else what its
        // instrument is: a MIDI track's name is its instrument's.
        let name = first
            .name()
            .or_else(|| {
                instruments
                    .first()
                    .and_then(|(_, instrument)| instrument.best_name())
            })
            .map(|name| name.trim().trim_end_matches(':').trim())
            .filter(|name| !name.is_empty())
            .unwrap_or(if id.is_empty() { "Part" } else { &id })
            .to_string();
        let program = instruments
            .iter()
            .find_map(|(_, instrument)| instrument.midi_program())
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
            match read_staff(part, &id, extras, &mut imported) {
                Ok(staff) => {
                    if staff.strokes {
                        imported
                            .skipped
                            .push(format!("{name}: its unpitched percussion"));
                    }
                    read.push((number, staff));
                }
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
