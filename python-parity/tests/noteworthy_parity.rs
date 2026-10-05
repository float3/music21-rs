//! The crate's NoteWorthy reader against music21's `noteworthy.translate`.
//!
//! Each subject is read by music21 and by `from_noteworthy`, and the two
//! scores must be the same: first as an outline of every part, measure and
//! element, then as the MusicXML each side's exporter writes with
//! `makeNotation=False`. The subjects are the four `.nwctxt` files music21
//! carries beside its reader and a few lines written for the test.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::metadata::MetadataValue;
use music21_rs::musicxml::{ExportOptions, to_musicxml};
use music21_rs::noteworthy::from_noteworthy;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::{OUTLINE, STRIP_LAYOUT, first_difference, normalize_ids, outline};

/// music21's own files, under `music21/music21/noteworthy/`.
const FILES: &[&str] = &[
    "verySimple.nwctxt",
    "cuthbert_test1.nwctxt",
    "Part_OWeisheit.nwctxt",
    "NWCTEXT_Really_complete_example_file.nwctxt",
];

/// Lines written for the test: what music21's own files do not reach.
const WRITTEN: &[(&str, &str)] = &[
    (
        "every clef",
        "|AddStaff|Name:\"One\"\n|Clef|Type:Treble|OctaveShift:Octave Up\n|Note|Dur:4th|Pos:0\n\
         |Clef|Type:Bass|OctaveShift:Octave Down\n|Note|Dur:4th|Pos:0\n|Clef|Type:Bass|OctaveShift:Octave Up\n\
         |Note|Dur:4th|Pos:0\n|Clef|Type:Tenor\n|Note|Dur:4th|Pos:0\n|Bar\n|Clef|Type:Treble\n|Note|Dur:Whole|Pos:12\n",
    ),
    (
        "accidentals, a key, triplets and dots",
        "|AddStaff|\n|Key|Signature:Bb,Eb\n|TimeSig|Signature:Common\n|Note|Dur:4th,Dotted|Pos:#-2\n\
         |Note|Dur:8th|Pos:-2\n|Note|Dur:8th,Triplet=First|Pos:x0\n|Note|Dur:8th,Triplet|Pos:v1\n\
         |Note|Dur:8th,Triplet=End|Pos:n4\n|Note|Dur:4th|Pos:0\n|Bar\n|Note|Dur:Half,DblDotted|Pos:4\n\
         |Note|Dur:8th|Pos:4\n",
    ),
    (
        "slurs and ties across a barline",
        "|AddStaff|\n|TimeSig|Signature:2/4\n|Note|Dur:4th,Slur|Pos:1\n|Note|Dur:4th,Slur|Pos:2^\n|Bar\n\
         |Note|Dur:4th|Pos:2\n|Rest|Dur:4th\n|Bar|Style:SectionClose\n",
    ),
    (
        "repeats and endings",
        "|AddStaff|\n|TimeSig|Signature:AllaBreve\n|Bar|Style:MasterRepeatOpen\n|Note|Dur:Whole|Pos:1\n\
         |Bar\n|Ending|Endings:1\n|Note|Dur:Whole|Pos:2\n|Bar\n|Note|Dur:Whole|Pos:3\n\
         |Bar|Style:MasterRepeatClose\n|Ending|Endings:2\n|Note|Dur:Whole|Pos:4\n|Bar|Style:Double\n\
         |Tempo|Tempo:96\n|Note|Dur:Whole|Pos:5\n|Bar|Style:SectionOpen\n|Note|Dur:Whole|Pos:6\n",
    ),
    (
        "a percussion clef",
        "|AddStaff|
|Clef|Type:Percussion
|Note|Dur:4th|Pos:0
|Note|Dur:4th|Pos:#2^
         |Note|Dur:4th|Pos:2
|Rest|Dur:4th
",
    ),
    (
        "grace notes, and a chord beside a rest",
        "|AddStaff|
|TimeSig|Signature:3/4
|Note|Dur:8th,Grace|Pos:3
|Note|Dur:4th|Pos:1
         |Chord|Dur:4th|Pos:1,3|Dur2:8th
|Note|Dur:4th|Pos:2
|Note|Dur:4th|Pos:4
|Bar
         |Note|Dur:Half,Dotted|Pos:0
",
    ),
    (
        "two staves with lyrics, dynamics and words",
        "|SongInfo|Title:\"Two\"|Author:\"Someone\"\n|AddStaff|Name:\"Upper\"\n\
         |StaffInstrument|Patch:73|Trans:-2\n|Lyric1|Text:\"La-la la-\"\n|Dynamic|Style:mf|Pos:-8\n\
         |Note|Dur:4th|Pos:1\n|Note|Dur:4th|Pos:2\n|Text|Text:\"dolce\"|Pos:8\n|Note|Dur:Half|Pos:3\n\
         |AddStaff|Name:\"Lower\"\n|Clef|Type:Bass\n|Flow|Style:Segno|Pos:8\n|Chord|Dur:Whole|Pos:-1,1,3\n",
    ),
];

const MUSIC21: &str = r#"
from music21 import converter, stream
from music21.noteworthy import translate
from music21.musicxml import m21ToXml

def read_file(path):
    return converter.parse(path, forceSource=True)

def read_text(text):
    return translate.NoteworthyTranslator().parseString(text)

def written(score, strip_layout):
    exporter = m21ToXml.GeneralObjectExporter(strip_layout(score))
    exporter.makeNotation = False
    return exporter.parse().decode('utf-8')
"#;

#[test]
fn the_crate_reads_noteworthy_as_music21_does() {
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
        let music21 = module(MUSIC21, "noteworthy_parity_music21")?;
        let helpers = module(STRIP_LAYOUT, "noteworthy_parity_helpers")?;
        let outlines = module(OUTLINE, "noteworthy_parity_outline")?;
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

        let directory = root.join("music21").join("music21").join("noteworthy");
        let mut subjects: Vec<(String, String, Bound<'_, PyAny>)> = Vec::new();
        for file in FILES {
            let path = directory.join(file);
            let bytes = std::fs::read(&path).expect("music21's NoteWorthy file");
            let text = match String::from_utf8(bytes.clone()) {
                Ok(text) => text,
                Err(_) => bytes.iter().map(|byte| char::from(*byte)).collect(),
            };
            let theirs = music21
                .getattr("read_file")?
                .call1((path.to_string_lossy().to_string(),))?;
            subjects.push((file.to_string(), text, theirs));
        }
        for (label, text) in WRITTEN {
            let theirs = music21.getattr("read_text")?.call1((*text,))?;
            subjects.push((label.to_string(), text.to_string(), theirs));
        }

        let mut failures = Vec::new();
        let mut compared = 0;
        for (label, text, theirs) in subjects {
            let ours = match from_noteworthy(&text) {
                // music21 signs the metadata it makes for a song's title.
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
                .filter(|line| {
                    !["TextExpression(", "RepeatExpression("]
                        .iter()
                        .any(|mark| line.contains(mark))
                })
                .collect::<Vec<_>>()
                .join(
                    "
",
                );
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
                        let target = root.join("target").join("noteworthy-parity");
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
