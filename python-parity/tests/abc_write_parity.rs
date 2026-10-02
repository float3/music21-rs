//! The crate's ABC writer, tune by tune.
//!
//! music21 writes no ABC, so there is no text of its to hold the writer to.
//! It is held to two readers instead, on tunes of music21's corpus:
//!
//! 1. The crate's own. A tune is read by `from_abc`, written by `to_abc` and
//!    read again, and the two scores must be the same -- compared as the
//!    MusicXML `to_musicxml` writes of each, or as an outline of each where
//!    that writer refuses the score.
//! 2. music21's. music21 reads what the crate wrote and reads the tune it was
//!    written from, and must make the same score of both -- compared as the
//!    MusicXML its exporter writes of each, or as an outline where its
//!    exporter refuses, which it does of a tune with no measures.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! A tune is named by its corpus file, with `#n` after it for the tune
//! numbered `n` of a file holding several. `ABC_WRITE_PARITY_TUNES`, a
//! `;`-separated list of such names or `@` and the path of a file of them,
//! runs those instead of the reader test's own. `ABC_WRITE_PARITY_REPORT`
//! names a file to write a line for every tune to: `ok`, or what differed.
//! A tune that differs leaves what was read and written under
//! `target/abc-write-parity/`.

use music21_rs::Stream;
use music21_rs::abc::{ExportOptions as AbcOptions, from_abc, from_abc_number, to_abc};
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{
    ABC_SOURCE_TEXT, ABC_TUNES, OUTLINE, STRIP_LAYOUT, first_difference, flat_outline,
    normalize_ids,
};

/// Tunes of the reader test's that cannot be written so as to be read back,
/// and why not.
const NOT_READ_BACK: &[(&str, &str)] = &[(
    "airdsAirs/book4.abc#0722",
    "barlines written among the header fields leave the meter in the part      outside every measure; ABC written properly states the meter in the      header, which is read into the first measure",
)];

/// music21 reading ABC text and writing what it read: as MusicXML where its
/// exporter will, as an outline where it will not.
const MUSIC21_READS: &str = r#"
from music21 import converter
from music21.musicxml import m21ToXml

def read(text, number, strip_layout, flat_outline):
    if number is None:
        score = converter.parse(text, format='abc')
    else:
        score = converter.parse(text, format='abc', number=number)
    score = strip_layout(score)
    try:
        exporter = m21ToXml.GeneralObjectExporter(score)
        exporter.makeNotation = False
        return 'musicxml', exporter.parse().decode('utf-8')
    except Exception:
        return 'outline', flat_outline(score)
"#;

/// The score as text to compare: MusicXML, or the outline where the writer
/// refuses it.
fn written_out(score: &Stream) -> (&'static str, String) {
    match to_musicxml(score, &ExportOptions::default()) {
        Ok(document) => ("musicxml", normalize_ids(&document)),
        Err(_) => ("outline", flat_outline(score)),
    }
}

#[test]
fn what_the_crate_writes_as_abc_is_read_back_as_the_same_score() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (lines, outlined) = Python::attach(|py| -> PyResult<(Vec<(String, String)>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
            PyModule::from_code(
                py,
                &std::ffi::CString::new(code).expect("no nul in the helper"),
                &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                &std::ffi::CString::new(name).expect("no nul in the name"),
            )
        };
        let helpers = module(STRIP_LAYOUT, "musicxml_parity_helpers")?;
        let outlines = module(OUTLINE, "outline_helpers")?;
        let sources = module(ABC_SOURCE_TEXT, "abc_source_text")?;
        let reads = module(MUSIC21_READS, "abc_write_reads")?;
        let strip_layout = helpers.getattr("strip_layout")?;
        let python_outline = outlines.getattr("flat_outline")?;

        let chosen: Vec<String> = match std::env::var("ABC_WRITE_PARITY_TUNES") {
            Ok(list) => list
                .strip_prefix('@')
                .map(|path| std::fs::read_to_string(path).expect("the list of tunes"))
                .unwrap_or(list)
                .split([';', '\n'])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => ABC_TUNES
                .iter()
                .map(|(name, _)| name.to_string())
                .filter(|name| !NOT_READ_BACK.iter().any(|(known, _)| known == name))
                .collect(),
        };
        let directory = root.join("target").join("abc-write-parity");
        let keep = |tune: &str, files: &[(&str, &str)]| {
            let stem = tune.replace(['/', '#'], "_");
            if std::fs::create_dir_all(&directory).is_ok() {
                for (suffix, text) in files {
                    let _ = std::fs::write(directory.join(format!("{stem}.{suffix}")), text);
                }
            }
        };

        let mut lines = Vec::new();
        let mut outlined = 0;
        let mut texts: Option<(String, String)> = None;
        for tune in &chosen {
            let (name, number) = match tune.split_once('#') {
                Some((name, number)) => (name, number.parse::<i32>().ok()),
                None => (tune.as_str(), None),
            };
            // One file's text serves every tune of it.
            if texts.as_ref().is_none_or(|(known, _)| known != name) {
                let text: String = sources.getattr("source_text")?.call1((name,))?.extract()?;
                texts = Some((name.to_string(), text));
            }
            let text = &texts.as_ref().expect("the text just read").1;
            let mut say = |verdict: String| lines.push((tune.clone(), verdict));

            let read = match number {
                Some(number) => from_abc_number(text, number),
                None => from_abc(text),
            };
            let score = match read {
                Ok(score) => score,
                Err(error) => {
                    say(format!("the crate cannot read the tune: {error}"));
                    continue;
                }
            };
            let written = match to_abc(&score, &AbcOptions::default()) {
                Ok(written) => written,
                Err(error) => {
                    say(format!("the crate refused to write it: {error}"));
                    continue;
                }
            };

            // The crate reads back what it wrote.
            let again = match from_abc(&written) {
                Ok(again) => again,
                Err(error) => {
                    keep(tune, &[("written.abc", &written)]);
                    say(format!("the crate cannot read what it wrote: {error}"));
                    continue;
                }
            };
            let (first_kind, first) = written_out(&score);
            let (second_kind, second) = written_out(&again);
            if let Some(difference) = first_difference(&second, &first) {
                keep(
                    tune,
                    &[
                        ("written.abc", &written),
                        ("crate-original.txt", &first),
                        ("crate-reread.txt", &second),
                    ],
                );
                say(format!(
                    "the crate reads its own text differently ({first_kind} against {second_kind}): {difference}"
                ));
                continue;
            }

            // music21 reads what the crate wrote as it reads the tune.
            let theirs = |text: &str, number: Option<i32>| -> PyResult<(String, String)> {
                reads
                    .getattr("read")?
                    .call1((text, number, &strip_layout, &python_outline))?
                    .extract()
            };
            let original = match theirs(text, number) {
                Ok(read) => read,
                Err(error) => {
                    say(format!("music21 cannot read the tune: {error}"));
                    continue;
                }
            };
            let rewritten = match theirs(&written, None) {
                Ok(read) => read,
                Err(error) => {
                    keep(tune, &[("written.abc", &written)]);
                    say(format!("music21 cannot read what the crate wrote: {error}"));
                    continue;
                }
            };
            let theirs_first = normalize_ids(original.1.trim_end());
            let theirs_second = normalize_ids(rewritten.1.trim_end());
            if let Some(difference) = first_difference(&theirs_second, &theirs_first) {
                keep(
                    tune,
                    &[
                        ("written.abc", &written),
                        ("music21-original.txt", &theirs_first),
                        ("music21-reread.txt", &theirs_second),
                    ],
                );
                say(format!(
                    "music21 reads the crate's text differently ({} against {}): {difference}",
                    original.0, rewritten.0
                ));
                continue;
            }
            if first_kind == "outline" || original.0 == "outline" {
                outlined += 1;
            }
            say("ok".to_string());
        }
        Ok((lines, outlined))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    if let Ok(path) = std::env::var("ABC_WRITE_PARITY_REPORT") {
        let report: String = lines
            .iter()
            .map(|(tune, verdict)| {
                format!("{tune}\t{}\n", verdict.lines().next().unwrap_or_default())
            })
            .collect();
        std::fs::write(path, report).expect("write the report");
    }
    let failures: Vec<String> = lines
        .iter()
        .filter(|(_, verdict)| verdict != "ok")
        .map(|(tune, verdict)| format!("{tune}: {verdict}"))
        .collect();
    println!(
        "{} tunes compared, {outlined} of them by outline alone",
        lines.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} tunes are not read back as written:\n\n{}",
        failures.len(),
        lines.len(),
        failures.join("\n\n")
    );
}
