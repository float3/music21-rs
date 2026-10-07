//! The crate's segment search against music21's: `search.segment`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Its parts are
//! indexed by music21's defaults and by the translation writing three
//! characters a note, which music21 reads measures off by character; and an
//! index of the score twice and of its first part is compared segment by
//! segment, both ways round. Every segment, measure and ratio must be music21's.
//! `SEARCH_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.

use music21_rs::FloatType;
use music21_rs::musicxml::from_musicxml;
use music21_rs::search::{
    Segments, index_score_parts, index_score_parts_with, score_similarity,
    translate_stream_to_string,
};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus
from music21.search import base, segment

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

def flat(index):
    return [(p['segmentList'], [tuple(m) for m in p['measureList']]) for p in index]

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    plain = segment.indexScoreParts(score)
    try:
        full = (flat(segment.indexScoreParts(score, algorithm=base.translateStreamToString)), '')
    except Exception as error:
        full = ([], type(error).__name__)
    rows = segment.scoreSimilarity({'first': plain, 'second': plain, 'third': plain[:1]},
                                   includeReverse=True, forceDifflib=True)
    return flat(plain), full, [(r[0], r[1], r[2], tuple(r[3]), r[4], r[5], r[6], tuple(r[7]), r[8])
                               for r in rows]
"#;

type Measures = (Option<i32>, Option<i32>);
type Part = (Vec<String>, Vec<Measures>);
type Row = (
    String,
    usize,
    usize,
    Measures,
    String,
    usize,
    usize,
    Measures,
    FloatType,
);
type Report = (Vec<Part>, (Vec<Part>, String), Vec<Row>);

fn flat(index: &[Segments]) -> Vec<Part> {
    index
        .iter()
        .map(|part| (part.segments.clone(), part.measures.clone()))
        .collect()
}

#[test]
fn the_crate_segments_scores_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"search_segment_parity_music21.py",
            c"search_segment_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("SEARCH_PARITY_SCORES") {
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
            let (their_plain, (their_full, their_error), their_rows): Report = match report {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;

            let plain = index_score_parts(&score).expect("music21's defaults");
            if flat(&plain) != their_plain {
                failures.push(format!(
                    "{name}: segments:\n  music21    {:?}\n  music21-rs {:?}",
                    their_plain.first(),
                    flat(&plain).first()
                ));
                continue;
            }
            match index_score_parts_with(&score, 30, 12, translate_stream_to_string) {
                Ok(full) => {
                    if !their_error.is_empty() || flat(&full) != their_full {
                        failures.push(format!(
                            "{name}: segments with lengths: music21 {their_error}, \
                             first music21 {:?}, music21-rs {:?}",
                            their_full.first(),
                            flat(&full).first()
                        ));
                    }
                }
                Err(error) => {
                    if their_error.is_empty() {
                        failures.push(format!(
                            "{name}: segments with lengths: music21-rs refused: {error}"
                        ));
                    }
                }
            }

            let index = [
                ("first".to_string(), plain.clone()),
                ("second".to_string(), plain.clone()),
                ("third".to_string(), plain[..plain.len().min(1)].to_vec()),
            ];
            let rows: Vec<Row> = score_similarity(&index, 20, true)
                .into_iter()
                .map(|row| {
                    (
                        row.this.score,
                        row.this.part,
                        row.this.segment,
                        row.this.measures,
                        row.that.score,
                        row.that.part,
                        row.that.segment,
                        row.that.measures,
                        row.ratio,
                    )
                })
                .collect();
            if rows != their_rows {
                let at = rows
                    .iter()
                    .zip(&their_rows)
                    .position(|(ours, theirs)| ours != theirs);
                failures.push(format!(
                    "{name}: similarity: {} rows, music21 {}; first difference at {at:?}: \
                     music21 {:?}, music21-rs {:?}",
                    rows.len(),
                    their_rows.len(),
                    at.and_then(|at| their_rows.get(at)),
                    at.and_then(|at| rows.get(at))
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
