//! The crate's `text` module and metadata text against music21's:
//! `text.assembleLyrics`, `assembleAllLyrics`, `prependArticle`,
//! `postpendArticle`, and each metadata attribute read as text.
//!
//! The articles are moved on a list of titles in every language and in
//! none. Then the text of each corpus score is read by music21 and by
//! `from_musicxml` (which `musicxml_read_parity` holds to music21's
//! reader): the lyrics of each line, all of them, and every attribute of
//! the metadata vocabulary must read the same, but for the software entry
//! music21 makes for itself in every score it reads. `TEXT_PARITY_SCORES`, a `;`-separated list of corpus
//! names, runs those instead of the writer test's scores, which is how the
//! corpus is swept.

use music21_rs::metadata::{Metadata, STANDARD_PROPERTIES};
use music21_rs::musicxml::from_musicxml;
use music21_rs::text::{
    ARTICLES, assemble_all_lyrics, assemble_lyrics, postpend_article, prepend_article,
};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
import music21
from music21 import converter, corpus, text

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

def articles(titles, languages):
    out = []
    for title in titles:
        for language in languages:
            out.append((text.prependArticle(title, language),
                        text.postpendArticle(title, language)))
    return out

OWN = 'music21 v.' + music21.VERSION_STR

def without_own(value):
    if value == OWN:
        return None
    if value is not None and value.startswith(OWN + ', '):
        return value[len(OWN) + 2:]
    return value

def report(name, unique_names):
    score = converter.parse(source_text(name), format='musicxml', forceSource=True)
    lines = [text.assembleLyrics(score, n) for n in (0, 1, 2, 3)]
    lines.append(text.assembleAllLyrics(score))
    lines.append(text.assembleLyrics(score, 1, wordSeparator='|'))
    md = score.metadata
    values = []
    for unique in unique_names:
        value = None if md is None else md._getSingularAttribute(unique)
        values.append(without_own(value) if unique == 'software' else value)
    return lines, values
"#;

const TITLES: [&str; 12] = [
    "Ale is Dear, The",
    "The Ale is Dear",
    "Combattimento di Tancredi e Clorinda, Il",
    "Il Combattimento di Tancredi e Clorinda",
    "Mädchen, Das",
    "Sonata, A",
    "Song, de la",
    "Lied, Der, Ein",
    "Untitled",
    "Two words",
    ", The",
    "het Wilhelmus",
];

fn our_value(metadata: Option<&Metadata>, unique_name: &str) -> Option<String> {
    metadata.and_then(|metadata| metadata.string_value(unique_name))
}

#[test]
fn the_crate_reads_text_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"text_parity_music21.py",
            c"text_parity_music21",
        )?;
        let mut failures = Vec::new();

        let mut languages: Vec<Option<&str>> = vec![None];
        languages.extend(ARTICLES.iter().map(|(code, _)| Some(*code)));
        let theirs: Vec<(String, String)> = music21
            .getattr("articles")?
            .call1((TITLES.to_vec(), languages.clone()))?
            .extract()?;
        let mut expected = theirs.into_iter();
        for title in TITLES {
            for language in &languages {
                let ours = (
                    prepend_article(title, *language).expect("a known language"),
                    postpend_article(title, *language).expect("a known language"),
                );
                let theirs = expected.next().expect("one answer per title and language");
                if ours != theirs {
                    failures.push(format!(
                        "{title:?} in {language:?}: music21 {theirs:?}, music21-rs {ours:?}"
                    ));
                }
            }
        }

        let unique_names: Vec<&str> = STANDARD_PROPERTIES
            .iter()
            .map(|(name, _, _)| *name)
            .collect();
        let chosen: Vec<String> = match std::env::var("TEXT_PARITY_SCORES") {
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
            let (their_lines, their_values): (Vec<String>, Vec<Option<String>>) = match music21
                .getattr("report")?
                .call1((name, unique_names.clone()))
            {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            let score = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            let mut our_lines: Vec<String> = [0, 1, 2, 3]
                .into_iter()
                .map(|line| assemble_lyrics(&score, line, " "))
                .collect();
            our_lines.push(assemble_all_lyrics(&score, 10, "\n", " "));
            our_lines.push(assemble_lyrics(&score, 1, "|"));
            for (index, (ours, theirs)) in our_lines.iter().zip(&their_lines).enumerate() {
                if ours != theirs {
                    failures.push(format!(
                        "{name}: lyrics {index}:\n  music21    {theirs:?}\n  music21-rs {ours:?}"
                    ));
                }
            }
            for (unique, theirs) in unique_names.iter().zip(their_values) {
                let ours = our_value(score.metadata(), unique);
                if ours != theirs {
                    failures.push(format!(
                        "{name}: {unique}: music21 {theirs:?}, music21-rs {ours:?}"
                    ));
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
