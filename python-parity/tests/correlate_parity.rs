//! The crate's pitch-to-dynamic correlation against music21's:
//! `analysis.correlate.ActivityMatch.pitchToDynamic`.
//!
//! The text of each corpus score is read by music21 and by `from_musicxml`
//! (which `musicxml_read_parity` holds to music21's reader), and the score
//! and each of its parts are correlated, every pair and the counted pairs:
//! the two must refuse the same streams and pair the rest alike.
//! `CORRELATE_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::FloatType;
use music21_rs::analysis::correlate::{pitch_to_dynamic, pitch_to_dynamic_counts};
use music21_rs::musicxml::from_musicxml;
use music21_rs::stream::Stream;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus
from music21.analysis import correlate

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

def correlate_one(stream):
    try:
        points = [(float(x), y) for x, y in correlate.ActivityMatch(stream).pitchToDynamic()]
        counts = [(float(x), y, n) for x, y, n in
                  correlate.ActivityMatch(stream).pitchToDynamic(dataPoints=False)]
        return points, counts, ''
    except Exception as error:
        return [], [], type(error).__name__

def report(text):
    score = converter.parse(text, format='musicxml', forceSource=True)
    return [correlate_one(score)] + [correlate_one(part) for part in score.parts]
"#;

type Correlation = (
    Vec<(FloatType, usize)>,
    Vec<(FloatType, usize, usize)>,
    String,
);

fn correlate_one(stream: &Stream) -> Correlation {
    match (pitch_to_dynamic(stream), pitch_to_dynamic_counts(stream)) {
        (Ok(points), Ok(counts)) => (points, counts, String::new()),
        (Err(error), _) | (_, Err(error)) => (Vec::new(), Vec::new(), error.to_string()),
    }
}

#[test]
fn the_crate_correlates_pitches_and_dynamics_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"correlate_parity_music21.py",
            c"correlate_parity_music21",
        )?;
        let mut failures = Vec::new();
        let chosen: Vec<String> = match std::env::var("CORRELATE_PARITY_SCORES") {
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
            let theirs: Vec<Correlation> = match music21.getattr("report")?.call1((&text,)) {
                Ok(report) => report.extract()?,
                Err(error) => {
                    failures.push(format!("{name}: music21 could not read it: {error}"));
                    continue;
                }
            };
            compared += 1;
            let mut ours = vec![correlate_one(&score)];
            ours.extend(score.parts().into_iter().map(correlate_one));
            for (index, (ours, theirs)) in ours.iter().zip(&theirs).enumerate() {
                let agree = if theirs.2.is_empty() {
                    ours.2.is_empty() && ours.0 == theirs.0 && ours.1 == theirs.1
                } else {
                    !ours.2.is_empty()
                };
                if !agree {
                    failures.push(format!(
                        "{name}: stream {index}: music21 {} {} pairs, music21-rs {} {} pairs; \
                         first: music21 {:?}, music21-rs {:?}",
                        theirs.2,
                        theirs.0.len(),
                        ours.2,
                        ours.0.len(),
                        theirs.0.first(),
                        ours.0.first()
                    ));
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
