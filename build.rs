use std::path::PathBuf;
use std::process::Command;

fn main() {
    let script_path = PathBuf::from("./generate_tables.py");

    let status = Command::new("python")
        .arg(&script_path)
        .status()
        .expect("Failed to execute Python script");

    if !status.success() {
        panic!("Python script exited with status: {}", status);
    }

    println!("cargo:rerun-if-changed={}", script_path.display());
    println!("cargo:rerun-if-changed=music21/music21/chord/tables.py");
}
