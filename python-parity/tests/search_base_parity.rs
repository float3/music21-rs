//! The crate's stream searches against music21's: `search.base`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Its notes and
//! rests are translated by each of music21's translations; searched by
//! rhythm, by name, by both and by a `StreamSearcher` for runs like its own
//! first three, one with a wildcard in the middle; its parts are ranked by
//! each approximate search against the whole; and its measures are grouped
//! by rhythm. Every answer must be music21's. `SEARCH_PARITY_SCORES`, a
//! `;`-separated list of corpus names, runs those instead of the writer
//! test's scores, which is how the corpus is swept.

use music21_rs::FloatType;
use music21_rs::musicxml::from_musicxml;
use music21_rs::search::{
    Algorithm, Filter, SearchTerm, StreamSearcher, approximate_note_search,
    approximate_note_search_no_rhythm, approximate_note_search_only_rhythm,
    approximate_note_search_weighted, most_common_measure_rhythms, note_name_rhythmic_search,
    note_name_search, notes_and_rests, rhythmic_search, translate_diatonic_stream_to_string,
    translate_intervals_and_speed, translate_stream_to_string,
    translate_stream_to_string_no_rhythm, translate_stream_to_string_only_rhythm,
};
use music21_rs::stream::Stream;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import copy
import zipfile
from music21 import converter, corpus, search
from music21.search import base

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

def attempt(function):
    try:
        return (function(), '')
    except Exception as error:
        return (None, type(error).__name__)

def translations(score):
    out = []
    for function in (base.translateStreamToString,
                     base.translateStreamToStringNoRhythm,
                     base.translateStreamToStringOnlyRhythm,
                     base.translateDiatonicStreamToString,
                     # an iterator rather than its .stream(), which would sort
                     # the notes by their offsets in their measures
                     base.translateIntervalsAndSpeed):
        found, error = attempt(
            lambda: function(score.recurse().notesAndRests, returnMeasures=True))
        out.append((tuple(found) if found else ('', []), error))
    return out

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    notes = list(score.recurse().notesAndRests)
    out = {'translations': translations(score)}
    searches = []
    if len(notes) >= 3:
        plain = [copy.deepcopy(n) for n in notes[:3]]
        wild = [plain[0], base.Wildcard(), plain[2]]
        for pattern in (plain, wild):
            found = []
            for function in (base.rhythmicSearch, base.noteNameSearch, base.noteNameRhythmicSearch):
                found.append(function(score.recurse().notesAndRests, pattern))
            searcher = base.StreamSearcher(score, pattern)
            searcher.recurse = True
            searcher.filterNotes = True
            searcher.algorithms.append(base.StreamSearcher.rhythmAlgorithm)
            searcher.algorithms.append(base.StreamSearcher.noteNameAlgorithm)
            found.append([m.index for m in searcher.run()])
            searches.append(found)
    out['searches'] = searches
    parts = list(score.parts)
    approximate = []
    for function in (base.approximateNoteSearch, base.approximateNoteSearchNoRhythm,
                     base.approximateNoteSearchOnlyRhythm, base.approximateNoteSearchWeighted):
        ranked = function(score, parts)
        approximate.append([(parts.index(p), p.matchProbability) for p in ranked])
    out['approximate'] = approximate
    # music21 reads a measure's first note's pitch, and a chord has none.
    rhythms, error = attempt(lambda: base.mostCommonMeasureRhythms(score))
    out['rhythms'] = ([(d['number'], d['rhythmString'], [m.number for m in d['measures']])
                       for d in rhythms] if rhythms is not None else None)
    return (out['translations'], out['searches'], out['approximate'], out['rhythms'])
"#;

type Translated = (Vec<(String, Vec<Option<i32>>)>, Vec<String>);
type Ranked = Vec<(usize, FloatType)>;
type Rhythm = (usize, String, Vec<i32>);
type Report = (
    Vec<((String, Vec<Option<i32>>), String)>,
    Vec<Vec<Vec<usize>>>,
    Vec<Ranked>,
    Option<Vec<Rhythm>>,
);

/// The crate's translations of a score's notes and rests, and the error
/// each refused with, if it did.
fn translations(score: &Stream) -> Translated {
    let notes = notes_and_rests(score);
    let mut found = Vec::new();
    let mut errors = Vec::new();
    for translated in [
        Ok(translate_stream_to_string(&notes)),
        Ok(translate_stream_to_string_no_rhythm(&notes)),
        Ok(translate_stream_to_string_only_rhythm(&notes)),
        translate_diatonic_stream_to_string(&notes),
        translate_intervals_and_speed(&notes),
    ] {
        match translated {
            Ok(translation) => {
                found.push((translation.text, translation.measures));
                errors.push(String::new());
            }
            Err(error) => {
                found.push((String::new(), Vec::new()));
                errors.push(error.to_string());
            }
        }
    }
    (found, errors)
}

/// The crate's answers to the searches music21 was asked.
fn searches(score: &Stream) -> Vec<Vec<Vec<usize>>> {
    let notes = notes_and_rests(score);
    if notes.len() < 3 {
        return Vec::new();
    }
    let plain: Vec<SearchTerm> = notes[..3]
        .iter()
        .map(|found| SearchTerm::Element(found.element.clone()))
        .collect();
    let wild = vec![plain[0].clone(), SearchTerm::Wildcard, plain[2].clone()];
    [plain, wild]
        .into_iter()
        .map(|pattern| {
            let mut searcher = StreamSearcher::new(pattern.clone());
            searcher.recurse = true;
            searcher.filter = Some(Filter::Notes);
            searcher.algorithms.push(Algorithm::Rhythm);
            searcher.algorithms.push(Algorithm::NoteName);
            vec![
                rhythmic_search(&notes, &pattern).expect("a pattern"),
                note_name_search(&notes, &pattern).expect("a pattern"),
                note_name_rhythmic_search(&notes, &pattern).expect("a pattern"),
                searcher
                    .run(score)
                    .expect("a pattern")
                    .iter()
                    .map(|found| found.index)
                    .collect(),
            ]
        })
        .collect()
}

#[test]
fn the_crate_searches_streams_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"search_base_parity_music21.py",
            c"search_base_parity_music21",
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
            let (their_translations, their_searches, their_approximate, their_rhythms): Report =
                match report {
                    Ok(report) => report.extract()?,
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not read it: {error}"));
                        continue;
                    }
                };
            compared += 1;

            let (our_translations, our_errors) = translations(&score);
            let labels = ["string", "no rhythm", "only rhythm", "diatonic", "intervals"];
            for (((label, ours), our_error), (theirs, their_error)) in labels
                .iter()
                .zip(&our_translations)
                .zip(&our_errors)
                .zip(&their_translations)
            {
                let agree = if their_error.is_empty() {
                    our_error.is_empty() && ours == theirs
                } else {
                    !our_error.is_empty()
                };
                if !agree {
                    failures.push(format!(
                        "{name}: translation {label}: music21 {their_error} {:?}, \
                         music21-rs {our_error} {:?}",
                        theirs.0, ours.0
                    ));
                }
            }

            let ours = searches(&score);
            if ours != their_searches {
                failures.push(format!(
                    "{name}: searches:\n  music21    {their_searches:?}\n  music21-rs {ours:?}"
                ));
            }

            let parts = score.parts();
            let ours: Vec<Ranked> = [
                approximate_note_search(&score, &parts),
                approximate_note_search_no_rhythm(&score, &parts),
                approximate_note_search_only_rhythm(&score, &parts),
                approximate_note_search_weighted(&score, &parts),
            ]
            .to_vec();
            if ours != their_approximate {
                failures.push(format!(
                    "{name}: approximate:\n  music21    {their_approximate:?}\n  music21-rs {ours:?}"
                ));
            }

            let ours: Vec<Rhythm> = most_common_measure_rhythms(&score)
                .into_iter()
                .map(|rhythm| {
                    (
                        rhythm.count,
                        rhythm.rhythm,
                        rhythm.measures.iter().map(|measure| measure.number()).collect(),
                    )
                })
                .collect();
            if let Some(their_rhythms) = their_rhythms
                && ours != their_rhythms
            {
                failures.push(format!(
                    "{name}: measure rhythms: music21 {} kinds, music21-rs {}; first: \
                     music21 {:?}, music21-rs {:?}",
                    their_rhythms.len(),
                    ours.len(),
                    their_rhythms.first(),
                    ours.first()
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
