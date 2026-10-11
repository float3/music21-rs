//! The crate's questions of a stream's layout against music21's: gaps,
//! overlaps, what sounds when an element starts or while it sounds,
//! simultaneous attacks, offset maps, measure offset maps, highest and
//! lowest offsets, and whether a stream is well-formed notation.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). The layering
//! questions are asked of each part's notes and rests, flattened, since
//! music21's flattened part also holds its spanners, which the crate keeps
//! apart; the rest of each part, measure and the score itself. Offsets and
//! lengths are compared bit for bit. `STREAM_QUERIES_PARITY_SCORES`, a
//! `;`-separated list of corpus names, runs those instead of the writer
//! test's scores, which is how the corpus is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::{Stream, StreamElement, StreamEvent, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import struct
import zipfile
from music21 import chord, converter, corpus, harmony, note, percussion, stream

def bits(value):
    return str(struct.unpack('<Q', struct.pack('<d', float(value)))[0])

def source_text(name):
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    path = str(path)
    lower = path.lower()
    if lower.endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            names = [n for n in archive.namelist()
                     if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
            data = archive.read(names[0])
    elif lower.endswith(('.xml', '.musicxml')):
        with open(path, 'rb') as handle:
            data = handle.read()
    else:
        return None
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')

def kind(e):
    if isinstance(e, (harmony.ChordSymbol, harmony.NoChord)):
        return 'symbol'
    if isinstance(e, percussion.PercussionChord):
        return 'pchord'
    if isinstance(e, chord.Chord):
        return 'chord'
    if isinstance(e, note.Unpitched):
        return 'unpitched'
    if isinstance(e, note.Note):
        return 'note'
    if isinstance(e, note.Rest):
        return 'rest'
    return None

def describe(e, offset):
    return f'{kind(e)}@{bits(offset)}/{bits(e.duration.quarterLength)}'

def notes_of(part):
    out = stream.Stream()
    flat = part.flatten()
    for e in flat.notesAndRests:
        out.coreInsert(flat.elementOffset(e), e)
    out.coreElementsChanged()
    return out

def gaps(s):
    found = s.findGaps()
    if found is None:
        return 'none'
    return ' '.join(f'{bits(found.elementOffset(r))}/{bits(r.quarterLength)}' for r in found)

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    lines = [f'score well-formed {score.isWellFormedNotation()}']
    lines.append('score measures ' + ' '.join(
        f'{bits(o)}:' + ','.join(str(m.number) for m in ms)
        for o, ms in score.measureOffsetMap().items()))
    parts = list(score.parts)
    notes = [notes_of(p) for p in parts]
    for i, p in enumerate(parts):
        lines.append(f'part {i} {p.hasMeasures()} {p.hasVoices()} {p.isWellFormedNotation()}')
        lines.append(f'part {i} measures ' + ' '.join(
            f'{bits(o)}:' + ','.join(str(m.number) for m in ms)
            for o, ms in p.measureOffsetMap().items()))
        s = notes[i]
        lines.append(f'part {i} gaps {gaps(s)} gapless {s.isGapless} sequence {s.isSequence()}')
        overlaps = s.getOverlaps()
        lines.append(f'part {i} overlaps ' + ' | '.join(
            bits(o) + ': ' + ' '.join(sorted(describe(e, s.elementOffset(e)) for e in es))
            for o, es in overlaps.items()))
        for mn, m in enumerate(p.getElementsByClass(stream.Measure)):
            entries = sorted(
                f'{kind(om.element)}@{bits(om.offset)}-{bits(om.endTime)}v{om.voiceIndex}'
                for om in m.offsetMap() if kind(om.element))
            lines.append(f'measure {i} {mn} {m.hasVoices()} {m.isWellFormedNotation()} '
                         f'{bits(m.highestOffset)} {bits(m.lowestOffset)} gaps {gaps(m)} map '
                         + ' '.join(entries))
        if i + 1 < len(parts):
            other = notes[i + 1]
            lines.append(f'attacks {i} ' + ' '.join(bits(o) for o in s.simultaneousAttacks(other)))
            for e in list(s)[:40]:
                o = s.elementOffset(e)
                found = other.playingWhenAttacked(e, s)
                lines.append(f'attacked {i} {describe(e, o)} '
                             + ('none' if found is None else describe(found, other.elementOffset(found))))
                sounding = other.allPlayingWhileSounding(e, s)
                lines.append(f'sounding {i} {describe(e, o)} ' + ' '.join(
                    describe(x, x.offset) for x in sounding))
    return lines
"#;

fn bits(value: f64) -> String {
    value.to_bits().to_string()
}

fn kind(element: &StreamElement) -> Option<&'static str> {
    Some(match element {
        StreamElement::ChordSymbol(_) => "symbol",
        StreamElement::PercussionChord(_) => "pchord",
        StreamElement::Chord(_) => "chord",
        StreamElement::Unpitched(_) => "unpitched",
        StreamElement::Note(_) => "note",
        StreamElement::Rest(_) => "rest",
        _ => return None,
    })
}

fn describe(event: &StreamEvent) -> String {
    format!(
        "{}@{}/{}",
        kind(event.element()).unwrap_or("None"),
        bits(event.offset()),
        bits(event.element().quarter_length())
    )
}

fn notes_of(part: &Stream) -> Stream {
    Stream::from_events(
        part.flatten()
            .events()
            .iter()
            .filter(|event| kind(event.element()).is_some())
            .cloned(),
    )
}

fn gaps(stream: &Stream) -> String {
    match stream.find_gaps() {
        None => "none".to_string(),
        Some(found) => found
            .events()
            .iter()
            .map(|event| {
                format!(
                    "{}/{}",
                    bits(event.offset()),
                    bits(event.element().quarter_length())
                )
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn measure_map(stream: &Stream) -> String {
    stream
        .measure_offset_map()
        .iter()
        .map(|(offset, measures)| {
            let numbers: Vec<String> = measures.iter().map(|m| m.number().to_string()).collect();
            format!("{}:{}", bits(*offset), numbers.join(","))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn title(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

fn our_report(score: &Stream) -> Vec<String> {
    let mut lines = vec![format!(
        "score well-formed {}",
        title(score.is_well_formed_notation())
    )];
    lines.push(format!("score measures {}", measure_map(score)));
    let parts = score.parts();
    let notes: Vec<Stream> = parts.iter().map(|part| notes_of(part)).collect();
    for (i, part) in parts.iter().enumerate() {
        lines.push(format!(
            "part {i} {} {} {}",
            title(part.has_measures()),
            title(part.has_voices()),
            title(part.is_well_formed_notation())
        ));
        lines.push(format!("part {i} measures {}", measure_map(part)));
        let s = &notes[i];
        lines.push(format!(
            "part {i} gaps {} gapless {} sequence {}",
            gaps(s),
            title(s.is_gapless()),
            title(s.is_sequence())
        ));
        let overlaps: Vec<String> = s
            .overlaps()
            .iter()
            .map(|(offset, events)| {
                let mut described: Vec<String> = events.iter().map(describe).collect();
                described.sort();
                format!("{}: {}", bits(*offset), described.join(" "))
            })
            .collect();
        lines.push(format!("part {i} overlaps {}", overlaps.join(" | ")));
        for (mn, measure) in part.measures().into_iter().enumerate() {
            let mut entries: Vec<String> = measure
                .offset_map()
                .iter()
                .filter_map(|entry| {
                    kind(&entry.element).map(|kind| {
                        format!(
                            "{kind}@{}-{}v{}",
                            bits(entry.offset),
                            bits(entry.end_time),
                            entry.voice.map_or("None".to_string(), |v| v.to_string())
                        )
                    })
                })
                .collect();
            entries.sort();
            lines.push(format!(
                "measure {i} {mn} {} {} {} {} gaps {} map {}",
                title(measure.has_voices()),
                title(measure.is_well_formed_notation()),
                bits(measure.highest_offset()),
                bits(measure.lowest_offset()),
                gaps(measure),
                entries.join(" ")
            ));
        }
        if let Some(other) = notes.get(i + 1) {
            let attacks: Vec<String> = s
                .simultaneous_attacks(other)
                .into_iter()
                .map(bits)
                .collect();
            lines.push(format!("attacks {i} {}", attacks.join(" ")));
            for event in s.events().iter().take(40) {
                let found = other
                    .playing_when_attacked(event.element(), event.offset())
                    .map_or("none".to_string(), describe);
                lines.push(format!("attacked {i} {} {found}", describe(event)));
                let sounding: Vec<String> = other
                    .all_playing_while_sounding(event.element(), event.offset())
                    .events()
                    .iter()
                    .map(describe)
                    .collect();
                lines.push(format!(
                    "sounding {i} {} {}",
                    describe(event),
                    sounding.join(" ")
                ));
            }
        }
    }
    lines
}

/// The first line two reports differ on, and what each has there.
fn first_difference(ours: &[String], theirs: &[String]) -> String {
    let at = ours
        .iter()
        .zip(theirs)
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(theirs.len()));
    let from = match (ours.get(at), theirs.get(at)) {
        (Some(a), Some(b)) => a
            .chars()
            .zip(b.chars())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()))
            .saturating_sub(120),
        _ => 0,
    };
    let cut = |line: Option<&String>| {
        line.map(|line| line.chars().skip(from).take(400).collect::<String>())
    };
    format!(
        "{} lines and {} in music21; first difference at line {at}:\n  music21    {:?}\n  music21-rs {:?}",
        ours.len(),
        theirs.len(),
        cut(theirs.get(at)),
        cut(ours.get(at))
    )
}

#[test]
fn the_crate_answers_questions_of_layout_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_queries_parity_music21.py",
            c"stream_queries_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_QUERIES_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES
                .iter()
                .map(|(name, _)| name.to_string())
                .filter(|name| !name.starts_with("built:"))
                .collect(),
        };
        let mut compared = 0;
        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let score = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            if score.kind() != StreamKind::Score {
                continue;
            }
            compared += 1;
            let theirs: Vec<String> = music21.getattr("report")?.call1((&text,))?.extract()?;
            let ours = our_report(&score);
            if ours != theirs {
                failures.push(format!("{name}: {}", first_difference(&ours, &theirs)));
            }
        }
        Ok((failures, compared))
    })
    .expect("the Python side runs");

    println!("{compared} scores compared");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
