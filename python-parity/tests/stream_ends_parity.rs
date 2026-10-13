//! The crate's `final_barline` and `staff_lines` against music21's
//! `Stream.finalBarline` and `staffLines`.
//!
//! Each corpus score is read by music21 and by `from_musicxml` (which
//! `musicxml_read_parity` holds to music21's reader). The score is asked
//! for the barline each of its parts ends with, and the score, each part
//! and every measure are asked how many lines their staff has. Then every
//! part is given a new final barline and asked again, which is music21's
//! setter reaching the last measure of each.
//!
//! `STREAM_ENDS_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::bar::{Barline, BarlineType};
use music21_rs::musicxml::from_musicxml;
use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import bar, stream

def source_text(name):
    from music21 import corpus
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

def score_of(text):
    from music21 import converter
    return converter.parse(text, format='musicxml', forceSource=True)

def named(barline):
    return 'none' if barline is None else barline.type

def final_barlines(score):
    """The barline each part ends with, as the score answers for them."""
    answer = score.finalBarline
    if not isinstance(answer, list):
        answer = [answer]
    return [named(b) for b in answer]

def staff_lines(score):
    """The score's, then each part's, then every measure's."""
    out = [score.staffLines]
    for p in score.parts:
        out.append(p.staffLines)
        for m in p.getElementsByClass(stream.Measure):
            out.append(m.staffLines)
    return out

def set_final_barline(score, name):
    score.finalBarline = name
    return final_barlines(score)
"#;

/// How the crate names a barline, as the Python side names it.
fn named(barline: Option<&Barline>) -> String {
    barline.map_or_else(
        || "none".to_string(),
        |barline| barline.bar_type().as_str().to_string(),
    )
}

/// The crate's answer for a score, each part and every measure.
fn staff_lines_of(score: &Stream) -> Vec<i64> {
    let mut out = vec![score.staff_lines()];
    for part in score.parts() {
        out.push(part.staff_lines());
        out.extend(part.measures().into_iter().map(Stream::staff_lines));
    }
    out
}

#[test]
fn the_crate_reads_the_ends_of_a_stream_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"stream_ends_parity_music21.py",
            c"stream_ends_parity_music21",
        )?;

        let chosen: Vec<String> = match std::env::var("STREAM_ENDS_PARITY_SCORES") {
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
        for name in &chosen {
            let Some(text): Option<String> =
                music21.getattr("source_text")?.call1((name,))?.extract()?
            else {
                continue;
            };
            let mut ours = match from_musicxml(&text) {
                Ok(score) => score,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            if ours.kind() != StreamKind::Score {
                continue;
            }
            let theirs = music21.getattr("score_of")?.call1((&text,))?;

            let their_barlines: Vec<String> = music21
                .getattr("final_barlines")?
                .call1((&theirs,))?
                .extract()?;
            let our_barlines: Vec<String> =
                ours.final_barlines().into_iter().map(named).collect();
            compared += 1;
            if our_barlines != their_barlines {
                failures.push(format!(
                    "{name}: the final barlines are {their_barlines:?} to music21, {our_barlines:?} to the crate"
                ));
            }

            let their_lines: Vec<i64> = music21
                .getattr("staff_lines")?
                .call1((&theirs,))?
                .extract()?;
            let our_lines = staff_lines_of(&ours);
            compared += 1;
            if our_lines != their_lines {
                failures.push(format!(
                    "{name}: the staff lines are {their_lines:?} to music21, {our_lines:?} to the crate"
                ));
            }

            // Writing one barline reaches the last measure of every part.
            let their_written: Vec<String> = music21
                .getattr("set_final_barline")?
                .call1((&theirs, "dotted"))?
                .extract()?;
            ours.set_final_barline(Some(Barline::new(BarlineType::Dotted)));
            let our_written: Vec<String> =
                ours.final_barlines().into_iter().map(named).collect();
            compared += 1;
            if our_written != their_written {
                failures.push(format!(
                    "{name}: after writing, the final barlines are {their_written:?} to music21, {our_written:?} to the crate"
                ));
            }
        }
        Ok((failures, compared))
    })
    .unwrap_or_else(|error| panic!("asking music21 for the ends of a stream: {error}"));

    let shown: Vec<&str> = failures.iter().take(10).map(String::as_str).collect();
    assert!(
        failures.is_empty(),
        "{} of {compared} answers differ, first {}:\n{}",
        failures.len(),
        shown.len(),
        shown.join("\n")
    );
    assert!(compared > 0, "nothing was compared");
    println!("{compared} answers compared");
}
