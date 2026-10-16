//! The crate's reader and writer of the lab's tabular analyses against
//! music21's, file by file.
//!
//! Each table is read twice, by music21's `romanText.tsvConverter` and by
//! `music21_rs::romantext::tsv::from_tsv`, and the two scores are compared
//! as an outline -- every measure and every chord in it, with its offset,
//! its length and its pitches -- and then as the MusicXML each side's
//! writer makes of its own. The crate's writer is held to music21's by
//! `musicxml_parity`, so the two documents are the same text only if the
//! two readers made the same score.
//!
//! Then each score is written back out as a table, by music21's `M21toTSV`
//! and by `to_tsv_with_extra`, and the two tables are compared as text --
//! every column of every row, music21's own `csv` writer against the
//! crate's. The cells music21 keeps on a numeral's editorial are handed to
//! the crate as what `tsv_to_chords` read them into, which is where a
//! chord of the crate's keeps nothing of the kind.
//!
//! The tables read are music21's own examples, which are the only ones
//! either side ships: one in the lab's first standard and three in its
//! second, among them a minor key and one with repeats. (A fourth of its
//! second, `tsvEg_v2_measures.tsv`, is a table of measures rather than of
//! chords -- `mn volta repeats` -- which music21 itself cannot read as
//! chords, so neither side is asked to.)
//!
//! What exercises the writer is every RomanText analysis of music21's
//! corpus, which neither side ships a table of: each is read by both
//! readers -- `romantext_parity` holds those two to each other -- and
//! written as a table by both writers, in both of the lab's standards. 140
//! tables agree: the four above and all 68 analyses twice over.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs::romantext::from_roman_text;
use music21_rs::romantext::tsv::{Version, from_tsv, to_tsv, to_tsv_with_extra, tsv_to_chords};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// The tables both readers read, the standard each is written in, and what
/// each exercises.
const FILES: &[(&str, u8, &str)] = &[
    ("tsvEg_v1.tsv", 1, "the lab's first standard, in C major"),
    ("tsvEg_v2major.tsv", 2, "its second, in a major key"),
    ("tsvEg_v2minor.tsv", 2, "its second, in a minor key"),
    ("tsvEg_v2_repeats.tsv", 2, "a table with repeats and voltas"),
];

/// The analyses neither side ships a table of: music21's corpus of
/// RomanText, read by each reader and written as a table by each writer.
const MUSIC21: &str = r#"
from music21 import converter, corpus
from music21.romanText import tsvConverter

def paths():
    return [str(path) for path in corpus.getPaths(fileExtensions=('rntxt',))]

def source(path):
    with open(path, encoding='utf-8') as handle:
        return handle.read()

def written(path, version, out):
    score = converter.parse(path, forceSource=True)
    tsvConverter.M21toTSV(score, dcml_version=version).write(out)
"#;

#[test]
fn the_crate_reads_the_labs_tables_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared, outline_only, written) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize, usize)> {
            init_py(py)?;
            add_dependency_venv(py, &root)?;
            let _ = py.import("music21")?;
            let helpers = PyModule::from_code(
                py,
                &std::ffi::CString::new(STRIP_LAYOUT).expect("no nul in the helper"),
                c"musicxml_parity_helpers.py",
                c"musicxml_parity_helpers",
            )?;
            let outlines = PyModule::from_code(
                py,
                &std::ffi::CString::new(OUTLINE).expect("no nul in the helper"),
                c"outline_helpers.py",
                c"outline_helpers",
            )?;
            let converter = py.import("music21.romanText.tsvConverter")?;
            let exporter = py.import("music21.musicxml.m21ToXml")?;
            let today = musicxml_common::pin_encoding_date(py)?;
            let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
            let software = format!("music21 v.{version}");

            let mut failures = Vec::new();
            let mut compared = 0;
            let mut outline_only = 0;
            let mut written = 0;
            for (name, standard, _) in FILES {
                let path = root
                    .join("music21")
                    .join("music21")
                    .join("romanText")
                    .join(name);
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("reading {name}: {error}"));
                let kind = Version::from_number(*standard).expect("a standard");

                let handler = converter
                    .getattr("TsvHandler")?
                    .call1((path.to_string_lossy().into_owned(), *standard))?;
                let read = match handler.call_method0("toM21Stream") {
                    Ok(score) => score,
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not read it: {error}"));
                        continue;
                    }
                };
                let ours = match from_tsv(&text, kind) {
                    Ok(score) => score,
                    Err(error) => {
                        failures.push(format!("{name}: the crate refused it: {error}"));
                        continue;
                    }
                };
                compared += 1;

                let their_outline: String =
                    outlines.getattr("outline")?.call1((&read,))?.extract()?;
                let our_outline = outline(&ours);
                if let Some(difference) = first_difference(&our_outline, &their_outline) {
                    write_both(&root, name, "txt", &their_outline, &our_outline);
                    failures.push(format!("{name}: outline {difference}"));
                    continue;
                }

                // The brackets over the endings, which the outline does
                // not show: music21 keeps a spanner per ending and the
                // crate says on each measure which ending it is in.
                // Through a list, since collecting one of music21's own
                // iterators asks it for a length it answers by walking
                // itself from the start -- which hands the first item back
                // twice.
                let brackets = listed(
                    py,
                    &read
                        .call_method0("recurse")?
                        .call_method1("getElementsByClass", ("RepeatBracket",))?,
                )?;
                let their_endings: Vec<Vec<String>> = brackets
                    .try_iter()?
                    .map(|bracket| -> PyResult<Vec<String>> {
                        bracket?
                            .call_method1("getSpannedElementsByClass", ("Measure",))?
                            .try_iter()?
                            .map(|measure| {
                                measure?
                                    .call_method0("measureNumberWithSuffix")?
                                    .extract::<String>()
                            })
                            .collect()
                    })
                    .collect::<PyResult<_>>()?;
                let our_endings = endings_of(&ours);
                if our_endings != their_endings {
                    failures.push(format!(
                        "{name}: the endings are {our_endings:?} where music21 has {their_endings:?}"
                    ));
                    continue;
                }

                // Each score written back out as a table. Both scores
                // are the same by now, so a difference here is the
                // writers'.
                let their_tsv = written_by_music21(py, &converter, &read, *standard, &root, name)?;
                let extra: Vec<Vec<(String, String)>> = tsv_to_chords(&text, kind)
                    .map(|rows| rows.into_iter().map(|row| row.extra).collect())
                    .unwrap_or_default();
                let our_tsv = match to_tsv_with_extra(&ours, kind, &extra) {
                    Ok(written) => written,
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not write it: {error}"));
                        continue;
                    }
                };
                if let Some(difference) = first_difference(&our_tsv, &their_tsv) {
                    write_both(&root, name, "tsv", &their_tsv, &our_tsv);
                    failures.push(format!("{name}: written back, {difference}"));
                    continue;
                }
                written += 1;

                let theirs = helpers
                    .getattr("strip_layout")?
                    .call1((read,))
                    .and_then(|score| {
                        let general = exporter.getattr("GeneralObjectExporter")?.call1((&score,))?;
                        general.setattr("makeNotation", false)?;
                        general
                            .call_method0("parse")?
                            .call_method1("decode", ("utf-8",))?
                            .extract::<String>()
                    });
                // What music21 cannot write as it stands has been compared
                // by its outline.
                let Ok(theirs) = theirs else {
                    outline_only += 1;
                    continue;
                };
                let options = ExportOptions {
                    encoding_date: Some(today.clone()),
                    software: software.clone(),
                    ..ExportOptions::default()
                };
                let ours = match to_musicxml(&ours, &options) {
                    Ok(text) => text,
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not write it: {error}"));
                        continue;
                    }
                };
                let (theirs, ours) = (normalize_ids(theirs.trim_end()), normalize_ids(&ours));
                if let Some(difference) = first_difference(&ours, &theirs) {
                    write_both(&root, name, "xml", &theirs, &ours);
                    failures.push(format!("{name}: {difference}"));
                }
            }
            // Every analysis of music21's corpus, written as a table by
            // both sides. `romantext_parity` holds the two readers of those
            // to each other, so a difference here is the writers'.
            let music21 = PyModule::from_code(
                py,
                &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
                c"tsv_parity_music21.py",
                c"tsv_parity_music21",
            )?;
            let paths: Vec<String> = music21.getattr("paths")?.call0()?.extract()?;
            for path in &paths {
                let text: String = music21.getattr("source")?.call1((path,))?.extract()?;
                let name = std::path::Path::new(path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.clone());
                let read = from_roman_text(&text);
                for standard in [1u8, 2] {
                    let kind = Version::from_number(standard).expect("a standard");
                    let directory = root.join("target").join("tsv-parity");
                    let _ = std::fs::create_dir_all(&directory);
                    let out = directory.join(format!("{name}.v{standard}.music21.tsv"));
                    let _ = std::fs::remove_file(&out);
                    let theirs = music21
                        .getattr("written")?
                        .call1((path, standard, out.to_string_lossy().into_owned()))
                        .map(|_| {
                            std::fs::read_to_string(&out).unwrap_or_else(|error| {
                                panic!("reading music21's table for {name}: {error}")
                            })
                        });
                    let ours = read
                        .as_ref()
                        .map_err(|error| error.to_string())
                        .and_then(|score| to_tsv(score, kind).map_err(|error| error.to_string()));
                    match (theirs, ours) {
                        (Ok(theirs), Ok(ours)) => {
                            if let Some(difference) = first_difference(&ours, &theirs) {
                                write_both(&root, &name, &format!("v{standard}.tsv"), &theirs, &ours);
                                failures.push(format!(
                                    "{name} as version {standard}: {difference}"
                                ));
                            } else {
                                written += 1;
                            }
                        }
                        // What one side cannot write the other must refuse
                        // too.
                        (Err(_), Err(_)) => {}
                        (Ok(_), Err(error)) => failures.push(format!(
                            "{name} as version {standard}: the crate could not write it: {error}"
                        )),
                        (Err(error), Ok(_)) => failures.push(format!(
                            "{name} as version {standard}: music21 could not write it: {error}"
                        )),
                    }
                }
            }

            Ok((failures, compared, outline_only, written))
        })
        .expect("the comparison runs");

    println!(
        "{compared} compared, {outline_only} of them by outline alone; {written} written back"
    );
    assert!(
        failures.is_empty(),
        "the two readers differ:\n  {}",
        failures.join("\n  ")
    );
}

/// What music21's own writer makes of a score, through its own `write`:
/// the file it appends to, removed first, since it appends.
fn written_by_music21(
    py: Python<'_>,
    converter: &Bound<'_, PyAny>,
    read: &Bound<'_, PyAny>,
    standard: u8,
    root: &std::path::Path,
    name: &str,
) -> PyResult<String> {
    let directory = root.join("target").join("tsv-parity");
    let _ = std::fs::create_dir_all(&directory);
    let path = directory.join(format!("{}.music21.written.tsv", name.replace('.', "_")));
    let _ = std::fs::remove_file(&path);
    let writer = converter.getattr("M21toTSV")?.call1((read, standard))?;
    writer.call_method1("write", (path.to_string_lossy().into_owned(),))?;
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading music21's table for {name}: {error}"));
    let _ = py;
    Ok(text)
}

/// A Python iterator as a list, so that nothing asks music21's own
/// iterators how long they are.
fn listed<'py>(py: Python<'py>, iterator: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    py.import("builtins")?.getattr("list")?.call1((iterator,))
}

/// The measures of each ending, in the order the brackets were made: what
/// music21 reads off a `RepeatBracket` and the crate off the measures
/// themselves.
fn endings_of(score: &music21_rs::Stream) -> Vec<Vec<String>> {
    let mut endings: Vec<(Vec<u32>, Vec<String>)> = Vec::new();
    for part in score.parts() {
        for measure in part.measures() {
            let Some(ending) = measure.ending() else {
                continue;
            };
            let name = format!(
                "{}{}",
                measure.number(),
                measure.number_suffix().unwrap_or_default()
            );
            match endings
                .iter_mut()
                .find(|(numbers, _)| numbers == ending.numbers())
            {
                Some((_, measures)) => measures.push(name),
                None => endings.push((ending.numbers().to_vec(), vec![name])),
            }
        }
    }
    endings.into_iter().map(|(_, measures)| measures).collect()
}

/// Both sides of a difference, left where a reader can look at them.
fn write_both(root: &std::path::Path, name: &str, kind: &str, theirs: &str, ours: &str) {
    let directory = root.join("target").join("tsv-parity");
    if std::fs::create_dir_all(&directory).is_ok() {
        let index = name.replace('.', "_");
        let _ = std::fs::write(directory.join(format!("{index}.music21.{kind}")), theirs);
        let _ = std::fs::write(directory.join(format!("{index}.music21-rs.{kind}")), ours);
    }
}
