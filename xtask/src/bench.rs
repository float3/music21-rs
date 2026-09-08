//! Time `music21_rs` against `music21`, operation by operation.
//!
//! Both are called through the same Python API with the same inputs, so what
//! is measured is the whole cost a caller pays: the interpreter's, the FFI's
//! and the implementation's. Every case is checked for agreement before it is
//! timed — a case whose two sides answer differently is reported and not
//! timed, since timing two different computations says nothing.
//!
//! music21 caches analysis on the object that was asked, so most cases build
//! a fresh object each iteration: that is the cost of *doing* the analysis.
//! The cases marked "cached" reuse one object on purpose, to show what
//! music21's memoization buys once it is warm.
//!
//! ```text
//! cargo run --release -p xtask --features python -- bench
//! cargo run --release -p xtask --features python -- bench --json out.json
//! ```
//!
//! The subject is the *installed wheel*, imported by name, exactly as the
//! Python this replaces imported it: what is measured has to be what a caller
//! would get from `pip install`.
//!
//! Two things this shape asks for that the Python got for free:
//!
//! - **Everything a case needs is resolved once, when the case is prepared.**
//!   The class, the argument string and every attribute name are Python
//!   objects held from then on, so a timed iteration calls and reads an
//!   attribute — the same work `m21pitch.Pitch('C#4').nameWithOctave` does in
//!   a Python loop. Resolving any of it inside the loop instead adds a fixed
//!   cost to both sides, and a fixed cost added to both sides drags every
//!   ratio towards 1: an early draft of this looked up `builtins.list` on
//!   every iteration and reported the chord cases at parity when they are not.
//! - **Each case is written once and run against both sides.** music21 keeps
//!   its classes one module per kind and the wheel is one flat namespace,
//!   which [`Side`] is what stands between. The Python this replaces spelled
//!   every case out twice, once per side, with nothing keeping the two
//!   spellings the same question.

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use music21_rs::{Chord as RsChord, Interval as RsInterval, Note as RsNote, Pitch as RsPitch};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};
use serde::Serialize;

/// The chords every chord-analysis case is asked about.
const CHORDS: [&str; 5] = [
    "C4 E4 G4",
    "D3 F#3 A3 C4",
    "B-2 D3 F3 A-3",
    "F#4 A4 C#5 E5",
    "C4 E-4 G-4 A4",
];

/// A prepared call: one side of one case, ready to be timed. Everything it
/// needs is already a Python object it holds.
type Thunk = Box<dyn Fn(Python<'_>) -> PyResult<Py<PyAny>>>;

/// Prepares one side of a case against the classes that side provides.
type Build = Box<dyn Fn(Python<'_>, &Side) -> PyResult<Thunk>>;

/// The same operation against the crate itself, with no Python in the way.
///
/// The wheel is what a caller installs, and timing it measures the whole cost
/// a caller pays — the interpreter's, the binding's and the crate's. This is
/// the other half of that pair: where the two differ is what the binding
/// costs, and where they agree the cost is the crate's own.
///
/// It answers a string, so it is held to the same rule as the other two: a
/// case is timed only once every side agrees, and what it answers is compared
/// against Python's `str()` of music21's answer. A case with no clean
/// counterpart has none, and its column reads as unmeasured rather than as
/// nought.
type Native = fn() -> String;

/// One row of the JSON `report` reads back, and of the table printed here.
#[derive(Debug, Serialize)]
struct Timing {
    case: String,
    group: String,
    notes: String,
    music21_ns: f64,
    /// The wheel, which is what a caller installs.
    music21_rs_ns: f64,
    /// The crate with no Python in the way, where the case has a counterpart.
    #[serde(skip_serializing_if = "Option::is_none")]
    music21_rs_native_ns: Option<f64>,
    speedup: f64,
}

#[derive(Debug, Serialize)]
struct Written {
    music21: String,
    python: String,
    platform: String,
    results: Vec<Timing>,
}

/// Where one side's classes live.
///
/// music21 spreads them across `music21.pitch`, `music21.note` and the rest;
/// the wheel is one flat `music21_rs`. Naming a class rather than a module is
/// what lets a case be written once. Only ever read while a case is being
/// prepared — a timed iteration holds what it needs directly.
struct Side {
    /// What to call this side in the output.
    label: &'static str,
    classes: HashMap<&'static str, Py<PyAny>>,
    /// Python's own `list`, held here so a case that needs it need not go
    /// looking during a timed loop.
    list: Py<PyAny>,
    /// Python's own `round`, for the same reason.
    round: Py<PyAny>,
}

impl Side {
    /// The named class or function, as this side provides it.
    fn get<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.classes
            .get(name)
            .map(|value| value.bind(py).clone())
            .ok_or_else(|| {
                pyo3::exceptions::PyAttributeError::new_err(format!("{} has no {name}", self.label))
            })
    }
}

/// One operation, and how to prepare it on either side.
struct Case {
    name: String,
    group: &'static str,
    notes: &'static str,
    build: Build,
    native: Option<Native>,
}

impl Case {
    fn new(name: impl Into<String>, group: &'static str, build: Build) -> Self {
        Self {
            name: name.into(),
            group,
            notes: "",
            build,
            native: None,
        }
    }

    /// The same operation written against the crate directly.
    fn natively(mut self, native: Native) -> Self {
        self.native = Some(native);
        self
    }

    fn noting(mut self, notes: &'static str) -> Self {
        self.notes = notes;
        self
    }
}

/// What one chord is asked. The eight chord-analysis cases differ only here.
#[derive(Clone, Copy)]
enum Ask {
    /// An attribute of the chord.
    Attribute(&'static str),
    /// An attribute, wrapped in `list(...)`: one side answers a tuple and the
    /// other a list, and in Python those are never equal.
    AttributeList(&'static str),
    /// A method taking no arguments.
    Method(&'static str),
    /// A method taking no arguments, then an attribute of what it answered.
    MethodAttribute(&'static str, &'static str),
}

/// One chord question, with its names already looked up.
type Question = Box<dyn Fn(Python<'_>, &Bound<'_, PyAny>) -> PyResult<Py<PyAny>>>;

impl Ask {
    /// How this question is written in the case's name.
    fn label(self) -> String {
        match self {
            Ask::Attribute(name) | Ask::AttributeList(name) => name.to_string(),
            Ask::Method(name) | Ask::MethodAttribute(name, _) => format!("{name}()"),
        }
    }

    fn prepare(self, py: Python<'_>, side: &Side) -> Question {
        match self {
            Ask::Attribute(name) => {
                let name = interned(py, name);
                Box::new(move |py, chord| Ok(chord.getattr(name.bind(py))?.unbind()))
            }
            Ask::AttributeList(name) => {
                let name = interned(py, name);
                let list = side.list.clone_ref(py);
                Box::new(move |py, chord| {
                    let value = chord.getattr(name.bind(py))?;
                    Ok(list.bind(py).call1((value,))?.unbind())
                })
            }
            Ask::Method(name) => {
                let name = interned(py, name);
                Box::new(move |py, chord| Ok(chord.call_method0(name.bind(py))?.unbind()))
            }
            Ask::MethodAttribute(name, attribute) => {
                let name = interned(py, name);
                let attribute = interned(py, attribute);
                Box::new(move |py, chord| {
                    Ok(chord
                        .call_method0(name.bind(py))?
                        .getattr(attribute.bind(py))?
                        .unbind())
                })
            }
        }
    }
}

/// Runs the whole comparison, answering the process exit code.
pub fn run(workspace_root: &Path, json: Option<PathBuf>) -> Result<i32, Box<dyn Error>> {
    let seconds = 0.4_f64;
    let repeats = 5_usize;

    Python::attach(|py| -> PyResult<i32> {
        add_dependency_venv(py, workspace_root)?;
        add_submodule_fallback(py, workspace_root)?;
        let platform = py.import("platform")?;
        let music21 = py.import("music21")?;
        let ours = py.import("music21_rs")?;

        let python_version: String = platform.call_method0("python_version")?.extract()?;
        let platform_name: String = platform.call_method0("platform")?.extract()?;
        let music21_version: String = music21.getattr("__version__")?.extract()?;
        println!("python {python_version} on {platform_name}");
        println!("music21 {music21_version}  vs  music21_rs (release build)\n");

        let theirs = music21_side(py)?;
        let mine = our_side(py, &ours)?;

        let mut results: Vec<Timing> = Vec::new();
        let mut disagreed: Vec<(String, String, String)> = Vec::new();
        let mut group: Option<&str> = None;

        for case in cases() {
            let slow_call = (case.build)(py, &theirs)?;
            let fast_call = (case.build)(py, &mine)?;
            let left = slow_call(py)?;
            let right = fast_call(py)?;
            if !left.bind(py).eq(right.bind(py))? {
                disagreed.push((
                    case.name.clone(),
                    left.bind(py).repr()?.extract()?,
                    right.bind(py).repr()?.extract()?,
                ));
                continue;
            }
            let slow = measure(py, &slow_call, seconds, repeats)?;
            let fast = measure(py, &fast_call, seconds, repeats)?;
            // The crate on its own, but only where it answers the same thing.
            // A case whose native counterpart disagrees is left unmeasured
            // rather than timed against a different question.
            let native = case.native.and_then(|native| {
                let answer = native();
                let expected = left.bind(py).str().ok()?.extract::<String>().ok()?;
                (answer == expected).then(|| measure_native(native, seconds, repeats))
            });
            if group != Some(case.group) {
                group = Some(case.group);
                println!(
                    "{:<38} {:>11} {:>11} {:>11} {:>9}",
                    case.group.to_uppercase(),
                    "music21",
                    "crate",
                    "wheel",
                    "speedup"
                );
            }
            println!(
                "  {:<36} {:>11} {:>11} {:>11} {:>8.1}x",
                case.name,
                humanise(slow),
                native.map_or_else(|| "—".to_string(), humanise),
                humanise(fast),
                slow / fast
            );
            results.push(Timing {
                case: case.name.clone(),
                group: case.group.to_string(),
                notes: case.notes.to_string(),
                music21_ns: slow,
                music21_rs_ns: fast,
                music21_rs_native_ns: native,
                speedup: slow / fast,
            });
        }

        if !results.is_empty() {
            let mut speedups: Vec<f64> = results.iter().map(|row| row.speedup).collect();
            speedups.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            println!(
                "\n{} cases: median {:.1}x, range {:.1}x to {:.1}x",
                speedups.len(),
                speedups[speedups.len() / 2],
                speedups[0],
                speedups[speedups.len() - 1]
            );
        }
        if !disagreed.is_empty() {
            println!(
                "\n{} cases not timed, the two sides disagreeing:",
                disagreed.len()
            );
            for (name, left, right) in &disagreed {
                println!("  {name}: music21 {left} vs music21_rs {right}");
            }
        }

        if let Some(path) = json {
            let written = Written {
                music21: music21_version,
                python: python_version,
                platform: platform_name,
                results,
            };
            let text = serde_json::to_string_pretty(&written)
                .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, text)?;
            println!("\nwrote {}", path.display());
        }
        Ok(0)
    })
    .map_err(Into::into)
}

/// Puts the dependency virtualenv on `sys.path`.
///
/// pyo3 embeds the interpreter `PYO3_PYTHON` names but takes its home from the
/// *base* prefix, so a virtualenv's own `site-packages` is not on the path
/// even when that virtualenv is the interpreter asked for. music21's
/// dependencies live in one here, as `python-parity`'s harness assumes and
/// does the same for; CI installs them into the interpreter itself and has no
/// such directory, so this finds nothing there and changes nothing.
pub(crate) fn add_dependency_venv(py: Python<'_>, workspace_root: &Path) -> PyResult<()> {
    let path = py.import("sys")?.getattr("path")?.cast_into::<PyList>()?;
    for relative in [".m21venv", "venv", ".venv"] {
        let venv = workspace_root.join(relative);
        if !venv.is_dir() {
            continue;
        }
        for site in [
            venv.join("Lib/site-packages"),
            venv.join("lib/site-packages"),
        ] {
            if site.is_dir() {
                path.insert(0, site.to_string_lossy().to_string())?;
            }
        }
        if let Ok(entries) = std::fs::read_dir(venv.join("lib")) {
            for entry in entries.flatten() {
                let site = entry.path().join("site-packages");
                if site.is_dir() {
                    path.insert(0, site.to_string_lossy().to_string())?;
                }
            }
        }
    }
    Ok(())
}

/// Falls back to the pinned submodule when no music21 is installed.
///
/// The comparison should be against the music21 this repository pins, and CI
/// installs music21's dependencies without music21 itself for exactly that
/// reason. The `origin` test matters because the submodule's own directory
/// sits in the working directory, and `find_spec` answers for it as a
/// namespace package with no code in it at all.
pub(crate) fn add_submodule_fallback(py: Python<'_>, workspace_root: &Path) -> PyResult<()> {
    let found = py
        .import("importlib.util")?
        .call_method1("find_spec", ("music21",))
        .ok()
        .filter(|spec| !spec.is_none())
        .and_then(|spec| spec.getattr("origin").ok())
        .is_some_and(|origin| !origin.is_none());
    if found {
        return Ok(());
    }
    let submodule = workspace_root.join("music21");
    if submodule.join("music21/__init__.py").is_file() {
        py.import("sys")?
            .getattr("path")?
            .cast_into::<PyList>()?
            .insert(0, submodule.to_string_lossy().to_string())?;
    }
    Ok(())
}

/// music21's classes, each from the module it keeps them in.
fn music21_side(py: Python<'_>) -> PyResult<Side> {
    let mut classes = HashMap::new();
    for (name, module) in [
        ("Pitch", "music21.pitch"),
        ("Note", "music21.note"),
        ("Interval", "music21.interval"),
        ("Chord", "music21.chord"),
        ("pcToToneRow", "music21.serial"),
    ] {
        classes.insert(name, py.import(module)?.getattr(name)?.unbind());
    }
    side(py, "music21", classes)
}

/// The wheel's, all on the one module.
fn our_side(py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<Side> {
    let mut classes = HashMap::new();
    for name in ["Pitch", "Note", "Interval", "Chord", "pcToToneRow"] {
        classes.insert(name, module.getattr(name)?.unbind());
    }
    side(py, "music21_rs", classes)
}

fn side(
    py: Python<'_>,
    label: &'static str,
    classes: HashMap<&'static str, Py<PyAny>>,
) -> PyResult<Side> {
    let builtins = py.import("builtins")?;
    Ok(Side {
        label,
        classes,
        list: builtins.getattr("list")?.unbind(),
        round: builtins.getattr("round")?.unbind(),
    })
}

/// Every operation timed, written once for both sides.
fn cases() -> Vec<Case> {
    let mut out = Vec::new();

    // ---- construction ----------------------------------------------------
    out.push(
        Case::new(
            "Pitch('C#4')",
            "construction",
            attribute_of("Pitch", "C#4", "nameWithOctave"),
        )
        .natively(|| {
            RsPitch::from_name("C#4")
                .map(|pitch| pitch.name_with_octave())
                .unwrap_or_default()
        }),
    );
    out.push(
        Case::new(
            "Note('C#4')",
            "construction",
            attribute_of("Note", "C#4", "nameWithOctave"),
        )
        .natively(|| {
            RsNote::from_name("C#4")
                .map(|note| note.pitch().name_with_octave())
                .unwrap_or_default()
        }),
    );
    out.push(
        Case::new(
            "Interval('P5')",
            "construction",
            attribute_of("Interval", "P5", "directedName"),
        )
        .natively(|| {
            RsInterval::from_name("P5")
                .map(|interval| interval.directed_name())
                .unwrap_or_default()
        }),
    );
    out.push(
        Case::new(
            "Chord('C4 E4 G4')",
            "construction",
            Box::new(|py, side| {
                let class = side.get(py, "Chord")?.unbind();
                let text = interned(py, "C4 E4 G4");
                let pitches = interned(py, "pitches");
                Ok(Box::new(move |py| {
                    let chord = class.bind(py).call1((text.bind(py),))?;
                    let count = chord.getattr(pitches.bind(py))?.len()?;
                    Ok(count.into_pyobject(py)?.into_any().unbind())
                }))
            }),
        )
        .natively(|| {
            RsChord::new("C4 E4 G4")
                .map(|chord| chord.pitches().len().to_string())
                .unwrap_or_default()
        }),
    );

    // ---- chord analysis, a fresh object each time ------------------------
    for ask in [
        Ask::Attribute("commonName"),
        Ask::MethodAttribute("root", "nameWithOctave"),
        Ask::Method("inversion"),
        Ask::Attribute("forteClass"),
        Ask::AttributeList("primeForm"),
        Ask::AttributeList("intervalVector"),
        Ask::Method("isDominantSeventh"),
        Ask::AttributeList("orderedPitchClasses"),
    ] {
        out.push(
            Case::new(
                format!("Chord.{}", ask.label()),
                "chord analysis",
                Box::new(move |py, side| {
                    let class = side.get(py, "Chord")?.unbind();
                    let texts: Vec<Py<PyString>> =
                        CHORDS.iter().map(|text| interned(py, text)).collect();
                    let question = ask.prepare(py, side);
                    Ok(Box::new(move |py| {
                        let answers = PyList::empty(py);
                        for text in &texts {
                            let chord = class.bind(py).call1((text.bind(py),))?;
                            answers.append(question(py, &chord)?)?;
                        }
                        Ok(answers.into_any().unbind())
                    }))
                }),
            )
            .noting("5 chords"),
        );
        if let Some(native) = native_for(ask) {
            let last = out.len() - 1;
            out[last].native = Some(native);
        }
    }

    // The pattern a memo is for: several set-class questions of one chord,
    // each of which would otherwise repeat the same table search.
    out.push(
        Case::new(
            "Chord: forteClass + primeForm + intervalVector",
            "chord analysis",
            Box::new(|py, side| {
                let class = side.get(py, "Chord")?.unbind();
                let text = interned(py, "D3 F#3 A3 C4");
                let forte = Ask::Attribute("forteClass").prepare(py, side);
                let prime = Ask::AttributeList("primeForm").prepare(py, side);
                let vector = Ask::AttributeList("intervalVector").prepare(py, side);
                Ok(Box::new(move |py| {
                    let chord = class.bind(py).call1((text.bind(py),))?;
                    let answers = PyList::empty(py);
                    answers.append(forte(py, &chord)?)?;
                    answers.append(prime(py, &chord)?)?;
                    answers.append(vector(py, &chord)?)?;
                    Ok(answers.into_any().unbind())
                }))
            }),
        )
        .noting("one chord, three questions"),
    );

    // ---- pitch and interval work -----------------------------------------
    out.push(
        Case::new(
            "Pitch.transpose('M3')",
            "pitch",
            Box::new(|py, side| {
                let class = side.get(py, "Pitch")?.unbind();
                let text = interned(py, "C#4");
                let third = interned(py, "M3");
                let transpose = interned(py, "transpose");
                let name = interned(py, "nameWithOctave");
                Ok(Box::new(move |py| {
                    Ok(class
                        .bind(py)
                        .call1((text.bind(py),))?
                        .call_method1(transpose.bind(py), (third.bind(py),))?
                        .getattr(name.bind(py))?
                        .unbind())
                }))
            }),
        )
        .natively(|| {
            let third = RsInterval::from_name("M3").expect("a major third");
            RsPitch::from_name("C#4")
                .and_then(|pitch| pitch.transpose(&third))
                .map(|pitch| pitch.name_with_octave())
                .unwrap_or_default()
        }),
    );
    out.push(Case::new(
        "Pitch.frequency",
        "pitch",
        Box::new(|py, side| {
            let class = side.get(py, "Pitch")?.unbind();
            let text = interned(py, "A4");
            let frequency = interned(py, "frequency");
            let round = side.round.clone_ref(py);
            Ok(Box::new(move |py| {
                let value = class
                    .bind(py)
                    .call1((text.bind(py),))?
                    .getattr(frequency.bind(py))?;
                Ok(round.bind(py).call1((value, 6))?.unbind())
            }))
        }),
    ));
    out.push(
        Case::new(
            "Pitch.getEnharmonic()",
            "pitch",
            Box::new(|py, side| {
                let class = side.get(py, "Pitch")?.unbind();
                let text = interned(py, "C#4");
                let enharmonic = interned(py, "getEnharmonic");
                let name = interned(py, "nameWithOctave");
                Ok(Box::new(move |py| {
                    Ok(class
                        .bind(py)
                        .call1((text.bind(py),))?
                        .call_method0(enharmonic.bind(py))?
                        .getattr(name.bind(py))?
                        .unbind())
                }))
            }),
        )
        .natively(|| {
            RsPitch::from_name("C#4")
                .and_then(|pitch| pitch.get_enharmonic())
                .map(|pitch| pitch.name_with_octave())
                .unwrap_or_default()
        }),
    );
    out.push(
        Case::new(
            "Interval(p1, p2)",
            "pitch",
            Box::new(|py, side| {
                let pitch = side.get(py, "Pitch")?.unbind();
                let interval = side.get(py, "Interval")?.unbind();
                let low = interned(py, "C4");
                let high = interned(py, "A-5");
                let name = interned(py, "directedName");
                Ok(Box::new(move |py| {
                    let pitch = pitch.bind(py);
                    let one = pitch.call1((low.bind(py),))?;
                    let two = pitch.call1((high.bind(py),))?;
                    Ok(interval
                        .bind(py)
                        .call1((one, two))?
                        .getattr(name.bind(py))?
                        .unbind())
                }))
            }),
        )
        .natively(|| {
            let low = RsPitch::from_name("C4").expect("a C");
            let high = RsPitch::from_name("A-5").expect("an A flat");
            RsInterval::between_pitches(&low, &high)
                .map(|interval| interval.directed_name())
                .unwrap_or_default()
        }),
    );

    // ---- twelve-tone rows -------------------------------------------------
    out.push(Case::new(
        "pcToToneRow(...).matrix()",
        "serial",
        Box::new(|py, side| {
            let function = side.get(py, "pcToToneRow")?.unbind();
            let row = PyList::new(py, 0..12)?.unbind();
            let matrix = interned(py, "matrix");
            Ok(Box::new(move |py| {
                let answer = function
                    .bind(py)
                    .call1((row.bind(py),))?
                    .call_method0(matrix.bind(py))?;
                // music21's matrix prints as a block of text; only its head is
                // compared and timed, as the Python this replaces did with
                // `str(...)[:40]`. Python slices a `str` by code point.
                let head: String = answer.str()?.to_string_lossy().chars().take(40).collect();
                Ok(PyString::new(py, &head).into_any().unbind())
            }))
        }),
    ));
    out.push(Case::new(
        "ToneRow.zeroCenteredTransformation",
        "serial",
        Box::new(|py, side| {
            let function = side.get(py, "pcToToneRow")?.unbind();
            let row = PyList::new(py, 0..12)?.unbind();
            let transform = interned(py, "zeroCenteredTransformation");
            let inversion = interned(py, "I");
            let classes = interned(py, "pitchClasses");
            Ok(Box::new(move |py| {
                Ok(function
                    .bind(py)
                    .call1((row.bind(py),))?
                    .call_method1(transform.bind(py), (inversion.bind(py), 3))?
                    .call_method0(classes.bind(py))?
                    .unbind())
            }))
        }),
    ));

    // ---- the same questions on an object that has already answered them ---
    for attribute in ["commonName", "forteClass"] {
        out.push(
            Case::new(
                format!("Chord.{attribute} (cached)"),
                "warm object",
                Box::new(move |py, side| {
                    let attribute = interned(py, attribute);
                    // Built once and asked once here, so that what is timed is
                    // an object that has already answered.
                    let chord = side.get(py, "Chord")?.call1(("D3 F#3 A3 C4",))?;
                    chord.getattr(attribute.bind(py))?;
                    let chord = chord.unbind();
                    Ok(Box::new(move |py| {
                        Ok(chord.bind(py).getattr(attribute.bind(py))?.unbind())
                    }))
                }),
            )
            .noting("music21 memoizes"),
        );
    }

    out
}

/// `Class('argument').attribute`, the shape most construction cases take.
fn attribute_of(class: &'static str, argument: &'static str, attribute: &'static str) -> Build {
    Box::new(move |py, side| {
        let class = side.get(py, class)?.unbind();
        let argument = interned(py, argument);
        let attribute = interned(py, attribute);
        Ok(Box::new(move |py| {
            Ok(class
                .bind(py)
                .call1((argument.bind(py),))?
                .getattr(attribute.bind(py))?
                .unbind())
        }))
    })
}

/// A list written the way Python prints one, since a native answer is checked
/// against the text of music21's.
fn python_list(items: impl IntoIterator<Item = String>) -> String {
    let joined: Vec<String> = items.into_iter().collect();
    format!("[{}]", joined.join(", "))
}

/// A string as Python reprs one, inside a list.
fn python_str(text: &str) -> String {
    format!("'{text}'")
}

/// The same question of each of the five chords, natively.
fn over_chords(ask: impl Fn(&RsChord) -> String) -> String {
    python_list(CHORDS.iter().map(|text| match RsChord::new(*text) {
        Ok(chord) => ask(&chord),
        Err(_) => String::new(),
    }))
}

fn numbers(values: &[u8]) -> String {
    python_list(values.iter().map(u8::to_string))
}

/// What the crate answers for each of the eight chord questions, where it can
/// be compared with music21's answer as text.
fn native_for(ask: Ask) -> Option<Native> {
    Some(match ask {
        Ask::Attribute("commonName") => || over_chords(|c| python_str(&c.common_name())),
        // The facade answers `N/A` where there is no Forte class, as music21
        // does, so the native side has to as well.
        Ask::Attribute("forteClass") => {
            || over_chords(|c| python_str(&c.forte_class().unwrap_or_else(|| "N/A".to_string())))
        }
        Ask::MethodAttribute("root", "nameWithOctave") => || {
            over_chords(|c| {
                python_str(&c.root().map(RsPitch::name_with_octave).unwrap_or_default())
            })
        },
        Ask::Method("isDominantSeventh") => || {
            over_chords(|c| {
                if c.is_dominant_seventh() {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            })
        },
        Ask::AttributeList("primeForm") => || over_chords(|c| numbers(&c.prime_form())),
        Ask::AttributeList("intervalVector") => {
            || over_chords(|c| numbers(&c.interval_class_vector().unwrap_or_else(|| vec![0; 6])))
        }
        Ask::AttributeList("orderedPitchClasses") => {
            || over_chords(|c| numbers(&c.pitch_classes()))
        }
        _ => return None,
    })
}

/// A string held as a Python object for the life of a case.
///
/// Python's own loops name an attribute with a constant the compiler interned
/// once; building a fresh `str` on each call instead would be timing this
/// program rather than the two libraries.
fn interned(py: Python<'_>, text: &str) -> Py<PyString> {
    PyString::intern(py, text).unbind()
}

/// Nanoseconds per call, taking the best of `repeats` timed batches.
///
/// The best batch is used rather than the mean: the true cost is bounded
/// below, and everything above it is noise from the machine.
fn measure(py: Python<'_>, call: &Thunk, seconds: f64, repeats: usize) -> PyResult<f64> {
    measure_with(|| call(py).map(|_| ()), seconds, repeats)
}

/// The same, for the crate on its own. `black_box` so that nothing is elided
/// for having an answer nobody reads.
fn measure_native(call: Native, seconds: f64, repeats: usize) -> f64 {
    measure_with(
        || {
            std::hint::black_box(call());
            Ok(())
        },
        seconds,
        repeats,
    )
    .unwrap_or(f64::NAN)
}

fn measure_with(
    mut run: impl FnMut() -> PyResult<()>,
    seconds: f64,
    repeats: usize,
) -> PyResult<f64> {
    // Calibrate: grow the batch until one takes long enough to time well.
    let target = seconds / repeats as f64;
    let mut batch: u64 = 1;
    loop {
        let start = Instant::now();
        for _ in 0..batch {
            run()?;
        }
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed >= target || batch >= 1 << 22 {
            break;
        }
        let scaled = (batch as f64 * target / elapsed.max(1e-9)) as u64;
        batch = (batch * 2).max(scaled);
    }

    let mut best = f64::INFINITY;
    for _ in 0..repeats {
        let start = Instant::now();
        for _ in 0..batch {
            run()?;
        }
        let each = start.elapsed().as_nanos() as f64 / batch as f64;
        best = best.min(each);
    }
    Ok(best)
}

/// A duration written the way the report's table writes it.
fn humanise(nanoseconds: f64) -> String {
    if nanoseconds < 1_000.0 {
        format!("{nanoseconds:.0} ns")
    } else if nanoseconds < 1_000_000.0 {
        format!("{:.1} us", nanoseconds / 1_000.0)
    } else {
        format!("{:.2} ms", nanoseconds / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_is_named_once() {
        let cases = cases();
        assert_eq!(cases.len(), 21, "the report quotes a case count");
        let mut names: Vec<&str> = cases.iter().map(|case| case.name.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two cases share a name");
    }

    #[test]
    fn a_question_is_named_the_way_the_case_reads() {
        assert_eq!(Ask::Attribute("commonName").label(), "commonName");
        assert_eq!(Ask::AttributeList("primeForm").label(), "primeForm");
        assert_eq!(Ask::Method("inversion").label(), "inversion()");
        assert_eq!(
            Ask::MethodAttribute("root", "nameWithOctave").label(),
            "root()"
        );
    }

    #[test]
    fn durations_are_written_in_the_unit_that_fits() {
        assert_eq!(humanise(450.0), "450 ns");
        assert_eq!(humanise(1_500.0), "1.5 us");
        assert_eq!(humanise(2_700_000.0), "2.70 ms");
    }
}
