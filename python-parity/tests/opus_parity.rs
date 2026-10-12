//! The crate's opus lookups against music21's `stream.Opus`.
//!
//! Each multi-tune ABC file of the corpus is read by music21 and by
//! `from_abc`. Both are asked for their scores' numbers, for the score
//! under each of those numbers, for the score under each title, and for the
//! one score the opus merges to. The answers are compared by the title the
//! found score carries, so a lookup that finds the wrong tune is caught.
//!
//! A title holding any of `*.|+?{}` is a regular expression to music21.
//! The crate compiles no pattern, so `MetadataQuery::Matcher` is handed one
//! `regex` built here for exactly those titles, which is what a caller with
//! an engine of their own would do.
//!
//! Ten titles of the Essen collections hold unbalanced brackets or a `(?)`,
//! which music21 compiles as a pattern and `re` refuses, so music21 raises
//! where it is asked for one of its own titles. Those are counted, and the
//! crate is held to finding the score.
//!
//! `OPUS_PARITY_WORKS`, a `;`-separated list of corpus files, runs those
//! instead of `WORKS`, which is how the rest of the corpus is swept.

use music21_rs::abc::from_abc;
use music21_rs::metadata::{Metadata, MetadataQuery};
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use regex::Regex;
use utils::{init_py, prepare};

/// The corpus's multi-tune ABC files, smallest first.
const WORKS: &[&str] = &[
    "josquin/4vPerIlludAveProlatum.abc",
    "josquin/fortunaDunGranTempo.abc",
    "josquin/laPlusDesPlus.abc",
    "josquin/oVenusBant.abc",
    "josquin/adieuMesAmours.abc",
    "josquin/milleRegrets.abc",
    "josquin/laDeplorationDeLaMorteDeJohannesOckeghem.abc",
    "josquin/petiteCamusette.abc",
    "essenFolksong/testd.abc",
    "essenFolksong/teste.abc",
    "oneills1850/0626-0635.abc",
];

const MUSIC21: &str = r#"
from music21 import converter, corpus, stream

def source_text(name):
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    with open(str(path), encoding='utf-8') as handle:
        return handle.read()

def opus(text):
    return converter.parse(text, format='abc')

def is_opus(o):
    return isinstance(o, stream.Opus)

def numbers(o):
    out = []
    for s in o.getElementsByClass(stream.Score):
        md = s.metadata
        out.append(None if md is None else md.number)
    return out

def titles(o):
    out = []
    for s in o.getElementsByClass(stream.Score):
        md = s.metadata
        out.append(None if md is None else md.title)
    return out

def title_of(s):
    if s is None:
        return None
    md = s.metadata
    return None if md is None else md.title

def by_number(o, query):
    return title_of(o.getScoreByNumber(query))

def by_title(o, query):
    """The title the query finds, or the class of the error it raises."""
    try:
        return (title_of(o.getScoreByTitle(query)), None)
    except Exception as error:
        return (None, type(error).__name__)

def merged(o):
    """The merged score's parts and metadata, or the error it raises."""
    try:
        s = o.mergeScores()
    except Exception as error:
        return (None, type(error).__name__)
    md = s.metadata
    return ((len(s.parts), None if md is None else md.title,
             None if md is None else md.composer), None)
"#;

/// Whether music21 reads a plain query as a regular expression.
fn reads_a_pattern(query: &str) -> bool {
    query.chars().any(|character| "*.|+?{}".contains(character))
}

/// The score a query finds, named by its title, as the Python side names it.
fn found_title(found: Option<&Stream>) -> Option<String> {
    found.and_then(Stream::metadata).and_then(Metadata::title)
}

/// The title under a query, music21's reading of a pattern in it kept.
fn by_title(opus: &Stream, query: &str) -> Option<String> {
    if reads_a_pattern(query)
        && let Ok(pattern) = Regex::new(&format!("(?i){query}"))
    {
        let matches = |text: &str| pattern.is_match(text);
        return found_title(opus.score_by_title(&MetadataQuery::Matcher(&matches)));
    }
    // A query no engine will compile is searched for as it stands.
    found_title(opus.score_by_title(&MetadataQuery::Text(query)))
}

#[test]
fn the_crate_looks_up_opus_scores_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared, uncompilable) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
            PyModule::from_code(
                py,
                &std::ffi::CString::new(code).expect("no nul in the helper"),
                &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                &std::ffi::CString::new(name).expect("no nul in the name"),
            )
        };
        let music21 = module(MUSIC21, "opus_parity_music21")?;

        let chosen: Vec<String> = match std::env::var("OPUS_PARITY_WORKS") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => WORKS.iter().map(|name| (*name).to_string()).collect(),
        };

        let mut failures = Vec::new();
        let mut compared = 0;
        let mut uncompilable = 0;
        for name in &chosen {
            let text: String = music21.getattr("source_text")?.call1((name,))?.extract()?;
            let our_opus = match from_abc(&text) {
                Ok(opus) => opus,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let their_opus = music21.getattr("opus")?.call1((&text,))?;
            let theirs_is_opus: bool = music21
                .getattr("is_opus")?
                .call1((&their_opus,))?
                .extract()?;
            if !theirs_is_opus || our_opus.kind() != StreamKind::Opus {
                failures.push(format!(
                    "{name}: an opus to music21 {theirs_is_opus}, to the crate {}",
                    our_opus.kind() == StreamKind::Opus
                ));
                continue;
            }
            compared += 1;

            let their_numbers: Vec<Option<String>> = music21
                .getattr("numbers")?
                .call1((&their_opus,))?
                .extract()?;
            if our_opus.score_numbers() != their_numbers {
                failures.push(format!(
                    "{name}: numbers are {their_numbers:?} to music21, {:?} to the crate",
                    our_opus.score_numbers()
                ));
                continue;
            }
            let their_titles: Vec<Option<String>> =
                music21.getattr("titles")?.call1((&their_opus,))?.extract()?;
            let our_titles: Vec<Option<String>> = our_opus
                .scores()
                .into_iter()
                .map(|score| found_title(Some(score)))
                .collect();
            if our_titles != their_titles {
                failures.push(format!(
                    "{name}: titles are {their_titles:?} to music21, {our_titles:?} to the crate"
                ));
                continue;
            }

            for number in their_numbers.iter().flatten() {
                let theirs: Option<String> = music21
                    .getattr("by_number")?
                    .call1((&their_opus, number))?
                    .extract()?;
                let ours = found_title(our_opus.score_by_number(&MetadataQuery::Text(number)));
                if ours != theirs {
                    failures.push(format!(
                        "{name}: number {number:?} is {theirs:?} to music21, {ours:?} to the crate"
                    ));
                }
            }
            for title in their_titles.iter().flatten() {
                let (theirs, raised): (Option<String>, Option<String>) = music21
                    .getattr("by_title")?
                    .call1((&their_opus, title))?
                    .extract()?;
                let ours = by_title(&our_opus, title);
                if raised.is_some() {
                    // music21 compiles a title holding `*.|+?{}` as a
                    // pattern, so ten titles of the Essen collections
                    // raise `re.error` there rather than finding their own
                    // score. The crate finds it.
                    uncompilable += 1;
                    if ours.as_deref() != Some(title.as_str()) {
                        failures.push(format!(
                            "{name}: title {title:?} raises {raised:?} in music21,                              and the crate answers {ours:?} where it should find the score"
                        ));
                    }
                    continue;
                }
                if ours != theirs {
                    failures.push(format!(
                        "{name}: title {title:?} is {theirs:?} to music21, {ours:?} to the crate"
                    ));
                }
            }
            // A title no score carries, and a pattern the first title answers.
            let missing =
                found_title(our_opus.score_by_title(&MetadataQuery::Text("no such tune")));
            let (their_missing, _): (Option<String>, Option<String>) = music21
                .getattr("by_title")?
                .call1((&their_opus, "no such tune"))?
                .extract()?;
            if missing != their_missing {
                failures.push(format!(
                    "{name}: a title no score carries is {their_missing:?} to music21, {missing:?} to the crate"
                ));
            }
            if let Some(Some(first)) = their_titles.first()
                && let Some(middle) = first.split_whitespace().nth(1)
            {
                let pattern = format!("(.*){middle}(.*)");
                let (theirs, _): (Option<String>, Option<String>) = music21
                    .getattr("by_title")?
                    .call1((&their_opus, &pattern))?
                    .extract()?;
                let ours = by_title(&our_opus, &pattern);
                if ours != theirs {
                    failures.push(format!(
                        "{name}: the pattern {pattern:?} is {theirs:?} to music21, {ours:?} to the crate"
                    ));
                }
            }

            let their_merge: (Option<(usize, Option<String>, Option<String>)>, Option<String>) =
                music21.getattr("merged")?.call1((&their_opus,))?.extract()?;
            let our_merge = our_opus.merge_scores();
            match (their_merge, our_merge) {
                ((Some(theirs), _), Ok(merged)) => {
                    let ours = (
                        merged.parts().len(),
                        merged.metadata().and_then(Metadata::title),
                        merged
                            .metadata()
                            .and_then(|metadata| metadata.string_value("composer")),
                    );
                    if ours != theirs {
                        failures.push(format!(
                            "{name}: merged is {theirs:?} to music21, {ours:?} to the crate"
                        ));
                    }
                }
                ((None, raised), Err(_)) => {
                    assert!(raised.is_some(), "music21 refused without naming a class");
                }
                ((theirs, raised), ours) => failures.push(format!(
                    "{name}: merging is {theirs:?}/{raised:?} to music21, {} to the crate",
                    match ours {
                        Ok(merged) => format!("{} parts", merged.parts().len()),
                        Err(error) => format!("refused: {error}"),
                    }
                )),
            }
        }
            Ok((failures, compared, uncompilable))
        })
        .unwrap_or_else(|error| panic!("asking music21 about its opuses: {error}"));

    assert!(
        failures.is_empty(),
        "{} differences over {compared} opuses:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(compared > 0, "no opus was compared");
    println!("{compared} opuses compared, {uncompilable} titles music21 cannot compile");
}
