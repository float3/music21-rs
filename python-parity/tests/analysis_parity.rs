//! The crate's consecutive-note walks and the small analyses built on them
//! against music21's: `findConsecutiveNotes`, `melodicIntervals`,
//! `analysis.patel`, `analysis.metrical`, `analysis.segmentByRests` and
//! `analysis.pitchAnalysis`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader), and each side writes a
//! line per question -- the score and each of its parts under every set of
//! options -- describing what it found: each element by what it is, its
//! pitches and where it stands. `nPVI` is compared to a few ulps, since
//! music21 subtracts a triplet's `Fraction` lengths exactly; everything else
//! is compared exactly.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//! `ANALYSIS_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.

use music21_rs::analysis::{metrical, patel, pitch_analysis, segment_by_rests};
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::{ConsecutiveOptions, StreamElement};
use music21_rs::{FloatType, Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{SCORES, music21_name};

const MUSIC21: &str = r#"
import copy
import zipfile
from music21 import chord, corpus, harmony, note, stream
from music21.analysis import metrical, patel, pitchAnalysis, segmentByRests

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

OPTIONS = [
    dict(),
    dict(skipRests=True, noNone=True),
    dict(skipUnisons=True),
    dict(skipOctaves=True, skipRests=True),
    dict(skipChords=True),
    dict(skipGaps=True, getOverlaps=True),
]

def kind(e):
    if isinstance(e, note.Rest):
        return 'rest'
    if isinstance(e, note.Note):
        return 'note'
    if isinstance(e, chord.Chord):
        return 'chord'
    return 'other'

def describe(e, top):
    if e is None:
        return 'None'
    names = ' '.join(p.nameWithOctave for p in e.pitches)
    offset = float(e.getOffsetInHierarchy(top))
    return f'{kind(e)} {names} @{offset:.6f}'


def answer(score, s, label, lines):
    for k, options in enumerate(OPTIONS):
        found = s.findConsecutiveNotes(**options)
        lines.append(f'{label} consecutive {k}: ' + ' | '.join(describe(e, s) for e in found))
        # music21 puts each interval at an offset in the container its note
        # stands in; read back in the order they were found.
        found_order = s.melodicIntervals(**options)
        names = [i.directedName for i in sorted(
            found_order, key=lambda i: i.sortTuple(found_order).insertIndex)]
        lines.append(f'{label} intervals {k}: ' + ' '.join(names))
    for k in (0, 3):
        try:
            value = repr(patel.melodicIntervalVariability(s, **OPTIONS[k]))
        except (ValueError, ZeroDivisionError):
            value = 'error'
        lines.append(f'{label} miv {k}: {value}')

def report(name):
    score = corpus.parse(name, forceSource=True)
    lines = []
    answer(score, score, 'score', lines)
    npvi = []
    for i, part in enumerate(score.parts):
        answer(score, part, f'part {i}', lines)
        rhythm = part.flatten().notesAndRests.stream()
        try:
            npvi.append(patel.nPVI(rhythm))
        except (ZeroDivisionError, IndexError):
            npvi.append(None)
        line = part.flatten().getElementsByClass(note.Note).stream()
        metrical.thomassenMelodicAccent(line)
        accents = ' '.join(repr(n.editorial.melodicAccent) for n in line)
        lines.append(f'part {i} accents: {accents}')
        labelled = copy.deepcopy(part)
        before = [len(n.lyrics) for n in labelled.recurse().notesAndRests]
        try:
            metrical.labelBeatDepth(labelled)
            after = [len(n.lyrics) for n in labelled.recurse().notesAndRests]
            depths = ' '.join(str(a - b) for a, b in zip(after, before))
        except Exception:
            depths = 'error'
        lines.append(f'part {i} depths: {depths}')
    segments = segmentByRests.Segmenter.getSegmentsList(score)
    lines.append('segments: ' + ' / '.join(
        ' '.join(describe(n, score) for n in segment) for segment in segments))
    kept = segmentByRests.Segmenter.getSegmentsList(score, removeEmptyLists=False)
    lines.append(f'segments kept: {len(kept)}')
    names = [i.directedName for i in segmentByRests.Segmenter.getIntervalList(score)]
    lines.append('interval list: ' + ' '.join(names))
    counts = pitchAnalysis.pitchAttributeCount(score, 'nameWithOctave')
    lines.append('pitch counts: ' + ' '.join(f'{k}:{v}' for k, v in counts.items()))
    return '\n'.join(lines), npvi
"#;

const OPTIONS: [ConsecutiveOptions; 6] = [
    ConsecutiveOptions {
        skip_rests: false,
        skip_chords: false,
        skip_unisons: false,
        skip_octaves: false,
        skip_gaps: false,
        get_overlaps: false,
        no_none: false,
    },
    ConsecutiveOptions {
        skip_rests: true,
        skip_chords: false,
        skip_unisons: false,
        skip_octaves: false,
        skip_gaps: false,
        get_overlaps: false,
        no_none: true,
    },
    ConsecutiveOptions {
        skip_rests: false,
        skip_chords: false,
        skip_unisons: true,
        skip_octaves: false,
        skip_gaps: false,
        get_overlaps: false,
        no_none: false,
    },
    ConsecutiveOptions {
        skip_rests: true,
        skip_chords: false,
        skip_unisons: false,
        skip_octaves: true,
        skip_gaps: false,
        get_overlaps: false,
        no_none: false,
    },
    ConsecutiveOptions {
        skip_rests: false,
        skip_chords: true,
        skip_unisons: false,
        skip_octaves: false,
        skip_gaps: false,
        get_overlaps: false,
        no_none: false,
    },
    ConsecutiveOptions {
        skip_rests: false,
        skip_chords: false,
        skip_unisons: false,
        skip_octaves: false,
        skip_gaps: true,
        get_overlaps: true,
        no_none: false,
    },
];

fn kind(element: &StreamElement) -> &'static str {
    match element {
        StreamElement::Rest(_) => "rest",
        StreamElement::Note(_) => "note",
        StreamElement::Chord(_) | StreamElement::ChordSymbol(_) => "chord",
        _ => "other",
    }
}

fn describe(offset: FloatType, element: &StreamElement) -> String {
    let pitches = match element {
        StreamElement::ChordSymbol(symbol) => symbol.pitches().expect("a chord symbol sounds"),
        element => element.pitches(),
    };
    let names: Vec<String> = pitches.iter().map(music21_name).collect();
    format!("{} {} @{offset:.6}", kind(element), names.join(" "))
}

fn is_general_note(element: &StreamElement) -> bool {
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

fn answer(stream: &Stream, label: &str, lines: &mut Vec<String>) {
    let leaves = stream.leaves();
    for (k, options) in OPTIONS.iter().enumerate() {
        let found = stream
            .find_consecutive_notes(options)
            .expect("the notes are found");
        let described: Vec<String> = found
            .iter()
            .map(|position| match position {
                Some(position) => describe(leaves[*position].0, leaves[*position].1),
                None => "None".to_string(),
            })
            .collect();
        lines.push(format!(
            "{label} consecutive {k}: {}",
            described.join(" | ")
        ));
        let names: Vec<String> = stream
            .melodic_intervals(options)
            .expect("the intervals are spelled")
            .iter()
            .map(|step| step.interval().directed_name())
            .collect();
        lines.push(format!("{label} intervals {k}: {}", names.join(" ")));
    }
    for k in [0, 3] {
        let value = match patel::melodic_interval_variability(stream, &OPTIONS[k]) {
            Ok(value) => format!("{value:?}"),
            Err(_) => "error".to_string(),
        };
        lines.push(format!("{label} miv {k}: {value}"));
    }
}

fn kept(stream: &Stream, keep: impl Fn(&StreamElement) -> bool) -> Stream {
    Stream::from_events(
        stream
            .flatten()
            .events()
            .iter()
            .filter(|event| keep(event.element()))
            .cloned(),
    )
}

fn report(score: &Stream) -> (String, Vec<Option<FloatType>>) {
    let mut lines = Vec::new();
    answer(score, "score", &mut lines);
    let mut npvi = Vec::new();
    for (i, part) in score.parts().into_iter().enumerate() {
        answer(part, &format!("part {i}"), &mut lines);
        npvi.push(patel::n_pvi(&kept(part, is_general_note)).ok());
        let line = kept(part, |element| matches!(element, StreamElement::Note(_)));
        let accents: Vec<String> = metrical::thomassen_melodic_accent(&line)
            .expect("a line of notes is accented")
            .iter()
            .map(|accent| format!("{accent:?}"))
            .collect();
        lines.push(format!("part {i} accents: {}", accents.join(" ")));
        let depths = match metrical::beat_depths(part) {
            Ok(depths) => part
                .leaves()
                .iter()
                .enumerate()
                .filter(|(_, (_, element))| is_general_note(element))
                .map(|(position, _)| {
                    depths
                        .iter()
                        .find(|(at, _)| *at == position)
                        .map_or(0, |(_, depth)| *depth)
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(" "),
            Err(_) => "error".to_string(),
        };
        lines.push(format!("part {i} depths: {depths}"));
    }
    let leaves = score.leaves();
    let segments: Vec<String> = segment_by_rests::segments_list(score, true)
        .iter()
        .map(|segment| {
            segment
                .iter()
                .map(|position| describe(leaves[*position].0, leaves[*position].1))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    lines.push(format!("segments: {}", segments.join(" / ")));
    lines.push(format!(
        "segments kept: {}",
        segment_by_rests::segments_list(score, false).len()
    ));
    let names: Vec<String> = segment_by_rests::interval_list(score)
        .expect("the intervals are spelled")
        .iter()
        .map(|interval| interval.directed_name())
        .collect();
    lines.push(format!("interval list: {}", names.join(" ")));
    let counts: Vec<String> = pitch_analysis::pitch_attribute_count(score, music21_name)
        .expect("the pitches are counted")
        .into_iter()
        .map(|(name, count)| format!("{name}:{count}"))
        .collect();
    lines.push(format!("pitch counts: {}", counts.join(" ")));
    (lines.join("\n"), npvi)
}

/// The first line the two reports differ on, by its label, and the first
/// item of that line they differ at, with a few either side of it.
fn first_difference(ours: &str, theirs: &str) -> Option<String> {
    let mut ours_lines = ours.lines();
    let mut theirs_lines = theirs.lines();
    loop {
        let (our_line, their_line) = (ours_lines.next(), theirs_lines.next());
        if our_line.is_none() && their_line.is_none() {
            return None;
        }
        if our_line == their_line {
            continue;
        }
        let (our_line, their_line) = (our_line.unwrap_or(""), their_line.unwrap_or(""));
        let split = |line: &str| -> Vec<String> {
            let separator = if line.contains(" | ") { " | " } else { " " };
            line.split(separator).map(str::to_string).collect()
        };
        let (our_items, their_items) = (split(our_line), split(their_line));
        let at = (0..our_items.len().max(their_items.len()))
            .find(|&index| our_items.get(index) != their_items.get(index))
            .unwrap_or(0);
        let window = |items: &[String]| {
            items
                .iter()
                .skip(at.saturating_sub(3))
                .take(7)
                .cloned()
                .collect::<Vec<_>>()
                .join(" , ")
        };
        let label: String = their_line.split(':').next().unwrap_or("").to_string();
        return Some(format!(
            "{label}, item {at}\n  music21:    {}\n  music21-rs: {}",
            window(&their_items),
            window(&our_items)
        ));
    }
}

#[test]
fn the_crate_analyses_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"analysis_parity_music21.py",
            c"analysis_parity_music21",
        )?;
        let chosen: Vec<String> = match std::env::var("ANALYSIS_PARITY_SCORES") {
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

        let mut failures = Vec::new();
        let mut compared = 0;
        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let (theirs, their_npvi): (String, Vec<Option<FloatType>>) =
                match music21.getattr("report")?.call1((name,)) {
                    Ok(answer) => answer.extract()?,
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not analyse it: {error}"));
                        continue;
                    }
                };
            let score = match from_musicxml(&text) {
                Ok(score) if score.kind() == StreamKind::Score => score,
                Ok(_) => continue,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            let (ours, our_npvi) = report(&score);
            if let Some(difference) = first_difference(&ours, &theirs) {
                failures.push(format!("{name}: {difference}"));
                continue;
            }
            let agree = our_npvi.len() == their_npvi.len()
                && our_npvi.iter().zip(&their_npvi).all(|pair| match pair {
                    (Some(ours), Some(theirs)) => {
                        (ours - theirs).abs() <= 1e-12 * theirs.abs().max(1.0)
                    }
                    (None, None) => true,
                    _ => false,
                });
            if !agree {
                failures.push(format!(
                    "{name}: nPVI {our_npvi:?} where music21 has {their_npvi:?}"
                ));
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
