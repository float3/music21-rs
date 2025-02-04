use std::path::PathBuf;
use std::process::Command;

use pyo3::{
    types::{PyAnyMethods, PyModule},
    PyResult, Python,
};

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
enum Cardinality {
    None = 0,
    Monad = 1,
    Diad = 2,
    Trichord = 3,
    Tetrachord = 4,
    Pentachord = 5,
    Hexachord = 6,
    Septachord = 7,
    Octachord = 8,
    Nonachord = 9,
    Decachord = 10,
    Undecachord = 11,
    Duodecachord = 12,
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
