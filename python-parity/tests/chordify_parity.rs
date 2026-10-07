//! The crate's `chordify` against music21's.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader), chordified on each
//! side, put in a score of its own -- music21 writes nothing else with
//! `makeNotation=False` -- and compared, first as an outline of every
//! measure and chord, then as the MusicXML each side writes.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//! `CHORDIFY_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::musicxml::{ExportOptions, from_musicxml, to_musicxml};
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, SCORES, STRIP_LAYOUT, first_difference, normalize_ids, outline};

const MUSIC21: &str = r#"
import zipfile
from music21 import corpus, stream
from music21.musicxml import m21ToXml

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

def chordified(name):
    score = stream.Score()
    score.insert(0, corpus.parse(name, forceSource=True).chordify())
    return score

def written(score, strip_layout):
    exporter = m21ToXml.GeneralObjectExporter(strip_layout(score))
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')
"#;

#[test]
fn the_crate_chordifies_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared, outlined) =
        Python::attach(|py| -> PyResult<(Vec<String>, usize, usize)> {
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
            let music21 = module(MUSIC21, "chordify_parity_music21")?;
            let helpers = module(STRIP_LAYOUT, "chordify_parity_helpers")?;
            let outlines = module(OUTLINE, "chordify_parity_outline")?;
            let today = musicxml_common::pin_encoding_date(py)?;
            let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
            let options = ExportOptions {
                encoding_date: Some(today),
                software: format!("music21 v.{version}"),
                ..ExportOptions::default()
            };
            let chosen: Vec<String> = match std::env::var("CHORDIFY_PARITY_SCORES") {
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

            let mut failures = Vec::new();
            let mut compared = 0;
            let mut outlined = 0;
            for name in &chosen {
                let Some(text): Option<String> =
                    music21.getattr("source_text")?.call1((name,))?.extract()?
                else {
                    continue;
                };
                let theirs = match music21.getattr("chordified")?.call1((name,)) {
                    Ok(score) => score,
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not chordify it: {error}"));
                        continue;
                    }
                };
                let ours = match from_musicxml(&text).and_then(|score| score.chordify()) {
                    Ok(part) => {
                        let mut score = Stream::with_kind(StreamKind::Score);
                        score.insert(0.0, part);
                        score
                    }
                    Err(error) => {
                        failures.push(format!("{name}: the crate could not chordify it: {error}"));
                        continue;
                    }
                };
                // The outline has no line for a tempo mark that says no
                // number; such a score is compared as MusicXML alone.
                if let Ok(their_outline) = outlines
                    .getattr("outline")?
                    .call1((&theirs,))
                    .and_then(|text| text.extract::<String>())
                {
                    let our_outline: String = outline(&ours)
                        .lines()
                        .filter(|line| !line.contains('('))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Some(difference) = first_difference(&our_outline, &their_outline) {
                        failures.push(format!("{name}: chordified differently: {difference}"));
                        continue;
                    }
                }
                let their_document: String = match music21
                    .getattr("written")?
                    .call1((&theirs, helpers.getattr("strip_layout")?))
                {
                    Ok(document) => document.extract()?,
                    // music21 cannot write every length a chord may last;
                    // the outline above is the comparison then.
                    Err(_) => {
                        outlined += 1;
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
                            let target = root.join("target").join("chordify-parity");
                            if std::fs::create_dir_all(&target).is_ok() {
                                let stem = name.replace(['/', '\\'], "_");
                                let _ = std::fs::write(
                                    target.join(format!("{stem}.music21.xml")),
                                    &their_document,
                                );
                                let _ = std::fs::write(
                                    target.join(format!("{stem}.music21-rs.xml")),
                                    &document,
                                );
                            }
                            failures.push(format!("{name}: as MusicXML: {difference}"));
                        }
                    }
                    Err(error) => failures.push(format!(
                        "{name}: the crate could not write it as MusicXML: {error}"
                    )),
                }
            }
            Ok((failures, compared, outlined))
        })
        .expect("the Python side runs");

    println!("{compared} compared as MusicXML, {outlined} by outline alone");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
