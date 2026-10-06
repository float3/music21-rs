//! The crate's windowed analysis and floating key against music21's:
//! `analysis.windowed` and `analysis.floatingKey`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). Both sides cut it into
//! quarter-note windows, count the notes and rests in windows of one, two
//! and four quarters of each window type, find the Krumhansl-Schmuckler key
//! of each, and read the key of every measure raw and smoothed. Names are
//! compared exactly and coefficients to a few ulps, since music21 squares
//! through the C library's `pow`.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//! `WINDOWED_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::analysis::floating_key::KeyAnalyzer;
use music21_rs::analysis::windowed::{WindowStep, WindowType, WindowedAnalysis};
use music21_rs::analysis::{KeyProfile, estimate_key_of_stream};
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::StreamElement;
use music21_rs::{FloatType, Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{SCORES, music21_name};

const MUSIC21: &str = r#"
import zipfile
from music21 import corpus
from music21.analysis import discrete, floatingKey, windowed

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

class Count:
    def process(self, s):
        return len(s.recurse().notesAndRests), None

TYPES = ['overlap', 'noOverlap', 'adjacentAverage']

def report(name):
    score = corpus.parse(name, forceSource=True)
    lines = []
    numbers = []
    keys = windowed.WindowedAnalysis(score, discrete.KrumhanslSchmuckler())
    counts = windowed.WindowedAnalysis(score, Count())
    lines.append(f'windows: {len(keys._windowedStream)}')
    for size in (1, 2, 4):
        for kind in TYPES:
            # music21 appends a measure to the stream of an adjacent average
            # once for every window holding it, which a stream refuses.
            if kind == 'adjacentAverage' and size > 1:
                continue
            data, _ = counts.analyze(size, windowType=kind)
            lines.append(f'count {size} {kind}: ' + ' '.join(str(d) for d in data))
            data, _ = keys.analyze(size, windowType=kind)
            names = []
            for p, mode, coefficient in data:
                names.append('None' if p is None else f'{p.name} {mode}')
                numbers.append(float(coefficient))
            lines.append(f'key {size} {kind}: ' + ' | '.join(names))
    _, _, meta = counts.process(1, None, '2x', 'overlap', True)
    lines.append('sizes: ' + ' '.join(str(m['windowSize']) for m in meta))
    try:
        analyzer = floatingKey.KeyAnalyzer(score)
    except floatingKey.FloatingKeyException:
        lines.append('floating: no measures')
        return '\n'.join(lines), numbers
    raw = analyzer.getRawKeyByMeasure()
    names = []
    for k in raw:
        if k is None:
            names.append('None')
        else:
            names.append(k.tonicPitchNameWithCase)
            numbers.append(float(k.correlationCoefficient))
    lines.append('raw: ' + ' '.join(names))
    for size in (4, 1):
        analyzer = floatingKey.KeyAnalyzer(score)
        analyzer.windowSize = size
        lines.append(f'smoothed {size}: ' + ' '.join(k.tonicPitchNameWithCase for k in analyzer.run()))
    return '\n'.join(lines), numbers
"#;

const TYPES: [(WindowType, &str); 3] = [
    (WindowType::Overlap, "overlap"),
    (WindowType::NoOverlap, "noOverlap"),
    (WindowType::AdjacentAverage, "adjacentAverage"),
];

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

/// A key name as music21 writes it, a flat as `-`.
fn music21_key(name: &str) -> String {
    let mut letters = name.chars();
    match letters.next() {
        Some(step) => format!("{step}{}", letters.as_str().replace('b', "-")),
        None => String::new(),
    }
}

fn report(score: &Stream) -> (String, Vec<FloatType>) {
    let mut lines = Vec::new();
    let mut numbers = Vec::new();
    let windowed = WindowedAnalysis::new(score).expect("the score is cut into windows");
    lines.push(format!("windows: {}", windowed.window_count()));
    let count = |window: &Stream| {
        window
            .leaves()
            .iter()
            .filter(|(_, element)| is_general_note(element))
            .count()
    };
    for size in [1, 2, 4] {
        for (window_type, label) in TYPES {
            if window_type == WindowType::AdjacentAverage && size > 1 {
                continue;
            }
            let counts: Vec<String> = windowed
                .analyze(size, window_type, count)
                .iter()
                .map(ToString::to_string)
                .collect();
            lines.push(format!("count {size} {label}: {}", counts.join(" ")));
            let keys = windowed.analyze(size, window_type, |window| {
                estimate_key_of_stream(KeyProfile::KrumhanslSchmuckler, window)
            });
            let mut names = Vec::new();
            for found in keys {
                match found {
                    Some(estimates) => {
                        let best = &estimates[0];
                        let tonic = music21_name(best.key().tonic_pitch());
                        names.push(format!("{tonic} {}", best.key().mode()));
                        numbers.push(best.score());
                    }
                    None => {
                        names.push("None".to_string());
                        numbers.push(0.0);
                    }
                }
            }
            lines.push(format!("key {size} {label}: {}", names.join(" | ")));
        }
    }
    let sizes: Vec<String> = windowed
        .process(
            Some(1),
            None,
            WindowStep::Multiply(2),
            WindowType::Overlap,
            true,
            count,
        )
        .expect("the windows are counted")
        .iter()
        .map(|(size, _)| size.to_string())
        .collect();
    lines.push(format!("sizes: {}", sizes.join(" ")));
    let Ok(mut analyzer) = KeyAnalyzer::new(score) else {
        lines.push("floating: no measures".to_string());
        return (lines.join("\n"), numbers);
    };
    let mut names = Vec::new();
    for found in analyzer.raw_key_by_measure() {
        match found {
            Some(estimates) => {
                names.push(music21_key(
                    &estimates[0].key().tonic_pitch_name_with_case(),
                ));
                numbers.push(estimates[0].score());
            }
            None => names.push("None".to_string()),
        }
    }
    lines.push(format!("raw: {}", names.join(" ")));
    for size in [4, 1] {
        analyzer.window_size = size;
        let keys: Vec<String> = analyzer
            .run()
            .expect("the keys are smoothed")
            .iter()
            .map(|key| music21_key(&key.tonic_pitch_name_with_case()))
            .collect();
        lines.push(format!("smoothed {size}: {}", keys.join(" ")));
    }
    (lines.join("\n"), numbers)
}

/// The first line the two reports differ on, by its label, and the first
/// item of that line they differ at, with a few either side of it.
fn first_difference(ours: &str, theirs: &str) -> Option<String> {
    let (ours, theirs): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), theirs.lines().collect());
    let at = (0..ours.len().max(theirs.len())).find(|&line| ours.get(line) != theirs.get(line))?;
    let split = |line: Option<&&str>| -> Vec<String> {
        let line = line.copied().unwrap_or("");
        let separator = if line.contains(" | ") { " | " } else { " " };
        line.split(separator).map(str::to_string).collect()
    };
    let (our_items, their_items) = (split(ours.get(at)), split(theirs.get(at)));
    let item = (0..our_items.len().max(their_items.len()))
        .find(|&index| our_items.get(index) != their_items.get(index))
        .unwrap_or(0);
    let window = |items: &[String]| {
        items
            .iter()
            .skip(item.saturating_sub(3))
            .take(7)
            .cloned()
            .collect::<Vec<_>>()
            .join(" , ")
    };
    let label = theirs
        .get(at)
        .copied()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    Some(format!(
        "{label}, item {item}\n  music21:    {}\n  music21-rs: {}",
        window(&their_items),
        window(&our_items)
    ))
}

#[test]
fn the_crate_windows_and_floats_keys_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"windowed_parity_music21.py",
            c"windowed_parity_music21",
        )?;
        let chosen: Vec<String> = match std::env::var("WINDOWED_PARITY_SCORES") {
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
            let (theirs, their_numbers): (String, Vec<FloatType>) =
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
            let (ours, our_numbers) = report(&score);
            if let Some(difference) = first_difference(&ours, &theirs) {
                failures.push(format!("{name}: {difference}"));
                continue;
            }
            let disagreeing = our_numbers
                .iter()
                .zip(&their_numbers)
                .position(|(ours, theirs)| (ours - theirs).abs() > 1e-12 * theirs.abs().max(1.0));
            if our_numbers.len() != their_numbers.len() || disagreeing.is_some() {
                failures.push(format!(
                    "{name}: coefficient {disagreeing:?} differs, {} against music21's {}",
                    our_numbers.len(),
                    their_numbers.len()
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
