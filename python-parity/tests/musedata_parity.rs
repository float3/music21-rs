//! The crate's MuseData reader against music21's `musedata`.
//!
//! Each subject is read by music21 and by `from_musedata`, and the two
//! scores must be the same: first as an outline of every part, measure and
//! element, then as the MusicXML each side's exporter writes with
//! `makeNotation=False`. The subjects are the work music21 carries beside
//! its reader -- a clarinet quintet movement in five files, read whole and
//! file by file -- and parts written for the test in both of MuseData's
//! stages: voices, chords, ties, shown and cautionary accidentals, beams,
//! articulations, dynamics, lyrics, repeats, a transposing part and a tempo
//! word, none of which music21's own file reaches all of. music21 has no
//! stage-1 file at all, and cannot write its own reading of one: it gives a
//! stage-1 part the id `None`, which reads back as a number -- the
//! object's address -- that it takes as the part's name too, and its
//! exporter refuses a number. So before writing, the test gives such a part
//! a string id, which is renumbered anyway, and no name.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::metadata::MetadataValue;
use music21_rs::musedata::from_musedata;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// A stage-2 note record: its pitch and divisions, then each later column
/// it fills, by its column counted from nought.
fn record(pitch: &str, divisions: u32, columns: &[(usize, &str)]) -> String {
    let mut line: Vec<char> = format!("{pitch:<5}{divisions:>3}").chars().collect();
    for (column, text) in columns {
        if line.len() < *column {
            line.resize(*column, ' ');
        }
        for (offset, c) in text.chars().enumerate() {
            let at = column + offset;
            if at < line.len() {
                line[at] = c;
            } else {
                line.push(c);
            }
        }
    }
    line.into_iter().collect()
}

/// A stage-2 part: the header lines music21 reads, the attribute record,
/// then the records given.
fn stage2(name: &str, attributes: &str, records: &[String]) -> String {
    let mut lines = vec![
        String::new(),
        String::new(),
        String::new(),
        "10/05/26 test".to_string(),
        "WK#:12  MV#:2b".to_string(),
        "Written for the test".to_string(),
        "Test Work".to_string(),
        "A movement".to_string(),
        name.to_string(),
        "1 0".to_string(),
        "Group memberships: score".to_string(),
        "score: part 1 of 1".to_string(),
        attributes.to_string(),
    ];
    lines.extend(records.iter().cloned());
    lines.push("/END".to_string());
    lines.join("\n")
}

fn subjects_written() -> Vec<(&'static str, String)> {
    let tie = |pitch: &str, divisions: u32| record(pitch, divisions, &[(8, "-")]);
    vec![
        (
            "a pickup, ties across a barline and a final barline",
            stage2(
                "Violin",
                "$  K:2   Q:2   T:3/4   C:4",
                &[
                    record("F#4", 2, &[(16, "q")]),
                    "measure 1".to_string(),
                    record("D5", 4, &[(16, "h")]),
                    tie("A4", 2),
                    "measure 2".to_string(),
                    record("A4", 6, &[(16, "h"), (17, ".")]),
                    "mheavy2".to_string(),
                ],
            ),
        ),
        (
            "chords, shown and cautionary accidentals, beams and notations",
            stage2(
                "Piano",
                "$  K:-1   Q:4   T:2/4   C:4   D:Allegro",
                &[
                    "measure 1".to_string(),
                    record("C4", 2, &[(16, "e"), (25, "["), (31, "."), (43, "La-")]),
                    record(" E4", 2, &[(16, "e")]),
                    record("Bf4", 2, &[(16, "e"), (25, "]"), (31, ">ff")]),
                    record("B4", 4, &[(16, "q"), (18, "n"), (31, "F"), (43, "la")]),
                    "measure 2".to_string(),
                    record("Bf4", 1, &[(16, "s"), (25, "[["), (31, "+")]),
                    record("C#5", 1, &[(16, "s"), (18, "#"), (25, "=="), (31, "t")]),
                    record("D5", 2, &[(16, "e"), (25, "]\\"), (31, "_Zp")]),
                    record("E5", 4, &[(16, "q"), (31, "A,mp")]),
                    "mdouble    :|".to_string(),
                ],
            ),
        ),
        (
            "two voices",
            stage2(
                "Organ",
                "$  K:0   Q:2   T:4/4   C:22",
                &[
                    "measure 1".to_string(),
                    record("C4", 8, &[(16, "w")]),
                    "back   8".to_string(),
                    record("E3", 4, &[(16, "h")]),
                    "rest   2".to_string(),
                    record("G3", 2, &[(16, "q")]),
                    "measure 2".to_string(),
                    record("D4", 8, &[(16, "w")]),
                    "mheavy3        |:".to_string(),
                    record("E4", 8, &[(16, "w")]),
                ],
            ),
        ),
        (
            "a transposing part",
            stage2(
                "Clarinet in Bb",
                "$  K:2   Q:1   T:2/4   C:4   X:-6",
                &[
                    "measure 1".to_string(),
                    record("D5", 1, &[(16, "q")]),
                    record("F#5", 1, &[(16, "q")]),
                    "measure 2".to_string(),
                    record("G#5", 1, &[(16, "q")]),
                    record("C#5", 1, &[(16, "q")]),
                ],
            ),
        ),
        (
            "stage 1",
            [
                "A Stage-One Work",
                "   12 3",
                "source one",
                "source two",
                "source three",
                "1 1",
                "A 0 4 0 2 4",
                "0 0 4",
                "measure 1",
                "C4    2",
                "E4    1",
                "G4    1-",
                "measure 2",
                "G4    2",
                "Bf3   2",
                "/END",
            ]
            .join("\n"),
        ),
        (
            "stage 1, low, in sharps",
            [
                "Low",
                "4,5   1",
                "s",
                "s",
                "s",
                "1 1",
                "A 3 3 0 3 8",
                "0 0 22",
                "measure 1",
                "F#2   1",
                "G#2   1",
                "A2    1",
                "measure 2",
                "C#3   3",
                "/END",
            ]
            .join("\n"),
        ),
    ]
}

const MUSIC21: &str = r#"
from music21 import converter
from music21.musicxml import m21ToXml

def read_path(path):
    return converter.parse(path, forceSource=True)

def read_text(text):
    return converter.parse(text, format='musedata', forceSource=True)

def written(score, strip_layout):
    # A stage-1 part is given the id None, which music21 then reads as the
    # number Python's id() gives, and takes as its name too; its exporter
    # cannot write a number. Ids are renumbered before comparing, so any
    # string will do, and the name an address gives is no name.
    for index, part in enumerate(score.parts):
        if not isinstance(part.id, str):
            part.id = f'P{index + 1}'
        if not isinstance(part.partName, str):
            part.partName = None
    exporter = m21ToXml.GeneralObjectExporter(strip_layout(score))
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')
"#;

/// A file's text as music21 decodes it: UTF-8, else latin-1.
fn text_of(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("music21's MuseData file");
    String::from_utf8(bytes.clone())
        .unwrap_or_else(|_| bytes.iter().map(|byte| char::from(*byte)).collect())
}

#[test]
fn the_crate_reads_musedata_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
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
        let music21 = module(MUSIC21, "musedata_parity_music21")?;
        let helpers = module(STRIP_LAYOUT, "musedata_parity_helpers")?;
        let outlines = module(OUTLINE, "musedata_parity_outline")?;
        let today: String = py
            .import("datetime")?
            .getattr("date")?
            .call_method0("today")?
            .str()?
            .extract()?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");
        let options = ExportOptions {
            encoding_date: Some(today),
            software: software.clone(),
            ..ExportOptions::default()
        };

        let directory = root
            .join("music21")
            .join("music21")
            .join("musedata")
            .join("testPrimitive")
            .join("test01");
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&directory)
            .expect("music21's MuseData work")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
            .collect();
        files.sort();

        let mut subjects: Vec<(
            String,
            music21_rs::Result<music21_rs::Stream>,
            Bound<'_, PyAny>,
        )> = Vec::new();
        let texts: Vec<String> = files.iter().map(|path| text_of(path)).collect();
        let all: Vec<&str> = texts.iter().map(String::as_str).collect();
        subjects.push((
            "the whole work".to_string(),
            from_musedata(&all),
            music21
                .getattr("read_path")?
                .call1((directory.to_string_lossy().to_string(),))?,
        ));
        for (path, text) in files.iter().zip(&texts) {
            subjects.push((
                path.file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                from_musedata(&[text]),
                music21
                    .getattr("read_path")?
                    .call1((path.to_string_lossy().to_string(),))?,
            ));
        }
        for (label, text) in subjects_written() {
            let theirs = match music21.getattr("read_text")?.call1((&text,)) {
                Ok(score) => score,
                Err(error) => {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "{label}: music21 could not read it: {error}"
                    )));
                }
            };
            subjects.push((label.to_string(), from_musedata(&[&text]), theirs));
        }

        let mut failures = Vec::new();
        let mut compared = 0;
        for (label, read, theirs) in subjects {
            let ours = match read {
                // music21 signs the metadata it makes for the work.
                Ok(mut score) => {
                    if let Some(mut metadata) = score.metadata().cloned() {
                        let mut signed = vec![MetadataValue::new(software.clone())];
                        signed.extend(metadata.get("software").iter().cloned());
                        metadata.set("software", signed);
                        score.set_metadata(Some(metadata));
                    }
                    score
                }
                Err(error) => {
                    failures.push(format!("{label}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let their_outline: String =
                outlines.getattr("outline")?.call1((&theirs,))?.extract()?;
            // The outline says nothing of marks it has no line for on
            // music21's side; the MusicXML below compares them.
            let our_outline: String = outline(&ours)
                .lines()
                .filter(|line| !line.contains("MetronomeMark("))
                .collect::<Vec<_>>()
                .join("\n");
            if let Some(difference) = first_difference(&our_outline, &their_outline) {
                failures.push(format!("{label}: read differently: {difference}"));
                continue;
            }
            let their_document: String = match music21
                .getattr("written")?
                .call1((&theirs, helpers.getattr("strip_layout")?))
            {
                Ok(document) => document.extract()?,
                Err(error) => {
                    failures.push(format!("{label}: music21 could not write it: {error}"));
                    continue;
                }
            };
            compared += 1;
            match to_musicxml(&ours, &options) {
                Ok(document) => {
                    if let Some(difference) = first_difference(
                        &normalize_ids(&document),
                        &normalize_ids(their_document.trim_end()),
                    ) {
                        let target = root.join("target").join("musedata-parity");
                        if std::fs::create_dir_all(&target).is_ok() {
                            let stem: String = label
                                .chars()
                                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                                .collect();
                            let _ = std::fs::write(
                                target.join(format!("{stem}.music21.xml")),
                                &their_document,
                            );
                            let _ = std::fs::write(
                                target.join(format!("{stem}.music21-rs.xml")),
                                &document,
                            );
                        }
                        failures.push(format!("{label}: as MusicXML: {difference}"));
                    }
                }
                Err(error) => failures.push(format!(
                    "{label}: the crate could not write it as MusicXML: {error}"
                )),
            }
        }
        Ok((failures, compared))
    })
    .expect("the Python side runs");

    println!("{compared} scores compared as MusicXML");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
