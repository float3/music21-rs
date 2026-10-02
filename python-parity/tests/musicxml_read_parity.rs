//! The crate's MusicXML reader against music21's, score by score.
//!
//! Each score's file is read twice: by music21, which then writes it out
//! again with its own `ScoreExporter`, and by
//! `music21_rs::musicxml::from_musicxml`, whose score the crate's writer
//! writes out. The writer is held to music21's by `musicxml_parity`, so the
//! two documents are the same text only if the two readers read the same
//! score. A score the test builds itself, for want of a corpus score holding
//! what it holds, is read from the document music21 writes of it.
//!
//! `MUSICXML_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those scores instead of the ones this test knows.

use music21_rs::metadata::MetadataValue;
use music21_rs::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_rs_facade;
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{SCORES, STRIP_LAYOUT, first_difference, normalize_ids};

/// Scores of the shared list the reader does not read as music21 does yet,
/// and what is in the way.
const NOT_YET: &[(&str, &str)] = &[];

/// The text of a corpus score's file, a compressed one unpacked.
const SOURCE_TEXT: &str = r#"
import os
import zipfile
from music21 import corpus

def source_text(name):
    """The MusicXML a corpus score is kept as, or None for another format."""
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
    file_name = os.path.split(path)[1]
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding), file_name
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1'), file_name
"#;

#[test]
fn the_crate_reads_musicxml_as_music21_does() {
    pyo3::append_to_inittab!(music21_rs_facade);
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        music21_rs_python::install_into_music21(py)?;
        let module = |code: &str, name: &str| -> PyResult<Bound<'_, PyModule>> {
            PyModule::from_code(
                py,
                &std::ffi::CString::new(code).expect("no nul in the helper"),
                &std::ffi::CString::new(format!("{name}.py")).expect("no nul in the name"),
                &std::ffi::CString::new(name).expect("no nul in the name"),
            )
        };
        let helpers = module(STRIP_LAYOUT, "musicxml_parity_helpers")?;
        let sources = module(SOURCE_TEXT, "musicxml_source_text")?;
        let corpus = py.import("music21.corpus")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today: String = py
            .import("datetime")?
            .getattr("date")?
            .call_method0("today")?
            .str()?
            .extract()?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");

        let chosen: Vec<String> = match std::env::var("MUSICXML_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES
                .iter()
                .map(|(name, _)| name.to_string())
                .filter(|name| !NOT_YET.iter().any(|(held, _)| held == name))
                .collect(),
        };
        let mut failures = Vec::new();
        let mut count = 0;
        for name in &chosen {
            let name = name.as_str();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("forceSource", true)?;
            let (text, file_name, score) = if name.starts_with("built:") {
                // A score the test builds is read from the document music21
                // writes of it, kept as a file so that music21's converter
                // reads it as it reads a corpus score.
                let directory = root.join("target").join("musicxml-read-parity");
                let (path, text): (String, String) = helpers
                    .getattr("built_source")?
                    .call1((name, directory.to_string_lossy().into_owned()))?
                    .extract()?;
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let score =
                    py.import("music21.converter")?
                        .call_method("parse", (path,), Some(&kwargs))?;
                (text, file_name, score)
            } else {
                let source: Option<(String, String)> =
                    sources.getattr("source_text")?.call1((name,))?.extract()?;
                let Some((text, file_name)) = source else {
                    // Kept in another format, which the reader is not for.
                    continue;
                };
                let score = corpus.call_method("parse", (name,), Some(&kwargs))?;
                (text, file_name, score)
            };
            count += 1;
            let score = helpers.getattr("strip_layout")?.call1((score,))?;
            let general = exporter
                .getattr("GeneralObjectExporter")?
                .call1((&score,))?;
            general.setattr("makeNotation", false)?;
            let theirs: String = match general
                .call_method0("parse")
                .and_then(|written| written.call_method1("decode", ("utf-8",)))
                .and_then(|text| text.extract())
            {
                Ok(text) => text,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not write it: {error}"));
                    continue;
                }
            };

            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                software: software.clone(),
                ..ExportOptions::default()
            };
            // music21's converter, not its reader, signs what it reads and
            // calls a score with no title after its file.
            let ours = from_musicxml(&text).and_then(|mut score| {
                let mut metadata = score.metadata().cloned().unwrap_or_default();
                let mut signed = vec![MetadataValue::new(software.clone())];
                signed.extend(metadata.get("software").iter().cloned());
                metadata.set("software", signed);
                if metadata.get("movementName").is_empty() {
                    metadata.add_text("movementName", file_name.clone());
                }
                score.set_metadata(Some(metadata));
                to_musicxml(&score, &options)
            });
            match ours {
                Ok(ours) => {
                    let ours = normalize_ids(&ours);
                    let theirs = normalize_ids(theirs.trim_end());
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        let directory = root.join("target").join("musicxml-read-parity");
                        let stem = name.replace('/', "_");
                        if std::fs::create_dir_all(&directory).is_ok() {
                            let _ = std::fs::write(
                                directory.join(format!("{stem}.music21.xml")),
                                &theirs,
                            );
                            let _ = std::fs::write(
                                directory.join(format!("{stem}.music21-rs.xml")),
                                &ours,
                            );
                        }
                        failures.push(format!("{name}: {difference}"));
                    }
                }
                Err(error) => failures.push(format!("{name}: the crate refused it: {error}")),
            }
        }
        Ok((failures, count))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} scores are read differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}
