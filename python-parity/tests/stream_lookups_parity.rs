//! The crate's finding of elements by where they stand against music21's:
//! `getElementAtOrBefore`, `getElementBeforeOffset`,
//! `getElementAfterElement`, `beatAndMeasureFromOffset`,
//! `getTimeSignatures` and `getInstruments`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Each part is asked at
//! every measure's start, a third and a half of a quarter into it, before
//! nought and past its end, for the note or rest and the clef, key or meter
//! at or before that point in the flattened part, for the measure there,
//! and for the beat and measure; for the next note after each note; and
//! the score, each part and each measure for their time signatures and
//! instruments. What is found -- its class, where it stands, how long it
//! is, its pitches -- must be the same, or both must find nothing or
//! refuse. Offsets and lengths are compared bit for bit.
//! `STREAM_LOOKUPS_PARITY_SCORES`, a `;`-separated list of corpus names,
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
from music21 import chord, clef, converter, corpus, harmony, key, meter, note, percussion, stream
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

def points(p):
    out = [-1.0]
    for m in p.getElementsByClass(stream.Measure):
        o = p.elementOffset(m)
        out += [o, opFrac(o + Fraction(1, 3)), opFrac(o + 0.5)]
    out += [p.highestTime, opFrac(p.highestTime + 1)]
    return out

def meters(s, label):
    return [label + ' ' + ' '.join(f'{e.ratioString}@{bits(e.offset)}'
                                   for e in s.getTimeSignatures(searchContext=False))]

def instruments(s, label):
    return [label + ' ' + ' '.join(f'{e.instrumentName}|{e.partName}'
                                   for e in s.getInstruments(searchActiveSite=False))]

SETTINGS = ['Clef', 'KeySignature', 'TimeSignature']

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    lines = []
    lines += attempt('score-meters', lambda: meters(score, ''))
    lines += attempt('score-instruments', lambda: instruments(score, ''))
    lines += attempt('score-beat', lambda: [
        bits(score.beatAndMeasureFromOffset(1.0)[0])])
    for i, p in enumerate(score.parts):
        flat = p.flatten()
        lines += attempt(f'meters {i}', lambda: meters(p, ''))
        lines += attempt(f'instruments {i}', lambda: instruments(p, ''))
        for x in points(p):
            label = f'{i} {bits(x)}'
            lines.append(f'at {label} ' + found(flat, flat.getElementAtOrBefore(x, ['GeneralNote'])))
            lines.append(f'before {label} ' + found(flat, flat.getElementBeforeOffset(x, ['GeneralNote'])))
            lines.append(f'setting {label} ' + found(flat, flat.getElementAtOrBefore(x, SETTINGS)))
            lines.append(f'measure {label} ' + found(p, p.getElementAtOrBefore(x, ['Measure'])))
            def beat(x=x):
                b, m = p.beatAndMeasureFromOffset(x)
                return [f'{bits(b)} {m.number}@{bits(p.elementOffset(m))}']
            lines += attempt(f'beat {label}', beat)
            lines += attempt(f'beat-raw {label}', lambda x=x: [
                bits(p.beatAndMeasureFromOffset(x, fixZeros=False)[0])])
        notes = list(flat.notes)
        lines.append(f'after {i} ' + ' '.join(
            found(flat, flat.getElementAfterElement(n, ['Note'])) for n in notes))
        for m in p.getElementsByClass(stream.Measure):
            lines += attempt(f'measure-meters {i} {m.number}', lambda m=m: meters(m, ''))
            lines += attempt(f'measure-instruments {i} {m.number}', lambda m=m: instruments(m, ''))
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

/// music21's `opFrac`, for the points asked about.
fn snapped(value: f64) -> f64 {
    music21_rs::makenotation::op_frac(value)
}

fn points(part: &Stream) -> Vec<f64> {
    let mut out = vec![-1.0];
    for event in part.events() {
        if let StreamElement::Stream(measure) = event.element()
            && measure.kind() == StreamKind::Measure
        {
            let at = event.offset();
            out.extend([at, snapped(at + 1.0 / 3.0), snapped(at + 0.5)]);
        }
    }
    let end = snapped(part.end_offset());
    out.extend([end, snapped(end + 1.0)]);
    out
}

fn meters(stream: &Stream) -> music21_rs::Result<Vec<String>> {
    Ok(vec![format!(
        " {}",
        stream
            .time_signatures(true)?
            .iter()
            .map(|(offset, meter)| format!("{}@{}", meter.ratio_string(), bits(*offset)))
            .collect::<Vec<_>>()
            .join(" ")
    )])
}

fn instruments(stream: &Stream) -> music21_rs::Result<Vec<String>> {
    Ok(vec![format!(
        " {}",
        stream
            .instruments(true, true)
            .iter()
            .map(|instrument| {
                format!(
                    "{}|{}",
                    instrument.name().unwrap_or("None"),
                    instrument.part_name().unwrap_or("None")
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    )])
}

const SETTINGS: [&str; 3] = ["Clef", "KeySignature", "TimeSignature"];

fn our_report(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    lines.extend(attempt("score-meters", || meters(score)));
    lines.extend(attempt("score-instruments", || instruments(score)));
    lines.extend(attempt("score-beat", || {
        Ok(vec![bits(score.beat_and_measure_from_offset(1.0, true)?.0)])
    }));
    for (i, part) in score.parts().into_iter().enumerate() {
        let flat = part.flatten();
        lines.extend(attempt(&format!("meters {i}"), || meters(part)));
        lines.extend(attempt(&format!("instruments {i}"), || instruments(part)));
        for x in points(part) {
            let label = format!("{i} {}", bits(x));
            lines.push(format!(
                "at {label} {}",
                found(flat.element_at_or_before(x, &["GeneralNote"]))
            ));
            lines.push(format!(
                "before {label} {}",
                found(flat.element_before_offset(x, &["GeneralNote"]))
            ));
            lines.push(format!(
                "setting {label} {}",
                found(flat.element_at_or_before(x, &SETTINGS))
            ));
            lines.push(format!(
                "measure {label} {}",
                found(part.element_at_or_before(x, &["Measure"]))
            ));
            lines.extend(attempt(&format!("beat {label}"), || {
                let (beat, measure) = part.beat_and_measure_from_offset(x, true)?;
                let at = part
                    .events()
                    .iter()
                    .find(|event| matches!(event.element(), StreamElement::Stream(inner) if std::ptr::eq(&**inner, measure)))
                    .map_or(f64::NAN, StreamEvent::offset);
                Ok(vec![format!("{} {}@{}", bits(beat), measure.number(), bits(at))])
            }));
            lines.extend(attempt(&format!("beat-raw {label}"), || {
                Ok(vec![bits(part.beat_and_measure_from_offset(x, false)?.0)])
            }));
        }
        let after: Vec<String> = flat
            .events()
            .iter()
            .enumerate()
            .filter(|(_, event)| event.element().is_instance("NotRest"))
            .map(|(index, _)| found(flat.element_after(index, &["Note"]).ok().flatten()))
            .collect();
        lines.push(format!("after {i} {}", after.join(" ")));
        for measure in part.measures() {
            let number = measure.number();
            lines.extend(attempt(&format!("measure-meters {i} {number}"), || {
                meters(measure)
            }));
            lines.extend(attempt(
                &format!("measure-instruments {i} {number}"),
                || instruments(measure),
            ));
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
fn the_crate_finds_elements_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_lookups_parity_music21.py",
            c"stream_lookups_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_LOOKUPS_PARITY_SCORES") {
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
