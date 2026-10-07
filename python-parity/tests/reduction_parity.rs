//! The crate's score and part reductions against music21's:
//! `analysis.reduction.ScoreReduction` and `PartReduction`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Every fifth
//! note, chord and chord symbol, counted part by part and measure by
//! measure, is given a lyric from a list that marks notes for reductions in
//! different groups and voices, octaves, fills, stems and texts -- a chord's
//! naming its last pitch -- or marks nothing. The reduction must be
//! music21's or fail where music21's does: its parts' ids, and in each part
//! made for a group its streams, notes, chords, rests and text expressions
//! with their offsets, lengths, pitches, lyrics, stems, notehead fills,
//! shown accidentals and hidden rests, and in each part of the score the
//! lyrics left on its notes. Each score's parts are also weighed by
//! `PartReduction` -- by measure and by run of notes, cut by dynamic or
//! not, normalized over the score, by part or not at all, and in groups
//! named by the parts' ids -- and each group's spans must be music21's.
//! Clefs, instruments and the like stand where each reader puts them,
//! which is `musicxml_read_parity`'s to compare. `REDUCTION_PARITY_SCORES`,
//! a `;`-separated list of corpus names, runs those instead of the writer
//! test's scores, which is how the corpus is swept.

use music21_rs::FloatType;
use music21_rs::analysis::reduction::{PartGroup, PartReduction, ScoreReduction};
use music21_rs::musicxml::from_musicxml;
use music21_rs::notation::Lyric;
use music21_rs::stream::{Stream, StreamElement, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import copy
import zipfile
from music21 import converter, corpus, note, chord, harmony, percussion, stream, expressions
from music21.analysis import reduction

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

SPECS = ['::/o:5/tb:a', '::/o:4/v:1/g:A', '::/v:2/g:B/ta:x y', '::/nf:no/sd:up',
         '::/nf:yes/v:1/tb:b c', '::/o:6', 'plain words', '::/g:A/v:2/nf:filled/sd:down']

def plan(score):
    out = []
    count = 0
    for pi, p in enumerate(score.parts):
        for mi, m in enumerate(p.getElementsByClass(stream.Measure)):
            for ni, n in enumerate(m.recurse().notes):
                if isinstance(n, (note.Unpitched, percussion.PercussionChord)):
                    continue
                count += 1
                if count % 5 != 2:
                    continue
                spec = SPECS[(count // 5) % len(SPECS)]
                if isinstance(n, chord.Chord) and n.pitches and spec.startswith('::'):
                    spec = '::/p:' + n.pitches[-1].name + spec[2:]
                out.append((pi, mi, ni, spec))
    return out

def kind(element):
    if isinstance(element, harmony.ChordSymbol):
        return 'ChordSymbol'
    if isinstance(element, chord.Chord):
        return 'Chord'
    if isinstance(element, note.Note):
        return 'Note'
    if isinstance(element, note.Rest):
        return 'Rest'
    if isinstance(element, expressions.TextExpression):
        return 'TextExpression'
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
        offset = round(float(s.elementOffset(el)), 6)
        if k == 'Stream':
            out.append((depth, offset, k, 0.0, [], [], '', None, ''))
            describe(el, depth + 1, out)
        elif k == 'TextExpression':
            out.append((depth, offset, k, 0.0, [], [], '', None, el.content))
        elif k == 'Rest':
            out.append((depth, offset, k, round(float(el.quarterLength), 6), [], [], '', None,
                        'hidden' if el.style.hideObjectOnPrint else ''))
        else:
            shown = ['' if p.accidental is None else str(p.accidental.displayStatus)
                     for p in el.pitches]
            out.append((depth, offset, k, round(float(el.quarterLength), 6),
                        [float(p.ps) for p in el.pitches], [ly.text or '' for ly in el.lyrics],
                        el.stemDirection, el.noteheadFill, ' '.join(shown)))
    return out

def lyrics_left(part):
    out = []
    for i, n in enumerate(part.recurse().notes):
        if n.lyrics:
            out.append((i, [(ly.number, ly.text or '') for ly in n.lyrics]))
    return out

def activity(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    ids = [str(p.id) for p in score.parts]
    groups = [('first', '#ff0000', [ids[0][:3]] if ids else None),
              ('staffs', '#00ff00', ['staff', 'p2']),
              ('nothing', '#0000ff', ['zzzz']),
              ('soprano', '#000000', None)]
    variants = [{}, {'fillByMeasure': False}, {'segmentByTarget': False}, {'normalize': False},
                {'normalizeByPart': True},
                {'partGroups': [{'name': n, 'color': c, 'match': m} for n, c, m in groups]}]
    out = []
    for keywords in variants:
        s = copy.deepcopy(score)
        try:
            pr = reduction.PartReduction(s, **keywords)
            pr.process()
            data = [(gid if isinstance(gid, str) else None,
                     [(float(a), float(b), float(c), d) for a, b, c, d in spans])
                    for gid, spans in pr.getGraphHorizontalBarWeightedData()]
            out.append((data, ''))
        except Exception as e:
            out.append(([], type(e).__name__))
    return (groups, out)

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    marks = plan(score)
    for pi, mi, ni, spec in marks:
        m = list(score.parts[pi].getElementsByClass(stream.Measure))[mi]
        list(m.recurse().notes)[ni].addLyric(spec)
    sr = reduction.ScoreReduction()
    sr.score = score
    try:
        post = sr.reduce()
    except Exception as e:
        return (marks, [], [], type(e).__name__)
    groups = len(post.parts) - len(score.parts)
    ids = [p.id if isinstance(p.id, str) else None for p in post.parts]
    made = [describe(p) for p in list(post.parts)[:groups]]
    left = [lyrics_left(p) for p in list(post.parts)[groups:]]
    return (marks, [(ids, made)], left, '')
"#;

type Entry = (
    usize,
    FloatType,
    String,
    FloatType,
    Vec<FloatType>,
    Vec<String>,
    String,
    Option<bool>,
    String,
);
type Left = Vec<(usize, Vec<(i64, String)>)>;
type Made = (Vec<Option<String>>, Vec<Vec<Entry>>);
type Report = (
    Vec<(usize, usize, usize, String)>,
    Vec<Made>,
    Vec<Left>,
    String,
);

fn rounded(value: FloatType) -> FloatType {
    (value * 1e6).round_ties_even() / 1e6
}

fn is_note(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_)
            | StreamElement::Chord(_)
            | StreamElement::ChordSymbol(_)
            | StreamElement::Unpitched(_)
            | StreamElement::PercussionChord(_)
    )
}

fn lyrics(element: &StreamElement) -> Vec<Lyric> {
    match element {
        StreamElement::Note(note) => note.lyrics().to_vec(),
        StreamElement::Chord(chord) => chord.lyrics().to_vec(),
        StreamElement::ChordSymbol(symbol) => symbol.lyrics().to_vec(),
        StreamElement::Unpitched(unpitched) => unpitched.written().lyrics().to_vec(),
        StreamElement::PercussionChord(chord) => chord.written().lyrics().to_vec(),
        _ => Vec::new(),
    }
}

fn add_lyric(element: &mut StreamElement, text: &str) {
    let added = match element {
        StreamElement::Note(note) => note.add_lyric(text, None, false),
        StreamElement::Chord(chord) => chord.add_lyric(text, None, false),
        StreamElement::ChordSymbol(symbol) => {
            symbol.add_lyric(text, None, false);
            Ok(())
        }
        _ => Ok(()),
    };
    added.expect("a note takes a lyric");
}

fn measure_mut(score: &mut Stream, part: usize, measure: usize) -> &mut Stream {
    let part = score
        .events_mut()
        .iter_mut()
        .filter_map(|event| match event.element_mut() {
            StreamElement::Stream(part)
                if matches!(part.kind(), StreamKind::Part | StreamKind::PartStaff) =>
            {
                Some(part)
            }
            _ => None,
        })
        .nth(part)
        .expect("the part music21 marked");
    part.events_mut()
        .iter_mut()
        .filter_map(|event| match event.element_mut() {
            StreamElement::Stream(measure) if measure.kind() == StreamKind::Measure => {
                Some(measure)
            }
            _ => None,
        })
        .nth(measure)
        .expect("the measure music21 marked")
}

fn describe(stream: &Stream) -> Vec<Entry> {
    fn walk(stream: &Stream, depth: usize, out: &mut Vec<Entry>) {
        for event in stream.events() {
            let offset = rounded(event.offset());
            let element = event.element();
            let length = rounded(element.quarter_length());
            let texts = |lyrics: &[Lyric]| -> Vec<String> {
                lyrics
                    .iter()
                    .map(|lyric| lyric.explicit_text().unwrap_or_default())
                    .collect()
            };
            let shown = |pitches: Vec<music21_rs::pitch::Pitch>| -> String {
                pitches
                    .iter()
                    .map(|pitch| match pitch.written_accidental() {
                        None => String::new(),
                        Some(accidental) => match accidental.display_status() {
                            Some(true) => "True".to_string(),
                            Some(false) => "False".to_string(),
                            None => "None".to_string(),
                        },
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            match element {
                StreamElement::Stream(inner) => {
                    out.push((
                        depth,
                        offset,
                        "Stream".to_string(),
                        0.0,
                        Vec::new(),
                        Vec::new(),
                        String::new(),
                        None,
                        String::new(),
                    ));
                    walk(inner, depth + 1, out);
                }
                StreamElement::TextExpression(text) => out.push((
                    depth,
                    offset,
                    "TextExpression".to_string(),
                    0.0,
                    Vec::new(),
                    Vec::new(),
                    String::new(),
                    None,
                    text.content().to_string(),
                )),
                StreamElement::Rest(rest) => out.push((
                    depth,
                    offset,
                    "Rest".to_string(),
                    length,
                    Vec::new(),
                    Vec::new(),
                    String::new(),
                    None,
                    if rest.hidden() { "hidden" } else { "" }.to_string(),
                )),
                StreamElement::Note(note) => out.push((
                    depth,
                    offset,
                    "Note".to_string(),
                    length,
                    vec![note.pitch().ps()],
                    texts(note.lyrics()),
                    note.stem_direction().as_str().to_string(),
                    note.notehead_fill(),
                    shown(vec![note.pitch().clone()]),
                )),
                StreamElement::Chord(chord) => out.push((
                    depth,
                    offset,
                    "Chord".to_string(),
                    length,
                    chord.pitches().iter().map(|pitch| pitch.ps()).collect(),
                    texts(chord.lyrics()),
                    chord.stem_direction().as_str().to_string(),
                    chord.notehead_fill(),
                    shown(chord.pitches()),
                )),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(stream, 0, &mut out);
    out
}

fn lyrics_left(part: &Stream) -> Left {
    part.leaves()
        .into_iter()
        .map(|(_, element)| element)
        .filter(|element| is_note(element))
        .enumerate()
        .filter_map(|(index, element)| {
            let lyrics = lyrics(element);
            (!lyrics.is_empty()).then(|| {
                (
                    index,
                    lyrics
                        .iter()
                        .map(|lyric| {
                            (
                                i64::from(lyric.number()),
                                lyric.explicit_text().unwrap_or_default(),
                            )
                        })
                        .collect(),
                )
            })
        })
        .collect()
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
fn the_crate_reduces_scores_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"reduction_parity_music21.py",
            c"reduction_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("REDUCTION_PARITY_SCORES") {
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
            let mut score = match from_musicxml(&text) {
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
                    failures.push(format!("{name}: music21 could not reduce it: {error}"));
                    continue;
                }
            };
            compared += 1;

            for (part, measure, index, spec) in &theirs.0 {
                let measure = measure_mut(&mut score, *part, *measure);
                let mut seen = 0;
                measure.for_each_mut(&mut |_, element| {
                    if is_note(element) {
                        if seen == *index {
                            add_lyric(element, spec);
                        }
                        seen += 1;
                    }
                });
            }
            let parts = score.parts().len();
            let mut reduction = ScoreReduction::new();
            reduction.set_score(score);
            let (made, left, error): (Vec<Made>, Vec<Left>, String) = match reduction.reduce() {
                Ok(reduced) => {
                    let all = reduced.parts();
                    let groups = all.len().saturating_sub(parts);
                    let ids: Vec<Option<String>> = all
                        .iter()
                        .map(|part| part.id().map(str::to_string))
                        .collect();
                    (
                        vec![(
                            ids,
                            all[..groups].iter().map(|part| describe(part)).collect(),
                        )],
                        all[groups..].iter().map(|part| lyrics_left(part)).collect(),
                        String::new(),
                    )
                }
                Err(error) => (Vec::new(), Vec::new(), error.to_string()),
            };
            if theirs.3.is_empty() != error.is_empty() {
                failures.push(format!(
                    "{name}: music21 {:?}, music21-rs {error:?}",
                    theirs.3
                ));
                continue;
            }
            let mut differences = vec![first_difference("lyrics left", &left, &theirs.2)];
            if let (Some((our_ids, our_parts)), Some((their_ids, their_parts))) =
                (made.first(), theirs.1.first())
            {
                differences.push(first_difference("part ids", our_ids, their_ids));
                for (index, (ours, theirs)) in our_parts.iter().zip(their_parts).enumerate() {
                    differences.push(first_difference(
                        &format!("reduction part {index}"),
                        ours,
                        theirs,
                    ));
                }
            }
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

type Spans = Vec<(
    Option<String>,
    Vec<(FloatType, FloatType, FloatType, String)>,
)>;
type Activity = (
    Vec<(String, String, Option<Vec<String>>)>,
    Vec<(Spans, String)>,
);

/// Whether two groups' spans are the same, their numbers to a billionth.
fn spans_agree(ours: &Spans, theirs: &Spans) -> bool {
    let close = |a: FloatType, b: FloatType| (a - b).abs() < 1e-9;
    ours.len() == theirs.len()
        && ours
            .iter()
            .zip(theirs)
            .all(|((our_id, our_spans), (their_id, their_spans))| {
                our_id == their_id
                    && our_spans.len() == their_spans.len()
                    && our_spans.iter().zip(their_spans).all(|(a, b)| {
                        close(a.0, b.0) && close(a.1, b.1) && close(a.2, b.2) && a.3 == b.3
                    })
            })
}

#[test]
fn the_crate_weighs_parts_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"part_reduction_parity_music21.py",
            c"part_reduction_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("REDUCTION_PARITY_SCORES") {
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
            let theirs: Activity = match music21.getattr("activity")?.call1((&text,)) {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not weigh it: {error}"));
                    continue;
                }
            };
            compared += 1;
            let groups: Vec<PartGroup> = theirs
                .0
                .iter()
                .map(|(name, color, matches)| PartGroup {
                    name: name.clone(),
                    color: color.clone(),
                    matches: matches.clone(),
                })
                .collect();
            let variants = [
                PartReduction::default(),
                PartReduction {
                    fill_by_measure: false,
                    ..PartReduction::default()
                },
                PartReduction {
                    segment_by_target: false,
                    ..PartReduction::default()
                },
                PartReduction {
                    normalize: false,
                    ..PartReduction::default()
                },
                PartReduction {
                    normalize_by_part: true,
                    ..PartReduction::default()
                },
                PartReduction {
                    part_groups: Some(groups),
                    ..PartReduction::default()
                },
            ];
            for (index, (variant, (their_spans, their_error))) in
                variants.iter().zip(&theirs.1).enumerate()
            {
                let (ours, error): (Spans, String) = match variant.weighted_spans(&score) {
                    Ok(activity) => (
                        activity
                            .into_iter()
                            .map(|part| {
                                (
                                    part.id,
                                    part.spans
                                        .into_iter()
                                        .map(|span| {
                                            (span.start, span.span, span.weight, span.color)
                                        })
                                        .collect(),
                                )
                            })
                            .collect(),
                        String::new(),
                    ),
                    Err(error) => (Vec::new(), error.to_string()),
                };
                if their_error.is_empty() != error.is_empty()
                    || (error.is_empty() && !spans_agree(&ours, their_spans))
                {
                    let at = ours
                        .iter()
                        .zip(their_spans)
                        .flat_map(|(a, b)| a.1.iter().zip(&b.1).map(move |(x, y)| (&a.0, x, y)))
                        .find(|(_, x, y)| {
                            (x.0 - y.0).abs() >= 1e-9
                                || (x.1 - y.1).abs() >= 1e-9
                                || (x.2 - y.2).abs() >= 1e-9
                                || x.3 != y.3
                        });
                    failures.push(format!(
                        "{name}: variant {index}: {their_error:?} {error:?}, {} groups and {} \
                         in music21; first different span (group, music21-rs, music21): {at:?}",
                        ours.len(),
                        their_spans.len()
                    ));
                }
            }
        }
        Ok((failures, compared))
    })
    .expect("the Python side runs");

    println!("{compared} scores weighed");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}
