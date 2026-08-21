//! Scala archive pipeline: submodule `.scl` files -> `data/scala_archive.toml`
//! -> `src/tuningsystem/scala_bundled.rs`.
//!
//! The archive ships as one generated TOML rather than 3,932 loose `.scl`
//! files. That keeps the whole collection in a single reviewable artifact, lets
//! `cargo package` carry one path instead of a directory tree, and means the
//! crate never parses Scala text at load time — the degrees are already
//! structured by the time they reach `scala_bundled.rs`.
//!
//! Picking up new upstream files is automatic: `regenerate-scala-archive` reads
//! whatever `.scl` files the submodule currently has, so a submodule bump that
//! adds scales needs no edit here.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// One scale, as stored in `data/scala_archive.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ArchiveScale {
    /// Source file name, such as `"partch_43.scl"`.
    pub(crate) file: String,
    /// The file's description line.
    pub(crate) description: String,
    /// Degrees of one period, starting at `1/1`.
    ///
    /// Each entry is either `"n/d"` for an exact ratio or a decimal string for
    /// cents, exactly as the Scala file wrote it.
    pub(crate) degrees: Vec<String>,
    /// The interval the scale repeats at, in the same notation.
    pub(crate) period: String,
}

/// The whole `data/scala_archive.toml` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScalaArchiveData {
    /// music21 version the archive was taken from.
    pub(crate) music21_version: String,
    pub(crate) scale: Vec<ArchiveScale>,
}

pub(crate) fn data_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("data/scala_archive.toml")
}

pub(crate) fn generated_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("src/tuningsystem/scala_bundled.rs")
}

fn submodule_scl_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("music21/music21/scale/scala/scl")
}

pub(crate) fn read(path: &Path) -> Result<ScalaArchiveData, Box<dyn Error>> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

/// Reads the music21 version out of the submodule, for the freshness stamp.
pub(crate) fn music21_version(workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    let path = workspace_root.join("music21/music21/_version.py");
    let text = fs::read_to_string(&path)
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

/// Splits a `.scl` file into its description, degrees and repeat interval.
///
/// Scala's convention is the inverse of this crate's: `1/1` is implicit and the
/// last entry is the repeat interval. The returned degrees make `1/1` explicit
/// and hold the repeat interval separately, matching `ScalaScale`.
///
/// Degree tokens are kept as written — `"3/2"` stays a ratio and `"701.955"`
/// stays cents — so the TOML records what the archive actually says rather than
/// a lossy normalization of it.
fn parse_scl(contents: &str) -> Result<(String, Vec<String>, String), Box<dyn Error>> {
    let lines: Vec<&str> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('!'))
        .collect();

    let description = lines.first().unwrap_or(&"").trim().to_string();
    let count_line = lines.get(1).ok_or("scl file has no degree count")?;
    let count: usize = count_line
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .parse()
        .map_err(|_| format!("invalid degree count {count_line:?}"))?;

    let entries: Vec<&str> = lines
        .iter()
        .skip(2)
        .filter(|line| !line.is_empty())
        .take(count)
        .copied()
        .collect();
    if entries.len() != count {
        return Err(format!("declares {count} degrees but lists {}", entries.len()).into());
    }

    let mut degrees = vec!["1/1".to_string()];
    for entry in entries {
        // A `!` opens a comment with or without leading whitespace.
        let token = entry.split('!').next().unwrap_or_default();
        let token = token.split_whitespace().next().unwrap_or_default();
        if token.is_empty() {
            return Err("empty degree".into());
        }
        degrees.push(if token.contains('.') || token.contains('/') {
            token.to_string()
        } else {
            // A bare integer is that many over one.
            format!("{token}/1")
        });
    }

    // A zero-degree file is legal Scala; it has no repeat interval to take.
    let period = if degrees.len() > 1 {
        degrees.pop().expect("length checked")
    } else {
        "2/1".to_string()
    };
    Ok((description, degrees, period))
}

/// Rebuilds the archive TOML from whatever `.scl` files the submodule has.
pub(crate) fn regenerate(workspace_root: &Path) -> Result<ScalaArchiveData, Box<dyn Error>> {
    let dir = submodule_scl_dir(workspace_root);
    if !dir.is_dir() {
        return Err(format!(
            "{} is missing; run `git submodule update --init --recursive`",
            dir.display()
        )
        .into());
    }

    let mut names: Vec<String> = fs::read_dir(&dir)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|name| name.ends_with(".scl"))
        .collect();
    names.sort_unstable();
    if names.is_empty() {
        return Err(format!("no .scl files under {}", dir.display()).into());
    }

    let mut scale = Vec::with_capacity(names.len());
    let mut skipped = Vec::new();
    for file in names {
        // The archive is latin-1, and 73 files are not valid UTF-8.
        let bytes = fs::read(dir.join(&file))?;
        let text: String = bytes.iter().map(|&byte| byte as char).collect();
        match parse_scl(&text) {
            Ok((description, degrees, period)) => scale.push(ArchiveScale {
                file,
                description,
                degrees,
                period,
            }),
            Err(error) => skipped.push(format!("{file}: {error}")),
        }
    }

    if !skipped.is_empty() {
        return Err(format!(
            "{} archive files could not be parsed:\n  {}",
            skipped.len(),
            skipped.join("\n  ")
        )
        .into());
    }

    println!("  {} scales read from {}", scale.len(), dir.display());
    Ok(ScalaArchiveData {
        music21_version: music21_version(workspace_root)?,
        scale,
    })
}

pub(crate) fn write(path: &Path, data: &ScalaArchiveData) -> Result<(), Box<dyn Error>> {
    let mut out = String::new();
    out.push_str(
        "# The Scala scale archive, generated from the music21 submodule by\n\
         # `cargo run -p xtask -- regenerate-scala-archive`.\n\
         #\n\
         # Never hand-edit. A submodule bump that adds, removes or corrects a\n\
         # `.scl` file is picked up automatically by re-running that command;\n\
         # nothing here lists files individually.\n\
         #\n\
         # Degrees are written as the archive writes them: `n/d` for an exact\n\
         # ratio, a decimal for cents. `1/1` is explicit and the repeat interval\n\
         # is held apart as `period`, which is this crate's convention rather\n\
         # than Scala's.\n\
        ",
    );
    writeln!(out, "music21_version = {:?}\n", data.music21_version)?;
    for scale in &data.scale {
        writeln!(out, "[[scale]]")?;
        writeln!(out, "file = {:?}", scale.file)?;
        writeln!(out, "description = {:?}", scale.description)?;
        let degrees: Vec<String> = scale.degrees.iter().map(|d| format!("{d:?}")).collect();
        writeln!(out, "degrees = [{}]", degrees.join(", "))?;
        writeln!(out, "period = {:?}\n", scale.period)?;
    }
    fs::write(path, out)?;
    println!("Scala archive TOML written to {}", path.display());
    Ok(())
}

/// Renders the bundled index as structured data, not as embedded file bytes.
pub(crate) fn render(data: &ScalaArchiveData) -> String {
    let mut out = String::new();
    out.push_str(
        "//! The bundled Scala scale archive, emitted by `xtask emit-scala-archive`.\n\
         //!\n\
         //! Never hand-edit: change `data/scala_archive.toml` and re-emit, or\n\
         //! re-run `xtask regenerate-scala-archive` to pick the archive up from\n\
         //! the submodule again.\n\n",
    );
    let _ = writeln!(
        out,
        "/// music21 version this archive was generated from.\n\
         pub const MUSIC21_VERSION: &str = {:?};\n",
        data.music21_version
    );
    out.push_str(
        "/// Every bundled scale as (file name, description, degrees, period),\n\
         /// sorted by file name. Degrees are written as the archive writes them.\n\
         #[rustfmt::skip]\n",
    );
    let _ = writeln!(
        out,
        "pub static SCALES: [(&str, &str, &[&str], &str); {}] = [",
        data.scale.len()
    );
    for scale in &data.scale {
        let degrees: Vec<String> = scale.degrees.iter().map(|d| format!("{d:?}")).collect();
        let _ = writeln!(
            out,
            "    ({:?}, {:?}, &[{}], {:?}),",
            scale.file,
            scale.description,
            degrees.join(", "),
            scale.period
        );
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::parse_scl;

    #[test]
    fn splits_description_degrees_and_period() {
        let scl = "! t.scl\n!\nA fifth and an octave\n 2\n!\n 3/2\n 2/1\n";
        let (description, degrees, period) = parse_scl(scl).unwrap();
        assert_eq!(description, "A fifth and an octave");
        assert_eq!(degrees, ["1/1", "3/2"]);
        assert_eq!(period, "2/1");
    }

    #[test]
    fn keeps_cents_as_written() {
        let scl = "!\nCents\n 2\n 701.955\n 1200.0\n";
        let (_, degrees, period) = parse_scl(scl).unwrap();
        assert_eq!(degrees, ["1/1", "701.955"]);
        assert_eq!(period, "1200.0");
    }

    #[test]
    fn normalizes_bare_integers_to_ratios() {
        let scl = "!\nIntegers\n 2\n 3\n 4\n";
        let (_, degrees, period) = parse_scl(scl).unwrap();
        assert_eq!(degrees, ["1/1", "3/1"]);
        assert_eq!(period, "4/1");
    }

    #[test]
    fn strips_inline_bang_comments() {
        let scl = "!\nCommented\n 2\n 2957/2048!Gb\n 2/1\n";
        let (_, degrees, _) = parse_scl(scl).unwrap();
        assert_eq!(degrees, ["1/1", "2957/2048"]);
    }

    #[test]
    fn accepts_a_zero_degree_file() {
        // xxx.scl declares no degrees; music21 reads it as a scale with none.
        let scl = "! xxx.scl\n!\nSaved scale from Scala\n 0\n!\n";
        let (description, degrees, period) = parse_scl(scl).unwrap();
        assert_eq!(description, "Saved scale from Scala");
        assert_eq!(degrees, ["1/1"]);
        assert_eq!(period, "2/1");
    }
}
