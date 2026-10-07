//! The crate's ABC reader against music21's, tune by tune.
//!
//! Each tune is read twice: by music21, which then writes it out as MusicXML
//! with its own `ScoreExporter`, and by `music21_rs::abc::from_abc`, whose
//! score the crate's MusicXML writer writes out. The writer is held to
//! music21's by `musicxml_parity`, so the two documents are the same text
//! only if the two readers read the same score.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! A tune is named by its corpus file, with `#n` after it for the tune
//! numbered `n` of a file holding several. `ABC_PARITY_TUNES`, a
//! `;`-separated list of such names or `@` and the path of a file of them,
//! runs those instead of `musicxml_common`'s `ABC_TUNES`.

use music21_rs::abc::{from_abc, from_abc_number};
use music21_rs::metadata::MetadataValue;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{
    ABC_SOURCE_TEXT as SOURCE_TEXT, ABC_TUNES as TUNES, STRIP_LAYOUT, first_difference,
    normalize_ids,
};

#[test]
fn the_crate_reads_abc_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
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
        let sources = module(SOURCE_TEXT, "abc_source_text")?;
        let corpus = py.import("music21.corpus")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today = musicxml_common::pin_encoding_date(py)?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");

        let chosen: Vec<String> = match std::env::var("ABC_PARITY_TUNES") {
            Ok(list) => list
                .strip_prefix('@')
                .map(|path| std::fs::read_to_string(path).expect("the list of tunes"))
                .unwrap_or(list)
                .split([';', '\n'])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => TUNES.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let mut failures = Vec::new();
        for tune in &chosen {
            let (name, number) = match tune.split_once('#') {
                Some((name, number)) => (name, number.parse::<i32>().ok()),
                None => (tune.as_str(), None),
            };
            let text: String = sources.getattr("source_text")?.call1((name,))?.extract()?;
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("forceSource", true)?;
            if let Some(number) = number {
                kwargs.set_item("number", number)?;
            }
            let theirs = corpus
                .call_method("parse", (name,), Some(&kwargs))
                .and_then(|score| helpers.getattr("strip_layout")?.call1((score,)))
                .and_then(|score| {
                    let general = exporter
                        .getattr("GeneralObjectExporter")?
                        .call1((&score,))?;
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
                        "{tune}: music21 could not read or write it: {error}"
                    ));
                    continue;
                }
            };

            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                software: software.clone(),
                ..ExportOptions::default()
            };
            let read = match number {
                Some(number) => from_abc_number(&text, number),
                None => from_abc(&text),
            };
            // music21 signs every score it makes.
            let ours = read.and_then(|mut score| {
                let mut metadata = score.metadata().cloned().unwrap_or_default();
                let mut signed = vec![MetadataValue::new(software.clone())];
                signed.extend(metadata.get("software").iter().cloned());
                metadata.set("software", signed);
                score.set_metadata(Some(metadata));
                to_musicxml(&score, &options)
            });
            match ours {
                Ok(ours) => {
                    // The layout strip takes the weight off a tempo word on
                    // music21's side, where the crate's writer always
                    // writes it.
                    let ours = normalize_ids(&ours).replace(
                        "<words default-y=\"45\" font-weight=\"bold\">",
                        "<words default-y=\"45\">",
                    );
                    let theirs = normalize_ids(theirs.trim_end());
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        let directory = root.join("target").join("abc-read-parity");
                        let stem = tune.replace(['/', '#'], "_");
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
                        failures.push(format!("{tune}: {difference}"));
                    }
                }
                Err(error) => failures.push(format!("{tune}: the crate refused it: {error}")),
            }
        }
        Ok((failures, chosen.len()))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} tunes are read differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}
