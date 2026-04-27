#[allow(unused)]
mod module {
    #[cfg(feature = "python")]
    use pyo3::{prelude::*, types::PyModule};
    use std::error::Error;
    #[cfg(feature = "python")]
    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::Command;
    use std::str::from_utf8;
    #[cfg(feature = "python")]
    use std::sync::LazyLock;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    #[cfg(feature = "python")]
    static PYTHON_EXE: LazyLock<String> = LazyLock::new(|| {
        if let Ok(explicit) = std::env::var("PYO3_PYTHON") {
            if !explicit.trim().is_empty() {
                return explicit;
            }
        }

        let version: (u8, u8) = Python::attach(|py| -> PyResult<(u8, u8)> {
            let sys = py.import("sys")?;
            let version_info = sys.getattr("version_info")?;
            let major: u8 = version_info.get_item(0)?.extract()?;
            let minor: u8 = version_info.get_item(1)?.extract()?;
            Ok((major, minor))
        })
        .unwrap();

        if cfg!(windows) {
            "python.exe".to_string()
        } else {
        format!("python{}.{}", version.0, version.1)
        }
    });

    #[cfg(feature = "python")]
    fn python_venv() -> String {
        if cfg!(windows) {
            "./venv/Scripts/python.exe".to_string()
        } else {
            format!("./venv/bin/{}", python_exe_name())
        }
    }

    #[cfg(feature = "python")]
    fn python_exe_name() -> String {
        Path::new(&*PYTHON_EXE)
            .file_name()
            .and_then(OsStr::to_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| PYTHON_EXE.clone())
    }

    #[cfg(feature = "python")]
    fn should_prepare_venv() -> bool {
        std::env::var("MUSIC21_RS_PREPARE_VENV")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    fn git_clone() -> Result<(), Box<dyn Error>> {
        if Path::new("./music21").exists() {
            println!("Repository already cloned.");
            return Ok(());
        }
        match run_command(
            &[
                "git",
                "clone",
                "--depth",
                "1",
                "https://github.com/cuthbertLab/music21.git",
                "./music21",
            ],
            "git clone",
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                if Path::new("./music21").exists() {
                    Ok(())
                } else {
                    Err(format!("Failed to clone repository: {e}").into())
                }
            }
        }
    }

    #[cfg(feature = "python")]
    fn create_venv() -> Result<(), Box<dyn Error>> {
        use std::path::Path;

        if Path::new(python_venv().as_str()).exists() {
            println!("venv already created.");
            return Ok(());
        }

        match Path::new(&python_venv()).exists() {
            true => Ok(()),
            false => run_command(&[&PYTHON_EXE, "-m", "venv", "venv"], "create venv"),
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
            eprintln!("{e}");
        }
    }

    static PREPARED: AtomicBool = AtomicBool::new(false);

    pub fn prepare() -> Result<(), Box<dyn Error>> {
        if PREPARED.load(Ordering::Acquire) {
            return Ok(());
        }
        println!("preparing environment");
        let res = (|| {
            git_clone()?;
            #[cfg(feature = "python")]
            if should_prepare_venv() {
                create_venv()?;
                pip_upgrade();
                install_dependencies()?;
            }
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
            .map_err(|e| format!("Failed to execute {description}: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = from_utf8(&output.stderr)
                .map_err(|e| format!("{description} failed: stderr not valid UTF-8: {e}"))?;
            Err(format!("{description} failed: {stderr}").into())
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
        let mut to_extend = vec![system_site_packages, "./music21".to_owned()];
        let venv_site = if cfg!(windows) {
            "./venv/Lib/site-packages".to_string()
        } else {
            format!("./venv/lib/{}/site-packages", python_exe_name())
        };
        if Path::new(&venv_site).exists() {
            to_extend.push(venv_site);
        }
        path.call_method1("extend", (to_extend,))?;
        Ok(())
    }

    /// Dummy function for music21.environment.Environment.
    /// This function does nothing and returns None.
    #[cfg(feature = "python")]
    #[pyfunction]
    fn dummy_environment(py: Python<'_>, _name: &str) -> PyResult<Py<PyAny>> {
        use pyo3::types::PyDict;

        let env = PyDict::new(py);
        env.set_item("warnings", 0)?;
        Ok(env.into_any().unbind())
    }

    /// Creates dummy modules for missing music21 dependencies.
    /// This should be called before importing tables.py.
    #[cfg(feature = "python")]
    fn create_dummy_modules(py: Python) -> PyResult<()> {
        use pyo3::types::PyList;
        use pyo3::types::PyMapping;
        use pyo3::types::PyModule;

        let sys = py.import("sys")?;
        let binding = sys.getattr("modules")?;
        let modules: &Bound<'_, PyMapping> = binding.cast()?;

        let music21_mod = PyModule::new(py, "music21")?;
        let path_list = PyList::new(py, ["./music21"])?;
        let path_list = path_list.into_pyobject(py)?;
        music21_mod.setattr("__path__", path_list)?;
        modules.set_item("music21", music21_mod)?;

        if !modules.contains("music21.chord")? {
            let chord_mod = PyModule::new(py, "music21.chord")?;
            let chord_path = PyList::new(py, ["./music21/chord"])?.into_pyobject(py)?;
            chord_mod.setattr("__path__", chord_path)?;
            modules.set_item("music21.chord", chord_mod)?;
        }

        let env_mod = PyModule::new(py, "music21.environment")?;
        let env_func = wrap_pyfunction!(dummy_environment, py)?;
        env_mod.add("Environment", env_func)?;
        modules.set_item("music21.environment", env_mod)?;

        let exc_mod = PyModule::new(py, "music21.exceptions21")?;
        let builtins = py.import("builtins")?;
        let exception_type = builtins.getattr("Exception")?;
        exc_mod.add("Music21Exception", exception_type)?;
        modules.set_item("music21.exceptions21", exc_mod)?;

        Ok(())
    }

    #[cfg(feature = "python")]
    pub fn init_py_with_dummies(py: Python) -> PyResult<()> {
        create_dummy_modules(py)?;
        Ok(())
    }

    #[cfg(feature = "python")]
    pub fn get_tables(py: Python<'_>) -> Result<Bound<'_, PyModule>, PyErr> {
        use pyo3::exceptions::PyIOError;
        use std::ffi::CStr;
        use std::ffi::CString;
        use std::fs;

        let local_path = "./music21/music21/chord/tables.py";
        let code_string = match fs::read_to_string(local_path) {
            Ok(code) => code,
            Err(local_err) => {
                let url = "https://raw.githubusercontent.com/cuthbertLab/music21/refs/heads/master/music21/chord/tables.py";
                let response = reqwest::blocking::get(url).map_err(|http_err| {
                    PyErr::new::<PyIOError, _>(format!(
                        "Failed reading local tables ({local_err}) and HTTP fallback failed ({http_err})"
                    ))
                })?;
                response
                    .text()
                    .map_err(|e| PyErr::new::<PyIOError, _>(format!("HTTP error: {e}")))?
            }
        };

        let code = CString::new(code_string)
            .map_err(|e| PyErr::new::<PyIOError, _>(format!("CString error: {e}")))?;
        let code: &CStr = &code;
        let tables = PyModule::from_code(py, code, c"tables.py", c"music21.chord.tables")?;
        Ok(tables)
    }
}

#[allow(unused_imports)]
pub use module::*;
