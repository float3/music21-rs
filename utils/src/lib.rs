use std::{error::Error, process::Command, str::from_utf8};

fn git_submodule() -> Result<(), Box<dyn Error>> {
    run_command(&["git", "submodule", "init"], "init submodule")?;
    run_command(&["git", "submodule", "update"], "update submodule")?;
    Ok(())
}

fn git_pull() {
    if let Err(e) = run_command(
        &["git", "-C", "./music21", "pull", "origin", "master"],
        "git pull",
    ) {
        eprintln!("{}", e);
    }
}

fn create_venv() -> Result<(), Box<dyn Error>> {
    run_command(&["python3.12", "-m", "venv", "venv"], "create venv")?;
    Ok(())
}

fn install_dependencies() -> Result<(), Box<dyn Error>> {
    run_command(
        &[
            "./venv/bin/python3.12",
            "-m",
            "pip",
            "install",
            "-r",
            "./music21/requirements.txt",
        ],
        "pip install",
    )?;
    Ok(())
}

fn pip_upgrade() {
    if let Err(e) = run_command(
        &[
            "./venv/bin/python3.12",
            "-m",
            "pip",
            "install",
            "--upgrade",
            "pip",
        ],
        "pip upgrade",
    ) {
        eprintln!("{}", e);
    }
}

pub fn run_command(args: &[&str], description: &str) -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::new(args[0]);
    cmd.args(&args[1..]);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute {}: {}", description, e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = from_utf8(&output.stderr)
            .map_err(|e| format!("{} failed: stderr not valid UTF-8: {}", description, e))?;
        Err(format!("{} failed: {}", description, stderr).into())
    }
}

pub fn prepare() -> Result<(), Box<dyn Error>> {
    git_submodule()?;
    git_pull();
    create_venv()?;
    pip_upgrade();
    install_dependencies()?;
    Ok(())
}

use pyo3::{
    types::{PyAnyMethods, PyModule},
    Bound, PyErr, Python,
};

pub type Tables<'py> = Bound<'py, PyModule>;

pub fn init_py(py: Python<'_>) -> Result<(), PyErr> {
    let sys = py.import("sys")?;
    let path = sys.getattr("path")?;
    path.call_method1("append", ("./venv/lib/python3.12/site-packages",))?;
    path.call_method1("append", ("./music21",))?;
    Ok(())
}
