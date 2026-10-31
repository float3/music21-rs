//! The crate's reading of a pitch's name in another language against
//! music21's: `languageExcerpts.naturalLanguageObjects`.
//!
//! music21 builds its dictionary by writing every pitch's name in the
//! language and turning the pairs round -- the seven letters, each with up
//! to four flats and four sharps -- and `pitch::language_dictionary` does
//! the same over the same names. Every pair of all four languages is
//! compared, and then every name read back through `toPitch`, which
//! answers a C natural for a name the language calls nothing.
//!
//! music21 is music21 here, with nothing of the crate installed over it.

use music21_rs::pitch::{Language, from_language_name, language_dictionary};
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use music21_rs_python_parity::music21_name;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use utils::{init_py, prepare};

const MUSIC21: &str = r#"
from music21.languageExcerpts import naturalLanguageObjects as nl

def dictionary(code):
    """Every name the language has, each with what music21 spells it."""
    return sorted(nl.generateLanguageDictionary(code).items())

def read(name, code):
    return nl.toPitch(name, code).name
"#;

/// Names no language calls a pitch by, which music21 answers a C for.
const UNKNOWN: &[&str] = &["", "Es", "H", "do", "ré", "zzz", "B", "mi####"];

#[test]
fn the_crate_reads_a_name_in_another_language_as_music21_does() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");

    let failures = Python::attach(|py| -> PyResult<Vec<String>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let _ = py.import("music21")?;
        let music21 = PyModule::from_code(
            py,
            &std::ffi::CString::new(MUSIC21).expect("no nul in the helper"),
            c"language_names_parity.py",
            c"language_names_parity",
        )?;

        let mut failures = Vec::new();
        for language in Language::ALL {
            let code = language.code();
            let theirs: Vec<(String, String)> =
                music21.getattr("dictionary")?.call1((code,))?.extract()?;
            let mut ours = language_dictionary(language);
            ours.sort();
            if ours != theirs {
                failures.push(format!(
                    "{code}: the dictionaries differ: {} against music21's {}",
                    ours.len(),
                    theirs.len()
                ));
                for (mine, theirs) in ours.iter().zip(&theirs) {
                    if mine != theirs {
                        failures.push(format!("  {mine:?} where music21 has {theirs:?}"));
                        break;
                    }
                }
            }
            // Every name the language has, read back, and some it has not.
            let names: Vec<String> = theirs
                .iter()
                .map(|(name, _)| name.clone())
                .chain(UNKNOWN.iter().map(|name| (*name).to_string()))
                .collect();
            for name in names {
                let theirs: String = music21
                    .getattr("read")?
                    .call1((name.as_str(), code))?
                    .extract()?;
                let ours = music21_name(&from_language_name(&name, language).name());
                if ours != theirs {
                    failures.push(format!(
                        "{code}: `{name}` is {ours} here and {theirs} to music21"
                    ));
                }
            }
        }
        // A language music21 does not support has no names at all, and
        // every name read under one is a C.
        let empty: Vec<(String, String)> =
            music21.getattr("dictionary")?.call1(("la",))?.extract()?;
        if !empty.is_empty() {
            failures.push(format!("music21 knows {} Latin names", empty.len()));
        }
        Ok(failures)
    })
    .expect("the comparison runs");

    assert!(
        failures.is_empty(),
        "the two readings differ:\n  {}",
        failures.join("\n  ")
    );
}
