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

/// Meters written in parts, part by part and with their numerators summed,
/// over one denominator and over several: the spellings a numerator and a
/// denominator alone cannot tell apart.
const METERS_WRITTEN_IN_PARTS: &[&str] = &[
    "3+2/8",
    "3/8+2/8",
    "2+3/8",
    "2/8+3/8",
    "3+2+2/8",
    "3/8+2/8+2/8",
    "2+2+3/8",
    "3+3+2/8",
    "3+2+3/8",
    "2+2+2+3/8",
    "2+2+3/16",
    "3/16+2/16",
    "2+2+3/4",
    "2/4+3/8",
    "2/4+1/8",
    "3/4+3/8",
    "4/4+3/8",
    "1/4+2/8",
    "2/8+3/16",
    "3+2+5/8+3/4",
];

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
            write_accidental_display(py, workspace_root, stamp)?,
            write_chord_names(py, workspace_root, stamp)?,
            write_chord_symbols(py, workspace_root, stamp)?,
            write_roman_figures(py, workspace_root, stamp)?,
            write_voice_leading(py, workspace_root, stamp)?,
            write_instruments(py, workspace_root, stamp)?,
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
/// What music21's `updateAccidentalDisplay` decides across a grid of
/// situations: the pitch, what came before it in this measure and the
/// last, what sounds with it, the key, its accidental's display type and
/// the cautionary switches. `Pitch::update_accidental_display` is checked
/// against it by `accidental_display_parity`.
fn write_accidental_display(
    py: Python<'_>,
    workspace_root: &Path,
    stamp: &Stamp,
) -> PyResult<PathBuf> {
    let pitch_class = py.import("music21.pitch")?.getattr("Pitch")?;
    let accidental_class = py.import("music21.pitch")?.getattr("Accidental")?;
    let signature_class = py.import("music21.key")?.getattr("KeySignature")?;

    const CURRENTS: [&str; 10] = [
        "F4", "F#4", "F-4", "Fn4", "G4", "G#4", "B-4", "B4", "C#5", "F#5",
    ];
    const PASTS: [&[&str]; 13] = [
        &[],
        &["F#4"],
        &["F4"],
        &["F#5"],
        &["Fn4"],
        &["F#4", "F4"],
        &["F#4", "F#4"],
        &["G4", "F#4", "A4"],
        &["F#4", "G4"],
        &["B-4"],
        &["F-4"],
        &["F#4", "F#5"],
        &["Fn4", "F#4"],
    ];
    const PAST_MEASURES: [&[&str]; 6] = [&[], &["F#4"], &["F4"], &["F#5"], &["B-3"], &["Fn4"]];
    const SIMULTANEOUS: [&[&str]; 5] = [&[], &["F4"], &["F#4"], &["F#5"], &["G4"]];
    const SHARPS: [i32; 4] = [0, 1, -2, 6];
    const DISPLAY_TYPES: [Option<&str>; 6] = [
        None,
        Some("always"),
        Some("never"),
        Some("even-tied"),
        Some("if-absolutely-necessary"),
        Some("unless-repeated"),
    ];
    // cautionaryPitchClass, cautionaryAll, overrideStatus,
    // cautionaryNotImmediateRepeat, lastNoteWasTied.
    const DEFAULT_FLAGS: [bool; 5] = [true, false, false, true, false];
    const FLAG_SETS: [[bool; 5]; 6] = [
        DEFAULT_FLAGS,
        [false, false, false, true, false],
        [true, true, false, true, false],
        [true, false, true, true, false],
        [true, false, false, false, false],
        [true, false, false, true, true],
    ];

    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct Case {
        current: &'static str,
        past: &'static [&'static str],
        past_measure: &'static [&'static str],
        simultaneous: &'static [&'static str],
        sharps: i32,
        display_type: Option<&'static str>,
        flags: [bool; 5],
    }
    let mut cases: Vec<Case> = Vec::new();
    let mut add = |current, past, past_measure, simultaneous, sharps, display_type, flags| {
        let case = Case {
            current,
            past,
            past_measure,
            simultaneous,
            sharps,
            display_type,
            flags,
        };
        if !cases.contains(&case) {
            cases.push(case);
        }
    };
    // Every pitch against every past, in every key, with the defaults.
    for current in CURRENTS {
        for past in PASTS {
            for sharps in SHARPS {
                add(current, past, &[], &[], sharps, None, DEFAULT_FLAGS);
            }
        }
    }
    // The previous measure and simultaneities, and the switches, on a
    // smaller grid.
    for current in &CURRENTS[..6] {
        for past_measure in PAST_MEASURES {
            for sharps in [0, 1] {
                add(current, &[], past_measure, &[], sharps, None, DEFAULT_FLAGS);
                add(
                    current,
                    &["G4"],
                    past_measure,
                    &[],
                    sharps,
                    None,
                    DEFAULT_FLAGS,
                );
            }
        }
        for simultaneous in SIMULTANEOUS {
            add(current, &[], &[], simultaneous, 0, None, DEFAULT_FLAGS);
            add(current, &[], &[], simultaneous, 0, None, FLAG_SETS[1]);
        }
        for past in &PASTS[..8] {
            for display_type in DISPLAY_TYPES {
                add(current, past, &[], &[], 0, display_type, DEFAULT_FLAGS);
                add(current, past, &["F#4"], &[], 1, display_type, DEFAULT_FLAGS);
            }
        }
        for past in &PASTS[..9] {
            for flags in FLAG_SETS {
                add(current, past, &[], &[], 0, None, flags);
                add(current, past, &[], &[], 1, None, flags);
                add(current, past, &["F#4"], &[], 0, None, flags);
            }
        }
    }

    let pitches = |names: &[&str]| -> PyResult<Vec<Bound<'_, PyAny>>> {
        names
            .iter()
            .map(|name| pitch_class.call1((*name,)))
            .collect()
    };
    let list = |names: &[&str]| -> String {
        format!(
            "[{}]",
            names
                .iter()
                .map(|name| toml_string(name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    let mut out = header(
        &[
            "# What music21's updateAccidentalDisplay decides for a grid of",
            "# situations, generated by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# `accidental` is the accidental the pitch carries afterwards, `none`",
            "# where it carries none, and `shown` its display status where one was",
            "# decided.",
        ],
        stamp,
    );
    let _ = writeln!(out, "case = [");
    for case in &cases {
        let pitch = pitch_class.call1((case.current,))?;
        if let Some(display_type) = case.display_type {
            if pitch.getattr("accidental")?.is_none() {
                pitch.setattr("accidental", accidental_class.call1(("natural",))?)?;
            }
            pitch
                .getattr("accidental")?
                .setattr("displayType", display_type)?;
        }
        let signature = signature_class.call1((case.sharps,))?;
        let keywords = PyDict::new(py);
        keywords.set_item("pitchPast", pitches(case.past)?)?;
        keywords.set_item("pitchPastMeasure", pitches(case.past_measure)?)?;
        keywords.set_item("otherSimultaneousPitches", pitches(case.simultaneous)?)?;
        keywords.set_item("alteredPitches", signature.getattr("alteredPitches")?)?;
        keywords.set_item("cautionaryPitchClass", case.flags[0])?;
        keywords.set_item("cautionaryAll", case.flags[1])?;
        keywords.set_item("overrideStatus", case.flags[2])?;
        keywords.set_item("cautionaryNotImmediateRepeat", case.flags[3])?;
        keywords.set_item("lastNoteWasTied", case.flags[4])?;
        pitch.call_method("updateAccidentalDisplay", (), Some(&keywords))?;
        let accidental = pitch.getattr("accidental")?;
        let (name, shown): (String, Option<bool>) = if accidental.is_none() {
            ("none".to_string(), None)
        } else {
            (
                accidental.getattr("name")?.extract()?,
                accidental.getattr("displayStatus")?.extract()?,
            )
        };
        let shown = match shown {
            Some(shown) => format!(", shown = {shown}"),
            None => String::new(),
        };
        let display_type = match case.display_type {
            Some(display_type) => format!(", display_type = {}", toml_string(display_type)),
            None => String::new(),
        };
        let _ = writeln!(
            out,
            "    {{ pitch = {}, past = {}, past_measure = {}, simultaneous = {}, sharps = {}{display_type}, \
             cautionary_pitch_class = {}, cautionary_all = {}, override_status = {}, \
             cautionary_not_immediate_repeat = {}, last_note_was_tied = {}, accidental = {}{shown} }},",
            toml_string(case.current),
            list(case.past),
            list(case.past_measure),
            list(case.simultaneous),
            case.sharps,
            case.flags[0],
            case.flags[1],
            case.flags[2],
            case.flags[3],
            case.flags[4],
            toml_string(&name),
        );
    }
    let _ = writeln!(out, "]");

    let path = workspace_root.join("data/accidental_display_expectations.toml");
    fs::write(&path, out)?;
    println!("  wrote {} ({} cases)", path.display(), cases.len());
    Ok(path)
}

/// A chord symbol figure with the pitches after `add` and after `omit` in
/// sorted order. music21 writes each group out of a set, in no fixed order.
pub(crate) fn sort_figure_modifications(figure: &str) -> String {
    let (head, rest) = match figure.find("add").or_else(|| figure.find("omit")) {
        Some(at) => figure.split_at(at),
        None => return figure.to_string(),
    };
    let mut out = head.to_string();
    let mut rest = rest;
    while !rest.is_empty() {
        let (marker, after) = if let Some(after) = rest.strip_prefix("add") {
            ("add", after)
        } else if let Some(after) = rest.strip_prefix("omit") {
            ("omit", after)
        } else {
            out.push_str(rest);
            break;
        };
        let end = after
            .find("add")
            .into_iter()
            .chain(after.find("omit"))
            .min()
            .unwrap_or(after.len());
        let mut items: Vec<&str> = after[..end]
            .split(',')
            .filter(|item| !item.is_empty())
            .collect();
        items.sort_unstable();
        out.push_str(marker);
        out.push_str(&items.join(","));
        rest = &after[end..];
        if !rest.is_empty() {
            out.push(',');
        }
    }
    out
}

/// A Python exception's class name, for a fixture to record where music21
/// refuses an input.
fn exception_name(py: Python<'_>, error: &PyErr) -> String {
    error
        .get_type(py)
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "Exception".to_string())
}

/// A TOML list of strings.
fn toml_list(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|item| toml_string(item))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The names with octaves of an object's `pitches`.
fn pitch_names(object: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    object
        .getattr("pitches")?
        .try_iter()?
        .map(|pitch| pitch?.getattr("nameWithOctave")?.extract())
        .collect()
}

/// What music21 calls a set of chords: the common and pitched names, the
/// quality, the Forte class, the inversion and the chord symbol of every
/// Forte prime form, of a list of spelled chords, and of chords built from
/// integers. `chord_name_parity` checks the crate's names against it.
fn write_chord_names(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let chord_module = py.import("music21.chord")?;
    let harmony = py.import("music21.harmony")?;
    let chord_class = chord_module.getattr("Chord")?;

    const SPELLED: [&str; 62] = [
        "C E G",
        "C E- G",
        "C E G#",
        "C E- G-",
        "C E G B-",
        "C E G B",
        "C E- G B-",
        "C E- G- B--",
        "C E- G- B-",
        "C E G B- D",
        "C E G B D",
        "C E- G B- D",
        "C E G A",
        "C E- G A",
        "C E G# B",
        "G B D F A",
        "G B D F A C",
        "G B D F A C E",
        "C D G",
        "C F G",
        "C G",
        "C4 C5",
        "C4 C5 C6",
        "C4 B#4",
        "C4 D--4",
        "C4 E4 E5",
        "C4 G4",
        "C4 F#4",
        "C4 C#4",
        "C4 D4",
        "C4 E4 G4 C5 E5",
        "E4 G4 C5",
        "G3 C4 E4",
        "C4",
        "C4 C4",
        "C4 E4",
        "C4 E-4",
        "C4 A4",
        "C4 E4 G4 A4",
        "D F# A C",
        "F A C E",
        "B D F A",
        "B D F A-",
        "C E- G- A",
        "A- C E- F#",
        "A- C D F#",
        "A- C E- G-",
        "C D E F G A B",
        "C D E F# G# A#",
        "C C# D D# E F F# G G# A A# B",
        "C# E- G",
        "C# E# G B",
        "C D F# A-",
        "C# E- G A",
        "C E F# A#",
        "D E G# B-",
        "E- F# A",
        "C# G A#",
        "C# D# F# A#",
        "C# E# G# A#",
        "E- G- A- C-",
        "C# E# F# A#",
    ];
    const SPELLED_TOO: [&str; 8] = [
        "E- F- A- C-",
        "E- G- B- C-",
        "E- F# A B",
        "F# A C E-",
        "C E G B- D F A",
        "C4 E4 G4 B-4 D5 F5 A5",
        "C4 E4 G4 B4 D5 F#5 A5",
        "C4 E4 G4 B4 D5 F#5",
    ];
    const INTEGERS: [&[i32]; 12] = [
        &[0, 4, 8],
        &[0, 3, 6, 9],
        &[0, 2, 6, 8],
        &[0, 1, 4, 6],
        &[0, 1, 2],
        &[0, 4, 7],
        &[0, 3, 7],
        &[0, 2, 4, 6, 8, 10],
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        &[60, 64, 67],
        &[2, 4, 8, 10],
        &[1, 3, 7, 10],
    ];
    // How many Forte classes each cardinality has, before inversion.
    const CLASSES_PER_CARDINALITY: [usize; 12] = [1, 6, 12, 29, 38, 50, 38, 29, 12, 6, 1, 1];

    #[derive(Clone)]
    enum Source {
        Forte(String),
        Spelled(&'static str),
        Integers(&'static [i32]),
    }
    let mut sources = Vec::new();
    for (index, count) in CLASSES_PER_CARDINALITY.iter().enumerate() {
        let cardinality = index + 1;
        for number in 1..=*count {
            for inversion in ["", "A", "B"] {
                sources.push(Source::Forte(format!("{cardinality}-{number}{inversion}")));
            }
        }
    }
    sources.extend(SPELLED.iter().map(|spelled| Source::Spelled(spelled)));
    sources.extend(SPELLED_TOO.iter().map(|spelled| Source::Spelled(spelled)));
    sources.extend(INTEGERS.iter().map(|integers| Source::Integers(integers)));

    let mut out = header(
        &[
            "# What music21 calls a chord: its common and pitched common names, its",
            "# quality, Forte class, inversion and chord symbol, for every Forte prime",
            "# form, a list of spelled chords and some chords built from integers.",
            "# Generated by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# A field music21 raises on is left out.",
        ],
        stamp,
    );
    let _ = writeln!(out, "chord = [");
    let mut written = 0;
    for source in &sources {
        let (label, built) = match source {
            Source::Forte(name) => (
                format!("forte = {}", toml_string(name)),
                chord_module.call_method1("fromForteClass", (name.as_str(),)),
            ),
            Source::Spelled(spelled) => (
                format!("pitches = {}", toml_string(spelled)),
                chord_class.call1((*spelled,)),
            ),
            Source::Integers(integers) => (
                format!(
                    "integers = [{}]",
                    integers
                        .iter()
                        .map(|each| each.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                chord_class.call1((integers.to_vec(),)),
            ),
        };
        let Ok(chord) = built else {
            // A Forte class with no B form, which is most of them.
            continue;
        };
        let text = |attribute: &str| -> PyResult<String> { chord.getattr(attribute)?.extract() };
        let common_name = text("commonName")?;
        let pitched = text("pitchedCommonName")?;
        let quality = text("quality")?;
        let forte_class = text("forteClass")?;
        let inversion = chord
            .call_method0("inversion")
            .ok()
            .and_then(|value| value.extract::<i64>().ok())
            .map(|value| format!(", inversion = {value}"))
            .unwrap_or_default();
        let inversion_name = chord
            .call_method0("inversionName")
            .ok()
            .filter(|value| !value.is_none())
            .and_then(|value| value.extract::<i64>().ok())
            .map(|value| format!(", inversion_name = {value}"))
            .unwrap_or_default();
        // Read before the figure, which fixes a root on the chord it reads.
        let root: String = chord.call_method0("root")?.getattr("name")?.extract()?;
        let bass: String = chord.call_method0("bass")?.getattr("name")?.extract()?;
        let figure: String = harmony
            .call_method1("chordSymbolFigureFromChord", (&chord,))?
            .extract()?;
        let figure = sort_figure_modifications(&figure);
        let _ = writeln!(
            out,
            "    {{ {label}, common_name = {}, pitched_common_name = {}, quality = {}, forte_class = {}{inversion}{inversion_name}, figure = {}, root = {}, bass = {} }},",
            toml_string(&common_name),
            toml_string(&pitched),
            toml_string(&quality),
            toml_string(&forte_class),
            toml_string(&figure),
            toml_string(&root),
            toml_string(&bass),
        );
        written += 1;
    }
    let _ = writeln!(out, "]");

    let path = workspace_root.join("data/chord_name_expectations.toml");
    fs::write(&path, out)?;
    println!("  wrote {} ({written} chords)", path.display());
    Ok(path)
}

/// What music21 realizes a chord symbol figure as: every abbreviation of
/// every kind on several roots, with bass notes, additions, omissions and
/// alterations on some. `chord_symbol_parity` checks the crate's parser
/// against it.
fn write_chord_symbols(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let harmony = py.import("music21.harmony")?;
    let symbol_class = harmony.getattr("ChordSymbol")?;
    let chord_types = harmony.getattr("CHORD_TYPES")?.cast_into::<PyDict>()?;
    let mut abbreviations: Vec<String> = Vec::new();
    for (_, value) in chord_types.iter() {
        let list = value.get_item(1)?;
        for abbreviation in list.try_iter()? {
            abbreviations.push(abbreviation?.extract()?);
        }
    }
    const ROOTS: [&str; 3] = ["C", "F#", "B-"];
    let mut figures: Vec<String> = Vec::new();
    for root in ROOTS {
        for abbreviation in &abbreviations {
            figures.push(format!("{root}{abbreviation}"));
        }
    }
    for abbreviation in &abbreviations {
        figures.push(format!("C{abbreviation}/E"));
        figures.push(format!("C{abbreviation}/G"));
    }
    for figure in [
        "Cpower",
        "Cpedal",
        "C/E",
        "C/G",
        "Cm/E-",
        "C7/B-",
        "Cmaj7/B",
        "N6",
        "It+6",
        "Fr+6",
        "Ger+6",
        "tristan",
        "C+11",
        "C#9",
        "C7b5",
        "C7#5",
        "C9#11",
        "C13b9",
        "Csus",
        "Csus2",
        "Csus4",
        "Cdim",
        "Cdim7",
        "Cm7b5",
        "Cø7",
        "C°7",
        "CM7",
        "Cmaj9",
        "Cm9",
        "Cm11",
        "C13",
        "C6",
        "Cm6",
        "Cadd2",
        "Cadd11",
        "Cmaj7#11",
        "C7sus4",
        "C-7",
        "C-M7",
        "Cb",
        "Cbm",
        "C#",
        "C#dim",
        "C##",
        "B#",
        "Fb",
        "E#7",
        "Cm7 add 11",
        "C omit 5",
        "C7 omit 3",
        "C7add9",
        "Cm7add11",
        "Cmaj7add13",
        "C7omit5",
        "Cm7omit5",
        "C9omit3",
        "C7b9",
        "C7#9",
        "C7#11",
        "C7b13",
        "C9b5",
        "Cm9b5",
        "C7#5b9",
        "Cadd#4",
        "Caddb6",
        "C7add#11",
        "Dm7b5",
        "Gm7b5/D-",
        "Am7b5",
        "F#7b9",
        "B-7#11",
        "E-maj7#5",
        "A-m6",
        "C#m7/E",
        "B7/D#",
        "F/A",
        "Fm/A-",
        "G7/F",
    ] {
        figures.push(figure.to_string());
    }
    figures.sort();
    figures.dedup();

    let mut out = header(
        &[
            "# What music21's ChordSymbol makes of a figure: the pitches it sounds,",
            "# its kind, root and bass, or the exception it raises. Generated by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
        ],
        stamp,
    );
    let _ = writeln!(out, "symbol = [");
    let mut errors = 0;
    for figure in &figures {
        let _ = write!(out, "    {{ figure = {}", toml_string(figure));
        match symbol_class.call1((figure.as_str(),)) {
            Ok(symbol) => {
                let pitches = pitch_names(&symbol)?;
                let kind: String = symbol.getattr("chordKind")?.extract()?;
                let root: String = symbol.call_method0("root")?.getattr("name")?.extract()?;
                let bass: String = symbol.call_method0("bass")?.getattr("name")?.extract()?;
                let _ = writeln!(
                    out,
                    ", pitches = {}, kind = {}, root = {}, bass = {} }},",
                    toml_list(&pitches),
                    toml_string(&kind),
                    toml_string(&root),
                    toml_string(&bass),
                );
            }
            Err(error) => {
                errors += 1;
                let _ = writeln!(
                    out,
                    ", error = {} }},",
                    toml_string(&exception_name(py, &error))
                );
            }
        }
    }
    let _ = writeln!(out, "]");

    let path = workspace_root.join("data/chord_symbol_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({} figures, {errors} music21 refuses)",
        path.display(),
        figures.len()
    );
    Ok(path)
}

/// What music21 realizes a roman numeral figure as, in several keys: the
/// pitches, the numeral read off it, its degree and inversion, or the
/// exception it raises. `roman_figure_parity` checks the crate against it.
fn write_roman_figures(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let roman_class = py.import("music21.roman")?.getattr("RomanNumeral")?;
    let key_class = py.import("music21.key")?.getattr("Key")?;
    const FIGURES: [&str; 118] = [
        "I",
        "i",
        "ii",
        "iio",
        "ii°",
        "iiø7",
        "iii",
        "III",
        "III+",
        "IV",
        "iv",
        "V",
        "v",
        "V7",
        "V65",
        "V43",
        "V42",
        "V9",
        "V7b9",
        "V11",
        "V13",
        "vi",
        "VI",
        "vii",
        "VII",
        "viio",
        "viio7",
        "viiø7",
        "viio6",
        "viio65",
        "viio43",
        "viio42",
        "I6",
        "I64",
        "i6",
        "i64",
        "IV6",
        "IV64",
        "V6",
        "V64",
        "ii6",
        "ii65",
        "ii7",
        "ii43",
        "ii42",
        "IV7",
        "I7",
        "IM7",
        "i7",
        "V+",
        "V+6",
        "bII",
        "bII6",
        "N6",
        "N",
        "It6",
        "It",
        "It+6",
        "Ger65",
        "Ger",
        "Ger7",
        "Ger6/5",
        "Fr43",
        "Fr",
        "Fr4/3",
        "Sw43",
        "Sw",
        "V/V",
        "V7/V",
        "V65/V",
        "viio7/V",
        "V/ii",
        "V/vi",
        "V7/IV",
        "V42/IV",
        "ii/V",
        "Cad64",
        "I[no3]",
        "V[no5]",
        "I[add9]",
        "V7[add4]",
        "I[no5][add9]",
        "V#5",
        "V7#5",
        "Vb5",
        "V7b5",
        "I#7",
        "i#7",
        "#ivo7",
        "#iv",
        "bVII",
        "bVI",
        "bIII",
        "iv6",
        "iv64",
        "IIø65",
        "IIø7",
        "Iø",
        "vii°7",
        "I9",
        "V7[add6]",
        "bIIb6",
        "I53",
        "I63",
        "I5",
        "V75",
        "V73",
        "V7[no3]",
        "I[#5]",
        "I[b3]",
        "ii[#3]",
        "IV[add#4]",
        "V[addb9]",
        "V6#5",
        "#viio7",
        "bviio",
        "V/bVII",
        "vi/vi",
    ];
    const KEYS: [&str; 6] = ["C", "c", "F#", "b-", "E-", "g"];

    let mut out = header(
        &[
            "# What music21's RomanNumeral realizes a figure as, in several keys:",
            "# the pitches, the numeral read back off it, its degree and inversion,",
            "# or the exception it raises. Generated by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
        ],
        stamp,
    );
    let _ = writeln!(out, "numeral = [");
    let mut errors = 0;
    for key_name in KEYS {
        let key = key_class.call1((key_name,))?;
        for figure in FIGURES {
            let _ = write!(
                out,
                "    {{ figure = {}, key = {}",
                toml_string(figure),
                toml_string(key_name)
            );
            match roman_class.call1((figure, &key)) {
                Ok(numeral) => {
                    let pitches = pitch_names(&numeral)?;
                    let roman: String = numeral.getattr("romanNumeral")?.extract()?;
                    let degree: i64 = numeral.getattr("scaleDegree")?.extract()?;
                    let inversion = numeral
                        .call_method0("inversion")
                        .ok()
                        .and_then(|value| value.extract::<i64>().ok())
                        .map(|value| format!(", inversion = {value}"))
                        .unwrap_or_default();
                    let _ = writeln!(
                        out,
                        ", pitches = {}, roman_numeral = {}, degree = {degree}{inversion} }},",
                        toml_list(&pitches),
                        toml_string(&roman),
                    );
                }
                Err(error) => {
                    errors += 1;
                    let _ = writeln!(
                        out,
                        ", error = {} }},",
                        toml_string(&exception_name(py, &error))
                    );
                }
            }
        }
    }
    let _ = writeln!(out, "]");

    let path = workspace_root.join("data/roman_figure_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({} figures x {} keys, {errors} music21 refuses)",
        path.display(),
        FIGURES.len(),
        KEYS.len()
    );
    Ok(path)
}

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
            "# library writes them in no fixed order, and degrees that share a",
            "# sort key stand in spelling order, as the library breaks that tie",
            "# by set order.",
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
                let root: String = chord
                    .call_method0("root")?
                    .getattr("nameWithOctave")?
                    .extract()?;
                let bass: String = chord
                    .call_method0("bass")?
                    .getattr("nameWithOctave")?
                    .extract()?;
                // Read before `prettify`, which takes the root out of the list.
                // The library sorts its degrees by a key that ties `b3` with
                // `bb3` and breaks the tie by set order, which changes from
                // one run to the next; the fixture breaks it by spelling.
                let mut pairs: Vec<(String, String)> = Vec::new();
                for (degree, pitch) in chord
                    .getattr("_all_degrees")?
                    .try_iter()?
                    .zip(chord.getattr("pitches")?.try_iter()?)
                {
                    pairs.push((
                        degree?.extract()?,
                        pitch?.getattr("nameWithOctave")?.extract()?,
                    ));
                }
                pairs.sort_by(|a, b| {
                    harte_degree_sort_key(&a.0)
                        .partial_cmp(&harte_degree_sort_key(&b.0))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
                let toml_list = |items: Vec<String>| {
                    format!(
                        "[{}]",
                        items
                            .iter()
                            .map(|item| toml_string(item))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                let degrees = toml_list(pairs.iter().map(|(degree, _)| degree.clone()).collect());
                let pitches = toml_list(pairs.iter().map(|(_, pitch)| pitch.clone()).collect());
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

/// Where a Harte degree sorts: by its number, a flat just below it and a
/// sharp just above, which is harte-library's `degree_to_sort_key`.
fn harte_degree_sort_key(degree: &str) -> f64 {
    let number: f64 = degree
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0.0);
    if degree.starts_with('b') {
        number - 0.49
    } else if degree.starts_with('#') {
        number + 0.49
    } else {
        number
    }
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

/// The corpus scores the voice-leading walk is checked over. The chorale is
/// music21's own example; the Schoenberg has chords and rests in it, and the
/// Haydn has notes written with a natural sign, which music21 holds unequal
/// to the same note written without one.
const VOICE_LEADING_SCORES: [&str; 3] = [
    "bwv66.6",
    "schoenberg/opus19/movement6",
    "haydn/opus1no1/movement1",
];

/// music21's keyword arguments to `iterateAllVoiceLeadingQuartets`, in the
/// order `includeRests`, `includeOblique`, `includeNoMotion`.
const VOICE_LEADING_OPTIONS: [(bool, bool, bool); 4] = [
    (true, true, false),
    (false, true, false),
    (true, false, false),
    (false, true, true),
];

/// A pitch spelled so that a written natural survives: music21 keeps `Dn`
/// apart from `D`, and `nameWithOctave` writes both as `D4`.
fn spelled_pitch(pitch: &Bound<'_, PyAny>) -> PyResult<String> {
    let step: String = pitch.getattr("step")?.extract()?;
    let accidental = pitch.getattr("accidental")?;
    let modifier: String = if accidental.is_none() {
        String::new()
    } else if accidental.getattr("alter")?.extract::<f64>()? == 0.0 {
        "n".to_string()
    } else {
        accidental.getattr("modifier")?.extract()?
    };
    let octave: i32 = pitch.getattr("octave")?.extract()?;
    Ok(format!("{step}{modifier}{octave}"))
}

/// Strings `fromString` is asked beyond the names in its tables: the ones its
/// own docstring and tests read, and a few that exercise the choosing between
/// several matches and the transposition that follows.
const INSTRUMENT_LOOKUPS: [&str; 22] = [
    "Contrabassoon",
    "Clarinet in B-flat",
    "Clarinetto in Si b",
    "Klarinette in B.",
    "Clarinet in A",
    "Horn in F",
    "Horn",
    "Trumpet in Bb",
    "Bb Piccolo Trumpet",
    "Trumpet in D",
    "Cl.",
    "Cl",
    "Vln. I",
    "Violino I",
    "Violoncello",
    "Voice",
    "Flûte",
    "Electric Piano",
    "Acoustic Grand Piano",
    "Soprano Saxophone in B-flat",
    "Bass Clarinet in H",
    "kazoo concerto",
];

/// Every instrument music21 has a class for, its MIDI programs, the tables
/// `fromString` reads names out of and what it makes of them.
fn write_instruments(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    let instrument = py.import("music21.instrument")?;
    let lookup = py.import("music21.languageExcerpts.instrumentLookup")?;
    let inspect = py.import("inspect")?;
    let base_class = instrument.getattr("Instrument")?;

    let mut out = header(
        &[
            "# Expected instruments, generated from music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "# The instruments and the name tables are transcribed into",
            "# src/instrument/tables.rs; the lookups are checked behaviourally.",
        ],
        stamp,
    );

    // A top-level key, so written before the first table.
    let ensembles: Vec<String> = instrument.getattr("ensembleNamesBySize")?.extract()?;
    let _ = writeln!(out, "ensemble_names = {}", toml_list(&ensembles));
    let _ = writeln!(out);

    // Every class, parents before children, so a transcription can read them
    // in order.
    let mut classes: Vec<(usize, String, Bound<'_, PyAny>)> = Vec::new();
    for member in inspect
        .call_method1("getmembers", (&instrument, inspect.getattr("isclass")?))?
        .try_iter()?
    {
        let (name, class): (String, Bound<'_, PyAny>) = member?.extract()?;
        if !class.is_instance_of::<pyo3::types::PyType>()
            || !class
                .cast::<pyo3::types::PyType>()?
                .is_subclass(&base_class)?
        {
            continue;
        }
        let depth = class.getattr("__mro__")?.len()?;
        classes.push((depth, name, class));
    }
    classes.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    for (_, name, class) in &classes {
        let made = class.call0()?;
        let mut parents = Vec::new();
        for ancestor in class.getattr("__mro__")?.try_iter()?.skip(1) {
            let ancestor = ancestor?;
            let ancestor_name: String = ancestor.getattr("__name__")?.extract()?;
            if ancestor_name == "Music21Object" {
                break;
            }
            parents.push(ancestor_name);
        }
        let text = |attribute: &str| -> PyResult<Option<String>> {
            let value = made.getattr(attribute)?;
            if value.is_none() {
                Ok(None)
            } else {
                Ok(Some(value.str()?.extract()?))
            }
        };
        let pitch = |attribute: &str| -> PyResult<Option<String>> {
            let value = made.getattr(attribute)?;
            if value.is_none() {
                Ok(None)
            } else {
                Ok(Some(value.getattr("nameWithOctave")?.extract()?))
            }
        };
        let _ = writeln!(out, "[[instrument]]");
        let _ = writeln!(out, "class = {}", toml_string(name));
        let _ = writeln!(out, "parents = {}", toml_list(&parents));
        for (key, attribute) in [
            ("name", "instrumentName"),
            ("abbreviation", "instrumentAbbreviation"),
            ("sound", "instrumentSound"),
            ("best_name", "bestName"),
        ] {
            let value = if attribute == "bestName" {
                let best = made.call_method0("bestName")?;
                if best.is_none() {
                    None
                } else {
                    Some(best.extract::<String>()?)
                }
            } else {
                text(attribute)?
            };
            if let Some(value) = value {
                let _ = writeln!(out, "{key} = {}", toml_string(&value));
            }
        }
        for (key, attribute) in [
            ("midi_program", "midiProgram"),
            ("midi_channel", "midiChannel"),
            ("percussion_pitch", "percMapPitch"),
        ] {
            // `percMapPitch` exists only on percussion.
            let Ok(value) = made.getattr(attribute) else {
                continue;
            };
            if !value.is_none() {
                let _ = writeln!(out, "{key} = {}", value.extract::<i64>()?);
            }
        }
        for (key, attribute) in [("lowest", "lowestNote"), ("highest", "highestNote")] {
            if let Some(value) = pitch(attribute)? {
                let _ = writeln!(out, "{key} = {}", toml_string(&value));
            }
        }
        let transposition = made.getattr("transposition")?;
        if !transposition.is_none() {
            let _ = writeln!(
                out,
                "transposition = {}",
                toml_string(&transposition.getattr("directedName")?.extract::<String>()?)
            );
        }
        let _ = writeln!(
            out,
            "percussion_map = {}",
            made.getattr("inGMPercMap")?.extract::<bool>()?
        );
        // What `getAllNamesForInstrument` answers, language by language.
        let all_names = instrument.call_method1("getAllNamesForInstrument", (&made,))?;
        let _ = writeln!(out, "[instrument.all_names]");
        for item in all_names.call_method0("items")?.try_iter()? {
            let (language, names): (String, Vec<String>) = item?.extract()?;
            let _ = writeln!(out, "{} = {}", toml_string(&language), toml_list(&names));
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "[midi_program]");
    for program in 0..128_i64 {
        match instrument.call_method1("instrumentFromMidiProgram", (program,)) {
            Ok(made) => {
                let class: String = made.get_type().getattr("__name__")?.extract()?;
                let _ = writeln!(
                    out,
                    "{} = {}",
                    toml_string(&program.to_string()),
                    toml_string(&class)
                );
            }
            Err(_) => continue,
        }
    }
    let _ = writeln!(out);

    let languages = [
        "english",
        "french",
        "german",
        "italian",
        "russian",
        "spanish",
        "abbreviation",
    ];
    for language in languages {
        let table = lookup
            .getattr(format!("{language}ToClassName"))?
            .cast_into::<PyDict>()?;
        let _ = writeln!(out, "[names.{language}]");
        let mut rows: Vec<(String, String)> = table
            .iter()
            .map(|(k, v)| Ok((k.extract()?, v.extract()?)))
            .collect::<PyResult<_>>()?;
        rows.sort();
        for (key, class) in rows {
            let _ = writeln!(out, "{} = {}", toml_string(&key), toml_string(&class));
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "[pitch_names]");
    let pitch_names = lookup
        .getattr("pitchFullNameToName")?
        .cast_into::<PyDict>()?;
    let mut rows: Vec<(String, String)> = pitch_names
        .iter()
        .map(|(k, v)| Ok((k.extract()?, v.extract()?)))
        .collect::<PyResult<_>>()?;
    rows.sort();
    for (key, value) in rows {
        let _ = writeln!(out, "{} = {}", toml_string(&key), toml_string(&value));
    }
    let _ = writeln!(out);
    let transpositions = lookup.getattr("transposition")?.cast_into::<PyDict>()?;
    let mut rows: Vec<(String, Bound<'_, PyAny>)> = transpositions
        .iter()
        .map(|(k, v)| Ok((k.extract()?, v)))
        .collect::<PyResult<_>>()?;
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    for (instrument_name, table) in rows {
        let _ = writeln!(out, "[transpositions.{}]", toml_string(&instrument_name));
        let mut entries: Vec<(String, String)> = table
            .cast_into::<PyDict>()?
            .iter()
            .map(|(k, v)| Ok((k.extract()?, v.extract()?)))
            .collect::<PyResult<_>>()?;
        entries.sort();
        for (pitch, interval) in entries {
            let _ = writeln!(out, "{} = {}", toml_string(&pitch), toml_string(&interval));
        }
        let _ = writeln!(out);
    }

    // What `fromString` makes of every name in every table, in that table's
    // language and across all of them, and of the strings above.
    let mut asked: Vec<(String, String)> = Vec::new();
    for language in languages {
        let table = lookup
            .getattr(format!("{language}ToClassName"))?
            .cast_into::<PyDict>()?;
        for (key, _) in table.iter() {
            let key: String = key.extract()?;
            asked.push((key.clone(), language.to_string()));
            asked.push((key, "all".to_string()));
        }
    }
    for text in INSTRUMENT_LOOKUPS {
        asked.push((text.to_string(), "all".to_string()));
    }
    asked.sort();
    asked.dedup();
    for (text, language) in asked {
        let _ = writeln!(out, "[[lookup]]");
        let _ = writeln!(out, "text = {}", toml_string(&text));
        let _ = writeln!(out, "language = {}", toml_string(&language));
        match instrument.call_method1("fromString", (text.as_str(), language.as_str())) {
            Ok(made) => {
                let class: String = made.get_type().getattr("__name__")?.extract()?;
                let _ = writeln!(out, "class = {}", toml_string(&class));
                let transposition = made.getattr("transposition")?;
                if !transposition.is_none() {
                    let _ = writeln!(
                        out,
                        "transposition = {}",
                        toml_string(&transposition.getattr("directedName")?.extract::<String>()?)
                    );
                }
            }
            Err(error) => {
                let _ = writeln!(out, "error = {}", toml_string(&exception_name(py, &error)));
            }
        }
        let _ = writeln!(out);
    }

    let path = workspace_root.join("data/instrument_expectations.toml");
    fs::write(&path, out)?;
    Ok(path)
}

/// Every quartet music21 finds in a few corpus scores, beside the notes of
/// each part, so the crate's walk can be checked with no music21 to hand.
fn write_voice_leading(py: Python<'_>, workspace_root: &Path, stamp: &Stamp) -> PyResult<PathBuf> {
    // A corpus score is read back from a pickle, and one written while these
    // classes were installed names `music21_rs` in it.
    crate::music21_suite::clear_corpus_cache(py)?;
    let corpus = py.import("music21.corpus")?;
    let voice_leading = py.import("music21.voiceLeading")?;
    let note_class = py.import("music21.note")?.getattr("Note")?;
    let rest_class = py.import("music21.note")?.getattr("Rest")?;
    let chord_class = py.import("music21.chord")?.getattr("Chord")?;

    let mut out = header(
        &[
            "# Expected voice-leading quartets, generated from music21 by",
            "# `cargo run --release -p xtask --features python -- regenerate-fixtures`.",
            "#",
            "# Each part is its flattened elements as `offset|quarterLength|kind|what`:",
            "# `n` a note, `c` a chord, `r` a rest, and `o` anything else, with its",
            "# classSortOrder, since a thing standing where a note starts can come",
            "# before it. A quartet is `v1n1 v1n2|v2n1 v2n2`. music21 orders the voices",
            "# within an offset by when each object was inserted, so both sides are",
            "# sorted before they are compared.",
        ],
        stamp,
    );

    let mut total = 0;
    for name in VOICE_LEADING_SCORES {
        let score = corpus.call_method1("parse", (name,))?;
        let _ = writeln!(out, "[[score]]");
        let _ = writeln!(out, "name = {}", toml_string(name));
        let _ = writeln!(out, "parts = [");
        for part in score.getattr("parts")?.try_iter()? {
            let flat = part?.call_method0("flatten")?;
            let mut elements = Vec::new();
            for element in flat.try_iter()? {
                let element = element?;
                let offset: f64 = flat
                    .call_method1("elementOffset", (&element,))?
                    .call_method0("__float__")?
                    .extract()?;
                let length: f64 = element
                    .getattr("duration")?
                    .getattr("quarterLength")?
                    .call_method0("__float__")?
                    .extract()?;
                let (kind, what) = if element.is_instance(&note_class)? {
                    ("n", spelled_pitch(&element.getattr("pitch")?)?)
                } else if element.is_instance(&chord_class)? {
                    let mut pitches = Vec::new();
                    for pitch in element.getattr("pitches")?.try_iter()? {
                        pitches.push(spelled_pitch(&pitch?)?);
                    }
                    ("c", pitches.join(" "))
                } else if element.is_instance(&rest_class)? {
                    ("r", String::new())
                } else {
                    let order: f64 = element.getattr("classSortOrder")?.extract()?;
                    ("o", float_repr(order))
                };
                elements.push(toml_string(&format!(
                    "{}|{}|{kind}|{what}",
                    float_repr(offset),
                    float_repr(length)
                )));
            }
            let _ = writeln!(out, "    [{}],", elements.join(", "));
        }
        let _ = writeln!(out, "]");
        for (rests, oblique, no_motion) in VOICE_LEADING_OPTIONS {
            let keywords = PyDict::new(py);
            keywords.set_item("includeRests", rests)?;
            keywords.set_item("includeOblique", oblique)?;
            keywords.set_item("includeNoMotion", no_motion)?;
            let mut quartets = Vec::new();
            for quartet in voice_leading
                .getattr("iterateAllVoiceLeadingQuartets")?
                .call((&score,), Some(&keywords))?
                .try_iter()?
            {
                let quartet = quartet?;
                let mut names = Vec::new();
                for voice in ["v1n1", "v1n2", "v2n1", "v2n2"] {
                    let name: String = quartet
                        .getattr(voice)?
                        .getattr("nameWithOctave")?
                        .extract()?;
                    names.push(name);
                }
                quartets.push(toml_string(&format!(
                    "{} {}|{} {}",
                    names[0], names[1], names[2], names[3]
                )));
            }
            total += quartets.len();
            let _ = writeln!(out, "[[score.run]]");
            let _ = writeln!(out, "include_rests = {rests}");
            let _ = writeln!(out, "include_oblique = {oblique}");
            let _ = writeln!(out, "include_no_motion = {no_motion}");
            let _ = writeln!(out, "quartets = [{}]", quartets.join(", "));
        }
        let _ = writeln!(out);
    }

    let path = workspace_root.join("data/voice_leading_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({} scores, {total} quartets)",
        path.display(),
        VOICE_LEADING_SCORES.len()
    );
    Ok(path)
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

    // A meter written in parts has beats of more than one length, so the
    // questions above -- which all assume one -- have no answer for it, and
    // music21 raises. What it does have is its four sequences, and those are
    // where writing a meter one way rather than another shows.
    for written in METERS_WRITTEN_IN_PARTS {
        let time_signature = meter.call_method1("TimeSignature", (*written,))?;
        let summed: bool = time_signature.getattr("summedNumerator")?.extract()?;
        let _ = writeln!(out, "[[written]]");
        let _ = writeln!(out, "ratio = {}", toml_string(written));
        let _ = writeln!(out, "summed_numerator = {summed}");
        for (key, attribute) in [
            ("display", "displaySequence"),
            ("beat", "beatSequence"),
            ("beam", "beamSequence"),
            ("accent", "accentSequence"),
        ] {
            let sequence = time_signature.getattr(attribute)?;
            let text: String = sequence.str()?.extract()?;
            let mut weights = Vec::new();
            for terminal in sequence.call_method0("flatten")?.try_iter()? {
                let weight: f64 = terminal?.getattr("weight")?.extract()?;
                weights.push(float_repr(weight));
            }
            let _ = writeln!(out, "{key} = {}", toml_string(&text));
            let _ = writeln!(out, "{key}_weights = [{}]", weights.join(", "));
        }
        let _ = writeln!(out);
    }

    let path = workspace_root.join("data/meter_expectations.toml");
    fs::write(&path, out)?;
    println!(
        "  wrote {} ({count} time signatures, {} written in parts)",
        path.display(),
        METERS_WRITTEN_IN_PARTS.len()
    );
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

    // Every mark music21 lists, niente, and a few it has no loudness for,
    // which it reads without their `s` and `z`.
    let dynamics = py.import("music21.dynamics")?;
    let mut marks: Vec<String> = dynamics.getattr("shortNames")?.extract()?;
    marks.extend(
        ["n", "sfz", "fz", "rfz", "sffz", "sfp", "xyz"]
            .iter()
            .map(|mark| (*mark).to_string()),
    );
    let mut dynamic_marks = 0;
    for mark in &marks {
        let dynamic = dynamics.call_method1("Dynamic", (mark,))?;
        let scalar: f64 = dynamic.getattr("volumeScalar")?.extract()?;
        let long_name: Option<String> = dynamic.getattr("longName")?.extract()?;
        let english_name: Option<String> = dynamic.getattr("englishName")?.extract()?;
        let _ = writeln!(out, "[[dynamic]]");
        let _ = writeln!(out, "mark = {}", toml_string(mark));
        let _ = writeln!(out, "volume_scalar = {}", float_repr(scalar));
        let _ = writeln!(
            out,
            "long_name = {}",
            toml_string(long_name.as_deref().unwrap_or(""))
        );
        let _ = writeln!(
            out,
            "english_name = {}",
            toml_string(english_name.as_deref().unwrap_or(""))
        );
        let _ = writeln!(out);
        dynamic_marks += 1;
    }
    // The mark each loudness falls under, a twentieth at a time and on each
    // side of nought and one.
    for step in -1..=21 {
        let value = f64::from(step) / 20.0;
        let mark: String = dynamics
            .call_method1("dynamicStrFromDecimal", (value,))?
            .extract()?;
        let _ = writeln!(out, "[[dynamic_decimal]]");
        let _ = writeln!(out, "value = {}", float_repr(value));
        let _ = writeln!(out, "mark = {}", toml_string(&mark));
        let _ = writeln!(out);
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
        "  wrote {} ({accidentals} accidentals, {mode_count} modes, {specifiers} specifier combos, {profiles} key profiles, {tempo_words} tempo words, {dynamic_marks} dynamics, {solfeg_rows} solfeg rows, {score_rows} functionality scores)",
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
