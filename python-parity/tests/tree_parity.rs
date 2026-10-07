//! The crate's timespan trees against music21's: `tree.fromStream`,
//! `TimespanTree` and `Verticality`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Every element
//! is read as a timespan, flat and semi-flat, and the notes and chords
//! alone; each note, chord, rest and stream's span, its stream's span, its
//! measure and its kind must be music21's, in music21's order -- clefs,
//! instruments and the like stand where each reader puts them, which is
//! `musicxml_read_parity`'s to compare -- and so must the tree's time
//! points, overlaps and every verticality of its notes and chords -- what
//! starts, sounds and stops there, its pitches, bass, measure and the time
//! to the next. `TREE_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those instead of the writer test's scores, which is how the corpus
//! is swept.

use music21_rs::FloatType;
use music21_rs::IntegerType;
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::StreamElement;
use music21_rs::tree::{ElementTimespan, Flatten, TimespanTree, as_timespans};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus, note, chord, harmony, percussion, stream
from music21.tree import fromStream

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

def spans(tree):
    return [(float(ts.offset), float(ts.endTime), kind(ts.element), float(ts.parentOffset),
             float(ts.parentEndTime), ts.measureNumber, hasattr(ts, 'pitches'))
            for ts in tree if kind(ts.element) != 'Other']

def ident(ts):
    return None if ts is None else (float(ts.offset), float(ts.endTime), kind(ts.element),
                                     ts.measureNumber)

def verticalities(tree):
    # music21 cannot walk the verticalities of an empty tree: the one it
    # starts from lasts to an endless end, which opFrac cannot read.
    if not len(tree):
        return []
    out = []
    for v in tree.iterateVerticalities():
        nxt = v.nextStartOffset
        tnext = v.timeToNextEvent
        out.append((float(v.offset), len(v.startTimespans), len(v.overlapTimespans),
                    len(v.stopTimespans),
                    sorted(float(p.ps) for p in v.pitchSet),
                    sorted(p.pitchClass for p in v.pitchClassSet),
                    ident(v.bassTimespan), v.measureNumber,
                    None if nxt is None else float(nxt),
                    None if tnext is None else float(tnext)))
    return out

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    flat = fromStream.asTimespans(score, flatten=True)
    semi = fromStream.asTimespans(score, flatten='semiFlat')
    notes = fromStream.asTimespans(score, flatten=True, classList=(note.Note, chord.Chord))
    return (spans(flat), spans(semi), spans(notes),
            [float(x) for x in notes.allTimePoints()],
            [float(x) for x in notes.overlapTimePoints()],
            notes.maximumOverlap(),
            verticalities(notes))
"#;

type Span = (
    FloatType,
    FloatType,
    String,
    FloatType,
    FloatType,
    Option<IntegerType>,
    bool,
);
type Ident = Option<(FloatType, FloatType, String, Option<IntegerType>)>;
type Vertical = (
    FloatType,
    usize,
    usize,
    usize,
    Vec<FloatType>,
    Vec<u8>,
    Ident,
    Option<IntegerType>,
    Option<FloatType>,
    Option<FloatType>,
);
type Report = (
    Vec<Span>,
    Vec<Span>,
    Vec<Span>,
    Vec<FloatType>,
    Vec<FloatType>,
    usize,
    Vec<Vertical>,
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

fn spans(tree: &TimespanTree<'_>) -> Vec<Span> {
    tree.timespans()
        .iter()
        .filter(|span| kind(span.element()) != "Other")
        .map(|span| {
            (
                span.offset(),
                span.end_time(),
                kind(span.element()).to_string(),
                span.parent_offset(),
                span.parent_end_time(),
                span.measure_number(),
                span.is_pitched(),
            )
        })
        .collect()
}

fn ident(span: Option<&ElementTimespan<'_>>) -> Ident {
    span.map(|span| {
        (
            span.offset(),
            span.end_time(),
            kind(span.element()).to_string(),
            span.measure_number(),
        )
    })
}

fn notes_and_chords(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::ChordSymbol(_)
    )
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
fn the_crate_reads_timespans_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"tree_parity_music21.py",
            c"tree_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("TREE_PARITY_SCORES") {
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

            let flat = as_timespans(&score, Flatten::Flat, None);
            let semi = as_timespans(&score, Flatten::SemiFlat, None);
            let notes = as_timespans(&score, Flatten::Flat, Some(notes_and_chords));
            let verticals: Vec<Vertical> = notes
                .verticalities()
                .iter()
                .map(|v| {
                    (
                        v.offset(),
                        v.start_timespans().len(),
                        v.overlap_timespans().len(),
                        v.stop_timespans().len(),
                        v.pitch_set().iter().map(|pitch| pitch.ps()).collect(),
                        v.pitch_class_set(),
                        ident(v.bass_timespan()),
                        v.measure_number(),
                        v.next_start_offset(),
                        v.time_to_next_event(),
                    )
                })
                .collect();
            let differences = [
                first_difference("flat timespans", &spans(&flat), &theirs.0),
                first_difference("semi-flat timespans", &spans(&semi), &theirs.1),
                first_difference("note timespans", &spans(&notes), &theirs.2),
                first_difference("time points", &notes.all_time_points(), &theirs.3),
                first_difference(
                    "overlap points",
                    &notes.overlap_time_points(false),
                    &theirs.4,
                ),
                first_difference("maximum overlap", &[notes.maximum_overlap()], &[theirs.5]),
                first_difference("verticalities", &verticals, &theirs.6),
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
