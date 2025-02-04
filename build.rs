use std::path::PathBuf;
use std::process::Command;

use pyo3::{
    types::{PyAnyMethods, PyModule},
    PyResult, Python,
};

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cardinality {
    None = 0,
    Monad,
    Diad,
    Trichord,
    Tetrachord,
    Pentachord,
    Hexachord,
    Septachord,
    Octachord,
    Nonachord,
    Decachord,
    Undecachord,
    Duodecachord,
}

fn main() -> PyResult<()> {
    let script_path = PathBuf::from("./generate_tables.sh");

    let status = Command::new("bash")
        .arg(script_path.clone())
        .status()
        .expect("Failed to execute command");

    if !status.success() {
        panic!("script exited with status: {}", status);
    };

    Python::with_gil(|py| {
        let sys = py.import("sys")?;
        let path = sys.getattr("path")?;
        path.call_method1("append", ("./venv/lib/python3.12/site-packages",))?;
        path.call_method1("append", ("./music21",))?;

        let chord = PyModule::import(py, "music21.chord")?;
        let tables = chord.getattr("tables")?;
        println!("tables: {:?}", tables);

        println!("cargo:rerun-if-changed={}", script_path.display());
        println!("cargo:rerun-if-changed=music21/music21/chord/tables.py");
        Ok(())
    })
}
