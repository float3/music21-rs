use std::{
    error::Error,
    process::Command,
    str::from_utf8,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock,
    },
};

const PYTHON_EXE: LazyLock<String> = LazyLock::new(|| {
    let x: (u8, u8) = Python::with_gil(|py| -> PyResult<(u8, u8)> {
        let sys = py.import("sys")?;
        let version_info = sys.getattr("version_info")?;
        let major: u8 = version_info.get_item(0)?.extract()?;
        let minor: u8 = version_info.get_item(1)?.extract()?;

        Ok((major, minor))
    })
    .unwrap();

    format!("python{}.{}", x.0, x.1)
});

fn python_venv() -> String {
    let s: &str = &PYTHON_EXE;
    format!("./venv/bin/{}", s)
}

fn git_submodule() -> Result<(), Box<dyn Error>> {
    run_command(
        &["git", "submodule", "update", "--init"],
        "init and update submodule",
    )?;
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
    run_command(&[&PYTHON_EXE, "-m", "venv", "venv"], "create venv")?;
    Ok(())
}

fn install_dependencies() -> Result<(), Box<dyn Error>> {
    run_command(
        &[
            python_venv().as_str(),
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
            python_venv().as_str(),
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

static PREPARED: AtomicBool = AtomicBool::new(false);

pub fn prepare() -> Result<(), Box<dyn Error>> {
    if PREPARED.load(Ordering::Acquire) {
        return Ok(());
    }
    println!("preparing environment");
    let res = (|| {
        git_submodule()?;
        git_pull();
        create_venv()?;
        pip_upgrade();
        install_dependencies()?;
        Ok(())
    })();
    PREPARED.store(true, Ordering::Release);
    res
}

pub fn run_command(args: &[&str], description: &str) -> Result<(), Box<dyn Error>> {
    println!("{} running command: {}", description, args.join(" "));
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

use pyo3::{
    types::{PyAnyMethods, PyModule},
    Bound, PyErr, PyResult, Python,
};

pub type Tables<'py> = Bound<'py, PyModule>;

pub fn init_py(py: Python<'_>) -> Result<(), PyErr> {
    let sys = py.import("sys")?;
    let sysconfig = py.import("sysconfig")?;
    let system_site_packages: String = sysconfig
        .call_method1("get_path", ("purelib",))?
        .extract()?;
    let path = sys.getattr("path")?;
    let s: &str = &PYTHON_EXE;
    path.call_method1(
        "extend",
        (vec![
            system_site_packages,
            format!("./venv/lib/{}/site-packages", s),
            "./music21".to_owned(),
        ],),
    )?;
    Ok(())
}
