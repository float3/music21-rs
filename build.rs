use std::path::PathBuf;
use std::process::Command;

fn main() {
    let script_path = PathBuf::from("./generate_tables.sh");
    let python_path = PathBuf::from("./generate_tables.py");

    let status = Command::new(script_path.clone())
        .status()
        .expect("Failed to execute command");

    if !status.success() {
        panic!("Python script exited with status: {}", status);
    }

    println!("cargo:rerun-if-changed={}", script_path.display());
    println!("cargo:rerun-if-changed={}", python_path.display());
    println!("cargo:rerun-if-changed=music21/music21/chord/tables.py");
    Command::new("cargo")
        .arg("fmt")
        .status()
        .expect("Failed to execute command");
}
