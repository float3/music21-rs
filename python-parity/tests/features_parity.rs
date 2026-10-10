//! The crate's feature extractors against music21's: `features.jSymbolic`
//! and `features.native`, the language feature among them.
//!
//! Every extractor's id, name, description, size and flags are compared
//! with music21's live class. Then each corpus score is read by music21 and
//! by `from_musicxml` (which `musicxml_read_parity` holds to music21's
//! reader), prepared once on each side as a `DataInstance`, and put through
//! every extractor: the two must refuse the same features and agree on the
//! rest to a few ulps, each value a whole number where music21's is.
//!
//! music21 is music21 here, with nothing of the crate installed over it.
//! `FEATURES_PARITY_SCORES`, a `;`-separated list of corpus names, runs
//! those instead of the writer test's scores, which is how the corpus is
//! swept.

use music21_rs::features::{DataInstance, Extractor, jsymbolic, native};
use music21_rs::musicxml::from_musicxml;
use music21_rs::{FloatType, StreamKind};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

mod musicxml_common;
use musicxml_common::SCORES;

const MUSIC21: &str = r#"
import zipfile
from music21 import corpus, features
from music21.features import jSymbolic, native

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

EXTRACTORS = {
    'jSymbolic': jSymbolic.featureExtractors,
    'native': native.featureExtractors,
}

def metadata(module):
    return [(cls.id, cls.name, ' '.join(cls.description.split()), cls.dimensions,
             cls.discrete, cls.normalize)
            for cls in EXTRACTORS[module]]

def report(name):
    data = features.DataInstance(corpus.parse(name, forceSource=True))
    out = []
    for module, extractors in EXTRACTORS.items():
        for cls in extractors:
            try:
                vector = cls(data).extract().vector
                out.append((module, cls.id,
                            [(float(value), type(value) is int, str(value)) for value in vector],
                            ''))
            except Exception as error:
                out.append((module, cls.id, [], type(error).__name__))
    return out
"#;

/// music21's answer for one extractor: its library and id, each value as
/// a float, whether it is a whole number and how Python writes it, and the
/// name of the exception it raised instead, if it did.
type Report = (String, String, Vec<(FloatType, bool, String)>, String);

const MODULES: [(&str, &[Extractor]); 2] = [
    ("jSymbolic", jsymbolic::JSYMBOLIC),
    ("native", native::NATIVE),
];

#[test]
fn the_crate_extracts_features_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let (failures, compared) = Python::attach(|py| -> PyResult<(Vec<String>, usize)> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"features_parity_music21.py",
            c"features_parity_music21",
        )?;
        let mut failures = Vec::new();

        type Metadata = (String, String, String, usize, bool, bool);
        for (module, extractors) in MODULES {
            let theirs: Vec<Metadata> =
                music21.getattr("metadata")?.call1((module,))?.extract()?;
            let ours: Vec<Metadata> = extractors
                .iter()
                .map(|extractor| {
                    (
                        extractor.id().to_string(),
                        extractor.name().to_string(),
                        extractor.description().to_string(),
                        extractor.dimensions(),
                        extractor.discrete(),
                        extractor.normalize(),
                    )
                })
                .collect();
            if ours != theirs {
                failures.push(format!(
                    "the {module} extractors differ:\n  music21    {theirs:?}\n  music21-rs {ours:?}"
                ));
            }
        }

        let chosen: Vec<String> = match std::env::var("FEATURES_PARITY_SCORES") {
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
            let theirs: Vec<Report> =
                match music21.getattr("report")?.call1((name,)) {
                    Ok(report) => report.extract()?,
                    Err(error) => {
                        failures.push(format!("{name}: music21 could not read it: {error}"));
                        continue;
                    }
                };
            let score = match from_musicxml(&text) {
                Ok(score) if score.kind() == StreamKind::Score => score,
                Ok(_) => continue,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not read it: {error}"));
                    continue;
                }
            };
            let data = match DataInstance::new(&score) {
                Ok(data) => data,
                Err(error) => {
                    failures.push(format!("{name}: the crate could not prepare it: {error}"));
                    continue;
                }
            };
            compared += 1;
            for (module, id, their_vector, their_error) in theirs {
                let extractor = match module.as_str() {
                    "jSymbolic" => jsymbolic::extractor(&id),
                    _ => native::extractor(&id),
                };
                let Some(extractor) = extractor else {
                    failures.push(format!("{name}: the crate has no {module} extractor {id}"));
                    continue;
                };
                let their_text: Vec<&str> = their_vector
                    .iter()
                    .take(16)
                    .map(|(_, _, text)| text.as_str())
                    .collect();
                match extractor.extract(&data) {
                    Ok(feature) => {
                        // Each value must be a whole number where music21's
                        // is, within a few ulps of it, and written as music21
                        // writes it wherever the two are the same number.
                        let agree = their_error.is_empty()
                            && feature.values().len() == their_vector.len()
                            && feature.values().iter().zip(&their_vector).all(
                                |(ours, (theirs, integer, text))| {
                                    let ours_float = ours.as_float();
                                    ours.is_integer() == *integer
                                        && (ours_float - theirs).abs() <= 1e-12 * theirs.abs().max(1.0)
                                        && (ours_float != *theirs || ours.to_string() == *text)
                                },
                            );
                        if !agree {
                            let our_text: Vec<String> = feature
                                .values()
                                .iter()
                                .take(16)
                                .map(ToString::to_string)
                                .collect();
                            failures.push(format!(
                                "{name}: {id}: music21 {their_error} {their_text:?}, music21-rs {our_text:?}"
                            ));
                        }
                    }
                    Err(error) => {
                        if their_error.is_empty() {
                            failures.push(format!(
                                "{name}: {id}: music21 {their_text:?}, music21-rs refused: {error}"
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
