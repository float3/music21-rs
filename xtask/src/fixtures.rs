//! Generates the music21 expectation fixtures under `data/`, from Rust.
//!
//! These fixtures record what music21 actually produces, so the crate's
//! hand-written tables and ports can be checked without importing music21 at
//! test time. Generating them needs the real `music21` package — not the
//! stubbed one the chord-table bridge builds — because it reaches
//! `music21.scale`, `music21.harmony`, `music21.meter` and `music21.pitch`.
//!
//! There is no Python source in this repository. The interpreter is driven
//! through pyo3, and the only thing the caller supplies is an environment where
//! `import music21` can succeed: the submodule for the package itself, plus a
//! virtualenv holding music21's own dependencies.

use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyList};

use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Numerators the meter fixture sweeps, matching music21's own coverage.
const METER_NUMERATORS: [u32; 21] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 20, 21, 24, 27,
];
const METER_DENOMINATORS: [u32; 6] = [1, 2, 4, 8, 16, 32];

/// Tonics the scale fixture realizes every scale on.
const SCALE_TONICS: [&str; 15] = [
    "C4", "G4", "D4", "A4", "E4", "B4", "F#4", "C#4", "F4", "B-4", "E-4", "A-4", "D-4", "G-4",
    "C-4",
];

/// Rust `ScaleType` variant name paired with its music21 class name.
const SCALE_TYPES: [(&str, &str); 20] = [
    ("Major", "MajorScale"),
    ("Minor", "MinorScale"),
    ("Dorian", "DorianScale"),
    ("Phrygian", "PhrygianScale"),
    ("Lydian", "LydianScale"),
    ("Mixolydian", "MixolydianScale"),
    ("Locrian", "LocrianScale"),
    ("Hypodorian", "HypodorianScale"),
    ("Hypophrygian", "HypophrygianScale"),
    ("Hypolydian", "HypolydianScale"),
    ("Hypomixolydian", "HypomixolydianScale"),
    ("Hypolocrian", "HypolocrianScale"),
    ("Hypoaeolian", "HypoaeolianScale"),
    ("HarmonicMinor", "HarmonicMinorScale"),
    ("MelodicMinor", "MelodicMinorScale"),
    ("Chromatic", "ChromaticScale"),
    ("WholeTone", "WholeToneScale"),
    ("Octatonic", "OctatonicScale"),
    ("RagAsawari", "RagAsawari"),
    ("RagMarwa", "RagMarwa"),
];

/// Locates a virtualenv holding music21's dependencies.
fn dependency_venv(workspace_root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    for relative in [".m21venv", "venv", ".venv"] {
        let candidate = workspace_root.join(relative);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    Err(concat!(
        "no virtualenv with music21's dependencies found. The fixtures need a ",
        "real music21 import, which needs its requirements. Create one with:\n",
        "  uv venv .m21venv --python 3.12\n",
        "  uv pip install --python .m21venv chardet joblib jsonpickle \\\n",
        "      more_itertools numpy requests webcolors"
    )
    .into())
}

/// Puts the submodule and the dependency venv on `sys.path`, then imports music21.
///
/// Deliberately does not use the chord-table bridge's stubbing: that exists so
/// `chord/tables.py` can be imported without music21's dependencies, and it
/// makes the rest of the package unreachable.
fn import_music21<'py>(py: Python<'py>, workspace_root: &Path) -> PyResult<Bound<'py, PyAny>> {
    let venv = dependency_venv(workspace_root)
        .map_err(|error| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(error.to_string()))?;

    let sys = py.import("sys")?;
    let path = sys.getattr("path")?.cast_into::<PyList>()?;

    // The venv's site-packages, wherever this platform puts it.
    for relative in ["Lib/site-packages", "lib/site-packages"] {
        let candidate = venv.join(relative);
        if candidate.is_dir() {
            path.insert(0, candidate.to_string_lossy().into_owned())?;
        }
    }
    if let Ok(entries) = fs::read_dir(venv.join("lib")) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("site-packages");
            if candidate.is_dir() {
                path.insert(0, candidate.to_string_lossy().into_owned())?;
            }
        }
    }
    // The submodule, so `music21` resolves to the pinned source.
    path.insert(
        0,
        workspace_root
            .join("music21")
            .to_string_lossy()
            .into_owned(),
    )?;

    Ok(py.import("music21")?.into_any())
}

/// Formats a float the way Python's `repr` does, so fixtures stay byte-stable.
fn float_repr(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Regenerates every fixture. Returns the paths written.
pub(crate) fn regenerate(workspace_root: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    Python::attach(|py| -> PyResult<Vec<PathBuf>> {
        let music21 = import_music21(py, workspace_root)?;
        let version: String = music21.getattr("__version__")?.extract()?;
        println!("  music21 {version} imported from the submodule");

        Ok(vec![
            write_scales(py, workspace_root, &version)?,
            write_chord_types(py, workspace_root, &version)?,
            write_meters(py, workspace_root, &version)?,
            write_small_tables(py, workspace_root, &version)?,
        ])
    })
    .map_err(|error| -> Box<dyn Error> { Box::new(error) })
}

fn header(lines: &[&str], version: &str) -> String {
    let mut out = String::new();
    for line in lines {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "#");
    let _ = writeln!(out, "# music21 {version}");
    let _ = writeln!(out);
    let _ = writeln!(out, "music21_version = {}", toml_string(version));
    let _ = writeln!(out);
    out
}

fn write_scales(py: Python<'_>, workspace_root: &Path, version: &str) -> PyResult<PathBuf> {
    let scale_module = py.import("music21.scale")?;
    let pitch_module = py.import("music21.pitch")?;

    let mut out = header(
        &[
            "# Expected scale realizations, generated from music21 by",
            "# `cargo run -p xtask --features python -- regenerate-fixtures`.",
            "# Checked in so the library can be verified without importing music21.",
        ],
        version,
    );

    for (scale_type, class_name) in SCALE_TYPES {
        let _ = writeln!(out, "[[scale]]");
        let _ = writeln!(out, "scale_type = {}", toml_string(scale_type));
        let _ = writeln!(out, "music21_class = {}", toml_string(class_name));
        let _ = writeln!(out, "cases = [");
        for tonic in SCALE_TONICS {
            let scale = scale_module.getattr(class_name)?.call1((tonic,))?;
            let top: String = pitch_module
                .call_method1("Pitch", (tonic,))?
                .call_method1("transpose", ("P8",))?
                .getattr("nameWithOctave")?
                .extract()?;
            let pitches = scale.call_method1("getPitches", (tonic, top))?;
            let mut names = Vec::new();
            for pitch in pitches.try_iter()? {
                let name: String = pitch?.getattr("nameWithOctave")?.extract()?;
                names.push(toml_string(&name));
            }
            let _ = writeln!(
                out,
                "    {{ tonic = {}, pitches = [{}] }},",
                toml_string(tonic),
                names.join(", ")
            );
        }
        let _ = writeln!(out, "]");
        let _ = writeln!(out);
    }

    let path = workspace_root.join("data/scale_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({} scales x {} tonics)",
        path.display(),
        SCALE_TYPES.len(),
        SCALE_TONICS.len()
    );
    Ok(path)
}

fn write_chord_types(py: Python<'_>, workspace_root: &Path, version: &str) -> PyResult<PathBuf> {
    let harmony = py.import("music21.harmony")?;
    let chord_types = harmony.getattr("CHORD_TYPES")?.cast_into::<PyDict>()?;
    let aliases = harmony.getattr("CHORD_ALIASES")?.cast_into::<PyDict>()?;

    let mut out = header(
        &[
            "# Expected chord types, generated from music21 by",
            "# `cargo run -p xtask --features python -- regenerate-fixtures`.",
            "# The crate keeps its own copy of this table in src/chordsymbol.rs;",
            "# the parity test compares them so the hand-maintained copy cannot",
            "# drift from upstream unnoticed.",
        ],
        version,
    );

    let mut count = 0;
    for (kind, value) in chord_types.iter() {
        let kind: String = kind.extract()?;
        let notation: String = value.get_item(0)?.extract()?;
        let abbreviations: Vec<String> = value.get_item(1)?.extract()?;
        let joined: Vec<String> = abbreviations.iter().map(|a| toml_string(a)).collect();
        let _ = writeln!(out, "[[chord_type]]");
        let _ = writeln!(out, "kind = {}", toml_string(&kind));
        let _ = writeln!(out, "notation = {}", toml_string(&notation));
        let _ = writeln!(out, "abbreviations = [{}]", joined.join(", "));
        let _ = writeln!(out);
        count += 1;
    }

    let _ = writeln!(out, "[aliases]");
    let mut alias_count = 0;
    for (alias, target) in aliases.iter() {
        let alias: String = alias.extract()?;
        let target: String = target.extract()?;
        let _ = writeln!(out, "{} = {}", toml_string(&alias), toml_string(&target));
        alias_count += 1;
    }
    let _ = writeln!(out);

    let path = workspace_root.join("data/chord_type_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({count} types, {alias_count} aliases)",
        path.display()
    );
    Ok(path)
}

fn write_meters(py: Python<'_>, workspace_root: &Path, version: &str) -> PyResult<PathBuf> {
    let meter = py.import("music21.meter")?;

    let mut out = header(
        &[
            "# Expected time-signature properties, generated from music21 by",
            "# `cargo run -p xtask --features python -- regenerate-fixtures`.",
            "# The crate derives these from the numerator and denominator alone;",
            "# music21 derives them from a MeterSequence partition tree, so this",
            "# file is what proves the shortcut agrees with the tree.",
        ],
        version,
    );

    let mut count = 0;
    for denominator in METER_DENOMINATORS {
        for numerator in METER_NUMERATORS {
            let ratio = format!("{numerator}/{denominator}");
            let time_signature = meter.call_method1("TimeSignature", (ratio.as_str(),))?;

            let bar: f64 = time_signature
                .getattr("barDuration")?
                .getattr("quarterLength")?
                .extract()?;
            let beat_count: u32 = time_signature.getattr("beatCount")?.extract()?;
            let beat: f64 = time_signature
                .getattr("beatDuration")?
                .getattr("quarterLength")?
                .extract()?;
            let division_count: u32 = time_signature.getattr("beatDivisionCount")?.extract()?;
            let beat_count_name: String = time_signature.getattr("beatCountName")?.extract()?;
            let division_name: String =
                time_signature.getattr("beatDivisionCountName")?.extract()?;
            let classification: String = time_signature.getattr("classification")?.extract()?;
            let offsets: Vec<f64> = time_signature.call_method0("getBeatOffsets")?.extract()?;
            let offsets: Vec<String> = offsets.into_iter().map(float_repr).collect();

            let _ = writeln!(out, "[[meter]]");
            let _ = writeln!(out, "ratio = {}", toml_string(&ratio));
            let _ = writeln!(out, "bar_quarter_length = {}", float_repr(bar));
            let _ = writeln!(out, "beat_count = {beat_count}");
            let _ = writeln!(out, "beat_quarter_length = {}", float_repr(beat));
            let _ = writeln!(out, "beat_division_count = {division_count}");
            let _ = writeln!(out, "beat_count_name = {}", toml_string(&beat_count_name));
            let _ = writeln!(
                out,
                "beat_division_count_name = {}",
                toml_string(&division_name)
            );
            let _ = writeln!(out, "classification = {}", toml_string(&classification));
            let _ = writeln!(out, "beat_offsets = [{}]", offsets.join(", "));
            let _ = writeln!(out);
            count += 1;
        }
    }

    let path = workspace_root.join("data/meter_expectations.toml");
    fs::write(&path, out)?;
    println!("  wrote {} ({count} time signatures)", path.display());
    Ok(path)
}

fn write_small_tables(py: Python<'_>, workspace_root: &Path, version: &str) -> PyResult<PathBuf> {
    let pitch_module = py.import("music21.pitch")?;
    let key_module = py.import("music21.key")?;
    let interval_module = py.import("music21.interval")?;

    let mut out = header(
        &[
            "# Expected values for the small music21 tables the crate transcribes",
            "# by hand, generated by",
            "# `cargo run -p xtask --features python -- regenerate-fixtures`.",
            "# They are small enough to transcribe, which is exactly why they drift",
            "# silently; these fixtures are what stops that.",
        ],
        version,
    );

    let modifiers = pitch_module
        .getattr("accidentalNameToModifier")?
        .cast_into::<PyDict>()?;
    let mut accidentals = 0;
    for (name, modifier) in modifiers.iter() {
        let name: String = name.extract()?;
        let modifier: String = modifier.extract()?;
        let accidental = pitch_module.call_method1("Accidental", (name.as_str(),))?;
        let alter: f64 = accidental.getattr("alter")?.extract()?;
        let unicode: String = accidental.getattr("unicode")?.extract()?;
        let _ = writeln!(out, "[[accidental]]");
        let _ = writeln!(out, "name = {}", toml_string(&name));
        let _ = writeln!(out, "modifier = {}", toml_string(&modifier));
        let _ = writeln!(out, "alter = {}", float_repr(alter));
        let _ = writeln!(out, "unicode = {}", toml_string(&unicode));
        let _ = writeln!(out);
        accidentals += 1;
    }

    let modes = key_module
        .getattr("modeSharpsAlter")?
        .cast_into::<PyDict>()?;
    let mut mode_count = 0;
    for (mode, alter) in modes.iter() {
        let mode: String = mode.extract()?;
        let alter: i32 = alter.extract()?;
        let _ = writeln!(out, "[[mode]]");
        let _ = writeln!(out, "name = {}", toml_string(&mode));
        let _ = writeln!(out, "sharps_alter = {alter}");
        let _ = writeln!(out);
        mode_count += 1;
    }

    let prefixes: Vec<String> = interval_module.getattr("prefixSpecs")?.extract()?;
    let mut specifiers = 0;
    // prefixSpecs[0] is the ERROR sentinel.
    for prefix in prefixes.iter().skip(1) {
        for number in 1..=8u32 {
            let name = format!("{prefix}{number}");
            let Ok(interval) = interval_module.call_method1("Interval", (name.as_str(),)) else {
                continue;
            };
            let semitones: i32 = interval
                .getattr("chromatic")?
                .getattr("semitones")?
                .extract()?;
            let _ = writeln!(out, "[[specifier]]");
            let _ = writeln!(out, "prefix = {}", toml_string(prefix));
            let _ = writeln!(out, "number = {number}");
            let _ = writeln!(out, "semitones = {semitones}");
            let _ = writeln!(out);
            specifiers += 1;
        }
    }

    let discrete = py.import("music21.analysis.discrete")?;
    let mut profiles = 0;
    for class in discrete
        .getattr("keyWeightKeyAnalysisClasses")?
        .try_iter()?
    {
        let class = class?;
        let name: String = class.getattr("__name__")?.extract()?;
        let instance = class.call0()?;
        for mode in ["major", "minor"] {
            let weights: Vec<f64> = instance.call_method1("getWeights", (mode,))?.extract()?;
            let weights = weights
                .iter()
                .map(|weight| float_repr(*weight))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "[[key_profile]]");
            let _ = writeln!(out, "name = {}", toml_string(&name));
            let _ = writeln!(out, "mode = {}", toml_string(mode));
            let _ = writeln!(out, "weights = [{weights}]");
            let _ = writeln!(out);
        }
        profiles += 1;
    }

    let path = workspace_root.join("data/table_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({accidentals} accidentals, {mode_count} modes, {specifiers} specifier combos, {profiles} key profiles)",
        path.display()
    );
    Ok(path)
}
