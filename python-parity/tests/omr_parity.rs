//! The crate's OMR correctors against music21's: `omr.correctors` and
//! `omr.evaluators`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader) and corrected by both
//! as music21's `ScoreCorrector` corrects one: the rhythm of every measure,
//! the measures flagged, each model's corrections and the distributions
//! they come from, the count of corrections made, and afterwards the
//! rhythms again and every measure's notes, rests, chords and barlines,
//! must be the same, or both must fail. `autoCorrelationBestMeasure` must
//! give the same counts. Floats are compared bit for bit.
//! `OMR_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.
//!
//! music21's own test pair, the first movement of Mozart's K. 525 read by
//! OMR and entered by hand, is evaluated by both with
//! `evaluateCorrectingModel`.

use music21_rs::musicxml::from_musicxml;
use music21_rs::omr::correctors::{MeasureRelationship, ScoreCorrector};
use music21_rs::omr::evaluators::{auto_correlation_best_measure, evaluate_correcting_model};
use music21_rs::{Stream, StreamElement};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import struct
import zipfile
from music21 import bar, converter, corpus, harmony, note, percussion, chord, stream
from music21.omr import correctors, evaluators

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

def hashes(corrector):
    return [' '.join(','.join(str(ord(c)) for c in h) for h in p.hashedNotes)
            for p in corrector.singleParts]

def relationships(lists):
    return [' '.join('(' + ','.join([str(r[0]), str(r[1]), str(r[2]), str(r[3]), bits(r[4])]) + ')'
                     for r in part) for part in lists]

def kind(el):
    if isinstance(el, (harmony.ChordSymbol, harmony.NoChord)):
        return 'ChordSymbol'
    if isinstance(el, percussion.PercussionChord):
        return 'PercussionChord'
    if isinstance(el, chord.Chord):
        return 'Chord'
    if isinstance(el, note.Unpitched):
        return 'Unpitched'
    if isinstance(el, note.Note):
        return 'Note'
    if isinstance(el, note.Rest):
        return 'Rest'
    if isinstance(el, bar.Barline):
        return 'Barline'
    return None

def pitch_text(p):
    alter = '' if p.accidental is None else bits(p.accidental.alter)
    return f'{p.step}{p.octave}:{alter}'

def measures(score):
    lines = []
    for pn, part in enumerate(score.parts):
        for mn, m in enumerate(part.getElementsByClass(stream.Measure)):
            items = []
            for el in m.recurse():
                k = kind(el)
                if k is None or el is m.leftBarline or el is m.rightBarline:
                    continue
                text = f'{k}@{bits(el.getOffsetInHierarchy(m))}'
                if k == 'Barline':
                    text += '=' + el.type
                else:
                    text += '/' + bits(el.duration.quarterLength)
                    if k in ('Note', 'Chord'):
                        text += '[' + ' '.join(pitch_text(p) for p in el.pitches) + ']'
                items.append(text)
            left = m.leftBarline.type if m.leftBarline is not None else '-'
            right = m.rightBarline.type if m.rightBarline is not None else '-'
            lines.append(f'measure {pn} {mn} {left} {right}: ' + ' '.join(sorted(items)))
    return lines

def report(text):
    lines = []
    score = converter.parse(text, format='musicxml', forceSource=True)
    try:
        corrector = correctors.ScoreCorrector(score)
    except Exception as e:
        return ['error'], f'{type(e).__name__}: {e}'
    lines += ['hashes ' + h for h in hashes(corrector)]
    lines += ['incorrect ' + str(list(p.incorrectMeasures)) for p in corrector.singleParts]
    try:
        horizontal = corrector.runHorizontalCorrectionModel()
        lines += ['horizontal ' + r for r in relationships(horizontal)]
        for p in corrector.singleParts:
            if p.probabilityDistribution is not None:
                lines.append('hdist ' + ' '.join(bits(x) for x in p.probabilityDistribution))
        vertical = corrector.runVerticalCorrectionModel()
        lines += ['vertical ' + r for r in relationships(vertical)]
        lines += ['vdist ' + ' '.join(bits(x) for x in row) for row in corrector.distributionArray]
        prior = corrector.generateCorrectedScore(horizontal, vertical)
        lines.append('prior ' + ' '.join(str(x) for x in prior))
        lines += ['after ' + h for h in hashes(corrector)]
        lines += measures(corrector.score)
    except Exception as e:
        lines.append('error')
        return lines, f'{type(e).__name__}: {e}'
    return lines, ''

def auto(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    try:
        return str(evaluators.autoCorrelationBestMeasure(score))
    except Exception as e:
        return 'error'

def k525(omr, ground):
    result = evaluators.evaluateCorrectingModel(omr, ground)
    return [result['originalEditDistance'], result['newEditDistance'],
            result['numberOfFlaggedMeasures'], result['totalNumberOfMeasures']]
"#;

fn bits(value: f64) -> String {
    value.to_bits().to_string()
}

fn hash_lines(corrector: &ScoreCorrector) -> Vec<String> {
    corrector
        .all_hashes()
        .iter()
        .map(|part| {
            part.iter()
                .map(|hash| {
                    hash.chars()
                        .map(|c| (c as u32).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn relationship_lines(lists: &[Vec<MeasureRelationship>]) -> Vec<String> {
    lists
        .iter()
        .map(|part| {
            part.iter()
                .map(|r| {
                    format!(
                        "({},{},{},{},{})",
                        r.flagged_measure_part,
                        r.flagged_measure_index,
                        r.correct_measure_part,
                        r.correct_measure_index,
                        bits(r.correction_probability)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
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

fn measure_lines(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    for (pn, part) in score.parts().into_iter().enumerate() {
        for (mn, measure) in part.measures().into_iter().enumerate() {
            let mut items = Vec::new();
            for (offset, element) in measure.recurse() {
                let (kind, pitches) = match element {
                    StreamElement::Note(note) => ("Note", Some(vec![note.pitch().clone()])),
                    StreamElement::Chord(chord) => (
                        "Chord",
                        Some(
                            chord
                                .notes()
                                .iter()
                                .map(|note| note.pitch().clone())
                                .collect(),
                        ),
                    ),
                    StreamElement::Rest(_) => ("Rest", None),
                    StreamElement::Unpitched(_) => ("Unpitched", None),
                    StreamElement::PercussionChord(_) => ("PercussionChord", None),
                    StreamElement::ChordSymbol(_) => ("ChordSymbol", None),
                    StreamElement::Barline(_) => ("Barline", None),
                    _ => continue,
                };
                let mut text = format!("{kind}@{}", bits(offset));
                if let StreamElement::Barline(barline) = element {
                    text += &format!("={}", barline.bar_type().as_str());
                } else {
                    text += &format!("/{}", bits(element.quarter_length()));
                    if let Some(pitches) = pitches {
                        let written: Vec<String> = pitches.iter().map(pitch_text).collect();
                        text += &format!("[{}]", written.join(" "));
                    }
                }
                items.push(text);
            }
            items.sort();
            let left = measure
                .left_barline()
                .map_or("-", |barline| barline.bar_type().as_str());
            let right = measure
                .right_barline()
                .map_or("-", |barline| barline.bar_type().as_str());
            lines.push(format!(
                "measure {pn} {mn} {left} {right}: {}",
                items.join(" ")
            ));
        }
    }
    lines
}

fn our_report(score: Stream) -> (Vec<String>, String) {
    let mut lines = Vec::new();
    let mut corrector = match ScoreCorrector::new(score) {
        Ok(corrector) => corrector,
        Err(error) => return (vec!["error".to_string()], error.to_string()),
    };
    lines.extend(
        hash_lines(&corrector)
            .into_iter()
            .map(|h| format!("hashes {h}")),
    );
    for part in corrector.single_parts() {
        lines.push(format!("incorrect {:?}", part.incorrect_measures()));
    }
    let horizontal = corrector.run_horizontal_correction_model();
    lines.extend(
        relationship_lines(&horizontal)
            .into_iter()
            .map(|r| format!("horizontal {r}")),
    );
    for part in 0..corrector.single_parts().len() {
        if corrector.single_parts()[part].index_array().is_some() {
            let dist = corrector
                .single_part_mut(part)
                .horizontal_probability_dist();
            let written: Vec<String> = dist.iter().map(|x| bits(*x)).collect();
            lines.push(format!("hdist {}", written.join(" ")));
        }
    }
    let rest = (|| -> music21_rs::Result<Vec<String>> {
        let mut lines = Vec::new();
        let vertical = corrector.run_vertical_correction_model()?;
        lines.extend(
            relationship_lines(&vertical)
                .into_iter()
                .map(|r| format!("vertical {r}")),
        );
        for row in corrector.vertical_probability_dist()? {
            let written: Vec<String> = row.iter().map(|x| bits(*x)).collect();
            lines.push(format!("vdist {}", written.join(" ")));
        }
        let prior = corrector.generate_corrected_score(&horizontal, &vertical)?;
        lines.push(format!(
            "prior {} {} {} {}",
            prior.total, prior.horizontal, prior.vertical, prior.ignored
        ));
        lines.extend(
            hash_lines(&corrector)
                .into_iter()
                .map(|h| format!("after {h}")),
        );
        lines.extend(measure_lines(corrector.score()));
        Ok(lines)
    })();
    match rest {
        Ok(more) => {
            lines.extend(more);
            (lines, String::new())
        }
        Err(error) => {
            lines.push("error".to_string());
            (lines, error.to_string())
        }
    }
}

/// The first line two reports differ on, and what each has there.
fn first_difference(ours: &[String], theirs: &[String]) -> String {
    let at = ours
        .iter()
        .zip(theirs)
        .position(|(a, b)| a != b)
        .unwrap_or(ours.len().min(theirs.len()));
    let cut = |line: Option<&String>| line.map(|line| line.chars().take(300).collect::<String>());
    format!(
        "{} lines and {} in music21; first difference at line {at}:\n  music21    {:?}\n  music21-rs {:?}",
        ours.len(),
        theirs.len(),
        cut(theirs.get(at)),
        cut(ours.get(at))
    )
}

fn load<'py>(py: Python<'py>, root: &std::path::Path) -> PyResult<Bound<'py, PyModule>> {
    init_py(py)?;
    add_dependency_venv(py, root)?;
    PyModule::from_code(
        py,
        &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
        c"omr_parity_music21.py",
        c"omr_parity_music21",
    )
}

#[test]
fn the_crate_corrects_scores_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        let music21 = load(py, &root)?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("OMR_PARITY_SCORES") {
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
            compared += 1;
            let (theirs, their_error): (Vec<String>, String) =
                music21.getattr("report")?.call1((&text,))?.extract()?;
            let (ours, our_error) = our_report(score.clone());
            if ours != theirs {
                failures.push(format!(
                    "{name}: {}\n  music21 error {their_error:?}, music21-rs error {our_error:?}",
                    first_difference(&ours, &theirs)
                ));
            }
            let their_auto: String = music21.getattr("auto")?.call1((&text,))?.extract()?;
            let our_auto = match auto_correlation_best_measure(score) {
                Ok(pair) => format!("{pair:?}"),
                Err(_) => "error".to_string(),
            };
            if our_auto != their_auto {
                failures.push(format!(
                    "{name}: autoCorrelationBestMeasure {their_auto} in music21, {our_auto} in music21-rs"
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

#[test]
fn the_crate_evaluates_music21s_omr_pair_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let directory = root.join("music21").join("music21").join("omr");
    for (omr, ground) in [
        ("k525OMRshort.xml", "k525GTshort.xml"),
        ("k525OMRMvt1.xml", "k525GTMvt1.xml"),
    ] {
        let (omr_path, ground_path) = (directory.join(omr), directory.join(ground));
        let theirs: Vec<usize> = Python::attach(|py| -> PyResult<Vec<usize>> {
            let music21 = load(py, &root)?;
            music21
                .getattr("k525")?
                .call1((omr_path.to_string_lossy(), ground_path.to_string_lossy()))?
                .extract()
        })
        .expect("the Python side runs");
        let read = |path: &std::path::Path| {
            let text = std::fs::read_to_string(path).expect("read the score");
            from_musicxml(&text).expect("the crate reads the score")
        };
        let ours = evaluate_correcting_model(read(&omr_path), read(&ground_path), None)
            .expect("the crate evaluates the pair");
        assert_eq!(
            vec![
                ours.original_edit_distance,
                ours.new_edit_distance,
                ours.number_of_flagged_measures,
                ours.total_number_of_measures,
            ],
            theirs,
            "{omr}"
        );
    }
}
