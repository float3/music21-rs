//! The crate's moving of music between voices and parts against
//! music21's: `voicesToParts` (by place and by id) and `partsToVoices` (two
//! parts to a part, one part to a part, and all parts in one, stems set or
//! not).
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader), and a copy of it is
//! rearranged each way. Every part's id and instruments, every measure's
//! offset, number, padding, barlines, clefs, meters and keys, and every
//! voice's id and notes, chords and rests -- where they start, how long
//! they are, their pitches, ties and stems -- must come out the same, or
//! both must refuse. Offsets and lengths are compared bit for bit.
//! `STREAM_VOICES_PARITY_SCORES`, a `;`-separated list of corpus names,
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
import copy
import struct
import zipfile
from music21 import chord, clef, converter, corpus, harmony, instrument, key, meter, note, percussion, stream

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

def stem(e):
    return e.stemDirection if isinstance(e, note.Note) else '-'

def setting(e, offset):
    if isinstance(e, clef.Clef):
        return f'clef:{e.sign}{e.line}/{e.octaveChange}@{bits(offset)}'
    if isinstance(e, meter.TimeSignature):
        return f'meter:{e.ratioString}@{bits(offset)}'
    if isinstance(e, key.KeySignature):
        return f'key:{e.sharps}@{bits(offset)}'
    return None

def notes_of(s):
    items = sorted(describe(e, s.elementOffset(e)) + stem(e)
                   for e in s if kind(e) is not None)
    return ' '.join(items)

def bar_type(b):
    return '-' if b is None else b.type

def structure(score):
    lines = []
    for p in score.getElementsByClass(stream.Stream):
        instruments = len(p.getElementsByClass(instrument.Instrument))
        # A stream music21 made has its memory address for an id, where the
        # crate's has none.
        ident = None if p.id == id(p) else p.id
        lines.append(f'part {ident} {type(p).__name__} instruments {instruments}')
        for m in p.getElementsByClass(stream.Measure):
            settings = ' '.join(t for t in (setting(e, m.elementOffset(e)) for e in m) if t)
            lines.append(f'measure {bits(p.elementOffset(m))} {m.number}{m.numberSuffix or ""} '
                         f'{bits(m.paddingLeft)}/{bits(m.paddingRight)} '
                         f'{bar_type(m.leftBarline)}|{bar_type(m.rightBarline)} {settings}')
            lines.append('  ' + notes_of(m))
            for v in m.voices:
                lines.append(f'  voice {v.id} {bits(m.elementOffset(v))} ' + notes_of(v))
        if not p.getElementsByClass(stream.Measure):
            lines.append('  ' + notes_of(p))
    return lines

def report(text):
    original = converter.parse(text, format='musicxml', forceSource=True)
    lines = []
    def run(label, op):
        # partsToVoices takes the score's own notes and sets their stems.
        nonlocal lines
        lines += attempt(label, lambda: structure(op(copy.deepcopy(original))))
    run('explode', lambda s: s.voicesToParts())
    run('by-id', lambda s: s.voicesToParts(separateById=True))
    for i, p in enumerate(original.parts):
        run(f'part-explode {i}', lambda s, i=i: s.parts[i].voicesToParts())
    run('implode', lambda s: s.partsToVoices())
    run('one-each', lambda s: s.partsToVoices(1, permitOneVoicePerPart=True, setStems=False))
    count = len(original.parts)
    run('all', lambda s: s.partsToVoices([list(reversed(range(count)))]))
    run('three', lambda s: s.partsToVoices(3, setStems=False))
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

fn stem(element: &StreamElement) -> &'static str {
    match element {
        StreamElement::Note(note) => note.stem_direction().as_str(),
        _ => "-",
    }
}

fn setting(element: &StreamElement, offset: f64) -> Option<String> {
    let at = bits(offset);
    Some(match element {
        StreamElement::Clef(clef) => format!(
            "clef:{}{}/{}@{at}",
            clef.sign().unwrap_or("None"),
            clef.line()
                .map_or("None".to_string(), |line| line.to_string()),
            clef.octave_change()
        ),
        StreamElement::TimeSignature(meter) => format!("meter:{}@{at}", meter.ratio_string()),
        StreamElement::KeySignature(signature) => format!("key:{}@{at}", sharps(signature)),
        StreamElement::Key(key) => format!("key:{}@{at}", sharps(&key.key_signature())),
        _ => return None,
    })
}

fn sharps(signature: &music21_rs::key::keysignature::KeySignature) -> String {
    signature
        .sharps()
        .map_or("None".to_string(), |sharps| sharps.to_string())
}

fn notes_of(stream: &Stream) -> String {
    let mut items: Vec<String> = stream
        .events()
        .iter()
        .filter(|event| kind(event.element()).is_some())
        .map(|event| describe(event.element(), event.offset()) + stem(event.element()))
        .collect();
    items.sort();
    items.join(" ")
}

fn bar_type(barline: Option<&music21_rs::bar::Barline>) -> &'static str {
    barline.map_or("-", |barline| barline.bar_type().as_str())
}

fn structure(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    for event in score.events() {
        let StreamElement::Stream(part) = event.element() else {
            continue;
        };
        let instruments = part
            .events()
            .iter()
            .filter(|event| matches!(event.element(), StreamElement::Instrument(_)))
            .count();
        let class = match part.kind() {
            StreamKind::PartStaff => "PartStaff",
            StreamKind::Part => "Part",
            _ => "Stream",
        };
        lines.push(format!(
            "part {} {class} instruments {instruments}",
            part.id().unwrap_or("None")
        ));
        let mut measured = false;
        for event in part.events() {
            let StreamElement::Stream(measure) = event.element() else {
                continue;
            };
            if measure.kind() != StreamKind::Measure {
                continue;
            }
            measured = true;
            let settings: Vec<String> = measure
                .events()
                .iter()
                .filter_map(|event| setting(event.element(), event.offset()))
                .collect();
            lines.push(format!(
                "measure {} {}{} {}/{} {}|{} {}",
                bits(event.offset()),
                measure.number(),
                measure.number_suffix().unwrap_or(""),
                bits(measure.padding_left()),
                bits(measure.padding_right()),
                bar_type(measure.left_barline()),
                bar_type(measure.right_barline()),
                settings.join(" ")
            ));
            lines.push(format!("  {}", notes_of(measure)));
            for voice in measure.events() {
                if let StreamElement::Stream(inner) = voice.element()
                    && inner.kind() == StreamKind::Voice
                {
                    lines.push(format!(
                        "  voice {} {} {}",
                        inner.id().unwrap_or("None"),
                        bits(voice.offset()),
                        notes_of(inner)
                    ));
                }
            }
        }
        if !measured {
            lines.push(format!("  {}", notes_of(part)));
        }
    }
    lines
}

fn our_report(score: &Stream) -> Vec<String> {
    use music21_rs::stream::VoiceAllocation;
    let mut lines = Vec::new();
    lines.extend(attempt("explode", || {
        Ok(structure(&score.voices_to_parts(false)?))
    }));
    lines.extend(attempt("by-id", || {
        Ok(structure(&score.voices_to_parts(true)?))
    }));
    for (i, part) in score.parts().into_iter().enumerate() {
        lines.extend(attempt(&format!("part-explode {i}"), || {
            Ok(structure(&part.voices_to_parts(false)?))
        }));
    }
    lines.extend(attempt("implode", || Ok(structure(&score.implode()?))));
    lines.extend(attempt("one-each", || {
        Ok(structure(&score.parts_to_voices(
            &VoiceAllocation::Each(1),
            true,
            false,
        )?))
    }));
    let count = score.parts().len();
    lines.extend(attempt("all", || {
        let group = (0..count).rev().collect();
        Ok(structure(&score.parts_to_voices(
            &VoiceAllocation::Groups(vec![group]),
            false,
            true,
        )?))
    }));
    lines.extend(attempt("three", || {
        Ok(structure(&score.parts_to_voices(
            &VoiceAllocation::Each(3),
            false,
            false,
        )?))
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
fn the_crate_moves_voices_and_parts_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = answers::helpers(
            py,
            MUSIC21,
            c"stream_voices_parity_music21.py",
            c"stream_voices_parity_music21",
            &["report"],
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("STREAM_VOICES_PARITY_SCORES") {
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
