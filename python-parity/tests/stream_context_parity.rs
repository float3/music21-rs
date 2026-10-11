//! The crate's keeping and taking out of a stream's elements, and its
//! harmonic intervals, against music21's: `extractContext`,
//! `removeByClass`, `removeByNotOfClass` and
//! `attachIntervalsBetweenStreams`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Around every fifth
//! element of each flattened part the context a quarter and a half before
//! and two after is taken; from a copy of each part's first measure the
//! rests, and everything but notes and chords, are taken out; and each
//! part's notes are given the interval to the next part's note sounding
//! with them. What is left -- each element's class, where it stands, how
//! long it is, its pitches, and the measure's barlines -- and each
//! interval's directed name must be the same, or both must refuse.
//! Offsets and lengths are compared bit for bit.
//! `STREAM_CONTEXT_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::notation::TieType;
use music21_rs::{Stream, StreamElement, StreamEvent, StreamKind};
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
    return ' '.join(found(s, e) for e in s._elements if not isinstance(e, bar.Barline))

def bar_type(b):
    return '-' if b is None else b.type

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    lines = []
    parts = list(score.parts)
    for i, p in enumerate(parts):
        flat = p.flatten()
        # The crate keeps spanners, and measures' barlines, beside the
        # elements.
        flat.removeByClass([spanner.Spanner, bar.Barline])
        elements = list(flat.elements)
        for at in range(0, len(elements), 5):
            lines += attempt(f'context {i} {at}', lambda at=at: [
                contents(flat.extractContext(elements[at], before=1.5, after=2.0))])
        measures = list(p.getElementsByClass(stream.Measure))
        if measures:
            def removed(run):
                m = copy.deepcopy(measures[0])
                run(m)
                return [contents(m) + f' {bar_type(m.leftBarline)}|{bar_type(m.rightBarline)}']
            lines += attempt(f'no-rests {i}', lambda: removed(lambda m: m.removeByClass(['Rest'])))
            lines += attempt(f'only-notes {i}', lambda: removed(
                lambda m: m.removeByNotOfClass(['Note', 'Chord'])))
            lines += attempt(f'no-bars {i}', lambda: removed(lambda m: m.removeByClass(['Barline'])))
        if i + 1 < len(parts):
            upper = copy.deepcopy(flat)
            lower = parts[i + 1].flatten()
            lower.removeByClass([spanner.Spanner, bar.Barline])
            upper.attachIntervalsBetweenStreams(lower)
            names = []
            for n in upper.getElementsByClass(note.Note):
                interval = n.editorial.harmonicInterval
                names.append('-' if interval is None else interval.directedName)
            lines.append(f'harmonic {i} ' + ' '.join(names))
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

fn our_report(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    let parts = score.parts();
    for (i, part) in parts.iter().enumerate() {
        let flat = part.flatten();
        for at in (0..flat.len()).step_by(5) {
            lines.extend(attempt(&format!("context {i} {at}"), || {
                Ok(vec![contents(&flat.extract_context(at, 1.5, 2.0)?)])
            }));
        }
        if let Some(first) = part.measures().first() {
            let removed = |run: &dyn Fn(&mut Stream)| -> music21_rs::Result<Vec<String>> {
                let mut measure = (*first).clone();
                run(&mut measure);
                Ok(vec![format!(
                    "{} {}|{}",
                    contents(&measure),
                    bar_type(measure.left_barline()),
                    bar_type(measure.right_barline())
                )])
            };
            lines.extend(attempt(&format!("no-rests {i}"), || {
                removed(&|measure| measure.remove_by_class(&["Rest"]))
            }));
            lines.extend(attempt(&format!("only-notes {i}"), || {
                removed(&|measure| measure.remove_by_not_of_class(&["Note", "Chord"]))
            }));
            lines.extend(attempt(&format!("no-bars {i}"), || {
                removed(&|measure| measure.remove_by_class(&["Barline"]))
            }));
        }
        if let Some(next) = parts.get(i + 1) {
            let names: Vec<String> = flat
                .harmonic_intervals(&next.flatten())
                .into_iter()
                .map(|(_, interval)| {
                    interval.map_or("-".to_string(), |interval| interval.directed_name())
                })
                .collect();
            lines.push(format!("harmonic {i} {}", names.join(" ")));
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
fn the_crate_keeps_and_takes_out_elements_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_context_parity_music21.py",
            c"stream_context_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_CONTEXT_PARITY_SCORES") {
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
