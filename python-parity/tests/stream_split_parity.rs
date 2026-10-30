//! The crate's cutting of a measure in two against music21's
//! `splitAtQuarterLength`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Every measure of
//! every part is cut a quarter in, a third of a quarter in, halfway, at
//! its end and past it, music21 finding what is in force before the
//! measure in its part and the crate given it by `in_force_before`. Both
//! halves -- each element's class, where it stands, how long it is, its
//! pitches and ties, and the barlines -- must be the same, or both must
//! refuse. Offsets and lengths are compared bit for bit.
//! `STREAM_SPLIT_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::notation::TieType;
use music21_rs::stream::InForce;
use music21_rs::{Stream, StreamElement, StreamEvent, StreamKind};
use music21_rs_python_parity::answers;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import copy
import struct
import zipfile
from fractions import Fraction
from music21 import bar, chord, clef, converter, corpus, harmony, key, meter, note, percussion, spanner, stream
from music21.common.numberTools import opFrac

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

def found(s, e):
    if e is None:
        return 'None'
    if isinstance(e, meter.TimeSignature):
        return f'meter:{e.ratioString}@{bits(s.elementOffset(e))}'
    if isinstance(e, key.KeySignature):
        return f'key:{e.sharps}@{bits(s.elementOffset(e))}'
    if isinstance(e, clef.Clef):
        return f'clef:{e.sign}{e.line}/{e.octaveChange}@{bits(s.elementOffset(e))}'
    if isinstance(e, stream.Measure):
        return f'measure:{e.number}@{bits(s.elementOffset(e))}'
    return describe(e, s.elementOffset(e))

def contents(s):
    # The crate keeps a measure's barlines beside its elements.
    return ' '.join(found(s, e) for e in s if not isinstance(e, bar.Barline))

def bar_type(b):
    return '-' if b is None else b.type

def halves(m, at):
    left, right = m.splitAtQuarterLength(at, retainOrigin=False)
    return [
        'left ' + contents(left) + f' {bar_type(left.leftBarline)}|{bar_type(left.rightBarline)}',
        'right ' + contents(right) + f' {bar_type(right.leftBarline)}|{bar_type(right.rightBarline)}',
    ]

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    lines = []
    for i, p in enumerate(score.parts):
        for j, m in enumerate(p.getElementsByClass(stream.Measure)):
            h = m.highestTime
            for at in (1.0, Fraction(1, 3), opFrac(h / 2), h, opFrac(h + 1)):
                lines += attempt(f'{i} {j} {bits(at)}', lambda m=m, at=at: halves(m, at))
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

fn attempt(label: &str, run: impl FnOnce() -> music21_rs::Result<Vec<String>>) -> Vec<String> {
    match run() {
        Ok(lines) => lines
            .into_iter()
            .map(|line| format!("{label} {line}"))
            .collect(),
        Err(_) => vec![format!("{label} error")],
    }
}

fn sharps(signature: &music21_rs::key::keysignature::KeySignature) -> String {
    signature
        .sharps()
        .map_or("None".to_string(), |sharps| sharps.to_string())
}

fn found(event: Option<&StreamEvent>) -> String {
    let Some(event) = event else {
        return "None".to_string();
    };
    let at = bits(event.offset());
    match event.element() {
        StreamElement::TimeSignature(meter) => format!("meter:{}@{at}", meter.ratio_string()),
        StreamElement::KeySignature(signature) => format!("key:{}@{at}", sharps(signature)),
        StreamElement::Key(key) => format!("key:{}@{at}", sharps(&key.key_signature())),
        StreamElement::Clef(clef) => format!(
            "clef:{}{}/{}@{at}",
            clef.sign().unwrap_or("None"),
            clef.line()
                .map_or("None".to_string(), |line| line.to_string()),
            clef.octave_change()
        ),
        StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
            format!("measure:{}@{at}", measure.number())
        }
        element => describe(element, event.offset()),
    }
}

fn contents(stream: &Stream) -> String {
    stream
        .events()
        .iter()
        .filter(|event| !matches!(event.element(), StreamElement::Barline(_)))
        .map(|event| found(Some(event)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn bar_type(barline: Option<&music21_rs::bar::Barline>) -> &'static str {
    barline.map_or("-", |barline| barline.bar_type().as_str())
}

fn halves(measure: &Stream, at: f64, in_force: &InForce) -> music21_rs::Result<Vec<String>> {
    let (left, right) = measure.split_at_quarter_length(at, in_force)?;
    Ok(vec![
        format!(
            "left {} {}|{}",
            contents(&left),
            bar_type(left.left_barline()),
            bar_type(left.right_barline())
        ),
        format!(
            "right {} {}|{}",
            contents(&right),
            bar_type(right.left_barline()),
            bar_type(right.right_barline())
        ),
    ])
}

fn our_report(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, part) in score.parts().into_iter().enumerate() {
        let measures: Vec<(f64, &Stream)> = part
            .events()
            .iter()
            .filter_map(|event| match event.element() {
                StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure => {
                    Some((event.offset(), &**inner))
                }
                _ => None,
            })
            .collect();
        for (j, (offset, measure)) in measures.into_iter().enumerate() {
            let in_force = part.in_force_before(offset);
            let h = snapped(measure.end_offset());
            for at in [
                1.0,
                snapped(1.0 / 3.0),
                snapped(h / 2.0),
                h,
                snapped(h + 1.0),
            ] {
                lines.extend(attempt(&format!("{i} {j} {}", bits(at)), || {
                    halves(measure, at, &in_force)
                }));
            }
        }
    }
    lines
}

/// music21's `opFrac`.
fn snapped(value: f64) -> f64 {
    music21_rs::makenotation::op_frac(value)
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
fn the_crate_splits_measures_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = answers::helpers(
            py,
            MUSIC21,
            c"stream_split_parity_music21.py",
            c"stream_split_parity_music21",
            &["report"],
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_SPLIT_PARITY_SCORES") {
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
