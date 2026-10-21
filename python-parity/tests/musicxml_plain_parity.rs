//! The crate's MusicXML writer over a score read as *music21's own
//! objects*, with nothing of this package installed over music21.
//!
//! `musicxml_parity` beside it is the same comparison with the classes
//! installed, where every element of the score is one of this package's and
//! the wheel reads it by casting to its own class. This one installs
//! nothing, so the score holds music21's own notes, rests, chords, clefs,
//! keys, meters and the rest, and the wheel has to read each off what
//! music21 keeps on it (`stream::music21_value` and the readers beside it).
//!
//! **It is the configuration a caller gets from `pip install music21
//! music21-rs`**, and before this test it did not work at all: the note
//! fell through to `a Note cannot be read into a score yet`. Holding it to
//! music21's own exporter, byte for byte, is what says the by-attribute
//! readers are as good as the casts -- and the same comparison on the same
//! scores, so a gap in one is a gap in the other.
//!
//! `MUSICXML_PLAIN_PARITY_SCORES`, a `;`-separated list of corpus names,
//! runs those scores instead.

use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_rs_facade;
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{SCORES, STRIP_LAYOUT, first_difference, normalize_ids};

#[test]
fn the_crate_writes_a_music21_score_as_music21_does() {
    pyo3::append_to_inittab!(music21_rs_facade);
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        // Nothing installed, on purpose: that is the whole of what this
        // test is for, and installing here would make it `musicxml_parity`
        // over again.
        let helpers = PyModule::from_code(
            py,
            &std::ffi::CString::new(STRIP_LAYOUT).expect("no nul in the helper"),
            c"musicxml_plain_helpers.py",
            c"musicxml_plain_helpers",
        )?;
        let corpus = py.import("music21.corpus")?;
        let exporter = py.import("music21.musicxml.m21ToXml")?;
        let today = musicxml_common::pin_encoding_date(py)?;

        let chosen: Vec<String> = match std::env::var("MUSICXML_PLAIN_PARITY_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let software = format!("music21 v.{version}");
        let mut failures = Vec::new();
        for name in &chosen {
            let name = name.as_str();
            let kwargs = pyo3::types::PyDict::new(py);
            kwargs.set_item("forceSource", true)?;
            let parsed = if name.starts_with("built:") {
                helpers.getattr("build")?.call1((name,))
            } else {
                corpus.call_method("parse", (name,), Some(&kwargs))
            };
            let score = match parsed {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not parse it: {error}"));
                    continue;
                }
            };
            let score = helpers.getattr("strip_layout")?.call1((score, true))?;

            // Read before music21 writes: its exporter changes the score as
            // it goes, sounding pitch to written and ids to fresh ones.
            let ours_stream = match music21_rs_python::stream::crate_stream(&score) {
                Ok(stream) => stream,
                Err(error) => {
                    failures.push(format!("{name}: the wheel could not read it: {error}"));
                    continue;
                }
            };
            let general = exporter
                .getattr("GeneralObjectExporter")?
                .call1((&score,))?;
            general.setattr("makeNotation", false)?;
            let theirs: String = match general
                .call_method0("parse")
                .and_then(|written| written.call_method1("decode", ("utf-8",)))
                .and_then(|text| text.extract())
            {
                Ok(text) => {
                    failures.extend(musicxml_common::music21_writes_after_all(name));
                    text
                }
                Err(error) => {
                    if musicxml_common::music21_cannot_write(name, &error.to_string()) {
                        continue;
                    }
                    failures.push(format!("{name}: music21 could not write it: {error}"));

                    continue;
                }
            };

            let options = ExportOptions {
                encoding_date: Some(today.clone()),
                // What writes a score with no metadata of its own names
                // itself, and the two writers have different names.
                software: software.clone(),
                ..ExportOptions::default()
            };
            match to_musicxml(&ours_stream, &options) {
                Ok(ours) => {
                    let ours = normalize_ids(&ours);
                    let theirs = normalize_ids(theirs.trim_end());
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        // Both documents, whole, for a diff tool.
                        let directory = root.join("target").join("musicxml-plain-parity");
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
        Ok((failures, chosen.len()))
    })
    .unwrap_or_else(|error| panic!("running music21: {error}"));

    assert!(
        failures.is_empty(),
        "{} of {} scores are written differently:\n\n{}",
        failures.len(),
        count,
        failures.join("\n\n")
    );
}
