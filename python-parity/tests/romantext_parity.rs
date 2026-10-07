//! The crate's RomanText reader against music21's, file by file.
//!
//! Each analysis is read twice, by music21 and by
//! `music21_rs::romantext::from_roman_text`, and the two scores are compared
//! as an outline -- every measure, and in it each chord with its notes, its
//! length, its ties and its lyric -- and then, where music21 can write the
//! score at all, as the MusicXML each side's writer makes of its own. The
//! crate's writer is held to music21's by `musicxml_parity`, so the two
//! texts are the same only if the two readers made the same chords of the
//! same figures, as long, tied and spelled alike.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! A file is named by its path in music21's corpus. `ROMANTEXT_PARITY_FILES`,
//! a `;`-separated list of such names or `@` and the path of a file of them,
//! runs those instead of the ones below.

use music21_rs::metadata::MetadataValue;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs::romantext::from_roman_text;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// Analyses both readers read alike, and what each exercises.
const FILES: &[(&str, &str)] = &[
    (
        "bach/choraleAnalyses/riemenschneider001.rntxt",
        "a chorale, with pivot chords",
    ),
    (
        "bach/choraleAnalyses/riemenschneider007.rntxt",
        "a numeral on a lowered degree",
    ),
    (
        "monteverdi/madrigal.3.1.rntxt",
        "a madrigal, with measures copied from earlier ones",
    ),
    (
        "monteverdi/madrigal.4.16.rntxt",
        "a run of measures copied into another key",
    ),
    (
        "monteverdi/madrigal.5.1.rntxt",
        "lengths no one note value has, and an eighth degree",
    ),
    (
        "monteverdi/madrigal.5.3.rntxt",
        "chords left out of their thirds",
    ),
];

#[test]
fn the_crate_reads_roman_text_as_music21_does() {
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
        let outlines = PyModule::from_code(
            py,
            &std::ffi::CString::new(OUTLINE).expect("no nul in the helper"),
            c"outline_helpers.py",
            c"outline_helpers",
        )?;
        let converter = py.import("music21.converter")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today = musicxml_common::pin_encoding_date(py)?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");

        let chosen: Vec<String> = match std::env::var("ROMANTEXT_PARITY_FILES") {
            Ok(list) => list
                .strip_prefix('@')
                .map(|path| std::fs::read_to_string(path).expect("the list of files"))
                .unwrap_or(list)
                .split([';', '\n'])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => FILES.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let mut failures = Vec::new();
        let mut outline_only = 0;
        for line in &chosen {
            let index = line.replace(['/', '.'], "_");
            let path = root
                .join("music21")
                .join("music21")
                .join("corpus")
                .join(line);
            let text = String::from_utf8_lossy(
                &std::fs::read(&path).unwrap_or_else(|error| panic!("reading {line}: {error}")),
            )
            .into_owned();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("format", "romantext")?;
            let read = match converter.call_method("parseData", (&text,), Some(&kwargs)) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{line}: music21 could not read it: {error}"));
                    continue;
                }
            };
            let ours = match from_roman_text(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{line}: the crate refused it: {error}"));
                    continue;
                }
            };
            // The outline first: music21 cannot write every analysis as it
            // stands, and an outline says plainly what differs.
            let their_outline: String = outlines.getattr("outline")?.call1((&read,))?.extract()?;
            let our_outline = outline(&ours);
            if let Some(difference) = first_difference(&our_outline, &their_outline) {
                let directory = root.join("target").join("romantext-parity");
                if std::fs::create_dir_all(&directory).is_ok() {
                    let _ = std::fs::write(
                        directory.join(format!("{index}.music21.txt")),
                        &their_outline,
                    );
                    let _ = std::fs::write(
                        directory.join(format!("{index}.music21-rs.txt")),
                        &our_outline,
                    );
                }
                failures.push(format!("{line}: outline {difference}"));
                continue;
            }
            let theirs = helpers
                .getattr("strip_layout")?
                .call1((read,))
                .and_then(|part| {
                    let general = exporter.getattr("GeneralObjectExporter")?.call1((&part,))?;
                    general.setattr("makeNotation", false)?;
                    general
                        .call_method0("parse")?
                        .call_method1("decode", ("utf-8",))?
                        .extract::<String>()
                });
            // What music21 cannot write as it stands has been compared.
            let Ok(theirs) = theirs else {
                outline_only += 1;
                continue;
            };
            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                software: software.clone(),
                ..ExportOptions::default()
            };
            // music21 signs every score it makes.
            let ours = {
                let mut score = ours;
                let mut metadata = score.metadata().cloned().unwrap_or_default();
                let mut signed = vec![MetadataValue::new(software.clone())];
                signed.extend(metadata.get("software").iter().cloned());
                metadata.set("software", signed);
                score.set_metadata(Some(metadata));
                to_musicxml(&score, &options)
            };
            match ours {
                Ok(ours) => {
                    let ours = normalize_ids(&ours);
                    let theirs = normalize_ids(theirs.trim_end());
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        let directory = root.join("target").join("romantext-parity");
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
        println!(
            "{} compared, {outline_only} of them by outline alone",
            chosen.len()
        );
        Ok((failures, chosen.len()))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} files are read differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}
