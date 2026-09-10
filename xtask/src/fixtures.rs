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

use crate::submodule::Stamp;
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
        "  uv pip install --python .m21venv chardet joblib jsonpickle lark \\\n",
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

    // Undo the chord-table bridge's stubbing if this process has already done
    // it. That bridge installs dummy `music21`, `music21.environment` and
    // `music21.exceptions21` modules so `chord/tables.py` imports without
    // music21's dependencies, and they stay in `sys.modules` for the life of
    // the interpreter -- so `regenerate-all`, which runs the bridge first and
    // the fixtures second in one process, would otherwise import the stub
    // package here and find no `__version__` on it.
    let modules = sys.getattr("modules")?.cast_into::<PyDict>()?;
    let stubbed: Vec<String> = modules
        .keys()
        .iter()
        .filter_map(|key| key.extract::<String>().ok())
        .filter(|name| name == "music21" || name.starts_with("music21."))
        .collect();
    for name in stubbed {
        modules.del_item(name)?;
    }

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
        // The version alone spans many commits, so the exact one is recorded
        // beside it; a bump inside one version would otherwise leave every
        // fixture looking fresh.
        let commit = crate::submodule::commit(workspace_root, "music21").map_err(|error| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(error.to_string())
        })?;
        let stamp = &Stamp { version, commit };

        Ok(vec![
            write_scales(py, workspace_root, stamp)?,
            write_chord_types(py, workspace_root, stamp)?,
            write_meters(py, workspace_root, stamp)?,
            write_small_tables(py, workspace_root, stamp)?,
            write_serial(py, workspace_root, stamp)?,
            write_doctest_totals(py, workspace_root, stamp)?,
            write_harte(py, workspace_root, stamp)?,
        ])
    })
    .map_err(|error| -> Box<dyn Error> { Box::new(error) })
}

fn header(lines: &[&str], stamp: &Stamp) -> String {
    let mut out = String::new();
    for line in lines {
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "#");
    let _ = writeln!(out, "# music21 {} at {}", stamp.version, stamp.commit);
    let _ = writeln!(out);
    let _ = writeln!(out, "music21_version = {}", toml_string(&stamp.version));
    let _ = writeln!(out, "music21_commit = {}", toml_string(&stamp.commit));
    let _ = writeln!(out);
    out
}

/// The music21 modules the crate ports, as import names, read out of
/// `data/feature_map.toml` so the two cannot drift. `chord/__init__.py` is
/// `music21.chord`, `meter/base.py` is `music21.meter.base`.
pub(crate) fn scoped_modules(workspace_root: &Path) -> PyResult<Vec<String>> {
    let text = fs::read_to_string(workspace_root.join("data/feature_map.toml"))?;
    let mut modules = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("python") else {
            continue;
        };
        let Some((_, value)) = rest.split_once('=') else {
            continue;
        };
        let path = value.trim().trim_matches(['\'', '"'].as_slice());
        let Some(stem) = path.strip_suffix(".py") else {
            continue;
        };
        let stem = stem.strip_suffix("/__init__").unwrap_or(stem);
        let module = format!("music21.{}", stem.replace('/', "."));
        if !modules.contains(&module) {
            modules.push(module);
        }
    }
    modules.sort();
    Ok(modules)
}

/// The modules holding a module's unit tests, found by looking rather than by
/// guessing at a name. music21 keeps them three ways — a `Test` class in the
/// module itself, a `tests.py` or `test_*.py` beside it inside its own
/// package, and `music21/test/test_<name>.py` — and a name guess gets it
/// wrong: `music21.meter.base` would take `music21/test/test_base.py`, which
/// is the test of `music21.base` and counts 43 tests that are nothing to do
/// with meter.
pub(crate) fn test_modules_for(workspace_root: &Path, module: &str) -> Vec<String> {
    let root = workspace_root.join("music21/music21");
    let Some(path) = module.strip_prefix("music21.") else {
        return Vec::new();
    };
    let segments: Vec<&str> = path.split('.').collect();
    let mut found = Vec::new();

    // A package of its own — `music21/scale`, `music21/meter` — keeps its
    // tests inside it.
    let package = (segments.len() > 1 || root.join(segments[0]).is_dir()).then_some(segments[0]);
    if let Some(package) = package
        && let Ok(entries) = fs::read_dir(root.join(package))
    {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_str()?.to_string();
                let stem = name.strip_suffix(".py")?.to_string();
                (stem == "tests" || stem.starts_with("test_"))
                    .then(|| format!("music21.{package}.{stem}"))
            })
            .collect();
        names.sort();
        found.extend(names);
    }

    // And `music21/test/` holds the rest, named after the top-level module.
    let sidecar = root.join("test").join(format!("test_{}.py", segments[0]));
    if sidecar.is_file() {
        found.push(format!("music21.test.test_{}", segments[0]));
    }
    found
}

/// How much documentation each module in scope has, counted by music21's own
/// `DocTestFinder` — the same collection the parity harness runs, so the two
/// agree on the denominator.
///
/// The report needs this for the modules the harness does *not* cover yet: it
/// reads the passing counts out of what the harness wrote, and without a total
/// from somewhere it could only leave those modules off the page, which
/// flatters the score. There is no counting this from the Rust side, because a
/// docstring is what `DocTestFinder` says it is — a text scan for `>>>` misses
/// by as much as 14% on `roman.py`.
fn write_doctest_totals(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let doctest = py.import("doctest")?;
    let finder = doctest.getattr("DocTestFinder")?.call0()?;
    let unittest = py.import("unittest")?;
    let loader = unittest.getattr("TestLoader")?.call0()?;
    let test_case = unittest.getattr("TestCase")?;

    // music21's own runner loads a module's `Test` class; `TestExternal`
    // needs a score viewer and is not counted here.
    let count_tests = |module: &Bound<'_, PyAny>| -> PyResult<usize> {
        let Ok(case) = module.getattr("Test") else {
            return Ok(0);
        };
        let Ok(case) = case.cast_into::<pyo3::types::PyType>() else {
            return Ok(0);
        };
        if !case.is_subclass(&test_case)? {
            return Ok(0);
        }
        loader
            .call_method1("loadTestsFromTestCase", (case,))?
            .call_method0("countTestCases")?
            .extract()
    };

    let mut out = header(
        &[
            "# How many doctests each music21 module the crate ports has, counted by",
            "# music21's own DocTestFinder and generated by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# The report reads these as the denominator for a module the parity",
            "# harness does not cover yet, which is how such a module shows as 0 of N",
            "# rather than being left off the page.",
            "#",
            "# `tests` counts music21's own unit tests for the module, wherever it",
            "# keeps them. It is a denominator of last resort: `xtask music21-suite`",
            "# runs those tests and records how each module fared, and the report",
            "# reads that. This is what the report falls back to where no such run",
            "# has happened.",
        ],
        stamp,
    );

    let modules = scoped_modules(workspace_root)?;
    for name in &modules {
        let module = py.import(name.as_str())?;
        let tests = finder.call_method1("find", (&module,))?;
        let mut docstrings = 0usize;
        let mut examples = 0usize;
        for test in tests.try_iter()? {
            let count = test?.getattr("examples")?.len()?;
            if count > 0 {
                docstrings += 1;
                examples += count;
            }
        }
        let mut tests = count_tests(&module)?;
        for test_module in test_modules_for(workspace_root, name) {
            let Ok(imported) = py.import(test_module.as_str()) else {
                continue;
            };
            tests += count_tests(&imported)?;
        }

        let _ = writeln!(out, "[[module]]");
        let _ = writeln!(out, "module = {}", toml_string(name));
        let _ = writeln!(out, "docstrings = {docstrings}");
        let _ = writeln!(out, "examples = {examples}");
        let _ = writeln!(out, "tests = {tests}");
        let _ = writeln!(out);
    }

    let path = workspace_root.join("data/doctest_totals.toml");
    fs::write(&path, out)?;
    println!("  wrote {} ({} modules)", path.display(), modules.len());
    Ok(path)
}

/// What harte-library, on music21, makes of every chord label in its own
/// coverage set: the pitches, root, bass, sounding degrees and prettified
/// spelling of each, or the exception it raises. `src/harte.rs` is checked
/// against it by `harte_parity`.
fn write_harte(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let checkout = crate::downstream::checkout(&workspace_root.join("target/downstream"))
        .map_err(|error| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(error.to_string()))?;
    let sys = py.import("sys")?;
    sys.getattr("path")?
        .cast_into::<PyList>()?
        .insert(0, checkout.to_string_lossy().into_owned())?;
    let harte = py.import("harte.harte")?.getattr("Harte")?;
    let counts = fs::read_to_string(checkout.join("test/chords_count.json"))?;
    let counts = py
        .import("json")?
        .call_method1("loads", (counts,))?
        .cast_into::<PyDict>()?;
    let mut labels: Vec<String> = counts
        .keys()
        .iter()
        .map(|key| key.extract::<String>())
        .collect::<PyResult<_>>()?;
    labels.sort();

    let mut out = header(
        &[
            "# What harte-library makes of every label in its coverage set,",
            "# generated on music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# Degrees inside the parentheses of `pretty` are sorted, as the",
            "# library writes them in no fixed order.",
        ],
        stamp,
    );
    let _ = writeln!(
        out,
        "harte_commit = {}",
        toml_string(crate::downstream::COMMIT)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "chord = [");

    let names_of = |values: Bound<'_, PyAny>, attribute: Option<&str>| -> PyResult<String> {
        let mut names = Vec::new();
        for value in values.try_iter()? {
            let value = value?;
            let name: String = match attribute {
                Some(attribute) => value.getattr(attribute)?.extract()?,
                None => value.extract()?,
            };
            names.push(toml_string(&name));
        }
        Ok(format!("[{}]", names.join(", ")))
    };

    let mut errors = 0;
    for label in &labels {
        let _ = write!(out, "    {{ label = {}", toml_string(label));
        match harte.call1((label.as_str(),)) {
            Ok(chord) if chord.getattr("_root")?.is_none() => {
                let _ = writeln!(
                    out,
                    ", pitches = [], degrees = [], pretty = {} }},",
                    toml_string(label)
                );
            }
            Ok(chord) => {
                let pitches = names_of(chord.getattr("pitches")?, Some("nameWithOctave"))?;
                let root: String = chord
                    .call_method0("root")?
                    .getattr("nameWithOctave")?
                    .extract()?;
                let bass: String = chord
                    .call_method0("bass")?
                    .getattr("nameWithOctave")?
                    .extract()?;
                // Read before `prettify`, which takes the root out of the list.
                let degrees = names_of(chord.getattr("_all_degrees")?, None)?;
                let pretty: String = chord.call_method0("prettify")?.extract()?;
                let _ = writeln!(
                    out,
                    ", pitches = {pitches}, root = {}, bass = {}, degrees = {degrees}, pretty = {} }},",
                    toml_string(&root),
                    toml_string(&bass),
                    toml_string(&sort_parenthesised(&pretty)),
                );
            }
            Err(error) => {
                errors += 1;
                let _ = writeln!(
                    out,
                    ", error = {} }},",
                    toml_string(error.get_type(py).name()?.to_str()?)
                );
            }
        }
    }
    let _ = writeln!(out, "]");

    let path = workspace_root.join("data/harte_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({} labels, {errors} the library refuses)",
        path.display(),
        labels.len()
    );
    Ok(path)
}

/// `C:maj7(#11,9)` with the degrees in its parentheses sorted.
fn sort_parenthesised(label: &str) -> String {
    let Some((head, rest)) = label.split_once('(') else {
        return label.to_string();
    };
    let Some((inside, tail)) = rest.split_once(')') else {
        return label.to_string();
    };
    let mut degrees: Vec<&str> = inside.split(',').collect();
    degrees.sort_unstable();
    format!("{head}({}){tail}", degrees.join(","))
}

fn write_scales(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let scale_module = py.import("music21.scale")?;
    let pitch_module = py.import("music21.pitch")?;

    let mut out = header(
        &[
            "# Expected scale realizations, generated from music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# Checked in so the library can be verified without importing music21.",
        ],
        stamp,
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

fn write_chord_types(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let harmony = py.import("music21.harmony")?;
    let chord_types = harmony.getattr("CHORD_TYPES")?.cast_into::<PyDict>()?;
    let aliases = harmony.getattr("CHORD_ALIASES")?.cast_into::<PyDict>()?;

    let mut out = header(
        &[
            "# Expected chord types, generated from music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# The crate keeps its own copy of this table in src/chordsymbol.rs;",
            "# the parity test compares them so the hand-maintained copy cannot",
            "# drift from upstream unnoticed.",
        ],
        stamp,
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

fn write_meters(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let meter = py.import("music21.meter")?;

    let mut out = header(
        &[
            "# Expected time-signature properties, generated from music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# The crate derives these from the numerator and denominator alone;",
            "# music21 derives them from a MeterSequence partition tree, so this",
            "# file is what proves the shortcut agrees with the tree.",
        ],
        stamp,
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
            let mut divisions = Vec::new();
            for division in time_signature
                .getattr("beatDivisionDurations")?
                .try_iter()?
            {
                let quarter_length: f64 = division?.getattr("quarterLength")?.extract()?;
                divisions.push(float_repr(quarter_length));
            }
            let offsets: Vec<String> = offsets.into_iter().map(float_repr).collect();
            let mut accent_weights = Vec::new();
            let mut beat_depths = Vec::new();
            let mut accent_partition = 0.0;
            let mut position = 0.0;
            for partition in time_signature.getattr("accentSequence")?.try_iter()? {
                let partition = partition?;
                let weight: f64 = partition.getattr("weight")?.extract()?;
                accent_partition = partition
                    .getattr("duration")?
                    .getattr("quarterLength")?
                    .extract()?;
                accent_weights.push(float_repr(weight));
                let depth: u32 = time_signature
                    .call_method1("getBeatDepth", (position,))?
                    .extract()?;
                beat_depths.push(depth.to_string());
                position += accent_partition;
            }

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
            let _ = writeln!(
                out,
                "beat_division_quarter_lengths = [{}]",
                divisions.join(", ")
            );
            let _ = writeln!(
                out,
                "accent_partition_quarter_length = {}",
                float_repr(accent_partition)
            );
            let _ = writeln!(out, "accent_weights = [{}]", accent_weights.join(", "));
            let _ = writeln!(out, "beat_depths = [{}]", beat_depths.join(", "));
            let _ = writeln!(out);
            count += 1;
        }
    }

    let path = workspace_root.join("data/meter_expectations.toml");
    fs::write(&path, out)?;
    println!("  wrote {} ({count} time signatures)", path.display());
    Ok(path)
}

fn write_small_tables(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let pitch_module = py.import("music21.pitch")?;
    let key_module = py.import("music21.key")?;
    let interval_module = py.import("music21.interval")?;

    let mut out = header(
        &[
            "# Expected values for the small music21 tables the crate transcribes",
            "# by hand, generated by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# They are small enough to transcribe, which is exactly why they drift",
            "# silently; these fixtures are what stops that.",
        ],
        stamp,
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
            let nice_name: String = interval.getattr("niceName")?.extract()?;
            let _ = writeln!(out, "semitones = {semitones}");
            let _ = writeln!(out, "nice_name = {}", toml_string(&nice_name));
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

    let tempo = py.import("music21.tempo")?;
    let tempo_values = tempo.getattr("defaultTempoValues")?.cast_into::<PyDict>()?;
    let mut tempo_words = 0;
    for (name, number) in tempo_values.iter() {
        let name: String = name.extract()?;
        let number: f64 = number.extract()?;
        let _ = writeln!(out, "[[tempo]]");
        let _ = writeln!(out, "name = {}", toml_string(&name));
        let _ = writeln!(out, "number = {}", float_repr(number));
        let _ = writeln!(out);
        tempo_words += 1;
    }

    let scale_module = py.import("music21.scale")?;
    let concrete = scale_module.getattr("ConcreteScale")?;
    let mut solfeg_rows = 0;
    for (variant, attribute) in [
        ("music21", "_solfegSyllables"),
        ("humdrum", "_humdrumSolfegSyllables"),
    ] {
        let table = concrete.getattr(attribute)?.cast_into::<PyDict>()?;
        for (degree, syllables) in table.iter() {
            let degree: u32 = degree.extract()?;
            let syllables = syllables.cast_into::<PyDict>()?;
            let mut ordered = Vec::new();
            for alter in -2..=2 {
                let syllable: String = syllables
                    .get_item(alter)?
                    .ok_or_else(|| {
                        pyo3::exceptions::PyKeyError::new_err(format!(
                            "{attribute}[{degree}] has no entry for {alter}"
                        ))
                    })?
                    .extract()?;
                ordered.push(toml_string(&syllable));
            }
            let _ = writeln!(out, "[[solfeg]]");
            let _ = writeln!(out, "variant = {}", toml_string(variant));
            let _ = writeln!(out, "degree = {degree}");
            let _ = writeln!(out, "syllables = [{}]", ordered.join(", "));
            let _ = writeln!(out);
            solfeg_rows += 1;
        }
    }

    let roman = py.import("music21.roman")?;
    let scores = roman
        .getattr("functionalityScores")?
        .cast_into::<PyDict>()?;
    let mut score_rows = 0;
    for (figure, score) in scores.iter() {
        let figure: String = figure.extract()?;
        let score: u32 = score.extract()?;
        let _ = writeln!(out, "[[functionality]]");
        let _ = writeln!(out, "figure = {}", toml_string(&figure));
        let _ = writeln!(out, "score = {score}");
        let _ = writeln!(out);
        score_rows += 1;
    }

    let path = workspace_root.join("data/table_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({accidentals} accidentals, {mode_count} modes, {specifiers} specifier combos, {profiles} key profiles, {tempo_words} tempo words, {solfeg_rows} solfeg rows, {score_rows} functionality scores)",
        path.display()
    );
    Ok(path)
}

fn pitch_classes_from_intervals(intervals: &str) -> Vec<u32> {
    let mut pitch_classes = vec![0u32];
    for interval in intervals.chars() {
        let step = match interval {
            'T' => 10,
            'E' => 11,
            digit => digit
                .to_digit(10)
                .expect("link interval strings are 0-9, T or E"),
        };
        let last = *pitch_classes.last().expect("seeded with 0");
        pitch_classes.push((last + step) % 12);
    }
    pitch_classes
}

fn write_link_classification(
    out: &mut String,
    row: &Bound<'_, PyAny>,
    key_prefix: &str,
) -> PyResult<()> {
    let (classification, special): (Option<u32>, Vec<String>) =
        row.call_method0("getLinkClassification")?.extract()?;
    if let Some(classification) = classification {
        let _ = writeln!(out, "{key_prefix}classification = {classification}");
    }
    let special = special
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "{key_prefix}special_intervals = [{special}]");
    Ok(())
}

fn write_serial(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let serial = py.import("music21.serial")?;

    let mut out = header(
        &[
            "# Expected tone-row values, generated from music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# The historical rows are transcribed into src/serial/tables.rs; the Link chord",
            "# table is checked behaviourally, by classifying a row built from each",
            "# of music21's interval strings.",
        ],
        stamp,
    );

    let historical = serial.getattr("historicalDict")?.cast_into::<PyDict>()?;
    let mut historical_count = 0;
    for (name, _) in historical.iter() {
        let name: String = name.extract()?;
        let row = serial.call_method1("getHistoricalRowByName", (name.as_str(),))?;
        let composer: String = row.getattr("composer")?.extract()?;
        let opus: Option<String> = row.getattr("opus")?.extract()?;
        let title: String = row.getattr("title")?.extract()?;
        let pitch_classes: Vec<u32> = row.call_method0("pitchClasses")?.extract()?;
        let intervals: String = row.call_method0("getIntervalsAsString")?.extract()?;
        let all_interval: bool = row.call_method0("isAllInterval")?.extract()?;
        let matrix: String = row.call_method0("matrix")?.str()?.extract()?;
        let _ = writeln!(out, "[[historical_row]]");
        let _ = writeln!(out, "name = {}", toml_string(&name));
        let _ = writeln!(out, "composer = {}", toml_string(&composer));
        if let Some(opus) = opus {
            let _ = writeln!(out, "opus = {}", toml_string(&opus));
        }
        let _ = writeln!(out, "title = {}", toml_string(&title));
        let _ = writeln!(
            out,
            "pitch_classes = [{}]",
            pitch_classes
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = writeln!(out, "intervals = {}", toml_string(&intervals));
        let _ = writeln!(out, "is_all_interval = {all_interval}");
        write_link_classification(&mut out, &row, "link_")?;
        let matrix = matrix
            .lines()
            .map(toml_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "matrix = [{matrix}]");
        let _ = writeln!(out);
        historical_count += 1;
    }

    let code = serial
        .getattr("TwelveToneRow")?
        .getattr("getLinkClassification")?
        .getattr("__code__")?;
    let mut link_count = 0;
    for constant in code.getattr("co_consts")?.try_iter()? {
        let Ok(strings) = constant?.extract::<Vec<String>>() else {
            continue;
        };
        if strings.is_empty() || strings.iter().any(|value| value.len() != 11) {
            continue;
        }
        for intervals in &strings {
            let pitch_classes = pitch_classes_from_intervals(intervals);
            let row = serial.call_method1("pcToToneRow", (pitch_classes.clone(),))?;
            let _ = writeln!(out, "[[link_chord]]");
            let _ = writeln!(out, "intervals = {}", toml_string(intervals));
            let _ = writeln!(
                out,
                "pitch_classes = [{}]",
                pitch_classes
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            write_link_classification(&mut out, &row, "")?;
            let _ = writeln!(out);
            link_count += 1;
        }
    }

    let path = workspace_root.join("data/serial_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({historical_count} historical rows, {link_count} link chords)",
        path.display()
    );
    Ok(path)
}
