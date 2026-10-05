//! The crate's RomanText writer against music21's `romanText.writeRoman`.
//!
//! Each analysis is read by both sides, music21's reading written by
//! music21's `RnWriter` and the crate's by `to_roman_text`, and the two
//! texts must be the same. `romantext_parity` holds the two readers to each
//! other, so this holds the writers. The subjects are every RomanText file
//! of music21's corpus, and analyses written for the test: repeats, a meter
//! changing, a chord held across a barline, a pickup and a key change.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::romantext::{from_roman_text, to_roman_text};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::first_difference;

const WRITTEN: &[(&str, &str)] = &[
    (
        "repeats and a meter changing",
        "Composer: Someone\nTitle: A Piece\nAnalyst: Me\nProofreader: You\n\
         Time Signature: 4/4\nm1 C: I b3 V\nm2 ||: I b2 IV b3 V7 b4 I\nm3 vi b3 ii6 :||\n\
         Time Signature: 3/4\nm4 V b2.5 V7 b3 I\nm5 I\n",
    ),
    (
        "a pickup, a chord held across the barline, and a key change",
        "Title: Held\nMovement: 2\nTime Signature: 6/8\nm0 b2 d: i\nm1 V b2 i6\n\
         m2 iv\nm3 b2 F: V7\nm4 I\n",
    ),
    (
        "beats that are not halves",
        "Time Signature: 3/4\nm1 G: I b1.33 V b1.67 I b2 IV b3 V\nm2 I\n",
    ),
];

const MUSIC21: &str = r#"
from music21 import converter, corpus
from music21.romanText import writeRoman

def paths():
    return [str(path) for path in corpus.getPaths(fileExtensions=('rntxt',))]

def source(path):
    with open(path, encoding='utf-8') as handle:
        return handle.read()

def written_file(path):
    score = converter.parse(path, forceSource=True)
    return ''.join(line + '\n' for line in writeRoman.RnWriter(score).combinedList)

def written_text(text):
    score = converter.parse(text, format='romantext', forceSource=True)
    return ''.join(line + '\n' for line in writeRoman.RnWriter(score).combinedList)
"#;

#[test]
fn the_crate_writes_roman_text_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"romantext_write_parity_music21.py",
            c"romantext_write_parity_music21",
        )?;
        let mut subjects: Vec<(String, String, PyResult<String>)> = Vec::new();
        let paths: Vec<String> = music21.getattr("paths")?.call0()?.extract()?;
        for path in &paths {
            let text: String = music21.getattr("source")?.call1((path,))?.extract()?;
            let theirs = music21
                .getattr("written_file")?
                .call1((path,))
                .and_then(|written| written.extract());
            let label = path
                .rsplit(['/', '\\'])
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("/");
            subjects.push((label, text, theirs));
        }
        for (label, text) in WRITTEN {
            let theirs = music21
                .getattr("written_text")?
                .call1((*text,))
                .and_then(|written| written.extract());
            subjects.push((label.to_string(), text.to_string(), theirs));
        }

        let mut failures = Vec::new();
        let count = subjects.len();
        for (label, text, theirs) in subjects {
            let theirs = match theirs {
                Ok(text) => text,
                Err(error) => {
                    failures.push(format!("{label}: music21 could not write it: {error}"));
                    continue;
                }
            };
            match from_roman_text(&text).and_then(|score| to_roman_text(&score)) {
                Ok(ours) => {
                    if let Some(difference) = first_difference(&ours, &theirs) {
                        failures.push(format!("{label}: {difference}"));
                    }
                }
                Err(error) => {
                    failures.push(format!("{label}: the crate could not write it: {error}"));
                }
            }
        }
        Ok((failures, count))
    })
    .expect("the Python side runs");

    println!("{count} analyses written");
    assert!(
        failures.is_empty(),
        "{} of {count} written differently:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
