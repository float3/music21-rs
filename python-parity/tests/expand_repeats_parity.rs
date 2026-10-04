//! `Stream::expand_repeats` against music21's `expandRepeats`.
//!
//! Each corpus score is read by music21's reader and by the crate's, and
//! each side plays its repeats out. For every part the measures played are
//! compared in order: the number each is given, with its suffix, and the
//! offset it stands at. The readers are held to music21's by their own
//! tests, so a difference is the expansion's. Repeats music21 refuses as
//! badly formed the crate has to refuse as well.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//!
//! `EXPAND_REPEATS_SCORES`, a `;`-separated list of corpus names, runs those
//! instead of the ones below.

use music21_rs::{Stream, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// Scores both sides play out alike, and what each exercises.
const SCORES: &[(&str, &str)] = &[
    (
        "beethoven/opus59no2/movement3.mxl",
        "a dal segno, with a jump standing past a barline",
    ),
    (
        "joplin/maple_leaf_rag.mxl",
        "repeats and endings on two staves",
    ),
    ("haydn/opus74no1/movement1.mxl", "a sonata movement"),
    (
        "haydn/opus74no1/movement3.mxl",
        "repeats music21 finds badly formed, refused by both",
    ),
    (
        "leadSheet/fosterBrownHair.mxl",
        "a first and second ending in a lead sheet",
    ),
    (
        "schumann_clara/polonaise_op1n4.mxl",
        "repeats in a piano piece",
    ),
    (
        "trecento/PMFC_06-Jacopo-01-Aquila-Altera.xml",
        "a first ending spanning its first and last measures only",
    ),
];

const HELPERS: &str = r#"
import re
import zipfile
from music21 import corpus

def source_text(name):
    path = str(corpus.getWork(name))
    if path.lower().endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            container = archive.read('META-INF/container.xml').decode('utf-8')
            root = re.search(r'full-path="([^"]+)"', container).group(1)
            data = archive.read(root)
    else:
        with open(path, 'rb') as handle:
            data = handle.read()
    for encoding in ('utf-8', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')

def played(name):
    score = corpus.parse(name, forceSource=True).expandRepeats()
    lines = []
    for part in score.parts:
        for measure in part.getElementsByClass('Measure'):
            lines.append(f'{measure.measureNumberWithSuffix()} @ {float(measure.offset):.6f}')
        lines.append('--')
    return lines
"#;

/// The crate's side of `played`.
fn played(score: &Stream) -> Vec<String> {
    let mut lines = Vec::new();
    for part in score.events().iter().filter_map(|event| {
        event
            .element()
            .as_stream()
            .filter(|inner| matches!(inner.kind(), StreamKind::Part | StreamKind::PartStaff))
    }) {
        for event in part.events() {
            if let Some(measure) = event.element().as_stream()
                && measure.kind() == StreamKind::Measure
            {
                lines.push(format!(
                    "{} @ {:.6}",
                    measure.number_with_suffix(),
                    event.offset()
                ));
            }
        }
        lines.push("--".to_string());
    }
    lines
}

#[test]
fn the_crate_plays_repeats_out_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, count) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let helpers = PyModule::from_code(
            py,
            &std::ffi::CString::new(HELPERS).expect("no nul in the helper"),
            c"expand_repeats_parity_helpers.py",
            c"expand_repeats_parity_helpers",
        )?;
        let chosen: Vec<String> = match std::env::var("EXPAND_REPEATS_SCORES") {
            Ok(list) => list
                .split(';')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => SCORES.iter().map(|(name, _)| name.to_string()).collect(),
        };
        let mut failures = Vec::new();
        for name in &chosen {
            let text: String = helpers
                .getattr("source_text")?
                .call1((name.as_str(),))?
                .extract()?;
            let theirs: Vec<String> = match helpers
                .getattr("played")?
                .call1((name.as_str(),))
                .and_then(|lines| lines.extract())
            {
                Ok(lines) => lines,
                Err(error) => {
                    // What music21 cannot play out the crate must refuse too.
                    let refused = music21_rs::musicxml::from_musicxml(&text)
                        .and_then(|score| score.expand_repeats())
                        .is_err();
                    if !refused {
                        failures.push(format!(
                            "{name}: music21 could not play it out, and the crate did: {error}"
                        ));
                    }
                    continue;
                }
            };
            let ours = match music21_rs::musicxml::from_musicxml(&text)
                .and_then(|score| score.expand_repeats())
            {
                Ok(score) => played(&score),
                Err(error) => {
                    failures.push(format!("{name}: the crate could not play it out: {error}"));
                    continue;
                }
            };
            if ours != theirs {
                let at = ours
                    .iter()
                    .zip(&theirs)
                    .position(|(a, b)| a != b)
                    .unwrap_or(ours.len().min(theirs.len()));
                failures.push(format!(
                    "{name}: measure {at} differs ({} from music21, {} from the crate): \
                     music21 {:?}, crate {:?}",
                    theirs.len(),
                    ours.len(),
                    theirs.get(at),
                    ours.get(at)
                ));
            }
        }
        Ok((failures, chosen.len()))
    })
    .expect("the Python side of the comparison");

    assert!(
        failures.is_empty(),
        "{} of {count} scores are played out differently:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}
