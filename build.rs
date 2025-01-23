use std::path::PathBuf;
use std::process::Command;

fn main() {
    let script_path = PathBuf::from("./generate_tables.py");

    //create venv
    let status = Command::new("python")
        .arg("-m")
        .arg("venv")
        .arg("venv")
        .status()
        .expect("Failed to create venv");

    if !status.success() {
        panic!("Failed to create venv");
    }

    // call pip
    let status = Command::new("venv/bin/pip")
        .arg("install")
        .arg("-r")
        .arg("music21/requirements.txt")
        .status()
        .expect("Failed to install requirements");

    if !status.success() {
        panic!("Failed to install requirements");
    }

    let status = Command::new("venv/bin/python")
        .arg(&script_path)
        .status()
        .expect("Failed to execute Python script");

    if !status.success() {
        panic!("Python script exited with status: {}", status);
    }

    println!("cargo:rerun-if-changed={}", script_path.display());
    println!("cargo:rerun-if-changed=music21/music21/chord/tables.py");
}
