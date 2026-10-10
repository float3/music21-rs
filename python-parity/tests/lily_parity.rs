//! The crate's LilyPond against music21's: `lily.translate`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader). Each is written
//! as LilyPond by both, as music21's `LilypondConverter` writes a score with
//! `textFromMusic21Object`, and must give the same text, space for space,
//! or both refuse. music21 asks the LilyPond it finds installed for the
//! version it writes; here it is told 2.24, the crate's default.
//! `LILY_PARITY_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the writer test's scores, which is how the corpus is swept.

use music21_rs::lily::{LilyOptions, to_lilypond};
use music21_rs::musicxml::from_musicxml;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus
from music21.lily import lilyObjects as lyo
from music21.lily import translate

def setupTools(self):
    self.majorVersion = '2'
    self.minorVersion = '24'
    self.versionString = (self.topLevelObject.backslash + 'version '
                          + self.topLevelObject.quoteString('2.24'))
    self.versionScheme = lyo.LyEmbeddedScm(self.versionString)
    self.headerScheme = lyo.LyEmbeddedScm(self.bookHeader)
    self.backend = 'ps'
    self.backendString = '-dbackend='

translate.LilypondConverter.setupTools = setupTools

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

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    try:
        return (translate.LilypondConverter().textFromMusic21Object(score), '')
    except Exception as e:
        return ('', f'{type(e).__name__}: {e}')
"#;

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
fn the_crate_writes_scores_as_lilypond_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"lily_parity_music21.py",
            c"lily_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("LILY_PARITY_SCORES") {
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
            let (theirs, their_error): (String, String) =
                music21.getattr("report")?.call1((&text,))?.extract()?;
            let written = to_lilypond(&score, &LilyOptions::default());
            if let Ok(directory) = std::env::var("LILY_PARITY_DUMP") {
                let stem = name.replace(['/', '\\', ' '], "_");
                let ours = written
                    .as_ref()
                    .map_or_else(ToString::to_string, Clone::clone);
                let theirs = if their_error.is_empty() {
                    &theirs
                } else {
                    &their_error
                };
                std::fs::write(format!("{directory}/{stem}.rs.ly"), ours).expect("dump ours");
                std::fs::write(format!("{directory}/{stem}.m21.ly"), theirs).expect("dump theirs");
            }
            match written {
                Ok(ours) => {
                    if !their_error.is_empty() {
                        failures.push(format!(
                            "{name}: music21 refused ({their_error}) but the crate wrote it"
                        ));
                    } else if ours != theirs {
                        failures.push(format!("{name}: {}", first_difference(&ours, &theirs)));
                    }
                }
                Err(error) => {
                    if their_error.is_empty() {
                        failures.push(format!(
                            "{name}: the crate refused ({error}) but music21 wrote it"
                        ));
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
