//! The harness that runs music21's own doctests against the `music21_rs`
//! Python classes.
//!
//! The classes themselves live in the `music21-rs-python` crate, which is
//! also what maturin builds into a wheel; this crate wraps them in a module
//! of its own so the doctest runner has somewhere to send its output, imports
//! the real music21 from the submodule, replaces the classes and functions of
//! one music21 module with ours, and runs that module's docstrings as they
//! are. What passes is what the crate reproduces to the letter; what fails is
//! either a fidelity gap or a feature the crate does not have.

#![forbid(unsafe_code)]

use pyo3::prelude::*;

pub mod doctest;
pub mod suite;

/// A name the crate writes with `b` for flat, as music21 writes it with
/// `-`: `Bb4` is `B-4`, `bb` (B-flat minor) `b-`. The first letter is the
/// step; every `b` after it is a flat. The fixtures hold music21's.
pub fn music21_name(name: &str) -> String {
    let mut letters = name.chars();
    let Some(step) = letters.next() else {
        return String::new();
    };
    format!("{step}{}", letters.as_str().replace('b', "-"))
}

/// A chord-symbol figure as music21 writes it: `Bbm7/Ab` is `B-m7/A-`,
/// `CaddDb` `CaddD-`. The root opens the figure, the bass follows `/`, and
/// the added and omitted notes follow `add` and `omit`; the `b`s of the
/// kind's own abbreviation, `Cm7b5`, are left.
pub fn music21_figure(figure: &str) -> String {
    let flats_after = |text: &str| -> String {
        let mut letters = text.chars();
        let Some(step) = letters.next() else {
            return String::new();
        };
        let rest = letters.as_str();
        let flats = rest.len() - rest.trim_start_matches('b').len();
        format!("{step}{}{}", "-".repeat(flats), &rest[flats..])
    };
    let (head, added) = match figure.split_once("add") {
        Some((head, added)) => (head, Some(added)),
        None => (figure, None),
    };
    let mut out = match head.split_once('/') {
        Some((body, bass)) => format!("{}/{}", flats_after(body), flats_after(bass)),
        None => flats_after(head),
    };
    if let Some(added) = added {
        let notes: Vec<String> = added
            .split(',')
            .map(|note| match note.strip_prefix("omit") {
                Some(omitted) => format!("omit{}", flats_after(omitted)),
                None => flats_after(note),
            })
            .collect();
        out.push_str("add");
        out.push_str(&notes.join(","));
    }
    out
}

/// Each name in a space-separated list as music21 writes it.
pub fn music21_names(names: &str) -> String {
    names
        .split(' ')
        .map(music21_name)
        .collect::<Vec<_>>()
        .join(" ")
}

/// One test that runs a music21 module's doctests against the crate: the
/// module's dotted name, the name its expectation file and log carry, and
/// the music21 modules to swap for the run, each with the names replaced.
/// Every `tests/doctest_*.rs` is one of these; each is its own binary, since
/// a swap lasts for the life of the interpreter.
#[macro_export]
macro_rules! doctest_suite {
    ($test:ident, $module:literal, $name:literal, [$(($python:literal, $names:expr)),* $(,)?]) => {
        #[test]
        fn $test() {
            use $crate::music21_rs_facade;
            ::pyo3::append_to_inittab!(music21_rs_facade);
            $crate::doctest::run($module, $name, &[$(($python, $names)),*]);
        }
    };
}

/// Buffer for doctest output, so a runner's report can be read back from Rust
/// instead of going to stdout.
static OUTPUT: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// A `doctest` `out` callback that collects into [`take_output`].
#[pyfunction]
fn collect_output(text: &str) {
    OUTPUT.lock().expect("doctest output buffer").push_str(text);
}

/// Everything written through [`collect_output`] since the last call.
pub fn take_output() -> String {
    std::mem::take(&mut *OUTPUT.lock().expect("doctest output buffer"))
}

/// The module the harness imports: every `music21_rs` class plus the output
/// callback the doctest runner writes through.
#[pymodule]
pub fn music21_rs_facade(m: &Bound<'_, PyModule>) -> PyResult<()> {
    music21_rs_python::register_all(m)?;
    m.add_function(wrap_pyfunction!(collect_output, m)?)?;
    Ok(())
}
