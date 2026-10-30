//! music21's answers kept on disk between runs.
//!
//! A parity test asks music21 the same questions on every run, and the
//! answers change only when music21 or the question does. [`helpers`] loads a
//! test's Python helper module and makes the functions it names look up an
//! answer before asking music21: the answer is kept under the hash of the
//! helper's source, the function's name, its arguments, the music21
//! submodule's commit and the Python version. Only a function that asks
//! music21 with nothing of the crate installed, and whose answer depends on
//! its arguments alone, can be named.
//!
//! Answers are kept in `python-parity/target/music21-answers`, or the
//! directory `MUSIC21_PARITY_ANSWERS` names. `MUSIC21_PARITY_ANSWERS=off`
//! asks music21 every time, as does a music21 checkout with uncommitted
//! changes, whose commit does not say what it holds.

use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use pyo3::prelude::*;

use crate::doctest::repo_root;

const KEEPER: &str = r#"
import hashlib
import os
import pickle
import platform


def _plain(value):
    if value is None or isinstance(value, (bool, int, float, str, bytes)):
        return True
    if isinstance(value, (list, tuple)):
        return all(_plain(item) for item in value)
    if isinstance(value, dict):
        return all(_plain(key) and _plain(item) for key, item in value.items())
    return False


def keep(module, names, directory, salt):
    salt = (salt, platform.python_version())
    for name in names:
        ask = getattr(module, name)

        def answer(*args, _ask=ask, _name=name):
            if not _plain(args):
                return _ask(*args)
            key = hashlib.sha256(pickle.dumps((salt, _name, args), protocol=4)).hexdigest()
            path = os.path.join(directory, key[:2], key + '.pickle')
            try:
                with open(path, 'rb') as handle:
                    return pickle.load(handle)
            except (OSError, EOFError, pickle.UnpicklingError):
                pass
            value = _ask(*args)
            if _plain(value):
                os.makedirs(os.path.dirname(path), exist_ok=True)
                partial = f'{path}.{os.getpid()}'
                with open(partial, 'wb') as handle:
                    pickle.dump(value, handle, protocol=4)
                os.replace(partial, path)
            return value

        setattr(module, name, answer)
"#;

/// The directory answers are kept in and the music21 commit they are kept
/// for, or `None` when answers are not kept.
fn store() -> Option<&'static (PathBuf, String)> {
    static STORE: OnceLock<Option<(PathBuf, String)>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            let directory = match std::env::var("MUSIC21_PARITY_ANSWERS") {
                Ok(value) if value == "off" => return None,
                Ok(value) if !value.is_empty() => PathBuf::from(value),
                _ => repo_root().join("python-parity/target/music21-answers"),
            };
            let music21 = repo_root().join("music21");
            let git = |args: &[&str]| -> Option<String> {
                let output = Command::new("git")
                    .arg("-C")
                    .arg(&music21)
                    .args(args)
                    .output()
                    .ok()?;
                output
                    .status
                    .success()
                    .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
            };
            let commit = git(&["rev-parse", "HEAD"])?;
            let changes = git(&["status", "--porcelain", "--untracked-files=no"])?;
            changes.is_empty().then_some((directory, commit))
        })
        .as_ref()
}

/// The helper module `code`, loaded as `PyModule::from_code` loads it, with
/// the functions in `kept` answering from disk where they have answered the
/// same arguments before.
pub fn helpers<'py>(
    py: Python<'py>,
    code: &str,
    file_name: &CStr,
    module_name: &CStr,
    kept: &[&str],
) -> PyResult<Bound<'py, PyModule>> {
    let module = PyModule::from_code(py, &CString::new(code)?, file_name, module_name)?;
    let Some((directory, commit)) = store() else {
        return Ok(module);
    };
    let keeper = PyModule::from_code(
        py,
        &CString::new(KEEPER)?,
        c"music21_rs_answers.py",
        c"music21_rs_answers",
    )?;
    keeper.getattr("keep")?.call1((
        &module,
        kept.to_vec(),
        directory.to_string_lossy().to_string(),
        (commit.as_str(), code),
    ))?;
    Ok(module)
}
