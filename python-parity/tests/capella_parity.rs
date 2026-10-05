//! The crate's Capella reader against music21's `capella.fromCapellaXML`.
//!
//! Each subject is read by music21 and by `from_capella`, and the two scores
//! must be the same: first as an outline of every part, measure and element,
//! then as the MusicXML each side's exporter writes with
//! `makeNotation=False`. The subjects are the `.capx` music21 carries beside
//! its reader -- unpacked by Python's `zipfile` before the crate is given its
//! `score.xml`, since the crate unpacks nothing -- and documents written for
//! the test.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::capella::from_capella;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// A document of systems, each a list of staves, each staff its layout id
/// and its voices' note objects.
fn document(systems: &[&[(&str, &[&str])]]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <score xmlns=\"http://www.capella.de/CapXML/2.0\"><systems>",
    );
    for staves in systems {
        out.push_str("<system><staves>");
        for (layout, voices) in *staves {
            out.push_str(&format!("<staff layout=\"{layout}\"><voices>"));
            for objects in *voices {
                out.push_str(&format!(
                    "<voice><noteObjects>{objects}</noteObjects></voice>"
                ));
            }
            out.push_str("</voices></staff>");
        }
        out.push_str("</staves></system>");
    }
    out.push_str("</systems></score>");
    out
}

fn note(pitch: &str, base: &str) -> String {
    format!("<chord><duration base=\"{base}\"/><heads><head pitch=\"{pitch}\"/></heads></chord>")
}

fn written() -> Vec<(&'static str, String)> {
    let upper_one = [
        "<clefSign clef=\"treble\"/><keySign fifths=\"-2\"/><timeSign time=\"3/4\"/>".to_string(),
        note("B5", "1/2"),
        "<chord><duration base=\"1/4\"/><heads><head pitch=\"E5\"><alter step=\"-1\" \
         display=\"suppress\"/></head></heads><lyric><verse>La</verse><verse i=\"1\" \
         hyphen=\"true\">Lo</verse></lyric></chord>"
            .to_string(),
        "<barline type=\"repBegin\"/>".to_string(),
        "<chord><duration base=\"1/4\" dots=\"1\"/><heads><head pitch=\"F5\">\
         <tie begin=\"true\"/></head></heads></chord>"
            .to_string(),
        "<chord><duration base=\"1/8\"/><heads><head pitch=\"F5\"><tie end=\"true\"/>\
         </head></heads></chord>"
            .to_string(),
        "<rest><duration base=\"1/4\"/></rest>".to_string(),
        "<barline type=\"repEnd\"/>".to_string(),
    ]
    .concat();
    let lower_one = [
        "<clefSign clef=\"bass\"/><keySign fifths=\"-2\"/><timeSign time=\"3/4\"/>".to_string(),
        "<chord><duration base=\"1/2\" dots=\"1\"/><heads><head pitch=\"B3\"/>\
         <head pitch=\"F4\"/><head pitch=\"D5\"/></heads></chord>"
            .to_string(),
        "<chord><duration base=\"1/8\"><tuplet count=\"3\"/></duration><heads>\
         <head pitch=\"C4\"/></heads></chord>"
            .to_string(),
        "<chord><duration base=\"1/8\"><tuplet count=\"3\"/></duration><heads>\
         <head pitch=\"D4\"><alter step=\"1\"/></head></heads></chord>"
            .to_string(),
        "<chord><duration base=\"1/8\"><tuplet count=\"3\"/></duration><heads>\
         <head pitch=\"E4\"/></heads></chord>"
            .to_string(),
        "<rest><duration base=\"1/2\"/></rest>".to_string(),
    ]
    .concat();
    // The second system restates the clef and the key, changes the clef in
    // the lower staff, and ends with a double barline.
    let upper_two = [
        "<clefSign clef=\"treble\"/><keySign fifths=\"-2\"/>".to_string(),
        note("C6", "1/2"),
        note("D6", "1/4"),
        "<barline type=\"double\"/>".to_string(),
    ]
    .concat();
    let lower_two = [
        "<clefSign clef=\"C3\"/><keySign fifths=\"-2\"/>".to_string(),
        note("C5", "1/2"),
        note("D5", "1/4"),
        "<barline type=\"double\"/>".to_string(),
    ]
    .concat();
    let quintuplets = [
        "<clefSign clef=\"F4+\"/><timeSign time=\"2/4\"/>".to_string(),
        "<chord><duration base=\"1/16\"><tuplet count=\"5\"/></duration><heads>\
         <head pitch=\"G4\"/></heads></chord>"
            .repeat(5),
        note("A4", "1/4"),
        "<clefSign clef=\"G2-\"/><timeSign time=\"infinite\"/>".to_string(),
        note("B4", "1/2"),
    ]
    .concat();
    vec![
        (
            "two systems of two staves",
            document(&[
                &[("Upper", &[&upper_one]), ("Lower", &[&lower_one])],
                &[("Upper", &[&upper_two]), ("Lower", &[&lower_two])],
            ]),
        ),
        (
            "a staff of two voices, read as empty",
            document(&[&[("One", &[&upper_two]), ("Two", &[&upper_two, &lower_two])]]),
        ),
        (
            "quintuplets and other clefs",
            document(&[&[("S", &[&quintuplets])]]),
        ),
    ]
}

const MUSIC21: &str = r#"
import zipfile
from music21.capella import fromCapellaXML
from music21.musicxml import m21ToXml

def unpacked(path):
    with zipfile.ZipFile(path) as archive:
        return archive.read('score.xml').decode('utf-8')

def read(text):
    importer = fromCapellaXML.CapellaImporter()
    return importer.partScoreFromSystemScore(
        importer.systemScoreFromScore(importer.parseXMLText(text)))

def written(score, strip_layout):
    exporter = m21ToXml.GeneralObjectExporter(strip_layout(score))
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')
"#;

#[test]
fn the_crate_reads_capella_as_music21_does() {
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
        let music21 = module(MUSIC21, "capella_parity_music21")?;
        let helpers = module(STRIP_LAYOUT, "capella_parity_helpers")?;
        let outlines = module(OUTLINE, "capella_parity_outline")?;
        let today: String = py
            .import("datetime")?
            .getattr("date")?
            .call_method0("today")?
            .str()?
            .extract()?;
        let version: String = py.import("music21")?.getattr("__version__")?.extract()?;
        let options = ExportOptions {
            encoding_date: Some(today),
            software: format!("music21 v.{version}"),
            ..ExportOptions::default()
        };

        let sample = root
            .join("music21")
            .join("music21")
            .join("capella")
            .join("Nu_rue_mit_sorgen.capx");
        let mut subjects: Vec<(String, String)> = vec![(
            "Nu_rue_mit_sorgen.capx".to_string(),
            music21
                .getattr("unpacked")?
                .call1((sample.to_string_lossy().to_string(),))?
                .extract()?,
        )];
        subjects.extend(
            written()
                .into_iter()
                .map(|(label, text)| (label.to_string(), text)),
        );

        let mut failures = Vec::new();
        let mut compared = 0;
        for (label, text) in subjects {
            let theirs = match music21.getattr("read")?.call1((&text,)) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{label}: music21 could not read it: {error}"));
                    continue;
                }
            };
            let ours = match from_capella(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{label}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let their_outline: String =
                outlines.getattr("outline")?.call1((&theirs,))?.extract()?;
            // music21's outline has no line for a barline standing in a
            // measure; the MusicXML below passes over one too.
            let our_outline: String = outline(&ours)
                .lines()
                .filter(|line| !line.contains("Barline("))
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
                        let target = root.join("target").join("capella-parity");
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
