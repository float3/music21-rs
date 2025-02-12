#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyModule};

use std::error::Error;
use std::process::Command;
use std::str::from_utf8;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "python")]
use std::sync::LazyLock;
use std::thread::sleep;
use std::time::Duration;

#[cfg(feature = "python")]
static PYTHON_EXE: LazyLock<String> = LazyLock::new(|| {
    let version: (u8, u8) = Python::with_gil(|py| -> PyResult<(u8, u8)> {
        let sys = py.import("sys")?;
        let version_info = sys.getattr("version_info")?;
        let major: u8 = version_info.get_item(0)?.extract()?;
        let minor: u8 = version_info.get_item(1)?.extract()?;
        Ok((major, minor))
    })
    .unwrap();
    format!("python{}.{}", version.0, version.1)
});

#[cfg(feature = "python")]
fn python_venv() -> String {
    format!("./venv/bin/{}", *PYTHON_EXE)
}

fn git_submodule() -> Result<(), Box<dyn Error>> {
    let max_attempts = 5;
    let mut attempts = 0;

    loop {
        match run_command(
            &["git", "submodule", "update", "--init"],
            "init and update submodule",
        ) {
            Ok(_) => return Ok(()),
            Err(e)
                if e.to_string().contains("could not lock config file")
                    && attempts < max_attempts =>
            {
                attempts += 1;
                sleep(Duration::from_millis(100)); // wait before retrying
            }
            Err(e) => return Err(e),
        }
    }
}

fn git_pull() {
    if let Err(e) = run_command(
        &["git", "-C", "./music21", "pull", "origin", "master"],
        "git pull",
    ) {
        eprintln!("{}", e);
    }
}

#[cfg(feature = "python")]
fn create_venv() -> Result<(), Box<dyn Error>> {
    use std::path::Path;

    match run_command(&[&PYTHON_EXE, "-m", "venv", "venv"], "create venv") {
        Ok(_) => Ok(()),
        Err(e) => match Path::new(&python_venv()).exists() {
            true => Ok(()),
            false => Err(e),
        },
    }
}

#[cfg(feature = "python")]
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

#[cfg(feature = "python")]
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
        #[cfg(feature = "python")]
        create_venv()?;
        #[cfg(feature = "python")]
        pip_upgrade();
        #[cfg(feature = "python")]
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

#[cfg(feature = "python")]
pub type Tables<'py> = pyo3::Bound<'py, PyModule>;

#[cfg(feature = "python")]
pub fn init_py(py: Python<'_>) -> pyo3::PyResult<()> {
    let sys = py.import("sys")?;
    let sysconfig = py.import("sysconfig")?;
    let system_site_packages: String = sysconfig
        .call_method1("get_path", ("purelib",))?
        .extract()?;
    let path = sys.getattr("path")?;
    path.call_method1(
        "extend",
        (vec![
            system_site_packages,
            format!("./venv/lib/{}/site-packages", *PYTHON_EXE),
            "./music21".to_owned(),
        ],),
    )?;
    Ok(())
}
