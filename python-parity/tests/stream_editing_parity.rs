//! The crate's editing of a stream against music21's: `extendTies`,
//! `scaleOffsets`, `scaleDurations`, `augmentOrDiminish`, `invertDiatonic`,
//! `shiftElements`, `repeatAppend`, `repeatInsert`, `insertAndShift`,
//! `barDurationProportion` and `padAsAnacrusis`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). A copy of each part
//! is edited each way, and of its first measure for the measure's
//! methods; every measure's notes, chords and rests -- where they start,
//! how long they are, their pitches and their ties -- must come out the
//! same, or both must refuse. Offsets and lengths are compared bit for bit.
//! `STREAM_EDITING_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::notation::TieType;
use music21_rs::{Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import copy
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

def measure_offsets(s):
    return ' '.join(bits(m.offset) for m in s.getElementsByClass(stream.Measure))

def flat_notes(s):
    flat = s.flatten()
    # A chord symbol's notes keep ties in music21, which the crate's chord
    # symbol has not got.
    return [' '.join(describe(e, flat.elementOffset(e)).replace(']stop', ']-').replace(']start', ']-').replace(']continue', ']-')
                     if isinstance(e, harmony.ChordSymbol) else describe(e, flat.elementOffset(e))
                     for e in flat.notes)]

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    lines = []
    for i, p in enumerate(score.parts):
        def ties(ignore):
            c = copy.deepcopy(p)
            c.extendTies(ignoreRests=ignore)
            return flat_notes(c)
        lines += attempt(f'ties {i}', lambda: ties(False))
        lines += attempt(f'ties-rests {i}', lambda: ties(True))
        def scaled(run):
            out = run()
            return [measure_offsets(out)] + dump(out)
        lines += attempt(f'offsets {i}', lambda: scaled(lambda: p.scaleOffsets(2)))
        lines += attempt(f'offsets-high {i}', lambda: scaled(
            lambda: p.scaleOffsets(0.5, anchorZero='highest', anchorZeroRecurse='lowest')))
        lines += attempt(f'durations {i}', lambda: scaled(lambda: p.scaleDurations(1.5)))
        lines += attempt(f'augment {i}', lambda: scaled(lambda: p.augmentOrDiminish(2)))
        lines += attempt(f'invert {i}', lambda: flat_notes(p.invertDiatonic()))
        lines += attempt(f'invert-e {i}', lambda: flat_notes(p.invertDiatonic(note.Note('E4'))))
        measures = list(p.getElementsByClass(stream.Measure))
        if measures:
            def edited(run):
                m = copy.deepcopy(measures[0])
                run(m)
                return dump(m) + [bits(m.paddingLeft)]
            lines += attempt(f'shift {i}', lambda: edited(
                lambda m: m.shiftElements(1.0, startOffset=1.0, endOffset=3.0)))
            lines += attempt(f'append {i}', lambda: edited(
                lambda m: m.repeatAppend(note.Note('C4'), 3)))
            lines += attempt(f'repeat {i}', lambda: edited(
                lambda m: m.repeatInsert(note.Note('D4'), [0, 1.5])))
            lines += attempt(f'insert-shift {i}', lambda: edited(
                lambda m: m.insertAndShift(1.0, note.Note('E4', quarterLength=2))))
            lines += attempt(f'pad {i}', lambda: edited(lambda m: m.padAsAnacrusis()))
            lines += attempt(f'pad-rests {i}', lambda: edited(
                lambda m: m.padAsAnacrusis(useInitialRests=True)))
            lines += attempt(f'proportion {i}', lambda: [
                ' '.join(bits(m.barDurationProportion()) for m in measures)])
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

fn measure_offsets(stream: &Stream) -> String {
    stream
        .events()
        .iter()
        .filter(|event| matches!(event.element(), StreamElement::Stream(inner) if inner.kind() == StreamKind::Measure))
        .map(|event| bits(event.offset()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn flat_notes(stream: &Stream) -> Vec<String> {
    vec![
        stream
            .flatten()
            .events()
            .iter()
            .filter(|event| kind(event.element()).is_some_and(|kind| kind != "rest"))
            .map(|event| describe(event.element(), event.offset()))
            .collect::<Vec<_>>()
            .join(" "),
    ]
}

fn edited(
    measure: &Stream,
    run: impl FnOnce(&mut Stream) -> music21_rs::Result<()>,
) -> music21_rs::Result<Vec<String>> {
    let mut copy = measure.clone();
    run(&mut copy)?;
    let mut lines = dump(&copy);
    lines.push(bits(copy.padding_left()));
    Ok(lines)
}

fn our_report(score: &Stream) -> Vec<String> {
    use music21_rs::stream::Anchor;
    use music21_rs::{Duration, Note, Pitch};
    let mut lines = Vec::new();
    for (i, part) in score.parts().into_iter().enumerate() {
        for (label, ignore) in [("ties", false), ("ties-rests", true)] {
            lines.extend(attempt(&format!("{label} {i}"), || {
                let mut copy = part.clone();
                copy.extend_ties(ignore);
                Ok(flat_notes(&copy))
            }));
        }
        let scaled = |out: Stream| -> music21_rs::Result<Vec<String>> {
            let mut lines = vec![measure_offsets(&out)];
            lines.extend(dump(&out));
            Ok(lines)
        };
        lines.extend(attempt(&format!("offsets {i}"), || {
            scaled(part.scale_offsets(2.0, Anchor::Lowest, Anchor::Zero)?)
        }));
        lines.extend(attempt(&format!("offsets-high {i}"), || {
            scaled(part.scale_offsets(0.5, Anchor::Highest, Anchor::Lowest)?)
        }));
        lines.extend(attempt(&format!("durations {i}"), || {
            scaled(part.scale_durations(1.5)?)
        }));
        lines.extend(attempt(&format!("augment {i}"), || {
            scaled(part.augment_or_diminish(2.0)?)
        }));
        lines.extend(attempt(&format!("invert {i}"), || {
            Ok(flat_notes(&part.invert_diatonic(None)?))
        }));
        lines.extend(attempt(&format!("invert-e {i}"), || {
            Ok(flat_notes(
                &part.invert_diatonic(Some(&Pitch::from_name("E4")?))?,
            ))
        }));
        let measures = part.measures();
        let Some(first) = measures.first() else {
            continue;
        };
        lines.extend(attempt(&format!("shift {i}"), || {
            edited(first, |m| {
                m.shift_elements(1.0, Some(1.0), Some(3.0));
                Ok(())
            })
        }));
        lines.extend(attempt(&format!("append {i}"), || {
            edited(first, |m| {
                m.repeat_append(&Note::from_name("C4")?.into(), 3);
                Ok(())
            })
        }));
        lines.extend(attempt(&format!("repeat {i}"), || {
            edited(first, |m| {
                m.repeat_insert(&Note::from_name("D4")?.into(), &[0.0, 1.5]);
                Ok(())
            })
        }));
        lines.extend(attempt(&format!("insert-shift {i}"), || {
            edited(first, |m| {
                let note = Note::from_name("E4")?.with_duration(Duration::half());
                m.insert_and_shift(1.0, note.into());
                Ok(())
            })
        }));
        lines.extend(attempt(&format!("pad {i}"), || {
            edited(first, |m| m.pad_as_anacrusis(None, false))
        }));
        lines.extend(attempt(&format!("pad-rests {i}"), || {
            edited(first, |m| m.pad_as_anacrusis(None, true))
        }));
        lines.extend(attempt(&format!("proportion {i}"), || {
            let mut meter = None;
            let mut out = Vec::new();
            for measure in &measures {
                if let Some(found) =
                    measure
                        .events()
                        .iter()
                        .find_map(|event| match event.element() {
                            StreamElement::TimeSignature(found) => Some(found.clone()),
                            _ => None,
                        })
                {
                    meter = Some(found);
                }
                out.push(bits(measure.bar_duration_proportion(meter.as_ref())?));
            }
            Ok(vec![out.join(" ")])
        }));
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
fn the_crate_edits_streams_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_editing_parity_music21.py",
            c"stream_editing_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_EDITING_PARITY_SCORES") {
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
