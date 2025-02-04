#!/usr/bin/env nix-shell
//! ```cargo
//! [dependencies]
//! pyo3 = { version = "0.23.4", features = ["auto-initialize"] }
//! ```
/*
#!nix-shell -i rust-script -p rustc -p rust-script -p cargo
*/
use std::process::Command;

use pyo3::{types::PyAnyMethods, PyResult, Python};

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
    Python::with_gil(|py| {
        let sys = py.import("sys")?;
        let path = sys.getattr("path")?;
        path.call_method1("append", ("./venv/lib/python3.12/site-packages",))?;
        path.call_method1("append", ("./music21",))?;

        let chord = py.import("music21.chord")?;
        let tables = chord.getattr("tables")?;
        println!("tables: {:?}", tables);

        Ok(())
    })
}
