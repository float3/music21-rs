//! The crate's braille scores against music21's: `braille.translate`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Each is written
//! in braille by both, as music21's `objectToBraille` writes a score --
//! its metadata, each part segment by segment and a piano's two staves side
//! by side -- and must give the same braille, and the same English listing
//! of each segment's groupings as music21's `debug` writes, or both refuse.
//! `BRAILLE_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.

use music21_rs::braille::translate::{BrailleOptions, stream_to_braille};
use music21_rs::musicxml::from_musicxml;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus, environment
from music21.braille import translate

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

def report(text, debug):
    score = converter.parse(text, format='musicxml', forceSource=True)
    try:
        return (translate.objectToBraille(score, debug=debug), '')
    except Exception as e:
        return ('', f'{type(e).__name__}: {e}')
"#;

/// Scores music21 cannot write in braille in one mode, for a bug of its own,
/// which the crate writes: the score, the mode, what music21 raises and why.
const MUSIC21_CANNOT_WRITE: &[(&str, &str, &str, &str)] = &[(
    "demos/incorrect_time_signature_pv.mxl",
    "English",
    "StreamException",
    "makeNotation on the lower staff alone has makeTies insert a measure the      staff already holds; the braille, which writes the metadata first, stops      earlier on a character braille has no sign for, as the crate does",
)];

/// The first line two texts differ on, and what each has there.
fn first_difference(ours: &str, theirs: &str) -> String {
    let our_lines: Vec<&str> = ours.lines().collect();
    let their_lines: Vec<&str> = theirs.lines().collect();
    let at = our_lines
        .iter()
        .zip(&their_lines)
        .position(|(a, b)| a != b)
        .unwrap_or(our_lines.len().min(their_lines.len()));
    format!(
        "{} lines and {} in music21; first difference at line {at}:\n  music21    {:?}\n  music21-rs {:?}",
        our_lines.len(),
        their_lines.len(),
        their_lines.get(at),
        our_lines.get(at)
    )
}

#[test]
fn the_crate_writes_scores_in_braille_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"braille_parity_music21.py",
            c"braille_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("BRAILLE_PARITY_SCORES") {
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
        let mut compared = 0;
        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let score = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            for debug in [false, true] {
                let (theirs, their_error): (String, String) =
                    music21.getattr("report")?.call1((&text, debug))?.extract()?;
                let options = BrailleOptions {
                    debug,
                    ..BrailleOptions::default()
                };
                let label = if debug { "English" } else { "braille" };
                match stream_to_braille(&score, &options) {
                    Ok(ours) => {
                        let known = MUSIC21_CANNOT_WRITE.iter().any(|(score, mode, raises, _)| {
                            score == name && *mode == label && their_error.contains(raises)
                        });
                        if known {
                            continue;
                        }
                        if !their_error.is_empty() {
                            failures.push(format!(
                                "{name}: {label}: music21 refused ({their_error}) but the crate wrote it"
                            ));
                        } else if ours != theirs {
                            failures.push(format!(
                                "{name}: {label}: {}",
                                first_difference(&ours, &theirs)
                            ));
                        }
                    }
                    Err(error) => {
                        if their_error.is_empty() {
                            failures.push(format!(
                                "{name}: {label}: the crate refused ({error}) but music21 wrote it"
                            ));
                        }
                    }
                }
            }
        }
        Ok((failures, compared))
    })
    .expect("the Python side runs");

    println!("{compared} scores compared");
    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}
