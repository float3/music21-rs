//! The CSS colour names against `webcolors`, which music21's MusicXML writer
//! reads a colour given by name out of.

use music21_rs::musicxml::CSS_COLORS;
use music21_rs_python_parity::doctest::{add_dependency_venv, repo_root};
use pyo3::prelude::*;
use utils::{init_py, prepare};

/// music21 writes a colour given by name as the hex value `webcolors` gives
/// it, and the crate's table has to be that one.
#[test]
fn css_colour_names_are_webcolors() {
    let root = repo_root();
    std::env::set_current_dir(&root).expect("chdir to the repository root");
    prepare().expect("prepare the music21 reference checkout");
    let theirs = Python::attach(|py| -> PyResult<Vec<(String, String)>> {
        init_py(py)?;
        add_dependency_venv(py, &root)?;
        let webcolors = py.import("webcolors")?;
        let mut rows = Vec::new();
        for name in webcolors.call_method1("names", ("css3",))?.try_iter()? {
            let name: String = name?.extract()?;
            let hex: String = webcolors
                .call_method1("name_to_hex", (&name,))?
                .call_method0("upper")?
                .extract()?;
            rows.push((name, hex));
        }
        rows.sort();
        Ok(rows)
    })
    .unwrap_or_else(|error| panic!("reading webcolors: {error}"));

    let ours: Vec<(String, String)> = CSS_COLORS
        .iter()
        .map(|(name, hex)| (name.to_string(), hex.to_string()))
        .collect();
    assert_eq!(ours, theirs);
}
