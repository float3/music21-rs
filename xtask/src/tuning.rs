//! Tuning-table pipeline: Scala `.scl` archive -> `data/tuning_tables.toml`
//! -> `src/tuningsystem/generated.rs`.
//!
//! This mirrors the chord-table pipeline, with one difference: the Scala
//! archive is plain text in the `music21` submodule, so regenerating needs no
//! Python and no `python` feature.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// A ratio table as stored in `data/tuning_tables.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TuningTable {
    /// Name of the emitted `pub const`.
    pub(crate) name: String,
    /// Doc comment placed above the emitted constant.
    pub(crate) doc: String,
    /// Scala file this table is derived from, relative to the archive root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scala_file: Option<String>,
    /// The Scala file's own description line, carried for traceability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scala_description: Option<String>,
    /// Why this table has no Scala source, when it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) note: Option<String>,
    /// Degrees, starting at `1/1`.
    ///
    /// `[numerator, denominator]` is an exact ratio. `[numerator, denominator,
    /// 2]` is `2^(numerator/denominator)`, which is how a scale written in
    /// cents is carried: `c` cents becomes `2^(c/1200)`.
    pub(crate) ratios: Vec<Vec<u32>>,
}

/// The whole `data/tuning_tables.toml` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TuningTables {
    pub(crate) table: Vec<TuningTable>,
}

pub(crate) fn data_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("data/tuning_tables.toml")
}

pub(crate) fn generated_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("src/tuningsystem/generated.rs")
}

fn scala_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join("music21/music21/scale/scala/scl")
}

pub(crate) fn read(path: &Path) -> Result<TuningTables, Box<dyn Error>> {
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

/// Parses a Scala `.scl` file into ratios.
///
/// Scala's convention is the inverse of this crate's: `1/1` is implicit and the
/// octave is the final entry. The returned vector uses the crate's convention -
/// a leading `1/1` and no trailing octave - so it can be compared directly
/// against a committed table.
///
/// Cents entries become `[numerator, denominator, 2]`, meaning
/// `2^(numerator/denominator)` - the exponent form `Fraction` already supports
/// through its `base` field.
pub(crate) fn parse_scl(contents: &str) -> Result<Vec<Vec<u32>>, Box<dyn Error>> {
    let lines: Vec<&str> = contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('!'))
        .collect();

    let count: usize = lines
        .get(1)
        .and_then(|line| line.split_whitespace().next())
        .ok_or("scl file has no note count")?
        .parse()?;

    let entries: Vec<&str> = lines
        .iter()
        .skip(2)
        .filter(|line| !line.is_empty())
        .take(count)
        .copied()
        .collect();
    if entries.len() != count {
        return Err(format!(
            "scl file declares {count} notes but lists {}",
            entries.len()
        )
        .into());
    }

    let mut ratios: Vec<Vec<u32>> = vec![vec![1, 1]];
    for entry in &entries {
        let token = entry.split('!').next().unwrap_or_default();
        let token = token.split_whitespace().next().unwrap_or_default();
        if token.contains('.') {
            ratios.push(cents_to_exponent(token)?);
            continue;
        }
        let ratio = match token.split_once('/') {
            Some((numerator, denominator)) => vec![numerator.parse()?, denominator.parse()?],
            None => vec![token.parse()?, 1],
        };
        ratios.push(ratio);
    }

    // Drop the trailing octave; the crate's tables stop one degree short of it.
    ratios.pop();
    Ok(ratios)
}

/// Converts a cents literal into an exact `2^(numerator/denominator)` exponent.
///
/// `c` cents is `2^(c/1200)`, so a value with `k` decimal places becomes
/// `c * 10^k` over `1200 * 10^k`, reduced. The conversion is exact rather than
/// rounded, because these tables are the crate's source of truth.
fn cents_to_exponent(token: &str) -> Result<Vec<u32>, Box<dyn Error>> {
    let (whole, fraction) = token.split_once('.').unwrap_or((token, ""));
    let fraction = fraction.trim_end_matches('0');
    let decimals = u32::try_from(fraction.len())?;

    let scale = 10u64
        .checked_pow(decimals)
        .ok_or("cents value has too many decimals")?;
    let whole: u64 = if whole.is_empty() { 0 } else { whole.parse()? };
    let fraction: u64 = if fraction.is_empty() {
        0
    } else {
        fraction.parse()?
    };

    let numerator = whole
        .checked_mul(scale)
        .and_then(|value| value.checked_add(fraction))
        .ok_or("cents value is too large")?;
    let denominator = 1200u64
        .checked_mul(scale)
        .ok_or("cents value is too large")?;

    let divisor = gcd(numerator, denominator).max(1);
    Ok(vec![
        u32::try_from(numerator / divisor)?,
        u32::try_from(denominator / divisor)?,
        2,
    ])
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Re-reads every table that declares a `scala_file` from the submodule.
///
/// Hand-maintained tables are carried through untouched.
pub(crate) fn regenerate(workspace_root: &Path) -> Result<TuningTables, Box<dyn Error>> {
    let dir = scala_dir(workspace_root);
    if !dir.is_dir() {
        return Err(format!(
            "{} is missing; run `git submodule update --init --recursive`",
            dir.display()
        )
        .into());
    }

    let mut data = read(&data_path(workspace_root))?;
    for table in &mut data.table {
        let Some(file) = table.scala_file.clone() else {
            println!("  {:<20} hand-maintained, left as-is", table.name);
            continue;
        };

        let path = dir.join(&file);
        let contents =
            fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        let ratios = parse_scl(&contents).map_err(|err| format!("{file}: {err}"))?;

        if ratios.len() != table.ratios.len() {
            return Err(format!(
                "{} has {} ratios but {file} yields {}",
                table.name,
                table.ratios.len(),
                ratios.len()
            )
            .into());
        }

        let changed = ratios != table.ratios;
        table.ratios = ratios;
        table.scala_description = contents
            .lines()
            .map(str::trim)
            .find(|line| !line.starts_with('!'))
            .map(str::to_string);
        println!(
            "  {:<20} <- {file}{}",
            table.name,
            if changed { "  (CHANGED)" } else { "" }
        );
    }
    Ok(data)
}

pub(crate) fn write(path: &Path, data: &TuningTables) -> Result<(), Box<dyn Error>> {
    let mut out = String::new();
    out.push_str(
        "# Source of truth for the tuning-system ratio tables.\n\
         #\n\
         # Regenerate with `cargo run -p xtask -- regenerate-tuning-tables`, which\n\
         # re-reads every table that declares `scala_file` from music21's Scala\n\
         # archive in the `music21` submodule. Tables without one are maintained by\n\
         # hand and are carried through untouched.\n\
         #\n\
         # Emit the Rust with `cargo run -p xtask -- emit-tuning-tables`.\n\n",
    );
    for table in &data.table {
        writeln!(out, "[[table]]")?;
        writeln!(out, "name = {}", toml_string(&table.name))?;
        writeln!(out, "doc = {}", toml_string(&table.doc))?;
        if let Some(file) = &table.scala_file {
            writeln!(out, "scala_file = {}", toml_string(file))?;
        }
        if let Some(description) = &table.scala_description {
            writeln!(out, "scala_description = {}", toml_string(description))?;
        }
        if let Some(note) = &table.note {
            writeln!(out, "note = {}", toml_string(note))?;
        }
        writeln!(out, "ratios = [")?;
        for degree in &table.ratios {
            let rendered: Vec<String> = degree.iter().map(u32::to_string).collect();
            writeln!(out, "    [{}],", rendered.join(", "))?;
        }
        writeln!(out, "]\n")?;
    }
    fs::write(path, out)?;
    println!("Tuning table TOML written to {}", path.display());
    Ok(())
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(crate) fn render(data: &TuningTables) -> String {
    let mut out = String::new();
    out.push_str(
        "// @generated by `cargo run -p xtask -- emit-tuning-tables`.\n\
         // Source of truth: data/tuning_tables.toml. Do not edit by hand.\n\n\
         use super::Fraction;\n\n",
    );
    for table in &data.table {
        let _ = writeln!(out, "/// {}", table.doc);
        if let (Some(file), Some(description)) = (&table.scala_file, &table.scala_description) {
            let _ = writeln!(out, "///");
            let _ = writeln!(
                out,
                "/// Derived from `{file}` in music21's Scala archive: {description}"
            );
        }
        let _ = writeln!(
            out,
            "pub const {}: [Fraction; {}] = [",
            table.name,
            table.ratios.len()
        );
        for degree in &table.ratios {
            let _ = match degree.as_slice() {
                [numerator, denominator] => {
                    writeln!(out, "    Fraction::new({numerator}, {denominator}),")
                }
                [numerator, denominator, base] => writeln!(
                    out,
                    "    Fraction::new_with_base({numerator}, {denominator}, {base}),"
                ),
                other => panic!("a tuning degree needs 2 or 3 numbers, got {other:?}"),
            };
        }
        out.push_str("];\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{cents_to_exponent, parse_scl};

    #[test]
    fn parses_ratios_into_the_crate_convention() {
        // 1/1 becomes explicit, the trailing octave is dropped.
        let scl = "! test.scl\n!\nA test scale\n 3\n!\n 9/8\n 3/2\n 2/1\n";
        assert_eq!(
            parse_scl(scl).unwrap(),
            vec![vec![1, 1], vec![9, 8], vec![3, 2]]
        );
    }

    #[test]
    fn accepts_bare_integers_as_whole_ratios() {
        let scl = "!\nA test scale\n 2\n 3\n 2/1\n";
        assert_eq!(parse_scl(scl).unwrap(), vec![vec![1, 1], vec![3, 1]]);
    }

    #[test]
    fn carries_cents_as_a_base_two_exponent() {
        // 600 cents is exactly half an octave, so 2^(1/2).
        let scl = "!\nA cents scale\n 2\n 600.0\n 2/1\n";
        assert_eq!(parse_scl(scl).unwrap(), vec![vec![1, 1], vec![1, 2, 2]]);
    }

    #[test]
    fn mixes_ratio_and_cents_degrees() {
        // werck3.scl opens exactly like this.
        let scl = "!\nMixed\n 3\n 256/243\n 192.18000\n 2/1\n";
        let parsed = parse_scl(scl).unwrap();
        assert_eq!(parsed[1], vec![256, 243]);
        assert_eq!(parsed[2].len(), 3, "cents degree carries a base");
    }

    #[test]
    fn cents_conversion_is_exact_and_reduced() {
        assert_eq!(cents_to_exponent("1200.0").unwrap(), vec![1, 1, 2]);
        assert_eq!(cents_to_exponent("600").unwrap(), vec![1, 2, 2]);
        assert_eq!(cents_to_exponent("100.0").unwrap(), vec![1, 12, 2]);
        // 192.18 / 1200 = 19218 / 120000, reduced by 6.
        assert_eq!(
            cents_to_exponent("192.18000").unwrap(),
            vec![3203, 20000, 2]
        );
    }

    #[test]
    fn ignores_an_inline_bang_comment() {
        let scl = "!\nCommented\n 2\n 3/2!the fifth\n 2/1\n";
        assert_eq!(parse_scl(scl).unwrap(), vec![vec![1, 1], vec![3, 2]]);
    }
}
