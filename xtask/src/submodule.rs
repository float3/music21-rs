//! What a generated file records about the submodule it was read from.
//!
//! A version string alone is not enough to pin the source: `11.0.0b9` is a
//! moving target on upstream master, so a bump inside one version would leave
//! every generated file looking fresh while its contents no longer matched.
//! The commit is exact, and the version is what a human reads, so both are
//! recorded.

use std::error::Error;
use std::path::Path;
use std::process::Command;

/// The version and commit a generated file was produced from.
#[derive(Debug, Clone)]
pub(crate) struct Stamp {
    /// What music21 calls itself, out of `music21/_version.py`.
    pub(crate) version: String,
    /// The commit the `music21` submodule is checked out at.
    pub(crate) commit: String,
}

impl Stamp {
    /// Reads both halves out of the `music21` submodule.
    pub(crate) fn music21(workspace_root: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            version: music21_version(workspace_root)?,
            commit: commit(workspace_root, "music21")?,
        })
    }
}

/// Reads the music21 version out of the submodule, without importing Python.
pub(crate) fn music21_version(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    let path = workspace_root.join("music21/music21/_version.py");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("{} is unreadable: {error}", path.display()))?;
    for line in text.lines() {
        if let Some((_, rest)) = line.split_once('=')
            && line.trim_start().starts_with("__version__")
        {
            return Ok(rest.trim().trim_matches(['\'', '"']).to_string());
        }
    }
    Err(format!("no __version__ in {}", path.display()).into())
}

/// The commit a submodule is checked out at, as a full forty-character hash.
///
/// Asks git rather than parsing `.git` by hand: a submodule's `.git` is a file
/// pointing into the superproject, and resolving a ref out of it means dealing
/// with packed refs.
pub(crate) fn commit(workspace_root: &Path, name: &str) -> Result<String, Box<dyn Error>> {
    let dir = workspace_root.join(name);
    if !dir.is_dir() {
        return Err(format!(
            "{} is missing; run `git submodule update --init --recursive`",
            dir.display()
        )
        .into());
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("could not run git for {}: {error}", dir.display()))?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse HEAD failed in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }

    let commit = String::from_utf8(output.stdout)
        .map_err(|error| format!("git printed a non-UTF-8 commit: {error}"))?
        .trim()
        .to_string();
    if commit.len() != 40 || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{} is not a commit hash", commit).into());
    }
    Ok(commit)
}
