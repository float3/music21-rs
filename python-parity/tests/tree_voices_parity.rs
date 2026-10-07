//! The crate's verticality voice-leading and sequences against music21's:
//! `Verticality.getPairedMotion`, `getAllVoiceLeadingQuartets`,
//! `TimespanTree.iterateVerticalitiesNwise`, `VerticalitySequence.unwrap`,
//! `Horizontality`, `iterateConsonanceBoundedVerticalities` and `splitAt`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader) and its notes
//! and chords read as a timespan tree. At every verticality the paired
//! motion in each way of counting rests and oblique motion, the quartets
//! with and without motionless ones and between the first two parts, and
//! the voice-leading quartets' motion must be music21's; so must each run of
//! three verticalities unwrapped into horizontalities and what they say of
//! passing and neighbour tones and motion, the consonance-bounded runs, and
//! the tree split at every half beat. music21's quartets take minutes over
//! a large score, so this runs ten small ones; `TREE_PARITY_SCORES`, a
//! `;`-separated list of corpus names, runs those instead, which is how the
//! corpus is swept.

use music21_rs::FloatType;
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::StreamElement;
use music21_rs::tree::{ElementTimespan, Flatten, as_timespans, unwrap};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Small scores of every kind: music21's quartets take minutes over a
/// large one.
const SCORES: [&str; 10] = [
    "bach/bwv66.6.mxl",
    "bach/bwv324.mxl",
    "schoenberg/opus19/movement2.mxl",
    "demos/two-voices.xml",
    "demos/voices_with_chords.xml",
    "demos/two-parts.xml",
    "demos/chorale_with_parallels.mxl",
    "ciconia/quod_jactatur.xml",
    "trecento/PMFC_06_8-In Verde Prato.xml",
    "monteverdi/madrigal.3.1.mxl",
];

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus, note, chord
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

def ident(ts):
    return (float(ts.offset), float(ts.endTime), [float(p.ps) for p in ts.pitches])

def attempt(function):
    try:
        return function(), ''
    except Exception as error:
        return None, type(error).__name__

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    tree = fromStream.asTimespans(score, flatten=True, classList=(note.Note, chord.Chord))
    if not len(tree):
        return [], [], [], []
    verticals = []
    for v in tree.iterateVerticalities():
        paired = [[(ident(a), ident(b)) for a, b in v.getPairedMotion(includeRests=r, includeOblique=o)]
                  for r in (True, False) for o in (True, False)]
        quartets = [[[ident(x) for pair in q for x in pair] for q in v.getAllVoiceLeadingQuartets(
                        returnObjects=False, includeNoMotion=m)] for m in (False, True)]
        between, error = attempt(lambda: [[ident(x) for pair in q for x in pair] for q in v.getAllVoiceLeadingQuartets(
                        returnObjects=False, partPairNumbers=[(0, 1)])])
        objects = [(q.motionType().value, float(q.v1n1.pitch.ps), float(q.v2n2.pitch.ps))
                   for q in v.getAllVoiceLeadingQuartets()]
        verticals.append((paired, quartets, (between or [], error), objects))
    windows = []
    for window in tree.iterateVerticalitiesNwise(3):
        unwrapped = window.unwrap()
        windows.append([(len(h), h.hasPassingTone, h.hasNeighborTone, h.hasNoMotion)
                        for h in unwrapped.values()])
    runs = [[float(v.offset) for v in run] for run in tree.iterateConsonanceBoundedVerticalities()]
    end = float(tree.endTime)
    tree.splitAt([x / 2 for x in range(int(end * 2) + 1)])
    split = [(float(ts.offset), float(ts.endTime)) for ts in tree]
    return verticals, windows, runs, split
"#;

type Ident = (FloatType, FloatType, Vec<FloatType>);
type Pair = (Ident, Ident);
type Motion = (String, FloatType, FloatType);
type Vertical = (
    Vec<Vec<Pair>>,
    Vec<Vec<Vec<Ident>>>,
    (Vec<Vec<Ident>>, String),
    Vec<Motion>,
);
type Window = Vec<(usize, bool, bool, bool)>;
type Report = (
    Vec<Vertical>,
    Vec<Window>,
    Vec<Vec<FloatType>>,
    Vec<(FloatType, FloatType)>,
);

fn ident(span: &ElementTimespan<'_>) -> Ident {
    (
        span.offset(),
        span.end_time(),
        span.pitches().iter().map(|pitch| pitch.ps()).collect(),
    )
}

fn notes_and_chords(element: &StreamElement) -> bool {
    matches!(
        element,
        StreamElement::Note(_) | StreamElement::Chord(_) | StreamElement::ChordSymbol(_)
    )
}

#[test]
fn the_crate_reads_voice_leading_in_trees_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"tree_voices_parity_music21.py",
            c"tree_voices_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("TREE_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES.iter().map(|name| name.to_string()).collect(),
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
            let (their_verticals, their_windows, their_runs, their_split): Report = match report {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            let mut tree = as_timespans(&score, Flatten::Flat, Some(notes_and_chords));
            if tree.is_empty() {
                continue;
            }

            let verticals: Vec<Vertical> = tree
                .verticalities()
                .iter()
                .map(|v| {
                    let paired = [(true, true), (true, false), (false, true), (false, false)]
                        .into_iter()
                        .map(|(rests, oblique)| {
                            v.paired_motion(rests, oblique)
                                .into_iter()
                                .map(|(a, b)| (ident(a), ident(b)))
                                .collect()
                        })
                        .collect();
                    let quartets = [false, true]
                        .into_iter()
                        .map(|motionless| {
                            v.voice_leading_timespans(true, true, motionless, None)
                                .expect("no part pairs")
                                .iter()
                                .map(|quartet| quartet.iter().map(|span| ident(span)).collect())
                                .collect()
                        })
                        .collect();
                    let between = match v.voice_leading_timespans(true, true, false, Some(&[(0, 1)])) {
                        Ok(found) => (
                            found
                                .iter()
                                .map(|quartet| quartet.iter().map(|span| ident(span)).collect())
                                .collect(),
                            String::new(),
                        ),
                        Err(error) => (Vec::new(), error.to_string()),
                    };
                    let objects = v
                        .voice_leading_quartets(true, true, false, None)
                        .unwrap_or_default()
                        .iter()
                        .map(|quartet| {
                            (
                                quartet.motion_type(false).as_str().to_string(),
                                quartet.v1n1().ps(),
                                quartet.v2n2().ps(),
                            )
                        })
                        .collect();
                    (paired, quartets, between, objects)
                })
                .collect();
            if verticals.len() != their_verticals.len() {
                failures.push(format!(
                    "{name}: {} verticalities, music21 {}",
                    verticals.len(),
                    their_verticals.len()
                ));
            }
            for (index, (ours, theirs)) in verticals.iter().zip(&their_verticals).enumerate() {
                let between_agree = if theirs.2.1.is_empty() {
                    ours.2.1.is_empty() && ours.2.0 == theirs.2.0
                } else {
                    !ours.2.1.is_empty()
                };
                if ours.0 != theirs.0 || ours.1 != theirs.1 || !between_agree || ours.3 != theirs.3 {
                    failures.push(format!(
                        "{name}: verticality {index} differs in paired {}, quartets {}, \
                         between {}, objects {}:\n  music21    {:?}\n  music21-rs {:?}",
                        ours.0 != theirs.0,
                        ours.1 != theirs.1,
                        !between_agree,
                        ours.3 != theirs.3,
                        theirs.3,
                        ours.3
                    ));
                    break;
                }
            }

            let windows: Vec<Window> = tree
                .verticalities_nwise(3, false, false)
                .expect("three at a time")
                .iter()
                .map(|window| {
                    unwrap(window)
                        .iter()
                        .map(|(_, horizontality)| {
                            (
                                horizontality.timespans().len(),
                                horizontality.has_passing_tone(),
                                horizontality.has_neighbor_tone(),
                                horizontality.has_no_motion(),
                            )
                        })
                        .collect()
                })
                .collect();
            if windows != their_windows {
                let at = windows.iter().zip(&their_windows).position(|(a, b)| a != b);
                failures.push(format!(
                    "{name}: windows {} vs music21 {}, first difference at {at:?}: music21 {:?}, music21-rs {:?}",
                    windows.len(),
                    their_windows.len(),
                    at.and_then(|at| their_windows.get(at)),
                    at.and_then(|at| windows.get(at))
                ));
            }

            let runs: Vec<Vec<FloatType>> = tree
                .consonance_bounded_verticalities()
                .expect("chords")
                .iter()
                .map(|run| run.iter().map(|v| v.offset()).collect())
                .collect();
            if runs != their_runs {
                failures.push(format!(
                    "{name}: consonance runs:\n  music21    {their_runs:?}\n  music21-rs {runs:?}"
                ));
            }

            let end = tree.end_time().unwrap_or(0.0);
            let offsets: Vec<FloatType> = (0..=(end * 2.0) as usize)
                .map(|half| half as FloatType / 2.0)
                .collect();
            tree.split_at(&offsets);
            let split: Vec<(FloatType, FloatType)> = tree
                .timespans()
                .iter()
                .map(|span| (span.offset(), span.end_time()))
                .collect();
            if split != their_split {
                let at = split.iter().zip(&their_split).position(|(a, b)| a != b);
                failures.push(format!(
                    "{name}: split {} vs music21 {}, first difference at {at:?}: music21 {:?}, music21-rs {:?}",
                    split.len(),
                    their_split.len(),
                    at.and_then(|at| their_split.get(at)),
                    at.and_then(|at| split.get(at))
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
        failures.join("\n")
    );
}
