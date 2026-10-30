//! The crate's slicing of a stream's notes against music21's:
//! `sliceByQuarterLengths`, `sliceByGreatestDivisor`, `sliceAtOffsets`,
//! `sliceByBeat` and `splitByClass`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Each part is sliced
//! by eighths, by a dotted-eighth-and-sixteenth pattern, by its greatest
//! divisor, at every offset of three-eighths, and by beat; the score by its
//! greatest divisor; and each part's notes and rests are split by length.
//! Every measure's notes, chords and rests -- where they start, how long
//! they are, their pitches and their ties -- must come out the same, or
//! both must refuse. Offsets and lengths are compared bit for bit.
//! `STREAM_SLICING_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::notation::TieType;
use music21_rs::{Stream, StreamElement, StreamKind};
use music21_rs_python_parity::answers;
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

def tie_of(e):
    if isinstance(e, chord.Chord) and not isinstance(e, harmony.ChordSymbol):
        return ','.join((n.tie.type if n.tie else '-') for n in e.notes)
    tie = getattr(e, 'tie', None)
    return tie.type if tie else '-'

def pitches(e):
    if isinstance(e, harmony.ChordSymbol):
        return ''
    if isinstance(e, (note.Note, chord.Chord)):
        return ' '.join(f"{p.step}{p.octave}:{'' if p.accidental is None else bits(p.accidental.alter)}"
                        for p in e.pitches)
    return ''

def describe(e, offset):
    return f'{kind(e)}@{bits(offset)}/{bits(e.quarterLength)}[{pitches(e)}]{tie_of(e)}'

def dump(s):
    lines = []
    measures = list(s.getElementsByClass(stream.Measure))
    holders = measures if measures else [s]
    for m in holders:
        items = sorted(describe(e, e.getOffsetInHierarchy(m))
                       for e in m.recurse() if kind(e) is not None)
        lines.append(' '.join(items))
    return lines

def attempt(label, run):
    try:
        return [label + ' ' + line for line in run()]
    except Exception as e:
        return [label + ' error']

def notes_of(part):
    out = stream.Stream()
    flat = part.flatten()
    for e in flat.notesAndRests:
        out.coreInsert(flat.elementOffset(e), e)
    out.coreElementsChanged()
    return out

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    lines = []
    for i, p in enumerate(score.parts):
        lines += attempt(f'eighths {i}', lambda: dump(p.sliceByQuarterLengths([0.5])))
        lines += attempt(f'dotted {i}', lambda: dump(p.sliceByQuarterLengths([0.75, 0.25])))
        lines += attempt(f'divisor {i}', lambda: dump(p.sliceByGreatestDivisor()))
        lines += attempt(f'offsets {i}', lambda: dump(p.sliceAtOffsets([x * 1.5 for x in range(400)])))
        lines += attempt(f'beat {i}', lambda: dump(p.sliceByBeat()))
        def split():
            a, b = notes_of(p).splitByClass(None, lambda e: e.quarterLength >= 1)
            return [' '.join(describe(e, a.elementOffset(e)) for e in a),
                    ' '.join(describe(e, b.elementOffset(e)) for e in b)]
        lines += attempt(f'split {i}', split)
    def score_divisor():
        out = []
        for p in score.sliceByGreatestDivisor().parts:
            out += dump(p)
        return out
    lines += attempt('score divisor', score_divisor)
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

fn pitch_text(pitch: &music21_rs::Pitch) -> String {
    let alter = pitch
        .written_accidental()
        .map_or(String::new(), |accidental| bits(accidental.alter()));
    let octave = pitch
        .octave()
        .map_or("None".to_string(), |octave| octave.to_string());
    let step = pitch.name().chars().next().unwrap_or('?');
    format!("{step}{octave}:{alter}")
}

fn tie_name(tie: Option<TieType>) -> &'static str {
    tie.map_or("-", |tie| tie.as_str())
}

fn describe(element: &StreamElement, offset: f64) -> String {
    let (pitches, tie) = match element {
        StreamElement::Note(note) => (
            pitch_text(note.pitch()),
            tie_name(note.tie().map(|tie| tie.tie_type())).to_string(),
        ),
        StreamElement::Chord(chord) => (
            chord
                .notes()
                .iter()
                .map(|note| pitch_text(note.pitch()))
                .collect::<Vec<_>>()
                .join(" "),
            chord
                .notes()
                .iter()
                .map(|note| tie_name(note.tie().map(|tie| tie.tie_type())))
                .collect::<Vec<_>>()
                .join(","),
        ),
        StreamElement::Rest(rest) => (
            String::new(),
            tie_name(rest.tie().map(|tie| tie.tie_type())).to_string(),
        ),
        StreamElement::Unpitched(stroke) => (
            String::new(),
            tie_name(stroke.written().tie().map(|tie| tie.tie_type())).to_string(),
        ),
        StreamElement::PercussionChord(chord) => (
            String::new(),
            tie_name(chord.written().tie().map(|tie| tie.tie_type())).to_string(),
        ),
        _ => (String::new(), "-".to_string()),
    };
    format!(
        "{}@{}/{}[{pitches}]{tie}",
        kind(element).unwrap_or("None"),
        bits(offset),
        bits(element.quarter_length())
    )
}

fn dump(stream: &Stream) -> Vec<String> {
    let measures = stream.measures();
    let holders: Vec<&Stream> = if measures.is_empty() {
        vec![stream]
    } else {
        measures
    };
    holders
        .into_iter()
        .map(|measure| {
            let mut items: Vec<String> = measure
                .recurse()
                .into_iter()
                .filter(|(_, element)| kind(element).is_some())
                .map(|(offset, element)| describe(element, offset))
                .collect();
            items.sort();
            items.join(" ")
        })
        .collect()
}

fn attempt(label: &str, run: impl FnOnce() -> music21_rs::Result<Vec<String>>) -> Vec<String> {
    match run() {
        Ok(lines) => lines
            .into_iter()
            .map(|line| format!("{label} {line}"))
            .collect(),
        Err(_) => vec![format!("{label} error")],
    }
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

fn our_report(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, part) in score.parts().into_iter().enumerate() {
        lines.extend(attempt(&format!("eighths {i}"), || {
            Ok(dump(&part.slice_by_quarter_lengths(&[0.5])?))
        }));
        lines.extend(attempt(&format!("dotted {i}"), || {
            Ok(dump(&part.slice_by_quarter_lengths(&[0.75, 0.25])?))
        }));
        lines.extend(attempt(&format!("divisor {i}"), || {
            Ok(dump(&part.slice_by_greatest_divisor()?))
        }));
        let offsets: Vec<f64> = (0..400).map(|x| f64::from(x) * 1.5).collect();
        lines.extend(attempt(&format!("offsets {i}"), || {
            Ok(dump(&part.slice_at_offsets(&offsets)?))
        }));
        lines.extend(attempt(&format!("beat {i}"), || {
            Ok(dump(&part.slice_by_beat()?))
        }));
        lines.extend(attempt(&format!("split {i}"), || {
            let (a, b) = notes_of(part).split_by(|element| element.quarter_length() >= 1.0);
            let line = |s: &Stream| {
                s.events()
                    .iter()
                    .map(|event| describe(event.element(), event.offset()))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            Ok(vec![line(&a), line(&b)])
        }));
    }
    lines.extend(attempt("score divisor", || {
        let sliced = score.slice_by_greatest_divisor()?;
        Ok(sliced.parts().into_iter().flat_map(dump).collect())
    }));
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
fn the_crate_slices_streams_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = answers::helpers(
            py,
            MUSIC21,
            c"stream_slicing_parity_music21.py",
            c"stream_slicing_parity_music21",
            &["report"],
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_SLICING_PARITY_SCORES") {
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
