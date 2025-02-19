#[allow(unused)]
mod module {
    #[cfg(feature = "python")]
    use pyo3::{prelude::*, types::PyModule};
    use std::error::Error;
    use std::path::Path;
    use std::process::Command;
    use std::str::from_utf8;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    #[cfg(feature = "python")]
    use std::sync::LazyLock;

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

    fn git_clone() -> Result<(), Box<dyn Error>> {
        if Path::new("./music21").exists() {
            println!("Repository already cloned.");
            return Ok(());
        }
        run_command(
            &[
                "git",
                "clone",
                "--depth",
                "1",
                "https://github.com/cuthbertLab/music21.git",
                "./music21",
            ],
            "git clone",
        )
    }

    #[cfg(feature = "python")]
    fn create_venv() -> Result<(), Box<dyn Error>> {
        use std::path::Path;

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
            git_clone()?;
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

    /// Dummy function for music21.environment.Environment.
    /// This function does nothing and returns None.
    #[cfg(feature = "python")]
    #[pyfunction]
    fn dummy_environment(_name: &str) -> PyResult<()> {
        // A minimal stub that satisfies the call signature.
        Ok(())
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
        let modules: &Bound<'_, PyMapping> = binding.downcast()?;

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

        let url = "https://raw.githubusercontent.com/cuthbertLab/music21/refs/heads/master/music21/chord/tables.py";
        let response = reqwest::blocking::get(url)
            .map_err(|e| PyErr::new::<PyIOError, _>(format!("HTTP error: {}", e)))?;
        let code = CString::new(
            response
                .text()
                .map_err(|e| PyErr::new::<PyIOError, _>(format!("HTTP error: {}", e)))?,
        )
        .map_err(|e| PyErr::new::<PyIOError, _>(format!("CString error: {}", e)))?;
        let code: &CStr = &code;
        let tables = PyModule::from_code(py, code, c"tables.py", c"music21.chord.tables")?;
        Ok(tables)
    }
}

pub use module::*;
