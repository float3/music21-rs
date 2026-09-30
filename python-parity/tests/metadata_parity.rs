//! The metadata vocabulary against music21's own.
//!
//! `music21_rs::metadata::STANDARD_PROPERTIES` is music21's
//! `metadata.properties.STANDARD_PROPERTY_DESCRIPTIONS` -- each property's
//! unique name, its namespaced name and whether it names a contributor --
//! and a MusicXML writer names every `<miscellaneous-field>` out of it. This
//! reads music21's table and asks for the same rows in the same order.

use music21_rs::metadata::STANDARD_PROPERTIES;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

#[test]
fn the_metadata_vocabulary_is_music21s() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let theirs = Python::attach(|py| -> PyResult<Vec<(String, String, bool)>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let properties = py.import("music21.metadata.properties")?;
        let mut rows = Vec::new();
        for description in properties
            .getattr("STANDARD_PROPERTY_DESCRIPTIONS")?
            .try_iter()?
        {
            let description = description?;
            let name: String = description.getattr("name")?.extract()?;
            let unique: Option<String> = description.getattr("uniqueName")?.extract()?;
            let namespace: String = description.getattr("namespace")?.extract()?;
            let contributor: bool = description.getattr("isContributor")?.extract()?;
            rows.push((
                unique.unwrap_or_else(|| name.clone()),
                format!("{namespace}:{name}"),
                contributor,
            ));
        }
        Ok(rows)
    })
    .unwrap_or_else(|error| panic!("reading music21's metadata properties: {error}"));

    let ours: Vec<(String, String, bool)> = STANDARD_PROPERTIES
        .iter()
        .map(|(unique, namespaced, contributor)| {
            (unique.to_string(), namespaced.to_string(), *contributor)
        })
        .collect();
    assert_eq!(ours, theirs);
}
