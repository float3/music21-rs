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

/// Where a bundled scale came from.
///
/// Both are git submodules, read the same way: the music21 archive supplies the
/// bulk of the scales and Plainsound Hexatone supplies a curated set. Neither is
/// stored in this repository; only the parsed result is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScaleSource {
    Music21,
    Hexatone,
}

impl ScaleSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Music21 => "music21",
            Self::Hexatone => "hexatone",
        }
    }
}

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
    /// Which directory this scale was read from.
    pub(crate) source: ScaleSource,
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

/// Scales from the Plainsound Hexatone submodule.
fn hexatone_scl_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("hexatone/scales")
}

/// Decodes `.scl` bytes, preferring UTF-8 and falling back to latin-1.
///
/// The Scala format is defined as latin-1 and 73 of music21's files are not
/// valid UTF-8, but files written this century generally are — the vendored
/// ones use ♭, ♯ and curly apostrophes. Decoding those as latin-1 turns each
/// UTF-8 byte into its own character and produces mojibake, so valid UTF-8 is
/// taken at face value and only the rest falls back.
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&byte| byte as char).collect(),
    }
}

/// Escapes a string as a TOML basic string.
///
/// Not `{:?}`: Rust's `Debug` writes non-printable characters as `\u{99}`,
/// which TOML rejects because it wants exactly four hex digits after `\u`.
fn toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 || control as u32 == 0x7f => {
                let _ = write!(out, "\\u{:04X}", control as u32);
            }
            character => out.push(character),
        }
    }
    out.push('"');
    out
}

/// Reads every `.scl` file in one directory, in sorted order.
fn read_directory(
    dir: &Path,
    source: ScaleSource,
    scale: &mut Vec<ArchiveScale>,
    failures: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let mut names: Vec<String> = fs::read_dir(dir)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|name| name.ends_with(".scl"))
        .collect();
    names.sort_unstable();

    for file in names {
        let bytes = fs::read(dir.join(&file))?;
        let text = decode(&bytes);
        match parse_scl(&text) {
            Ok((description, degrees, period)) => scale.push(ArchiveScale {
                file,
                description,
                degrees,
                period,
                source,
            }),
            Err(error) => failures.push(format!("{file}: {error}")),
        }
    }
    Ok(())
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

    // The format is description, count, then degrees. Real files break that in
    // three ways, so the strict reading is tried first and the fallbacks only
    // run when it fails — a file that parses today cannot start parsing
    // differently.
    let (description, count, entries): (String, usize, Vec<&str>) = match lines
        .get(1)
        .and_then(|line| line.split_whitespace().next())
        .and_then(|token| token.parse::<usize>().ok())
    {
        Some(count) => (
            lines.first().unwrap_or(&"").to_string(),
            count,
            lines.iter().skip(2).copied().collect(),
        ),
        None => {
            // Blank lines are only skipped on this path: a file may legitimately
            // have an empty description, and dropping blanks up front would read
            // its first degree as the count.
            let dense: Vec<&str> = lines.iter().copied().filter(|l| !l.is_empty()).collect();
            let number = |index: usize| {
                dense
                    .get(index)
                    .and_then(|line| line.split_whitespace().next())
                    .and_then(|token| token.parse::<usize>().ok())
            };
            if let Some(count) = number(1) {
                // A blank line sat between the description and the count.
                (
                    dense[0].to_string(),
                    count,
                    dense.iter().skip(2).copied().collect(),
                )
            } else if let Some(count) = number(0) {
                // The description was written as a `!` comment, so it was
                // stripped and the count now leads.
                (
                    String::new(),
                    count,
                    dense.iter().skip(1).copied().collect(),
                )
            } else {
                // No count line at all: take it from the degrees present.
                let entries: Vec<&str> = dense.iter().skip(1).copied().collect();
                (
                    dense.first().unwrap_or(&"").to_string(),
                    entries.len(),
                    entries,
                )
            }
        }
    };

    let entries: Vec<&str> = entries
        .into_iter()
        .filter(|line| !line.is_empty())
        .take(count)
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
        degrees.push(normalize_degree(token)?);
    }

    // A zero-degree file is legal Scala; it has no repeat interval to take.
    let period = if degrees.len() > 1 {
        degrees.pop().expect("length checked")
    } else {
        "2/1".to_string()
    };
    Ok((description, degrees, period))
}

/// Normalizes one degree token into a ratio or a cents value.
///
/// Scala also writes equal-tempered steps as `n\m`, meaning `n` steps of
/// `m`-EDO. Those are converted to cents here so the emitted archive only ever
/// contains the two forms the library understands.
fn normalize_degree(token: &str) -> Result<String, Box<dyn Error>> {
    if let Some((steps, divisions)) = token.split_once('\\') {
        let steps: f64 = steps.trim().parse()?;
        let divisions: f64 = divisions.trim().parse()?;
        if divisions == 0.0 {
            return Err(format!("degree {token:?} divides the octave into zero steps").into());
        }
        return Ok(format!("{:.5}", 1200.0 * steps / divisions));
    }
    if token.contains('.') || token.contains('/') {
        return Ok(token.to_string());
    }
    // A bare integer is that many over one.
    Ok(format!("{token}/1"))
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

    let mut scale = Vec::new();
    let mut failures = Vec::new();
    read_directory(&dir, ScaleSource::Music21, &mut scale, &mut failures)?;
    let from_submodule = scale.len();

    let hexatone = hexatone_scl_dir(workspace_root);
    if !hexatone.is_dir() {
        return Err(format!(
            "{} is missing; run `git submodule update --init --recursive`",
            hexatone.display()
        )
        .into());
    }
    read_directory(&hexatone, ScaleSource::Hexatone, &mut scale, &mut failures)?;

    if !failures.is_empty() {
        return Err(format!(
            "{} archive files could not be parsed:\n  {}",
            failures.len(),
            failures.join("\n  ")
        )
        .into());
    }

    // A vendored scale must not silently shadow one from the submodule.
    let mut seen = std::collections::BTreeSet::new();
    let duplicates: Vec<&str> = scale
        .iter()
        .filter(|entry| !seen.insert(entry.file.as_str()))
        .map(|entry| entry.file.as_str())
        .collect();
    if !duplicates.is_empty() {
        return Err(format!("duplicate scale file names: {duplicates:?}").into());
    }

    scale.sort_by(|left, right| left.file.cmp(&right.file));

    println!(
        "  {from_submodule} scales from music21, {} from hexatone",
        scale.len() - from_submodule
    );
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
        writeln!(out, "file = {}", toml_string(&scale.file))?;
        writeln!(out, "description = {}", toml_string(&scale.description))?;
        let degrees: Vec<String> = scale
            .degrees
            .iter()
            .map(|degree| toml_string(degree))
            .collect();
        writeln!(out, "degrees = [{}]", degrees.join(", "))?;
        writeln!(out, "period = {}", toml_string(&scale.period))?;
        writeln!(out, "source = {}\n", toml_string(scale.source.as_str()))?;
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
        let degrees: Vec<String> = scale
            .degrees
            .iter()
            .map(|degree| toml_string(degree))
            .collect();
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
    fn skips_a_blank_line_before_the_count() {
        // 53edo.scl leaves a blank line between description and count.
        let scl = "!\nFifty-three\n\n 2\n 3/2\n 2/1\n";
        let (description, degrees, _) = parse_scl(scl).unwrap();
        assert_eq!(description, "Fifty-three");
        assert_eq!(degrees, ["1/1", "3/2"]);
    }

    #[test]
    fn accepts_a_description_written_as_a_comment() {
        // 55edo.scl and 43-MT-1_5-Comma.scl put the description behind a `!`,
        // so it is stripped and the count leads.
        let scl = "! only a comment\n 2\n 3/2\n 2/1\n";
        let (description, degrees, period) = parse_scl(scl).unwrap();
        assert_eq!(description, "");
        assert_eq!(degrees, ["1/1", "3/2"]);
        assert_eq!(period, "2/1");
    }

    #[test]
    fn infers_a_missing_count_from_the_degrees() {
        // 41edo.scl has no count line at all.
        let scl = "!\nNo count here\n 9/8\n 3/2\n 2/1\n";
        let (description, degrees, period) = parse_scl(scl).unwrap();
        assert_eq!(description, "No count here");
        assert_eq!(degrees, ["1/1", "9/8", "3/2"]);
        assert_eq!(period, "2/1");
    }

    #[test]
    fn converts_equal_tempered_step_notation_to_cents() {
        let scl = "!\nForty-one\n 2\n 1\\41\n 41\\41\n";
        let (_, degrees, period) = parse_scl(scl).unwrap();
        // 1200 / 41 = 29.26829
        assert_eq!(degrees, ["1/1", "29.26829"]);
        assert_eq!(period, "1200.00000");
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
