//! The crate's stream templates, notes added into what sounds and voices
//! flattened against music21's: `Stream.template`,
//! `insertIntoNoteOrChord` and `flattenUnnecessaryVoices`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). The score's
//! template -- by default, without rests, without voices and with
//! everything taken -- must hold music21's streams at music21's offsets
//! and its notes, chords and rests at music21's offsets and lengths, and
//! its spanners must name as many of the template's own elements as
//! music21's do. Each note, chord and rest of the second part's measures
//! is added into a copy of the first part's
//! measure, as notes and as chords alone: the notes, chords and rests that
//! makes, with their pitches, lyrics, stems, notehead fills, articulations
//! and expressions, or the refusal, must be music21's. Each measure with
//! voices, its voices flattened when one is left and when forced, must be
//! music21's too. Clefs, instruments and the like stand where each reader
//! puts them, which is `musicxml_read_parity`'s to compare.
//! `TEMPLATE_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::FloatType;
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::{Stream, StreamElement, TemplateOptions};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import copy
import zipfile
from music21 import converter, corpus, note, chord, harmony, percussion, spanner, stream

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

def kind(element):
    if isinstance(element, harmony.ChordSymbol):
        return 'ChordSymbol'
    if isinstance(element, percussion.PercussionChord):
        return 'PercussionChord'
    if isinstance(element, chord.Chord):
        return 'Chord'
    if isinstance(element, note.Unpitched):
        return 'Unpitched'
    if isinstance(element, note.Note):
        return 'Note'
    if isinstance(element, note.Rest):
        return 'Rest'
    if isinstance(element, stream.Stream):
        return 'Stream'
    return 'Other'

def describe(s, depth=0, out=None):
    if out is None:
        out = []
    for el in s:
        k = kind(el)
        if k == 'Other':
            continue
        length = 0.0 if k == 'Stream' else round(float(el.quarterLength), 6)
        out.append((depth, round(float(s.elementOffset(el)), 6), k, length))
        if k == 'Stream':
            describe(el, depth + 1, out)
    return out

def spanners(s, out=None):
    if out is None:
        out = []
    leaves = {id(e) for e in s.recurse()
              if not isinstance(e, (stream.Stream, spanner.Spanner))}
    for sp in s.getElementsByClass(spanner.Spanner):
        spanned = sp.getSpannedElements()
        if any(isinstance(e, stream.Stream) for e in spanned):
            continue
        out.append((type(sp).__name__, sum(id(e) in leaves for e in spanned)))
    for inner in s.getElementsByClass(stream.Stream):
        spanners(inner, out)
    return sorted(out)

def stem(el):
    return el.stemDirection if hasattr(el, 'stemDirection') and not el.isRest else ''

def notes(m):
    out = []
    for el in m.notesAndRests:
        if kind(el) in ('Unpitched', 'PercussionChord', 'ChordSymbol'):
            out.append((float(m.elementOffset(el)), kind(el), float(el.quarterLength),
                        [], [], '', None, [], 0, 0))
            continue
        pitches = [float(p.ps) for p in el.pitches]
        fills = [n.noteheadFill for n in el] if isinstance(el, chord.Chord) else []
        fill = el.noteheadFill if hasattr(el, 'noteheadFill') and not el.isRest else None
        out.append((float(m.elementOffset(el)), kind(el), float(el.quarterLength), pitches,
                    [ly.text or '' for ly in el.lyrics], stem(el), fill, fills,
                    len(el.articulations), len(el.expressions)))
    return out

def inserted(score, chords_only):
    parts = list(score.parts)
    if len(parts) < 2:
        return []
    out = []
    for m0, m1 in zip(parts[0].getElementsByClass(stream.Measure),
                      parts[1].getElementsByClass(stream.Measure)):
        target = copy.deepcopy(m0)
        error = ''
        for el in m1.notesAndRests:
            try:
                target.insertIntoNoteOrChord(m1.elementOffset(el), copy.deepcopy(el),
                                             chordsOnly=chords_only)
            except Exception as e:
                error = type(e).__name__
                break
        out.append((notes(target), error))
    return out

def flattened(score):
    out = []
    for part in score.parts:
        for m in part.getElementsByClass(stream.Measure):
            if not m.voices:
                continue
            for force in (False, True):
                out.append(describe(m.flattenUnnecessaryVoices(force=force)))
    return out

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    template = score.template()
    return (describe(template),
            describe(score.template(fillWithRests=False)),
            describe(score.template(retainVoices=False)),
            describe(score.template(removeAll=True)),
            spanners(template),
            inserted(score, False),
            inserted(score, True),
            flattened(score))
"#;

type Entry = (usize, FloatType, String, FloatType);
type Noted = (
    FloatType,
    String,
    FloatType,
    Vec<FloatType>,
    Vec<String>,
    String,
    Option<bool>,
    Vec<Option<bool>>,
    usize,
    usize,
);
type Report = (
    Vec<Entry>,
    Vec<Entry>,
    Vec<Entry>,
    Vec<Entry>,
    Vec<(String, usize)>,
    Vec<(Vec<Noted>, String)>,
    Vec<(Vec<Noted>, String)>,
    Vec<Vec<Entry>>,
);

fn kind(element: &StreamElement) -> &'static str {
    match element {
        StreamElement::ChordSymbol(_) => "ChordSymbol",
        StreamElement::PercussionChord(_) => "PercussionChord",
        StreamElement::Chord(_) => "Chord",
        StreamElement::Unpitched(_) => "Unpitched",
        StreamElement::Note(_) => "Note",
        StreamElement::Rest(_) => "Rest",
        StreamElement::Stream(_) => "Stream",
        _ => "Other",
    }
}

/// A length rounded as the helper rounds it, so that music21's fractions
/// and the crate's floats meet.
fn rounded(value: FloatType) -> FloatType {
    (value * 1e6).round_ties_even() / 1e6
}

fn describe(stream: &Stream) -> Vec<Entry> {
    fn walk(stream: &Stream, depth: usize, out: &mut Vec<Entry>) {
        for event in stream.events() {
            let element = event.element();
            let kind = kind(element);
            if kind == "Other" {
                continue;
            }
            out.push((
                depth,
                rounded(event.offset()),
                kind.to_string(),
                match element {
                    StreamElement::Stream(_) => 0.0,
                    _ => rounded(element.quarter_length()),
                },
            ));
            if let StreamElement::Stream(inner) = element {
                walk(inner, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(stream, 0, &mut out);
    out
}

fn spanners(stream: &Stream) -> Vec<(String, usize)> {
    fn walk(stream: &Stream, out: &mut Vec<(String, usize)>) {
        for spanner in stream.spanners() {
            out.push((
                spanner.kind().class_name().to_string(),
                spanner.spanned().iter().flatten().count(),
            ));
        }
        for event in stream.events() {
            if let StreamElement::Stream(inner) = event.element() {
                walk(inner, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(stream, &mut out);
    out.sort();
    out
}

fn notes(measure: &Stream) -> Vec<Noted> {
    let texts = |lyrics: &[music21_rs::notation::Lyric]| -> Vec<String> {
        lyrics
            .iter()
            .map(|lyric| lyric.explicit_text().unwrap_or_default())
            .collect()
    };
    measure
        .events()
        .iter()
        .filter_map(|event| {
            let element = event.element();
            let offset = event.offset();
            let length = element.quarter_length();
            let kind = kind(element).to_string();
            Some(match element {
                StreamElement::Note(note) => (
                    offset,
                    kind,
                    length,
                    vec![note.pitch().ps()],
                    texts(note.lyrics()),
                    note.stem_direction().as_str().to_string(),
                    note.notehead_fill(),
                    Vec::new(),
                    note.articulations().len(),
                    note.expressions().len(),
                ),
                StreamElement::Chord(chord) => (
                    offset,
                    kind,
                    length,
                    chord.pitches().iter().map(|pitch| pitch.ps()).collect(),
                    texts(chord.lyrics()),
                    chord.stem_direction().as_str().to_string(),
                    chord.notehead_fill(),
                    chord
                        .notes()
                        .iter()
                        .map(|note| note.notehead_fill())
                        .collect(),
                    chord.articulations().len(),
                    chord.expressions().len(),
                ),
                StreamElement::Rest(rest) => (
                    offset,
                    kind,
                    length,
                    Vec::new(),
                    texts(rest.lyrics()),
                    String::new(),
                    None,
                    Vec::new(),
                    rest.articulations().len(),
                    rest.expressions().len(),
                ),
                StreamElement::Unpitched(_)
                | StreamElement::PercussionChord(_)
                | StreamElement::ChordSymbol(_) => (
                    offset,
                    kind,
                    length,
                    Vec::new(),
                    Vec::new(),
                    String::new(),
                    None,
                    Vec::new(),
                    0,
                    0,
                ),
                _ => return None,
            })
        })
        .collect()
}

fn is_note_or_rest(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::Rest(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
            | StreamElement::ChordSymbol(_)
    )
}

fn inserted(score: &Stream, chords_only: bool) -> Vec<(Vec<Noted>, String)> {
    let parts = score.parts();
    if parts.len() < 2 {
        return Vec::new();
    }
    parts[0]
        .measures()
        .into_iter()
        .zip(parts[1].measures())
        .map(|(first, second)| {
            let mut target = first.clone();
            let mut error = String::new();
            for event in second.events() {
                if !is_note_or_rest(event.element()) {
                    continue;
                }
                if target
                    .insert_into_note_or_chord(event.offset(), event.element().clone(), chords_only)
                    .is_err()
                {
                    error = "StreamException".to_string();
                    break;
                }
            }
            (notes(&target), error)
        })
        .collect()
}

fn flattened(score: &Stream) -> Vec<Vec<Entry>> {
    let mut out = Vec::new();
    for part in score.parts() {
        for measure in part.measures() {
            if measure.voices().is_empty() {
                continue;
            }
            for force in [false, true] {
                let mut flat = measure.clone();
                flat.flatten_unnecessary_voices(force);
                out.push(describe(&flat));
            }
        }
    }
    out
}

/// The first place two lists differ, and what each has there.
fn first_difference<T: PartialEq + std::fmt::Debug>(
    label: &str,
    ours: &[T],
    theirs: &[T],
) -> Option<String> {
    if ours == theirs {
        return None;
    }
    let at = ours
        .iter()
        .zip(theirs)
        .position(|(ours, theirs)| ours != theirs)
        .unwrap_or(ours.len().min(theirs.len()));
    Some(format!(
        "{label}: {} entries, music21 {}; first difference at {at}:\n  music21    {:?}\n  music21-rs {:?}",
        ours.len(),
        theirs.len(),
        theirs.get(at),
        ours.get(at)
    ))
}

#[test]
fn the_crate_makes_templates_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"template_parity_music21.py",
            c"template_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("TEMPLATE_PARITY_SCORES") {
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
            let report = music21.getattr("report")?.call1((&text,));
            let theirs: Report = match report {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;

            let template = score.template(&TemplateOptions::default());
            let without_rests = score.template(&TemplateOptions {
                fill_with_rests: false,
                ..TemplateOptions::default()
            });
            let without_voices = score.template(&TemplateOptions {
                retain_voices: false,
                ..TemplateOptions::default()
            });
            let emptied = score.template(&TemplateOptions {
                remove_all: true,
                ..TemplateOptions::default()
            });
            let differences = [
                first_difference("template", &describe(&template), &theirs.0),
                first_difference(
                    "template without rests",
                    &describe(&without_rests),
                    &theirs.1,
                ),
                first_difference(
                    "template without voices",
                    &describe(&without_voices),
                    &theirs.2,
                ),
                first_difference("template of everything", &describe(&emptied), &theirs.3),
                first_difference("template spanners", &spanners(&template), &theirs.4),
                first_difference("inserted notes", &inserted(&score, false), &theirs.5),
                first_difference("inserted chords", &inserted(&score, true), &theirs.6),
                first_difference("flattened voices", &flattened(&score), &theirs.7),
            ];
            for difference in differences.into_iter().flatten() {
                failures.push(format!("{name}: {difference}"));
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
        failures.join("\n")
    );
}
