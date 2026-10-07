//! The crate's feature data sets against music21's: `features.DataSet`,
//! its tab, CSV and ARFF output, `allFeaturesAsList`, `extractorsById` and
//! `getIndex`.
//!
//! Every extractor of both libraries, but music21's language feature, reads
//! a handful of corpus scores into one data set on each side, music21 and
//! the crate each reading the score's MusicXML text; each table must come
//! out the same in every format. A cell that is a number may differ from
//! music21's in its last digits, as `features_parity` allows, but must be
//! written the same wherever the two numbers are equal. Every extractor's
//! name must index the same, and ids in every spelling must find the same
//! extractors.

use music21_rs::features::{
    DataSet, Library, OutputFormat, all_features_as_list, extractors_by_id, index_of,
};
use music21_rs::musicxml::from_musicxml;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

const MUSIC21: &str = r#"
import zipfile
from music21 import converter, corpus, features
from music21.features import base, jSymbolic, native

# joblib's workers cannot be started from inside the test binary.
base.safeToParallize = lambda: False

def source_text(name):
    path = corpus.getWork(name)
    if isinstance(path, (list, tuple)):
        path = path[0]
    path = str(path)
    if path.lower().endswith('.mxl'):
        with zipfile.ZipFile(path) as archive:
            names = [n for n in archive.namelist()
                     if not n.startswith('META-INF') and n.lower().endswith(('.xml', '.musicxml'))]
            data = archive.read(names[0])
    else:
        with open(path, 'rb') as handle:
            data = handle.read()
    for encoding in ('utf-8-sig', 'utf-16'):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            pass
    return data.decode('latin-1')

EXTRACTORS = (list(jSymbolic.featureExtractors)
              + [cls for cls in native.featureExtractors if cls.id != 'TX1'])

def tables(names):
    data_set = features.DataSet(classLabel='Composer Name')
    data_set.runParallel = False
    data_set.addFeatureExtractors(EXTRACTORS)
    for name in names:
        score = converter.parse(source_text(name), format='musicxml', forceSource=True)
        data_set.addData(score, classValue=name.split('/')[0])
    data_set.process()
    return [data_set.getString(form) for form in ('tab', 'csv', 'arff')]

def all_features(name):
    score = converter.parse(source_text(name), format='musicxml', forceSource=True)
    return [[str(value) for value in vector]
            for vector in features.allFeaturesAsList(score)[:-1]]

def indexes(names):
    out = []
    for name in names:
        found = features.getIndex(name)
        out.append(None if found is None else (found[0], found[1]))
    return out

def ids(queries):
    return [[cls.id for cls in features.extractorsById(query)] for query in queries]
"#;

const SCORES: [&str; 7] = [
    "bach/bwv66.6.mxl",
    "bach/bwv324.mxl",
    "handel/rinaldo/Lascia_chio_pianga.mxl",
    "schoenberg/opus19/movement2.mxl",
    "monteverdi/madrigal.3.1.mxl",
    "leadSheet/fosterBrownHair.mxl",
    "trecento/PMFC_06_8-In Verde Prato.xml",
];

/// Whether two cells agree: the same text, or numbers within a few ulps
/// written the same where they are the same number.
fn cells_agree(ours: &str, theirs: &str) -> bool {
    if ours == theirs {
        return true;
    }
    match (ours.parse::<f64>(), theirs.parse::<f64>()) {
        (Ok(ours), Ok(theirs)) => {
            ours != theirs && (ours - theirs).abs() <= 1e-12 * theirs.abs().max(1.0)
        }
        _ => false,
    }
}

/// The lines of two tables where they disagree.
fn table_differences(label: &str, ours: &str, theirs: &str) -> Vec<String> {
    let our_lines: Vec<&str> = ours.split('\n').collect();
    let their_lines: Vec<&str> = theirs.split('\n').collect();
    if our_lines.len() != their_lines.len() {
        return vec![format!(
            "{label}: {} lines, music21 {}",
            our_lines.len(),
            their_lines.len()
        )];
    }
    let separator = if label == "tab" { '\t' } else { ',' };
    let mut differences = Vec::new();
    for (number, (ours, theirs)) in our_lines.iter().zip(&their_lines).enumerate() {
        let our_cells: Vec<&str> = ours.split(separator).collect();
        let their_cells: Vec<&str> = theirs.split(separator).collect();
        let agree = our_cells.len() == their_cells.len()
            && our_cells
                .iter()
                .zip(&their_cells)
                .all(|(ours, theirs)| cells_agree(ours, theirs));
        if !agree {
            let at = our_cells
                .iter()
                .zip(&their_cells)
                .position(|(ours, theirs)| !cells_agree(ours, theirs));
            differences.push(format!(
                "{label} line {number}, cell {at:?}: music21 {:?}, music21-rs {:?}",
                at.and_then(|at| their_cells.get(at)),
                at.and_then(|at| our_cells.get(at)),
            ));
        }
    }
    differences
}

#[test]
fn the_crate_writes_data_sets_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"dataset_parity_music21.py",
            c"dataset_parity_music21",
        )?;
        let mut failures = Vec::new();

        let mut ours = DataSet::new("Composer Name");
        ours.add_extractors(Library::ALL.iter().flat_map(|library| library.extractors()));
        for name in SCORES {
            let text: String = music21.getattr("source_text")?.call1((name,))?.extract()?;
            let score = from_musicxml(&text).expect("the crate reads the score");
            let composer = name.split('/').next().expect("a corpus directory");
            ours.add_data(&score, composer, None)
                .expect("the crate prepares the score");
        }
        ours.process();
        let theirs: Vec<String> = music21
            .getattr("tables")?
            .call1((SCORES.to_vec(),))?
            .extract()?;
        for ((label, format), theirs) in [
            ("tab", OutputFormat::Tab),
            ("csv", OutputFormat::Csv),
            ("arff", OutputFormat::Arff),
        ]
        .into_iter()
        .zip(theirs)
        {
            failures.extend(table_differences(label, &ours.to_text(format), &theirs));
        }

        let text: String = music21
            .getattr("source_text")?
            .call1((SCORES[0],))?
            .extract()?;
        let score = from_musicxml(&text).expect("the crate reads the score");
        let theirs: Vec<Vec<String>> = music21
            .getattr("all_features")?
            .call1((SCORES[0],))?
            .extract()?;
        let ours: Vec<Vec<String>> = all_features_as_list(&score)
            .expect("the crate prepares the score")
            .into_iter()
            .map(|values| values.iter().map(ToString::to_string).collect())
            .collect();
        let agree = ours.len() == theirs.len()
            && ours.iter().zip(&theirs).all(|(ours, theirs)| {
                ours.len() == theirs.len()
                    && ours
                        .iter()
                        .zip(theirs)
                        .all(|(ours, theirs)| cells_agree(ours, theirs))
            });
        if !agree {
            failures.push(format!(
                "allFeaturesAsList:\n  music21    {theirs:?}\n  music21-rs {ours:?}"
            ));
        }

        let names: Vec<&str> = Library::ALL
            .iter()
            .flat_map(|library| library.extractors())
            .map(|extractor| extractor.name())
            .chain(["aBrandNewFeature!", "Language Feature"])
            .collect();
        let theirs: Vec<Option<(usize, String)>> = music21
            .getattr("indexes")?
            .call1((names.clone(),))?
            .extract()?;
        for (name, theirs) in names.iter().zip(theirs) {
            // music21's native list ends with the language feature, which
            // the crate has not got.
            if *name == "Language Feature" {
                continue;
            }
            let ours =
                index_of(name, None).map(|(index, library)| (index, library.name().to_string()));
            if ours != theirs {
                failures.push(format!(
                    "getIndex({name:?}): music21 {theirs:?}, music21-rs {ours:?}"
                ));
            }
        }

        let queries: Vec<Vec<&str>> = vec![
            vec!["p20"],
            vec!["p19", "p20"],
            vec!["r31", "r32", "r33", "r34", "r35", "p1", "p2"],
            vec!["p-20", "p 21"],
            vec!["P22"],
            vec!["ql1", "CS12", "mc1", "M10", "m17"],
            vec!["nothing"],
        ];
        let theirs: Vec<Vec<String>> = music21
            .getattr("ids")?
            .call1((queries.clone(),))?
            .extract()?;
        for (query, theirs) in queries.iter().zip(theirs) {
            let ours: Vec<String> = extractors_by_id(query, &Library::ALL)
                .iter()
                .map(|extractor| extractor.id().to_string())
                .collect();
            if ours != theirs {
                failures.push(format!(
                    "extractorsById({query:?}): music21 {theirs:?}, music21-rs {ours:?}"
                ));
            }
        }
        Ok(failures)
    })
    .expect("the Python side runs");

    assert!(
        failures.is_empty(),
        "{} differences:\n\n{}",
        failures.len(),
        failures.join("\n")
    );
}
