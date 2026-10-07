//! The crate's lyric search against music21's: `search.lyrics.LyricSearcher`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). The text each
//! side indexes, every lyric's place in it, and what a handful of searches
//! find -- the first word, a piece of the middle, a stretch across a word
//! break, a single letter, a verse break and text that is not there -- must
//! be the same. `SEARCH_PARITY_SCORES`, a `;`-separated list of corpus
//! names, runs those instead of the writer test's scores, which is how the
//! corpus is swept.

use music21_rs::musicxml::from_musicxml;
use music21_rs::search::{LINE_BREAK, LyricIdentifier, LyricSearcher};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus
from music21.search import lyrics

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

def identifier(value):
    return ('number', value, '') if isinstance(value, int) else ('name', 0, str(value))

def report(text, queries):
    score = converter.parse(text, format='musicxml', forceSource=True)
    searcher = lyrics.LyricSearcher(score)
    indexed = [(i.start, i.end, i.measure, i.text, identifier(i.identifier),
                i.absoluteStart, i.absoluteEnd)
               for i in searcher.indexTuples]
    found = []
    for query in queries:
        try:
            found.append(([(m.mStart, m.mEnd, m.matchText, identifier(m.identifier),
                            [i.absoluteStart for i in m.indices])
                           for m in searcher.search(query)], ''))
        except Exception as error:
            found.append(([], type(error).__name__))
    return searcher.indexText, indexed, found
"#;

type Identifier = (String, i64, String);

/// Each lyric's start and end in its verse, measure, text, verse, and
/// start and end in the whole text.
type Indexed = (usize, usize, Option<i32>, String, Identifier, usize, usize);

/// A match's first and last measure, text, verse and the start of each
/// lyric it touches.
type Found = (Option<i32>, Option<i32>, String, Identifier, Vec<usize>);

/// music21's index text, its lyrics, and for each search its matches or
/// the exception it raised.
type Report = (String, Vec<Indexed>, Vec<(Vec<Found>, String)>);

fn identifier(value: &LyricIdentifier) -> Identifier {
    match value {
        LyricIdentifier::Number(number) => {
            ("number".to_string(), i64::from(*number), String::new())
        }
        LyricIdentifier::Name(name) => ("name".to_string(), 0, name.clone()),
    }
}

/// Searches worth making in a text: its first word, a piece of its middle,
/// a stretch across a word break, a letter, a verse break and text that is
/// not there.
fn queries(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut queries = vec!["a".to_string(), LINE_BREAK.to_string(), "zqzqz".to_string()];
    if let Some(word) = text.split(' ').find(|word| !word.is_empty()) {
        queries.push(word.to_string());
    }
    if chars.len() > 8 {
        let middle = chars.len() / 2;
        queries.push(chars[middle - 2..middle + 2].iter().collect());
    }
    if let Some(space) = chars.iter().skip(2).position(|c| *c == ' ') {
        let space = space + 2;
        if space + 3 <= chars.len() {
            queries.push(chars[space - 2..space + 3].iter().collect());
        }
    }
    queries
}

#[test]
fn the_crate_searches_lyrics_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"search_parity_music21.py",
            c"search_parity_music21",
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
            let searcher = LyricSearcher::new(&score);
            let queries = queries(searcher.index_text());
            let report = music21.getattr("report")?.call1((&text, queries.clone()));
            let (their_text, their_indexed, their_found): Report = match report {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            if searcher.index_text() != their_text {
                failures.push(format!(
                    "{name}: index text:\n  music21    {their_text:?}\n  music21-rs {:?}",
                    searcher.index_text()
                ));
                continue;
            }
            let our_indexed: Vec<Indexed> = searcher
                .indexed()
                .iter()
                .map(|lyric| {
                    (
                        lyric.start,
                        lyric.end,
                        lyric.measure,
                        lyric.text.clone(),
                        identifier(&lyric.identifier),
                        lyric.absolute_start,
                        lyric.absolute_end,
                    )
                })
                .collect();
            if our_indexed != their_indexed {
                let at = our_indexed
                    .iter()
                    .zip(&their_indexed)
                    .position(|(ours, theirs)| ours != theirs);
                failures.push(format!(
                    "{name}: {} lyrics indexed, music21 {}; first difference at {at:?}:\n  \
                     music21    {:?}\n  music21-rs {:?}",
                    our_indexed.len(),
                    their_indexed.len(),
                    at.and_then(|at| their_indexed.get(at)),
                    at.and_then(|at| our_indexed.get(at)),
                ));
                continue;
            }
            for (query, (theirs, their_error)) in queries.iter().zip(their_found) {
                match searcher.search(query) {
                    Ok(found) => {
                        let ours: Vec<Found> = found
                            .iter()
                            .map(|found| {
                                (
                                    found.measure_start,
                                    found.measure_end,
                                    found.text.clone(),
                                    identifier(&found.identifier),
                                    found
                                        .indices
                                        .iter()
                                        .map(|index| index.absolute_start)
                                        .collect(),
                                )
                            })
                            .collect();
                        if !their_error.is_empty() || ours != theirs {
                            failures.push(format!(
                                "{name}: search {query:?}: music21 {their_error} {} matches, \
                                 music21-rs {}; first:\n  music21    {:?}\n  music21-rs {:?}",
                                theirs.len(),
                                ours.len(),
                                theirs.first(),
                                ours.first()
                            ));
                        }
                    }
                    Err(error) => {
                        if their_error.is_empty() {
                            failures.push(format!(
                                "{name}: search {query:?}: music21 {} matches, \
                                 music21-rs refused: {error}",
                                theirs.len()
                            ));
                        }
                    }
                }
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
