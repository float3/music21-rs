//! The crate's TinyNotation reader against music21's, line by line.
//!
//! Each line is read twice: by music21, which then writes it out as MusicXML
//! with its own exporter, and by
//! `music21_rs::tinynotation::from_tiny_notation`, whose part the crate's
//! MusicXML writer writes out. The writer is held to music21's by
//! `musicxml_parity`, so the two documents are the same text only if the two
//! readers read the same part and cut it into the same measures.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! `TINY_PARITY_LINES`, a `;`-separated list, runs those lines instead of
//! the ones below.

use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs::tinynotation::from_tiny_notation;
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{STRIP_LAYOUT, first_difference, normalize_ids};

/// Lines both readers read alike, and what each exercises.
const LINES: &[(&str, &str)] = &[
    (
        "4/4 c4 d8 e f#2 g-4. an16 b c'1",
        "lengths kept until the next, accidentals",
    ),
    (
        "3/4 E4 r f# g=lastG trip{b-8 a g} c'2.",
        "a rest, a name, a triplet",
    ),
    ("CC4 C c c' c'' GG#1", "octaves either side of middle C"),
    ("c2~ c~ c4 d e1~ e", "ties, one running on from another"),
    ("c4 d e f g a b c' d' e'", "no meter, so common time"),
    ("6/8 c8 d e f g a 3/4 b4 c' d' 2/4 e' f'", "meters changing"),
    ("4/4 c1 d0 e4", "a whole bar under a fermata"),
    ("3/4 c2. quad{d4 e f g} a2.", "four in the time of three"),
    ("4/4 c4_doe d_ray e_me r_shh", "lyrics"),
    (
        "2/4 trip{c8 d e} trip{f16 g a} b8 trip{c'4 r d'}",
        "triplets of several values, one with a rest",
    ),
    (
        "c4 hello d r4 3 e..",
        "what is no token, and dots with no number",
    ),
    ("4/4 c2 d1 e4 f", "a note running past its barline"),
    ("5/8 c8 d e f g a4. b4", "an uneven meter"),
    (
        "4/4 c(#)4 d(-) e# f--2",
        "editorial accidentals, which are not the note's",
    ),
    (
        "4/4 trip{c4 d e~} e2 f4 g",
        "a brace closing the tie before the triplet",
    ),
];

#[test]
fn the_crate_reads_tiny_notation_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let helpers = PyModule::from_code(
            py,
            &std::ffi::CString::new(STRIP_LAYOUT).expect("no nul in the helper"),
            c"musicxml_parity_helpers.py",
            c"musicxml_parity_helpers",
        )?;
        let converter = py.import("music21.converter")?;
        let stream = py.import("music21.stream")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today: String = py
            .import("datetime")?
            .getattr("date")?
            .call_method0("today")?
            .str()?
            .extract()?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");

        let chosen: Vec<String> = match std::env::var("TINY_PARITY_LINES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => LINES.iter().map(|(line, _)| line.to_string()).collect(),
        };
        let mut failures = Vec::new();
        for (index, line) in chosen.iter().enumerate() {
            let theirs = converter
                .call_method1("parse", (format!("tinyNotation: {line}"),))
                .and_then(|part| {
                    // The exporter writes only a score as it stands.
                    let score = stream.getattr("Score")?.call0()?;
                    score.call_method1("insert", (0, part))?;
                    helpers.getattr("strip_layout")?.call1((score,))
                })
                .and_then(|part| {
                    let general = exporter.getattr("GeneralObjectExporter")?.call1((&part,))?;
                    general.setattr("makeNotation", false)?;
                    general
                        .call_method0("parse")?
                        .call_method1("decode", ("utf-8",))?
                        .extract::<String>()
                });
            let theirs = match theirs {
                Ok(text) => text,
                Err(error) => {
                    failures.push(format!(
                        "{line}: music21 could not read or write it: {error}"
                    ));
                    continue;
                }
            };

            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                software: software.clone(),
                ..ExportOptions::default()
            };
            // music21's exporter puts a part on its own into a score.
            let ours = from_tiny_notation(line).and_then(|part| {
                let mut score = Stream::with_kind(StreamKind::Score);
                score.insert(0.0, part);
                to_musicxml(&score, &options)
            });
            match ours {
                Ok(ours) => {
                    let ours = normalize_ids(&ours);
                    let theirs = normalize_ids(theirs.trim_end());
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        let directory = root.join("target").join("tinynotation-parity");
                        if std::fs::create_dir_all(&directory).is_ok() {
                            let _ = std::fs::write(
                                directory.join(format!("{index}.music21.xml")),
                                &theirs,
                            );
                            let _ = std::fs::write(
                                directory.join(format!("{index}.music21-rs.xml")),
                                &ours,
                            );
                        }
                        failures.push(format!("{line}: {difference}"));
                    }
                }
                Err(error) => failures.push(format!("{line}: the crate refused it: {error}")),
            }
        }
        Ok((failures, chosen.len()))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} lines are read differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}
